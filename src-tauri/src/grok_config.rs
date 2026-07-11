//! Grok 配置文件读写模块
//!
//! 处理 `~/.grok/config.toml` 的安全读写（TOML，保留注释与顺序）。
//! 提供 document 级读写与 provider live 操作（model upsert/remove/switch/auth）。

use crate::config::{atomic_write, get_app_config_dir, get_home_dir};
use crate::error::AppError;
use crate::settings::{effective_backup_retain_count, get_grok_override_dir};
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use toml_edit::{value, DocumentMut, Item, Table};

/// Grok 写入结果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GrokWriteOutcome {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<String>,
}

/// 官方登录态（只读摘要，不含 token）
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GrokAuthStatus {
    pub logged_in: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub expired: bool,
}

fn grok_write_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// 获取 Grok 配置目录
///
/// 默认路径: `~/.grok/`
/// 可通过 settings.grok_config_dir 覆盖
pub fn get_grok_dir() -> PathBuf {
    if let Some(override_dir) = get_grok_override_dir() {
        return override_dir;
    }
    get_home_dir().join(".grok")
}

/// 获取 Grok 配置文件路径：`~/.grok/config.toml`
pub fn get_grok_config_path() -> PathBuf {
    get_grok_dir().join("config.toml")
}

/// 获取 Grok 登录态路径：`~/.grok/auth.json`
pub fn get_grok_auth_path() -> PathBuf {
    get_grok_dir().join("auth.json")
}

/// 读取 Grok config.toml 为可编辑 document。
///
/// 文件不存在时返回空 document，便于后续 upsert。
pub fn read_config_document() -> Result<DocumentMut, AppError> {
    let path = get_grok_config_path();
    if !path.exists() {
        return Ok(DocumentMut::new());
    }

    let content = fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    if content.trim().is_empty() {
        return Ok(DocumentMut::new());
    }

    content.parse::<DocumentMut>().map_err(|e| {
        AppError::Config(format!(
            "Failed to parse Grok config.toml at {}: {e}",
            path.display()
        ))
    })
}

/// 备份并原子写入 Grok config.toml。
///
/// 调用方应通过 `toml_edit::DocumentMut` 只改目标键，以保留注释与无关段。
pub fn write_config_document(doc: &DocumentMut) -> Result<GrokWriteOutcome, AppError> {
    let _guard = grok_write_lock()
        .lock()
        .map_err(|_| AppError::Config("Grok config write lock poisoned".into()))?;
    write_config_document_locked(doc)
}

fn write_config_document_locked(doc: &DocumentMut) -> Result<GrokWriteOutcome, AppError> {
    let path = get_grok_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
    }

    let backup_path = if path.exists() {
        let raw = fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
        if raw.trim().is_empty() {
            None
        } else {
            Some(create_grok_backup(&raw)?)
        }
    } else {
        None
    };

    let serialized = doc.to_string();
    atomic_write(&path, serialized.as_bytes())?;

    Ok(GrokWriteOutcome {
        backup_path: backup_path.map(|p| p.display().to_string()),
    })
}

fn create_grok_backup(source: &str) -> Result<PathBuf, AppError> {
    let backup_dir = get_app_config_dir().join("backups").join("grok");
    fs::create_dir_all(&backup_dir).map_err(|e| AppError::io(&backup_dir, e))?;

    let base_id = format!("grok_{}", Local::now().format("%Y%m%d_%H%M%S"));
    let mut filename = format!("{base_id}.toml");
    let mut backup_path = backup_dir.join(&filename);
    let mut counter = 1u32;
    while backup_path.exists() {
        filename = format!("{base_id}_{counter}.toml");
        backup_path = backup_dir.join(&filename);
        counter += 1;
    }

    atomic_write(&backup_path, source.as_bytes())?;
    cleanup_grok_backups(&backup_dir)?;
    Ok(backup_path)
}

fn cleanup_grok_backups(dir: &Path) -> Result<(), AppError> {
    let retain = effective_backup_retain_count();
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| AppError::io(dir, e))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .map(|name| {
                    name.ends_with(".toml")
                        && (name.starts_with("grok_") || name.starts_with("config-"))
                })
                .unwrap_or(false)
        })
        .collect();

    entries.sort_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok());
    let remove_count = entries.len().saturating_sub(retain);
    for entry in entries.into_iter().take(remove_count) {
        if let Err(err) = fs::remove_file(entry.path()) {
            log::warn!(
                "Failed to remove old Grok config backup {}: {err}",
                entry.path().display()
            );
        }
    }

    Ok(())
}

