//! Configure Servers widget for managing LSP server configurations.
//!
//! Displays all servers (installed + configured) and allows:
//! - [Enter/a] Add to config (if binary installed but not configured)
//! - [d] Disable/Enable (if configured)
//! - [x] Remove from config (if configured)

use super::super::app::App;
use codex_lsp::ServerStatus;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

/// Help text for ConfigServers mode - shared between UI and help bar
pub const HELP_TEXT: &str =
    "[↑↓] Navigate  [Enter/a] Add  [d] Disable/Enable  [x] Remove  [r] Refresh  [Esc/q] Back";

// Column widths (fixed)
const COL_SERVER: usize = 20;
const COL_EXT: usize = 14;
const COL_BINARY: usize = 10;
const COL_CONFIG: usize = 8;
const COL_DISABLED: usize = 9;
const COL_STATUS: usize = 8;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let servers = &app.cached_config_servers;

    if servers.is_empty() {
        let msg = Paragraph::new("No LSP servers found")
            .block(
                Block::default()
                    .title(" Configure Servers ")
                    .borders(Borders::ALL),
            )
            .alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    let mut lines = Vec::new();

    // Help hint at top
    lines.push(Line::from(vec![" ".into(), HELP_TEXT.dim()]));
    lines.push(Line::from(""));

    // Header - fixed width columns
    lines.push(Line::from(vec![
        Span::raw("   "),
        Span::styled(
            format!("{:<COL_SERVER$}", "Server"),
            Style::default().bold(),
        ),
        Span::styled(
            format!("{:<COL_EXT$}", "Extensions"),
            Style::default().bold(),
        ),
        Span::styled(
            format!("{:<COL_BINARY$}", "Binary"),
            Style::default().bold(),
        ),
        Span::styled(
            format!("{:<COL_CONFIG$}", "Config"),
            Style::default().bold(),
        ),
        Span::styled(
            format!("{:<COL_DISABLED$}", "Disabled"),
            Style::default().bold(),
        ),
        Span::styled(
            format!("{:<COL_STATUS$}", "Status"),
            Style::default().bold(),
        ),
    ]));

    // Separator
    lines.push(Line::from(
        format!(
            "   {}",
            "─".repeat(COL_SERVER + COL_EXT + COL_BINARY + COL_CONFIG + COL_DISABLED + COL_STATUS)
        )
        .dim(),
    ));

    // Server rows with selection highlighting
    for (idx, server) in servers.iter().enumerate().skip(app.config_servers_scroll) {
        let is_selected = idx == app.selected_config_server;

        // Binary column
        let (binary_text, binary_style): (&str, Style) = if server.binary_installed {
            ("Installed", Style::default().green())
        } else {
            ("Missing", Style::default().red())
        };

        // Config column
        let config_text = server
            .config_level
            .as_ref()
            .map(|l| l.to_string())
            .unwrap_or_else(|| "-".to_string());
        let config_style = if server.config_level.is_some() {
            Style::default().cyan()
        } else {
            Style::default().dim()
        };

        // Disabled column - show Yes/No/- based on config
        let (disabled_text, disabled_style): (&str, Style) = match server.status {
            ServerStatus::Disabled => ("Yes", Style::default().yellow()),
            _ if server.config_level.is_some() => ("No", Style::default().green()),
            _ => ("-", Style::default().dim()),
        };

        // Status column
        let (status_text, status_style): (&str, Style) = match server.status {
            ServerStatus::Running => ("Running", Style::default().green()),
            ServerStatus::Idle => ("Idle", Style::default().dim()),
            ServerStatus::NotInstalled => ("-", Style::default().dim()),
            ServerStatus::Disabled => ("Idle", Style::default().dim()),
        };

        // Truncate extensions
        let extensions = server.extensions.join(" ");
        let ext_display = if extensions.len() > COL_EXT - 2 {
            format!("{}...", &extensions[..COL_EXT - 5])
        } else {
            extensions
        };

        // Truncate server id
        let id_display = if server.id.len() > COL_SERVER - 2 {
            format!("{}...", &server.id[..COL_SERVER - 5])
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
                Span::styled(format!("{:<COL_SERVER$}", id_display), id_style),
                Span::styled(format!("{:<COL_EXT$}", ext_display), row_style),
                Span::styled(format!("{:<COL_BINARY$}", binary_text), binary_style),
                Span::styled(format!("{:<COL_CONFIG$}", config_text), config_style),
                Span::styled(format!("{:<COL_DISABLED$}", disabled_text), disabled_style),
                Span::styled(format!("{:<COL_STATUS$}", status_text), status_style),
            ])
            .style(row_style),
        );
    }

    // Add restart reminder if config changed
    if app.config_changed {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            " ".into(),
            "Config changed. Restart TUI to apply changes.".yellow(),
        ]));
    }

    // Build title_bottom based on selected server state
    let title_bottom = if let Some(server) = servers.get(app.selected_config_server) {
        if server.binary_installed && server.config_level.is_none() {
            " [Enter/a] Add to config "
        } else if server.config_level.is_some() {
            if matches!(server.status, ServerStatus::Disabled) {
                " [d] Enable  [x] Remove "
            } else {
                " [d] Disable  [x] Remove "
            }
        } else {
            " Binary not installed "
        }
    } else {
        ""
    };

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(" Configure LSP Servers ")
            .title_bottom(title_bottom)
            .borders(Borders::ALL)
            .border_style(Style::default().dim()),
    );

    frame.render_widget(paragraph, area);
}
