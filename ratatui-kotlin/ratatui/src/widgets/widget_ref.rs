use alloc::string::String;

use super::Widget;
use crate::buffer::Buffer;
use crate::layout::Rect;
use crate::style::Style;

/// A `WidgetRef` is a trait that allows rendering a widget by reference.
///
/// This trait is useful when you want to store a reference to a widget and render it later. It also
/// allows you to render boxed widgets.
///
/// Boxed widgets allow you to store widgets with a type that is not known at compile time. This is
/// useful when you want to store a collection of widgets with different types. You can then iterate
/// over the collection and render each widget.
///
/// This trait was introduced in Ratatui 0.26.0 and is implemented for all the internal widgets. It
/// is currently marked as unstable as we are still evaluating the API and may make changes in the
/// future. See <https://github.com/ratatui/ratatui/issues/1287> for more information.
///
/// A blanket implementation of `Widget` for `&W` where `W` implements `WidgetRef` is provided.
///
/// A blanket implementation of `WidgetRef` for `Option<W>` where `W` implements `WidgetRef` is
/// provided. This is a convenience approach to make it easier to attach child widgets to parent
/// widgets. It allows you to render an optional widget by reference.
///
/// For comprehensive information about widget implementation patterns, rendering, and usage,
/// see the [`widgets`] module documentation.
///
/// [`widgets`]: crate::widgets
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "unstable-widget-ref")] {
/// use ratatui::widgets::WidgetRef;
/// use ratatui_core::buffer::Buffer;
/// use ratatui_core::layout::Rect;
/// use ratatui_core::text::Line;
/// use ratatui_core::widgets::Widget;
///
/// struct Greeting;
///
/// struct Farewell;
///
/// impl WidgetRef for Greeting {
///     fn render_ref(&self, area: Rect, buf: &mut Buffer) {
///         Line::raw("Hello").render(area, buf);
///     }
/// }
///
/// /// Only needed for backwards compatibility
/// impl Widget for Greeting {
///     fn render(self, area: Rect, buf: &mut Buffer) {
///         self.render_ref(area, buf);
///     }
/// }
///
/// impl WidgetRef for Farewell {
///     fn render_ref(&self, area: Rect, buf: &mut Buffer) {
///         Line::raw("Goodbye").right_aligned().render(area, buf);
///     }
/// }
///
/// /// Only needed for backwards compatibility
/// impl Widget for Farewell {
///     fn render(self, area: Rect, buf: &mut Buffer) {
///         self.render_ref(area, buf);
///     }
/// }
///
/// # fn render(area: Rect, buf: &mut Buffer) {
/// let greeting = Greeting;
/// let farewell = Farewell;
///
/// // these calls do not consume the widgets, so they can be used again later
/// greeting.render_ref(area, buf);
/// farewell.render_ref(area, buf);
///
/// // a collection of widgets with different types
/// let widgets: Vec<Box<dyn WidgetRef>> = vec![Box::new(greeting), Box::new(farewell)];
/// for widget in widgets {
///     widget.render_ref(area, buf);
/// }
/// # }
/// # }
/// ```
#[instability::unstable(feature = "widget-ref")]
pub trait WidgetRef {
    /// Draws the current state of the widget in the given buffer. That is the only method required
    /// to implement a custom widget.
    fn render_ref(&self, area: Rect, buf: &mut Buffer);
}

/// This allows you to render a widget by reference.
impl<W> WidgetRef for &W
where
    for<'a> &'a W: Widget,
{
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        self.render(area, buf);
    }
}

/// Provides the ability to render a string slice by reference.
///
/// This trait implementation ensures that a string slice, which is an immutable view over a
/// `String`, can be drawn on demand without requiring ownership of the string itself. It utilizes
/// the default text style when rendering onto the provided [`Buffer`] at the position defined by
/// [`Rect`].
impl WidgetRef for &str {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        buf.set_stringn(area.x, area.y, self, area.width as usize, Style::new());
    }
}

