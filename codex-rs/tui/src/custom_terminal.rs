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
use std::io::Write;

use crossterm::cursor::MoveTo;
use crossterm::cursor::SetCursorStyle;
use crossterm::queue;
use crossterm::style::Colors;
use crossterm::style::Print;
use crossterm::style::SetAttribute;
use crossterm::style::SetBackgroundColor;
use crossterm::style::SetColors;
use crossterm::style::SetForegroundColor;
use crossterm::terminal::Clear;
use derive_more::IsVariant;
use ratatui::backend::Backend;
use ratatui::backend::ClearType;
use ratatui::backend::IntoCrossterm;
use ratatui::buffer::Buffer;
use ratatui::buffer::CellDiffOption;
use ratatui::buffer::CellWidth;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::widgets::WidgetRef;

fn osc8_hyperlink_parts(symbol: &str) -> Option<(&str, &str)> {
    let content = symbol.strip_prefix("\x1b]8;;")?;
    let destination_end = content.find('\x07')?;
    let destination = &content[..destination_end];
    if destination.is_empty() {
        return None;
    }
    let visible = content[destination_end + 1..].strip_suffix("\x1b]8;;\x07")?;
    Some((destination, visible))
}

pub struct Frame<'a> {
    /// Where should the cursor be after drawing this frame?
    ///
    /// If `None`, the cursor is hidden and its position is controlled by the backend. If `Some((x,
    /// y))`, the cursor is shown and placed at `(x, y)` after the call to `Terminal::draw()`.
    pub(crate) cursor_position: Option<Position>,

    /// Visible cursor shape to apply after drawing this frame.
    cursor_style: SetCursorStyle,

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

    /// After drawing this frame, set the terminal's visible cursor style.
    pub fn set_cursor_style(&mut self, style: SetCursorStyle) {
        self.cursor_style = style;
    }

    /// Gets the buffer that this `Frame` draws into as a mutable reference.
    pub fn buffer_mut(&mut self) -> &mut Buffer {
        self.buffer
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Terminal<B>
where
    B: Backend<Error = io::Error> + Write,
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
    /// Count of visible history rows rendered above the viewport in inline mode.
    visible_history_rows: u16,
    #[cfg(test)]
    screen_size_override: Option<Size>,
}

impl<B> Drop for Terminal<B>
where
    B: Backend<Error = io::Error>,
    B: Write,
{
    #[allow(clippy::print_stderr)]
    fn drop(&mut self) {
        // Attempt to restore the cursor state
        if let Err(err) = self.reset_cursor_style() {
            eprintln!("Failed to reset the cursor style: {err}");
        }

        if self.hidden_cursor
            && let Err(err) = self.show_cursor()
        {
            eprintln!("Failed to show the cursor: {err}");
        }
    }
}

