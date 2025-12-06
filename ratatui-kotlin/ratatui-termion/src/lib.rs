// show the feature flags in the generated documentation
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/ratatui/ratatui/main/assets/logo.png",
    html_favicon_url = "https://raw.githubusercontent.com/ratatui/ratatui/main/assets/favicon.ico"
)]
#![warn(missing_docs)]
//! This module provides the [`TermionBackend`] implementation for the [`Backend`] trait. It uses
//! the [Termion] crate to interact with the terminal.
//!
//! [`Backend`]: ratatui_core::backend::Backend
//! [Termion]: https://docs.rs/termion
//!
//! # Crate Organization
//!
//! `ratatui-termion` is part of the Ratatui workspace that was modularized in version 0.30.0.
//! This crate provides the [Termion] backend implementation for Ratatui.
//!
//! **When to use `ratatui-termion`:**
//!
//! - You need fine-grained control over dependencies
//! - Building a widget library that needs backend functionality
//! - You want to use only the Termion backend without other backends
//! - You prefer Termion's Unix-focused approach
//!
//! **When to use the main [`ratatui`] crate:**
//!
//! - Building applications (recommended - includes termion backend when enabled)
//! - You want the convenience of having everything available
//!
//! For detailed information about the workspace organization, see [ARCHITECTURE.md].
//!
//! [`ratatui`]: https://crates.io/crates/ratatui
//! [ARCHITECTURE.md]: https://github.com/ratatui/ratatui/blob/main/ARCHITECTURE.md
#![cfg_attr(feature = "document-features", doc = "\n## Features")]
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]

use std::fmt;
use std::io::{self, Write};

use ratatui_core::backend::{Backend, ClearType, WindowSize};
use ratatui_core::buffer::Cell;
use ratatui_core::layout::{Position, Size};
use ratatui_core::style::{Color, Modifier, Style};
pub use termion;
use termion::color::Color as _;
use termion::{color as tcolor, style as tstyle};

/// A [`Backend`] implementation that uses [Termion] to render to the terminal.
///
/// The `TermionBackend` struct is a wrapper around a writer implementing [`Write`], which is used
/// to send commands to the terminal. It provides methods for drawing content, manipulating the
/// cursor, and clearing the terminal screen.
///
/// Most applications should not call the methods on `TermionBackend` directly, but will instead
/// use the [`Terminal`] struct, which provides a more ergonomic interface.
///
/// Usually applications will enable raw mode and switch to alternate screen mode when starting.
/// This is done by calling [`IntoRawMode::into_raw_mode()`] and
/// [`IntoAlternateScreen::into_alternate_screen()`] on the writer before creating the backend.
/// This is not done automatically by the backend because it is possible that the application may
/// want to use the terminal for other purposes (like showing help text) before entering alternate
/// screen mode. This backend automatically disable raw mode and switches back to the primary
/// screen when the writer is dropped.
///
/// # Example
///
/// ```rust,ignore
/// use std::io::{stderr, stdout};
///
/// use ratatui::Terminal;
/// use ratatui::backend::TermionBackend;
/// use ratatui::termion::raw::IntoRawMode;
/// use ratatui::termion::screen::IntoAlternateScreen;
///
/// let writer = stdout().into_raw_mode()?.into_alternate_screen()?;
/// let mut backend = TermionBackend::new(writer);
/// // or
/// let writer = stderr().into_raw_mode()?.into_alternate_screen()?;
/// let backend = TermionBackend::new(stderr());
/// let mut terminal = Terminal::new(backend)?;
///
/// terminal.clear()?;
/// terminal.draw(|frame| {
///     // -- snip --
/// })?;
/// # std::io::Result::Ok(())
/// ```
///
/// [`IntoRawMode::into_raw_mode()`]: termion::raw::IntoRawMode
/// [`IntoAlternateScreen::into_alternate_screen()`]: termion::screen::IntoAlternateScreen
/// [`Terminal`]: ratatui_core::terminal::Terminal
/// [Termion]: https://docs.rs/termion
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct TermionBackend<W>
where
    W: Write,
{
    writer: W,
}

