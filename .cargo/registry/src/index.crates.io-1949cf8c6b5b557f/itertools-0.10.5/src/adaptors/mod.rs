//! Licensed under the Apache License, Version 2.0
//! <https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
//! <https://opensource.org/licenses/MIT>, at your
//! option. This file may not be copied, modified, or distributed
//! except according to those terms.

mod coalesce;
mod map;
mod multi_product;
pub use self::coalesce::*;
pub use self::map::{map_into, map_ok, MapInto, MapOk};
#[allow(deprecated)]
pub use self::map::MapResults;
#[cfg(feature = "use_alloc")]
pub use self::multi_product::*;

use std::fmt;
use std::iter::{Fuse, Peekable, FromIterator, FusedIterator};
use std::marker::PhantomData;
use crate::size_hint;

/// An iterator adaptor that alternates elements from two iterators until both
/// run out.
///
/// This iterator is *fused*.
///
/// See [`.interleave()`](crate::Itertools::interleave) for more information.
#[derive(Clone, Debug)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Interleave<I, J> {
    a: Fuse<I>,
    b: Fuse<J>,
    flag: bool,
}

/// Create an iterator that interleaves elements in `i` and `j`.
///
/// [`IntoIterator`] enabled version of `[Itertools::interleave]`.
pub fn interleave<I, J>(i: I, j: J) -> Interleave<<I as IntoIterator>::IntoIter, <J as IntoIterator>::IntoIter>
    where I: IntoIterator,
          J: IntoIterator<Item = I::Item>
{
    Interleave {
        a: i.into_iter().fuse(),
        b: j.into_iter().fuse(),
        flag: false,
    }
}

