//! Streaming iterators.
//!
//! The iterator APIs in the Rust standard library do not allow elements to be yielded which borrow
//! from the iterator itself. That means, for example, that the `std::io::Lines` iterator must
//! allocate a new `String` for each line rather than reusing an internal buffer. The
//! `StreamingIterator` trait instead provides access to elements being iterated over only by
//! reference rather than by value.
//!
//! `StreamingIterator`s cannot be used in Rust `for` loops, but `while let` loops offer a similar
//! level of ergonomics:
//!
//! ```ignore
//! while let Some(item) = iter.next() {
//!     // work with item
//! }
//! ```
//!
//! However, **make sure to only use the above form with a mutable reference to an existing iterator**,
//! not with an expression that creates an iterator.
//! For example, the following code will loop forever over the first element of the array:
//!
//! ```no_run
//! use streaming_iterator::{convert, StreamingIterator};
//! let array = [0, 1, 2, 3];
//!
//! while let Some(item) = convert(array.iter()).next() {
//!   // This is an infinite loop!
//! }
//! ```
//!
//! While the standard `Iterator` trait's functionality is based off of the `next` method,
//! `StreamingIterator`'s functionality is based off of a pair of methods: `advance` and `get`. This
//! essentially splits the logic of `next` in half (in fact, `StreamingIterator`'s `next` method
//! does nothing but call `advance` followed by `get`).
//!
//! This is required because of Rust's lexical handling of borrows (more specifically a lack of
//! single entry, multiple exit borrows). If `StreamingIterator` was defined like `Iterator` with
//! just a required `next` method, operations like `filter` would be impossible to define.
#![doc(html_root_url = "https://docs.rs/streaming-iterator/0.1")]
#![warn(missing_docs)]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::cmp;

#[cfg(feature = "alloc")]
use alloc::{borrow::ToOwned, boxed::Box};

mod slice;
pub use crate::slice::{windows_mut, WindowsMut};

mod sources;
pub use crate::sources::{convert, Convert};
pub use crate::sources::{convert_mut, ConvertMut};
pub use crate::sources::{convert_ref, ConvertRef};
pub use crate::sources::{empty, Empty};
pub use crate::sources::{from_fn, FromFn};
pub use crate::sources::{once, Once};
pub use crate::sources::{once_with, OnceWith};
pub use crate::sources::{repeat, Repeat};
pub use crate::sources::{repeat_with, RepeatWith};
pub use crate::sources::{successors, Successors};

/// An interface for dealing with streaming iterators.
pub trait StreamingIterator {
    /// The type of the elements being iterated over.
    type Item: ?Sized;

    /// Advances the iterator to the next element.
    ///
    /// Iterators start just before the first element, so this should be called before `get`.
    ///
    /// The behavior of calling this method after the end of the iterator has been reached is
    /// unspecified.
    fn advance(&mut self);

    /// Returns a reference to the current element of the iterator.
    ///
    /// The behavior of calling this method before `advance` has been called is unspecified.
    fn get(&self) -> Option<&Self::Item>;

    /// Advances the iterator and returns the next value.
    ///
    /// The behavior of calling this method after the end of the iterator has been reached is
    /// unspecified.
    ///
    /// The default implementation simply calls `advance` followed by `get`.
    #[inline]
    fn next(&mut self) -> Option<&Self::Item> {
        self.advance();
        (*self).get()
    }

    /// Returns the bounds on the remaining length of the iterator.
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }

    /// Checks if `get()` will return `None`.
    fn is_done(&self) -> bool {
        self.get().is_none()
    }

    /// Determines if all elements of the iterator satisfy a predicate.
    #[inline]
    fn all<F>(&mut self, mut f: F) -> bool
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> bool,
    {
        while let Some(i) = self.next() {
            if !f(i) {
                return false;
            }
        }

        true
    }

    /// Determines if any elements of the iterator satisfy a predicate.
    #[inline]
    fn any<F>(&mut self, mut f: F) -> bool
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> bool,
    {
        !self.all(|i| !f(i))
    }

    /// Borrows an iterator, rather than consuming it.
    ///
    /// This is useful to allow the application of iterator adaptors while still retaining ownership
    /// of the original adaptor.
    #[inline]
    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }

    /// Consumes two iterators and returns a new iterator that iterates over both in sequence.
    #[inline]
    fn chain<I>(self, other: I) -> Chain<Self, I>
    where
        Self: Sized,
        I: StreamingIterator<Item = Self::Item> + Sized,
    {
        Chain {
            a: self,
            b: other,
            state: ChainState::BothForward,
        }
    }

    /// Produces a normal, non-streaming, iterator by cloning the elements of this iterator.
    #[inline]
    fn cloned(self) -> Cloned<Self>
    where
        Self: Sized,
        Self::Item: Clone,
    {
        Cloned(self)
    }

    /// Produces a normal, non-streaming, iterator by copying the elements of this iterator.
    #[inline]
    fn copied(self) -> Copied<Self>
    where
        Self: Sized,
        Self::Item: Copy,
    {
        Copied(self)
    }

    /// Consumes the iterator, counting the number of remaining elements and returning it.
    #[inline]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.fold(0, |count, _| count + 1)
    }

    /// Creates an iterator which uses a closure to determine if an element should be yielded.
    #[inline]
    fn filter<F>(self, f: F) -> Filter<Self, F>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> bool,
    {
        Filter { it: self, f }
    }

    /// Creates an iterator which both filters and maps by applying a closure to elements.
    #[inline]
    fn filter_map<B, F>(self, f: F) -> FilterMap<Self, B, F>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> Option<B>,
    {
        FilterMap {
            it: self,
            f,
            item: None,
        }
    }

    /// Creates an iterator which flattens iterators obtained by applying a closure to elements.
    /// Note that the returned iterators must be streaming iterators.
    #[inline]
    fn flat_map<J, F>(self, f: F) -> FlatMap<Self, J, F>
    where
        Self: Sized,
        J: StreamingIterator,
        F: FnMut(&Self::Item) -> J,
    {
        FlatMap {
            it: self,
            f,
            sub_iter: None,
        }
    }

    /// Creates a regular, non-streaming iterator which both filters and maps by applying a closure to elements.
    #[inline]
    fn filter_map_deref<B, F>(self, f: F) -> FilterMapDeref<Self, F>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> Option<B>,
    {
        FilterMapDeref { it: self, f }
    }

    /// Returns the first element of the iterator that satisfies the predicate.
    #[inline]
    fn find<F>(&mut self, mut f: F) -> Option<&Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> bool,
    {
        loop {
            self.advance();
            match self.get() {
                Some(i) => {
                    if f(i) {
                        break;
                    }
                }
                None => break,
            }
        }

        (*self).get()
    }

    /// Creates an iterator which is "well behaved" at the beginning and end of iteration.
    ///
    /// The behavior of calling `get` before iteration has been started, and of continuing to call
    /// `advance` after `get` has returned `None` is normally unspecified, but this guarantees that
    /// `get` will return `None` in both cases.
    #[inline]
    fn fuse(self) -> Fuse<Self>
    where
        Self: Sized,
    {
        Fuse {
            it: self,
            state: FuseState::Start,
        }
    }

    /// Call a closure on each element, passing the element on.
    /// The closure is called upon calls to `advance` or `advance_back`, and exactly once per element
    /// regardless of how many times (if any) `get` is called.
    #[inline]
    fn inspect<F>(self, f: F) -> Inspect<Self, F>
    where
        F: FnMut(&Self::Item),
        Self: Sized,
    {
        Inspect { it: self, f }
    }

    /// Creates an iterator which transforms elements of this iterator by passing them to a closure.
    #[inline]
    fn map<B, F>(self, f: F) -> Map<Self, B, F>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> B,
    {
        Map {
            it: self,
            f,
            item: None,
        }
    }

    /// Creates a regular, non-streaming iterator which transforms elements of this iterator by passing them to a closure.
    #[inline]
    fn map_deref<B, F>(self, f: F) -> MapDeref<Self, F>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> B,
    {
        MapDeref { it: self, f }
    }

    /// Creates an iterator which transforms elements of this iterator by passing them to a closure.
    ///
    /// Unlike `map`, this method takes a closure that returns a reference into the original value.
    ///
    /// The mapping function is only guaranteed to be called at some point before an element
    /// is actually consumed. This allows an expensive mapping function to be ignored
    /// during skipping (e.g. `nth`).
    #[inline]
    fn map_ref<B: ?Sized, F>(self, f: F) -> MapRef<Self, F>
    where
        Self: Sized,
        F: Fn(&Self::Item) -> &B,
    {
        MapRef { it: self, f }
    }

    /// Consumes the first `n` elements of the iterator, returning the next one.
    #[inline]
    fn nth(&mut self, n: usize) -> Option<&Self::Item> {
        for _ in 0..n {
            self.advance();
            if self.is_done() {
                return None;
            }
        }
        self.next()
    }

    /// Creates a normal, non-streaming, iterator with elements produced by calling `to_owned` on
    /// the elements of this iterator.
    ///
    /// Requires the `alloc` feature.
    #[cfg(feature = "alloc")]
    #[inline]
    fn owned(self) -> Owned<Self>
    where
        Self: Sized,
        Self::Item: ToOwned,
    {
        Owned(self)
    }

    /// Returns the index of the first element of the iterator matching a predicate.
    #[inline]
    fn position<F>(&mut self, mut f: F) -> Option<usize>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> bool,
    {
        let mut n = 0;

        while let Some(i) = self.next() {
            if f(i) {
                return Some(n);
            }
            n += 1;
        }

        None
    }

    /// Creates an iterator which skips the first `n` elements.
    #[inline]
    fn skip(self, n: usize) -> Skip<Self>
    where
        Self: Sized,
    {
        Skip { it: self, n }
    }

    /// Creates an iterator that skips initial elements matching a predicate.
    #[inline]
    fn skip_while<F>(self, f: F) -> SkipWhile<Self, F>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> bool,
    {
        SkipWhile {
            it: self,
            f,
            done: false,
        }
    }

    /// Creates an iterator which only returns the first `n` elements.
    #[inline]
    fn take(self, n: usize) -> Take<Self>
    where
        Self: Sized,
    {
        Take {
            it: self,
            n,
            done: false,
        }
    }

    /// Creates an iterator which only returns initial elements matching a predicate.
    #[inline]
    fn take_while<F>(self, f: F) -> TakeWhile<Self, F>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> bool,
    {
        TakeWhile {
            it: self,
            f,
            done: false,
        }
    }

    /// Creates an iterator which returns elemens in the opposite order.
    #[inline]
    fn rev(self) -> Rev<Self>
    where
        Self: Sized + DoubleEndedStreamingIterator,
    {
        Rev(self)
    }

    /// Reduces the iterator's elements to a single, final value.
    #[inline]
    fn fold<B, F>(mut self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, &Self::Item) -> B,
    {
        let mut acc = init;
        while let Some(item) = self.next() {
            acc = f(acc, item);
        }
        acc
    }

    /// Calls a closure on each element of an iterator.
    #[inline]
    fn for_each<F>(self, mut f: F)
    where
        Self: Sized,
        F: FnMut(&Self::Item),
    {
        self.fold((), move |(), item| f(item));
    }
}

