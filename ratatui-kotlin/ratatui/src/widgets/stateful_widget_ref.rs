use ratatui_core::widgets::StatefulWidget;

use crate::buffer::Buffer;
use crate::layout::Rect;

/// A `StatefulWidgetRef` is a trait that allows rendering a stateful widget by reference.
///
/// This is the stateful equivalent of `WidgetRef`. It is useful when you need to store a reference
/// to a stateful widget and render it later. It also allows you to render boxed stateful widgets.
///
/// This trait was introduced in Ratatui 0.26.0. It is currently marked as unstable as we are still
/// evaluating the API and may make changes in the future. See
/// <https://github.com/ratatui/ratatui/issues/1287> for more information.
///
/// A blanket implementation of `StatefulWidgetRef` for `&W` where `W` implements `StatefulWidget`
/// is provided. Most of the time you will want to implement `StatefulWidget` against a reference to
/// the widget instead of implementing `StatefulWidgetRef` directly.
///
/// See the documentation for [`WidgetRef`] for more information on boxed widgets. See the
/// documentation for [`StatefulWidget`] for more information on stateful widgets.
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
/// use ratatui::widgets::StatefulWidgetRef;
/// use ratatui_core::buffer::Buffer;
/// use ratatui_core::layout::Rect;
/// use ratatui_core::style::Stylize;
/// use ratatui_core::text::Line;
/// use ratatui_core::widgets::{StatefulWidget, Widget};
///
/// struct PersonalGreeting;
///
/// impl StatefulWidgetRef for PersonalGreeting {
///     type State = String;
///     fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
///         Line::raw(format!("Hello {}", state)).render(area, buf);
///     }
/// }
///
/// impl StatefulWidget for PersonalGreeting {
///     type State = String;
///     fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
///         (&self).render_ref(area, buf, state);
///     }
/// }
///
/// fn render(area: Rect, buf: &mut Buffer) {
///     let widget = PersonalGreeting;
///     let mut state = "world".to_string();
///     widget.render(area, buf, &mut state);
/// }
/// # }
/// ```
#[instability::unstable(feature = "widget-ref")]
pub trait StatefulWidgetRef {
    /// State associated with the stateful widget.
    ///
    /// If you don't need this then you probably want to implement [`WidgetRef`] instead.
    ///
    /// [`WidgetRef`]: super::WidgetRef
    type State: ?Sized;
    /// Draws the current state of the widget in the given buffer. That is the only method required
    /// to implement a custom stateful widget.
    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State);
}

/// Blanket implementation of `StatefulWidgetRef` for `&W` where `W` implements `StatefulWidget`.
///
/// This allows you to render a stateful widget by reference.
impl<W, State: ?Sized> StatefulWidgetRef for &W
where
    for<'a> &'a W: StatefulWidget<State = State>,
{
    type State = State;
    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        self.render(area, buf, state);
    }
}

#[cfg(test)]
mod tests {
    use alloc::borrow::ToOwned;
    use alloc::boxed::Box;
    use alloc::format;
    use alloc::string::{String, ToString};

    use rstest::{fixture, rstest};

    use super::*;
    use crate::buffer::Buffer;
    use crate::layout::Rect;
    use crate::text::Line;
    use crate::widgets::Widget;

    #[fixture]
    fn buf() -> Buffer {
        Buffer::empty(Rect::new(0, 0, 20, 1))
    }

    #[fixture]
    fn state() -> String {
        "world".to_string()
    }

    struct PersonalGreeting;

    impl StatefulWidget for &PersonalGreeting {
        type State = String;
        fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
            Line::from(format!("Hello {state}")).render(area, buf);
        }
    }

    #[rstest]
    fn render_ref(mut buf: Buffer, mut state: String) {
        let widget = &PersonalGreeting;
        widget.render_ref(buf.area, &mut buf, &mut state);
        assert_eq!(buf, Buffer::with_lines(["Hello world         "]));
    }

    #[rstest]
    fn box_render_ref(mut buf: Buffer, mut state: String) {
        let widget = Box::new(&PersonalGreeting);
        widget.render_ref(buf.area, &mut buf, &mut state);
        assert_eq!(buf, Buffer::with_lines(["Hello world         "]));
    }

    #[rstest]
    fn render_stateful_widget_ref_with_unsized_state(mut buf: Buffer) {
        struct Bytes;

        impl StatefulWidgetRef for Bytes {
            type State = [u8];
            fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
                let slice = core::str::from_utf8(state).unwrap();
                Line::from(format!("Bytes: {slice}")).render(area, buf);
            }
        }
        let widget = Bytes;
        let state = b"hello";
        widget.render_ref(buf.area, &mut buf, &mut state.clone());
        assert_eq!(buf, Buffer::with_lines(["Bytes: hello        "]));
    }

    #[rstest]
    fn render_stateful_widget_with_unsized_state(mut buf: Buffer) {
        struct Bytes;
        impl StatefulWidget for &Bytes {
            type State = [u8];
            fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
                let slice = core::str::from_utf8(state).unwrap();
                Line::from(format!("Bytes: {slice}")).render(area, buf);
            }
        }
        let widget = &Bytes;
        let mut state = b"hello".to_owned();
        let state = state.as_mut_slice();
        widget.render_ref(buf.area, &mut buf, state);
        assert_eq!(buf, Buffer::with_lines(["Bytes: hello        "]));
    }
}