impl<I, J> Iterator for Interleave<I, J>
    where I: Iterator,
          J: Iterator<Item = I::Item>
{
    type Item = I::Item;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.flag = !self.flag;
        if self.flag {
            match self.a.next() {
                None => self.b.next(),
                r => r,
            }
        } else {
            match self.b.next() {
                None => self.a.next(),
                r => r,
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        size_hint::add(self.a.size_hint(), self.b.size_hint())
    }
}

impl<I, J> FusedIterator for Interleave<I, J>
    where I: Iterator,
          J: Iterator<Item = I::Item>
{}

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
    where I: Iterator,
          J: Iterator<Item = I::Item>
{
    it0: I,
    it1: J,
    phase: bool, // false ==> it0, true ==> it1
}

/// Create a new `InterleaveShortest` iterator.
pub fn interleave_shortest<I, J>(a: I, b: J) -> InterleaveShortest<I, J>
    where I: Iterator,
          J: Iterator<Item = I::Item>
{
    InterleaveShortest {
        it0: a,
        it1: b,
        phase: false,
    }
}

impl<I, J> Iterator for InterleaveShortest<I, J>
    where I: Iterator,
          J: Iterator<Item = I::Item>
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let e = if self.phase { self.it1.next() } else { self.it0.next() };
        if e.is_some() {
            self.phase = !self.phase;
        }
        e
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (curr_hint, next_hint) = {
            let it0_hint = self.it0.size_hint();
            let it1_hint = self.it1.size_hint();
            if self.phase {
                (it1_hint, it0_hint)
            } else {
                (it0_hint, it1_hint)
            }
        };
        let (curr_lower, curr_upper) = curr_hint;
        let (next_lower, next_upper) = next_hint;
        let (combined_lower, combined_upper) =
            size_hint::mul_scalar(size_hint::min(curr_hint, next_hint), 2);
        let lower =
            if curr_lower > next_lower {
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
}

impl<I, J> FusedIterator for InterleaveShortest<I, J>
    where I: FusedIterator,
          J: FusedIterator<Item = I::Item>
{}

#[derive(Clone, Debug)]
/// An iterator adaptor that allows putting back a single
/// item to the front of the iterator.
///
/// Iterator element type is `I::Item`.
pub struct PutBack<I>
    where I: Iterator
{
    top: Option<I::Item>,
    iter: I,
}

/// Create an iterator where you can put back a single item
pub fn put_back<I>(iterable: I) -> PutBack<I::IntoIter>
    where I: IntoIterator
{
    PutBack {
        top: None,
        iter: iterable.into_iter(),
    }
}

impl<I> PutBack<I>
    where I: Iterator
{
    /// put back value `value` (builder method)
    pub fn with_value(mut self, value: I::Item) -> Self {
        self.put_back(value);
        self
    }

    /// Split the `PutBack` into its parts.
    #[inline]
    pub fn into_parts(self) -> (Option<I::Item>, I) {
        let PutBack{top, iter} = self;
        (top, iter)
    }

    /// Put back a single value to the front of the iterator.
    ///
    /// If a value is already in the put back slot, it is overwritten.
    #[inline]
    pub fn put_back(&mut self, x: I::Item) {
        self.top = Some(x);
    }
}

impl<I> Iterator for PutBack<I>
    where I: Iterator
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
        where G: FnMut(Self::Item) -> bool
    {
        if let Some(elt) = self.top.take() {
            if !f(elt) {
                return false;
            }
        }
        self.iter.all(f)
    }

    fn fold<Acc, G>(mut self, init: Acc, mut f: G) -> Acc
        where G: FnMut(Acc, Self::Item) -> Acc,
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
    where I: Iterator
{
    a: I,
    a_cur: Option<I::Item>,
    b: J,
    b_orig: J,
}

/// Create a new cartesian product iterator
///
/// Iterator element type is `(I::Item, J::Item)`.
pub fn cartesian_product<I, J>(mut i: I, j: J) -> Product<I, J>
    where I: Iterator,
          J: Clone + Iterator,
          I::Item: Clone
{
    Product {
        a_cur: i.next(),
        a: i,
        b: j.clone(),
        b_orig: j,
    }
}

impl<I, J> Iterator for Product<I, J>
    where I: Iterator,
          J: Clone + Iterator,
          I::Item: Clone
{
    type Item = (I::Item, J::Item);

    fn next(&mut self) -> Option<Self::Item> {
        let elt_b = match self.b.next() {
            None => {
                self.b = self.b_orig.clone();
                match self.b.next() {
                    None => return None,
                    Some(x) => {
                        self.a_cur = self.a.next();
                        x
                    }
                }
            }
            Some(x) => x
        };
        self.a_cur.as_ref().map(|a| (a.clone(), elt_b))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let has_cur = self.a_cur.is_some() as usize;
        // Not ExactSizeIterator because size may be larger than usize
        let (b_min, b_max) = self.b.size_hint();

        // Compute a * b_orig + b for both lower and upper bound
        size_hint::add(
            size_hint::mul(self.a.size_hint(), self.b_orig.size_hint()),
            (b_min * has_cur, b_max.map(move |x| x * has_cur)))
    }

    fn fold<Acc, G>(mut self, mut accum: Acc, mut f: G) -> Acc
        where G: FnMut(Acc, Self::Item) -> Acc,
    {
        // use a split loop to handle the loose a_cur as well as avoiding to
        // clone b_orig at the end.
        if let Some(mut a) = self.a_cur.take() {
            let mut b = self.b;
            loop {
                accum = b.fold(accum, |acc, elt| f(acc, (a.clone(), elt)));

                // we can only continue iterating a if we had a first element;
                if let Some(next_a) = self.a.next() {
                    b = self.b_orig.clone();
                    a = next_a;
                } else {
                    break;
                }
            }
        }
        accum
    }
}

impl<I, J> FusedIterator for Product<I, J>
    where I: FusedIterator,
          J: Clone + FusedIterator,
          I::Item: Clone
{}

/// A “meta iterator adaptor”. Its closure receives a reference to the iterator
/// and may pick off as many elements as it likes, to produce the next iterator element.
///
/// Iterator element type is *X*, if the return type of `F` is *Option\<X\>*.
///
/// See [`.batching()`](crate::Itertools::batching) for more information.
#[derive(Clone)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Batching<I, F> {
    f: F,
    iter: I,
}

impl<I, F> fmt::Debug for Batching<I, F> where I: fmt::Debug {
    debug_fmt_fields!(Batching, iter);
}

/// Create a new Batching iterator.
pub fn batching<I, F>(iter: I, f: F) -> Batching<I, F> {
    Batching { f, iter }
}

impl<B, F, I> Iterator for Batching<I, F>
    where I: Iterator,
          F: FnMut(&mut I) -> Option<B>
{
    type Item = B;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        (self.f)(&mut self.iter)
    }
}

