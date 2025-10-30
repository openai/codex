//! Automated TUI smoke tests
//!
//! Basic coverage of public TUI APIs to ensure core functionality remains stable.
//! Tests focus on CLI parsing and ComposerInput interactions without accessing
//! private internals or requiring heavy dependencies.

#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use code_core::history::state::{
    AssistantStreamDelta,
    HistoryId,
    HistoryRecord,
    HistoryState,
    MessageMetadata,
    PatchEventType,
};
use code_core::protocol::{
    AgentMessageDeltaEvent,
    AgentMessageEvent,
    ApplyPatchApprovalRequestEvent,
    CustomToolCallBeginEvent,
    CustomToolCallEndEvent,
    ExecApprovalRequestEvent,
    ExecCommandBeginEvent,
    ExecCommandEndEvent,
    ExecCommandOutputDeltaEvent,
    ExecOutputStream,
    Event,
    EventMsg,
    FileChange,
    McpInvocation,
    McpToolCallBeginEvent,
    McpToolCallEndEvent,
    OrderMeta,
    TokenUsage,
};
use code_tui::test_helpers::ChatWidgetHarness;
use code_tui::{Cli, ComposerAction, ComposerInput};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mcp_types::CallToolResult;
use serde_bytes::ByteBuf;
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind: crossterm::event::KeyEventKind::Press,
        state: crossterm::event::KeyEventState::empty(),
    }
}

#[test]
fn cli_web_search_flag_defaults() {
    // Default behavior: web search should be enabled after finalization
    let mut cli = Cli {
        prompt: None,
        images: Vec::new(),
        model: None,
        oss: false,
        config_profile: None,
        sandbox_mode: None,
        approval_policy: None,
        full_auto: false,
        dangerously_bypass_approvals_and_sandbox: false,
        cwd: None,
        enable_web_search: false,
        disable_web_search: false,
        web_search: false,
        debug: false,
        order: false,
        timing: false,
        config_overrides: Default::default(),
        resume_picker: false,
        resume_last: false,
        resume_session_id: None,
    };

    cli.finalize_defaults();
    assert!(cli.web_search, "web_search should default to true when no flags are set");
}

#[test]
fn cli_web_search_flag_explicit_enable() {
    // Explicit enable: web search should be enabled
    let mut cli = Cli {
        prompt: None,
        images: Vec::new(),
        model: None,
        oss: false,
        config_profile: None,
        sandbox_mode: None,
        approval_policy: None,
        full_auto: false,
        dangerously_bypass_approvals_and_sandbox: false,
        cwd: None,
        enable_web_search: true,
        disable_web_search: false,
        web_search: false,
        debug: false,
        order: false,
        timing: false,
        config_overrides: Default::default(),
        resume_picker: false,
        resume_last: false,
        resume_session_id: None,
    };

    cli.finalize_defaults();
    assert!(cli.web_search, "web_search should be true when enable_web_search is set");
}

#[test]
fn cli_web_search_flag_disable() {
    // Explicit disable: web search should be disabled
    let mut cli = Cli {
        prompt: None,
        images: Vec::new(),
        model: None,
        oss: false,
        config_profile: None,
        sandbox_mode: None,
        approval_policy: None,
        full_auto: false,
        dangerously_bypass_approvals_and_sandbox: false,
        cwd: None,
        enable_web_search: false,
        disable_web_search: true,
        web_search: false,
        debug: false,
        order: false,
        timing: false,
        config_overrides: Default::default(),
        resume_picker: false,
        resume_last: false,
        resume_session_id: None,
    };

    cli.finalize_defaults();
    assert!(!cli.web_search, "web_search should be false when disable_web_search is set");
}

#[test]
fn composer_input_paste_and_submit() {
    // Create a new ComposerInput and verify it starts empty
    let mut composer = ComposerInput::new();
    assert!(composer.is_empty(), "new ComposerInput should be empty");

    // Simulate pasting text
    let pasted_text = "Hello, world!".to_string();
    let handled = composer.handle_paste(pasted_text.clone());
    assert!(handled, "paste should be handled");

    // Verify the composer is no longer empty
    assert!(!composer.is_empty(), "ComposerInput should contain pasted text");

    // Simulate pressing Enter to submit
    let enter_key = make_key(KeyCode::Enter, KeyModifiers::NONE);

    match composer.input(enter_key) {
        ComposerAction::Submitted(text) => {
            assert_eq!(text, pasted_text, "submitted text should match pasted text");
        }
        ComposerAction::None => {
            panic!("Enter key should have submitted the text");
        }
    }

    // After submission, the composer should be empty
    assert!(composer.is_empty(), "ComposerInput should be empty after submission");
}

