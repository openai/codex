//! Show one complete diagnostic with bounded scrolling and the viewer's own footer.

use super::*;
use crate::bottom_pane::footer_hint_items_line;
use crate::bottom_pane::selection_popup_common::menu_surface_padding_height;
use crate::bottom_pane::selection_popup_common::render_menu_surface;
use crate::render::renderable::Renderable;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Margin;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;

const WARNING_PANEL_CONTENT_ROWS: u16 = 10;

impl Renderable for WarningsView {
    fn desired_height(&self, _width: u16) -> u16 {
        WARNING_PANEL_CONTENT_ROWS + menu_surface_padding_height() + 1
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let [panel, footer] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);
        let content = render_menu_surface(panel, buf);
        let [header, _, body] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(u16::from(content.height > 2)),
            Constraint::Min(0),
        ])
        .areas(content);
        let footer = footer.inner(Margin::new(/*horizontal*/ 2, /*vertical*/ 0));
        let entry = self.entries.get(self.current);
        let title = match entry {
            Some(entry) => format!(
                "Warnings · {} of {} · {}",
                self.current + 1,
                self.entries.len(),
                entry.source
            ),
            None => "Warnings".to_string(),
        };
        Line::from(title.bold()).render(header, buf);
        let details = entry.map_or("No warnings", |entry| entry.details.as_str());
        let restart_hint = (self.daemon_restart_hint_at == Some(self.current)).then_some(
            "\nThese warnings may reflect settings from when the background server started. If you’ve already fixed them, use /daemon to restart it.",
        );
        if body.width > 0
            && body.height > 0
            && let Some(visited) = self.visited.get(self.current)
        {
            visited.set(/*val*/ true);
        }
        let lines = details
            .lines()
            .chain(restart_hint.into_iter().flat_map(str::lines))
            .map(|text| {
                crate::terminal_hyperlinks::annotate_web_urls_in_line(text.to_owned().into())
            })
            .collect();
        let layout = crate::transcript_view::TextLayout::new(lines, body.width);
        self.body_area.set(body);
        self.max_offset.set(if body.is_empty() {
            0
        } else {
            layout.row_count().saturating_sub(usize::from(body.height))
        });
        let offset = self.offset.get().min(self.max_offset.get());
        self.offset.set(offset);
        layout.render(body, buf, offset);
        self.body_layout.set(Some((layout, offset)));
        // Plain k belongs to this viewer, including when configured as a chord prefix.
        let hint = |context, action| {
            self.keymap.primary_hint(context, action).filter(|hint| {
                let binding = match hint {
                    crate::key_hint::ShortcutHint::Single(binding)
                    | crate::key_hint::ShortcutHint::Chord {
                        prefix: binding, ..
                    } => binding,
                };
                *binding != key_hint::plain(KeyCode::Char('k'))
            })
        };
        let list = |action| {
            hint(KeymapContext::List, action).map(crate::key_hint::ShortcutHint::display_label)
        };
        let navigation = ["move_left", "move_right"]
            .into_iter()
            .filter_map(list)
            .collect::<Vec<_>>()
            .join("/");
        let mut items = Vec::new();
        for (hint, label) in [
            (Some("k".to_string()), "keep & next"),
            (
                Some(
                    hint(KeymapContext::List, "cancel")
                        .unwrap_or_else(|| key_hint::ctrl(KeyCode::Char('c')).into())
                        .display_label(),
                ),
                "dismiss & close",
            ),
            (
                hint(KeymapContext::Global, "copy")
                    .map(crate::key_hint::ShortcutHint::display_label),
                "copy",
            ),
            ((!navigation.is_empty()).then_some(navigation), "warning"),
            (
                crate::footer_hint::scroll_keys(
                    offset,
                    self.max_offset.get(),
                    list("move_up"),
                    list("move_down"),
                ),
                "scroll",
            ),
        ] {
            if let Some(hint) = hint {
                items.push((hint, label.to_string()));
                if footer_hint_items_line(&items).width() > usize::from(footer.width) {
                    items.pop();
                }
            }
        }
        if let Some((line, expires_at)) = &self.flash
            && std::time::Instant::now() < *expires_at
        {
            line.clone().render(footer, buf);
        } else {
            footer_hint_items_line(self.pending_hint.as_ref().unwrap_or(&items))
                .render(footer, buf);
        }
    }
}
