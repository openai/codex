use std::path::Path;
use std::time::Duration;

use codex_common::elapsed::format_duration;
use crossterm::terminal;
use owo_colors::OwoColorize;
use textwrap::Options as WrapOptions;
use textwrap::wrap;

pub(crate) fn print_run_summary_box(
    color_enabled: bool,
    run_id: &str,
    run_path: &Path,
    deliverable_path: &Path,
    summary: Option<&str>,
    objective: Option<&str>,
    duration: Duration,
) {
    let mut items = Vec::new();
    items.push(("Run ID".to_string(), run_id.to_string()));
    items.push(("Run Directory".to_string(), run_path.display().to_string()));
    if let Some(objective) = objective
        && !objective.trim().is_empty()
    {
        items.push(("Objective".to_string(), objective.trim().to_string()));
    }
    items.push((
        "Deliverable".to_string(),
        deliverable_path.display().to_string(),
    ));
    items.push(("Total Time".to_string(), format_duration(duration)));
    if let Some(summary) = summary {
        let trimmed = summary.trim();
        if !trimmed.is_empty() {
            items.push(("Summary".to_string(), trimmed.to_string()));
        }
    }

    let label_width = items
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(0)
        .max(12);

    const DEFAULT_MAX_WIDTH: usize = 84;
    const MIN_VALUE_WIDTH: usize = 20;
    let label_padding = label_width + 7;
    let min_total_width = label_padding + MIN_VALUE_WIDTH;
    let available_width = terminal::size()
        .ok()
        .map(|(cols, _)| usize::from(cols).saturating_sub(2))
        .unwrap_or(DEFAULT_MAX_WIDTH);
    let max_width = available_width.min(DEFAULT_MAX_WIDTH);
    let lower_bound = min_total_width.min(available_width);
    let mut total_width = max_width.max(lower_bound).max(label_padding + 1);
    let mut value_width = total_width.saturating_sub(label_padding);
    if value_width < MIN_VALUE_WIDTH {
        value_width = MIN_VALUE_WIDTH;
        total_width = label_padding + value_width;
    }

    let inner_width = total_width.saturating_sub(4);
    let top_border = format!("+{}+", "=".repeat(total_width.saturating_sub(2)));
    let separator = format!("+{}+", "-".repeat(total_width.saturating_sub(2)));
    let title_line = format!(
        "| {:^inner_width$} |",
        "Run Summary",
        inner_width = inner_width
    );

    println!();
    println!("{top_border}");
    if color_enabled {
        println!("{}", title_line.bold());
    } else {
        println!("{title_line}");
    }
    println!("{separator}");

    for (index, (label, value)) in items.iter().enumerate() {
        let mut rows = Vec::new();
        for (idx, paragraph) in value.split('\n').enumerate() {
            let trimmed = paragraph.trim();
            if trimmed.is_empty() {
                if idx > 0 {
                    rows.push(String::new());
                }
                continue;
            }
            let wrapped = wrap(trimmed, WrapOptions::new(value_width).break_words(false));
            if wrapped.is_empty() {
                rows.push(String::new());
            } else {
                rows.extend(wrapped.into_iter().map(std::borrow::Cow::into_owned));
            }
        }
        if rows.is_empty() {
            rows.push(String::new());
        }

        for (line_idx, line) in rows.iter().enumerate() {
            let label_cell = if line_idx == 0 { label.as_str() } else { "" };
            let row_line = format!("| {label_cell:<label_width$} | {line:<value_width$} |");
            if color_enabled {
                match label.as_str() {
                    "Deliverable" => println!("{}", row_line.green()),
                    "Summary" => println!("{}", row_line.bold()),
                    _ => println!("{row_line}"),
                }
            } else {
                println!("{row_line}");
            }
        }

        if index + 1 < items.len() {
            println!("{separator}");
        }
    }

    println!("{top_border}");
    println!();
}
