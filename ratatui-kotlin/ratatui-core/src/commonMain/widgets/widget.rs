use alloc::string::String;

use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;

/// A `Widget` is a type that can be drawn on a [`Buffer`] in a given [`Rect`].
///
/// For a comprehensive guide to widgets, including trait explanations, implementation patterns,
/// and available widgets, see the [`widgets`] module documentation.
///
/// [`widgets`]: ../../ratatui/widgets/index.html
///
/// Prior to Ratatui 0.26.0, widgets generally were created for each frame as they were consumed
/// during rendering. This meant that they were not meant to be stored but used as *commands* to
/// draw common figures in the UI.
///
/// Starting with Ratatui 0.26.0, all the internal widgets implement Widget for a reference to
/// themselves. This allows you to store a reference to a widget and render it later. Widget crates
/// should consider also doing this to allow for more flexibility in how widgets are used.
///
/// In Ratatui 0.26.0, we also added an unstable `WidgetRef` trait and implemented this on all the
/// internal widgets. In addition to the above benefit of rendering references to widgets, this also
/// allows you to render boxed widgets. This is useful when you want to store a collection of
/// widgets with different types. You can then iterate over the collection and render each widget.
/// See <https://github.com/ratatui/ratatui/issues/1287> for more information.
///
/// In general where you expect a widget to immutably work on its data, we recommended to implement
/// `Widget` for a reference to the widget (`impl Widget for &MyWidget`). If you need to store state
/// between draw calls, implement `StatefulWidget` if you want the Widget to be immutable, or
/// implement `Widget` for a mutable reference to the widget (`impl Widget for &mut MyWidget`) if
/// you want the widget to be mutable. The mutable widget pattern is used infrequently in apps, but
/// can be quite useful.
///
/// A blanket implementation of `Widget` for `&W` where `W` implements `WidgetRef` is provided.
/// Widget is also implemented for `&str` and `String` types.
///
/// # Examples
///
/// ```rust,ignore
/// use ratatui::{
///     backend::TestBackend,
///     widgets::{Clear, Widget},
///     Terminal,
/// };
/// # let backend = TestBackend::new(5, 5);
/// # let mut terminal = Terminal::new(backend).unwrap();
///
/// terminal.draw(|frame| {
///     frame.render_widget(Clear, frame.area());
/// });
/// ```
///
/// It's common to render widgets inside other widgets:
///
/// ```rust
/// use ratatui_core::buffer::Buffer;
/// use ratatui_core::layout::Rect;
/// use ratatui_core::text::Line;
/// use ratatui_core::widgets::Widget;
///
/// struct MyWidget;
///
/// impl Widget for MyWidget {
///     fn render(self, area: Rect, buf: &mut Buffer) {
///         Line::raw("Hello").render(area, buf);
///     }
/// }
/// ```
pub trait Widget {
    /// Draws the current state of the widget in the given buffer. That is the only method required
    /// to implement a custom widget.
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized;
}

/// Renders a string slice as a widget.
///
/// This implementation allows a string slice (`&str`) to act as a widget, meaning it can be drawn
/// onto a [`Buffer`] in a specified [`Rect`]. The slice represents a static string which can be
/// rendered by reference, thereby avoiding the need for string cloning or ownership transfer when
/// drawing the text to the screen.
impl Widget for &str {
    fn render(self, area: Rect, buf: &mut Buffer) {
        buf.set_stringn(area.x, area.y, self, area.width as usize, Style::new());
    }
}

/// Renders a `String` object as a widget.
///
/// This implementation enables an owned `String` to be treated as a widget, which can be rendered
/// on a [`Buffer`] within the bounds of a given [`Rect`].
impl Widget for String {
    fn render(self, area: Rect, buf: &mut Buffer) {
        buf.set_stringn(area.x, area.y, self, area.width as usize, Style::new());
    }
}

impl<W: Widget> Widget for Option<W> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if let Some(widget) = self {
            widget.render(area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};

    use super::*;
    use crate::buffer::Buffer;
    use crate::layout::Rect;
    use crate::text::Line;

    #[fixture]
    fn buf() -> Buffer {
        Buffer::empty(Rect::new(0, 0, 20, 1))
    }

    struct Greeting;

    impl Widget for Greeting {
        fn render(self, area: Rect, buf: &mut Buffer) {
            Line::from("Hello").render(area, buf);
        }
    }

    #[rstest]
    fn render(mut buf: Buffer) {
        let widget = Greeting;
        widget.render(buf.area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["Hello               "]));
    }

    #[rstest]
    fn render_str(mut buf: Buffer) {
        "hello world".render(buf.area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["hello world         "]));
    }

    #[rstest]
    fn render_str_truncate(mut buf: Buffer) {
        let area = Rect::new(buf.area.x, buf.area.y, 11, buf.area.height);
        "hello world, just hello".render(area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["hello world         "]));
    }

    #[rstest]
    fn render_option_str(mut buf: Buffer) {
        Some("hello world").render(buf.area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["hello world         "]));
    }

    #[rstest]
    fn render_string(mut buf: Buffer) {
        String::from("hello world").render(buf.area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["hello world         "]));
    }

    #[rstest]
    fn render_string_truncate(mut buf: Buffer) {
        let area = Rect::new(buf.area.x, buf.area.y, 11, buf.area.height);
        String::from("hello world, just hello").render(area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["hello world         "]));
    }

    #[rstest]
    fn render_option_string(mut buf: Buffer) {
        Some(String::from("hello world")).render(buf.area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["hello world         "]));
    }
}