impl<'a, I: ?Sized> StreamingIterator for &'a mut I
where
    I: StreamingIterator,
{
    type Item = I::Item;

    #[inline]
    fn advance(&mut self) {
        (**self).advance()
    }

    #[inline]
    fn is_done(&self) -> bool {
        (**self).is_done()
    }

    #[inline]
    fn get(&self) -> Option<&Self::Item> {
        (**self).get()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (**self).size_hint()
    }

    #[inline]
    fn next(&mut self) -> Option<&Self::Item> {
        (**self).next()
    }
}

#[cfg(feature = "alloc")]
impl<I: ?Sized> StreamingIterator for Box<I>
where
    I: StreamingIterator,
{
    type Item = I::Item;

    #[inline]
    fn advance(&mut self) {
        (**self).advance()
    }

    #[inline]
    fn is_done(&self) -> bool {
        (**self).is_done()
    }

    #[inline]
    fn get(&self) -> Option<&Self::Item> {
        (**self).get()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (**self).size_hint()
    }

    #[inline]
    fn next(&mut self) -> Option<&Self::Item> {
        (**self).next()
    }
}

/// A streaming iterator able to yield elements from both ends.
pub trait DoubleEndedStreamingIterator: StreamingIterator {
    /// Advances the iterator to the next element from the back of the iterator.
    ///
    /// Double ended iterators just after the last element, so this should be called before `get`
    /// when iterating in reverse.
    ///
    /// The behavior of calling this method after the iterator has been exhausted is unspecified.
    fn advance_back(&mut self);

    /// Advances the iterator and returns the next value from the back.
    ///
    /// The behavior of calling this method after the iterator has been exhausted is unspecified.
    ///
    /// The default implementation simply calls `advance_back` followed by `get`.
    #[inline]
    fn next_back(&mut self) -> Option<&Self::Item> {
        self.advance_back();
        (*self).get()
    }

    /// Reduces the iterator's elements to a single, final value, starting from the back.
    #[inline]
    fn rfold<B, F>(mut self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, &Self::Item) -> B,
    {
        let mut acc = init;
        while let Some(item) = self.next_back() {
            acc = f(acc, item);
        }
        acc
    }
}

/// An interface for dealing with mutable streaming iterators.
pub trait StreamingIteratorMut: StreamingIterator {
    /// Returns a mutable reference to the current element of the iterator.
    ///
    /// The behavior of calling this method before `advance` has been called is unspecified.
    ///
    /// Modifications through this reference may also have an unspecified effect on further
    /// iterator advancement, but implementations are encouraged to document this.
    fn get_mut(&mut self) -> Option<&mut Self::Item>;

    /// Advances the iterator and returns the next mutable value.
    ///
    /// The behavior of calling this method after the end of the iterator has been reached is
    /// unspecified.
    ///
    /// The default implementation simply calls `advance` followed by `get_mut`.
    #[inline]
    fn next_mut(&mut self) -> Option<&mut Self::Item> {
        self.advance();
        (*self).get_mut()
    }

    /// Reduces the iterator's mutable elements to a single, final value.
    #[inline]
    fn fold_mut<B, F>(mut self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, &mut Self::Item) -> B,
    {
        let mut acc = init;
        while let Some(item) = self.next_mut() {
            acc = f(acc, item);
        }
        acc
    }

    /// Calls a closure on each mutable element of an iterator.
    #[inline]
    fn for_each_mut<F>(self, mut f: F)
    where
        Self: Sized,
        F: FnMut(&mut Self::Item),
    {
        self.fold_mut((), move |(), item| f(item));
    }

    /// Creates a regular, non-streaming iterator which transforms mutable elements
    /// of this iterator by passing them to a closure.
    #[inline]
    fn map_deref_mut<B, F>(self, f: F) -> MapDerefMut<Self, F>
    where
        Self: Sized,
        F: FnMut(&mut Self::Item) -> B,
    {
        MapDerefMut { it: self, f }
    }

    /// Creates an iterator which flattens nested streaming iterators.
    #[inline]
    fn flatten(self) -> Flatten<Self>
    where
        Self: Sized,
        Self::Item: StreamingIterator,
    {
        Flatten {
            iter: self,
            first: true,
        }
    }
}

impl<'a, I: ?Sized> StreamingIteratorMut for &'a mut I
where
    I: StreamingIteratorMut,
{
    #[inline]
    fn get_mut(&mut self) -> Option<&mut Self::Item> {
        (**self).get_mut()
    }

    #[inline]
    fn next_mut(&mut self) -> Option<&mut Self::Item> {
        (**self).next_mut()
    }
}

#[cfg(feature = "alloc")]
impl<I: ?Sized> StreamingIteratorMut for Box<I>
where
    I: StreamingIteratorMut,
{
    #[inline]
    fn get_mut(&mut self) -> Option<&mut Self::Item> {
        (**self).get_mut()
    }

    #[inline]
    fn next_mut(&mut self) -> Option<&mut Self::Item> {
        (**self).next_mut()
    }
}

