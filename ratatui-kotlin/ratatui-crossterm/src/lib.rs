// show the feature flags in the generated documentation
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/ratatui/ratatui/main/assets/logo.png",
    html_favicon_url = "https://raw.githubusercontent.com/ratatui/ratatui/main/assets/favicon.ico"
)]
#![warn(missing_docs)]
//! This crate provides [`CrosstermBackend`], an implementation of the [`Backend`] trait for the
//! [Ratatui] library. It uses the [Crossterm] library for all terminal manipulation.
//!
//! ## Crossterm Version and Re-export
//!
//! `ratatui-crossterm` requires you to specify a version of the [Crossterm] library to be used.
//! This is managed via feature flags. The highest enabled feature flag of the available
//! `crossterm_0_xx` features (e.g., `crossterm_0_28`, `crossterm_0_29`) takes precedence. These
//! features determine which version of Crossterm is compiled and used by the backend. Feature
//! unification may mean that any crate in your dependency graph that chooses to depend on a
//! specific version of Crossterm may be affected by the feature flags you enable.
//!
//! Ratatui will support at least the two most recent versions of Crossterm (though we may increase
//! this if crossterm release cadence increases). We will remove support for older versions in major
//! (0.x) releases of `ratatui-crossterm`, and we may add support for newer versions in minor
//! (0.x.y) releases.
//!
//! To promote interoperability within the [Ratatui] ecosystem, the selected Crossterm crate is
//! re-exported as `ratatui_crossterm::crossterm`. This re-export is essential for authors of widget
//! libraries or any applications that need to perform direct Crossterm operations while ensuring
//! compatibility with the version used by `ratatui-crossterm`. By using
//! `ratatui_crossterm::crossterm` for such operations, developers can avoid version conflicts and
//! ensure that all parts of their application use a consistent set of Crossterm types and
//! functions.
//!
//! For example, if your application's `Cargo.toml` enables the `crossterm_0_29` feature for
//! `ratatui-crossterm`, then any code using `ratatui_crossterm::crossterm` will refer to the 0.29
//! version of Crossterm.
//!
//! For more information on how to use the backend, see the documentation for the
//! [`CrosstermBackend`] struct.
//!
//! [Ratatui]: https://ratatui.rs
//! [Crossterm]: https://crates.io/crates/crossterm
//! [`Backend`]: ratatui_core::backend::Backend
//!
//! # Crate Organization
//!
//! `ratatui-crossterm` is part of the Ratatui workspace that was modularized in version 0.30.0.
//! This crate provides the [Crossterm] backend implementation for Ratatui.
//!
//! **When to use `ratatui-crossterm`:**
//!
//! - You need fine-grained control over dependencies
//! - Building a widget library that needs backend functionality
//! - You want to use only the Crossterm backend without other backends
//!
//! **When to use the main [`ratatui`] crate:**
//!
//! - Building applications (recommended - includes crossterm backend by default)
//! - You want the convenience of having everything available
//!
//! For detailed information about the workspace organization, see [ARCHITECTURE.md].
//!
//! [`ratatui`]: https://crates.io/crates/ratatui
//! [ARCHITECTURE.md]: https://github.com/ratatui/ratatui/blob/main/ARCHITECTURE.md
#![cfg_attr(feature = "document-features", doc = "\n## Features")]
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]

use std::io::{self, Write};

use crossterm::cursor::{Hide, MoveTo, Show};
#[cfg(feature = "underline-color")]
use crossterm::style::SetUnderlineColor;
use crossterm::style::{
    Attribute as CrosstermAttribute, Attributes as CrosstermAttributes, Color as CrosstermColor,
    Colors as CrosstermColors, ContentStyle, Print, SetAttribute, SetBackgroundColor, SetColors,
    SetForegroundColor,
};
use crossterm::terminal::{self, Clear};
use crossterm::{execute, queue};
cfg_if::cfg_if! {
    // Re-export the selected Crossterm crate making sure to choose the latest version. We do this
    // to make it possible to easily enable all features when compiling `ratatui-crossterm`.
    if #[cfg(feature = "crossterm_0_29")] {
        pub use crossterm_0_29 as crossterm;
    } else if #[cfg(feature = "crossterm_0_28")] {
        pub use crossterm_0_28 as crossterm;
    } else {
        compile_error!(
            "At least one crossterm feature must be enabled. See the crate docs for more information."
        );
    }
}
use ratatui_core::backend::{Backend, ClearType, WindowSize};
use ratatui_core::buffer::Cell;
use ratatui_core::layout::{Position, Size};
use ratatui_core::style::{Color, Modifier, Style};

