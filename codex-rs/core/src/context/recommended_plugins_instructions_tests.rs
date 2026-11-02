#![cfg(test)]

//! Checks final recommendation bounds without truncating runtime identities.

use super::*;
use codex_protocol_test_support::ContentItemTestExt;
use codex_tools::DiscoverablePluginInfo;
use pretty_assertions::assert_eq;

#[test]
fn recommendation_wording_keeps_runtime_identities_within_the_derived_final_bound() {
    // Endpoint recommendations validate names at 64 chars. Display names permit four-byte
    // characters; config IDs use the ASCII slug and the curated marketplace suffix.
    let name = "🦀".repeat(64);
    let id = format!("{}@openai-curated-remote", "a".repeat(64));
    let plugins = vec![
        DiscoverableTool::Plugin(Box::new(DiscoverablePluginInfo {
            id: id.clone(),
            remote_plugin_id: None,
            name: name.clone(),
            description: None,
            has_skills: true,
            mcp_server_names: vec![],
            app_connector_ids: vec![],
        }));
        51
    ];
    let messages = codex_prompts::ResolvedPluginRecommendationMessages {
        intro: "é".repeat(250),
    };
    let fragment = RecommendedPluginsInstructions::from_plugins(&plugins, &messages).unwrap();
    let rendered = fragment.render().expect_text();
    let rows = vec![format!("- {name} ({id})"); 50].join("\n");
    assert_eq!(
        rendered,
        format!(
            "<recommended_plugins>\n{}\n\n{rows}\n</recommended_plugins>",
            messages.intro
        )
    );
    assert_eq!(rendered.len(), 17_946);
    assert_eq!(fragment.role(), "developer");
    assert_eq!(
        fragment.content_kind(),
        ContentItemKind("plugins.recommendations".into())
    );
    assert!(RecommendedPluginsInstructions::from_plugins(&[], &messages).is_none());
}
