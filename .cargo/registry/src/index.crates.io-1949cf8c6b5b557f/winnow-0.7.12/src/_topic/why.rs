//! # Why `winnow`?
//!
//! To answer this question, it will be useful to contrast this with other approaches to parsing.
//!
//! <div class="warning">
//!
//! **Note:** This will focus on principles and priorities. For a deeper and wider wider
//! comparison with other Rust parser libraries, see
//! [parse-rosetta-rs](https://github.com/rosetta-rs/parse-rosetta-rs).
//!
//! </div>
//!
//! ## Hand-written parsers
//!
//! Typically, a hand-written parser gives you the flexibility to get
//! - Fast parse performance
//! - Fast compile-time
//! - Small binary sizes
//! - High quality error message
//! - Fewer dependencies to audit
//!
//! However, this comes at the cost of doing it all yourself, including
//! - Optimizing for each of the above characteristics you care about
//! - Ensuring the safety of any `unsafe` code (buffer overflows being a common bug with parsers)
//! - Being aware of, familiar with, and correctly implement the relevant algorithms.
//!   matklad, who has written two rust compile frontends, commented
//!   ["I’ve implemented a production-grade Pratt parser once, but I no longer immediately understand that code :-)"](https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html)
//!
//! This approach works well if:
//! - Your format is small and is unlikely to change
//! - Your format is large but you have people who can focus solely on parsing, like with large
//!   programming languages
//!
//! ## `winnow`
//!
//! Unlike traditional programming language parsers that use
//! [lex](https://en.wikipedia.org/wiki/Lex_(software)) or
//! [yacc](https://en.wikipedia.org/wiki/Yacc), you can think of `winnow` as a general version of
//! the helpers you would create along the way to writing a hand-written parser.
//!
//! `winnow` includes support for:
//! - Zero-copy parsing
//! - [Parse traces][trace] for easier debugging
//! - [Streaming parsing][Partial] for network communication or large file
//! - [Stateful] parsers
//!
//! For binary formats, `winnow` includes:
//! - [A hexadecimal view][crate::Bytes] in [trace]
//! - [TLV](https://en.wikipedia.org/wiki/Type-length-value) (e.g. [`length_take`])
//! - Some common parsers to help get started, like numbers
//!
//! For text formats, `winnow` includes:
//! - [Tracking of spans][crate::LocatingSlice]
//! - [A textual view when parsing as bytes][crate::BStr] in [trace]
//! - Ability to evaluate directly, parse to an AST, or lex and parse the format
//!
//! This works well for:
//! - Prototyping for what will be a hand-written parser
//! - When you want to minimize the work to evolve your format
//! - When you don't have contributors focused solely on parsing and your grammar is large enough
//!   to be unwieldy to hand write.
//!
//! ## `nom`
//!
//! `winnow` is a fork of the venerable [`nom`](https://crates.io/crates/nom). The difference
//! between them is largely in priorities.  `nom` prioritizes:
//! - Lower churn for existing users while `winnow` is trying to find ways to make things better
//!   for the parsers yet to be written.
//! - Having a small core, relying on external crates like
//!   [`nom-locate`](https://crates.io/crates/nom_locate) and
//!   [`nom-supreme`](https://crates.io/crates/nom-supreme), encouraging flexibility among users
//!   and to not block users on new features being merged while `winnow` aims to include all the
//!   fundamentals for parsing to ensure the experience is cohesive and high quality.
//!
//! For more details, see the [design differences][super::nom#api-differences].
//!
//! See also our [nom migration guide][super::nom#migrating-from-nom].
//!
//! ## `chumsky`
//!
//! [`chumsky`](https://crates.io/crates/chumsky) is an up and coming parser-combinator library
//! that includes advanced features like error recovery.
//!
//! Probably the biggest diverging philosophy is `chumsky`s stance:
//!
//! > "If you need to implement either `Parser` or `Strategy` by hand, that's a problem that needs fixing".
//!
//! This is under "batteries included" but it also ties into the feeling that `chumsky` acts more like
//! a framework. Instead of composing together helpers, you are expected to do everything through
//! their system to the point that it is non-trivial to implement their `Parser` trait and are
//! encouraged to use the
//! [`custom`](https://docs.rs/chumsky/0.9.0/chumsky/primitive/fn.custom.html) helper. This
//! requires re-framing everything to fit within their model and makes the code harder to understand
//! and debug as you are working with abstract operations that will eventually be applied
//! rather than directly with the parsers.
//!
//! In contrast, `winnow` is an introspectable toolbox that can easily be customized at any level.
//! Probably the biggest thing that `winnow` loses out on is optimizations from ["parse modes" via
//! GATs](https://github.com/zesterer/chumsky/pull/82) which allows downstream parsers to tell
//! upstream parsers when information will be discarded, allowing bypassing expensive operations,
//! like allocations. This requires a lot more complex interaction with parsers that isn't as
//! trivial to do with bare functions which would lose out on any of that side-band information.
//! Instead, we work around this with things like the [`Accumulate`] trait.

#![allow(unused_imports)]
use crate::binary::length_take;
use crate::combinator::trace;
use crate::stream::Accumulate;
use crate::stream::Partial;
use crate::stream::Stateful;
