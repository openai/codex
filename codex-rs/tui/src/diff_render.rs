//! Renders unified diffs with line numbers, gutter signs, and optional syntax
//! highlighting.
//!
//! Each `FileChange` variant (Add / Delete / Update) is rendered as a block of
//! diff lines, each prefixed by a right-aligned line number, a gutter sign
//! (`+` / `-` / ` `), and the content text.  When a recognized file extension
//! is present, the content text is syntax-highlighted using
//! [`crate::render::highlight`].
//!
//! **Theme-aware styling:** diff backgrounds adapt to the terminal's
//! background lightness via [`DiffTheme`].  Dark terminals get muted tints
//! (`#212922` green, `#3C170F` red); light terminals get GitHub-style pastels
//! with distinct gutter backgrounds for contrast. The renderer uses fixed
//! palettes for truecolor / 256-color / 16-color terminals so add/delete lines
//! remain visually distinct even when quantizing to limited palettes.
//!
//! **Highlighting strategy for `Update` diffs:** the renderer highlights each
//! hunk as a single concatenated block rather than line-by-line.  This
//! preserves syntect's parser state across consecutive lines within a hunk
//! (important for multi-line strings, block comments, etc.).  Cross-hunk state
//! is intentionally *not* preserved because hunks are visually separated and
//! re-synchronize at context boundaries anyway.
//!
//! **Wrapping:** long lines are hard-wrapped at the available column width.
//! Syntax-highlighted spans are split at character boundaries with styles
//! preserved across the split so that no color information is lost.

use diffy::Hunk;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line as RtLine;
use ratatui::text::Span as RtSpan;
use ratatui::widgets::Paragraph;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use unicode_width::UnicodeWidthChar;

/// Display width of a tab character in columns.
const TAB_WIDTH: usize = 4;

// -- Diff background palette --------------------------------------------------
//
// Dark-theme tints are subtle enough to avoid clashing with syntax colors.
// Light-theme values match GitHub's diff colors for familiarity.  The gutter
// (line-number column) uses slightly more saturated variants on light
// backgrounds so the numbers remain readable against the pastel line background.
// Truecolor palette.
const DARK_TC_ADD_LINE_BG_RGB: (u8, u8, u8) = (33, 58, 43); // #213A2B
const DARK_TC_DEL_LINE_BG_RGB: (u8, u8, u8) = (74, 34, 29); // #4A221D
const LIGHT_TC_ADD_LINE_BG_RGB: (u8, u8, u8) = (218, 251, 225); // #dafbe1
const LIGHT_TC_DEL_LINE_BG_RGB: (u8, u8, u8) = (255, 235, 233); // #ffebe9
const LIGHT_TC_ADD_NUM_BG_RGB: (u8, u8, u8) = (172, 238, 187); // #aceebb
const LIGHT_TC_DEL_NUM_BG_RGB: (u8, u8, u8) = (255, 206, 203); // #ffcecb
const LIGHT_TC_GUTTER_FG_RGB: (u8, u8, u8) = (31, 35, 40); // #1f2328

// 256-color palette.
const DARK_256_ADD_LINE_BG_IDX: u8 = 22;
const DARK_256_DEL_LINE_BG_IDX: u8 = 52;
const LIGHT_256_ADD_LINE_BG_IDX: u8 = 194;
const LIGHT_256_DEL_LINE_BG_IDX: u8 = 224;
const LIGHT_256_ADD_NUM_BG_IDX: u8 = 157;
const LIGHT_256_DEL_NUM_BG_IDX: u8 = 217;
const LIGHT_256_GUTTER_FG_IDX: u8 = 236;

use crate::color::is_light;
use crate::exec_command::relativize_to_home;
use crate::render::Insets;
use crate::render::highlight::exceeds_highlight_limits;
use crate::render::highlight::highlight_code_to_styled_spans;
use crate::render::line_utils::prefix_lines;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::InsetRenderable;
use crate::render::renderable::Renderable;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::default_bg;
use crate::terminal_palette::indexed_color;
use crate::terminal_palette::rgb_color;
use crate::terminal_palette::stdout_color_level;
use codex_core::git_info::get_git_repo_root;
use codex_protocol::protocol::FileChange;

/// Classifies a diff line for gutter sign rendering and style selection.
///
/// `Insert` renders with a `+` sign and green text, `Delete` with `-` and red
/// text (plus dim overlay when syntax-highlighted), and `Context` with a space
/// and default styling.
#[derive(Clone, Copy)]
pub(crate) enum DiffLineType {
    Insert,
    Delete,
    Context,
}

/// Controls which color palette the diff renderer uses for backgrounds and
/// gutter styling.
///
/// Determined once per `render_change` call via [`diff_theme`], which probes
/// the terminal's queried background color.  When the background cannot be
/// determined (common in CI or piped output), `Dark` is used as the safe
/// default.
#[derive(Clone, Copy)]
enum DiffTheme {
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffColorLevel {
    TrueColor,
    Ansi256,
    Ansi16,
}

pub struct DiffSummary {
    changes: HashMap<PathBuf, FileChange>,
    cwd: PathBuf,
}

impl DiffSummary {
    pub fn new(changes: HashMap<PathBuf, FileChange>, cwd: PathBuf) -> Self {
        Self { changes, cwd }
    }
}

impl Renderable for FileChange {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let mut lines = vec![];
        render_change(self, &mut lines, area.width as usize, None);
        Paragraph::new(lines).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        let mut lines = vec![];
        render_change(self, &mut lines, width as usize, None);
        lines.len() as u16
    }
}

impl From<DiffSummary> for Box<dyn Renderable> {
    fn from(val: DiffSummary) -> Self {
        let mut rows: Vec<Box<dyn Renderable>> = vec![];

        for (i, row) in collect_rows(&val.changes).into_iter().enumerate() {
            if i > 0 {
                rows.push(Box::new(RtLine::from("")));
            }
            let mut path = RtLine::from(display_path_for(&row.path, &val.cwd));
            path.push_span(" ");
            path.extend(render_line_count_summary(row.added, row.removed));
            rows.push(Box::new(path));
            rows.push(Box::new(RtLine::from("")));
            rows.push(Box::new(InsetRenderable::new(
                Box::new(row.change) as Box<dyn Renderable>,
                Insets::tlbr(0, 2, 0, 0),
            )));
        }

        Box::new(ColumnRenderable::with(rows))
    }
}

pub(crate) fn create_diff_summary(
    changes: &HashMap<PathBuf, FileChange>,
    cwd: &Path,
    wrap_cols: usize,
) -> Vec<RtLine<'static>> {
    let rows = collect_rows(changes);
    render_changes_block(rows, wrap_cols, cwd)
}

// Shared row for per-file presentation
#[derive(Clone)]
struct Row {
    #[allow(dead_code)]
    path: PathBuf,
    move_path: Option<PathBuf>,
    added: usize,
    removed: usize,
    change: FileChange,
}

fn collect_rows(changes: &HashMap<PathBuf, FileChange>) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for (path, change) in changes.iter() {
        let (added, removed) = match change {
            FileChange::Add { content } => (content.lines().count(), 0),
            FileChange::Delete { content } => (0, content.lines().count()),
            FileChange::Update { unified_diff, .. } => calculate_add_remove_from_diff(unified_diff),
        };
        let move_path = match change {
            FileChange::Update {
                move_path: Some(new),
                ..
            } => Some(new.clone()),
            _ => None,
        };
        rows.push(Row {
            path: path.clone(),
            move_path,
            added,
            removed,
            change: change.clone(),
        });
    }
    rows.sort_by_key(|r| r.path.clone());
    rows
}