// ============================================================================
// Live provider helpers
// ============================================================================

fn item_to_json(item: &Item) -> Value {
    match item {
        Item::Value(v) => value_to_json(v),
        Item::Table(t) => table_to_json(t),
        Item::ArrayOfTables(arr) => Value::Array(
            arr.iter()
                .map(|t| Value::Object(table_to_json_map(t)))
                .collect(),
        ),
        Item::None => Value::Null,
    }
}

fn value_to_json(v: &toml_edit::Value) -> Value {
    match v {
        toml_edit::Value::String(s) => Value::String(s.value().clone()),
        toml_edit::Value::Integer(i) => Value::Number((*i.value()).into()),
        toml_edit::Value::Float(f) => serde_json::Number::from_f64(*f.value())
            .map(Value::Number)
            .unwrap_or(Value::Null),
        toml_edit::Value::Boolean(b) => Value::Bool(*b.value()),
        toml_edit::Value::Datetime(d) => Value::String(d.value().to_string()),
        toml_edit::Value::Array(arr) => Value::Array(arr.iter().map(value_to_json).collect()),
        toml_edit::Value::InlineTable(t) => {
            let mut map = Map::new();
            for (k, v) in t.iter() {
                map.insert(k.to_string(), value_to_json(v));
            }
            Value::Object(map)
        }
    }
}

fn table_to_json_map(table: &Table) -> Map<String, Value> {
    let mut map = Map::new();
    for (k, item) in table.iter() {
        map.insert(k.to_string(), item_to_json(item));
    }
    map
}

fn table_to_json(table: &Table) -> Value {
    Value::Object(table_to_json_map(table))
}

fn json_to_toml_value(v: &Value) -> Option<toml_edit::Value> {
    match v {
        Value::Null => None,
        Value::Bool(b) => Some((*b).into()),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(i.into())
            } else if let Some(u) = n.as_u64() {
                Some((u as i64).into())
            } else if let Some(f) = n.as_f64() {
                Some(f.into())
            } else {
                None
            }
        }
        Value::String(s) => Some(s.clone().into()),
        Value::Array(arr) => {
            let mut out = toml_edit::Array::new();
            for item in arr {
                if let Some(tv) = json_to_toml_value(item) {
                    out.push(tv);
                }
            }
            Some(toml_edit::Value::Array(out))
        }
        Value::Object(obj) => {
            let mut table = toml_edit::InlineTable::new();
            for (k, v) in obj {
                if let Some(tv) = json_to_toml_value(v) {
                    table.insert(k, tv);
                }
            }
            Some(toml_edit::Value::InlineTable(table))
        }
    }
}

fn set_table_string(table: &mut Table, key: &str, raw: &str) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        table.remove(key);
    } else {
        table.insert(key, value(trimmed));
    }
}

fn set_table_optional_number(table: &mut Table, key: &str, number: Option<i64>) {
    match number {
        Some(n) if n > 0 => {
            table.insert(key, value(n));
        }
        _ => {
            table.remove(key);
        }
    }
}

fn extract_context_window(settings: &Value) -> Option<i64> {
    if let Some(n) = settings.get("context_window").and_then(|v| v.as_i64()) {
        return Some(n);
    }
    if let Some(n) = settings.get("contextWindow").and_then(|v| v.as_i64()) {
        return Some(n);
    }
    // fallback: default mapping row
    settings
        .get("modelMappings")
        .and_then(|v| v.as_array())
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("isDefault").and_then(|v| v.as_bool()) == Some(true))
                .or_else(|| rows.first())
        })
        .and_then(|row| {
            row.get("contextWindow")
                .or_else(|| row.get("context_window"))
                .and_then(|v| v.as_i64())
        })
}

