//! VT100-backed snapshot tests for ChatWidget.
//!
//! These tests render `ChatWidget` into a `VT100Backend` terminal at a fixed
//! size and snapshot the screen contents using `insta`. The harness ensures
//! deterministic output (e.g. it fixes the greeting hour) so diffs stay stable.

#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use code_core::protocol::{
    AgentInfo,
    AgentMessageDeltaEvent,
    AgentMessageEvent,
    AgentStatusUpdateEvent,
    BackgroundEventEvent,
    BrowserScreenshotUpdateEvent,
    CustomToolCallBeginEvent,
    CustomToolCallEndEvent,
    Event,
    EventMsg,
    OrderMeta,
    WebSearchBeginEvent,
    WebSearchCompleteEvent,
};
use code_tui::test_helpers::{
    force_scroll_offset as harness_force_scroll_offset,
    layout_metrics as harness_layout_metrics,
    render_chat_widget_to_vt100,
    AutoContinueModeFixture,
    ChatWidgetHarness,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use regex_lite::{Captures, Regex};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use once_cell::sync::Lazy;
use tempfile::TempDir;

fn normalize_output(text: String) -> String {
    text
        .chars()
        .map(normalize_glyph)
        .collect::<String>()
        .pipe(normalize_ellipsis)
        .pipe(normalize_timers)
        .pipe(normalize_auto_drive_layout)
        .pipe(normalize_agent_history_details)
        .pipe(normalize_spacer_rows)
        .pipe(normalize_trailing_whitespace)
}

fn normalize_ellipsis(text: String) -> String {
    text.replace('…', "...")
}

fn normalize_trailing_whitespace(text: String) -> String {
    let mut normalized = String::with_capacity(text.len());
    for segment in text.split_inclusive('\n') {
        if let Some(stripped) = segment.strip_suffix('\n') {
            normalized.push_str(stripped.trim_end());
            normalized.push('\n');
        } else {
            normalized.push_str(segment.trim_end());
        }
    }
    normalized
}

static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

struct EnvGuard {
    saved: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
    fn new(keys: &[&'static str]) -> Self {
        let mut saved = Vec::with_capacity(keys.len());
        for key in keys {
            saved.push((*key, std::env::var(key).ok()));
            unsafe {
                std::env::remove_var(key);
            }
        }
        Self { saved }
    }

    fn set_path(&self, key: &'static str, path: &Path) {
        unsafe {
            std::env::set_var(key, path);
        }
    }

    fn remove(&self, key: &'static str) {
        unsafe {
            std::env::remove_var(key);
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.saved.drain(..) {
            match value {
                Some(v) => unsafe {
                    std::env::set_var(key, v);
                },
                None => unsafe {
                    std::env::remove_var(key);
                },
            }
        }
    }
}

fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    }
}

fn normalize_glyph(ch: char) -> char {
    match ch {
        // Decorative sparkles → single sentinel to keep intent without variation.
        '✧' | '◇' | '✦' | '◆' | '✨' => '✶',
        // Box-drawing corners → '+' for ASCII snapshots.
        '┌' | '┐' | '└' | '┘'
        | '┏' | '┓' | '┗' | '┛'
        | '╔' | '╗' | '╚' | '╝'
        | '╒' | '╕' | '╛' | '╜'
        | '╓' | '╖' | '╙' | '╘'
        | '╭' | '╮' | '╯' | '╰' => '+',
        // Tee / cross junctions also collapse to '+' to keep structure recognizable.
        '┬' | '┴' | '┼' | '├' | '┤'
        | '┽' | '┾' | '┿'
        | '╀' | '╁' | '╂' | '╃' | '╄'
        | '╅' | '╆' | '╇' | '╈' | '╉'
        | '╊' | '╋' | '╟' | '╠' | '╡'
        | '╢' | '╫' | '╪' | '╬' => '+',
        // Horizontal box drawing and variants → '-'.
        '─' | '━' | '═' | '╼' | '╾'
        | '╸' | '╺' | '╴' | '╶'
        | '┄' | '┅' | '┈' | '┉' => '-',
        // Vertical box drawing variants → '|'.
        '│' | '┃' | '║' | '╽' | '╿'
        | '╏' | '╎' | '┆' | '┇'
        | '╷' | '╹' => '|',
        // Diagonal strokes → ASCII approximations.
        '╱' => '/',
        '╲' => '/',
        '╳' => 'X',
        // Shade blocks → space to avoid rendering noise.
        '█' | '▓' | '▒' | '░' => ' ',
        // Various unicode dash variants → ASCII hyphen.
        '‐' | '‑' | '‒' | '–' | '—' | '―' => '-',
        other => other,
    }
}

fn normalize_timers(text: String) -> String {
    static IN_SECONDS_RE: OnceLock<Regex> = OnceLock::new();
    static T_MINUS_RE: OnceLock<Regex> = OnceLock::new();
    static MM_SS_RE: OnceLock<Regex> = OnceLock::new();
    static MM_SS_SUFFIX_RE: OnceLock<Regex> = OnceLock::new();
    static MS_RE: OnceLock<Regex> = OnceLock::new();
    static MIN_SEC_RE: OnceLock<Regex> = OnceLock::new();
    static PAREN_SECONDS_RE: OnceLock<Regex> = OnceLock::new();
    static PAREN_MINUTES_RE: OnceLock<Regex> = OnceLock::new();
    static PAREN_MINUTES_SECONDS_RE: OnceLock<Regex> = OnceLock::new();
    static PAREN_HOURS_MINUTES_RE: OnceLock<Regex> = OnceLock::new();
    static PAREN_HOURS_MINUTES_SECONDS_RE: OnceLock<Regex> = OnceLock::new();
    static RAW_SECONDS_SUFFIX_RE: OnceLock<Regex> = OnceLock::new();

    let text = IN_SECONDS_RE
        .get_or_init(|| Regex::new(r"\bin\s+\d+s\b").expect("valid in seconds regex"))
        .replace_all(&text, "in XS")
        .into_owned();

    let text = T_MINUS_RE
        .get_or_init(|| Regex::new(r"\bT-\d+\b").expect("valid T-minus regex"))
        .replace_all(&text, "T-X")
        .into_owned();

    let text = MM_SS_RE
        .get_or_init(|| Regex::new(r"\b\d{1,2}:\d{2}\b").expect("valid mm:ss regex"))
        .replace_all(&text, "MM:SS")
        .into_owned();

    let text = MM_SS_SUFFIX_RE
        .get_or_init(|| Regex::new(r"MM:SS:\d{2}").expect("valid mm:ss suffix regex"))
        .replace_all(&text, "MM:SS")
        .into_owned();

    let text = MS_RE
        .get_or_init(|| Regex::new(r"\b\d+ms\b").expect("valid milliseconds regex"))
        .replace_all(&text, "Xms")
        .into_owned();

    let text = PAREN_SECONDS_RE
        .get_or_init(|| Regex::new(r"\(\d+s\)").expect("valid paren seconds regex"))
        .replace_all(&text, "(Xs)")
        .into_owned();

    let text = PAREN_MINUTES_RE
        .get_or_init(|| Regex::new(r"\(\d+m\)").expect("valid paren minutes regex"))
        .replace_all(&text, "(Xm)")
        .into_owned();

    let text = PAREN_MINUTES_SECONDS_RE
        .get_or_init(|| Regex::new(r"\(\d+m\s+\d+s\)").expect("valid paren minutes seconds regex"))
        .replace_all(&text, "(Xm Xs)")
        .into_owned();

    let text = PAREN_HOURS_MINUTES_RE
        .get_or_init(|| Regex::new(r"\(\d+h\s+\d+m\)").expect("valid paren hours minutes regex"))
        .replace_all(&text, "(Xh Xm)")
        .into_owned();

    let text = PAREN_HOURS_MINUTES_SECONDS_RE
        .get_or_init(|| Regex::new(r"\(\d+h\s+\d+m\s+\d+s\)").expect("valid paren hours minutes seconds regex"))
        .replace_all(&text, "(Xh Xm Xs)")
        .into_owned();

    let text = RAW_SECONDS_SUFFIX_RE
        .get_or_init(|| Regex::new(r"\b\d+s\b").expect("valid raw seconds suffix regex"))
        .replace_all(&text, "Xs")
        .into_owned();

    MIN_SEC_RE
        .get_or_init(|| Regex::new(r"\b\d+m\s+\d+s\b").expect("valid minute-second regex"))
        .replace_all(&text, "Xm Ys")
        .into_owned()
}

fn normalize_auto_drive_layout(text: String) -> String {
    static AUTO_DRIVE_STATUS_ALIGN_RE: OnceLock<Regex> = OnceLock::new();
    static AUTO_DRIVE_BANNER_RE: OnceLock<Regex> = OnceLock::new();

    let text = AUTO_DRIVE_STATUS_ALIGN_RE
        .get_or_init(|| {
            Regex::new(r"(?m)^(?P<prefix>\s*Auto Drive >[^\n]*?)(?P<spaces>\s+)✶(?P<rest>[^\n]*)$")
                .expect("valid auto drive status alignment regex")
        })
        .replace_all(&text, |caps: &Captures| {
            format!("{}  ✶{}", &caps["prefix"], &caps["rest"])
        })
        .into_owned();

    AUTO_DRIVE_BANNER_RE
        .get_or_init(|| {
            Regex::new(
                r"(?m)^(?P<indent>\s*)\+(?P<left>-+)\s+✶\s*(?P<title>[^\-\n]+?)\s+(?P<right>-+)\+$",
            )
            .expect("valid auto drive banner regex")
        })
        .replace_all(&text, |caps: &Captures| {
            let indent = &caps["indent"];
            let title = caps["title"].trim();
            format!("{indent}+----------------------------- ✶ {title} -----------------------------+")
        })
        .into_owned()
}

fn normalize_agent_history_details(text: String) -> String {
    let mut blank_next_detail_line = false;
    let mut result = String::new();

    for line in text.lines() {
        let mut transformed = line.to_string();
        let mut handled_detail = false;

        for (token, label) in [("progress:", "detail:"), ("result:", "detail:")] {
            if let Some(idx) = line.find(token) {
                let prefix = &line[..idx];
                let label = label;
                if let Some(tail_start) = line.rfind("| |") {
                    let tail = &line[tail_start..];
                    const DETAIL_FILLER: &str = " ...                     ";
                    transformed = format!("{prefix}{label}{DETAIL_FILLER}{tail}");
                } else {
                    transformed = format!("{}{label} ...", prefix);
                }
                handled_detail = true;
                break;
            }
        }

        if handled_detail {
            blank_next_detail_line = true;
        } else if blank_next_detail_line {
            if let Some(blanked) = blank_between_pipes(line) {
                transformed = blanked;
            }
            blank_next_detail_line = false;
        }

        result.push_str(&transformed);
        result.push('\n');
    }

    if !text.ends_with('\n') {
        result.pop();
    }

    result
}

fn blank_between_pipes(line: &str) -> Option<String> {
    let (first_pipe, last_pipe) = (line.find('|')?, line.rfind('|')?);
    if last_pipe <= first_pipe {
        return None;
    }

    let indent = &line[..first_pipe + 1];
    let tail = &line[last_pipe..];
    let span_width = last_pipe.saturating_sub(first_pipe + 1);
    Some(format!("{indent}{}{tail}", " ".repeat(span_width)))
}

fn normalize_spacer_rows(text: String) -> String {
    let ends_with_newline = text.ends_with('\n');
    let mut lines: Vec<String> = Vec::new();
    let mut pending_blank: Option<usize> = None;

    for line in text.lines() {
        if is_spacer_border_line(line) {
            pending_blank = Some(line.chars().count());
            continue;
        }

        lines.push(line.to_string());

        if let Some(width) = pending_blank.take() {
            lines.push(" ".repeat(width));
        }
    }

    if let Some(width) = pending_blank {
        lines.push(" ".repeat(width));
    }

    let mut normalized = lines.join("\n");
    if ends_with_newline && (!normalized.is_empty() || text.is_empty()) {
        normalized.push('\n');
    }

    normalized
}

fn is_spacer_border_line(line: &str) -> bool {
    if line.trim().is_empty() {
        return false;
    }

    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }

    let bytes = trimmed.as_bytes();
    if bytes.first() != Some(&b'|') || bytes.last() != Some(&b'|') {
        return false;
    }

    let inner = &trimmed[1..trimmed.len() - 1];
    if inner.trim().is_empty() {
        return false;
    }

    inner.chars().all(|ch| ch == '-' || ch == ' ')
}

trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R;
}

impl<T> Pipe for T {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}

fn is_history_header(line: &str) -> bool {
    let trimmed = line.trim_start();
    matches!(trimmed.chars().next(), Some(ch) if matches!(ch, '›' | '•' | '⋮' | '⚙' | '✔' | '✖' | '✶'))
}

fn count_collapsed_boundaries(output: &str) -> usize {
    let mut collapsed = 0usize;
    let mut saw_header = false;
    let mut blank_since_last_header = false;

    for line in output.lines() {
        if line.trim_end().is_empty() {
            if saw_header {
                blank_since_last_header = true;
            }
            continue;
        }

        if is_history_header(line) {
            if saw_header && !blank_since_last_header {
                collapsed = collapsed.saturating_add(1);
            }
            saw_header = true;
            blank_since_last_header = false;
        }
    }

    collapsed
}

fn push_ordered_event(
    harness: &mut ChatWidgetHarness,
    event_seq: &mut u64,
    order_seq: &mut u64,
    msg: EventMsg,
) {
    let seq = *event_seq;
    let ord = *order_seq;
    let event = Event {
        id: "turn-1".into(),
        event_seq: seq,
        msg,
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(0),
            sequence_number: Some(ord),
        }),
    };
    harness.handle_event(event);
    *event_seq = seq.saturating_add(1);
    *order_seq = ord.saturating_add(1);
}