/// A mutable streaming iterator able to yield elements from both ends.
pub trait DoubleEndedStreamingIteratorMut:
    DoubleEndedStreamingIterator + StreamingIteratorMut
{
    /// Advances the iterator and returns the next mutable value from the back.
    ///
    /// The behavior of calling this method after the end of the iterator has been reached is
    /// unspecified.
    ///
    /// The default implementation simply calls `advance_back` followed by `get_mut`.
    #[inline]
    fn next_back_mut(&mut self) -> Option<&mut Self::Item> {
        self.advance_back();
        (*self).get_mut()
    }

    /// Reduces the iterator's mutable elements to a single, final value, starting from the back.
    #[inline]
    fn rfold_mut<B, F>(mut self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, &mut Self::Item) -> B,
    {
        let mut acc = init;
        while let Some(item) = self.next_back_mut() {
            acc = f(acc, item);
        }
        acc
    }
}
// Note, in theory we could blanket-impl `DoubleEndedStreamingIteratorMut`, but that
// wouldn't allow custom folding until we can do it with Rust specialization.

/// A streaming iterator that concatenates two streaming iterators
#[derive(Debug)]
pub struct Chain<A, B> {
    a: A,
    b: B,
    state: ChainState,
}

#[derive(Debug)]
enum ChainState {
    // Both iterators have items remaining and we are iterating forward
    BothForward,
    // Both iterators have items remaining and we are iterating backward
    BothBackward,
    // Only the front iterator has items
    Front,
    // Only the back iterator has items
    Back,
}

impl<A, B> StreamingIterator for Chain<A, B>
where
    A: StreamingIterator,
    B: StreamingIterator<Item = A::Item>,
{
    type Item = A::Item;

    #[inline]
    fn advance(&mut self) {
        use crate::ChainState::*;

        match self.state {
            BothForward | BothBackward => {
                self.a.advance();
                self.state = if self.a.is_done() {
                    self.b.advance();
                    Back
                } else {
                    BothForward
                };
            }
            Front => self.a.advance(),
            Back => self.b.advance(),
        }
    }

    #[inline]
    fn is_done(&self) -> bool {
        use crate::ChainState::*;

        match self.state {
            BothForward | Front => self.a.is_done(),
            BothBackward | Back => self.b.is_done(),
        }
    }

    #[inline]
    fn get(&self) -> Option<&Self::Item> {
        use crate::ChainState::*;

        match self.state {
            BothForward | Front => self.a.get(),
            BothBackward | Back => self.b.get(),
        }
    }

    #[inline]
    fn fold<Acc, F>(self, init: Acc, mut f: F) -> Acc
    where
        Self: Sized,
        F: FnMut(Acc, &Self::Item) -> Acc,
    {
        let mut accum = init;
        match self.state {
            ChainState::Back => {}
            _ => accum = self.a.fold(accum, &mut f),
        }
        match self.state {
            ChainState::Front => {}
            _ => accum = self.b.fold(accum, &mut f),
        }
        accum
    }
}

impl<A, B> DoubleEndedStreamingIterator for Chain<A, B>
where
    A: DoubleEndedStreamingIterator,
    B: DoubleEndedStreamingIterator<Item = A::Item>,
{
    #[inline]
    fn advance_back(&mut self) {
        use crate::ChainState::*;

        match self.state {
            BothForward | BothBackward => {
                self.b.advance_back();
                self.state = if self.b.is_done() {
                    self.a.advance_back();
                    Front
                } else {
                    BothBackward
                };
            }
            Front => self.a.advance_back(),
            Back => self.b.advance_back(),
        }
    }

    #[inline]
    fn rfold<Acc, F>(self, init: Acc, mut f: F) -> Acc
    where
        Self: Sized,
        F: FnMut(Acc, &Self::Item) -> Acc,
    {
        let mut accum = init;
        match self.state {
            ChainState::Front => {}
            _ => accum = self.b.rfold(accum, &mut f),
        }
        match self.state {
            ChainState::Back => {}
            _ => accum = self.a.rfold(accum, &mut f),
        }
        accum
    }
}

impl<A, B> StreamingIteratorMut for Chain<A, B>
where
    A: StreamingIteratorMut,
    B: StreamingIteratorMut<Item = A::Item>,
{
    #[inline]
    fn get_mut(&mut self) -> Option<&mut Self::Item> {
        use crate::ChainState::*;

        match self.state {
            BothForward | Front => self.a.get_mut(),
            BothBackward | Back => self.b.get_mut(),
        }
    }

    #[inline]
    fn fold_mut<Acc, F>(self, init: Acc, mut f: F) -> Acc
    where
        Self: Sized,
        F: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        let mut accum = init;
        match self.state {
            ChainState::Back => {}
            _ => accum = self.a.fold_mut(accum, &mut f),
        }
        match self.state {
            ChainState::Front => {}
            _ => accum = self.b.fold_mut(accum, &mut f),
        }
        accum
    }
}

impl<A, B> DoubleEndedStreamingIteratorMut for Chain<A, B>
where
    A: DoubleEndedStreamingIteratorMut,
    B: DoubleEndedStreamingIteratorMut<Item = A::Item>,
{
    fn rfold_mut<Acc, F>(self, init: Acc, mut f: F) -> Acc
    where
        Self: Sized,
        F: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        let mut accum = init;
        match self.state {
            ChainState::Front => {}
            _ => accum = self.b.rfold_mut(accum, &mut f),
        }
        match self.state {
            ChainState::Back => {}
            _ => accum = self.a.rfold_mut(accum, &mut f),
        }
        accum
    }
}

/// A normal, non-streaming, iterator which converts the elements of a streaming iterator into owned
/// values by cloning them.
#[derive(Clone, Debug)]
pub struct Cloned<I>(I);

impl<I> Iterator for Cloned<I>
where
    I: StreamingIterator,
    I::Item: Clone,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        self.0.next().cloned()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, mut f: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.0.fold(init, move |acc, item| f(acc, item.clone()))
    }
}

impl<I> DoubleEndedIterator for Cloned<I>
where
    I: DoubleEndedStreamingIterator,
    I::Item: Clone,
{
    #[inline]
    fn next_back(&mut self) -> Option<I::Item> {
        self.0.next_back().cloned()
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, mut f: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.0.rfold(init, move |acc, item| f(acc, item.clone()))
    }
}

/// A normal, non-streaming, iterator which converts the elements of a streaming iterator into owned
/// values by copying them.
#[derive(Clone, Debug)]
pub struct Copied<I>(I);

impl<I> Iterator for Copied<I>
where
    I: StreamingIterator,
    I::Item: Copy,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        self.0.next().copied()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, mut f: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.0.fold(init, move |acc, &item| f(acc, item))
    }
}

impl<I> DoubleEndedIterator for Copied<I>
where
    I: DoubleEndedStreamingIterator,
    I::Item: Copy,
{
    #[inline]
    fn next_back(&mut self) -> Option<I::Item> {
        self.0.next_back().copied()
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, mut f: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.0.rfold(init, move |acc, &item| f(acc, item))
    }
}

/// A streaming iterator which filters the elements of a streaming iterator with a predicate.
#[derive(Debug)]
pub struct Filter<I, F> {
    it: I,
    f: F,
}

impl<I, F> StreamingIterator for Filter<I, F>
where
    I: StreamingIterator,
    F: FnMut(&I::Item) -> bool,
{
    type Item = I::Item;

    #[inline]
    fn advance(&mut self) {
        while let Some(i) = self.it.next() {
            if (self.f)(i) {
                break;
            }
        }
    }

    #[inline]
    fn is_done(&self) -> bool {
        self.it.is_done()
    }

    #[inline]
    fn get(&self) -> Option<&I::Item> {
        self.it.get()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.it.size_hint().1)
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.fold(
            init,
            move |acc, item| {
                if f(item) {
                    fold(acc, item)
                } else {
                    acc
                }
            },
        )
    }
}

