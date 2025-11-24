//! # For `nom` users
//!
//! ## Migrating from `nom`
//!
//! For comparisons with `nom`, see
//! - [Why `winnow`][super::why]
//! - [parse-rosetta-rs](https://github.com/rosetta-rs/parse-rosetta-rs/)
//!
//! What approach you take depends on the size and complexity of your parser.
//! For small, simple parsers, its likely easiest to directly port from `nom`.
//! When trying to look for the equivalent of a `nom` combinator, search in the docs for the name
//! of the `nom` combinator.  It is expected that, where names diverge, a doc alias exists.
//! See also the [List of combinators][crate::combinator].
//!
//! ### Complex migrations
//!
//! For larger parsers, it is likely best to take smaller steps
//! - Easier to debug when something goes wrong
//! - Deprecation messages will help assist through the process
//!
//! The workflow goes something like:
//! 1. Run `cargo rm nom && cargo add winnow@0.3`
//! 1. Ensure everything compiles and tests pass, ignoring deprecation messages (see [migration
//!    notes](https://github.com/winnow-rs/winnow/blob/main/CHANGELOG.md#nom-migration-guide))
//! 1. Commit
//! 1. Switch any `impl FnMut(I) -> IResult<I, O, E>` to `impl Parser<I, O, E>`
//! 1. Resolve deprecation messages
//! 1. Commit
//! 1. Run `cargo add winnow@0.4`
//! 1. Ensure everything compiles and tests pass, ignoring deprecation messages (see [changelog](https://github.com/winnow-rs/winnow/blob/main/CHANGELOG.md#040---2023-03-18) for more details)
//! 1. Commit
//! 1. Resolve deprecation messages
//! 1. Commit
//! 1. Run `cargo add winnow@0.5`
//! 1. Ensure everything compiles and tests pass, ignoring deprecation messages
//!    (see [migration notes](https://github.com/winnow-rs/winnow/blob/main/CHANGELOG.md#050---2023-07-13))
//! 1. Commit
//! 1. Resolve deprecation messages
//! 1. Commit
//!
//! ### Examples
//!
//! For example migrations, see
//! - [git-config-env](https://github.com/gitext-rs/git-config-env/pull/11) (nom to winnow 0.3)
//! - [git-conventional](https://github.com/crate-ci/git-conventional/pull/37) (nom to winnow 0.3,
//!   adds explicit tracing for easier debugging)
//! - [typos](https://github.com/crate-ci/typos/pull/664) (nom to winnow 0.3)
//! - [cargo-smart-release](https://github.com/Byron/gitoxide/pull/948) (gradual migration from nom
//!   to winnow 0.5)
//! - [gix-config](https://github.com/Byron/gitoxide/pull/951) (gradual migration from nom
//!   to winnow 0.5)
//! - [gix-protocol](https://github.com/Byron/gitoxide/pull/1009) (gradual migration from nom
//!   to winnow 0.5)
//! - [gitoxide](https://github.com/Byron/gitoxide/pull/956) (gradual migration from nom
//!   to winnow 0.5)
//!
//! ## Differences
//!
//! These are key differences to help Nom users adapt to writing parsers with Winnow.
//!
//! ### Renamed APIs
//!
//! Names have changed for consistency or clarity.
//!
//! To find a parser you are looking for,
//! - Search the docs for the `nom` parser
//! - See the [List of combinators][crate::combinator]
//!
//! ### GATs
//!
//! `nom` v8 back-propagates how you will use a parser to parser functions using a language feature
//! called GATs.
//! Winnow has made the conscious choice not to use this feature, finding alternative ways of
//! getting most of the benefits.
//!
//! Benefits of GATs:
//! - Performance as the compiler is able to instantiate copies of a parser that are
//!   better tailored to how it will be used, like discarding unused allocations for output or
//!   errors.
//!
//! Benefits of not using GATs:
//! - Predictable performance:
//!   With GATs, seemingly innocuous changes like choosing to hand write a parser using idiomatic function parsers
//!   (`fn(&mut I) -> Result<O>`) can cause surprising slow downs because these functions sever the back-propagation from GATs.
//!   The causes of these slowdowns could be hard to identify by inspection or profiling.
//! - No "eek out X% perf improvement" pressure to contort a parser
//!   that is more easily written imperatively
//!   to be written declaratively
//!   so it can preserve the back-propagation from GATs.
//! - Built-in parsers serve are can serve as examples to users of idiomatic function parsers
//!   (`fn(&mut I) -> Result<O>`).
//!   With GATs, built-in parsers tend to be complex implementations of traits.
//! - Faster build times and smaller binary size as parsers only need to be generated for one mode,
//!   not upto 8
//!
//! #### Partial/streaming parsers
//!
//! `nom` v8 back-propagates whether `Parser::parse_complete` was used to select `complete`
//! parsers.
//! Previously, users had ensure consistently using a parser from the `streaming` or `complete` module.
//!
//! Instead, you tag the input type (`I`) by wrapping it in [`Partial<I>`] and parsers will adjust
//! their behavior accordingly.
//! See [partial] special topic.
//!
//! #### Eliding Output
//!
//! `nom` v8 back-propagates whether an Output will be used and skips its creation.
//! For example, `value(Null, many0(_))` will avoid creating and pushing to a `Vec`.
//! Previously, users had to select `count_many0` over `many0` to avoid creating a `Vec`.
//!
//! Instead, `repeat` returns an `impl Accumulate<T>` which could be a `Vec`, a `usize` for `count`
//! variants, or `()` to do no extra work.
//!
//! #### Eliding Backtracked Errors
//!
//! Under the hood, [`alt`] is an `if-not-error-else` ladder, see [`_tutorial::chapter_3`].
//! nom v8 back-propagates whether the error will be discarded and avoids any expensive work done
//! for rich error messages.
//!
//! Instead, [`ContextError`] and other changes have made it so errors have very little overhead.
//! [`dispatch!`] can also be used in some situations to avoid `alt`s overhead.
//!
//! ### Parsers return [`Stream::Slice`], rather than [`Stream`]
//!
//! In `nom`, parsers like [`take_while`] parse a [`Stream`] and return a [`Stream`].
//! When wrapping the input, like with [`Stateful`],
//! you have to unwrap the input to integrate it in your application,
//! and it requires [`Stream`] to be `Clone`
//! (which requires `RefCell` for mutable external state and can be expensive).
//!
//! Instead, [`Stream::Slice`] was added to track the intended type for parsers to return.
//! If you want to then parse the slice, you then need to take it and turn it back into a
//! [`Stream`].
//!
//! ### `&mut I`
//!
//! `winnow` switched from pure-function parser (`Fn(I) -> (I, O)` to `Fn(&mut I) -> O`).
//! On error, `i` is left pointing at where the error happened.
//!
//! Benefits of `Fn(&mut I) -> O`:
//! - Cleaner code: Removes need to pass `i` everywhere and makes changes to `i` more explicit
//! - Correctness: No forgetting to chain `i` through a parser
//! - Flexibility: `I` does not need to be `Copy` or even `Clone`. For example, [`Stateful`] can use `&mut S` instead of `RefCell<S>`.
//! - Performance: `Result::Ok` is smaller without `i`, reducing the risk that the output will be
//!   returned on the stack, rather than the much faster CPU registers.
//!   `Result::Err` can also be smaller because the error type does not need to carry `i` to point
//!   to the error.
//!   See also [#72](https://github.com/winnow-rs/winnow/issues/72).
//!
//! Benefits of `Fn(I) -> (I, O)`:
//! - Pure functions can be easier to reason about
//! - Less boilerplate in some situations (see below)
//! - Less syntactic noise in some situations (see below)
//!
//! When returning a slice from the input, you have to add a lifetime:
//! ```rust
//! # use winnow::prelude::*;
//! fn foo<'i>(i: &mut &'i str) -> ModalResult<&'i str> {
//! #   Ok("")
//!     // ...
//! }
//! ```
//!
//! When writing a closure, you need to annotate the type
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::combinator::alt;
//! # use winnow::error::ContextError;
//! # let mut input = "";
//! # fn foo<'i>() -> impl ModalParser<&'i str, &'i str, ContextError> {
//! alt((
//!     |i: &mut _| {
//! #       Ok("")
//!         // ...
//!     },
//!     |i: &mut _| {
//! #       Ok("")
//!         // ...
//!     },
//! ))
//! # }
//! ```
//! *(at least the full type isn't needed)*
//!
//! To save and restore from intermediate states, [`Stream::checkpoint`] and [`Stream::reset`] can help:
//! ```rust
//! use winnow::prelude::*;
//! # let mut i = "";
//! # let i = &mut i;
//!
//! let start = i.checkpoint();
//! // ...
//! i.reset(&start);
//! ```
//!
//! When the Output of a parser is a slice, you have to add a lifetime:
//! ```rust
//! # use winnow::prelude::*;
//! fn foo<'i>(i: &mut &'i str) -> ModalResult<&'i str> {
//!     // ...
//! #   winnow::token::rest.parse_next(i)
//! }
//! ```
//!
//! When writing a closure, you need to annotate the type:
//! ```rust
//! # use winnow::prelude::*;
//! # use winnow::combinator::trace;
//! fn foo(i: &mut &str) -> ModalResult<usize> {
//!     trace("foo", |i: &mut _| {
//!         // ...
//! #       Ok(0)
//!     }).parse_next(i)
//! }
//! ```
//!
//! ### Optional [`ErrMode`]
//!
//! Called `Err` in `nom`, [`ErrMode`] is responsible for
//! - Deciding whether to backtrack and try another branch in cases like `alt` or report back to
//!   the error back to users
//! - Tracking incomplete input on partial parsing
//!
//! As this isn't needed in every parser, it was made optional.  [`ModalResult`] is a convenience
//! type for using [`ErrMode`].

#![allow(unused_imports)]
use crate::_topic::partial;
use crate::_tutorial;
use crate::combinator::alt;
use crate::combinator::dispatch;
use crate::error::ContextError;
use crate::error::ErrMode;
use crate::error::ModalResult;
use crate::stream::Accumulate;
use crate::stream::Partial;
use crate::stream::Stateful;
use crate::stream::Stream;
use crate::token::take_while;
