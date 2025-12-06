// show the feature flags in the generated documentation
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/ratatui/ratatui/main/assets/logo.png",
    html_favicon_url = "https://raw.githubusercontent.com/ratatui/ratatui/main/assets/favicon.ico"
)]
#![warn(missing_docs)]
//! This module provides the [`TermwizBackend`] implementation for the [`Backend`] trait. It uses
//! the [Termwiz] crate to interact with the terminal.
//!
//! [`Backend`]: trait.Backend.html
//! [Termwiz]: https://crates.io/crates/termwiz
//!
//! # Crate Organization
//!
//! `ratatui-termwiz` is part of the Ratatui workspace that was modularized in version 0.30.0.
//! This crate provides the [Termwiz] backend implementation for Ratatui.
//!
//! **When to use `ratatui-termwiz`:**
//!
//! - You need fine-grained control over dependencies
//! - Building a widget library that needs backend functionality
//! - You want to use only the Termwiz backend without other backends
//! - You need Termwiz's advanced terminal capabilities
//!
//! **When to use the main [`ratatui`] crate:**
//!
//! - Building applications (recommended - includes termwiz backend when enabled)
//! - You want the convenience of having everything available
//!
//! For detailed information about the workspace organization, see [ARCHITECTURE.md].
//!
//! [`ratatui`]: https://crates.io/crates/ratatui
//! [ARCHITECTURE.md]: https://github.com/ratatui/ratatui/blob/main/ARCHITECTURE.md
#![cfg_attr(feature = "document-features", doc = "\n## Features")]
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]

use std::error::Error;
use std::io;

use ratatui_core::backend::{Backend, ClearType, WindowSize};
use ratatui_core::buffer::Cell;
use ratatui_core::layout::{Position, Size};
use ratatui_core::style::{Color, Modifier, Style};
pub use termwiz;
use termwiz::caps::Capabilities;
use termwiz::cell::{AttributeChange, Blink, CellAttributes, Intensity, Underline};
use termwiz::color::{AnsiColor, ColorAttribute, ColorSpec, LinearRgba, RgbColor, SrgbaTuple};
use termwiz::surface::{Change, CursorVisibility, Position as TermwizPosition};
use termwiz::terminal::buffered::BufferedTerminal;
use termwiz::terminal::{ScreenSize, SystemTerminal, Terminal};

/// A [`Backend`] implementation that uses [Termwiz] to render to the terminal.
///
/// The `TermwizBackend` struct is a wrapper around a [`BufferedTerminal`], which is used to send
/// commands to the terminal. It provides methods for drawing content, manipulating the cursor, and
/// clearing the terminal screen.
///
/// Most applications should not call the methods on `TermwizBackend` directly, but will instead
/// use the [`Terminal`] struct, which provides a more ergonomic interface.
///
/// This backend automatically enables raw mode and switches to the alternate screen when it is
/// created using the [`TermwizBackend::new`] method (and disables raw mode and returns to the main
/// screen when dropped). Use the [`TermwizBackend::with_buffered_terminal`] to create a new
/// instance with a custom [`BufferedTerminal`] if this is not desired.
///
/// # Example
///
/// ```rust,no_run
/// use ratatui::Terminal;
/// use ratatui::backend::TermwizBackend;
///
/// let backend = TermwizBackend::new()?;
/// let mut terminal = Terminal::new(backend)?;
///
/// terminal.clear()?;
/// terminal.draw(|frame| {
///     // -- snip --
/// })?;
/// # std::result::Result::Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// See the the [Examples] directory for more examples. See the [`backend`] module documentation
/// for more details on raw mode and alternate screen.
///
/// [`backend`]: ratatui_core::backend
/// [`Terminal`]: https://docs.rs/ratatui/latest/ratatui/struct.Terminal.html
/// [`BufferedTerminal`]: termwiz::terminal::buffered::BufferedTerminal
/// [Termwiz]: https://crates.io/crates/termwiz
/// [Examples]: https://github.com/ratatui/ratatui/tree/main/ratatui/examples/README.md
pub struct TermwizBackend {
    buffered_terminal: BufferedTerminal<SystemTerminal>,
}

