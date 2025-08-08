use crossterm::terminal;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line as RtLine;
use ratatui::text::Span as RtSpan;
use std::collections::HashMap;
use std::path::PathBuf;

use codex_core::protocol::FileChange;

const DEFAULT_WRAP_COLS: usize = 96;
const SPACES_AFTER_LINE_NUMBER: usize = 6;

pub(crate) fn render_patch_details(changes: &HashMap<PathBuf, FileChange>) -> Vec<RtLine<'static>> {
    let mut out: Vec<RtLine<'static>> = Vec::new();
    let term_cols: usize = terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(DEFAULT_WRAP_COLS);

    let mut is_first_file = true;
    for (path, change) in changes.iter() {
        // Add separator only between files (not at the very start)
        if !is_first_file {
            out.push(RtLine::from(vec![
                RtSpan::raw("    "),
                RtSpan::styled("...", style_dim()),
            ]));
        }
        match change {
            FileChange::Add { content } => {
                let ln_width = usize::max(2, digits_len(content.lines().count()));
                for (i, raw) in content.lines().enumerate() {
                    let ln = i + 1;
                    push_wrapped_diff_line(
                        &mut out,
                        ln,
                        '+',
                        raw,
                        Some(style_add()),
                        term_cols,
                        ln_width,
                    );
                }
            }
            FileChange::Delete => {
                let original = std::fs::read_to_string(path).unwrap_or_default();
                let ln_width = usize::max(2, digits_len(original.lines().count()));
                for (i, raw) in original.lines().enumerate() {
                    let ln = i + 1;
                    push_wrapped_diff_line(
                        &mut out,
                        ln,
                        '-',
                        raw,
                        Some(style_del()),
                        term_cols,
                        ln_width,
                    );
                }
            }
            FileChange::Update {
                unified_diff,
                move_path: _,
            } => {
                if let Ok(patch) = diffy::Patch::from_str(unified_diff) {
                    for h in patch.hunks() {
                        // determine a reasonable ln field width for this hunk
                        let old_end = h.old_range().end();
                        let new_end = h.new_range().end();
                        let ln_width = usize::max(2, digits_len(old_end.max(new_end)));

                        let mut old_ln = h.old_range().start();
                        let mut new_ln = h.new_range().start();
                        for l in h.lines() {
                            match l {
                                diffy::Line::Insert(text) => {
                                    let s = text.trim_end_matches('\n');
                                    push_wrapped_diff_line(
                                        &mut out,
                                        new_ln,
                                        '+',
                                        s,
                                        Some(style_add()),
                                        term_cols,
                                        ln_width,
                                    );
                                    new_ln += 1;
                                }
                                diffy::Line::Delete(text) => {
                                    let s = text.trim_end_matches('\n');
                                    push_wrapped_diff_line(
                                        &mut out,
                                        old_ln,
                                        '-',
                                        s,
                                        Some(style_del()),
                                        term_cols,
                                        ln_width,
                                    );
                                    old_ln += 1;
                                }
                                diffy::Line::Context(text) => {
                                    let s = text.trim_end_matches('\n');
                                    push_wrapped_diff_line(
                                        &mut out, new_ln, ' ', s, None, term_cols, ln_width,
                                    );
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
        is_first_file = false;
    }

    out
}

pub(crate) fn create_diff_summary(
    title: &str,
    changes: &HashMap<PathBuf, FileChange>,
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
            // Fallback: manual scan to preserve counts even for unparseable diffs
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
        line.spans
            .iter_mut()
            .for_each(|span| span.style = span.style.add_modifier(Modifier::DIM));
        out.push(line);
    }

    out
}

fn push_wrapped_diff_line(
    out: &mut Vec<RtLine<'static>>,
    line_number: usize,
    sign: char,
    text: &str,
    bg_style: Option<Style>,
    term_cols: usize,
    _ln_width: usize,
) {
    let indent = "    ";
    let ln_str = line_number.to_string();
    let mut remaining: &str = text;

    // Reserve a fixed number of spaces after the line number so that content starts
    // at a consistent column. The sign ("+"/"-") is rendered as part of the content
    // with the same background as the edit, not as a separate dimmed column.
    let gap_after_ln = SPACES_AFTER_LINE_NUMBER.saturating_sub(ln_str.len());
    let first_prefix_cols = indent.len() + ln_str.len() + gap_after_ln;
    let cont_prefix_cols = indent.len() + ln_str.len() + gap_after_ln;

    let mut first = true;
    while !remaining.is_empty() {
        let prefix_cols = if first {
            first_prefix_cols
        } else {
            cont_prefix_cols
        };
        let available = term_cols.saturating_sub(prefix_cols).max(1);
        let take = remaining
            .char_indices()
            .nth(available)
            .map(|(i, _)| i)
            .unwrap_or_else(|| remaining.len());
        let (chunk, rest) = remaining.split_at(take);
        remaining = rest;

        if first {
            let mut spans: Vec<RtSpan<'static>> = Vec::new();
            spans.push(RtSpan::raw(indent));
            spans.push(RtSpan::styled(ln_str.clone(), style_dim()));
            spans.push(RtSpan::raw(" ".repeat(gap_after_ln)));

            // Prefix the content with the sign if it is an insertion or deletion, and color
            // the sign with the same background as the edited text.
            let display_chunk = match sign {
                '+' | '-' => {
                    let mut s = String::with_capacity(1 + chunk.len());
                    s.push(sign);
                    s.push_str(chunk);
                    s
                }
                _ => chunk.to_string(),
            };

            let content_span = match bg_style {
                Some(style) => RtSpan::styled(display_chunk, style),
                None => RtSpan::raw(display_chunk),
            };
            spans.push(content_span);
            out.push(RtLine::from(spans));
            first = false;
        } else {
            let hang_prefix = format!(
                "{indent}{}{}",
                " ".repeat(ln_str.len()),
                " ".repeat(gap_after_ln)
            );
            let content_span = match bg_style {
                Some(style) => RtSpan::styled(chunk.to_string(), style),
                None => RtSpan::raw(chunk.to_string()),
            };
            out.push(RtLine::from(vec![RtSpan::raw(hang_prefix), content_span]));
        }
    }
}

fn style_dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

fn style_add() -> Style {
    Style::default().bg(Color::Green)
}

fn style_del() -> Style {
    Style::default().bg(Color::Red)
}

#[inline]
fn digits_len(n: usize) -> usize {
    let mut d = 1;
    let mut x = n;
    while x >= 10 {
        x /= 10;
        d += 1;
    }
    d
}