#[test]
fn composer_input_clear() {
    // Verify that clear() empties the composer
    let mut composer = ComposerInput::new();

    // Paste some text
    composer.handle_paste("Some text".to_string());
    assert!(!composer.is_empty(), "should contain text after paste");

    // Clear the composer
    composer.clear();
    assert!(composer.is_empty(), "should be empty after clear()");
}

#[test]
fn composer_input_normalizes_windows_newlines_on_paste() {
    let mut composer = ComposerInput::new();
    let windows_text = "line1\r\nline2\r\nline3";

    composer.handle_paste(windows_text.to_string());

    assert_eq!(composer.text(), "line1\nline2\nline3");
}

#[test]
fn composer_input_multiline_paste_submits_all_content() {
    let mut composer = ComposerInput::new();
    let multiline = "alpha\nbeta\ngamma";

    composer.handle_paste(multiline.to_string());

    let enter_key = make_key(KeyCode::Enter, KeyModifiers::NONE);
    match composer.input(enter_key) {
        ComposerAction::Submitted(text) => assert_eq!(text, multiline),
        ComposerAction::None => panic!("Enter key should submit multiline paste"),
    }

    assert!(composer.is_empty(), "composer should clear after submission");
}

#[test]
fn composer_input_shift_enter_no_submit() {
    // Verify that Shift+Enter does NOT submit (it should add a newline instead)
    let mut composer = ComposerInput::new();

    // Paste some initial text
    composer.handle_paste("First line".to_string());

    // Simulate Shift+Enter (should add newline, not submit)
    let shift_enter = make_key(KeyCode::Enter, KeyModifiers::SHIFT);

    match composer.input(shift_enter) {
        ComposerAction::None => {
            // Expected: Shift+Enter should not submit
            assert!(!composer.is_empty(), "composer should still contain text");
        }
        ComposerAction::Submitted(_) => {
            panic!("Shift+Enter should not submit text");
        }
    }
}

#[test]
fn composer_input_backslash_continuation_inserts_newline() {
    let mut composer = ComposerInput::new();
    composer.handle_paste("Hello\\".to_string());

    let enter = make_key(KeyCode::Enter, KeyModifiers::NONE);
    match composer.input(enter) {
        ComposerAction::None => {
            assert_eq!(composer.text(), "Hello\n");
            assert!(!composer.is_empty(), "composer should retain text after continuation");
        }
        ComposerAction::Submitted(_) => panic!("continuation should not submit"),
    }
}

#[test]
fn composer_input_backslash_continuation_trailing_space_submits() {
    let mut composer = ComposerInput::new();
    composer.handle_paste("Hello\\ ".to_string());

    match composer.input(make_key(KeyCode::Enter, KeyModifiers::NONE)) {
        ComposerAction::Submitted(text) => {
            assert_eq!(text, "Hello\\ ");
            assert!(composer.is_empty(), "composer should clear after submission");
        }
        ComposerAction::None => panic!("trailing space should cancel continuation"),
    }
}

#[test]
fn composer_input_backslash_even_count_submits() {
    let mut composer = ComposerInput::new();
    composer.handle_paste(String::from(r"Path: C\\"));

    match composer.input(make_key(KeyCode::Enter, KeyModifiers::NONE)) {
        ComposerAction::Submitted(text) => assert_eq!(text, String::from(r"Path: C\\")),
        ComposerAction::None => panic!("even number of backslashes should submit"),
    }
}

#[test]
fn composer_input_backslash_multiple_continuations() {
    let mut composer = ComposerInput::new();
    composer.handle_paste("line1\\".to_string());

    assert!(matches!(
        composer.input(make_key(KeyCode::Enter, KeyModifiers::NONE)),
        ComposerAction::None
    ));
    assert_eq!(composer.text(), "line1\n");

    composer.handle_paste("line2\\".to_string());
    assert!(matches!(
        composer.input(make_key(KeyCode::Enter, KeyModifiers::NONE)),
        ComposerAction::None
    ));
    assert_eq!(composer.text(), "line1\nline2\n");
}

#[test]
fn composer_input_backslash_continuation_preserves_indent() {
    let mut composer = ComposerInput::new();
    composer.handle_paste("    indented\\".to_string());

    assert!(matches!(
        composer.input(make_key(KeyCode::Enter, KeyModifiers::NONE)),
        ComposerAction::None
    ));
    assert_eq!(composer.text(), "    indented\n    ");
}

