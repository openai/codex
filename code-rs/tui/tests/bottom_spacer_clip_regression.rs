#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use regex_lite::Regex;
use std::sync::OnceLock;
use code_core::protocol::{
    AgentReasoningDeltaEvent, AgentReasoningEvent, Event, EventMsg, OrderMeta,
};
use code_tui::test_helpers::{layout_metrics, render_chat_widget_to_vt100, ChatWidgetHarness};

#[test]
fn bottom_spacer_short_wrapped_content_80x24() {
    let mut harness = ChatWidgetHarness::new();
    seed_short_wrapped_transcript(&mut harness);

    let raw = render_chat_widget_to_vt100(&mut harness, 80, 24);
    assert_viewport_invariants(&raw, 24);
    let output = normalize_output(raw);
    insta::assert_snapshot!("bottom_spacer_short_wrapped_80x24", output);
}

#[test]
fn bottom_spacer_overflow_wrapped_content_100x30() {
    let mut harness = ChatWidgetHarness::new();
    seed_overflow_wrapped_transcript(&mut harness);

    let raw = render_chat_widget_to_vt100(&mut harness, 100, 30);
    assert_viewport_invariants(&raw, 30);
    let output = normalize_output(raw);
    insta::assert_snapshot!("bottom_spacer_overflow_wrapped_100x30", output);
}

#[test]
fn bottom_spacer_collapsed_vs_expanded_reasoning_120x40() {
    let mut harness = ChatWidgetHarness::new();
    seed_reasoning_transcript(&mut harness);

    let expanded_raw = render_chat_widget_to_vt100(&mut harness, 120, 40);
    assert_viewport_invariants(&expanded_raw, 40);
    let expanded = normalize_output(expanded_raw);

    // Toggle reasoning visibility (Ctrl+R)
    harness.send_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    let collapsed_raw = render_chat_widget_to_vt100(&mut harness, 120, 40);
    assert_viewport_invariants(&collapsed_raw, 40);
    let collapsed = normalize_output(collapsed_raw);

    insta::assert_snapshot!(
        "bottom_spacer_collapsed_reasoning_120x40",
        collapsed
    );
    insta::assert_snapshot!(
        "bottom_spacer_expanded_reasoning_120x40",
        expanded
    );
}

#[test]
fn bottom_spacer_skips_when_history_fits() {
    let mut harness = ChatWidgetHarness::new();
    harness.push_user_prompt("Keep it brief.");
    harness.push_assistant_markdown("The transcript stays short so it fits without scrolling.");

    let _ = render_chat_widget_to_vt100(&mut harness, 120, 40);
    let metrics = layout_metrics(&harness);

    assert!(
        metrics.last_max_scroll >= 4,
        "with the command header we intentionally reserve at least four spacer rows, got {}",
        metrics.last_max_scroll
    );
}

fn seed_short_wrapped_transcript(harness: &mut ChatWidgetHarness) {
    harness.push_user_prompt(
        "Could you provide a detailed overview of how the anti-clipping spacer works?\n\nMake sure the explanation spans multiple lines so we can exercise wrapping in the viewport.",
    );
    harness.push_assistant_markdown(
        "Absolutely! The spacer is activated whenever the rendered history height lands directly on or near the viewport boundary.\n\nThis explanation intentionally repeats a few phrases so that the text wraps even on wider terminals, ensuring we cover the regression scenario where the final row historically vanished.",
    );
}

fn seed_overflow_wrapped_transcript(harness: &mut ChatWidgetHarness) {
    harness.push_user_prompt("Let’s keep talking so we can overflow the viewport.");
    for idx in 0..8 {
        harness.push_assistant_markdown(format!(
            "Assistant reply #{idx} contains a deliberately verbose paragraph that discusses wrapping,\nstream padding, viewport math, and the importance of not clipping the final line.\n\nWe repeat the insight to guarantee the total height exceeds the viewport."
        ));
        harness.push_user_prompt(format!(
            "User follow-up #{idx}: keep adding details about history layout calculations and how they interact with scroll offsets."
        ));
    }
    harness.push_assistant_markdown(
        "Final assistant response to seal the deal with yet another wrapped explanation.\nIt mentions CODE_TUI_ANTICUTOFF repeatedly to highlight the feature’s role in protecting the last row of history.",
    );
}

