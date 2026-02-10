use std::path::Path;

use crate::app_event::AppEvent;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::SideContentWidth;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::render::highlight;
use crate::render::renderable::Renderable;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Widget;

/// Rust snippet for the theme preview — compact enough to fit in the picker,
/// varied enough to exercise keywords, types, strings, and macros.
const PREVIEW_CODE: &str = "\
fn greet(name: &str) -> String {
    let msg = format!(\"Hello, {name}!\");
    println!(\"{msg}\");
    msg
}

/// Count words in the given text.
fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}";

/// Compact fallback preview used in stacked (narrow) mode.
const NARROW_PREVIEW_LINES: usize = 3;

/// Minimum side-panel width for side-by-side theme preview.
const WIDE_PREVIEW_MIN_WIDTH: u16 = 44;

/// Left inset used for wide preview content.
const WIDE_PREVIEW_LEFT_INSET: u16 = 2;

/// Minimum frame padding used for vertically centered wide preview.
const PREVIEW_FRAME_PADDING: u16 = 1;

/// Renders a syntax-highlighted code snippet below the theme list so users can
/// preview what each theme looks like on real code.
struct ThemePreviewWideRenderable;
struct ThemePreviewNarrowRenderable;

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
    max_lines: usize,
    center_vertically: bool,
    left_inset: u16,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let syntax_lines = highlight::highlight_code_to_styled_spans(PREVIEW_CODE, "rust");

    let preview_lines: Vec<(usize, &str)> = PREVIEW_CODE
        .lines()
        .enumerate()
        .take(max_lines)
        .map(|(idx, line)| (idx + 1, line))
        .collect();
    if preview_lines.is_empty() {
        return;
    }
    let max_line_no = preview_lines
        .last()
        .map(|(line_no, _)| *line_no)
        .unwrap_or(1);
    let ln_width = max_line_no.to_string().len();

    let content_height = (preview_lines.len() as u16).min(area.height);

    let left_pad = left_inset.min(area.width.saturating_sub(1));
    let top_pad = if center_vertically {
        centered_offset(area.height, content_height, PREVIEW_FRAME_PADDING)
    } else {
        0
    };

    let mut y = area.y.saturating_add(top_pad);
    let render_width = area.width.saturating_sub(left_pad);
    for (line_idx, raw_line) in preview_lines {
        if y >= area.y + area.height {
            break;
        }
        let gutter = format!("{line_idx:>ln_width$} ");
        let mut spans: Vec<Span<'static>> = vec![Span::from(gutter).dim()];
        if let Some(syn) = syntax_lines.as_ref().and_then(|sl| sl.get(line_idx - 1)) {
            spans.extend(syn.iter().cloned());
        } else {
            spans.push(Span::raw(raw_line.to_string()));
        }
        Line::from(spans).render(
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
        render_preview(area, buf, usize::MAX, true, WIDE_PREVIEW_LEFT_INSET);
    }
}

impl Renderable for ThemePreviewNarrowRenderable {
    fn desired_height(&self, _width: u16) -> u16 {
        NARROW_PREVIEW_LINES as u16
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        render_preview(area, buf, NARROW_PREVIEW_LINES, false, 0);
    }
}

/// Builds [`SelectionViewParams`] for the `/theme` picker dialog.
///
/// Lists all bundled themes plus custom `.tmTheme` files, with live preview
/// on cursor movement and cancel-restore.
pub(crate) fn build_theme_picker_params(
    current_name: Option<&str>,
    codex_home: Option<&Path>,
) -> SelectionViewParams {
    // Snapshot the current theme so we can restore on cancel.
    let original_theme = highlight::current_syntax_theme();

    let entries = highlight::list_available_themes(codex_home);
    let codex_home_owned = codex_home.map(|p| p.to_path_buf());

    // Resolve the effective theme name: honor explicit config, fall back to
    // the auto-detected default so the picker pre-selects even when no theme
    // is configured.
    let effective_name = current_name
        .map(str::to_string)
        .unwrap_or_else(highlight::current_theme_name);

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

    // Capture theme names from the stable entry list built above so the
    // preview closure indexes into the same ordering the picker uses.
    let preview_names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
    let preview_home = codex_home_owned.clone();
    let on_selection_changed = Some(Box::new(move |idx: usize, _tx: &_| {
        if let Some(name) = preview_names.get(idx) {
            if let Some(theme) = highlight::resolve_theme_by_name(name, preview_home.as_deref()) {
                highlight::set_syntax_theme(theme);
            }
        }
    })
        as Box<dyn Fn(usize, &crate::app_event_sender::AppEventSender) + Send + Sync>);

    // Restore original theme on cancel.
    let on_cancel = Some(Box::new(move |_tx: &_| {
        highlight::set_syntax_theme(original_theme.clone());
    })
        as Box<dyn Fn(&crate::app_event_sender::AppEventSender) + Send + Sync>);
    let themes_dir_display = codex_home_owned
        .as_ref()
        .map(|home| home.join("themes"))
        .unwrap_or_else(|| Path::new("$CODEX_HOME").join("themes"))
        .display()
        .to_string();

    SelectionViewParams {
        title: Some("Select Syntax Theme".to_string()),
        subtitle: Some(format!(
            "Custom .tmTheme files can be added to the {themes_dir_display} directory."
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

    fn render_lines(renderable: &dyn Renderable, width: u16, height: u16) -> Vec<String> {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        renderable.render(area, &mut buf);
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

    fn preview_line_number(line: &str) -> Option<usize> {
        let trimmed = line.trim_start();
        let digits_len = trimmed.chars().take_while(|ch| ch.is_ascii_digit()).count();
        if digits_len == 0 {
            return None;
        }
        let digits = &trimmed[..digits_len];
        if !trimmed[digits_len..].starts_with(' ') {
            return None;
        }
        digits.parse::<usize>().ok()
    }

    #[test]
    fn theme_picker_uses_half_width_with_stacked_fallback_preview() {
        let params = build_theme_picker_params(None, None);
        assert_eq!(params.side_content_width, SideContentWidth::Half);
        assert_eq!(params.side_content_min_width, WIDE_PREVIEW_MIN_WIDTH);
        assert!(params.stacked_side_content.is_some());
    }

    #[test]
    fn wide_preview_renders_all_lines_with_vertical_center_and_left_inset() {
        let lines = render_lines(&ThemePreviewWideRenderable, 80, 20);
        let numbered_rows: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter_map(|(idx, line)| preview_line_number(line).map(|_| idx))
            .collect();
        let total_preview_lines = PREVIEW_CODE.lines().count();

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
            first_line.starts_with("   1 fn greet"),
            "expected wide preview to start after a 2-char inset"
        );
    }

    #[test]
    fn narrow_preview_renders_three_lines() {
        let lines = render_lines(&ThemePreviewNarrowRenderable, 80, 6);
        let numbered_lines: Vec<usize> = lines
            .iter()
            .filter_map(|line| preview_line_number(line))
            .collect();

        assert_eq!(numbered_lines, vec![1, 2, 3]);
        let first_numbered = lines
            .iter()
            .find(|line| preview_line_number(line).is_some())
            .expect("expected at least one rendered preview row");
        assert!(
            first_numbered.starts_with("1 fn greet"),
            "expected narrow preview line numbers to start at the left edge"
        );
    }
}
