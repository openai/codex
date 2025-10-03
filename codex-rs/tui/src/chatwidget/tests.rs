use super::*;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::test_backend::VT100Backend;
use crate::tui::FrameRequester;
use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigToml;
use codex_core::protocol::AgentMessageDeltaEvent;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::AgentReasoningDeltaEvent;
use codex_core::protocol::AgentReasoningEvent;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::ExecCommandBeginEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::ExitedReviewModeEvent;
use codex_core::protocol::FileChange;
use codex_core::protocol::InputMessageKind;
use codex_core::protocol::Op;
use codex_core::protocol::PatchApplyBeginEvent;
use codex_core::protocol::PatchApplyEndEvent;
use codex_core::protocol::ReviewCodeLocation;
use codex_core::protocol::ReviewFinding;
use codex_core::protocol::ReviewLineRange;
use codex_core::protocol::ReviewOutputEvent;
use codex_core::protocol::ReviewRequest;
use codex_core::protocol::StreamErrorEvent;
use codex_core::protocol::TaskCompleteEvent;
use codex_core::protocol::TaskStartedEvent;
use codex_core::protocol::ViewImageToolCallEvent;
use codex_protocol::ConversationId;
use codex_protocol::plan_tool::PlanItemArg;
use codex_protocol::plan_tool::StepStatus;
use codex_protocol::plan_tool::UpdatePlanArgs;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::unbounded_channel;

fn test_config() -> Config {
    // Use base defaults to avoid depending on host state.
    Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("config")
}

// Backward-compat shim for older session logs that predate the
// `formatted_output` field on ExecCommandEnd events.
fn upgrade_event_payload_for_tests(mut payload: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = payload.as_object_mut()
        && let Some(msg) = obj.get_mut("msg")
        && let Some(m) = msg.as_object_mut()
    {
        let ty = m.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if ty == "exec_command_end" && !m.contains_key("formatted_output") {
            let stdout = m.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
            let stderr = m.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
            let formatted = if stderr.is_empty() {
                stdout.to_string()
            } else {
                format!("{stdout}{stderr}")
            };
            m.insert(
                "formatted_output".to_string(),
                serde_json::Value::String(formatted),
            );
        }
    }
    payload
}

#[test]
fn resumed_initial_messages_render_history() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual();

    let conversation_id = ConversationId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = codex_core::protocol::SessionConfiguredEvent {
        session_id: conversation_id,
        model: "test-model".to_string(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: Some(vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "hello from user".to_string(),
                kind: Some(InputMessageKind::Plain),
                images: None,
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "assistant reply".to_string(),
            }),
        ]),
        rollout_path: rollout_file.path().to_path_buf(),
    };

    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });

    let cells = drain_insert_history(&mut rx);
    let mut merged_lines = Vec::new();
    for lines in cells {
        let text = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.clone())
            .collect::<String>();
        merged_lines.push(text);
    }

    let text_blob = merged_lines.join("\n");
    assert!(
        text_blob.contains("hello from user"),
        "expected replayed user message",
    );
    assert!(
        text_blob.contains("assistant reply"),
        "expected replayed agent message",
    );
}

