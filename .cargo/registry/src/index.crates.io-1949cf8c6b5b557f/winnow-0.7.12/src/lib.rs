//! > winnow, making parsing a breeze
//!
//! `winnow` is a parser combinator library
//!
//! Quick links:
//! - [List of combinators][crate::combinator]
//! - [Tutorial][_tutorial::chapter_0]
//! - [Special Topics][_topic]
//! - [Discussions](https://github.com/winnow-rs/winnow/discussions)
//! - [CHANGELOG](https://github.com/winnow-rs/winnow/blob/v0.7.12/CHANGELOG.md) (includes major version migration
//!   guides)
//!
//! ## Aspirations
//!
//! `winnow` aims to be your "do everything" parser, much like people treat regular expressions.
//!
//! In roughly priority order:
//! 1. Support writing parser declaratively while not getting in the way of imperative-style
//!    parsing when needed, working as an open-ended toolbox rather than a close-ended framework.
//! 2. Flexible enough to be used for any application, including parsing strings, binary data,
//!    or separate [lexing and parsing phases][_topic::lexing]
//! 3. Zero-cost abstractions, making it easy to write high performance parsers
//! 4. Easy to use, making it trivial for one-off uses
//!
//! In addition:
//! - Resilient maintainership, including
//!   - Willing to break compatibility rather than batching up breaking changes in large releases
//!   - Leverage feature flags to keep one active branch
//! - We will support the last 6 months of rust releases (MSRV, currently 1.64.0)
//!
//! See also [Special Topic: Why winnow?][crate::_topic::why]
//!
//! ## Example
//!
//! Run
//! ```console
//! $ cargo add winnow
//! ```
//!
//! Then use it to parse:
//! ```rust
//! # #[cfg(feature = "alloc")] {
#![doc = include_str!("../examples/css/parser.rs")]
//! # }
//! ```
//!
//! See also the [Tutorial][_tutorial::chapter_0] and [Special Topics][_topic]

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, feature(extended_key_value_attributes))]
#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]
#![warn(missing_docs)]
#![warn(clippy::std_instead_of_core)]
#![warn(clippy::std_instead_of_alloc)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

#[cfg(feature = "alloc")]
#[cfg_attr(test, macro_use)]
#[allow(unused_extern_crates)]
extern crate alloc;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;

/// Lib module to re-export everything needed from `std` or `core`/`alloc`. This is how `serde` does
/// it, albeit there it is not public.
#[doc(hidden)]
pub(crate) mod lib {
    #![allow(unused_imports)]

    /// `std` facade allowing `std`/`core` to be interchangeable. Reexports `alloc` crate optionally,
    /// as well as `core` or `std`
    #[cfg(not(feature = "std"))]
    /// internal std exports for no_std compatibility
    pub(crate) mod std {
        #[doc(hidden)]
        #[cfg(not(feature = "alloc"))]
        pub(crate) use core::borrow;

        #[cfg(feature = "alloc")]
        #[doc(hidden)]
        pub(crate) use alloc::{borrow, boxed, collections, string, vec};

        #[doc(hidden)]
        pub(crate) use core::{
            cmp, convert, fmt, hash, iter, mem, ops, option, result, slice, str,
        };
    }

    #[cfg(feature = "std")]
    /// internal std exports for `no_std` compatibility
    pub(crate) mod std {
        #![allow(clippy::std_instead_of_core)]
        #![allow(clippy::std_instead_of_alloc)]
        #[doc(hidden)]
        pub(crate) use std::{
            borrow, boxed, cmp, collections, convert, fmt, hash, iter, mem, ops, result, slice,
            str, string, vec,
        };
    }
}

pub(crate) mod util {
    #[allow(dead_code)]
    pub(crate) fn from_fn<F: Fn(&mut core::fmt::Formatter<'_>) -> core::fmt::Result>(
        f: F,
    ) -> FromFn<F> {
        FromFn(f)
    }

    pub(crate) struct FromFn<F>(F)
    where
        F: Fn(&mut core::fmt::Formatter<'_>) -> core::fmt::Result;

    impl<F> core::fmt::Debug for FromFn<F>
    where
        F: Fn(&mut core::fmt::Formatter<'_>) -> core::fmt::Result,
    {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            (self.0)(f)
        }
    }

    impl<F> core::fmt::Display for FromFn<F>
    where
        F: Fn(&mut core::fmt::Formatter<'_>) -> core::fmt::Result,
    {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            (self.0)(f)
        }
    }
}

#[macro_use]
mod macros;

#[macro_use]
pub mod error;

mod parser;

pub mod stream;

pub mod ascii;
pub mod binary;
pub mod combinator;
pub mod token;

#[cfg(feature = "unstable-doc")]
pub mod _topic;
#[cfg(feature = "unstable-doc")]
pub mod _tutorial;

/// Core concepts available for glob import
///
/// Including
/// - [`StreamIsPartial`][crate::stream::StreamIsPartial]
/// - [`Parser`]
///
/// ## Example
///
/// ```rust
/// use winnow::prelude::*;
///
/// fn parse_data(input: &mut &str) -> ModalResult<u64> {
///     // ...
/// #   winnow::ascii::dec_uint(input)
/// }
///
/// fn main() {
///   let result = parse_data.parse("100");
///   assert_eq!(result, Ok(100));
/// }
/// ```
pub mod prelude {
    pub use crate::error::ModalError as _;
    pub use crate::error::ParserError as _;
    pub use crate::stream::AsChar as _;
    pub use crate::stream::ContainsToken as _;
    pub use crate::stream::Stream as _;
    pub use crate::stream::StreamIsPartial as _;
    pub use crate::ModalParser;
    pub use crate::ModalResult;
    pub use crate::Parser;
    #[cfg(feature = "unstable-recover")]
    #[cfg(feature = "std")]
    pub use crate::RecoverableParser as _;

    #[cfg(test)]
    pub(crate) use crate::TestResult;
}

pub use error::ModalResult;
pub use error::Result;
pub use parser::*;
pub use stream::BStr;
pub use stream::Bytes;
pub use stream::LocatingSlice;
pub use stream::Partial;
pub use stream::Stateful;
pub use stream::Str;

#[cfg(test)]
pub(crate) use error::TestResult;
