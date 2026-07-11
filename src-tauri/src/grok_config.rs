//! Grok 配置文件读写模块
//!
//! 处理 `~/.grok/config.toml` 的安全读写（TOML，保留注释与顺序）。
//! 第一期只提供路径解析与 document 级读写；provider live 操作在后续任务实现。

use crate::config::{atomic_write, get_app_config_dir, get_home_dir};
use crate::error::AppError;
use crate::settings::{effective_backup_retain_count, get_grok_override_dir};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use toml_edit::DocumentMut;

/// Grok 写入结果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GrokWriteOutcome {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<String>,
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

            let mut settings = crate::settings::get_settings();
            settings.grok_config_dir = None;
            crate::settings::update_settings(settings).unwrap();

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
            let mut settings = crate::settings::get_settings();
            settings.grok_config_dir = None;
            crate::settings::update_settings(settings).unwrap();

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
            doc["models"]["default"] = toml_edit::value("proxy-1");
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

            let mut settings = crate::settings::get_settings();
            settings.grok_config_dir = None;
            crate::settings::update_settings(settings).unwrap();
        });
    }
}