#[test]
fn composer_input_ctrl_c_aborts_without_submit() {
    let mut composer = ComposerInput::new();
    composer.handle_paste("pending".to_string());
    assert!(!composer.is_empty());

    let ctrl_c = make_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
    match composer.input(ctrl_c) {
        ComposerAction::None => assert!(
            !composer.is_empty(),
            "Ctrl+C should leave the composed text intact"
        ),
        ComposerAction::Submitted(_) => panic!("Ctrl+C should not submit input"),
    }
}

#[test]
fn composer_input_custom_hints_reset() {
    let mut composer = ComposerInput::new();
    composer.set_hint_items(vec![("F1", "Help"), ("F2", "Scratchpad")]);
    // Setting hints should not mark the composer as in a paste burst or modify text.
    assert!(composer.is_empty());
    assert!(!composer.is_in_paste_burst());

    composer.clear_hint_items();
    composer.handle_paste("hello".to_string());
    assert!(!composer.is_empty());
}

#[test]
fn render_markdown_text_handles_simple_lists() {
    let rendered = code_tui::render_markdown_text("- item one\n- item two");
    let text = rendered
        .lines
        .iter()
        .flat_map(|line| line.spans.iter().map(|span| span.content.clone()))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(text.contains("item one"));
    assert!(text.contains("item two"));
}

#[test]
fn smoke_history_state_initialization() {
    let state = HistoryState::new();
    assert!(state.records.is_empty());
    assert_eq!(state.next_id, 1);

    let snapshot = state.snapshot();
    assert!(snapshot.records.is_empty());
    assert!(snapshot.order.is_empty());
    assert!(snapshot.order_debug.is_empty());
}

#[test]
fn smoke_order_meta_round_trip() {
    let meta = OrderMeta {
        request_ordinal: 7,
        output_index: Some(2),
        sequence_number: Some(9),
    };

    let encoded = serde_json::to_string(&meta).expect("serialize order meta");
    let decoded: OrderMeta = serde_json::from_str(&encoded).expect("deserialize order meta");

    assert_eq!(decoded.request_ordinal, 7);
    assert_eq!(decoded.output_index, Some(2));
    assert_eq!(decoded.sequence_number, Some(9));
}

#[test]
fn smoke_exec_approval_event_structure() {
    let approval = ExecApprovalRequestEvent {
        call_id: "call-1".into(),
        command: vec!["echo".into(), "hi".into()],
        cwd: PathBuf::from("/tmp"),
        reason: Some("verify".into()),
    };

    let encoded = serde_json::to_string(&approval).expect("serialize exec approval");
    let decoded: ExecApprovalRequestEvent = serde_json::from_str(&encoded).expect("deserialize exec approval");

    assert_eq!(decoded.command.len(), 2);
    assert_eq!(decoded.reason.as_deref(), Some("verify"));
    assert_eq!(decoded.cwd, PathBuf::from("/tmp"));
}

#[test]
fn smoke_exec_begin_end_consistency() {
    let begin = ExecCommandBeginEvent {
        call_id: "exec-42".into(),
        command: vec!["pwd".into()],
        cwd: PathBuf::from("/workspace"),
        parsed_cmd: Vec::new(),
    };

    let end = ExecCommandEndEvent {
        call_id: begin.call_id.clone(),
        stdout: ".".into(),
        stderr: String::new(),
        exit_code: 0,
        duration: Duration::from_secs(2),
    };

    assert_eq!(end.call_id, begin.call_id);
    assert!(end.stderr.is_empty());
    assert_eq!(end.exit_code, 0);

    let delta = ExecCommandOutputDeltaEvent {
        call_id: begin.call_id.clone(),
        stream: ExecOutputStream::Stdout,
        chunk: vec![b'o', b'k'].into(),
    };

    assert!(matches!(delta.stream, ExecOutputStream::Stdout));
    assert_eq!(delta.call_id, begin.call_id);
}

#[test]
fn smoke_mcp_tool_call_event_structure() {
    let begin = CustomToolCallBeginEvent {
        call_id: "tool-1".into(),
        tool_name: "browser_navigate".into(),
        parameters: Some(json!({"url": "https://example.com"})),
    };

    let end = CustomToolCallEndEvent {
        call_id: begin.call_id.clone(),
        tool_name: begin.tool_name.clone(),
        parameters: begin.parameters.clone(),
        duration: Duration::from_millis(120),
        result: Ok("navigated".into()),
    };

    let encoded = serde_json::to_string(&end).expect("serialize tool call");
    let decoded: CustomToolCallEndEvent = serde_json::from_str(&encoded).expect("deserialize tool call");

    assert_eq!(decoded.tool_name, "browser_navigate");
    assert_eq!(decoded.duration, Duration::from_millis(120));
    assert_eq!(decoded.result, Ok("navigated".into()));
    assert_eq!(decoded.parameters, begin.parameters);
}

