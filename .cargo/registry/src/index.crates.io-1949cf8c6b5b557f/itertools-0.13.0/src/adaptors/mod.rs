//! Licensed under the Apache License, Version 2.0
//! <https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
//! <https://opensource.org/licenses/MIT>, at your
//! option. This file may not be copied, modified, or distributed
//! except according to those terms.

mod coalesce;
pub(crate) mod map;
mod multi_product;
pub use self::coalesce::*;
pub use self::map::{map_into, map_ok, MapInto, MapOk};
#[cfg(feature = "use_alloc")]
pub use self::multi_product::*;

use crate::size_hint::{self, SizeHint};
use std::fmt;
use std::iter::{Enumerate, FromIterator, Fuse, FusedIterator};
use std::marker::PhantomData;

/// An iterator adaptor that alternates elements from two iterators until both
/// run out.
///
/// This iterator is *fused*.
///
/// See [`.interleave()`](crate::Itertools::interleave) for more information.
#[derive(Clone, Debug)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Interleave<I, J> {
    i: Fuse<I>,
    j: Fuse<J>,
    next_coming_from_j: bool,
}

/// Create an iterator that interleaves elements in `i` and `j`.
///
/// [`IntoIterator`] enabled version of [`Itertools::interleave`](crate::Itertools::interleave).
pub fn interleave<I, J>(
    i: I,
    j: J,
) -> Interleave<<I as IntoIterator>::IntoIter, <J as IntoIterator>::IntoIter>
where
    I: IntoIterator,
    J: IntoIterator<Item = I::Item>,
{
    Interleave {
        i: i.into_iter().fuse(),
        j: j.into_iter().fuse(),
        next_coming_from_j: false,
    }
}

impl<I, J> Iterator for Interleave<I, J>
where
    I: Iterator,
    J: Iterator<Item = I::Item>,
{
    type Item = I::Item;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.next_coming_from_j = !self.next_coming_from_j;
        if self.next_coming_from_j {
            match self.i.next() {
                None => self.j.next(),
                r => r,
            }
        } else {
            match self.j.next() {
                None => self.i.next(),
                r => r,
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        size_hint::add(self.i.size_hint(), self.j.size_hint())
    }

    fn fold<B, F>(self, mut init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        let Self {
            mut i,
            mut j,
            next_coming_from_j,
        } = self;
        if next_coming_from_j {
            match j.next() {
                Some(y) => init = f(init, y),
                None => return i.fold(init, f),
            }
        }
        let res = i.try_fold(init, |mut acc, x| {
            acc = f(acc, x);
            match j.next() {
                Some(y) => Ok(f(acc, y)),
                None => Err(acc),
            }
        });
        match res {
            Ok(acc) => j.fold(acc, f),
            Err(acc) => i.fold(acc, f),
        }
    }
}

impl<I, J> FusedIterator for Interleave<I, J>
where
    I: Iterator,
    J: Iterator<Item = I::Item>,
{
}

/// An iterator adaptor that alternates elements from the two iterators until
/// one of them runs out.
///
/// This iterator is *fused*.
///
/// See [`.interleave_shortest()`](crate::Itertools::interleave_shortest)
/// for more information.
#[derive(Clone, Debug)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct InterleaveShortest<I, J>
where
    I: Iterator,
    J: Iterator<Item = I::Item>,
{
    i: I,
    j: J,
    next_coming_from_j: bool,
}

/// Create a new `InterleaveShortest` iterator.
pub fn interleave_shortest<I, J>(i: I, j: J) -> InterleaveShortest<I, J>
where
    I: Iterator,
    J: Iterator<Item = I::Item>,
{
    InterleaveShortest {
        i,
        j,
        next_coming_from_j: false,
    }
}

impl<I, J> Iterator for InterleaveShortest<I, J>
where
    I: Iterator,
    J: Iterator<Item = I::Item>,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let e = if self.next_coming_from_j {
            self.j.next()
        } else {
            self.i.next()
        };
        if e.is_some() {
            self.next_coming_from_j = !self.next_coming_from_j;
        }
        e
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (curr_hint, next_hint) = {
            let i_hint = self.i.size_hint();
            let j_hint = self.j.size_hint();
            if self.next_coming_from_j {
                (j_hint, i_hint)
            } else {
                (i_hint, j_hint)
            }
        };
        let (curr_lower, curr_upper) = curr_hint;
        let (next_lower, next_upper) = next_hint;
        let (combined_lower, combined_upper) =
            size_hint::mul_scalar(size_hint::min(curr_hint, next_hint), 2);
        let lower = if curr_lower > next_lower {
            combined_lower + 1
        } else {
            combined_lower
        };
        let upper = {
            let extra_elem = match (curr_upper, next_upper) {
                (_, None) => false,
                (None, Some(_)) => true,
                (Some(curr_max), Some(next_max)) => curr_max > next_max,
            };
            if extra_elem {
                combined_upper.and_then(|x| x.checked_add(1))
            } else {
                combined_upper
            }
        };
        (lower, upper)
    }

    fn fold<B, F>(self, mut init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        let Self {
            mut i,
            mut j,
            next_coming_from_j,
        } = self;
        if next_coming_from_j {
            match j.next() {
                Some(y) => init = f(init, y),
                None => return init,
            }
        }
        let res = i.try_fold(init, |mut acc, x| {
            acc = f(acc, x);
            match j.next() {
                Some(y) => Ok(f(acc, y)),
                None => Err(acc),
            }
        });
        match res {
            Ok(val) => val,
            Err(val) => val,
        }
    }
}

