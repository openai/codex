//! # Custom [`Stream`]
//!
//! `winnow` is batteries included with support for
//! - Basic inputs like `&str`, newtypes with
//! - Improved debug output like [`Bytes`]
//! - [`Stateful`] for passing state through your parser, like tracking recursion
//!   depth
//! - [`LocatingSlice`] for looking up the absolute position of a token
//!
//! ## Implementing a custom token
//!
//! The first level of customization is parsing [`&[MyItem]`][Stream#impl-Stream-for-%26%5BT%5D]
//! or [`TokenSlice<MyItem>`].
//!
//! The basic traits you may want for a custom token type are:
//!
//! | trait | usage |
//! |---|---|
//! | [`AsChar`] |Transforms common types to a char for basic token parsing|
//! | [`ContainsToken`] |Look for the token in the given set|
//!
//! See also [`TokenSlice<MyItem>`], [lexing].
//!
//! ## Implementing a custom stream
//!
//! Let's assume we have an input type we'll call `MyStream`.
//! `MyStream` is a sequence of `MyItem` tokens.
//!
//! The goal is to define parsers with this signature: `&mut MyStream -> ModalResult<Output>`.
//! ```rust
//! # use winnow::prelude::*;
//! # type MyStream<'i> = &'i str;
//! # type Output<'i> = &'i str;
//! fn parser<'s>(i: &mut MyStream<'s>) -> ModalResult<Output<'s>> {
//!     "test".parse_next(i)
//! }
//! ```
//!
//! Like above, you'll need to implement the related token traits for `MyItem`.
//!
//! The traits you may want to implement for `MyStream` include:
//!
//! | trait | usage |
//! |---|---|
//! | [`Stream`] |Core trait for driving parsing|
//! | [`StreamIsPartial`] | Marks the input as being the complete buffer or a partial buffer for streaming input |
//! | [`AsBytes`] |Casts the input type to a byte slice|
//! | [`AsBStr`] |Casts the input type to a slice of ASCII / UTF-8-like bytes|
//! | [`Compare`] |Character comparison operations|
//! | [`FindSlice`] |Look for a substring in self|
//! | [`Location`] |Calculate location within initial input|
//! | [`Offset`] |Calculate the offset between slices|
//!
//! And for `&[MyItem]` (slices returned by [`Stream`]):
//!
//! | trait | usage |
//! |---|---|
//! | [`SliceLen`] |Calculate the input length|
//! | [`ParseSlice`] |Used to integrate `&str`'s `parse()` method|

#![allow(unused_imports)] // Here for intra-dock links
use super::lexing;
use crate::stream::*;
