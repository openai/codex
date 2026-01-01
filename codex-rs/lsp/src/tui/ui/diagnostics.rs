//! Diagnostics view widget.

use super::super::app::App;
use super::utils::relative_path_buf;
use codex_lsp::DiagnosticSeverityLevel;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let diagnostics = &app.cached_diagnostics;

    if diagnostics.is_empty() {
        let lines = vec![Line::from(""), Line::from(" No diagnostics reported".dim())];
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Diagnostics ".bold())
                    .borders(Borders::ALL)
                    .border_style(Style::default().dim()),
            )
            .render(area, frame.buffer_mut());
        return;
    }

    let mut lines = vec![Line::from(format!(
        " {} diagnostic(s) reported:",
        diagnostics.len()
    ))];
    lines.push(Line::from(""));

    for (i, diag) in diagnostics.iter().enumerate() {
        // Severity indicator
        let severity_span = match diag.severity {
            DiagnosticSeverityLevel::Error => "E".red().bold(),
            DiagnosticSeverityLevel::Warning => "W".yellow().bold(),
            DiagnosticSeverityLevel::Info => "I".cyan(),
            DiagnosticSeverityLevel::Hint => "H".dim(),
        };

        // File path (relative if possible)
        let file_path = relative_path_buf(&diag.file, &app.workspace);

        let line_num = diag.line;

        // First line: severity, file, line
        let header_line = Line::from(vec![
            format!(" {}. [", i + 1).dim(),
            severity_span,
            "] ".dim(),
            file_path.cyan(),
            ":".dim(),
            line_num.to_string().into(),
        ]);
        lines.push(header_line);

        // Second line: message (truncated if too long)
        // Use chars() instead of byte slicing to handle UTF-8 correctly
        let msg = if diag.message.chars().count() > 80 {
            let truncated: String = diag.message.chars().take(77).collect();
            format!("{truncated}...")
        } else {
            diag.message.clone()
        };
        lines.push(Line::from(format!("    {msg}").dim()));
    }

    // Apply scroll offset
    let total_lines = lines.len();
    let visible_lines: Vec<Line> = lines.into_iter().skip(app.diag_scroll).collect();

    // Build title with scroll indicator
    let title = if total_lines > 1 {
        format!(
            " Diagnostics ({}/{}) ",
            (app.diag_scroll + 1).min(total_lines),
            total_lines
        )
    } else {
        " Diagnostics ".to_string()
    };

    Paragraph::new(visible_lines)
        .block(
            Block::default()
                .title(title.bold())
                .borders(Borders::ALL)
                .border_style(Style::default().dim()),
        )
        .render(area, frame.buffer_mut());
}
