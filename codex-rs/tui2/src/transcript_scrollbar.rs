//! Transcript scrollbar rendering for `codex-tui2`.
//!
//! `codex-tui2` renders the transcript as a flattened list of wrapped visual lines and tracks the
//! viewport as a top-row offset (`transcript_view_top`) into that flattened list.
//!
//! This module renders a scrollbar for that viewport using the `tui-scrollbar` crate:
//!
//! - `content_len`: total flattened transcript lines
//! - `viewport_len`: number of visible transcript rows
//! - `offset`: top-row offset (0-based)
//!
//! The scrollbar is only shown while the transcript is *not* pinned to the bottom of the viewport
//! (`offset < max_offset`). This keeps the UI clean during normal streaming (where the view
//! follows the latest output) and provides an affordance only when the user is actively browsing
//! scrollback.
//!
//! Arrow endcaps are disabled because the transcript already supports wheel / key scrolling and we
//! do not currently treat the scrollbar as an interactive control.
//!
//! ## `ratatui` vs `ratatui-core`
//!
//! `codex-tui2` uses the `ratatui` crate, while `tui-scrollbar` is built on `ratatui-core`.
//! Because the buffer and style types are distinct, we render the scrollbar into a
//! `ratatui-core` scratch buffer and then copy the resulting glyphs into the main `ratatui`
//! buffer.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui_core::buffer::Buffer as CoreBuffer;
use ratatui_core::layout::Rect as CoreRect;
use ratatui_core::widgets::Widget as _;
use tui_scrollbar::GlyphSet;
use tui_scrollbar::ScrollBar;
use tui_scrollbar::ScrollBarArrows;
use tui_scrollbar::ScrollLengths;

pub(crate) fn render_transcript_scrollbar_if_active(
    buf: &mut Buffer,
    transcript_area: Rect,
    total_lines: usize,
    viewport_lines: usize,
    top_offset: usize,
) {
    if transcript_area.width == 0 || transcript_area.height == 0 {
        return;
    }

    if total_lines <= viewport_lines {
        return;
    }

    let max_offset = total_lines.saturating_sub(viewport_lines);
    if top_offset >= max_offset {
        return;
    }

    let bar_area = Rect {
        x: transcript_area.right().saturating_sub(1),
        y: transcript_area.y,
        width: 1,
        height: transcript_area.height,
    };

    let lengths = ScrollLengths {
        content_len: total_lines,
        viewport_len: viewport_lines,
    };

    let scrollbar = ScrollBar::vertical(lengths).offset(top_offset);

    let core_bar_area = CoreRect {
        x: bar_area.x,
        y: bar_area.y,
        width: bar_area.width,
        height: bar_area.height,
    };
    let mut scratch = CoreBuffer::empty(core_bar_area);
    (&scrollbar).render(core_bar_area, &mut scratch);

    for row in 0..bar_area.height {
        let x = bar_area.x;
        let y = bar_area.y + row;
        let symbol = scratch[(x, y)].symbol();
        if symbol == " " {
            continue;
        }

        let modifier = if symbol == "│" {
            Modifier::DIM
        } else {
            Modifier::BOLD
        };

        let cell = &mut buf[(x, y)];
        cell.set_symbol(symbol);
        cell.set_style(cell.style().add_modifier(modifier));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn bar_column(buf: &Buffer, area: Rect) -> String {
        let x = area.right().saturating_sub(1);
        (0..area.height)
            .map(|row| {
                buf[(x, area.y + row)]
                    .symbol()
                    .chars()
                    .next()
                    .unwrap_or(' ')
            })
            .collect()
    }

    #[test]
    fn does_not_render_when_pinned_to_bottom() {
        let area = Rect::new(0, 0, 10, 6);
        let mut buf = Buffer::empty(area);

        render_transcript_scrollbar_if_active(&mut buf, area, 100, 6, 94);

        assert_eq!(bar_column(&buf, area), "      ");
    }

    #[test]
    fn renders_when_scrolled_away_from_bottom() {
        let area = Rect::new(0, 0, 10, 6);
        let mut buf = Buffer::empty(area);

        render_transcript_scrollbar_if_active(&mut buf, area, 100, 6, 80);

        assert_ne!(bar_column(&buf, area), "      ");
    }
}
