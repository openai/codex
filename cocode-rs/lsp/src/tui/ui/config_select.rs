//! Config level selection UI for installation.

use super::super::app::App;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let server_name = app.pending_install_server.as_deref().unwrap_or("Unknown");

    let user_dir = cocode_lsp::config::find_codex_home()
        .map(|h| h.display().to_string())
        .unwrap_or_else(|| "~/.codex".to_string());

    let project_dir = app.workspace.join(".codex").display().to_string();

    let mut lines = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(
        format!("  Installing: {}", server_name).cyan().bold(),
    ));
    lines.push(Line::from(""));
    lines.push(Line::from(
        "  Where should the LSP server be configured?".dim(),
    ));
    lines.push(Line::from(""));

    // Option 1: User level
    let user_prefix = if app.config_level_selection == 0 {
        " > "
    } else {
        "   "
    };
    let user_style = if app.config_level_selection == 0 {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };
    lines.push(
        Line::from(vec![
            Span::styled(user_prefix, user_style),
            Span::styled("1. User level", user_style.cyan().bold()),
        ])
        .style(user_style),
    );
    lines.push(
        Line::from(vec![
            Span::styled("      ", user_style),
            Span::styled(format!("({})", user_dir), Style::default().dim()),
        ])
        .style(user_style),
    );
    lines.push(
        Line::from(vec![
            Span::styled("      ", user_style),
            Span::styled("Global for all projects", Style::default().dim()),
        ])
        .style(user_style),
    );

    lines.push(Line::from(""));

    // Option 2: Project level
    let project_prefix = if app.config_level_selection == 1 {
        " > "
    } else {
        "   "
    };
    let project_style = if app.config_level_selection == 1 {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };
    lines.push(
        Line::from(vec![
            Span::styled(project_prefix, project_style),
            Span::styled("2. Project level", project_style.cyan().bold()),
        ])
        .style(project_style),
    );
    lines.push(
        Line::from(vec![
            Span::styled("      ", project_style),
            Span::styled(format!("({})", project_dir), Style::default().dim()),
        ])
        .style(project_style),
    );
    lines.push(
        Line::from(vec![
            Span::styled("      ", project_style),
            Span::styled("Only for this project", Style::default().dim()),
        ])
        .style(project_style),
    );

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(" Select Config Location ")
            .title_bottom(" [Enter] Select  [Esc] Cancel ")
            .borders(Borders::ALL)
            .border_style(Style::default().dim()),
    );

    frame.render_widget(paragraph, area);
}
