#![cfg(feature = "vt100-tests")]

use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::channel;

use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigToml;
use codex_core::protocol::Event as CodexEvent;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

fn normalize_text(s: &str) -> String {
    // Remove inline code backticks and normalize curly quotes to straight quotes.
    let no_ticks = s.replace('`', "");
    no_ticks
        .replace(['\u{2019}', '\u{2018}'], "'") // left single quote
        .replace(['\u{201C}', '\u{201D}'], "\"") // right double quote
}

// Common test helpers are provided by `crate::test_utils`.

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
    // 2) Fallback to parent (workspace root) — current repo layout
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

// Drive the real ChatWidget from recorded events and render using the
// production history insertion pipeline into a vt100 parser. This is a
// faithful replay of a simple conversation, and should currently fail
// because the final model message is cut off in the UI.
#[tokio::test(flavor = "current_thread")]
async fn vt100_replay_hello_conversation_from_log() {
    // Terminal: make it large enough so the entire short conversation fits on-screen.
    let width: u16 = 100;
    let height: u16 = 40;
    let viewport = Rect::new(0, height - 1, width, 1);
    let backend = TestBackend::new(width, height);
    let mut terminal = crate::custom_terminal::Terminal::with_options(backend)
        .expect("failed to construct terminal");
    terminal.set_viewport_area(viewport);

    // App event channel to capture InsertHistory from the widget.
    let (tx_raw, rx): (std::sync::mpsc::Sender<AppEvent>, Receiver<AppEvent>) = channel();
    let app_sender = AppEventSender::new(tx_raw);

    // Light-weight config that does not depend on host state.
    let cfg: Config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("config");

    // Construct the real ChatWidget. Provide an initial prompt so the user
    // message appears when SessionConfigured is received.
    let mut widget = crate::chatwidget::ChatWidget::new(
        cfg,
        app_sender.clone(),
        Some("hello".to_string()),
        Vec::new(),
        false,
    );

    // Collected ANSI bytes emitted by the history insertion pipeline.
    let mut ansi: Vec<u8> = Vec::new();

    // Replay the recorded session.
    // Resolve the log path (works whether CWD is workspace root or crate dir).
    let file = open_fixture("hello-log.jsonl");
    let reader = BufReader::new(file);
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
                    let ev: CodexEvent =
                        serde_json::from_value(payload.clone()).expect("parse codex event");
                    widget.handle_codex_event(ev);
                    crate::test_utils::drain_insert_history(&mut terminal, &rx, &mut ansi);
                }
            }
            "app_event" => {
                if let Some(variant) = v.get("variant").and_then(|s| s.as_str()) {
                    match variant {
                        "CommitTick" => {
                            widget.on_commit_tick();
                            crate::test_utils::drain_insert_history(&mut terminal, &rx, &mut ansi);
                        }
                        _ => { /* ignored in this replay */ }
                    }
                }
            }
            _ => { /* ignore other kinds */ }
        }
    }

    // Parse the ANSI stream with vt100 to reconstruct the final screen.
    let mut parser = vt100::Parser::new(height, width, 0);
    parser.process(&ansi);
    let mut visible = String::new();
    for row in 0..height {
        for col in 0..width {
            if let Some(cell) = parser.screen().cell(row, col) {
                if let Some(ch) = cell.contents().chars().next() {
                    visible.push(ch);
                } else {
                    visible.push(' ');
                }
            } else {
                visible.push(' ');
            }
        }
        visible.push('\n');
    }

    // Expect the full conversation segments. This currently fails because the
    // model answer gets cut off and does not render.
    assert!(
        visible.contains("user"),
        "missing user header on screen\n{visible}"
    );
    assert!(
        visible.contains("hello"),
        "missing user text on screen\n{visible}"
    );
    assert!(
        visible.contains("thinking"),
        "missing thinking header on screen\n{visible}"
    );
    assert!(
        visible.contains("Responding to user greeting"),
        "missing reasoning summary on screen\n{visible}"
    );
    assert!(
        visible.contains("codex"),
        "missing assistant header on screen\n{visible}"
    );
    assert!(
        visible.contains("Hi! How can I help with codex-rs or anything else today?"),
        "assistant greeting was cut off or missing\n{visible}"
    );
}

