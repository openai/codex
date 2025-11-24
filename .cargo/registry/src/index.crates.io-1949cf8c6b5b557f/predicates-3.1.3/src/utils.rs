// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt;

use crate::reflection;
use crate::Predicate;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct DebugAdapter<T>
where
    T: fmt::Debug,
{
    pub(crate) debug: T,
}

impl<T> DebugAdapter<T>
where
    T: fmt::Debug,
{
    pub(crate) fn new(debug: T) -> Self {
        Self { debug }
    }
}

impl<T> fmt::Display for DebugAdapter<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#?}", self.debug)
    }
}

impl<T> fmt::Debug for DebugAdapter<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.debug.fmt(f)
    }
}

pub(crate) fn default_find_case<'a, P, Item>(
    pred: &'a P,
    expected: bool,
    variable: &Item,
) -> Option<reflection::Case<'a>>
where
    P: Predicate<Item>,
    Item: ?Sized,
{
    let actual = pred.eval(variable);
    if expected == actual {
        Some(reflection::Case::new(Some(pred), actual))
    } else {
        None
    }
}
