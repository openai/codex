//! The textwrap library provides functions for word wrapping and
//! indenting text.
//!
//! # Wrapping Text
//!
//! Wrapping text can be very useful in command-line programs where
//! you want to format dynamic output nicely so it looks good in a
//! terminal. A quick example:
//!
//! ```
//! # #[cfg(feature = "smawk")] {
//! let text = "textwrap: a small library for wrapping text.";
//! assert_eq!(textwrap::wrap(text, 18),
//!            vec!["textwrap: a",
//!                 "small library for",
//!                 "wrapping text."]);
//! # }
//! ```
//!
//! The [`wrap()`] function returns the individual lines, use
//! [`fill()`] is you want the lines joined with `'\n'` to form a
//! `String`.
//!
//! If you enable the `hyphenation` Cargo feature, you can get
//! automatic hyphenation for a number of languages:
//!
//! ```
//! #[cfg(feature = "hyphenation")] {
//! use hyphenation::{Language, Load, Standard};
//! use textwrap::{wrap, Options, WordSplitter};
//!
//! let text = "textwrap: a small library for wrapping text.";
//! let dictionary = Standard::from_embedded(Language::EnglishUS).unwrap();
//! let options = Options::new(18).word_splitter(WordSplitter::Hyphenation(dictionary));
//! assert_eq!(wrap(text, &options),
//!            vec!["textwrap: a small",
//!                 "library for wrap-",
//!                 "ping text."]);
//! }
//! ```
//!
//! See also the [`unfill()`] and [`refill()`] functions which allow
//! you to manipulate already wrapped text.
//!
//! ## Wrapping Strings at Compile Time
//!
//! If your strings are known at compile time, please take a look at
//! the procedural macros from the [textwrap-macros] crate.
//!
//! ## Displayed Width vs Byte Size
//!
//! To word wrap text, one must know the width of each word so one can
//! know when to break lines. This library will by default measure the
//! width of text using the _displayed width_, not the size in bytes.
//! The `unicode-width` Cargo feature controls this.
//!
//! This is important for non-ASCII text. ASCII characters such as `a`
//! and `!` are simple and take up one column each. This means that
//! the displayed width is equal to the string length in bytes.
//! However, non-ASCII characters and symbols take up more than one
//! byte when UTF-8 encoded: `é` is `0xc3 0xa9` (two bytes) and `⚙` is
//! `0xe2 0x9a 0x99` (three bytes) in UTF-8, respectively.
//!
//! This is why we take care to use the displayed width instead of the
//! byte count when computing line lengths. All functions in this
//! library handle Unicode characters like this when the
//! `unicode-width` Cargo feature is enabled (it is enabled by
//! default).
//!
//! # Indentation and Dedentation
//!
//! The textwrap library also offers functions for adding a prefix to
//! every line of a string and to remove leading whitespace. As an
//! example, [`indent()`] allows you to turn lines of text into a
//! bullet list:
//!
//! ```
//! let before = "\
//! foo
//! bar
//! baz
//! ";
//! let after = "\
//! * foo
//! * bar
//! * baz
//! ";
//! assert_eq!(textwrap::indent(before, "* "), after);
//! ```
//!
//! Removing leading whitespace is done with [`dedent()`]:
//!
//! ```
//! let before = "
//!     Some
//!       indented
//!         text
//! ";
//! let after = "
//! Some
//!   indented
//!     text
//! ";
//! assert_eq!(textwrap::dedent(before), after);
//! ```
//!
//! # Cargo Features
//!
//! The textwrap library can be slimmed down as needed via a number of
//! Cargo features. This means you only pay for the features you
//! actually use.
//!
//! The full dependency graph, where dashed lines indicate optional
//! dependencies, is shown below:
//!
//! <img src="https://raw.githubusercontent.com/mgeisler/textwrap/master/images/textwrap-0.16.2.svg">
//!
//! ## Default Features
//!
//! These features are enabled by default:
//!
//! * `unicode-linebreak`: enables finding words using the
//!   [unicode-linebreak] crate, which implements the line breaking
//!   algorithm described in [Unicode Standard Annex
//!   #14](https://www.unicode.org/reports/tr14/).
//!
//!   This feature can be disabled if you are happy to find words
//!   separated by ASCII space characters only. People wrapping text
//!   with emojis or East-Asian characters will want most likely want
//!   to enable this feature. See [`WordSeparator`] for details.
//!
//! * `unicode-width`: enables correct width computation of non-ASCII
//!   characters via the [unicode-width] crate. Without this feature,
//!   every [`char`] is 1 column wide, except for emojis which are 2
//!   columns wide. See [`core::display_width()`] for details.
//!
//!   This feature can be disabled if you only need to wrap ASCII
//!   text, or if the functions in [`core`] are used directly with
//!   [`core::Fragment`]s for which the widths have been computed in
//!   other ways.
//!
//! * `smawk`: enables linear-time wrapping of the whole paragraph via
//!   the [smawk] crate. See [`wrap_algorithms::wrap_optimal_fit()`]
//!   for details on the optimal-fit algorithm.
//!
//!   This feature can be disabled if you only ever intend to use
//!   [`wrap_algorithms::wrap_first_fit()`].
//!
//! <!-- begin binary-sizes -->
//!
//! With Rust 1.64.0, the size impact of the above features on your
//! binary is as follows:
//!
//! | Configuration                            |  Binary Size |    Delta |
//! | :---                                     |         ---: |     ---: |
//! | quick-and-dirty implementation           |       289 KB |     — KB |
//! | textwrap without default features        |       305 KB |    16 KB |
//! | textwrap with smawk                      |       317 KB |    28 KB |
//! | textwrap with unicode-width              |       309 KB |    20 KB |
//! | textwrap with unicode-linebreak          |       342 KB |    53 KB |
//!
//! <!-- end binary-sizes -->
//!
//! The above sizes are the stripped sizes and the binary is compiled
//! in release mode with this profile:
//!
//! ```toml
//! [profile.release]
//! lto = true
//! codegen-units = 1
//! ```
//!
//! See the [binary-sizes demo] if you want to reproduce these
//! results.
//!
//! ## Optional Features
//!
//! These Cargo features enable new functionality:
//!
//! * `terminal_size`: enables automatic detection of the terminal
//!   width via the [terminal_size] crate. See
//!   [`Options::with_termwidth()`] for details.
//!
//! * `hyphenation`: enables language-sensitive hyphenation via the
//!   [hyphenation] crate. See the [`word_splitters::WordSplitter`]
//!   trait for details.
//!
//! [unicode-linebreak]: https://docs.rs/unicode-linebreak/
//! [unicode-width]: https://docs.rs/unicode-width/
//! [smawk]: https://docs.rs/smawk/
//! [binary-sizes demo]: https://github.com/mgeisler/textwrap/tree/master/examples/binary-sizes
//! [textwrap-macros]: https://docs.rs/textwrap-macros/
//! [terminal_size]: https://docs.rs/terminal_size/
//! [hyphenation]: https://docs.rs/hyphenation/

#![doc(html_root_url = "https://docs.rs/textwrap/0.16.2")]
#![forbid(unsafe_code)] // See https://github.com/mgeisler/textwrap/issues/210
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![allow(clippy::redundant_field_names)]

// Make `cargo test` execute the README doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_doctest {}

pub mod core;
#[cfg(fuzzing)]
pub mod fuzzing;
pub mod word_splitters;
pub mod wrap_algorithms;

mod columns;
mod fill;
mod indentation;
mod line_ending;
mod options;
mod refill;
#[cfg(feature = "terminal_size")]
mod termwidth;
mod word_separators;
mod wrap;

pub use columns::wrap_columns;
pub use fill::{fill, fill_inplace};
pub use indentation::{dedent, indent};
pub use line_ending::LineEnding;
pub use options::Options;
pub use refill::{refill, unfill};
#[cfg(feature = "terminal_size")]
pub use termwidth::termwidth;
pub use word_separators::WordSeparator;
pub use word_splitters::WordSplitter;
pub use wrap::wrap;
pub use wrap_algorithms::WrapAlgorithm;
