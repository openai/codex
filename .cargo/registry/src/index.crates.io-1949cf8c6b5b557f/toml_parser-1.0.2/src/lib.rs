//! TOML lexer and parser
//!
//! Characteristics:
//! - Error recovery
//! - Lazy validation
//! - `forbid(unsafe)` by default, requiring the `unsafe` feature otherwise
//! - `no_std` support, including putting users in charge of allocation choices (including not
//!   allocating)
//!
//! Full parsing is broken into three phases:
//! 1. [Lexing tokens][lexer]
//! 2. [Parsing tokens][parser] (push parser)
//! 3. Organizing the physical layout into the logical layout,
//!    including [decoding keys and values][decoder]

#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![cfg_attr(not(feature = "unsafe"), forbid(unsafe_code))]
#![warn(clippy::std_instead_of_core)]
#![warn(clippy::std_instead_of_alloc)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[macro_use]
mod macros;

#[cfg(feature = "debug")]
pub(crate) mod debug;
mod error;
mod source;

pub mod decoder;
pub mod lexer;
pub mod parser;

pub use error::ErrorSink;
pub use error::Expected;
pub use error::ParseError;
pub use source::Raw;
pub use source::Source;
pub use source::SourceIndex;
pub use source::Span;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
