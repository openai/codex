//! Shared selection gestures and source-text boundaries; each view owns its hit testing and state.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use std::ops::Range;
use std::time::Duration;
use std::time::Instant;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectionUnit {
    Character,
    Word,
    Line,
}

impl SelectionUnit {
    pub(crate) fn from_clicks(clicks: u8) -> Self {
        match clicks {
            2 => Self::Word,
            3 => Self::Line,
            _ => Self::Character,
        }
    }

    pub(crate) fn range(self, text: &str, offset: usize) -> Range<usize> {
        let offset = text.floor_char_boundary(offset.min(text.len()));
        match self {
            Self::Character => offset..offset,
            Self::Word => text
                .split_word_bound_indices()
                .map(|(start, word)| start..start + word.len())
                .find(|range| range.contains(&offset))
                .unwrap_or(text.len()..text.len()),
            // Logical lines include their hard newline, regardless of visual wrapping.
            Self::Line => {
                let start = text[..offset]
                    .rfind('\n')
                    .map_or(/*default*/ 0, |newline| newline + 1);
                let end = text[offset..]
                    .find('\n')
                    .map_or(text.len(), |newline| offset + newline + 1);
                start..end
            }
        }
    }
}

pub(crate) fn click_count(
    last_click: &mut Option<(Instant, u16, u16, u8)>,
    column: u16,
    row: u16,
) -> u8 {
    let now = Instant::now();
    let clicks = last_click
        .filter(|(at, x, y, _)| {
            now.duration_since(*at) < Duration::from_millis(/*millis*/ 400)
                && *x == column
                && *y == row
        })
        .map_or(/*default*/ 1, |(_, _, _, clicks)| clicks % 3 + 1);
    *last_click = Some((now, column, row, clicks));
    clicks
}

pub(crate) fn is_copy_key(key: KeyEvent) -> bool {
    // Kitty reports Cmd+C as Super+C, including over SSH. Crossterm may report
    // Ctrl+Shift+C as uppercase C with only Control set.
    let (code, modifiers) = crate::key_hint::normalize_key_parts(key.code, key.modifiers);
    key.kind != KeyEventKind::Release
        && code == KeyCode::Char('c')
        && (matches!(modifiers, KeyModifiers::CONTROL | KeyModifiers::SUPER)
            || modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT))
}
