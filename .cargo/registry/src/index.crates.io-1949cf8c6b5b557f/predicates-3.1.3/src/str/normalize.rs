// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/license/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::reflection;
use crate::Predicate;
use std::fmt;

use normalize_line_endings::normalized;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Predicate adapter that normalizes the newlines contained in the variable being tested.
///
/// This is created by `pred.normalize()`.
pub struct NormalizedPredicate<P>
where
    P: Predicate<str>,
{
    pub(crate) p: P,
}

impl<P> reflection::PredicateReflection for NormalizedPredicate<P>
where
    P: Predicate<str>,
{
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = reflection::Child<'a>> + 'a> {
        let params = vec![reflection::Child::new("predicate", &self.p)];
        Box::new(params.into_iter())
    }
}

impl<P> Predicate<str> for NormalizedPredicate<P>
where
    P: Predicate<str>,
{
    fn eval(&self, variable: &str) -> bool {
        let variable = normalized(variable.chars()).collect::<String>();
        self.p.eval(&variable)
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &str) -> Option<reflection::Case<'a>> {
        let variable = normalized(variable.chars()).collect::<String>();
        self.p.find_case(expected, &variable)
    }
}

impl<P> fmt::Display for NormalizedPredicate<P>
where
    P: Predicate<str>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.p.fmt(f)
    }
}