fn render_line_count_summary(added: usize, removed: usize) -> Vec<RtSpan<'static>> {
    let mut spans = Vec::new();
    spans.push("(".into());
    spans.push(format!("+{added}").green());
    spans.push(" ".into());
    spans.push(format!("-{removed}").red());
    spans.push(")".into());
    spans
}

fn render_changes_block(rows: Vec<Row>, wrap_cols: usize, cwd: &Path) -> Vec<RtLine<'static>> {
    let mut out: Vec<RtLine<'static>> = Vec::new();

    let render_path = |row: &Row| -> Vec<RtSpan<'static>> {
        let mut spans = Vec::new();
        spans.push(display_path_for(&row.path, cwd).into());
        if let Some(move_path) = &row.move_path {
            spans.push(format!(" → {}", display_path_for(move_path, cwd)).into());
        }
        spans
    };

    // Header
    let total_added: usize = rows.iter().map(|r| r.added).sum();
    let total_removed: usize = rows.iter().map(|r| r.removed).sum();
    let file_count = rows.len();
    let noun = if file_count == 1 { "file" } else { "files" };
    let mut header_spans: Vec<RtSpan<'static>> = vec!["• ".dim()];
    if let [row] = &rows[..] {
        let verb = match &row.change {
            FileChange::Add { .. } => "Added",
            FileChange::Delete { .. } => "Deleted",
            _ => "Edited",
        };
        header_spans.push(verb.bold());
        header_spans.push(" ".into());
        header_spans.extend(render_path(row));
        header_spans.push(" ".into());
        header_spans.extend(render_line_count_summary(row.added, row.removed));
    } else {
        header_spans.push("Edited".bold());
        header_spans.push(format!(" {file_count} {noun} ").into());
        header_spans.extend(render_line_count_summary(total_added, total_removed));
    }
    out.push(RtLine::from(header_spans));

    for (idx, r) in rows.into_iter().enumerate() {
        // Insert a blank separator between file chunks (except before the first)
        if idx > 0 {
            out.push("".into());
        }
        // File header line (skip when single-file header already shows the name)
        let skip_file_header = file_count == 1;
        if !skip_file_header {
            let mut header: Vec<RtSpan<'static>> = Vec::new();
            header.push("  └ ".dim());
            header.extend(render_path(&r));
            header.push(" ".into());
            header.extend(render_line_count_summary(r.added, r.removed));
            out.push(RtLine::from(header));
        }

        // For renames, use the destination extension for highlighting — the
        // diff content reflects the new file, not the old one.
        let lang_path = r.move_path.as_deref().unwrap_or(&r.path);
        let lang = detect_lang_for_path(lang_path);
        let mut lines = vec![];
        render_change(&r.change, &mut lines, wrap_cols - 4, lang.as_deref());
        out.extend(prefix_lines(lines, "    ".into(), "    ".into()));
    }

    out
}

/// Detect the programming language for a file path by its extension.
/// Returns the raw extension string for `normalize_lang` / `find_syntax`
/// to resolve downstream.
fn detect_lang_for_path(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?;
    Some(ext.to_string())
}

