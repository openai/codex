//! Menu widget for operation selection.

use super::super::app::App;
use super::super::app::Operation;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let operations = Operation::all();

    let mut lines = vec![Line::from(" Select an LSP operation:".dim())];
    lines.push(Line::from(""));

    for (i, op) in operations.iter().enumerate() {
        let is_selected = i == app.menu_index;
        let prefix = if is_selected { ">" } else { " " };
        let num = i + 1;

        let line = if is_selected {
            Line::from(vec![
                format!(" {prefix} {num}. ").cyan().bold(),
                op.display_name().cyan().bold(),
            ])
        } else {
            Line::from(vec![
                format!(" {prefix} {num}. ").dim(),
                op.display_name().into(),
            ])
        };
        lines.push(line);
    }

    // Add operation details for selected item
    lines.push(Line::from(""));
    let selected_op = operations[app.menu_index];
    let details = get_operation_details(selected_op);
    lines.push(Line::from(vec![" Details: ".dim(), details.into()]));

    Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Operations ".bold())
                .borders(Borders::ALL)
                .border_style(Style::default().dim()),
        )
        .render(area, frame.buffer_mut());
}

fn get_operation_details(op: Operation) -> &'static str {
    match op {
        Operation::Definition => "Go to the definition of a symbol",
        Operation::TypeDefinition => "Find the type's definition (struct, trait, etc.)",
        Operation::Declaration => "Go to where a symbol is declared",
        Operation::References => "Find all references to a symbol",
        Operation::Implementation => "Find implementations of a trait/interface",
        Operation::Hover => "Get hover information for a symbol",
        Operation::WorkspaceSymbol => "Search for symbols across the workspace",
        Operation::DocumentSymbols => "List all symbols in a document",
        Operation::CallHierarchy => "Show incoming and outgoing calls for a function",
        Operation::HealthCheck => "Check if the language server is healthy",
    }
}
