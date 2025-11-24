// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! String Predicates
//!
//! This module contains predicates specific to string handling.

mod basics;
pub use self::basics::*;
mod adapters;
pub use self::adapters::*;

#[cfg(feature = "diff")]
mod difference;
#[cfg(feature = "diff")]
pub use self::difference::{diff, DifferencePredicate};
#[cfg(feature = "normalize-line-endings")]
mod normalize;
#[cfg(feature = "normalize-line-endings")]
pub use self::normalize::NormalizedPredicate;

#[cfg(feature = "regex")]
mod regex;
#[cfg(feature = "regex")]
pub use self::regex::{is_match, RegexError, RegexPredicate};
