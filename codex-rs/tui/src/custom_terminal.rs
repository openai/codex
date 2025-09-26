// This is derived from `ratatui::Terminal`, which is licensed under the following terms:
//
// The MIT License (MIT)
// Copyright (c) 2016-2022 Florian Dehau
// Copyright (c) 2023-2025 The Ratatui Developers
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
use std::io;

use ratatui::backend::Backend;
use ratatui::backend::ClearType;
use ratatui::buffer::Buffer;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::widgets::WidgetRef;

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

    /// Render a [`WidgetRef`] to the current buffer using [`WidgetRef::render_ref`].
    ///
    /// Usually the area argument is the size of the current frame or a sub-area of the current
    /// frame (which can be obtained using [`Layout`] to split the total area).
    #[allow(clippy::needless_pass_by_value)]
    pub fn render_widget_ref<W: WidgetRef>(&mut self, widget: W, area: Rect) {
        widget.render_ref(area, self.buffer);
    }

    /// After drawing this frame, make the cursor visible and put it at the specified (x, y)
    /// coordinates. If this method is not called, the cursor will be hidden.
    ///
    /// Note that this will interfere with calls to [`Terminal::hide_cursor`],
    /// [`Terminal::show_cursor`], and [`Terminal::set_cursor_position`]. Pick one of the APIs and
    /// stick with it.
    ///
    /// [`Terminal::hide_cursor`]: crate::Terminal::hide_cursor
    /// [`Terminal::show_cursor`]: crate::Terminal::show_cursor
    /// [`Terminal::set_cursor_position`]: crate::Terminal::set_cursor_position
    pub fn set_cursor_position<P: Into<Position>>(&mut self, position: P) {
        self.cursor_position = Some(position.into());
    }

    /// Gets the buffer that this `Frame` draws into as a mutable reference.
    pub fn buffer_mut(&mut self) -> &mut Buffer {
        self.buffer
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Terminal<B>
where
    B: Backend,
{
    /// The backend used to interface with the terminal
    backend: B,
    /// Holds the results of the current and previous draw calls. The two are compared at the end
    /// of each draw pass to output the necessary updates to the terminal
    buffers: [Buffer; 2],
    /// Index of the current buffer in the previous array
    current: usize,
    /// Whether the cursor is currently hidden
    pub hidden_cursor: bool,
    /// Area of the viewport
    pub viewport_area: Rect,
    /// Last known size of the terminal. Used to detect if the internal buffers have to be resized.
    pub last_known_screen_size: Size,
    /// Last known position of the cursor. Used to find the new area when the viewport is inlined
    /// and the terminal resized.
    pub last_known_cursor_pos: Position,
}

impl<B> Drop for Terminal<B>
where
    B: Backend,
{
    #[allow(clippy::print_stderr)]
    fn drop(&mut self) {
        // Attempt to restore the cursor state
        if self.hidden_cursor
            && let Err(err) = self.show_cursor()
        {
            eprintln!("Failed to show the cursor: {err}");
        }
    }
}

impl<B> Terminal<B>
where
    B: Backend,
{
    /// Creates a new [`Terminal`] with the given [`Backend`] and [`TerminalOptions`].
    pub fn with_options(mut backend: B) -> io::Result<Self> {
        let screen_size = backend.size()?;
        let cursor_pos = backend.get_cursor_position()?;
        Ok(Self {
            backend,
            buffers: [
                Buffer::empty(Rect::new(0, 0, 0, 0)),
                Buffer::empty(Rect::new(0, 0, 0, 0)),
            ],
            current: 0,
            hidden_cursor: false,
            viewport_area: Rect::new(0, cursor_pos.y, 0, 0),
            last_known_screen_size: screen_size,
            last_known_cursor_pos: cursor_pos,
        })
    }

    /// Get a Frame object which provides a consistent view into the terminal state for rendering.
    pub fn get_frame(&mut self) -> Frame<'_> {
        Frame {
            cursor_position: None,
            viewport_area: self.viewport_area,
            buffer: self.current_buffer_mut(),
        }
    }

    /// Gets the current buffer as a mutable reference.
    pub fn current_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.current]
    }

    /// Gets the backend
    pub const fn backend(&self) -> &B {
        &self.backend
    }

    /// Gets the backend as a mutable reference
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Obtains a difference between the previous and the current buffer and passes it to the
    /// current backend for drawing.
    pub fn flush(&mut self) -> io::Result<()> {
        let previous_buffer = &self.buffers[1 - self.current];
        let current_buffer = &self.buffers[self.current];
        let updates = previous_buffer.diff(current_buffer);
        if let Some((col, row, _)) = updates.last() {
            self.last_known_cursor_pos = Position { x: *col, y: *row };
        }
        self.backend.draw(updates.into_iter())
    }

    /// Updates the Terminal so that internal buffers match the requested area.
    ///
    /// Requested area will be saved to remain consistent when rendering. This leads to a full clear
    /// of the screen.
    pub fn resize(&mut self, screen_size: Size) -> io::Result<()> {
        self.last_known_screen_size = screen_size;
        Ok(())
    }

    /// Sets the viewport area.
    pub fn set_viewport_area(&mut self, area: Rect) {
        self.buffers[self.current].resize(area);
        self.buffers[1 - self.current].resize(area);
        self.viewport_area = area;
    }

    /// Queries the backend for size and resizes if it doesn't match the previous size.
    pub fn autoresize(&mut self) -> io::Result<()> {
        let screen_size = self.size()?;
        if screen_size != self.last_known_screen_size {
            self.resize(screen_size)?;
        }
        Ok(())
    }

    /// Draws a single frame to the terminal.
    ///
    /// Returns a [`CompletedFrame`] if successful, otherwise a [`std::io::Error`].
    ///
    /// If the render callback passed to this method can fail, use [`try_draw`] instead.
    ///
    /// Applications should call `draw` or [`try_draw`] in a loop to continuously render the
    /// terminal. These methods are the main entry points for drawing to the terminal.
    ///
    /// [`try_draw`]: Terminal::try_draw
    ///
    /// This method will:
    ///
    /// - autoresize the terminal if necessary
    /// - call the render callback, passing it a [`Frame`] reference to render to
    /// - flush the current internal state by copying the current buffer to the backend
    /// - move the cursor to the last known position if it was set during the rendering closure
    ///
    /// The render callback should fully render the entire frame when called, including areas that
    /// are unchanged from the previous frame. This is because each frame is compared to the
    /// previous frame to determine what has changed, and only the changes are written to the
    /// terminal. If the render callback does not fully render the frame, the terminal will not be
    /// in a consistent state.
    pub fn draw<F>(&mut self, render_callback: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame),
    {
        self.try_draw(|frame| {
            render_callback(frame);
            io::Result::Ok(())
        })
    }

    /// Tries to draw a single frame to the terminal.
    ///
    /// Returns [`Result::Ok`] containing a [`CompletedFrame`] if successful, otherwise
    /// [`Result::Err`] containing the [`std::io::Error`] that caused the failure.
    ///
    /// This is the equivalent of [`Terminal::draw`] but the render callback is a function or
    /// closure that returns a `Result` instead of nothing.
    ///
    /// Applications should call `try_draw` or [`draw`] in a loop to continuously render the
    /// terminal. These methods are the main entry points for drawing to the terminal.
    ///
    /// [`draw`]: Terminal::draw
    ///
    /// This method will:
    ///
    /// - autoresize the terminal if necessary
    /// - call the render callback, passing it a [`Frame`] reference to render to
    /// - flush the current internal state by copying the current buffer to the backend
    /// - move the cursor to the last known position if it was set during the rendering closure
    /// - return a [`CompletedFrame`] with the current buffer and the area of the terminal
    ///
    /// The render callback passed to `try_draw` can return any [`Result`] with an error type that
    /// can be converted into an [`std::io::Error`] using the [`Into`] trait. This makes it possible
    /// to use the `?` operator to propagate errors that occur during rendering. If the render
    /// callback returns an error, the error will be returned from `try_draw` as an
    /// [`std::io::Error`] and the terminal will not be updated.
    ///
    /// The [`CompletedFrame`] returned by this method can be useful for debugging or testing
    /// purposes, but it is often not used in regular applicationss.
    ///
    /// The render callback should fully render the entire frame when called, including areas that
    /// are unchanged from the previous frame. This is because each frame is compared to the
    /// previous frame to determine what has changed, and only the changes are written to the
    /// terminal. If the render function does not fully render the frame, the terminal will not be
    /// in a consistent state.
    pub fn try_draw<F, E>(&mut self, render_callback: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame) -> Result<(), E>,
        E: Into<io::Error>,
    {
        // Autoresize - otherwise we get glitches if shrinking or potential desync between widgets
        // and the terminal (if growing), which may OOB.
        self.autoresize()?;

        let mut frame = self.get_frame();

        render_callback(&mut frame).map_err(Into::into)?;

        // We can't change the cursor position right away because we have to flush the frame to
        // stdout first. But we also can't keep the frame around, since it holds a &mut to
        // Buffer. Thus, we're taking the important data out of the Frame and dropping it.
        let cursor_position = frame.cursor_position;

        // Draw to stdout
        self.flush()?;

        match cursor_position {
            None => self.hide_cursor()?,
            Some(position) => {
                self.show_cursor()?;
                self.set_cursor_position(position)?;
            }
        }

        self.swap_buffers();

        // Flush
        self.backend.flush()?;

        Ok(())
    }

    /// Hides the cursor.
    pub fn hide_cursor(&mut self) -> io::Result<()> {
        self.backend.hide_cursor()?;
        self.hidden_cursor = true;
        Ok(())
    }

    /// Shows the cursor.
    pub fn show_cursor(&mut self) -> io::Result<()> {
        self.backend.show_cursor()?;
        self.hidden_cursor = false;
        Ok(())
    }

    /// Gets the current cursor position.
    ///
    /// This is the position of the cursor after the last draw call.
    #[allow(dead_code)]
    pub fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.backend.get_cursor_position()
    }

    /// Sets the cursor position.
    pub fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let position = position.into();
        self.backend.set_cursor_position(position)?;
        self.last_known_cursor_pos = position;
        Ok(())
    }

    /// Clear the terminal and force a full redraw on the next draw call.
    pub fn clear(&mut self) -> io::Result<()> {
        if self.viewport_area.is_empty() {
            return Ok(());
        }
        self.backend
            .set_cursor_position(self.viewport_area.as_position())?;
        self.backend.clear_region(ClearType::AfterCursor)?;
        // Reset the back buffer to make sure the next update will redraw everything.
        self.buffers[1 - self.current].reset();
        Ok(())
    }

    /// Clears the inactive buffer and swaps it with the current buffer
    pub fn swap_buffers(&mut self) {
        self.buffers[1 - self.current].reset();
        self.current = 1 - self.current;
    }

    /// Queries the real size of the backend.
    pub fn size(&self) -> io::Result<Size> {
        self.backend.size()
    }
}