fn push_unordered_event(
    harness: &mut ChatWidgetHarness,
    event_seq: &mut u64,
    msg: EventMsg,
) {
    let seq = *event_seq;
    harness.handle_event(Event {
        id: format!("unordered-{seq}"),
        event_seq: seq,
        msg,
        order: None,
    });
    *event_seq = seq.saturating_add(1);
}

#[test]
fn baseline_empty_chat() {
    let mut harness = ChatWidgetHarness::new();
    code_tui::test_helpers::set_standard_terminal_mode(&mut harness, false);

    let output = normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 24));
    insta::assert_snapshot!("empty_chat", output);
}

#[test]
fn auto_drive_continue_mode_transitions() {
    let mut harness = ChatWidgetHarness::new();

    harness.auto_drive_activate(
        "Expand Auto Drive validation",
        true,
        true,
        AutoContinueModeFixture::TenSeconds,
    );
    harness.auto_drive_set_awaiting_submission(
        "cargo nextest run --no-fail-fast",
        "Auto Drive ready to run cargo nextest",
        Some("Proposed action: run focused tests before continuing.".to_string()),
    );
    harness.auto_drive_override_countdown(9);

    let mut frames = Vec::new();
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 18)));

    harness.auto_drive_set_continue_mode(AutoContinueModeFixture::Manual);
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 18)));

    harness.auto_drive_set_continue_mode(AutoContinueModeFixture::Immediate);
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 18)));

    insta::assert_snapshot!(
        "auto_drive_continue_mode_transitions",
        frames.join("\n---FRAME---\n"),
    );
}

#[test]
fn auto_drive_action_transitions() {
    let mut harness = ChatWidgetHarness::new();

    harness.auto_drive_activate(
        "Diagnose Auto Drive regressions",
        true,
        true,
        AutoContinueModeFixture::TenSeconds,
    );
    harness.auto_drive_set_waiting_for_response(
        "Analyzing workspace changes",
        Some("Comparing diffs against last run.".to_string()),
        Some("Completed git status check.".to_string()),
    );

    let mut frames = Vec::new();
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 18)));

    harness.auto_drive_set_awaiting_submission(
        "cargo test --workspace",
        "Ready: run cargo test --workspace",
        Some("Suggested step: confirm tests before resuming.".to_string()),
    );
    harness.auto_drive_override_countdown(6);
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 18)));

    harness.auto_drive_set_waiting_for_review(Some(
        "Waiting for code review to complete.".to_string(),
    ));
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 18)));

    harness.auto_drive_set_waiting_for_response(
        "Resuming automated investigation",
        Some("Queued new command for execution.".to_string()),
        None,
    );
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 18)));

    insta::assert_snapshot!(
        "auto_drive_action_transitions",
        frames.join("\n---FRAME---\n"),
    );
}

#[test]
fn auto_drive_cli_progress_header() {
    let mut harness = ChatWidgetHarness::new();

    harness.auto_drive_activate(
        "Highlight CLI progress",
        true,
        true,
        AutoContinueModeFixture::TenSeconds,
    );
    harness.auto_drive_set_waiting_for_response(
        "Preparing workspace",
        Some("Running cargo check...".to_string()),
        Some("Installed dependencies".to_string()),
    );
    harness.auto_drive_set_awaiting_submission(
        "cargo check",
        "Ready: run cargo check",
        Some("Running cargo check...".to_string()),
    );

    let mut frames = Vec::new();
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 12)));

    harness.auto_drive_simulate_cli_submission();
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 12)));

    harness.auto_drive_mark_cli_running();
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 12)));

    insta::assert_snapshot!(
        "auto_drive_cli_progress_header",
        frames.join("\n---FRAME---\n"),
    );
}

#[test]
fn auto_drive_countdown_auto_submit() {
    let mut harness = ChatWidgetHarness::new();

    harness.auto_drive_activate(
        "Handle countdown exhaustion",
        true,
        false,
        AutoContinueModeFixture::TenSeconds,
    );
    harness.auto_drive_set_awaiting_submission(
        "cargo fmt --check",
        "Auto Drive queued cargo fmt --check",
        Some("Will run formatter unless cancelled.".to_string()),
    );
    harness.auto_drive_override_countdown(3);

    let mut frames = Vec::new();
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 18)));

    harness.auto_drive_advance_countdown(1);
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 18)));

    harness.auto_drive_advance_countdown(0);
    harness.auto_drive_set_waiting_for_response(
        "Auto Drive executing formatter",
        Some("Running cargo fmt --check.".to_string()),
        None,
    );
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 18)));

    insta::assert_snapshot!(
        "auto_drive_countdown_auto_submit",
        frames.join("\n---FRAME---\n"),
    );
}

#[test]
fn auto_drive_review_footer_persists() {
    let mut harness = ChatWidgetHarness::new();

    harness.auto_drive_activate(
        "Verify review footer",
        true,
        true,
        AutoContinueModeFixture::TenSeconds,
    );
    harness.auto_drive_set_waiting_for_review(Some(
        "Waiting for code review to complete.".to_string(),
    ));

    let frame = normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 18));

    insta::assert_snapshot!("auto_drive_review_footer_persists", frame);
}

#[test]
fn auto_drive_manual_mode_waits() {
    let mut harness = ChatWidgetHarness::new();

    harness.auto_drive_activate(
        "Manual mode requires explicit continue",
        true,
        true,
        AutoContinueModeFixture::TenSeconds,
    );
    harness.auto_drive_set_awaiting_submission(
        "justfile run manual",
        "Auto Drive prepared to run justfile target",
        Some("User confirmation required before continuing.".to_string()),
    );
    harness.auto_drive_set_continue_mode(AutoContinueModeFixture::Manual);

    let frame = normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 18));

    insta::assert_snapshot!("auto_drive_manual_mode_waits", frame);
}

#[test]
fn auto_drive_review_resume_returns_to_running() {
    let mut harness = ChatWidgetHarness::new();

    harness.auto_drive_activate(
        "Resume after review",
        true,
        true,
        AutoContinueModeFixture::TenSeconds,
    );
    harness.auto_drive_set_waiting_for_review(Some(
        "Waiting for reviewer feedback.".to_string(),
    ));

    let mut frames = Vec::new();
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 18)));

    harness.auto_drive_set_waiting_for_response(
        "Review complete — resuming tasks",
        Some("Coordinator resumed the workflow.".to_string()),
        Some("Review cleared open issues.".to_string()),
    );
    frames.push(normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 18)));

    insta::assert_snapshot!(
        "auto_drive_review_resume_returns_to_running",
        frames.join("\n---FRAME---\n"),
    );
}

