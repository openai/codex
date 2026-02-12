//! Builds the `/theme` picker dialog for the TUI.
//!
//! The picker lists all bundled themes plus any custom `.tmTheme` files found
//! under `{CODEX_HOME}/themes/`.  It provides:
//!
//! - **Live preview:** the `on_selection_changed` callback swaps the runtime
//!   syntax theme as the user navigates, giving instant visual feedback in both
//!   the preview panel and any visible code blocks.
//! - **Cancel-restore:** on dismiss (Esc / Ctrl+C) the `on_cancel` callback
//!   restores the theme snapshot taken when the picker opened.
//! - **Persist on confirm:** the `AppEvent::SyntaxThemeSelected` action persists
//!   `[tui] theme = "..."` to `config.toml` via `ConfigEditsBuilder`.
//!
//! Two preview renderables adapt to terminal width:
//!
//! - `ThemePreviewWideRenderable` -- vertically centered, inset by 2 columns,
//!   shown in the side panel when the terminal is wide enough for side-by-side
//!   layout (>= 44-column side panel and >= 40-column list).
//! - `ThemePreviewNarrowRenderable` -- compact 4-line snippet stacked below the
//!   list when side-by-side does not fit.

use std::path::Path;

use crate::app_event::AppEvent;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::SideContentWidth;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::bottom_pane::popup_content_width;
use crate::bottom_pane::side_by_side_layout_widths;
use crate::diff_render::DiffLineType;
use crate::diff_render::line_number_width;
use crate::diff_render::push_wrapped_diff_line;
use crate::diff_render::push_wrapped_diff_line_with_syntax;
use crate::render::highlight;
use crate::render::renderable::Renderable;
use crate::status::format_directory_display;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreviewDiffKind {
    Context,
    Added,
    Removed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreviewRow {
    line_no: usize,
    kind: PreviewDiffKind,
    code: &'static str,
}

/// Compact fallback preview used in stacked (narrow) mode.
/// Keep exactly one removed and one added line visible at all times.
const NARROW_PREVIEW_ROWS: [PreviewRow; 4] = [
    PreviewRow {
        line_no: 12,
        kind: PreviewDiffKind::Context,
        code: "fn greet(name: &str) -> String {",
    },
    PreviewRow {
        line_no: 13,
        kind: PreviewDiffKind::Removed,
        code: "    format!(\"Hello, {}!\", name)",
    },
    PreviewRow {
        line_no: 13,
        kind: PreviewDiffKind::Added,
        code: "    format!(\"Hello, {name}!\")",
    },
    PreviewRow {
        line_no: 14,
        kind: PreviewDiffKind::Context,
        code: "}",
    },
];

/// Wider diff preview used in side-by-side mode.
/// This sample intentionally mixes context, additions, and removals.
const WIDE_PREVIEW_ROWS: [PreviewRow; 8] = [
    PreviewRow {
        line_no: 31,
        kind: PreviewDiffKind::Context,
        code: "fn summarize(users: &[User]) -> String {",
    },
    PreviewRow {
        line_no: 32,
        kind: PreviewDiffKind::Removed,
        code: "    let active = users.iter().filter(|u| u.is_active).count();",
    },
    PreviewRow {
        line_no: 32,
        kind: PreviewDiffKind::Added,
        code: "    let active = users.iter().filter(|u| u.is_active()).count();",
    },
    PreviewRow {
        line_no: 33,
        kind: PreviewDiffKind::Context,
        code: "    let names: Vec<&str> = users.iter().map(User::name).take(3).collect();",
    },
    PreviewRow {
        line_no: 34,
        kind: PreviewDiffKind::Removed,
        code: "    format!(\"{} active: {}\", active, names.join(\", \"))",
    },
    PreviewRow {
        line_no: 34,
        kind: PreviewDiffKind::Added,
        code: "    format!(\"{active} active users: {}\", names.join(\", \"))",
    },
    PreviewRow {
        line_no: 35,
        kind: PreviewDiffKind::Added,
        code: "        .trim()",
    },
    PreviewRow {
        line_no: 36,
        kind: PreviewDiffKind::Context,
        code: "}",
    },
];

/// Minimum side-panel width for side-by-side theme preview.
const WIDE_PREVIEW_MIN_WIDTH: u16 = 44;

/// Left inset used for wide preview content.
const WIDE_PREVIEW_LEFT_INSET: u16 = 2;

/// Minimum frame padding used for vertically centered wide preview.
const PREVIEW_FRAME_PADDING: u16 = 1;

const PREVIEW_FALLBACK_SUBTITLE: &str = "Move up/down to live preview themes";

/// Side-by-side preview: syntax-highlighted Rust diff snippet, vertically
/// centered with a 2-column left inset.  Fills the entire side panel height.
struct ThemePreviewWideRenderable;

/// Stacked preview: compact 4-line Rust diff snippet shown below the list
/// when the terminal is too narrow for side-by-side layout.
struct ThemePreviewNarrowRenderable;

fn preview_diff_line_type(kind: PreviewDiffKind) -> DiffLineType {
    match kind {
        PreviewDiffKind::Context => DiffLineType::Context,
        PreviewDiffKind::Added => DiffLineType::Insert,
        PreviewDiffKind::Removed => DiffLineType::Delete,
    }
}

fn centered_offset(available: u16, content: u16, min_frame: u16) -> u16 {
    let free = available.saturating_sub(content);
    let frame = if free >= min_frame.saturating_mul(2) {
        min_frame
    } else {
        0
    };
    frame + free.saturating_sub(frame.saturating_mul(2)) / 2
}

fn render_preview(
    area: Rect,
    buf: &mut Buffer,
    preview_rows: &[PreviewRow],
    center_vertically: bool,
    left_inset: u16,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    if preview_rows.is_empty() {
        return;
    }
    let preview_code = preview_rows
        .iter()
        .map(|row| row.code)
        .collect::<Vec<_>>()
        .join("\n");
    let syntax_lines = highlight::highlight_code_to_styled_spans(&preview_code, "rust");

    let max_line_no = preview_rows
        .iter()
        .map(|row| row.line_no)
        .max()
        .unwrap_or(1);
    let ln_width = line_number_width(max_line_no);

    let content_height = (preview_rows.len() as u16).min(area.height);

    let left_pad = left_inset.min(area.width.saturating_sub(1));
    let top_pad = if center_vertically {
        centered_offset(area.height, content_height, PREVIEW_FRAME_PADDING)
    } else {
        0
    };

    let mut y = area.y.saturating_add(top_pad);
    let render_width = area.width.saturating_sub(left_pad);
    for (idx, row) in preview_rows.iter().enumerate() {
        if y >= area.y + area.height {
            break;
        }
        let diff_type = preview_diff_line_type(row.kind);
        let wrapped = if let Some(syn) = syntax_lines.as_ref().and_then(|sl| sl.get(idx)) {
            push_wrapped_diff_line_with_syntax(
                row.line_no,
                diff_type,
                row.code,
                render_width as usize,
                ln_width,
                syn,
            )
        } else {
            push_wrapped_diff_line(
                row.line_no,
                diff_type,
                row.code,
                render_width as usize,
                ln_width,
            )
        };
        let first_line = wrapped.into_iter().next().unwrap_or_else(|| Line::from(""));
        first_line.render(
            Rect::new(area.x.saturating_add(left_pad), y, render_width, 1),
            buf,
        );
        y += 1;
    }
}

impl Renderable for ThemePreviewWideRenderable {
    fn desired_height(&self, _width: u16) -> u16 {
        u16::MAX
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        render_preview(area, buf, &WIDE_PREVIEW_ROWS, true, WIDE_PREVIEW_LEFT_INSET);
    }
}

impl Renderable for ThemePreviewNarrowRenderable {
    fn desired_height(&self, _width: u16) -> u16 {
        NARROW_PREVIEW_ROWS.len() as u16
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        render_preview(area, buf, &NARROW_PREVIEW_ROWS, false, 0);
    }
}

fn subtitle_available_width(terminal_width: Option<u16>) -> usize {
    let width = terminal_width.unwrap_or(80);
    let content_width = popup_content_width(width);
    if let Some((list_width, _side_width)) = side_by_side_layout_widths(
        content_width,
        SideContentWidth::Half,
        WIDE_PREVIEW_MIN_WIDTH,
    ) {
        list_width as usize
    } else {
        content_width as usize
    }
}

fn theme_picker_subtitle(codex_home: Option<&Path>, terminal_width: Option<u16>) -> String {
    let themes_dir = codex_home.map(|home| home.join("themes"));
    let themes_dir_display = themes_dir
        .as_deref()
        .map(|path| format_directory_display(path, None));
    let available_width = subtitle_available_width(terminal_width);

    if let Some(path) = themes_dir_display
        && path.starts_with('~')
    {
        let subtitle = format!("Custom .tmTheme files can be added to the {path} directory.");
        if UnicodeWidthStr::width(subtitle.as_str()) <= available_width {
            return subtitle;
        }
    }

    PREVIEW_FALLBACK_SUBTITLE.to_string()
}

/// Builds [`SelectionViewParams`] for the `/theme` picker dialog.
///
/// Lists all bundled themes plus custom `.tmTheme` files, with live preview
/// on cursor movement and cancel-restore.
///
/// `current_name` should be the value of `Config::tui_theme` (the persisted
/// preference).  When it names a theme that is currently available the picker
/// pre-selects it; otherwise the picker falls back to the configured name (or
/// adaptive default) so opening the picker without a persisted preference still
/// highlights the most likely intended entry.
pub(crate) fn build_theme_picker_params(
    current_name: Option<&str>,
    codex_home: Option<&Path>,
    terminal_width: Option<u16>,
) -> SelectionViewParams {
    // Snapshot the current theme so we can restore on cancel.
    let original_theme = highlight::current_syntax_theme();

    let entries = highlight::list_available_themes(codex_home);
    let codex_home_owned = codex_home.map(Path::to_path_buf);

    // Resolve the effective theme name: honor explicit config only when it is
    // currently available; otherwise fall back to configured/default selection
    // so opening `/theme` does not auto-preview an unrelated first entry.
    let effective_name = if let Some(name) = current_name
        && entries.iter().any(|entry| entry.name == name)
    {
        name.to_string()
    } else {
        highlight::configured_theme_name()
    };

    // Track the index of the current theme so we can pre-select it.
    let mut initial_idx = None;

    let items: Vec<SelectionItem> = entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let display_name = if entry.is_custom {
                format!("{} (custom)", entry.name)
            } else {
                entry.name.clone()
            };
            let is_current = entry.name == effective_name;
            if is_current {
                initial_idx = Some(idx);
            }
            let name_for_action = entry.name.clone();
            SelectionItem {
                name: display_name,
                is_current,
                dismiss_on_select: true,
                search_value: Some(entry.name.clone()),
                actions: vec![Box::new(move |tx| {
                    tx.send(AppEvent::SyntaxThemeSelected {
                        name: name_for_action.clone(),
                    });
                })],
                ..Default::default()
            }
        })
        .collect();

    // Derive preview targets from the final `items` list (not from `entries`)
    // so preview ordering stays aligned if item construction/sorting changes.
    let preview_theme_names: Vec<Option<String>> =
        items.iter().map(|item| item.search_value.clone()).collect();
    let preview_home = codex_home_owned.clone();
    let on_selection_changed = Some(Box::new(move |idx: usize, _tx: &_| {
        if let Some(Some(name)) = preview_theme_names.get(idx)
            && let Some(theme) = highlight::resolve_theme_by_name(name, preview_home.as_deref())
        {
            highlight::set_syntax_theme(theme);
        }
    })
        as Box<dyn Fn(usize, &crate::app_event_sender::AppEventSender) + Send + Sync>);

    // Restore original theme on cancel.
    let on_cancel = Some(Box::new(move |_tx: &_| {
        highlight::set_syntax_theme(original_theme.clone());
    })
        as Box<dyn Fn(&crate::app_event_sender::AppEventSender) + Send + Sync>);
    SelectionViewParams {
        title: Some("Select Syntax Theme".to_string()),
        subtitle: Some(theme_picker_subtitle(
            codex_home_owned.as_deref(),
            terminal_width,
        )),
        footer_hint: Some(standard_popup_hint_line()),
        items,
        is_searchable: true,
        search_placeholder: Some("Type to filter themes...".to_string()),
        initial_selected_idx: initial_idx,
        side_content: Box::new(ThemePreviewWideRenderable),
        side_content_width: SideContentWidth::Half,
        side_content_min_width: WIDE_PREVIEW_MIN_WIDTH,
        stacked_side_content: Some(Box::new(ThemePreviewNarrowRenderable)),
        on_selection_changed,
        on_cancel,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::style::Modifier;

    fn render_buffer(renderable: &dyn Renderable, width: u16, height: u16) -> Buffer {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        renderable.render(area, &mut buf);
        buf
    }

    fn render_lines(renderable: &dyn Renderable, width: u16, height: u16) -> Vec<String> {
        let buf = render_buffer(renderable, width, height);
        (0..height)
            .map(|row| {
                let mut line = String::new();
                for col in 0..width {
                    let symbol = buf[(col, row)].symbol();
                    if symbol.is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(symbol);
                    }
                }
                line
            })
            .collect()
    }

    fn first_non_space_style_after_marker(buf: &Buffer, row: u16, width: u16) -> Option<Modifier> {
        let marker_col = (0..width)
            .find(|&col| buf[(col, row)].symbol() == "-" || buf[(col, row)].symbol() == "+")?;
        for col in marker_col + 1..width {
            if buf[(col, row)].symbol() != " " {
                return Some(buf[(col, row)].style().add_modifier);
            }
        }
        None
    }

    fn preview_line_number(line: &str) -> Option<usize> {
        let trimmed = line.trim_start();
        let digits_len = trimmed.chars().take_while(char::is_ascii_digit).count();
        if digits_len == 0 {
            return None;
        }
        let digits = &trimmed[..digits_len];
        if !trimmed[digits_len..].starts_with(' ') {
            return None;
        }
        digits.parse::<usize>().ok()
    }

    fn preview_line_marker(line: &str) -> Option<char> {
        let trimmed = line.trim_start();
        let digits_len = trimmed.chars().take_while(char::is_ascii_digit).count();
        if digits_len == 0 {
            return None;
        }
        let mut chars = trimmed[digits_len..].chars();
        if chars.next()? != ' ' {
            return None;
        }
        chars.next()
    }

    #[test]
    fn theme_picker_uses_half_width_with_stacked_fallback_preview() {
        let params = build_theme_picker_params(None, None, None);
        assert_eq!(params.side_content_width, SideContentWidth::Half);
        assert_eq!(params.side_content_min_width, WIDE_PREVIEW_MIN_WIDTH);
        assert!(params.stacked_side_content.is_some());
    }

    #[test]
    fn theme_picker_items_include_search_values_for_preview_mapping() {
        let params = build_theme_picker_params(None, None, None);
        assert!(
            params.items.iter().all(|item| item.search_value.is_some()),
            "theme picker preview mapping relies on item search_value to stay aligned with final item order"
        );
    }

    #[test]
    fn wide_preview_renders_all_lines_with_vertical_center_and_left_inset() {
        let lines = render_lines(&ThemePreviewWideRenderable, 80, 20);
        let numbered_rows: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter_map(|(idx, line)| preview_line_number(line).map(|_| idx))
            .collect();
        let total_preview_lines = WIDE_PREVIEW_ROWS.len();

        assert_eq!(numbered_rows.len(), total_preview_lines);
        let first_row = *numbered_rows
            .first()
            .expect("expected at least one preview row");
        let last_row = *numbered_rows
            .last()
            .expect("expected at least one preview row");
        assert!(
            first_row > 0,
            "expected top padding before centered preview"
        );
        assert!(
            last_row < 19,
            "expected bottom padding after centered preview"
        );

        let first_line = &lines[first_row];
        assert!(
            first_line.starts_with("  31  fn summarize"),
            "expected wide preview to start after a 2-char inset"
        );

        let markers: Vec<char> = lines
            .iter()
            .filter_map(|line| preview_line_marker(line))
            .collect();
        assert!(
            markers.contains(&'+'),
            "expected wide preview to include at least one addition line"
        );
        assert!(
            markers.contains(&'-'),
            "expected wide preview to include at least one removal line"
        );
    }

    #[test]
    fn narrow_preview_renders_single_add_and_single_remove_in_four_lines() {
        let lines = render_lines(&ThemePreviewNarrowRenderable, 80, 6);
        let numbered_lines: Vec<usize> = lines
            .iter()
            .filter_map(|line| preview_line_number(line))
            .collect();
        let markers: Vec<char> = lines
            .iter()
            .filter_map(|line| preview_line_marker(line))
            .collect();

        assert_eq!(numbered_lines, vec![12, 13, 13, 14]);
        assert_eq!(markers.len(), 4);
        assert_eq!(markers.iter().filter(|&&m| m == '+').count(), 1);
        assert_eq!(markers.iter().filter(|&&m| m == '-').count(), 1);
        let first_numbered = lines
            .iter()
            .find(|line| preview_line_number(line).is_some())
            .expect("expected at least one rendered preview row");
        assert!(
            first_numbered.starts_with("12  fn greet"),
            "expected narrow preview line numbers to start at the left edge"
        );
    }

    #[test]
    fn deleted_preview_code_uses_dim_overlay_like_real_diff_renderer() {
        let width = 80;
        let height = 6;
        let buf = render_buffer(&ThemePreviewNarrowRenderable, width, height);
        let lines = render_lines(&ThemePreviewNarrowRenderable, width, height);
        let deleted_row = lines
            .iter()
            .enumerate()
            .find_map(|(row, line)| (preview_line_marker(line) == Some('-')).then_some(row as u16))
            .expect("expected a deleted preview row");
        let modifiers = first_non_space_style_after_marker(&buf, deleted_row, width)
            .expect("expected code text after diff marker");
        assert!(
            modifiers.contains(Modifier::DIM),
            "expected deleted preview code to be dimmed"
        );
    }

    #[test]
    fn subtitle_uses_tilde_path_when_codex_home_under_home_directory() {
        let home = dirs::home_dir().expect("home directory should be available");
        let codex_home = home.join(".codex");

        let subtitle = theme_picker_subtitle(Some(&codex_home), Some(200));

        assert!(subtitle.contains("~"));
        assert!(subtitle.contains("directory"));
    }

    #[test]
    fn subtitle_falls_back_when_tilde_path_subtitle_is_too_wide() {
        let home = dirs::home_dir().expect("home directory should be available");
        let long_segment = "a".repeat(120);
        let codex_home = home.join(long_segment).join(".codex");

        let subtitle = theme_picker_subtitle(Some(&codex_home), Some(140));

        assert_eq!(subtitle, PREVIEW_FALLBACK_SUBTITLE);
    }

    #[test]
    fn subtitle_falls_back_to_preview_instructions_without_tilde_path() {
        let subtitle = theme_picker_subtitle(None, None);
        assert_eq!(subtitle, PREVIEW_FALLBACK_SUBTITLE);
    }

    #[test]
    fn subtitle_falls_back_for_94_column_terminal_side_by_side_layout() {
        let home = dirs::home_dir().expect("home directory should be available");
        let codex_home = home.join(".codex");

        let subtitle = theme_picker_subtitle(Some(&codex_home), Some(94));

        assert_eq!(subtitle, PREVIEW_FALLBACK_SUBTITLE);
    }

    #[test]
    fn unavailable_configured_theme_falls_back_to_configured_or_default_selection() {
        let configured_or_default_theme = highlight::configured_theme_name();
        let params = build_theme_picker_params(Some("not-a-real-theme"), None, Some(120));
        let selected_idx = params
            .initial_selected_idx
            .expect("expected selected index for active fallback theme");
        let selected_name = params.items[selected_idx]
            .search_value
            .as_deref()
            .expect("expected search value to contain canonical theme name");

        assert_eq!(selected_name, configured_or_default_theme);
    }
}
