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
/// - When `selection` is `Some`, each item line includes a checkbox marker:
///   "[x]" for selected items and "[ ]" for unselected. Missing indices
///   default to selected.
/// - When `selection` is `None`, the marker is omitted and a simple bullet is
///   rendered ("- Title — path:start-end").
pub fn format_review_findings_block(
    findings: &[ReviewFinding],
    selection: Option<&[bool]>,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(String::new());

    // Header
    if findings.len() > 1 {
        lines.push("Full review comments:".to_string());
    } else {
        lines.push("Review comment:".to_string());
    }

    for (idx, item) in findings.iter().enumerate() {
        lines.push(String::new());

        let title = &item.title;
        let location = format_location(item);

        if let Some(flags) = selection {
            // Default to selected if index is out of bounds.
            let checked = flags.get(idx).copied().unwrap_or(true);
            let marker = if checked { "[x]" } else { "[ ]" };
            lines.push(format!("- {marker} {title} — {location}"));
        } else {
            lines.push(format!("- {title} — {location}"));
        }

        for body_line in item.body.lines() {
            lines.push(format!("  {body_line}"));
        }
    }

    lines.join("\n")
}