impl<B> Terminal<B>
where
    B: Backend<Error = io::Error>,
    B: Write,
{
    /// Creates a new [`Terminal`] with the given [`Backend`] and [`TerminalOptions`].
    pub fn with_options(mut backend: B) -> io::Result<Self> {
        let screen_size = backend.size()?;
        let cursor_pos = backend.get_cursor_position().unwrap_or_else(|err| {
            // Some PTYs do not answer CPR (`ESC[6n`); continue with a safe default instead
            // of failing TUI startup.
            tracing::warn!("failed to read initial cursor position; defaulting to origin: {err}");
            Position { x: 0, y: 0 }
        });
        Ok(Self::with_screen_size_and_cursor_position(
            backend,
            screen_size,
            cursor_pos,
        ))
    }

    /// Creates a new [`Terminal`] from a caller-provided initial cursor position.
    ///
    /// Startup code uses this when cursor probing has already happened outside the backend, for
    /// example through a bounded terminal probe. Supplying a stale or synthetic position changes
    /// the inline viewport anchor, so callers should only use this after they have chosen the same
    /// fallback they want the first render to honor.
    pub fn with_options_and_cursor_position(backend: B, cursor_pos: Position) -> io::Result<Self> {
        let screen_size = backend.size()?;
        Ok(Self::with_screen_size_and_cursor_position(
            backend,
            screen_size,
            cursor_pos,
        ))
    }

    fn with_screen_size_and_cursor_position(
        backend: B,
        screen_size: Size,
        cursor_pos: Position,
    ) -> Self {
        Self {
            backend,
            buffers: [Buffer::empty(Rect::ZERO), Buffer::empty(Rect::ZERO)],
            current: 0,
            hidden_cursor: false,
            viewport_area: Rect::new(
                /*x*/ 0,
                cursor_pos.y,
                /*width*/ 0,
                /*height*/ 0,
            ),
            last_known_screen_size: screen_size,
            last_known_cursor_pos: cursor_pos,
            visible_history_rows: 0,
            #[cfg(test)]
            screen_size_override: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_screen_size_and_cursor_position_for_test(
        backend: B,
        screen_size: Size,
        cursor_pos: Position,
    ) -> Self {
        let mut terminal =
            Self::with_screen_size_and_cursor_position(backend, screen_size, cursor_pos);
        terminal.screen_size_override = Some(screen_size);
        terminal
    }

    /// Get a Frame object which provides a consistent view into the terminal state for rendering.
    pub fn get_frame(&mut self) -> Frame<'_> {
        Frame {
            cursor_position: None,
            cursor_style: SetCursorStyle::DefaultUserShape,
            viewport_area: self.viewport_area,
            buffer: self.current_buffer_mut(),
        }
    }

    /// Gets the current buffer as a reference.
    fn current_buffer(&self) -> &Buffer {
        &self.buffers[self.current]
    }

    /// Gets the current buffer as a mutable reference.
    fn current_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.current]
    }

    /// Gets the previous buffer as a reference.
    fn previous_buffer(&self) -> &Buffer {
        &self.buffers[1 - self.current]
    }

    /// Gets the previous buffer as a mutable reference.
    fn previous_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[1 - self.current]
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
        let updates = diff_buffers(self.previous_buffer(), self.current_buffer());
        let last_put_command = updates.iter().rfind(|command| command.is_put());
        if let Some(&DrawCommand::Put { x, y, .. }) = last_put_command {
            self.last_known_cursor_pos = Position { x, y };
        }
        draw(&mut self.backend, updates.into_iter())
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
        self.current_buffer_mut().resize(area);
        self.previous_buffer_mut().resize(area);
        self.viewport_area = area;
        self.visible_history_rows = self.visible_history_rows.min(area.top());
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
        let screen_size = self.size()?;
        self.draw_with_size(screen_size, render_callback)
    }

    /// Draws a single frame using a screen size already obtained by the caller.
    pub(crate) fn draw_with_size<F>(&mut self, screen_size: Size, render: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame),
    {
        self.try_draw_with_size(screen_size, |frame| {
            render(frame);
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
        let screen_size = self.size()?;
        self.try_draw_with_size(screen_size, render_callback)
    }

    fn try_draw_with_size<F, E>(&mut self, screen_size: Size, render_callback: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame) -> Result<(), E>,
        E: Into<io::Error>,
    {
        if screen_size != self.last_known_screen_size {
            self.resize(screen_size)?;
        }
        let mut frame = self.get_frame();

        render_callback(&mut frame).map_err(Into::into)?;

        // We can't change the cursor position right away because we have to flush the frame to
        // stdout first. But we also can't keep the frame around, since it holds a &mut to
        // Buffer. Thus, we're taking the important data out of the Frame and dropping it.
        let cursor_position = frame.cursor_position;
        let cursor_style = frame.cursor_style;

        // Draw to stdout
        self.flush()?;

        match cursor_position {
            None => self.hide_cursor()?,
            Some(position) => {
                self.set_cursor_style(cursor_style)?;
                self.show_cursor()?;
                self.set_cursor_position(position)?;
            }
        }

        self.swap_buffers();

        Backend::flush(&mut self.backend)?;

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

    /// Sets the visible terminal cursor style.
    pub fn set_cursor_style(&mut self, style: SetCursorStyle) -> io::Result<()> {
        queue!(self.backend, style)
    }

    /// Restores the user-configured terminal cursor style.
    pub fn reset_cursor_style(&mut self) -> io::Result<()> {
        self.set_cursor_style(SetCursorStyle::DefaultUserShape)
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
        self.clear_after_position(self.viewport_area.as_position())
    }

    /// Clear from `position` through the end of the visible screen and force a full redraw.
    pub(crate) fn clear_after_position(&mut self, position: Position) -> io::Result<()> {
        self.backend.set_cursor_position(position)?;
        self.backend.clear_region(ClearType::AfterCursor)?;
        // Reset the back buffer to make sure the next update will redraw everything.
        self.previous_buffer_mut().reset();
        Ok(())
    }

    /// Force the next draw pass to repaint the entire viewport after raw terminal
    /// operations move screen content outside ratatui's knowledge. Resetting the
    /// diff buffer alone would leave default-style spaces equal to their previous
    /// cells, allowing stale terminal content to show through those spaces.
    pub fn invalidate_viewport(&mut self) {
        let previous_buffer = self.previous_buffer_mut();
        previous_buffer.reset();
        for cell in &mut previous_buffer.content {
            cell.set_diff_option(CellDiffOption::AlwaysUpdate);
        }
    }

    /// Clear the entire visible screen (not just the viewport) and force a full redraw.
    pub fn clear_visible_screen(&mut self) -> io::Result<()> {
        let home = Position { x: 0, y: 0 };
        // Some terminals (notably Terminal.app) behave more reliably if we pair ED2
        // with an explicit cursor-home before/after, matching the common `clear`
        // sequence (`CSI 2J` + `CSI H`).
        self.set_cursor_position(home)?;
        self.backend.clear_region(ClearType::All)?;
        self.set_cursor_position(home)?;
        std::io::Write::flush(&mut self.backend)?;
        self.visible_history_rows = 0;
        self.previous_buffer_mut().reset();
        Ok(())
    }

    /// Hard-reset scrollback + visible screen using an explicit ANSI sequence.
    ///
    /// Some terminals behave more reliably when purge + clear are emitted as a
    /// single ANSI sequence instead of separate backend commands.
    pub fn clear_scrollback_and_visible_screen_ansi(&mut self) -> io::Result<()> {
        if self.viewport_area.is_empty() {
            return Ok(());
        }

        // Reset scroll region + style state, home cursor, clear screen, purge scrollback.
        // The order matches the common shell `clear && printf '\\e[3J'` behavior.
        write!(self.backend, "\x1b[r\x1b[0m\x1b[H\x1b[2J\x1b[3J\x1b[H")?;
        std::io::Write::flush(&mut self.backend)?;
        self.last_known_cursor_pos = Position { x: 0, y: 0 };
        self.visible_history_rows = 0;
        self.previous_buffer_mut().reset();
        Ok(())
    }

    pub(crate) fn note_history_rows_inserted(&mut self, inserted_rows: u16) {
        self.visible_history_rows = self
            .visible_history_rows
            .saturating_add(inserted_rows)
            .min(self.viewport_area.top());
    }

    /// Clears the inactive buffer and swaps it with the current buffer
    pub fn swap_buffers(&mut self) {
        self.previous_buffer_mut().reset();
        self.current = 1 - self.current;
    }

    /// Queries the real size of the backend.
    pub fn size(&self) -> io::Result<Size> {
        #[cfg(test)]
        if let Some(size) = self.screen_size_override {
            return Ok(size);
        }
        self.backend.size()
    }
}

use ratatui::buffer::Cell;

#[derive(Debug, IsVariant)]
enum DrawCommand {
    Put { x: u16, y: u16, cell: Cell },
    ClearToEnd { x: u16, y: u16, bg: Color },
}

fn diff_buffers(a: &Buffer, b: &Buffer) -> Vec<DrawCommand> {
    let next_buffer = &b.content;

    let mut updates = vec![];
    let mut last_nonblank_columns = vec![0; a.area.height as usize];
    for y in 0..a.area.height {
        let row_start = y as usize * a.area.width as usize;
        let row_end = row_start + a.area.width as usize;
        let previous_row = &a.content[row_start..row_end];
        let row = &next_buffer[row_start..row_end];
        let bg = row.last().map(|cell| cell.bg).unwrap_or(Color::Reset);

        // Scan the row to find the rightmost column that still matters: any non-space glyph,
        // any cell whose bg differs from the row’s trailing bg, any cell with modifiers,
        // or any cell explicitly marked for updating.
        // Multi-width glyphs extend that region through their full displayed width.
        // After that point the rest of the row can be cleared with a single ClearToEnd, a perf win
        // versus emitting multiple space Put commands.
        let mut last_nonblank_column = 0usize;
        let mut column = 0usize;
        while column < row.len() {
            let cell = &row[column];
            let width = usize::from(cell.cell_width());
            // Keep AlwaysUpdate blanks in the drawable prefix; otherwise filtering the tail
            // would discard the repaint explicitly requested by Ratatui.
            if cell.symbol() != " "
                || cell.bg != bg
                || cell.modifier != Modifier::empty()
                || cell.diff_option == CellDiffOption::AlwaysUpdate
            {
                last_nonblank_column = column + (width.saturating_sub(1));
            }
            column += width.max(1); // treat zero-width symbols as width 1
        }

        let clear_start = last_nonblank_column + 1;
        if clear_start < row.len() {
            // Equal cached tails need no clear when the buffers reflect the terminal.
            // Viewport invalidation marks old cells, so out-of-band writes force inequality.
            let tail_changed = previous_row[clear_start..] != row[clear_start..];

            // Wide-glyph continuation cells look blank, so an equal tail can still overlap
            // a glyph whose leader lies before the clear boundary.
            let wide_char_overlaps_tail = previous_row[..clear_start]
                .iter()
                .enumerate()
                .rev()
                .find(|(_, cell)| cell.symbol() != " " || cell.cell_width() > 1)
                .is_some_and(|(column, cell)| {
                    column + usize::from(cell.cell_width()) > clear_start
                });
            if tail_changed || wide_char_overlaps_tail {
                let (x, y) = a.pos_of(row_start + clear_start);
                updates.push(DrawCommand::ClearToEnd { x, y, bg });
            }
        }

        last_nonblank_columns[y as usize] = last_nonblank_column as u16;
    }

    // Preserve Ratatui's native Skip, AlwaysUpdate, and multi-width diff semantics.
    let mut cell_updates = a.diff_iter(b).collect::<Vec<_>>();
    // Ratatui's ForcedWidth path skips trailing-cell invalidation when a styled wide cell shrinks.
    let visible_on_blank = Modifier::REVERSED
        .union(Modifier::UNDERLINED)
        .union(Modifier::SLOW_BLINK)
        .union(Modifier::RAPID_BLINK)
        .union(Modifier::CROSSED_OUT);
    for (i, (current, previous)) in next_buffer.iter().zip(a.content.iter()).enumerate() {
        let CellDiffOption::ForcedWidth(current_width) = current.diff_option else {
            continue;
        };
        let current_width = usize::from(current_width.get());
        let previous_width = usize::from(previous.cell_width());
        if previous_width <= current_width
            || (previous.bg == Color::Reset && !previous.modifier.intersects(visible_on_blank))
        {
            continue;
        }

        for (index, cell) in next_buffer
            .iter()
            .enumerate()
            .skip(i + current_width)
            .take(previous_width - current_width)
        {
            #[allow(deprecated)]
            let is_skip = cell.diff_option == CellDiffOption::Skip
                || (cell.skip && cell.diff_option == CellDiffOption::None);
            if !is_skip {
                let (x, y) = a.pos_of(index);
                cell_updates.push((x, y, cell));
            }
        }
    }
    cell_updates.sort_unstable_by_key(|(x, y, _)| (*y, *x));
    cell_updates.dedup_by_key(|(x, y, _)| (*y, *x));

    for (x, y, cell) in cell_updates {
        let row = usize::from(y - a.area.y);
        if x <= last_nonblank_columns[row] {
            updates.push(DrawCommand::Put {
                x,
                y,
                cell: cell.clone(),
            });
        }
    }
    updates
}

fn draw<I>(writer: &mut impl Write, commands: I) -> io::Result<()>
where
    I: Iterator<Item = DrawCommand>,
{
    let mut fg = Color::Reset;
    let mut bg = Color::Reset;
    let mut modifier = Modifier::empty();
    let mut last_pos: Option<Position> = None;
    let mut active_hyperlink: Option<String> = None;
    for command in commands {
        let (x, y) = match &command {
            DrawCommand::Put { x, y, .. } => (x, y),
            DrawCommand::ClearToEnd { x, y, .. } => (x, y),
        };
        let hyperlink = match &command {
            DrawCommand::Put { cell, .. } => osc8_hyperlink_parts(cell.symbol()),
            DrawCommand::ClearToEnd { .. } => None,
        };
        let destination = hyperlink.map(|(destination, _)| destination);
        let hyperlink_changed = active_hyperlink.as_deref() != destination;
        if hyperlink_changed && active_hyperlink.is_some() {
            queue!(writer, Print("\x1b]8;;\x07"))?;
        }
        // Move the cursor if the previous location was not (x - 1, y)
        if !matches!(last_pos, Some(p) if *x == p.x + 1 && *y == p.y) {
            queue!(writer, MoveTo(*x, *y))?;
        }
        last_pos = Some(Position { x: *x, y: *y });
        match &command {
            DrawCommand::Put { cell, .. } => {
                if cell.modifier != modifier {
                    let diff = ModifierDiff {
                        from: modifier,
                        to: cell.modifier,
                    };
                    diff.queue(writer)?;
                    modifier = cell.modifier;
                }
                if cell.fg != fg || cell.bg != bg {
                    queue!(
                        writer,
                        SetColors(Colors::new(
                            cell.fg.into_crossterm(),
                            cell.bg.into_crossterm()
                        ))
                    )?;
                    fg = cell.fg;
                    bg = cell.bg;
                }

                if hyperlink_changed && let Some(destination) = destination {
                    queue!(writer, Print(format!("\x1b]8;;{destination}\x07")))?;
                }
                let symbol = hyperlink.map_or_else(|| cell.symbol(), |(_, visible)| visible);
                queue!(writer, Print(symbol))?;
            }
            DrawCommand::ClearToEnd { bg: clear_bg, .. } => {
                queue!(writer, SetAttribute(crossterm::style::Attribute::Reset))?;
                modifier = Modifier::empty();
                queue!(writer, SetBackgroundColor((*clear_bg).into_crossterm()))?;
                bg = *clear_bg;
                queue!(writer, Clear(crossterm::terminal::ClearType::UntilNewLine))?;
            }
        }
        if hyperlink_changed {
            active_hyperlink = destination.map(str::to_owned);
        }
    }
    if active_hyperlink.is_some() {
        queue!(writer, Print("\x1b]8;;\x07"))?;
    }

    queue!(
        writer,
        SetForegroundColor(crossterm::style::Color::Reset),
        SetBackgroundColor(crossterm::style::Color::Reset),
        SetAttribute(crossterm::style::Attribute::Reset),
    )?;

    Ok(())
}

/// The `ModifierDiff` struct is used to calculate the difference between two `Modifier`
/// values. This is useful when updating the terminal display, as it allows for more
/// efficient updates by only sending the necessary changes.
struct ModifierDiff {
    pub from: Modifier,
    pub to: Modifier,
}

impl ModifierDiff {
    fn queue<W: io::Write>(self, w: &mut W) -> io::Result<()> {
        use crossterm::style::Attribute as CAttribute;
        let removed = self.from - self.to;
        if removed.contains(Modifier::REVERSED) {
            queue!(w, SetAttribute(CAttribute::NoReverse))?;
        }
        if removed.contains(Modifier::BOLD) {
            queue!(w, SetAttribute(CAttribute::NormalIntensity))?;
            if self.to.contains(Modifier::DIM) {
                queue!(w, SetAttribute(CAttribute::Dim))?;
            }
        }
        if removed.contains(Modifier::ITALIC) {
            queue!(w, SetAttribute(CAttribute::NoItalic))?;
        }
        if removed.contains(Modifier::UNDERLINED) {
            queue!(w, SetAttribute(CAttribute::NoUnderline))?;
        }
        if removed.contains(Modifier::DIM) {
            queue!(w, SetAttribute(CAttribute::NormalIntensity))?;
        }
        if removed.contains(Modifier::CROSSED_OUT) {
            queue!(w, SetAttribute(CAttribute::NotCrossedOut))?;
        }
        if removed.contains(Modifier::SLOW_BLINK) || removed.contains(Modifier::RAPID_BLINK) {
            queue!(w, SetAttribute(CAttribute::NoBlink))?;
        }

        let added = self.to - self.from;
        if added.contains(Modifier::REVERSED) {
            queue!(w, SetAttribute(CAttribute::Reverse))?;
        }
        if added.contains(Modifier::BOLD) {
            queue!(w, SetAttribute(CAttribute::Bold))?;
        }
        if added.contains(Modifier::ITALIC) {
            queue!(w, SetAttribute(CAttribute::Italic))?;
        }
        if added.contains(Modifier::UNDERLINED) {
            queue!(w, SetAttribute(CAttribute::Underlined))?;
        }
        if added.contains(Modifier::DIM) {
            queue!(w, SetAttribute(CAttribute::Dim))?;
        }
        if added.contains(Modifier::CROSSED_OUT) {
            queue!(w, SetAttribute(CAttribute::CrossedOut))?;
        }
        if added.contains(Modifier::SLOW_BLINK) {
            queue!(w, SetAttribute(CAttribute::SlowBlink))?;
        }
        if added.contains(Modifier::RAPID_BLINK) {
            queue!(w, SetAttribute(CAttribute::RapidBlink))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU16;

    use crate::test_backend::VT100Backend;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::backend::WindowSize;
    use ratatui::layout::Rect;
    use ratatui::style::Style;
    use ratatui::style::Stylize;
    use ratatui::text::Line;
    use ratatui::widgets::Paragraph;
    use ratatui::widgets::Widget;
    use ratatui::widgets::Wrap;

    struct CaptureBackend {
        output: Vec<u8>,
        size: Size,
        cursor: Position,
        size_call_count: std::cell::Cell<usize>,
    }

    impl CaptureBackend {
        fn new(width: u16, height: u16) -> Self {
            Self {
                output: Vec::new(),
                size: Size { width, height },
                cursor: Position { x: 0, y: 0 },
                size_call_count: std::cell::Cell::new(/*value*/ 0),
            }
        }

        fn output(&self) -> String {
            String::from_utf8_lossy(&self.output).into_owned()
        }
    }

    impl Write for CaptureBackend {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.output.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Backend for CaptureBackend {
        type Error = io::Error;

        fn draw<'a, I>(&mut self, _content: I) -> io::Result<()>
        where
            I: Iterator<Item = (u16, u16, &'a Cell)>,
        {
            Ok(())
        }

        fn hide_cursor(&mut self) -> io::Result<()> {
            Ok(())
        }

        fn show_cursor(&mut self) -> io::Result<()> {
            Ok(())
        }

        fn get_cursor_position(&mut self) -> io::Result<Position> {
            Ok(self.cursor)
        }

        fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
            self.cursor = position.into();
            Ok(())
        }

        fn clear(&mut self) -> io::Result<()> {
            Ok(())
        }

        fn clear_region(&mut self, _clear_type: ClearType) -> io::Result<()> {
            Ok(())
        }

        fn append_lines(&mut self, _line_count: u16) -> io::Result<()> {
            Ok(())
        }

        fn scroll_region_up(
            &mut self,
            _region: std::ops::Range<u16>,
            _scroll_by: u16,
        ) -> io::Result<()> {
            Ok(())
        }

        fn scroll_region_down(
            &mut self,
            _region: std::ops::Range<u16>,
            _scroll_by: u16,
        ) -> io::Result<()> {
            Ok(())
        }

        fn size(&self) -> io::Result<Size> {
            self.size_call_count
                .set(self.size_call_count.get().saturating_add(/*rhs*/ 1));
            Ok(self.size)
        }

        fn window_size(&mut self) -> io::Result<WindowSize> {
            Ok(WindowSize {
                columns_rows: self.size,
                pixels: self.size,
            })
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn invalidate_viewport_repaints_default_style_spaces_over_stale_terminal_cells() {
        let width = 32;
        let height = 2;
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
        let mut terminal =
            Terminal::with_options(VT100Backend::new(width, height)).expect("terminal");
        terminal.set_viewport_area(area);

        // History insertion writes directly to the terminal, leaving cells that are absent from
        // the diff buffers. Interior default-style spaces must still overwrite those stale cells.
        write!(
            terminal.backend_mut(),
            "probe-08tcleantwords stale\r\nprobe-09xcleanxwords stale"
        )
        .expect("prefill terminal");
        assert!(
            terminal
                .backend()
                .vt100()
                .screen()
                .contents()
                .contains("probe-08tcleantwords")
        );

        terminal.invalidate_viewport();
        terminal
            .draw(|frame| {
                Paragraph::new(vec![
                    Line::from("probe-08 clean words"),
                    Line::from("probe-09 clean words"),
                ])
                .render(area, frame.buffer_mut());
            })
            .expect("redraw invalidated viewport");

        assert_snapshot!(terminal.backend().vt100().screen().contents(), @r"
        probe-08 clean words
        probe-09 clean words
        ");
    }

    #[test]
    fn ordinary_redraws_with_known_size_do_not_query_backend_size() {
        let mut terminal =
            Terminal::with_options(CaptureBackend::new(/*width*/ 80, /*height*/ 24))
                .expect("terminal");
        let screen_size = terminal.last_known_screen_size;

        for _ in 0..3 {
            terminal.draw_with_size(screen_size, |_| {}).expect("draw");
        }

        terminal.set_viewport_area(Rect::new(
            /*x*/ 0, /*y*/ 23, /*width*/ 80, /*height*/ 1,
        ));
        crate::insert_history::insert_history_lines(&mut terminal, vec![Line::from("history")])
            .expect("insert history");

        assert_eq!(terminal.backend().size_call_count.get(), 1);
    }

    #[test]
    fn resize_draw_applies_event_dimensions_without_querying_backend_size() {
        let mut terminal =
            Terminal::with_options(CaptureBackend::new(/*width*/ 12, /*height*/ 4))
                .expect("terminal");
        let mut snapshots = Vec::new();

        for width in [12, 8] {
            let size = Size::new(width, /*height*/ 4);
            let area = Rect::new(/*x*/ 0, /*y*/ 0, size.width, size.height);
            terminal.set_viewport_area(area);
            terminal
                .draw_with_size(size, |frame| {
                    Paragraph::new("alpha beta")
                        .wrap(Wrap { trim: false })
                        .render(area, frame.buffer_mut());
                })
                .expect("draw resized frame");

            let rendered = (0..size.height)
                .map(|y| {
                    (0..size.width)
                        .map(|x| terminal.previous_buffer()[(x, y)].symbol())
                        .collect::<String>()
                        .trim_end()
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join("\n");
            snapshots.push(rendered.trim_end().to_string());
        }

        assert_eq!(terminal.backend().size_call_count.get(), 1);
        assert_eq!(
            terminal.last_known_screen_size,
            Size::new(/*width*/ 8, /*height*/ 4)
        );
        assert_snapshot!(snapshots.join("\n\n"), @r"
        alpha beta

        alpha
        beta
        ");
    }

    #[test]
    fn diff_buffers_only_updates_changed_cells_when_row_tails_are_unchanged() {
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 10, /*height*/ 2,
        );
        let mut previous = Buffer::empty(area);
        previous.set_string(0, 0, "-", Style::default());
        previous.set_string(0, 1, "中", Style::default());

        assert_eq!(diff_buffers(&previous, &previous).len(), 0);

        let mut next = previous.clone();
        next.set_string(0, 0, "\\", Style::default());

        let commands = diff_buffers(&previous, &next);
        assert_eq!(commands.len(), 1, "unexpected draw commands: {commands:?}");
        assert!(matches!(
            commands.as_slice(),
            [DrawCommand::Put { x: 0, y: 0, cell }] if cell.symbol() == "\\"
        ));
    }

    #[test]
    fn diff_buffers_does_not_emit_clear_to_end_for_full_width_row() {
        let area = Rect::new(0, 0, 3, 2);
        let previous = Buffer::empty(area);
        let mut next = Buffer::empty(area);

        next.cell_mut((2, 0))
            .expect("cell should exist")
            .set_symbol("X");

        let commands = diff_buffers(&previous, &next);

        let clear_count = commands
            .iter()
            .filter(|command| matches!(command, DrawCommand::ClearToEnd { y, .. } if *y == 0))
            .count();
        assert_eq!(
            0, clear_count,
            "expected diff_buffers not to emit ClearToEnd; commands: {commands:?}",
        );
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, DrawCommand::Put { x: 2, y: 0, .. })),
            "expected diff_buffers to update the final cell; commands: {commands:?}",
        );
    }

    #[test]
    fn diff_buffers_clear_to_end_starts_after_wide_char() {
        let area = Rect::new(0, 0, 10, 1);
        for (before, after) in [("中文", "中"), ("ｶﾞﾞ", "ｶﾞ")] {
            let mut previous = Buffer::empty(area);
            let mut next = Buffer::empty(area);

            previous.set_string(0, 0, before, Style::default());
            next.set_string(0, 0, after, Style::default());

            let commands = diff_buffers(&previous, &next);
            assert!(
                commands
                    .iter()
                    .any(|command| matches!(command, DrawCommand::ClearToEnd { x: 2, y: 0, .. })),
                "expected clear-to-end after {before:?} became {after:?}; commands: {commands:?}"
            );
        }

        let mut terminal =
            Terminal::with_options(VT100Backend::new(area.width, area.height)).expect("terminal");
        terminal.set_viewport_area(area);
        for text in ["ｶﾞﾞ", "ｶﾞ"] {
            terminal
                .draw(|frame| Paragraph::new(text).render(area, frame.buffer_mut()))
                .expect("draw");
        }
        assert_snapshot!(terminal.backend().vt100().screen().contents(), @"ｶﾞ");
    }

    #[test]
    fn terminal_draw_coalesces_wrapped_hyperlink_output() {
        let auth_url = format!(
            "https://auth.openai.com/oauth/authorize?response_type=code&state={}",
            "x".repeat(/*n*/ 400)
        );
        let width = 44;
        let height = 20;
        let area = Rect::new(0, 0, width, height);
        let mut terminal =
            Terminal::with_options(CaptureBackend::new(width, height)).expect("terminal");
        terminal.set_viewport_area(area);

        terminal
            .draw(|frame| {
                Paragraph::new(vec![
                    Line::from(vec!["  ".into(), auth_url.as_str().cyan().underlined()]),
                    "".into(),
                    "  Press Esc to cancel".into(),
                ])
                .wrap(Wrap { trim: false })
                .render(area, frame.buffer_mut());
                crate::terminal_hyperlinks::mark_url_hyperlink(frame.buffer_mut(), area, &auth_url);
            })
            .expect("draw");

        let output = terminal.backend().output();
        let open = format!("\x1b]8;;{auth_url}\x07");
        let close = "\x1b]8;;\x07";
        assert_eq!(output.matches(&open).count(), 1);
        assert_eq!(output.matches(close).count(), 1);
        let footer = output.find("Press").expect("footer");
        assert!(output.find(close).expect("hyperlink close") < footer);
    }

    #[test]
    fn diff_buffers_emits_always_update_cells() {
        use ratatui::buffer::CellDiffOption;

        for text in ["abc", "a  "] {
            let mut previous = Buffer::with_lines([text]);
            let mut next = Buffer::with_lines([text]);
            previous[(1, 0)].set_diff_option(CellDiffOption::AlwaysUpdate);
            next[(1, 0)].set_diff_option(CellDiffOption::AlwaysUpdate);

            let commands = diff_buffers(&previous, &next);
            assert!(
                commands
                    .iter()
                    .any(|command| matches!(command, DrawCommand::Put { x: 1, y: 0, .. })),
                "expected the always-update cell in {text:?} to be emitted; commands: {commands:?}"
            );
        }
    }

    #[test]
    fn diff_buffers_clears_styled_trailing_cell_replaced_by_forced_width_cell() {
        use ratatui::buffer::CellDiffOption;

        let area = Rect::new(0, 0, 7, 1);
        let mut previous = Buffer::empty(area);
        let mut next = Buffer::empty(area);
        previous.set_string(
            0,
            0,
            "漢 tail",
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
        );
        next.set_string(0, 0, "a tail", Style::default());
        next[(0, 0)]
            .set_symbol("\x1b]8;;https://example.com\x07a\x1b]8;;\x07")
            .set_diff_option(CellDiffOption::ForcedWidth(NonZeroU16::MIN));

        let commands = diff_buffers(&previous, &next);

        assert!(
            commands
                .iter()
                .any(|command| matches!(command, DrawCommand::Put { x: 1, y: 0, .. })),
            "expected the styled trailing cell to be cleared; commands: {commands:?}"
        );
    }

    #[test]
    fn terminal_draw_applies_requested_cursor_style() {
        let mut output = Vec::new();
        let mut terminal =
            Terminal::with_options(CaptureBackend::new(/*width*/ 2, /*height*/ 1))
                .expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 2, 1));

        terminal
            .try_draw(|frame| {
                frame.set_cursor_style(SetCursorStyle::SteadyBar);
                frame.set_cursor_position((0, 0));
                io::Result::Ok(())
            })
            .expect("draw");

        queue!(output, SetCursorStyle::SteadyBar).expect("queue style");
        let expected = String::from_utf8(output).expect("utf8");
        let actual = terminal.backend().output();
        assert!(
            actual.contains(&expected),
            "expected terminal output to contain cursor style {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn reset_cursor_style_emits_default_user_shape() {
        let mut output = Vec::new();
        let mut terminal =
            Terminal::with_options(CaptureBackend::new(/*width*/ 2, /*height*/ 1))
                .expect("terminal");

        terminal.reset_cursor_style().expect("reset cursor style");
        ratatui::backend::Backend::flush(terminal.backend_mut()).expect("flush backend");

        queue!(output, SetCursorStyle::DefaultUserShape).expect("queue style");
        let expected = String::from_utf8(output).expect("utf8");
        let actual = terminal.backend().output();
        assert!(
            actual.contains(&expected),
            "expected terminal output to contain cursor style reset {expected:?}, got {actual:?}"
        );
    }
}