/// Entering review mode uses the hint provided by the review request.
#[test]
fn entered_review_mode_uses_request_hint() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual();

    chat.handle_codex_event(Event {
        id: "review-start".into(),
        msg: EventMsg::EnteredReviewMode(ReviewRequest {
            prompt: "Review the latest changes".to_string(),
            user_facing_hint: "feature branch".to_string(),
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let banner = lines_to_single_string(cells.last().expect("review banner"));
    assert_eq!(banner, ">> Code review started: feature branch <<\n");
    assert!(chat.is_review_mode);
}

/// Entering review mode renders the current changes banner when requested.
#[test]
fn entered_review_mode_defaults_to_current_changes_banner() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual();

    chat.handle_codex_event(Event {
        id: "review-start".into(),
        msg: EventMsg::EnteredReviewMode(ReviewRequest {
            prompt: "Review the current changes".to_string(),
            user_facing_hint: "current changes".to_string(),
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let banner = lines_to_single_string(cells.last().expect("review banner"));
    assert_eq!(banner, ">> Code review started: current changes <<\n");
    assert!(chat.is_review_mode);
}

/// Completing review with findings shows the selection popup and finishes with
/// the closing banner while clearing review mode state.
#[test]
fn exited_review_mode_emits_results_and_finishes() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual();

    let review = ReviewOutputEvent {
        findings: vec![ReviewFinding {
            title: "[P1] Fix bug".to_string(),
            body: "Something went wrong".to_string(),
            confidence_score: 0.9,
            priority: 1,
            code_location: ReviewCodeLocation {
                absolute_file_path: PathBuf::from("src/lib.rs"),
                line_range: ReviewLineRange { start: 10, end: 12 },
            },
        }],
        overall_correctness: "needs work".to_string(),
        overall_explanation: "Investigate the failure".to_string(),
        overall_confidence_score: 0.5,
    };

    chat.handle_codex_event(Event {
        id: "review-end".into(),
        msg: EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
            review_output: Some(review),
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let banner = lines_to_single_string(cells.last().expect("finished banner"));
    assert_eq!(banner, "\n<< Code review finished >>\n");
    assert!(!chat.is_review_mode);
}

#[cfg_attr(
    target_os = "macos",
    ignore = "system configuration APIs are blocked under macOS seatbelt"
)]
#[tokio::test(flavor = "current_thread")]
async fn helpers_are_available_and_do_not_panic() {
    let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let cfg = test_config();
    let conversation_manager = Arc::new(ConversationManager::with_auth(CodexAuth::from_api_key(
        "test",
    )));
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test"));
    let init = ChatWidgetInit {
        config: cfg,
        frame_requester: FrameRequester::test_dummy(),
        app_event_tx: tx,
        initial_prompt: None,
        initial_images: Vec::new(),
        enhanced_keys_supported: false,
        auth_manager,
    };
    let mut w = ChatWidget::new(init, conversation_manager);
    // Basic construction sanity.
    let _ = &mut w;
}

// --- Helpers for tests that need direct construction and event draining ---
fn make_chatwidget_manual() -> (
    ChatWidget,
    tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    tokio::sync::mpsc::UnboundedReceiver<Op>,
) {
    let (tx_raw, rx) = unbounded_channel::<AppEvent>();
    let app_event_tx = AppEventSender::new(tx_raw);
    let (op_tx, op_rx) = unbounded_channel::<Op>();
    let cfg = test_config();
    let bottom = BottomPane::new(BottomPaneParams {
        app_event_tx: app_event_tx.clone(),
        frame_requester: FrameRequester::test_dummy(),
        has_input_focus: true,
        enhanced_keys_supported: false,
        placeholder_text: "Ask Codex to do anything".to_string(),
        disable_paste_burst: false,
    });
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test"));
    let widget = ChatWidget {
        app_event_tx,
        codex_op_tx: op_tx,
        bottom_pane: bottom,
        active_cell: None,
        config: cfg.clone(),
        auth_manager,
        session_header: SessionHeader::new(cfg.model),
        initial_user_message: None,
        token_info: None,
        rate_limit_snapshot: None,
        rate_limit_warnings: RateLimitWarningState::default(),
        stream_controller: None,
        running_commands: HashMap::new(),
        task_complete_pending: false,
        interrupts: InterruptManager::new(),
        reasoning_buffer: String::new(),
        full_reasoning_buffer: String::new(),
        conversation_id: None,
        frame_requester: FrameRequester::test_dummy(),
        show_welcome_banner: true,
        queued_user_messages: VecDeque::new(),
        suppress_session_configured_redraw: false,
        pending_notification: None,
        is_review_mode: false,
        ghost_snapshots: Vec::new(),
        ghost_snapshots_disabled: false,
        needs_final_message_separator: false,
        last_rendered_width: std::cell::Cell::new(None),
    };
    (widget, rx, op_rx)
}

pub(crate) fn make_chatwidget_manual_with_sender() -> (
    ChatWidget,
    AppEventSender,
    tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    tokio::sync::mpsc::UnboundedReceiver<Op>,
) {
    let (widget, rx, op_rx) = make_chatwidget_manual();
    let app_event_tx = widget.app_event_tx.clone();
    (widget, app_event_tx, rx, op_rx)
}

fn drain_insert_history(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> Vec<Vec<ratatui::text::Line<'static>>> {
    let mut out = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = ev {
            let mut lines = cell.display_lines(80);
            if !cell.is_stream_continuation() && !out.is_empty() && !lines.is_empty() {
                lines.insert(0, "".into());
            }
            out.push(lines)
        }
    }
    out
}

fn lines_to_single_string(lines: &[ratatui::text::Line<'static>]) -> String {
    let mut s = String::new();
    for line in lines {
        for span in &line.spans {
            s.push_str(&span.content);
        }
        s.push('\n');
    }
    s
}

#[test]
fn rate_limit_warnings_emit_thresholds() {
    let mut state = RateLimitWarningState::default();
    let mut warnings: Vec<String> = Vec::new();

    warnings.extend(state.take_warnings(Some(10.0), Some(10079), Some(55.0), Some(299)));
    warnings.extend(state.take_warnings(Some(55.0), Some(10081), Some(10.0), Some(299)));
    warnings.extend(state.take_warnings(Some(10.0), Some(10081), Some(80.0), Some(299)));
    warnings.extend(state.take_warnings(Some(80.0), Some(10081), Some(10.0), Some(299)));
    warnings.extend(state.take_warnings(Some(10.0), Some(10081), Some(95.0), Some(299)));
    warnings.extend(state.take_warnings(Some(95.0), Some(10079), Some(10.0), Some(299)));

    assert_eq!(
        warnings,
        vec![
            String::from(
                "Heads up, you've used over 75% of your 5h limit. Run /status for a breakdown."
            ),
            String::from(
                "Heads up, you've used over 75% of your weekly limit. Run /status for a breakdown.",
            ),
            String::from(
                "Heads up, you've used over 95% of your 5h limit. Run /status for a breakdown."
            ),
            String::from(
                "Heads up, you've used over 95% of your weekly limit. Run /status for a breakdown.",
            ),
        ],
        "expected one warning per limit for the highest crossed threshold"
    );
}

#[test]
fn test_rate_limit_warnings_monthly() {
    let mut state = RateLimitWarningState::default();
    let mut warnings: Vec<String> = Vec::new();

    warnings.extend(state.take_warnings(Some(75.0), Some(43199), None, None));
    assert_eq!(
        warnings,
        vec![String::from(
            "Heads up, you've used over 75% of your monthly limit. Run /status for a breakdown.",
        ),],
        "expected one warning per limit for the highest crossed threshold"
    );
}

// (removed experimental resize snapshot test)

#[test]
fn exec_approval_emits_proposed_command_and_decision_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Trigger an exec approval request with a short, single-line command
    let ev = ExecApprovalRequestEvent {
        call_id: "call-short".into(),
        command: vec!["bash".into(), "-lc".into(), "echo hello world".into()],
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        reason: Some(
            "this is a test reason such as one that would be produced by the model".into(),
        ),
    };
    chat.handle_codex_event(Event {
        id: "sub-short".into(),
        msg: EventMsg::ExecApprovalRequest(ev),
    });

    let proposed_cells = drain_insert_history(&mut rx);
    assert!(
        proposed_cells.is_empty(),
        "expected approval request to render via modal without emitting history cells"
    );

    // The approval modal should display the command snippet for user confirmation.
    let area = Rect::new(0, 0, 80, chat.desired_height(80));
    let mut buf = ratatui::buffer::Buffer::empty(area);
    (&chat).render_ref(area, &mut buf);
    assert_snapshot!("exec_approval_modal_exec", format!("{buf:?}"));

    // Approve via keyboard and verify a concise decision history line is added
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    let decision = drain_insert_history(&mut rx)
        .pop()
        .expect("expected decision cell in history");
    assert_snapshot!(
        "exec_approval_history_decision_approved_short",
        lines_to_single_string(&decision)
    );
}

#[test]
fn exec_approval_decision_truncates_multiline_and_long_commands() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Multiline command: modal should show full command, history records decision only
    let ev_multi = ExecApprovalRequestEvent {
        call_id: "call-multi".into(),
        command: vec!["bash".into(), "-lc".into(), "echo line1\necho line2".into()],
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        reason: Some(
            "this is a test reason such as one that would be produced by the model".into(),
        ),
    };
    chat.handle_codex_event(Event {
        id: "sub-multi".into(),
        msg: EventMsg::ExecApprovalRequest(ev_multi),
    });
    let proposed_multi = drain_insert_history(&mut rx);
    assert!(
        proposed_multi.is_empty(),
        "expected multiline approval request to render via modal without emitting history cells"
    );

    let area = Rect::new(0, 0, 80, chat.desired_height(80));
    let mut buf = ratatui::buffer::Buffer::empty(area);
    (&chat).render_ref(area, &mut buf);
    let mut saw_first_line = false;
    for y in 0..area.height {
        let mut row = String::new();
        for x in 0..area.width {
            row.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
        }
        if row.contains("echo line1") {
            saw_first_line = true;
            break;
        }
    }
    assert!(
        saw_first_line,
        "expected modal to show first line of multiline snippet"
    );

    // Deny via keyboard; decision snippet should be single-line and elided with " ..."
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
    let aborted_multi = drain_insert_history(&mut rx)
        .pop()
        .expect("expected aborted decision cell (multiline)");
    assert_snapshot!(
        "exec_approval_history_decision_aborted_multiline",
        lines_to_single_string(&aborted_multi)
    );

    // Very long single-line command: decision snippet should be truncated <= 80 chars with trailing ...
    let long = format!("echo {}", "a".repeat(200));
    let ev_long = ExecApprovalRequestEvent {
        call_id: "call-long".into(),
        command: vec!["bash".into(), "-lc".into(), long],
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        reason: None,
    };
    chat.handle_codex_event(Event {
        id: "sub-long".into(),
        msg: EventMsg::ExecApprovalRequest(ev_long),
    });
    let proposed_long = drain_insert_history(&mut rx);
    assert!(
        proposed_long.is_empty(),
        "expected long approval request to avoid emitting history cells before decision"
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
    let aborted_long = drain_insert_history(&mut rx)
        .pop()
        .expect("expected aborted decision cell (long)");
    assert_snapshot!(
        "exec_approval_history_decision_aborted_long",
        lines_to_single_string(&aborted_long)
    );
}

// --- Small helpers to tersely drive exec begin/end and snapshot active cell ---
fn begin_exec(chat: &mut ChatWidget, call_id: &str, raw_cmd: &str) {
    // Build the full command vec and parse it using core's parser,
    // then convert to protocol variants for the event payload.
    let command = vec!["bash".to_string(), "-lc".to_string(), raw_cmd.to_string()];
    let parsed_cmd: Vec<ParsedCommand> = codex_core::parse_command::parse_command(&command)
        .into_iter()
        .map(Into::into)
        .collect();
    chat.handle_codex_event(Event {
        id: call_id.to_string(),
        msg: EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id: call_id.to_string(),
            command,
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            parsed_cmd,
        }),
    });
}

fn end_exec(chat: &mut ChatWidget, call_id: &str, stdout: &str, stderr: &str, exit_code: i32) {
    let aggregated = if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}{stderr}")
    };
    chat.handle_codex_event(Event {
        id: call_id.to_string(),
        msg: EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: call_id.to_string(),
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            aggregated_output: aggregated.clone(),
            exit_code,
            duration: std::time::Duration::from_millis(5),
            formatted_output: aggregated,
        }),
    });
}

fn active_blob(chat: &ChatWidget) -> String {
    let lines = chat
        .active_cell
        .as_ref()
        .expect("active cell present")
        .display_lines(80);
    lines_to_single_string(&lines)
}

fn open_fixture(name: &str) -> File {
    // 1) Prefer fixtures within this crate
    {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push("fixtures");
        p.push(name);
        if let Ok(f) = File::open(&p) {
            return f;
        }
    }
    // 2) Fallback to parent (workspace root)
    {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("..");
        p.push(name);
        if let Ok(f) = File::open(&p) {
            return f;
        }
    }
    // 3) Last resort: CWD
    File::open(name).expect("open fixture file")
}