impl<I, J> FusedIterator for InterleaveShortest<I, J>
where
    I: FusedIterator,
    J: FusedIterator<Item = I::Item>,
{
}

#[derive(Clone, Debug)]
/// An iterator adaptor that allows putting back a single
/// item to the front of the iterator.
///
/// Iterator element type is `I::Item`.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct PutBack<I>
where
    I: Iterator,
{
    top: Option<I::Item>,
    iter: I,
}

/// Create an iterator where you can put back a single item
pub fn put_back<I>(iterable: I) -> PutBack<I::IntoIter>
where
    I: IntoIterator,
{
    PutBack {
        top: None,
        iter: iterable.into_iter(),
    }
}

impl<I> PutBack<I>
where
    I: Iterator,
{
    /// put back value `value` (builder method)
    pub fn with_value(mut self, value: I::Item) -> Self {
        self.put_back(value);
        self
    }

    /// Split the `PutBack` into its parts.
    #[inline]
    pub fn into_parts(self) -> (Option<I::Item>, I) {
        let Self { top, iter } = self;
        (top, iter)
    }

    /// Put back a single value to the front of the iterator.
    ///
    /// If a value is already in the put back slot, it is returned.
    #[inline]
    pub fn put_back(&mut self, x: I::Item) -> Option<I::Item> {
        self.top.replace(x)
    }
}

impl<I> Iterator for PutBack<I>
where
    I: Iterator,
{
    type Item = I::Item;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.top {
            None => self.iter.next(),
            ref mut some => some.take(),
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // Not ExactSizeIterator because size may be larger than usize
        size_hint::add_scalar(self.iter.size_hint(), self.top.is_some() as usize)
    }

    fn count(self) -> usize {
        self.iter.count() + (self.top.is_some() as usize)
    }

    fn last(self) -> Option<Self::Item> {
        self.iter.last().or(self.top)
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match self.top {
            None => self.iter.nth(n),
            ref mut some => {
                if n == 0 {
                    some.take()
                } else {
                    *some = None;
                    self.iter.nth(n - 1)
                }
            }
        }
    }

    fn all<G>(&mut self, mut f: G) -> bool
    where
        G: FnMut(Self::Item) -> bool,
    {
        if let Some(elt) = self.top.take() {
            if !f(elt) {
                return false;
            }
        }
        self.iter.all(f)
    }

    fn fold<Acc, G>(mut self, init: Acc, mut f: G) -> Acc
    where
        G: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut accum = init;
        if let Some(elt) = self.top.take() {
            accum = f(accum, elt);
        }
        self.iter.fold(accum, f)
    }
}

#[derive(Debug, Clone)]
/// An iterator adaptor that iterates over the cartesian product of
/// the element sets of two iterators `I` and `J`.
///
/// Iterator element type is `(I::Item, J::Item)`.
///
/// See [`.cartesian_product()`](crate::Itertools::cartesian_product) for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Product<I, J>
where
    I: Iterator,
{
    a: I,
    /// `a_cur` is `None` while no item have been taken out of `a` (at definition).
    /// Then `a_cur` will be `Some(Some(item))` until `a` is exhausted,
    /// in which case `a_cur` will be `Some(None)`.
    a_cur: Option<Option<I::Item>>,
    b: J,
    b_orig: J,
}