impl<W> TermionBackend<W>
where
    W: Write,
{
    /// Creates a new Termion backend with the given writer.
    ///
    /// Most applications will use either [`stdout`](std::io::stdout) or
    /// [`stderr`](std::io::stderr) as writer. See the [FAQ] to determine which one to use.
    ///
    /// [FAQ]: https://ratatui.rs/faq/#should-i-use-stdout-or-stderr
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::io::stdout;
    ///
    /// use ratatui::backend::TermionBackend;
    ///
    /// let backend = TermionBackend::new(stdout());
    /// ```
    pub const fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Gets the writer.
    #[instability::unstable(
        feature = "backend-writer",
        issue = "https://github.com/ratatui/ratatui/pull/991"
    )]
    pub const fn writer(&self) -> &W {
        &self.writer
    }

    /// Gets the writer as a mutable reference.
    /// Note: writing to the writer may cause incorrect output after the write. This is due to the
    /// way that the Terminal implements diffing Buffers.
    #[instability::unstable(
        feature = "backend-writer",
        issue = "https://github.com/ratatui/ratatui/pull/991"
    )]
    pub const fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }
}

impl<W> Write for TermionBackend<W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl<W> Backend for TermionBackend<W>
where
    W: Write,
{
    type Error = io::Error;

    fn clear(&mut self) -> io::Result<()> {
        self.clear_region(ClearType::All)
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        match clear_type {
            ClearType::All => write!(self.writer, "{}", termion::clear::All)?,
            ClearType::AfterCursor => write!(self.writer, "{}", termion::clear::AfterCursor)?,
            ClearType::BeforeCursor => write!(self.writer, "{}", termion::clear::BeforeCursor)?,
            ClearType::CurrentLine => write!(self.writer, "{}", termion::clear::CurrentLine)?,
            ClearType::UntilNewLine => write!(self.writer, "{}", termion::clear::UntilNewline)?,
        }
        self.writer.flush()
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        for _ in 0..n {
            writeln!(self.writer)?;
        }
        self.writer.flush()
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        write!(self.writer, "{}", termion::cursor::Hide)?;
        self.writer.flush()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        write!(self.writer, "{}", termion::cursor::Show)?;
        self.writer.flush()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        termion::cursor::DetectCursorPos::cursor_pos(&mut self.writer)
            .map(|(x, y)| Position { x: x - 1, y: y - 1 })
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let Position { x, y } = position.into();
        write!(self.writer, "{}", termion::cursor::Goto(x + 1, y + 1))?;
        self.writer.flush()
    }

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        use std::fmt::Write;

        let mut string = String::with_capacity(content.size_hint().0 * 3);
        let mut fg = Color::Reset;
        let mut bg = Color::Reset;
        let mut modifier = Modifier::empty();
        let mut last_pos: Option<Position> = None;
        for (x, y, cell) in content {
            // Move the cursor if the previous location was not (x - 1, y)
            if !matches!(last_pos, Some(p) if x == p.x + 1 && y == p.y) {
                write!(string, "{}", termion::cursor::Goto(x + 1, y + 1)).unwrap();
            }
            last_pos = Some(Position { x, y });
            if cell.modifier != modifier {
                write!(
                    string,
                    "{}",
                    ModifierDiff {
                        from: modifier,
                        to: cell.modifier
                    }
                )
                .unwrap();
                modifier = cell.modifier;
            }
            if cell.fg != fg {
                write!(string, "{}", Fg(cell.fg)).unwrap();
                fg = cell.fg;
            }
            if cell.bg != bg {
                write!(string, "{}", Bg(cell.bg)).unwrap();
                bg = cell.bg;
            }
            string.push_str(cell.symbol());
        }
        write!(
            self.writer,
            "{string}{}{}{}",
            Fg(Color::Reset),
            Bg(Color::Reset),
            termion::style::Reset,
        )
    }

    fn size(&self) -> io::Result<Size> {
        let terminal = termion::terminal_size()?;
        Ok(Size::new(terminal.0, terminal.1))
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        Ok(WindowSize {
            columns_rows: termion::terminal_size()?.into(),
            pixels: termion::terminal_size_pixels()?.into(),
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    #[cfg(feature = "scrolling-regions")]
    fn scroll_region_up(&mut self, region: std::ops::Range<u16>, amount: u16) -> io::Result<()> {
        write!(
            self.writer,
            "{}{}{}",
            SetRegion(region.start.saturating_add(1), region.end),
            termion::scroll::Up(amount),
            ResetRegion,
        )?;
        self.writer.flush()
    }

    #[cfg(feature = "scrolling-regions")]
    fn scroll_region_down(&mut self, region: std::ops::Range<u16>, amount: u16) -> io::Result<()> {
        write!(
            self.writer,
            "{}{}{}",
            SetRegion(region.start.saturating_add(1), region.end),
            termion::scroll::Down(amount),
            ResetRegion,
        )?;
        self.writer.flush()
    }
}
struct Fg(Color);

struct Bg(Color);

/// The `ModifierDiff` struct is used to calculate the difference between two `Modifier`
/// values. This is useful when updating the terminal display, as it allows for more
/// efficient updates by only sending the necessary changes.
struct ModifierDiff {
    from: Modifier,
    to: Modifier,
}

impl fmt::Display for Fg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Color::Reset => termion::color::Reset.write_fg(f),
            Color::Black => termion::color::Black.write_fg(f),
            Color::Red => termion::color::Red.write_fg(f),
            Color::Green => termion::color::Green.write_fg(f),
            Color::Yellow => termion::color::Yellow.write_fg(f),
            Color::Blue => termion::color::Blue.write_fg(f),
            Color::Magenta => termion::color::Magenta.write_fg(f),
            Color::Cyan => termion::color::Cyan.write_fg(f),
            Color::Gray => termion::color::White.write_fg(f),
            Color::DarkGray => termion::color::LightBlack.write_fg(f),
            Color::LightRed => termion::color::LightRed.write_fg(f),
            Color::LightGreen => termion::color::LightGreen.write_fg(f),
            Color::LightBlue => termion::color::LightBlue.write_fg(f),
            Color::LightYellow => termion::color::LightYellow.write_fg(f),
            Color::LightMagenta => termion::color::LightMagenta.write_fg(f),
            Color::LightCyan => termion::color::LightCyan.write_fg(f),
            Color::White => termion::color::LightWhite.write_fg(f),
            Color::Indexed(i) => termion::color::AnsiValue(i).write_fg(f),
            Color::Rgb(r, g, b) => termion::color::Rgb(r, g, b).write_fg(f),
        }
    }
}
impl fmt::Display for Bg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Color::Reset => termion::color::Reset.write_bg(f),
            Color::Black => termion::color::Black.write_bg(f),
            Color::Red => termion::color::Red.write_bg(f),
            Color::Green => termion::color::Green.write_bg(f),
            Color::Yellow => termion::color::Yellow.write_bg(f),
            Color::Blue => termion::color::Blue.write_bg(f),
            Color::Magenta => termion::color::Magenta.write_bg(f),
            Color::Cyan => termion::color::Cyan.write_bg(f),
            Color::Gray => termion::color::White.write_bg(f),
            Color::DarkGray => termion::color::LightBlack.write_bg(f),
            Color::LightRed => termion::color::LightRed.write_bg(f),
            Color::LightGreen => termion::color::LightGreen.write_bg(f),
            Color::LightBlue => termion::color::LightBlue.write_bg(f),
            Color::LightYellow => termion::color::LightYellow.write_bg(f),
            Color::LightMagenta => termion::color::LightMagenta.write_bg(f),
            Color::LightCyan => termion::color::LightCyan.write_bg(f),
            Color::White => termion::color::LightWhite.write_bg(f),
            Color::Indexed(i) => termion::color::AnsiValue(i).write_bg(f),
            Color::Rgb(r, g, b) => termion::color::Rgb(r, g, b).write_bg(f),
        }
    }
}