/// A [`Backend`] implementation that uses [Crossterm] to render to the terminal.
///
/// The `CrosstermBackend` struct is a wrapper around a writer implementing [`Write`], which is
/// used to send commands to the terminal. It provides methods for drawing content, manipulating
/// the cursor, and clearing the terminal screen.
///
/// Most applications should not call the methods on `CrosstermBackend` directly, but will instead
/// use the [`Terminal`] struct, which provides a more ergonomic interface.
///
/// Usually applications will enable raw mode and switch to alternate screen mode after creating
/// a `CrosstermBackend`. This is done by calling [`crossterm::terminal::enable_raw_mode`] and
/// [`crossterm::terminal::EnterAlternateScreen`] (and the corresponding disable/leave functions
/// when the application exits). This is not done automatically by the backend because it is
/// possible that the application may want to use the terminal for other purposes (like showing
/// help text) before entering alternate screen mode.
///
/// # Example
///
/// ```rust,ignore
/// use std::io::{stderr, stdout};
///
/// use crossterm::ExecutableCommand;
/// use crossterm::terminal::{
///     EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
/// };
/// use ratatui::Terminal;
/// use ratatui::backend::CrosstermBackend;
///
/// let mut backend = CrosstermBackend::new(stdout());
/// // or
/// let backend = CrosstermBackend::new(stderr());
/// let mut terminal = Terminal::new(backend)?;
///
/// enable_raw_mode()?;
/// stdout().execute(EnterAlternateScreen)?;
///
/// terminal.clear()?;
/// terminal.draw(|frame| {
///     // -- snip --
/// })?;
///
/// stdout().execute(LeaveAlternateScreen)?;
/// disable_raw_mode()?;
///
/// # std::io::Result::Ok(())
/// ```
///
/// See the the [Examples] directory for more examples. See the [`backend`] module documentation
/// for more details on raw mode and alternate screen.
///
/// [`Write`]: std::io::Write
/// [`Terminal`]: https://docs.rs/ratatui/latest/ratatui/struct.Terminal.html
/// [`backend`]: ratatui_core::backend
/// [Crossterm]: https://crates.io/crates/crossterm
/// [Examples]: https://github.com/ratatui/ratatui/tree/main/ratatui/examples/README.md
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct CrosstermBackend<W: Write> {
    /// The writer used to send commands to the terminal.
    writer: W,
}

