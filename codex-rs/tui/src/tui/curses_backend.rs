use std::collections::HashMap;
use std::io;

use pancurses::*;
use ratatui::backend::Backend;
use ratatui::backend::ClearType;
use ratatui::backend::WindowSize;
use ratatui::buffer::Cell;
use ratatui::layout::Position;
use ratatui::layout::Size;
use ratatui::style::Color;
use ratatui::style::Modifier;

/// Backend implementation using curses APIs via pancurses.
pub(crate) struct CursesBackend {
    window: Window,
    pairs: HashMap<(Color, Color), i16>,
    next_pair: i16,
    has_colors: bool,
}

impl CursesBackend {
    /// Creates a new curses backend.
    ///
    /// # Returns
    /// - `io::Result<CursesBackend>`: Initialized backend or an error.
    pub fn new() -> io::Result<Self> {
        let window = std::panic::catch_unwind(initscr)
            .map_err(|_| io::Error::other("failed to initialize curses"))?;
        raw();
        noecho();
        window.keypad(true);
        let has_colors = has_colors();
        if has_colors {
            start_color();
            use_default_colors();
        }
        Ok(Self {
            window,
            pairs: HashMap::new(),
            next_pair: 1,
            has_colors,
        })
    }

    /// Computes the curses attribute for a cell.
    ///
    /// # Arguments
    /// - `cell` (&Cell): Cell to style.
    ///
    /// # Returns
    /// - `attr_t`: Curses attribute mask.
    fn attr_for_cell(&mut self, cell: &Cell) -> chtype {
        let mut attr: chtype = 0;
        if cell.modifier.contains(Modifier::BOLD) {
            attr |= A_BOLD;
        }
        if cell.modifier.contains(Modifier::DIM) || cell.fg == Color::DarkGray {
            attr |= A_DIM;
        }
        if cell.modifier.contains(Modifier::UNDERLINED) {
            attr |= A_UNDERLINE;
        }
        if cell.modifier.contains(Modifier::REVERSED) {
            attr |= A_REVERSE;
        }
        if self.has_colors {
            let pair = self.pair_for(cell.fg, cell.bg);
            if pair > 0 {
                attr |= COLOR_PAIR(pair as chtype);
            }
        }
        attr
    }

    /// Returns a color pair id for the foreground/background colors.
    ///
    /// # Arguments
    /// - `fg` (Color): Foreground color.
    /// - `bg` (Color): Background color.
    ///
    /// # Returns
    /// - `i16`: Color pair id or 0 when unavailable.
    fn pair_for(&mut self, fg: Color, bg: Color) -> i16 {
        if !self.has_colors {
            return 0;
        }
        let key = (fg, bg);
        if let Some(pair) = self.pairs.get(&key) {
            return *pair;
        }
        let max_pairs = COLOR_PAIRS();
        if i32::from(self.next_pair) >= max_pairs {
            return 0;
        }
        let pair = self.next_pair;
        self.next_pair = self.next_pair.saturating_add(1);
        init_pair(pair, to_curses_color(fg), to_curses_color(bg));
        self.pairs.insert(key, pair);
        pair
    }

    /// Clears a region of a single line by writing spaces.
    ///
    /// # Arguments
    /// - `y` (u16): Line index.
    /// - `start` (u16): Starting column.
    /// - `end` (u16): Ending column (exclusive).
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the clear operation.
    fn clear_line_region(&mut self, y: u16, start: u16, end: u16) -> io::Result<()> {
        if end <= start {
            return Ok(());
        }
        let width = end.saturating_sub(start) as usize;
        let blanks = " ".repeat(width);
        self.window
            .mvaddnstr(y as i32, start as i32, &blanks, width as i32);
        Ok(())
    }

    /// Returns the current terminal size.
    ///
    /// # Returns
    /// - `Size`: Current size in columns and rows.
    fn current_size(&self) -> Size {
        let (rows, cols) = self.window.get_max_yx();
        Size::new(cols.max(0) as u16, rows.max(0) as u16)
    }
}