/// A trait for converting a Termion type to a Ratatui type.
///
/// This trait is necessary to avoid the orphan rule, as we cannot implement a trait for a type
/// defined in another crate.
pub trait FromTermion<T> {
    /// Convert the Termion type to the Ratatui type.
    fn from_termion(termion: T) -> Self;
}

/// A trait for converting a Ratatui type to a Termion type.
///
/// This trait is necessary to avoid the orphan rule, as we cannot implement a trait for a type
/// defined in another crate.
pub trait IntoTermion<T> {
    /// Convert the Ratatui type to the Termion type.
    fn into_termion(self) -> T;
}

macro_rules! from_termion_for_color {
    ($termion_color:ident, $color:ident) => {
        impl FromTermion<tcolor::$termion_color> for Color {
            fn from_termion(_: tcolor::$termion_color) -> Self {
                Color::$color
            }
        }

        impl FromTermion<tcolor::Bg<tcolor::$termion_color>> for Style {
            fn from_termion(_: tcolor::Bg<tcolor::$termion_color>) -> Self {
                Style::default().bg(Color::$color)
            }
        }

        impl FromTermion<tcolor::Fg<tcolor::$termion_color>> for Style {
            fn from_termion(_: tcolor::Fg<tcolor::$termion_color>) -> Self {
                Style::default().fg(Color::$color)
            }
        }
    };
}

