use std::fmt;
use std::io;
use std::io::Write;

use crate::wrapping::word_wrap_lines_borrowed;
use crossterm::Command;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Color as CColor;
use crossterm::style::Colors;
use crossterm::style::Print;
use crossterm::style::SetAttribute;
use crossterm::style::SetBackgroundColor;
use crossterm::style::SetColors;
use crossterm::style::SetForegroundColor;
use crossterm::terminal::Clear;
use crossterm::terminal::ClearType;
use ratatui::layout::Size;
use ratatui::prelude::Backend;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::text::Line;
use ratatui::text::Span;

/// Insert `lines` above the viewport using the terminal's backend writer
/// (avoids direct stdout references).
pub fn insert_history_lines<B>(
    terminal: &mut crate::custom_terminal::Terminal<B>,
    lines: Vec<Line>,
) -> io::Result<()>
where
    B: Backend + Write,
{
    let use_native_scrollback_history_mode = should_use_native_scrollback_history_mode();
    insert_history_lines_with_mode(terminal, lines, use_native_scrollback_history_mode)
}

fn insert_history_lines_with_mode<B>(
    terminal: &mut crate::custom_terminal::Terminal<B>,
    lines: Vec<Line>,
    use_native_scrollback_history_mode: bool,
) -> io::Result<()>
where
    B: Backend + Write,
{
    let screen_size = terminal.backend().size().unwrap_or(Size::new(0, 0));

    let mut area = terminal.viewport_area;
    let mut should_update_area = false;
    // Restore to the last explicit app cursor (composer/input), not the transient backend cursor.
    let restore_cursor_pos = terminal
        .last_explicit_cursor_pos
        .unwrap_or(terminal.last_known_cursor_pos);
    let writer = terminal.backend_mut();

    // Pre-wrap lines using word-aware wrapping so terminal scrollback sees the same
    // formatting as the TUI. This avoids character-level hard wrapping by the terminal.
    let wrapped = word_wrap_lines_borrowed(&lines, area.width.max(1) as usize);
    let wrapped_lines = wrapped.len() as u16;
    let cursor_top = if area.bottom() < screen_size.height {
        // If the viewport is not at the bottom of the screen, scroll it down to make room.
        // Don't scroll it past the bottom of the screen.
        let available_bottom_gap = screen_size.height - area.bottom();
        let scroll_amount = if use_native_scrollback_history_mode {
            // In native scrollback mode, full-screen scroll insertion already advances history.
            // Pre-shifting viewport metadata introduces blank rows between streamed chunks.
            0
        } else {
            wrapped_lines.min(available_bottom_gap)
        };
        let old_top = area.top();

        if !use_native_scrollback_history_mode {
            // Emit ANSI to scroll the lower region (from the top of the viewport to the bottom
            // of the screen) downward by `scroll_amount` lines. We do this by:
            //   1) Limiting the scroll region to [area.top()+1 .. screen_height] (1-based bounds)
            //   2) Placing the cursor at the top margin of that region
            //   3) Emitting Reverse Index (RI, ESC M) `scroll_amount` times
            //   4) Resetting the scroll region back to full screen
            let top_1based = old_top + 1; // Convert 0-based row to 1-based for DECSTBM
            queue!(writer, SetScrollRegion(top_1based..screen_size.height))?;
            queue!(writer, MoveTo(0, old_top))?;
            for _ in 0..scroll_amount {
                // Reverse Index (RI): ESC M
                queue!(writer, Print("\x1bM"))?;
            }
            queue!(writer, ResetScrollRegion)?;
        }

        if scroll_amount > 0 {
            area.y += scroll_amount;
            should_update_area = true;
        }

        if use_native_scrollback_history_mode {
            // In native scrollback mode we print history lines after a full-screen scroll.
            // Anchor writes to one row above the viewport top so streamed lines remain contiguous
            // across chunks without introducing artificial spacing.
            area.top().saturating_sub(1)
        } else {
            old_top.saturating_sub(1)
        }
    } else {
        // If the viewport is not at the bottom of the screen, scroll it down to make room.
        area.top().saturating_sub(1)
    };

    if use_native_scrollback_history_mode && area.top() > 0 {
        if screen_size.height == 0 {
            return Ok(());
        }
        queue!(writer, ResetScrollRegion)?;
        for line in wrapped {
            // Some terminals do not reliably preserve scrollback for lines scrolled out of DECSTBM
            // regions. Force a natural full-screen scroll by emitting CRLF on the last row.
            queue!(writer, MoveTo(0, screen_size.height.saturating_sub(1)))?;
            queue!(writer, Print("\r\n"))?;
            queue!(writer, MoveTo(0, cursor_top))?;
            queue!(
                writer,
                SetColors(Colors::new(
                    line.style
                        .fg
                        .map(std::convert::Into::into)
                        .unwrap_or(CColor::Reset),
                    line.style
                        .bg
                        .map(std::convert::Into::into)
                        .unwrap_or(CColor::Reset)
                ))
            )?;
            queue!(writer, Clear(ClearType::UntilNewLine))?;
            let merged_spans: Vec<Span> = line
                .spans
                .iter()
                .map(|s| Span {
                    style: s.style.patch(line.style),
                    content: s.content.clone(),
                })
                .collect();
            write_spans(writer, merged_spans.iter())?;
        }

        queue!(writer, MoveTo(restore_cursor_pos.x, restore_cursor_pos.y))?;

        let _ = writer;
        if should_update_area {
            terminal.set_viewport_area(area);
        }
        // Native mode insertion scrolls the whole screen, so invalidate the
        // previous frame cache to force a full repaint on the next draw.
        terminal.invalidate_previous_frame();

        return Ok(());
    }

    // Limit the scroll region to the lines from the top of the screen to the
    // top of the viewport. With this in place, when we add lines inside this
    // area, only the lines in this area will be scrolled. We place the cursor
    // at the end of the scroll region, and add lines starting there.
    //
    // ┌─Screen───────────────────────┐
    // │┌╌Scroll region╌╌╌╌╌╌╌╌╌╌╌╌╌╌┐│
    // │┆                            ┆│
    // │┆                            ┆│
    // │┆                            ┆│
    // │█╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┘│
    // │╭─Viewport───────────────────╮│
    // ││                            ││
    // │╰────────────────────────────╯│
    // └──────────────────────────────┘
    queue!(writer, SetScrollRegion(1..area.top()))?;

    // NB: we are using MoveTo instead of set_cursor_position here to avoid messing with the
    // terminal's last_known_cursor_position, which hopefully will still be accurate after we
    // fetch/restore the cursor position. insert_history_lines should be cursor-position-neutral :)
    queue!(writer, MoveTo(0, cursor_top))?;

    for line in wrapped {
        queue!(writer, Print("\r\n"))?;
        queue!(
            writer,
            SetColors(Colors::new(
                line.style
                    .fg
                    .map(std::convert::Into::into)
                    .unwrap_or(CColor::Reset),
                line.style
                    .bg
                    .map(std::convert::Into::into)
                    .unwrap_or(CColor::Reset)
            ))
        )?;
        queue!(writer, Clear(ClearType::UntilNewLine))?;
        // Merge line-level style into each span so that ANSI colors reflect
        // line styles (e.g., blockquotes with green fg).
        let merged_spans: Vec<Span> = line
            .spans
            .iter()
            .map(|s| Span {
                style: s.style.patch(line.style),
                content: s.content.clone(),
            })
            .collect();
        write_spans(writer, merged_spans.iter())?;
    }

    queue!(writer, ResetScrollRegion)?;

    // Restore the cursor position to where it was before we started.
    queue!(writer, MoveTo(restore_cursor_pos.x, restore_cursor_pos.y))?;

    let _ = writer;
    if should_update_area {
        terminal.set_viewport_area(area);
    }

    Ok(())
}

