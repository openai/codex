use super::*;
use crate::plugins::test_support::load_plugins_config;
use crate::plugins::test_support::write_curated_plugin_sha;
use crate::plugins::test_support::write_openai_curated_marketplace;
use crate::plugins::test_support::write_plugins_feature_config;
use codex_config::CONFIG_TOML_FILE;
use codex_config::config_toml::ConfigToml;
use codex_config::types::ToolSuggestConfig;
use codex_config::types::ToolSuggestDisabledTool;
use codex_core_plugins::PluginInstallRequest;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::startup_sync::curated_plugins_repo_path;
use codex_rmcp_client::ElicitationResponse;
use codex_tools::DiscoverablePluginInfo;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::PathExt;
use pretty_assertions::assert_eq;
use rmcp::model::ElicitationAction;
use serde_json::json;
use tempfile::tempdir;

#[tokio::test]
async fn verified_plugin_install_completed_requires_installed_plugin() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["sample"]);
    write_curated_plugin_sha(codex_home.path());
    write_plugins_feature_config(codex_home.path());

    let config = load_plugins_config(codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());

    assert!(!verified_plugin_install_completed(
        "sample@openai-curated",
        &config,
        &plugins_manager,
    ));

    plugins_manager
        .install_plugin(
            &config.config_layer_stack,
            PluginInstallRequest {
                plugin_name: "sample".to_string(),
                marketplace_path: AbsolutePathBuf::try_from(
                    curated_root.join(".agents/plugins/marketplace.json"),
                )
                .expect("marketplace path"),
            },
        )
        .await
        .expect("plugin should install");

    let refreshed_config = load_plugins_config(codex_home.path()).await;
    assert!(verified_plugin_install_completed(
        "sample@openai-curated",
        &refreshed_config,
        &plugins_manager,
    ));
}

#[test]
fn remote_plugin_install_suggestions_skip_core_installed_verification() {
    assert!(is_remote_plugin_install_suggestion(
        "snowflake@openai-curated-remote"
    ));
    assert!(!is_remote_plugin_install_suggestion(
        "snowflake@openai-curated"
    ));
    assert!(!is_remote_plugin_install_suggestion("Plugin_123"));
}

#[test]
fn plugin_install_completion_requires_base_and_claimed_extension_checks() {
    assert!(!plugin_install_completed_with_extensions(
        /*base_completed*/ false,
        Some(true)
    ));
    assert!(!plugin_install_completed_with_extensions(
        /*base_completed*/ true,
        Some(false)
    ));
    assert!(plugin_install_completed_with_extensions(
        /*base_completed*/ true,
        Some(true)
    ));
    assert!(plugin_install_completed_with_extensions(
        /*base_completed*/ true, /*extension_completed*/ None
    ));
}

#[test]
fn recommended_plugin_install_args_accept_legacy_tool_id() {
    let current: RecommendedPluginInstallArgs = serde_json::from_value(json!({
        "plugin_id": "google-drive@openai-curated-remote",
        "suggest_reason": "Use Google Drive for this request"
    }))
    .expect("current arguments should deserialize");
    let legacy: RecommendedPluginInstallArgs = serde_json::from_value(json!({
        "tool_type": "plugin",
        "action_type": "install",
        "tool_id": "google-drive@openai-curated-remote",
        "suggest_reason": "Use Google Drive for this request"
    }))
    .expect("legacy arguments should deserialize");

    assert_eq!(current, legacy);
}

#[test]
fn request_plugin_install_response_persists_only_decline_always_mode() {
    assert!(request_plugin_install_response_requests_persistent_disable(
        &ElicitationResponse {
            action: ElicitationAction::Decline,
            content: None,
            meta: Some(json!({
                REQUEST_PLUGIN_INSTALL_PERSIST_KEY: REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE
            })),
        }
    ));
    assert!(
        !request_plugin_install_response_requests_persistent_disable(&ElicitationResponse {
            action: ElicitationAction::Accept,
            content: None,
            meta: Some(json!({
                REQUEST_PLUGIN_INSTALL_PERSIST_KEY: REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE
            })),
        })
    );
    assert!(
        !request_plugin_install_response_requests_persistent_disable(&ElicitationResponse {
            action: ElicitationAction::Decline,
            content: None,
            meta: Some(json!({ REQUEST_PLUGIN_INSTALL_PERSIST_KEY: "session" })),
        })
    );
    assert!(
        !request_plugin_install_response_requests_persistent_disable(&ElicitationResponse {
            action: ElicitationAction::Decline,
            content: None,
            meta: None,
        })
    );
}

#[tokio::test]
async fn persist_disabled_install_request_writes_plugin_config() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let plugin = DiscoverablePluginInfo {
        id: "slack@openai-curated".to_string(),
        remote_plugin_id: None,
        name: "Slack".to_string(),
        description: None,
        has_skills: true,
        mcp_server_names: Vec::new(),
        ..DiscoverablePluginInfo::default()
    };

    persist_disabled_install_request(&codex_home.path().abs(), &plugin)
        .await
        .expect("persist plugin disable");

    let contents =
        std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE)).expect("read config");
    let parsed: ConfigToml = toml::from_str(&contents).expect("parse config");
    assert_eq!(
        parsed.tool_suggest,
        Some(ToolSuggestConfig {
            discoverables: Vec::new(),
            disabled_tools: vec![ToolSuggestDisabledTool::plugin("slack@openai-curated")],
        })
    );
}
