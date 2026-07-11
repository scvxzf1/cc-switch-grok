//! Grok local-route request transforms.
//!
//! Phase-1 keeps the OpenAI Chat Completions wire format and only:
//! 1. rewrites `model` via `settings.modelMappings`
//! 2. injects thinking/effort from `settings.grokChatReasoning`

use crate::provider::{CodexChatReasoningConfig, Provider};
use serde_json::{json, Value};

/// Resolve upstream model for a local-route entry model id.
pub fn map_grok_model(provider: &Provider, request_model: &str) -> String {
    let request_model = request_model.trim();
    if request_model.is_empty() {
        return request_model.to_string();
    }

    let mappings = provider
        .settings_config
        .get("modelMappings")
        .or_else(|| provider.settings_config.get("model_mappings"))
        .and_then(|v| v.as_array());

    if let Some(rows) = mappings {
        for row in rows {
            let id = row
                .get("id")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let upstream = row
                .get("upstreamModel")
                .or_else(|| row.get("upstream_model"))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty());

            if id == Some(request_model) {
                if let Some(upstream) = upstream {
                    return upstream.to_string();
                }
            }
        }

        // Fallback: default mapping row when request equals default entry id.
        if let Some(default_row) = rows
            .iter()
            .find(|row| row.get("isDefault").and_then(|v| v.as_bool()) == Some(true))
            .or_else(|| rows.first())
        {
            let default_id = default_row
                .get("id")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .unwrap_or("");
            if request_model == default_id {
                if let Some(upstream) = default_row
                    .get("upstreamModel")
                    .or_else(|| default_row.get("upstream_model"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    return upstream.to_string();
                }
            }
        }
    }

    // Direct custom without mappings: settings.model is the upstream name.
    if let Some(upstream) = provider
        .settings_config
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if request_model == provider.id || request_model == upstream {
            return upstream.to_string();
        }
    }

    request_model.to_string()
}

/// Parse grokChatReasoning from provider settings into the shared reasoning config shape.
pub fn resolve_grok_chat_reasoning(provider: &Provider) -> Option<CodexChatReasoningConfig> {
    let raw = provider
        .settings_config
        .get("grokChatReasoning")
        .or_else(|| provider.settings_config.get("grok_chat_reasoning"))
        .cloned()?;

    let mut config: CodexChatReasoningConfig = serde_json::from_value(raw).ok()?;
    if config.supports_effort.unwrap_or(false) && config.supports_thinking.is_none() {
        config.supports_thinking = Some(true);
    }
    Some(config)
}

fn default_reasoning_effort(provider: &Provider) -> Option<String> {
    provider
        .settings_config
        .get("defaultReasoningEffort")
        .or_else(|| provider.settings_config.get("default_reasoning_effort"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn map_effort_value(effort: &str, mode: Option<&str>) -> Option<&'static str> {
    let effort = effort.trim().to_ascii_lowercase();
    if matches!(effort.as_str(), "none" | "off" | "disabled") {
        return None;
    }

    match mode.unwrap_or("openai") {
        "openrouter" => match effort.as_str() {
            "low" | "minimal" => Some("low"),
            "medium" => Some("medium"),
            "high" => Some("high"),
            "xhigh" | "max" => Some("xhigh"),
            _ => Some("high"),
        },
        _ => match effort.as_str() {
            "low" | "minimal" => Some("low"),
            "medium" => Some("medium"),
            "high" => Some("high"),
            "xhigh" | "max" => Some("xhigh"),
            _ => None,
        },
    }
}

/// Inject thinking/effort fields into an OpenAI Chat Completions body.
pub fn apply_grok_reasoning(provider: &Provider, body: &mut Value) {
    let Some(config) = resolve_grok_chat_reasoning(provider) else {
        return;
    };

    let supports_effort = config.supports_effort.unwrap_or(false);
    let supports_thinking = config.supports_thinking.unwrap_or(false) || supports_effort;
    if !supports_thinking && !supports_effort {
        return;
    }

    // Prefer request-provided reasoning.effort, else provider default.
    let effort = body
        .pointer("/reasoning/effort")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            body.get("reasoning_effort")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .or_else(|| default_reasoning_effort(provider));

    let reasoning_enabled = match effort.as_deref() {
        Some(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "none" | "off" | "disabled"
        ),
        None => supports_thinking || supports_effort,
    };

    if supports_thinking {
        match config
            .thinking_param
            .as_deref()
            .unwrap_or("thinking")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "thinking" => {
                body["thinking"] = json!({
                    "type": if reasoning_enabled { "enabled" } else { "disabled" }
                });
            }
            "enable_thinking" => {
                body["enable_thinking"] = json!(reasoning_enabled);
            }
            "reasoning_split" => {
                body["reasoning_split"] = json!(reasoning_enabled);
            }
            _ => {}
        }
    }

    if !reasoning_enabled || !supports_effort {
        return;
    }

    let Some(effort) = effort.as_deref() else {
        return;
    };
    let Some(mapped) = map_effort_value(effort, config.effort_value_mode.as_deref()) else {
        return;
    };

    match config
        .effort_param
        .as_deref()
        .unwrap_or("reasoning_effort")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "reasoning_effort" => {
            body["reasoning_effort"] = json!(mapped);
        }
        "reasoning.effort" => {
            body["reasoning"] = json!({ "effort": mapped });
        }
        _ => {}
    }
}

/// Apply model rewrite + reasoning injection for Grok chat completions requests.
pub fn apply_grok_chat_request_transforms(provider: &Provider, body: &mut Value) {
    // Prefer the shared config-side transformer so takeover + proxy stay aligned.
    if let Ok(transformed) =
        crate::grok_config::transform_proxy_chat_body(body.clone(), &provider.settings_config)
    {
        *body = transformed;
        return;
    }

    if let Some(model) = body
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string)
    {
        let mapped = map_grok_model(provider, &model);
        body["model"] = Value::String(mapped);
    }
    apply_grok_reasoning(provider, body);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Provider;
    use serde_json::json;

    fn provider_with(settings: Value) -> Provider {
        let mut provider = Provider::with_id(
            "route-proxy".to_string(),
            "Route Proxy".to_string(),
            settings,
            None,
        );
        provider.category = Some("custom".to_string());
        provider
    }

    #[test]
    fn grok_proxy_rewrites_model_and_injects_reasoning_effort() {
        let provider = provider_with(json!({
            "kind": "custom",
            "base_url": "https://openrouter.ai/api/v1",
            "api_key": "sk-test",
            "useLocalRoute": true,
            "defaultReasoningEffort": "high",
            "modelMappings": [
                {
                    "id": "route-entry",
                    "upstreamModel": "anthropic/claude-sonnet-4",
                    "isDefault": true
                }
            ],
            "grokChatReasoning": {
                "supportsThinking": true,
                "supportsEffort": true,
                "thinkingParam": "thinking",
                "effortParam": "reasoning_effort",
                "effortValueMode": "openai"
            }
        }));

        let mut body = json!({
            "model": "route-entry",
            "messages": [{"role":"user","content":"hi"}]
        });

        apply_grok_chat_request_transforms(&provider, &mut body);

        assert_eq!(body["model"], "anthropic/claude-sonnet-4");
        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["reasoning_effort"], "high");
    }

    #[test]
    fn official_provider_rejects_local_route() {
        let err = crate::grok_config::apply_switch_defaults(
            "xai-official",
            &json!({
                "kind": "official",
                "useLocalRoute": true,
                "model": "grok-build"
            }),
        )
        .unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("local route")
                || msg.contains("本地路由")
                || msg.contains("Official"),
            "unexpected error: {msg}"
        );
    }
}