from_termion_for_color!(Reset, Reset);
from_termion_for_color!(Black, Black);
from_termion_for_color!(Red, Red);
from_termion_for_color!(Green, Green);
from_termion_for_color!(Yellow, Yellow);
from_termion_for_color!(Blue, Blue);
from_termion_for_color!(Magenta, Magenta);
from_termion_for_color!(Cyan, Cyan);
from_termion_for_color!(White, Gray);
from_termion_for_color!(LightBlack, DarkGray);
from_termion_for_color!(LightRed, LightRed);
from_termion_for_color!(LightGreen, LightGreen);
from_termion_for_color!(LightBlue, LightBlue);
from_termion_for_color!(LightYellow, LightYellow);
from_termion_for_color!(LightMagenta, LightMagenta);
from_termion_for_color!(LightCyan, LightCyan);
from_termion_for_color!(LightWhite, White);

impl FromTermion<tcolor::AnsiValue> for Color {
    fn from_termion(value: tcolor::AnsiValue) -> Self {
        Self::Indexed(value.0)
    }
}

impl FromTermion<tcolor::Bg<tcolor::AnsiValue>> for Style {
    fn from_termion(value: tcolor::Bg<tcolor::AnsiValue>) -> Self {
        Self::default().bg(Color::Indexed(value.0.0))
    }
}

impl FromTermion<tcolor::Fg<tcolor::AnsiValue>> for Style {
    fn from_termion(value: tcolor::Fg<tcolor::AnsiValue>) -> Self {
        Self::default().fg(Color::Indexed(value.0.0))
    }
}

impl FromTermion<tcolor::Rgb> for Color {
    fn from_termion(value: tcolor::Rgb) -> Self {
        Self::Rgb(value.0, value.1, value.2)
    }
}

impl FromTermion<tcolor::Bg<tcolor::Rgb>> for Style {
    fn from_termion(value: tcolor::Bg<tcolor::Rgb>) -> Self {
        Self::default().bg(Color::Rgb(value.0.0, value.0.1, value.0.2))
    }
}

impl FromTermion<tcolor::Fg<tcolor::Rgb>> for Style {
    fn from_termion(value: tcolor::Fg<tcolor::Rgb>) -> Self {
        Self::default().fg(Color::Rgb(value.0.0, value.0.1, value.0.2))
    }
}

impl fmt::Display for ModifierDiff {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let remove = self.from - self.to;
        if remove.contains(Modifier::REVERSED) {
            write!(f, "{}", termion::style::NoInvert)?;
        }
        if remove.contains(Modifier::BOLD) {
            // XXX: the termion NoBold flag actually enables double-underline on ECMA-48 compliant
            // terminals, and NoFaint additionally disables bold... so we use this trick to get
            // the right semantics.
            write!(f, "{}", termion::style::NoFaint)?;

            if self.to.contains(Modifier::DIM) {
                write!(f, "{}", termion::style::Faint)?;
            }
        }
        if remove.contains(Modifier::ITALIC) {
            write!(f, "{}", termion::style::NoItalic)?;
        }
        if remove.contains(Modifier::UNDERLINED) {
            write!(f, "{}", termion::style::NoUnderline)?;
        }
        if remove.contains(Modifier::DIM) {
            write!(f, "{}", termion::style::NoFaint)?;

            // XXX: the NoFaint flag additionally disables bold as well, so we need to re-enable it
            // here if we want it.
            if self.to.contains(Modifier::BOLD) {
                write!(f, "{}", termion::style::Bold)?;
            }
        }
        if remove.contains(Modifier::CROSSED_OUT) {
            write!(f, "{}", termion::style::NoCrossedOut)?;
        }
        if remove.contains(Modifier::SLOW_BLINK) || remove.contains(Modifier::RAPID_BLINK) {
            write!(f, "{}", termion::style::NoBlink)?;
        }

        let add = self.to - self.from;
        if add.contains(Modifier::REVERSED) {
            write!(f, "{}", termion::style::Invert)?;
        }
        if add.contains(Modifier::BOLD) {
            write!(f, "{}", termion::style::Bold)?;
        }
        if add.contains(Modifier::ITALIC) {
            write!(f, "{}", termion::style::Italic)?;
        }
        if add.contains(Modifier::UNDERLINED) {
            write!(f, "{}", termion::style::Underline)?;
        }
        if add.contains(Modifier::DIM) {
            write!(f, "{}", termion::style::Faint)?;
        }
        if add.contains(Modifier::CROSSED_OUT) {
            write!(f, "{}", termion::style::CrossedOut)?;
        }
        if add.contains(Modifier::SLOW_BLINK) || add.contains(Modifier::RAPID_BLINK) {
            write!(f, "{}", termion::style::Blink)?;
        }

