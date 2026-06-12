use std::sync::Arc;

use codex_core_skills::SkillPolicy;
use codex_extension_api::ConversationHistory;
use codex_extension_api::NoopTurnItemEmitter;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::protocol::SkillScope;
use codex_protocol::protocol::TruncationPolicy;
use codex_tools::ToolPayload;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

fn skill(name: &str, description: &str, path: &str) -> SkillMetadata {
    SkillMetadata {
        name: name.to_string(),
        description: description.to_string(),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: Some(SkillPolicy {
            allow_implicit_invocation: Some(true),
            products: Vec::new(),
        }),
        path_to_skills_md: test_path_buf(path).abs(),
        scope: SkillScope::User,
        plugin_id: None,
    }
}

fn call(arguments: serde_json::Value) -> ToolCall {
    ToolCall {
        turn_id: "turn-skill-search".to_string(),
        call_id: "call-skill-search".to_string(),
        tool_name: ToolName::plain(SKILL_SEARCH_TOOL_NAME),
        model: "gpt-test".to_string(),
        truncation_policy: TruncationPolicy::Bytes(1024),
        conversation_history: ConversationHistory::default(),
        turn_item_emitter: Arc::new(NoopTurnItemEmitter),
        environments: Vec::new(),
        payload: ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    }
}

async fn output_text(tool: &SkillSearchTool, arguments: serde_json::Value) -> String {
    let output = tool
        .handle(call(arguments))
        .await
        .expect("skill_search should return output");
    let response = output.to_response_item(
        "call-skill-search",
        &ToolPayload::Function {
            arguments: "{}".to_string(),
        },
    );
    let ResponseInputItem::FunctionCallOutput { output, .. } = response else {
        panic!("expected function output");
    };
    let FunctionCallOutputBody::Text(text) = output.body else {
        panic!("expected text output");
    };
    text
}

#[tokio::test]
async fn search_returns_plain_text_skill_lines() {
    let tool = SkillSearchTool::new(vec![
        skill("slides", "Build presentation decks", "/tmp/slides/SKILL.md"),
        skill("sheets", "Analyze spreadsheet data", "/tmp/sheets/SKILL.md"),
    ]);
    let expected_path = test_path_buf("/tmp/slides/SKILL.md")
        .abs()
        .to_string_lossy()
        .replace('\\', "/");

    assert_eq!(
        output_text(&tool, json!({ "query": "presentation deck" })).await,
        format!("- slides: Build presentation decks (file: {expected_path})")
    );
}

#[tokio::test]
async fn search_returns_empty_text_when_no_skill_matches() {
    let tool = SkillSearchTool::new(vec![skill(
        "slides",
        "Build presentation decks",
        "/tmp/slides/SKILL.md",
    )]);

    assert_eq!(output_text(&tool, json!({ "query": "quantum" })).await, "");
}

#[tokio::test]
async fn search_defaults_to_eight_and_caps_requested_limit_at_sixteen() {
    let skills = (0..20)
        .map(|index| {
            skill(
                &format!("common-{index}"),
                "Common workflow",
                &format!("/tmp/common-{index}/SKILL.md"),
            )
        })
        .collect();
    let tool = SkillSearchTool::new(skills);

    let default_output = output_text(&tool, json!({ "query": "common workflow" })).await;
    let capped_output =
        output_text(&tool, json!({ "query": "common workflow", "limit": 100 })).await;

    assert_eq!(default_output.lines().count(), DEFAULT_SKILL_SEARCH_LIMIT);
    assert_eq!(capped_output.lines().count(), MAX_SKILL_SEARCH_LIMIT);
}

#[tokio::test]
async fn search_rejects_empty_query_and_zero_limit() {
    let tool = SkillSearchTool::new(Vec::new());

    let empty_query = tool
        .handle(call(json!({ "query": "   " })))
        .await
        .err()
        .expect("empty query should fail");
    let zero_limit = tool
        .handle(call(json!({ "query": "docs", "limit": 0 })))
        .await
        .err()
        .expect("zero limit should fail");

    assert_eq!(empty_query.to_string(), "query must not be empty");
    assert_eq!(zero_limit.to_string(), "limit must be greater than zero");
}