#[test]
fn baseline_simple_conversation() {
    let mut harness = ChatWidgetHarness::new();

    harness.push_user_prompt("Can you help me understand the available commands?");

    // Assistant greeting
    harness.handle_event(Event {
        id: "msg-1".into(),
        event_seq: 0,
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "Hello! ".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(0),
            sequence_number: Some(0),
        }),
    });
    harness.handle_event(Event {
        id: "msg-1".into(),
        event_seq: 1,
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "How can I help you today?".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(0),
            sequence_number: Some(1),
        }),
    });
    harness.handle_event(Event {
        id: "msg-1".into(),
        event_seq: 2,
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "Hello! How can I help you today?".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(0),
            sequence_number: Some(2),
        }),
    });

    // Assistant continues with another message.
    harness.handle_event(Event {
        id: "msg-2".into(),
        event_seq: 0,
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "I can help with ".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 2,
            output_index: Some(0),
            sequence_number: Some(0),
        }),
    });
    harness.handle_event(Event {
        id: "msg-2".into(),
        event_seq: 1,
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "various tasks including:\n\n".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 2,
            output_index: Some(0),
            sequence_number: Some(1),
        }),
    });
    harness.handle_event(Event {
        id: "msg-2".into(),
        event_seq: 2,
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "- Writing code\n".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 2,
            output_index: Some(0),
            sequence_number: Some(2),
        }),
    });
    harness.handle_event(Event {
        id: "msg-2".into(),
        event_seq: 3,
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "- Reading files\n".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 2,
            output_index: Some(0),
            sequence_number: Some(3),
        }),
    });
    harness.handle_event(Event {
        id: "msg-2".into(),
        event_seq: 4,
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "- Running commands".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 2,
            output_index: Some(0),
            sequence_number: Some(4),
        }),
    });
    harness.handle_event(Event {
        id: "msg-2".into(),
        event_seq: 5,
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "I can help with various tasks including:\n\n- Writing code\n- Reading files\n- Running commands".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 2,
            output_index: Some(0),
            sequence_number: Some(5),
        }),
    });

    let output = normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 24));
    insta::assert_snapshot!("simple_conversation", output);
}

#[test]
fn scroll_spacing_remains_when_scrolled_up() {
    let mut harness = ChatWidgetHarness::new();

    harness.push_user_prompt("First user message about scrolling behaviour.");
    harness.push_assistant_markdown("Assistant reply number one with enough text to wrap the layout and ensure spacing stays visible while at the bottom of the viewport.");
    harness.push_user_prompt("Second user follow-up that also contributes to the total height so we can scroll.");
    harness.push_assistant_markdown("Assistant reply number two with multiple paragraphs.\n\nHere is another paragraph to expand height.\n\nYet another paragraph for good measure.");
    harness.push_user_prompt("Third user prompt to push history further.");
    harness.push_assistant_markdown("Assistant reply number three, still going strong.\n\n- Bullet one\n- Bullet two\n- Bullet three");
    harness.push_user_prompt("Fourth user prompt to guarantee overflow beyond the viewport height.");
    harness.push_assistant_markdown("Assistant reply number four with extra padding to pad out the history list even more.\n\nFinal paragraph to top it off.");

    let _bottom = normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 24));
    let metrics = harness_layout_metrics(&harness);
    assert!(
        metrics.last_max_scroll > 0,
        "scenario must overflow the history viewport to exercise scrolling"
    );

    let offset = metrics.last_max_scroll.min(5).max(1);
    harness_force_scroll_offset(&mut harness, offset);
    let scrolled = normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 24));

    let collapsed_boundaries = count_collapsed_boundaries(&scrolled);
    assert_eq!(
        0,
        collapsed_boundaries,
        "Spacing collapsed unexpectedly when scrolled; investigate history layout spacing"
    );

    insta::assert_snapshot!(
        "scroll_spacing_scrolled_intact",
        scrolled
    );
}

#[test]
fn baseline_multiline_formatting() {
    let mut harness = ChatWidgetHarness::new();

    harness.handle_event(Event {
        id: "msg-code".into(),
        event_seq: 0,
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "Here's a simple function:\n\n```rust\n".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(0),
            sequence_number: Some(0),
        }),
    });
    harness.handle_event(Event {
        id: "msg-code".into(),
        event_seq: 1,
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "fn hello() {\n    println!(\"Hello, world!\");\n}\n```".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(0),
            sequence_number: Some(1),
        }),
    });
    harness.handle_event(Event {
        id: "msg-code".into(),
        event_seq: 2,
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "Here's a simple function:\n\n```rust\nfn hello() {\n    println!(\"Hello, world!\");\n}\n```".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(0),
            sequence_number: Some(2),
        }),
    });

    let output = normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 24));
    insta::assert_snapshot!("multiline_formatting", output);
}

#[test]
fn tool_activity_showcase() {
    let mut harness = ChatWidgetHarness::new();

    harness.push_user_prompt("Can you gather details from the latest docs update?");

    let mut event_seq = 0u64;
    let mut order_seq = 0u64;

    // Completed web search call
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::WebSearchBegin(WebSearchBeginEvent {
            call_id: "search-complete".into(),
            query: Some("ratatui widget patterns".into()),
        }),
    );
    harness.override_running_tool_elapsed("search-complete", Duration::from_millis(0));
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::WebSearchComplete(WebSearchCompleteEvent {
            call_id: "search-complete".into(),
            query: Some("ratatui widget patterns".into()),
        }),
    );

    // Running web search call
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::WebSearchBegin(WebSearchBeginEvent {
            call_id: "search-running".into(),
            query: Some("async rust tui example".into()),
        }),
    );
    harness.override_running_tool_elapsed("search-running", Duration::from_secs(75));

    // Browser tool: completed click
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "browser-finished".into(),
            tool_name: "browser_click".into(),
            parameters: Some(json!({
                "type": "double",
                "x": 512,
                "y": 284,
                "selector": "#login-button"
            })),
        }),
    );
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallEnd(CustomToolCallEndEvent {
            call_id: "browser-finished".into(),
            tool_name: "browser_click".into(),
            parameters: Some(json!({
                "type": "double",
                "x": 512,
                "y": 284,
                "selector": "#login-button"
            })),
            duration: Duration::from_secs(8),
            result: Ok("{\n  \"status\": \"ok\",\n  \"notes\": \"Login button clicked\"\n}".into()),
        }),
    );

    // Browser tool: active scroll
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "browser-running".into(),
            tool_name: "browser_scroll".into(),
            parameters: Some(json!({
                "dx": 0,
                "dy": 640,
                "speed": "smooth"
            })),
        }),
    );
    harness.override_running_tool_elapsed("browser-running", Duration::from_secs(95));

    // Agent tool: completed run
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "agent-done".into(),
            tool_name: "agent".into(),
            parameters: Some(json!({
                "action": "create",
                "batch_id": "batch-demo",
                "create": {
                    "name": "qa-bot",
                    "task": "Run targeted regression suite"
                }
            })),
        }),
    );
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallEnd(CustomToolCallEndEvent {
            call_id: "agent-done".into(),
            tool_name: "agent".into(),
            parameters: Some(json!({
                "action": "create",
                "batch_id": "batch-demo",
                "create": {
                    "name": "qa-bot",
                    "task": "Run targeted regression suite"
                }
            })),
            duration: Duration::from_secs(94),
            result: Ok("Regression sweep complete\n- 58 tests passed\n- 0 failures".into()),
        }),
    );

    // Agent tool: active wait
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "agent-pending".into(),
            tool_name: "agent".into(),
            parameters: Some(json!({
                "action": "wait",
                "wait": {
                    "agent_id": "deploy-helper",
                    "batch_id": "batch-demo",
                    "timeout_seconds": 600
                }
            })),
        }),
    );
    harness.override_running_tool_elapsed("agent-pending", Duration::from_secs(185));

    let output = normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 40));
    insta::assert_snapshot!("tool_activity_showcase", output);
}

