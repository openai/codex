//! Exercises `insert_history` rendering paths with a vt100-backed terminal.
//!
//! These tests validate the history insertion path used by the TUI to ensure
//! cursor restoration, word wrapping, and mixed-width glyph handling work with
//! a fixed viewport.

#![cfg(feature = "vt100-tests")]
#![expect(clippy::expect_used)]

use crate::test_backend::VT100Backend;
use pretty_assertions::assert_eq;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;

/// Asserts a collection contains the expected item with a clearer failure message.
macro_rules! assert_contains {
    ($collection:expr, $item:expr $(,)?) => {
        assert!(
            $collection.contains(&$item),
            "Expected {:?} to contain {:?}",
            $collection,
            $item
        );
    };
    ($collection:expr, $item:expr, $($arg:tt)+) => {
        assert!($collection.contains(&$item), $($arg)+);
    };
}

/// Holds the terminal and viewport configuration for vt100 history assertions.
struct TestScenario {
    term: codex_tui::custom_terminal::Terminal<VT100Backend>,
}

impl TestScenario {
    /// Builds a vt100-backed terminal with a custom viewport for history insertion.
    fn new(width: u16, height: u16, viewport: Rect) -> Self {
        let backend = VT100Backend::new(width, height);
        let mut term = codex_tui::custom_terminal::Terminal::with_options(backend)
            .expect("failed to construct terminal");
        term.set_viewport_area(viewport);
        Self { term }
    }

    /// Inserts history lines into the terminal while preserving cursor state.
    fn run_insert(&mut self, lines: Vec<Line<'static>>) {
        codex_tui::insert_history::insert_history_lines(&mut self.term, lines)
            .expect("Failed to insert history lines in test");
    }
}

/// Confirms that short lines land in the history region without wrapping.
#[test]
fn basic_insertion_no_wrap() {
    // Screen of 20x6; viewport is the last row (height=1 at y=5)
    let area = Rect::new(0, 5, 20, 1);
    let mut scenario = TestScenario::new(20, 6, area);

    let lines = vec!["first".into(), "second".into()];
    scenario.run_insert(lines);
    let rows = scenario.term.backend().vt100().screen().contents();
    assert_contains!(rows, String::from("first"));
    assert_contains!(rows, String::from("second"));
}

/// Ensures vt100 wrapping does not drop any bytes when a single token overflows.
#[test]
fn long_token_wraps() {
    let area = Rect::new(0, 5, 20, 1);
    let mut scenario = TestScenario::new(20, 6, area);

    let long = "A".repeat(45); // > 2 lines at width 20
    let lines = vec![long.clone().into()];
    scenario.run_insert(lines);
    let screen = scenario.term.backend().vt100().screen();

    // Count total A's on the screen
    let mut count_a = 0usize;
    for row in 0..6 {
        for col in 0..20 {
            if let Some(cell) = screen.cell(row, col)
                && let Some(ch) = cell.contents().chars().next()
                && ch == 'A'
            {
                count_a += 1;
            }
        }
    }

    assert_eq!(
        count_a,
        long.len(),
        "wrapped content did not preserve all characters"
    );
}

/// Verifies multicolumn emoji and CJK glyphs survive the history insertion path.
#[test]
fn emoji_and_cjk() {
    let area = Rect::new(0, 5, 20, 1);
    let mut scenario = TestScenario::new(20, 6, area);

    let text = String::from("😀😀😀😀😀 你好世界");
    let lines = vec![text.clone().into()];
    scenario.run_insert(lines);
    let rows = scenario.term.backend().vt100().screen().contents();
    for ch in text.chars().filter(|c| !c.is_whitespace()) {
        assert!(
            rows.contains(ch),
            "missing character {ch:?} in reconstructed screen"
        );
    }
}

/// Checks that ANSI-styled spans preserve text content in vt100 output.
#[test]
fn mixed_ansi_spans() {
    let area = Rect::new(0, 5, 20, 1);
    let mut scenario = TestScenario::new(20, 6, area);

    let line = vec!["red".red(), "+plain".into()].into();
    scenario.run_insert(vec![line]);
    let rows = scenario.term.backend().vt100().screen().contents();
    assert_contains!(rows, String::from("red+plain"));
}

/// Verifies history insertion restores the cursor position for future edits.
#[test]
fn cursor_restoration() {
    let area = Rect::new(0, 5, 20, 1);
    let mut scenario = TestScenario::new(20, 6, area);

    let lines = vec!["x".into()];
    scenario.run_insert(lines);
    assert_eq!(scenario.term.last_known_cursor_pos, (0, 0).into());
}

/// Ensures the word wrapper never splits a word across terminal lines.
#[test]
fn word_wrap_no_mid_word_split() {
    // Screen of 40x10; viewport is the last row
    let area = Rect::new(0, 9, 40, 1);
    let mut scenario = TestScenario::new(40, 10, area);

    let sample = "Years passed, and Willowmere thrived in peace and friendship. Mira’s herb garden flourished with both ordinary and enchanted plants, and travelers spoke of the kindness of the woman who tended them.";
    scenario.run_insert(vec![sample.into()]);
    let joined = scenario.term.backend().vt100().screen().contents();
    assert!(
        !joined.contains("bo\nth"),
        "word 'both' should not be split across lines:\n{joined}"
    );
}

/// Guards the em-dash spacing case that previously split a word mid-line.
#[test]
fn em_dash_and_space_word_wrap() {
    // Repro from report: ensure we break before "inside", not mid-word.
    let area = Rect::new(0, 9, 40, 1);
    let mut scenario = TestScenario::new(40, 10, area);

    let sample = "Mara found an old key on the shore. Curious, she opened a tarnished box half-buried in sand—and inside lay a single, glowing seed.";
    scenario.run_insert(vec![sample.into()]);
    let joined = scenario.term.backend().vt100().screen().contents();
    assert!(
        !joined.contains("insi\nde"),
        "word 'inside' should not be split across lines:\n{joined}"
    );
}
