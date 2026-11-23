//! Builds preview shortcuts and reserves only the footer rows that are actually used.

use crate::footer_hint::shortcut;
use crate::key_hint::KeyBinding;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::KeymapContext;
use crate::keymap::RuntimeKeymap;
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

pub(super) fn reader_hints(keymap: &RuntimeKeymap, close_label: &str) -> Vec<Line<'static>> {
    [
        Some(shortcut(
            &crate::key_hint::plain(KeyCode::Esc).display_label(),
            close_label,
        )),
        Some(shortcut("↑/↓", "read")),
        Some(shortcut(
            &keymap
                .pager
                .find
                .first()
                .map_or_else(|| "F3".to_owned(), KeyBinding::display_label),
            "find",
        )),
        Some(shortcut("Tab", "link")),
        keymap
            .primary_hint(KeymapContext::Global, "select_transcript")
            .map(|hint| shortcut(&hint.display_label(), "select")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub(super) fn available_path_keys(keymap: &RuntimeKeymap) -> [bool; 2] {
    let pager = &keymap.pager;
    ['c', 'p'].map(|ch| {
        let key = KeyCode::Char(ch).into();
        ![
            &pager.jump_top,
            &pager.jump_bottom,
            &pager.scroll_up,
            &pager.scroll_down,
            &pager.page_up,
            &pager.page_down,
            &pager.half_page_up,
            &pager.half_page_down,
            &pager.find,
            &keymap.app.find_transcript,
            &keymap.app.select_transcript,
        ]
        .iter()
        .any(|bindings| bindings.is_pressed(key))
    })
}

pub(super) fn paint(body: &mut Rect, lines: Vec<Line<'_>>, buf: &mut Buffer) {
    let height = (lines.len() as u16).min(body.height);
    body.height -= height;
    Paragraph::new(lines).render(Rect::new(body.x, body.bottom(), body.width, height), buf);
}
