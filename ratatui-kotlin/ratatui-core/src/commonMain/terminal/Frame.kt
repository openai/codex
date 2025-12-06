use crate::buffer::Buffer;
use crate::layout::{Position, Rect};
use crate::widgets::{StatefulWidget, Widget};

/// A consistent view into the terminal state for rendering a single frame.
///
/// This is obtained via the closure argument of [`Terminal::draw`]. It is used to render widgets
/// to the terminal and control the cursor position.
///
/// The changes drawn to the frame are applied only to the current [`Buffer`]. After the closure
/// returns, the current buffer is compared to the previous buffer and only the changes are applied
/// to the terminal. This avoids drawing redundant cells.
///
/// [`Buffer`]: crate::buffer::Buffer
/// [`Terminal::draw`]: crate::terminal::Terminal::draw
#[derive(Debug, Hash)]
pub struct Frame<'a> {
    /// Where should the cursor be after drawing this frame?
    ///
    /// If `None`, the cursor is hidden and its position is controlled by the backend. If `Some((x,
    /// y))`, the cursor is shown and placed at `(x, y)` after the call to `Terminal::draw()`.
    pub(crate) cursor_position: Option<Position>,

    /// The area of the viewport
    pub(crate) viewport_area: Rect,

    /// The buffer that is used to draw the current frame
    pub(crate) buffer: &'a mut Buffer,

    /// The frame count indicating the sequence number of this frame.
    pub(crate) count: usize,
}

/// `CompletedFrame` represents the state of the terminal after all changes performed in the last
/// [`Terminal::draw`] call have been applied. Therefore, it is only valid until the next call to
/// [`Terminal::draw`].
///
/// [`Terminal::draw`]: crate::terminal::Terminal::draw
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct CompletedFrame<'a> {
    /// The buffer that was used to draw the last frame.
    pub buffer: &'a Buffer,
    /// The size of the last frame.
    pub area: Rect,
    /// The frame count indicating the sequence number of this frame.
    pub count: usize,
}

