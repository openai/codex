#![cfg(feature = "vt100-tests")]
#![expect(clippy::expect_used)]

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;

// Small helper macro to assert a collection contains an item with a clearer
// failure message.
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

struct TestScenario {
    width: u16,
    height: u16,
    term: codex_tui::custom_terminal::Terminal<TestBackend>,
}

impl TestScenario {
    fn new(width: u16, height: u16, viewport: Rect) -> Self {
        let backend = TestBackend::new(width, height);
        let mut term = codex_tui::custom_terminal::Terminal::with_options(backend)
            .expect("failed to construct terminal");
        term.set_viewport_area(viewport);
        Self {
            width,
            height,
            term,
        }
    }

    fn run_insert(&mut self, lines: Vec<Line<'static>>) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::new();
        codex_tui::insert_history::insert_history_lines_to_writer(&mut self.term, &mut buf, lines);
        buf
    }

    fn screen_rows_from_bytes(&self, bytes: &[u8]) -> Vec<String> {
        let mut parser = vt100::Parser::new(self.height, self.width, 0);
        parser.process(bytes);
        let screen = parser.screen();

        let mut rows: Vec<String> = Vec::with_capacity(self.height as usize);
        for row in 0..self.height {
            let mut s = String::with_capacity(self.width as usize);
            for col in 0..self.width {
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
            rows.push(s.trim_end().to_string());
        }
        rows
    }
}

#[test]
fn basic_insertion_no_wrap() {
    // Screen of 20x6; viewport is the last row (height=1 at y=5)
    let area = Rect::new(0, 5, 20, 1);
    let mut scenario = TestScenario::new(20, 6, area);

    let lines = vec![Line::from("first"), Line::from("second")];
    let buf = scenario.run_insert(lines);
    let rows = scenario.screen_rows_from_bytes(&buf);
    assert_contains!(rows, String::from("first"));
    assert_contains!(rows, String::from("second"));
    let first_idx = rows
        .iter()
        .position(|r| r == "first")
        .expect("expected 'first' row to be present");
    let second_idx = rows
        .iter()
        .position(|r| r == "second")
        .expect("expected 'second' row to be present");
    assert_eq!(second_idx, first_idx + 1, "rows should be adjacent");
}

#[test]
fn long_token_wraps() {
    let area = Rect::new(0, 5, 20, 1);
    let mut scenario = TestScenario::new(20, 6, area);

    let long = "A".repeat(45); // > 2 lines at width 20
    let lines = vec![Line::from(long.clone())];
    let buf = scenario.run_insert(lines);
    let mut parser = vt100::Parser::new(6, 20, 0);
    parser.process(&buf);
    let screen = parser.screen();

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

#[test]
fn emoji_and_cjk() {
    let area = Rect::new(0, 5, 20, 1);
    let mut scenario = TestScenario::new(20, 6, area);

    let text = String::from("😀😀😀😀😀 你好世界");
    let lines = vec![Line::from(text.clone())];
    let buf = scenario.run_insert(lines);
    let rows = scenario.screen_rows_from_bytes(&buf);
    let reconstructed: String = rows.join("").chars().filter(|c| *c != ' ').collect();
    for ch in text.chars().filter(|c| !c.is_whitespace()) {
        assert!(
            reconstructed.contains(ch),
            "missing character {ch:?} in reconstructed screen"
        );
    }
}

#[test]
fn mixed_ansi_spans() {
    let area = Rect::new(0, 5, 20, 1);
    let mut scenario = TestScenario::new(20, 6, area);

    let line = Line::from(vec![
        Span::styled("red", Style::default().fg(Color::Red)),
        Span::raw("+plain"),
    ]);
    let buf = scenario.run_insert(vec![line]);
    let rows = scenario.screen_rows_from_bytes(&buf);
    assert_contains!(rows, String::from("red+plain"));
}

#[test]
fn cursor_restoration() {
    let area = Rect::new(0, 5, 20, 1);
    let mut scenario = TestScenario::new(20, 6, area);

    let lines = vec![Line::from("x")];
    let buf = scenario.run_insert(lines);
    let s = String::from_utf8_lossy(&buf);
    // CUP to 1;1 (ANSI: ESC[1;1H)
    assert!(
        s.contains("\u{1b}[1;1H"),
        "expected final CUP to 1;1 in output, got: {s:?}"
    );
    // Reset scroll region
    assert!(
        s.contains("\u{1b}[r"),
        "expected reset scroll region in output, got: {s:?}"
    );
}

#[test]
fn word_wrap_no_mid_word_split() {
    // Screen of 40x10; viewport is the last row
    let area = Rect::new(0, 9, 40, 1);
    let mut scenario = TestScenario::new(40, 10, area);

    let sample = "Years passed, and Willowmere thrived in peace and friendship. Mira’s herb garden flourished with both ordinary and enchanted plants, and travelers spoke of the kindness of the woman who tended them.";
    let buf = scenario.run_insert(vec![Line::from(sample)]);
    let rows = scenario.screen_rows_from_bytes(&buf);
    let joined = rows.join("\n");
    assert!(
        !joined.contains("bo\nth"),
        "word 'both' should not be split across lines:\n{joined}"
    );
}

#[test]
fn em_dash_and_space_word_wrap() {
    // Repro from report: ensure we break before "inside", not mid-word.
    let area = Rect::new(0, 9, 40, 1);
    let mut scenario = TestScenario::new(40, 10, area);

    let sample = "Mara found an old key on the shore. Curious, she opened a tarnished box half-buried in sand—and inside lay a single, glowing seed.";
    let buf = scenario.run_insert(vec![Line::from(sample)]);
    let rows = scenario.screen_rows_from_bytes(&buf);
    let joined = rows.join("\n");
    assert!(
        !joined.contains("insi\nde"),
        "word 'inside' should not be split across lines:\n{joined}"
    );
}

#[test]
fn pre_scroll_region_down() {
    // Viewport not at bottom: y=3 (0-based), height=1
    let area = Rect::new(0, 3, 20, 1);
    let mut scenario = TestScenario::new(20, 6, area);

    let lines = vec![Line::from("first"), Line::from("second")];
    let buf = scenario.run_insert(lines);
    let s = String::from_utf8_lossy(&buf);
    // Expect we limited scroll region to [top+1 .. screen_height] => [4 .. 6] (1-based)
    assert!(
        s.contains("\u{1b}[4;6r"),
        "expected pre-scroll SetScrollRegion 4..6, got: {s:?}"
    );
    // Expect we moved cursor to top of that region: row 3 (0-based) => CUP 4;1H
    assert!(
        s.contains("\u{1b}[4;1H"),
        "expected cursor at top of pre-scroll region, got: {s:?}"
    );
    // Expect at least two Reverse Index commands (ESC M) for two inserted lines
    let ri_count = s.matches("\u{1b}M").count();
    assert!(
        ri_count >= 1,
        "expected at least one RI (ESC M), got: {s:?}"
    );
    // After pre-scroll, we set insertion scroll region to [1 .. new_top] => [1 .. 5]
    assert!(
        s.contains("\u{1b}[1;5r"),
        "expected insertion SetScrollRegion 1..5, got: {s:?}"
    );
}
