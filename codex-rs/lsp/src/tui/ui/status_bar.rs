//! Status bar widget showing server status and current file.

use super::super::app::App;
use codex_lsp::ServerStatus;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let title = " LSP Test TUI ";

    // Server status - show running count if we have cached server info
    let server_status = if !app.cached_servers.is_empty() {
        let running_count = app
            .cached_servers
            .iter()
            .filter(|s| matches!(s.status, ServerStatus::Running))
            .count();
        let total_count = app.cached_servers.len();
        if running_count > 0 {
            format!("{running_count}/{total_count} running").green()
        } else {
            format!("0/{total_count} running").dim()
        }
    } else if app.client.is_some() {
        "connected".green()
    } else {
        "disconnected".dim()
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