impl<W> CrosstermBackend<W>
where
    W: Write,
{
    /// Creates a new `CrosstermBackend` with the given writer.
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
    /// use ratatui::backend::CrosstermBackend;
    ///
    /// let backend = CrosstermBackend::new(stdout());
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
    ///
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

impl<W> Write for CrosstermBackend<W>
where
    W: Write,
{
    /// Writes a buffer of bytes to the underlying buffer.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }

    /// Flushes the underlying buffer.
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl<W> Backend for CrosstermBackend<W>
where
    W: Write,
{
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut fg = Color::Reset;
        let mut bg = Color::Reset;
        #[cfg(feature = "underline-color")]
        let mut underline_color = Color::Reset;
        let mut modifier = Modifier::empty();
        let mut last_pos: Option<Position> = None;
        for (x, y, cell) in content {
            // Move the cursor if the previous location was not (x - 1, y)
            if !matches!(last_pos, Some(p) if x == p.x + 1 && y == p.y) {
                queue!(self.writer, MoveTo(x, y))?;
            }
            last_pos = Some(Position { x, y });
            if cell.modifier != modifier {
                let diff = ModifierDiff {
                    from: modifier,
                    to: cell.modifier,
                };
                diff.queue(&mut self.writer)?;
                modifier = cell.modifier;
            }
            if cell.fg != fg || cell.bg != bg {
                queue!(
                    self.writer,
                    SetColors(CrosstermColors::new(
                        cell.fg.into_crossterm(),
                        cell.bg.into_crossterm(),
                    ))
                )?;
                fg = cell.fg;
                bg = cell.bg;
            }
            #[cfg(feature = "underline-color")]
            if cell.underline_color != underline_color {
                let color = cell.underline_color.into_crossterm();
                queue!(self.writer, SetUnderlineColor(color))?;
                underline_color = cell.underline_color;
            }

            queue!(self.writer, Print(cell.symbol()))?;
        }

        #[cfg(feature = "underline-color")]
        return queue!(
            self.writer,
            SetForegroundColor(CrosstermColor::Reset),
            SetBackgroundColor(CrosstermColor::Reset),
            SetUnderlineColor(CrosstermColor::Reset),
            SetAttribute(CrosstermAttribute::Reset),
        );
        #[cfg(not(feature = "underline-color"))]
        return queue!(
            self.writer,
            SetForegroundColor(CrosstermColor::Reset),
            SetBackgroundColor(CrosstermColor::Reset),
            SetAttribute(CrosstermAttribute::Reset),
        );
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        execute!(self.writer, Hide)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        execute!(self.writer, Show)
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        crossterm::cursor::position()
            .map(|(x, y)| Position { x, y })
            .map_err(io::Error::other)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let Position { x, y } = position.into();
        execute!(self.writer, MoveTo(x, y))
    }

    fn clear(&mut self) -> io::Result<()> {
        self.clear_region(ClearType::All)
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        execute!(
            self.writer,
            Clear(match clear_type {
                ClearType::All => crossterm::terminal::ClearType::All,
                ClearType::AfterCursor => crossterm::terminal::ClearType::FromCursorDown,
                ClearType::BeforeCursor => crossterm::terminal::ClearType::FromCursorUp,
                ClearType::CurrentLine => crossterm::terminal::ClearType::CurrentLine,
                ClearType::UntilNewLine => crossterm::terminal::ClearType::UntilNewLine,
            })
        )
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        for _ in 0..n {
            queue!(self.writer, Print("\n"))?;
        }
        self.writer.flush()
    }

    fn size(&self) -> io::Result<Size> {
        let (width, height) = terminal::size()?;
        Ok(Size { width, height })
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        let crossterm::terminal::WindowSize {
            columns,
            rows,
            width,
            height,
        } = terminal::window_size()?;
        Ok(WindowSize {
            columns_rows: Size {
                width: columns,
                height: rows,
            },
            pixels: Size { width, height },
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    #[cfg(feature = "scrolling-regions")]
    fn scroll_region_up(&mut self, region: std::ops::Range<u16>, amount: u16) -> io::Result<()> {
        queue!(
            self.writer,
            ScrollUpInRegion {
                first_row: region.start,
                last_row: region.end.saturating_sub(1),
                lines_to_scroll: amount,
            }
        )?;
        self.writer.flush()
    }

    #[cfg(feature = "scrolling-regions")]
    fn scroll_region_down(&mut self, region: std::ops::Range<u16>, amount: u16) -> io::Result<()> {
        queue!(
            self.writer,
            ScrollDownInRegion {
                first_row: region.start,
                last_row: region.end.saturating_sub(1),
                lines_to_scroll: amount,
            }
        )?;
        self.writer.flush()
    }
}

/// A trait for converting a Ratatui type to a Crossterm type.
///
/// This trait is needed for avoiding the orphan rule when implementing `From` for crossterm types
/// once these are moved to a separate crate.
pub trait IntoCrossterm<C> {
    /// Converts the ratatui type to a crossterm type.
    fn into_crossterm(self) -> C;
}

/// A trait for converting a Crossterm type to a Ratatui type.
///
/// This trait is needed for avoiding the orphan rule when implementing `From` for crossterm types
/// once these are moved to a separate crate.
pub trait FromCrossterm<C> {
    /// Converts the crossterm type to a ratatui type.
    fn from_crossterm(value: C) -> Self;
}

impl IntoCrossterm<CrosstermColor> for Color {
    fn into_crossterm(self) -> CrosstermColor {
        match self {
            Self::Reset => CrosstermColor::Reset,
            Self::Black => CrosstermColor::Black,
            Self::Red => CrosstermColor::DarkRed,
            Self::Green => CrosstermColor::DarkGreen,
            Self::Yellow => CrosstermColor::DarkYellow,
            Self::Blue => CrosstermColor::DarkBlue,
            Self::Magenta => CrosstermColor::DarkMagenta,
            Self::Cyan => CrosstermColor::DarkCyan,
            Self::Gray => CrosstermColor::Grey,
            Self::DarkGray => CrosstermColor::DarkGrey,
            Self::LightRed => CrosstermColor::Red,
            Self::LightGreen => CrosstermColor::Green,
            Self::LightBlue => CrosstermColor::Blue,
            Self::LightYellow => CrosstermColor::Yellow,
            Self::LightMagenta => CrosstermColor::Magenta,
            Self::LightCyan => CrosstermColor::Cyan,
            Self::White => CrosstermColor::White,
            Self::Indexed(i) => CrosstermColor::AnsiValue(i),
            Self::Rgb(r, g, b) => CrosstermColor::Rgb { r, g, b },
        }
    }
}

impl FromCrossterm<CrosstermColor> for Color {
    fn from_crossterm(value: CrosstermColor) -> Self {
        match value {
            CrosstermColor::Reset => Self::Reset,
            CrosstermColor::Black => Self::Black,
            CrosstermColor::DarkRed => Self::Red,
            CrosstermColor::DarkGreen => Self::Green,
            CrosstermColor::DarkYellow => Self::Yellow,
            CrosstermColor::DarkBlue => Self::Blue,
            CrosstermColor::DarkMagenta => Self::Magenta,
            CrosstermColor::DarkCyan => Self::Cyan,
            CrosstermColor::Grey => Self::Gray,
            CrosstermColor::DarkGrey => Self::DarkGray,
            CrosstermColor::Red => Self::LightRed,
            CrosstermColor::Green => Self::LightGreen,
            CrosstermColor::Blue => Self::LightBlue,
            CrosstermColor::Yellow => Self::LightYellow,
            CrosstermColor::Magenta => Self::LightMagenta,
            CrosstermColor::Cyan => Self::LightCyan,
            CrosstermColor::White => Self::White,
            CrosstermColor::Rgb { r, g, b } => Self::Rgb(r, g, b),
            CrosstermColor::AnsiValue(v) => Self::Indexed(v),
        }
    }
}

/// The `ModifierDiff` struct is used to calculate the difference between two `Modifier`
/// values. This is useful when updating the terminal display, as it allows for more
/// efficient updates by only sending the necessary changes.
struct ModifierDiff {
    pub from: Modifier,
    pub to: Modifier,
}

impl ModifierDiff {
    fn queue<W>(self, mut w: W) -> io::Result<()>
    where
        W: io::Write,
    {
        //use crossterm::Attribute;
        let removed = self.from - self.to;
        if removed.contains(Modifier::REVERSED) {
            queue!(w, SetAttribute(CrosstermAttribute::NoReverse))?;
        }
        if removed.contains(Modifier::BOLD) || removed.contains(Modifier::DIM) {
            // Bold and Dim are both reset by applying the Normal intensity
            queue!(w, SetAttribute(CrosstermAttribute::NormalIntensity))?;

            // The remaining Bold and Dim attributes must be
            // reapplied after the intensity reset above.
            if self.to.contains(Modifier::DIM) {
                queue!(w, SetAttribute(CrosstermAttribute::Dim))?;
            }

            if self.to.contains(Modifier::BOLD) {
                queue!(w, SetAttribute(CrosstermAttribute::Bold))?;
            }
        }
        if removed.contains(Modifier::ITALIC) {
            queue!(w, SetAttribute(CrosstermAttribute::NoItalic))?;
        }
        if removed.contains(Modifier::UNDERLINED) {
            queue!(w, SetAttribute(CrosstermAttribute::NoUnderline))?;
        }
        if removed.contains(Modifier::CROSSED_OUT) {
            queue!(w, SetAttribute(CrosstermAttribute::NotCrossedOut))?;
        }
        if removed.contains(Modifier::SLOW_BLINK) || removed.contains(Modifier::RAPID_BLINK) {
            queue!(w, SetAttribute(CrosstermAttribute::NoBlink))?;
        }

        let added = self.to - self.from;
        if added.contains(Modifier::REVERSED) {
            queue!(w, SetAttribute(CrosstermAttribute::Reverse))?;
        }
        if added.contains(Modifier::BOLD) {
            queue!(w, SetAttribute(CrosstermAttribute::Bold))?;
        }
        if added.contains(Modifier::ITALIC) {
            queue!(w, SetAttribute(CrosstermAttribute::Italic))?;
        }
        if added.contains(Modifier::UNDERLINED) {
            queue!(w, SetAttribute(CrosstermAttribute::Underlined))?;
        }
        if added.contains(Modifier::DIM) {
            queue!(w, SetAttribute(CrosstermAttribute::Dim))?;
        }
        if added.contains(Modifier::CROSSED_OUT) {
            queue!(w, SetAttribute(CrosstermAttribute::CrossedOut))?;
        }
        if added.contains(Modifier::SLOW_BLINK) {
            queue!(w, SetAttribute(CrosstermAttribute::SlowBlink))?;
        }
        if added.contains(Modifier::RAPID_BLINK) {
            queue!(w, SetAttribute(CrosstermAttribute::RapidBlink))?;
        }

        Ok(())
    }
}

impl FromCrossterm<CrosstermAttribute> for Modifier {
    fn from_crossterm(value: CrosstermAttribute) -> Self {
        // `Attribute*s*` (note the *s*) contains multiple `Attribute` We convert `Attribute` to
        // `Attribute*s*` (containing only 1 value) to avoid implementing the conversion again
        Self::from_crossterm(CrosstermAttributes::from(value))
    }
}

impl FromCrossterm<CrosstermAttributes> for Modifier {
    fn from_crossterm(value: CrosstermAttributes) -> Self {
        let mut res = Self::empty();
        if value.has(CrosstermAttribute::Bold) {
            res |= Self::BOLD;
        }
        if value.has(CrosstermAttribute::Dim) {
            res |= Self::DIM;
        }
        if value.has(CrosstermAttribute::Italic) {
            res |= Self::ITALIC;
        }
        if value.has(CrosstermAttribute::Underlined)
            || value.has(CrosstermAttribute::DoubleUnderlined)
            || value.has(CrosstermAttribute::Undercurled)
            || value.has(CrosstermAttribute::Underdotted)
            || value.has(CrosstermAttribute::Underdashed)
        {
            res |= Self::UNDERLINED;
        }
        if value.has(CrosstermAttribute::SlowBlink) {
            res |= Self::SLOW_BLINK;
        }
        if value.has(CrosstermAttribute::RapidBlink) {
            res |= Self::RAPID_BLINK;
        }
        if value.has(CrosstermAttribute::Reverse) {
            res |= Self::REVERSED;
        }
        if value.has(CrosstermAttribute::Hidden) {
            res |= Self::HIDDEN;
        }
        if value.has(CrosstermAttribute::CrossedOut) {
            res |= Self::CROSSED_OUT;
        }
        res
    }
}

impl FromCrossterm<ContentStyle> for Style {
    fn from_crossterm(value: ContentStyle) -> Self {
        let mut sub_modifier = Modifier::empty();
        if value.attributes.has(CrosstermAttribute::NoBold) {
            sub_modifier |= Modifier::BOLD;
        }
        if value.attributes.has(CrosstermAttribute::NoItalic) {
            sub_modifier |= Modifier::ITALIC;
        }
        if value.attributes.has(CrosstermAttribute::NotCrossedOut) {
            sub_modifier |= Modifier::CROSSED_OUT;
        }
        if value.attributes.has(CrosstermAttribute::NoUnderline) {
            sub_modifier |= Modifier::UNDERLINED;
        }
        if value.attributes.has(CrosstermAttribute::NoHidden) {
            sub_modifier |= Modifier::HIDDEN;
        }
        if value.attributes.has(CrosstermAttribute::NoBlink) {
            sub_modifier |= Modifier::RAPID_BLINK | Modifier::SLOW_BLINK;
        }
        if value.attributes.has(CrosstermAttribute::NoReverse) {
            sub_modifier |= Modifier::REVERSED;
        }

        Self {
            fg: value.foreground_color.map(FromCrossterm::from_crossterm),
            bg: value.background_color.map(FromCrossterm::from_crossterm),
            #[cfg(feature = "underline-color")]
            underline_color: value.underline_color.map(FromCrossterm::from_crossterm),
            add_modifier: Modifier::from_crossterm(value.attributes),
            sub_modifier,
        }
    }
}

/// A command that scrolls the terminal screen a given number of rows up in a specific scrolling
/// region.
///
/// This will hopefully be replaced by a struct in crossterm proper. There are two outstanding
/// crossterm PRs that will address this:
///   - [918](https://github.com/crossterm-rs/crossterm/pull/918)
///   - [923](https://github.com/crossterm-rs/crossterm/pull/923)
#[cfg(feature = "scrolling-regions")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScrollUpInRegion {
    /// The first row of the scrolling region.
    pub first_row: u16,

    /// The last row of the scrolling region.
    pub last_row: u16,

    /// The number of lines to scroll up by.
    pub lines_to_scroll: u16,
}

#[cfg(feature = "scrolling-regions")]
impl crate::crossterm::Command for ScrollUpInRegion {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        if self.lines_to_scroll != 0 {
            // Set a scrolling region that contains just the desired lines.
            write!(
                f,
                crate::crossterm::csi!("{};{}r"),
                self.first_row.saturating_add(1),
                self.last_row.saturating_add(1)
            )?;
            // Scroll the region by the desired count.
            write!(f, crate::crossterm::csi!("{}S"), self.lines_to_scroll)?;
            // Reset the scrolling region to be the whole screen.
            write!(f, crate::crossterm::csi!("r"))?;
        }
        Ok(())
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "ScrollUpInRegion command not supported for winapi",
        ))
    }
}