impl Backend for CursesBackend {
    /// Draws the provided cell updates to the terminal.
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
        let mut last_attr: Option<chtype> = None;
        for (x, y, cell) in content {
            if cell.skip {
                continue;
            }
            let attr = self.attr_for_cell(cell);
            if last_attr != Some(attr) {
                self.window.attrset(attr);
                last_attr = Some(attr);
            }
            let symbol = cell.symbol();
            if !symbol.is_empty() {
                self.window
                    .mvaddnstr(y as i32, x as i32, symbol, symbol.len() as i32);
            }
        }
        Ok(())
    }

    /// Hides the cursor.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the cursor operation.
    fn hide_cursor(&mut self) -> io::Result<()> {
        let result = curs_set(0);
        if result == ERR {
            return Err(io::Error::other("failed to hide cursor"));
        }
        Ok(())
    }

    /// Shows the cursor.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the cursor operation.
    fn show_cursor(&mut self) -> io::Result<()> {
        let result = curs_set(1);
        if result == ERR {
            return Err(io::Error::other("failed to show cursor"));
        }
        Ok(())
    }

    /// Gets the current cursor position.
    ///
    /// # Returns
    /// - `io::Result<Position>`: Current cursor position.
    fn get_cursor_position(&mut self) -> io::Result<Position> {
        let (y, x) = self.window.get_cur_yx();
        Ok(Position {
            x: x.max(0) as u16,
            y: y.max(0) as u16,
        })
    }

    /// Sets the cursor position.
    ///
    /// # Arguments
    /// - `position` (Position): New cursor position.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the cursor operation.
    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let position = position.into();
        self.window.mv(position.y as i32, position.x as i32);
        Ok(())
    }

    /// Clears the entire screen.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the clear operation.
    fn clear(&mut self) -> io::Result<()> {
        self.window.clear();
        Ok(())
    }

    /// Clears a region based on the clear type.
    ///
    /// # Arguments
    /// - `clear_type` (ClearType): Region to clear.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the clear operation.
    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        if clear_type == ClearType::All {
            return self.clear();
        }
        let size = self.current_size();
        let Position { x, y } = self.get_cursor_position()?;
        match clear_type {
            ClearType::AfterCursor => {
                self.clear_line_region(y, x, size.width)?;
                for row in (y.saturating_add(1))..size.height {
                    self.clear_line_region(row, 0, size.width)?;
                }
            }
            ClearType::BeforeCursor => {
                for row in 0..y {
                    self.clear_line_region(row, 0, size.width)?;
                }
                self.clear_line_region(y, 0, x.saturating_add(1))?;
            }
            ClearType::CurrentLine => {
                self.clear_line_region(y, 0, size.width)?;
            }
            ClearType::UntilNewLine => {
                self.clear_line_region(y, x, size.width)?;
            }
            ClearType::All => {}
        }
        Ok(())
    }

    /// Returns the terminal size.
    ///
    /// # Returns
    /// - `io::Result<Size>`: Current terminal size.
    fn size(&self) -> io::Result<Size> {
        Ok(self.current_size())
    }

    /// Returns the terminal size in rows/columns and pixels.
    ///
    /// # Returns
    /// - `io::Result<WindowSize>`: Window size details.
    fn window_size(&mut self) -> io::Result<WindowSize> {
        let size = self.current_size();
        Ok(WindowSize {
            columns_rows: size,
            pixels: Size::new(0, 0),
        })
    }

    /// Flushes any pending output to the terminal.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the flush operation.
    fn flush(&mut self) -> io::Result<()> {
        self.window.refresh();
        Ok(())
    }

    /// Appends blank lines to the screen.
    ///
    /// # Arguments
    /// - `n` (u16): Number of lines to append.
    ///
    /// # Returns
    /// - `io::Result<()>`: Result of the append operation.
    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        for _ in 0..n {
            self.window.addstr("\n");
        }
        Ok(())
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
        if line_count == 0 {
            return Ok(());
        }
        let size = self.current_size();
        let start = region.start.min(size.height);
        let end = region.end.min(size.height);
        if start >= end {
            return Ok(());
        }
        self.window
            .setscrreg(start as i32, end.saturating_sub(1) as i32);
        self.window.mv(start as i32, 0);
        self.window.insdelln(-(line_count as i32));
        if size.height > 0 {
            self.window
                .setscrreg(0, size.height.saturating_sub(1) as i32);
        }
        Ok(())
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
        if line_count == 0 {
            return Ok(());
        }
        let size = self.current_size();
        let start = region.start.min(size.height);
        let end = region.end.min(size.height);
        if start >= end {
            return Ok(());
        }
        self.window
            .setscrreg(start as i32, end.saturating_sub(1) as i32);
        self.window.mv(start as i32, 0);
        self.window.insdelln(line_count as i32);
        if size.height > 0 {
            self.window
                .setscrreg(0, size.height.saturating_sub(1) as i32);
        }
        Ok(())
    }
}

impl Drop for CursesBackend {
    /// Restores the terminal when the backend is dropped.
    fn drop(&mut self) {
        endwin();
    }
}

/// Converts a ratatui color into a curses color value.
///
/// # Arguments
/// - `color` (Color): Ratatui color value.
///
/// # Returns
/// - `i16`: Curses color constant.
fn to_curses_color(color: Color) -> i16 {
    match color {
        Color::Reset => -1,
        Color::Black => COLOR_BLACK,
        Color::Red | Color::LightRed => COLOR_RED,
        Color::Green | Color::LightGreen => COLOR_GREEN,
        Color::Yellow | Color::LightYellow => COLOR_YELLOW,
        Color::Blue | Color::LightBlue => COLOR_BLUE,
        Color::Magenta | Color::LightMagenta => COLOR_MAGENTA,
        Color::Cyan | Color::LightCyan => COLOR_CYAN,
        Color::Gray | Color::White | Color::DarkGray => COLOR_WHITE,
        Color::Rgb(_, _, _) | Color::Indexed(_) => -1,
    }
}
