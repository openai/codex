use super::*;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
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
use codex_core::protocol::ExecCommandBeginEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::FileChange;
use codex_core::protocol::PatchApplyBeginEvent;
use codex_core::protocol::PatchApplyEndEvent;
use codex_core::protocol::StreamErrorEvent;
use codex_core::protocol::TaskCompleteEvent;
use codex_login::CodexAuth;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::path::PathBuf;
use tokio::sync::mpsc::unbounded_channel;

fn test_config() -> Config {
    // Use base defaults to avoid depending on host state.
    codex_core::config::Config::load_from_base_config_with_overrides(
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
    let height: u16 = 2000;
    let viewport = ratatui::layout::Rect::new(0, height - 1, width, 1);
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

#[tokio::test(flavor = "current_thread")]
async fn helpers_are_available_and_do_not_panic() {
    let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let cfg = test_config();
    let conversation_manager = Arc::new(ConversationManager::with_auth(CodexAuth::from_api_key(
        "test",
    )));
    let mut w = ChatWidget::new(
        cfg,
        conversation_manager,
        crate::tui::FrameRequester::test_dummy(),
        tx,
        None,
        Vec::new(),
        false,
    );
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
        frame_requester: crate::tui::FrameRequester::test_dummy(),
        has_input_focus: true,
        enhanced_keys_supported: false,
        placeholder_text: "Ask Codex to do anything".to_string(),
    });
    let widget = ChatWidget {
        app_event_tx,
        codex_op_tx: op_tx,
        bottom_pane: bottom,
        active_exec_cell: None,
        config: cfg.clone(),
        initial_user_message: None,
        total_token_usage: TokenUsage::default(),
        last_token_usage: TokenUsage::default(),
        stream: StreamController::new(cfg),
        running_commands: HashMap::new(),
        pending_exec_completions: Vec::new(),
        task_complete_pending: false,
        interrupts: InterruptManager::new(),
        needs_redraw: false,
        reasoning_buffer: String::new(),
        full_reasoning_buffer: String::new(),
        session_id: None,
        frame_requester: crate::tui::FrameRequester::test_dummy(),
        show_welcome_banner: true,
        last_history_was_exec: false,
    };
    (widget, rx, op_rx)
}

fn drain_insert_history(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> Vec<Vec<ratatui::text::Line<'static>>> {
    let mut out = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        match ev {
            AppEvent::InsertHistoryLines(lines) => out.push(lines),
            AppEvent::InsertHistoryCell(cell) => out.push(cell.display_lines()),
            _ => {}
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

fn open_fixture(name: &str) -> std::fs::File {
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
fn exec_history_cell_shows_working_then_completed() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Begin command
    chat.handle_codex_event(Event {
        id: "call-1".into(),
        msg: EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id: "call-1".into(),
            command: vec!["bash".into(), "-lc".into(), "echo done".into()],
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            parsed_cmd: vec![
                codex_core::parse_command::ParsedCommand::Unknown {
                    cmd: "echo done".into(),
                }
                .into(),
            ],
        }),
    });

    // End command successfully
    chat.handle_codex_event(Event {
        id: "call-1".into(),
        msg: EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: "call-1".into(),
            stdout: "done".into(),
            stderr: String::new(),
            aggregated_output: "done".into(),
            exit_code: 0,
            duration: std::time::Duration::from_millis(5),
            formatted_output: "done".into(),
        }),
    });

    let cells = drain_insert_history(&mut rx);
    assert_eq!(
        cells.len(),
        1,
        "expected only the completed exec cell to be inserted into history"
    );
    let blob = lines_to_single_string(&cells[0]);
    assert!(
        blob.contains('✓'),
        "expected completed exec cell to show success marker: {blob:?}"
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
    chat.handle_codex_event(Event {
        id: "call-2".into(),
        msg: EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id: "call-2".into(),
            command: vec!["bash".into(), "-lc".into(), "false".into()],
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            parsed_cmd: vec![
                codex_core::parse_command::ParsedCommand::Unknown {
                    cmd: "false".into(),
                }
                .into(),
            ],
        }),
    });

    // End command with failure
    chat.handle_codex_event(Event {
        id: "call-2".into(),
        msg: EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: "call-2".into(),
            stdout: String::new(),
            stderr: "error".into(),
            aggregated_output: "error".into(),
            exit_code: 2,
            duration: std::time::Duration::from_millis(7),
            formatted_output: "".into(),
        }),
    });

    let cells = drain_insert_history(&mut rx);
    assert_eq!(
        cells.len(),
        1,
        "expected only the completed exec cell to be inserted into history"
    );
    let blob = lines_to_single_string(&cells[0]);
    assert!(
        blob.contains('✗'),
        "expected failure marker present: {blob:?}"
    );
    assert!(
        blob.contains("false"),
        "expected command text present: {blob:?}"
    );
}

