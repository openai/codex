use crate::protocol::ReviewFinding;

// Note: We keep this module UI-agnostic. It returns plain strings that
// higher layers (e.g., TUI) may style as needed.

fn format_location(item: &ReviewFinding) -> String {
    let path = item.code_location.absolute_file_path.display();
    let start = item.code_location.line_range.start;
    let end = item.code_location.line_range.end;
    format!("{path}:{start}-{end}")
}

/// Format a full review findings block as plain text lines.
///
/// - When `include_checkboxes` is true, each item line includes a checkbox
///   marker: "[x]" for selected items and "[ ]" for unselected. If
///   `selection` is `None`, all items are treated as selected.
/// - When `include_checkboxes` is false, the marker is omitted and a simple
///   bullet is rendered ("- Title — path:start-end").
pub fn format_review_findings_block(
    findings: &[ReviewFinding],
    include_checkboxes: bool,
    selection: Option<&[bool]>,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    // Header
    let header = if findings.len() > 1 {
        "Full review comments:"
    } else {
        "Review comment:"
    };
    lines.push(header.to_string());

    for (idx, item) in findings.iter().enumerate() {
        lines.push(String::new());

        let title = &item.title;
        let location = format_location(item);

        if include_checkboxes {
            // Default to selected if selection flags are not provided.
            let checked = selection.and_then(|v| v.get(idx).copied()).unwrap_or(true);
            let marker = if checked { "[x]" } else { "[ ]" };
            lines.push(format!("- {marker} {title} — {location}"));
        } else {
            lines.push(format!("- {title} — {location}"));
        }

        for body_line in item.body.lines() {
            lines.push(format!("  {body_line}"));
        }
    }

    lines
}
