//! Builds preview shortcuts and reserves only the footer rows that are actually used.

use crate::footer_hint::Hint;
use crate::key_hint::KeyBinding;
use crate::key_hint::KeyBindingListExt;
use crate::key_hint::ShortcutHint;
use crate::keymap::KeymapContext;
use crate::keymap::RuntimeKeymap;
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

/// Watch-only readers handle pager keys directly without dispatching pager chords.
pub(in crate::app) enum PagerHints {
    SingleKeys,
    WithChords,
}

pub(in crate::app) fn reading_hints(
    keymap: &RuntimeKeymap,
    pager_hints: PagerHints,
    link: Option<Hint>,
) -> impl Iterator<Item = Hint> {
    let pager = &keymap.pager;
    let hint = |action, bindings: &[KeyBinding]| match pager_hints {
        PagerHints::SingleKeys => {
            crate::keymap::primary_binding(bindings).map(ShortcutHint::Single)
        }
        PagerHints::WithChords => pager.primary_hint(action, bindings),
    };
    let movement = [
        hint("scroll_up", &pager.scroll_up),
        hint("scroll_down", &pager.scroll_down),
    ]
    .into_iter()
    .flatten()
    .map(crate::key_hint::ShortcutHint::display_label)
    .collect::<Vec<_>>()
    .join("/");
    [
        (!movement.is_empty()).then(|| Hint::key(movement, "read")),
        hint("find", &pager.find)
            .or_else(|| keymap.primary_hint(KeymapContext::Global, "find_transcript"))
            .map(|hint| Hint::key(hint.display_label(), "find")),
        link,
        keymap
            .primary_hint(KeymapContext::Global, "select_transcript")
            .map(|hint| Hint::key(hint.display_label(), "select")),
    ]
    .into_iter()
    .flatten()
}

pub(super) fn key_available(keymap: &RuntimeKeymap) -> impl Fn(KeyCode) -> bool + '_ {
    let pager = &keymap.pager;
    |code| {
        let key = code.into();
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
    }
}

pub(super) fn paint(body: &mut Rect, lines: Vec<Line<'_>>, buf: &mut Buffer) {
    let height = (lines.len() as u16).min(body.height);
    body.height -= height;
    Paragraph::new(lines).render(Rect::new(body.x, body.bottom(), body.width, height), buf);
}
