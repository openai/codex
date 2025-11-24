regex-syntax
============
This crate provides a robust regular expression parser.

[![Build status](https://github.com/rust-lang/regex/workflows/ci/badge.svg)](https://github.com/rust-lang/regex/actions)
[![Crates.io](https://img.shields.io/crates/v/regex-syntax.svg)](https://crates.io/crates/regex-syntax)
[![Rust](https://img.shields.io/badge/rust-1.28.0%2B-blue.svg?maxAge=3600)](https://github.com/rust-lang/regex)


### Documentation

https://docs.rs/regex-syntax


### Overview

There are two primary types exported by this crate: `Ast` and `Hir`. The former
is a faithful abstract syntax of a regular expression, and can convert regular
expressions back to their concrete syntax while mostly preserving its original
form. The latter type is a high level intermediate representation of a regular
expression that is amenable to analysis and compilation into byte codes or
automata. An `Hir` achieves this by drastically simplifying the syntactic
structure of the regular expression. While an `Hir` can be converted back to
its equivalent concrete syntax, the result is unlikely to resemble the original
concrete syntax that produced the `Hir`.


### Example

This example shows how to parse a pattern string into its HIR:

```rust
use regex_syntax::Parser;
use regex_syntax::hir::{self, Hir};

let hir = Parser::new().parse("a|b").unwrap();
assert_eq!(hir, Hir::alternation(vec![
    Hir::literal(hir::Literal::Unicode('a')),
    Hir::literal(hir::Literal::Unicode('b')),
]));
```


### Safety

This crate has no `unsafe` code and sets `forbid(unsafe_code)`. While it's
possible this crate could use `unsafe` code in the future, the standard
for doing so is extremely high. In general, most code in this crate is not
performance critical, since it tends to be dwarfed by the time it takes to
compile a regular expression into an automaton. Therefore, there is little need
for extreme optimization, and therefore, use of `unsafe`.

The standard for using `unsafe` in this crate is extremely high because this
crate is intended to be reasonably safe to use with user supplied regular
expressions. Therefore, while there may be bugs in the regex parser itself,
they should _never_ result in memory unsafety unless there is either a bug
in the compiler or the standard library. (Since `regex-syntax` has zero
dependencies.)


### Crate features

By default, this crate bundles a fairly large amount of Unicode data tables
(a source size of ~750KB). Because of their large size, one can disable some
or all of these data tables. If a regular expression attempts to use Unicode
data that is not available, then an error will occur when translating the `Ast`
to the `Hir`.

The full set of features one can disable are
[in the "Crate features" section of the documentation](https://docs.rs/regex-syntax/*/#crate-features).


### Testing

Simply running `cargo test` will give you very good coverage. However, because
of the large number of features exposed by this crate, a `test` script is
included in this directory which will test several feature combinations. This
is the same script that is run in CI.


### Motivation

The primary purpose of this crate is to provide the parser used by `regex`.
Specifically, this crate is treated as an implementation detail of the `regex`,
and is primarily developed for the needs of `regex`.

Since this crate is an implementation detail of `regex`, it may experience
breaking change releases at a different cadence from `regex`. This is only
possible because this crate is _not_ a public dependency of `regex`.

Another consequence of this de-coupling is that there is no direct way to
compile a `regex::Regex` from a `regex_syntax::hir::Hir`. Instead, one must
first convert the `Hir` to a string (via its `std::fmt::Display`) and then
compile that via `Regex::new`. While this does repeat some work, compilation
typically takes much longer than parsing.

Stated differently, the coupling between `regex` and `regex-syntax` exists only
at the level of the concrete syntax.
