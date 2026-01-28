use std::io;

use ratatui::backend::Backend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::text::Line;
use ratatui::widgets::WidgetRef;

use crate::render::adapter_ratatui::from_ratatui_lines;
use crate::render::adapter_ratatui::to_ratatui_lines;
use crate::render::model::RenderLine;
use crate::wrapping::word_wrap_lines_borrowed;

/// Inserts ratatui lines above the viewport using the backend.
///
/// # Arguments
/// - `terminal` (&mut crate::custom_terminal::Terminal<B>): Terminal to update.
/// - `lines` (Vec<Line<'static>>): Lines to insert.
///
/// # Returns
/// - `io::Result<()>`: Result of the insert operation.
pub fn insert_history_ratatui_lines<B>(
    terminal: &mut crate::custom_terminal::Terminal<B>,
    lines: Vec<Line<'static>>,
) -> io::Result<()>
where
    B: Backend,
{
    let lines = from_ratatui_lines(&lines);
    insert_history_lines(terminal, lines)
}

/// Inserts render lines above the viewport using the backend.
///
/// # Arguments
/// - `terminal` (&mut crate::custom_terminal::Terminal<B>): Terminal to update.
/// - `lines` (Vec<RenderLine>): Lines to insert.
///
/// # Returns
/// - `io::Result<()>`: Result of the insert operation.
pub fn insert_history_lines<B>(
    terminal: &mut crate::custom_terminal::Terminal<B>,
    lines: Vec<RenderLine>,
) -> io::Result<()>
where
    B: Backend,
{
    let screen_size = terminal.backend().size().unwrap_or(Size::new(0, 0));
    let mut area = terminal.viewport_area;
    let mut should_update_area = false;
    let last_cursor_pos = terminal.last_known_cursor_pos;

    let wrapped = word_wrap_lines_borrowed(&lines, area.width.max(1) as usize);
    let wrapped = to_ratatui_lines(&wrapped);
    let wrapped_lines = wrapped.len() as u16;

    if area.bottom() < screen_size.height && wrapped_lines > 0 {
        let scroll_amount = wrapped_lines.min(screen_size.height - area.bottom());
        if scroll_amount > 0 {
            terminal
                .backend_mut()
                .scroll_region_down(area.top()..screen_size.height, scroll_amount)?;
            area.y = area.y.saturating_add(scroll_amount);
            should_update_area = true;
        }
    }

    let available = area.top();
    let visible_count = wrapped_lines.min(available);
    if visible_count > 0 {
        terminal
            .backend_mut()
            .scroll_region_up(0..area.top(), visible_count)?;
        let start_index = wrapped.len().saturating_sub(visible_count as usize);
        let start_y = area.top().saturating_sub(visible_count);
        for (offset, line) in wrapped[start_index..].iter().enumerate() {
            draw_line(
                terminal.backend_mut(),
                start_y.saturating_add(offset as u16),
                line,
                area.width,
            )?;
        }
    }

    terminal
        .backend_mut()
        .set_cursor_position(last_cursor_pos)?;
    if should_update_area {
        terminal.set_viewport_area(area);
    }
    Ok(())
}

