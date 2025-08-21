use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line as RtLine;
use ratatui::text::Span as RtSpan;
use std::collections::HashMap;
use std::path::PathBuf;

use codex_core::protocol::FileChange;

struct FileSummary {
    display_path: String,
    added: usize,
    removed: usize,
}

pub(crate) fn create_diff_summary(
    title: &str,
    changes: HashMap<PathBuf, FileChange>,
) -> Vec<RtLine<'static>> {
    let mut files: Vec<FileSummary> = Vec::new();

    // Count additions/deletions from a unified diff body
    let count_from_unified = |diff: &str| -> (usize, usize) {
        if let Ok(patch) = diffy::Patch::from_str(diff) {
            let mut adds = 0usize;
            let mut dels = 0usize;
            for hunk in patch.hunks() {
                for line in hunk.lines() {
                    match line {
                        diffy::Line::Insert(_) => adds += 1,
                        diffy::Line::Delete(_) => dels += 1,
                        _ => {}
                    }
                }
            }
            (adds, dels)
        } else {
            let mut adds = 0usize;
            let mut dels = 0usize;
            for l in diff.lines() {
                if l.starts_with("+++") || l.starts_with("---") || l.starts_with("@@") {
                    continue;
                }
                match l.as_bytes().first() {
                    Some(b'+') => adds += 1,
                    Some(b'-') => dels += 1,
                    _ => {}
                }
            }
            (adds, dels)
        }
    };

    for (path, change) in &changes {
        use codex_core::protocol::FileChange::*;
        match change {
            Add { content } => {
                let added = content.lines().count();
                files.push(FileSummary {
                    display_path: path.display().to_string(),
                    added,
                    removed: 0,
                });
            }
            Delete => {
                let removed = std::fs::read_to_string(path)
                    .ok()
                    .map(|s| s.lines().count())
                    .unwrap_or(0);
                files.push(FileSummary {
                    display_path: path.display().to_string(),
                    added: 0,
                    removed,
                });
            }
            Update {
                unified_diff,
                move_path,
            } => {
                let (added, removed) = count_from_unified(unified_diff);
                let display_path = if let Some(new_path) = move_path {
                    format!("{} → {}", path.display(), new_path.display())
                } else {
                    path.display().to_string()
                };
                files.push(FileSummary {
                    display_path,
                    added,
                    removed,
                });
            }
        }
    }

    let file_count = files.len();
    let total_added: usize = files.iter().map(|f| f.added).sum();
    let total_removed: usize = files.iter().map(|f| f.removed).sum();
    let noun = if file_count == 1 { "file" } else { "files" };

    let mut out: Vec<RtLine<'static>> = Vec::new();

    // Header
    let mut header_spans: Vec<RtSpan<'static>> = Vec::new();
    header_spans.push(RtSpan::styled(
        title.to_owned(),
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    ));
    header_spans.push(RtSpan::raw(" to "));
    header_spans.push(RtSpan::raw(format!("{file_count} {noun} ")));
    header_spans.push(RtSpan::raw("("));
    header_spans.push(RtSpan::styled(
        format!("+{total_added}"),
        Style::default().fg(Color::Green),
    ));
    header_spans.push(RtSpan::raw(" "));
    header_spans.push(RtSpan::styled(
        format!("-{total_removed}"),
        Style::default().fg(Color::Red),
    ));
    header_spans.push(RtSpan::raw(")"));
    out.push(RtLine::from(header_spans));

    // Dimmed per-file lines with prefix
    for (idx, f) in files.iter().enumerate() {
        let mut spans: Vec<RtSpan<'static>> = Vec::new();
        spans.push(RtSpan::raw(f.display_path.clone()));
        spans.push(RtSpan::raw(" ("));
        spans.push(RtSpan::styled(
            format!("+{}", f.added),
            Style::default().fg(Color::Green),
        ));
        spans.push(RtSpan::raw(" "));
        spans.push(RtSpan::styled(
            format!("-{}", f.removed),
            Style::default().fg(Color::Red),
        ));
        spans.push(RtSpan::raw(")"));

        let mut line = RtLine::from(spans);
        let prefix = if idx == 0 { "  ⎿ " } else { "    " };
        line.spans.insert(0, prefix.into());
        line.spans.iter_mut().for_each(|span| {
            span.style = span.style.add_modifier(Modifier::DIM);
        });
        out.push(line);
    }

    out
}