#[test]
fn empty_enter_during_task_does_not_queue() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();

    // Simulate running task so submissions would normally be queued.
    chat.bottom_pane.set_task_running(true);

    // Press Enter with an empty composer.
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Ensure nothing was queued.
    assert!(chat.queued_user_messages.is_empty());
}

#[test]
fn alt_up_edits_most_recent_queued_message() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();

    // Simulate a running task so messages would normally be queued.
    chat.bottom_pane.set_task_running(true);

    // Seed two queued messages.
    chat.queued_user_messages
        .push_back(UserMessage::from("first queued".to_string()));
    chat.queued_user_messages
        .push_back(UserMessage::from("second queued".to_string()));
    chat.refresh_queued_user_messages();

    // Press Alt+Up to edit the most recent (last) queued message.
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));

    // Composer should now contain the last queued message.
    assert_eq!(
        chat.bottom_pane.composer_text(),
        "second queued".to_string()
    );
    // And the queue should now contain only the remaining (older) item.
    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().text,
        "first queued"
    );
}

#[test]
fn streaming_final_answer_keeps_task_running_state() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual();

    chat.on_task_started();
    chat.on_agent_message_delta("Final answer line\n".to_string());
    chat.on_commit_tick();

    assert!(chat.bottom_pane.is_task_running());
    assert!(chat.bottom_pane.status_widget().is_none());

    chat.bottom_pane
        .set_composer_text("queued submission".to_string());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().text,
        "queued submission"
    );
    assert!(matches!(op_rx.try_recv(), Err(TryRecvError::Empty)));

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    match op_rx.try_recv() {
        Ok(Op::Interrupt) => {}
        other => panic!("expected Op::Interrupt, got {other:?}"),
    }
    assert!(chat.bottom_pane.ctrl_c_quit_hint_visible());
}

#[test]
fn exec_history_cell_shows_working_then_completed() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Begin command
    begin_exec(&mut chat, "call-1", "echo done");

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 0, "no exec cell should have been flushed yet");

    // End command successfully
    end_exec(&mut chat, "call-1", "done", "", 0);

    let cells = drain_insert_history(&mut rx);
    // Exec end now finalizes and flushes the exec cell immediately.
    assert_eq!(cells.len(), 1, "expected finalized exec cell to flush");
    // Inspect the flushed exec cell rendering.
    let lines = &cells[0];
    let blob = lines_to_single_string(lines);
    // New behavior: no glyph markers; ensure command is shown and no panic.
    assert!(
        blob.contains("• Ran"),
        "expected summary header present: {blob:?}"
    );
    assert!(
        blob.contains("echo done"),
        "expected command text to be present: {blob:?}"
    );
}

#[test]
fn exec_history_cell_shows_working_then_failed() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Begin command
    begin_exec(&mut chat, "call-2", "false");
    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 0, "no exec cell should have been flushed yet");

    // End command with failure
    end_exec(&mut chat, "call-2", "", "Bloop", 2);

    let cells = drain_insert_history(&mut rx);
    // Exec end with failure should also flush immediately.
    assert_eq!(cells.len(), 1, "expected finalized exec cell to flush");
    let lines = &cells[0];
    let blob = lines_to_single_string(lines);
    assert!(
        blob.contains("• Ran false"),
        "expected command and header text present: {blob:?}"
    );
    assert!(blob.to_lowercase().contains("bloop"), "expected error text");
}

/// Selecting the custom prompt option from the review popup sends
/// OpenReviewCustomPrompt to the app event channel.
#[test]
fn review_popup_custom_prompt_action_sends_event() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Open the preset selection popup
    chat.open_review_popup();

    // Move selection down to the fourth item: "Custom review instructions"
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    // Activate
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Drain events and ensure we saw the OpenReviewCustomPrompt request
    let mut found = false;
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::OpenReviewCustomPrompt = ev {
            found = true;
            break;
        }
    }
    assert!(found, "expected OpenReviewCustomPrompt event to be sent");
}

/// The commit picker shows only commit subjects (no timestamps).
#[test]
fn review_commit_picker_shows_subjects_without_timestamps() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();

    // Open the Review presets parent popup.
    chat.open_review_popup();

    // Show commit picker with synthetic entries.
    let entries = vec![
        codex_core::git_info::CommitLogEntry {
            sha: "1111111deadbeef".to_string(),
            timestamp: 0,
            subject: "Add new feature X".to_string(),
        },
        codex_core::git_info::CommitLogEntry {
            sha: "2222222cafebabe".to_string(),
            timestamp: 0,
            subject: "Fix bug Y".to_string(),
        },
    ];
    super::show_review_commit_picker_with_entries(&mut chat, entries);

    // Render the bottom pane and inspect the lines for subjects and absence of time words.
    let width = 72;
    let height = chat.desired_height(width);
    let area = ratatui::layout::Rect::new(0, 0, width, height);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    (&chat).render_ref(area, &mut buf);

    let mut blob = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            let s = buf[(x, y)].symbol();
            if s.is_empty() {
                blob.push(' ');
            } else {
                blob.push_str(s);
            }
        }
        blob.push('\n');
    }

    assert!(
        blob.contains("Add new feature X"),
        "expected subject in output"
    );
    assert!(blob.contains("Fix bug Y"), "expected subject in output");

    // Ensure no relative-time phrasing is present.
    let lowered = blob.to_lowercase();
    assert!(
        !lowered.contains("ago")
            && !lowered.contains(" second")
            && !lowered.contains(" minute")
            && !lowered.contains(" hour")
            && !lowered.contains(" day"),
        "expected no relative time in commit picker output: {blob:?}"
    );
}

/// Submitting the custom prompt view sends Op::Review with the typed prompt
/// and uses the same text for the user-facing hint.
#[test]
fn custom_prompt_submit_sends_review_op() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    chat.show_review_custom_prompt();
    // Paste prompt text via ChatWidget handler, then submit
    chat.handle_paste("  please audit dependencies  ".to_string());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Expect AppEvent::CodexOp(Op::Review { .. }) with trimmed prompt
    let evt = rx.try_recv().expect("expected one app event");
    match evt {
        AppEvent::CodexOp(Op::Review { review_request }) => {
            assert_eq!(
                review_request.prompt,
                "please audit dependencies".to_string()
            );
            assert_eq!(
                review_request.user_facing_hint,
                "please audit dependencies".to_string()
            );
        }
        other => panic!("unexpected app event: {other:?}"),
    }
}

/// Hitting Enter on an empty custom prompt view does not submit.
#[test]
fn custom_prompt_enter_empty_does_not_send() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    chat.show_review_custom_prompt();
    // Enter without any text
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // No AppEvent::CodexOp should be sent
    assert!(rx.try_recv().is_err(), "no app event should be sent");
}

