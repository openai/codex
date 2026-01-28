//! This is derived from `ratatui::Terminal`, which is licensed under the following terms:
//!
//! The MIT License (MIT)
//! Copyright (c) 2016-2022 Florian Dehau
//! Copyright (c) 2023-2025 The Ratatui Developers
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the \"Software\"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.
use ratatui::backend::Backend;
use ratatui::backend::ClearType;
use ratatui::buffer::Buffer;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::widgets::WidgetRef;
use std::io;

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
    /// Returns the area of the current frame.
    ///
    /// This is guaranteed not to change during rendering, so may be called multiple times.
    ///
    /// If your app listens for a resize event from the backend, it should ignore the values from
    /// the event for any calculations that are used to render the current frame and use this value
    /// instead as this is the area of the buffer that is used to render the current frame.
    ///
    /// # Returns
    /// - `Rect`: Current frame area.
    pub const fn area(&self) -> Rect {
        self.viewport_area
    }

    /// Renders a [`WidgetRef`] to the current buffer.
    ///
    /// Usually the area argument is the size of the current frame or a sub-area of the current
    /// frame (which can be obtained using [`Layout`] to split the total area).
    ///
    /// # Arguments
    /// - `widget` (W): Widget to render.
    /// - `area` (Rect): Target area.
    ///
    /// # Returns
    /// - `()`: No return value.
    #[allow(clippy::needless_pass_by_value)]
    pub fn render_widget_ref<W: WidgetRef>(&mut self, widget: W, area: Rect) {
        widget.render_ref(area, self.buffer);
    }

    /// Makes the cursor visible and sets it to the specified position.
    /// coordinates. If this method is not called, the cursor will be hidden.
    ///
    /// Note that this will interfere with calls to [`Terminal::hide_cursor`],
    /// [`Terminal::show_cursor`], and [`Terminal::set_cursor_position`]. Pick one of the APIs and
    /// stick with it.
    ///
    /// [`Terminal::hide_cursor`]: crate::Terminal::hide_cursor
    /// [`Terminal::show_cursor`]: crate::Terminal::show_cursor
    /// [`Terminal::set_cursor_position`]: crate::Terminal::set_cursor_position
    ///
    /// # Arguments
    /// - `position` (P): Cursor position.
    ///
    /// # Returns
    /// - `()`: No return value.
    pub fn set_cursor_position<P: Into<Position>>(&mut self, position: P) {
        self.cursor_position = Some(position.into());
    }

    /// Returns the mutable buffer for this frame.
    ///
    /// # Returns
    /// - `&mut Buffer`: Mutable frame buffer.
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
    /// Creates a new terminal with the given backend.
    ///
    /// # Arguments
    /// - `backend` (B): Backend implementation.
    ///
    /// # Returns
    /// - `io::Result<Terminal<B>>`: Initialized terminal or error.
    pub fn with_options(mut backend: B) -> io::Result<Self> {
        let screen_size = backend.size()?;
        let cursor_pos = backend.get_cursor_position()?;
        Ok(Self {
            backend,
            buffers: [Buffer::empty(Rect::ZERO), Buffer::empty(Rect::ZERO)],
            current: 0,
            hidden_cursor: false,
            viewport_area: Rect::new(0, cursor_pos.y, 0, 0),
            last_known_screen_size: screen_size,
            last_known_cursor_pos: cursor_pos,
        })
    }

    /// Returns a frame that provides a consistent view for rendering.
    ///
    /// # Returns
    /// - `Frame<'_>`: Rendering frame.
    pub fn get_frame(&mut self) -> Frame<'_> {
        Frame {
            cursor_position: None,
            viewport_area: self.viewport_area,
            buffer: self.current_buffer_mut(),
        }
    }

    /// Returns the current buffer as a reference.
    ///
    /// # Returns
    /// - `&Buffer`: Current buffer reference.
    fn current_buffer(&self) -> &Buffer {
        &self.buffers[self.current]
    }

    /// Returns the current buffer as a mutable reference.
    ///
    /// # Returns
    /// - `&mut Buffer`: Mutable current buffer reference.
    fn current_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.current]
    }

    /// Returns the previous buffer as a reference.
    ///
    /// # Returns
    /// - `&Buffer`: Previous buffer reference.
    fn previous_buffer(&self) -> &Buffer {
        &self.buffers[1 - self.current]
    }

    /// Returns the previous buffer as a mutable reference.
    ///
    /// # Returns
    /// - `&mut Buffer`: Mutable previous buffer reference.
    fn previous_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[1 - self.current]
    }

    /// Returns the backend reference.
    ///
    /// # Returns
    /// - `&B`: Backend reference.
    pub const fn backend(&self) -> &B {
        &self.backend
    }

    /// Returns the backend as a mutable reference.
    ///
    /// # Returns
    /// - `&mut B`: Mutable backend reference.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Flushes buffered changes to the backend.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the flush operation.
    pub fn flush(&mut self) -> io::Result<()> {
        let updates = diff_buffers(self.previous_buffer(), self.current_buffer());
        if let Some((x, y, _)) = updates.last() {
            self.last_known_cursor_pos = Position { x: *x, y: *y };
        }
        self.backend
            .draw(updates.iter().map(|(x, y, cell)| (*x, *y, cell)))?;
        self.backend.flush()
    }

    /// Updates the terminal buffers to match the requested area.
    ///
    /// Requested area will be saved to remain consistent when rendering. This leads to a full clear
    /// of the screen.
    ///
    /// # Arguments
    /// - `screen_size` (Size): New terminal size.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the resize operation.
    pub fn resize(&mut self, screen_size: Size) -> io::Result<()> {
        self.last_known_screen_size = screen_size;
        Ok(())
    }

    /// Sets the viewport area.
    ///
    /// # Arguments
    /// - `area` (Rect): New viewport area.
    ///
    /// # Returns
    /// - `()`: No return value.
    pub fn set_viewport_area(&mut self, area: Rect) {
        self.current_buffer_mut().resize(area);
        self.previous_buffer_mut().resize(area);
        self.viewport_area = area;
    }

    /// Queries the backend for size and resizes if it does not match.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the resize operation.
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
    ///
    /// # Arguments
    /// - `render_callback` (F): Render callback.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the draw operation.
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
    ///
    /// # Arguments
    /// - `render_callback` (F): Render callback.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the draw operation.
    pub fn try_draw<F, E>(&mut self, render_callback: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame) -> Result<(), E>,
        E: Into<io::Error>,
    {
        self.autoresize()?;

        let mut frame = self.get_frame();

        render_callback(&mut frame).map_err(Into::into)?;

        let cursor_position = frame.cursor_position;

        self.flush()?;

        match cursor_position {
            None => self.hide_cursor()?,
            Some(position) => {
                self.show_cursor()?;
                self.set_cursor_position(position)?;
            }
        }

        self.swap_buffers();

        Backend::flush(&mut self.backend)?;

        Ok(())
    }

    /// Hides the cursor.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the cursor operation.
    pub fn hide_cursor(&mut self) -> io::Result<()> {
        self.backend.hide_cursor()?;
        self.hidden_cursor = true;
        Ok(())
    }

    /// Shows the cursor.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the cursor operation.
    pub fn show_cursor(&mut self) -> io::Result<()> {
        self.backend.show_cursor()?;
        self.hidden_cursor = false;
        Ok(())
    }

    /// Gets the current cursor position.
    ///
    /// This is the position of the cursor after the last draw call.
    ///
    /// # Returns
    /// - `io::Result<Position>`: Current cursor position.
    #[allow(dead_code)]
    pub fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.backend.get_cursor_position()
    }

    /// Sets the cursor position.
    ///
    /// # Arguments
    /// - `position` (P): Cursor position.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the cursor operation.
    pub fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let position = position.into();
        self.backend.set_cursor_position(position)?;
        self.last_known_cursor_pos = position;
        Ok(())
    }

    /// Clears the terminal and forces a full redraw on the next draw call.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the clear operation.
    pub fn clear(&mut self) -> io::Result<()> {
        if self.viewport_area.is_empty() {
            return Ok(());
        }
        self.backend
            .set_cursor_position(self.viewport_area.as_position())?;
        self.backend.clear_region(ClearType::AfterCursor)?;
        self.previous_buffer_mut().reset();
        Ok(())
    }

    /// Clears terminal scrollback and forces a full redraw.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the clear operation.
    pub fn clear_scrollback(&mut self) -> io::Result<()> {
        if self.viewport_area.is_empty() {
            return Ok(());
        }
        self.backend
            .set_cursor_position(self.viewport_area.as_position())?;
        self.backend.clear()?;
        self.previous_buffer_mut().reset();
        Ok(())
    }

    /// Clears the inactive buffer and swaps it with the current buffer.
    ///
    /// # Returns
    /// - `()`: No return value.
    pub fn swap_buffers(&mut self) {
        self.previous_buffer_mut().reset();
        self.current = 1 - self.current;
    }

    /// Queries the real size of the backend.
    ///
    /// # Returns
    /// - `io::Result<Size>`: Backend size.
    pub fn size(&self) -> io::Result<Size> {
        self.backend.size()
    }
}