#[test]
fn browser_session_grouped_desired_layout() {
    let mut harness = ChatWidgetHarness::new();
    harness.push_user_prompt("Open docs and find login button");

    let mut event_seq = 0u64;
    let mut order_seq = 0u64;

    // Browser open -> sets initial URL
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "browser-session".into(),
            tool_name: "browser_open".into(),
            parameters: Some(json!({
                "url": "https://example.com/docs"
            })),
        }),
    );
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallEnd(CustomToolCallEndEvent {
            call_id: "browser-session".into(),
            tool_name: "browser_open".into(),
            parameters: Some(json!({
                "url": "https://example.com/docs"
            })),
            duration: Duration::from_secs(5),
            result: Ok("{\n  \"status\": \"ok\"\n}".into()),
        }),
    );

    // Additional browser interactions
    let actions = [
        (
            "browser_click",
            json!({
                "selector": "#sign-in",
                "description": "Sign in button"
            }),
            Duration::from_secs(13),
        ),
        (
            "browser_scroll",
            json!({
                "dx": 0,
                "dy": 640
            }),
            Duration::from_secs(14),
        ),
        (
            "browser_type",
            json!({
                "text": "docs search"
            }),
            Duration::from_secs(33),
        ),
    ];

    for (tool, params, dur) in actions {
        push_ordered_event(
            &mut harness,
            &mut event_seq,
            &mut order_seq,
            EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
                call_id: format!("browser-session-{tool}"),
                tool_name: tool.into(),
                parameters: Some(params.clone()),
            }),
        );
        push_ordered_event(
            &mut harness,
            &mut event_seq,
            &mut order_seq,
            EventMsg::CustomToolCallEnd(CustomToolCallEndEvent {
                call_id: format!("browser-session-{tool}"),
                tool_name: tool.into(),
                parameters: Some(params),
                duration: dur,
                result: Ok("{\n  \"status\": \"ok\"\n}".into()),
            }),
        );
    }

    // Capture console warning (represented as background event)
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::BackgroundEvent(BackgroundEventEvent {
            message: "cdp warning: Refused to load script from cdn.example.com".into(),
        }),
    );

    // Screenshot update for active tab
    harness.handle_event(Event {
        id: "browser-shot".into(),
        event_seq,
        msg: EventMsg::BrowserScreenshotUpdate(BrowserScreenshotUpdateEvent {
            screenshot_path: PathBuf::from("/tmp/browser_session.png"),
            url: "https://example.com/docs".into(),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(1),
            sequence_number: Some(order_seq),
        }),
    });

    let output = normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 32));
    insta::assert_snapshot!("browser_session_grouped_desired_layout", output);
}

#[test]
fn browser_session_grouped_with_unordered_actions() {
    let mut harness = ChatWidgetHarness::new();
    harness.push_user_prompt("Handle captcha gracefully");

    let mut event_seq = 0u64;
    let mut order_seq = 0u64;

    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "browser-session-open".into(),
            tool_name: "browser_open".into(),
            parameters: Some(json!({ "url": "https://example.com" })),
        }),
    );
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallEnd(CustomToolCallEndEvent {
            call_id: "browser-session-open".into(),
            tool_name: "browser_open".into(),
            parameters: Some(json!({ "url": "https://example.com" })),
            duration: Duration::from_secs(3),
            result: Ok("{ \"status\": \"ok\" }".into()),
        }),
    );

    push_unordered_event(
        &mut harness,
        &mut event_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "browser-session-type".into(),
            tool_name: "browser_type".into(),
            parameters: Some(json!({ "text": "pizza" })),
        }),
    );
    push_unordered_event(
        &mut harness,
        &mut event_seq,
        EventMsg::BackgroundEvent(BackgroundEventEvent {
            message: "Encountering captcha block".into(),
        }),
    );
    push_unordered_event(
        &mut harness,
        &mut event_seq,
        EventMsg::CustomToolCallEnd(CustomToolCallEndEvent {
            call_id: "browser-session-type".into(),
            tool_name: "browser_type".into(),
            parameters: Some(json!({ "text": "pizza" })),
            duration: Duration::from_secs(2),
            result: Ok("{ \"status\": \"typed\" }".into()),
        }),
    );

    push_unordered_event(
        &mut harness,
        &mut event_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "browser-session-key".into(),
            tool_name: "browser_key".into(),
            parameters: Some(json!({ "key": "Enter" })),
        }),
    );
    push_unordered_event(
        &mut harness,
        &mut event_seq,
        EventMsg::CustomToolCallEnd(CustomToolCallEndEvent {
            call_id: "browser-session-key".into(),
            tool_name: "browser_key".into(),
            parameters: Some(json!({ "key": "Enter" })),
            duration: Duration::from_secs(1),
            result: Ok("{ \"status\": \"ok\" }".into()),
        }),
    );

    let output = render_chat_widget_to_vt100(&mut harness, 80, 32);
    let output = normalize_output(output);
    insta::assert_snapshot!(
        "browser_session_grouped_with_unordered_actions",
        output
    );
}

#[test]
fn agent_run_grouped_desired_layout() {
    let mut harness = ChatWidgetHarness::new();
    harness.push_user_prompt("Kick off QA bot regression run");

    let mut event_seq = 0u64;
    let mut order_seq = 0u64;

    // Agent run begins
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "agent-run".into(),
            tool_name: "agent".into(),
            parameters: Some(json!({
                "action": "create",
                "batch_id": "batch-qa",
                "create": {
                    "name": "qa-bot",
                    "task": "Run targeted regression suite"
                }
            })),
        }),
    );

    // Status update with multiple agents
    harness.handle_event(Event {
        id: "agent-status".into(),
        event_seq,
        msg: EventMsg::AgentStatusUpdate(AgentStatusUpdateEvent {
            agents: vec![
                AgentInfo {
                    id: "qa-bot".into(),
                    name: "QA Bot".into(),
                    status: "running tests".into(),
                    batch_id: Some("batch-qa".into()),
                    model: None,
                    last_progress: None,
                    result: None,
                    error: None,
                    elapsed_ms: Some(29_000),
                    token_count: Some(12_400),
                },
                AgentInfo {
                    id: "doc-writer".into(),
                    name: "Doc Writer".into(),
                    status: "planning".into(),
                    batch_id: Some("batch-qa".into()),
                    model: None,
                    last_progress: None,
                    result: None,
                    error: None,
                    elapsed_ms: Some(4_500),
                    token_count: None,
                },
            ],
            context: Some("regression sweep".into()),
            task: Some("Ship bugfix patch".into()),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(1),
            sequence_number: Some(order_seq),
        }),
    });
    event_seq += 1;
    order_seq += 1;

    // Agent result
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallEnd(CustomToolCallEndEvent {
            call_id: "agent-run".into(),
            tool_name: "agent".into(),
            parameters: Some(json!({
                "action": "create",
                "batch_id": "batch-qa",
                "create": {
                    "name": "qa-bot",
                    "task": "Run targeted regression suite"
                }
            })),
            duration: Duration::from_secs(94),
            result: Ok("Regression sweep complete\n- 58 tests passed\n- 0 failures".into()),
        }),
    );

    let output = render_chat_widget_to_vt100(&mut harness, 80, 32);
    let output = normalize_output(output);
    insta::assert_snapshot!(
        "agent_run_grouped_desired_layout",
        &output,
        @"agent_run_grouped_desired_layout"
    );
}

