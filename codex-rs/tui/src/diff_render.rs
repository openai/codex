use crossterm::terminal;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line as RtLine;
use ratatui::text::Span as RtSpan;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::common::DEFAULT_WRAP_COLS;
use codex_core::protocol::FileChange;

use crate::history_cell::PatchEventType;

const SPACES_AFTER_LINE_NUMBER: usize = 6;

// Internal representation for diff line rendering
enum DiffLineType {
    Insert,
    Delete,
    Context,
}

pub(crate) fn create_diff_summary(
    title: &str,
    changes: &HashMap<PathBuf, FileChange>,
    event_type: PatchEventType,
) -> Vec<RtLine<'static>> {
    struct FileSummary {
        display_path: String,
        added: usize,
        removed: usize,
    }

    let count_from_unified = |diff: &str| -> (usize, usize) {
        if let Ok(patch) = diffy::Patch::from_str(diff) {
            patch
                .hunks()
                .iter()
                .flat_map(|h| h.lines())
                .fold((0, 0), |(a, d), l| match l {
                    diffy::Line::Insert(_) => (a + 1, d),
                    diffy::Line::Delete(_) => (a, d + 1),
                    _ => (a, d),
                })
        } else {
            // Fallback: manual scan to preserve counts even for unparsable diffs
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

    let mut files: Vec<FileSummary> = Vec::new();
    for (path, change) in changes.iter() {
        match change {
            FileChange::Add { content } => files.push(FileSummary {
                display_path: path.display().to_string(),
                added: content.lines().count(),
                removed: 0,
            }),
            FileChange::Delete => files.push(FileSummary {
                display_path: path.display().to_string(),
                added: 0,
                removed: std::fs::read_to_string(path)
                    .ok()
                    .map(|s| s.lines().count())
                    .unwrap_or(0),
            }),
            FileChange::Update {
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
        // Show per-file +/- counts only when there are multiple files
        if file_count > 1 {
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
        }

        let mut line = RtLine::from(spans);
        let prefix = if idx == 0 { "  └ " } else { "    " };
        line.spans.insert(0, prefix.into());
        line.spans
            .iter_mut()
            .for_each(|span| span.style = span.style.add_modifier(Modifier::DIM));
        out.push(line);
    }

    let show_details = matches!(
        event_type,
        PatchEventType::ApplyBegin {
            auto_approved: true
        } | PatchEventType::ApprovalRequest
    );

    if show_details {
        out.extend(render_patch_details(changes));
    }

    out
}

fn render_patch_details(changes: &HashMap<PathBuf, FileChange>) -> Vec<RtLine<'static>> {
    let mut out: Vec<RtLine<'static>> = Vec::new();
    let term_cols: usize = terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(DEFAULT_WRAP_COLS.into());

    for (index, (path, change)) in changes.iter().enumerate() {
        let is_first_file = index == 0;
        // Add separator only between files (not at the very start)
        if !is_first_file {
            out.push(RtLine::from(vec![
                RtSpan::raw("    "),
                RtSpan::styled("...", style_dim()),
            ]));
        }
        match change {
            FileChange::Add { content } => {
                for (i, raw) in content.lines().enumerate() {
                    let ln = i + 1;
                    out.extend(push_wrapped_diff_line(
                        ln,
                        DiffLineType::Insert,
                        raw,
                        term_cols,
                    ));
                }
            }
            FileChange::Delete => {
                let original = std::fs::read_to_string(path).unwrap_or_default();
                for (i, raw) in original.lines().enumerate() {
                    let ln = i + 1;
                    out.extend(push_wrapped_diff_line(
                        ln,
                        DiffLineType::Delete,
                        raw,
                        term_cols,
                    ));
                }
            }
            FileChange::Update {
                unified_diff,
                move_path: _,
            } => {
                if let Ok(patch) = diffy::Patch::from_str(unified_diff) {
                    let mut is_first_hunk = true;
                    for h in patch.hunks() {
                        // Render a simple separator between non-contiguous hunks
                        // instead of diff-style @@ headers.
                        if !is_first_hunk {
                            out.push(RtLine::from(vec![
                                RtSpan::raw("    "),
                                RtSpan::styled("⋮", style_dim()),
                            ]));
                        }
                        is_first_hunk = false;

                        let mut old_ln = h.old_range().start();
                        let mut new_ln = h.new_range().start();
                        for l in h.lines() {
                            match l {
                                diffy::Line::Insert(text) => {
                                    let s = text.trim_end_matches('\n');
                                    out.extend(push_wrapped_diff_line(
                                        new_ln,
                                        DiffLineType::Insert,
                                        s,
                                        term_cols,
                                    ));
                                    new_ln += 1;
                                }
                                diffy::Line::Delete(text) => {
                                    let s = text.trim_end_matches('\n');
                                    out.extend(push_wrapped_diff_line(
                                        old_ln,
                                        DiffLineType::Delete,
                                        s,
                                        term_cols,
                                    ));
                                    old_ln += 1;
                                }
                                diffy::Line::Context(text) => {
                                    let s = text.trim_end_matches('\n');
                                    out.extend(push_wrapped_diff_line(
                                        new_ln,
                                        DiffLineType::Context,
                                        s,
                                        term_cols,
                                    ));
                                    old_ln += 1;
                                    new_ln += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        out.push(RtLine::from(RtSpan::raw("")));
    }

    out
}

fn push_wrapped_diff_line(
    line_number: usize,
    kind: DiffLineType,
    text: &str,
    term_cols: usize,
) -> Vec<RtLine<'static>> {
    let indent = "    ";
    let ln_str = line_number.to_string();
    let mut remaining_text: &str = text;

    // Reserve a fixed number of spaces after the line number so that content starts
    // at a consistent column. Content includes a 1-character diff sign prefix
    // ("+"/"-" for inserts/deletes, or a space for context lines) so alignment
    // stays consistent across all diff lines.
    let gap_after_ln = SPACES_AFTER_LINE_NUMBER.saturating_sub(ln_str.len());
    let prefix_cols = indent.len() + ln_str.len() + gap_after_ln;

    let mut first = true;
    let (sign_opt, line_style) = match kind {
        DiffLineType::Insert => (Some('+'), Some(style_add())),
        DiffLineType::Delete => (Some('-'), Some(style_del())),
        DiffLineType::Context => (None, None),
    };
    let mut lines: Vec<RtLine<'static>> = Vec::new();

    loop {
        // Fit the content for the current terminal row:
        // compute how many columns are available after the prefix, then split
        // at a UTF-8 character boundary so this row's chunk fits exactly.
        let available_content_cols = term_cols
            .saturating_sub(if first { prefix_cols + 1 } else { prefix_cols })
            .max(1);
        let split_at_byte_index = remaining_text
            .char_indices()
            .nth(available_content_cols)
            .map(|(i, _)| i)
            .unwrap_or_else(|| remaining_text.len());
        let (chunk, rest) = remaining_text.split_at(split_at_byte_index);
        remaining_text = rest;

        if first {
            let mut spans: Vec<RtSpan<'static>> = Vec::new();
            spans.push(RtSpan::raw(indent));
            spans.push(RtSpan::styled(ln_str.clone(), style_dim()));
            spans.push(RtSpan::raw(" ".repeat(gap_after_ln)));
            // Always include a sign character at the start of the displayed chunk
            // ('+' for insert, '-' for delete, ' ' for context) so gutters align.
            let sign_char = sign_opt.unwrap_or(' ');
            let display_chunk = format!("{sign_char}{chunk}");
            let content_span = match line_style {
                Some(style) => RtSpan::styled(display_chunk, style),
                None => RtSpan::raw(display_chunk),
            };
            spans.push(content_span);
            let mut line = RtLine::from(spans);
            if let Some(style) = line_style {
                line.style = line.style.patch(style);
            }
            lines.push(line);
            first = false;
        } else {
            // Continuation lines keep a space for the sign column so content aligns
            let hang_prefix = format!(
                "{indent}{}{} ",
                " ".repeat(ln_str.len()),
                " ".repeat(gap_after_ln)
            );
            let content_span = match line_style {
                Some(style) => RtSpan::styled(chunk.to_string(), style),
                None => RtSpan::raw(chunk.to_string()),
            };
            let mut line = RtLine::from(vec![RtSpan::raw(hang_prefix), content_span]);
            if let Some(style) = line_style {
                line.style = line.style.patch(style);
            }
            lines.push(line);
        }
        if remaining_text.is_empty() {
            break;
        }
    }
    lines
}

fn style_dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

fn style_add() -> Style {
    Style::default().fg(Color::Green)
}

fn style_del() -> Style {
    Style::default().fg(Color::Red)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::text::Text;
    use ratatui::widgets::Paragraph;
    use ratatui::widgets::WidgetRef;
    use ratatui::widgets::Wrap;

    fn snapshot_lines(name: &str, lines: Vec<RtLine<'static>>, width: u16, height: u16) {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|f| {
                Paragraph::new(Text::from(lines))
                    .wrap(Wrap { trim: false })
                    .render_ref(f.area(), f.buffer_mut())
            })
            .expect("draw");
        assert_snapshot!(name, terminal.backend());
    }

    #[test]
    fn ui_snapshot_add_details() {
        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            PathBuf::from("README.md"),
            FileChange::Add {
                content: "first line\nsecond line\n".to_string(),
            },
        );

        let lines =
            create_diff_summary("proposed patch", &changes, PatchEventType::ApprovalRequest);

        snapshot_lines("add_details", lines, 80, 10);
    }

    #[test]
    fn ui_snapshot_update_details_with_rename() {
        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();

        let original = "line one\nline two\nline three\n";
        let modified = "line one\nline two changed\nline three\n";
        let patch = diffy::create_patch(original, modified).to_string();

        changes.insert(
            PathBuf::from("src/lib.rs"),
            FileChange::Update {
                unified_diff: patch,
                move_path: Some(PathBuf::from("src/lib_new.rs")),
            },
        );

        let lines =
            create_diff_summary("proposed patch", &changes, PatchEventType::ApprovalRequest);

        snapshot_lines("update_details_with_rename", lines, 80, 12);
    }

    #[test]
    fn ui_snapshot_wrap_behavior_insert() {
        // Narrow width to force wrapping within our diff line rendering
        let long_line = "this is a very long line that should wrap across multiple terminal columns and continue";

        // Call the wrapping function directly so we can precisely control the width
        let lines =
            push_wrapped_diff_line(1, DiffLineType::Insert, long_line, DEFAULT_WRAP_COLS.into());

        // Render into a small terminal to capture the visual layout
        snapshot_lines("wrap_behavior_insert", lines, DEFAULT_WRAP_COLS + 10, 8);
    }

    #[test]
    fn ui_snapshot_single_line_replacement_counts() {
        // Reproduce: one deleted line replaced by one inserted line, no extra context
        let original = "# Codex CLI (Rust Implementation)\n";
        let modified = "# Codex CLI (Rust Implementation) banana\n";
        let patch = diffy::create_patch(original, modified).to_string();

        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            PathBuf::from("README.md"),
            FileChange::Update {
                unified_diff: patch,
                move_path: None,
            },
        );

        let lines =
            create_diff_summary("proposed patch", &changes, PatchEventType::ApprovalRequest);

        snapshot_lines("single_line_replacement_counts", lines, 80, 8);
    }

    #[test]
    fn ui_snapshot_blank_context_line() {
        // Ensure a hunk that includes a blank context line at the beginning is rendered visibly
        let original = "\nY\n";
        let modified = "\nY changed\n";
        let patch = diffy::create_patch(original, modified).to_string();

        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            PathBuf::from("example.txt"),
            FileChange::Update {
                unified_diff: patch,
                move_path: None,
            },
        );

        let lines =
            create_diff_summary("proposed patch", &changes, PatchEventType::ApprovalRequest);

        snapshot_lines("blank_context_line", lines, 80, 10);
    }

    #[test]
    fn ui_snapshot_vertical_ellipsis_between_hunks() {
        // Create a patch with two separate hunks to ensure we render the vertical ellipsis (⋮)
        let original =
            "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\n";
        let modified = "line 1\nline two changed\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline nine changed\nline 10\n";
        let patch = diffy::create_patch(original, modified).to_string();

        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            PathBuf::from("example.txt"),
            FileChange::Update {
                unified_diff: patch,
                move_path: None,
            },
        );

        let lines =
            create_diff_summary("proposed patch", &changes, PatchEventType::ApprovalRequest);

        // Height is large enough to show both hunks and the separator
        snapshot_lines("vertical_ellipsis_between_hunks", lines, 80, 16);
    }
}