/// Provides the ability to render a `String` by reference.
///
/// This trait allows for a `String` to be rendered onto the [`Buffer`], similarly using the default
/// style settings. It ensures that an owned `String` can be rendered efficiently by reference,
/// without the need to give up ownership of the underlying text.
impl WidgetRef for String {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        buf.set_stringn(area.x, area.y, self, area.width as usize, Style::new());
    }
}

/// A blanket implementation of `WidgetExt` for `Option<W>` where `W` implements `WidgetRef`.
///
/// This is a convenience implementation that makes it easy to attach child widgets to parent
/// widgets. It allows you to render an optional widget by reference.
///
/// The internal widgets use this pattern to render the optional `Block` widgets that are included
/// on most widgets.
/// Blanket implementation of `WidgetExt` for `Option<W>` where `W` implements `WidgetRef`.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "unstable-widget-ref")] {
/// use ratatui::widgets::WidgetRef;
/// use ratatui_core::buffer::Buffer;
/// use ratatui_core::layout::Rect;
/// use ratatui_core::text::Line;
/// use ratatui_core::widgets::Widget;
///
/// struct Parent {
///     child: Option<Child>,
/// }
///
/// struct Child;
///
/// impl WidgetRef for Child {
///     fn render_ref(&self, area: Rect, buf: &mut Buffer) {
///         Line::raw("Hello from child").render(area, buf);
///     }
/// }
///
/// impl WidgetRef for Parent {
///     fn render_ref(&self, area: Rect, buf: &mut Buffer) {
///         self.child.render_ref(area, buf);
///     }
/// }
/// # }
/// ```
impl<W: WidgetRef> WidgetRef for Option<W> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        if let Some(widget) = self {
            widget.render_ref(area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::vec;
    use alloc::vec::Vec;

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

    struct Farewell;

    impl Widget for &Greeting {
        fn render(self, area: Rect, buf: &mut Buffer) {
            Line::from("Hello").render(area, buf);
        }
    }

    impl Widget for &Farewell {
        fn render(self, area: Rect, buf: &mut Buffer) {
            Line::from("Goodbye").right_aligned().render(area, buf);
        }
    }

    /// Ensure that the blanket implementation of `WidgetRef` for `&W` where `W` implements
    /// `Widget` works as expected.
    #[rstest]
    fn render_ref(mut buf: Buffer) {
        let widget = &Greeting;
        widget.render_ref(buf.area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["Hello               "]));
    }

    #[rstest]
    fn render_ref_box(mut buf: Buffer) {
        let widget: Box<dyn WidgetRef> = Box::new(&Greeting);
        widget.render_ref(buf.area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["Hello               "]));
    }

    #[rstest]
    fn render_ref_box_vec(mut buf: Buffer) {
        let widgets: Vec<Box<dyn WidgetRef>> = vec![Box::new(&Greeting), Box::new(&Farewell)];
        for widget in widgets {
            widget.render_ref(buf.area, &mut buf);
        }
        assert_eq!(buf, Buffer::with_lines(["Hello        Goodbye"]));
    }

    #[rstest]
    fn render_ref_some(mut buf: Buffer) {
        let widget = Some(&Greeting);
        widget.render_ref(buf.area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["Hello               "]));
    }

    #[rstest]
    fn render_ref_none(mut buf: Buffer) {
        let widget: Option<&Greeting> = None;
        widget.render_ref(buf.area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["                    "]));
    }

    #[rstest]
    fn render_ref_str(mut buf: Buffer) {
        "hello world".render_ref(buf.area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["hello world         "]));
    }

    #[rstest]
    fn render_ref_option_str(mut buf: Buffer) {
        Some("hello world").render_ref(buf.area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["hello world         "]));
    }

    #[rstest]
    fn render_ref_string(mut buf: Buffer) {
        String::from("hello world").render_ref(buf.area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["hello world         "]));
    }

    #[rstest]
    fn render_ref_option_string(mut buf: Buffer) {
        Some(String::from("hello world")).render_ref(buf.area, &mut buf);
        assert_eq!(buf, Buffer::with_lines(["hello world         "]));
    }
}