#[test]
fn agent_run_grouped_plain_tool_name() {
    let mut harness = ChatWidgetHarness::new();
    harness.push_user_prompt("Kick off QA bot regression run");

    let mut event_seq = 0u64;
    let mut order_seq = 0u64;

    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "agent-run-plain".into(),
            tool_name: "agent".into(),
            parameters: Some(json!({
                "action": "create",
                "batch_id": "batch-plain",
                "create": {
                    "name": "qa-bot",
                    "task": "Run targeted regression suite"
                }
            })),
        }),
    );

    harness.handle_event(Event {
        id: "agent-status".into(),
        event_seq,
        msg: EventMsg::AgentStatusUpdate(AgentStatusUpdateEvent {
            agents: vec![
                AgentInfo {
                    id: "qa-bot".into(),
                    name: "QA Bot".into(),
                    status: "running".into(),
                    batch_id: Some("batch-plain".into()),
                    model: Some("claude".into()),
                    last_progress: Some("Executing smoke tests".into()),
                    result: None,
                    error: None,
                    elapsed_ms: Some(18_750),
                    token_count: Some(8_900),
                },
            ],
            context: Some("regression sweep".into()),
            task: Some("Ship bugfix patch".into()),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(1),
            sequence_number: Some(order_seq),
        }),
    });
    event_seq += 1;
    order_seq += 1;

    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallEnd(CustomToolCallEndEvent {
            call_id: "agent-run-plain".into(),
            tool_name: "agent".into(),
            parameters: Some(json!({
                "action": "create",
                "batch_id": "batch-plain",
                "create": {
                    "name": "qa-bot",
                    "task": "Run targeted regression suite"
                }
            })),
            duration: Duration::from_secs(104),
            result: Ok("Regression sweep complete\n- 58 tests passed\n- 0 failures".into()),
        }),
    );

    let output = render_chat_widget_to_vt100(&mut harness, 80, 32);
    let output = normalize_output(output);
    insta::assert_snapshot!("agent_run_grouped_plain_tool_name", output);
}

#[test]
fn agents_terminal_overlay_full_details() {
    let mut harness = ChatWidgetHarness::new();
    let mut event_seq = 1;
    let mut order_seq = 1;

    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "agent-run".into(),
            tool_name: "agent".into(),
            parameters: Some(json!({
                "action": "create",
                "batch_id": "batch-docs",
                "create": {
                    "name": "Docs Sweep",
                    "task": "Compile the release highlights",
                    "context": "Focus on October 2025 product changes"
                }
            })),
        }),
    );

    harness.handle_event(Event {
        id: "agent-status-0".into(),
        event_seq,
        msg: EventMsg::AgentStatusUpdate(AgentStatusUpdateEvent {
            agents: vec![
                AgentInfo {
                    id: "docs-sweep-claude".into(),
                    name: "Docs Sweep (Claude)".into(),
                    status: "running".into(),
                    batch_id: Some("batch-docs".into()),
                    model: Some("claude-3-opus".into()),
                    last_progress: Some("Collecting release notes\nReviewing eng updates".into()),
                    result: None,
                    error: None,
                    elapsed_ms: Some(4_200),
                    token_count: Some(3_500),
                },
                AgentInfo {
                    id: "docs-sweep-gpt".into(),
                    name: "Docs Sweep (GPT)".into(),
                    status: "pending".into(),
                    batch_id: Some("batch-docs".into()),
                    model: Some("gpt-4o".into()),
                    last_progress: None,
                    result: None,
                    error: None,
                    elapsed_ms: Some(1_100),
                    token_count: None,
                },
            ],
            context: Some("Focus on October 2025 product changes".into()),
            task: Some("Compile the release highlights".into()),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(0),
            sequence_number: Some(order_seq),
        }),
    });
    event_seq += 1;
    order_seq += 1;

    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::AgentStatusUpdate(AgentStatusUpdateEvent {
            agents: vec![
                AgentInfo {
                    id: "docs-sweep-claude".into(),
                    name: "Docs Sweep (Claude)".into(),
                    status: "completed".into(),
                    batch_id: Some("batch-docs".into()),
                    model: Some("claude-3-opus".into()),
                    last_progress: Some("Synthesizing highlights".into()),
                    result: Some("### Highlights\n- New Auto Drive controls\n- Faster release approvals".into()),
                    error: None,
                    elapsed_ms: Some(12_700),
                    token_count: Some(7_200),
                },
                AgentInfo {
                    id: "docs-sweep-gpt".into(),
                    name: "Docs Sweep (GPT)".into(),
                    status: "failed".into(),
                    batch_id: Some("batch-docs".into()),
                    model: Some("gpt-4o".into()),
                    last_progress: Some("Drafting rollout summary".into()),
                    result: None,
                    error: Some("Timed out waiting for GitHub diff".into()),
                    elapsed_ms: Some(9_300),
                    token_count: Some(4_900),
                },
            ],
            context: Some("Focus on October 2025 product changes".into()),
            task: Some("Compile the release highlights".into()),
        }),
    );

    harness.send_key(make_key(KeyCode::Char('a'), KeyModifiers::CONTROL));

    let output = render_chat_widget_to_vt100(&mut harness, 96, 30);
    let output = normalize_output(output);
    insta::assert_snapshot!("agents_terminal_overlay_full_details", &output);
}

