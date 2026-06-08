use anyhow::Context;
use anyhow::Result;
use codex_app_server_protocol::AnalyticsConfig;
use codex_app_server_protocol::AppConfig;
use codex_app_server_protocol::AppToolApproval;
use codex_app_server_protocol::AppToolConfig;
use codex_app_server_protocol::AppToolsConfig;
use codex_app_server_protocol::AppsConfig;
use codex_app_server_protocol::AppsDefaultConfig;
use codex_app_server_protocol::Config as ApiConfig;
use codex_app_server_protocol::ForcedChatgptWorkspaceIds as ApiForcedChatgptWorkspaceIds;
use codex_app_server_protocol::SandboxWorkspaceWrite as ApiSandboxWorkspaceWrite;
use codex_app_server_protocol::ToolsV2;
use codex_config::config_toml::ConfigToml;
use codex_config::config_toml::ForcedChatgptWorkspaceIds;
use codex_config::types;
use serde_json::Map as JsonMap;
use serde_json::Number as JsonNumber;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use toml::Value as TomlValue;

const TYPED_CONFIG_KEYS: &[&str] = &[
    "model",
    "review_model",
    "model_context_window",
    "model_auto_compact_token_limit",
    "model_auto_compact_token_limit_scope",
    "model_provider",
    "approval_policy",
    "approvals_reviewer",
    "sandbox_mode",
    "sandbox_workspace_write",
    "forced_chatgpt_workspace_id",
    "forced_login_method",
    "web_search",
    "tools",
    "instructions",
    "developer_instructions",
    "compact_prompt",
    "model_reasoning_effort",
    "model_reasoning_summary",
    "model_verbosity",
    "service_tier",
    "analytics",
    "apps",
    "desktop",
];

pub(crate) fn config_toml_to_api(config: ConfigToml, effective: &TomlValue) -> Result<ApiConfig> {
    Ok(ApiConfig {
        model: config.model,
        review_model: config.review_model,
        model_context_window: config.model_context_window,
        model_auto_compact_token_limit: config.model_auto_compact_token_limit,
        model_auto_compact_token_limit_scope: config.model_auto_compact_token_limit_scope,
        model_provider: config.model_provider,
        approval_policy: config.approval_policy.map(Into::into),
        approvals_reviewer: config.approvals_reviewer.map(Into::into),
        sandbox_mode: config.sandbox_mode.map(Into::into),
        sandbox_workspace_write: config
            .sandbox_workspace_write
            .map(sandbox_workspace_write_to_api),
        forced_chatgpt_workspace_id: config
            .forced_chatgpt_workspace_id
            .map(forced_workspace_ids_to_api),
        forced_login_method: config.forced_login_method,
        web_search: config.web_search,
        tools: config.tools.map(|tools| ToolsV2 {
            web_search: tools.web_search,
        }),
        instructions: config.instructions,
        developer_instructions: config.developer_instructions,
        compact_prompt: config.compact_prompt,
        model_reasoning_effort: config.model_reasoning_effort,
        model_reasoning_summary: config.model_reasoning_summary,
        model_verbosity: config.model_verbosity,
        service_tier: config.service_tier,
        analytics: config.analytics.map(|analytics| AnalyticsConfig {
            enabled: analytics.enabled,
            additional: HashMap::new(),
        }),
        apps: config.apps.map(apps_to_api),
        desktop: config.desktop,
        additional: additional_config(effective)?,
    })
}

fn sandbox_workspace_write_to_api(
    sandbox: types::SandboxWorkspaceWrite,
) -> ApiSandboxWorkspaceWrite {
    ApiSandboxWorkspaceWrite {
        writable_roots: sandbox.writable_roots.into_iter().map(Into::into).collect(),
        network_access: sandbox.network_access,
        exclude_tmpdir_env_var: sandbox.exclude_tmpdir_env_var,
        exclude_slash_tmp: sandbox.exclude_slash_tmp,
    }
}

fn forced_workspace_ids_to_api(
    workspace_ids: ForcedChatgptWorkspaceIds,
) -> ApiForcedChatgptWorkspaceIds {
    match workspace_ids {
        ForcedChatgptWorkspaceIds::Single(workspace_id) => {
            ApiForcedChatgptWorkspaceIds::Single(workspace_id)
        }
        ForcedChatgptWorkspaceIds::Multiple(workspace_ids) => {
            ApiForcedChatgptWorkspaceIds::Multiple(workspace_ids)
        }
    }
}

fn apps_to_api(apps: types::AppsConfigToml) -> AppsConfig {
    AppsConfig {
        default: apps.default.map(|default| AppsDefaultConfig {
            enabled: default.enabled,
            destructive_enabled: default.destructive_enabled,
            open_world_enabled: default.open_world_enabled,
        }),
        apps: apps
            .apps
            .into_iter()
            .map(|(name, config)| (name, app_to_api(config)))
            .collect(),
    }
}

fn app_to_api(app: types::AppConfig) -> AppConfig {
    AppConfig {
        enabled: app.enabled,
        approvals_reviewer: app.approvals_reviewer.map(Into::into),
        destructive_enabled: app.destructive_enabled,
        open_world_enabled: app.open_world_enabled,
        default_tools_approval_mode: app
            .default_tools_approval_mode
            .map(app_tool_approval_to_api),
        default_tools_enabled: app.default_tools_enabled,
        tools: app.tools.map(|tools| AppToolsConfig {
            tools: tools
                .tools
                .into_iter()
                .map(|(name, config)| {
                    (
                        name,
                        AppToolConfig {
                            enabled: config.enabled,
                            approval_mode: config.approval_mode.map(app_tool_approval_to_api),
                        },
                    )
                })
                .collect(),
        }),
    }
}

fn app_tool_approval_to_api(approval: types::AppToolApproval) -> AppToolApproval {
    match approval {
        types::AppToolApproval::Auto => AppToolApproval::Auto,
        types::AppToolApproval::Prompt => AppToolApproval::Prompt,
        types::AppToolApproval::Approve => AppToolApproval::Approve,
    }
}

fn additional_config(effective: &TomlValue) -> Result<HashMap<String, JsonValue>> {
    let TomlValue::Table(table) = effective else {
        return Ok(HashMap::new());
    };

    table
        .iter()
        .filter(|(key, _)| !TYPED_CONFIG_KEYS.contains(&key.as_str()))
        .map(|(key, value)| Ok((key.clone(), toml_value_to_json(value)?)))
        .collect()
}

fn toml_value_to_json(value: &TomlValue) -> Result<JsonValue> {
    match value {
        TomlValue::String(value) => Ok(JsonValue::String(value.clone())),
        TomlValue::Integer(value) => Ok(JsonValue::Number((*value).into())),
        TomlValue::Float(value) => JsonNumber::from_f64(*value)
            .map(JsonValue::Number)
            .context("configuration contains a non-finite float"),
        TomlValue::Boolean(value) => Ok(JsonValue::Bool(*value)),
        TomlValue::Datetime(value) => Ok(JsonValue::String(value.to_string())),
        TomlValue::Array(values) => values
            .iter()
            .map(toml_value_to_json)
            .collect::<Result<Vec<_>>>()
            .map(JsonValue::Array),
        TomlValue::Table(table) => table
            .iter()
            .map(|(key, value)| Ok((key.clone(), toml_value_to_json(value)?)))
            .collect::<Result<JsonMap<_, _>>>()
            .map(JsonValue::Object),
    }
}

#[cfg(test)]
#[path = "config_api_tests.rs"]
mod tests;
