use std::path::Path;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Widget;

use crate::app_event::AppEvent;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::render::highlight;
use crate::render::renderable::Renderable;

/// Rust snippet for the theme preview — compact enough to fit in the picker,
/// varied enough to exercise keywords, types, strings, and macros.
const PREVIEW_CODE: &str = "\
fn greet(name: &str) -> String {
    let msg = format!(\"Hello, {name}!\");
    println!(\"{msg}\");
    msg
}";

/// Fixed height: 5 code lines.
const PREVIEW_HEIGHT: u16 = 5;

/// Renders a syntax-highlighted code snippet below the theme list so users can
/// preview what each theme looks like on real code.
struct ThemePreviewRenderable;

impl Renderable for ThemePreviewRenderable {
    fn desired_height(&self, _width: u16) -> u16 {
        PREVIEW_HEIGHT
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let mut y = area.y;

        // Syntax-highlight the full snippet as one block.
        let syntax_lines = highlight::highlight_code_to_styled_spans(PREVIEW_CODE, "rust");
        let line_count = PREVIEW_CODE.lines().count();
        let ln_width = if line_count == 0 { 1 } else { line_count.to_string().len() };

        // Render each line with a dim gutter (line number).
        for (i, raw_line) in PREVIEW_CODE.lines().enumerate() {
            if y >= area.y + area.height {
                break;
            }
            let gutter = format!("{:>ln_width$} ", i + 1);
            let mut spans: Vec<Span<'static>> = vec![Span::from(gutter).dim()];
            if let Some(syn) = syntax_lines.as_ref().and_then(|sl| sl.get(i)) {
                spans.extend(syn.iter().cloned());
            } else {
                spans.push(Span::raw(raw_line.to_string()));
            }
            Line::from(spans).render(Rect::new(area.x, y, area.width, 1), buf);
            y += 1;
        }

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

    // Live preview: resolve and apply theme on each cursor movement,
    // and update the preview label name.
    let preview_home = codex_home_owned.clone();
    let on_selection_changed = Some(Box::new(move |idx: usize, _tx: &_| {
        let all = highlight::list_available_themes(preview_home.as_deref());
        if let Some(entry) = all.get(idx) {
            if let Some(theme) =
                highlight::resolve_theme_by_name(&entry.name, preview_home.as_deref())
            {
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

    SelectionViewParams {
        title: Some("Select Syntax Theme".to_string()),
        subtitle: Some("Arrow keys to preview, Enter to select, Esc to cancel.".to_string()),
        items,
        is_searchable: true,
        search_placeholder: Some("Type to filter themes...".to_string()),
        initial_selected_idx: initial_idx,
        footer_content: Box::new(ThemePreviewRenderable),
        on_selection_changed,
        on_cancel,
        ..Default::default()
    }
}