use ratatui::buffer::Cell;
use unicode_width::UnicodeWidthStr;

/// Computes the list of cells that differ between two buffers.
///
/// # Arguments
/// - `a` (&Buffer): Previous buffer.
/// - `b` (&Buffer): Current buffer.
///
/// # Returns
/// - `Vec<(u16, u16, Cell)>`: Updates to apply.
fn diff_buffers(a: &Buffer, b: &Buffer) -> Vec<(u16, u16, Cell)> {
    let previous_buffer = &a.content;
    let next_buffer = &b.content;

    let mut updates = vec![];
    let mut last_nonblank_columns = vec![0; a.area.height as usize];
    for y in 0..a.area.height {
        let row_start = y as usize * a.area.width as usize;
        let row_end = row_start + a.area.width as usize;
        let row = &next_buffer[row_start..row_end];
        let bg = row.last().map(|cell| cell.bg).unwrap_or(Color::Reset);

        let mut last_nonblank_column = 0usize;
        let mut column = 0usize;
        while column < row.len() {
            let cell = &row[column];
            let width = cell.symbol().width();
            if cell.symbol() != " " || cell.bg != bg || cell.modifier != Modifier::empty() {
                last_nonblank_column = column + (width.saturating_sub(1));
            }
            column += width.max(1);
        }

        if last_nonblank_column + 1 < row.len() {
            for column in (last_nonblank_column + 1)..row.len() {
                let mut cell = Cell::new(" ");
                cell.set_bg(bg);
                updates.push((column as u16, y, cell));
            }
        }

        last_nonblank_columns[y as usize] = last_nonblank_column as u16;
    }

    let mut invalidated: usize = 0;
    let mut to_skip: usize = 0;
    for (i, (current, previous)) in next_buffer.iter().zip(previous_buffer.iter()).enumerate() {
        if !current.skip && (current != previous || invalidated > 0) && to_skip == 0 {
            let (x, y) = a.pos_of(i);
            let row = i / a.area.width as usize;
            if x <= last_nonblank_columns[row] {
                updates.push((x, y, next_buffer[i].clone()));
            }
        }

        to_skip = current.symbol().width().saturating_sub(1);

        let affected_width = std::cmp::max(current.symbol().width(), previous.symbol().width());
        invalidated = std::cmp::max(affected_width, invalidated).saturating_sub(1);
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::*;

    use ratatui::layout::Rect;
    use ratatui::style::Style;

    /// Ensures diff_buffers updates the last cell without trailing clears.
    #[test]
    fn diff_buffers_does_not_emit_clear_to_end_for_full_width_row() {
        let area = Rect::new(0, 0, 3, 2);
        let previous = Buffer::empty(area);
        let mut next = Buffer::empty(area);

        next.cell_mut((2, 0))
            .expect("cell should exist")
            .set_symbol("X");

        let updates = diff_buffers(&previous, &next);

        assert!(
            updates
                .iter()
                .any(|(x, y, cell)| *x == 2 && *y == 0 && cell.symbol() == "X"),
            "expected diff_buffers to update the final cell; updates: {updates:?}",
        );
    }

    /// Ensures diff_buffers clears after a wide character removal.
    #[test]
    fn diff_buffers_clear_to_end_starts_after_wide_char() {
        let area = Rect::new(0, 0, 10, 1);
        let mut previous = Buffer::empty(area);
        let mut next = Buffer::empty(area);

        previous.set_string(0, 0, "中文", Style::default());
        next.set_string(0, 0, "中", Style::default());

        let updates = diff_buffers(&previous, &next);
        assert!(
            updates
                .iter()
                .any(|(x, y, cell)| *x == 2 && *y == 0 && cell.symbol() == " "),
            "expected spaces to start after the remaining wide char; updates: {updates:?}"
        );
    }
}