#[test]
fn view_image_tool_call_adds_history_cell() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();
    let image_path = chat.config.cwd.join("example.png");

    chat.handle_codex_event(Event {
        id: "sub-image".into(),
        msg: EventMsg::ViewImageToolCall(ViewImageToolCallEvent {
            call_id: "call-image".into(),
            path: image_path,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected a single history cell");
    let combined = lines_to_single_string(&cells[0]);
    assert_snapshot!("local_image_attachment_history_snapshot", combined);
}

// Snapshot test: interrupting a running exec finalizes the active cell with a red ✗
// marker (replacing the spinner) and flushes it into history.
#[test]
fn interrupt_exec_marks_failed_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Begin a long-running command so we have an active exec cell with a spinner.
    begin_exec(&mut chat, "call-int", "sleep 1");

    // Simulate the task being aborted (as if ESC was pressed), which should
    // cause the active exec cell to be finalized as failed and flushed.
    chat.handle_codex_event(Event {
        id: "call-int".into(),
        msg: EventMsg::TurnAborted(codex_core::protocol::TurnAbortedEvent {
            reason: TurnAbortReason::Interrupted,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    assert!(
        !cells.is_empty(),
        "expected finalized exec cell to be inserted into history"
    );

    // The first inserted cell should be the finalized exec; snapshot its text.
    let exec_blob = lines_to_single_string(&cells[0]);
    assert_snapshot!("interrupt_exec_marks_failed", exec_blob);
}

/// Opening custom prompt from the review popup, pressing Esc returns to the
/// parent popup, pressing Esc again dismisses all panels (back to normal mode).
#[test]
fn review_custom_prompt_escape_navigates_back_then_dismisses() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();

    // Open the Review presets parent popup.
    chat.open_review_popup();

    // Open the custom prompt submenu (child view) directly.
    chat.show_review_custom_prompt();

    // Verify child view is on top.
    let header = render_bottom_first_row(&chat, 60);
    assert!(
        header.contains("Custom review instructions"),
        "expected custom prompt view header: {header:?}"
    );

    // Esc once: child view closes, parent (review presets) remains.
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let header = render_bottom_first_row(&chat, 60);
    assert!(
        header.contains("Select a review preset"),
        "expected to return to parent review popup: {header:?}"
    );

    // Esc again: parent closes; back to normal composer state.
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        chat.is_normal_backtrack_mode(),
        "expected to be back in normal composer mode"
    );
}

/// Opening base-branch picker from the review popup, pressing Esc returns to the
/// parent popup, pressing Esc again dismisses all panels (back to normal mode).
#[tokio::test(flavor = "current_thread")]
async fn review_branch_picker_escape_navigates_back_then_dismisses() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();

    // Open the Review presets parent popup.
    chat.open_review_popup();

    // Open the branch picker submenu (child view). Using a temp cwd with no git repo is fine.
    let cwd = std::env::temp_dir();
    chat.show_review_branch_picker(&cwd).await;

    // Verify child view header.
    let header = render_bottom_first_row(&chat, 60);
    assert!(
        header.contains("Select a base branch"),
        "expected branch picker header: {header:?}"
    );

    // Esc once: child view closes, parent remains.
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let header = render_bottom_first_row(&chat, 60);
    assert!(
        header.contains("Select a review preset"),
        "expected to return to parent review popup: {header:?}"
    );

    // Esc again: parent closes; back to normal composer state.
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        chat.is_normal_backtrack_mode(),
        "expected to be back in normal composer mode"
    );
}

fn render_bottom_first_row(chat: &ChatWidget, width: u16) -> String {
    let height = chat.desired_height(width);
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    (chat).render_ref(area, &mut buf);
    for y in 0..area.height {
        let mut row = String::new();
        for x in 0..area.width {
            let s = buf[(x, y)].symbol();
            if s.is_empty() {
                row.push(' ');
            } else {
                row.push_str(s);
            }
        }
        if !row.trim().is_empty() {
            return row;
        }
    }
    String::new()
}

fn render_bottom_popup(chat: &ChatWidget, width: u16) -> String {
    let height = chat.desired_height(width);
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    (chat).render_ref(area, &mut buf);

    let mut lines: Vec<String> = (0..area.height)
        .map(|row| {
            let mut line = String::new();
            for col in 0..area.width {
                let symbol = buf[(area.x + col, area.y + row)].symbol();
                if symbol.is_empty() {
                    line.push(' ');
                } else {
                    line.push_str(symbol);
                }
            }
            line.trim_end().to_string()
        })
        .collect();

    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}

#[test]
fn model_selection_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();

    chat.config.model = "gpt-5-codex".to_string();
    chat.open_model_popup();

    let popup = render_bottom_popup(&chat, 80);
    assert_snapshot!("model_selection_popup", popup);
}

#[test]
fn model_reasoning_selection_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();

    chat.config.model = "gpt-5-codex".to_string();
    chat.config.model_reasoning_effort = Some(ReasoningEffortConfig::High);

    let presets = builtin_model_presets(None)
        .into_iter()
        .filter(|preset| preset.model == "gpt-5-codex")
        .collect::<Vec<_>>();
    chat.open_reasoning_popup("gpt-5-codex".to_string(), presets);

    let popup = render_bottom_popup(&chat, 80);
    assert_snapshot!("model_reasoning_selection_popup", popup);
}

#[test]
fn reasoning_popup_escape_returns_to_model_popup() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();

    chat.config.model = "gpt-5".to_string();
    chat.open_model_popup();

    let presets = builtin_model_presets(None)
        .into_iter()
        .filter(|preset| preset.model == "gpt-5-codex")
        .collect::<Vec<_>>();
    chat.open_reasoning_popup("gpt-5-codex".to_string(), presets);

    let before_escape = render_bottom_popup(&chat, 80);
    assert!(before_escape.contains("Select Reasoning Level"));

    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    let after_escape = render_bottom_popup(&chat, 80);
    assert!(after_escape.contains("Select Model"));
    assert!(!after_escape.contains("Select Reasoning Level"));
}

#[test]
fn exec_history_extends_previous_when_consecutive() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();

    // 1) Start "ls -la" (List)
    begin_exec(&mut chat, "call-ls", "ls -la");
    assert_snapshot!("exploring_step1_start_ls", active_blob(&chat));

    // 2) Finish "ls -la"
    end_exec(&mut chat, "call-ls", "", "", 0);
    assert_snapshot!("exploring_step2_finish_ls", active_blob(&chat));

    // 3) Start "cat foo.txt" (Read)
    begin_exec(&mut chat, "call-cat-foo", "cat foo.txt");
    assert_snapshot!("exploring_step3_start_cat_foo", active_blob(&chat));

    // 4) Complete "cat foo.txt"
    end_exec(&mut chat, "call-cat-foo", "hello from foo", "", 0);
    assert_snapshot!("exploring_step4_finish_cat_foo", active_blob(&chat));

    // 5) Start & complete "sed -n 100,200p foo.txt" (treated as Read of foo.txt)
    begin_exec(&mut chat, "call-sed-range", "sed -n 100,200p foo.txt");
    end_exec(&mut chat, "call-sed-range", "chunk", "", 0);
    assert_snapshot!("exploring_step5_finish_sed_range", active_blob(&chat));

    // 6) Start & complete "cat bar.txt"
    begin_exec(&mut chat, "call-cat-bar", "cat bar.txt");
    end_exec(&mut chat, "call-cat-bar", "hello from bar", "", 0);
    assert_snapshot!("exploring_step6_finish_cat_bar", active_blob(&chat));
}

#[test]
fn disabled_slash_command_while_task_running_snapshot() {
    // Build a chat widget and simulate an active task
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();
    chat.bottom_pane.set_task_running(true);

    // Dispatch a command that is unavailable while a task runs (e.g., /model)
    chat.dispatch_command(SlashCommand::Model);

    // Drain history and snapshot the rendered error line(s)
    let cells = drain_insert_history(&mut rx);
    assert!(
        !cells.is_empty(),
        "expected an error message history cell to be emitted",
    );
    let blob = lines_to_single_string(cells.last().unwrap());
    assert_snapshot!(blob);
}

