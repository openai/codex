//! UI module for LSP Test TUI.

mod diagnostics;
mod help;
mod input_box;
mod menu;
mod result_view;
mod status_bar;
pub mod utils;

use super::app::App;
use super::app::Mode;
use ratatui::prelude::*;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

/// Main render function for the TUI.
pub fn render(app: &App, frame: &mut Frame) {
    let area = frame.area();

    // Layout: status bar (3), mode bar (1), main content, help bar (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Status bar
            Constraint::Length(1), // Mode bar
            Constraint::Min(10),   // Main content
            Constraint::Length(1), // Help bar
        ])
        .split(area);

    // Render status bar
    status_bar::render(app, frame, chunks[0]);

    // Render mode bar
    render_mode_bar(app, frame, chunks[1]);

    // Render main content based on mode
    match app.mode {
        Mode::Menu => menu::render(app, frame, chunks[2]),
        Mode::FileInput => input_box::render_file_input(app, frame, chunks[2]),
        Mode::SymbolInput => input_box::render_symbol_input(app, frame, chunks[2]),
        Mode::Results => result_view::render(app, frame, chunks[2]),
        Mode::Diagnostics => diagnostics::render(app, frame, chunks[2]),
        Mode::Help => help::render(app, frame, chunks[2]),
    }

    // Render help bar
    render_help_bar(app, frame, chunks[3]);
}

fn render_mode_bar(app: &App, frame: &mut Frame, area: Rect) {
    use ratatui::style::Stylize;

    let mode_text = match app.mode {
        Mode::Menu => "[0-9] Select  [d] Diagnostics  [?] Help  [q] Quit".to_string(),
        Mode::FileInput => format!("File: {}", app.file_input.text),
        Mode::SymbolInput => format!("Symbol: {}", app.symbol_input.text),
        Mode::Results => "Results".to_string(),
        Mode::Diagnostics => "Diagnostics".to_string(),
        Mode::Help => "Press Esc or ? to close".to_string(),
    };

    let mode_line = Line::from(vec![
        " Mode: ".dim(),
        app.mode.display_name().cyan().bold(),
        " | ".dim(),
        mode_text.into(),
    ]);

    Paragraph::new(mode_line).render(area, frame.buffer_mut());
}

fn render_help_bar(app: &App, frame: &mut Frame, area: Rect) {
    use ratatui::style::Stylize;

    let help_text = match app.mode {
        Mode::Menu => "[Enter] Select  [↑↓] Navigate  [d] Diagnostics  [?] Help  [q] Quit",
        Mode::FileInput | Mode::SymbolInput => "[Enter] Confirm  [Esc] Cancel  [?] Help",
        Mode::Results => "[↑↓] Scroll  [Esc/q] Back",
        Mode::Diagnostics => "[↑↓] Scroll  [Esc/q] Back",
        Mode::Help => "[Esc/?/q] Close Help",
    };

    let loading_indicator = if app.loading { " [Loading...]" } else { "" };

    let help_line = Line::from(vec![" ".into(), help_text.dim(), loading_indicator.cyan()]);

    Paragraph::new(help_line)
        .block(Block::default().borders(Borders::TOP))
        .render(area, frame.buffer_mut());
}
