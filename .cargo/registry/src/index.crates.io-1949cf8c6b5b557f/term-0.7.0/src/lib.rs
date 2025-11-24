// Copyright 2013-2019 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Terminal formatting library.
//!
//! This crate provides the `Terminal` trait, which abstracts over an [ANSI
//! Terminal][ansi] to provide color printing, among other things. There are two
//! implementations, the `TerminfoTerminal`, which uses control characters from
//! a [terminfo][ti] database, and `WinConsole`, which uses the [Win32 Console
//! API][win].
//!
//! # Usage
//!
//! This crate is [on crates.io](https://crates.io/crates/term) and can be
//! used by adding `term` to the dependencies in your project's `Cargo.toml`.
//!
//! ```toml
//! [dependencies]
//!
//! term = "*"
//! ```
//!
//! and this to your crate root:
//!
//! ```rust
//! extern crate term;
//! ```
//!
//! # Examples
//!
//! ```no_run
//! extern crate term;
//! use std::io::prelude::*;
//!
//! fn main() {
//!     let mut t = term::stdout().unwrap();
//!
//!     t.fg(term::color::GREEN).unwrap();
//!     write!(t, "hello, ").unwrap();
//!
//!     t.fg(term::color::RED).unwrap();
//!     writeln!(t, "world!").unwrap();
//!
//!     t.reset().unwrap();
//! }
//! ```
//!
//! [ansi]: https://en.wikipedia.org/wiki/ANSI_escape_code
//! [win]: http://msdn.microsoft.com/en-us/library/windows/desktop/ms682010%28v=vs.85%29.aspx
//! [ti]: https://en.wikipedia.org/wiki/Terminfo

#![doc(
    html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk-v2.png",
    html_favicon_url = "https://doc.rust-lang.org/favicon.ico",
    html_root_url = "https://stebalien.github.io/doc/term/term/",
    test(attr(deny(warnings)))
)]
#![deny(missing_docs)]
#![cfg_attr(test, deny(warnings))]
#![allow(clippy::redundant_field_names)]

use std::io::prelude::*;

pub use crate::terminfo::TerminfoTerminal;
#[cfg(windows)]
pub use win::{WinConsole, WinConsoleInfo};

use std::io::{self, Stderr, Stdout};

pub mod terminfo;

#[cfg(windows)]
mod win;

/// Alias for stdout terminals.
pub type StdoutTerminal = dyn Terminal<Output = Stdout> + Send;
/// Alias for stderr terminals.
pub type StderrTerminal = dyn Terminal<Output = Stderr> + Send;

#[cfg(not(windows))]
/// Return a Terminal wrapping stdout, or None if a terminal couldn't be
/// opened.
pub fn stdout() -> Option<Box<StdoutTerminal>> {
    TerminfoTerminal::new(io::stdout()).map(|t| Box::new(t) as Box<StdoutTerminal>)
}

#[cfg(windows)]
/// Return a Terminal wrapping stdout, or None if a terminal couldn't be
/// opened.
pub fn stdout() -> Option<Box<StdoutTerminal>> {
    TerminfoTerminal::new(io::stdout())
        .map(|t| Box::new(t) as Box<StdoutTerminal>)
        .or_else(|| {
            WinConsole::new(io::stdout())
                .ok()
                .map(|t| Box::new(t) as Box<StdoutTerminal>)
        })
}

#[cfg(not(windows))]
/// Return a Terminal wrapping stderr, or None if a terminal couldn't be
/// opened.
pub fn stderr() -> Option<Box<StderrTerminal>> {
    TerminfoTerminal::new(io::stderr()).map(|t| Box::new(t) as Box<StderrTerminal>)
}

#[cfg(windows)]
/// Return a Terminal wrapping stderr, or None if a terminal couldn't be
/// opened.
pub fn stderr() -> Option<Box<StderrTerminal>> {
    TerminfoTerminal::new(io::stderr())
        .map(|t| Box::new(t) as Box<StderrTerminal>)
        .or_else(|| {
            WinConsole::new(io::stderr())
                .ok()
                .map(|t| Box::new(t) as Box<StderrTerminal>)
        })
}

/// Terminal color definitions
#[allow(missing_docs)]
pub mod color {
    /// Number for a terminal color
    pub type Color = u32;