/// Create a new cartesian product iterator
///
/// Iterator element type is `(I::Item, J::Item)`.
pub fn cartesian_product<I, J>(i: I, j: J) -> Product<I, J>
where
    I: Iterator,
    J: Clone + Iterator,
    I::Item: Clone,
{
    Product {
        a_cur: None,
        a: i,
        b: j.clone(),
        b_orig: j,
    }
}

impl<I, J> Iterator for Product<I, J>
where
    I: Iterator,
    J: Clone + Iterator,
    I::Item: Clone,
{
    type Item = (I::Item, J::Item);

    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            a,
            a_cur,
            b,
            b_orig,
        } = self;
        let elt_b = match b.next() {
            None => {
                *b = b_orig.clone();
                match b.next() {
                    None => return None,
                    Some(x) => {
                        *a_cur = Some(a.next());
                        x
                    }
                }
            }
            Some(x) => x,
        };
        a_cur
            .get_or_insert_with(|| a.next())
            .as_ref()
            .map(|a| (a.clone(), elt_b))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Not ExactSizeIterator because size may be larger than usize
        // Compute a * b_orig + b for both lower and upper bound
        let mut sh = size_hint::mul(self.a.size_hint(), self.b_orig.size_hint());
        if matches!(self.a_cur, Some(Some(_))) {
            sh = size_hint::add(sh, self.b.size_hint());
        }
        sh
    }

    fn fold<Acc, G>(self, mut accum: Acc, mut f: G) -> Acc
    where
        G: FnMut(Acc, Self::Item) -> Acc,
    {
        // use a split loop to handle the loose a_cur as well as avoiding to
        // clone b_orig at the end.
        let Self {
            mut a,
            a_cur,
            mut b,
            b_orig,
        } = self;
        if let Some(mut elt_a) = a_cur.unwrap_or_else(|| a.next()) {
            loop {
                accum = b.fold(accum, |acc, elt| f(acc, (elt_a.clone(), elt)));

                // we can only continue iterating a if we had a first element;
                if let Some(next_elt_a) = a.next() {
                    b = b_orig.clone();
                    elt_a = next_elt_a;
                } else {
                    break;
                }
            }
        }
        accum
    }
}

impl<I, J> FusedIterator for Product<I, J>
where
    I: FusedIterator,
    J: Clone + FusedIterator,
    I::Item: Clone,
{
}

/// A “meta iterator adaptor”. Its closure receives a reference to the iterator
/// and may pick off as many elements as it likes, to produce the next iterator element.
///
/// Iterator element type is `X` if the return type of `F` is `Option<X>`.
///
/// See [`.batching()`](crate::Itertools::batching) for more information.
#[derive(Clone)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Batching<I, F> {
    f: F,
    iter: I,
}

impl<I, F> fmt::Debug for Batching<I, F>
where
    I: fmt::Debug,
{
    debug_fmt_fields!(Batching, iter);
}

/// Create a new Batching iterator.
pub fn batching<I, F>(iter: I, f: F) -> Batching<I, F> {
    Batching { f, iter }
}

impl<B, F, I> Iterator for Batching<I, F>
where
    I: Iterator,
    F: FnMut(&mut I) -> Option<B>,
{
    type Item = B;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        (self.f)(&mut self.iter)
    }
}

/// An iterator adaptor that borrows from a `Clone`-able iterator
/// to only pick off elements while the predicate returns `true`.
///
/// See [`.take_while_ref()`](crate::Itertools::take_while_ref) for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct TakeWhileRef<'a, I: 'a, F> {
    iter: &'a mut I,
    f: F,
}

impl<'a, I, F> fmt::Debug for TakeWhileRef<'a, I, F>
where
    I: Iterator + fmt::Debug,
{
    debug_fmt_fields!(TakeWhileRef, iter);
}

/// Create a new `TakeWhileRef` from a reference to clonable iterator.
pub fn take_while_ref<I, F>(iter: &mut I, f: F) -> TakeWhileRef<I, F>
where
    I: Iterator + Clone,
{
    TakeWhileRef { iter, f }
}

