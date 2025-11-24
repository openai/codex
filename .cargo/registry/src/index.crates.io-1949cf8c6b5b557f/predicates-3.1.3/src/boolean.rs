// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/license/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Definition of boolean logic combinators over `Predicate`s.

use std::fmt;
use std::marker::PhantomData;

use crate::reflection;
use crate::Predicate;

/// Predicate that combines two `Predicate`s, returning the AND of the results.
///
/// This is created by the `Predicate::and` function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AndPredicate<M1, M2, Item>
where
    M1: Predicate<Item>,
    M2: Predicate<Item>,
    Item: ?Sized,
{
    a: M1,
    b: M2,
    _phantom: PhantomData<Item>,
}

unsafe impl<M1, M2, Item> Send for AndPredicate<M1, M2, Item>
where
    M1: Predicate<Item> + Send,
    M2: Predicate<Item> + Send,
    Item: ?Sized,
{
}

unsafe impl<M1, M2, Item> Sync for AndPredicate<M1, M2, Item>
where
    M1: Predicate<Item> + Sync,
    M2: Predicate<Item> + Sync,
    Item: ?Sized,
{
}

impl<M1, M2, Item> AndPredicate<M1, M2, Item>
where
    M1: Predicate<Item>,
    M2: Predicate<Item>,
    Item: ?Sized,
{
    /// Create a new `AndPredicate` over predicates `a` and `b`.
    pub fn new(a: M1, b: M2) -> AndPredicate<M1, M2, Item> {
        AndPredicate {
            a,
            b,
            _phantom: PhantomData,
        }
    }
}

impl<M1, M2, Item> Predicate<Item> for AndPredicate<M1, M2, Item>
where
    M1: Predicate<Item>,
    M2: Predicate<Item>,
    Item: ?Sized,
{
    fn eval(&self, item: &Item) -> bool {
        self.a.eval(item) && self.b.eval(item)
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &Item) -> Option<reflection::Case<'a>> {
        let child_a = self.a.find_case(expected, variable);
        match (expected, child_a) {
            (true, Some(child_a)) => self.b.find_case(expected, variable).map(|child_b| {
                reflection::Case::new(Some(self), expected)
                    .add_child(child_a)
                    .add_child(child_b)
            }),
            (true, None) => None,
            (false, Some(child_a)) => {
                Some(reflection::Case::new(Some(self), expected).add_child(child_a))
            }
            (false, None) => self
                .b
                .find_case(expected, variable)
                .map(|child_b| reflection::Case::new(Some(self), expected).add_child(child_b)),
        }
    }
}

impl<M1, M2, Item> reflection::PredicateReflection for AndPredicate<M1, M2, Item>
where
    M1: Predicate<Item>,
    M2: Predicate<Item>,
    Item: ?Sized,
{
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = reflection::Child<'a>> + 'a> {
        let params = vec![
            reflection::Child::new("left", &self.a),
            reflection::Child::new("right", &self.b),
        ];
        Box::new(params.into_iter())
    }
}

impl<M1, M2, Item> fmt::Display for AndPredicate<M1, M2, Item>
where
    M1: Predicate<Item>,
    M2: Predicate<Item>,
    Item: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({} && {})", self.a, self.b)
    }
}

#[cfg(test)]
mod test_and {
    use crate::prelude::*;

    #[test]
    fn find_case_true() {
        assert!(predicate::always()
            .and(predicate::always())
            .find_case(true, &5)
            .is_some());
    }

    #[test]
    fn find_case_true_left_fail() {
        assert!(predicate::never()
            .and(predicate::always())
            .find_case(true, &5)
            .is_none());
    }

    #[test]
    fn find_case_true_right_fail() {
        assert!(predicate::always()
            .and(predicate::never())
            .find_case(true, &5)
            .is_none());
    }

    #[test]
    fn find_case_true_fails() {
        assert!(predicate::never()
            .and(predicate::never())
            .find_case(true, &5)
            .is_none());
    }

    #[test]
    fn find_case_false() {
        assert!(predicate::never()
            .and(predicate::never())
            .find_case(false, &5)
            .is_some());
    }

    #[test]
    fn find_case_false_fails() {
        assert!(predicate::always()
            .and(predicate::always())
            .find_case(false, &5)
            .is_none());
    }

    #[test]
    fn find_case_false_left_fail() {
        assert!(predicate::never()
            .and(predicate::always())
            .find_case(false, &5)
            .is_some());
    }

