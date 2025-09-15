use super::*;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigToml;
use codex_core::plan_tool::PlanItemArg;
use codex_core::plan_tool::StepStatus;
use codex_core::plan_tool::UpdatePlanArgs;
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
use codex_core::protocol::FileChange;
use codex_core::protocol::InputMessageKind;
use codex_core::protocol::PatchApplyBeginEvent;
use codex_core::protocol::PatchApplyEndEvent;
use codex_core::protocol::StreamErrorEvent;
use codex_core::protocol::TaskCompleteEvent;
use codex_core::protocol::TaskStartedEvent;
use codex_protocol::mcp_protocol::ConversationId;
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
fn final_answer_without_newline_is_flushed_immediately() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Set up a VT100 test terminal to capture ANSI visual output
    let width: u16 = 80;
    // Increased height to keep the initial banner/help lines in view even if
    // the session renders an extra header line or minor layout changes occur.
    let height: u16 = 2500;
    let viewport = Rect::new(0, height - 1, width, 1);
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = crate::custom_terminal::Terminal::with_options(backend)
        .expect("failed to construct terminal");
    terminal.set_viewport_area(viewport);

    // Simulate a streaming answer without any newline characters.
    chat.handle_codex_event(Event {
        id: "sub-a".into(),
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "Hi! How can I help with codex-rs or anything else today?".into(),
        }),
    });

    // Now simulate the final AgentMessage which should flush the pending line immediately.
    chat.handle_codex_event(Event {
        id: "sub-a".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "Hi! How can I help with codex-rs or anything else today?".into(),
        }),
    });

    // Drain history insertions and verify the final line is present.
    let cells = drain_insert_history(&mut rx);
    assert!(
        cells.iter().any(|lines| {
            let s = lines
                .iter()
                .flat_map(|l| l.spans.iter())
                .map(|sp| sp.content.clone())
                .collect::<String>();
            s.contains("codex")
        }),
        "expected 'codex' header to be emitted",
    );
    let found_final = cells.iter().any(|lines| {
        let s = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|sp| sp.content.clone())
            .collect::<String>();
        s.contains("Hi! How can I help with codex-rs or anything else today?")
    });
    assert!(
        found_final,
        "expected final answer text to be flushed to history"
    );
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
        active_exec_cell: None,
        config: cfg.clone(),
        auth_manager,
        session_header: SessionHeader::new(cfg.model.clone()),
        initial_user_message: None,
        token_info: None,
        stream: StreamController::new(cfg),
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

    // Snapshot the Proposed Command cell emitted into history
    let proposed = drain_insert_history(&mut rx)
        .pop()
        .expect("expected proposed command cell");
    assert_snapshot!(
        "exec_approval_history_proposed_short",
        lines_to_single_string(&proposed)
    );

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

    // Multiline command: should render proposed command fully in history with prefixes
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
    let proposed_multi = drain_insert_history(&mut rx)
        .pop()
        .expect("expected proposed multiline command cell");
    assert_snapshot!(
        "exec_approval_history_proposed_multiline",
        lines_to_single_string(&proposed_multi)
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
    drain_insert_history(&mut rx); // proposed cell not needed for this assertion
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
        .active_exec_cell
        .as_ref()
        .expect("active exec cell present")
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
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = crate::custom_terminal::Terminal::with_options(backend)
        .expect("failed to construct terminal");
    terminal.set_viewport_area(viewport);

    // Replay the recorded session into the widget and collect transcript
    let file = open_fixture("binary-size-log.jsonl");
    let reader = BufReader::new(file);
    let mut transcript = String::new();
    let mut ansi: Vec<u8> = Vec::new();
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
                                    parsed_cmd: parsed_cmd.into_iter().map(|c| c.into()).collect(),
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
                            crate::insert_history::insert_history_lines_to_writer(
                                &mut terminal,
                                &mut ansi,
                                lines,
                            );
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
                            crate::insert_history::insert_history_lines_to_writer(
                                &mut terminal,
                                &mut ansi,
                                lines,
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Build the final VT100 visual by parsing the ANSI stream. Trim trailing spaces per line
    // and drop trailing empty lines so the shape matches the ideal fixture exactly.
    let mut parser = vt100::Parser::new(height, width, 0);
    parser.process(&ansi);
    let mut lines: Vec<String> = Vec::with_capacity(height as usize);
    for row in 0..height {
        let mut s = String::with_capacity(width as usize);
        for col in 0..width {
            if let Some(cell) = parser.screen().cell(row, col) {
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
    while lines.last().is_some_and(|l| l.is_empty()) {
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
        .find(|&idx| lines[idx].trim_start().starts_with('>'))
        .unwrap_or_else(|| {
            (last_marker_line_idx + 1..lines.len())
                .find(|&idx| {
                    let t = lines[idx].trim_start();
                    !t.is_empty() && t != "thinking"
                })
                .expect("no content line found after marker")
        });

    let mut compare_lines: Vec<String> = Vec::new();
    // Ensure the first line is trimmed-left to match the fixture shape.
    compare_lines.push(lines[start_idx].trim_start().to_string());
    compare_lines.extend(lines[(start_idx + 1)..].iter().cloned());
    let visible_after = compare_lines.join("\n");

    // Normalize: drop a leading 'thinking' line if present to avoid coupling
    // to whether the reasoning header is rendered in history.
    fn drop_leading_thinking(s: &str) -> String {
        let mut it = s.lines();
        let first = it.next();
        let rest = it.collect::<Vec<_>>().join("\n");
        if first.is_some_and(|l| l.trim() == "thinking") {
            rest
        } else {
            s.to_string()
        }
    }
    let visible_after = drop_leading_thinking(&visible_after);

    // Snapshot the normalized visible transcript following the banner.
    assert_snapshot!("binary_size_ideal_response", visible_after);
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
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, height))
        .expect("create terminal");
    terminal
        .draw(|f| f.render_widget_ref(&chat, f.area()))
        .expect("draw approval modal");
    assert_snapshot!("approval_modal_exec", terminal.backend());
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
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, height))
        .expect("create terminal");
    terminal
        .draw(|f| f.render_widget_ref(&chat, f.area()))
        .expect("draw approval modal (no reason)");
    assert_snapshot!("approval_modal_exec_no_reason", terminal.backend());
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
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, height))
        .expect("create terminal");
    terminal
        .draw(|f| f.render_widget_ref(&chat, f.area()))
        .expect("draw patch approval modal");
    assert_snapshot!("approval_modal_patch", terminal.backend());
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
    assert!(!cells.is_empty(), "expected pending patch cell to be sent");
    let blob = lines_to_single_string(cells.last().unwrap());
    assert!(
        blob.contains("Proposed Change"),
        "missing proposed change header: {blob:?}"
    );

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
        blob.contains("Change Approved foo.txt"),
        "expected change approved summary: {blob:?}"
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
    let proposed_lines = drain_insert_history(&mut rx)
        .pop()
        .expect("proposed patch cell");

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
        "apply_patch_manual_flow_history_proposed",
        lines_to_single_string(&proposed_lines)
    );
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
        if row.contains("Apply changes?") {
            contains_title = true;
            break;
        }
    }
    assert!(
        contains_title,
        "expected approval modal to be visible with title 'Apply changes?'"
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

    // Drain history insertions and verify the diff summary is present
    let cells = drain_insert_history(&mut rx);
    assert!(
        !cells.is_empty(),
        "expected a history cell with the proposed patch summary"
    );
    let blob = lines_to_single_string(cells.last().unwrap());

    // Header should summarize totals
    assert!(
        blob.contains("Proposed Change README.md (+2 -0)"),
        "missing or incorrect diff header: {blob:?}"
    );

    // Per-file summary line should include the file path and counts
    assert!(
        blob.contains("README.md"),
        "missing per-file diff summary: {blob:?}"
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
    // Setup identical scenario
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();
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
    chat.handle_codex_event(Event {
        id: "t1".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent { message: "I’m going to search the repo for where “Change Approved” is rendered to update that view.".into() }),
    });

    // Dimensions
    let width: u16 = 80;
    let ui_height: u16 = chat.desired_height(width);
    let vt_height: u16 = 40;
    let viewport = Rect::new(0, vt_height - ui_height, width, ui_height);

    // Use TestBackend for the terminal (no real ANSI emitted by drawing),
    // but capture VT100 escape stream for history insertion with a separate writer.
    let backend = ratatui::backend::TestBackend::new(width, vt_height);
    let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
    term.set_viewport_area(viewport);

    // 1) Apply any pending history insertions by emitting ANSI to a buffer via insert_history_lines_to_writer
    let mut ansi: Vec<u8> = Vec::new();
    for lines in drain_insert_history(&mut rx) {
        crate::insert_history::insert_history_lines_to_writer(&mut term, &mut ansi, lines);
    }

    // 2) Render the ChatWidget UI into an off-screen buffer using WidgetRef directly
    let mut ui_buf = Buffer::empty(viewport);
    (&chat).render_ref(viewport, &mut ui_buf);

    // 3) Build VT100 visual from the captured ANSI
    let mut parser = vt100::Parser::new(vt_height, width, 0);
    parser.process(&ansi);
    let mut vt_lines: Vec<String> = (0..vt_height)
        .map(|row| {
            let mut s = String::with_capacity(width as usize);
            for col in 0..width {
                if let Some(cell) = parser.screen().cell(row, col) {
                    if let Some(ch) = cell.contents().chars().next() {
                        s.push(ch);
                    } else {
                        s.push(' ');
                    }
                } else {
                    s.push(' ');
                }
            }
            s.trim_end().to_string()
        })
        .collect();

    // 4) Overlay UI buffer content into the viewport region of the VT output
    for rel_y in 0..viewport.height {
        let y = viewport.y + rel_y;
        let mut line = String::with_capacity(width as usize);
        for x in 0..viewport.width {
            let ch = ui_buf[(viewport.x + x, viewport.y + rel_y)]
                .symbol()
                .chars()
                .next()
                .unwrap_or(' ');
            line.push(ch);
        }
        vt_lines[y as usize] = line.trim_end().to_string();
    }

    let visual = vt_lines.join("\n");
    assert_snapshot!(visual);
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
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
    // Place viewport at the last line so that history lines insert above it
    term.set_viewport_area(Rect::new(0, height - 1, width, 1));

    let mut ansi: Vec<u8> = Vec::new();

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
                    crate::insert_history::insert_history_lines_to_writer(
                        &mut term, &mut ansi, lines,
                    );
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
        crate::insert_history::insert_history_lines_to_writer(&mut term, &mut ansi, lines);
    }

    let mut parser = vt100::Parser::new(height, width, 0);
    parser.process(&ansi);

    let mut vt_lines: Vec<String> = (0..height)
        .map(|row| {
            let mut s = String::with_capacity(width as usize);
            for col in 0..width {
                if let Some(cell) = parser.screen().cell(row, col) {
                    if let Some(ch) = cell.contents().chars().next() {
                        s.push(ch);
                    } else {
                        s.push(' ');
                    }
                } else {
                    s.push(' ');
                }
            }
            s.trim_end().to_string()
        })
        .collect();

    // Compact trailing blank rows for a stable snapshot
    while matches!(vt_lines.last(), Some(l) if l.trim().is_empty()) {
        vt_lines.pop();
    }
    let visual = vt_lines.join("\n");
    assert_snapshot!(visual);
}