impl<I, F> DoubleEndedStreamingIterator for Filter<I, F>
where
    I: DoubleEndedStreamingIterator,
    F: FnMut(&I::Item) -> bool,
{
    #[inline]
    fn advance_back(&mut self) {
        while let Some(i) = self.it.next_back() {
            if (self.f)(i) {
                break;
            }
        }
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.rfold(
            init,
            move |acc, item| {
                if f(item) {
                    fold(acc, item)
                } else {
                    acc
                }
            },
        )
    }
}

impl<I, F> StreamingIteratorMut for Filter<I, F>
where
    I: StreamingIteratorMut,
    F: FnMut(&I::Item) -> bool,
{
    #[inline]
    fn get_mut(&mut self) -> Option<&mut I::Item> {
        self.it.get_mut()
    }

    #[inline]
    fn fold_mut<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.fold_mut(
            init,
            move |acc, item| {
                if f(&*item) {
                    fold(acc, item)
                } else {
                    acc
                }
            },
        )
    }
}

impl<I, F> DoubleEndedStreamingIteratorMut for Filter<I, F>
where
    I: DoubleEndedStreamingIteratorMut,
    F: FnMut(&I::Item) -> bool,
{
    #[inline]
    fn rfold_mut<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.rfold_mut(
            init,
            move |acc, item| {
                if f(&*item) {
                    fold(acc, item)
                } else {
                    acc
                }
            },
        )
    }
}

/// An iterator which both filters and maps elements of a streaming iterator with a closure.
#[derive(Debug)]
pub struct FilterMap<I, B, F> {
    it: I,
    f: F,
    item: Option<B>,
}

impl<I, B, F> StreamingIterator for FilterMap<I, B, F>
where
    I: StreamingIterator,
    F: FnMut(&I::Item) -> Option<B>,
{
    type Item = B;

    #[inline]
    fn advance(&mut self) {
        loop {
            match self.it.next() {
                Some(i) => {
                    if let Some(i) = (self.f)(i) {
                        self.item = Some(i);
                        break;
                    }
                }
                None => {
                    self.item = None;
                    break;
                }
            }
        }
    }

    #[inline]
    fn get(&self) -> Option<&B> {
        self.item.as_ref()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.it.size_hint().1)
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.fold(init, move |acc, item| match f(item) {
            Some(item) => fold(acc, &item),
            None => acc,
        })
    }
}

impl<I, B, F> DoubleEndedStreamingIterator for FilterMap<I, B, F>
where
    I: DoubleEndedStreamingIterator,
    F: FnMut(&I::Item) -> Option<B>,
{
    #[inline]
    fn advance_back(&mut self) {
        loop {
            match self.it.next_back() {
                Some(i) => {
                    if let Some(i) = (self.f)(i) {
                        self.item = Some(i);
                        break;
                    }
                }
                None => {
                    self.item = None;
                    break;
                }
            }
        }
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.rfold(init, move |acc, item| match f(item) {
            Some(item) => fold(acc, &item),
            None => acc,
        })
    }
}

impl<I, B, F> StreamingIteratorMut for FilterMap<I, B, F>
where
    I: StreamingIterator,
    F: FnMut(&I::Item) -> Option<B>,
{
    #[inline]
    fn get_mut(&mut self) -> Option<&mut B> {
        self.item.as_mut()
    }

    #[inline]
    fn fold_mut<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.fold(init, move |acc, item| match f(item) {
            Some(mut item) => fold(acc, &mut item),
            None => acc,
        })
    }
}

impl<I, B, F> DoubleEndedStreamingIteratorMut for FilterMap<I, B, F>
where
    I: DoubleEndedStreamingIterator,
    F: FnMut(&I::Item) -> Option<B>,
{
    #[inline]
    fn rfold_mut<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.rfold(init, move |acc, item| match f(item) {
            Some(mut item) => fold(acc, &mut item),
            None => acc,
        })
    }
}

/// A streaming iterator that maps elements to iterators with a closure and then yields the
/// concatenation of the obtained iterators
#[derive(Debug)]
pub struct FlatMap<I, J, F> {
    it: I,
    f: F,
    sub_iter: Option<J>,
}

impl<I, J, F> StreamingIterator for FlatMap<I, J, F>
where
    I: StreamingIterator,
    F: FnMut(&I::Item) -> J,
    J: StreamingIterator,
{
    type Item = J::Item;

    #[inline]
    fn advance(&mut self) {
        loop {
            if let Some(ref mut iter) = self.sub_iter {
                iter.advance();
                if !iter.is_done() {
                    break;
                }
            }
            if let Some(item) = self.it.next() {
                self.sub_iter = Some((self.f)(item));
            } else {
                break;
            }
        }
    }

    #[inline]
    fn is_done(&self) -> bool {
        match self.sub_iter {
            Some(ref iter) => iter.is_done(),
            None => true,
        }
    }

    #[inline]
    fn get(&self) -> Option<&Self::Item> {
        self.sub_iter.as_ref().and_then(J::get)
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        let mut acc = init;
        if let Some(iter) = self.sub_iter {
            acc = iter.fold(acc, &mut fold);
        }
        let mut f = self.f;
        self.it.fold(acc, |acc, item| f(item).fold(acc, &mut fold))
    }
}

impl<I, J, F> StreamingIteratorMut for FlatMap<I, J, F>
where
    I: StreamingIterator,
    F: FnMut(&I::Item) -> J,
    J: StreamingIteratorMut,
{
    #[inline]
    fn get_mut(&mut self) -> Option<&mut Self::Item> {
        self.sub_iter.as_mut().and_then(J::get_mut)
    }

    #[inline]
    fn fold_mut<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        let mut acc = init;
        if let Some(iter) = self.sub_iter {
            acc = iter.fold_mut(acc, &mut fold);
        }
        let mut f = self.f;
        self.it
            .fold(acc, |acc, item| f(item).fold_mut(acc, &mut fold))
    }
}

/// A streaming iterator that flattens nested streaming iterators.
#[derive(Debug)]
pub struct Flatten<I> {
    iter: I,
    first: bool,
}

impl<I> StreamingIterator for Flatten<I>
where
    I: StreamingIteratorMut,
    I::Item: StreamingIterator,
{
    type Item = <I::Item as StreamingIterator>::Item;

    #[inline]
    fn advance(&mut self) {
        if self.first {
            self.first = false;
            self.iter.advance();
        }
        while let Some(iter) = self.iter.get_mut() {
            iter.advance();
            if !iter.is_done() {
                break;
            }
            self.iter.advance(); // since we got Some, self.iter is not done and can be advanced
        }
    }

    #[inline]
    fn is_done(&self) -> bool {
        match self.iter.get() {
            Some(iter) => iter.is_done(),
            None => true,
        }
    }

    #[inline]
    fn get(&self) -> Option<&Self::Item> {
        self.iter.get().and_then(I::Item::get)
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        self.iter
            .fold_mut(init, |acc, item| item.fold(acc, &mut fold))
    }
}

impl<I> StreamingIteratorMut for Flatten<I>
where
    I: StreamingIteratorMut,
    I::Item: StreamingIteratorMut,
{
    #[inline]
    fn get_mut(&mut self) -> Option<&mut Self::Item> {
        self.iter.get_mut().and_then(I::Item::get_mut)
    }

    #[inline]
    fn fold_mut<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        self.iter
            .fold_mut(init, |acc, item| item.fold_mut(acc, &mut fold))
    }
}

/// A regular, non-streaming iterator which both filters and maps elements of a streaming iterator with a closure.
#[derive(Debug)]
pub struct FilterMapDeref<I, F> {
    it: I,
    f: F,
}

impl<I, B, F> Iterator for FilterMapDeref<I, F>
where
    I: StreamingIterator,
    F: FnMut(&I::Item) -> Option<B>,
{
    type Item = B;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(item) = self.it.next() {
            if let Some(mapped) = (self.f)(item) {
                return Some(mapped);
            }
        }

        None
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, mut f: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut map = self.f;
        self.it.fold(init, move |acc, item| match map(item) {
            Some(mapped) => f(acc, mapped),
            None => acc,
        })
    }
}

