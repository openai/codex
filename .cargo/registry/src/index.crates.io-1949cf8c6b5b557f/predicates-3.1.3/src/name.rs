// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Name predicate expressions.

use std::fmt;
use std::marker::PhantomData;

use crate::reflection;
use crate::Predicate;

/// Augment an existing predicate with a name.
///
/// This is created by the `PredicateNameExt::name` function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamePredicate<M, Item>
where
    M: Predicate<Item>,
    Item: ?Sized,
{
    inner: M,
    name: &'static str,
    _phantom: PhantomData<Item>,
}

unsafe impl<M, Item> Send for NamePredicate<M, Item>
where
    M: Predicate<Item> + Send,
    Item: ?Sized,
{
}

unsafe impl<M, Item> Sync for NamePredicate<M, Item>
where
    M: Predicate<Item> + Sync,
    Item: ?Sized,
{
}

impl<M, Item> Predicate<Item> for NamePredicate<M, Item>
where
    M: Predicate<Item>,
    Item: ?Sized,
{
    fn eval(&self, item: &Item) -> bool {
        self.inner.eval(item)
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &Item) -> Option<reflection::Case<'a>> {
        self.inner
            .find_case(expected, variable)
            .map(|child_case| reflection::Case::new(Some(self), expected).add_child(child_case))
    }
}

impl<M, Item> reflection::PredicateReflection for NamePredicate<M, Item>
where
    M: Predicate<Item>,
    Item: ?Sized,
{
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = reflection::Child<'a>> + 'a> {
        let params = vec![reflection::Child::new(self.name, &self.inner)];
        Box::new(params.into_iter())
    }
}

impl<M, Item> fmt::Display for NamePredicate<M, Item>
where
    M: Predicate<Item>,
    Item: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::new(f.alternate());
        write!(f, "{}", palette.description(self.name))
    }
}

/// `Predicate` extension that adds naming predicate expressions.
pub trait PredicateNameExt<Item: ?Sized>
where
    Self: Predicate<Item>,
{
    /// Name a predicate expression.
    ///
    /// # Examples
    ///
    /// ```
    /// use predicates::prelude::*;
    ///
    /// let predicate_fn = predicate::str::is_empty().not().name("non-empty");
    /// println!("{}", predicate_fn);
    /// ```
    fn name(self, name: &'static str) -> NamePredicate<Self, Item>
    where
        Self: Sized,
    {
        NamePredicate {
            inner: self,
            name,
            _phantom: PhantomData,
        }
    }
}

impl<P, Item> PredicateNameExt<Item> for P
where
    P: Predicate<Item>,
    Item: ?Sized,
{
}
