use std::cell::RefCell;
use std::fmt::{self};
use std::io::Write;
use std::io::{self};
use std::rc::Rc;

use ratatui::prelude::CrosstermBackend;

use ratatui::backend::Backend;
use ratatui::backend::ClearType;
use ratatui::backend::WindowSize;
use ratatui::buffer::Cell;
use ratatui::layout::Position;
use ratatui::layout::Size;

/// A writer that feeds bytes to a vt100::Parser
struct VT100Writer {
    parser: Rc<RefCell<vt100::Parser>>,
}

impl Write for VT100Writer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.parser.borrow_mut().process(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// This wraps a CrosstermBackend and a vt100::Parser to mock
/// a "real" terminal.
///
/// Importantly, this wrapper avoids calling any crossterm methods
/// which write to stdout regardless of the writer. This includes:
/// - getting the terminal size
/// - getting the cursor position
pub struct VT100Backend {
    parser: Rc<RefCell<vt100::Parser>>,
    backend: CrosstermBackend<VT100Writer>,
}

impl VT100Backend {
    /// Creates a new `TestBackend` with the specified width and height.
    pub fn new(width: u16, height: u16) -> Self {
        let parser = Rc::new(RefCell::new(vt100::Parser::new(height, width, 0)));
        let writer = VT100Writer {
            parser: Rc::clone(&parser),
        };
        let backend = CrosstermBackend::new(writer);
        Self { parser, backend }
    }

    pub fn vt100(&self) -> std::cell::Ref<'_, vt100::Parser> {
        self.parser.borrow()
    }
}

impl Write for VT100Backend {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.backend.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        std::io::Write::flush(&mut self.backend)
    }
}

impl fmt::Display for VT100Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.vt100().screen().contents())
    }
}

impl Backend for VT100Backend {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.backend.draw(content)?;
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.backend.hide_cursor()?;
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.backend.show_cursor()?;
        Ok(())
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        Ok(self.vt100().screen().cursor_position().into())
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.backend.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.backend.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.backend.clear_region(clear_type)
    }

    fn append_lines(&mut self, line_count: u16) -> io::Result<()> {
        self.backend.append_lines(line_count)
    }

    fn size(&self) -> io::Result<Size> {
        Ok(self.vt100().screen().size().into())
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        Ok(WindowSize {
            columns_rows: self.vt100().screen().size().into(),
            // Arbitrary size, we don't rely on this in testing.
            pixels: Size {
                width: 640,
                height: 480,
            },
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        std::io::Write::flush(&mut self.backend)
    }

    fn scroll_region_up(&mut self, region: std::ops::Range<u16>, scroll_by: u16) -> io::Result<()> {
        self.backend.scroll_region_up(region, scroll_by)
    }

    fn scroll_region_down(
        &mut self,
        region: std::ops::Range<u16>,
        scroll_by: u16,
    ) -> io::Result<()> {
        self.backend.scroll_region_down(region, scroll_by)
    }
}
