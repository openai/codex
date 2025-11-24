// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::reflection;

/// Trait for generically evaluating a type against a dynamically created
/// predicate function.
///
/// The exact meaning of `eval` depends on the situation, but will usually
/// mean that the evaluated item is in some sort of pre-defined set.  This is
/// different from `Ord` and `Eq` in that an `item` will almost never be the
/// same type as the implementing `Predicate` type.
pub trait Predicate<Item: ?Sized>: reflection::PredicateReflection {
    /// Execute this `Predicate` against `variable`, returning the resulting
    /// boolean.
    fn eval(&self, variable: &Item) -> bool;

    /// Find a case that proves this predicate as `expected` when run against `variable`.
    fn find_case<'a>(&'a self, expected: bool, variable: &Item) -> Option<reflection::Case<'a>> {
        let actual = self.eval(variable);
        if expected == actual {
            Some(reflection::Case::new(None, actual))
        } else {
            None
        }
    }
}
