//! # Chapter 2: Tokens and Tags
//!
//! The simplest *useful* parser you can write is one which matches tokens.
//! In our case, tokens are `char`.
//!
//! ## Tokens
//!
//! [`Stream`] provides some core operations to help with parsing. For example, to process a
//! single token, you can do:
//! ```rust
//! # use winnow::Parser;
//! # use winnow::Result;
//! use winnow::stream::Stream;
//! use winnow::error::ParserError;
//!
//! fn parse_prefix(input: &mut &str) -> Result<char> {
//!     let c = input.next_token().ok_or_else(|| {
//!         ParserError::from_input(input)
//!     })?;
//!     if c != '0' {
//!         return Err(ParserError::from_input(input));
//!     }
//!     Ok(c)
//! }
//!
//! fn main()  {
//!     let mut input = "0x1a2b Hello";
//!
//!     let output = parse_prefix.parse_next(&mut input).unwrap();
//!
//!     assert_eq!(input, "x1a2b Hello");
//!     assert_eq!(output, '0');
//!
//!     assert!(parse_prefix.parse_next(&mut "d").is_err());
//! }
//! ```
//!
//! This extraction of a token is encapsulated in the [`any`] parser:
//! ```rust
//! # use winnow::Result;
//! # use winnow::error::ParserError;
//! use winnow::Parser;
//! use winnow::token::any;
//!
//! fn parse_prefix(input: &mut &str) -> Result<char> {
//!     let c = any
//!         .parse_next(input)?;
//!     if c != '0' {
//!         return Err(ParserError::from_input(input));
//!     }
//!     Ok(c)
//! }
//! #
//! # fn main()  {
//! #     let mut input = "0x1a2b Hello";
//! #
//! #     let output = parse_prefix.parse_next(&mut input).unwrap();
//! #
//! #     assert_eq!(input, "x1a2b Hello");
//! #     assert_eq!(output, '0');
//! #
//! #     assert!(parse_prefix.parse_next(&mut "d").is_err());
//! # }
//! ```
//!
//! Using the higher level [`any`] parser opens `parse_prefix` to the helpers on the [`Parser`] trait,
//! like [`Parser::verify`] which fails a parse if a condition isn't met, like our check above:
//! ```rust
//! # use winnow::Result;
//! use winnow::Parser;
//! use winnow::token::any;
//!
//! fn parse_prefix(input: &mut &str) -> Result<char> {
//!     let c = any
//!         .verify(|c| *c == '0')
//!         .parse_next(input)?;
//!     Ok(c)
//! }
//! #
//! # fn main()  {
//! #     let mut input = "0x1a2b Hello";
//! #
//! #     let output = parse_prefix.parse_next(&mut input).unwrap();
//! #
//! #     assert_eq!(input, "x1a2b Hello");
//! #     assert_eq!(output, '0');
//! #
//! #     assert!(parse_prefix.parse_next(&mut "d").is_err());
//! # }
//! ```
//!
//! Matching a single token literal is common enough that [`Parser`] is implemented for
//! the `char` type, encapsulating both [`any`] and [`Parser::verify`]:
//! ```rust
//! # use winnow::Result;
//! use winnow::Parser;
//!
//! fn parse_prefix(input: &mut &str) -> Result<char> {
//!     let c = '0'.parse_next(input)?;
//!     Ok(c)
//! }
//! #
//! # fn main()  {
//! #     let mut input = "0x1a2b Hello";
//! #
//! #     let output = parse_prefix.parse_next(&mut input).unwrap();
//! #
//! #     assert_eq!(input, "x1a2b Hello");
//! #     assert_eq!(output, '0');
//! #
//! #     assert!(parse_prefix.parse_next(&mut "d").is_err());
//! # }
//! ```
//!
//! ## Tags
//!
//! [`Stream`] also supports processing slices of tokens:
//! ```rust
//! # use winnow::Parser;
//! # use winnow::Result;
//! use winnow::stream::Stream;
//! use winnow::error::ParserError;
//!
//! fn parse_prefix<'s>(input: &mut &'s str) -> Result<&'s str> {
//!     let expected = "0x";
//!     if input.len() < expected.len() {
//!         return Err(ParserError::from_input(input));
//!     }
//!     let actual = input.next_slice(expected.len());
//!     if actual != expected {
//!         return Err(ParserError::from_input(input));
//!     }
//!     Ok(actual)
//! }
//!
//! fn main()  {
//!     let mut input = "0x1a2b Hello";
//!
//!     let output = parse_prefix.parse_next(&mut input).unwrap();
//!     assert_eq!(input, "1a2b Hello");
//!     assert_eq!(output, "0x");
//!
//!     assert!(parse_prefix.parse_next(&mut "0o123").is_err());
//! }
//! ```
//!
//! Matching the input position against a string literal is encapsulated in the [`literal`] parser:
//! ```rust
//! # use winnow::Result;
//! # use winnow::Parser;
//! use winnow::token::literal;
//!
//! fn parse_prefix<'s>(input: &mut &'s str) -> Result<&'s str> {
//!     let expected = "0x";
//!     let actual = literal(expected).parse_next(input)?;
//!     Ok(actual)
//! }
//! #
//! # fn main()  {
//! #     let mut input = "0x1a2b Hello";
//! #
//! #     let output = parse_prefix.parse_next(&mut input).unwrap();
//! #     assert_eq!(input, "1a2b Hello");
//! #     assert_eq!(output, "0x");
//! #
//! #     assert!(parse_prefix.parse_next(&mut "0o123").is_err());
//! # }
//! ```
//!
//! Like for a single token, matching a string literal is common enough that [`Parser`] is implemented for the `&str` type:
//! ```rust
//! # use winnow::Result;
//! use winnow::Parser;
//!
//! fn parse_prefix<'s>(input: &mut &'s str) -> Result<&'s str> {
//!     let actual = "0x".parse_next(input)?;
//!     Ok(actual)
//! }
//! #
//! # fn main()  {
//! #     let mut input = "0x1a2b Hello";
//! #
//! #     let output = parse_prefix.parse_next(&mut input).unwrap();
//! #     assert_eq!(input, "1a2b Hello");
//! #     assert_eq!(output, "0x");
//! #
//! #     assert!(parse_prefix.parse_next(&mut "0o123").is_err());
//! # }
//! ```
//!
//! See [`token`] for additional individual and token-slice parsers.
//!
//! ## Character Classes
//!
//! Selecting a single `char` or a [`literal`] is fairly limited. Sometimes, you will want to select one of several
//! `chars` of a specific class, like digits. For this, we use the [`one_of`] parser:
//!
//! ```rust
//! # use winnow::Parser;
//! # use winnow::Result;
//! use winnow::token::one_of;
//!
//! fn parse_digits(input: &mut &str) -> Result<char> {
//!     one_of(('0'..='9', 'a'..='f', 'A'..='F')).parse_next(input)
//! }
//!
//! fn main() {
//!     let mut input = "1a2b Hello";
//!
//!     let output = parse_digits.parse_next(&mut input).unwrap();
//!     assert_eq!(input, "a2b Hello");
//!     assert_eq!(output, '1');
//!
//!     assert!(parse_digits.parse_next(&mut "Z").is_err());
//! }
//! ```
//!
//! > **Aside:** [`one_of`] might look straightforward, a function returning a value that implements `Parser`.
//! > Let's look at it more closely as its used above (resolving all generic parameters):
//! > ```rust
//! > # use winnow::prelude::*;
//! > # use winnow::error::ContextError;
//! > pub fn one_of<'i>(
//! >     list: &'static [char]
//! > ) -> impl Parser<&'i str, char, ContextError> {
//! >     // ...
//! > #    winnow::token::one_of(list)
//! > }
//! > ```
//! > If you have not programmed in a language where functions are values, the type signature of the
//! > [`one_of`] function might be a surprise.
//! > The function [`one_of`] *returns a function*. The function it returns is a
//! > [`Parser`], taking a `&str` and returning an [`Result`]. This is a common pattern in winnow for
//! > configurable or stateful parsers.
//!
//! Some of character classes are common enough that a named parser is provided, like with:
//! - [`line_ending`][crate::ascii::line_ending]: Recognizes an end of line (both `\n` and `\r\n`)
//! - [`newline`][crate::ascii::newline]: Matches a newline character `\n`
//! - [`tab`][crate::ascii::tab]: Matches a tab character `\t`
//!
//! You can then capture sequences of these characters with parsers like [`take_while`].
//! ```rust
//! # use winnow::Parser;
//! # use winnow::Result;
//! use winnow::token::take_while;
//!
//! fn parse_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//!     take_while(1.., ('0'..='9', 'a'..='f', 'A'..='F')).parse_next(input)
//! }
//!
//! fn main() {
//!     let mut input = "1a2b Hello";
//!
//!     let output = parse_digits.parse_next(&mut input).unwrap();
//!     assert_eq!(input, " Hello");
//!     assert_eq!(output, "1a2b");
//!
//!     assert!(parse_digits.parse_next(&mut "Z").is_err());
//! }
//! ```
//!
//! We could simplify this further by using one of the built-in character classes, [`hex_digit1`]:
//! ```rust
//! # use winnow::Parser;
//! # use winnow::Result;
//! use winnow::ascii::hex_digit1;
//!
//! fn parse_digits<'s>(input: &mut &'s str) -> Result<&'s str> {
//!     hex_digit1.parse_next(input)
//! }
//!
//! fn main() {
//!     let mut input = "1a2b Hello";
//!
//!     let output = parse_digits.parse_next(&mut input).unwrap();
//!     assert_eq!(input, " Hello");
//!     assert_eq!(output, "1a2b");
//!
//!     assert!(parse_digits.parse_next(&mut "Z").is_err());
//! }
//! ```
//!
//! See [`ascii`] for more text-based parsers.

#![allow(unused_imports)]
use crate::ascii;
use crate::ascii::hex_digit1;
use crate::stream::ContainsToken;
use crate::stream::Stream;
use crate::token;
use crate::token::any;
use crate::token::literal;
use crate::token::one_of;
use crate::token::take_while;
use crate::Parser;
use crate::Result;
use std::ops::RangeInclusive;

pub use super::chapter_1 as previous;
pub use super::chapter_3 as next;
pub use crate::_tutorial as table_of_contents;