    #[test]
    fn find_case_false_right_fail() {
        assert!(predicate::always()
            .and(predicate::never())
            .find_case(false, &5)
            .is_some());
    }
}

/// Predicate that combines two `Predicate`s, returning the OR of the results.
///
/// This is created by the `Predicate::or` function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrPredicate<M1, M2, Item>
where
    M1: Predicate<Item>,
    M2: Predicate<Item>,
    Item: ?Sized,
{
    a: M1,
    b: M2,
    _phantom: PhantomData<Item>,
}

unsafe impl<M1, M2, Item> Send for OrPredicate<M1, M2, Item>
where
    M1: Predicate<Item> + Send,
    M2: Predicate<Item> + Send,
    Item: ?Sized,
{
}

unsafe impl<M1, M2, Item> Sync for OrPredicate<M1, M2, Item>
where
    M1: Predicate<Item> + Sync,
    M2: Predicate<Item> + Sync,
    Item: ?Sized,
{
}

impl<M1, M2, Item> OrPredicate<M1, M2, Item>
where
    M1: Predicate<Item>,
    M2: Predicate<Item>,
    Item: ?Sized,
{
    /// Create a new `OrPredicate` over predicates `a` and `b`.
    pub fn new(a: M1, b: M2) -> OrPredicate<M1, M2, Item> {
        OrPredicate {
            a,
            b,
            _phantom: PhantomData,
        }
    }
}

impl<M1, M2, Item> Predicate<Item> for OrPredicate<M1, M2, Item>
where
    M1: Predicate<Item>,
    M2: Predicate<Item>,
    Item: ?Sized,
{
    fn eval(&self, item: &Item) -> bool {
        self.a.eval(item) || self.b.eval(item)
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &Item) -> Option<reflection::Case<'a>> {
        let child_a = self.a.find_case(expected, variable);
        match (expected, child_a) {
            (true, Some(child_a)) => {
                Some(reflection::Case::new(Some(self), expected).add_child(child_a))
            }
            (true, None) => self
                .b
                .find_case(expected, variable)
                .map(|child_b| reflection::Case::new(Some(self), expected).add_child(child_b)),
            (false, Some(child_a)) => self.b.find_case(expected, variable).map(|child_b| {
                reflection::Case::new(Some(self), expected)
                    .add_child(child_a)
                    .add_child(child_b)
            }),
            (false, None) => None,
        }
    }
}

impl<M1, M2, Item> reflection::PredicateReflection for OrPredicate<M1, M2, Item>
where
    M1: Predicate<Item>,
    M2: Predicate<Item>,
    Item: ?Sized,
{
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = reflection::Child<'a>> + 'a> {
        let params = vec![
            reflection::Child::new("left", &self.a),
            reflection::Child::new("right", &self.b),
        ];
        Box::new(params.into_iter())
    }
}

impl<M1, M2, Item> fmt::Display for OrPredicate<M1, M2, Item>
where
    M1: Predicate<Item>,
    M2: Predicate<Item>,
    Item: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({} || {})", self.a, self.b)
    }
}

#[cfg(test)]
mod test_or {
    use crate::prelude::*;

    #[test]
    fn find_case_true() {
        assert!(predicate::always()
            .or(predicate::always())
            .find_case(true, &5)
            .is_some());
    }

    #[test]
    fn find_case_true_left_fail() {
        assert!(predicate::never()
            .or(predicate::always())
            .find_case(true, &5)
            .is_some());
    }

    #[test]
    fn find_case_true_right_fail() {
        assert!(predicate::always()
            .or(predicate::never())
            .find_case(true, &5)
            .is_some());
    }

    #[test]
    fn find_case_true_fails() {
        assert!(predicate::never()
            .or(predicate::never())
            .find_case(true, &5)
            .is_none());
    }

    #[test]
    fn find_case_false() {
        assert!(predicate::never()
            .or(predicate::never())
            .find_case(false, &5)
            .is_some());
    }

    #[test]
    fn find_case_false_fails() {
        assert!(predicate::always()
            .or(predicate::always())
            .find_case(false, &5)
            .is_none());
    }

    #[test]
    fn find_case_false_left_fail() {
        assert!(predicate::never()
            .or(predicate::always())
            .find_case(false, &5)
            .is_none());
    }