#[test]
fn exec_history_extends_previous_when_consecutive() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // First command
    chat.handle_codex_event(Event {
        id: "call-a".into(),
        msg: EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id: "call-a".into(),
            command: vec!["bash".into(), "-lc".into(), "echo one".into()],
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            parsed_cmd: vec![
                codex_core::parse_command::ParsedCommand::Unknown {
                    cmd: "echo one".into(),
                }
                .into(),
            ],
        }),
    });
    chat.handle_codex_event(Event {
        id: "call-a".into(),
        msg: EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: "call-a".into(),
            stdout: "one".into(),
            stderr: String::new(),
            aggregated_output: "one".into(),
            exit_code: 0,
            duration: std::time::Duration::from_millis(5),
            formatted_output: "one".into(),
        }),
    });
    let first_cells = drain_insert_history(&mut rx);
    assert_eq!(first_cells.len(), 1, "first exec should insert history");

    // Second command
    chat.handle_codex_event(Event {
        id: "call-b".into(),
        msg: EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id: "call-b".into(),
            command: vec!["bash".into(), "-lc".into(), "echo two".into()],
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            parsed_cmd: vec![
                codex_core::parse_command::ParsedCommand::Unknown {
                    cmd: "echo two".into(),
                }
                .into(),
            ],
        }),
    });
    chat.handle_codex_event(Event {
        id: "call-b".into(),
        msg: EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: "call-b".into(),
            stdout: "two".into(),
            stderr: String::new(),
            aggregated_output: "two".into(),
            exit_code: 0,
            duration: std::time::Duration::from_millis(5),
            formatted_output: "two".into(),
        }),
    });
    let second_cells = drain_insert_history(&mut rx);
    assert_eq!(second_cells.len(), 1, "second exec should extend history");
    let first_blob = lines_to_single_string(&first_cells[0]);
    let second_blob = lines_to_single_string(&second_cells[0]);
    assert!(first_blob.contains('✓'));
    assert!(second_blob.contains("echo two"));
}