#[tokio::test(flavor = "current_thread")]
async fn binary_size_transcript_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Set up a VT100 test terminal to capture ANSI visual output
    let width: u16 = 80;
    let height: u16 = 2000;
    let viewport = Rect::new(0, height - 1, width, 1);
    let backend = VT100Backend::new(width, height);
    let mut terminal = crate::custom_terminal::Terminal::with_options(backend)
        .expect("failed to construct terminal");
    terminal.set_viewport_area(viewport);

    // Replay the recorded session into the widget and collect transcript
    let file = open_fixture("binary-size-log.jsonl");
    let reader = BufReader::new(file);
    let mut transcript = String::new();
    let mut has_emitted_history = false;

    for line in reader.lines() {
        let line = line.expect("read line");
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let Ok(v): Result<serde_json::Value, _> = serde_json::from_str(&line) else {
            continue;
        };
        let Some(dir) = v.get("dir").and_then(|d| d.as_str()) else {
            continue;
        };
        if dir != "to_tui" {
            continue;
        }
        let Some(kind) = v.get("kind").and_then(|k| k.as_str()) else {
            continue;
        };

        match kind {
            "codex_event" => {
                if let Some(payload) = v.get("payload") {
                    let ev: Event =
                        serde_json::from_value(upgrade_event_payload_for_tests(payload.clone()))
                            .expect("parse");
                    let ev = match ev {
                        Event {
                            msg: EventMsg::ExecCommandBegin(e),
                            ..
                        } => {
                            // Re-parse the command
                            let parsed_cmd = codex_core::parse_command::parse_command(&e.command);
                            Event {
                                id: ev.id,
                                msg: EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                                    call_id: e.call_id.clone(),
                                    command: e.command,
                                    cwd: e.cwd,
                                    parsed_cmd: parsed_cmd
                                        .into_iter()
                                        .map(std::convert::Into::into)
                                        .collect(),
                                }),
                            }
                        }
                        _ => ev,
                    };
                    chat.handle_codex_event(ev);
                    while let Ok(app_ev) = rx.try_recv() {
                        if let AppEvent::InsertHistoryCell(cell) = app_ev {
                            let mut lines = cell.display_lines(width);
                            if has_emitted_history
                                && !cell.is_stream_continuation()
                                && !lines.is_empty()
                            {
                                lines.insert(0, "".into());
                            }
                            has_emitted_history = true;
                            transcript.push_str(&lines_to_single_string(&lines));
                            crate::insert_history::insert_history_lines(&mut terminal, lines);
                        }
                    }
                }
            }
            "app_event" => {
                if let Some(variant) = v.get("variant").and_then(|s| s.as_str())
                    && variant == "CommitTick"
                {
                    chat.on_commit_tick();
                    while let Ok(app_ev) = rx.try_recv() {
                        if let AppEvent::InsertHistoryCell(cell) = app_ev {
                            let mut lines = cell.display_lines(width);
                            if has_emitted_history
                                && !cell.is_stream_continuation()
                                && !lines.is_empty()
                            {
                                lines.insert(0, "".into());
                            }
                            has_emitted_history = true;
                            transcript.push_str(&lines_to_single_string(&lines));
                            crate::insert_history::insert_history_lines(&mut terminal, lines);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Build the final VT100 visual by parsing the ANSI stream. Trim trailing spaces per line
    // and drop trailing empty lines so the shape matches the ideal fixture exactly.
    let screen = terminal.backend().vt100().screen();
    let mut lines: Vec<String> = Vec::with_capacity(height as usize);
    for row in 0..height {
        let mut s = String::with_capacity(width as usize);
        for col in 0..width {
            if let Some(cell) = screen.cell(row, col) {
                if let Some(ch) = cell.contents().chars().next() {
                    s.push(ch);
                } else {
                    s.push(' ');
                }
            } else {
                s.push(' ');
            }
        }
        // Trim trailing spaces to match plain text fixture
        lines.push(s.trim_end().to_string());
    }
    while lines.last().is_some_and(std::string::String::is_empty) {
        lines.pop();
    }
    // Consider content only after the last session banner marker. Skip the transient
    // 'thinking' header if present, and start from the first non-empty content line
    // that follows. This keeps the snapshot stable across sessions.
    const MARKER_PREFIX: &str = "To get started, describe a task or try one of these commands:";
    let last_marker_line_idx = lines
        .iter()
        .rposition(|l| l.trim_start().starts_with(MARKER_PREFIX))
        .expect("marker not found in visible output");
    // Prefer the first assistant content line (blockquote '>' prefix) after the marker;
    // fallback to the first non-empty, non-'thinking' line.
    let start_idx = (last_marker_line_idx + 1..lines.len())
        .find(|&idx| lines[idx].trim_start().starts_with('•'))
        .unwrap_or_else(|| {
            (last_marker_line_idx + 1..lines.len())
                .find(|&idx| {
                    let t = lines[idx].trim_start();
                    !t.is_empty() && t != "thinking"
                })
                .expect("no content line found after marker")
        });

    // Snapshot the normalized visible transcript following the banner.
    assert_snapshot!("binary_size_ideal_response", lines[start_idx..].join("\n"));
}

//
// Snapshot test: command approval modal
//
// Synthesizes a Codex ExecApprovalRequest event to trigger the approval modal
// and snapshots the visual output using the ratatui TestBackend.
#[test]
fn approval_modal_exec_snapshot() {
    // Build a chat widget with manual channels to avoid spawning the agent.
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();
    // Ensure policy allows surfacing approvals explicitly (not strictly required for direct event).
    chat.config.approval_policy = AskForApproval::OnRequest;
    // Inject an exec approval request to display the approval modal.
    let ev = ExecApprovalRequestEvent {
        call_id: "call-approve-cmd".into(),
        command: vec!["bash".into(), "-lc".into(), "echo hello world".into()],
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        reason: Some(
            "this is a test reason such as one that would be produced by the model".into(),
        ),
    };
    chat.handle_codex_event(Event {
        id: "sub-approve".into(),
        msg: EventMsg::ExecApprovalRequest(ev),
    });
    // Render to a fixed-size test terminal and snapshot.
    // Call desired_height first and use that exact height for rendering.
    let height = chat.desired_height(80);
    let mut terminal =
        crate::custom_terminal::Terminal::with_options(VT100Backend::new(80, height))
            .expect("create terminal");
    let viewport = Rect::new(0, 0, 80, height);
    terminal.set_viewport_area(viewport);

    terminal
        .draw(|f| f.render_widget_ref(&chat, f.area()))
        .expect("draw approval modal");
    assert!(
        terminal
            .backend()
            .vt100()
            .screen()
            .contents()
            .contains("echo hello world")
    );
    assert_snapshot!(
        "approval_modal_exec",
        terminal.backend().vt100().screen().contents()
    );
}

// Snapshot test: command approval modal without a reason
// Ensures spacing looks correct when no reason text is provided.
#[test]
fn approval_modal_exec_without_reason_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();
    chat.config.approval_policy = AskForApproval::OnRequest;

    let ev = ExecApprovalRequestEvent {
        call_id: "call-approve-cmd-noreason".into(),
        command: vec!["bash".into(), "-lc".into(), "echo hello world".into()],
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        reason: None,
    };
    chat.handle_codex_event(Event {
        id: "sub-approve-noreason".into(),
        msg: EventMsg::ExecApprovalRequest(ev),
    });

    let height = chat.desired_height(80);
    let mut terminal =
        ratatui::Terminal::new(VT100Backend::new(80, height)).expect("create terminal");
    terminal.set_viewport_area(Rect::new(0, 0, 80, height));
    terminal
        .draw(|f| f.render_widget_ref(&chat, f.area()))
        .expect("draw approval modal (no reason)");
    assert_snapshot!(
        "approval_modal_exec_no_reason",
        terminal.backend().vt100().screen().contents()
    );
}

// Snapshot test: patch approval modal
#[test]
fn approval_modal_patch_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();
    chat.config.approval_policy = AskForApproval::OnRequest;

    // Build a small changeset and a reason/grant_root to exercise the prompt text.
    let mut changes = HashMap::new();
    changes.insert(
        PathBuf::from("README.md"),
        FileChange::Add {
            content: "hello\nworld\n".into(),
        },
    );
    let ev = ApplyPatchApprovalRequestEvent {
        call_id: "call-approve-patch".into(),
        changes,
        reason: Some("The model wants to apply changes".into()),
        grant_root: Some(PathBuf::from("/tmp")),
    };
    chat.handle_codex_event(Event {
        id: "sub-approve-patch".into(),
        msg: EventMsg::ApplyPatchApprovalRequest(ev),
    });

    // Render at the widget's desired height and snapshot.
    let height = chat.desired_height(80);
    let mut terminal =
        ratatui::Terminal::new(VT100Backend::new(80, height)).expect("create terminal");
    terminal.set_viewport_area(Rect::new(0, 0, 80, height));
    terminal
        .draw(|f| f.render_widget_ref(&chat, f.area()))
        .expect("draw patch approval modal");
    assert_snapshot!(
        "approval_modal_patch",
        terminal.backend().vt100().screen().contents()
    );
}

#[test]
fn interrupt_restores_queued_messages_into_composer() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual();

    // Simulate a running task to enable queuing of user inputs.
    chat.bottom_pane.set_task_running(true);

    // Queue two user messages while the task is running.
    chat.queued_user_messages
        .push_back(UserMessage::from("first queued".to_string()));
    chat.queued_user_messages
        .push_back(UserMessage::from("second queued".to_string()));
    chat.refresh_queued_user_messages();

    // Deliver a TurnAborted event with Interrupted reason (as if Esc was pressed).
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnAborted(codex_core::protocol::TurnAbortedEvent {
            reason: TurnAbortReason::Interrupted,
        }),
    });

    // Composer should now contain the queued messages joined by newlines, in order.
    assert_eq!(
        chat.bottom_pane.composer_text(),
        "first queued\nsecond queued"
    );

    // Queue should be cleared and no new user input should have been auto-submitted.
    assert!(chat.queued_user_messages.is_empty());
    assert!(
        op_rx.try_recv().is_err(),
        "unexpected outbound op after interrupt"
    );

    // Drain rx to avoid unused warnings.
    let _ = drain_insert_history(&mut rx);
}

