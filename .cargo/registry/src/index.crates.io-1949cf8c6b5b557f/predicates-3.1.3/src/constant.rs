// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/license/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Definition of a constant (always true or always false) `Predicate`.

use std::fmt;

use crate::reflection;
use crate::utils;
use crate::Predicate;

/// Predicate that always returns a constant (boolean) result.
///
/// This is created by the `predicate::always` and `predicate::never` functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BooleanPredicate {
    retval: bool,
}

impl<Item: ?Sized> Predicate<Item> for BooleanPredicate {
    fn eval(&self, _variable: &Item) -> bool {
        self.retval
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &Item) -> Option<reflection::Case<'a>> {
        utils::default_find_case(self, expected, variable)
    }
}

impl reflection::PredicateReflection for BooleanPredicate {
    fn parameters<'a>(&'a self) -> Box<dyn Iterator<Item = reflection::Parameter<'a>> + 'a> {
        let params = vec![reflection::Parameter::new("value", &self.retval)];
        Box::new(params.into_iter())
    }
}

impl fmt::Display for BooleanPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::new(f.alternate());
        write!(f, "{}", palette.expected(self.retval))
    }
}

/// Creates a new `Predicate` that always returns `true`.
///
/// # Examples
///
/// ```
/// use predicates::prelude::*;
///
/// let predicate_fn = predicate::always();
/// assert_eq!(true, predicate_fn.eval(&5));
/// assert_eq!(true, predicate_fn.eval(&10));
/// assert_eq!(true, predicate_fn.eval(&15));
/// // Won't work - Predicates can only operate on a single type
/// // assert_eq!(true, predicate_fn.eval("hello"))
/// ```
pub fn always() -> BooleanPredicate {
    BooleanPredicate { retval: true }
}

/// Creates a new `Predicate` that always returns `false`.
///
/// # Examples
///
/// ```
/// use predicates::prelude::*;
///
/// let predicate_fn = predicate::never();
/// assert_eq!(false, predicate_fn.eval(&5));
/// assert_eq!(false, predicate_fn.eval(&10));
/// assert_eq!(false, predicate_fn.eval(&15));
/// // Won't work - Predicates can only operate on a single type
/// // assert_eq!(false, predicate_fn.eval("hello"))
/// ```
pub fn never() -> BooleanPredicate {
    BooleanPredicate { retval: false }
}
