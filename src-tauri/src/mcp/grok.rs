//! Grok MCP sync and import module
//!
//! Handles conversion between CC Switch unified MCP format and Grok
//! `~/.grok/config.toml` `[mcp_servers.<name>]` sections.
//!
//! ## Format mapping
//!
//! | CC Switch unified (JSON) | Grok config.toml |
//! |---|---|
//! | `{"type":"stdio","command":"...","args":[...],"env":{}}` | `command` / `args` / `env` |
//! | `{"type":"sse"|"http","url":"...","headers":{}}` | `url` / `headers` |
//!
//! Grok does not require a `type` field; transport is inferred from
//! `command` vs `url`. Optional Grok fields (`enabled`, timeouts) are
//! preserved on merge-on-write.

use serde_json::{json, Map, Value};
use std::collections::HashMap;

use crate::app_config::{McpApps, McpServer, MultiAppConfig};
use crate::error::AppError;
use crate::grok_config;

use super::validation::validate_server_spec;

/// Grok-specific fields preserved on merge-on-write / stripped on import.
const GROK_EXTRA_FIELDS: &[&str] = &[
    "enabled",
    "startup_timeout_sec",
    "tool_timeout_sec",
    "tool_timeouts",
];

fn should_sync_grok_mcp() -> bool {
    grok_config::get_grok_dir().exists()
}

/// Convert CC Switch unified JSON to Grok mcp_servers entry (JSON intermediate).
fn convert_to_grok_format(spec: &Value) -> Result<Value, AppError> {
    let obj = spec
        .as_object()
        .ok_or_else(|| AppError::McpValidation("MCP spec must be a JSON object".into()))?;

    let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("stdio");
    let mut result = Map::new();

    match typ {
        "stdio" => {
            if let Some(command) = obj.get("command") {
                result.insert("command".into(), command.clone());
            }
            if let Some(args) = obj.get("args") {
                if args.is_array() && !args.as_array().map(|a| a.is_empty()).unwrap_or(true) {
                    result.insert("args".into(), args.clone());
                }
            }
            if let Some(env) = obj.get("env") {
                if env.is_object() && !env.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                    result.insert("env".into(), env.clone());
                }
            }
            if let Some(cwd) = obj.get("cwd").and_then(|v| v.as_str()) {
                if !cwd.trim().is_empty() {
                    result.insert("cwd".into(), json!(cwd));
                }
            }
        }
        "sse" | "http" => {
            if let Some(url) = obj.get("url") {
                result.insert("url".into(), url.clone());
            }
            // Accept either headers or Codex-style http_headers
            let headers = obj
                .get("headers")
                .or_else(|| obj.get("http_headers"));
            if let Some(headers) = headers {
                if headers.is_object()
                    && !headers.as_object().map(|o| o.is_empty()).unwrap_or(true)
                {
                    result.insert("headers".into(), headers.clone());
                }
            }
        }
        other => {
            return Err(AppError::McpValidation(format!(
                "Unknown MCP type for Grok: {other}"
            )));
        }
    }

    // Grok defaults to enabled when the section exists.
    result.insert("enabled".into(), json!(true));

    // Carry through optional Grok timeout fields if the unified spec already has them.
    for key in ["startup_timeout_sec", "tool_timeout_sec", "tool_timeouts"] {
        if let Some(v) = obj.get(key) {
            result.insert(key.into(), v.clone());
        }
    }

    Ok(Value::Object(result))
}

/// Convert a Grok mcp_servers entry back to CC Switch unified format.
fn convert_from_grok_format(id: &str, spec: &Value) -> Result<Value, AppError> {
    let obj = spec.as_object().ok_or_else(|| {
        AppError::McpValidation(format!("Grok MCP server '{id}' must be a table/object"))
    })?;

    let mut result = Map::new();

    if obj.contains_key("command") {
        result.insert("type".into(), json!("stdio"));
        if let Some(command) = obj.get("command") {
            result.insert("command".into(), command.clone());
        }
        if let Some(args) = obj.get("args") {
            if args.is_array() && !args.as_array().map(|a| a.is_empty()).unwrap_or(true) {
                result.insert("args".into(), args.clone());
            }
        }
        if let Some(env) = obj.get("env") {
            if env.is_object() && !env.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                result.insert("env".into(), env.clone());
            }
        }
        if let Some(cwd) = obj.get("cwd").and_then(|v| v.as_str()) {
            if !cwd.trim().is_empty() {
                result.insert("cwd".into(), json!(cwd));
            }
        }
    } else if obj.contains_key("url") {
        // Prefer http for generic remote; SSE and streamable HTTP share url/headers.
        result.insert("type".into(), json!("http"));
        if let Some(url) = obj.get("url") {
            result.insert("url".into(), url.clone());
        }
        if let Some(headers) = obj.get("headers") {
            if headers.is_object()
                && !headers.as_object().map(|o| o.is_empty()).unwrap_or(true)
            {
                result.insert("headers".into(), headers.clone());
            }
        }
    } else {
        return Err(AppError::McpValidation(format!(
            "Grok MCP server '{id}' has neither 'command' nor 'url'"
        )));
    }

    Ok(Value::Object(result))
}

