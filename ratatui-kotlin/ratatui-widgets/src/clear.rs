//! The [`Clear`] widget allows you to clear a certain area to allow overdrawing (e.g. for popups).
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::widgets::Widget;

/// A widget to clear/reset a certain area to allow overdrawing (e.g. for popups).
///
/// This widget **cannot be used to clear the terminal on the first render** as `ratatui` assumes
/// the render area is empty. Use `Terminal::clear` instead.
///
/// # Examples
///
/// ```
/// use ratatui::Frame;
/// use ratatui::layout::Rect;
/// use ratatui::widgets::{Block, Clear};
///
/// fn draw_on_clear(f: &mut Frame, area: Rect) {
///     let block = Block::bordered().title("Block");
///     f.render_widget(Clear, area); // <- this will clear/reset the area first
///     f.render_widget(block, area); // now render the block widget
/// }
/// ```
///
/// # Popup Example
///
/// For a more complete example how to utilize `Clear` to realize popups see
/// the example `examples/popup.rs`
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Clear;

impl Widget for Clear {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl Widget for &Clear {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for x in area.left()..area.right() {
            for y in area.top()..area.bottom() {
                buf[(x, y)].reset();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    use ratatui_core::widgets::Widget;

    use super::*;

    #[test]
    fn render() {
        let mut buffer = Buffer::with_lines(["xxxxxxxxxxxxxxx"; 7]);
        let clear = Clear;
        clear.render(Rect::new(1, 2, 3, 4), &mut buffer);
        let expected = Buffer::with_lines([
            "xxxxxxxxxxxxxxx",
            "xxxxxxxxxxxxxxx",
            "x   xxxxxxxxxxx",
            "x   xxxxxxxxxxx",
            "x   xxxxxxxxxxx",
            "x   xxxxxxxxxxx",
            "xxxxxxxxxxxxxxx",
        ]);
        assert_eq!(buffer, expected);
    }
}