impl<'a, I, F> Iterator for TakeWhileRef<'a, I, F>
where
    I: Iterator + Clone,
    F: FnMut(&I::Item) -> bool,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let old = self.iter.clone();
        match self.iter.next() {
            None => None,
            Some(elt) => {
                if (self.f)(&elt) {
                    Some(elt)
                } else {
                    *self.iter = old;
                    None
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }
}

/// An iterator adaptor that filters `Option<A>` iterator elements
/// and produces `A`. Stops on the first `None` encountered.
///
/// See [`.while_some()`](crate::Itertools::while_some) for more information.
#[derive(Clone, Debug)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct WhileSome<I> {
    iter: I,
}

/// Create a new `WhileSome<I>`.
pub fn while_some<I>(iter: I) -> WhileSome<I> {
    WhileSome { iter }
}

impl<I, A> Iterator for WhileSome<I>
where
    I: Iterator<Item = Option<A>>,
{
    type Item = A;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            None | Some(None) => None,
            Some(elt) => elt,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }

    fn fold<B, F>(mut self, acc: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        let res = self.iter.try_fold(acc, |acc, item| match item {
            Some(item) => Ok(f(acc, item)),
            None => Err(acc),
        });

        match res {
            Ok(val) => val,
            Err(val) => val,
        }
    }
}

/// An iterator to iterate through all combinations in a `Clone`-able iterator that produces tuples
/// of a specific size.
///
/// See [`.tuple_combinations()`](crate::Itertools::tuple_combinations) for more
/// information.
#[derive(Clone, Debug)]
#[must_use = "this iterator adaptor is not lazy but does nearly nothing unless consumed"]
pub struct TupleCombinations<I, T>
where
    I: Iterator,
    T: HasCombination<I>,
{
    iter: T::Combination,
    _mi: PhantomData<I>,
}

pub trait HasCombination<I>: Sized {
    type Combination: From<I> + Iterator<Item = Self>;
}

/// Create a new `TupleCombinations` from a clonable iterator.
pub fn tuple_combinations<T, I>(iter: I) -> TupleCombinations<I, T>
where
    I: Iterator + Clone,
    I::Item: Clone,
    T: HasCombination<I>,
{
    TupleCombinations {
        iter: T::Combination::from(iter),
        _mi: PhantomData,
    }
}

impl<I, T> Iterator for TupleCombinations<I, T>
where
    I: Iterator,
    T: HasCombination<I>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    fn size_hint(&self) -> SizeHint {
        self.iter.size_hint()
    }

    fn count(self) -> usize {
        self.iter.count()
    }

    fn fold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, f)
    }
}

impl<I, T> FusedIterator for TupleCombinations<I, T>
where
    I: FusedIterator,
    T: HasCombination<I>,
{
}

#[derive(Clone, Debug)]
pub struct Tuple1Combination<I> {
    iter: I,
}

impl<I> From<I> for Tuple1Combination<I> {
    fn from(iter: I) -> Self {
        Self { iter }
    }
}

impl<I: Iterator> Iterator for Tuple1Combination<I> {
    type Item = (I::Item,);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|x| (x,))
    }

    fn size_hint(&self) -> SizeHint {
        self.iter.size_hint()
    }

    fn count(self) -> usize {
        self.iter.count()
    }

    fn fold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.map(|x| (x,)).fold(init, f)
    }
}

impl<I: Iterator> HasCombination<I> for (I::Item,) {
    type Combination = Tuple1Combination<I>;
}

