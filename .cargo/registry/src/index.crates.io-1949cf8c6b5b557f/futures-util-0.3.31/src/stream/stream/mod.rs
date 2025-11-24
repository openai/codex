//! Streams
//!
//! This module contains a number of functions for working with `Stream`s,
//! including the `StreamExt` trait which adds methods to `Stream` types.

use crate::future::{assert_future, Either};
use crate::stream::assert_stream;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::pin::Pin;
#[cfg(feature = "sink")]
use futures_core::stream::TryStream;
#[cfg(feature = "alloc")]
use futures_core::stream::{BoxStream, LocalBoxStream};
use futures_core::{
    future::Future,
    stream::{FusedStream, Stream},
    task::{Context, Poll},
};
#[cfg(feature = "sink")]
use futures_sink::Sink;

use crate::fns::{inspect_fn, InspectFn};

mod chain;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::chain::Chain;

mod collect;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::collect::Collect;

mod unzip;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::unzip::Unzip;

mod concat;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::concat::Concat;

mod count;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::count::Count;

mod cycle;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::cycle::Cycle;

mod enumerate;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::enumerate::Enumerate;

mod filter;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::filter::Filter;

mod filter_map;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::filter_map::FilterMap;

mod flatten;

delegate_all!(
    /// Stream for the [`flatten`](StreamExt::flatten) method.
    Flatten<St>(
        flatten::Flatten<St, St::Item>
    ): Debug + Sink + Stream + FusedStream + AccessInner[St, (.)] + New[|x: St| flatten::Flatten::new(x)]
    where St: Stream
);

mod fold;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::fold::Fold;

mod any;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::any::Any;

mod all;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::all::All;

#[cfg(feature = "sink")]
mod forward;

#[cfg(feature = "sink")]
delegate_all!(
    /// Future for the [`forward`](super::StreamExt::forward) method.
    #[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
    Forward<St, Si>(
        forward::Forward<St, Si, St::Ok>
    ): Debug + Future + FusedFuture + New[|x: St, y: Si| forward::Forward::new(x, y)]
    where St: TryStream
);

mod for_each;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::for_each::ForEach;

mod fuse;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::fuse::Fuse;

mod into_future;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::into_future::StreamFuture;

delegate_all!(
    /// Stream for the [`inspect`](StreamExt::inspect) method.
    Inspect<St, F>(
        map::Map<St, InspectFn<F>>
    ): Debug + Sink + Stream + FusedStream + AccessInner[St, (.)] + New[|x: St, f: F| map::Map::new(x, inspect_fn(f))]
);

mod map;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::map::Map;

delegate_all!(
    /// Stream for the [`flat_map`](StreamExt::flat_map) method.
    FlatMap<St, U, F>(
        flatten::Flatten<Map<St, F>, U>
    ): Debug + Sink + Stream + FusedStream + AccessInner[St, (. .)] + New[|x: St, f: F| flatten::Flatten::new(Map::new(x, f))]
);

mod next;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::next::Next;

mod select_next_some;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::select_next_some::SelectNextSome;

mod peek;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::peek::{NextIf, NextIfEq, Peek, PeekMut, Peekable};

mod skip;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::skip::Skip;

mod skip_while;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::skip_while::SkipWhile;

mod take;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::take::Take;

mod take_while;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::take_while::TakeWhile;

mod take_until;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::take_until::TakeUntil;

mod then;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::then::Then;

mod zip;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::zip::Zip;

#[cfg(feature = "alloc")]
mod chunks;
#[cfg(feature = "alloc")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::chunks::Chunks;

#[cfg(feature = "alloc")]
mod ready_chunks;
#[cfg(feature = "alloc")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::ready_chunks::ReadyChunks;

mod scan;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::scan::Scan;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod buffer_unordered;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::buffer_unordered::BufferUnordered;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod buffered;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::buffered::Buffered;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
pub(crate) mod flatten_unordered;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
#[allow(unreachable_pub)]
pub use self::flatten_unordered::FlattenUnordered;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
delegate_all!(
    /// Stream for the [`flat_map_unordered`](StreamExt::flat_map_unordered) method.
    FlatMapUnordered<St, U, F>(
        FlattenUnordered<Map<St, F>>
    ): Debug + Sink + Stream + FusedStream + AccessInner[St, (. .)] + New[|x: St, limit: Option<usize>, f: F| FlattenUnordered::new(Map::new(x, f), limit)]
    where St: Stream, U: Stream, U: Unpin, F: FnMut(St::Item) -> U
);

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod for_each_concurrent;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::for_each_concurrent::ForEachConcurrent;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
#[cfg(feature = "alloc")]
mod split;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
#[cfg(feature = "alloc")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::split::{ReuniteError, SplitSink, SplitStream};

#[cfg(feature = "std")]
mod catch_unwind;
#[cfg(feature = "std")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::catch_unwind::CatchUnwind;

impl<T: ?Sized> StreamExt for T where T: Stream {}

/// An extension trait for `Stream`s that provides a variety of convenient
/// combinator functions.
pub trait StreamExt: Stream {
    /// Creates a future that resolves to the next item in the stream.
    ///
    /// Note that because `next` doesn't take ownership over the stream,
    /// the [`Stream`] type must be [`Unpin`]. If you want to use `next` with a
    /// [`!Unpin`](Unpin) stream, you'll first have to pin the stream. This can
    /// be done by boxing the stream using [`Box::pin`] or
    /// pinning it to the stack using the `pin_mut!` macro from the `pin_utils`
    /// crate.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let mut stream = stream::iter(1..=3);
    ///
    /// assert_eq!(stream.next().await, Some(1));
    /// assert_eq!(stream.next().await, Some(2));
    /// assert_eq!(stream.next().await, Some(3));
    /// assert_eq!(stream.next().await, None);
    /// # });
    /// ```
    fn next(&mut self) -> Next<'_, Self>
    where
        Self: Unpin,
    {
        assert_future::<Option<Self::Item>, _>(Next::new(self))
    }

