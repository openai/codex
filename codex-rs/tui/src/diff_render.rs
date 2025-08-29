use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line as RtLine;
use ratatui::text::Span as RtSpan;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

// No DEFAULT_WRAP_COLS in runtime paths; tests pass explicit widths.
use crate::exec_command::relativize_to_home;
use codex_core::protocol::FileChange;
use codex_core::util::is_inside_git_repo;

use crate::history_cell::PatchEventType;

const SPACES_AFTER_LINE_NUMBER: usize = 6;

// Internal representation for diff line rendering
enum DiffLineType {
    Insert,
    Delete,
    Context,
}

// Old create_diff_summary removed; callers must use create_diff_summary

pub(crate) fn create_diff_summary(
    changes: &HashMap<PathBuf, FileChange>,
    event_type: PatchEventType,
    wrap_cols: usize,
) -> Vec<RtLine<'static>> {
    match event_type {
        PatchEventType::ApplyBegin { auto_approved } => {
            render_applied_changes_block(changes, wrap_cols, auto_approved)
        }
        PatchEventType::ApprovalRequest => render_proposed_blocks(changes, wrap_cols),
    }
}

// Shared row for per-file presentation
#[derive(Clone)]
struct Row {
    path: PathBuf,
    display: String,
    added: usize,
    removed: usize,
    change: FileChange,
}

fn collect_rows(changes: &HashMap<PathBuf, FileChange>, cwd: &Path) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for (path, change) in changes.iter() {
        let (a, d) = match change {
            FileChange::Add { content } => (content.lines().count(), 0),
            FileChange::Delete => (
                0,
                std::fs::read_to_string(path)
                    .ok()
                    .map(|s| s.lines().count())
                    .unwrap_or(0),
            ),
            FileChange::Update { unified_diff, .. } => calculate_add_remove_from_diff(unified_diff),
        };
        let display = match change {
            FileChange::Update {
                move_path: Some(new),
                ..
            } => display_path_for(path, Some(new), cwd),
            _ => display_path_for(path, None, cwd),
        };
        rows.push(Row {
            path: path.clone(),
            display,
            added: a,
            removed: d,
            change: change.clone(),
        });
    }
    rows.sort_by_key(|r| r.display.clone());
    rows
}

enum HeaderKind {
    ProposedChange,
    Edited,
    ChangeApproved,
}