#[test]
fn interrupt_prepends_queued_messages_before_existing_composer_text() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual();

    chat.bottom_pane.set_task_running(true);
    chat.bottom_pane
        .set_composer_text("current draft".to_string());

    chat.queued_user_messages
        .push_back(UserMessage::from("first queued".to_string()));
    chat.queued_user_messages
        .push_back(UserMessage::from("second queued".to_string()));
    chat.refresh_queued_user_messages();

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnAborted(codex_core::protocol::TurnAbortedEvent {
            reason: TurnAbortReason::Interrupted,
        }),
    });

    assert_eq!(
        chat.bottom_pane.composer_text(),
        "first queued\nsecond queued\ncurrent draft"
    );
    assert!(chat.queued_user_messages.is_empty());
    assert!(
        op_rx.try_recv().is_err(),
        "unexpected outbound op after interrupt"
    );

    let _ = drain_insert_history(&mut rx);
}

// Snapshot test: ChatWidget at very small heights (idle)
// Ensures overall layout behaves when terminal height is extremely constrained.
#[test]
fn ui_snapshots_small_heights_idle() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let (chat, _rx, _op_rx) = make_chatwidget_manual();
    for h in [1u16, 2, 3] {
        let name = format!("chat_small_idle_h{h}");
        let mut terminal = Terminal::new(TestBackend::new(40, h)).expect("create terminal");
        terminal
            .draw(|f| f.render_widget_ref(&chat, f.area()))
            .expect("draw chat idle");
        assert_snapshot!(name, terminal.backend());
    }
}

// Snapshot test: ChatWidget at very small heights (task running)
// Validates how status + composer are presented within tight space.
#[test]
fn ui_snapshots_small_heights_task_running() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();
    // Activate status line
    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::TaskStarted(TaskStartedEvent {
            model_context_window: None,
        }),
    });
    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
            delta: "**Thinking**".into(),
        }),
    });
    for h in [1u16, 2, 3] {
        let name = format!("chat_small_running_h{h}");
        let mut terminal = Terminal::new(TestBackend::new(40, h)).expect("create terminal");
        terminal
            .draw(|f| f.render_widget_ref(&chat, f.area()))
            .expect("draw chat running");
        assert_snapshot!(name, terminal.backend());
    }
}

// Snapshot test: status widget + approval modal active together
// The modal takes precedence visually; this captures the layout with a running
// task (status indicator active) while an approval request is shown.
#[test]
fn status_widget_and_approval_modal_snapshot() {
    use codex_core::protocol::ExecApprovalRequestEvent;

    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();
    // Begin a running task so the status indicator would be active.
    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::TaskStarted(TaskStartedEvent {
            model_context_window: None,
        }),
    });
    // Provide a deterministic header for the status line.
    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
            delta: "**Analyzing**".into(),
        }),
    });

    // Now show an approval modal (e.g. exec approval).
    let ev = ExecApprovalRequestEvent {
        call_id: "call-approve-exec".into(),
        command: vec!["echo".into(), "hello world".into()],
        cwd: std::path::PathBuf::from("/tmp"),
        reason: Some(
            "this is a test reason such as one that would be produced by the model".into(),
        ),
    };
    chat.handle_codex_event(Event {
        id: "sub-approve-exec".into(),
        msg: EventMsg::ExecApprovalRequest(ev),
    });

    // Render at the widget's desired height and snapshot.
    let height = chat.desired_height(80);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, height))
        .expect("create terminal");
    terminal
        .draw(|f| f.render_widget_ref(&chat, f.area()))
        .expect("draw status + approval modal");
    assert_snapshot!("status_widget_and_approval_modal", terminal.backend());
}

// Snapshot test: status widget active (StatusIndicatorView)
// Ensures the VT100 rendering of the status indicator is stable when active.
#[test]
fn status_widget_active_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();
    // Activate the status indicator by simulating a task start.
    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::TaskStarted(TaskStartedEvent {
            model_context_window: None,
        }),
    });
    // Provide a deterministic header via a bold reasoning chunk.
    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
            delta: "**Analyzing**".into(),
        }),
    });
    // Render and snapshot.
    let height = chat.desired_height(80);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, height))
        .expect("create terminal");
    terminal
        .draw(|f| f.render_widget_ref(&chat, f.area()))
        .expect("draw status widget");
    assert_snapshot!("status_widget_active", terminal.backend());
}

#[test]
fn apply_patch_events_emit_history_cells() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // 1) Approval request -> proposed patch summary cell
    let mut changes = HashMap::new();
    changes.insert(
        PathBuf::from("foo.txt"),
        FileChange::Add {
            content: "hello\n".to_string(),
        },
    );
    let ev = ApplyPatchApprovalRequestEvent {
        call_id: "c1".into(),
        changes,
        reason: None,
        grant_root: None,
    };
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::ApplyPatchApprovalRequest(ev),
    });
    let cells = drain_insert_history(&mut rx);
    assert!(
        cells.is_empty(),
        "expected approval request to surface via modal without emitting history cells"
    );

    let area = Rect::new(0, 0, 80, chat.desired_height(80));
    let mut buf = ratatui::buffer::Buffer::empty(area);
    (&chat).render_ref(area, &mut buf);
    let mut saw_summary = false;
    for y in 0..area.height {
        let mut row = String::new();
        for x in 0..area.width {
            row.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
        }
        if row.contains("foo.txt (+1 -0)") {
            saw_summary = true;
            break;
        }
    }
    assert!(saw_summary, "expected approval modal to show diff summary");

    // 2) Begin apply -> per-file apply block cell (no global header)
    let mut changes2 = HashMap::new();
    changes2.insert(
        PathBuf::from("foo.txt"),
        FileChange::Add {
            content: "hello\n".to_string(),
        },
    );
    let begin = PatchApplyBeginEvent {
        call_id: "c1".into(),
        auto_approved: true,
        changes: changes2,
    };
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::PatchApplyBegin(begin),
    });
    let cells = drain_insert_history(&mut rx);
    assert!(!cells.is_empty(), "expected apply block cell to be sent");
    let blob = lines_to_single_string(cells.last().unwrap());
    assert!(
        blob.contains("Added foo.txt") || blob.contains("Edited foo.txt"),
        "expected single-file header with filename (Added/Edited): {blob:?}"
    );

    // 3) End apply success -> success cell
    let end = PatchApplyEndEvent {
        call_id: "c1".into(),
        stdout: "ok\n".into(),
        stderr: String::new(),
        success: true,
    };
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::PatchApplyEnd(end),
    });
    let cells = drain_insert_history(&mut rx);
    assert!(
        cells.is_empty(),
        "no success cell should be emitted anymore"
    );
}

#[test]
fn apply_patch_manual_approval_adjusts_header() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    let mut proposed_changes = HashMap::new();
    proposed_changes.insert(
        PathBuf::from("foo.txt"),
        FileChange::Add {
            content: "hello\n".to_string(),
        },
    );
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
            call_id: "c1".into(),
            changes: proposed_changes,
            reason: None,
            grant_root: None,
        }),
    });
    drain_insert_history(&mut rx);

    let mut apply_changes = HashMap::new();
    apply_changes.insert(
        PathBuf::from("foo.txt"),
        FileChange::Add {
            content: "hello\n".to_string(),
        },
    );
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
            call_id: "c1".into(),
            auto_approved: false,
            changes: apply_changes,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    assert!(!cells.is_empty(), "expected apply block cell to be sent");
    let blob = lines_to_single_string(cells.last().unwrap());
    assert!(
        blob.contains("Added foo.txt") || blob.contains("Edited foo.txt"),
        "expected apply summary header for foo.txt: {blob:?}"
    );
}

