//! Installation progress widget for displaying LSP server installation output.

use super::super::app::App;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let server_name = app.installing_server.as_deref().unwrap_or("Unknown");
    let title = format!(" Installing {} ", server_name);

    // Build output lines
    let mut lines: Vec<Line> = vec![
        Line::from(format!("Installing {}...", server_name).cyan().bold()),
        Line::from(""),
    ];

    // Show last N lines of install_output based on area height
    // Reserve 4 lines for: border (2), title line (1), empty line (1)
    let max_lines = (area.height as usize).saturating_sub(6);
    let start = app.install_output.len().saturating_sub(max_lines);

    for line in app.install_output.iter().skip(start) {
        // Color error lines in red
        if line.starts_with("ERROR:") || line.contains("error:") || line.contains("Error:") {
            lines.push(Line::from(line.as_str().red()));
        } else if line.starts_with("Successfully") {
            lines.push(Line::from(line.as_str().green().bold()));
        } else if line.starts_with("$") {
            // Command being executed
            lines.push(Line::from(line.as_str().yellow()));
        } else if line.starts_with("Added '") {
            // Config file updated
            lines.push(Line::from(line.as_str().green()));
        } else if line.starts_with("Warning:") {
            lines.push(Line::from(line.as_str().yellow()));
        } else {
            lines.push(Line::from(line.as_str().dim()));
        }
    }

    // Show loading indicator if still installing
    if app.loading {
        lines.push(Line::from(""));
        lines.push(Line::from("Installing...".dim().italic()));
    }

    let title_bottom = if app.loading {
        " Installing... "
    } else {
        " [Esc/q] Back to Servers "
    };

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .title_bottom(title_bottom)
                .borders(Borders::ALL)
                .border_style(Style::default().dim()),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}
