use codex_features::Feature;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use pretty_assertions::assert_eq;

use super::*;

#[test]
fn recognizes_only_agent_hook_subagent_source() {
    assert!(is_agent_hook_source(&SessionSource::SubAgent(
        SubAgentSource::Other("agent_hook".to_string())
    )));
    assert!(!is_agent_hook_source(&SessionSource::SubAgent(
        SubAgentSource::Other("other".to_string())
    )));
    assert!(!is_agent_hook_source(&SessionSource::Cli));
}

#[tokio::test]
async fn agent_hook_config_is_isolated_and_inherits_active_permissions() {
    let (_, turn) = crate::session::tests::make_session_and_context().await;
    let mut parent_config = (*turn.config).clone();
    for feature in [Feature::CodexHooks, Feature::Apps, Feature::Goals] {
        parent_config
            .features
            .enable(feature)
            .expect("test config should allow feature update");
    }

    let config =
        build_agent_hook_config(&parent_config, &turn, "gpt-hook").expect("agent hook config");

    assert_eq!(config.model.as_deref(), Some("gpt-hook"));
    assert_eq!(
        config.base_instructions.as_deref(),
        Some(AGENT_HOOK_BASE_INSTRUCTIONS)
    );
    assert_eq!(config.user_instructions, None);
    assert_eq!(config.developer_instructions, None);
    assert_eq!(config.compact_prompt, None);
    assert_eq!(config.guardian_policy_config, None);
    assert_eq!(config.personality, None);
    assert_eq!(config.project_doc_max_bytes, 0);
    assert!(!config.include_permissions_instructions);
    assert!(!config.include_apps_instructions);
    assert!(!config.include_collaboration_mode_instructions);
    assert!(!config.include_skill_instructions);
    assert!(!config.include_environment_context);
    assert!(!config.experimental_request_user_input_enabled);
    assert_eq!(config.notify, None);
    assert!(config.ephemeral);
    assert_eq!(
        config.permissions.approval_policy.value(),
        AskForApproval::Never
    );
    assert_eq!(
        config.permissions.permission_profile(),
        &turn.permission_profile()
    );
    assert!(config.mcp_servers.get().is_empty());
    assert_eq!(*config.web_search_mode.get(), WebSearchMode::Disabled);
    for feature in [
        Feature::CodexHooks,
        Feature::MemoryTool,
        Feature::ChildAgentsMd,
        Feature::Collab,
        Feature::MultiAgentV2,
        Feature::Plugins,
        Feature::BrowserUse,
        Feature::ComputerUse,
        Feature::WebSearchRequest,
        Feature::ImageGeneration,
        Feature::RequestPermissionsTool,
        Feature::Goals,
    ] {
        assert!(!config.features.enabled(feature), "{feature:?}");
    }
}