#[test]
fn apply_patch_manual_flow_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    let mut proposed_changes = HashMap::new();
    proposed_changes.insert(
        PathBuf::from("foo.txt"),
        FileChange::Add {
            content: "hello\n".to_string(),
        },
    );
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
            call_id: "c1".into(),
            changes: proposed_changes,
            reason: Some("Manual review required".into()),
            grant_root: None,
        }),
    });
    let history_before_apply = drain_insert_history(&mut rx);
    assert!(
        history_before_apply.is_empty(),
        "expected approval modal to defer history emission"
    );

    let mut apply_changes = HashMap::new();
    apply_changes.insert(
        PathBuf::from("foo.txt"),
        FileChange::Add {
            content: "hello\n".to_string(),
        },
    );
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
            call_id: "c1".into(),
            auto_approved: false,
            changes: apply_changes,
        }),
    });
    let approved_lines = drain_insert_history(&mut rx)
        .pop()
        .expect("approved patch cell");

    assert_snapshot!(
        "apply_patch_manual_flow_history_approved",
        lines_to_single_string(&approved_lines)
    );
}

#[test]
fn apply_patch_approval_sends_op_with_submission_id() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();
    // Simulate receiving an approval request with a distinct submission id and call id
    let mut changes = HashMap::new();
    changes.insert(
        PathBuf::from("file.rs"),
        FileChange::Add {
            content: "fn main(){}\n".into(),
        },
    );
    let ev = ApplyPatchApprovalRequestEvent {
        call_id: "call-999".into(),
        changes,
        reason: None,
        grant_root: None,
    };
    chat.handle_codex_event(Event {
        id: "sub-123".into(),
        msg: EventMsg::ApplyPatchApprovalRequest(ev),
    });

    // Approve via key press 'y'
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

    // Expect a CodexOp with PatchApproval carrying the submission id, not call id
    let mut found = false;
    while let Ok(app_ev) = rx.try_recv() {
        if let AppEvent::CodexOp(Op::PatchApproval { id, decision }) = app_ev {
            assert_eq!(id, "sub-123");
            assert!(matches!(
                decision,
                codex_core::protocol::ReviewDecision::Approved
            ));
            found = true;
            break;
        }
    }
    assert!(found, "expected PatchApproval op to be sent");
}

#[test]
fn apply_patch_full_flow_integration_like() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual();

    // 1) Backend requests approval
    let mut changes = HashMap::new();
    changes.insert(
        PathBuf::from("pkg.rs"),
        FileChange::Add { content: "".into() },
    );
    chat.handle_codex_event(Event {
        id: "sub-xyz".into(),
        msg: EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
            call_id: "call-1".into(),
            changes,
            reason: None,
            grant_root: None,
        }),
    });

    // 2) User approves via 'y' and App receives a CodexOp
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    let mut maybe_op: Option<Op> = None;
    while let Ok(app_ev) = rx.try_recv() {
        if let AppEvent::CodexOp(op) = app_ev {
            maybe_op = Some(op);
            break;
        }
    }
    let op = maybe_op.expect("expected CodexOp after key press");

    // 3) App forwards to widget.submit_op, which pushes onto codex_op_tx
    chat.submit_op(op);
    let forwarded = op_rx
        .try_recv()
        .expect("expected op forwarded to codex channel");
    match forwarded {
        Op::PatchApproval { id, decision } => {
            assert_eq!(id, "sub-xyz");
            assert!(matches!(
                decision,
                codex_core::protocol::ReviewDecision::Approved
            ));
        }
        other => panic!("unexpected op forwarded: {other:?}"),
    }

    // 4) Simulate patch begin/end events from backend; ensure history cells are emitted
    let mut changes2 = HashMap::new();
    changes2.insert(
        PathBuf::from("pkg.rs"),
        FileChange::Add { content: "".into() },
    );
    chat.handle_codex_event(Event {
        id: "sub-xyz".into(),
        msg: EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
            call_id: "call-1".into(),
            auto_approved: false,
            changes: changes2,
        }),
    });
    chat.handle_codex_event(Event {
        id: "sub-xyz".into(),
        msg: EventMsg::PatchApplyEnd(PatchApplyEndEvent {
            call_id: "call-1".into(),
            stdout: String::from("ok"),
            stderr: String::new(),
            success: true,
        }),
    });
}

#[test]
fn apply_patch_untrusted_shows_approval_modal() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual();
    // Ensure approval policy is untrusted (OnRequest)
    chat.config.approval_policy = AskForApproval::OnRequest;

    // Simulate a patch approval request from backend
    let mut changes = HashMap::new();
    changes.insert(
        PathBuf::from("a.rs"),
        FileChange::Add { content: "".into() },
    );
    chat.handle_codex_event(Event {
        id: "sub-1".into(),
        msg: EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
            call_id: "call-1".into(),
            changes,
            reason: None,
            grant_root: None,
        }),
    });

    // Render and ensure the approval modal title is present
    let area = Rect::new(0, 0, 80, 12);
    let mut buf = Buffer::empty(area);
    (&chat).render_ref(area, &mut buf);

    let mut contains_title = false;
    for y in 0..area.height {
        let mut row = String::new();
        for x in 0..area.width {
            row.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
        }
        if row.contains("Would you like to make the following edits?") {
            contains_title = true;
            break;
        }
    }
    assert!(
        contains_title,
        "expected approval modal to be visible with title 'Would you like to make the following edits?'"
    );
}

#[test]
fn apply_patch_request_shows_diff_summary() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Ensure we are in OnRequest so an approval is surfaced
    chat.config.approval_policy = AskForApproval::OnRequest;

    // Simulate backend asking to apply a patch adding two lines to README.md
    let mut changes = HashMap::new();
    changes.insert(
        PathBuf::from("README.md"),
        FileChange::Add {
            // Two lines (no trailing empty line counted)
            content: "line one\nline two\n".into(),
        },
    );
    chat.handle_codex_event(Event {
        id: "sub-apply".into(),
        msg: EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
            call_id: "call-apply".into(),
            changes,
            reason: None,
            grant_root: None,
        }),
    });

    // No history entries yet; the modal should contain the diff summary
    let cells = drain_insert_history(&mut rx);
    assert!(
        cells.is_empty(),
        "expected approval request to render via modal instead of history"
    );

    let area = Rect::new(0, 0, 80, chat.desired_height(80));
    let mut buf = ratatui::buffer::Buffer::empty(area);
    (&chat).render_ref(area, &mut buf);

    let mut saw_header = false;
    let mut saw_line1 = false;
    let mut saw_line2 = false;
    for y in 0..area.height {
        let mut row = String::new();
        for x in 0..area.width {
            row.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
        }
        if row.contains("README.md (+2 -0)") {
            saw_header = true;
        }
        if row.contains("+line one") {
            saw_line1 = true;
        }
        if row.contains("+line two") {
            saw_line2 = true;
        }
        if saw_header && saw_line1 && saw_line2 {
            break;
        }
    }
    assert!(saw_header, "expected modal to show diff header with totals");
    assert!(
        saw_line1 && saw_line2,
        "expected modal to show per-line diff summary"
    );
}

#[test]
fn plan_update_renders_history_cell() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();
    let update = UpdatePlanArgs {
        explanation: Some("Adapting plan".to_string()),
        plan: vec![
            PlanItemArg {
                step: "Explore codebase".into(),
                status: StepStatus::Completed,
            },
            PlanItemArg {
                step: "Implement feature".into(),
                status: StepStatus::InProgress,
            },
            PlanItemArg {
                step: "Write tests".into(),
                status: StepStatus::Pending,
            },
        ],
    };
    chat.handle_codex_event(Event {
        id: "sub-1".into(),
        msg: EventMsg::PlanUpdate(update),
    });
    let cells = drain_insert_history(&mut rx);
    assert!(!cells.is_empty(), "expected plan update cell to be sent");
    let blob = lines_to_single_string(cells.last().unwrap());
    assert!(
        blob.contains("Updated Plan"),
        "missing plan header: {blob:?}"
    );
    assert!(blob.contains("Explore codebase"));
    assert!(blob.contains("Implement feature"));
    assert!(blob.contains("Write tests"));
}

