use std::io;
use std::io::Stdout;
use std::io::stdout;

use ratatui::backend::Backend;
use ratatui::backend::ClearType;
use ratatui::backend::CrosstermBackend;
use ratatui::backend::WindowSize;
use ratatui::buffer::Cell;
use ratatui::layout::Position;
use ratatui::layout::Size;

use super::curses_backend::CursesBackend;

/// Unified backend that selects curses by default and falls back to crossterm.
pub(crate) enum TuiBackend {
    Curses(CursesBackend),
    Crossterm(CrosstermBackend<Stdout>),
}

impl TuiBackend {
    /// Creates a backend, preferring curses and falling back to crossterm.
    ///
    /// # Returns
    /// - `io::Result<TuiBackend>`: Initialized backend.
    pub fn new_default() -> io::Result<Self> {
        match CursesBackend::new() {
            Ok(backend) => Ok(Self::Curses(backend)),
            Err(_) => Ok(Self::Crossterm(CrosstermBackend::new(stdout()))),
        }
    }

    /// Returns true when the backend is crossterm.
    ///
    /// # Returns
    /// - `bool`: True when crossterm backend is active.
    pub fn is_crossterm(&self) -> bool {
        matches!(self, Self::Crossterm(_))
    }

    /// Returns true when the backend is curses.
    ///
    /// # Returns
    /// - `bool`: True when curses backend is active.
    pub fn is_curses(&self) -> bool {
        matches!(self, Self::Curses(_))
    }
}

impl Backend for TuiBackend {
    /// Draws content to the terminal.
    ///
    /// # Arguments
    /// - `content` (Iterator): Iterator of cell updates.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the draw operation.
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        match self {
            Self::Curses(backend) => backend.draw(content),
            Self::Crossterm(backend) => backend.draw(content),
        }
    }

    /// Hides the cursor.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the cursor operation.
    fn hide_cursor(&mut self) -> io::Result<()> {
        match self {
            Self::Curses(backend) => backend.hide_cursor(),
            Self::Crossterm(backend) => backend.hide_cursor(),
        }
    }

    /// Shows the cursor.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the cursor operation.
    fn show_cursor(&mut self) -> io::Result<()> {
        match self {
            Self::Curses(backend) => backend.show_cursor(),
            Self::Crossterm(backend) => backend.show_cursor(),
        }
    }

    /// Gets the current cursor position.
    ///
    /// # Returns
    /// - `io::Result<Position>`: Current cursor position.
    fn get_cursor_position(&mut self) -> io::Result<Position> {
        match self {
            Self::Curses(backend) => backend.get_cursor_position(),
            Self::Crossterm(backend) => backend.get_cursor_position(),
        }
    }

    /// Sets the cursor position.
    ///
    /// # Arguments
    /// - `position` (Position): New cursor position.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the cursor operation.
    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        match self {
            Self::Curses(backend) => backend.set_cursor_position(position),
            Self::Crossterm(backend) => backend.set_cursor_position(position),
        }
    }

    /// Clears the entire terminal.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the clear operation.
    fn clear(&mut self) -> io::Result<()> {
        match self {
            Self::Curses(backend) => backend.clear(),
            Self::Crossterm(backend) => backend.clear(),
        }
    }

    /// Clears a region of the terminal.
    ///
    /// # Arguments
    /// - `clear_type` (ClearType): Region to clear.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the clear operation.
    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        match self {
            Self::Curses(backend) => backend.clear_region(clear_type),
            Self::Crossterm(backend) => backend.clear_region(clear_type),
        }
    }

    /// Returns the terminal size.
    ///
    /// # Returns
    /// - `io::Result<Size>`: Current terminal size.
    fn size(&self) -> io::Result<Size> {
        match self {
            Self::Curses(backend) => backend.size(),
            Self::Crossterm(backend) => backend.size(),
        }
    }

    /// Returns the terminal size in rows/columns and pixels.
    ///
    /// # Returns
    /// - `io::Result<WindowSize>`: Window size details.
    fn window_size(&mut self) -> io::Result<WindowSize> {
        match self {
            Self::Curses(backend) => backend.window_size(),
            Self::Crossterm(backend) => backend.window_size(),
        }
    }

    /// Flushes pending output to the terminal.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the flush operation.
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Curses(backend) => backend.flush(),
            Self::Crossterm(backend) => backend.flush(),
        }
    }

    /// Appends blank lines to the terminal.
    ///
    /// # Arguments
    /// - `n` (u16): Number of lines to append.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the append operation.
    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        match self {
            Self::Curses(backend) => backend.append_lines(n),
            Self::Crossterm(backend) => backend.append_lines(n),
        }
    }

    /// Scrolls a region upward.
    ///
    /// # Arguments
    /// - `region` (Range<u16>): Row range to scroll.
    /// - `line_count` (u16): Number of lines to scroll.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the scroll operation.
    fn scroll_region_up(
        &mut self,
        region: std::ops::Range<u16>,
        line_count: u16,
    ) -> io::Result<()> {
        match self {
            Self::Curses(backend) => backend.scroll_region_up(region, line_count),
            Self::Crossterm(backend) => backend.scroll_region_up(region, line_count),
        }
    }

    /// Scrolls a region downward.
    ///
    /// # Arguments
    /// - `region` (Range<u16>): Row range to scroll.
    /// - `line_count` (u16): Number of lines to scroll.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the scroll operation.
    fn scroll_region_down(
        &mut self,
        region: std::ops::Range<u16>,
        line_count: u16,
    ) -> io::Result<()> {
        match self {
            Self::Curses(backend) => backend.scroll_region_down(region, line_count),
            Self::Crossterm(backend) => backend.scroll_region_down(region, line_count),
        }
    }
}
