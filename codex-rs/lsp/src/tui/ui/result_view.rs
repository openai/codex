//! Result view widget for displaying LSP operation results.

use super::super::app::App;
use super::super::app::CallHierarchyResult;
use super::super::app::LspResult;
use super::utils::relative_path;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let Some(result) = &app.result else {
        let lines = vec![Line::from(" No results yet".dim())];
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Results ".bold())
                    .borders(Borders::ALL)
                    .border_style(Style::default().dim()),
            )
            .render(area, frame.buffer_mut());
        return;
    };

    let lines: Vec<Line> = match result {
        LspResult::Locations(locations) => {
            if locations.is_empty() {
                vec![Line::from(" No locations found".dim())]
            } else {
                let mut lines = vec![Line::from(format!(
                    " Found {} location(s):",
                    locations.len()
                ))];
                lines.push(Line::from(""));

                for (i, loc) in locations.iter().enumerate() {
                    let path = loc.uri.path();
                    let line_num = loc.range.start.line + 1;
                    let col = loc.range.start.character + 1;

                    // Try to make path relative
                    let display_path = relative_path(path, &app.workspace);

                    let line = Line::from(vec![
                        format!(" {}. ", i + 1).dim(),
                        display_path.to_string().cyan(),
                        ":".dim(),
                        line_num.to_string().into(),
                        ":".dim(),
                        col.to_string().dim(),
                    ]);
                    lines.push(line);
                }
                lines
            }
        }
        LspResult::HoverInfo(info) => match info {
            Some(content) => {
                let mut lines = vec![Line::from(" Hover Info:".bold())];
                lines.push(Line::from(""));
                for line in content.lines() {
                    lines.push(Line::from(format!(" {line}")));
                }
                lines
            }
            None => vec![Line::from(" No hover information available".dim())],
        },
        LspResult::Symbols(symbols) => {
            if symbols.is_empty() {
                vec![Line::from(" No symbols found".dim())]
            } else {
                let mut lines = vec![Line::from(format!(" Found {} symbol(s):", symbols.len()))];
                lines.push(Line::from(""));

                for (i, sym) in symbols.iter().enumerate() {
                    let kind_str = format!("{:?}", sym.kind);
                    let line_num = sym.range_start_line + 1;

                    let line = Line::from(vec![
                        format!(" {}. ", i + 1).dim(),
                        sym.name.clone().cyan().bold(),
                        " (".dim(),
                        kind_str.into(),
                        ") L".dim(),
                        line_num.to_string().into(),
                    ]);
                    lines.push(line);
                }
                lines
            }
        }
        LspResult::WorkspaceSymbols(symbols) => {
            if symbols.is_empty() {
                vec![Line::from(" No symbols found".dim())]
            } else {
                let mut lines = vec![Line::from(format!(" Found {} symbol(s):", symbols.len()))];
                lines.push(Line::from(""));

                for (i, sym) in symbols.iter().enumerate() {
                    let kind_str = format!("{:?}", sym.kind);
                    let line_num = sym.location.range.start.line + 1;

                    let uri_info = relative_path(sym.location.uri.path(), &app.workspace);

                    let line = Line::from(vec![
                        format!(" {}. ", i + 1).dim(),
                        sym.name.clone().cyan().bold(),
                        " (".dim(),
                        kind_str.into(),
                        ") L".dim(),
                        line_num.to_string().into(),
                        format!(" @ {uri_info}").dim(),
                    ]);
                    lines.push(line);
                }
                lines
            }
        }
        LspResult::CallHierarchy(CallHierarchyResult {
            items,
            incoming,
            outgoing,
        }) => {
            let mut lines = vec![];

            // Show the item(s)
            lines.push(Line::from(" Call Hierarchy:".bold()));
            lines.push(Line::from(""));
            for item in items {
                lines.push(Line::from(vec![
                    "   Symbol: ".dim(),
                    item.clone().cyan().bold(),
                ]));
            }

            // Show incoming calls
            lines.push(Line::from(""));
            lines.push(Line::from(" Incoming calls (who calls this):".bold()));
            if incoming.is_empty() {
                lines.push(Line::from("   (none)".dim()));
            } else {
                for (i, call) in incoming.iter().enumerate() {
                    lines.push(Line::from(vec![
                        format!("   {}. ", i + 1).dim(),
                        call.clone().into(),
                    ]));
                }
            }

            // Show outgoing calls
            lines.push(Line::from(""));
            lines.push(Line::from(" Outgoing calls (what this calls):".bold()));
            if outgoing.is_empty() {
                lines.push(Line::from("   (none)".dim()));
            } else {
                for (i, call) in outgoing.iter().enumerate() {
                    lines.push(Line::from(vec![
                        format!("   {}. ", i + 1).dim(),
                        call.clone().into(),
                    ]));
                }
            }

            lines
        }
        LspResult::HealthOk(msg) => {
            vec![
                Line::from(""),
                Line::from(vec![" ".into(), msg.clone().green().bold()]),
            ]
        }
        LspResult::Error(err) => {
            vec![
                Line::from(""),
                Line::from(vec![" Error: ".red().bold(), err.clone().red()]),
            ]
        }
    };

    // Apply scroll offset
    let total_lines = lines.len();
    let visible_lines: Vec<Line> = lines.into_iter().skip(app.result_scroll).collect();

    // Build title with scroll indicator
    let title = if total_lines > 1 {
        format!(
            " Results ({}/{}) ",
            (app.result_scroll + 1).min(total_lines),
            total_lines
        )
    } else {
        " Results ".to_string()
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
