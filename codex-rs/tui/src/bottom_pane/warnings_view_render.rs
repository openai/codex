//! Show one complete diagnostic with bounded scrolling and the viewer's own footer.

use super::*;
use crate::bottom_pane::footer_hint_items_line;
use crate::render::renderable::Renderable;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Margin;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;

impl Renderable for WarningsView {
    fn desired_height(&self, _width: u16) -> u16 {
        12
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let area = area.inner(Margin::new(/*horizontal*/ 2, /*vertical*/ 0));
        let [header, _, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);
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
        if body.width > 0
            && body.height > 0
            && let Some(visited) = self.visited.get(self.current)
        {
            visited.set(/*val*/ true);
        }
        let lines: Vec<_> = details
            .lines()
            .flat_map(|line| textwrap::wrap(line, usize::from(body.width.max(/*other*/ 1))))
            .collect();
        let page_size = usize::from(body.height.max(/*other*/ 1));
        self.page_size.set(page_size);
        self.max_offset.set(lines.len().saturating_sub(page_size));
        let offset = self.offset.get().min(self.max_offset.get());
        self.offset.set(offset);
        for (row, line) in lines
            .iter()
            .skip(offset)
            .take(usize::from(body.height))
            .enumerate()
        {
            Line::from(line.as_ref()).render(
                Rect::new(body.x, body.y + row as u16, body.width, /*height*/ 1),
                buf,
            );
        }
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
        let navigation = ["move_left", "move_right"]
            .into_iter()
            .filter_map(|action| hint(KeymapContext::List, action))
            .map(crate::key_hint::ShortcutHint::display_label)
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
                hint(KeymapContext::List, "move_down")
                    .map(crate::key_hint::ShortcutHint::display_label),
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