// Replay a more complex markdown session and verify headers render for each
// assistant response. Specifically, ensure the second request shows the
// 'codex' header before the assistant's message.
#[tokio::test(flavor = "current_thread")]
async fn vt100_replay_markdown_session_from_log() {
    // Terminal large enough to fit the visible conversation segments
    let width: u16 = 110;
    let height: u16 = 50;
    let viewport = Rect::new(0, height - 1, width, 1);
    let backend = TestBackend::new(width, height);
    let mut terminal = crate::custom_terminal::Terminal::with_options(backend)
        .expect("failed to construct terminal");
    terminal.set_viewport_area(viewport);

    let (tx_raw, rx): (std::sync::mpsc::Sender<AppEvent>, Receiver<AppEvent>) = channel();
    let app_sender = AppEventSender::new(tx_raw);

    let cfg: Config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("config");

    let mut widget =
        crate::chatwidget::ChatWidget::new(cfg, app_sender.clone(), None, Vec::new(), false);

    let mut ansi: Vec<u8> = Vec::new();

    // Open the markdown session log relative to workspace root or crate dir.
    let file = open_fixture("markdown-session.jsonl");
    let reader = BufReader::new(file);

    // Track per-turn counts and expected/actual content for full-answer checks
    let mut codex_headers_per_turn: Vec<usize> = Vec::new();
    let mut current_turn_index: Option<usize> = None;
    let mut expected_full_answer_per_turn: Vec<Option<String>> = Vec::new();
    let mut transcript_per_turn: Vec<String> = Vec::new();

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
                    let ev: CodexEvent =
                        serde_json::from_value(payload.clone()).expect("parse codex event");
                    // Track task boundaries and expected answers.
                    if let CodexEvent { msg, .. } = &ev {
                        if matches!(msg, codex_core::protocol::EventMsg::TaskStarted) {
                            codex_headers_per_turn.push(0);
                            expected_full_answer_per_turn.push(None);
                            transcript_per_turn.push(String::new());
                            current_turn_index = Some(codex_headers_per_turn.len() - 1);
                        }
                        if let codex_core::protocol::EventMsg::AgentMessage(m) = msg {
                            if let Some(idx) = current_turn_index {
                                expected_full_answer_per_turn[idx] = Some(m.message.clone());
                            }
                        }
                        if let codex_core::protocol::EventMsg::TaskComplete(tc) = msg {
                            if let Some(idx) = current_turn_index {
                                if tc.last_agent_message.is_some() {
                                    expected_full_answer_per_turn[idx] =
                                        tc.last_agent_message.clone();
                                }
                            }
                        }
                    }
                    widget.handle_codex_event(ev);
                    // Drain and render; count 'codex' header insertions for current turn.
                    while let Ok(app_ev) = rx.try_recv() {
                        if let AppEvent::InsertHistory(lines) = app_ev {
                            if let Some(idx) = current_turn_index {
                                let texts = crate::test_utils::lines_to_plain_strings(&lines);
                                let turn_count =
                                    texts.iter().filter(|s| s.as_str() == "codex").count();
                                codex_headers_per_turn[idx] += turn_count;
                                crate::test_utils::append_lines_to_transcript(
                                    &lines,
                                    &mut transcript_per_turn[idx],
                                );
                            }
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
                if let Some(variant) = v.get("variant").and_then(|s| s.as_str()) {
                    if variant == "CommitTick" {
                        widget.on_commit_tick();
                        while let Ok(app_ev) = rx.try_recv() {
                            if let AppEvent::InsertHistory(lines) = app_ev {
                                if let Some(idx) = current_turn_index {
                                    let texts =
                                        crate::test_utils::lines_to_plain_strings(&lines);
                                    let turn_count = texts
                                        .iter()
                                        .filter(|s| s.as_str() == "codex")
                                        .count();
                                    codex_headers_per_turn[idx] += turn_count;
                                    crate::test_utils::append_lines_to_transcript(
                                        &lines,
                                        &mut transcript_per_turn[idx],
                                    );
                                }
                                crate::insert_history::insert_history_lines_to_writer(
                                    &mut terminal,
                                    &mut ansi,
                                    lines,
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Reconstruct the final screen
    let mut parser = vt100::Parser::new(height, width, 0);
    parser.process(&ansi);
    let mut visible = String::new();
    for row in 0..height {
        for col in 0..width {
            if let Some(cell) = parser.screen().cell(row, col) {
                if let Some(ch) = cell.contents().chars().next() {
                    visible.push(ch);
                } else {
                    visible.push(' ');
                }
            } else {
                visible.push(' ');
            }
        }
        visible.push('\n');
    }

    // Assert exactly one 'codex' header per turn for the first two turns.
    assert!(
        codex_headers_per_turn.len() >= 2,
        "expected at least two turns; counts = {codex_headers_per_turn:?}"
    );
    assert_eq!(
        codex_headers_per_turn[0], 1,
        "first turn should have exactly one 'codex' header; counts = {codex_headers_per_turn:?}"
    );
    assert_eq!(
        codex_headers_per_turn[1], 1,
        "second turn should have exactly one 'codex' header; counts = {codex_headers_per_turn:?}"
    );

    // Verify every turn's transcript contains the expected full answer.
    for (i, maybe_expected) in expected_full_answer_per_turn.iter().enumerate() {
        if let Some(expected) = maybe_expected {
            let exp = normalize_text(expected);
            let got = normalize_text(&transcript_per_turn[i]);
            assert!(
                got.contains(&exp),
                "turn {} transcript missing expected full answer.\nexpected: {:?}\ntranscript: {}",
                i,
                expected,
                transcript_per_turn[i]
            );
        }
    }
}

// Replay a longer markdown session with multiple turns and assert a 'codex' header
// is emitted exactly once per turn, especially the second turn which previously
// failed to show the header.
#[tokio::test(flavor = "current_thread")]
async fn vt100_replay_longer_markdown_session_from_log() {
    let width: u16 = 120;
    let height: u16 = 55;
    let viewport = Rect::new(0, height - 1, width, 1);
    let backend = TestBackend::new(width, height);
    let mut terminal = crate::custom_terminal::Terminal::with_options(backend)
        .expect("failed to construct terminal");
    terminal.set_viewport_area(viewport);

    let (tx_raw, rx): (std::sync::mpsc::Sender<AppEvent>, Receiver<AppEvent>) = channel();
    let app_sender = AppEventSender::new(tx_raw);

    let cfg: Config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("config");

    let mut widget =
        crate::chatwidget::ChatWidget::new(cfg, app_sender.clone(), None, Vec::new(), false);

    let mut ansi: Vec<u8> = Vec::new();

    let file = open_fixture("longer-markdown-session.jsonl");
    let reader = BufReader::new(file);

    let mut codex_headers_per_turn: Vec<usize> = Vec::new();
    let mut first_non_header_line_per_turn: Vec<Option<String>> = Vec::new();
    let mut saw_codex_header_in_turn: Vec<bool> = Vec::new();
    let mut header_batched_with_content: Vec<bool> = Vec::new();
    let mut current_turn_index: Option<usize> = None;

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
                    let ev: CodexEvent =
                        serde_json::from_value(payload.clone()).expect("parse codex event");
                    if let CodexEvent { msg, .. } = &ev {
                        if matches!(msg, codex_core::protocol::EventMsg::TaskStarted) {
                            codex_headers_per_turn.push(0);
                            first_non_header_line_per_turn.push(None);
                            saw_codex_header_in_turn.push(false);
                            header_batched_with_content.push(false);
                            current_turn_index = Some(codex_headers_per_turn.len() - 1);
                        }
                    }
                    widget.handle_codex_event(ev);
                    while let Ok(app_ev) = rx.try_recv() {
                        if let AppEvent::InsertHistory(lines) = app_ev {
                            if let Some(idx) = current_turn_index {
                                let texts = crate::test_utils::lines_to_plain_strings(&lines);
                                let mut turn_count = 0usize;
                                for (i, s) in texts.iter().enumerate() {
                                    if s == "codex" {
                                        turn_count += 1;
                                        saw_codex_header_in_turn[idx] = true;
                                        if texts
                                            .iter()
                                            .skip(i + 1)
                                            .any(|t| !t.trim().is_empty())
                                        {
                                            header_batched_with_content[idx] = true;
                                        }
                                    } else if saw_codex_header_in_turn[idx]
                                        && !s.trim().is_empty()
                                        && first_non_header_line_per_turn[idx].is_none()
                                    {
                                        first_non_header_line_per_turn[idx] = Some(s.clone());
                                    }
                                }
                                codex_headers_per_turn[idx] += turn_count;
                            }
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
                if let Some(variant) = v.get("variant").and_then(|s| s.as_str()) {
                    if variant == "CommitTick" {
                        widget.on_commit_tick();
                        while let Ok(app_ev) = rx.try_recv() {
                            if let AppEvent::InsertHistory(lines) = app_ev {
                                if let Some(idx) = current_turn_index {
                                    let texts =
                                        crate::test_utils::lines_to_plain_strings(&lines);
                                    let mut turn_count = 0usize;
                                    for (i, s) in texts.iter().enumerate() {
                                        if s == "codex" {
                                            turn_count += 1;
                                            saw_codex_header_in_turn[idx] = true;
                                            if texts
                                                .iter()
                                                .skip(i + 1)
                                                .any(|t| !t.trim().is_empty())
                                            {
                                                header_batched_with_content[idx] = true;
                                            }
                                        } else if saw_codex_header_in_turn[idx]
                                            && !s.trim().is_empty()
                                            && first_non_header_line_per_turn[idx].is_none()
                                        {
                                            first_non_header_line_per_turn[idx] =
                                                Some(s.clone());
                                        }
                                    }
                                    codex_headers_per_turn[idx] += turn_count;
                                }
                                crate::insert_history::insert_history_lines_to_writer(
                                    &mut terminal,
                                    &mut ansi,
                                    lines,
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Expect at least two turns with exactly one 'codex' header each; specifically the second
    // turn previously missed the header.
    assert!(
        codex_headers_per_turn.len() >= 2,
        "expected at least two turns; counts = {codex_headers_per_turn:?}"
    );
    assert_eq!(
        codex_headers_per_turn[0], 1,
        "first turn should have exactly one 'codex' header; counts = {codex_headers_per_turn:?}"
    );
    assert_eq!(
        codex_headers_per_turn[1], 1,
        "second turn should have exactly one 'codex' header; counts = {codex_headers_per_turn:?}"
    );

    // Additionally, ensure the header and the first content are batched together in the same
    // history insertion for turn 2, so content is not separated or cut off.
    assert!(
        header_batched_with_content[1],
        "header and first content were not batched together for turn 2; counts = {:?}, first_line = {:?}",
        codex_headers_per_turn,
        first_non_header_line_per_turn[1].as_ref()
    );

    // Verify every turn's transcript contains the expected full answer.
    // Note: We reuse the transcript logic from the markdown replay test by re-parsing
    // expected answers from the JSON here is heavier; for now this test focuses on header/content
    // batching to prevent the cut-off at the start.
}

// drain helper moved to `crate::test_utils::drain_insert_history`

// Replay a longer hello session and ensure every turn's full answer is present
// in the transcript by extracting expected answers from AgentMessage/TaskComplete
// events and comparing them to accumulated InsertHistory content.
#[tokio::test(flavor = "current_thread")]
async fn vt100_replay_longer_hello_session_from_log() {
    let width: u16 = 100;
    let height: u16 = 50;
    let viewport = Rect::new(0, height - 1, width, 1);
    let backend = TestBackend::new(width, height);
    let mut terminal = crate::custom_terminal::Terminal::with_options(backend)
        .expect("failed to construct terminal");
    terminal.set_viewport_area(viewport);

    let (tx_raw, rx): (std::sync::mpsc::Sender<AppEvent>, Receiver<AppEvent>) = channel();
    let app_sender = AppEventSender::new(tx_raw);

    let cfg: Config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("config");

    let mut widget =
        crate::chatwidget::ChatWidget::new(cfg, app_sender.clone(), None, Vec::new(), false);

    let mut ansi: Vec<u8> = Vec::new();

    let file = open_fixture("longer-hello.jsonl");
    let reader = BufReader::new(file);

    let mut current_turn_index: Option<usize> = None;
    let mut expected_full_answer_per_turn: Vec<Option<String>> = Vec::new();
    let mut transcript_per_turn: Vec<String> = Vec::new();

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
                    let ev: CodexEvent = serde_json::from_value(payload.clone()).expect("parse");
                    if let CodexEvent { msg, .. } = &ev {
                        if matches!(msg, codex_core::protocol::EventMsg::TaskStarted) {
                            expected_full_answer_per_turn.push(None);
                            transcript_per_turn.push(String::new());
                            current_turn_index = Some(expected_full_answer_per_turn.len() - 1);
                        }
                        if let codex_core::protocol::EventMsg::AgentMessage(m) = msg {
                            if let Some(idx) = current_turn_index {
                                expected_full_answer_per_turn[idx] = Some(m.message.clone());
                            }
                        }
                        if let codex_core::protocol::EventMsg::TaskComplete(tc) = msg {
                            if let Some(idx) = current_turn_index {
                                if tc.last_agent_message.is_some() {
                                    expected_full_answer_per_turn[idx] =
                                        tc.last_agent_message.clone();
                                }
                            }
                        }
                    }
                    widget.handle_codex_event(ev);
                    while let Ok(app_ev) = rx.try_recv() {
                        if let AppEvent::InsertHistory(lines) = app_ev {
                            if let Some(idx) = current_turn_index {
                                crate::test_utils::append_lines_to_transcript(
                                    &lines,
                                    &mut transcript_per_turn[idx],
                                );
                            }
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
                if let Some(variant) = v.get("variant").and_then(|s| s.as_str()) {
                    if variant == "CommitTick" {
                        widget.on_commit_tick();
                        while let Ok(app_ev) = rx.try_recv() {
                            if let AppEvent::InsertHistory(lines) = app_ev {
                                if let Some(idx) = current_turn_index {
                                    crate::test_utils::append_lines_to_transcript(
                                        &lines,
                                        &mut transcript_per_turn[idx],
                                    );
                                }
                                crate::insert_history::insert_history_lines_to_writer(
                                    &mut terminal,
                                    &mut ansi,
                                    lines,
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Verify every turn's transcript contains the expected full answer.
    for (i, maybe_expected) in expected_full_answer_per_turn.iter().enumerate() {
        if let Some(expected) = maybe_expected {
            assert!(
                transcript_per_turn[i].contains(expected),
                "turn {} transcript missing expected full answer.\nexpected: {:?}\ntranscript: {}",
                i,
                expected,
                transcript_per_turn[i]
            );
        }
    }
}