macro_rules! impl_tuple_combination {
    ($C:ident $P:ident ; $($X:ident)*) => (
        #[derive(Clone, Debug)]
        pub struct $C<I: Iterator> {
            item: Option<I::Item>,
            iter: I,
            c: $P<I>,
        }

        impl<I: Iterator + Clone> From<I> for $C<I> {
            fn from(mut iter: I) -> Self {
                Self {
                    item: iter.next(),
                    iter: iter.clone(),
                    c: iter.into(),
                }
            }
        }

        impl<I: Iterator + Clone> From<I> for $C<Fuse<I>> {
            fn from(iter: I) -> Self {
                Self::from(iter.fuse())
            }
        }

        impl<I, A> Iterator for $C<I>
            where I: Iterator<Item = A> + Clone,
                  A: Clone,
        {
            type Item = (A, $(ignore_ident!($X, A)),*);

            fn next(&mut self) -> Option<Self::Item> {
                if let Some(($($X,)*)) = self.c.next() {
                    let z = self.item.clone().unwrap();
                    Some((z, $($X),*))
                } else {
                    self.item = self.iter.next();
                    self.item.clone().and_then(|z| {
                        self.c = self.iter.clone().into();
                        self.c.next().map(|($($X,)*)| (z, $($X),*))
                    })
                }
            }

            fn size_hint(&self) -> SizeHint {
                const K: usize = 1 + count_ident!($($X)*);
                let (mut n_min, mut n_max) = self.iter.size_hint();
                n_min = checked_binomial(n_min, K).unwrap_or(usize::MAX);
                n_max = n_max.and_then(|n| checked_binomial(n, K));
                size_hint::add(self.c.size_hint(), (n_min, n_max))
            }

            fn count(self) -> usize {
                const K: usize = 1 + count_ident!($($X)*);
                let n = self.iter.count();
                checked_binomial(n, K).unwrap() + self.c.count()
            }

            fn fold<B, F>(self, mut init: B, mut f: F) -> B
            where
                F: FnMut(B, Self::Item) -> B,
            {
                let Self { c, item, mut iter } = self;
                if let Some(z) = item.as_ref() {
                    init = c
                        .map(|($($X,)*)| (z.clone(), $($X),*))
                        .fold(init, &mut f);
                }
                while let Some(z) = iter.next() {
                    let c: $P<I> = iter.clone().into();
                    init = c
                        .map(|($($X,)*)| (z.clone(), $($X),*))
                        .fold(init, &mut f);
                }
                init
            }
        }

        impl<I, A> HasCombination<I> for (A, $(ignore_ident!($X, A)),*)
            where I: Iterator<Item = A> + Clone,
                  I::Item: Clone
        {
            type Combination = $C<Fuse<I>>;
        }
    )
}

// This snippet generates the twelve `impl_tuple_combination!` invocations:
//    use core::iter;
//    use itertools::Itertools;
//
//    for i in 2..=12 {
//        println!("impl_tuple_combination!(Tuple{arity}Combination Tuple{prev}Combination; {idents});",
//            arity = i,
//            prev = i - 1,
//            idents = ('a'..'z').take(i - 1).join(" "),
//        );
//    }
// It could probably be replaced by a bit more macro cleverness.
impl_tuple_combination!(Tuple2Combination Tuple1Combination; a);
impl_tuple_combination!(Tuple3Combination Tuple2Combination; a b);
impl_tuple_combination!(Tuple4Combination Tuple3Combination; a b c);
impl_tuple_combination!(Tuple5Combination Tuple4Combination; a b c d);
impl_tuple_combination!(Tuple6Combination Tuple5Combination; a b c d e);
impl_tuple_combination!(Tuple7Combination Tuple6Combination; a b c d e f);
impl_tuple_combination!(Tuple8Combination Tuple7Combination; a b c d e f g);
impl_tuple_combination!(Tuple9Combination Tuple8Combination; a b c d e f g h);
impl_tuple_combination!(Tuple10Combination Tuple9Combination; a b c d e f g h i);
impl_tuple_combination!(Tuple11Combination Tuple10Combination; a b c d e f g h i j);
impl_tuple_combination!(Tuple12Combination Tuple11Combination; a b c d e f g h i j k);

// https://en.wikipedia.org/wiki/Binomial_coefficient#In_programming_languages
pub(crate) fn checked_binomial(mut n: usize, mut k: usize) -> Option<usize> {
    if n < k {
        return Some(0);
    }
    // `factorial(n) / factorial(n - k) / factorial(k)` but trying to avoid it overflows:
    k = (n - k).min(k); // symmetry
    let mut c = 1;
    for i in 1..=k {
        c = (c / i)
            .checked_mul(n)?
            .checked_add((c % i).checked_mul(n)? / i)?;
        n -= 1;
    }
    Some(c)
}

