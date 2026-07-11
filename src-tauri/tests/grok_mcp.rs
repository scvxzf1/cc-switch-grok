//! Grok MCP live projection tests
use std::fs;

use cc_switch_lib::{
    remove_server_from_grok, sync_single_server_to_grok, AppType, McpApps, McpServer, McpService,
};
use serde_json::json;

#[path = "support.rs"]
mod support;
use support::{create_test_state, ensure_test_home, reset_test_fs, test_mutex};

#[test]
fn sync_stdio_mcp_to_grok_config() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();
    let grok_dir = home.join(".grok");
    fs::create_dir_all(&grok_dir).expect("create grok dir");
    let cfg = grok_dir.join("config.toml");
    fs::write(&cfg, "[ui]\ncompact_mode = true\n").expect("seed config");

    sync_single_server_to_grok(
        &Default::default(),
        "github",
        &json!({
            "type": "stdio",
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-github"],
            "env": {"GITHUB_PERSONAL_ACCESS_TOKEN": "ghp_test"}
        }),
    )
    .expect("sync stdio mcp");

    let text = fs::read_to_string(&cfg).expect("read config");
    assert!(
        text.contains("command = \"npx\""),
        "expected command field, got:\n{text}"
    );
    assert!(
        text.contains("@modelcontextprotocol/server-github"),
        "expected package arg, got:\n{text}"
    );
    assert!(
        text.contains("compact_mode = true"),
        "must preserve unrelated sections, got:\n{text}"
    );
    assert!(
        text.contains("mcp_servers"),
        "expected mcp_servers section, got:\n{text}"
    );

    remove_server_from_grok("github").expect("remove");
    let text = fs::read_to_string(&cfg).expect("read after remove");
    assert!(
        !text.contains("server-github"),
        "mcp entry should be gone, got:\n{text}"
    );
    assert!(
        text.contains("compact_mode = true"),
        "preserve after remove, got:\n{text}"
    );
}

#[test]
fn toggle_mcp_app_enables_and_syncs_to_grok() {
    let _guard = test_mutex().lock().expect("acquire test mutex");
    reset_test_fs();
    let home = ensure_test_home();
    let grok_dir = home.join(".grok");
    fs::create_dir_all(&grok_dir).expect("create grok dir");
    fs::write(grok_dir.join("config.toml"), "").expect("seed empty config");

    let state = create_test_state().expect("state");
    let server = McpServer {
        id: "echo-mcp".into(),
        name: "Echo".into(),
        server: json!({
            "type": "stdio",
            "command": "echo",
            "args": ["hello"]
        }),
        apps: McpApps {
            grok: false,
            ..Default::default()
        },
        description: None,
        homepage: None,
        docs: None,
        tags: vec![],
    };
    McpService::upsert_server(&state, server).expect("upsert");

    McpService::toggle_app(&state, "echo-mcp", AppType::Grok, true).expect("enable grok");

    let text = fs::read_to_string(grok_dir.join("config.toml")).expect("read live");
    assert!(
        text.contains("command = \"echo\""),
        "live should contain projected mcp, got:\n{text}"
    );

    let servers = McpService::get_all_servers(&state).expect("list");
    assert!(
        servers.get("echo-mcp").unwrap().apps.grok,
        "db flag should be true"
    );

    McpService::toggle_app(&state, "echo-mcp", AppType::Grok, false).expect("disable");
    let text = fs::read_to_string(grok_dir.join("config.toml")).expect("read after disable");
    assert!(
        !text.contains("command = \"echo\""),
        "should remove on disable, got:\n{text}"
    );
}