/// An iterator adaptor that steps a number elements in the base iterator
/// for each iteration.
///
/// The iterator steps by yielding the next element from the base iterator,
/// then skipping forward *n-1* elements.
///
/// See [`.step()`](crate::Itertools::step) for more information.
#[deprecated(note="Use std .step_by() instead", since="0.8.0")]
#[allow(deprecated)]
#[derive(Clone, Debug)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Step<I> {
    iter: Fuse<I>,
    skip: usize,
}

/// Create a `Step` iterator.
///
/// **Panics** if the step is 0.
#[allow(deprecated)]
pub fn step<I>(iter: I, step: usize) -> Step<I>
    where I: Iterator
{
    assert!(step != 0);
    Step {
        iter: iter.fuse(),
        skip: step - 1,
    }
}

#[allow(deprecated)]
impl<I> Iterator for Step<I>
    where I: Iterator
{
    type Item = I::Item;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let elt = self.iter.next();
        if self.skip > 0 {
            self.iter.nth(self.skip - 1);
        }
        elt
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (low, high) = self.iter.size_hint();
        let div = |x: usize| {
            if x == 0 {
                0
            } else {
                1 + (x - 1) / (self.skip + 1)
            }
        };
        (div(low), high.map(div))
    }
}

// known size
#[allow(deprecated)]
impl<I> ExactSizeIterator for Step<I>
    where I: ExactSizeIterator
{}

pub trait MergePredicate<T> {
    fn merge_pred(&mut self, a: &T, b: &T) -> bool;
}

#[derive(Clone, Debug)]
pub struct MergeLte;

impl<T: PartialOrd> MergePredicate<T> for MergeLte {
    fn merge_pred(&mut self, a: &T, b: &T) -> bool {
        a <= b
    }
}

/// An iterator adaptor that merges the two base iterators in ascending order.
/// If both base iterators are sorted (ascending), the result is sorted.
///
/// Iterator element type is `I::Item`.
///
/// See [`.merge()`](crate::Itertools::merge_by) for more information.
pub type Merge<I, J> = MergeBy<I, J, MergeLte>;

/// Create an iterator that merges elements in `i` and `j`.
///
/// [`IntoIterator`] enabled version of [`Itertools::merge`](crate::Itertools::merge).
///
/// ```
/// use itertools::merge;
///
/// for elt in merge(&[1, 2, 3], &[2, 3, 4]) {
///     /* loop body */
/// }
/// ```
pub fn merge<I, J>(i: I, j: J) -> Merge<<I as IntoIterator>::IntoIter, <J as IntoIterator>::IntoIter>
    where I: IntoIterator,
          J: IntoIterator<Item = I::Item>,
          I::Item: PartialOrd
{
    merge_by_new(i, j, MergeLte)
}

/// An iterator adaptor that merges the two base iterators in ascending order.
/// If both base iterators are sorted (ascending), the result is sorted.
///
/// Iterator element type is `I::Item`.
///
/// See [`.merge_by()`](crate::Itertools::merge_by) for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct MergeBy<I, J, F>
    where I: Iterator,
          J: Iterator<Item = I::Item>
{
    a: Peekable<I>,
    b: Peekable<J>,
    fused: Option<bool>,
    cmp: F,
}

impl<I, J, F> fmt::Debug for MergeBy<I, J, F>
    where I: Iterator + fmt::Debug, J: Iterator<Item = I::Item> + fmt::Debug,
          I::Item: fmt::Debug,
{
    debug_fmt_fields!(MergeBy, a, b);
}

impl<T, F: FnMut(&T, &T)->bool> MergePredicate<T> for F {
    fn merge_pred(&mut self, a: &T, b: &T) -> bool {
        self(a, b)
    }
}

/// Create a `MergeBy` iterator.
pub fn merge_by_new<I, J, F>(a: I, b: J, cmp: F) -> MergeBy<I::IntoIter, J::IntoIter, F>
    where I: IntoIterator,
          J: IntoIterator<Item = I::Item>,
          F: MergePredicate<I::Item>,
{
    MergeBy {
        a: a.into_iter().peekable(),
        b: b.into_iter().peekable(),
        fused: None,
        cmp,
    }
}

impl<I, J, F> Clone for MergeBy<I, J, F>
    where I: Iterator,
          J: Iterator<Item = I::Item>,
          Peekable<I>: Clone,
          Peekable<J>: Clone,
          F: Clone
{
    clone_fields!(a, b, fused, cmp);
}