/// Resolve the Grok model table id / `[models].default` value.
/// Custom providers never use the upstream `settings.model` string as the table id.
fn default_model_id_from_settings(provider_id: &str, settings: &Value) -> String {
    if settings
        .get("kind")
        .and_then(|v| v.as_str())
        .map(|s| s.eq_ignore_ascii_case("official"))
        .unwrap_or(false)
    {
        return settings
            .get("model")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("grok-build")
            .to_string();
    }

    if let Some(id) = settings
        .get("modelMappings")
        .and_then(|v| v.as_array())
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("isDefault").and_then(|v| v.as_bool()) == Some(true))
                .or_else(|| rows.first())
                .and_then(|row| row.get("id").and_then(|v| v.as_str()))
        })
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return id.to_string();
    }

    // Direct custom tables are keyed by provider id (or mapping id above).
    // Upstream model names stay in the table's `model` field only.
    provider_id.trim().to_string()
}

fn apply_model_settings_to_table(table: &mut Table, settings: &Value) {
    // Keep unknown keys that already exist; only set known managed keys.
    if let Some(model) = settings.get("model").and_then(|v| v.as_str()) {
        set_table_string(table, "model", model);
    }
    if let Some(base_url) = settings
        .get("base_url")
        .or_else(|| settings.get("baseUrl"))
        .and_then(|v| v.as_str())
    {
        set_table_string(table, "base_url", base_url);
    }
    if let Some(api_key) = settings
        .get("api_key")
        .or_else(|| settings.get("apiKey"))
        .and_then(|v| v.as_str())
    {
        // Empty api_key: do not write key
        if api_key.trim().is_empty() {
            table.remove("api_key");
        } else {
            table.insert("api_key", value(api_key.trim()));
        }
    }
    if let Some(api_backend) = settings
        .get("api_backend")
        .or_else(|| settings.get("apiBackend"))
        .and_then(|v| v.as_str())
    {
        set_table_string(table, "api_backend", api_backend);
    }
    if let Some(name) = settings.get("name").and_then(|v| v.as_str()) {
        set_table_string(table, "name", name);
    }
    if let Some(description) = settings.get("description").and_then(|v| v.as_str()) {
        set_table_string(table, "description", description);
    }
    // Only touch context_window when the caller actually provided a value
    // (or a mapping default row carries one). Partial updates must not wipe it.
    if let Some(window) = extract_context_window(settings) {
        set_table_optional_number(table, "context_window", Some(window));
    }

    // Optional numeric sampling fields if present
    for (src, dest) in [
        ("temperature", "temperature"),
        ("top_p", "top_p"),
        ("max_completion_tokens", "max_completion_tokens"),
    ] {
        if let Some(n) = settings.get(src).and_then(|v| v.as_f64()) {
            if let Some(tv) = json_to_toml_value(&Value::from(n)) {
                table.insert(dest, Item::Value(tv));
            }
        }
    }

    if let Some(headers) = settings
        .get("extra_headers")
        .or_else(|| settings.get("extraHeaders"))
    {
        if let Some(tv) = json_to_toml_value(headers) {
            table.insert("extra_headers", Item::Value(tv));
        }
    }
}

/// 读取 config.toml 中所有 `[model.<id>]` 段。
pub fn get_models() -> Result<Map<String, Value>, AppError> {
    let doc = read_config_document()?;
    let mut out = Map::new();

    if let Some(model_item) = doc.get("model") {
        match model_item {
            Item::Table(table) => {
                // Either dotted [model.x] groups under table "model", or a single table.
                // In toml_edit, [model.foo] becomes doc["model"]["foo"] as tables.
                for (key, item) in table.iter() {
                    if let Item::Table(model_table) = item {
                        out.insert(key.to_string(), table_to_json(model_table));
                    } else {
                        // scalar fields under [model] itself — ignore for provider map
                    }
                }
            }
            _ => {}
        }
    }

    Ok(out)
}

fn upsert_model_into_doc(
    doc: &mut DocumentMut,
    id: &str,
    settings: &Value,
) -> Result<(), AppError> {
    if doc.get("model").is_none() {
        doc["model"] = Item::Table(Table::new());
    }
    let model_root = doc["model"].as_table_mut().ok_or_else(|| {
        AppError::Config("Grok config.toml has non-table `model` key".into())
    })?;

    if model_root.get(id).and_then(|i| i.as_table()).is_none() {
        model_root.insert(id, Item::Table(Table::new()));
    }
    let table = model_root
        .get_mut(id)
        .and_then(|i| i.as_table_mut())
        .ok_or_else(|| AppError::Config(format!("Failed to open [model.{id}] table")))?;

    apply_model_settings_to_table(table, settings);
    Ok(())
}

