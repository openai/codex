//! Input box widgets for file and symbol input.

use super::super::app::App;
use super::super::app::InputState;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

/// Create an input line with cursor highlighting.
fn input_line_with_cursor(input: &InputState) -> Line<'_> {
    let (before, after) = input.text.split_at(input.cursor.min(input.text.len()));
    let cursor_char = after.chars().next().unwrap_or(' ');
    let after_cursor = if after.is_empty() {
        ""
    } else {
        &after[cursor_char.len_utf8()..]
    };

    Line::from(vec![
        " > ".cyan(),
        before.into(),
        Span::styled(
            cursor_char.to_string(),
            Style::default().bg(Color::Gray).fg(Color::Black),
        ),
        after_cursor.into(),
    ])
}

pub fn render_file_input(app: &App, frame: &mut Frame, area: Rect) {
    let op_name = app
        .operation
        .map(|op| op.display_name())
        .unwrap_or("Unknown");

    let mut lines = vec![
        Line::from(vec![" Operation: ".dim(), op_name.cyan().bold()]),
        Line::from(""),
        Line::from(" Enter file path (relative to workspace):".dim()),
        Line::from(""),
    ];

    lines.push(input_line_with_cursor(&app.file_input));

    // Hints
    lines.push(Line::from(""));
    lines.push(Line::from(
        " Hint: Use relative path like 'src/lib.rs'".dim(),
    ));

    Paragraph::new(lines)
        .block(
            Block::default()
                .title(" File Input ".bold())
                .borders(Borders::ALL)
                .border_style(Style::default().dim()),
        )
        .render(area, frame.buffer_mut());

    // Set cursor position (offset: 1 border + 3 chars " > ")
    let cursor_x = area.x + 4 + app.file_input.cursor as u16;
    let cursor_y = area.y + 5;
    frame.set_cursor_position(Position::new(cursor_x, cursor_y));
}

pub fn render_symbol_input(app: &App, frame: &mut Frame, area: Rect) {
    let op_name = app
        .operation
        .map(|op| op.display_name())
        .unwrap_or("Unknown");

    let file_name = app
        .current_file
        .as_ref()
        .map(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.display().to_string())
        })
        .unwrap_or_else(|| "None".to_string());

    let mut lines = vec![
        Line::from(vec![
            " Operation: ".dim(),
            op_name.cyan().bold(),
            " | ".dim(),
            "File: ".dim(),
            file_name.into(),
        ]),
        Line::from(""),
        Line::from(" Enter symbol name:".dim()),
        Line::from(""),
    ];

    lines.push(input_line_with_cursor(&app.symbol_input));

    // Hints
    lines.push(Line::from(""));
    lines.push(Line::from(
        " Hint: Enter function/struct/trait name (e.g., 'Config', 'new')".dim(),
    ));

    Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Symbol Input ".bold())
                .borders(Borders::ALL)
                .border_style(Style::default().dim()),
        )
        .render(area, frame.buffer_mut());

    // Set cursor position (offset: 1 border + 3 chars " > ")
    let cursor_x = area.x + 4 + app.symbol_input.cursor as u16;
    let cursor_y = area.y + 5;
    frame.set_cursor_position(Position::new(cursor_x, cursor_y));
}