impl TermwizBackend {
    /// Creates a new Termwiz backend instance.
    ///
    /// The backend will automatically enable raw mode and enter the alternate screen.
    ///
    /// # Errors
    ///
    /// Returns an error if unable to do any of the following:
    /// - query the terminal capabilities.
    /// - enter raw mode.
    /// - enter the alternate screen.
    /// - create the system or buffered terminal.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use ratatui::backend::TermwizBackend;
    ///
    /// let backend = TermwizBackend::new()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let mut buffered_terminal =
            BufferedTerminal::new(SystemTerminal::new(Capabilities::new_from_env()?)?)?;
        buffered_terminal.terminal().set_raw_mode()?;
        buffered_terminal.terminal().enter_alternate_screen()?;
        Ok(Self { buffered_terminal })
    }

    /// Creates a new Termwiz backend instance with the given buffered terminal.
    pub const fn with_buffered_terminal(instance: BufferedTerminal<SystemTerminal>) -> Self {
        Self {
            buffered_terminal: instance,
        }
    }

    /// Returns a reference to the buffered terminal used by the backend.
    pub const fn buffered_terminal(&self) -> &BufferedTerminal<SystemTerminal> {
        &self.buffered_terminal
    }

    /// Returns a mutable reference to the buffered terminal used by the backend.
    pub const fn buffered_terminal_mut(&mut self) -> &mut BufferedTerminal<SystemTerminal> {
        &mut self.buffered_terminal
    }
}

impl Backend for TermwizBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        for (x, y, cell) in content {
            self.buffered_terminal.add_changes(vec![
                Change::CursorPosition {
                    x: TermwizPosition::Absolute(x as usize),
                    y: TermwizPosition::Absolute(y as usize),
                },
                Change::Attribute(AttributeChange::Foreground(cell.fg.into_termwiz())),
                Change::Attribute(AttributeChange::Background(cell.bg.into_termwiz())),
            ]);

            self.buffered_terminal
                .add_change(Change::Attribute(AttributeChange::Intensity(
                    if cell.modifier.contains(Modifier::BOLD) {
                        Intensity::Bold
                    } else if cell.modifier.contains(Modifier::DIM) {
                        Intensity::Half
                    } else {
                        Intensity::Normal
                    },
                )));

            self.buffered_terminal
                .add_change(Change::Attribute(AttributeChange::Italic(
                    cell.modifier.contains(Modifier::ITALIC),
                )));

            self.buffered_terminal
                .add_change(Change::Attribute(AttributeChange::Underline(
                    if cell.modifier.contains(Modifier::UNDERLINED) {
                        Underline::Single
                    } else {
                        Underline::None
                    },
                )));

            self.buffered_terminal
                .add_change(Change::Attribute(AttributeChange::Reverse(
                    cell.modifier.contains(Modifier::REVERSED),
                )));

            self.buffered_terminal
                .add_change(Change::Attribute(AttributeChange::Invisible(
                    cell.modifier.contains(Modifier::HIDDEN),
                )));

            self.buffered_terminal
                .add_change(Change::Attribute(AttributeChange::StrikeThrough(
                    cell.modifier.contains(Modifier::CROSSED_OUT),
                )));

            self.buffered_terminal
                .add_change(Change::Attribute(AttributeChange::Blink(
                    if cell.modifier.contains(Modifier::SLOW_BLINK) {
                        Blink::Slow
                    } else if cell.modifier.contains(Modifier::RAPID_BLINK) {
                        Blink::Rapid
                    } else {
                        Blink::None
                    },
                )));

