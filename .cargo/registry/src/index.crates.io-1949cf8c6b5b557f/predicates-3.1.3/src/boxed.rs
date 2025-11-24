// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/license/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Predicate that can wrap other dynamically-called predicates in an
//! easy-to-manage type.

use std::fmt;

use crate::reflection;
use crate::utils;
use crate::Predicate;

/// `Predicate` that wraps another `Predicate` as a trait object, allowing
/// sized storage of predicate types.
pub struct BoxPredicate<Item: ?Sized>(Box<dyn Predicate<Item> + Send + Sync>);

impl<Item> BoxPredicate<Item>
where
    Item: ?Sized,
{
    /// Creates a new `BoxPredicate`, a wrapper around a dynamically-dispatched
    /// `Predicate` type with useful trait impls.
    pub fn new<P>(inner: P) -> BoxPredicate<Item>
    where
        P: Predicate<Item> + Send + Sync + 'static,
    {
        BoxPredicate(Box::new(inner))
    }
}

impl<Item> fmt::Debug for BoxPredicate<Item>
where
    Item: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoxPredicate").finish()
    }
}

impl<Item> reflection::PredicateReflection for BoxPredicate<Item>
where
    Item: ?Sized,
{
    fn parameters<'a>(&'a self) -> Box<dyn Iterator<Item = reflection::Parameter<'a>> + 'a> {
        self.0.parameters()
    }

    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = reflection::Child<'a>> + 'a> {
        self.0.children()
    }
}

impl<Item> fmt::Display for BoxPredicate<Item>
where
    Item: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<Item> Predicate<Item> for BoxPredicate<Item>
where
    Item: ?Sized,
{
    fn eval(&self, variable: &Item) -> bool {
        self.0.eval(variable)
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &Item) -> Option<reflection::Case<'a>> {
        utils::default_find_case(self, expected, variable)
    }
}

/// `Predicate` extension for boxing a `Predicate`.
pub trait PredicateBoxExt<Item: ?Sized>
where
    Self: Predicate<Item>,
{
    /// Returns a `BoxPredicate` wrapper around this `Predicate` type.
    ///
    /// Returns a `BoxPredicate` wrapper around this `Predicate` type. The
    /// `BoxPredicate` type has a number of useful properties:
    ///
    ///   - It stores the inner predicate as a trait object, so the type of
    ///     `BoxPredicate` will always be the same even if steps are added or
    ///     removed from the predicate.
    ///   - It is a common type, allowing it to be stored in vectors or other
    ///     collection types.
    ///   - It implements `Debug` and `Display`.
    ///
    /// # Examples
    ///
    /// ```
    /// use predicates::prelude::*;
    ///
    /// let predicates = vec![
    ///     predicate::always().boxed(),
    ///     predicate::never().boxed(),
    /// ];
    /// assert_eq!(true, predicates[0].eval(&4));
    /// assert_eq!(false, predicates[1].eval(&4));
    /// ```
    fn boxed(self) -> BoxPredicate<Item>
    where
        Self: Sized + Send + Sync + 'static,
    {
        BoxPredicate::new(self)
    }
}

impl<P, Item: ?Sized> PredicateBoxExt<Item> for P where P: Predicate<Item> {}

#[cfg(test)]
mod test {
    use crate::prelude::*;

    #[test]
    fn unsized_boxed() {
        let p = predicate::always().boxed();
        p.eval("4");
    }
}