/// A command that scrolls the terminal screen a given number of rows down in a specific scrolling
/// region.
///
/// This will hopefully be replaced by a struct in crossterm proper. There are two outstanding
/// crossterm PRs that will address this:
///   - [918](https://github.com/crossterm-rs/crossterm/pull/918)
///   - [923](https://github.com/crossterm-rs/crossterm/pull/923)
#[cfg(feature = "scrolling-regions")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScrollDownInRegion {
    /// The first row of the scrolling region.
    pub first_row: u16,

    /// The last row of the scrolling region.
    pub last_row: u16,

    /// The number of lines to scroll down by.
    pub lines_to_scroll: u16,
}

#[cfg(feature = "scrolling-regions")]
impl crate::crossterm::Command for ScrollDownInRegion {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        if self.lines_to_scroll != 0 {
            // Set a scrolling region that contains just the desired lines.
            write!(
                f,
                crate::crossterm::csi!("{};{}r"),
                self.first_row.saturating_add(1),
                self.last_row.saturating_add(1)
            )?;
            // Scroll the region by the desired count.
            write!(f, crate::crossterm::csi!("{}T"), self.lines_to_scroll)?;
            // Reset the scrolling region to be the whole screen.
            write!(f, crate::crossterm::csi!("r"))?;
        }
        Ok(())
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "ScrollDownInRegion command not supported for winapi",
        ))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(CrosstermColor::Reset, Color::Reset)]
    #[case(CrosstermColor::Black, Color::Black)]
    #[case(CrosstermColor::DarkGrey, Color::DarkGray)]
    #[case(CrosstermColor::Red, Color::LightRed)]
    #[case(CrosstermColor::DarkRed, Color::Red)]
    #[case(CrosstermColor::Green, Color::LightGreen)]
    #[case(CrosstermColor::DarkGreen, Color::Green)]
    #[case(CrosstermColor::Yellow, Color::LightYellow)]
    #[case(CrosstermColor::DarkYellow, Color::Yellow)]
    #[case(CrosstermColor::Blue, Color::LightBlue)]
    #[case(CrosstermColor::DarkBlue, Color::Blue)]
    #[case(CrosstermColor::Magenta, Color::LightMagenta)]
    #[case(CrosstermColor::DarkMagenta, Color::Magenta)]
    #[case(CrosstermColor::Cyan, Color::LightCyan)]
    #[case(CrosstermColor::DarkCyan, Color::Cyan)]
    #[case(CrosstermColor::White, Color::White)]
    #[case(CrosstermColor::Grey, Color::Gray)]
    #[case(CrosstermColor::Rgb { r: 0, g: 0, b: 0 }, Color::Rgb(0, 0, 0) )]
    #[case(CrosstermColor::Rgb { r: 10, g: 20, b: 30 }, Color::Rgb(10, 20, 30) )]
    #[case(CrosstermColor::AnsiValue(32), Color::Indexed(32))]
    #[case(CrosstermColor::AnsiValue(37), Color::Indexed(37))]
    fn from_crossterm_color(#[case] crossterm_color: CrosstermColor, #[case] color: Color) {
        assert_eq!(Color::from_crossterm(crossterm_color), color);
    }

    mod modifier {
        use super::*;

        #[rstest]
        #[case(CrosstermAttribute::Reset, Modifier::empty())]
        #[case(CrosstermAttribute::Bold, Modifier::BOLD)]
        #[case(CrosstermAttribute::NoBold, Modifier::empty())]
        #[case(CrosstermAttribute::Italic, Modifier::ITALIC)]
        #[case(CrosstermAttribute::NoItalic, Modifier::empty())]
        #[case(CrosstermAttribute::Underlined, Modifier::UNDERLINED)]
        #[case(CrosstermAttribute::NoUnderline, Modifier::empty())]
        #[case(CrosstermAttribute::OverLined, Modifier::empty())]
        #[case(CrosstermAttribute::NotOverLined, Modifier::empty())]
        #[case(CrosstermAttribute::DoubleUnderlined, Modifier::UNDERLINED)]
        #[case(CrosstermAttribute::Undercurled, Modifier::UNDERLINED)]
        #[case(CrosstermAttribute::Underdotted, Modifier::UNDERLINED)]
        #[case(CrosstermAttribute::Underdashed, Modifier::UNDERLINED)]
        #[case(CrosstermAttribute::Dim, Modifier::DIM)]
        #[case(CrosstermAttribute::NormalIntensity, Modifier::empty())]
        #[case(CrosstermAttribute::CrossedOut, Modifier::CROSSED_OUT)]
        #[case(CrosstermAttribute::NotCrossedOut, Modifier::empty())]
        #[case(CrosstermAttribute::NoUnderline, Modifier::empty())]
        #[case(CrosstermAttribute::SlowBlink, Modifier::SLOW_BLINK)]
        #[case(CrosstermAttribute::RapidBlink, Modifier::RAPID_BLINK)]
        #[case(CrosstermAttribute::Hidden, Modifier::HIDDEN)]
        #[case(CrosstermAttribute::NoHidden, Modifier::empty())]
        #[case(CrosstermAttribute::Reverse, Modifier::REVERSED)]
        #[case(CrosstermAttribute::NoReverse, Modifier::empty())]
        fn from_crossterm_attribute(
            #[case] crossterm_attribute: CrosstermAttribute,
            #[case] ratatui_modifier: Modifier,
        ) {
            assert_eq!(
                Modifier::from_crossterm(crossterm_attribute),
                ratatui_modifier
            );
        }

        #[rstest]
        #[case(&[CrosstermAttribute::Bold], Modifier::BOLD)]
        #[case(&[CrosstermAttribute::Bold, CrosstermAttribute::Italic], Modifier::BOLD | Modifier::ITALIC)]
        #[case(&[CrosstermAttribute::Bold, CrosstermAttribute::NotCrossedOut], Modifier::BOLD)]
        #[case(&[CrosstermAttribute::Dim, CrosstermAttribute::Underdotted], Modifier::DIM | Modifier::UNDERLINED)]
        #[case(&[CrosstermAttribute::Dim, CrosstermAttribute::SlowBlink, CrosstermAttribute::Italic], Modifier::DIM | Modifier::SLOW_BLINK | Modifier::ITALIC)]
        #[case(&[CrosstermAttribute::Hidden, CrosstermAttribute::NoUnderline, CrosstermAttribute::NotCrossedOut], Modifier::HIDDEN)]
        #[case(&[CrosstermAttribute::Reverse], Modifier::REVERSED)]
        #[case(&[CrosstermAttribute::Reset], Modifier::empty())]
        #[case(&[CrosstermAttribute::RapidBlink, CrosstermAttribute::CrossedOut], Modifier::RAPID_BLINK | Modifier::CROSSED_OUT)]
        fn from_crossterm_attributes(
            #[case] crossterm_attributes: &[CrosstermAttribute],
            #[case] ratatui_modifier: Modifier,
        ) {
            assert_eq!(
                Modifier::from_crossterm(CrosstermAttributes::from(crossterm_attributes)),
                ratatui_modifier
            );
        }
    }

    #[rstest]
    #[case(ContentStyle::default(), Style::default())]
    #[case(
        ContentStyle {
            foreground_color: Some(CrosstermColor::DarkYellow),
            ..Default::default()
        },
        Style::default().fg(Color::Yellow)
    )]
    #[case(
        ContentStyle {
            background_color: Some(CrosstermColor::DarkYellow),
            ..Default::default()
        },
        Style::default().bg(Color::Yellow)
    )]
    #[case(
        ContentStyle {
            attributes: CrosstermAttributes::from(CrosstermAttribute::Bold),
            ..Default::default()
        },
        Style::default().add_modifier(Modifier::BOLD)
    )]
    #[case(
        ContentStyle {
            attributes: CrosstermAttributes::from(CrosstermAttribute::NoBold),
            ..Default::default()
        },
        Style::default().remove_modifier(Modifier::BOLD)
    )]
    #[case(
        ContentStyle {
            attributes: CrosstermAttributes::from(CrosstermAttribute::Italic),
            ..Default::default()
        },
        Style::default().add_modifier(Modifier::ITALIC)
    )]
    #[case(
        ContentStyle {
            attributes: CrosstermAttributes::from(CrosstermAttribute::NoItalic),
            ..Default::default()
        },
        Style::default().remove_modifier(Modifier::ITALIC)
    )]
    #[case(
        ContentStyle {
            attributes: CrosstermAttributes::from(
                [CrosstermAttribute::Bold, CrosstermAttribute::Italic].as_ref()
            ),
            ..Default::default()
        },
        Style::default()
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::ITALIC)
    )]
    #[case(
        ContentStyle {
            attributes: CrosstermAttributes::from(
                [CrosstermAttribute::NoBold, CrosstermAttribute::NoItalic].as_ref()
            ),
            ..Default::default()
        },
        Style::default()
            .remove_modifier(Modifier::BOLD)
            .remove_modifier(Modifier::ITALIC)
    )]
    fn from_crossterm_content_style(#[case] content_style: ContentStyle, #[case] style: Style) {
        assert_eq!(Style::from_crossterm(content_style), style);
    }

    #[test]
    #[cfg(feature = "underline-color")]
    fn from_crossterm_content_style_underline() {
        let content_style = ContentStyle {
            underline_color: Some(CrosstermColor::DarkRed),
            ..Default::default()
        };
        assert_eq!(
            Style::from_crossterm(content_style),
            Style::default().underline_color(Color::Red)
        );
    }
}
