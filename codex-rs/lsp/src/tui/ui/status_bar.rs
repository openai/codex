//! Status bar widget showing server status and current file.

use super::super::app::App;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let title = " LSP Test TUI ";

    // Server status
    let server_status = if app.client.is_some() {
        "connected".green()
    } else {
        "disconnected".red()
    };

    // Current file
    let file_info = app
        .current_file
        .as_ref()
        .map(|p| {
            // Show relative path if possible
            p.strip_prefix(&app.workspace)
                .unwrap_or(p)
                .display()
                .to_string()
        })
        .unwrap_or_else(|| "No file".to_string());

    // Operation info
    let op_info = app.operation.map(|op| op.display_name()).unwrap_or("None");

    // Build status lines
    let lines = vec![
        Line::from(vec![
            " Server: ".dim(),
            server_status,
            " | ".dim(),
            "Workspace: ".dim(),
            app.workspace.display().to_string().cyan(),
        ]),
        Line::from(vec![
            " File: ".dim(),
            file_info.into(),
            " | ".dim(),
            "Operation: ".dim(),
            op_info.cyan(),
        ]),
    ];

    Paragraph::new(lines)
        .block(
            Block::default()
                .title(title.bold())
                .borders(Borders::ALL)
                .border_style(Style::default().dim()),
        )
        .render(area, frame.buffer_mut());
}