            self.buffered_terminal.add_change(cell.symbol());
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.buffered_terminal
            .add_change(Change::CursorVisibility(CursorVisibility::Hidden));
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.buffered_terminal
            .add_change(Change::CursorVisibility(CursorVisibility::Visible));
        Ok(())
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        let (x, y) = self.buffered_terminal.cursor_position();
        Ok(Position::new(x as u16, y as u16))
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let Position { x, y } = position.into();
        self.buffered_terminal.add_change(Change::CursorPosition {
            x: TermwizPosition::Absolute(x as usize),
            y: TermwizPosition::Absolute(y as usize),
        });

        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.buffered_terminal
            .add_change(Change::ClearScreen(termwiz::color::ColorAttribute::Default));
        Ok(())
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        match clear_type {
            ClearType::All => self.clear(),
            ClearType::AfterCursor
            | ClearType::BeforeCursor
            | ClearType::CurrentLine
            | ClearType::UntilNewLine => Err(io::Error::other(format!(
                "clear_type [{clear_type:?}] not supported with this backend"
            ))),
        }
    }

    fn size(&self) -> io::Result<Size> {
        let (cols, rows) = self.buffered_terminal.dimensions();
        Ok(Size::new(u16_max(cols), u16_max(rows)))
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        let ScreenSize {
            cols,
            rows,
            xpixel,
            ypixel,
        } = self
            .buffered_terminal
            .terminal()
            .get_screen_size()
            .map_err(io::Error::other)?;
        Ok(WindowSize {
            columns_rows: Size {
                width: u16_max(cols),
                height: u16_max(rows),
            },
            pixels: Size {
                width: u16_max(xpixel),
                height: u16_max(ypixel),
            },
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buffered_terminal.flush().map_err(io::Error::other)?;
        Ok(())
    }

    #[cfg(feature = "scrolling-regions")]
    fn scroll_region_up(&mut self, region: std::ops::Range<u16>, amount: u16) -> io::Result<()> {
        // termwiz doesn't have a command to just set the scrolling region. Instead, setting the
        // scrolling region and scrolling are combined. However, this has the side-effect of
        // leaving the scrolling region set. To reset the scrolling region, termwiz advises one to
        // make a scrolling-region scroll command that contains the entire screen, but scrolls by 0
        // lines. See [`Change::ScrollRegionUp`] for more details.
        let (_, rows) = self.buffered_terminal.dimensions();
        self.buffered_terminal.add_changes(vec![
            Change::ScrollRegionUp {
                first_row: region.start as usize,
                region_size: region.len(),
                scroll_count: amount as usize,
            },
            Change::ScrollRegionUp {
                first_row: 0,
                region_size: rows,
                scroll_count: 0,
            },
        ]);
        Ok(())
    }

    #[cfg(feature = "scrolling-regions")]
    fn scroll_region_down(&mut self, region: std::ops::Range<u16>, amount: u16) -> io::Result<()> {
        // termwiz doesn't have a command to just set the scrolling region. Instead, setting the
        // scrolling region and scrolling are combined. However, this has the side-effect of
        // leaving the scrolling region set. To reset the scrolling region, termwiz advises one to
        // make a scrolling-region scroll command that contains the entire screen, but scrolls by 0
        // lines. See [`Change::ScrollRegionDown`] for more details.
        let (_, rows) = self.buffered_terminal.dimensions();
        self.buffered_terminal.add_changes(vec![
            Change::ScrollRegionDown {
                first_row: region.start as usize,
                region_size: region.len(),
                scroll_count: amount as usize,
            },
            Change::ScrollRegionDown {
                first_row: 0,
                region_size: rows,
                scroll_count: 0,
            },
        ]);
        Ok(())
    }
}

/// A trait for converting types from Termwiz to Ratatui.
///
/// This trait replaces the `From` trait for converting types from Termwiz to Ratatui. It is
/// necessary because the `From` trait is not implemented for types defined in external crates.
pub trait FromTermwiz<T> {
    /// Converts the given Termwiz type to the Ratatui type.
    fn from_termwiz(termwiz: T) -> Self;
}

/// A trait for converting types from Ratatui to Termwiz.
///
/// This trait replaces the `Into` trait for converting types from Ratatui to Termwiz. It is
/// necessary because the `Into` trait is not implemented for types defined in external crates.
pub trait IntoTermwiz<T> {
    /// Converts the given Ratatui type to the Termwiz type.
    fn into_termwiz(self) -> T;
}

/// A replacement for the `Into` trait for converting types from Ratatui to Termwiz.
///
/// This trait is necessary because the `Into` trait is not implemented for types defined in
/// external crates.
///
/// A blanket implementation is provided for all types that implement `FromTermwiz`.
///
/// This trait is private to the module as it would otherwise conflict with the other backend
/// modules. It is mainly used to avoid rewriting all the `.into()` calls in this module.
trait IntoRatatui<R> {
    fn into_ratatui(self) -> R;
}

impl<C, R: FromTermwiz<C>> IntoRatatui<R> for C {
    fn into_ratatui(self) -> R {
        R::from_termwiz(self)
    }
}

impl FromTermwiz<CellAttributes> for Style {
    fn from_termwiz(value: CellAttributes) -> Self {
        let mut style = Self::new()
            .add_modifier(value.intensity().into_ratatui())
            .add_modifier(value.underline().into_ratatui())
            .add_modifier(value.blink().into_ratatui());

        if value.italic() {
            style.add_modifier |= Modifier::ITALIC;
        }
        if value.reverse() {
            style.add_modifier |= Modifier::REVERSED;
        }
        if value.strikethrough() {
            style.add_modifier |= Modifier::CROSSED_OUT;
        }
        if value.invisible() {
            style.add_modifier |= Modifier::HIDDEN;
        }

        style.fg = Some(value.foreground().into_ratatui());
        style.bg = Some(value.background().into_ratatui());
        #[cfg(feature = "underline-color")]
        {
            style.underline_color = Some(value.underline_color().into_ratatui());
        }

        style
    }
}

impl FromTermwiz<Intensity> for Modifier {
    fn from_termwiz(value: Intensity) -> Self {
        match value {
            Intensity::Normal => Self::empty(),
            Intensity::Bold => Self::BOLD,
            Intensity::Half => Self::DIM,
        }
    }
}

impl FromTermwiz<Underline> for Modifier {
    fn from_termwiz(value: Underline) -> Self {
        match value {
            Underline::None => Self::empty(),
            _ => Self::UNDERLINED,
        }
    }
}

impl FromTermwiz<Blink> for Modifier {
    fn from_termwiz(value: Blink) -> Self {
        match value {
            Blink::None => Self::empty(),
            Blink::Slow => Self::SLOW_BLINK,
            Blink::Rapid => Self::RAPID_BLINK,
        }
    }
}

impl IntoTermwiz<ColorAttribute> for Color {
    fn into_termwiz(self) -> ColorAttribute {
        match self {
            Self::Reset => ColorAttribute::Default,
            Self::Black => AnsiColor::Black.into(),
            Self::DarkGray => AnsiColor::Grey.into(),
            Self::Gray => AnsiColor::Silver.into(),
            Self::Red => AnsiColor::Maroon.into(),
            Self::LightRed => AnsiColor::Red.into(),
            Self::Green => AnsiColor::Green.into(),
            Self::LightGreen => AnsiColor::Lime.into(),
            Self::Yellow => AnsiColor::Olive.into(),
            Self::LightYellow => AnsiColor::Yellow.into(),
            Self::Magenta => AnsiColor::Purple.into(),
            Self::LightMagenta => AnsiColor::Fuchsia.into(),
            Self::Cyan => AnsiColor::Teal.into(),
            Self::LightCyan => AnsiColor::Aqua.into(),
            Self::White => AnsiColor::White.into(),
            Self::Blue => AnsiColor::Navy.into(),
            Self::LightBlue => AnsiColor::Blue.into(),
            Self::Indexed(i) => ColorAttribute::PaletteIndex(i),
            Self::Rgb(r, g, b) => {
                ColorAttribute::TrueColorWithDefaultFallback(SrgbaTuple::from((r, g, b)))
            }
        }
    }
}

impl FromTermwiz<AnsiColor> for Color {
    fn from_termwiz(value: AnsiColor) -> Self {
        match value {
            AnsiColor::Black => Self::Black,
            AnsiColor::Grey => Self::DarkGray,
            AnsiColor::Silver => Self::Gray,
            AnsiColor::Maroon => Self::Red,
            AnsiColor::Red => Self::LightRed,
            AnsiColor::Green => Self::Green,
            AnsiColor::Lime => Self::LightGreen,
            AnsiColor::Olive => Self::Yellow,
            AnsiColor::Yellow => Self::LightYellow,
            AnsiColor::Purple => Self::Magenta,
            AnsiColor::Fuchsia => Self::LightMagenta,
            AnsiColor::Teal => Self::Cyan,
            AnsiColor::Aqua => Self::LightCyan,
            AnsiColor::White => Self::White,
            AnsiColor::Navy => Self::Blue,
            AnsiColor::Blue => Self::LightBlue,
        }
    }
}

impl FromTermwiz<ColorAttribute> for Color {
    fn from_termwiz(value: ColorAttribute) -> Self {
        match value {
            ColorAttribute::TrueColorWithDefaultFallback(srgba)
            | ColorAttribute::TrueColorWithPaletteFallback(srgba, _) => srgba.into_ratatui(),
            ColorAttribute::PaletteIndex(i) => Self::Indexed(i),
            ColorAttribute::Default => Self::Reset,
        }
    }
}

impl FromTermwiz<ColorSpec> for Color {
    fn from_termwiz(value: ColorSpec) -> Self {
        match value {
            ColorSpec::Default => Self::Reset,
            ColorSpec::PaletteIndex(i) => Self::Indexed(i),
            ColorSpec::TrueColor(srgba) => srgba.into_ratatui(),
        }
    }
}

impl FromTermwiz<SrgbaTuple> for Color {
    fn from_termwiz(value: SrgbaTuple) -> Self {
        let (r, g, b, _) = value.to_srgb_u8();
        Self::Rgb(r, g, b)
    }
}

impl FromTermwiz<RgbColor> for Color {
    fn from_termwiz(value: RgbColor) -> Self {
        let (r, g, b) = value.to_tuple_rgb8();
        Self::Rgb(r, g, b)
    }
}

impl FromTermwiz<LinearRgba> for Color {
    fn from_termwiz(value: LinearRgba) -> Self {
        value.to_srgb().into_ratatui()
    }
}

#[inline]
fn u16_max(i: usize) -> u16 {
    u16::try_from(i).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod into_color {
        use Color as C;

        use super::*;

        #[test]
        fn from_linear_rgba() {
            // full black + opaque
            assert_eq!(
                C::from_termwiz(LinearRgba(0., 0., 0., 1.)),
                Color::Rgb(0, 0, 0)
            );
            // full black + transparent
            assert_eq!(
                C::from_termwiz(LinearRgba(0., 0., 0., 0.)),
                Color::Rgb(0, 0, 0)
            );

            // full white + opaque
            assert_eq!(
                C::from_termwiz(LinearRgba(1., 1., 1., 1.)),
                C::Rgb(254, 254, 254)
            );
            // full white + transparent
            assert_eq!(
                C::from_termwiz(LinearRgba(1., 1., 1., 0.)),
                C::Rgb(254, 254, 254)
            );

            // full red
            assert_eq!(
                C::from_termwiz(LinearRgba(1., 0., 0., 1.)),
                C::Rgb(254, 0, 0)
            );
            // full green
            assert_eq!(
                C::from_termwiz(LinearRgba(0., 1., 0., 1.)),
                C::Rgb(0, 254, 0)
            );
            // full blue
            assert_eq!(
                C::from_termwiz(LinearRgba(0., 0., 1., 1.)),
                C::Rgb(0, 0, 254)
            );

            // See https://stackoverflow.com/questions/12524623/what-are-the-practical-differences-when-working-with-colors-in-a-linear-vs-a-no
            // for an explanation

            // half red
            assert_eq!(
                C::from_termwiz(LinearRgba(0.214, 0., 0., 1.)),
                C::Rgb(127, 0, 0)
            );
            // half green
            assert_eq!(
                C::from_termwiz(LinearRgba(0., 0.214, 0., 1.)),
                C::Rgb(0, 127, 0)
            );
            // half blue
            assert_eq!(
                C::from_termwiz(LinearRgba(0., 0., 0.214, 1.)),
                C::Rgb(0, 0, 127)
            );
        }

        #[test]
        fn from_srgba() {
            // full black + opaque
            assert_eq!(
                C::from_termwiz(SrgbaTuple(0., 0., 0., 1.)),
                Color::Rgb(0, 0, 0)
            );
            // full black + transparent
            assert_eq!(
                C::from_termwiz(SrgbaTuple(0., 0., 0., 0.)),
                Color::Rgb(0, 0, 0)
            );

            // full white + opaque
            assert_eq!(
                C::from_termwiz(SrgbaTuple(1., 1., 1., 1.)),
                C::Rgb(255, 255, 255)
            );
            // full white + transparent
            assert_eq!(
                C::from_termwiz(SrgbaTuple(1., 1., 1., 0.)),
                C::Rgb(255, 255, 255)
            );

            // full red
            assert_eq!(
                C::from_termwiz(SrgbaTuple(1., 0., 0., 1.)),
                C::Rgb(255, 0, 0)
            );
            // full green
            assert_eq!(
                C::from_termwiz(SrgbaTuple(0., 1., 0., 1.)),
                C::Rgb(0, 255, 0)
            );
            // full blue
            assert_eq!(
                C::from_termwiz(SrgbaTuple(0., 0., 1., 1.)),
                C::Rgb(0, 0, 255)
            );

            // half red
            assert_eq!(
                C::from_termwiz(SrgbaTuple(0.5, 0., 0., 1.)),
                C::Rgb(127, 0, 0)
            );
            // half green
            assert_eq!(
                C::from_termwiz(SrgbaTuple(0., 0.5, 0., 1.)),
                C::Rgb(0, 127, 0)
            );
            // half blue
            assert_eq!(
                C::from_termwiz(SrgbaTuple(0., 0., 0.5, 1.)),
                C::Rgb(0, 0, 127)
            );
        }

        #[test]
        fn from_rgbcolor() {
            // full black
            assert_eq!(
                C::from_termwiz(RgbColor::new_8bpc(0, 0, 0)),
                Color::Rgb(0, 0, 0)
            );
            // full white
            assert_eq!(
                C::from_termwiz(RgbColor::new_8bpc(255, 255, 255)),
                C::Rgb(255, 255, 255)
            );

            // full red
            assert_eq!(
                C::from_termwiz(RgbColor::new_8bpc(255, 0, 0)),
                C::Rgb(255, 0, 0)
            );
            // full green
            assert_eq!(
                C::from_termwiz(RgbColor::new_8bpc(0, 255, 0)),
                C::Rgb(0, 255, 0)
            );
            // full blue
            assert_eq!(
                C::from_termwiz(RgbColor::new_8bpc(0, 0, 255)),
                C::Rgb(0, 0, 255)
            );

            // half red
            assert_eq!(
                C::from_termwiz(RgbColor::new_8bpc(127, 0, 0)),
                C::Rgb(127, 0, 0)
            );
            // half green
            assert_eq!(
                C::from_termwiz(RgbColor::new_8bpc(0, 127, 0)),
                C::Rgb(0, 127, 0)
            );
            // half blue
            assert_eq!(
                C::from_termwiz(RgbColor::new_8bpc(0, 0, 127)),
                C::Rgb(0, 0, 127)
            );
        }

        #[test]
        fn from_colorspec() {
            assert_eq!(C::from_termwiz(ColorSpec::Default), C::Reset);
            assert_eq!(C::from_termwiz(ColorSpec::PaletteIndex(33)), C::Indexed(33));
            assert_eq!(
                C::from_termwiz(ColorSpec::TrueColor(SrgbaTuple(0., 0., 0., 1.))),
                C::Rgb(0, 0, 0)
            );
        }

        #[test]
        fn from_colorattribute() {
            assert_eq!(C::from_termwiz(ColorAttribute::Default), C::Reset);
            assert_eq!(
                C::from_termwiz(ColorAttribute::PaletteIndex(32)),
                C::Indexed(32)
            );
            assert_eq!(
                C::from_termwiz(ColorAttribute::TrueColorWithDefaultFallback(SrgbaTuple(
                    0., 0., 0., 1.
                ))),
                C::Rgb(0, 0, 0)
            );
            assert_eq!(
                C::from_termwiz(ColorAttribute::TrueColorWithPaletteFallback(
                    SrgbaTuple(0., 0., 0., 1.),
                    31
                )),
                C::Rgb(0, 0, 0)
            );
        }

        #[test]
        fn from_ansicolor() {
            assert_eq!(C::from_termwiz(AnsiColor::Black), Color::Black);
            assert_eq!(C::from_termwiz(AnsiColor::Grey), Color::DarkGray);
            assert_eq!(C::from_termwiz(AnsiColor::Silver), Color::Gray);
            assert_eq!(C::from_termwiz(AnsiColor::Maroon), Color::Red);
            assert_eq!(C::from_termwiz(AnsiColor::Red), Color::LightRed);
            assert_eq!(C::from_termwiz(AnsiColor::Green), Color::Green);
            assert_eq!(C::from_termwiz(AnsiColor::Lime), Color::LightGreen);
            assert_eq!(C::from_termwiz(AnsiColor::Olive), Color::Yellow);
            assert_eq!(C::from_termwiz(AnsiColor::Yellow), Color::LightYellow);
            assert_eq!(C::from_termwiz(AnsiColor::Purple), Color::Magenta);
            assert_eq!(C::from_termwiz(AnsiColor::Fuchsia), Color::LightMagenta);
            assert_eq!(C::from_termwiz(AnsiColor::Teal), Color::Cyan);
            assert_eq!(C::from_termwiz(AnsiColor::Aqua), Color::LightCyan);
            assert_eq!(C::from_termwiz(AnsiColor::White), Color::White);
            assert_eq!(C::from_termwiz(AnsiColor::Navy), Color::Blue);
            assert_eq!(C::from_termwiz(AnsiColor::Blue), Color::LightBlue);
        }
    }

    mod into_modifier {
        use super::*;

        #[test]
        fn from_intensity() {
            assert_eq!(Modifier::from_termwiz(Intensity::Normal), Modifier::empty());
            assert_eq!(Modifier::from_termwiz(Intensity::Bold), Modifier::BOLD);
            assert_eq!(Modifier::from_termwiz(Intensity::Half), Modifier::DIM);
        }

        #[test]
        fn from_underline() {
            assert_eq!(Modifier::from_termwiz(Underline::None), Modifier::empty());
            assert_eq!(
                Modifier::from_termwiz(Underline::Single),
                Modifier::UNDERLINED
            );
            assert_eq!(
                Modifier::from_termwiz(Underline::Double),
                Modifier::UNDERLINED
            );
            assert_eq!(
                Modifier::from_termwiz(Underline::Curly),
                Modifier::UNDERLINED
            );
            assert_eq!(
                Modifier::from_termwiz(Underline::Dashed),
                Modifier::UNDERLINED
            );
            assert_eq!(
                Modifier::from_termwiz(Underline::Dotted),
                Modifier::UNDERLINED
            );
        }

        #[test]
        fn from_blink() {
            assert_eq!(Modifier::from_termwiz(Blink::None), Modifier::empty());
            assert_eq!(Modifier::from_termwiz(Blink::Slow), Modifier::SLOW_BLINK);
            assert_eq!(Modifier::from_termwiz(Blink::Rapid), Modifier::RAPID_BLINK);
        }
    }

    #[test]
    fn from_cell_attribute_for_style() {
        #[cfg(feature = "underline-color")]
        const STYLE: Style = Style::new()
            .underline_color(Color::Reset)
            .fg(Color::Reset)
            .bg(Color::Reset);
        #[cfg(not(feature = "underline-color"))]
        const STYLE: Style = Style::new().fg(Color::Reset).bg(Color::Reset);

        // default
        assert_eq!(Style::from_termwiz(CellAttributes::default()), STYLE);

        // foreground color
        assert_eq!(
            Style::from_termwiz(
                CellAttributes::default()
                    .set_foreground(ColorAttribute::PaletteIndex(31))
                    .to_owned()
            ),
            STYLE.fg(Color::Indexed(31))
        );
        // background color
        assert_eq!(
            Style::from_termwiz(
                CellAttributes::default()
                    .set_background(ColorAttribute::PaletteIndex(31))
                    .to_owned()
            ),
            STYLE.bg(Color::Indexed(31))
        );
        // underlined
        assert_eq!(
            Style::from_termwiz(
                CellAttributes::default()
                    .set_underline(Underline::Single)
                    .to_owned()
            ),
            STYLE.underlined()
        );
        // blink
        assert_eq!(
            Style::from_termwiz(CellAttributes::default().set_blink(Blink::Slow).to_owned()),
            STYLE.slow_blink()
        );
        // intensity
        assert_eq!(
            Style::from_termwiz(
                CellAttributes::default()
                    .set_intensity(Intensity::Bold)
                    .to_owned()
            ),
            STYLE.bold()
        );
        // italic
        assert_eq!(
            Style::from_termwiz(CellAttributes::default().set_italic(true).to_owned()),
            STYLE.italic()
        );
        // reversed
        assert_eq!(
            Style::from_termwiz(CellAttributes::default().set_reverse(true).to_owned()),
            STYLE.reversed()
        );
        // strikethrough
        assert_eq!(
            Style::from_termwiz(CellAttributes::default().set_strikethrough(true).to_owned()),
            STYLE.crossed_out()
        );
        // hidden
        assert_eq!(
            Style::from_termwiz(CellAttributes::default().set_invisible(true).to_owned()),
            STYLE.hidden()
        );

        // underline color
        #[cfg(feature = "underline-color")]
        assert_eq!(
            Style::from_termwiz(
                CellAttributes::default()
                    .set_underline_color(AnsiColor::Red)
                    .to_owned()
            ),
            STYLE.underline_color(Color::Indexed(9))
        );
    }
}
