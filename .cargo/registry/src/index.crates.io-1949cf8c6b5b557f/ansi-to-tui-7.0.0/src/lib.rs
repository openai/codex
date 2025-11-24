#![allow(unused_imports)]
#![warn(missing_docs)]
//! Parses a `Vec<u8>` as an byte sequence with ansi colors to
//! [`tui::text::Text`][Text].  
//!
//! Invalid ansi colors / sequences will be ignored.  
//!
//!
//! Supported features
//! - UTF-8 using `String::from_utf8` or [`simdutf8`][simdutf8].
//! - Most stuff like **Bold** / *Italic* / <u>Underline</u> / ~~Strikethrough~~.
//! - Supports 4-bit color palletes.
//! - Supports 8-bit color.
//! - Supports True color ( RGB / 24-bit color ).
//!
//!
//! ## Example
//! The argument to the function `ansi_to_text` implements `IntoIterator` so it will be consumed on
//! use.
//! ```rust
//! use ansi_to_tui::IntoText;
//! let bytes = b"\x1b[38;2;225;192;203mAAAAA\x1b[0m".to_owned().to_vec();
//! let text = bytes.into_text().unwrap();
//! ```
//! Example parsing from a file.
//! ```rust
//! use ansi_to_tui::IntoText;
//! let buffer = std::fs::read("ascii/text.ascii").unwrap();
//! let text = buffer.into_text().unwrap();
//! ```
//!
//! If you want to use [`simdutf8`][simdutf8] instead of `String::from_utf8()`  
//! for parsing UTF-8 then enable optional feature `simd`  
//!  
//! [Text]: https://docs.rs/tui/0.15.0/tui/text/struct.Text.html
//! [ansi-to-tui]: https://github.com/uttarayan21/ansi-to-tui
//! [simdutf8]: https://github.com/rusticstuff/simdutf8

// mod ansi;
mod code;
mod error;
mod parser;
pub use error::Error;
use tui::text::Text;

/// IntoText will convert any type that has a AsRef<[u8]> to a Text.
pub trait IntoText {
    /// Convert the type to a Text.
    #[allow(clippy::wrong_self_convention)]
    fn into_text(&self) -> Result<Text<'static>, Error>;
    /// Convert the type to a Text while trying to copy as less as possible
    #[cfg(feature = "zero-copy")]
    fn to_text(&self) -> Result<Text<'_>, Error>;
}
impl<T> IntoText for T
where
    T: AsRef<[u8]>,
{
    fn into_text(&self) -> Result<Text<'static>, Error> {
        Ok(crate::parser::text(self.as_ref())?.1)
    }

    #[cfg(feature = "zero-copy")]
    fn to_text(&self) -> Result<Text<'_>, Error> {
        Ok(crate::parser::text_fast(self.as_ref())?.1)
    }
}