#[test]
fn test_checked_binomial() {
    // With the first row: [1, 0, 0, ...] and the first column full of 1s, we check
    // row by row the recurrence relation of binomials (which is an equivalent definition).
    // For n >= 1 and k >= 1 we have:
    //   binomial(n, k) == binomial(n - 1, k - 1) + binomial(n - 1, k)
    const LIMIT: usize = 500;
    let mut row = vec![Some(0); LIMIT + 1];
    row[0] = Some(1);
    for n in 0..=LIMIT {
        for k in 0..=LIMIT {
            assert_eq!(row[k], checked_binomial(n, k));
        }
        row = std::iter::once(Some(1))
            .chain((1..=LIMIT).map(|k| row[k - 1]?.checked_add(row[k]?)))
            .collect();
    }
}

/// An iterator adapter to filter values within a nested `Result::Ok`.
///
/// See [`.filter_ok()`](crate::Itertools::filter_ok) for more information.
#[derive(Clone)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct FilterOk<I, F> {
    iter: I,
    f: F,
}

impl<I, F> fmt::Debug for FilterOk<I, F>
where
    I: fmt::Debug,
{
    debug_fmt_fields!(FilterOk, iter);
}

/// Create a new `FilterOk` iterator.
pub fn filter_ok<I, F, T, E>(iter: I, f: F) -> FilterOk<I, F>
where
    I: Iterator<Item = Result<T, E>>,
    F: FnMut(&T) -> bool,
{
    FilterOk { iter, f }
}

impl<I, F, T, E> Iterator for FilterOk<I, F>
where
    I: Iterator<Item = Result<T, E>>,
    F: FnMut(&T) -> bool,
{
    type Item = Result<T, E>;

    fn next(&mut self) -> Option<Self::Item> {
        let f = &mut self.f;
        self.iter.find(|res| match res {
            Ok(t) => f(t),
            _ => true,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }

    fn fold<Acc, Fold>(self, init: Acc, fold_f: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.iter
            .filter(|v| v.as_ref().map(&mut f).unwrap_or(true))
            .fold(init, fold_f)
    }

    fn collect<C>(self) -> C
    where
        C: FromIterator<Self::Item>,
    {
        let mut f = self.f;
        self.iter
            .filter(|v| v.as_ref().map(&mut f).unwrap_or(true))
            .collect()
    }
}

impl<I, F, T, E> FusedIterator for FilterOk<I, F>
where
    I: FusedIterator<Item = Result<T, E>>,
    F: FnMut(&T) -> bool,
{
}

/// An iterator adapter to filter and apply a transformation on values within a nested `Result::Ok`.
///
/// See [`.filter_map_ok()`](crate::Itertools::filter_map_ok) for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Clone)]
pub struct FilterMapOk<I, F> {
    iter: I,
    f: F,
}

impl<I, F> fmt::Debug for FilterMapOk<I, F>
where
    I: fmt::Debug,
{
    debug_fmt_fields!(FilterMapOk, iter);
}

fn transpose_result<T, E>(result: Result<Option<T>, E>) -> Option<Result<T, E>> {
    match result {
        Ok(Some(v)) => Some(Ok(v)),
        Ok(None) => None,
        Err(e) => Some(Err(e)),
    }
}

/// Create a new `FilterOk` iterator.
pub fn filter_map_ok<I, F, T, U, E>(iter: I, f: F) -> FilterMapOk<I, F>
where
    I: Iterator<Item = Result<T, E>>,
    F: FnMut(T) -> Option<U>,
{
    FilterMapOk { iter, f }
}

impl<I, F, T, U, E> Iterator for FilterMapOk<I, F>
where
    I: Iterator<Item = Result<T, E>>,
    F: FnMut(T) -> Option<U>,
{
    type Item = Result<U, E>;

    fn next(&mut self) -> Option<Self::Item> {
        let f = &mut self.f;
        self.iter.find_map(|res| match res {
            Ok(t) => f(t).map(Ok),
            Err(e) => Some(Err(e)),
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }

    fn fold<Acc, Fold>(self, init: Acc, fold_f: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.iter
            .filter_map(|v| transpose_result(v.map(&mut f)))
            .fold(init, fold_f)
    }

    fn collect<C>(self) -> C
    where
        C: FromIterator<Self::Item>,
    {
        let mut f = self.f;
        self.iter
            .filter_map(|v| transpose_result(v.map(&mut f)))
            .collect()
    }
}

impl<I, F, T, U, E> FusedIterator for FilterMapOk<I, F>
where
    I: FusedIterator<Item = Result<T, E>>,
    F: FnMut(T) -> Option<U>,
{
}

/// An iterator adapter to get the positions of each element that matches a predicate.
///
/// See [`.positions()`](crate::Itertools::positions) for more information.
#[derive(Clone)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Positions<I, F> {
    iter: Enumerate<I>,
    f: F,
}

impl<I, F> fmt::Debug for Positions<I, F>
where
    I: fmt::Debug,
{
    debug_fmt_fields!(Positions, iter);
}

/// Create a new `Positions` iterator.
pub fn positions<I, F>(iter: I, f: F) -> Positions<I, F>
where
    I: Iterator,
    F: FnMut(I::Item) -> bool,
{
    let iter = iter.enumerate();
    Positions { iter, f }
}

impl<I, F> Iterator for Positions<I, F>
where
    I: Iterator,
    F: FnMut(I::Item) -> bool,
{
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let f = &mut self.f;
        // TODO: once MSRV >= 1.62, use `then_some`.
        self.iter
            .find_map(|(count, val)| if f(val) { Some(count) } else { None })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }

    fn fold<B, G>(self, init: B, mut func: G) -> B
    where
        G: FnMut(B, Self::Item) -> B,
    {
        let mut f = self.f;
        self.iter.fold(init, |mut acc, (count, val)| {
            if f(val) {
                acc = func(acc, count);
            }
            acc
        })
    }
}

impl<I, F> DoubleEndedIterator for Positions<I, F>
where
    I: DoubleEndedIterator + ExactSizeIterator,
    F: FnMut(I::Item) -> bool,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        let f = &mut self.f;
        // TODO: once MSRV >= 1.62, use `then_some`.
        self.iter
            .by_ref()
            .rev()
            .find_map(|(count, val)| if f(val) { Some(count) } else { None })
    }

    fn rfold<B, G>(self, init: B, mut func: G) -> B
    where
        G: FnMut(B, Self::Item) -> B,
    {
        let mut f = self.f;
        self.iter.rfold(init, |mut acc, (count, val)| {
            if f(val) {
                acc = func(acc, count);
            }
            acc
        })
    }
}