    pub const BLACK: Color = 0;
    pub const RED: Color = 1;
    pub const GREEN: Color = 2;
    pub const YELLOW: Color = 3;
    pub const BLUE: Color = 4;
    pub const MAGENTA: Color = 5;
    pub const CYAN: Color = 6;
    pub const WHITE: Color = 7;

    pub const BRIGHT_BLACK: Color = 8;
    pub const BRIGHT_RED: Color = 9;
    pub const BRIGHT_GREEN: Color = 10;
    pub const BRIGHT_YELLOW: Color = 11;
    pub const BRIGHT_BLUE: Color = 12;
    pub const BRIGHT_MAGENTA: Color = 13;
    pub const BRIGHT_CYAN: Color = 14;
    pub const BRIGHT_WHITE: Color = 15;
}

/// Terminal attributes for use with term.attr().
///
/// Most attributes can only be turned on and must be turned off with term.reset().
/// The ones that can be turned off explicitly take a boolean value.
/// Color is also represented as an attribute for convenience.
#[derive(Debug, PartialEq, Hash, Eq, Copy, Clone)]
pub enum Attr {
    /// Bold (or possibly bright) mode
    Bold,
    /// Dim mode, also called faint or half-bright. Often not supported
    Dim,
    /// Italics mode. Often not supported
    Italic(bool),
    /// Underline mode
    Underline(bool),
    /// Blink mode
    Blink,
    /// Standout mode. Often implemented as Reverse, sometimes coupled with Bold
    Standout(bool),
    /// Reverse mode, inverts the foreground and background colors
    Reverse,
    /// Secure mode, also called invis mode. Hides the printed text
    Secure,
    /// Convenience attribute to set the foreground color
    ForegroundColor(color::Color),
    /// Convenience attribute to set the background color
    BackgroundColor(color::Color),
}

/// An error arising from interacting with the terminal.
#[derive(Debug)]
pub enum Error {
    /// Indicates an error from any underlying IO
    Io(io::Error),
    /// Indicates an error during terminfo parsing
    TerminfoParsing(terminfo::Error),
    /// Indicates an error expanding a parameterized string from the terminfo database
    ParameterizedExpansion(terminfo::parm::Error),
    /// Indicates that the terminal does not support the requested operation.
    NotSupported,
    /// Indicates that the `TERM` environment variable was unset, and thus we were unable to detect
    /// which terminal we should be using.
    TermUnset,
    /// Indicates that we were unable to find a terminfo entry for the requested terminal.
    TerminfoEntryNotFound,
    /// Indicates that the cursor could not be moved to the requested position.
    CursorDestinationInvalid,
    /// Indicates that the terminal does not support displaying the requested color.
    ///
    /// This is like `NotSupported`, but more specific.
    ColorOutOfRange,
    #[doc(hidden)]
    /// Please don't match against this - if you do, we can't promise we won't break your crate
    /// with a semver-compliant version bump.
    __Nonexhaustive,
}

// manually implemented because std::io::Error does not implement Eq/PartialEq
impl std::cmp::PartialEq for Error {
    fn eq(&self, other: &Error) -> bool {
        use crate::Error::*;
        match *self {
            Io(_) => false,
            TerminfoParsing(ref inner1) => match *other {
                TerminfoParsing(ref inner2) => inner1 == inner2,
                _ => false,
            },
            ParameterizedExpansion(ref inner1) => match *other {
                ParameterizedExpansion(ref inner2) => inner1 == inner2,
                _ => false,
            },
            NotSupported => match *other {
                NotSupported => true,
                _ => false,
            },
            TermUnset => match *other {
                TermUnset => true,
                _ => false,
            },
            TerminfoEntryNotFound => match *other {
                TerminfoEntryNotFound => true,
                _ => false,
            },
            CursorDestinationInvalid => match *other {
                CursorDestinationInvalid => true,
                _ => false,
            },
            ColorOutOfRange => match *other {
                ColorOutOfRange => true,
                _ => false,
            },
            __Nonexhaustive => match *other {
                __Nonexhaustive => true,
                _ => false,
            },
        }
    }
}