impl<I, J, F> Iterator for MergeBy<I, J, F>
    where I: Iterator,
          J: Iterator<Item = I::Item>,
          F: MergePredicate<I::Item>
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let less_than = match self.fused {
            Some(lt) => lt,
            None => match (self.a.peek(), self.b.peek()) {
                (Some(a), Some(b)) => self.cmp.merge_pred(a, b),
                (Some(_), None) => {
                    self.fused = Some(true);
                    true
                }
                (None, Some(_)) => {
                    self.fused = Some(false);
                    false
                }
                (None, None) => return None,
            }
        };
        if less_than {
            self.a.next()
        } else {
            self.b.next()
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Not ExactSizeIterator because size may be larger than usize
        size_hint::add(self.a.size_hint(), self.b.size_hint())
    }
}

impl<I, J, F> FusedIterator for MergeBy<I, J, F>
    where I: FusedIterator,
          J: FusedIterator<Item = I::Item>,
          F: MergePredicate<I::Item>
{}

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
    where I: Iterator + fmt::Debug,
{
    debug_fmt_fields!(TakeWhileRef, iter);
}

/// Create a new `TakeWhileRef` from a reference to clonable iterator.
pub fn take_while_ref<I, F>(iter: &mut I, f: F) -> TakeWhileRef<I, F>
    where I: Iterator + Clone
{
    TakeWhileRef { iter, f }
}

impl<'a, I, F> Iterator for TakeWhileRef<'a, I, F>
    where I: Iterator + Clone,
          F: FnMut(&I::Item) -> bool
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
    where I: Iterator<Item = Option<A>>
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
}

/// An iterator to iterate through all combinations in a `Clone`-able iterator that produces tuples
/// of a specific size.
///
/// See [`.tuple_combinations()`](crate::Itertools::tuple_combinations) for more
/// information.
#[derive(Clone, Debug)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct TupleCombinations<I, T>
    where I: Iterator,
          T: HasCombination<I>
{
    iter: T::Combination,
    _mi: PhantomData<I>,
}

pub trait HasCombination<I>: Sized {
    type Combination: From<I> + Iterator<Item = Self>;
}

/// Create a new `TupleCombinations` from a clonable iterator.
pub fn tuple_combinations<T, I>(iter: I) -> TupleCombinations<I, T>
    where I: Iterator + Clone,
          I::Item: Clone,
          T: HasCombination<I>,
{
    TupleCombinations {
        iter: T::Combination::from(iter),
        _mi: PhantomData,
    }
}

impl<I, T> Iterator for TupleCombinations<I, T>
    where I: Iterator,
          T: HasCombination<I>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<I, T> FusedIterator for TupleCombinations<I, T>
    where I: FusedIterator,
          T: HasCombination<I>,
{}

#[derive(Clone, Debug)]
pub struct Tuple1Combination<I> {
    iter: I,
}

impl<I> From<I> for Tuple1Combination<I> {
    fn from(iter: I) -> Self {
        Tuple1Combination { iter }
    }
}

impl<I: Iterator> Iterator for Tuple1Combination<I> {
    type Item = (I::Item,);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|x| (x,))
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
                  I::Item: Clone
        {
            type Item = (A, $(ignore_ident!($X, A)),*);

            fn next(&mut self) -> Option<Self::Item> {
                if let Some(($($X),*,)) = self.c.next() {
                    let z = self.item.clone().unwrap();
                    Some((z, $($X),*))
                } else {
                    self.item = self.iter.next();
                    self.item.clone().and_then(|z| {
                        self.c = self.iter.clone().into();
                        self.c.next().map(|($($X),*,)| (z, $($X),*))
                    })
                }
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

/// An iterator adapter to filter values within a nested `Result::Ok`.
///
/// See [`.filter_ok()`](crate::Itertools::filter_ok) for more information.
#[derive(Clone)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct FilterOk<I, F> {
    iter: I,
    f: F
}

impl<I, F> fmt::Debug for FilterOk<I, F>
where
    I: fmt::Debug,
{
    debug_fmt_fields!(FilterOk, iter);
}

/// Create a new `FilterOk` iterator.
pub fn filter_ok<I, F, T, E>(iter: I, f: F) -> FilterOk<I, F>
    where I: Iterator<Item = Result<T, E>>,
          F: FnMut(&T) -> bool,
{
    FilterOk {
        iter,
        f,
    }
}

impl<I, F, T, E> Iterator for FilterOk<I, F>
    where I: Iterator<Item = Result<T, E>>,
          F: FnMut(&T) -> bool,
{
    type Item = Result<T, E>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                Some(Ok(v)) => {
                    if (self.f)(&v) {
                        return Some(Ok(v));
                    }
                },
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }

    fn fold<Acc, Fold>(self, init: Acc, fold_f: Fold) -> Acc
        where Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.iter.filter(|v| {
            v.as_ref().map(&mut f).unwrap_or(true)
        }).fold(init, fold_f)
    }

    fn collect<C>(self) -> C
        where C: FromIterator<Self::Item>
    {
        let mut f = self.f;
        self.iter.filter(|v| {
            v.as_ref().map(&mut f).unwrap_or(true)
        }).collect()
    }
}