#[test]
fn stream_error_is_rendered_to_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();
    let msg = "stream error: stream disconnected before completion: idle timeout waiting for SSE; retrying 1/5 in 211ms…";
    chat.handle_codex_event(Event {
        id: "sub-1".into(),
        msg: EventMsg::StreamError(StreamErrorEvent {
            message: msg.to_string(),
        }),
    });

    let cells = drain_insert_history(&mut rx);
    assert!(!cells.is_empty(), "expected a history cell for StreamError");
    let blob = lines_to_single_string(cells.last().unwrap());
    assert!(blob.contains('⚠'));
    assert!(blob.contains("stream error:"));
    assert!(blob.contains("idle timeout waiting for SSE"));
}

#[test]
fn multiple_agent_messages_in_single_turn_emit_multiple_headers() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Begin turn
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::TaskStarted(TaskStartedEvent {
            model_context_window: None,
        }),
    });

    // First finalized assistant message
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "First message".into(),
        }),
    });

    // Second finalized assistant message in the same turn
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "Second message".into(),
        }),
    });

    // End turn
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::TaskComplete(TaskCompleteEvent {
            last_agent_message: None,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let combined: String = cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect();
    assert!(
        combined.contains("First message"),
        "missing first message: {combined}"
    );
    assert!(
        combined.contains("Second message"),
        "missing second message: {combined}"
    );
    let first_idx = combined.find("First message").unwrap();
    let second_idx = combined.find("Second message").unwrap();
    assert!(first_idx < second_idx, "messages out of order: {combined}");
}

#[test]
fn final_reasoning_then_message_without_deltas_are_rendered() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // No deltas; only final reasoning followed by final message.
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::AgentReasoning(AgentReasoningEvent {
            text: "I will first analyze the request.".into(),
        }),
    });
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "Here is the result.".into(),
        }),
    });

    // Drain history and snapshot the combined visible content.
    let cells = drain_insert_history(&mut rx);
    let combined = cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_snapshot!(combined);
}

#[test]
fn deltas_then_same_final_message_are_rendered_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Stream some reasoning deltas first.
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
            delta: "I will ".into(),
        }),
    });
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
            delta: "first analyze the ".into(),
        }),
    });
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
            delta: "request.".into(),
        }),
    });
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::AgentReasoning(AgentReasoningEvent {
            text: "request.".into(),
        }),
    });

    // Then stream answer deltas, followed by the exact same final message.
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "Here is the ".into(),
        }),
    });
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "result.".into(),
        }),
    });

    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "Here is the result.".into(),
        }),
    });

    // Snapshot the combined visible content to ensure we render as expected
    // when deltas are followed by the identical final message.
    let cells = drain_insert_history(&mut rx);
    let combined = cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_snapshot!(combined);
}

// Combined visual snapshot using vt100 for history + direct buffer overlay for UI.
// This renders the final visual as seen in a terminal: history above, then a blank line,
// then the exec block, another blank line, the status line, a blank line, and the composer.
#[test]
fn chatwidget_exec_and_status_layout_vt100_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();
    chat.handle_codex_event(Event {
        id: "t1".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent { message: "I’m going to search the repo for where “Change Approved” is rendered to update that view.".into() }),
    });

    chat.handle_codex_event(Event {
        id: "c1".into(),
        msg: EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id: "c1".into(),
            command: vec!["bash".into(), "-lc".into(), "rg \"Change Approved\"".into()],
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            parsed_cmd: vec![
                codex_core::parse_command::ParsedCommand::Search {
                    query: Some("Change Approved".into()),
                    path: None,
                    cmd: "rg \"Change Approved\"".into(),
                }
                .into(),
                codex_core::parse_command::ParsedCommand::Read {
                    name: "diff_render.rs".into(),
                    cmd: "cat diff_render.rs".into(),
                }
                .into(),
            ],
        }),
    });
    chat.handle_codex_event(Event {
        id: "c1".into(),
        msg: EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: "c1".into(),
            stdout: String::new(),
            stderr: String::new(),
            aggregated_output: String::new(),
            exit_code: 0,
            duration: std::time::Duration::from_millis(16000),
            formatted_output: String::new(),
        }),
    });
    chat.handle_codex_event(Event {
        id: "t1".into(),
        msg: EventMsg::TaskStarted(TaskStartedEvent {
            model_context_window: None,
        }),
    });
    chat.handle_codex_event(Event {
        id: "t1".into(),
        msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
            delta: "**Investigating rendering code**".into(),
        }),
    });
    chat.bottom_pane
        .set_composer_text("Summarize recent commits".to_string());

    let width: u16 = 80;
    let ui_height: u16 = chat.desired_height(width);
    let vt_height: u16 = 40;
    let viewport = Rect::new(0, vt_height - ui_height - 1, width, ui_height);

    let backend = VT100Backend::new(width, vt_height);
    let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
    term.set_viewport_area(viewport);

    for lines in drain_insert_history(&mut rx) {
        crate::insert_history::insert_history_lines(&mut term, lines);
    }

    term.draw(|f| {
        (&chat).render_ref(f.area(), f.buffer_mut());
    })
    .unwrap();

    assert_snapshot!(term.backend().vt100().screen().contents());
}

// E2E vt100 snapshot for complex markdown with indented and nested fenced code blocks
#[test]
fn chatwidget_markdown_code_blocks_vt100_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Simulate a final agent message via streaming deltas instead of a single message

    chat.handle_codex_event(Event {
        id: "t1".into(),
        msg: EventMsg::TaskStarted(TaskStartedEvent {
            model_context_window: None,
        }),
    });
    // Build a vt100 visual from the history insertions only (no UI overlay)
    let width: u16 = 80;
    let height: u16 = 50;
    let backend = VT100Backend::new(width, height);
    let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
    // Place viewport at the last line so that history lines insert above it
    term.set_viewport_area(Rect::new(0, height - 1, width, 1));

    // Simulate streaming via AgentMessageDelta in 2-character chunks (no final AgentMessage).
    let source: &str = r#"

    -- Indented code block (4 spaces)
    SELECT *
    FROM "users"
    WHERE "email" LIKE '%@example.com';

````markdown
```sh
printf 'fenced within fenced\n'
```
````

```jsonc
{
  // comment allowed in jsonc
  "path": "C:\\Program Files\\App",
  "regex": "^foo.*(bar)?$"
}
```
"#;

    let mut it = source.chars();
    loop {
        let mut delta = String::new();
        match it.next() {
            Some(c) => delta.push(c),
            None => break,
        }
        if let Some(c2) = it.next() {
            delta.push(c2);
        }

        chat.handle_codex_event(Event {
            id: "t1".into(),
            msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta }),
        });
        // Drive commit ticks and drain emitted history lines into the vt100 buffer.
        loop {
            chat.on_commit_tick();
            let mut inserted_any = false;
            while let Ok(app_ev) = rx.try_recv() {
                if let AppEvent::InsertHistoryCell(cell) = app_ev {
                    let lines = cell.display_lines(width);
                    crate::insert_history::insert_history_lines(&mut term, lines);
                    inserted_any = true;
                }
            }
            if !inserted_any {
                break;
            }
        }
    }

    // Finalize the stream without sending a final AgentMessage, to flush any tail.
    chat.handle_codex_event(Event {
        id: "t1".into(),
        msg: EventMsg::TaskComplete(TaskCompleteEvent {
            last_agent_message: None,
        }),
    });
    for lines in drain_insert_history(&mut rx) {
        crate::insert_history::insert_history_lines(&mut term, lines);
    }

    assert_snapshot!(term.backend().vt100().screen().contents());
}