impl<I, B, F> DoubleEndedIterator for FilterMapDeref<I, F>
where
    I: DoubleEndedStreamingIterator,
    F: FnMut(&I::Item) -> Option<B>,
{
    #[inline]
    fn next_back(&mut self) -> Option<B> {
        while let Some(item) = self.it.next_back() {
            if let Some(mapped) = (self.f)(item) {
                return Some(mapped);
            }
        }

        None
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, mut f: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut map = self.f;
        self.it.rfold(init, move |acc, item| match map(item) {
            Some(mapped) => f(acc, mapped),
            None => acc,
        })
    }
}

#[derive(Copy, Clone, Debug)]
enum FuseState {
    Start,
    Middle,
    End,
}

/// A streaming iterator which is well-defined before and after iteration.
#[derive(Clone, Debug)]
pub struct Fuse<I> {
    it: I,
    state: FuseState,
}

impl<I> StreamingIterator for Fuse<I>
where
    I: StreamingIterator,
{
    type Item = I::Item;

    #[inline]
    fn advance(&mut self) {
        match self.state {
            FuseState::Start => {
                self.it.advance();
                self.state = if self.it.is_done() {
                    FuseState::End
                } else {
                    FuseState::Middle
                };
            }
            FuseState::Middle => {
                self.it.advance();
                if self.it.is_done() {
                    self.state = FuseState::End;
                }
            }
            FuseState::End => {}
        }
    }

    #[inline]
    fn is_done(&self) -> bool {
        match self.state {
            FuseState::Start | FuseState::End => true,
            FuseState::Middle => false,
        }
    }

    #[inline]
    fn get(&self) -> Option<&I::Item> {
        match self.state {
            FuseState::Start | FuseState::End => None,
            FuseState::Middle => self.it.get(),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }

    #[inline]
    fn next(&mut self) -> Option<&I::Item> {
        match self.state {
            FuseState::Start => match self.it.next() {
                Some(i) => {
                    self.state = FuseState::Middle;
                    Some(i)
                }
                None => {
                    self.state = FuseState::End;
                    None
                }
            },
            FuseState::Middle => match self.it.next() {
                Some(i) => Some(i),
                None => {
                    self.state = FuseState::End;
                    None
                }
            },
            FuseState::End => None,
        }
    }

    #[inline]
    fn count(self) -> usize {
        match self.state {
            FuseState::Start | FuseState::Middle => self.it.count(),
            FuseState::End => 0,
        }
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        match self.state {
            FuseState::Start | FuseState::Middle => self.it.fold(init, fold),
            FuseState::End => init,
        }
    }
}

impl<I> StreamingIteratorMut for Fuse<I>
where
    I: StreamingIteratorMut,
{
    #[inline]
    fn get_mut(&mut self) -> Option<&mut I::Item> {
        match self.state {
            FuseState::Start | FuseState::End => None,
            FuseState::Middle => self.it.get_mut(),
        }
    }

    #[inline]
    fn fold_mut<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        match self.state {
            FuseState::Start | FuseState::Middle => self.it.fold_mut(init, fold),
            FuseState::End => init,
        }
    }
}

/// A streaming iterator that calls a function with element before yielding it.
#[derive(Debug)]
pub struct Inspect<I, F> {
    it: I,
    f: F,
}

impl<I, F> StreamingIterator for Inspect<I, F>
where
    I: StreamingIterator,
    F: FnMut(&I::Item),
{
    type Item = I::Item;

    fn advance(&mut self) {
        if let Some(item) = self.it.next() {
            (self.f)(item);
        }
    }

    #[inline]
    fn is_done(&self) -> bool {
        self.it.is_done()
    }

    fn get(&self) -> Option<&Self::Item> {
        self.it.get()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.fold(init, |acc, item| {
            f(item);
            fold(acc, item)
        })
    }
}

impl<I, F> DoubleEndedStreamingIterator for Inspect<I, F>
where
    I: DoubleEndedStreamingIterator,
    F: FnMut(&I::Item),
{
    fn advance_back(&mut self) {
        if let Some(item) = self.it.next_back() {
            (self.f)(item);
        }
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.rfold(init, |acc, item| {
            f(item);
            fold(acc, item)
        })
    }
}

impl<I, F> StreamingIteratorMut for Inspect<I, F>
where
    I: StreamingIteratorMut,
    F: FnMut(&I::Item),
{
    fn get_mut(&mut self) -> Option<&mut Self::Item> {
        self.it.get_mut()
    }

    #[inline]
    fn fold_mut<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.fold_mut(init, |acc, item| {
            f(&*item);
            fold(acc, item)
        })
    }
}

impl<I, F> DoubleEndedStreamingIteratorMut for Inspect<I, F>
where
    I: DoubleEndedStreamingIteratorMut,
    F: FnMut(&I::Item),
{
    #[inline]
    fn rfold_mut<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.rfold_mut(init, |acc, item| {
            f(&*item);
            fold(acc, item)
        })
    }
}

/// A streaming iterator which transforms the elements of a streaming iterator.
#[derive(Debug)]
pub struct Map<I, B, F> {
    it: I,
    f: F,
    item: Option<B>,
}

impl<I, B, F> StreamingIterator for Map<I, B, F>
where
    I: StreamingIterator,
    F: FnMut(&I::Item) -> B,
{
    type Item = B;

    #[inline]
    fn advance(&mut self) {
        self.item = self.it.next().map(&mut self.f);
    }

    #[inline]
    fn get(&self) -> Option<&B> {
        self.item.as_ref()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.fold(init, move |acc, item| fold(acc, &f(item)))
    }
}

impl<I, B, F> DoubleEndedStreamingIterator for Map<I, B, F>
where
    I: DoubleEndedStreamingIterator,
    F: FnMut(&I::Item) -> B,
{
    #[inline]
    fn advance_back(&mut self) {
        self.item = self.it.next_back().map(&mut self.f);
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.rfold(init, move |acc, item| fold(acc, &f(item)))
    }
}

impl<I, B, F> StreamingIteratorMut for Map<I, B, F>
where
    I: StreamingIterator,
    F: FnMut(&I::Item) -> B,
{
    #[inline]
    fn get_mut(&mut self) -> Option<&mut B> {
        self.item.as_mut()
    }

    #[inline]
    fn fold_mut<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it.fold(init, move |acc, item| fold(acc, &mut f(item)))
    }
}

impl<I, B, F> DoubleEndedStreamingIteratorMut for Map<I, B, F>
where
    I: DoubleEndedStreamingIterator,
    F: FnMut(&I::Item) -> B,
{
    #[inline]
    fn rfold_mut<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        let mut f = self.f;
        self.it
            .rfold(init, move |acc, item| fold(acc, &mut f(item)))
    }
}

/// A regular, non-streaming iterator which transforms the elements of a streaming iterator.
#[derive(Debug)]
pub struct MapDeref<I, F> {
    it: I,
    f: F,
}

impl<I, B, F> Iterator for MapDeref<I, F>
where
    I: StreamingIterator,
    F: FnMut(&I::Item) -> B,
{
    type Item = B;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.it.next().map(&mut self.f)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, mut f: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut map = self.f;
        self.it.fold(init, move |acc, item| f(acc, map(item)))
    }
}

impl<I, B, F> DoubleEndedIterator for MapDeref<I, F>
where
    I: DoubleEndedStreamingIterator,
    F: FnMut(&I::Item) -> B,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.it.next_back().map(&mut self.f)
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, mut f: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut map = self.f;
        self.it.rfold(init, move |acc, item| f(acc, map(item)))
    }
}

/// A regular, non-streaming iterator which transforms the elements of a mutable streaming iterator.
#[derive(Debug)]
pub struct MapDerefMut<I, F> {
    it: I,
    f: F,
}

impl<I, B, F> Iterator for MapDerefMut<I, F>
where
    I: StreamingIteratorMut,
    F: FnMut(&mut I::Item) -> B,
{
    type Item = B;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.it.next_mut().map(&mut self.f)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, mut f: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut map = self.f;
        self.it.fold_mut(init, move |acc, item| f(acc, map(item)))
    }
}