impl<I, F> FusedIterator for Positions<I, F>
where
    I: FusedIterator,
    F: FnMut(I::Item) -> bool,
{
}

/// An iterator adapter to apply a mutating function to each element before yielding it.
///
/// See [`.update()`](crate::Itertools::update) for more information.
#[derive(Clone)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Update<I, F> {
    iter: I,
    f: F,
}

impl<I, F> fmt::Debug for Update<I, F>
where
    I: fmt::Debug,
{
    debug_fmt_fields!(Update, iter);
}

/// Create a new `Update` iterator.
pub fn update<I, F>(iter: I, f: F) -> Update<I, F>
where
    I: Iterator,
    F: FnMut(&mut I::Item),
{
    Update { iter, f }
}

impl<I, F> Iterator for Update<I, F>
where
    I: Iterator,
    F: FnMut(&mut I::Item),
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(mut v) = self.iter.next() {
            (self.f)(&mut v);
            Some(v)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn fold<Acc, G>(self, init: Acc, mut g: G) -> Acc
    where
        G: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.iter.fold(init, move |acc, mut v| {
            f(&mut v);
            g(acc, v)
        })
    }

    // if possible, re-use inner iterator specializations in collect
    fn collect<C>(self) -> C
    where
        C: FromIterator<Self::Item>,
    {
        let mut f = self.f;
        self.iter
            .map(move |mut v| {
                f(&mut v);
                v
            })
            .collect()
    }
}

impl<I, F> ExactSizeIterator for Update<I, F>
where
    I: ExactSizeIterator,
    F: FnMut(&mut I::Item),
{
}

impl<I, F> DoubleEndedIterator for Update<I, F>
where
    I: DoubleEndedIterator,
    F: FnMut(&mut I::Item),
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(mut v) = self.iter.next_back() {
            (self.f)(&mut v);
            Some(v)
        } else {
            None
        }
    }
}

impl<I, F> FusedIterator for Update<I, F>
where
    I: FusedIterator,
    F: FnMut(&mut I::Item),
{
}