fn seed_reasoning_transcript(harness: &mut ChatWidgetHarness) {
    harness.push_user_prompt("Share your reasoning and then your final answer.");

    let reasoning_chunks = [
        "The assistant considers multiple hypotheses.",
        "It weighs edge cases involving narrow terminals.",
        "It confirms the spacer should activate when height hits exact multiples of the viewport.",
    ];

    for (i, chunk) in reasoning_chunks.iter().enumerate() {
        harness.handle_event(Event {
            id: "reasoning-1".into(),
            event_seq: (i as u64) + 1,
            msg: EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent {
                delta: format!("{chunk}\n"),
            }),
            order: Some(OrderMeta {
                request_ordinal: 7,
                output_index: Some(0),
                sequence_number: Some(i as u64),
            }),
        });
    }

    let final_reasoning = reasoning_chunks.join("\n");
    harness.handle_event(Event {
        id: "reasoning-1".into(),
        event_seq: (reasoning_chunks.len() as u64) + 1,
        msg: EventMsg::AgentReasoning(AgentReasoningEvent {
            text: final_reasoning,
        }),
        order: Some(OrderMeta {
            request_ordinal: 7,
            output_index: Some(0),
            sequence_number: Some(reasoning_chunks.len() as u64 + 1),
        }),
    });

    harness.push_assistant_markdown(
        "Having reasoned through the constraints, the assistant now presents a final answer summarising why the spacer preserves the last visible line."
    );
}

fn assert_viewport_invariants(output: &str, expected_rows: u16) {
    let lines: Vec<&str> = output.lines().collect();
    assert!(
        lines.len() >= expected_rows.saturating_sub(1) as usize,
        "snapshot lost too many rows: expected ≈{expected_rows}, got {}",
        lines.len()
    );
    assert!(
        lines.len() <= expected_rows as usize,
        "snapshot exceeded viewport height: expected {expected_rows}, got {}",
        lines.len()
    );
    assert!(
        output.contains("Ctrl+H help"),
        "composer footer is expected to remain visible"
    );
    if let Some(last_line) = lines.last() {
        assert!(
            last_line.trim().is_empty() || last_line.contains("help"),
            "last visible line looks truncated: {last_line:?}"
        );
    }
}

fn normalize_output(text: String) -> String {
    text
        .chars()
        .map(normalize_glyph)
        .collect::<String>()
        .pipe(normalize_timers)
        .pipe(normalize_spacer_rows)
}

fn normalize_glyph(ch: char) -> char {
    match ch {
        '┌' | '┐' | '└' | '┘'
        | '┏' | '┓' | '┗' | '┛'
        | '╔' | '╗' | '╚' | '╝'
        | '╒' | '╕' | '╛' | '╜'
        | '╓' | '╖' | '╙' | '╘'
        | '╭' | '╮' | '╯' | '╰' => '+',
        '┬' | '┴' | '┼' | '├' | '┤'
        | '┽' | '┾' | '┿'
        | '╀' | '╁' | '╂' | '╃' | '╄'
        | '╅' | '╆' | '╇' | '╈' | '╉'
        | '╊' | '╋' | '╟' | '╠' | '╡'
        | '╢' | '╫' | '╪' | '╬' => '+',
        '─' | '━' | '═' | '╼' | '╾'
        | '╸' | '╺' | '╴' | '╶'
        | '┄' | '┅' | '┈' | '┉' => '-',
        '│' | '┃' | '║' | '╽' | '╿'
        | '╏' | '╎' | '┆' | '┇'
        | '╷' | '╹' => '|',
        other => other,
    }
}

fn normalize_timers(text: String) -> String {
    static IN_SECONDS_RE: OnceLock<Regex> = OnceLock::new();
    static T_MINUS_RE: OnceLock<Regex> = OnceLock::new();
    static MM_SS_RE: OnceLock<Regex> = OnceLock::new();
    static MS_RE: OnceLock<Regex> = OnceLock::new();
    static MIN_SEC_RE: OnceLock<Regex> = OnceLock::new();

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

    let text = MS_RE
        .get_or_init(|| Regex::new(r"\b\d+ms\b").expect("valid milliseconds regex"))
        .replace_all(&text, "Xms")
        .into_owned();

    MIN_SEC_RE
        .get_or_init(|| Regex::new(r"\b\d+m\s+\d+s\b").expect("valid minute-second regex"))
        .replace_all(&text, "Xm Ys")
        .into_owned()
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

    if ends_with_newline {
        lines.push(String::new());
    }

    lines.join("\n")
}

fn is_spacer_border_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('+') && trimmed.ends_with('+')
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
