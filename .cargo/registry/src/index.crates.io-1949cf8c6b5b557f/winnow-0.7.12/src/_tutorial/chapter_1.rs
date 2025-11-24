//! # Chapter 1: The Winnow Way
//!
//! First of all, we need to understand the way that winnow thinks about parsing.
//! As discussed in the introduction, winnow lets us compose more complex parsers from more simple
//! ones (using "combinators").
//!
//! Let's discuss what a "parser" actually does. A parser takes an input and advances it until it returns
//! a result, where:
//!  - `Ok` indicates the parser successfully found what it was looking for; or
//!  - `Err` indicates the parser could not find what it was looking for.
//!
//! Parsers do more than just return a binary "success"/"failure" code.
//! - On success, the parser will return the processed data. The input will be advanced to the end of
//!   what was processed, pointing to what will be parsed next.
//! - If the parser failed, then there are multiple errors that could be returned.
//!   We'll explore this further in [`chapter_7`].
//!
//! ```text
//!                                  ┌─► Ok(what matched the parser)
//!             ┌────────┐           │
//! my input───►│a parser├──►either──┤
//!             └────────┘           └─► Err(...)
//! ```
//!
//!
//! To represent this model of the world, winnow uses the [`Result<O>`] type.
//! The `Ok` variant has `output: O`;
//! whereas the `Err` variant stores an error.
//!
//! You can import that from:
//!
//! ```rust
//! use winnow::Result;
//! ```
//!
//! To combine parsers, we need a common way to refer to them which is where the [`Parser<I, O, E>`]
//! trait comes in with [`Parser::parse_next`] being the primary way to drive
//! parsing forward.
//! In [`chapter_6`], we'll cover how to integrate these into your application, particularly with
//! [`Parser::parse`].
//!
//! You'll note that `I` and `O` are parameterized -- while most of the examples in this book
//! will be with `&str` (i.e. parsing a string); [they do not have to be strings][stream]; nor do they
//! have to be the same type (consider the simple example where `I = &str`, and `O = u64` -- this
//! parses a string into an unsigned integer.)
//!
//! # Let's write our first parser!
//!
//! The simplest parser we can write is one which successfully does nothing.
//!
//! To make it easier to implement a [`Parser`], the trait is implemented for
//! functions of the form `Fn(&mut I) -> Result<O>`.
//!
//! This parser function should take in a `&str`:
//!
//!  - Since it is supposed to succeed, we know it will return the `Ok` variant.
//!  - Since it does nothing to our input, the input will be left where it started.
//!  - Since it doesn't parse anything, it also should just return an empty string.
//!
//! ```rust
//! use winnow::Result;
//! use winnow::Parser;
//!
//! pub fn do_nothing_parser<'s>(input: &mut &'s str) -> Result<&'s str> {
//!     Ok("")
//! }
//!
//! fn main() {
//!     let mut input = "0x1a2b Hello";
//!
//!     let output = do_nothing_parser.parse_next(&mut input).unwrap();
//!     // Same as:
//!     // let output = do_nothing_parser(&mut input).unwrap();
//!
//!     assert_eq!(input, "0x1a2b Hello");
//!     assert_eq!(output, "");
//! }
//! ```

#![allow(unused_imports)]
use super::chapter_6;
use super::chapter_7;
use crate::Parser;
use crate::_topic::stream;

pub use super::chapter_0 as previous;
pub use super::chapter_2 as next;
pub use crate::_tutorial as table_of_contents;