impl Frame<'_> {
    /// The area of the current frame
    ///
    /// This is guaranteed not to change during rendering, so may be called multiple times.
    ///
    /// If your app listens for a resize event from the backend, it should ignore the values from
    /// the event for any calculations that are used to render the current frame and use this value
    /// instead as this is the area of the buffer that is used to render the current frame.
    pub const fn area(&self) -> Rect {
        self.viewport_area
    }

    /// The area of the current frame
    ///
    /// This is guaranteed not to change during rendering, so may be called multiple times.
    ///
    /// If your app listens for a resize event from the backend, it should ignore the values from
    /// the event for any calculations that are used to render the current frame and use this value
    /// instead as this is the area of the buffer that is used to render the current frame.
    #[deprecated = "use `area()` instead"]
    pub const fn size(&self) -> Rect {
        self.viewport_area
    }

    /// Render a [`Widget`] to the current buffer using [`Widget::render`].
    ///
    /// Usually the area argument is the size of the current frame or a sub-area of the current
    /// frame (which can be obtained using [`Layout`] to split the total area).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use ratatui::{backend::TestBackend, Terminal};
    /// # let backend = TestBackend::new(5, 5);
    /// # let mut terminal = Terminal::new(backend).unwrap();
    /// # let mut frame = terminal.get_frame();
    /// use ratatui::{layout::Rect, widgets::Block};
    ///
    /// let block = Block::new();
    /// let area = Rect::new(0, 0, 5, 5);
    /// frame.render_widget(block, area);
    /// ```
    ///
    /// [`Layout`]: crate::layout::Layout
    pub fn render_widget<W: Widget>(&mut self, widget: W, area: Rect) {
        widget.render(area, self.buffer);
    }

    /// Render a [`StatefulWidget`] to the current buffer using [`StatefulWidget::render`].
    ///
    /// Usually the area argument is the size of the current frame or a sub-area of the current
    /// frame (which can be obtained using [`Layout`] to split the total area).
    ///
    /// The last argument should be an instance of the [`StatefulWidget::State`] associated to the
    /// given [`StatefulWidget`].
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use ratatui::{backend::TestBackend, Terminal};
    /// # let backend = TestBackend::new(5, 5);
    /// # let mut terminal = Terminal::new(backend).unwrap();
    /// # let mut frame = terminal.get_frame();
    /// use ratatui::{
    ///     layout::Rect,
    ///     widgets::{List, ListItem, ListState},
    /// };
    ///
    /// let mut state = ListState::default().with_selected(Some(1));
    /// let list = List::new(vec![ListItem::new("Item 1"), ListItem::new("Item 2")]);
    /// let area = Rect::new(0, 0, 5, 5);
    /// frame.render_stateful_widget(list, area, &mut state);
    /// ```
    ///
    /// [`Layout`]: crate::layout::Layout
    pub fn render_stateful_widget<W>(&mut self, widget: W, area: Rect, state: &mut W::State)
    where
        W: StatefulWidget,
    {
        widget.render(area, self.buffer, state);
    }

    /// After drawing this frame, make the cursor visible and put it at the specified (x, y)
    /// coordinates. If this method is not called, the cursor will be hidden.
    ///
    /// Note that this will interfere with calls to [`Terminal::hide_cursor`],
    /// [`Terminal::show_cursor`], and [`Terminal::set_cursor_position`]. Pick one of the APIs and
    /// stick with it.
    ///
    /// [`Terminal::hide_cursor`]: crate::terminal::Terminal::hide_cursor
    /// [`Terminal::show_cursor`]: crate::terminal::Terminal::show_cursor
    /// [`Terminal::set_cursor_position`]: crate::terminal::Terminal::set_cursor_position
    pub fn set_cursor_position<P: Into<Position>>(&mut self, position: P) {
        self.cursor_position = Some(position.into());
    }

    /// After drawing this frame, make the cursor visible and put it at the specified (x, y)
    /// coordinates. If this method is not called, the cursor will be hidden.
    ///
    /// Note that this will interfere with calls to [`Terminal::hide_cursor`],
    /// [`Terminal::show_cursor`], and [`Terminal::set_cursor_position`]. Pick one of the APIs and
    /// stick with it.
    ///
    /// [`Terminal::hide_cursor`]: crate::terminal::Terminal::hide_cursor
    /// [`Terminal::show_cursor`]: crate::terminal::Terminal::show_cursor
    /// [`Terminal::set_cursor_position`]: crate::terminal::Terminal::set_cursor_position
    #[deprecated = "use `set_cursor_position((x, y))` instead which takes `impl Into<Position>`"]
    pub fn set_cursor(&mut self, x: u16, y: u16) {
        self.set_cursor_position(Position { x, y });
    }

    /// Gets the buffer that this `Frame` draws into as a mutable reference.
    pub const fn buffer_mut(&mut self) -> &mut Buffer {
        self.buffer
    }

    /// Returns the current frame count.
    ///
    /// This method provides access to the frame count, which is a sequence number indicating
    /// how many frames have been rendered up to (but not including) this one. It can be used
    /// for purposes such as animation, performance tracking, or debugging.
    ///
    /// Each time a frame has been rendered, this count is incremented,
    /// providing a consistent way to reference the order and number of frames processed by the
    /// terminal. When count reaches its maximum value (`usize::MAX`), it wraps around to zero.
    ///
    /// This count is particularly useful when dealing with dynamic content or animations where the
    /// state of the display changes over time. By tracking the frame count, developers can
    /// synchronize updates or changes to the content with the rendering process.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # use ratatui::{backend::TestBackend, Terminal};
    /// # let backend = TestBackend::new(5, 5);
    /// # let mut terminal = Terminal::new(backend).unwrap();
    /// # let mut frame = terminal.get_frame();
    /// let current_count = frame.count();
    /// println!("Current frame count: {}", current_count);
    /// ```
    pub const fn count(&self) -> usize {
        self.count
    }
}