impl<I, B, F> DoubleEndedIterator for MapDerefMut<I, F>
where
    I: DoubleEndedStreamingIteratorMut,
    F: FnMut(&mut I::Item) -> B,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.it.next_back_mut().map(&mut self.f)
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, mut f: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut map = self.f;
        self.it.rfold_mut(init, move |acc, item| f(acc, map(item)))
    }
}

/// A streaming iterator which transforms the elements of a streaming iterator.
#[derive(Debug)]
pub struct MapRef<I, F> {
    it: I,
    f: F,
}

impl<I, B: ?Sized, F> StreamingIterator for MapRef<I, F>
where
    I: StreamingIterator,
    F: Fn(&I::Item) -> &B,
{
    type Item = B;

    #[inline]
    fn advance(&mut self) {
        self.it.advance();
    }

    #[inline]
    fn is_done(&self) -> bool {
        self.it.is_done()
    }

    #[inline]
    fn get(&self) -> Option<&B> {
        self.it.get().map(&self.f)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }

    #[inline]
    fn next(&mut self) -> Option<&B> {
        self.it.next().map(&self.f)
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        let f = self.f;
        self.it.fold(init, move |acc, item| fold(acc, f(item)))
    }
}

/// A normal, non-streaming, iterator which converts the elements of a streaming iterator into owned
/// versions.
///
/// Requires the `alloc` feature.
#[cfg(feature = "alloc")]
#[derive(Clone, Debug)]
pub struct Owned<I>(I);

#[cfg(feature = "alloc")]
impl<I> Iterator for Owned<I>
where
    I: StreamingIterator,
    I::Item: ToOwned,
{
    type Item = <I::Item as ToOwned>::Owned;

    #[inline]
    fn next(&mut self) -> Option<<I::Item as ToOwned>::Owned> {
        self.0.next().map(ToOwned::to_owned)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, mut f: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.0.fold(init, move |acc, item| f(acc, item.to_owned()))
    }
}

#[cfg(feature = "alloc")]
impl<I> DoubleEndedIterator for Owned<I>
where
    I: DoubleEndedStreamingIterator,
    I::Item: Sized + ToOwned,
{
    #[inline]
    fn next_back(&mut self) -> Option<<I::Item as ToOwned>::Owned> {
        self.0.next_back().map(ToOwned::to_owned)
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, mut f: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.0.rfold(init, move |acc, item| f(acc, item.to_owned()))
    }
}

/// A streaming iterator which skips a number of elements in a streaming iterator.
#[derive(Clone, Debug)]
pub struct Skip<I> {
    it: I,
    n: usize,
}

impl<I> StreamingIterator for Skip<I>
where
    I: StreamingIterator,
{
    type Item = I::Item;

    #[inline]
    fn advance(&mut self) {
        self.it.nth(self.n);
        self.n = 0;
    }

    #[inline]
    fn is_done(&self) -> bool {
        self.it.is_done()
    }

    #[inline]
    fn get(&self) -> Option<&I::Item> {
        self.it.get()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let hint = self.it.size_hint();
        (
            hint.0.saturating_sub(self.n),
            hint.1.map(|n| n.saturating_sub(self.n)),
        )
    }

    #[inline]
    fn fold<Acc, Fold>(mut self, init: Acc, fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        if self.n > 0 {
            // nth(n) skips n+1
            if self.it.nth(self.n - 1).is_none() {
                return init;
            }
        }
        self.it.fold(init, fold)
    }
}

impl<I> StreamingIteratorMut for Skip<I>
where
    I: StreamingIteratorMut,
{
    fn get_mut(&mut self) -> Option<&mut Self::Item> {
        self.it.get_mut()
    }

    #[inline]
    fn fold_mut<Acc, Fold>(mut self, init: Acc, fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        if self.n > 0 {
            // nth(n) skips n+1
            if self.it.nth(self.n - 1).is_none() {
                return init;
            }
        }
        self.it.fold_mut(init, fold)
    }
}

/// A streaming iterator which skips initial elements that match a predicate
#[derive(Clone, Debug)]
pub struct SkipWhile<I, F> {
    it: I,
    f: F,
    done: bool,
}

impl<I, F> StreamingIterator for SkipWhile<I, F>
where
    I: StreamingIterator,
    F: FnMut(&I::Item) -> bool,
{
    type Item = I::Item;

    #[inline]
    fn advance(&mut self) {
        if !self.done {
            let f = &mut self.f;
            self.it.find(|i| !f(i));
            self.done = true;
        } else {
            self.it.advance();
        }
    }

    #[inline]
    fn is_done(&self) -> bool {
        self.it.is_done()
    }

    #[inline]
    fn get(&self) -> Option<&I::Item> {
        self.it.get()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let hint = self.it.size_hint();
        (0, hint.1)
    }

    #[inline]
    fn fold<Acc, Fold>(mut self, mut init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        if !self.done {
            match self.next() {
                Some(item) => init = fold(init, item),
                None => return init,
            }
        }
        self.it.fold(init, fold)
    }
}

impl<I, F> StreamingIteratorMut for SkipWhile<I, F>
where
    I: StreamingIteratorMut,
    F: FnMut(&I::Item) -> bool,
{
    fn get_mut(&mut self) -> Option<&mut Self::Item> {
        self.it.get_mut()
    }

    #[inline]
    fn fold_mut<Acc, Fold>(mut self, mut init: Acc, mut fold: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &mut Self::Item) -> Acc,
    {
        if !self.done {
            match self.next_mut() {
                Some(item) => init = fold(init, item),
                None => return init,
            }
        }
        self.it.fold_mut(init, fold)
    }
}

/// A streaming iterator which only yields a limited number of elements in a streaming iterator.
#[derive(Clone, Debug)]
pub struct Take<I> {
    it: I,
    n: usize,
    done: bool,
}

