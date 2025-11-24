//! # Chapter 6: Integrating the Parser
//!
//! So far, we've highlighted how to incrementally parse, but how do we bring this all together
//! into our application?
//!
//! Parsers we've been working with look like:
//! ```rust
//! # use winnow::error::ContextError;
//! # use winnow::Parser;
//! use winnow::Result;
//!
//! pub fn parser<'s>(input: &mut &'s str) -> Result<&'s str> {
//!     // ...
//! #     Ok("")
//! }
//! ```
//! 1. We have to decide what to do about the "remainder" of the `input`.
//! 2. The [`Result`] may not be compatible with the rest of the Rust ecosystem.
//!    Normally, Rust applications want errors that are `std::error::Error + Send + Sync + 'static`
//!    meaning:
//!    - They implement the [`std::error::Error`] trait
//!    - They can be sent across threads
//!    - They are safe to be referenced across threads
//!    - They do not borrow
//!
//! winnow provides [`Parser::parse`] to help with this:
//! - Ensures we hit [`eof`]
//! - Wraps the error in [`ParseError`]
//!   - For simple cases, [`ParseError`] provides a [`std::fmt::Display`] impl to render the error.
//!   - For more involved cases, [`ParseError`] provides the original [`input`][ParseError::input] and the
//!     [`offset`][ParseError::offset] of where it failed so you can capture this information in
//!     your error, [rendering it as you wish][chapter_7#error-adaptation-and-rendering].
//! - Converts from [`ModalResult`] to [`Result`] (if used, more on this in [`chapter_7`])
//!
//! However, [`ParseError`] will still need some level of adaptation to integrate with your
//! application's error type (like with `?`).
//!
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::Result;
//! # use winnow::token::take_while;
//! # use winnow::combinator::dispatch;
//! # use winnow::token::take;
//! # use winnow::combinator::fail;
//! use winnow::Parser;
//!
//! #[derive(Debug, PartialEq, Eq)]
//! pub struct Hex(usize);
//!
//! impl std::str::FromStr for Hex {
//!     type Err = anyhow::Error;
//!
//!     fn from_str(input: &str) -> Result<Self, Self::Err> {
//!         parse_digits
//!             .map(Hex)
//!             .parse(input)
//!             .map_err(|e| anyhow::format_err!("{e}"))
//!     }
//! }
//!
//! // ...
//! # fn parse_digits<'s>(input: &mut &'s str) -> Result<usize> {
//! #     dispatch!(take(2usize);
//! #         "0b" => parse_bin_digits.try_map(|s| usize::from_str_radix(s, 2)),
//! #         "0o" => parse_oct_digits.try_map(|s| usize::from_str_radix(s, 8)),
//! #         "0d" => parse_dec_digits.try_map(|s| usize::from_str_radix(s, 10)),
//! #         "0x" => parse_hex_digits.try_map(|s| usize::from_str_radix(s, 16)),
//! #         _ => fail,
//! #     ).parse_next(input)
//! # }
//! #
//! # fn parse_bin_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='1'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_oct_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='7'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_dec_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #     )).parse_next(input)
//! # }
//! #
//! # fn parse_hex_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//! #     take_while(1.., (
//! #         ('0'..='9'),
//! #         ('A'..='F'),
//! #         ('a'..='f'),
//! #     )).parse_next(input)
//! # }
//!
//! fn main() {
//!     let input = "0x1a2b";
//!     assert_eq!(input.parse::<Hex>().unwrap(), Hex(0x1a2b));
//!
//!     let input = "0x1a2b Hello";
//!     assert!(input.parse::<Hex>().is_err());
//!     let input = "ghiHello";
//!     assert!(input.parse::<Hex>().is_err());
//! }
//! ```

#![allow(unused_imports)]
use super::chapter_1;
use super::chapter_7;
use crate::combinator::eof;
use crate::error::ErrMode;
use crate::error::ParseError;
use crate::ModalResult;
use crate::Parser;

pub use super::chapter_5 as previous;
pub use super::chapter_7 as next;
pub use crate::_tutorial as table_of_contents;