fn should_use_native_scrollback_history_mode() -> bool {
    // Enable this path for environments where lines scrolled out of DECSTBM regions may not be
    // appended to global scrollback. Zellij is the currently known case.
    std::env::var_os("ZELLIJ").is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetScrollRegion(pub std::ops::Range<u16>);

impl Command for SetScrollRegion {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[{};{}r", self.0.start, self.0.end)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        panic!("tried to execute SetScrollRegion command using WinAPI, use ANSI instead");
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        // TODO(nornagon): is this supported on Windows?
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetScrollRegion;

impl Command for ResetScrollRegion {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[r")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        panic!("tried to execute ResetScrollRegion command using WinAPI, use ANSI instead");
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        // TODO(nornagon): is this supported on Windows?
        true
    }
}

struct ModifierDiff {
    pub from: Modifier,
    pub to: Modifier,
}

impl ModifierDiff {
    fn queue<W>(self, mut w: W) -> io::Result<()>
    where
        W: io::Write,
    {
        use crossterm::style::Attribute as CAttribute;
        let removed = self.from - self.to;
        if removed.contains(Modifier::REVERSED) {
            queue!(w, SetAttribute(CAttribute::NoReverse))?;
        }
        if removed.contains(Modifier::BOLD) {
            queue!(w, SetAttribute(CAttribute::NormalIntensity))?;
            if self.to.contains(Modifier::DIM) {
                queue!(w, SetAttribute(CAttribute::Dim))?;
            }
        }
        if removed.contains(Modifier::ITALIC) {
            queue!(w, SetAttribute(CAttribute::NoItalic))?;
        }
        if removed.contains(Modifier::UNDERLINED) {
            queue!(w, SetAttribute(CAttribute::NoUnderline))?;
        }
        if removed.contains(Modifier::DIM) {
            queue!(w, SetAttribute(CAttribute::NormalIntensity))?;
        }
        if removed.contains(Modifier::CROSSED_OUT) {
            queue!(w, SetAttribute(CAttribute::NotCrossedOut))?;
        }
        if removed.contains(Modifier::SLOW_BLINK) || removed.contains(Modifier::RAPID_BLINK) {
            queue!(w, SetAttribute(CAttribute::NoBlink))?;
        }

        let added = self.to - self.from;
        if added.contains(Modifier::REVERSED) {
            queue!(w, SetAttribute(CAttribute::Reverse))?;
        }
        if added.contains(Modifier::BOLD) {
            queue!(w, SetAttribute(CAttribute::Bold))?;
        }
        if added.contains(Modifier::ITALIC) {
            queue!(w, SetAttribute(CAttribute::Italic))?;
        }
        if added.contains(Modifier::UNDERLINED) {
            queue!(w, SetAttribute(CAttribute::Underlined))?;
        }
        if added.contains(Modifier::DIM) {
            queue!(w, SetAttribute(CAttribute::Dim))?;
        }
        if added.contains(Modifier::CROSSED_OUT) {
            queue!(w, SetAttribute(CAttribute::CrossedOut))?;
        }
        if added.contains(Modifier::SLOW_BLINK) {
            queue!(w, SetAttribute(CAttribute::SlowBlink))?;
        }
        if added.contains(Modifier::RAPID_BLINK) {
            queue!(w, SetAttribute(CAttribute::RapidBlink))?;
        }

        Ok(())
    }
}

fn write_spans<'a, I>(mut writer: &mut impl Write, content: I) -> io::Result<()>
where
    I: IntoIterator<Item = &'a Span<'a>>,
{
    let mut fg = Color::Reset;
    let mut bg = Color::Reset;
    let mut last_modifier = Modifier::empty();
    for span in content {
        let mut modifier = Modifier::empty();
        modifier.insert(span.style.add_modifier);
        modifier.remove(span.style.sub_modifier);
        if modifier != last_modifier {
            let diff = ModifierDiff {
                from: last_modifier,
                to: modifier,
            };
            diff.queue(&mut writer)?;
            last_modifier = modifier;
        }
        let next_fg = span.style.fg.unwrap_or(Color::Reset);
        let next_bg = span.style.bg.unwrap_or(Color::Reset);
        if next_fg != fg || next_bg != bg {
            queue!(
                writer,
                SetColors(Colors::new(next_fg.into(), next_bg.into()))
            )?;
            fg = next_fg;
            bg = next_bg;
        }

        queue!(writer, Print(span.content.clone()))?;
    }

    queue!(
        writer,
        SetForegroundColor(CColor::Reset),
        SetBackgroundColor(CColor::Reset),
        SetAttribute(crossterm::style::Attribute::Reset),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_cell::AgentMessageCell;
    use crate::history_cell::HistoryCell;
    use crate::markdown_render::render_markdown_text;
    use crate::test_backend::VT100Backend;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use ratatui::text::Text;
    use ratatui::widgets::Paragraph;

    #[test]
    fn writes_bold_then_regular_spans() {
        use ratatui::style::Stylize;

        let spans = ["A".bold(), "B".into()];

        let mut actual: Vec<u8> = Vec::new();
        write_spans(&mut actual, spans.iter()).unwrap();

        let mut expected: Vec<u8> = Vec::new();
        queue!(
            expected,
            SetAttribute(crossterm::style::Attribute::Bold),
            Print("A"),
            SetAttribute(crossterm::style::Attribute::NormalIntensity),
            Print("B"),
            SetForegroundColor(CColor::Reset),
            SetBackgroundColor(CColor::Reset),
            SetAttribute(crossterm::style::Attribute::Reset),
        )
        .unwrap();

        assert_eq!(
            String::from_utf8(actual).unwrap(),
            String::from_utf8(expected).unwrap()
        );
    }

    #[test]
    fn vt100_blockquote_line_emits_green_fg() {
        // Set up a small off-screen terminal
        let width: u16 = 40;
        let height: u16 = 10;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        // Place viewport on the last line so history inserts scroll upward
        let viewport = Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        // Build a blockquote-like line: apply line-level green style and prefix "> "
        let mut line: Line<'static> = Line::from(vec!["> ".into(), "Hello world".into()]);
        line = line.style(Color::Green);
        insert_history_lines(&mut term, vec![line])
            .expect("Failed to insert history lines in test");

        let mut saw_colored = false;
        'outer: for row in 0..height {
            for col in 0..width {
                if let Some(cell) = term.backend().vt100().screen().cell(row, col)
                    && cell.has_contents()
                    && cell.fgcolor() != vt100::Color::Default
                {
                    saw_colored = true;
                    break 'outer;
                }
            }
        }
        assert!(
            saw_colored,
            "expected at least one colored cell in vt100 output"
        );
    }

    #[test]
    fn vt100_blockquote_wrap_preserves_color_on_all_wrapped_lines() {
        // Force wrapping by using a narrow viewport width and a long blockquote line.
        let width: u16 = 20;
        let height: u16 = 8;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        // Viewport is the last line so history goes directly above it.
        let viewport = Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        // Create a long blockquote with a distinct prefix and enough text to wrap.
        let mut line: Line<'static> = Line::from(vec![
            "> ".into(),
            "This is a long quoted line that should wrap".into(),
        ]);
        line = line.style(Color::Green);

        insert_history_lines(&mut term, vec![line])
            .expect("Failed to insert history lines in test");

        // Parse and inspect the final screen buffer.
        let screen = term.backend().vt100().screen();

        // Collect rows that are non-empty; these should correspond to our wrapped lines.
        let mut non_empty_rows: Vec<u16> = Vec::new();
        for row in 0..height {
            let mut any = false;
            for col in 0..width {
                if let Some(cell) = screen.cell(row, col)
                    && cell.has_contents()
                    && cell.contents() != "\0"
                    && cell.contents() != " "
                {
                    any = true;
                    break;
                }
            }
            if any {
                non_empty_rows.push(row);
            }
        }

        // Expect at least two rows due to wrapping.
        assert!(
            non_empty_rows.len() >= 2,
            "expected wrapped output to span >=2 rows, got {non_empty_rows:?}",
        );

        // For each non-empty row, ensure all non-space cells are using a non-default fg color.
        for row in non_empty_rows {
            for col in 0..width {
                if let Some(cell) = screen.cell(row, col) {
                    let contents = cell.contents();
                    if !contents.is_empty() && contents != " " {
                        assert!(
                            cell.fgcolor() != vt100::Color::Default,
                            "expected non-default fg on row {row} col {col}, got {:?}",
                            cell.fgcolor()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn vt100_colored_prefix_then_plain_text_resets_color() {
        let width: u16 = 40;
        let height: u16 = 6;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        let viewport = Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        // First span colored, rest plain.
        let line: Line<'static> = Line::from(vec![
            Span::styled("1. ", ratatui::style::Style::default().fg(Color::LightBlue)),
            Span::raw("Hello world"),
        ]);

        insert_history_lines(&mut term, vec![line])
            .expect("Failed to insert history lines in test");

        let screen = term.backend().vt100().screen();

        // Find the first non-empty row; verify first three cells are colored, following cells default.
        'rows: for row in 0..height {
            let mut has_text = false;
            for col in 0..width {
                if let Some(cell) = screen.cell(row, col)
                    && cell.has_contents()
                    && cell.contents() != " "
                {
                    has_text = true;
                    break;
                }
            }
            if !has_text {
                continue;
            }

            // Expect "1. Hello world" starting at col 0.
            for col in 0..3 {
                let cell = screen.cell(row, col).unwrap();
                assert!(
                    cell.fgcolor() != vt100::Color::Default,
                    "expected colored prefix at col {col}, got {:?}",
                    cell.fgcolor()
                );
            }
            for col in 3..(3 + "Hello world".len() as u16) {
                let cell = screen.cell(row, col).unwrap();
                assert_eq!(
                    cell.fgcolor(),
                    vt100::Color::Default,
                    "expected default color for plain text at col {col}, got {:?}",
                    cell.fgcolor()
                );
            }
            break 'rows;
        }
    }

    fn insert_streamed_response_in_two_chunks(
        term: &mut crate::custom_terminal::Terminal<VT100Backend>,
        width: u16,
        token: &str,
        start: u16,
        end: u16,
    ) {
        assert!(start <= end);

        let first =
            AgentMessageCell::new(vec![Line::from(format!("{token} line {start:02}"))], true);
        let first_display = first.display_lines(width);
        assert_eq!(
            first_display.len(),
            1,
            "first streamed chunk should emit exactly one line",
        );
        assert!(
            !first_display
                .iter()
                .any(crate::render::line_utils::is_blank_line_spaces_only),
            "first streamed chunk display should not contain blank lines",
        );
        insert_history_lines_with_mode(term, first_display, true)
            .expect("failed inserting first response chunk");

        if start < end {
            let rest_lines: Vec<Line<'static>> = ((start + 1)..=end)
                .map(|n| Line::from(format!("{token} line {n:02}")))
                .collect();
            let rest = AgentMessageCell::new(rest_lines, false);
            let rest_display = rest.display_lines(width);
            assert_eq!(
                rest_display.len(),
                usize::from(end - start),
                "continuation chunk should emit one line per numbered row",
            );
            assert!(
                !rest_display
                    .iter()
                    .any(crate::render::line_utils::is_blank_line_spaces_only),
                "continuation streamed chunk display should not contain blank lines",
            );
            insert_history_lines_with_mode(term, rest_display, true)
                .expect("failed inserting continuation response chunk");
        }
    }

    fn row_index_containing(rows: &[String], needle: &str) -> usize {
        rows.iter()
            .position(|row| row.contains(needle))
            .unwrap_or_else(|| panic!("could not find row containing {needle:?}"))
    }

    fn rows_contain(rows: &[String], needle: &str) -> bool {
        rows.iter().any(|row| row.contains(needle))
    }

    mod native_scrollback_suite {
        use super::*;

        #[test]
        fn native_scrollback_mode_keeps_viewport_metadata_stable_for_varied_inserts() {
            let cases: [(&str, u16, u16, Rect, u16); 4] = [
                // Deliberately leave rows below the viewport to mirror inline mode setups where
                // the viewport can lag behind the screen bottom.
                ("multiline", 32, 12, Rect::new(0, 6, 32, 3), 5),
                // Bottom gap is 2 rows (viewport bottom=10, screen bottom=12).
                ("large-multiline", 40, 12, Rect::new(0, 7, 40, 3), 6),
                ("single-line", 40, 12, Rect::new(0, 7, 40, 3), 1),
                // Bottom gap is 20 rows (viewport bottom=20, screen bottom=40).
                ("single-line-large-gap", 80, 40, Rect::new(0, 10, 80, 10), 1),
            ];

            for (name, width, height, viewport, line_count) in cases {
                let backend = VT100Backend::new(width, height);
                let mut term =
                    crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
                term.set_viewport_area(viewport);

                let lines: Vec<Line<'static>> = (1..=line_count)
                    .map(|i| Line::from(format!("{name} line {i:02}")))
                    .collect();
                insert_history_lines_with_mode(&mut term, lines, true)
                    .expect("failed to insert history lines in native scrollback mode test");

                assert_eq!(
                    term.viewport_area, viewport,
                    "expected native scrollback mode to leave viewport metadata unchanged for {name}",
                );
            }
        }

        #[test]
        fn native_scrollback_mode_preserves_oldest_lines_when_history_overflows_viewport_region() {
            fn run_case(use_native_scrollback_mode: bool) -> (Vec<String>, Vec<String>) {
                let width: u16 = 77;
                let height: u16 = 42;
                let backend = VT100Backend::new_with_scrollback(width, height, 4096);
                let mut term =
                    crate::custom_terminal::Terminal::with_options(backend).expect("terminal");

                // Keep viewport above bottom so insertion must go through pre-viewport history
                // region logic.
                term.set_viewport_area(Rect::new(0, 20, width, 20));

                let lines = AgentMessageCell::new(
                    (1..=50)
                        .map(|n| Line::from(format!("OVERFLOW_CASE line {n:02}")))
                        .collect(),
                    true,
                )
                .display_lines(width);

                insert_history_lines_with_mode(&mut term, lines, use_native_scrollback_mode)
                    .expect("insert");

                // Capture oldest visible scrollback slice.
                term.backend_mut()
                    .vt100_mut()
                    .screen_mut()
                    .set_scrollback(4096);
                let top_rows: Vec<String> =
                    term.backend().vt100().screen().rows(0, width).collect();

                // Capture live bottom slice.
                term.backend_mut()
                    .vt100_mut()
                    .screen_mut()
                    .set_scrollback(0);
                let bottom_rows: Vec<String> =
                    term.backend().vt100().screen().rows(0, width).collect();

                (top_rows, bottom_rows)
            }

            let (without_newline_top, without_newline_bottom) = run_case(false);
            let (with_newline_top, with_newline_bottom) = run_case(true);

            assert!(
                !rows_contain(&without_newline_top, "OVERFLOW_CASE line 01"),
                "expected old DECSTBM-only path to drop earliest history under overflow"
            );
            assert!(
                rows_contain(&with_newline_top, "OVERFLOW_CASE line 01"),
                "expected native scrollback mode to preserve earliest history line under overflow"
            );
            assert!(
                rows_contain(&without_newline_bottom, "OVERFLOW_CASE line 50"),
                "expected old path to keep latest history at live bottom"
            );
            assert!(
                rows_contain(&with_newline_bottom, "OVERFLOW_CASE line 50"),
                "expected native scrollback mode to keep latest history at live bottom"
            );
        }

        #[test]
        fn single_line_chunk_uses_bottom_row_newline_scroll() {
            let width: u16 = 77;
            let height: u16 = 42;
            let backend = VT100Backend::new(width, height);
            let mut term =
                crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
            term.set_viewport_area(Rect::new(0, 20, width, 20));

            let first = AgentMessageCell::new(vec![Line::from("CMD_CASE line 01")], true);
            insert_history_lines_with_mode(&mut term, first.display_lines(width), true)
                .expect("insert");

            let output = String::from_utf8_lossy(term.backend().write_log()).into_owned();
            let bottom_move = format!("\x1b[{height};1H");
            assert!(
                output.contains(&bottom_move),
                "single-line chunk should use bottom-row newline scroll path in native scrollback mode (missing {bottom_move:?} in output: {output:?})",
            );
            let output_bytes = output.as_bytes();
            assert!(output_bytes.windows(2).any(|window| window == b"\r\n"));
        }

        #[test]
        fn native_scrollback_mode_restores_cursor_to_previous_position() {
            let width: u16 = 64;
            let height: u16 = 24;
            let backend = VT100Backend::new(width, height);
            let mut term =
                crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
            term.set_viewport_area(Rect::new(0, 10, width, 10));
            term.set_cursor_position((4, 20)).expect("set cursor");

            let lines = vec![
                Line::from("line 01"),
                Line::from("line 02"),
                Line::from("line 03"),
            ];
            insert_history_lines_with_mode(&mut term, lines, true).expect("insert");

            let output = String::from_utf8_lossy(term.backend().write_log()).into_owned();
            let expected_restore = "\x1b[21;5H";
            assert!(
                output.contains(expected_restore),
                "expected cursor restore to preserve previous position in native scrollback mode (missing {expected_restore:?} in output: {output:?})",
            );
        }

        #[test]
        fn native_scrollback_mode_restores_last_explicit_cursor_when_cursor_state_drifts() {
            let width: u16 = 64;
            let height: u16 = 24;
            let backend = VT100Backend::new(width, height);
            let mut term =
                crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
            term.set_viewport_area(Rect::new(0, 10, width, 10));

            // Seed explicit app cursor.
            term.set_cursor_position((4, 20))
                .expect("set initial cursor");

            // Drift the backend cursor and cached metadata independently.
            queue!(term.backend_mut(), MoveTo(11, 18)).expect("move backend cursor");
            term.last_known_cursor_pos = ratatui::layout::Position { x: 1, y: 1 };
            assert_eq!(term.last_explicit_cursor_pos, Some((4, 20).into()));

            insert_history_lines_with_mode(&mut term, vec![Line::from("line 01")], true)
                .expect("insert");

            let restored = term.get_cursor_position().expect("get restored cursor");
            assert_eq!(
                restored,
                ratatui::layout::Position { x: 4, y: 20 },
                "expected native scrollback mode to restore the last explicit app cursor position",
            );
        }

        #[test]
        fn native_scrollback_mode_keeps_prompt_and_footer_on_bottom_rows_after_insert() {
            let width: u16 = 64;
            let height: u16 = 16;
            let backend = VT100Backend::new_with_scrollback(width, height, 256);
            let mut term =
                crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
            term.set_viewport_area(Rect::new(0, 2, width, height.saturating_sub(2)));

            let draw_layout = |term: &mut crate::custom_terminal::Terminal<VT100Backend>| {
                term.draw(|frame| {
                    let footer_row = frame.area().height.saturating_sub(1);
                    let prompt_row = frame.area().height.saturating_sub(2);
                    let rows: Vec<Line<'static>> = (0..frame.area().height)
                        .map(|row| match row {
                            r if r == prompt_row => {
                                Line::from("› Summarize recent commits".to_string())
                            }
                            r if r == footer_row => {
                                Line::from("? for shortcuts  100% context left".to_string())
                            }
                            _ => Line::from(format!("frame row {row:02}")),
                        })
                        .collect();
                    let paragraph = Paragraph::new(Text::from(rows));
                    frame.render_widget_ref(paragraph, frame.area());
                })
                .expect("draw");
            };

            draw_layout(&mut term);
            insert_history_lines_with_mode(&mut term, vec![Line::from("history insert 01")], true)
                .expect("insert");
            // Render the same frame again to mirror app draws where bottom pane text
            // is unchanged across history insertions.
            draw_layout(&mut term);

            term.backend_mut()
                .vt100_mut()
                .screen_mut()
                .set_scrollback(0);
            let rows: Vec<String> = term.backend().vt100().screen().rows(0, width).collect();
            let prompt_row = usize::from(height.saturating_sub(2));
            let footer_row = usize::from(height.saturating_sub(1));
            assert!(
                rows[prompt_row].contains("Summarize") && rows[prompt_row].contains("commits"),
                "expected prompt row {prompt_row} to be repainted after native history insert, got {:?}",
                rows[prompt_row],
            );
            assert!(
                rows[footer_row].contains("shortcuts") && rows[footer_row].contains("context"),
                "expected footer row {footer_row} to be repainted after native history insert, got {:?}",
                rows[footer_row],
            );
        }
    }

    mod native_scrollback_spacing_suite {
        use super::*;

        #[test]
        fn streamed_numbered_lines_remain_contiguous_across_chunks() {
            let width: u16 = 120;
            let height: u16 = 240;
            let backend = VT100Backend::new(width, height);
            let mut term =
                crate::custom_terminal::Terminal::with_options(backend).expect("terminal");

            // Keep a gap below viewport so native scrollback mode has to exercise its
            // viewport-shift path.
            term.set_viewport_area(Rect::new(0, 200, width, 20));

            // Raw assistant payloads in session logs have no blank lines between these numbers.
            insert_streamed_response_in_two_chunks(&mut term, width, "SPACING_CASE", 1, 50);
            insert_streamed_response_in_two_chunks(&mut term, width, "SPACING_CASE", 51, 60);
            insert_streamed_response_in_two_chunks(&mut term, width, "SPACING_CASE", 61, 80);

            let rows: Vec<String> = term.backend().vt100().screen().rows(0, width).collect();

            for (lhs, rhs) in [(1_u16, 2_u16), (51_u16, 52_u16), (61_u16, 62_u16)] {
                let left = row_index_containing(&rows, &format!("SPACING_CASE line {lhs:02}"));
                let right = row_index_containing(&rows, &format!("SPACING_CASE line {rhs:02}"));
                assert_eq!(
                    right,
                    left + 1,
                    "expected contiguous rows for {lhs:02}->{rhs:02}, got row {left} then {right}",
                );
            }
        }

        #[test]
        fn two_single_line_stream_inserts_stay_adjacent() {
            let width: u16 = 120;
            let height: u16 = 160;
            let backend = VT100Backend::new(width, height);
            let mut term =
                crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
            term.set_viewport_area(Rect::new(0, 120, width, 20));

            insert_history_lines_with_mode(&mut term, vec![Line::from("MINCASE line 01")], true)
                .expect("first insert");
            insert_history_lines_with_mode(&mut term, vec![Line::from("MINCASE line 02")], true)
                .expect("second insert");

            let rows: Vec<String> = term.backend().vt100().screen().rows(0, width).collect();
            let r1 = row_index_containing(&rows, "MINCASE line 01");
            let r2 = row_index_containing(&rows, "MINCASE line 02");
            assert_eq!(
                r2,
                r1 + 1,
                "expected contiguous rows for MINCASE line 01->02, got row {r1} then {r2}",
            );
        }

        #[test]
        fn single_large_insert_keeps_line_01_and_02_adjacent() {
            let width: u16 = 77;
            let height: u16 = 42;
            let backend = VT100Backend::new_with_scrollback(width, height, 2048);
            let mut term =
                crate::custom_terminal::Terminal::with_options(backend).expect("terminal");

            // Mirror live inline-mode layout where viewport can sit above the last rows.
            term.set_viewport_area(Rect::new(0, 20, width, 20));

            let lines = AgentMessageCell::new(
                (1..=20)
                    .map(|n| Line::from(format!("LIVE_BIG line {n:02}")))
                    .collect(),
                true,
            )
            .display_lines(width);

            insert_history_lines_with_mode(&mut term, lines, true).expect("insert");

            term.backend_mut()
                .vt100_mut()
                .screen_mut()
                .set_scrollback(2048);
            let rows: Vec<String> = term.backend().vt100().screen().rows(0, width).collect();
            let r1 = row_index_containing(&rows, "LIVE_BIG line 01");
            let r2 = row_index_containing(&rows, "LIVE_BIG line 02");
            assert_eq!(
                r2,
                r1 + 1,
                "expected contiguous rows for LIVE_BIG line 01->02, got row {r1} then {r2}",
            );
        }

        #[test]
        fn streamed_numbered_lines_remain_contiguous_with_interleaved_draws() {
            let width: u16 = 77;
            let height: u16 = 42;
            let backend = VT100Backend::new_with_scrollback(width, height, 4096);
            let mut term =
                crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
            term.set_viewport_area(Rect::new(0, 20, width, 20));

            let draw_frame = |term: &mut crate::custom_terminal::Terminal<VT100Backend>| {
                term.draw(|frame| {
                    // Render full viewport rows so we model the regular frame lifecycle.
                    let filler = (0..frame.area().height)
                        .map(|row| Line::from(format!("FRAME row {row:02}")))
                        .collect::<Vec<_>>();
                    let paragraph = Paragraph::new(Text::from(filler));
                    frame.render_widget_ref(paragraph, frame.area());
                })
                .expect("draw");
            };

            let first = AgentMessageCell::new(vec![Line::from("DRAW_CASE line 01")], true);
            insert_history_lines_with_mode(&mut term, first.display_lines(width), true)
                .expect("01");
            draw_frame(&mut term);

            let mid = AgentMessageCell::new(
                (2..=6)
                    .map(|n| Line::from(format!("DRAW_CASE line {n:02}")))
                    .collect(),
                false,
            );
            insert_history_lines_with_mode(&mut term, mid.display_lines(width), true)
                .expect("02-06");
            draw_frame(&mut term);

            let next = AgentMessageCell::new(vec![Line::from("DRAW_CASE line 07")], true);
            insert_history_lines_with_mode(&mut term, next.display_lines(width), true).expect("07");
            draw_frame(&mut term);

            let next_tail = AgentMessageCell::new(
                (8..=12)
                    .map(|n| Line::from(format!("DRAW_CASE line {n:02}")))
                    .collect(),
                false,
            );
            insert_history_lines_with_mode(&mut term, next_tail.display_lines(width), true)
                .expect("08-12");
            draw_frame(&mut term);

            let third = AgentMessageCell::new(vec![Line::from("DRAW_CASE line 13")], true);
            insert_history_lines_with_mode(&mut term, third.display_lines(width), true)
                .expect("13");
            draw_frame(&mut term);

            let third_tail = AgentMessageCell::new(
                (14..=18)
                    .map(|n| Line::from(format!("DRAW_CASE line {n:02}")))
                    .collect(),
                false,
            );
            insert_history_lines_with_mode(&mut term, third_tail.display_lines(width), true)
                .expect("14-18");

            term.backend_mut()
                .vt100_mut()
                .screen_mut()
                .set_scrollback(0);
            let rows: Vec<String> = term.backend().vt100().screen().rows(0, width).collect();
            for (lhs, rhs) in [(1_u16, 2_u16), (7_u16, 8_u16), (13_u16, 14_u16)] {
                let left = row_index_containing(&rows, &format!("DRAW_CASE line {lhs:02}"));
                let right = row_index_containing(&rows, &format!("DRAW_CASE line {rhs:02}"));
                assert_eq!(
                    right,
                    left + 1,
                    "expected contiguous rows for DRAW_CASE {lhs:02}->{rhs:02}, got row {left} then {right}",
                );
            }
        }

        #[test]
        fn five_line_stream_keeps_line_01_and_02_adjacent_in_small_viewport() {
            let width: u16 = 77;
            let height: u16 = 42;
            let backend = VT100Backend::new_with_scrollback(width, height, 4096);
            let mut term =
                crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
            term.set_viewport_area(Rect::new(0, 20, width, 20));

            let draw_frame = |term: &mut crate::custom_terminal::Terminal<VT100Backend>| {
                term.draw(|frame| {
                    let filler = (0..frame.area().height)
                        .map(|row| Line::from(format!("FRAME5 row {row:02}")))
                        .collect::<Vec<_>>();
                    let paragraph = Paragraph::new(Text::from(filler));
                    frame.render_widget_ref(paragraph, frame.area());
                })
                .expect("draw");
            };

            // Prime scrollback/view state similarly to a long-running interactive session.
            let warmup = AgentMessageCell::new(
                (1..=120)
                    .map(|n| Line::from(format!("WARMUP line {n:03}")))
                    .collect(),
                true,
            );
            insert_history_lines_with_mode(&mut term, warmup.display_lines(width), true)
                .expect("warmup");
            draw_frame(&mut term);

            // Mirror the live pattern: first streamed chunk carries the initial bullet line,
            // then continuation lines are inserted in the next chunk.
            let first = AgentMessageCell::new(vec![Line::from("FIVE_CASE line 01")], true);
            let rest = AgentMessageCell::new(
                (2..=5)
                    .map(|n| Line::from(format!("FIVE_CASE line {n:02}")))
                    .collect(),
                false,
            );

            insert_history_lines_with_mode(&mut term, first.display_lines(width), true)
                .expect("01");
            draw_frame(&mut term);
            insert_history_lines_with_mode(&mut term, rest.display_lines(width), true)
                .expect("02-05");

            let mut found: Option<(usize, usize, usize)> = None;
            for scrollback in 0..=4096 {
                term.backend_mut()
                    .vt100_mut()
                    .screen_mut()
                    .set_scrollback(scrollback);
                let rows: Vec<String> = term.backend().vt100().screen().rows(0, width).collect();
                if let Some(r1) = rows
                    .iter()
                    .position(|row| row.contains("FIVE_CASE line 01"))
                    && let Some(r2) = rows
                        .iter()
                        .position(|row| row.contains("FIVE_CASE line 02"))
                {
                    found = Some((scrollback, r1, r2));
                    break;
                }
            }
            let Some((scrollback, r1, r2)) = found else {
                panic!("could not find FIVE_CASE line 01/02 in any scrollback slice");
            };
            assert_eq!(
                r2,
                r1 + 1,
                "expected contiguous rows for FIVE_CASE line 01->02, got row {r1} then {r2} at scrollback={scrollback}",
            );
        }
    }

    #[test]
    fn vt100_deep_nested_mixed_list_third_level_marker_is_colored() {
        // Markdown with five levels (ordered → unordered → ordered → unordered → unordered).
        let md = "1. First\n   - Second level\n     1. Third level (ordered)\n        - Fourth level (bullet)\n          - Fifth level to test indent consistency\n";
        let text = render_markdown_text(md);
        let lines: Vec<Line<'static>> = text.lines.clone();

        let width: u16 = 60;
        let height: u16 = 12;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        let viewport = ratatui::layout::Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        insert_history_lines(&mut term, lines).expect("Failed to insert history lines in test");

        let screen = term.backend().vt100().screen();

        // Reconstruct screen rows as strings to locate the 3rd level line.
        let rows: Vec<String> = screen.rows(0, width).collect();

        let needle = "1. Third level (ordered)";
        let row_idx = rows
            .iter()
            .position(|r| r.contains(needle))
            .unwrap_or_else(|| {
                panic!("expected to find row containing {needle:?}, have rows: {rows:?}")
            });
        let col_start = rows[row_idx].find(needle).unwrap() as u16; // column where '1' starts

        // Verify that the numeric marker ("1.") at the third level is colored
        // (non-default fg) and the content after the following space resets to default.
        for c in [col_start, col_start + 1] {
            let cell = screen.cell(row_idx as u16, c).unwrap();
            assert!(
                cell.fgcolor() != vt100::Color::Default,
                "expected colored 3rd-level marker at row {row_idx} col {c}, got {:?}",
                cell.fgcolor()
            );
        }
        let content_col = col_start + 3; // skip '1', '.', and the space
        if let Some(cell) = screen.cell(row_idx as u16, content_col) {
            assert_eq!(
                cell.fgcolor(),
                vt100::Color::Default,
                "expected default color for 3rd-level content at row {row_idx} col {content_col}, got {:?}",
                cell.fgcolor()
            );
        }
    }
}
