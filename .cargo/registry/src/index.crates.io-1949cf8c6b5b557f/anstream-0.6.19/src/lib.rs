//! **Auto-adapting [`stdout`] / [`stderr`] streams**
//!
//! *A portmanteau of "ansi stream"*
//!
//! [`AutoStream`] always accepts [ANSI escape codes](https://en.wikipedia.org/wiki/ANSI_escape_code),
//! [adapting to the user's terminal's capabilities][AutoStream].
//!
//! Benefits
//! - Allows the caller to not be concerned with the terminal's capabilities
//! - Semver safe way of passing styled text between crates as ANSI escape codes offer more
//!   compatibility than most crate APIs.
//!
//! Available styling crates:
//! - [anstyle](https://docs.rs/anstyle) for minimal runtime styling, designed to go in public APIs
//! - [owo-colors](https://docs.rs/owo-colors) for feature-rich runtime styling
//! - [color-print](https://docs.rs/color-print) for feature-rich compile-time styling
//!
//! # Example
//!
//! ```
//! #  #[cfg(feature = "auto")] {
//! use anstream::println;
//! use owo_colors::OwoColorize as _;
//!
//! // Foreground colors
//! println!("My number is {:#x}!", 10.green());
//! // Background colors
//! println!("My number is not {}!", 4.on_red());
//! # }
//! ```
//!
//! And this will correctly handle piping to a file, etc

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

pub mod adapter;
pub mod stream;
#[doc(hidden)]
#[macro_use]
pub mod _macros;

mod auto;
mod buffer;
mod fmt;
mod strip;
#[cfg(all(windows, feature = "wincon"))]
mod wincon;

pub use auto::AutoStream;
pub use strip::StripStream;
#[cfg(all(windows, feature = "wincon"))]
pub use wincon::WinconStream;

#[allow(deprecated)]
pub use buffer::Buffer;

/// An adaptive wrapper around the global standard output stream of the current process
pub type Stdout = AutoStream<std::io::Stdout>;
/// An adaptive wrapper around the global standard error stream of the current process
pub type Stderr = AutoStream<std::io::Stderr>;

/// Create an ANSI escape code compatible stdout
///
/// **Note:** Call [`AutoStream::lock`] in loops to avoid the performance hit of acquiring/releasing
/// from the implicit locking in each [`std::io::Write`] call
#[cfg(feature = "auto")]
pub fn stdout() -> Stdout {
    let stdout = std::io::stdout();
    AutoStream::auto(stdout)
}

/// Create an ANSI escape code compatible stderr
///
/// **Note:** Call [`AutoStream::lock`] in loops to avoid the performance hit of acquiring/releasing
/// from the implicit locking in each [`std::io::Write`] call
#[cfg(feature = "auto")]
pub fn stderr() -> Stderr {
    let stderr = std::io::stderr();
    AutoStream::auto(stderr)
}

/// Selection for overriding color output
pub use colorchoice::ColorChoice;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