    /// Converts this stream into a future of `(next_item, tail_of_stream)`.
    /// If the stream terminates, then the next item is [`None`].
    ///
    /// The returned future can be used to compose streams and futures together
    /// by placing everything into the "world of futures".
    ///
    /// Note that because `into_future` moves the stream, the [`Stream`] type
    /// must be [`Unpin`]. If you want to use `into_future` with a
    /// [`!Unpin`](Unpin) stream, you'll first have to pin the stream. This can
    /// be done by boxing the stream using [`Box::pin`] or
    /// pinning it to the stack using the `pin_mut!` macro from the `pin_utils`
    /// crate.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=3);
    ///
    /// let (item, stream) = stream.into_future().await;
    /// assert_eq!(Some(1), item);
    ///
    /// let (item, stream) = stream.into_future().await;
    /// assert_eq!(Some(2), item);
    /// # });
    /// ```
    fn into_future(self) -> StreamFuture<Self>
    where
        Self: Sized + Unpin,
    {
        assert_future::<(Option<Self::Item>, Self), _>(StreamFuture::new(self))
    }

    /// Maps this stream's items to a different type, returning a new stream of
    /// the resulting type.
    ///
    /// The provided closure is executed over all elements of this stream as
    /// they are made available. It is executed inline with calls to
    /// [`poll_next`](Stream::poll_next).
    ///
    /// Note that this function consumes the stream passed into it and returns a
    /// wrapped version of it, similar to the existing `map` methods in the
    /// standard library.
    ///
    /// See [`StreamExt::then`](Self::then) if you want to use a closure that
    /// returns a future instead of a value.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=3);
    /// let stream = stream.map(|x| x + 3);
    ///
    /// assert_eq!(vec![4, 5, 6], stream.collect::<Vec<_>>().await);
    /// # });
    /// ```
    fn map<T, F>(self, f: F) -> Map<Self, F>
    where
        F: FnMut(Self::Item) -> T,
        Self: Sized,
    {
        assert_stream::<T, _>(Map::new(self, f))
    }

    /// Creates a stream which gives the current iteration count as well as
    /// the next value.
    ///
    /// The stream returned yields pairs `(i, val)`, where `i` is the
    /// current index of iteration and `val` is the value returned by the
    /// stream.
    ///
    /// `enumerate()` keeps its count as a [`usize`]. If you want to count by a
    /// different sized integer, the [`zip`](StreamExt::zip) function provides similar
    /// functionality.
    ///
    /// # Overflow Behavior
    ///
    /// The method does no guarding against overflows, so enumerating more than
    /// [`usize::MAX`] elements either produces the wrong result or panics. If
    /// debug assertions are enabled, a panic is guaranteed.
    ///
    /// # Panics
    ///
    /// The returned stream might panic if the to-be-returned index would
    /// overflow a [`usize`].
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(vec!['a', 'b', 'c']);
    ///
    /// let mut stream = stream.enumerate();
    ///
    /// assert_eq!(stream.next().await, Some((0, 'a')));
    /// assert_eq!(stream.next().await, Some((1, 'b')));
    /// assert_eq!(stream.next().await, Some((2, 'c')));
    /// assert_eq!(stream.next().await, None);
    /// # });
    /// ```
    fn enumerate(self) -> Enumerate<Self>
    where
        Self: Sized,
    {
        assert_stream::<(usize, Self::Item), _>(Enumerate::new(self))
    }

    /// Filters the values produced by this stream according to the provided
    /// asynchronous predicate.
    ///
    /// As values of this stream are made available, the provided predicate `f`
    /// will be run against them. If the predicate returns a `Future` which
    /// resolves to `true`, then the stream will yield the value, but if the
    /// predicate returns a `Future` which resolves to `false`, then the value
    /// will be discarded and the next value will be produced.
    ///
    /// Note that this function consumes the stream passed into it and returns a
    /// wrapped version of it, similar to the existing `filter` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=10);
    /// let events = stream.filter(|x| future::ready(x % 2 == 0));
    ///
    /// assert_eq!(vec![2, 4, 6, 8, 10], events.collect::<Vec<_>>().await);
    /// # });
    /// ```
    fn filter<Fut, F>(self, f: F) -> Filter<Self, Fut, F>
    where
        F: FnMut(&Self::Item) -> Fut,
        Fut: Future<Output = bool>,
        Self: Sized,
    {
        assert_stream::<Self::Item, _>(Filter::new(self, f))
    }