fn render_change(
    change: &FileChange,
    out: &mut Vec<RtLine<'static>>,
    width: usize,
    lang: Option<&str>,
) {
    let theme = diff_theme();
    let color_level = diff_color_level();
    match change {
        FileChange::Add { content } => {
            // Pre-highlight the entire file content as a whole.
            let syntax_lines = lang.and_then(|l| highlight_code_to_styled_spans(content, l));
            let line_number_width = line_number_width(content.lines().count());
            for (i, raw) in content.lines().enumerate() {
                let syn = syntax_lines.as_ref().and_then(|sl| sl.get(i));
                if let Some(spans) = syn {
                    out.extend(push_wrapped_diff_line_inner_with_theme_and_color_level(
                        i + 1,
                        DiffLineType::Insert,
                        raw,
                        width,
                        line_number_width,
                        Some(spans),
                        theme,
                        color_level,
                    ));
                } else {
                    out.extend(push_wrapped_diff_line_inner_with_theme_and_color_level(
                        i + 1,
                        DiffLineType::Insert,
                        raw,
                        width,
                        line_number_width,
                        None,
                        theme,
                        color_level,
                    ));
                }
            }
        }
        FileChange::Delete { content } => {
            let syntax_lines = lang.and_then(|l| highlight_code_to_styled_spans(content, l));
            let line_number_width = line_number_width(content.lines().count());
            for (i, raw) in content.lines().enumerate() {
                let syn = syntax_lines.as_ref().and_then(|sl| sl.get(i));
                if let Some(spans) = syn {
                    out.extend(push_wrapped_diff_line_inner_with_theme_and_color_level(
                        i + 1,
                        DiffLineType::Delete,
                        raw,
                        width,
                        line_number_width,
                        Some(spans),
                        theme,
                        color_level,
                    ));
                } else {
                    out.extend(push_wrapped_diff_line_inner_with_theme_and_color_level(
                        i + 1,
                        DiffLineType::Delete,
                        raw,
                        width,
                        line_number_width,
                        None,
                        theme,
                        color_level,
                    ));
                }
            }
        }
        FileChange::Update { unified_diff, .. } => {
            if let Ok(patch) = diffy::Patch::from_str(unified_diff) {
                let mut max_line_number = 0;
                let mut total_diff_bytes: usize = 0;
                let mut total_diff_lines: usize = 0;
                for h in patch.hunks() {
                    let mut old_ln = h.old_range().start();
                    let mut new_ln = h.new_range().start();
                    for l in h.lines() {
                        let text = match l {
                            diffy::Line::Insert(t)
                            | diffy::Line::Delete(t)
                            | diffy::Line::Context(t) => t,
                        };
                        total_diff_bytes += text.len();
                        total_diff_lines += 1;
                        match l {
                            diffy::Line::Insert(_) => {
                                max_line_number = max_line_number.max(new_ln);
                                new_ln += 1;
                            }
                            diffy::Line::Delete(_) => {
                                max_line_number = max_line_number.max(old_ln);
                                old_ln += 1;
                            }
                            diffy::Line::Context(_) => {
                                max_line_number = max_line_number.max(new_ln);
                                old_ln += 1;
                                new_ln += 1;
                            }
                        }
                    }
                }

                // Skip per-line syntax highlighting when the patch is too
                // large — avoids thousands of parser initializations that
                // would stall rendering on big diffs.
                let diff_lang = if exceeds_highlight_limits(total_diff_bytes, total_diff_lines) {
                    None
                } else {
                    lang
                };

                let line_number_width = line_number_width(max_line_number);
                let mut is_first_hunk = true;
                for h in patch.hunks() {
                    if !is_first_hunk {
                        let spacer = format!("{:width$} ", "", width = line_number_width.max(1));
                        let spacer_span = RtSpan::styled(
                            spacer,
                            style_gutter_for(DiffLineType::Context, theme, color_level),
                        );
                        out.push(RtLine::from(vec![spacer_span, "⋮".dim()]));
                    }
                    is_first_hunk = false;

                    // Highlight each hunk as a single block so syntect parser
                    // state is preserved across consecutive lines.
                    let hunk_syntax_lines = diff_lang.and_then(|language| {
                        let hunk_text: String = h
                            .lines()
                            .iter()
                            .map(|line| match line {
                                diffy::Line::Insert(text)
                                | diffy::Line::Delete(text)
                                | diffy::Line::Context(text) => *text,
                            })
                            .collect();
                        let syntax_lines = highlight_code_to_styled_spans(&hunk_text, language)?;
                        (syntax_lines.len() == h.lines().len()).then_some(syntax_lines)
                    });

                    let mut old_ln = h.old_range().start();
                    let mut new_ln = h.new_range().start();
                    for (line_idx, l) in h.lines().iter().enumerate() {
                        let syntax_spans = hunk_syntax_lines
                            .as_ref()
                            .and_then(|syntax_lines| syntax_lines.get(line_idx));
                        match l {
                            diffy::Line::Insert(text) => {
                                let s = text.trim_end_matches('\n');
                                if let Some(syn) = syntax_spans {
                                    out.extend(
                                        push_wrapped_diff_line_inner_with_theme_and_color_level(
                                            new_ln,
                                            DiffLineType::Insert,
                                            s,
                                            width,
                                            line_number_width,
                                            Some(syn),
                                            theme,
                                            color_level,
                                        ),
                                    );
                                } else {
                                    out.extend(
                                        push_wrapped_diff_line_inner_with_theme_and_color_level(
                                            new_ln,
                                            DiffLineType::Insert,
                                            s,
                                            width,
                                            line_number_width,
                                            None,
                                            theme,
                                            color_level,
                                        ),
                                    );
                                }
                                new_ln += 1;
                            }
                            diffy::Line::Delete(text) => {
                                let s = text.trim_end_matches('\n');
                                if let Some(syn) = syntax_spans {
                                    out.extend(
                                        push_wrapped_diff_line_inner_with_theme_and_color_level(
                                            old_ln,
                                            DiffLineType::Delete,
                                            s,
                                            width,
                                            line_number_width,
                                            Some(syn),
                                            theme,
                                            color_level,
                                        ),
                                    );
                                } else {
                                    out.extend(
                                        push_wrapped_diff_line_inner_with_theme_and_color_level(
                                            old_ln,
                                            DiffLineType::Delete,
                                            s,
                                            width,
                                            line_number_width,
                                            None,
                                            theme,
                                            color_level,
                                        ),
                                    );
                                }
                                old_ln += 1;
                            }
                            diffy::Line::Context(text) => {
                                let s = text.trim_end_matches('\n');
                                if let Some(syn) = syntax_spans {
                                    out.extend(
                                        push_wrapped_diff_line_inner_with_theme_and_color_level(
                                            new_ln,
                                            DiffLineType::Context,
                                            s,
                                            width,
                                            line_number_width,
                                            Some(syn),
                                            theme,
                                            color_level,
                                        ),
                                    );
                                } else {
                                    out.extend(
                                        push_wrapped_diff_line_inner_with_theme_and_color_level(
                                            new_ln,
                                            DiffLineType::Context,
                                            s,
                                            width,
                                            line_number_width,
                                            None,
                                            theme,
                                            color_level,
                                        ),
                                    );
                                }
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

/// Format a path for display relative to the current working directory when
/// possible, keeping output stable in jj/no-`.git` workspaces (e.g. image
/// tool calls should show `example.png` instead of an absolute path).
pub(crate) fn display_path_for(path: &Path, cwd: &Path) -> String {
    if path.is_relative() {
        return path.display().to_string();
    }

    if let Ok(stripped) = path.strip_prefix(cwd) {
        return stripped.display().to_string();
    }

    let path_in_same_repo = match (get_git_repo_root(cwd), get_git_repo_root(path)) {
        (Some(cwd_repo), Some(path_repo)) => cwd_repo == path_repo,
        _ => false,
    };
    let chosen = if path_in_same_repo {
        pathdiff::diff_paths(path, cwd).unwrap_or_else(|| path.to_path_buf())
    } else {
        relativize_to_home(path)
            .map(|p| PathBuf::from_iter([Path::new("~"), p.as_path()]))
            .unwrap_or_else(|| path.to_path_buf())
    };
    chosen.display().to_string()
}

pub(crate) fn calculate_add_remove_from_diff(diff: &str) -> (usize, usize) {
    if let Ok(patch) = diffy::Patch::from_str(diff) {
        patch
            .hunks()
            .iter()
            .flat_map(Hunk::lines)
            .fold((0, 0), |(a, d), l| match l {
                diffy::Line::Insert(_) => (a + 1, d),
                diffy::Line::Delete(_) => (a, d + 1),
                diffy::Line::Context(_) => (a, d),
            })
    } else {
        // For unparsable diffs, return 0 for both counts.
        (0, 0)
    }
}

/// Render a single diff line (no syntax highlighting) as one or more wrapped
/// ratatui `Line`s.  The first output line carries the gutter sign; continuation
/// lines are indented under the line number column.
pub(crate) fn push_wrapped_diff_line(
    line_number: usize,
    kind: DiffLineType,
    text: &str,
    width: usize,
    line_number_width: usize,
) -> Vec<RtLine<'static>> {
    push_wrapped_diff_line_inner(line_number, kind, text, width, line_number_width, None)
}

/// Render a single diff line with pre-computed syntax spans.  The sign character
/// uses the diff color; content gets syntax colors with a dim overlay for delete
/// lines.
pub(crate) fn push_wrapped_diff_line_with_syntax(
    line_number: usize,
    kind: DiffLineType,
    text: &str,
    width: usize,
    line_number_width: usize,
    syntax_spans: &[RtSpan<'static>],
) -> Vec<RtLine<'static>> {
    push_wrapped_diff_line_inner(
        line_number,
        kind,
        text,
        width,
        line_number_width,
        Some(syntax_spans),
    )
}

fn push_wrapped_diff_line_inner(
    line_number: usize,
    kind: DiffLineType,
    text: &str,
    width: usize,
    line_number_width: usize,
    syntax_spans: Option<&[RtSpan<'static>]>,
) -> Vec<RtLine<'static>> {
    push_wrapped_diff_line_inner_with_theme(
        line_number,
        kind,
        text,
        width,
        line_number_width,
        syntax_spans,
        diff_theme(),
    )
}

/// Core line renderer: builds one or more wrapped `RtLine`s for a single diff
/// line, applying gutter, sign, content, and full-width background styles
/// according to the given `theme`.
///
/// Split out from `push_wrapped_diff_line_inner` so that tests can exercise
/// specific `DiffTheme` variants without depending on the terminal's queried
/// background.
fn push_wrapped_diff_line_inner_with_theme(
    line_number: usize,
    kind: DiffLineType,
    text: &str,
    width: usize,
    line_number_width: usize,
    syntax_spans: Option<&[RtSpan<'static>]>,
    theme: DiffTheme,
) -> Vec<RtLine<'static>> {
    push_wrapped_diff_line_inner_with_theme_and_color_level(
        line_number,
        kind,
        text,
        width,
        line_number_width,
        syntax_spans,
        theme,
        diff_color_level(),
    )
}

#[allow(clippy::too_many_arguments)]
fn push_wrapped_diff_line_inner_with_theme_and_color_level(
    line_number: usize,
    kind: DiffLineType,
    text: &str,
    width: usize,
    line_number_width: usize,
    syntax_spans: Option<&[RtSpan<'static>]>,
    theme: DiffTheme,
    color_level: DiffColorLevel,
) -> Vec<RtLine<'static>> {
    let ln_str = line_number.to_string();

    // Reserve a fixed number of spaces (equal to the widest line number plus a
    // trailing spacer) so the sign column stays aligned across the diff block.
    let gutter_width = line_number_width.max(1);
    let prefix_cols = gutter_width + 1;

    let (sign_char, sign_style, content_style) = match kind {
        DiffLineType::Insert => (
            '+',
            style_sign_add(theme, color_level),
            style_add(theme, color_level),
        ),
        DiffLineType::Delete => (
            '-',
            style_sign_del(theme, color_level),
            style_del(theme, color_level),
        ),
        DiffLineType::Context => (' ', style_context(), style_context()),
    };

    let line_bg = style_line_bg_for(kind, theme, color_level);
    let gutter_style = style_gutter_for(kind, theme, color_level);

    // When we have syntax spans, compose them with the diff style for a richer
    // view. The sign character keeps the diff color; content gets syntax colors
    // with an overlay modifier for delete lines (dim).
    if let Some(syn_spans) = syntax_spans {
        let gutter = format!("{ln_str:>gutter_width$} ");
        let sign = format!("{sign_char}");
        let styled: Vec<RtSpan<'static>> = syn_spans
            .iter()
            .map(|sp| {
                let style = if matches!(kind, DiffLineType::Delete) {
                    sp.style.add_modifier(Modifier::DIM)
                } else {
                    sp.style
                };
                RtSpan::styled(sp.content.clone().into_owned(), style)
            })
            .collect();

        // Determine how many display columns remain for content after the
        // gutter and sign character.
        let available_content_cols = width.saturating_sub(prefix_cols + 1).max(1);

        // Wrap the styled content spans to fit within the available columns.
        let wrapped_chunks = wrap_styled_spans(&styled, available_content_cols);

        let mut lines: Vec<RtLine<'static>> = Vec::new();
        for (i, chunk) in wrapped_chunks.into_iter().enumerate() {
            let mut row_spans: Vec<RtSpan<'static>> = Vec::new();
            if i == 0 {
                // First line: gutter + sign + content
                row_spans.push(RtSpan::styled(gutter.clone(), gutter_style));
                row_spans.push(RtSpan::styled(sign.clone(), sign_style));
            } else {
                // Continuation: empty gutter + two-space indent (matches
                // the plain-text wrapping continuation style).
                let cont_gutter = format!("{:gutter_width$}  ", "");
                row_spans.push(RtSpan::styled(cont_gutter, gutter_style));
            }
            row_spans.extend(chunk);
            lines.push(RtLine::from(row_spans).style(line_bg));
        }
        return lines;
    }

    let available_content_cols = width.saturating_sub(prefix_cols + 1).max(1);
    let styled = vec![RtSpan::styled(text.to_string(), content_style)];
    let wrapped_chunks = wrap_styled_spans(&styled, available_content_cols);

    let mut lines: Vec<RtLine<'static>> = Vec::new();
    for (i, chunk) in wrapped_chunks.into_iter().enumerate() {
        let mut row_spans: Vec<RtSpan<'static>> = Vec::new();
        if i == 0 {
            let gutter = format!("{ln_str:>gutter_width$} ");
            let sign = format!("{sign_char}");
            row_spans.push(RtSpan::styled(gutter, gutter_style));
            row_spans.push(RtSpan::styled(sign, sign_style));
        } else {
            let cont_gutter = format!("{:gutter_width$}  ", "");
            row_spans.push(RtSpan::styled(cont_gutter, gutter_style));
        }
        row_spans.extend(chunk);
        lines.push(RtLine::from(row_spans).style(line_bg));
    }

    lines
}

/// Split styled spans into chunks that fit within `max_cols` display columns.
///
/// Returns one `Vec<RtSpan>` per output line.  Styles are preserved across
/// split boundaries so that wrapping never loses syntax coloring.
///
/// The algorithm walks characters using their Unicode display width (with tabs
/// expanded to [`TAB_WIDTH`] columns).  When a character would overflow the
/// current line, the accumulated text is flushed and a new line begins.  A
/// single character wider than the remaining space forces a line break *before*
/// the character so that progress is always made (avoiding infinite loops on
/// CJK characters or tabs at the end of a line).
fn wrap_styled_spans(spans: &[RtSpan<'static>], max_cols: usize) -> Vec<Vec<RtSpan<'static>>> {
    let mut result: Vec<Vec<RtSpan<'static>>> = Vec::new();
    let mut current_line: Vec<RtSpan<'static>> = Vec::new();
    let mut col: usize = 0;

    for span in spans {
        let style = span.style;
        let text = span.content.as_ref();
        let mut remaining = text;

        while !remaining.is_empty() {
            // Accumulate characters until we fill the line.
            let mut byte_end = 0;
            let mut chars_col = 0;

            for ch in remaining.chars() {
                // Tabs have no Unicode width; treat them as TAB_WIDTH columns.
                let w = ch.width().unwrap_or(if ch == '\t' { TAB_WIDTH } else { 0 });
                if col + chars_col + w > max_cols {
                    // Adding this character would exceed the line width.
                    // Break here; if this is the first character in `remaining`
                    // we will flush/start a new line in the `byte_end == 0`
                    // branch below before consuming it.
                    break;
                }
                byte_end += ch.len_utf8();
                chars_col += w;
            }

            if byte_end == 0 {
                // Single character wider than remaining space — force onto a
                // new line so we make progress.
                if !current_line.is_empty() {
                    result.push(std::mem::take(&mut current_line));
                }
                // Take at least one character to avoid an infinite loop.
                let Some(ch) = remaining.chars().next() else {
                    break;
                };
                let ch_len = ch.len_utf8();
                current_line.push(RtSpan::styled(remaining[..ch_len].to_string(), style));
                // Use fallback width 1 (not 0) so this branch always advances
                // even if `ch` has unknown/zero display width.
                col = ch.width().unwrap_or(if ch == '\t' { TAB_WIDTH } else { 1 });
                remaining = &remaining[ch_len..];
                continue;
            }

            let (chunk, rest) = remaining.split_at(byte_end);
            current_line.push(RtSpan::styled(chunk.to_string(), style));
            col += chars_col;
            remaining = rest;

            // If we exactly filled or exceeded the line, start a new one.
            // Do not gate on !remaining.is_empty() — the next span in the
            // outer loop may still have content that must start on a fresh line.
            if col >= max_cols {
                result.push(std::mem::take(&mut current_line));
                col = 0;
            }
        }
    }

    // Push the last line (always at least one, even if empty).
    if !current_line.is_empty() || result.is_empty() {
        result.push(current_line);
    }

    result
}

pub(crate) fn line_number_width(max_line_number: usize) -> usize {
    if max_line_number == 0 {
        1
    } else {
        max_line_number.to_string().len()
    }
}

/// Testable helper: picks `DiffTheme` from an explicit background sample.
fn diff_theme_for_bg(bg: Option<(u8, u8, u8)>) -> DiffTheme {
    if let Some(rgb) = bg
        && is_light(rgb)
    {
        return DiffTheme::Light;
    }
    DiffTheme::Dark
}

/// Probe the terminal's background and return the appropriate diff palette.
fn diff_theme() -> DiffTheme {
    diff_theme_for_bg(default_bg())
}

fn diff_color_level() -> DiffColorLevel {
    match stdout_color_level() {
        StdoutColorLevel::TrueColor => DiffColorLevel::TrueColor,
        StdoutColorLevel::Ansi256 => DiffColorLevel::Ansi256,
        StdoutColorLevel::Ansi16 | StdoutColorLevel::Unknown => DiffColorLevel::Ansi16,
    }
}

// -- Style helpers ------------------------------------------------------------
//
// Each diff line is composed of three visual regions, styled independently:
//
//   ┌──────────┬──────┬──────────────────────────────────────────┐
//   │  gutter  │ sign │              content                     │
//   │ (line #) │ +/-  │  (plain or syntax-highlighted text)      │
//   └──────────┴──────┴──────────────────────────────────────────┘
//
// A fourth, full-width layer — `line_bg` — is applied via `RtLine::style()`
// so that the background tint extends from the leftmost column to the right
// edge of the terminal, including any padding beyond the content.
//
// On dark terminals, the sign and content share one style (colored fg + tinted
// bg), and the gutter is simply dimmed.  On light terminals, sign and content
// are split: the sign gets only a colored foreground (no bg, so the line bg
// shows through), while content relies on the line bg alone; the gutter gets
// an opaque, more-saturated background so line numbers stay readable against
// the pastel line tint.

/// Full-width background applied to the `RtLine` itself (not individual spans).
/// Context lines intentionally leave the background unset so the terminal
/// default shows through.
fn style_line_bg_for(kind: DiffLineType, theme: DiffTheme, color_level: DiffColorLevel) -> Style {
    match kind {
        DiffLineType::Insert => Style::default().bg(add_line_bg(theme, color_level)),
        DiffLineType::Delete => Style::default().bg(del_line_bg(theme, color_level)),
        DiffLineType::Context => Style::default(),
    }
}

fn style_context() -> Style {
    Style::default()
}

fn add_line_bg(theme: DiffTheme, color_level: DiffColorLevel) -> Color {
    match (theme, color_level) {
        (DiffTheme::Dark, DiffColorLevel::TrueColor) => rgb_color(DARK_TC_ADD_LINE_BG_RGB),
        (DiffTheme::Dark, DiffColorLevel::Ansi256) => indexed_color(DARK_256_ADD_LINE_BG_IDX),
        (DiffTheme::Dark, DiffColorLevel::Ansi16) => Color::Green,
        (DiffTheme::Light, DiffColorLevel::TrueColor) => rgb_color(LIGHT_TC_ADD_LINE_BG_RGB),
        (DiffTheme::Light, DiffColorLevel::Ansi256) => indexed_color(LIGHT_256_ADD_LINE_BG_IDX),
        (DiffTheme::Light, DiffColorLevel::Ansi16) => Color::LightGreen,
    }
}

fn del_line_bg(theme: DiffTheme, color_level: DiffColorLevel) -> Color {
    match (theme, color_level) {
        (DiffTheme::Dark, DiffColorLevel::TrueColor) => rgb_color(DARK_TC_DEL_LINE_BG_RGB),
        (DiffTheme::Dark, DiffColorLevel::Ansi256) => indexed_color(DARK_256_DEL_LINE_BG_IDX),
        (DiffTheme::Dark, DiffColorLevel::Ansi16) => Color::Red,
        (DiffTheme::Light, DiffColorLevel::TrueColor) => rgb_color(LIGHT_TC_DEL_LINE_BG_RGB),
        (DiffTheme::Light, DiffColorLevel::Ansi256) => indexed_color(LIGHT_256_DEL_LINE_BG_IDX),
        (DiffTheme::Light, DiffColorLevel::Ansi16) => Color::LightRed,
    }
}

fn light_gutter_fg(color_level: DiffColorLevel) -> Color {
    match color_level {
        DiffColorLevel::TrueColor => rgb_color(LIGHT_TC_GUTTER_FG_RGB),
        DiffColorLevel::Ansi256 => indexed_color(LIGHT_256_GUTTER_FG_IDX),
        DiffColorLevel::Ansi16 => Color::Black,
    }
}

fn light_add_num_bg(color_level: DiffColorLevel) -> Color {
    match color_level {
        DiffColorLevel::TrueColor => rgb_color(LIGHT_TC_ADD_NUM_BG_RGB),
        DiffColorLevel::Ansi256 => indexed_color(LIGHT_256_ADD_NUM_BG_IDX),
        DiffColorLevel::Ansi16 => Color::Green,
    }
}

fn light_del_num_bg(color_level: DiffColorLevel) -> Color {
    match color_level {
        DiffColorLevel::TrueColor => rgb_color(LIGHT_TC_DEL_NUM_BG_RGB),
        DiffColorLevel::Ansi256 => indexed_color(LIGHT_256_DEL_NUM_BG_IDX),
        DiffColorLevel::Ansi16 => Color::Red,
    }
}

/// Line-number gutter style.  On light backgrounds the gutter has an opaque
/// tinted background so numbers contrast against the pastel line fill.  On
/// dark backgrounds a simple `DIM` modifier is sufficient.
fn style_gutter_for(kind: DiffLineType, theme: DiffTheme, color_level: DiffColorLevel) -> Style {
    match (theme, kind) {
        (DiffTheme::Light, DiffLineType::Insert) => Style::default()
            .fg(light_gutter_fg(color_level))
            .bg(light_add_num_bg(color_level)),
        (DiffTheme::Light, DiffLineType::Delete) => Style::default()
            .fg(light_gutter_fg(color_level))
            .bg(light_del_num_bg(color_level)),
        _ => style_gutter_dim(),
    }
}

/// Sign character (`+`) for insert lines.  On dark terminals it inherits the
/// full content style (green fg + tinted bg).  On light terminals it uses only
/// a green foreground and lets the line-level bg show through.
fn style_sign_add(theme: DiffTheme, color_level: DiffColorLevel) -> Style {
    match theme {
        DiffTheme::Light => Style::default().fg(Color::Green),
        DiffTheme::Dark => style_add(theme, color_level),
    }
}

/// Sign character (`-`) for delete lines.  Mirror of [`style_sign_add`].
fn style_sign_del(theme: DiffTheme, color_level: DiffColorLevel) -> Style {
    match theme {
        DiffTheme::Light => Style::default().fg(Color::Red),
        DiffTheme::Dark => style_del(theme, color_level),
    }
}

/// Content style for insert lines (plain, non-syntax-highlighted text).
fn style_add(theme: DiffTheme, color_level: DiffColorLevel) -> Style {
    match (theme, color_level) {
        (DiffTheme::Dark, DiffColorLevel::Ansi16) => Style::default()
            .fg(Color::Black)
            .bg(add_line_bg(theme, color_level)),
        (DiffTheme::Light, _) => Style::default().bg(add_line_bg(theme, color_level)),
        (DiffTheme::Dark, _) => Style::default()
            .fg(Color::Green)
            .bg(add_line_bg(theme, color_level)),
    }
}

/// Content style for delete lines (plain, non-syntax-highlighted text).
fn style_del(theme: DiffTheme, color_level: DiffColorLevel) -> Style {
    match (theme, color_level) {
        (DiffTheme::Dark, DiffColorLevel::Ansi16) => Style::default()
            .fg(Color::Black)
            .bg(del_line_bg(theme, color_level)),
        (DiffTheme::Light, _) => Style::default().bg(del_line_bg(theme, color_level)),
        (DiffTheme::Dark, _) => Style::default()
            .fg(Color::Red)
            .bg(del_line_bg(theme, color_level)),
    }
}

fn style_gutter_dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::text::Text;
    use ratatui::widgets::Paragraph;

    #[test]
    fn dark_ansi16_add_style_has_contrast() {
        let style = style_add(DiffTheme::Dark, DiffColorLevel::Ansi16);
        assert_eq!(style.fg, Some(Color::Black));
        assert_eq!(style.bg, Some(Color::Green));
        assert_ne!(style.fg, style.bg);
    }

    #[test]
    fn dark_ansi16_del_style_has_contrast() {
        let style = style_del(DiffTheme::Dark, DiffColorLevel::Ansi16);
        assert_eq!(style.fg, Some(Color::Black));
        assert_eq!(style.bg, Some(Color::Red));
        assert_ne!(style.fg, style.bg);
    }

    #[test]
    fn dark_ansi16_sign_styles_have_contrast() {
        let add_sign = style_sign_add(DiffTheme::Dark, DiffColorLevel::Ansi16);
        assert_eq!(add_sign.fg, Some(Color::Black));
        assert_eq!(add_sign.bg, Some(Color::Green));
        assert_ne!(add_sign.fg, add_sign.bg);

        let del_sign = style_sign_del(DiffTheme::Dark, DiffColorLevel::Ansi16);
        assert_eq!(del_sign.fg, Some(Color::Black));
        assert_eq!(del_sign.bg, Some(Color::Red));
        assert_ne!(del_sign.fg, del_sign.bg);
    }
    use ratatui::widgets::WidgetRef;
    use ratatui::widgets::Wrap;
    fn diff_summary_for_tests(changes: &HashMap<PathBuf, FileChange>) -> Vec<RtLine<'static>> {
        create_diff_summary(changes, &PathBuf::from("/"), 80)
    }

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

    fn display_width(text: &str) -> usize {
        text.chars()
            .map(|ch| ch.width().unwrap_or(if ch == '\t' { TAB_WIDTH } else { 0 }))
            .sum()
    }

    fn line_display_width(line: &RtLine<'static>) -> usize {
        line.spans
            .iter()
            .map(|span| display_width(span.content.as_ref()))
            .sum()
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

    fn diff_gallery_changes() -> HashMap<PathBuf, FileChange> {
        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();

        let rust_original =
            "fn greet(name: &str) {\n    println!(\"hello\");\n    println!(\"bye\");\n}\n";
        let rust_modified = "fn greet(name: &str) {\n    println!(\"hello {name}\");\n    println!(\"emoji: 🚀✨ and CJK: 你好世界\");\n}\n";
        let rust_patch = diffy::create_patch(rust_original, rust_modified).to_string();
        changes.insert(
            PathBuf::from("src/lib.rs"),
            FileChange::Update {
                unified_diff: rust_patch,
                move_path: None,
            },
        );

        let py_original = "def add(a, b):\n\treturn a + b\n\nprint(add(1, 2))\n";
        let py_modified = "def add(a, b):\n\treturn a + b + 42\n\nprint(add(1, 2))\n";
        let py_patch = diffy::create_patch(py_original, py_modified).to_string();
        changes.insert(
            PathBuf::from("scripts/calc.txt"),
            FileChange::Update {
                unified_diff: py_patch,
                move_path: Some(PathBuf::from("scripts/calc.py")),
            },
        );

        changes.insert(
            PathBuf::from("assets/banner.txt"),
            FileChange::Add {
                content: "HEADER\tVALUE\nrocket\t🚀\ncity\t東京\n".to_string(),
            },
        );
        changes.insert(
            PathBuf::from("examples/new_sample.rs"),
            FileChange::Add {
                content: "pub fn greet(name: &str) {\n    println!(\"Hello, {name}!\");\n}\n"
                    .to_string(),
            },
        );

        changes.insert(
            PathBuf::from("tmp/obsolete.log"),
            FileChange::Delete {
                content: "old line 1\nold line 2\nold line 3\n".to_string(),
            },
        );
        changes.insert(
            PathBuf::from("legacy/old_script.py"),
            FileChange::Delete {
                content: "def legacy(x):\n    return x + 1\nprint(legacy(3))\n".to_string(),
            },
        );

        changes
    }

    fn snapshot_diff_gallery(name: &str, width: u16, height: u16) {
        let lines = create_diff_summary(
            &diff_gallery_changes(),
            &PathBuf::from("/"),
            usize::from(width),
        );
        snapshot_lines(name, lines, width, height);
    }

    #[test]
    fn display_path_prefers_cwd_without_git_repo() {
        let cwd = if cfg!(windows) {
            PathBuf::from(r"C:\workspace\codex")
        } else {
            PathBuf::from("/workspace/codex")
        };
        let path = cwd.join("tui").join("example.png");

        let rendered = display_path_for(&path, &cwd);

        assert_eq!(
            rendered,
            PathBuf::from("tui")
                .join("example.png")
                .display()
                .to_string()
        );
    }

    #[test]
    fn ui_snapshot_wrap_behavior_insert() {
        // Narrow width to force wrapping within our diff line rendering
        let long_line = "this is a very long line that should wrap across multiple terminal columns and continue";

        // Call the wrapping function directly so we can precisely control the width
        let lines =
            push_wrapped_diff_line(1, DiffLineType::Insert, long_line, 80, line_number_width(1));

        // Render into a small terminal to capture the visual layout
        snapshot_lines("wrap_behavior_insert", lines, 90, 8);
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

        let lines = diff_summary_for_tests(&changes);

        snapshot_lines("apply_update_block", lines, 80, 12);
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

        let lines = diff_summary_for_tests(&changes);

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

        let lines = diff_summary_for_tests(&changes);

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

        let lines = diff_summary_for_tests(&changes);

        snapshot_lines("apply_add_block", lines, 80, 10);
    }

    #[test]
    fn ui_snapshot_apply_delete_block() {
        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            PathBuf::from("tmp_delete_example.txt"),
            FileChange::Delete {
                content: "first\nsecond\nthird\n".to_string(),
            },
        );

        let lines = diff_summary_for_tests(&changes);
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

        let lines = create_diff_summary(&changes, &PathBuf::from("/"), 72);

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

        let lines = create_diff_summary(&changes, &PathBuf::from("/"), 28);
        snapshot_lines_text("apply_update_block_wraps_long_lines_text", &lines);
    }

    #[test]
    fn ui_snapshot_apply_update_block_line_numbers_three_digits_text() {
        let original = (1..=110).map(|i| format!("line {i}\n")).collect::<String>();
        let modified = (1..=110)
            .map(|i| {
                if i == 100 {
                    format!("line {i} changed\n")
                } else {
                    format!("line {i}\n")
                }
            })
            .collect::<String>();
        let patch = diffy::create_patch(&original, &modified).to_string();

        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            PathBuf::from("hundreds.txt"),
            FileChange::Update {
                unified_diff: patch,
                move_path: None,
            },
        );

        let lines = create_diff_summary(&changes, &PathBuf::from("/"), 80);
        snapshot_lines_text("apply_update_block_line_numbers_three_digits_text", &lines);
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
            abs_old,
            FileChange::Update {
                unified_diff: patch,
                move_path: Some(abs_new),
            },
        );

        let lines = create_diff_summary(&changes, &cwd, 80);

        snapshot_lines("apply_update_block_relativizes_path", lines, 80, 10);
    }

    #[test]
    fn ui_snapshot_syntax_highlighted_insert_wraps() {
        // A long Rust line that exceeds 80 cols with syntax highlighting should
        // wrap to multiple output lines rather than being clipped.
        let long_rust = "fn very_long_function_name(arg_one: String, arg_two: String, arg_three: String, arg_four: String) -> Result<String, Box<dyn std::error::Error>> { Ok(arg_one) }";

        let syntax_spans =
            highlight_code_to_styled_spans(long_rust, "rust").expect("rust highlighting");
        let spans = &syntax_spans[0];

        let lines = push_wrapped_diff_line_with_syntax(
            1,
            DiffLineType::Insert,
            long_rust,
            80,
            line_number_width(1),
            spans,
        );

        assert!(
            lines.len() > 1,
            "syntax-highlighted long line should wrap to multiple lines, got {}",
            lines.len()
        );

        snapshot_lines("syntax_highlighted_insert_wraps", lines, 90, 10);
    }

    #[test]
    fn ui_snapshot_syntax_highlighted_insert_wraps_text() {
        let long_rust = "fn very_long_function_name(arg_one: String, arg_two: String, arg_three: String, arg_four: String) -> Result<String, Box<dyn std::error::Error>> { Ok(arg_one) }";

        let syntax_spans =
            highlight_code_to_styled_spans(long_rust, "rust").expect("rust highlighting");
        let spans = &syntax_spans[0];

        let lines = push_wrapped_diff_line_with_syntax(
            1,
            DiffLineType::Insert,
            long_rust,
            80,
            line_number_width(1),
            spans,
        );

        snapshot_lines_text("syntax_highlighted_insert_wraps_text", &lines);
    }

    #[test]
    fn ui_snapshot_diff_gallery_80x24() {
        snapshot_diff_gallery("diff_gallery_80x24", 80, 24);
    }

    #[test]
    fn ui_snapshot_diff_gallery_94x35() {
        snapshot_diff_gallery("diff_gallery_94x35", 94, 35);
    }

    #[test]
    fn ui_snapshot_diff_gallery_120x40() {
        snapshot_diff_gallery("diff_gallery_120x40", 120, 40);
    }

    #[test]
    fn truecolor_dark_theme_uses_configured_backgrounds() {
        assert_eq!(
            style_line_bg_for(
                DiffLineType::Insert,
                DiffTheme::Dark,
                DiffColorLevel::TrueColor
            ),
            Style::default().bg(rgb_color(DARK_TC_ADD_LINE_BG_RGB))
        );
        assert_eq!(
            style_line_bg_for(
                DiffLineType::Delete,
                DiffTheme::Dark,
                DiffColorLevel::TrueColor
            ),
            Style::default().bg(rgb_color(DARK_TC_DEL_LINE_BG_RGB))
        );
        assert_eq!(
            style_gutter_for(
                DiffLineType::Insert,
                DiffTheme::Dark,
                DiffColorLevel::TrueColor
            ),
            style_gutter_dim()
        );
        assert_eq!(
            style_gutter_for(
                DiffLineType::Delete,
                DiffTheme::Dark,
                DiffColorLevel::TrueColor
            ),
            style_gutter_dim()
        );
    }

    #[test]
    fn ansi256_dark_theme_uses_distinct_add_and_delete_backgrounds() {
        assert_eq!(
            style_line_bg_for(
                DiffLineType::Insert,
                DiffTheme::Dark,
                DiffColorLevel::Ansi256
            ),
            Style::default().bg(indexed_color(DARK_256_ADD_LINE_BG_IDX))
        );
        assert_eq!(
            style_line_bg_for(
                DiffLineType::Delete,
                DiffTheme::Dark,
                DiffColorLevel::Ansi256
            ),
            Style::default().bg(indexed_color(DARK_256_DEL_LINE_BG_IDX))
        );
        assert_ne!(
            style_line_bg_for(
                DiffLineType::Insert,
                DiffTheme::Dark,
                DiffColorLevel::Ansi256
            ),
            style_line_bg_for(
                DiffLineType::Delete,
                DiffTheme::Dark,
                DiffColorLevel::Ansi256
            ),
            "256-color mode should keep add/delete backgrounds distinct"
        );
    }

    #[test]
    fn light_truecolor_theme_uses_readable_gutter_and_line_backgrounds() {
        assert_eq!(
            style_line_bg_for(
                DiffLineType::Insert,
                DiffTheme::Light,
                DiffColorLevel::TrueColor
            ),
            Style::default().bg(rgb_color(LIGHT_TC_ADD_LINE_BG_RGB))
        );
        assert_eq!(
            style_line_bg_for(
                DiffLineType::Delete,
                DiffTheme::Light,
                DiffColorLevel::TrueColor
            ),
            Style::default().bg(rgb_color(LIGHT_TC_DEL_LINE_BG_RGB))
        );
        assert_eq!(
            style_gutter_for(
                DiffLineType::Insert,
                DiffTheme::Light,
                DiffColorLevel::TrueColor
            ),
            Style::default()
                .fg(rgb_color(LIGHT_TC_GUTTER_FG_RGB))
                .bg(rgb_color(LIGHT_TC_ADD_NUM_BG_RGB))
        );
        assert_eq!(
            style_gutter_for(
                DiffLineType::Delete,
                DiffTheme::Light,
                DiffColorLevel::TrueColor
            ),
            Style::default()
                .fg(rgb_color(LIGHT_TC_GUTTER_FG_RGB))
                .bg(rgb_color(LIGHT_TC_DEL_NUM_BG_RGB))
        );
    }

    #[test]
    fn light_theme_wrapped_lines_keep_number_gutter_contrast() {
        let lines = push_wrapped_diff_line_inner_with_theme_and_color_level(
            12,
            DiffLineType::Insert,
            "abcdefghij",
            8,
            line_number_width(12),
            None,
            DiffTheme::Light,
            DiffColorLevel::TrueColor,
        );

        assert!(
            lines.len() > 1,
            "expected wrapped output for gutter style verification"
        );
        assert_eq!(
            lines[0].spans[0].style,
            Style::default()
                .fg(rgb_color(LIGHT_TC_GUTTER_FG_RGB))
                .bg(rgb_color(LIGHT_TC_ADD_NUM_BG_RGB))
        );
        assert_eq!(
            lines[1].spans[0].style,
            Style::default()
                .fg(rgb_color(LIGHT_TC_GUTTER_FG_RGB))
                .bg(rgb_color(LIGHT_TC_ADD_NUM_BG_RGB))
        );
        assert_eq!(lines[0].style.bg, Some(rgb_color(LIGHT_TC_ADD_LINE_BG_RGB)));
        assert_eq!(lines[1].style.bg, Some(rgb_color(LIGHT_TC_ADD_LINE_BG_RGB)));
    }

    #[test]
    fn add_diff_uses_path_extension_for_highlighting() {
        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            PathBuf::from("highlight_add.rs"),
            FileChange::Add {
                content: "pub fn sum(a: i32, b: i32) -> i32 { a + b }\n".to_string(),
            },
        );

        let lines = create_diff_summary(&changes, &PathBuf::from("/"), 80);
        let has_rgb = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|s| matches!(s.style.fg, Some(ratatui::style::Color::Rgb(..))))
        });
        assert!(
            has_rgb,
            "add diff for .rs file should produce syntax-highlighted (RGB) spans"
        );
    }

    #[test]
    fn delete_diff_uses_path_extension_for_highlighting() {
        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            PathBuf::from("highlight_delete.py"),
            FileChange::Delete {
                content: "def scale(x):\n    return x * 2\n".to_string(),
            },
        );

        let lines = create_diff_summary(&changes, &PathBuf::from("/"), 80);
        let has_rgb = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|s| matches!(s.style.fg, Some(ratatui::style::Color::Rgb(..))))
        });
        assert!(
            has_rgb,
            "delete diff for .py file should produce syntax-highlighted (RGB) spans"
        );
    }

    #[test]
    fn detect_lang_for_common_paths() {
        // Standard extensions are detected.
        assert!(detect_lang_for_path(Path::new("foo.rs")).is_some());
        assert!(detect_lang_for_path(Path::new("bar.py")).is_some());
        assert!(detect_lang_for_path(Path::new("app.tsx")).is_some());

        // Extensionless files return None.
        assert!(detect_lang_for_path(Path::new("Makefile")).is_none());
        assert!(detect_lang_for_path(Path::new("randomfile")).is_none());
    }

    #[test]
    fn wrap_styled_spans_single_line() {
        // Content that fits in one line should produce exactly one chunk.
        let spans = vec![RtSpan::raw("short")];
        let result = wrap_styled_spans(&spans, 80);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn wrap_styled_spans_splits_long_content() {
        // Content wider than max_cols should produce multiple chunks.
        let long_text = "a".repeat(100);
        let spans = vec![RtSpan::raw(long_text)];
        let result = wrap_styled_spans(&spans, 40);
        assert!(
            result.len() >= 3,
            "100 chars at 40 cols should produce at least 3 lines, got {}",
            result.len()
        );
    }

    #[test]
    fn wrap_styled_spans_flushes_at_span_boundary() {
        // When span A fills exactly to max_cols and span B follows, the line
        // must be flushed before B starts. Otherwise B's first character lands
        // on an already-full line, producing over-width output.
        let style_a = Style::default().fg(Color::Red);
        let style_b = Style::default().fg(Color::Blue);
        let spans = vec![
            RtSpan::styled("aaaa", style_a), // 4 cols, fills line exactly at max_cols=4
            RtSpan::styled("bb", style_b),   // should start on a new line
        ];
        let result = wrap_styled_spans(&spans, 4);
        assert_eq!(
            result.len(),
            2,
            "span ending exactly at max_cols should flush before next span: {result:?}"
        );
        // First line should only contain the 'a' span.
        let first_width: usize = result[0].iter().map(|s| s.content.chars().count()).sum();
        assert!(
            first_width <= 4,
            "first line should be at most 4 cols wide, got {first_width}"
        );
    }

    #[test]
    fn wrap_styled_spans_preserves_styles() {
        // Verify that styles survive split boundaries.
        let style = Style::default().fg(Color::Green);
        let text = "x".repeat(50);
        let spans = vec![RtSpan::styled(text, style)];
        let result = wrap_styled_spans(&spans, 20);
        for chunk in &result {
            for span in chunk {
                assert_eq!(span.style, style, "style should be preserved across wraps");
            }
        }
    }

    #[test]
    fn wrap_styled_spans_tabs_have_visible_width() {
        // A tab should count as TAB_WIDTH columns, not zero.
        // With max_cols=8, a tab (4 cols) + "abcde" (5 cols) = 9 cols → must wrap.
        let spans = vec![RtSpan::raw("\tabcde")];
        let result = wrap_styled_spans(&spans, 8);
        assert!(
            result.len() >= 2,
            "tab + 5 chars should exceed 8 cols and wrap, got {} line(s): {result:?}",
            result.len()
        );
    }

    #[test]
    fn wrap_styled_spans_wraps_before_first_overflowing_char() {
        let spans = vec![RtSpan::raw("abcd\t界")];
        let result = wrap_styled_spans(&spans, 5);

        let line_text: Vec<String> = result
            .iter()
            .map(|line| {
                line.iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect();
        assert_eq!(line_text, vec!["abcd", "\t", "界"]);

        let line_width = |line: &[RtSpan<'static>]| -> usize {
            line.iter()
                .flat_map(|span| span.content.chars())
                .map(|ch| ch.width().unwrap_or(if ch == '\t' { TAB_WIDTH } else { 0 }))
                .sum()
        };
        for line in &result {
            assert!(
                line_width(line) <= 5,
                "wrapped line exceeded width 5: {line:?}"
            );
        }
    }

    #[test]
    fn fallback_wrapping_uses_display_width_for_tabs_and_wide_chars() {
        let width = 8;
        let lines = push_wrapped_diff_line(
            1,
            DiffLineType::Insert,
            "abcd\t界🙂",
            width,
            line_number_width(1),
        );

        assert!(lines.len() >= 2, "expected wrapped output, got {lines:?}");
        for line in &lines {
            assert!(
                line_display_width(line) <= width,
                "fallback wrapped line exceeded width {width}: {line:?}"
            );
        }
    }

    #[test]
    fn large_update_diff_skips_highlighting() {
        // Build a patch large enough to exceed MAX_HIGHLIGHT_LINES (10_000).
        // Without the pre-check this would attempt 10k+ parser initializations.
        let line_count = 10_500;
        let original: String = (0..line_count).map(|i| format!("line {i}\n")).collect();
        let modified: String = (0..line_count)
            .map(|i| {
                if i % 2 == 0 {
                    format!("line {i} changed\n")
                } else {
                    format!("line {i}\n")
                }
            })
            .collect();
        let patch = diffy::create_patch(&original, &modified).to_string();

        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            PathBuf::from("huge.rs"),
            FileChange::Update {
                unified_diff: patch,
                move_path: None,
            },
        );

        // Should complete quickly (no per-line parser init). If guardrails
        // are bypassed this would be extremely slow.
        let lines = create_diff_summary(&changes, &PathBuf::from("/"), 80);

        // The diff rendered without timing out — the guardrails prevented
        // thousands of per-line parser initializations.  Verify we actually
        // got output (the patch is non-empty).
        assert!(
            lines.len() > 100,
            "expected many output lines from large diff, got {}",
            lines.len(),
        );

        // No span should contain an RGB foreground color (syntax themes
        // produce RGB; plain diff styles only use named Color variants).
        for line in &lines {
            for span in &line.spans {
                if let Some(ratatui::style::Color::Rgb(..)) = span.style.fg {
                    panic!(
                        "large diff should not have syntax-highlighted spans, \
                         got RGB color in style {:?} for {:?}",
                        span.style, span.content,
                    );
                }
            }
        }
    }

    #[test]
    fn rename_diff_uses_destination_extension_for_highlighting() {
        // A rename from an unknown extension to .rs should highlight as Rust.
        // Without the fix, detect_lang_for_path uses the source path (.xyzzy),
        // which has no syntax definition, so highlighting is skipped.
        let original = "fn main() {}\n";
        let modified = "fn main() { println!(\"hi\"); }\n";
        let patch = diffy::create_patch(original, modified).to_string();

        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            PathBuf::from("foo.xyzzy"),
            FileChange::Update {
                unified_diff: patch,
                move_path: Some(PathBuf::from("foo.rs")),
            },
        );

        let lines = create_diff_summary(&changes, &PathBuf::from("/"), 80);
        let has_rgb = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|s| matches!(s.style.fg, Some(ratatui::style::Color::Rgb(..))))
        });
        assert!(
            has_rgb,
            "rename from .xyzzy to .rs should produce syntax-highlighted (RGB) spans"
        );
    }

    #[test]
    fn update_diff_preserves_multiline_highlight_state_within_hunk() {
        let original = "fn demo() {\n    let s = \"hello\";\n}\n";
        let modified = "fn demo() {\n    let s = \"hello\nworld\";\n}\n";
        let patch = diffy::create_patch(original, modified).to_string();

        let mut changes: HashMap<PathBuf, FileChange> = HashMap::new();
        changes.insert(
            PathBuf::from("demo.rs"),
            FileChange::Update {
                unified_diff: patch,
                move_path: None,
            },
        );

        let expected_multiline =
            highlight_code_to_styled_spans("    let s = \"hello\nworld\";\n", "rust")
                .expect("rust highlighting");
        let expected_style = expected_multiline
            .get(1)
            .and_then(|line| {
                line.iter()
                    .find(|span| span.content.as_ref().contains("world"))
            })
            .map(|span| span.style)
            .expect("expected highlighted span for second multiline string line");

        let lines = create_diff_summary(&changes, &PathBuf::from("/"), 120);
        let actual_style = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.as_ref().contains("world"))
            .map(|span| span.style)
            .expect("expected rendered diff span containing 'world'");

        assert_eq!(actual_style, expected_style);
    }
}
