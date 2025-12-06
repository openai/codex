use crate::buffer::Buffer;
use crate::layout::Rect;

/// A `StatefulWidget` is a widget that can take advantage of some local state to remember things
/// between two draw calls.
///
/// For a comprehensive guide to widgets, including trait explanations, implementation patterns,
/// and available widgets, see the [`widgets`] module documentation.
///
/// [`widgets`]: ../../ratatui/widgets/index.html
///
/// Most widgets can be drawn directly based on the input parameters. However, some features may
/// require some kind of associated state to be implemented.
///
/// For example, the `List` widget can highlight the item currently selected. This can be translated
/// in an offset, which is the number of elements to skip in order to have the selected item within
/// the viewport currently allocated to this widget. The widget can therefore only provide the
/// following behavior: whenever the selected item is out of the viewport scroll to a predefined
/// position (making the selected item the last viewable item or the one in the middle for example).
/// Nonetheless, if the widget has access to the last computed offset then it can implement a
/// natural scrolling experience where the last offset is reused until the selected item is out of
/// the viewport.
///
/// ## Examples
///
/// ```rust,ignore
/// use std::io;
///
/// use ratatui::{
///     backend::TestBackend,
///     widgets::{List, ListItem, ListState, StatefulWidget, Widget},
///     Terminal,
/// };
///
/// // Let's say we have some events to display.
/// struct Events {
///     // `items` is the state managed by your application.
///     items: Vec<String>,
///     // `state` is the state that can be modified by the UI. It stores the index of the selected
///     // item as well as the offset computed during the previous draw call (used to implement
///     // natural scrolling).
///     state: ListState,
/// }
///
/// impl Events {
///     fn new(items: Vec<String>) -> Events {
///         Events {
///             items,
///             state: ListState::default(),
///         }
///     }
///
///     pub fn set_items(&mut self, items: Vec<String>) {
///         self.items = items;
///         // We reset the state as the associated items have changed. This effectively reset
///         // the selection as well as the stored offset.
///         self.state = ListState::default();
///     }
///
///     // Select the next item. This will not be reflected until the widget is drawn in the
///     // `Terminal::draw` callback using `Frame::render_stateful_widget`.
///     pub fn next(&mut self) {
///         let i = match self.state.selected() {
///             Some(i) => {
///                 if i >= self.items.len() - 1 {
///                     0
///                 } else {
///                     i + 1
///                 }
///             }
///             None => 0,
///         };
///         self.state.select(Some(i));
///     }
///
///     // Select the previous item. This will not be reflected until the widget is drawn in the
///     // `Terminal::draw` callback using `Frame::render_stateful_widget`.
///     pub fn previous(&mut self) {
///         let i = match self.state.selected() {
///             Some(i) => {
///                 if i == 0 {
///                     self.items.len() - 1
///                 } else {
///                     i - 1
///                 }
///             }
///             None => 0,
///         };
///         self.state.select(Some(i));
///     }
///
///     // Unselect the currently selected item if any. The implementation of `ListState` makes
///     // sure that the stored offset is also reset.
///     pub fn unselect(&mut self) {
///         self.state.select(None);
///     }
/// }
///
/// # let backend = TestBackend::new(5, 5);
/// # let mut terminal = Terminal::new(backend).unwrap();
///
/// let mut events = Events::new(vec![String::from("Item 1"), String::from("Item 2")]);
///
/// loop {
///     terminal.draw(|f| {
///         // The items managed by the application are transformed to something
///         // that is understood by ratatui.
///         let items: Vec<ListItem> = events
///             .items
///             .iter()
///             .map(|i| ListItem::new(i.as_str()))
///             .collect();
///         // The `List` widget is then built with those items.
///         let list = List::new(items);
///         // Finally the widget is rendered using the associated state. `events.state` is
///         // effectively the only thing that we will "remember" from this draw call.
///         f.render_stateful_widget(list, f.size(), &mut events.state);
///     });
///
///     // In response to some input events or an external http request or whatever:
///     events.next();
/// }
/// ```
pub trait StatefulWidget {
    /// State associated with the stateful widget.
    ///
    /// If you don't need this then you probably want to implement [`Widget`] instead.
    ///
    /// [`Widget`]: super::Widget
    type State: ?Sized;
    /// Draws the current state of the widget in the given buffer. That is the only method required
    /// to implement a custom stateful widget.
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State);
}

#[cfg(test)]
mod tests {
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

    impl StatefulWidget for PersonalGreeting {
        type State = String;
        fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
            Line::from(format!("Hello {state}")).render(area, buf);
        }
    }

    #[rstest]
    fn render(mut buf: Buffer, mut state: String) {
        let widget = PersonalGreeting;
        widget.render(buf.area, &mut buf, &mut state);
        assert_eq!(buf, Buffer::with_lines(["Hello world         "]));
    }

    struct Bytes;

    /// A widget with an unsized state type.
    impl StatefulWidget for Bytes {
        type State = [u8];
        fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
            let slice = core::str::from_utf8(state).unwrap();
            Line::from(format!("Bytes: {slice}")).render(area, buf);
        }
    }

    #[rstest]
    fn render_unsized_state_type(mut buf: Buffer) {
        let widget = Bytes;
        let state = b"hello";
        widget.render(buf.area, &mut buf, &mut state.clone());
        assert_eq!(buf, Buffer::with_lines(["Bytes: hello        "]));
    }
}