        Ok(())
    }
}

macro_rules! from_termion_for_modifier {
    ($termion_modifier:ident, $modifier:ident) => {
        impl FromTermion<tstyle::$termion_modifier> for Modifier {
            fn from_termion(_: tstyle::$termion_modifier) -> Self {
                Modifier::$modifier
            }
        }
    };
}

from_termion_for_modifier!(Invert, REVERSED);
from_termion_for_modifier!(Bold, BOLD);
from_termion_for_modifier!(Italic, ITALIC);
from_termion_for_modifier!(Underline, UNDERLINED);
from_termion_for_modifier!(Faint, DIM);
from_termion_for_modifier!(CrossedOut, CROSSED_OUT);
from_termion_for_modifier!(Blink, SLOW_BLINK);

impl FromTermion<termion::style::Reset> for Modifier {
    fn from_termion(_: termion::style::Reset) -> Self {
        Self::empty()
    }
}

/// Set scrolling region.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct SetRegion(pub u16, pub u16);

impl fmt::Display for SetRegion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\x1B[{};{}r", self.0, self.1)
    }
}

/// Reset scrolling region.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ResetRegion;

impl fmt::Display for ResetRegion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\x1B[r")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_termion_color() {
        assert_eq!(Color::from_termion(tcolor::Reset), Color::Reset);
        assert_eq!(Color::from_termion(tcolor::Black), Color::Black);
        assert_eq!(Color::from_termion(tcolor::Red), Color::Red);
        assert_eq!(Color::from_termion(tcolor::Green), Color::Green);
        assert_eq!(Color::from_termion(tcolor::Yellow), Color::Yellow);
        assert_eq!(Color::from_termion(tcolor::Blue), Color::Blue);
        assert_eq!(Color::from_termion(tcolor::Magenta), Color::Magenta);
        assert_eq!(Color::from_termion(tcolor::Cyan), Color::Cyan);
        assert_eq!(Color::from_termion(tcolor::White), Color::Gray);
        assert_eq!(Color::from_termion(tcolor::LightBlack), Color::DarkGray);
        assert_eq!(Color::from_termion(tcolor::LightRed), Color::LightRed);
        assert_eq!(Color::from_termion(tcolor::LightGreen), Color::LightGreen);
        assert_eq!(Color::from_termion(tcolor::LightBlue), Color::LightBlue);
        assert_eq!(Color::from_termion(tcolor::LightYellow), Color::LightYellow);
        assert_eq!(
            Color::from_termion(tcolor::LightMagenta),
            Color::LightMagenta
        );
        assert_eq!(Color::from_termion(tcolor::LightCyan), Color::LightCyan);
        assert_eq!(Color::from_termion(tcolor::LightWhite), Color::White);
        assert_eq!(
            Color::from_termion(tcolor::AnsiValue(31)),
            Color::Indexed(31)
        );
        assert_eq!(
            Color::from_termion(tcolor::Rgb(1, 2, 3)),
            Color::Rgb(1, 2, 3)
        );
    }

    #[test]
    fn from_termion_bg() {
        use tc::Bg;
        use tcolor as tc;

        assert_eq!(
            Style::from_termion(Bg(tc::Reset)),
            Style::new().bg(Color::Reset)
        );
        assert_eq!(Style::from_termion(Bg(tc::Black)), Style::new().on_black());
        assert_eq!(Style::from_termion(Bg(tc::Red)), Style::new().on_red());
        assert_eq!(Style::from_termion(Bg(tc::Green)), Style::new().on_green());
        assert_eq!(
            Style::from_termion(Bg(tc::Yellow)),
            Style::new().on_yellow()
        );
        assert_eq!(Style::from_termion(Bg(tc::Blue)), Style::new().on_blue());
        assert_eq!(
            Style::from_termion(Bg(tc::Magenta)),
            Style::new().on_magenta()
        );
        assert_eq!(Style::from_termion(Bg(tc::Cyan)), Style::new().on_cyan());
        assert_eq!(Style::from_termion(Bg(tc::White)), Style::new().on_gray());
        assert_eq!(
            Style::from_termion(Bg(tc::LightBlack)),
            Style::new().on_dark_gray()
        );
        assert_eq!(
            Style::from_termion(Bg(tc::LightRed)),
            Style::new().on_light_red()
        );
        assert_eq!(
            Style::from_termion(Bg(tc::LightGreen)),
            Style::new().on_light_green()
        );
        assert_eq!(
            Style::from_termion(Bg(tc::LightBlue)),
            Style::new().on_light_blue()
        );
        assert_eq!(
            Style::from_termion(Bg(tc::LightYellow)),
            Style::new().on_light_yellow()
        );
        assert_eq!(
            Style::from_termion(Bg(tc::LightMagenta)),
            Style::new().on_light_magenta()
        );
        assert_eq!(
            Style::from_termion(Bg(tc::LightCyan)),
            Style::new().on_light_cyan()
        );
        assert_eq!(
            Style::from_termion(Bg(tc::LightWhite)),
            Style::new().on_white()
        );
        assert_eq!(
            Style::from_termion(Bg(tc::AnsiValue(31))),
            Style::new().bg(Color::Indexed(31))
        );
        assert_eq!(
            Style::from_termion(Bg(tc::Rgb(1, 2, 3))),
            Style::new().bg(Color::Rgb(1, 2, 3))
        );
    }

    #[test]
    fn from_termion_fg() {
        use tc::Fg;
        use tcolor as tc;

        assert_eq!(
            Style::from_termion(Fg(tc::Reset)),
            Style::new().fg(Color::Reset)
        );
        assert_eq!(Style::from_termion(Fg(tc::Black)), Style::new().black());
        assert_eq!(Style::from_termion(Fg(tc::Red)), Style::new().red());
        assert_eq!(Style::from_termion(Fg(tc::Green)), Style::new().green());
        assert_eq!(Style::from_termion(Fg(tc::Yellow)), Style::new().yellow());
        assert_eq!(Style::from_termion(Fg(tc::Blue)), Style::default().blue());
        assert_eq!(
            Style::from_termion(Fg(tc::Magenta)),
            Style::default().magenta()
        );
        assert_eq!(Style::from_termion(Fg(tc::Cyan)), Style::default().cyan());
        assert_eq!(Style::from_termion(Fg(tc::White)), Style::default().gray());
        assert_eq!(
            Style::from_termion(Fg(tc::LightBlack)),
            Style::new().dark_gray()
        );
        assert_eq!(
            Style::from_termion(Fg(tc::LightRed)),
            Style::new().light_red()
        );
        assert_eq!(
            Style::from_termion(Fg(tc::LightGreen)),
            Style::new().light_green()
        );
        assert_eq!(
            Style::from_termion(Fg(tc::LightBlue)),
            Style::new().light_blue()
        );
        assert_eq!(
            Style::from_termion(Fg(tc::LightYellow)),
            Style::new().light_yellow()
        );
        assert_eq!(
            Style::from_termion(Fg(tc::LightMagenta)),
            Style::new().light_magenta()
        );
        assert_eq!(
            Style::from_termion(Fg(tc::LightCyan)),
            Style::new().light_cyan()
        );
        assert_eq!(
            Style::from_termion(Fg(tc::LightWhite)),
            Style::new().white()
        );
        assert_eq!(
            Style::from_termion(Fg(tc::AnsiValue(31))),
            Style::default().fg(Color::Indexed(31))
        );
        assert_eq!(
            Style::from_termion(Fg(tc::Rgb(1, 2, 3))),
            Style::default().fg(Color::Rgb(1, 2, 3))
        );
    }

    #[test]
    fn from_termion_style() {
        assert_eq!(Modifier::from_termion(tstyle::Invert), Modifier::REVERSED);
        assert_eq!(Modifier::from_termion(tstyle::Bold), Modifier::BOLD);
        assert_eq!(Modifier::from_termion(tstyle::Italic), Modifier::ITALIC);
        assert_eq!(
            Modifier::from_termion(tstyle::Underline),
            Modifier::UNDERLINED
        );
        assert_eq!(Modifier::from_termion(tstyle::Faint), Modifier::DIM);
        assert_eq!(
            Modifier::from_termion(tstyle::CrossedOut),
            Modifier::CROSSED_OUT
        );
        assert_eq!(Modifier::from_termion(tstyle::Blink), Modifier::SLOW_BLINK);
        assert_eq!(Modifier::from_termion(tstyle::Reset), Modifier::empty());
    }
}
