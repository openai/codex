//! # Chapter 4: Parsers With Custom Return Types
//!
//! So far, we have seen mostly functions that take an `&str`, and return a
//! [`Result<&str>`]. Splitting strings into smaller strings and characters is certainly
//! useful, but it's not the only thing winnow is capable of!
//!
//! A useful operation when parsing is to convert between types; for example
//! parsing from `&str` to another primitive, like [`usize`].
//!
//! All we need to do for our parser to return a different type is to change
//! the type parameter of [`Result`] to the desired return type.
//! For example, to return a `usize`, return a `Result<usize>`.
//!
//! One winnow-native way of doing a type conversion is to use the
//! [`Parser::parse_to`] combinator
//! to convert from a successful parse to a particular type using [`FromStr`].
//!
//! The following code converts from a string containing a number to `usize`:
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::Result;
//! # use winnow::ascii::digit1;
//! #
//! fn parse_digits(input: &mut &str) -> Result<usize> {
//!     digit1
//!         .parse_to()
//!         .parse_next(input)
//! }
//!
//! fn main() {
//!     let mut input = "1024 Hello";
//!
//!     let output = parse_digits.parse_next(&mut input).unwrap();
//!     assert_eq!(input, " Hello");
//!     assert_eq!(output, 1024);
//!
//!     assert!(parse_digits(&mut "Z").is_err());
//! }
//! ```
//!
//! `Parser::parse_to` is just a convenient form of [`Parser::try_map`] which we can use to handle
//! all radices of numbers:
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::Result;
//! # use winnow::token::take_while;
//! use winnow::combinator::dispatch;
//! use winnow::token::take;
//! use winnow::combinator::fail;
//!
//! fn parse_digits(input: &mut &str) -> Result<usize> {
//!     dispatch!(take(2usize);
//!         "0b" => parse_bin_digits.try_map(|s| usize::from_str_radix(s, 2)),
//!         "0o" => parse_oct_digits.try_map(|s| usize::from_str_radix(s, 8)),
//!         "0d" => parse_dec_digits.try_map(|s| usize::from_str_radix(s, 10)),
//!         "0x" => parse_hex_digits.try_map(|s| usize::from_str_radix(s, 16)),
//!         _ => fail,
//!     ).parse_next(input)
//! }
//!
//! // ...
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
//!     let mut input = "0x1a2b Hello";
//!
//!     let digits = parse_digits.parse_next(&mut input).unwrap();
//!
//!     assert_eq!(input, " Hello");
//!     assert_eq!(digits, 0x1a2b);
//!
//!     assert!(parse_digits(&mut "ghiWorld").is_err());
//! }
//! ```
//!
//! See also [`Parser`] for more output-modifying parsers.

#![allow(unused_imports)]
use crate::Parser;
use crate::Result;
use std::str::FromStr;

pub use super::chapter_3 as previous;
pub use super::chapter_5 as next;
pub use crate::_tutorial as table_of_contents;
