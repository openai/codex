use super::*;
use crate::plugins::PluginInstallRequest;
use crate::plugins::PluginsManager;
use crate::plugins::test_support::load_plugins_config;
use crate::plugins::test_support::write_curated_plugin_sha;
use crate::plugins::test_support::write_openai_curated_marketplace;
use crate::plugins::test_support::write_plugins_feature_config;
use codex_utils_absolute_path::AbsolutePathBuf;
use tempfile::tempdir;

#[test]
fn invalid_suggest_reason_message_rejects_placeholder_reasons() {
    for reason in [
        "",
        "   ",
        "placeholder",
        " Placeholder ",
        "<placeholder>",
        "[TODO]",
        "TBD",
        "n/a",
        "reason",
        "suggestion reason",
        "reason goes here",
        "Concise one-line user-facing reason why this tool can help with the current request",
        "Specific one-line user-facing reason why this tool can help with the current request",
    ] {
        assert!(
            invalid_suggest_reason_message(reason).is_some(),
            "expected {reason:?} to be rejected"
        );
    }
}

#[test]
fn invalid_suggest_reason_message_allows_specific_reasons() {
    for reason in [
        "Search Slack messages related to the release plan.",
        "Find and reference issues in GitHub for this bug report.",
        "Use Google Calendar to compare attendee availability.",
        "Use Figma to inspect placeholder text in the selected design.",
    ] {
        assert_eq!(
            invalid_suggest_reason_message(reason),
            None,
            "expected {reason:?} to be allowed"
        );
    }
}

#[tokio::test]
async fn verified_plugin_suggestion_completed_requires_installed_plugin() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = crate::plugins::curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["sample"]);
    write_curated_plugin_sha(codex_home.path());
    write_plugins_feature_config(codex_home.path());

    let config = load_plugins_config(codex_home.path()).await;
    let plugins_manager = PluginsManager::new(codex_home.path().to_path_buf());

    assert!(!verified_plugin_suggestion_completed(
        "sample@openai-curated",
        &config,
        &plugins_manager,
    ));

    plugins_manager
        .install_plugin(PluginInstallRequest {
            plugin_name: "sample".to_string(),
            marketplace_path: AbsolutePathBuf::try_from(
                curated_root.join(".agents/plugins/marketplace.json"),
            )
            .expect("marketplace path"),
        })
        .await
        .expect("plugin should install");

    let refreshed_config = load_plugins_config(codex_home.path()).await;
    assert!(verified_plugin_suggestion_completed(
        "sample@openai-curated",
        &refreshed_config,
        &plugins_manager,
    ));
}