#[test]
fn assistant_stream_creates_new_record() {
    let mut state = HistoryState::new();
    let delta = AssistantStreamDelta {
        delta: "hello".into(),
        sequence: Some(1),
        received_at: SystemTime::now(),
    };

    let stream_id = "stream-1";
    let record_id = state.upsert_assistant_stream_state(stream_id, "Hello".into(), Some(delta.clone()), None);

    assert_ne!(record_id, HistoryId::ZERO);
    match state.records.last() {
        Some(HistoryRecord::AssistantStream(stream_state)) => {
            assert_eq!(stream_state.stream_id, stream_id);
            assert_eq!(stream_state.preview_markdown, "Hello");
            assert_eq!(stream_state.deltas.len(), 1);
            assert_eq!(stream_state.deltas[0], delta);
            assert!(stream_state.in_progress);
        }
        other => panic!("expected assistant stream record, got {other:?}"),
    }
}

#[test]
fn assistant_stream_finalize_transitions_to_message() {
    let mut state = HistoryState::new();
    let stream_id = "stream-42";
    let metadata = MessageMetadata {
        citations: vec!["ref:1".into()],
        token_usage: Some(TokenUsage {
            input_tokens: 3,
            cached_input_tokens: 1,
            output_tokens: 4,
            reasoning_output_tokens: 0,
            total_tokens: 8,
        }),
    };

    state.upsert_assistant_stream_state(stream_id, "First".into(), None, Some(&metadata));
    state.upsert_assistant_stream_state(
        stream_id,
        "Second".into(),
        Some(AssistantStreamDelta {
            delta: " world".into(),
            sequence: Some(2),
            received_at: SystemTime::now(),
        }),
        None,
    );

    let message = state.finalize_assistant_stream_state(
        Some(stream_id),
        "Hello world".into(),
        Some(&metadata),
        metadata.token_usage.as_ref(),
    );

    assert_eq!(message.markdown, "Hello world");
    assert_eq!(message.citations, metadata.citations);
    assert_eq!(message.token_usage, metadata.token_usage);
    assert!(state.assistant_stream_state(stream_id).is_none());
}

#[test]
fn assistant_stream_snapshot_round_trip() {
    let mut state = HistoryState::new();
    state.upsert_assistant_stream_state(
        "stream-snapshot",
        "Preview".into(),
        Some(AssistantStreamDelta {
            delta: "chunk".into(),
            sequence: Some(7),
            received_at: SystemTime::now(),
        }),
        None,
    );

    let snapshot = state.snapshot();

    // Serialize/deserialize snapshot to ensure it stays portable.
    let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
    let restored_snapshot: _ = serde_json::from_str(&json).expect("deserialize snapshot");

    let mut restored = HistoryState::new();
    restored.restore(&restored_snapshot);

    assert_eq!(restored.records.len(), 1);
    match &restored.records[0] {
        HistoryRecord::AssistantStream(stream) => {
            assert_eq!(stream.preview_markdown, "Preview");
            assert_eq!(stream.stream_id, "stream-snapshot");
            assert_eq!(stream.deltas.len(), 1);
        }
        other => panic!("expected assistant stream record after restore, got {other:?}"),
    }
}

#[test]
fn smoke_exec_command_stream() {
    let mut harness = ChatWidgetHarness::new();
    let call_id = "exec-smoke-1".to_string();

    harness.handle_event(Event {
        id: "sub-exec".into(),
        event_seq: 0,
        msg: EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id: call_id.clone(),
            command: vec!["echo".to_string(), "hello".to_string()],
            cwd: PathBuf::from("/tmp"),
            parsed_cmd: Vec::new(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(0),
            sequence_number: Some(0),
        }),
    });

    harness.handle_event(Event {
        id: "sub-exec".into(),
        event_seq: 1,
        msg: EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
            call_id: call_id.clone(),
            stream: ExecOutputStream::Stdout,
            chunk: ByteBuf::from(vec![b'h', b'i', b'\n']),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(0),
            sequence_number: Some(1),
        }),
    });

    harness.handle_event(Event {
        id: "sub-exec".into(),
        event_seq: 2,
        msg: EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: call_id.clone(),
            stdout: "hi\n".into(),
            stderr: String::new(),
            exit_code: 0,
            duration: Duration::from_millis(20),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(0),
            sequence_number: Some(2),
        }),
    });

    let records = code_tui::test_helpers::history_records(&mut harness);
    let exec_record = records
        .into_iter()
        .find_map(|record| match record {
            HistoryRecord::Exec(record) if record.call_id.as_deref() == Some(&call_id) => Some(record),
            _ => None,
        })
        .expect("exec record present");

    assert_eq!(exec_record.exit_code, Some(0), "exec exit code should be success");
    let stdout = exec_record
        .stdout_chunks
        .iter()
        .map(|chunk| chunk.content.as_str())
        .collect::<String>();
    assert_eq!(stdout, "hi\n", "stdout should include streamed content");
}

