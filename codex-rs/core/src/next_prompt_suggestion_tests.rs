use super::HistorySnapshot;
use super::filter_next_prompt_suggestion;
use super::has_unpaired_tool_flow;
use super::history_matches_snapshot;
use super::suggestion_prompt_has_headroom;
use crate::context_manager::ContextManager;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_utils_output_truncation::TruncationPolicy;
use pretty_assertions::assert_eq;

#[test]
fn filter_keeps_specific_prompt() {
    assert_eq!(
        filter_next_prompt_suggestion("run the tests"),
        Some("run the tests".to_string())
    );
}

#[test]
fn filter_keeps_allowed_single_word_prompt() {
    assert_eq!(
        filter_next_prompt_suggestion("commit"),
        Some("commit".to_string())
    );
}

#[test]
fn filter_keeps_code_identifier_prompt() {
    assert_eq!(
        filter_next_prompt_suggestion("set CODEX_HOME"),
        Some("set CODEX_HOME".to_string())
    );
}

#[test]
fn filter_keeps_dotted_file_prompt() {
    assert_eq!(
        filter_next_prompt_suggestion("update Cargo.toml"),
        Some("update Cargo.toml".to_string())
    );
    assert_eq!(
        filter_next_prompt_suggestion("open app-server/README.md"),
        Some("open app-server/README.md".to_string())
    );
}

#[test]
fn history_snapshot_detects_appends_and_rewrites() {
    let mut history = ContextManager::new();
    let snapshot = HistorySnapshot::from_history(&history);
    assert!(history_matches_snapshot(&history, snapshot));

    let item = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "next".to_string(),
        }],
        phase: None,
    };
    history.record_items([&item], TruncationPolicy::Tokens(10_000));
    assert!(!history_matches_snapshot(&history, snapshot));

    let appended_snapshot = HistorySnapshot::from_history(&history);
    history.replace(history.raw_items().to_vec());
    assert!(!history_matches_snapshot(&history, appended_snapshot));
}

#[test]
fn suggestion_prompt_skips_without_context_window() {
    assert!(!suggestion_prompt_has_headroom(
        /*estimated_token_count*/ Some(10),
        /*model_context_window*/ None,
    ));
}

#[test]
fn suggestion_prompt_skips_near_context_window() {
    assert!(!suggestion_prompt_has_headroom(
        /*estimated_token_count*/ Some(127_100),
        /*model_context_window*/ Some(128_000)
    ));
}

#[test]
fn incomplete_custom_tool_flow_is_suppressed() {
    assert!(has_unpaired_tool_flow(&[ResponseItem::CustomToolCall {
        id: None,
        status: None,
        call_id: "call-1".to_string(),
        name: "exec".to_string(),
        input: "{}".to_string(),
    }]));
}

#[test]
fn completed_custom_tool_flow_is_allowed() {
    assert!(!has_unpaired_tool_flow(&[
        ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "call-1".to_string(),
            name: "exec".to_string(),
            input: "{}".to_string(),
        },
        ResponseItem::CustomToolCallOutput {
            call_id: "call-1".to_string(),
            name: Some("exec".to_string()),
            output: FunctionCallOutputPayload::from_text("done".to_string()),
        },
    ]));
}

#[test]
fn server_tool_search_output_without_call_is_allowed() {
    assert!(!has_unpaired_tool_flow(&[ResponseItem::ToolSearchOutput {
        call_id: Some("call-1".to_string()),
        status: "completed".to_string(),
        execution: "server".to_string(),
        tools: Vec::new(),
    }]));
}

#[test]
fn client_tool_search_output_without_call_is_suppressed() {
    assert!(has_unpaired_tool_flow(&[ResponseItem::ToolSearchOutput {
        call_id: Some("call-1".to_string()),
        status: "completed".to_string(),
        execution: "client".to_string(),
        tools: Vec::new(),
    }]));
}

#[test]
fn filter_rejects_invalid_prompts() {
    for suggestion in [
        "",
        "done",
        "Suggestion: run the tests",
        "(stay silent)",
        "looks good",
        "thanks",
        "let me run tests",
        "what about tests?",
        "run tests.",
        "run\ntests",
        "continue with every possible next step in this project and explain every detail now",
    ] {
        assert_eq!(
            filter_next_prompt_suggestion(suggestion),
            None,
            "expected {suggestion:?} to be filtered"
        );
    }
}