#[test]
fn plan_agent_keeps_single_aggregate_block() {
    let mut harness = ChatWidgetHarness::new();
    harness.push_user_prompt("/plan deduplicate agent aggregates");

    let mut event_seq = 0u64;
    let mut order_seq = 0u64;

    // Planner agent begins with an ordered event so the tracker stores a request-scoped key.
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "plan-call".into(),
            tool_name: "agent".into(),
            parameters: Some(json!({
                "action": "create",
                "batch_id": "batch-plan",
                "create": {
                    "name": "planner",
                    "task": "Draft implementation plan"
                }
            })),
        }),
    );

    // Status update arrives without ordering metadata; this rewrites the tracker key
    // to the batch form while leaving agent_run_by_order pointing at the old key.
    harness.handle_event(Event {
        id: "agent-status".into(),
        event_seq,
        msg: EventMsg::AgentStatusUpdate(AgentStatusUpdateEvent {
            agents: vec![AgentInfo {
                id: "planner".into(),
                name: "Planner".into(),
                status: "running".into(),
                batch_id: Some("batch-plan".into()),
                model: Some("gpt-4o".into()),
                last_progress: Some("refining steps".into()),
                result: None,
                error: None,
                elapsed_ms: Some(12_300),
                token_count: Some(6_100),
            }],
            context: Some("/plan coordination".into()),
            task: Some("Draft implementation plan".into()),
        }),
        order: None,
    });
    event_seq += 1;

    // A follow-up agent action arrives with ordering metadata. Because the status update above
    // rewired the tracker to a batch-scoped key without updating the order map, this ordered
    // begin cannot find the existing tracker and inserts a second aggregate block.
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "plan-result".into(),
            tool_name: "agent_result".into(),
            parameters: Some(json!({
                "action": "result"
            })),
        }),
    );

    let output = render_chat_widget_to_vt100(&mut harness, 80, 40);
    let agent_blocks = harness.count_agent_run_cells();
    assert_eq!(agent_blocks, 1, "expected a single aggregate agent block, saw {}\n{}", agent_blocks, output);
}

#[test]
fn settings_overlay_overview_layout() {
    let mut harness = ChatWidgetHarness::new();

    harness.suppress_rate_limit_refresh();
    harness.open_settings_overlay_overview();

    let frame = render_chat_widget_to_vt100(&mut harness, 100, 28);
    let output = normalize_output(frame);
    insta::assert_snapshot!("settings_overlay_overview_layout", output);
}

#[test]
fn settings_overlay_overview_truncates() {
    let mut harness = ChatWidgetHarness::new();

    harness.suppress_rate_limit_refresh();
    harness.open_settings_overlay_overview();

    let frame = render_chat_widget_to_vt100(&mut harness, 60, 20);
    let output = normalize_output(frame);
    insta::assert_snapshot!("settings_overlay_overview_truncates", output);
}

#[test]
fn settings_overview_hints_clean() {
    let mut harness = ChatWidgetHarness::new();

    harness.suppress_rate_limit_refresh();
    harness.open_settings_overlay_overview();

    let frame = render_chat_widget_to_vt100(&mut harness, 100, 22);
    let output = normalize_output(frame);
    assert!(
        output
            .lines()
            .any(|line| line.contains("Settings ▸ Overview")),
        "border title should show settings breadcrumb\n{}",
        output
    );
    assert!(
        output
            .lines()
            .any(|line| line.contains("↑ ↓ Move    Enter Open    Esc Close    ? Help")),
        "footer hints should appear on the last row\n{}",
        output
    );
    insta::assert_snapshot!("settings_overview_hints_clean", output);
}

#[test]
fn settings_overlay_theme_swatch_visible() {
    let mut harness = ChatWidgetHarness::new();

    harness.suppress_rate_limit_refresh();
    harness.open_settings_overlay_overview();

    let frame = render_chat_widget_to_vt100(&mut harness, 100, 28);
    let normalized = normalize_output(frame.clone());
    assert!(
        normalized.contains("Theme: "),
        "theme summary should include labeled theme value\n{}",
        frame
    );
    assert!(
        normalized.contains("Spinner:"),
        "theme summary should include spinner label\n{}",
        frame
    );
}

#[test]
fn settings_help_overlay_toggles() {
    let mut harness = ChatWidgetHarness::new();

    harness.suppress_rate_limit_refresh();
    harness.open_settings_overlay_overview();

    harness.send_key(make_key(KeyCode::Char('?'), KeyModifiers::NONE));
    let open = normalize_output(render_chat_widget_to_vt100(&mut harness, 100, 28));
    insta::assert_snapshot!("settings_help_overlay_open", open);

    harness.send_key(make_key(KeyCode::Esc, KeyModifiers::NONE));
    let closed = normalize_output(render_chat_widget_to_vt100(&mut harness, 100, 28));
    insta::assert_snapshot!("settings_help_overlay_closed", closed);
}

#[test]
fn settings_help_overlay_from_section() {
    let mut harness = ChatWidgetHarness::new();

    harness.suppress_rate_limit_refresh();
    harness.open_settings_overlay_overview();

    harness.send_key(make_key(KeyCode::Enter, KeyModifiers::NONE));
    harness.send_key(make_key(KeyCode::Char('?'), KeyModifiers::NONE));
    let section_open = normalize_output(render_chat_widget_to_vt100(&mut harness, 100, 28));
    insta::assert_snapshot!("settings_help_overlay_section_open", section_open);

    harness.send_key(make_key(KeyCode::Esc, KeyModifiers::NONE));
    let section_closed = normalize_output(render_chat_widget_to_vt100(&mut harness, 100, 28));
    insta::assert_snapshot!("settings_help_overlay_section_closed", section_closed);
}

#[test]
fn agent_status_missing_batch_displays_error() {
    let mut harness = ChatWidgetHarness::new();
    harness.push_user_prompt("/agents");

    harness.handle_event(Event {
        id: "agent-status-missing-batch".into(),
        event_seq: 0,
        msg: EventMsg::AgentStatusUpdate(AgentStatusUpdateEvent {
            agents: vec![AgentInfo {
                id: "orphan-agent".into(),
                name: "Orphan".into(),
                status: "running".into(),
                batch_id: None,
                model: Some("code".into()),
                last_progress: Some("trying something risky".into()),
                result: None,
                error: None,
                elapsed_ms: Some(8_000),
                token_count: Some(3_200),
            }],
            context: Some("debug orphan".into()),
            task: Some("Investigate logs".into()),
        }),
        order: None,
    });

    let output = render_chat_widget_to_vt100(&mut harness, 80, 20);
    let output = normalize_output(output);
    insta::assert_snapshot!("agent_status_missing_batch_displays_error", output);
}