#[test]
fn smoke_approval_flow() {
    let mut harness = ChatWidgetHarness::new();
    let mut changes = std::collections::HashMap::new();
    changes.insert(
        PathBuf::from("sample.txt"),
        FileChange::Add {
            content: "Hello world".into(),
        },
    );

    harness.handle_event(Event {
        id: "sub-approval".into(),
        event_seq: 0,
        msg: EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
            call_id: "apply-1".into(),
            changes,
            reason: Some("Apply change".into()),
            grant_root: None,
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(1),
            sequence_number: Some(0),
        }),
    });

    let records = code_tui::test_helpers::history_records(&mut harness);
    let patch_record = records
        .into_iter()
        .find_map(|record| match record {
            HistoryRecord::Patch(record) => Some(record),
            _ => None,
        })
        .expect("patch approval record present");

    assert!(
        matches!(patch_record.patch_type, PatchEventType::ApprovalRequest),
        "expected approval request patch cell"
    );
    let change = patch_record
        .changes
        .get(&PathBuf::from("sample.txt"))
        .expect("change for sample.txt present");
    match change {
        FileChange::Add { content } => {
            assert_eq!(content, "Hello world", "patch change should include file contents");
        }
        other => panic!("expected FileChange::Add, got {other:?}"),
    }
}

#[test]
fn smoke_custom_tool_call() {
    let mut harness = ChatWidgetHarness::new();
    harness.handle_event(Event {
        id: "sub-tool".into(),
        event_seq: 0,
        msg: EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "tool-1".into(),
            tool_name: "browser_navigate".into(),
            parameters: Some(json!({ "url": "https://example.com" })),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(1),
            sequence_number: Some(0),
        }),
    });

    harness.handle_event(Event {
        id: "sub-tool".into(),
        event_seq: 1,
        msg: EventMsg::CustomToolCallEnd(CustomToolCallEndEvent {
            call_id: "tool-1".into(),
            tool_name: "browser_navigate".into(),
            parameters: Some(json!({ "url": "https://example.com" })),
            duration: Duration::from_millis(40),
            result: Ok("ok".into()),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(1),
            sequence_number: Some(1),
        }),
    });

    code_tui::test_helpers::assert_has_codex_event(&mut harness);
}

#[test]
fn smoke_mcp_tool_invocation() {
    let mut harness = ChatWidgetHarness::new();
    let invocation = McpInvocation {
        server: "fs".into(),
        tool: "list".into(),
        arguments: None,
    };

    harness.handle_event(Event {
        id: "sub-mcp".into(),
        event_seq: 0,
        msg: EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
            call_id: "mcp-1".into(),
            invocation: invocation.clone(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(1),
            sequence_number: Some(0),
        }),
    });

    harness.handle_event(Event {
        id: "sub-mcp".into(),
        event_seq: 1,
        msg: EventMsg::McpToolCallEnd(McpToolCallEndEvent {
            call_id: "mcp-1".into(),
            invocation,
            duration: Duration::from_millis(60),
            result: Ok(CallToolResult {
                content: Vec::new(),
                is_error: Some(false),
                structured_content: None,
            }),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(1),
            sequence_number: Some(1),
        }),
    });

    code_tui::test_helpers::assert_has_codex_event(&mut harness);
}

#[test]
fn smoke_streaming_assistant_message() {
    let mut harness = ChatWidgetHarness::new();
    harness.handle_event(Event {
        id: "sub-msg".into(),
        event_seq: 0,
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "Hello".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(0),
            sequence_number: Some(0),
        }),
    });

    harness.handle_event(Event {
        id: "sub-msg".into(),
        event_seq: 1,
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: " world".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(0),
            sequence_number: Some(1),
        }),
    });

    harness.handle_event(Event {
        id: "sub-msg".into(),
        event_seq: 2,
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "Hello world".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(0),
            sequence_number: Some(2),
        }),
    });

    code_tui::test_helpers::assert_has_insert_history(&mut harness);
}
