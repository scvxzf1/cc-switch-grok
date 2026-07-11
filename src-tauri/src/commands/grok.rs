use tauri::State;

use crate::grok_config::{self, GrokAuthStatus};
use crate::store::AppState;

/// Import providers from Grok live config (`~/.grok/config.toml` model tables).
#[tauri::command]
pub fn import_grok_providers_from_live(state: State<'_, AppState>) -> Result<usize, String> {
    crate::services::provider::import_grok_providers_from_live(state.inner())
        .map_err(|e| e.to_string())
}

/// Get model table ids present in Grok live config.
#[tauri::command]
pub fn get_grok_live_provider_ids() -> Result<Vec<String>, String> {
    grok_config::get_models()
        .map(|models| models.keys().cloned().collect())
        .map_err(|e| e.to_string())
}

/// Get a single Grok model table fragment from live config.
#[tauri::command]
pub fn get_grok_live_provider(
    #[allow(non_snake_case)] providerId: String,
) -> Result<Option<serde_json::Value>, String> {
    let models = grok_config::get_models().map_err(|e| e.to_string())?;
    Ok(models.get(&providerId).cloned())
}

/// Read-only Grok auth status from `~/.grok/auth.json` (no tokens).
#[tauri::command]
pub fn get_grok_auth_status() -> Result<GrokAuthStatus, String> {
    grok_config::read_auth_status().map_err(|e| e.to_string())
}