#[test]
fn agents_toggle_claude_opus_persists_via_slash_command() {
    let _lock = ENV_LOCK.lock().unwrap();
    let env = EnvGuard::new(&["HOME", "CODE_HOME", "CODEX_HOME"]);
    let home_dir = TempDir::new().expect("temp home");
    let code_home = TempDir::new().expect("code home");
    env.set_path("HOME", home_dir.path());
    env.set_path("CODE_HOME", code_home.path());
    env.remove("CODEX_HOME");

    let mut harness = ChatWidgetHarness::new();

    harness.open_agents_settings_overlay();

    let overlay_initial = normalize_output(render_chat_widget_to_vt100(&mut harness, 100, 28));
    assert!(overlay_initial.contains("Agents"), "Agents overlay did not open");

    harness.show_agent_editor("claude-opus-4.1");

    let editor_frame = normalize_output(render_chat_widget_to_vt100(&mut harness, 100, 28));
    assert!(editor_frame.to_lowercase().contains("enabled"));

    harness.send_key(make_key(KeyCode::Char(' '), KeyModifiers::NONE));
    let editor_after_toggle = normalize_output(render_chat_widget_to_vt100(&mut harness, 100, 28));
    assert!(editor_after_toggle.to_lowercase().contains("disabled"));
    for _ in 0..4 {
        harness.send_key(make_key(KeyCode::Down, KeyModifiers::NONE));
    }
    harness.send_key(make_key(KeyCode::Enter, KeyModifiers::NONE));

    let overview_after_save = normalize_output(render_chat_widget_to_vt100(&mut harness, 100, 28));
    let overview_lower = overview_after_save.to_lowercase();
    assert!(overview_lower.contains("claude-opus-4.1"));
    assert!(overview_lower.contains("disabled"));

    harness.send_key(make_key(KeyCode::Esc, KeyModifiers::NONE));

    harness.open_agents_settings_overlay();
    harness.send_key(make_key(KeyCode::Down, KeyModifiers::NONE));
    harness.send_key(make_key(KeyCode::Down, KeyModifiers::NONE));
    let overlay_reopen = normalize_output(render_chat_widget_to_vt100(&mut harness, 100, 28));
    let reopen_lower = overlay_reopen.to_lowercase();
    assert!(reopen_lower.contains("claude-opus-4.1"));
    assert!(reopen_lower.contains("disabled"));
}

#[test]
fn agent_parallel_batches_do_not_duplicate_cells() {
    let mut harness = ChatWidgetHarness::new();
    harness.push_user_prompt("Run parallel meal plans");

    let mut event_seq = 0u64;
    let mut order_seq = 0u64;

    // Begin pizza batch
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "agent-pizza".into(),
            tool_name: "agent".into(),
            parameters: Some(json!({
                "action": "create",
                "batch_id": "batch-pizza",
                "create": {
                    "name": "Pizza Plan",
                    "task": "Plan how to make a pizza with prep and bake timelines"
                }
            })),
        }),
    );

    // Begin burger batch
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallBegin(CustomToolCallBeginEvent {
            call_id: "agent-burger".into(),
            tool_name: "agent".into(),
            parameters: Some(json!({
                "action": "create",
                "batch_id": "batch-burger",
                "create": {
                    "name": "Burger Plan",
                    "task": "Plan how to make a burger with toppings and timing"
                }
            })),
        }),
    );

    // Status update for both batches while running
    harness.handle_event(Event {
        id: "status-running".into(),
        event_seq,
        msg: EventMsg::AgentStatusUpdate(AgentStatusUpdateEvent {
            agents: vec![
                AgentInfo {
                    id: "pizza-agent".into(),
                    name: "Pizza Plan".into(),
                    status: "running".into(),
                    batch_id: Some("batch-pizza".into()),
                    model: Some("code".into()),
                    last_progress: Some("assembling ingredient checklist".into()),
                    result: None,
                    error: None,
                    elapsed_ms: Some(4_000),
                    token_count: Some(2_400),
                },
                AgentInfo {
                    id: "burger-agent".into(),
                    name: "Burger Plan".into(),
                    status: "running".into(),
                    batch_id: Some("batch-burger".into()),
                    model: Some("code".into()),
                    last_progress: Some("drafting grill timing".into()),
                    result: None,
                    error: None,
                    elapsed_ms: Some(3_200),
                    token_count: Some(1_900),
                },
            ],
            context: Some("Parallel meal planning".into()),
            task: Some("Plan how to make a pizza.".into()),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(1),
            sequence_number: Some(order_seq),
        }),
    });
    event_seq += 1;
    order_seq += 1;

    // Completion update with results
    harness.handle_event(Event {
        id: "status-complete".into(),
        event_seq,
        msg: EventMsg::AgentStatusUpdate(AgentStatusUpdateEvent {
            agents: vec![
                AgentInfo {
                    id: "pizza-agent".into(),
                    name: "Pizza Plan".into(),
                    status: "completed".into(),
                    batch_id: Some("batch-pizza".into()),
                    model: Some("code".into()),
                    last_progress: Some("documented bake schedule".into()),
                    result: Some("1. Prep dough\n2. Simmer sauce\n3. Bake at 475°F".into()),
                    error: None,
                    elapsed_ms: Some(9_500),
                    token_count: Some(4_200),
                },
                AgentInfo {
                    id: "burger-agent".into(),
                    name: "Burger Plan".into(),
                    status: "completed".into(),
                    batch_id: Some("batch-burger".into()),
                    model: Some("code".into()),
                    last_progress: Some("outlined topping staging".into()),
                    result: Some("1. Toast buns\n2. Grill patties\n3. Layer toppings".into()),
                    error: None,
                    elapsed_ms: Some(8_100),
                    token_count: Some(3_600),
                },
            ],
            context: Some("Parallel meal planning".into()),
            task: Some("Plan how to make a pizza.".into()),
        }),
        order: Some(OrderMeta {
            request_ordinal: 1,
            output_index: Some(2),
            sequence_number: Some(order_seq),
        }),
    });
    event_seq += 1;
    order_seq += 1;

    // End events for each batch
    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallEnd(CustomToolCallEndEvent {
            call_id: "agent-pizza".into(),
            tool_name: "agent".into(),
            parameters: Some(json!({
                "action": "create",
                "batch_id": "batch-pizza",
                "create": {
                    "name": "Pizza Plan",
                    "task": "Plan how to make a pizza with prep and bake timelines"
                }
            })),
            duration: Duration::from_secs(12),
            result: Ok("Pizza plan ready".into()),
        }),
    );

    push_ordered_event(
        &mut harness,
        &mut event_seq,
        &mut order_seq,
        EventMsg::CustomToolCallEnd(CustomToolCallEndEvent {
            call_id: "agent-burger".into(),
            tool_name: "agent".into(),
            parameters: Some(json!({
                "action": "create",
                "batch_id": "batch-burger",
                "create": {
                    "name": "Burger Plan",
                    "task": "Plan how to make a burger with toppings and timing"
                }
            })),
            duration: Duration::from_secs(10),
            result: Ok("Burger plan ready".into()),
        }),
    );

    let output = normalize_output(render_chat_widget_to_vt100(&mut harness, 80, 32));
    assert_eq!(harness.count_agent_run_cells(), 2, "expected one card per batch\n{output}");
    assert!(output.contains("Pizza Plan"), "missing pizza batch details\n{output}");
    assert!(output.contains("Burger Plan"), "missing burger batch details\n{output}");
    assert!(
        output.contains("Plan how to make a pizza with prep and bake timelines"),
        "pizza task missing\n{output}"
    );
    assert!(
        output.contains("Plan how to make a burger with toppings and timing"),
        "burger task missing or overwritten\n{output}"
    );
    assert!(!output.contains("batch-pizza"), "raw pizza batch id leaked into header\n{output}");
}