fn render_changes_block(
    rows: Vec<Row>,
    wrap_cols: usize,
    header_kind: HeaderKind,
) -> Vec<RtLine<'static>> {
    let mut out: Vec<RtLine<'static>> = Vec::new();
    let term_cols = wrap_cols;

    // Header
    let total_added: usize = rows.iter().map(|r| r.added).sum();
    let total_removed: usize = rows.iter().map(|r| r.removed).sum();
    let file_count = rows.len();
    let noun = if file_count == 1 { "file" } else { "files" };
    let mut header_spans: Vec<RtSpan<'static>> = vec!["• ".into()];
    let single_file_inline = file_count == 1;
    let first_row_opt = rows.first().cloned();
    match header_kind {
        HeaderKind::ProposedChange => {
            header_spans.push("Proposed Change".bold());
            if single_file_inline {
                if let Some(fr) = &first_row_opt {
                    header_spans.push(format!(" {} ", fr.display).into());
                    header_spans.push("(".into());
                    header_spans.push(format!("+{}", fr.added).green());
                    header_spans.push(" ".into());
                    header_spans.push(format!("-{}", fr.removed).red());
                    header_spans.push(")".into());
                }
            } else {
                header_spans.push(format!(" to {file_count} {noun} ").into());
                header_spans.push("(".into());
                header_spans.push(format!("+{total_added}").green());
                header_spans.push(" ".into());
                header_spans.push(format!("-{total_removed}").red());
                header_spans.push(")".into());
            }
        }
        HeaderKind::Edited => {
            // For a single file, specialize the verb based on the change kind.
            // Otherwise, use the generic "Edited" summary.
            let verb = if single_file_inline {
                match first_row_opt.as_ref().map(|r| &r.change) {
                    Some(FileChange::Add { .. }) => "Added",
                    Some(FileChange::Delete) => "Deleted",
                    _ => "Edited",
                }
            } else {
                "Edited"
            };
            header_spans.push(verb.bold());
            if single_file_inline {
                if let Some(fr) = &first_row_opt {
                    header_spans.push(format!(" {} ", fr.display).into());
                    header_spans.push("(".into());
                    header_spans.push(format!("+{}", fr.added).green());
                    header_spans.push(" ".into());
                    header_spans.push(format!("-{}", fr.removed).red());
                    header_spans.push(")".into());
                } else {
                    header_spans.push(format!(" {file_count} {noun} ").into());
                    header_spans.push("(".into());
                    header_spans.push(RtSpan::styled(
                        format!("+{total_added}"),
                        Style::default().fg(Color::Green),
                    ));
                    header_spans.push(" ".into());
                    header_spans.push(RtSpan::styled(
                        format!("-{total_removed}"),
                        Style::default().fg(Color::Red),
                    ));
                    header_spans.push(")".into());
                }
            } else {
                header_spans.push(format!(" {file_count} {noun} ").into());
                header_spans.push("(".into());
                header_spans.push(format!("+{total_added}").green());
                header_spans.push(" ".into());
                header_spans.push(format!("-{total_removed}").red());
                header_spans.push(")".into());
            }
        }
        HeaderKind::ChangeApproved => {
            header_spans.push("Change Approved".bold());
            header_spans.push(format!(" {file_count} {noun} ").into());
            header_spans.push("(".into());
            header_spans.push(format!("+{total_added}").green());
            header_spans.push(" ".into());
            header_spans.push(format!("-{total_removed}").red());
            header_spans.push(")".into());
        }
    }
    out.push(RtLine::from(header_spans));

    // For Change Approved, we only show the header summary and no per-file/diff details.
    if matches!(header_kind, HeaderKind::ChangeApproved) {
        return out;
    }

    for (idx, r) in rows.into_iter().enumerate() {
        // Insert a blank separator between file chunks (except before the first)
        if idx > 0 {
            out.push(RtLine::from(RtSpan::raw("")));
        }
        // File header line (skip when single-file header already shows the name)
        let skip_file_header =
            matches!(header_kind, HeaderKind::ProposedChange | HeaderKind::Edited)
                && file_count == 1;
        if !skip_file_header {
            let mut header: Vec<RtSpan<'static>> = Vec::new();
            header.push("  └ ".dim());
            header.push(r.display.clone().into());
            let mut parts: Vec<RtSpan<'static>> = Vec::new();
            if r.added > 0 {
                parts.push(format!("+{}", r.added).green());
            }
            if r.removed > 0 {
                if !parts.is_empty() {
                    parts.push(" ".into());
                }
                parts.push(format!("-{}", r.removed).red());
            }
            if !parts.is_empty() {
                header.push(" (".into());
                header.extend(parts);
                header.push(")".into());
            }
            out.push(RtLine::from(header));
        }

        match r.change {
            FileChange::Add { content } => {
                for (i, raw) in content.lines().enumerate() {
                    out.extend(push_wrapped_diff_line(
                        i + 1,
                        DiffLineType::Insert,
                        raw,
                        term_cols,
                    ));
                }
            }
            FileChange::Delete => {
                let original = std::fs::read_to_string(r.path).unwrap_or_default();
                for (i, raw) in original.lines().enumerate() {
                    out.extend(push_wrapped_diff_line(
                        i + 1,
                        DiffLineType::Delete,
                        raw,
                        term_cols,
                    ));
                }
            }
            FileChange::Update { unified_diff, .. } => {
                if let Ok(patch) = diffy::Patch::from_str(&unified_diff) {
                    let mut is_first_hunk = true;
                    for h in patch.hunks() {
                        if !is_first_hunk {
                            out.push(RtLine::from(vec!["    ".into(), "⋮".dim()]));
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
    }

    out
}

fn render_proposed_blocks(
    changes: &HashMap<PathBuf, FileChange>,
    wrap_cols: usize,
) -> Vec<RtLine<'static>> {
    let cwd: PathBuf = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let rows = collect_rows(changes, &cwd);
    render_changes_block(rows, wrap_cols, HeaderKind::ProposedChange)
}

fn render_applied_changes_block(
    changes: &HashMap<PathBuf, FileChange>,
    wrap_cols: usize,
    auto_approved: bool,
) -> Vec<RtLine<'static>> {
    let cwd: PathBuf = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let rows = collect_rows(changes, &cwd);
    let header_kind = if auto_approved {
        HeaderKind::Edited
    } else {
        HeaderKind::ChangeApproved
    };
    render_changes_block(rows, wrap_cols, header_kind)
}

fn relative_from(base: &Path, path: &Path) -> Option<PathBuf> {
    // Only produce a relative path if both are absolute with compatible prefixes
    #[cfg(windows)]
    {
        use std::path::Component;
        let mut base_iter = base.components();
        let mut path_iter = path.components();
        let base_prefix = match base_iter.next() {
            Some(Component::Prefix(p)) => Some(p),
            _ => None,
        };
        let path_prefix = match path_iter.next() {
            Some(Component::Prefix(p)) => Some(p),
            _ => None,
        };
        if base_prefix != path_prefix {
            return None;
        }
        // Put back first non-root dir component
    }

    // Find common ancestor
    let base_abs = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let path_abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut base_iter = base_abs.components();
    let mut path_iter = path_abs.components();
    let mut common: Vec<std::path::Component> = Vec::new();
    loop {
        match (base_iter.clone().next(), path_iter.clone().next()) {
            (Some(bc), Some(pc)) if bc == pc => {
                common.push(bc);
                base_iter.next();
                path_iter.next();
            }
            _ => break,
        }
    }
    // Count remaining components in base_iter
    let mut rel = PathBuf::new();
    for _ in base_iter {
        rel.push("..");
    }
    for c in path_iter {
        rel.push(c.as_os_str());
    }
    Some(rel)
}

fn display_path_for(path: &Path, move_path: Option<&Path>, cwd: &Path) -> String {
    // Determine if cwd is inside a git repo; if so, and path looks like it's inside
    // a git repo (based on TurnDiffTracker's relative_to_git_root_str), then render
    // it relative to cwd (may include ".." segments). Otherwise, relativize to home.
    let in_repo = is_inside_git_repo(cwd);

    let one = |p: &Path| -> String {
        // Consider the path "in a repo" if its parent is inside a git repo
        // (lightweight check that doesn't invoke git).
        let looks_in_repo = is_inside_git_repo(p.parent().unwrap_or(p));
        let chosen = if in_repo && looks_in_repo {
            relative_from(cwd, p).unwrap_or_else(|| p.to_path_buf())
        } else {
            relativize_to_home(p).unwrap_or_else(|| p.to_path_buf())
        };
        chosen.display().to_string()
    };
    match move_path {
        Some(new_path) => format!("{} → {}", one(path), one(new_path)),
        None => one(path),
    }
}

fn calculate_add_remove_from_diff(diff: &str) -> (usize, usize) {
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
        let available_content_cols = term_cols.saturating_sub(prefix_cols + 1).max(1);
        let split_at_byte_index = remaining_text
            .char_indices()
            .nth(available_content_cols)
            .map(|(i, _)| i)
            .unwrap_or_else(|| remaining_text.len());
        let (chunk, rest) = remaining_text.split_at(split_at_byte_index);
        remaining_text = rest;

        if first {
            // Build gutter (indent + line number + spacing) as a dimmed span
            let gutter = format!("{indent}{ln_str}{}", " ".repeat(gap_after_ln));
            let mut spans: Vec<RtSpan<'static>> = Vec::new();
            spans.push(RtSpan::styled(gutter, style_dim()));
            // Content with a sign ('+'/'-'/' ') styled per diff kind
            let sign_char = sign_opt.unwrap_or(' ');
            let content = format!("{sign_char}{chunk}");
            let content_span = match line_style {
                Some(style) => RtSpan::styled(content, style),
                None => RtSpan::raw(content),
            };
            spans.push(content_span);
            lines.push(RtLine::from(spans));
            first = false;
        } else {
            // Continuation lines keep a space for the sign column so content aligns
            let hang_prefix = format!(
                "{indent}{}{} ",
                " ".repeat(ln_str.len()),
                " ".repeat(gap_after_ln)
            );
            let mut spans: Vec<RtSpan<'static>> = Vec::new();
            spans.push(RtSpan::styled(hang_prefix, style_dim()));
            let content_span = match line_style {
                Some(style) => RtSpan::styled(chunk.to_string(), style),
                None => RtSpan::raw(chunk.to_string()),
            };
            spans.push(content_span);
            lines.push(RtLine::from(spans));
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

    fn snapshot_lines_text(name: &str, lines: &[RtLine<'static>]) {
        // Convert Lines to plain text rows and trim trailing spaces so it's
        // easier to validate indentation visually in snapshots.
        let text = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .map(|s| s.trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert_snapshot!(name, text);
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

        let lines = create_diff_summary(&changes, PatchEventType::ApprovalRequest, 80);

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

        let lines = create_diff_summary(&changes, PatchEventType::ApprovalRequest, 80);

        snapshot_lines("update_details_with_rename", lines, 80, 12);
    }

    #[test]
    fn ui_snapshot_wrap_behavior_insert() {
        // Narrow width to force wrapping within our diff line rendering
        let long_line = "this is a very long line that should wrap across multiple terminal columns and continue";

        // Call the wrapping function directly so we can precisely control the width
        let lines = push_wrapped_diff_line(1, DiffLineType::Insert, long_line, 80);

        // Render into a small terminal to capture the visual layout
        snapshot_lines("wrap_behavior_insert", lines, 90, 8);
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

        let lines = create_diff_summary(&changes, PatchEventType::ApprovalRequest, 80);

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

        let lines = create_diff_summary(&changes, PatchEventType::ApprovalRequest, 80);

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

        let lines = create_diff_summary(&changes, PatchEventType::ApprovalRequest, 80);

        // Height is large enough to show both hunks and the separator
        snapshot_lines("vertical_ellipsis_between_hunks", lines, 80, 16);
    }

    #[test]
    fn ui_snapshot_apply_update_block() {
        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        let original = "line one\nline two\nline three\n";
        let modified = "line one\nline two changed\nline three\n";
        let patch = diffy::create_patch(original, modified).to_string();

        changes.insert(
            PathBuf::from("example.txt"),
            FileChange::Update {
                unified_diff: patch,
                move_path: None,
            },
        );

        for (name, auto_approved) in [
            ("apply_update_block", true),
            ("apply_update_block_manual", false),
        ] {
            let lines =
                create_diff_summary(&changes, PatchEventType::ApplyBegin { auto_approved }, 80);

            snapshot_lines(name, lines, 80, 12);
        }
    }

    #[test]
    fn ui_snapshot_apply_update_with_rename_block() {
        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        let original = "A\nB\nC\n";
        let modified = "A\nB changed\nC\n";
        let patch = diffy::create_patch(original, modified).to_string();

        changes.insert(
            PathBuf::from("old_name.rs"),
            FileChange::Update {
                unified_diff: patch,
                move_path: Some(PathBuf::from("new_name.rs")),
            },
        );

        let lines = create_diff_summary(
            &changes,
            PatchEventType::ApplyBegin {
                auto_approved: true,
            },
            80,
        );

        snapshot_lines("apply_update_with_rename_block", lines, 80, 12);
    }

    #[test]
    fn ui_snapshot_apply_multiple_files_block() {
        // Two files: one update and one add, to exercise combined header and per-file rows
        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();

        // File a.txt: single-line replacement (one delete, one insert)
        let patch_a = diffy::create_patch("one\n", "one changed\n").to_string();
        changes.insert(
            PathBuf::from("a.txt"),
            FileChange::Update {
                unified_diff: patch_a,
                move_path: None,
            },
        );

        // File b.txt: newly added with one line
        changes.insert(
            PathBuf::from("b.txt"),
            FileChange::Add {
                content: "new\n".to_string(),
            },
        );

        let lines = create_diff_summary(
            &changes,
            PatchEventType::ApplyBegin {
                auto_approved: true,
            },
            80,
        );

        snapshot_lines("apply_multiple_files_block", lines, 80, 14);
    }

    #[test]
    fn ui_snapshot_apply_add_block() {
        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            PathBuf::from("new_file.txt"),
            FileChange::Add {
                content: "alpha\nbeta\n".to_string(),
            },
        );

        let lines = create_diff_summary(
            &changes,
            PatchEventType::ApplyBegin {
                auto_approved: true,
            },
            80,
        );

        snapshot_lines("apply_add_block", lines, 80, 10);
    }

    #[test]
    fn ui_snapshot_apply_delete_block() {
        // Write a temporary file so the delete renderer can read original content
        let tmp_path = PathBuf::from("tmp_delete_example.txt");
        std::fs::write(&tmp_path, "first\nsecond\nthird\n").expect("write tmp file");

        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(tmp_path.clone(), FileChange::Delete);

        let lines = create_diff_summary(
            &changes,
            PatchEventType::ApplyBegin {
                auto_approved: true,
            },
            80,
        );

        // Cleanup best-effort; rendering has already read the file
        let _ = std::fs::remove_file(&tmp_path);

        snapshot_lines("apply_delete_block", lines, 80, 12);
    }

    #[test]
    fn ui_snapshot_apply_update_block_wraps_long_lines() {
        // Create a patch with a long modified line to force wrapping
        let original = "line 1\nshort\nline 3\n";
        let modified = "line 1\nshort this_is_a_very_long_modified_line_that_should_wrap_across_multiple_terminal_columns_and_continue_even_further_beyond_eighty_columns_to_force_multiple_wraps\nline 3\n";
        let patch = diffy::create_patch(original, modified).to_string();

        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            PathBuf::from("long_example.txt"),
            FileChange::Update {
                unified_diff: patch,
                move_path: None,
            },
        );

        let lines = create_diff_summary(
            &changes,
            PatchEventType::ApplyBegin {
                auto_approved: true,
            },
            72,
        );

        // Render with backend width wider than wrap width to avoid Paragraph auto-wrap.
        snapshot_lines("apply_update_block_wraps_long_lines", lines, 80, 12);
    }

    #[test]
    fn ui_snapshot_apply_update_block_wraps_long_lines_text() {
        // This mirrors the desired layout example: sign only on first inserted line,
        // subsequent wrapped pieces start aligned under the line number gutter.
        let original = "1\n2\n3\n4\n";
        let modified = "1\nadded long line which wraps and_if_there_is_a_long_token_it_will_be_broken\n3\n4 context line which also wraps across\n";
        let patch = diffy::create_patch(original, modified).to_string();

        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            PathBuf::from("wrap_demo.txt"),
            FileChange::Update {
                unified_diff: patch,
                move_path: None,
            },
        );

        let mut lines = create_diff_summary(
            &changes,
            PatchEventType::ApplyBegin {
                auto_approved: true,
            },
            28,
        );
        // Drop the combined header for this text-only snapshot
        if !lines.is_empty() {
            lines.remove(0);
        }
        snapshot_lines_text("apply_update_block_wraps_long_lines_text", &lines);
    }

    #[test]
    fn ui_snapshot_apply_update_block_relativizes_path() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let abs_old = cwd.join("abs_old.rs");
        let abs_new = cwd.join("abs_new.rs");

        let original = "X\nY\n";
        let modified = "X changed\nY\n";
        let patch = diffy::create_patch(original, modified).to_string();

        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            abs_old.clone(),
            FileChange::Update {
                unified_diff: patch,
                move_path: Some(abs_new.clone()),
            },
        );

        let lines = create_diff_summary(
            &changes,
            PatchEventType::ApplyBegin {
                auto_approved: true,
            },
            80,
        );

        snapshot_lines("apply_update_block_relativizes_path", lines, 80, 10);
    }
}
