//! # Chapter 5: Repetition
//!
//! In [`chapter_3`], we covered how to sequence different parsers into a tuple but sometimes you need to run a
//! single parser multiple times, collecting the result into a container, like `Vec`.
//!
//! Let's collect the result of `parse_digits`:
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::Result;
//! # use winnow::token::take_while;
//! # use winnow::combinator::dispatch;
//! # use winnow::token::take;
//! # use winnow::combinator::fail;
//! use winnow::combinator::opt;
//! use winnow::combinator::repeat;
//! use winnow::combinator::terminated;
//!
//! fn parse_list(input: &mut &str) -> Result<Vec<usize>> {
//!     let mut list = Vec::new();
//!     while let Some(output) = opt(terminated(parse_digits, opt(','))).parse_next(input)? {
//!         list.push(output);
//!     }
//!     Ok(list)
//! }
//!
//! // ...
//! # fn parse_digits(input: &mut &str) -> Result<usize> {
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
//!     let mut input = "0x1a2b,0x3c4d,0x5e6f Hello";
//!
//!     let digits = parse_list.parse_next(&mut input).unwrap();
//!
//!     assert_eq!(input, " Hello");
//!     assert_eq!(digits, vec![0x1a2b, 0x3c4d, 0x5e6f]);
//!
//!     assert!(parse_digits(&mut "ghiWorld").is_err());
//! }
//! ```
//!
//! We can implement this declaratively with [`repeat`]:
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::Result;
//! # use winnow::token::take_while;
//! # use winnow::combinator::dispatch;
//! # use winnow::token::take;
//! # use winnow::combinator::fail;
//! use winnow::combinator::opt;
//! use winnow::combinator::repeat;
//! use winnow::combinator::terminated;
//!
//! fn parse_list(input: &mut &str) -> Result<Vec<usize>> {
//!     repeat(0..,
//!         terminated(parse_digits, opt(','))
//!     ).parse_next(input)
//! }
//! #
//! # fn parse_digits(input: &mut &str) -> Result<usize> {
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
//! #
//! # fn main() {
//! #     let mut input = "0x1a2b,0x3c4d,0x5e6f Hello";
//! #
//! #     let digits = parse_list.parse_next(&mut input).unwrap();
//! #
//! #     assert_eq!(input, " Hello");
//! #     assert_eq!(digits, vec![0x1a2b, 0x3c4d, 0x5e6f]);
//! #
//! #     assert!(parse_digits(&mut "ghiWorld").is_err());
//! # }
//! ```
//!
//! You'll notice that the above allows trailing `,`. However, if that's not desired, it
//! can easily be fixed by using [`separated`] instead of [`repeat`]:
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::Result;
//! # use winnow::token::take_while;
//! # use winnow::combinator::dispatch;
//! # use winnow::token::take;
//! # use winnow::combinator::fail;
//! use winnow::combinator::separated;
//!
//! fn parse_list(input: &mut &str) -> Result<Vec<usize>> {
//!     separated(0.., parse_digits, ",").parse_next(input)
//! }
//!
//! // ...
//! # fn parse_digits(input: &mut &str) -> Result<usize> {
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
//!     let mut input = "0x1a2b,0x3c4d,0x5e6f Hello";
//!
//!     let digits = parse_list.parse_next(&mut input).unwrap();
//!
//!     assert_eq!(input, " Hello");
//!     assert_eq!(digits, vec![0x1a2b, 0x3c4d, 0x5e6f]);
//!
//!     assert!(parse_digits(&mut "ghiWorld").is_err());
//! }
//! ```
//!
//! If you look closely at [`separated`] and [`repeat`], they aren't limited to collecting
//! the result into a `Vec`, but rather anything that implements the [`Accumulate`] trait.
//! [`Accumulate`] is for instance also implemented for [`HashSet`], [`String`] and `()`.
//!
//! This lets us build more complex parsers than we did in
//! [`chapter_2`] by accumulating the results into a `()` and [`take`][Parser::take]-ing
//! the consumed input.
//!
//! `take` works by
//! 1. Creating a [`checkpoint`][Stream::checkpoint]
//! 2. Running the inner parser, in our case the `parse_list` parser, which will advance the input
//! 3. Returning the slice from the first checkpoint to the current position.
//!
//! Since the result of `parse_list` gets thrown away, we
//! accumulates into a `()` to not waste work creating an unused `Vec`.
//!
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::Result;
//! # use winnow::token::take_while;
//! # use winnow::combinator::dispatch;
//! # use winnow::token::take;
//! # use winnow::combinator::fail;
//! # use winnow::combinator::separated;
//! #
//! fn take_list<'s>(input: &mut &'s str) -> Result<&'s str> {
//!     parse_list.take().parse_next(input)
//! }
//!
//! fn parse_list(input: &mut &str) -> Result<()> {
//!     separated(0.., parse_digits, ",").parse_next(input)
//! }
//!
//! # fn parse_digits(input: &mut &str) -> Result<usize> {
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
//!     let mut input = "0x1a2b,0x3c4d,0x5e6f Hello";
//!
//!     let digits = take_list.parse_next(&mut input).unwrap();
//!
//!     assert_eq!(input, " Hello");
//!     assert_eq!(digits, "0x1a2b,0x3c4d,0x5e6f");
//!
//!     assert!(parse_digits(&mut "ghiWorld").is_err());
//! }
//! ```
//! See [`combinator`] for more repetition parsers.

#![allow(unused_imports)]
use super::chapter_2;
use super::chapter_3;
use crate::combinator;
use crate::combinator::repeat;
use crate::combinator::separated;
use crate::stream::Accumulate;
use crate::stream::Stream;
use crate::Parser;
use std::collections::HashSet;

pub use super::chapter_4 as previous;
pub use super::chapter_6 as next;
pub use crate::_tutorial as table_of_contents;