fn merge_grok_spec(existing: &Value, incoming: &Value) -> Value {
    let mut out = incoming.as_object().cloned().unwrap_or_default();
    if let Some(existing_obj) = existing.as_object() {
        for key in GROK_EXTRA_FIELDS {
            if !out.contains_key(*key) {
                if let Some(v) = existing_obj.get(*key) {
                    out.insert((*key).to_string(), v.clone());
                }
            }
        }
    }
    // Ensure enabled stays true when CC Switch is projecting the server.
    out.insert("enabled".into(), json!(true));
    Value::Object(out)
}

fn json_to_toml_table(spec: &Value) -> Result<toml_edit::Table, AppError> {
    use toml_edit::{Array, Item, Table};

    let obj = spec
        .as_object()
        .ok_or_else(|| AppError::McpValidation("Grok MCP entry must be an object".into()))?;

    let mut t = Table::new();

    if let Some(command) = obj.get("command").and_then(|v| v.as_str()) {
        t["command"] = toml_edit::value(command);
    }
    if let Some(args) = obj.get("args").and_then(|v| v.as_array()) {
        let mut arr = Array::default();
        for a in args.iter().filter_map(|x| x.as_str()) {
            arr.push(a);
        }
        if !arr.is_empty() {
            t["args"] = Item::Value(toml_edit::Value::Array(arr));
        }
    }
    if let Some(cwd) = obj.get("cwd").and_then(|v| v.as_str()) {
        if !cwd.trim().is_empty() {
            t["cwd"] = toml_edit::value(cwd);
        }
    }
    if let Some(env) = obj.get("env").and_then(|v| v.as_object()) {
        let mut env_tbl = Table::new();
        for (k, v) in env.iter() {
            if let Some(s) = v.as_str() {
                env_tbl[&k[..]] = toml_edit::value(s);
            }
        }
        if !env_tbl.is_empty() {
            t["env"] = Item::Table(env_tbl);
        }
    }
    if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
        t["url"] = toml_edit::value(url);
    }
    if let Some(headers) = obj.get("headers").and_then(|v| v.as_object()) {
        let mut h_tbl = Table::new();
        for (k, v) in headers.iter() {
            if let Some(s) = v.as_str() {
                h_tbl[&k[..]] = toml_edit::value(s);
            }
        }
        if !h_tbl.is_empty() {
            t["headers"] = Item::Table(h_tbl);
        }
    }
    if let Some(enabled) = obj.get("enabled").and_then(|v| v.as_bool()) {
        t["enabled"] = toml_edit::value(enabled);
    }
    if let Some(n) = obj.get("startup_timeout_sec").and_then(|v| v.as_i64()) {
        t["startup_timeout_sec"] = toml_edit::value(n);
    }
    if let Some(n) = obj.get("tool_timeout_sec").and_then(|v| v.as_i64()) {
        t["tool_timeout_sec"] = toml_edit::value(n);
    }
    if let Some(tool_timeouts) = obj.get("tool_timeouts").and_then(|v| v.as_object()) {
        let mut tt = Table::new();
        for (k, v) in tool_timeouts.iter() {
            if let Some(n) = v.as_i64() {
                tt[&k[..]] = toml_edit::value(n);
            }
        }
        if !tt.is_empty() {
            t["tool_timeouts"] = Item::Table(tt);
        }
    }

    Ok(t)
}

fn toml_table_to_json(table: &toml_edit::Table) -> Value {
    let mut map = Map::new();
    for (key, item) in table.iter() {
        if let Some(v) = toml_item_to_json(item) {
            map.insert(key.to_string(), v);
        }
    }
    Value::Object(map)
}

fn toml_item_to_json(item: &toml_edit::Item) -> Option<Value> {
    match item {
        toml_edit::Item::Value(v) => toml_value_to_json(v),
        toml_edit::Item::Table(t) => Some(toml_table_to_json(t)),
        toml_edit::Item::None => None,
        _ => None,
    }
}

fn toml_value_to_json(v: &toml_edit::Value) -> Option<Value> {
    use toml_edit::Value as Tv;
    match v {
        Tv::String(s) => Some(json!(s.value())),
        Tv::Integer(i) => Some(json!(*i.value())),
        Tv::Float(f) => Some(json!(*f.value())),
        Tv::Boolean(b) => Some(json!(*b.value())),
        Tv::Array(arr) => {
            let items: Vec<Value> = arr.iter().filter_map(toml_value_to_json).collect();
            Some(Value::Array(items))
        }
        Tv::InlineTable(t) => {
            let mut map = Map::new();
            for (k, vv) in t.iter() {
                if let Some(jv) = toml_value_to_json(vv) {
                    map.insert(k.to_string(), jv);
                }
            }
            Some(Value::Object(map))
        }
        _ => None,
    }
}

