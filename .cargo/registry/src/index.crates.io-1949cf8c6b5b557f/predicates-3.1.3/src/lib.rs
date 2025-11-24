// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/license/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Composable first-order predicate functions.
//!
//! This library implements an interface to "predicates" - boolean-valued
//! functions of one argument. This allows combinatorial logic to be created and
//! assembled at runtime and then used one or more times for evaluating values.
//! This sort of object is really useful when creating filters and checks that
//! can be changed at runtime with user interaction - it allows a clean
//! separation of concerns where the configuration code can be used to build up
//! a predicate, and then that predicate can be given to the code that does the
//! actual filtering without the filtering code knowing anything about user
//! configuration. See the examples for how this can work.
//!
//! ## Installation
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! predicates = "3.1.3"
//! ```
//!
//! A [prelude] is available to bring in all extension traits as well as providing
//! `prelude::predicate` which focuses on the 90% case of the API.
//! ```rust
//! use predicates::prelude::*;
//! ```
//!
//! ## Examples
//!
//! The simplest predicates are [`predicate::always`] and [`predicate::never`], which always
//! returns `true` and always returns `false`, respectively. The values are simply ignored when
//! evaluating against these predicates:
//! ```rust
//! use predicates::prelude::*;
//!
//! let always_true = predicate::always();
//! assert_eq!(true, always_true.eval(&5));
//! let always_false = predicate::never();
//! assert_eq!(false, always_false.eval(&5));
//! ```
//!
//! Pre-made predicates are available for types that implement the `PartialOrd` and
//! `PartialEq` traits. The following example uses `lt`, but `eq`, `ne`, `le`, `gt`,
//! `ge` are also available.
//! ```rust
//! use predicates::prelude::*;
//!
//! let less_than_ten = predicate::lt(10);
//! assert_eq!(true, less_than_ten.eval(&9));
//! assert_eq!(false, less_than_ten.eval(&11));
//! ```
//!
//! Any function over a reference to the desired `Item` that returns `bool`
//! can easily be made into a `Predicate` using the [`predicate::function`]
//! function.
//! ```rust
//! use predicates::prelude::*;
//!
//! let bound = 5;
//! let predicate_fn = predicate::function(|&x| x >= bound);
//! let between_5_and_10 = predicate_fn.and(predicate::le(10));
//! assert_eq!(true, between_5_and_10.eval(&7));
//! assert_eq!(false, between_5_and_10.eval(&3));
//! ```
//!
//! The `Predicate` type is actually a trait, and that trait implements a
//! number of useful combinator functions. For example, evaluating for a value
//! between two other values can be accomplished as follows:
//! ```rust
//! use predicates::prelude::*;
//!
//! let between_5_and_10 = predicate::ge(5).and(predicate::le(10));
//! assert_eq!(true, between_5_and_10.eval(&7));
//! assert_eq!(false, between_5_and_10.eval(&11));
//! assert_eq!(false, between_5_and_10.eval(&4));
//! ```
//!
//! The `Predicate` trait is pretty simple, the core of it is an
//! implementation of a `eval` function that takes a single argument and
//! returns a `bool`. Implementing a custom `Predicate` still allows all the
//! usual combinators of the `Predicate` trait to work!
//! ```rust
//! use std::fmt;
//!
//! use predicates::prelude::*;
//!
//! struct IsTheAnswer;
//! impl Predicate<i32> for IsTheAnswer {
//!     fn eval(&self, variable: &i32) -> bool {
//!         *variable == 42
//!     }
//! }
//! impl predicates::reflection::PredicateReflection for IsTheAnswer {}
//! impl fmt::Display for IsTheAnswer {
//!     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//!         write!(f, "var.is_the_answer()")
//!     }
//! }
//!
//! assert_eq!(true, IsTheAnswer.eval(&42));
//! let almost_the_answer = IsTheAnswer.or(predicate::in_iter(vec![41, 43]));
//! assert_eq!(true, almost_the_answer.eval(&41));
//! ```
//!
//! ## Choosing a Predicate
//!
//! General predicates
//! - [`predicate::always`]
//! - [`predicate::never`]
//! - [`predicate::function`]
//! - [`predicate::in_iter`]: Specified value must be in the `Iterator`.
//!   - [`predicate::in_iter(...).sort`]: Optimization for repeatedly called predicates.
//!   - [`predicate::in_hash`]: Optimization for repeatedly called predicates.
//! - [`predicate::eq`]
//!   - [`predicate::float::is_close`]: Use this instead of `eq` for floating point values.
//! - [`predicate::ne`]
//! - [`predicate::ge`]
//! - [`predicate::gt`]
//! - [`predicate::le`]
//! - [`predicate::lt`]
//! - [`predicate::name`]: Improve readability of failure reporting by providing a meaningful name.
//!
//! Combinators
//! - [`pred_a.and(pred_b)`]: Both predicates must succeed.
//! - [`pred_a.or(pred_b)`]: One or both predicates must succeed.
//! - [`pred_a.not()`]: The predicate must fail.
//!
//! `String` predicates
//! - [`predicate::str::is_empty`]: Specified string must be empty
//! - [`str_pred = predicate::path::eq_file(...).utf8`]: Specified string must equal the contents
//!   of the given file.
//! - [`predicate::str::diff`]: Same as `eq` except report a diff.  See [`DifferencePredicate`]
//!   for more features.
//! - [`predicate::str::starts_with`]: Specified string must start with the given needle.
//! - [`predicate::str::ends_with`]: Specified string must end with the given needle.
//! - [`predicate::str::contains`]: Specified string must contain the given needle.
//!   - [`predicate::str::contains(...).count`]: Required number of times the needle must show up.
//! - [`predicate::str::is_match`]: Specified string must match the given regex.
//!   - [`predicate::str::is_match(...).count`]: Required number of times the match must show up.
//! - [`str_pred.trim`]: Trim whitespace before passing it to `str_pred`.
//! - [`str_pred.normalize`]: Normalize the line endings before passing it to `str_pred`.
//! - [`bytes_pred = str_pred.from_utf8()`]: Reuse string predicates in other contexts, like the
//!   file system.
//!
//! File system predicates
//! - [`predicate::path::exists`]: Specified path must exist on disk.
//! - [`predicate::path::missing`]: Specified path must not exist on disk.
//! - [`predicate::path::is_dir`]: Specified path is a directory.
//! - [`predicate::path::is_file`]: Specified path is a file.
//! - [`predicate::path::is_symlink`]: Specified path is a symlink.
//! - [`path_pred = predicate::path::eq_file`]: Specified path's contents must equal the contents of the given
//!   file.
//! - [`path_pred = bytes_pred.from_file_path`]: Specified path's contents must equal the `bytes_pred`.
//!
//! [`DifferencePredicate`]: crate::str::DifferencePredicate
//! [`bytes_pred = str_pred.from_utf8()`]: prelude::PredicateStrExt::from_utf8()
//! [`path_pred = bytes_pred.from_file_path`]: prelude::PredicateFileContentExt::from_file_path()
//! [`path_pred = predicate::path::eq_file`]: prelude::predicate::path::eq_file()
//! [`pred_a.and(pred_b)`]: boolean::PredicateBooleanExt::and()
//! [`pred_a.not()`]: boolean::PredicateBooleanExt::not()
//! [`pred_a.or(pred_b)`]: boolean::PredicateBooleanExt::or()
//! [`predicate::always`]: constant::always()
//! [`predicate::eq`]: ord::eq()
//! [`predicate::float::is_close`]: prelude::predicate::float::is_close()
//! [`predicate::function`]: function::function()
//! [`predicate::ge`]: ord::ge()
//! [`predicate::gt`]: ord::gt()
//! [`predicate::in_hash`]: iter::in_hash()
//! [`predicate::in_iter(...).sort`]: iter::InPredicate::sort()
//! [`predicate::in_iter`]: iter::in_iter()
//! [`predicate::le`]: ord::le()
//! [`predicate::lt`]: ord::lt()
//! [`predicate::name`]: name::PredicateNameExt::name()
//! [`predicate::ne`]: ord::ne()
//! [`predicate::never`]: constant::never()
//! [`predicate::path::exists`]: prelude::predicate::path::exists()
//! [`predicate::path::is_dir`]: prelude::predicate::path::is_dir()
//! [`predicate::path::is_file`]: prelude::predicate::path::is_file()
//! [`predicate::path::is_symlink`]: prelude::predicate::path::is_symlink()
//! [`predicate::path::missing`]: prelude::predicate::path::missing()
//! [`predicate::str::contains(...).count`]: str::ContainsPredicate::count()
//! [`predicate::str::contains`]: prelude::predicate::str::contains()
//! [`predicate::str::diff`]: prelude::predicate::str::diff()
//! [`predicate::str::ends_with`]: prelude::predicate::str::ends_with()
//! [`predicate::str::is_empty`]: prelude::predicate::str::is_empty()
//! [`predicate::str::is_match(...).count`]: str::RegexPredicate::count()
//! [`predicate::str::is_match`]: prelude::predicate::str::is_match()
//! [`predicate::str::starts_with`]: prelude::predicate::str::starts_with()
//! [`str_pred = predicate::path::eq_file(...).utf8`]: path::BinaryFilePredicate::utf8()
//! [`str_pred.normalize`]: prelude::PredicateStrExt::normalize()
//! [`str_pred.trim`]: prelude::PredicateStrExt::trim()

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

pub mod prelude;

pub use predicates_core::*;
mod boxed;
pub use crate::boxed::*;

// core predicates
pub mod constant;
pub mod function;
pub mod iter;
pub mod name;
pub mod ord;

// combinators
pub mod boolean;

// specialized primitive `Predicate` types
pub mod float;
pub mod path;
pub mod str;

mod color;
use color::Palette;
mod utils;