impl<I, F, T, E> FusedIterator for FilterOk<I, F>
    where I: FusedIterator<Item = Result<T, E>>,
          F: FnMut(&T) -> bool,
{}

/// An iterator adapter to filter and apply a transformation on values within a nested `Result::Ok`.
///
/// See [`.filter_map_ok()`](crate::Itertools::filter_map_ok) for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct FilterMapOk<I, F> {
    iter: I,
    f: F
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
    where I: Iterator<Item = Result<T, E>>,
          F: FnMut(T) -> Option<U>,
{
    FilterMapOk {
        iter,
        f,
    }
}

impl<I, F, T, U, E> Iterator for FilterMapOk<I, F>
    where I: Iterator<Item = Result<T, E>>,
          F: FnMut(T) -> Option<U>,
{
    type Item = Result<U, E>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                Some(Ok(v)) => {
                    if let Some(v) = (self.f)(v) {
                        return Some(Ok(v));
                    }
                },
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }

    fn fold<Acc, Fold>(self, init: Acc, fold_f: Fold) -> Acc
        where Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.iter.filter_map(|v| {
            transpose_result(v.map(&mut f))
        }).fold(init, fold_f)
    }

    fn collect<C>(self) -> C
        where C: FromIterator<Self::Item>
    {
        let mut f = self.f;
        self.iter.filter_map(|v| {
            transpose_result(v.map(&mut f))
        }).collect()
    }
}

impl<I, F, T, U, E> FusedIterator for FilterMapOk<I, F>
    where I: FusedIterator<Item = Result<T, E>>,
          F: FnMut(T) -> Option<U>,
{}

/// An iterator adapter to get the positions of each element that matches a predicate.
///
/// See [`.positions()`](crate::Itertools::positions) for more information.
#[derive(Clone)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Positions<I, F> {
    iter: I,
    f: F,
    count: usize,
}

impl<I, F> fmt::Debug for Positions<I, F>
where
    I: fmt::Debug,
{
    debug_fmt_fields!(Positions, iter, count);
}

/// Create a new `Positions` iterator.
pub fn positions<I, F>(iter: I, f: F) -> Positions<I, F>
    where I: Iterator,
          F: FnMut(I::Item) -> bool,
{
    Positions {
        iter,
        f,
        count: 0
    }
}

impl<I, F> Iterator for Positions<I, F>
    where I: Iterator,
          F: FnMut(I::Item) -> bool,
{
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(v) = self.iter.next() {
            let i = self.count;
            self.count = i + 1;
            if (self.f)(v) {
                return Some(i);
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }
}

impl<I, F> DoubleEndedIterator for Positions<I, F>
    where I: DoubleEndedIterator + ExactSizeIterator,
          F: FnMut(I::Item) -> bool,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        while let Some(v) = self.iter.next_back() {
            if (self.f)(v) {
                return Some(self.count + self.iter.len())
            }
        }
        None
    }
}

impl<I, F> FusedIterator for Positions<I, F>
    where I: FusedIterator,
          F: FnMut(I::Item) -> bool,
{}

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
        where G: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.iter.fold(init, move |acc, mut v| { f(&mut v); g(acc, v) })
    }

    // if possible, re-use inner iterator specializations in collect
    fn collect<C>(self) -> C
        where C: FromIterator<Self::Item>
    {
        let mut f = self.f;
        self.iter.map(move |mut v| { f(&mut v); v }).collect()
    }
}

impl<I, F> ExactSizeIterator for Update<I, F>
where
    I: ExactSizeIterator,
    F: FnMut(&mut I::Item),
{}

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
{}
