//! Server list widget showing all configured LSP servers and their status.

use super::super::app::App;
use codex_lsp::ServerStatus;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let servers = &app.cached_servers;

    if servers.is_empty() {
        let msg = Paragraph::new("No LSP servers configured")
            .block(
                Block::default()
                    .title(" LSP Servers ")
                    .borders(Borders::ALL),
            )
            .alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    let mut lines = Vec::new();

    // Header
    lines.push(Line::from(vec![
        "   Server".bold(),
        "                ".into(),
        "Extensions".bold(),
        "        ".into(),
        "Status".bold(),
        "          ".into(),
        "Install".bold(),
    ]));

    // Separator
    lines.push(Line::from(
        "   ─────────────────────────────────────────────────────────────────────────".dim(),
    ));

    // Server rows with selection highlighting
    for (idx, server) in servers.iter().enumerate().skip(app.servers_scroll) {
        let is_selected = idx == app.selected_server;

        let (status_text, status_style): (&str, Style) = match server.status {
            ServerStatus::Running => ("Running", Style::default().green()),
            ServerStatus::Idle => ("Idle", Style::default().dim()),
            ServerStatus::NotInstalled => ("Not Installed", Style::default().red()),
            ServerStatus::Disabled => ("Disabled", Style::default().dim().italic()),
        };

        let extensions = server.extensions.join(" ");
        let ext_display = if extensions.len() > 14 {
            format!("{}...", &extensions[..11])
        } else {
            extensions
        };

        let install_hint = if matches!(server.status, ServerStatus::NotInstalled) {
            if server.install_hint.len() > 25 {
                format!("{}...", &server.install_hint[..22])
            } else {
                server.install_hint.clone()
            }
        } else {
            "-".to_string()
        };

        let id_display = if server.id.len() > 20 {
            format!("{}...", &server.id[..17])
        } else {
            server.id.clone()
        };

        // Selection indicator and row styling
        let prefix = if is_selected { " > " } else { "   " };
        let row_style = if is_selected {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };

        let id_style = if is_selected {
            Style::default().cyan().bold()
        } else {
            Style::default().cyan()
        };

        lines.push(
            Line::from(vec![
                Span::styled(prefix, row_style),
                Span::styled(format!("{:<20}", id_display), id_style),
                Span::styled(" ", row_style),
                Span::styled(format!("{:<14}", ext_display), row_style),
                Span::styled(" ", row_style),
                Span::styled(format!("{:<14}", status_text), status_style),
                Span::styled(" ", row_style),
                Span::styled(install_hint, Style::default().dim()),
            ])
            .style(row_style),
        );
    }

    // Build title_bottom based on selected server status
    let title_bottom = if app
        .cached_servers
        .get(app.selected_server)
        .map(|s| matches!(s.status, ServerStatus::NotInstalled))
        .unwrap_or(false)
    {
        " [Enter] Install  [r] Refresh  [Esc/q] Back "
    } else {
        " [r] Refresh  [Esc/q] Back "
    };

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(" LSP Servers ")
            .title_bottom(title_bottom)
            .borders(Borders::ALL)
            .border_style(Style::default().dim()),
    );

    frame.render_widget(paragraph, area);
}