impl<I> StreamingIterator for Take<I>
where
    I: StreamingIterator,
{
    type Item = I::Item;

    #[inline]
    fn advance(&mut self) {
        if self.n != 0 {
            self.it.advance();
            self.n -= 1;
        } else {
            self.done = true;
        }
    }

    #[inline]
    fn is_done(&self) -> bool {
        self.done || self.it.is_done()
    }

    #[inline]
    fn get(&self) -> Option<&I::Item> {
        if self.done {
            None
        } else {
            self.it.get()
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let hint = self.it.size_hint();
        (cmp::min(hint.0, self.n), Some(self.n))
    }
}

impl<I> StreamingIteratorMut for Take<I>
where
    I: StreamingIteratorMut,
{
    #[inline]
    fn get_mut(&mut self) -> Option<&mut I::Item> {
        if self.done {
            None
        } else {
            self.it.get_mut()
        }
    }
}

/// A streaming iterator which only returns initial elements matching a predicate.
#[derive(Debug)]
pub struct TakeWhile<I, F> {
    it: I,
    f: F,
    done: bool,
}

impl<I, F> StreamingIterator for TakeWhile<I, F>
where
    I: StreamingIterator,
    F: FnMut(&I::Item) -> bool,
{
    type Item = I::Item;

    #[inline]
    fn advance(&mut self) {
        if !self.done {
            self.it.advance();
            if let Some(i) = self.it.get() {
                if !(self.f)(i) {
                    self.done = true;
                }
            }
        }
    }

    #[inline]
    fn is_done(&self) -> bool {
        self.done || self.it.is_done()
    }

    #[inline]
    fn get(&self) -> Option<&I::Item> {
        if self.done {
            None
        } else {
            self.it.get()
        }
    }

    #[inline]
    fn next(&mut self) -> Option<&I::Item> {
        if self.done {
            None
        } else {
            match self.it.next() {
                Some(i) => {
                    if (self.f)(i) {
                        Some(i)
                    } else {
                        self.done = true;
                        None
                    }
                }
                None => None,
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let upper = if self.done {
            Some(0)
        } else {
            self.it.size_hint().1
        };
        (0, upper)
    }
}

impl<I, F> StreamingIteratorMut for TakeWhile<I, F>
where
    I: StreamingIteratorMut,
    F: FnMut(&I::Item) -> bool,
{
    #[inline]
    fn get_mut(&mut self) -> Option<&mut I::Item> {
        if self.done {
            None
        } else {
            self.it.get_mut()
        }
    }
}

/// A streaming iterator which returns elements in the opposite order.
pub struct Rev<I>(I);

impl<I> StreamingIterator for Rev<I>
where
    I: DoubleEndedStreamingIterator,
{
    type Item = I::Item;

    #[inline]
    fn advance(&mut self) {
        self.0.advance_back();
    }

    #[inline]
    fn is_done(&self) -> bool {
        self.0.is_done()
    }

    #[inline]
    fn get(&self) -> Option<&I::Item> {
        self.0.get()
    }

    #[inline]
    fn next(&mut self) -> Option<&I::Item> {
        self.0.next_back()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, f: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        self.0.rfold(init, f)
    }
}

impl<I> DoubleEndedStreamingIterator for Rev<I>
where
    I: DoubleEndedStreamingIterator,
{
    #[inline]
    fn advance_back(&mut self) {
        self.0.advance();
    }

    #[inline]
    fn next_back(&mut self) -> Option<&I::Item> {
        self.0.next()
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, f: Fold) -> Acc
    where
        Self: Sized,
        Fold: FnMut(Acc, &Self::Item) -> Acc,
    {
        self.0.fold(init, f)
    }
}

impl<I> StreamingIteratorMut for Rev<I>
where
    I: DoubleEndedStreamingIteratorMut,
{
    #[inline]
    fn get_mut(&mut self) -> Option<&mut I::Item> {
        self.0.get_mut()
    }

    #[inline]
    fn fold_mut<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, &mut Self::Item) -> B,
    {
        self.0.rfold_mut(init, f)
    }
}

impl<I> DoubleEndedStreamingIteratorMut for Rev<I>
where
    I: DoubleEndedStreamingIteratorMut,
{
    #[inline]
    fn rfold_mut<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, &mut Self::Item) -> B,
    {
        self.0.fold_mut(init, f)
    }
}

/// Conversion from [`IntoIterator`] to [`StreamingIterator`].
pub trait IntoStreamingIterator: IntoIterator
where
    Self: Sized,
{
    /// Turns an [`IntoIterator`] into a [`StreamingIterator`].
    ///
    /// Calling this method on an [`IntoIterator`] is equivalent to using [`convert`].
    #[inline]
    fn into_streaming_iter(self) -> Convert<Self::IntoIter> {
        convert(self)
    }

    /// Turns an [`IntoIterator`] of references into a [`StreamingIterator`].
    ///
    /// Calling this method on an [`IntoIterator`] is equivalent to using [`convert_ref`].
    #[inline]
    fn into_streaming_iter_ref<'a, T: ?Sized>(self) -> ConvertRef<'a, Self::IntoIter, T>
    where
        Self: IntoIterator<Item = &'a T>,
    {
        convert_ref(self)
    }

    /// Turns an [`IntoIterator`] of mutable references into a [`StreamingIteratorMut`].
    ///
    /// Calling this method on an [`IntoIterator`] is equivalent to using [`convert_mut`].
    #[inline]
    fn into_streaming_iter_mut<'a, T: ?Sized>(self) -> ConvertMut<'a, Self::IntoIter, T>
    where
        Self: IntoIterator<Item = &'a mut T>,
    {
        convert_mut(self)
    }
}

impl<I> IntoStreamingIterator for I where I: IntoIterator {}

#[cfg(test)]
mod test {
    use core::fmt::Debug;

    #[cfg(feature = "alloc")]
    use alloc::vec::Vec;

    use super::*;

    fn test<I>(mut it: I, expected: &[I::Item])
    where
        I: StreamingIterator,
        I::Item: Sized + PartialEq + Debug,
    {
        for item in expected {
            it.advance();
            assert_eq!(it.get(), Some(item));
            assert_eq!(it.get(), Some(item));
        }
        it.advance();
        assert_eq!(it.get(), None);
        assert_eq!(it.get(), None);
    }

    fn test_back<I>(mut it: I, expected: &[I::Item])
    where
        I: DoubleEndedStreamingIterator,
        I::Item: Sized + PartialEq + Debug,
    {
        for item in expected {
            it.advance_back();
            assert_eq!(it.get(), Some(item));
            assert_eq!(it.get(), Some(item));
        }
        it.advance_back();
        assert_eq!(it.get(), None);
        assert_eq!(it.get(), None);
    }

    fn test_deref<I>(mut it: I, expected: &[I::Item])
    where
        I: Iterator,
        I::Item: Sized + PartialEq + Debug,
    {
        for item in expected {
            assert_eq!(it.next().as_ref(), Some(item));
        }
        assert_eq!(it.next(), None)
    }

    #[test]
    fn all() {
        let items = [0, 1, 2];
        let mut it = convert(items);
        assert!(it.clone().all(|&i| i < 3));
        assert!(!it.all(|&i| i % 2 == 0));
    }

    #[test]
    fn any() {
        let items = [0, 1, 2];
        let mut it = convert(items);
        assert!(it.clone().any(|&i| i > 1));
        assert!(!it.any(|&i| i > 2));
    }

    #[test]
    fn test_chain() {
        let items_a = [0, 1, 2, 3];
        let items_b = [10, 20, 30];
        let expected = [0, 1, 2, 3, 10, 20, 30];

        let it = convert(items_a).chain(convert(items_b));
        test(it, &expected);
    }

    #[test]
    fn test_chain_back() {
        let items_a = [0, 1, 2, 3];
        let items_b = [10, 20, 30];
        let expected = [30, 20, 10, 3, 2, 1, 0];

        let it = convert(items_a).chain(convert(items_b));
        test_back(it, &expected);
    }

    #[test]
    fn test_chain_mixed() {
        let items_a = [0, 1, 2, 3];
        let items_b = [10, 20, 30];

        let mut it = convert(items_a).chain(convert(items_b));

        assert_eq!(it.get(), None);
        it.advance();
        assert_eq!(it.get().copied(), Some(0));
        it.advance_back();
        assert_eq!(it.get().copied(), Some(30));
        it.advance();
        assert_eq!(it.get().copied(), Some(1));
        it.advance_back();
        assert_eq!(it.get().copied(), Some(20));
        it.advance();
        assert_eq!(it.get().copied(), Some(2));
        it.advance_back();
        assert_eq!(it.get().copied(), Some(10));
        it.advance_back();
        assert_eq!(it.get().copied(), Some(3));
    }

    #[test]
    fn cloned() {
        let items = [0, 1];
        let mut it = convert(items).cloned();
        assert_eq!(it.next(), Some(0));
        assert_eq!(it.next(), Some(1));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn copied() {
        let items = [0, 1];
        let mut it = convert(items).copied();
        assert_eq!(it.next(), Some(0));
        assert_eq!(it.next(), Some(1));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn test_convert() {
        let items = [0, 1];
        let it = convert(items);
        test(it, &items);
    }

    #[test]
    fn test_convert_ref() {
        let items = [&0, &1];
        let it = convert_ref(items.iter());
        test(it, &items);
    }

    #[test]
    fn count() {
        let items = [0, 1, 2, 3];
        let it = convert(items);
        assert_eq!(it.count(), 4);
    }

    #[test]
    fn filter() {
        let items = [0, 1, 2, 3];
        let it = convert(items).filter(|x| x % 2 == 0);
        test(it, &[0, 2]);
    }

    #[test]
    fn fuse() {
        struct Flicker(i32);

        impl StreamingIterator for Flicker {
            type Item = i32;

            fn advance(&mut self) {
                self.0 += 1;
            }

            fn get(&self) -> Option<&i32> {
                if self.0 % 4 == 3 {
                    None
                } else {
                    Some(&self.0)
                }
            }
        }

        let mut it = Flicker(0).fuse();
        assert_eq!(it.get(), None);
        it.advance();
        assert_eq!(it.get(), Some(&1));
        assert_eq!(it.get(), Some(&1));
        it.advance();
        assert_eq!(it.get(), Some(&2));
        assert_eq!(it.get(), Some(&2));
        it.advance();
        assert_eq!(it.get(), None);
        assert_eq!(it.get(), None);
        it.advance();
        assert_eq!(it.get(), None);
        assert_eq!(it.get(), None);
    }

    #[test]
    fn inspect() {
        let items = [0, 1, 2, 3];
        let mut idx = 0;
        let mut items_inspected = [-1, -1, -1, -1];

        {
            let it = convert(items).inspect(|&i| {
                items_inspected[idx] = i;
                idx += 1;
            });

            test(it, &items);
        }

        assert_eq!(&items_inspected, &items);
    }

    #[test]
    fn map() {
        let items = [0, 1];
        let it = convert(items.iter().map(|&i| i as usize)).map(|&i| i as i32);
        test(it, &items);
    }

    #[test]
    fn map_deref() {
        let items = [0, 1];
        let it = convert(items.iter().map(|&i| i as usize)).map_deref(|&i| i as i32);
        test_deref(it, &items);
    }

    #[test]
    fn map_deref_mut() {
        let mut items = [1, 2, 3];
        {
            let it = convert_mut(&mut items).map_deref_mut(|i| -core::mem::replace(i, 0));
            test_deref(it, &[-1, -2, -3]);
        }
        assert_eq!(items, [0, 0, 0]);
    }

    #[test]
    fn map_ref() {
        #[derive(Clone)]
        struct Foo(i32);

        let items = [Foo(0), Foo(1)];
        let it = convert(items).map_ref(|f| &f.0);
        test(it, &[0, 1]);
    }

    #[test]
    fn flat_map() {
        let items = [[0, 1, 2], [3, 4, 5]];
        let it = convert(items).flat_map(|&i| convert(i));

        test(it, &[0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn flatten() {
        let mut items = [
            convert_ref([].as_ref()),
            convert_ref([1].as_ref()),
            convert_ref([].as_ref()),
            convert_ref([2, 3].as_ref()),
            convert_ref([].as_ref()),
        ];
        let it = convert_mut(&mut items).flatten();

        test(it, &[1, 2, 3]);
    }

    #[test]
    fn flatten_unsized() {
        type DynI32 = dyn StreamingIterator<Item = i32>;
        let mut items = [
            &mut once(1) as &mut DynI32,
            &mut empty(),
            &mut convert(2..=3),
        ];
        let iters = items.iter_mut().map(|iter| &mut **iter);
        let it = convert_mut(iters).flatten();

        test(it, &[1, 2, 3]);
    }

    #[test]
    fn nth() {
        let items = [0, 1];
        let mut it = convert(items);
        assert_eq!(it.clone().nth(0), Some(&0));
        assert_eq!(it.clone().nth(1), Some(&1));
        assert_eq!(it.nth(2), None);
    }

    #[test]
    fn filter_map() {
        let items = [0u8, 1, 1, 2, 4];
        let it = convert(items).filter_map(|&i| if i % 2 == 0 { Some(i) } else { None });
        test(it, &[0, 2, 4])
    }

    #[test]
    fn filter_map_deref() {
        let items = [0u8, 1, 1, 2, 4];
        let it = convert(items).filter_map_deref(|&i| if i % 2 == 0 { Some(i) } else { None });
        test_deref(it, &[0, 2, 4])
    }

    #[test]
    fn find() {
        let items = [0, 1];
        let mut it = convert(items);
        assert_eq!(it.clone().find(|&x| x % 2 == 1), Some(&1));
        assert_eq!(it.find(|&x| x % 3 == 2), None);
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn owned() {
        let items = [0, 1];
        let it = convert(items).owned();
        assert_eq!(it.collect::<Vec<_>>(), items);
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn owned_str() {
        let s = "The quick brown fox jumps over the lazy dog";
        let words = s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        let it = convert_ref(s.split_whitespace()).owned();
        assert_eq!(it.collect::<Vec<_>>(), words);
    }

    #[test]
    fn position() {
        let items = [0, 1];
        let mut it = convert(items);
        assert_eq!(it.clone().position(|&x| x % 2 == 1), Some(1));
        assert_eq!(it.position(|&x| x % 3 == 2), None);
    }

    #[test]
    fn skip() {
        let items = [0, 1, 2, 3];
        let it = convert(items);
        test(it.clone().skip(0), &[0, 1, 2, 3]);
        test(it.clone().skip(2), &[2, 3]);
        test(it.skip(5), &[]);
    }

    #[test]
    fn skip_while() {
        let items = [0, 1, 2, 3];
        let it = convert(items);
        test(it.clone().skip_while(|&i| i < 0), &[0, 1, 2, 3]);
        test(it.clone().skip_while(|&i| i < 2), &[2, 3]);
        test(it.skip_while(|&i| i < 5), &[]);
    }

    #[test]
    fn take() {
        let items = [0, 1, 2, 3];
        let it = convert(items);
        test(it.clone().take(0), &[]);
        test(it.clone().take(2), &[0, 1]);
        test(it.take(5), &[0, 1, 2, 3]);
    }

    #[test]
    fn take_while() {
        let items = [0, 1, 2, 3];
        let it = convert(items);
        test(it.clone().take_while(|&i| i < 0), &[]);
        test(it.clone().take_while(|&i| i < 2), &[0, 1]);
        test(it.take_while(|&i| i < 5), &[0, 1, 2, 3]);
    }

    fn _is_object_safe(_: &dyn StreamingIterator<Item = ()>) {}

    fn _is_object_safe_mut(_: &dyn StreamingIteratorMut<Item = ()>) {}

    #[test]
    fn empty_iterator() {
        let mut it: Empty<u8> = empty();

        assert_eq!(it.next(), None);
    }

    #[test]
    fn is_done_empty() {
        let mut empty = empty::<u8>();
        empty.advance();
        assert!(empty.is_done());
    }

    #[test]
    fn is_done_map() {
        let items = [1];
        let mut it = convert(items).map_ref::<u16, _>(|_| panic!("only called during get()"));
        it.advance();
        assert!(!it.is_done());
        it.advance();
        assert!(it.is_done());
    }

    #[test]
    fn rev() {
        let items = [0, 1, 2, 3];
        let it = convert(items);
        test(it.rev(), &[3, 2, 1, 0]);
    }

    #[test]
    fn fold() {
        let items = [0, 1, 2, 3];
        let it = convert(items);
        assert_eq!(it.fold(0, |acc, i| acc * 10 + i), 123);
    }

    #[test]
    fn for_each() {
        let items = [0, 1, 2, 3];
        let it = convert(items);
        let mut acc = 0;
        it.for_each(|i| acc = acc * 10 + i);
        assert_eq!(acc, 123);
    }

    #[test]
    fn rfold() {
        let items = [0, 1, 2, 3];
        let it = convert(items);
        assert_eq!(it.rfold(0, |acc, i| acc * 10 + i), 3210);
    }

    #[test]
    fn for_each_rev() {
        let items = [0, 1, 2, 3];
        let it = convert(items);
        let mut acc = 0;
        it.rev().for_each(|i| acc = acc * 10 + i);
        assert_eq!(acc, 3210);
    }

    #[test]
    fn for_each_mut() {
        let mut items = [0, 1, 2, 3];

        convert_mut(&mut items).for_each_mut(|i: &mut i32| *i += 10);
        assert_eq!(items, [10, 11, 12, 13]);

        // Note that filter sees a direct `&T` reference,
        // rather than `&&mut T` like `Iterator` would.
        convert_mut(&mut items)
            .filter(|&i: &i32| i & 1 == 0)
            .for_each_mut(|i: &mut i32| *i /= 2);
        assert_eq!(items, [5, 11, 6, 13]);
    }

    #[test]
    fn into_streaming_iter() {
        let items = [0, 1, 2, 3];
        let iter = items.into_streaming_iter();
        test(iter, &items);
        let iter = (&items).into_streaming_iter_ref();
        test(iter, &items);
        let mut mut_items = items;
        let iter = (&mut mut_items).into_streaming_iter_mut();
        test(iter, &items);
    }
}