/// The canonical `Result` type using this crate's Error type.
pub type Result<T> = std::result::Result<T, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::Error::*;
        match *self {
            Io(ref io) => io.fmt(f),
            TerminfoParsing(ref e) => e.fmt(f),
            ParameterizedExpansion(ref e) => e.fmt(f),
            NotSupported => f.write_str("operation not supported by the terminal"),
            TermUnset => {
                f.write_str("TERM environment variable unset, unable to detect a terminal")
            }
            TerminfoEntryNotFound => {
                f.write_str("could not find a terminfo entry for this terminal")
            }
            CursorDestinationInvalid => f.write_str("could not move cursor to requested position"),
            ColorOutOfRange => f.write_str("color not supported by the terminal"),
            __Nonexhaustive => f.write_str("placeholder variant that shouldn't be used"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            Error::Io(ref io) => Some(io),
            Error::TerminfoParsing(ref e) => Some(e),
            Error::ParameterizedExpansion(ref e) => Some(e),
            _ => None,
        }
    }
}

impl From<Error> for io::Error {
    fn from(err: Error) -> io::Error {
        let kind = match err {
            Error::Io(ref e) => e.kind(),
            _ => io::ErrorKind::Other,
        };
        io::Error::new(kind, err)
    }
}

impl std::convert::From<io::Error> for Error {
    fn from(val: io::Error) -> Self {
        Error::Io(val)
    }
}

impl std::convert::From<terminfo::Error> for Error {
    fn from(val: terminfo::Error) -> Self {
        Error::TerminfoParsing(val)
    }
}

impl std::convert::From<terminfo::parm::Error> for Error {
    fn from(val: terminfo::parm::Error) -> Self {
        Error::ParameterizedExpansion(val)
    }
}

/// A terminal with similar capabilities to an ANSI Terminal
/// (foreground/background colors etc).
pub trait Terminal: Write {
    /// The terminal's output writer type.
    type Output: Write;

    /// Sets the foreground color to the given color.
    ///
    /// If the color is a bright color, but the terminal only supports 8 colors,
    /// the corresponding normal color will be used instead.
    ///
    /// Returns `Ok(())` if the color change code was sent to the terminal, or `Err(e)` if there
    /// was an error.
    fn fg(&mut self, color: color::Color) -> Result<()>;

    /// Sets the background color to the given color.
    ///
    /// If the color is a bright color, but the terminal only supports 8 colors,
    /// the corresponding normal color will be used instead.
    ///
    /// Returns `Ok(())` if the color change code was sent to the terminal, or `Err(e)` if there
    /// was an error.
    fn bg(&mut self, color: color::Color) -> Result<()>;

    /// Sets the given terminal attribute, if supported.  Returns `Ok(())` if the attribute is
    /// supported and was sent to the terminal, or `Err(e)` if there was an error or the attribute
    /// wasn't supported.
    fn attr(&mut self, attr: Attr) -> Result<()>;

    /// Returns whether the given terminal attribute is supported.
    fn supports_attr(&self, attr: Attr) -> bool;

    /// Resets all terminal attributes and colors to their defaults.
    ///
    /// Returns `Ok(())` if the reset code was printed, or `Err(e)` if there was an error.
    ///
    /// *Note: This does not flush.*
    ///
    /// That means the reset command may get buffered so, if you aren't planning on doing anything
    /// else that might flush stdout's buffer (e.g. writing a line of text), you should flush after
    /// calling reset.
    fn reset(&mut self) -> Result<()>;

    /// Returns true if reset is supported.
    fn supports_reset(&self) -> bool;

    /// Returns true if color is fully supported.
    ///
    /// If this function returns `true`, `bg`, `fg`, and `reset` will never
    /// return `Err(Error::NotSupported)`.
    fn supports_color(&self) -> bool;

    /// Moves the cursor up one line.
    ///
    /// Returns `Ok(())` if the cursor movement code was printed, or `Err(e)` if there was an
    /// error.
    fn cursor_up(&mut self) -> Result<()>;

    /// Deletes the text from the cursor location to the end of the line.
    ///
    /// Returns `Ok(())` if the deletion code was printed, or `Err(e)` if there was an error.
    fn delete_line(&mut self) -> Result<()>;

    /// Moves the cursor to the left edge of the current line.
    ///
    /// Returns `Ok(true)` if the deletion code was printed, or `Err(e)` if there was an error.
    fn carriage_return(&mut self) -> Result<()>;

    /// Gets an immutable reference to the stream inside
    fn get_ref(&self) -> &Self::Output;

    /// Gets a mutable reference to the stream inside
    fn get_mut(&mut self) -> &mut Self::Output;

    /// Returns the contained stream, destroying the `Terminal`
    fn into_inner(self) -> Self::Output
    where
        Self: Sized;
}