    /// Filters the values produced by this stream while simultaneously mapping
    /// them to a different type according to the provided asynchronous closure.
    ///
    /// As values of this stream are made available, the provided function will
    /// be run on them. If the future returned by the predicate `f` resolves to
    /// [`Some(item)`](Some) then the stream will yield the value `item`, but if
    /// it resolves to [`None`] then the next value will be produced.
    ///
    /// Note that this function consumes the stream passed into it and returns a
    /// wrapped version of it, similar to the existing `filter_map` methods in
    /// the standard library.
    ///
    /// # Examples
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=10);
    /// let events = stream.filter_map(|x| async move {
    ///     if x % 2 == 0 { Some(x + 1) } else { None }
    /// });
    ///
    /// assert_eq!(vec![3, 5, 7, 9, 11], events.collect::<Vec<_>>().await);
    /// # });
    /// ```
    fn filter_map<Fut, T, F>(self, f: F) -> FilterMap<Self, Fut, F>
    where
        F: FnMut(Self::Item) -> Fut,
        Fut: Future<Output = Option<T>>,
        Self: Sized,
    {
        assert_stream::<T, _>(FilterMap::new(self, f))
    }

    /// Computes from this stream's items new items of a different type using
    /// an asynchronous closure.
    ///
    /// The provided closure `f` will be called with an `Item` once a value is
    /// ready, it returns a future which will then be run to completion
    /// to produce the next value on this stream.
    ///
    /// Note that this function consumes the stream passed into it and returns a
    /// wrapped version of it.
    ///
    /// See [`StreamExt::map`](Self::map) if you want to use a closure that
    /// returns a value instead of a future.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=3);
    /// let stream = stream.then(|x| async move { x + 3 });
    ///
    /// assert_eq!(vec![4, 5, 6], stream.collect::<Vec<_>>().await);
    /// # });
    /// ```
    fn then<Fut, F>(self, f: F) -> Then<Self, Fut, F>
    where
        F: FnMut(Self::Item) -> Fut,
        Fut: Future,
        Self: Sized,
    {
        assert_stream::<Fut::Output, _>(Then::new(self, f))
    }

    /// Transforms a stream into a collection, returning a
    /// future representing the result of that computation.
    ///
    /// The returned future will be resolved when the stream terminates.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::mpsc;
    /// use futures::stream::StreamExt;
    /// use std::thread;
    ///
    /// let (tx, rx) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     for i in 1..=5 {
    ///         tx.unbounded_send(i).unwrap();
    ///     }
    /// });
    ///
    /// let output = rx.collect::<Vec<i32>>().await;
    /// assert_eq!(output, vec![1, 2, 3, 4, 5]);
    /// # });
    /// ```
    fn collect<C: Default + Extend<Self::Item>>(self) -> Collect<Self, C>
    where
        Self: Sized,
    {
        assert_future::<C, _>(Collect::new(self))
    }

    /// Converts a stream of pairs into a future, which
    /// resolves to pair of containers.
    ///
    /// `unzip()` produces a future, which resolves to two
    /// collections: one from the left elements of the pairs,
    /// and one from the right elements.
    ///
    /// The returned future will be resolved when the stream terminates.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::mpsc;
    /// use futures::stream::StreamExt;
    /// use std::thread;
    ///
    /// let (tx, rx) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     tx.unbounded_send((1, 2)).unwrap();
    ///     tx.unbounded_send((3, 4)).unwrap();
    ///     tx.unbounded_send((5, 6)).unwrap();
    /// });
    ///
    /// let (o1, o2): (Vec<_>, Vec<_>) = rx.unzip().await;
    /// assert_eq!(o1, vec![1, 3, 5]);
    /// assert_eq!(o2, vec![2, 4, 6]);
    /// # });
    /// ```
    fn unzip<A, B, FromA, FromB>(self) -> Unzip<Self, FromA, FromB>
    where
        FromA: Default + Extend<A>,
        FromB: Default + Extend<B>,
        Self: Sized + Stream<Item = (A, B)>,
    {
        assert_future::<(FromA, FromB), _>(Unzip::new(self))
    }

    /// Concatenate all items of a stream into a single extendable
    /// destination, returning a future representing the end result.
    ///
    /// This combinator will extend the first item with the contents
    /// of all the subsequent results of the stream. If the stream is
    /// empty, the default value will be returned.
    ///
    /// Works with all collections that implement the
    /// [`Extend`](std::iter::Extend) trait.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::mpsc;
    /// use futures::stream::StreamExt;
    /// use std::thread;
    ///
    /// let (tx, rx) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     for i in (0..3).rev() {
    ///         let n = i * 3;
    ///         tx.unbounded_send(vec![n + 1, n + 2, n + 3]).unwrap();
    ///     }
    /// });
    ///
    /// let result = rx.concat().await;
    ///
    /// assert_eq!(result, vec![7, 8, 9, 4, 5, 6, 1, 2, 3]);
    /// # });
    /// ```
    fn concat(self) -> Concat<Self>
    where
        Self: Sized,
        Self::Item: Extend<<<Self as Stream>::Item as IntoIterator>::Item> + IntoIterator + Default,
    {
        assert_future::<Self::Item, _>(Concat::new(self))
    }

    /// Drives the stream to completion, counting the number of items.
    ///
    /// # Overflow Behavior
    ///
    /// The method does no guarding against overflows, so counting elements of a
    /// stream with more than [`usize::MAX`] elements either produces the wrong
    /// result or panics. If debug assertions are enabled, a panic is guaranteed.
    ///
    /// # Panics
    ///
    /// This function might panic if the iterator has more than [`usize::MAX`]
    /// elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=10);
    /// let count = stream.count().await;
    ///
    /// assert_eq!(count, 10);
    /// # });
    /// ```
    fn count(self) -> Count<Self>
    where
        Self: Sized,
    {
        assert_future::<usize, _>(Count::new(self))
    }

    /// Repeats a stream endlessly.
    ///
    /// The stream never terminates. Note that you likely want to avoid
    /// usage of `collect` or such on the returned stream as it will exhaust
    /// available memory as it tries to just fill up all RAM.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    /// let a = [1, 2, 3];
    /// let mut s = stream::iter(a.iter()).cycle();
    ///
    /// assert_eq!(s.next().await, Some(&1));
    /// assert_eq!(s.next().await, Some(&2));
    /// assert_eq!(s.next().await, Some(&3));
    /// assert_eq!(s.next().await, Some(&1));
    /// assert_eq!(s.next().await, Some(&2));
    /// assert_eq!(s.next().await, Some(&3));
    /// assert_eq!(s.next().await, Some(&1));
    /// # });
    /// ```
    fn cycle(self) -> Cycle<Self>
    where
        Self: Sized + Clone,
    {
        assert_stream::<Self::Item, _>(Cycle::new(self))
    }

    /// Execute an accumulating asynchronous computation over a stream,
    /// collecting all the values into one final result.
    ///
    /// This combinator will accumulate all values returned by this stream
    /// according to the closure provided. The initial state is also provided to
    /// this method and then is returned again by each execution of the closure.
    /// Once the entire stream has been exhausted the returned future will
    /// resolve to this value.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let number_stream = stream::iter(0..6);
    /// let sum = number_stream.fold(0, |acc, x| async move { acc + x });
    /// assert_eq!(sum.await, 15);
    /// # });
    /// ```
    fn fold<T, Fut, F>(self, init: T, f: F) -> Fold<Self, Fut, T, F>
    where
        F: FnMut(T, Self::Item) -> Fut,
        Fut: Future<Output = T>,
        Self: Sized,
    {
        assert_future::<T, _>(Fold::new(self, f, init))
    }

    /// Execute predicate over asynchronous stream, and return `true` if any element in stream satisfied a predicate.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let number_stream = stream::iter(0..10);
    /// let contain_three = number_stream.any(|i| async move { i == 3 });
    /// assert_eq!(contain_three.await, true);
    /// # });
    /// ```
    fn any<Fut, F>(self, f: F) -> Any<Self, Fut, F>
    where
        F: FnMut(Self::Item) -> Fut,
        Fut: Future<Output = bool>,
        Self: Sized,
    {
        assert_future::<bool, _>(Any::new(self, f))
    }

    /// Execute predicate over asynchronous stream, and return `true` if all element in stream satisfied a predicate.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let number_stream = stream::iter(0..10);
    /// let less_then_twenty = number_stream.all(|i| async move { i < 20 });
    /// assert_eq!(less_then_twenty.await, true);
    /// # });
    /// ```
    fn all<Fut, F>(self, f: F) -> All<Self, Fut, F>
    where
        F: FnMut(Self::Item) -> Fut,
        Fut: Future<Output = bool>,
        Self: Sized,
    {
        assert_future::<bool, _>(All::new(self, f))
    }

    /// Flattens a stream of streams into just one continuous stream.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::mpsc;
    /// use futures::stream::StreamExt;
    /// use std::thread;
    ///
    /// let (tx1, rx1) = mpsc::unbounded();
    /// let (tx2, rx2) = mpsc::unbounded();
    /// let (tx3, rx3) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     tx1.unbounded_send(1).unwrap();
    ///     tx1.unbounded_send(2).unwrap();
    /// });
    /// thread::spawn(move || {
    ///     tx2.unbounded_send(3).unwrap();
    ///     tx2.unbounded_send(4).unwrap();
    /// });
    /// thread::spawn(move || {
    ///     tx3.unbounded_send(rx1).unwrap();
    ///     tx3.unbounded_send(rx2).unwrap();
    /// });
    ///
    /// let output = rx3.flatten().collect::<Vec<i32>>().await;
    /// assert_eq!(output, vec![1, 2, 3, 4]);
    /// # });
    /// ```
    fn flatten(self) -> Flatten<Self>
    where
        Self::Item: Stream,
        Self: Sized,
    {
        assert_stream::<<Self::Item as Stream>::Item, _>(Flatten::new(self))
    }

    /// Flattens a stream of streams into just one continuous stream. Polls
    /// inner streams produced by the base stream concurrently.
    ///
    /// The only argument is an optional limit on the number of concurrently
    /// polled streams. If this limit is not `None`, no more than `limit` streams
    /// will be polled at the same time. The `limit` argument is of type
    /// `Into<Option<usize>>`, and so can be provided as either `None`,
    /// `Some(10)`, or just `10`. Note: a limit of zero is interpreted as
    /// no limit at all, and will have the same result as passing in `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::mpsc;
    /// use futures::stream::StreamExt;
    /// use std::thread;
    ///
    /// let (tx1, rx1) = mpsc::unbounded();
    /// let (tx2, rx2) = mpsc::unbounded();
    /// let (tx3, rx3) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     tx1.unbounded_send(1).unwrap();
    ///     tx1.unbounded_send(2).unwrap();
    /// });
    /// thread::spawn(move || {
    ///     tx2.unbounded_send(3).unwrap();
    ///     tx2.unbounded_send(4).unwrap();
    /// });
    /// thread::spawn(move || {
    ///     tx3.unbounded_send(rx1).unwrap();
    ///     tx3.unbounded_send(rx2).unwrap();
    /// });
    ///
    /// let mut output = rx3.flatten_unordered(None).collect::<Vec<i32>>().await;
    /// output.sort();
    ///
    /// assert_eq!(output, vec![1, 2, 3, 4]);
    /// # });
    /// ```
    #[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
    #[cfg(feature = "alloc")]
    fn flatten_unordered(self, limit: impl Into<Option<usize>>) -> FlattenUnordered<Self>
    where
        Self::Item: Stream + Unpin,
        Self: Sized,
    {
        assert_stream::<<Self::Item as Stream>::Item, _>(FlattenUnordered::new(self, limit.into()))
    }

    /// Maps a stream like [`StreamExt::map`] but flattens nested `Stream`s.
    ///
    /// [`StreamExt::map`] is very useful, but if it produces a `Stream` instead,
    /// you would have to chain combinators like `.map(f).flatten()` while this
    /// combinator provides ability to write `.flat_map(f)` instead of chaining.
    ///
    /// The provided closure which produces inner streams is executed over all elements
    /// of stream as last inner stream is terminated and next stream item is available.
    ///
    /// Note that this function consumes the stream passed into it and returns a
    /// wrapped version of it, similar to the existing `flat_map` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=3);
    /// let stream = stream.flat_map(|x| stream::iter(vec![x + 3; x]));
    ///
    /// assert_eq!(vec![4, 5, 5, 6, 6, 6], stream.collect::<Vec<_>>().await);
    /// # });
    /// ```
    fn flat_map<U, F>(self, f: F) -> FlatMap<Self, U, F>
    where
        F: FnMut(Self::Item) -> U,
        U: Stream,
        Self: Sized,
    {
        assert_stream::<U::Item, _>(FlatMap::new(self, f))
    }

    /// Maps a stream like [`StreamExt::map`] but flattens nested `Stream`s
    /// and polls them concurrently, yielding items in any order, as they made
    /// available.
    ///
    /// [`StreamExt::map`] is very useful, but if it produces `Stream`s
    /// instead, and you need to poll all of them concurrently, you would
    /// have to use something like `for_each_concurrent` and merge values
    /// by hand. This combinator provides ability to collect all values
    /// from concurrently polled streams into one stream.
    ///
    /// The first argument is an optional limit on the number of concurrently
    /// polled streams. If this limit is not `None`, no more than `limit` streams
    /// will be polled at the same time. The `limit` argument is of type
    /// `Into<Option<usize>>`, and so can be provided as either `None`,
    /// `Some(10)`, or just `10`. Note: a limit of zero is interpreted as
    /// no limit at all, and will have the same result as passing in `None`.
    ///
    /// The provided closure which produces inner streams is executed over
    /// all elements of stream as next stream item is available and limit
    /// of concurrently processed streams isn't exceeded.
    ///
    /// Note that this function consumes the stream passed into it and
    /// returns a wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..5);
    /// let stream = stream.flat_map_unordered(1, |x| stream::iter(vec![x; x]));
    /// let mut values = stream.collect::<Vec<_>>().await;
    /// values.sort();
    ///
    /// assert_eq!(vec![1usize, 2, 2, 3, 3, 3, 4, 4, 4, 4], values);
    /// # });
    /// ```
    #[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
    #[cfg(feature = "alloc")]
    fn flat_map_unordered<U, F>(
        self,
        limit: impl Into<Option<usize>>,
        f: F,
    ) -> FlatMapUnordered<Self, U, F>
    where
        U: Stream + Unpin,
        F: FnMut(Self::Item) -> U,
        Self: Sized,
    {
        assert_stream::<U::Item, _>(FlatMapUnordered::new(self, limit.into(), f))
    }

    /// Combinator similar to [`StreamExt::fold`] that holds internal state
    /// and produces a new stream.
    ///
    /// Accepts initial state and closure which will be applied to each element
    /// of the stream until provided closure returns `None`. Once `None` is
    /// returned, stream will be terminated.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=10);
    ///
    /// let stream = stream.scan(0, |state, x| {
    ///     *state += x;
    ///     future::ready(if *state < 10 { Some(x) } else { None })
    /// });
    ///
    /// assert_eq!(vec![1, 2, 3], stream.collect::<Vec<_>>().await);
    /// # });
    /// ```
    fn scan<S, B, Fut, F>(self, initial_state: S, f: F) -> Scan<Self, S, Fut, F>
    where
        F: FnMut(&mut S, Self::Item) -> Fut,
        Fut: Future<Output = Option<B>>,
        Self: Sized,
    {
        assert_stream::<B, _>(Scan::new(self, initial_state, f))
    }

    /// Skip elements on this stream while the provided asynchronous predicate
    /// resolves to `true`.
    ///
    /// This function, like `Iterator::skip_while`, will skip elements on the
    /// stream until the predicate `f` resolves to `false`. Once one element
    /// returns `false`, all future elements will be returned from the underlying
    /// stream.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=10);
    ///
    /// let stream = stream.skip_while(|x| future::ready(*x <= 5));
    ///
    /// assert_eq!(vec![6, 7, 8, 9, 10], stream.collect::<Vec<_>>().await);
    /// # });
    /// ```
    fn skip_while<Fut, F>(self, f: F) -> SkipWhile<Self, Fut, F>
    where
        F: FnMut(&Self::Item) -> Fut,
        Fut: Future<Output = bool>,
        Self: Sized,
    {
        assert_stream::<Self::Item, _>(SkipWhile::new(self, f))
    }

    /// Take elements from this stream while the provided asynchronous predicate
    /// resolves to `true`.
    ///
    /// This function, like `Iterator::take_while`, will take elements from the
    /// stream until the predicate `f` resolves to `false`. Once one element
    /// returns `false`, it will always return that the stream is done.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=10);
    ///
    /// let stream = stream.take_while(|x| future::ready(*x <= 5));
    ///
    /// assert_eq!(vec![1, 2, 3, 4, 5], stream.collect::<Vec<_>>().await);
    /// # });
    /// ```
    fn take_while<Fut, F>(self, f: F) -> TakeWhile<Self, Fut, F>
    where
        F: FnMut(&Self::Item) -> Fut,
        Fut: Future<Output = bool>,
        Self: Sized,
    {
        assert_stream::<Self::Item, _>(TakeWhile::new(self, f))
    }

    /// Take elements from this stream until the provided future resolves.
    ///
    /// This function will take elements from the stream until the provided
    /// stopping future `fut` resolves. Once the `fut` future becomes ready,
    /// this stream combinator will always return that the stream is done.
    ///
    /// The stopping future may return any type. Once the stream is stopped
    /// the result of the stopping future may be accessed with `TakeUntil::take_result()`.
    /// The stream may also be resumed with `TakeUntil::take_future()`.
    /// See the documentation of [`TakeUntil`] for more information.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    /// use futures::task::Poll;
    ///
    /// let stream = stream::iter(1..=10);
    ///
    /// let mut i = 0;
    /// let stop_fut = future::poll_fn(|_cx| {
    ///     i += 1;
    ///     if i <= 5 {
    ///         Poll::Pending
    ///     } else {
    ///         Poll::Ready(())
    ///     }
    /// });
    ///
    /// let stream = stream.take_until(stop_fut);
    ///
    /// assert_eq!(vec![1, 2, 3, 4, 5], stream.collect::<Vec<_>>().await);
    /// # });
    /// ```
    fn take_until<Fut>(self, fut: Fut) -> TakeUntil<Self, Fut>
    where
        Fut: Future,
        Self: Sized,
    {
        assert_stream::<Self::Item, _>(TakeUntil::new(self, fut))
    }

    /// Runs this stream to completion, executing the provided asynchronous
    /// closure for each element on the stream.
    ///
    /// The closure provided will be called for each item this stream produces,
    /// yielding a future. That future will then be executed to completion
    /// before moving on to the next item.
    ///
    /// The returned value is a `Future` where the `Output` type is `()`; it is
    /// executed entirely for its side effects.
    ///
    /// To process each item in the stream and produce another stream instead
    /// of a single future, use `then` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let mut x = 0;
    ///
    /// {
    ///     let fut = stream::repeat(1).take(3).for_each(|item| {
    ///         x += item;
    ///         future::ready(())
    ///     });
    ///     fut.await;
    /// }
    ///
    /// assert_eq!(x, 3);
    /// # });
    /// ```
    fn for_each<Fut, F>(self, f: F) -> ForEach<Self, Fut, F>
    where
        F: FnMut(Self::Item) -> Fut,
        Fut: Future<Output = ()>,
        Self: Sized,
    {
        assert_future::<(), _>(ForEach::new(self, f))
    }

    /// Runs this stream to completion, executing the provided asynchronous
    /// closure for each element on the stream concurrently as elements become
    /// available.
    ///
    /// This is similar to [`StreamExt::for_each`], but the futures
    /// produced by the closure are run concurrently (but not in parallel--
    /// this combinator does not introduce any threads).
    ///
    /// The closure provided will be called for each item this stream produces,
    /// yielding a future. That future will then be executed to completion
    /// concurrently with the other futures produced by the closure.
    ///
    /// The first argument is an optional limit on the number of concurrent
    /// futures. If this limit is not `None`, no more than `limit` futures
    /// will be run concurrently. The `limit` argument is of type
    /// `Into<Option<usize>>`, and so can be provided as either `None`,
    /// `Some(10)`, or just `10`. Note: a limit of zero is interpreted as
    /// no limit at all, and will have the same result as passing in `None`.
    ///
    /// This method is only available when the `std` or `alloc` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::oneshot;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let (tx1, rx1) = oneshot::channel();
    /// let (tx2, rx2) = oneshot::channel();
    /// let (tx3, rx3) = oneshot::channel();
    ///
    /// let fut = stream::iter(vec![rx1, rx2, rx3]).for_each_concurrent(
    ///     /* limit */ 2,
    ///     |rx| async move {
    ///         rx.await.unwrap();
    ///     }
    /// );
    /// tx1.send(()).unwrap();
    /// tx2.send(()).unwrap();
    /// tx3.send(()).unwrap();
    /// fut.await;
    /// # })
    /// ```
    #[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
    #[cfg(feature = "alloc")]
    fn for_each_concurrent<Fut, F>(
        self,
        limit: impl Into<Option<usize>>,
        f: F,
    ) -> ForEachConcurrent<Self, Fut, F>
    where
        F: FnMut(Self::Item) -> Fut,
        Fut: Future<Output = ()>,
        Self: Sized,
    {
        assert_future::<(), _>(ForEachConcurrent::new(self, limit.into(), f))
    }

    /// Creates a new stream of at most `n` items of the underlying stream.
    ///
    /// Once `n` items have been yielded from this stream then it will always
    /// return that the stream is done.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=10).take(3);
    ///
    /// assert_eq!(vec![1, 2, 3], stream.collect::<Vec<_>>().await);
    /// # });
    /// ```
    fn take(self, n: usize) -> Take<Self>
    where
        Self: Sized,
    {
        assert_stream::<Self::Item, _>(Take::new(self, n))
    }

    /// Creates a new stream which skips `n` items of the underlying stream.
    ///
    /// Once `n` items have been skipped from this stream then it will always
    /// return the remaining items on this stream.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(1..=10).skip(5);
    ///
    /// assert_eq!(vec![6, 7, 8, 9, 10], stream.collect::<Vec<_>>().await);
    /// # });
    /// ```
    fn skip(self, n: usize) -> Skip<Self>
    where
        Self: Sized,
    {
        assert_stream::<Self::Item, _>(Skip::new(self, n))
    }

    /// Fuse a stream such that [`poll_next`](Stream::poll_next) will never
    /// again be called once it has finished. This method can be used to turn
    /// any `Stream` into a `FusedStream`.
    ///
    /// Normally, once a stream has returned [`None`] from
    /// [`poll_next`](Stream::poll_next) any further calls could exhibit bad
    /// behavior such as block forever, panic, never return, etc. If it is known
    /// that [`poll_next`](Stream::poll_next) may be called after stream
    /// has already finished, then this method can be used to ensure that it has
    /// defined semantics.
    ///
    /// The [`poll_next`](Stream::poll_next) method of a `fuse`d stream
    /// is guaranteed to return [`None`] after the underlying stream has
    /// finished.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::executor::block_on_stream;
    /// use futures::stream::{self, StreamExt};
    /// use futures::task::Poll;
    ///
    /// let mut x = 0;
    /// let stream = stream::poll_fn(|_| {
    ///     x += 1;
    ///     match x {
    ///         0..=2 => Poll::Ready(Some(x)),
    ///         3 => Poll::Ready(None),
    ///         _ => panic!("should not happen")
    ///     }
    /// }).fuse();
    ///
    /// let mut iter = block_on_stream(stream);
    /// assert_eq!(Some(1), iter.next());
    /// assert_eq!(Some(2), iter.next());
    /// assert_eq!(None, iter.next());
    /// assert_eq!(None, iter.next());
    /// // ...
    /// ```
    fn fuse(self) -> Fuse<Self>
    where
        Self: Sized,
    {
        assert_stream::<Self::Item, _>(Fuse::new(self))
    }

    /// Borrows a stream, rather than consuming it.
    ///
    /// This is useful to allow applying stream adaptors while still retaining
    /// ownership of the original stream.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let mut stream = stream::iter(1..5);
    ///
    /// let sum = stream.by_ref()
    ///                 .take(2)
    ///                 .fold(0, |a, b| async move { a + b })
    ///                 .await;
    /// assert_eq!(sum, 3);
    ///
    /// // You can use the stream again
    /// let sum = stream.take(2)
    ///                 .fold(0, |a, b| async move { a + b })
    ///                 .await;
    /// assert_eq!(sum, 7);
    /// # });
    /// ```
    fn by_ref(&mut self) -> &mut Self {
        self
    }

    /// Catches unwinding panics while polling the stream.
    ///
    /// Caught panic (if any) will be the last element of the resulting stream.
    ///
    /// In general, panics within a stream can propagate all the way out to the
    /// task level. This combinator makes it possible to halt unwinding within
    /// the stream itself. It's most commonly used within task executors. This
    /// method should not be used for error handling.
    ///
    /// Note that this method requires the `UnwindSafe` bound from the standard
    /// library. This isn't always applied automatically, and the standard
    /// library provides an `AssertUnwindSafe` wrapper type to apply it
    /// after-the fact. To assist using this method, the [`Stream`] trait is
    /// also implemented for `AssertUnwindSafe<St>` where `St` implements
    /// [`Stream`].
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(vec![Some(10), None, Some(11)]);
    /// // Panic on second element
    /// let stream_panicking = stream.map(|o| o.unwrap());
    /// // Collect all the results
    /// let stream = stream_panicking.catch_unwind();
    ///
    /// let results: Vec<Result<i32, _>> = stream.collect().await;
    /// match results[0] {
    ///     Ok(10) => {}
    ///     _ => panic!("unexpected result!"),
    /// }
    /// assert!(results[1].is_err());
    /// assert_eq!(results.len(), 2);
    /// # });
    /// ```
    #[cfg(feature = "std")]
    fn catch_unwind(self) -> CatchUnwind<Self>
    where
        Self: Sized + std::panic::UnwindSafe,
    {
        assert_stream(CatchUnwind::new(self))
    }

    /// Wrap the stream in a Box, pinning it.
    ///
    /// This method is only available when the `std` or `alloc` feature of this
    /// library is activated, and it is activated by default.
    #[cfg(feature = "alloc")]
    fn boxed<'a>(self) -> BoxStream<'a, Self::Item>
    where
        Self: Sized + Send + 'a,
    {
        assert_stream::<Self::Item, _>(Box::pin(self))
    }

    /// Wrap the stream in a Box, pinning it.
    ///
    /// Similar to `boxed`, but without the `Send` requirement.
    ///
    /// This method is only available when the `std` or `alloc` feature of this
    /// library is activated, and it is activated by default.
    #[cfg(feature = "alloc")]
    fn boxed_local<'a>(self) -> LocalBoxStream<'a, Self::Item>
    where
        Self: Sized + 'a,
    {
        assert_stream::<Self::Item, _>(Box::pin(self))
    }

    /// An adaptor for creating a buffered list of pending futures.
    ///
    /// If this stream's item can be converted into a future, then this adaptor
    /// will buffer up to at most `n` futures and then return the outputs in the
    /// same order as the underlying stream. No more than `n` futures will be
    /// buffered at any point in time, and less than `n` may also be buffered
    /// depending on the state of each future.
    ///
    /// The returned stream will be a stream of each future's output.
    ///
    /// This method is only available when the `std` or `alloc` feature of this
    /// library is activated, and it is activated by default.
    #[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
    #[cfg(feature = "alloc")]
    fn buffered(self, n: usize) -> Buffered<Self>
    where
        Self::Item: Future,
        Self: Sized,
    {
        assert_stream::<<Self::Item as Future>::Output, _>(Buffered::new(self, n))
    }

    /// An adaptor for creating a buffered list of pending futures (unordered).
    ///
    /// If this stream's item can be converted into a future, then this adaptor
    /// will buffer up to `n` futures and then return the outputs in the order
    /// in which they complete. No more than `n` futures will be buffered at
    /// any point in time, and less than `n` may also be buffered depending on
    /// the state of each future.
    ///
    /// The returned stream will be a stream of each future's output.
    ///
    /// This method is only available when the `std` or `alloc` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::oneshot;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let (send_one, recv_one) = oneshot::channel();
    /// let (send_two, recv_two) = oneshot::channel();
    ///
    /// let stream_of_futures = stream::iter(vec![recv_one, recv_two]);
    /// let mut buffered = stream_of_futures.buffer_unordered(10);
    ///
    /// send_two.send(2i32)?;
    /// assert_eq!(buffered.next().await, Some(Ok(2i32)));
    ///
    /// send_one.send(1i32)?;
    /// assert_eq!(buffered.next().await, Some(Ok(1i32)));
    ///
    /// assert_eq!(buffered.next().await, None);
    /// # Ok::<(), i32>(()) }).unwrap();
    /// ```
    #[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
    #[cfg(feature = "alloc")]
    fn buffer_unordered(self, n: usize) -> BufferUnordered<Self>
    where
        Self::Item: Future,
        Self: Sized,
    {
        assert_stream::<<Self::Item as Future>::Output, _>(BufferUnordered::new(self, n))
    }

    /// An adapter for zipping two streams together.
    ///
    /// The zipped stream waits for both streams to produce an item, and then
    /// returns that pair. If either stream ends then the zipped stream will
    /// also end.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream1 = stream::iter(1..=3);
    /// let stream2 = stream::iter(5..=10);
    ///
    /// let vec = stream1.zip(stream2)
    ///                  .collect::<Vec<_>>()
    ///                  .await;
    /// assert_eq!(vec![(1, 5), (2, 6), (3, 7)], vec);
    /// # });
    /// ```
    ///
    fn zip<St>(self, other: St) -> Zip<Self, St>
    where
        St: Stream,
        Self: Sized,
    {
        assert_stream::<(Self::Item, St::Item), _>(Zip::new(self, other))
    }

    /// Adapter for chaining two streams.
    ///
    /// The resulting stream emits elements from the first stream, and when
    /// first stream reaches the end, emits the elements from the second stream.
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream1 = stream::iter(vec![Ok(10), Err(false)]);
    /// let stream2 = stream::iter(vec![Err(true), Ok(20)]);
    ///
    /// let stream = stream1.chain(stream2);
    ///
    /// let result: Vec<_> = stream.collect().await;
    /// assert_eq!(result, vec![
    ///     Ok(10),
    ///     Err(false),
    ///     Err(true),
    ///     Ok(20),
    /// ]);
    /// # });
    /// ```
    fn chain<St>(self, other: St) -> Chain<Self, St>
    where
        St: Stream<Item = Self::Item>,
        Self: Sized,
    {
        assert_stream::<Self::Item, _>(Chain::new(self, other))
    }

    /// Creates a new stream which exposes a `peek` method.
    ///
    /// Calling `peek` returns a reference to the next item in the stream.
    fn peekable(self) -> Peekable<Self>
    where
        Self: Sized,
    {
        assert_stream::<Self::Item, _>(Peekable::new(self))
    }

    /// An adaptor for chunking up items of the stream inside a vector.
    ///
    /// This combinator will attempt to pull items from this stream and buffer
    /// them into a local vector. At most `capacity` items will get buffered
    /// before they're yielded from the returned stream.
    ///
    /// Note that the vectors returned from this iterator may not always have
    /// `capacity` elements. If the underlying stream ended and only a partial
    /// vector was created, it'll be returned. Additionally if an error happens
    /// from the underlying stream then the currently buffered items will be
    /// yielded.
    ///
    /// This method is only available when the `std` or `alloc` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Panics
    ///
    /// This method will panic if `capacity` is zero.
    #[cfg(feature = "alloc")]
    fn chunks(self, capacity: usize) -> Chunks<Self>
    where
        Self: Sized,
    {
        assert_stream::<Vec<Self::Item>, _>(Chunks::new(self, capacity))
    }

    /// An adaptor for chunking up ready items of the stream inside a vector.
    ///
    /// This combinator will attempt to pull ready items from this stream and
    /// buffer them into a local vector. At most `capacity` items will get
    /// buffered before they're yielded from the returned stream. If underlying
    /// stream returns `Poll::Pending`, and collected chunk is not empty, it will
    /// be immediately returned.
    ///
    /// If the underlying stream ended and only a partial vector was created,
    /// it will be returned.
    ///
    /// This method is only available when the `std` or `alloc` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Panics
    ///
    /// This method will panic if `capacity` is zero.
    #[cfg(feature = "alloc")]
    fn ready_chunks(self, capacity: usize) -> ReadyChunks<Self>
    where
        Self: Sized,
    {
        assert_stream::<Vec<Self::Item>, _>(ReadyChunks::new(self, capacity))
    }

    /// A future that completes after the given stream has been fully processed
    /// into the sink and the sink has been flushed and closed.
    ///
    /// This future will drive the stream to keep producing items until it is
    /// exhausted, sending each item to the sink. It will complete once the
    /// stream is exhausted, the sink has received and flushed all items, and
    /// the sink is closed. Note that neither the original stream nor provided
    /// sink will be output by this future. Pass the sink by `Pin<&mut S>`
    /// (for example, via `forward(&mut sink)` inside an `async` fn/block) in
    /// order to preserve access to the `Sink`. If the stream produces an error,
    /// that error will be returned by this future without flushing/closing the sink.
    #[cfg(feature = "sink")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
    fn forward<S>(self, sink: S) -> Forward<Self, S>
    where
        S: Sink<Self::Ok, Error = Self::Error>,
        Self: TryStream + Sized,
        // Self: TryStream + Sized + Stream<Item = Result<<Self as TryStream>::Ok, <Self as TryStream>::Error>>,
    {
        // TODO: type mismatch resolving `<Self as futures_core::Stream>::Item == std::result::Result<<Self as futures_core::TryStream>::Ok, <Self as futures_core::TryStream>::Error>`
        // assert_future::<Result<(), Self::Error>, _>(Forward::new(self, sink))
        Forward::new(self, sink)
    }

    /// Splits this `Stream + Sink` object into separate `Sink` and `Stream`
    /// objects.
    ///
    /// This can be useful when you want to split ownership between tasks, or
    /// allow direct interaction between the two objects (e.g. via
    /// `Sink::send_all`).
    ///
    /// This method is only available when the `std` or `alloc` feature of this
    /// library is activated, and it is activated by default.
    #[cfg(feature = "sink")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
    #[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
    #[cfg(feature = "alloc")]
    fn split<Item>(self) -> (SplitSink<Self, Item>, SplitStream<Self>)
    where
        Self: Sink<Item> + Sized,
    {
        let (sink, stream) = split::split(self);
        (
            crate::sink::assert_sink::<Item, Self::Error, _>(sink),
            assert_stream::<Self::Item, _>(stream),
        )
    }

    /// Do something with each item of this stream, afterwards passing it on.
    ///
    /// This is similar to the `Iterator::inspect` method in the standard
    /// library where it allows easily inspecting each value as it passes
    /// through the stream, for example to debug what's going on.
    fn inspect<F>(self, f: F) -> Inspect<Self, F>
    where
        F: FnMut(&Self::Item),
        Self: Sized,
    {
        assert_stream::<Self::Item, _>(Inspect::new(self, f))
    }

    /// Wrap this stream in an `Either` stream, making it the left-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `right_stream` method to write `if`
    /// statements that evaluate to different streams in different branches.
    fn left_stream<B>(self) -> Either<Self, B>
    where
        B: Stream<Item = Self::Item>,
        Self: Sized,
    {
        assert_stream::<Self::Item, _>(Either::Left(self))
    }

    /// Wrap this stream in an `Either` stream, making it the right-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `left_stream` method to write `if`
    /// statements that evaluate to different streams in different branches.
    fn right_stream<B>(self) -> Either<B, Self>
    where
        B: Stream<Item = Self::Item>,
        Self: Sized,
    {
        assert_stream::<Self::Item, _>(Either::Right(self))
    }

    /// A convenience method for calling [`Stream::poll_next`] on [`Unpin`]
    /// stream types.
    fn poll_next_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>
    where
        Self: Unpin,
    {
        Pin::new(self).poll_next(cx)
    }

    /// Returns a [`Future`] that resolves when the next item in this stream is
    /// ready.
    ///
    /// This is similar to the [`next`][StreamExt::next] method, but it won't
    /// resolve to [`None`] if used on an empty [`Stream`]. Instead, the
    /// returned future type will return `true` from
    /// [`FusedFuture::is_terminated`][] when the [`Stream`] is empty, allowing
    /// [`select_next_some`][StreamExt::select_next_some] to be easily used with
    /// the [`select!`] macro.
    ///
    /// If the future is polled after this [`Stream`] is empty it will panic.
    /// Using the future with a [`FusedFuture`][]-aware primitive like the
    /// [`select!`] macro will prevent this.
    ///
    /// [`FusedFuture`]: futures_core::future::FusedFuture
    /// [`FusedFuture::is_terminated`]: futures_core::future::FusedFuture::is_terminated
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::{future, select};
    /// use futures::stream::{StreamExt, FuturesUnordered};
    ///
    /// let mut fut = future::ready(1);
    /// let mut async_tasks = FuturesUnordered::new();
    /// let mut total = 0;
    /// loop {
    ///     select! {
    ///         num = fut => {
    ///             // First, the `ready` future completes.
    ///             total += num;
    ///             // Then we spawn a new task onto `async_tasks`,
    ///             async_tasks.push(async { 5 });
    ///         },
    ///         // On the next iteration of the loop, the task we spawned
    ///         // completes.
    ///         num = async_tasks.select_next_some() => {
    ///             total += num;
    ///         }
    ///         // Finally, both the `ready` future and `async_tasks` have
    ///         // finished, so we enter the `complete` branch.
    ///         complete => break,
    ///     }
    /// }
    /// assert_eq!(total, 6);
    /// # });
    /// ```
    ///
    /// [`select!`]: crate::select
    fn select_next_some(&mut self) -> SelectNextSome<'_, Self>
    where
        Self: Unpin + FusedStream,
    {
        assert_future::<Self::Item, _>(SelectNextSome::new(self))
    }
}
