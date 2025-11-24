//! ANSI Text Styling
//!
//! *A portmanteau of "ansi style"*
//!
//! `anstyle` provides core types describing [ANSI styling escape
//! codes](https://en.wikipedia.org/wiki/ANSI_escape_code) for interoperability
//! between crates.
//!
//! Example use cases:
//! - An argument parser allowing callers to define the colors used in the help-output without
//!   putting the text formatting crate in the public API
//! - A style description parser that can work with any text formatting crate
//!
//! Priorities:
//! 1. API stability
//! 2. Low compile-time and binary-size overhead
//! 3. `const` friendly API for callers to statically define their stylesheet
//!
//! For integration with text styling crate, see:
//! - [anstyle-ansi-term](https://docs.rs/anstyle-ansi-term)
//! - [anstyle-crossterm](https://docs.rs/anstyle-crossterm)
//! - [anstyle-owo-colors](https://docs.rs/anstyle-owo-colors)
//! - [anstyle-termcolor](https://docs.rs/anstyle-termcolor)
//! - [anstyle-yansi](https://docs.rs/anstyle-yansi)
//!
//! User-styling parsers:
//! - [anstyle-git](https://docs.rs/anstyle-git): Parse Git style descriptions
//! - [anstyle-ls](https://docs.rs/anstyle-ls): Parse `LS_COLORS` style descriptions
//!
//! Convert to other formats
//! - [anstream](https://docs.rs/anstream): A simple cross platform library for writing colored text to a terminal
//! - [anstyle-roff](https://docs.rs/anstyle-roff): For converting to ROFF
//! - [anstyle-syntect](https://docs.rs/anstyle-syntect): For working with syntax highlighting
//!
//! Utilities
//! - [anstyle-lossy](https://docs.rs/anstyle-lossy): Convert between `anstyle::Color` types
//! - [anstyle-parse](https://docs.rs/anstyle-parse): Parsing ANSI Style Escapes
//! - [anstyle-wincon](https://docs.rs/anstyle-wincon): Styling legacy Microsoft terminals
//!
//! # Examples
//!
//! The core type is [`Style`]:
//! ```rust
//! let style = anstyle::Style::new().bold();
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![warn(missing_docs)]
#![warn(clippy::std_instead_of_core)]
#![warn(clippy::std_instead_of_alloc)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

#[macro_use]
mod macros;

mod color;
mod effect;
mod reset;
mod style;

pub use color::*;
pub use effect::*;
pub use reset::*;
pub use style::*;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