#[tokio::test(flavor = "current_thread")]
async fn binary_size_transcript_matches_ideal_fixture() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Set up a VT100 test terminal to capture ANSI visual output
    let width: u16 = 80;
    let height: u16 = 2000;
    let viewport = ratatui::layout::Rect::new(0, height - 1, width, 1);
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = crate::custom_terminal::Terminal::with_options(backend)
        .expect("failed to construct terminal");
    terminal.set_viewport_area(viewport);

    // Replay the recorded session into the widget and collect transcript
    let file = open_fixture("binary-size-log.jsonl");
    let reader = BufReader::new(file);
    let mut transcript = String::new();
    let mut ansi: Vec<u8> = Vec::new();

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
                    chat.handle_codex_event(ev);
                    while let Ok(app_ev) = rx.try_recv() {
                        match app_ev {
                            AppEvent::InsertHistoryLines(lines) => {
                                transcript.push_str(&lines_to_single_string(&lines));
                                crate::insert_history::insert_history_lines_to_writer(
                                    &mut terminal,
                                    &mut ansi,
                                    lines,
                                );
                            }
                            AppEvent::InsertHistoryCell(cell) => {
                                let lines = cell.display_lines();
                                transcript.push_str(&lines_to_single_string(&lines));
                                crate::insert_history::insert_history_lines_to_writer(
                                    &mut terminal,
                                    &mut ansi,
                                    lines,
                                );
                            }
                            _ => {}
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
                        match app_ev {
                            AppEvent::InsertHistoryLines(lines) => {
                                transcript.push_str(&lines_to_single_string(&lines));
                                crate::insert_history::insert_history_lines_to_writer(
                                    &mut terminal,
                                    &mut ansi,
                                    lines,
                                );
                            }
                            AppEvent::InsertHistoryCell(cell) => {
                                let lines = cell.display_lines();
                                transcript.push_str(&lines_to_single_string(&lines));
                                crate::insert_history::insert_history_lines_to_writer(
                                    &mut terminal,
                                    &mut ansi,
                                    lines,
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Read the ideal fixture as-is
    let mut f = open_fixture("ideal-binary-response.txt");
    let mut ideal = String::new();
    f.read_to_string(&mut ideal)
        .expect("read ideal-binary-response.txt");
    // Normalize line endings for Windows vs. Unix checkouts
    let ideal = ideal.replace("\r\n", "\n");
    let ideal_first_line = ideal
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .to_string();

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
    // Compare only after the last session banner marker. Skip the transient
    // 'thinking' header if present, and start from the first non-empty line
    // of content that follows.
    const MARKER_PREFIX: &str = ">_ You are using OpenAI Codex in ";
    let last_marker_line_idx = lines
        .iter()
        .rposition(|l| l.starts_with(MARKER_PREFIX))
        .expect("marker not found in visible output");
    // Anchor to the first ideal line if present; otherwise use heuristics.
    let start_idx = (last_marker_line_idx + 1..lines.len())
        .find(|&idx| lines[idx].trim_start() == ideal_first_line)
        .or_else(|| {
            // Prefer the first assistant content line (blockquote '>' prefix) after the marker.
            (last_marker_line_idx + 1..lines.len())
                .find(|&idx| lines[idx].trim_start().starts_with('>'))
        })
        .unwrap_or_else(|| {
            // Fallback: first non-empty, non-'thinking' line
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

    // Normalize: drop a leading 'thinking' line if present in either side to
    // avoid coupling to whether the reasoning header is rendered in history.
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
    let ideal = drop_leading_thinking(&ideal);

    // Normalize: strip leading Markdown blockquote markers ('>' or '> ') which
    // may be present in rendered transcript lines but not in the ideal text.
    fn strip_blockquotes(s: &str) -> String {
        s.lines()
            .map(|l| {
                l.strip_prefix("> ")
                    .or_else(|| l.strip_prefix('>'))
                    .unwrap_or(l)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    let visible_after = strip_blockquotes(&visible_after);
    let ideal = strip_blockquotes(&ideal);

    // Optionally update the fixture when env var is set
    if std::env::var("UPDATE_IDEAL").as_deref() == Ok("1") {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push("fixtures");
        p.push("ideal-binary-response.txt");
        std::fs::write(&p, &visible_after).expect("write updated ideal fixture");
        return;
    }

    // Exact equality with pretty diff on failure
    assert_eq!(visible_after, ideal);
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
        blob.contains("proposed patch"),
        "missing proposed patch header: {blob:?}"
    );

    // 2) Begin apply -> applying patch cell
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
    assert!(!cells.is_empty(), "expected applying patch cell to be sent");
    let blob = lines_to_single_string(cells.last().unwrap());
    assert!(
        blob.contains("Applying patch"),
        "missing applying patch header: {blob:?}"
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
    assert!(!cells.is_empty(), "expected applied patch cell to be sent");
    let blob = lines_to_single_string(cells.last().unwrap());
    assert!(
        blob.contains("Applied patch"),
        "missing applied patch header: {blob:?}"
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
    chat.config.approval_policy = codex_core::protocol::AskForApproval::OnRequest;

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
    let area = ratatui::layout::Rect::new(0, 0, 80, 12);
    let mut buf = ratatui::buffer::Buffer::empty(area);
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
    chat.config.approval_policy = codex_core::protocol::AskForApproval::OnRequest;

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
        blob.contains("proposed patch to 1 file (+2 -0)"),
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
        blob.contains("Update plan"),
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
    assert!(blob.contains("⚠ "));
    assert!(blob.contains("stream error:"));
    assert!(blob.contains("idle timeout waiting for SSE"));
}

#[test]
fn headers_emitted_on_stream_begin_for_answer_and_not_for_reasoning() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Answer: no header until a newline commit
    chat.handle_codex_event(Event {
        id: "sub-a".into(),
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "Hello".into(),
        }),
    });
    let mut saw_codex_pre = false;
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::InsertHistoryLines(lines) = ev {
            let s = lines
                .iter()
                .flat_map(|l| l.spans.iter())
                .map(|sp| sp.content.clone())
                .collect::<Vec<_>>()
                .join("");
            if s.contains("codex") {
                saw_codex_pre = true;
                break;
            }
        }
    }
    assert!(
        !saw_codex_pre,
        "answer header should not be emitted before first newline commit"
    );

    // Newline arrives, then header is emitted
    chat.handle_codex_event(Event {
        id: "sub-a".into(),
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "!\n".into(),
        }),
    });
    chat.on_commit_tick();
    let mut saw_codex_post = false;
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::InsertHistoryLines(lines) = ev {
            let s = lines
                .iter()
                .flat_map(|l| l.spans.iter())
                .map(|sp| sp.content.clone())
                .collect::<Vec<_>>()
                .join("");
            if s.contains("codex") {
                saw_codex_post = true;
                break;
            }
        }
    }
    assert!(
        saw_codex_post,
        "expected 'codex' header to be emitted after first newline commit"
    );

    // Reasoning: do NOT emit a history header; status text is updated instead
    let (mut chat2, mut rx2, _op_rx2) = make_chatwidget_manual();
    chat2.handle_codex_event(Event {
        id: "sub-b".into(),
        msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
            delta: "Thinking".into(),
        }),
    });
    let mut saw_thinking = false;
    while let Ok(ev) = rx2.try_recv() {
        if let AppEvent::InsertHistoryLines(lines) = ev {
            let s = lines
                .iter()
                .flat_map(|l| l.spans.iter())
                .map(|sp| sp.content.clone())
                .collect::<Vec<_>>()
                .join("");
            if s.contains("thinking") {
                saw_thinking = true;
                break;
            }
        }
    }
    assert!(
        !saw_thinking,
        "reasoning deltas should not emit history headers"
    );
}

#[test]
fn multiple_agent_messages_in_single_turn_emit_multiple_headers() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual();

    // Begin turn
    chat.handle_codex_event(Event {
        id: "s1".into(),
        msg: EventMsg::TaskStarted,
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
    let mut header_count = 0usize;
    let mut combined = String::new();
    for lines in &cells {
        for l in lines {
            for sp in &l.spans {
                let s = &sp.content;
                if s == "codex" {
                    header_count += 1;
                }
                combined.push_str(s);
            }
            combined.push('\n');
        }
    }
    assert_eq!(
        header_count,
        2,
        "expected two 'codex' headers for two AgentMessage events in one turn; cells={:?}",
        cells.len()
    );
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