fn set_default_model_into_doc(doc: &mut DocumentMut, id: &str) -> Result<(), AppError> {
    if doc.get("models").is_none() {
        doc["models"] = Item::Table(Table::new());
    }
    let models = doc["models"].as_table_mut().ok_or_else(|| {
        AppError::Config("Grok config.toml has non-table `models` key".into())
    })?;
    models.insert("default", value(id));
    Ok(())
}

/// Upsert `[model.<id>]` using known settings fields.
pub fn set_model(id: &str, settings: Value) -> Result<GrokWriteOutcome, AppError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(AppError::Config("Grok model id cannot be empty".into()));
    }

    let _guard = grok_write_lock()
        .lock()
        .map_err(|_| AppError::Config("Grok config write lock poisoned".into()))?;

    let mut doc = read_config_document()?;
    upsert_model_into_doc(&mut doc, id, &settings)?;
    write_config_document_locked(&doc)
}

/// 删除 `[model.<id>]` 段。
pub fn remove_model(id: &str) -> Result<GrokWriteOutcome, AppError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(AppError::Config("Grok model id cannot be empty".into()));
    }

    let _guard = grok_write_lock()
        .lock()
        .map_err(|_| AppError::Config("Grok config write lock poisoned".into()))?;

    let mut doc = read_config_document()?;
    if let Some(model_root) = doc.get_mut("model").and_then(|i| i.as_table_mut()) {
        model_root.remove(id);
        if model_root.is_empty() {
            doc.as_table_mut().remove("model");
        }
    }
    write_config_document_locked(&doc)
}

/// 设置 `[models].default`。
pub fn set_default_model(id: &str) -> Result<GrokWriteOutcome, AppError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(AppError::Config("Grok default model id cannot be empty".into()));
    }

    let _guard = grok_write_lock()
        .lock()
        .map_err(|_| AppError::Config("Grok config write lock poisoned".into()))?;

    let mut doc = read_config_document()?;
    set_default_model_into_doc(&mut doc, id)?;
    write_config_document_locked(&doc)
}

