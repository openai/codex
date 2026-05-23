use super::HistorySnapshot;
use super::filter_next_prompt_suggestion;
use super::has_unpaired_tool_flow;
use super::history_ends_at_assistant_response;
use super::history_matches_snapshot;
use super::suggestion_prompt_has_headroom;
use crate::context_manager::ContextManager;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_utils_output_truncation::TruncationPolicy;
use pretty_assertions::assert_eq;

#[test]
fn filter_keeps_valid_prompts() {
    for (suggestion, expected) in [
        ("run the tests", "run the tests"),
        (" run the tests\n", "run the tests"),
        ("commit", "commit"),
        ("set CODEX_HOME", "set CODEX_HOME"),
        ("update Cargo.toml", "update Cargo.toml"),
        ("open app-server/README.md", "open app-server/README.md"),
    ] {
        assert_eq!(
            filter_next_prompt_suggestion(suggestion),
            Some(expected.to_string()),
            "expected {suggestion:?} to be retained"
        );
    }
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
fn history_boundary_requires_final_assistant_message() {
    let assistant = ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: "done".to_string(),
        }],
        phase: None,
    };
    let user = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "next".to_string(),
        }],
        phase: None,
    };

    assert!(history_ends_at_assistant_response(std::slice::from_ref(
        &assistant
    )));
    for tail in [
        user,
        ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text("done".to_string()),
        },
    ] {
        assert!(!history_ends_at_assistant_response(&[
            assistant.clone(),
            tail,
        ]));
    }
}

#[test]
fn suggestion_prompt_skips_near_context_window() {
    assert!(!suggestion_prompt_has_headroom(
        /*estimated_token_count*/ 127_100, /*model_context_window*/ 128_000
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
fn completed_server_tool_search_flow_is_allowed() {
    assert!(!has_unpaired_tool_flow(&[
        ResponseItem::ToolSearchCall {
            id: None,
            call_id: Some("call-1".to_string()),
            status: Some("completed".to_string()),
            execution: "server".to_string(),
            arguments: serde_json::json!({"query": "tool"}),
        },
        ResponseItem::ToolSearchOutput {
            call_id: Some("call-1".to_string()),
            status: "completed".to_string(),
            execution: "server".to_string(),
            tools: Vec::new(),
        },
    ]));
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
