use ratatui::prelude::*;

/// Trait implemented by every type that can live inside the conversation
/// history list.  It provides two primitives that the parent scroll-view
/// needs: how *tall* the widget is at a given width and how to render an
/// arbitrary contiguous *window* of that widget.
///
/// The `first_visible_line` argument to [`render_window`] allows partial
/// rendering when the top of the widget is scrolled off-screen.  The caller
/// guarantees that `first_visible_line + area.height as usize` never exceeds
/// the total height previously returned by [`height`].
pub(crate) trait CellWidget {
    /// Total height measured in wrapped terminal lines when drawn with the
    /// given *content* width (no scrollbar column included).
    fn height(&self, width: u16) -> usize;

    /// Render a *window* that starts `first_visible_line` lines below the top
    /// of the widget. The window’s size is given by `area`.
    fn render_window(&self, first_visible_line: usize, area: Rect, buf: &mut Buffer);
}
