//! Help view widget.

use super::super::app::App;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

pub fn render(_app: &App, frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(" LSP Test TUI - Help".bold().cyan()),
        Line::from(""),
        Line::from(" Global Keys:".bold()),
        Line::from(vec!["   Ctrl+C".cyan(), " - Quit application".dim()]),
        Line::from(vec!["   ?/h   ".cyan(), " - Show this help".dim()]),
        Line::from(""),
        Line::from(" Menu Mode:".bold()),
        Line::from(vec!["   ↑/↓   ".cyan(), " - Navigate operations".dim()]),
        Line::from(vec!["   1-7   ".cyan(), " - Quick select operation".dim()]),
        Line::from(vec!["   Enter ".cyan(), " - Select operation".dim()]),
        Line::from(vec!["   d     ".cyan(), " - View diagnostics".dim()]),
        Line::from(vec!["   q     ".cyan(), " - Quit".dim()]),
        Line::from(""),
        Line::from(" Input Mode (File/Symbol):".bold()),
        Line::from(vec!["   Enter ".cyan(), " - Confirm input".dim()]),
        Line::from(vec![
            "   Esc   ".cyan(),
            " - Cancel and return to menu".dim(),
        ]),
        Line::from(vec!["   ←/→   ".cyan(), " - Move cursor".dim()]),
        Line::from(vec!["   Home  ".cyan(), " - Jump to start".dim()]),
        Line::from(vec!["   End   ".cyan(), " - Jump to end".dim()]),
        Line::from(""),
        Line::from(" Results/Diagnostics Mode:".bold()),
        Line::from(vec!["   ↑/↓   ".cyan(), " - Scroll results".dim()]),
        Line::from(vec!["   Home  ".cyan(), " - Jump to top".dim()]),
        Line::from(vec!["   Esc/q ".cyan(), " - Return to menu".dim()]),
        Line::from(""),
        Line::from(" LSP Operations:".bold()),
        Line::from("   1. Go to Definition    - Find where a symbol is defined".dim()),
        Line::from("   2. Type Definition     - Find the type's definition".dim()),
        Line::from("   3. Go to Declaration   - Find where symbol is declared".dim()),
        Line::from("   4. Find References     - Find all usages of a symbol".dim()),
        Line::from("   5. Find Implementations- Find trait/interface impls".dim()),
        Line::from("   6. Hover Info          - Get documentation/type info".dim()),
        Line::from("   7. Workspace Symbol    - Search symbols across project".dim()),
        Line::from("   8. Document Symbols    - List all symbols in a file".dim()),
        Line::from("   9. Call Hierarchy      - Show incoming/outgoing calls".dim()),
        Line::from("  10. Health Check        - Check LSP server status".dim()),
        Line::from(""),
        Line::from(" Press Esc, q, or ? to close this help.".dim()),
    ];

    Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Help ".bold())
                .borders(Borders::ALL)
                .border_style(Style::default().dim()),
        )
        .render(area, frame.buffer_mut());
}
