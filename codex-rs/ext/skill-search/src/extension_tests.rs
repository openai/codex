use codex_extension_api::PromptSlot;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolName;
use pretty_assertions::assert_eq;

use super::*;
use crate::tool::SKILL_SEARCH_TOOL_NAME;

#[tokio::test]
async fn prompt_contribution_is_gated_by_feature_config() {
    let extension = SkillSearchExtension;
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");

    assert!(
        extension
            .contribute(&session_store, &thread_store)
            .await
            .is_empty()
    );

    thread_store.insert(SkillSearchExtensionConfig { enabled: true });
    let fragments = extension.contribute(&session_store, &thread_store).await;

    assert_eq!(fragments.len(), 1);
    assert_eq!(fragments[0].slot(), PromptSlot::DeveloperPolicy);
    assert!(fragments[0].text().contains(SKILL_SEARCH_TOOL_NAME));
}

#[test]
fn tool_contribution_is_gated_by_feature_config() {
    let extension = SkillSearchExtension;
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let turn_store = ExtensionData::new("turn");

    assert!(
        extension
            .tools(&session_store, &thread_store, &turn_store)
            .is_empty()
    );

    thread_store.insert(SkillSearchExtensionConfig { enabled: true });
    let tool_names = extension
        .tools(&session_store, &thread_store, &turn_store)
        .into_iter()
        .map(|tool| tool.tool_name())
        .collect::<Vec<_>>();

    assert_eq!(tool_names, vec![ToolName::plain(SKILL_SEARCH_TOOL_NAME)]);
}