/// Draws a single line to the backend at the given row.
///
/// # Arguments
/// - `backend` (&mut B): Backend to draw into.
/// - `y` (u16): Target row.
/// - `line` (&Line<'static>): Line to draw.
/// - `width` (u16): Line width.
///
/// # Returns
/// - `io::Result<()>`: Result of the draw operation.
fn draw_line<B>(backend: &mut B, y: u16, line: &Line<'static>, width: u16) -> io::Result<()>
where
    B: Backend,
{
    if width == 0 {
        return Ok(());
    }
    let area = Rect::new(0, 0, width, 1);
    let mut buffer = Buffer::empty(area);
    line.render_ref(area, &mut buffer);
    let content = buffer.content.iter().enumerate().map(|(index, cell)| {
        let (x, _) = buffer.pos_of(index);
        (x, y, cell)
    });
    backend.draw(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown_render::render_markdown_text;
    use crate::render::adapter_ratatui::from_ratatui_lines;
    use crate::render::model::RenderCell;
    use crate::render::model::RenderColor;
    use crate::test_backend::VT100Backend;
    use ratatui::layout::Rect;

    /// Verifies blockquote lines use a non-default color after insertion.
    #[test]
    fn vt100_blockquote_line_emits_green_fg() {
        let width: u16 = 40;
        let height: u16 = 10;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        let viewport = Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        let line = RenderLine::from(vec!["> ".into(), "Hello world".into()]).green();
        insert_history_lines(&mut term, vec![line]).expect("history insert failed");

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
        assert!(saw_colored, "expected colored cell in vt100 output");
    }

    /// Verifies wrapped blockquote lines keep non-default colors.
    #[test]
    fn vt100_blockquote_wrap_preserves_color_on_all_wrapped_lines() {
        let width: u16 = 20;
        let height: u16 = 8;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        let viewport = Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        let line = RenderLine::from(vec![
            "> ".into(),
            "This is a long quoted line that should wrap".into(),
        ])
        .green();

        insert_history_lines(&mut term, vec![line]).expect("history insert failed");

        let screen = term.backend().vt100().screen();
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

        assert!(
            non_empty_rows.len() >= 2,
            "expected wrapped output to span multiple rows"
        );

        for row in non_empty_rows {
            for col in 0..width {
                if let Some(cell) = screen.cell(row, col) {
                    let contents = cell.contents();
                    if !contents.is_empty() && contents != " " {
                        assert!(
                            cell.fgcolor() != vt100::Color::Default,
                            "expected non-default fg on row {row} col {col}"
                        );
                    }
                }
            }
        }
    }

    /// Verifies colored prefixes reset to default for subsequent spans.
    #[test]
    fn vt100_colored_prefix_then_plain_text_resets_color() {
        let width: u16 = 40;
        let height: u16 = 6;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        let viewport = Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        let line = RenderLine::from(vec![
            RenderCell::color("1. ", RenderColor::LightBlue),
            RenderCell::raw("Hello world"),
        ]);

        insert_history_lines(&mut term, vec![line]).expect("history insert failed");

        let screen = term.backend().vt100().screen();

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

            for col in 0..3 {
                let cell = screen.cell(row, col).unwrap();
                assert!(
                    cell.fgcolor() != vt100::Color::Default,
                    "expected colored prefix at col {col}"
                );
            }
            for col in 3..(3 + "Hello world".len() as u16) {
                let cell = screen.cell(row, col).unwrap();
                assert_eq!(
                    cell.fgcolor(),
                    vt100::Color::Default,
                    "expected default color for plain text at col {col}"
                );
            }
            break 'rows;
        }
    }

    /// Verifies nested list markers keep color at deeper nesting levels.
    #[test]
    fn vt100_deep_nested_mixed_list_third_level_marker_is_colored() {
        let md = "1. First\n   - Second level\n     1. Third level (ordered)\n        - Fourth level (bullet)\n          - Fifth level to test indent consistency\n";
        let text = render_markdown_text(md);
        let lines: Vec<RenderLine> = from_ratatui_lines(&text.lines);

        let width: u16 = 60;
        let height: u16 = 12;
        let backend = VT100Backend::new(width, height);
        let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        let viewport = ratatui::layout::Rect::new(0, height - 1, width, 1);
        term.set_viewport_area(viewport);

        insert_history_lines(&mut term, lines).expect("history insert failed");

        let screen = term.backend().vt100().screen();
        let rows: Vec<String> = screen.rows(0, width).collect();

        let target_row = rows
            .iter()
            .position(|row| row.contains("Third level"))
            .expect("expected third level row");

        let mut saw_marker = false;
        for col in 0..width {
            if let Some(cell) = screen.cell(target_row as u16, col)
                && cell.contents().contains('1')
            {
                assert!(
                    cell.fgcolor() != vt100::Color::Default,
                    "expected colored marker"
                );
                saw_marker = true;
                break;
            }
        }
        assert!(saw_marker, "expected to find a marker cell");
    }
}