    #[test]
    fn find_case_false_right_fail() {
        assert!(predicate::always()
            .or(predicate::never())
            .find_case(false, &5)
            .is_none());
    }
}

/// Predicate that returns a `Predicate` taking the logical NOT of the result.
///
/// This is created by the `Predicate::not` function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotPredicate<M, Item>
where
    M: Predicate<Item>,
    Item: ?Sized,
{
    inner: M,
    _phantom: PhantomData<Item>,
}

unsafe impl<M, Item> Send for NotPredicate<M, Item>
where
    M: Predicate<Item> + Send,
    Item: ?Sized,
{
}

unsafe impl<M, Item> Sync for NotPredicate<M, Item>
where
    M: Predicate<Item> + Sync,
    Item: ?Sized,
{
}

impl<M, Item> NotPredicate<M, Item>
where
    M: Predicate<Item>,
    Item: ?Sized,
{
    /// Create a new `NotPredicate` over predicate `inner`.
    pub fn new(inner: M) -> NotPredicate<M, Item> {
        NotPredicate {
            inner,
            _phantom: PhantomData,
        }
    }
}

impl<M, Item> Predicate<Item> for NotPredicate<M, Item>
where
    M: Predicate<Item>,
    Item: ?Sized,
{
    fn eval(&self, item: &Item) -> bool {
        !self.inner.eval(item)
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &Item) -> Option<reflection::Case<'a>> {
        self.inner
            .find_case(!expected, variable)
            .map(|child| reflection::Case::new(Some(self), expected).add_child(child))
    }
}

impl<M, Item> reflection::PredicateReflection for NotPredicate<M, Item>
where
    M: Predicate<Item>,
    Item: ?Sized,
{
    fn children<'a>(&'a self) -> Box<dyn Iterator<Item = reflection::Child<'a>> + 'a> {
        let params = vec![reflection::Child::new("predicate", &self.inner)];
        Box::new(params.into_iter())
    }
}

impl<M, Item> fmt::Display for NotPredicate<M, Item>
where
    M: Predicate<Item>,
    Item: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(! {})", self.inner)
    }
}

/// `Predicate` extension that adds boolean logic.
pub trait PredicateBooleanExt<Item: ?Sized>
where
    Self: Predicate<Item>,
{
    /// Compute the logical AND of two `Predicate` results, returning the result.
    ///
    /// # Examples
    ///
    /// ```
    /// use predicates::prelude::*;
    ///
    /// let predicate_fn1 = predicate::always().and(predicate::always());
    /// let predicate_fn2 = predicate::always().and(predicate::never());
    /// assert_eq!(true, predicate_fn1.eval(&4));
    /// assert_eq!(false, predicate_fn2.eval(&4));
    fn and<B>(self, other: B) -> AndPredicate<Self, B, Item>
    where
        B: Predicate<Item>,
        Self: Sized,
    {
        AndPredicate::new(self, other)
    }

    /// Compute the logical OR of two `Predicate` results, returning the result.
    ///
    /// # Examples
    ///
    /// ```
    /// use predicates::prelude::*;
    ///
    /// let predicate_fn1 = predicate::always().or(predicate::always());
    /// let predicate_fn2 = predicate::always().or(predicate::never());
    /// let predicate_fn3 = predicate::never().or(predicate::never());
    /// assert_eq!(true, predicate_fn1.eval(&4));
    /// assert_eq!(true, predicate_fn2.eval(&4));
    /// assert_eq!(false, predicate_fn3.eval(&4));
    fn or<B>(self, other: B) -> OrPredicate<Self, B, Item>
    where
        B: Predicate<Item>,
        Self: Sized,
    {
        OrPredicate::new(self, other)
    }

    /// Compute the logical NOT of a `Predicate`, returning the result.
    ///
    /// # Examples
    ///
    /// ```
    /// use predicates::prelude::*;
    ///
    /// let predicate_fn1 = predicate::always().not();
    /// let predicate_fn2 = predicate::never().not();
    /// assert_eq!(false, predicate_fn1.eval(&4));
    /// assert_eq!(true, predicate_fn2.eval(&4));
    fn not(self) -> NotPredicate<Self, Item>
    where
        Self: Sized,
    {
        NotPredicate::new(self)
    }
}

impl<P, Item> PredicateBooleanExt<Item> for P
where
    P: Predicate<Item>,
    Item: ?Sized,
{
}