/// Sync one MCP server into `~/.grok/config.toml` `[mcp_servers.<id>]`.
pub fn sync_single_server_to_grok(
    _config: &MultiAppConfig,
    id: &str,
    server_spec: &Value,
) -> Result<(), AppError> {
    if !should_sync_grok_mcp() {
        return Ok(());
    }
    validate_server_spec(server_spec)?;

    let grok_spec = convert_to_grok_format(server_spec)?;
    let id_owned = id.to_string();
    let mut doc = grok_config::read_config_document()?;

    if doc.get("mcp_servers").is_none() {
        doc["mcp_servers"] = toml_edit::table();
    }
    let mcp_root = doc["mcp_servers"].as_table_mut().ok_or_else(|| {
        AppError::Config("Grok config.toml has non-table `mcp_servers` key".into())
    })?;

    let merged = if let Some(existing_item) = mcp_root.get(&id_owned) {
        if let Some(existing_tbl) = existing_item.as_table() {
            let existing_json = toml_table_to_json(existing_tbl);
            merge_grok_spec(&existing_json, &grok_spec)
        } else {
            grok_spec
        }
    } else {
        grok_spec
    };

    let table = json_to_toml_table(&merged)?;
    mcp_root.insert(&id_owned, toml_edit::Item::Table(table));

    grok_config::write_config_document(&doc)?;
    log::debug!("Grok MCP server '{id}' written to live config");
    Ok(())
}

/// Remove one MCP server from Grok live config.
pub fn remove_server_from_grok(id: &str) -> Result<(), AppError> {
    if !should_sync_grok_mcp() {
        return Ok(());
    }
    let path = grok_config::get_grok_config_path();
    if !path.exists() {
        return Ok(());
    }

    let mut doc = grok_config::read_config_document()?;
    if let Some(mcp_root) = doc.get_mut("mcp_servers").and_then(|i| i.as_table_mut()) {
        mcp_root.remove(id);
        if mcp_root.is_empty() {
            doc.as_table_mut().remove("mcp_servers");
        }
        grok_config::write_config_document(&doc)?;
        log::debug!("Grok MCP server '{id}' removed from live config");
    }
    Ok(())
}

/// Import MCP servers from `~/.grok/config.toml` into unified structure.
///
/// Existing servers only flip `apps.grok = true`; other fields are preserved.
pub fn import_from_grok(config: &mut MultiAppConfig) -> Result<usize, AppError> {
    if !should_sync_grok_mcp() {
        return Ok(0);
    }
    let path = grok_config::get_grok_config_path();
    if !path.exists() {
        return Ok(0);
    }

    let doc = grok_config::read_config_document()?;
    let Some(mcp_root) = doc.get("mcp_servers").and_then(|i| i.as_table()) else {
        return Ok(0);
    };

    let servers = config.mcp.servers.get_or_insert_with(HashMap::new);
    let mut changed = 0usize;

    for (id, item) in mcp_root.iter() {
        let Some(tbl) = item.as_table() else {
            continue;
        };
        // Skip disabled entries when importing.
        if let Some(enabled) = tbl
            .get("enabled")
            .and_then(|i| i.as_value())
            .and_then(|v| v.as_bool())
        {
            if !enabled {
                continue;
            }
        }

        let raw = toml_table_to_json(tbl);
        let spec = match convert_from_grok_format(id, &raw) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Skip invalid Grok MCP '{id}': {e}");
                continue;
            }
        };
        if let Err(e) = validate_server_spec(&spec) {
            log::warn!("Skip invalid Grok MCP '{id}': {e}");
            continue;
        }

        if let Some(existing) = servers.get_mut(id) {
            if !existing.apps.grok {
                existing.apps.grok = true;
                changed += 1;
            }
        } else {
            servers.insert(
                id.to_string(),
                McpServer {
                    id: id.to_string(),
                    name: id.to_string(),
                    server: spec,
                    apps: McpApps {
                        grok: true,
                        ..McpApps::default()
                    },
                    description: None,
                    homepage: None,
                    docs: None,
                    tags: vec![],
                },
            );
            changed += 1;
        }
    }

    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn converts_stdio_and_http() {
        let stdio = convert_to_grok_format(&json!({
            "type": "stdio",
            "command": "npx",
            "args": ["-y", "mcp-server"],
            "env": {"TOKEN": "x"}
        }))
        .unwrap();
        assert_eq!(stdio["command"], "npx");
        assert_eq!(stdio["enabled"], true);

        let http = convert_to_grok_format(&json!({
            "type": "http",
            "url": "https://example.com/mcp",
            "headers": {"Auth": "t"}
        }))
        .unwrap();
        assert_eq!(http["url"], "https://example.com/mcp");
        assert!(http.get("headers").is_some());
    }

    #[test]
    fn roundtrip_stdio_import() {
        let raw = json!({
            "command": "npx",
            "args": ["-y", "foo"],
            "enabled": true
        });
        let unified = convert_from_grok_format("foo", &raw).unwrap();
        assert_eq!(unified["type"], "stdio");
        assert_eq!(unified["command"], "npx");
    }
}