/// 切换供应商时写默认模型，并在非本地路由时 upsert model 段。
///
/// `useLocalRoute=true` 时只准备 default（及调用方可先写入的元数据），
/// 不在此任务写 proxy base_url。
pub fn apply_switch_defaults(
    provider_id: &str,
    settings: &Value,
) -> Result<GrokWriteOutcome, AppError> {
    let provider_id = provider_id.trim();
    if provider_id.is_empty() {
        return Err(AppError::Config("Grok provider id cannot be empty".into()));
    }

    let kind = settings
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("custom");
    let use_local_route = settings
        .get("useLocalRoute")
        .or_else(|| settings.get("use_local_route"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if kind.eq_ignore_ascii_case("official") && use_local_route {
        return Err(AppError::Config(
            "Official Grok provider cannot enable local route".into(),
        ));
    }

    let default_id = default_model_id_from_settings(provider_id, settings);

    let _guard = grok_write_lock()
        .lock()
        .map_err(|_| AppError::Config("Grok config write lock poisoned".into()))?;

    let mut doc = read_config_document()?;

    // For custom direct mode, upsert model table first.
    if !kind.eq_ignore_ascii_case("official") && !use_local_route {
        let mut model_settings = settings.clone();
        if model_settings.get("model").and_then(|v| v.as_str()).is_none() {
            if let Some(obj) = model_settings.as_object_mut() {
                obj.insert("model".into(), Value::String(default_id.clone()));
            }
        }
        upsert_model_into_doc(&mut doc, &default_id, &model_settings)?;
    }

    set_default_model_into_doc(&mut doc, &default_id)?;
    write_config_document_locked(&doc)
}

/// 只读解析 `auth.json`，返回脱敏登录摘要。
pub fn read_auth_status() -> Result<GrokAuthStatus, AppError> {
    let path = get_grok_auth_path();
    if !path.exists() {
        return Ok(GrokAuthStatus {
            logged_in: false,
            email: None,
            expired: false,
        });
    }

    let raw = fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    if raw.trim().is_empty() {
        return Ok(GrokAuthStatus {
            logged_in: false,
            email: None,
            expired: false,
        });
    }

    let parsed: Value = serde_json::from_str(&raw).map_err(|e| {
        AppError::Config(format!(
            "Failed to parse Grok auth.json at {}: {e}",
            path.display()
        ))
    })?;

    // auth.json is a map of account entries.
    let mut best_email: Option<String> = None;
    let mut any_entry = false;
    let mut any_valid = false;
    let mut all_expired = true;

    if let Some(obj) = parsed.as_object() {
        for (_key, entry) in obj {
            any_entry = true;
            let email = entry
                .get("email")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            if best_email.is_none() {
                best_email = email;
            }

            let expired = entry
                .get("expires_at")
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc) <= Utc::now())
                .unwrap_or(false);

            if !expired {
                any_valid = true;
                all_expired = false;
            }
        }
    }

    if !any_entry {
        return Ok(GrokAuthStatus {
            logged_in: false,
            email: None,
            expired: false,
        });
    }

    Ok(GrokAuthStatus {
        logged_in: any_valid || !all_expired,
        email: best_email,
        expired: all_expired,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use std::sync::{Mutex, OnceLock};

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    fn with_test_home<T>(test_fn: impl FnOnce() -> T) -> T {
        let _guard = test_guard();
        let tmp = tempfile::tempdir().unwrap();
        let old_test_home = std::env::var_os("CC_SWITCH_TEST_HOME");
        std::env::set_var("CC_SWITCH_TEST_HOME", tmp.path());

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(test_fn));

        match old_test_home {
            Some(value) => std::env::set_var("CC_SWITCH_TEST_HOME", value),
            None => std::env::remove_var("CC_SWITCH_TEST_HOME"),
        }

        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn reset_grok_override() {
        let mut settings = crate::settings::get_settings();
        settings.grok_config_dir = None;
        crate::settings::update_settings(settings).unwrap();
    }

    #[test]
    #[serial]
    fn get_grok_dir_uses_settings_override() {
        with_test_home(|| {
            let custom = tempfile::tempdir().unwrap();
            let custom_path = custom.path().to_path_buf();

            let mut settings = crate::settings::get_settings();
            settings.grok_config_dir = Some(custom_path.to_string_lossy().to_string());
            crate::settings::update_settings(settings).unwrap();

            let dir = get_grok_dir();
            let config_path = get_grok_config_path();
            let auth_path = get_grok_auth_path();

            reset_grok_override();

            assert_eq!(
                dir, custom_path,
                "settings.grok_config_dir should override the default ~/.grok path"
            );
            assert_eq!(config_path, custom_path.join("config.toml"));
            assert_eq!(auth_path, custom_path.join("auth.json"));
        });
    }

    #[test]
    #[serial]
    fn write_default_model_preserves_unrelated_sections() {
        with_test_home(|| {
            reset_grok_override();

            let config_path = get_grok_config_path();
            if let Some(parent) = config_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }

            let seed = r#"# keep me
[models]
default = "grok-build"

[ui]
yolo = true

[marketplace]
enabled = false
"#;
            fs::write(&config_path, seed).unwrap();

            let mut doc = read_config_document().unwrap();
            doc["models"]["default"] = value("proxy-1");
            let outcome = write_config_document(&doc).unwrap();
            assert!(
                outcome.backup_path.is_some(),
                "writing over an existing config should create a backup"
            );

            let written = fs::read_to_string(&config_path).unwrap();
            assert!(
                written.contains("# keep me"),
                "comments should be preserved by toml_edit rewrite"
            );
            assert!(
                written.contains("proxy-1"),
                "default model should be updated, got:\n{written}"
            );
            assert!(
                written.contains("[ui]") && written.contains("yolo = true"),
                "[ui] section must remain intact, got:\n{written}"
            );
            assert!(
                written.contains("[marketplace]") && written.contains("enabled = false"),
                "[marketplace] section must remain intact, got:\n{written}"
            );
        });
    }

    #[test]
    #[serial]
    fn set_model_writes_model_table_and_keeps_ui() {
        with_test_home(|| {
            reset_grok_override();
            let config_path = get_grok_config_path();
            fs::create_dir_all(config_path.parent().unwrap()).unwrap();
            fs::write(
                &config_path,
                r#"[ui]
yolo = true

[marketplace]
enabled = false
"#,
            )
            .unwrap();

            set_model(
                "my-proxy",
                serde_json::json!({
                    "model": "gpt-5.5",
                    "base_url": "https://api.example.com/v1",
                    "api_key": "sk-test",
                    "api_backend": "chat_completions",
                    "name": "My Proxy",
                    "context_window": 128000
                }),
            )
            .unwrap();

            let written = fs::read_to_string(&config_path).unwrap();
            assert!(written.contains("[model.my-proxy]"), "got:\n{written}");
            assert!(written.contains("gpt-5.5"), "got:\n{written}");
            assert!(written.contains("https://api.example.com/v1"), "got:\n{written}");
            assert!(written.contains("sk-test"), "got:\n{written}");
            assert!(written.contains("chat_completions"), "got:\n{written}");
            assert!(written.contains("128000"), "got:\n{written}");
            assert!(written.contains("[ui]") && written.contains("yolo = true"), "got:\n{written}");
            assert!(
                written.contains("[marketplace]") && written.contains("enabled = false"),
                "got:\n{written}"
            );

            // empty api_key should not write key
            set_model(
                "my-proxy",
                serde_json::json!({
                    "model": "gpt-5.5",
                    "base_url": "https://api.example.com/v1",
                    "api_key": "",
                    "api_backend": "chat_completions"
                }),
            )
            .unwrap();
            let written = fs::read_to_string(&config_path).unwrap();
            assert!(
                !written.contains("api_key"),
                "empty api_key must not be written, got:\n{written}"
            );

            let models = get_models().unwrap();
            assert!(models.contains_key("my-proxy"));
            assert_eq!(
                models["my-proxy"]["model"].as_str().unwrap(),
                "gpt-5.5"
            );
        });
    }

    #[test]
    #[serial]
    fn apply_switch_defaults_sets_models_default() {
        with_test_home(|| {
            reset_grok_override();
            let config_path = get_grok_config_path();
            fs::create_dir_all(config_path.parent().unwrap()).unwrap();
            fs::write(
                &config_path,
                r#"[ui]
compact_mode = true
"#,
            )
            .unwrap();

            // official
            apply_switch_defaults(
                "xai-official",
                &serde_json::json!({
                    "kind": "official",
                    "model": "grok-build",
                    "useLocalRoute": false
                }),
            )
            .unwrap();
            let written = fs::read_to_string(&config_path).unwrap();
            assert!(written.contains("[models]"), "got:\n{written}");
            assert!(written.contains("default = \"grok-build\""), "got:\n{written}");
            assert!(written.contains("compact_mode = true"), "got:\n{written}");

            // custom direct
            apply_switch_defaults(
                "openrouter",
                &serde_json::json!({
                    "kind": "custom",
                    "model": "anthropic/claude-sonnet",
                    "base_url": "https://openrouter.ai/api/v1",
                    "api_key": "sk-or",
                    "api_backend": "chat_completions",
                    "useLocalRoute": false,
                    "modelMappings": [
                        {
                            "id": "openrouter",
                            "upstreamModel": "anthropic/claude-sonnet",
                            "contextWindow": 200000,
                            "isDefault": true
                        }
                    ]
                }),
            )
            .unwrap();
            let written = fs::read_to_string(&config_path).unwrap();
            assert!(written.contains("[model.openrouter]"), "got:\n{written}");
            assert!(written.contains("default = \"openrouter\""), "got:\n{written}");
            assert!(written.contains("compact_mode = true"), "got:\n{written}");

            // custom direct without modelMappings: default + table share provider_id
            apply_switch_defaults(
                "proxy-a",
                &serde_json::json!({
                    "kind": "custom",
                    "model": "anthropic/claude-sonnet",
                    "base_url": "https://proxy.example/v1",
                    "api_key": "sk-proxy",
                    "api_backend": "chat_completions",
                    "useLocalRoute": false
                }),
            )
            .unwrap();
            let written = fs::read_to_string(&config_path).unwrap();
            assert!(
                written.contains("[model.proxy-a]"),
                "expected table keyed by provider id, got:\n{written}"
            );
            assert!(
                written.contains("default = \"proxy-a\""),
                "default must match table id, not upstream model name, got:\n{written}"
            );
            assert!(
                written.contains("model = \"anthropic/claude-sonnet\""),
                "upstream model stays in table field, got:\n{written}"
            );
            assert!(
                !written.contains("default = \"anthropic/claude-sonnet\""),
                "must not use upstream model as default id, got:\n{written}"
            );

            // local route only sets default, no forced proxy rewrite here
            apply_switch_defaults(
                "route-proxy",
                &serde_json::json!({
                    "kind": "custom",
                    "useLocalRoute": true,
                    "model": "upstream-model",
                    "modelMappings": [
                        {"id": "route-entry", "upstreamModel": "upstream-model", "isDefault": true}
                    ]
                }),
            )
            .unwrap();
            let written = fs::read_to_string(&config_path).unwrap();
            assert!(written.contains("default = \"route-entry\""), "got:\n{written}");
            assert!(
                !written.contains("[model.route-proxy]"),
                "local-route switch should not upsert direct model table in task2, got:\n{written}"
            );

            // official + local route rejected
            let err = apply_switch_defaults(
                "xai-official",
                &serde_json::json!({"kind":"official","useLocalRoute":true,"model":"grok-build"}),
            )
            .unwrap_err();
            let msg = format!("{err}");
            assert!(
                msg.to_lowercase().contains("local route")
                    || msg.contains("本地路由")
                    || msg.contains("Official"),
                "unexpected error: {msg}"
            );
        });
    }

    #[test]
    #[serial]
    fn remove_model_deletes_only_target_table() {
        with_test_home(|| {
            reset_grok_override();
            let config_path = get_grok_config_path();
            fs::create_dir_all(config_path.parent().unwrap()).unwrap();
            fs::write(
                &config_path,
                r#"[ui]
yolo = true

[model.keep]
model = "a"

[model.drop]
model = "b"
"#,
            )
            .unwrap();

            remove_model("drop").unwrap();
            let written = fs::read_to_string(&config_path).unwrap();
            assert!(written.contains("[model.keep]"), "got:\n{written}");
            assert!(!written.contains("[model.drop]"), "got:\n{written}");
            assert!(written.contains("[ui]") && written.contains("yolo = true"), "got:\n{written}");
        });
    }

    #[test]
    #[serial]
    fn set_model_partial_update_preserves_context_window() {
        with_test_home(|| {
            reset_grok_override();
            let config_path = get_grok_config_path();
            fs::create_dir_all(config_path.parent().unwrap()).unwrap();
            fs::write(
                &config_path,
                r#"[model.keep]
model = "a"
context_window = 64000
base_url = "https://old.example/v1"
"#,
            )
            .unwrap();

            set_model(
                "keep",
                serde_json::json!({
                    "model": "b",
                    "base_url": "https://new.example/v1",
                    "api_key": "sk-new"
                }),
            )
            .unwrap();

            let written = fs::read_to_string(&config_path).unwrap();
            assert!(
                written.contains("context_window = 64000"),
                "partial update must keep existing context_window, got:\n{written}"
            );
            assert!(written.contains("model = \"b\""), "got:\n{written}");
            assert!(
                written.contains("https://new.example/v1"),
                "got:\n{written}"
            );
        });
    }

    #[test]
    #[serial]
    fn read_auth_status_redacts_to_email_and_expiry() {
        with_test_home(|| {
            reset_grok_override();
            let auth_path = get_grok_auth_path();
            fs::create_dir_all(auth_path.parent().unwrap()).unwrap();

            // missing => not logged in
            let status = read_auth_status().unwrap();
            assert!(!status.logged_in);
            assert!(status.email.is_none());

            // valid future expiry
            fs::write(
                &auth_path,
                r#"{
  "https://auth.x.ai::client": {
    "email": "user@example.com",
    "refresh_token": "secret-should-not-surface",
    "expires_at": "2099-01-01T00:00:00Z"
  }
}"#,
            )
            .unwrap();
            let status = read_auth_status().unwrap();
            assert!(status.logged_in);
            assert_eq!(status.email.as_deref(), Some("user@example.com"));
            assert!(!status.expired);
            let debug = format!("{status:?}");
            assert!(!debug.contains("secret-should-not-surface"));

            // expired
            fs::write(
                &auth_path,
                r#"{
  "https://auth.x.ai::client": {
    "email": "old@example.com",
    "expires_at": "2000-01-01T00:00:00Z"
  }
}"#,
            )
            .unwrap();
            let status = read_auth_status().unwrap();
            assert_eq!(status.email.as_deref(), Some("old@example.com"));
            assert!(status.expired);
            assert!(!status.logged_in);
        });
    }
}
