//! Streams
//!
//! This module contains a number of functions for working with `Streams`s
//! that return `Result`s, allowing for short-circuiting computations.

#[cfg(feature = "compat")]
use crate::compat::Compat;
use crate::fns::{
    inspect_err_fn, inspect_ok_fn, into_fn, map_err_fn, map_ok_fn, InspectErrFn, InspectOkFn,
    IntoFn, MapErrFn, MapOkFn,
};
use crate::future::assert_future;
use crate::stream::assert_stream;
use crate::stream::{Inspect, Map};
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::pin::Pin;

use futures_core::{
    future::{Future, TryFuture},
    stream::TryStream,
    task::{Context, Poll},
};

mod and_then;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::and_then::AndThen;

delegate_all!(
    /// Stream for the [`err_into`](super::TryStreamExt::err_into) method.
    ErrInto<St, E>(
        MapErr<St, IntoFn<E>>
    ): Debug + Sink + Stream + FusedStream + AccessInner[St, (.)] + New[|x: St| MapErr::new(x, into_fn())]
);

delegate_all!(
    /// Stream for the [`inspect_ok`](super::TryStreamExt::inspect_ok) method.
    InspectOk<St, F>(
        Inspect<IntoStream<St>, InspectOkFn<F>>
    ): Debug + Sink + Stream + FusedStream + AccessInner[St, (. .)] + New[|x: St, f: F| Inspect::new(IntoStream::new(x), inspect_ok_fn(f))]
);

delegate_all!(
    /// Stream for the [`inspect_err`](super::TryStreamExt::inspect_err) method.
    InspectErr<St, F>(
        Inspect<IntoStream<St>, InspectErrFn<F>>
    ): Debug + Sink + Stream + FusedStream + AccessInner[St, (. .)] + New[|x: St, f: F| Inspect::new(IntoStream::new(x), inspect_err_fn(f))]
);

mod into_stream;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::into_stream::IntoStream;

delegate_all!(
    /// Stream for the [`map_ok`](super::TryStreamExt::map_ok) method.
    MapOk<St, F>(
        Map<IntoStream<St>, MapOkFn<F>>
    ): Debug + Sink + Stream + FusedStream + AccessInner[St, (. .)] + New[|x: St, f: F| Map::new(IntoStream::new(x), map_ok_fn(f))]
);

delegate_all!(
    /// Stream for the [`map_err`](super::TryStreamExt::map_err) method.
    MapErr<St, F>(
        Map<IntoStream<St>, MapErrFn<F>>
    ): Debug + Sink + Stream + FusedStream + AccessInner[St, (. .)] + New[|x: St, f: F| Map::new(IntoStream::new(x), map_err_fn(f))]
);

mod or_else;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::or_else::OrElse;

mod try_next;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_next::TryNext;

mod try_for_each;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_for_each::TryForEach;

mod try_filter;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_filter::TryFilter;

mod try_filter_map;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_filter_map::TryFilterMap;

mod try_flatten;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_flatten::TryFlatten;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod try_flatten_unordered;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_flatten_unordered::TryFlattenUnordered;

mod try_collect;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_collect::TryCollect;

mod try_concat;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_concat::TryConcat;

#[cfg(feature = "alloc")]
mod try_chunks;
#[cfg(feature = "alloc")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_chunks::{TryChunks, TryChunksError};

#[cfg(feature = "alloc")]
mod try_ready_chunks;
#[cfg(feature = "alloc")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_ready_chunks::{TryReadyChunks, TryReadyChunksError};

mod try_fold;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_fold::TryFold;

mod try_unfold;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_unfold::{try_unfold, TryUnfold};

mod try_skip_while;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_skip_while::TrySkipWhile;

mod try_take_while;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_take_while::TryTakeWhile;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod try_buffer_unordered;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_buffer_unordered::TryBufferUnordered;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod try_buffered;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_buffered::TryBuffered;

#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
mod try_for_each_concurrent;
#[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
#[cfg(feature = "alloc")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_for_each_concurrent::TryForEachConcurrent;

#[cfg(feature = "io")]
#[cfg(feature = "std")]
mod into_async_read;
#[cfg(feature = "io")]
#[cfg_attr(docsrs, doc(cfg(feature = "io")))]
#[cfg(feature = "std")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::into_async_read::IntoAsyncRead;

mod try_all;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_all::TryAll;

mod try_any;
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::try_any::TryAny;

impl<S: ?Sized + TryStream> TryStreamExt for S {}

/// Adapters specific to `Result`-returning streams
pub trait TryStreamExt: TryStream {
    /// Wraps the current stream in a new stream which converts the error type
    /// into the one provided.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, TryStreamExt};
    ///
    /// let mut stream =
    ///     stream::iter(vec![Ok(()), Err(5i32)])
    ///         .err_into::<i64>();
    ///
    /// assert_eq!(stream.try_next().await, Ok(Some(())));
    /// assert_eq!(stream.try_next().await, Err(5i64));
    /// # })
    /// ```
    fn err_into<E>(self) -> ErrInto<Self, E>
    where
        Self: Sized,
        Self::Error: Into<E>,
    {
        assert_stream::<Result<Self::Ok, E>, _>(ErrInto::new(self))
    }

    /// Wraps the current stream in a new stream which maps the success value
    /// using the provided closure.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, TryStreamExt};
    ///
    /// let mut stream =
    ///     stream::iter(vec![Ok(5), Err(0)])
    ///         .map_ok(|x| x + 2);
    ///
    /// assert_eq!(stream.try_next().await, Ok(Some(7)));
    /// assert_eq!(stream.try_next().await, Err(0));
    /// # })
    /// ```
    fn map_ok<T, F>(self, f: F) -> MapOk<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Ok) -> T,
    {
        assert_stream::<Result<T, Self::Error>, _>(MapOk::new(self, f))
    }

    /// Wraps the current stream in a new stream which maps the error value
    /// using the provided closure.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, TryStreamExt};
    ///
    /// let mut stream =
    ///     stream::iter(vec![Ok(5), Err(0)])
    ///         .map_err(|x| x + 2);
    ///
    /// assert_eq!(stream.try_next().await, Ok(Some(5)));
    /// assert_eq!(stream.try_next().await, Err(2));
    /// # })
    /// ```
    fn map_err<E, F>(self, f: F) -> MapErr<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Error) -> E,
    {
        assert_stream::<Result<Self::Ok, E>, _>(MapErr::new(self, f))
    }

    /// Chain on a computation for when a value is ready, passing the successful
    /// results to the provided closure `f`.
    ///
    /// This function can be used to run a unit of work when the next successful
    /// value on a stream is ready. The closure provided will be yielded a value
    /// when ready, and the returned future will then be run to completion to
    /// produce the next value on this stream.
    ///
    /// Any errors produced by this stream will not be passed to the closure,
    /// and will be passed through.
    ///
    /// The returned value of the closure must implement the `TryFuture` trait
    /// and can represent some more work to be done before the composed stream
    /// is finished.
    ///
    /// Note that this function consumes the receiving stream and returns a
    /// wrapped version of it.
    ///
    /// To process the entire stream and return a single future representing
    /// success or error, use `try_for_each` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::channel::mpsc;
    /// use futures::future;
    /// use futures::stream::TryStreamExt;
    ///
    /// let (_tx, rx) = mpsc::channel::<Result<i32, ()>>(1);
    ///
    /// let rx = rx.and_then(|result| {
    ///     future::ok(if result % 2 == 0 {
    ///         Some(result)
    ///     } else {
    ///         None
    ///     })
    /// });
    /// ```
    fn and_then<Fut, F>(self, f: F) -> AndThen<Self, Fut, F>
    where
        F: FnMut(Self::Ok) -> Fut,
        Fut: TryFuture<Error = Self::Error>,
        Self: Sized,
    {
        assert_stream::<Result<Fut::Ok, Fut::Error>, _>(AndThen::new(self, f))
    }

    /// Chain on a computation for when an error happens, passing the
    /// erroneous result to the provided closure `f`.
    ///
    /// This function can be used to run a unit of work and attempt to recover from
    /// an error if one happens. The closure provided will be yielded an error
    /// when one appears, and the returned future will then be run to completion
    /// to produce the next value on this stream.
    ///
    /// Any successful values produced by this stream will not be passed to the
    /// closure, and will be passed through.
    ///
    /// The returned value of the closure must implement the [`TryFuture`](futures_core::future::TryFuture) trait
    /// and can represent some more work to be done before the composed stream
    /// is finished.
    ///
    /// Note that this function consumes the receiving stream and returns a
    /// wrapped version of it.
    fn or_else<Fut, F>(self, f: F) -> OrElse<Self, Fut, F>
    where
        F: FnMut(Self::Error) -> Fut,
        Fut: TryFuture<Ok = Self::Ok>,
        Self: Sized,
    {
        assert_stream::<Result<Self::Ok, Fut::Error>, _>(OrElse::new(self, f))
    }

    /// Do something with the success value of this stream, afterwards passing
    /// it on.
    ///
    /// This is similar to the `StreamExt::inspect` method where it allows
    /// easily inspecting the success value as it passes through the stream, for
    /// example to debug what's going on.
    fn inspect_ok<F>(self, f: F) -> InspectOk<Self, F>
    where
        F: FnMut(&Self::Ok),
        Self: Sized,
    {
        assert_stream::<Result<Self::Ok, Self::Error>, _>(InspectOk::new(self, f))
    }

    /// Do something with the error value of this stream, afterwards passing it on.
    ///
    /// This is similar to the `StreamExt::inspect` method where it allows
    /// easily inspecting the error value as it passes through the stream, for
    /// example to debug what's going on.
    fn inspect_err<F>(self, f: F) -> InspectErr<Self, F>
    where
        F: FnMut(&Self::Error),
        Self: Sized,
    {
        assert_stream::<Result<Self::Ok, Self::Error>, _>(InspectErr::new(self, f))
    }

    /// Wraps a [`TryStream`] into a type that implements
    /// [`Stream`](futures_core::stream::Stream)
    ///
    /// [`TryStream`]s currently do not implement the
    /// [`Stream`](futures_core::stream::Stream) trait because of limitations
    /// of the compiler.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::stream::{Stream, TryStream, TryStreamExt};
    ///
    /// # type T = i32;
    /// # type E = ();
    /// fn make_try_stream() -> impl TryStream<Ok = T, Error = E> { // ... }
    /// # futures::stream::empty()
    /// # }
    /// fn take_stream(stream: impl Stream<Item = Result<T, E>>) { /* ... */ }
    ///
    /// take_stream(make_try_stream().into_stream());
    /// ```
    fn into_stream(self) -> IntoStream<Self>
    where
        Self: Sized,
    {
        assert_stream::<Result<Self::Ok, Self::Error>, _>(IntoStream::new(self))
    }

    /// Creates a future that attempts to resolve the next item in the stream.
    /// If an error is encountered before the next item, the error is returned
    /// instead.
    ///
    /// This is similar to the `Stream::next` combinator, but returns a
    /// `Result<Option<T>, E>` rather than an `Option<Result<T, E>>`, making
    /// for easy use with the `?` operator.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, TryStreamExt};
    ///
    /// let mut stream = stream::iter(vec![Ok(()), Err(())]);
    ///
    /// assert_eq!(stream.try_next().await, Ok(Some(())));
    /// assert_eq!(stream.try_next().await, Err(()));
    /// # })
    /// ```
    fn try_next(&mut self) -> TryNext<'_, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<Option<Self::Ok>, Self::Error>, _>(TryNext::new(self))
    }

    /// Attempts to run this stream to completion, executing the provided
    /// asynchronous closure for each element on the stream.
    ///
    /// The provided closure will be called for each item this stream produces,
    /// yielding a future. That future will then be executed to completion
    /// before moving on to the next item.
    ///
    /// The returned value is a [`Future`](futures_core::future::Future) where the
    /// [`Output`](futures_core::future::Future::Output) type is
    /// `Result<(), Self::Error>`. If any of the intermediate
    /// futures or the stream returns an error, this future will return
    /// immediately with an error.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future;
    /// use futures::stream::{self, TryStreamExt};
    ///
    /// let mut x = 0i32;
    ///
    /// {
    ///     let fut = stream::repeat(Ok(1)).try_for_each(|item| {
    ///         x += item;
    ///         future::ready(if x == 3 { Err(()) } else { Ok(()) })
    ///     });
    ///     assert_eq!(fut.await, Err(()));
    /// }
    ///
    /// assert_eq!(x, 3);
    /// # })
    /// ```
    fn try_for_each<Fut, F>(self, f: F) -> TryForEach<Self, Fut, F>
    where
        F: FnMut(Self::Ok) -> Fut,
        Fut: TryFuture<Ok = (), Error = Self::Error>,
        Self: Sized,
    {
        assert_future::<Result<(), Self::Error>, _>(TryForEach::new(self, f))
    }

    /// Skip elements on this stream while the provided asynchronous predicate
    /// resolves to `true`.
    ///
    /// This function is similar to
    /// [`StreamExt::skip_while`](crate::stream::StreamExt::skip_while) but exits
    /// early if an error occurs.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future;
    /// use futures::stream::{self, TryStreamExt};
    ///
    /// let stream = stream::iter(vec![Ok::<i32, i32>(1), Ok(3), Ok(2)]);
    /// let stream = stream.try_skip_while(|x| future::ready(Ok(*x < 3)));
    ///
    /// let output: Result<Vec<i32>, i32> = stream.try_collect().await;
    /// assert_eq!(output, Ok(vec![3, 2]));
    /// # })
    /// ```
    fn try_skip_while<Fut, F>(self, f: F) -> TrySkipWhile<Self, Fut, F>
    where
        F: FnMut(&Self::Ok) -> Fut,
        Fut: TryFuture<Ok = bool, Error = Self::Error>,
        Self: Sized,
    {
        assert_stream::<Result<Self::Ok, Self::Error>, _>(TrySkipWhile::new(self, f))
    }

    /// Take elements on this stream while the provided asynchronous predicate
    /// resolves to `true`.
    ///
    /// This function is similar to
    /// [`StreamExt::take_while`](crate::stream::StreamExt::take_while) but exits
    /// early if an error occurs.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future;
    /// use futures::stream::{self, TryStreamExt};
    ///
    /// let stream = stream::iter(vec![Ok::<i32, i32>(1), Ok(2), Ok(3), Ok(2)]);
    /// let stream = stream.try_take_while(|x| future::ready(Ok(*x < 3)));
    ///
    /// let output: Result<Vec<i32>, i32> = stream.try_collect().await;
    /// assert_eq!(output, Ok(vec![1, 2]));
    /// # })
    /// ```
    fn try_take_while<Fut, F>(self, f: F) -> TryTakeWhile<Self, Fut, F>
    where
        F: FnMut(&Self::Ok) -> Fut,
        Fut: TryFuture<Ok = bool, Error = Self::Error>,
        Self: Sized,
    {
        assert_stream::<Result<Self::Ok, Self::Error>, _>(TryTakeWhile::new(self, f))
    }

    /// Attempts to run this stream to completion, executing the provided asynchronous
    /// closure for each element on the stream concurrently as elements become
    /// available, exiting as soon as an error occurs.
    ///
    /// This is similar to
    /// [`StreamExt::for_each_concurrent`](crate::stream::StreamExt::for_each_concurrent),
    /// but will resolve to an error immediately if the underlying stream or the provided
    /// closure return an error.
    ///
    /// This method is only available when the `std` or `alloc` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::oneshot;
    /// use futures::stream::{self, StreamExt, TryStreamExt};
    ///
    /// let (tx1, rx1) = oneshot::channel();
    /// let (tx2, rx2) = oneshot::channel();
    /// let (_tx3, rx3) = oneshot::channel();
    ///
    /// let stream = stream::iter(vec![rx1, rx2, rx3]);
    /// let fut = stream.map(Ok).try_for_each_concurrent(
    ///     /* limit */ 2,
    ///     |rx| async move {
    ///         let res: Result<(), oneshot::Canceled> = rx.await;
    ///         res
    ///     }
    /// );
    ///
    /// tx1.send(()).unwrap();
    /// // Drop the second sender so that `rx2` resolves to `Canceled`.
    /// drop(tx2);
    ///
    /// // The final result is an error because the second future
    /// // resulted in an error.
    /// assert_eq!(Err(oneshot::Canceled), fut.await);
    /// # })
    /// ```
    #[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
    #[cfg(feature = "alloc")]
    fn try_for_each_concurrent<Fut, F>(
        self,
        limit: impl Into<Option<usize>>,
        f: F,
    ) -> TryForEachConcurrent<Self, Fut, F>
    where
        F: FnMut(Self::Ok) -> Fut,
        Fut: Future<Output = Result<(), Self::Error>>,
        Self: Sized,
    {
        assert_future::<Result<(), Self::Error>, _>(TryForEachConcurrent::new(
            self,
            limit.into(),
            f,
        ))
    }

    /// Attempt to transform a stream into a collection,
    /// returning a future representing the result of that computation.
    ///
    /// This combinator will collect all successful results of this stream and
    /// collect them into the specified collection type. If an error happens then all
    /// collected elements will be dropped and the error will be returned.
    ///
    /// The returned future will be resolved when the stream terminates.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::mpsc;
    /// use futures::stream::TryStreamExt;
    /// use std::thread;
    ///
    /// let (tx, rx) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     for i in 1..=5 {
    ///         tx.unbounded_send(Ok(i)).unwrap();
    ///     }
    ///     tx.unbounded_send(Err(6)).unwrap();
    /// });
    ///
    /// let output: Result<Vec<i32>, i32> = rx.try_collect().await;
    /// assert_eq!(output, Err(6));
    /// # })
    /// ```
    fn try_collect<C: Default + Extend<Self::Ok>>(self) -> TryCollect<Self, C>
    where
        Self: Sized,
    {
        assert_future::<Result<C, Self::Error>, _>(TryCollect::new(self))
    }

    /// An adaptor for chunking up successful items of the stream inside a vector.
    ///
    /// This combinator will attempt to pull successful items from this stream and buffer
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
    /// This function is similar to
    /// [`StreamExt::chunks`](crate::stream::StreamExt::chunks) but exits
    /// early if an error occurs.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, TryChunksError, TryStreamExt};
    ///
    /// let stream = stream::iter(vec![Ok::<i32, i32>(1), Ok(2), Ok(3), Err(4), Ok(5), Ok(6)]);
    /// let mut stream = stream.try_chunks(2);
    ///
    /// assert_eq!(stream.try_next().await, Ok(Some(vec![1, 2])));
    /// assert_eq!(stream.try_next().await, Err(TryChunksError(vec![3], 4)));
    /// assert_eq!(stream.try_next().await, Ok(Some(vec![5, 6])));
    /// # })
    /// ```
    ///
    /// # Panics
    ///
    /// This method will panic if `capacity` is zero.
    #[cfg(feature = "alloc")]
    fn try_chunks(self, capacity: usize) -> TryChunks<Self>
    where
        Self: Sized,
    {
        assert_stream::<Result<Vec<Self::Ok>, TryChunksError<Self::Ok, Self::Error>>, _>(
            TryChunks::new(self, capacity),
        )
    }

    /// An adaptor for chunking up successful, ready items of the stream inside a vector.
    ///
    /// This combinator will attempt to pull successful items from this stream and buffer
    /// them into a local vector. At most `capacity` items will get buffered
    /// before they're yielded from the returned stream. If the underlying stream
    /// returns `Poll::Pending`, and the collected chunk is not empty, it will
    /// be immediately returned.
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
    /// This function is similar to
    /// [`StreamExt::ready_chunks`](crate::stream::StreamExt::ready_chunks) but exits
    /// early if an error occurs.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, TryReadyChunksError, TryStreamExt};
    ///
    /// let stream = stream::iter(vec![Ok::<i32, i32>(1), Ok(2), Ok(3), Err(4), Ok(5), Ok(6)]);
    /// let mut stream = stream.try_ready_chunks(2);
    ///
    /// assert_eq!(stream.try_next().await, Ok(Some(vec![1, 2])));
    /// assert_eq!(stream.try_next().await, Err(TryReadyChunksError(vec![3], 4)));
    /// assert_eq!(stream.try_next().await, Ok(Some(vec![5, 6])));
    /// # })
    /// ```
    ///
    /// # Panics
    ///
    /// This method will panic if `capacity` is zero.
    #[cfg(feature = "alloc")]
    fn try_ready_chunks(self, capacity: usize) -> TryReadyChunks<Self>
    where
        Self: Sized,
    {
        assert_stream::<Result<Vec<Self::Ok>, TryReadyChunksError<Self::Ok, Self::Error>>, _>(
            TryReadyChunks::new(self, capacity),
        )
    }

    /// Attempt to filter the values produced by this stream according to the
    /// provided asynchronous closure.
    ///
    /// As values of this stream are made available, the provided predicate `f`
    /// will be run on them. If the predicate returns a `Future` which resolves
    /// to `true`, then the stream will yield the value, but if the predicate
    /// return a `Future` which resolves to `false`, then the value will be
    /// discarded and the next value will be produced.
    ///
    /// All errors are passed through without filtering in this combinator.
    ///
    /// Note that this function consumes the stream passed into it and returns a
    /// wrapped version of it, similar to the existing `filter` methods in
    /// the standard library.
    ///
    /// # Examples
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future;
    /// use futures::stream::{self, StreamExt, TryStreamExt};
    ///
    /// let stream = stream::iter(vec![Ok(1i32), Ok(2i32), Ok(3i32), Err("error")]);
    /// let mut evens = stream.try_filter(|x| {
    ///     future::ready(x % 2 == 0)
    /// });
    ///
    /// assert_eq!(evens.next().await, Some(Ok(2)));
    /// assert_eq!(evens.next().await, Some(Err("error")));
    /// # })
    /// ```
    fn try_filter<Fut, F>(self, f: F) -> TryFilter<Self, Fut, F>
    where
        Fut: Future<Output = bool>,
        F: FnMut(&Self::Ok) -> Fut,
        Self: Sized,
    {
        assert_stream::<Result<Self::Ok, Self::Error>, _>(TryFilter::new(self, f))
    }

    /// Attempt to filter the values produced by this stream while
    /// simultaneously mapping them to a different type according to the
    /// provided asynchronous closure.
    ///
    /// As values of this stream are made available, the provided function will
    /// be run on them. If the future returned by the predicate `f` resolves to
    /// [`Some(item)`](Some) then the stream will yield the value `item`, but if
    /// it resolves to [`None`] then the next value will be produced.
    ///
    /// All errors are passed through without filtering in this combinator.
    ///
    /// Note that this function consumes the stream passed into it and returns a
    /// wrapped version of it, similar to the existing `filter_map` methods in
    /// the standard library.
    ///
    /// # Examples
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt, TryStreamExt};
    /// use futures::pin_mut;
    ///
    /// let stream = stream::iter(vec![Ok(1i32), Ok(6i32), Err("error")]);
    /// let halves = stream.try_filter_map(|x| async move {
    ///     let ret = if x % 2 == 0 { Some(x / 2) } else { None };
    ///     Ok(ret)
    /// });
    ///
    /// pin_mut!(halves);
    /// assert_eq!(halves.next().await, Some(Ok(3)));
    /// assert_eq!(halves.next().await, Some(Err("error")));
    /// # })
    /// ```
    fn try_filter_map<Fut, F, T>(self, f: F) -> TryFilterMap<Self, Fut, F>
    where
        Fut: TryFuture<Ok = Option<T>, Error = Self::Error>,
        F: FnMut(Self::Ok) -> Fut,
        Self: Sized,
    {
        assert_stream::<Result<T, Self::Error>, _>(TryFilterMap::new(self, f))
    }

    /// Flattens a stream of streams into just one continuous stream. Produced streams
    /// will be polled concurrently and any errors will be passed through without looking at them.
    /// If the underlying base stream returns an error, it will be **immediately** propagated.
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
    /// use futures::stream::{StreamExt, TryStreamExt};
    /// use std::thread;
    ///
    /// let (tx1, rx1) = mpsc::unbounded();
    /// let (tx2, rx2) = mpsc::unbounded();
    /// let (tx3, rx3) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     tx1.unbounded_send(Ok(1)).unwrap();
    /// });
    /// thread::spawn(move || {
    ///     tx2.unbounded_send(Ok(2)).unwrap();
    ///     tx2.unbounded_send(Err(3)).unwrap();
    ///     tx2.unbounded_send(Ok(4)).unwrap();
    /// });
    /// thread::spawn(move || {
    ///     tx3.unbounded_send(Ok(rx1)).unwrap();
    ///     tx3.unbounded_send(Ok(rx2)).unwrap();
    ///     tx3.unbounded_send(Err(5)).unwrap();
    /// });
    ///
    /// let stream = rx3.try_flatten_unordered(None);
    /// let mut values: Vec<_> = stream.collect().await;
    /// values.sort();
    ///
    /// assert_eq!(values, vec![Ok(1), Ok(2), Ok(4), Err(3), Err(5)]);
    /// # });
    /// ```
    #[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
    #[cfg(feature = "alloc")]
    fn try_flatten_unordered(self, limit: impl Into<Option<usize>>) -> TryFlattenUnordered<Self>
    where
        Self::Ok: TryStream + Unpin,
        <Self::Ok as TryStream>::Error: From<Self::Error>,
        Self: Sized,
    {
        assert_stream::<Result<<Self::Ok as TryStream>::Ok, <Self::Ok as TryStream>::Error>, _>(
            TryFlattenUnordered::new(self, limit),
        )
    }

    /// Flattens a stream of streams into just one continuous stream.
    ///
    /// If this stream's elements are themselves streams then this combinator
    /// will flatten out the entire stream to one long chain of elements. Any
    /// errors are passed through without looking at them, but otherwise each
    /// individual stream will get exhausted before moving on to the next.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::mpsc;
    /// use futures::stream::{StreamExt, TryStreamExt};
    /// use std::thread;
    ///
    /// let (tx1, rx1) = mpsc::unbounded();
    /// let (tx2, rx2) = mpsc::unbounded();
    /// let (tx3, rx3) = mpsc::unbounded();
    ///
    /// thread::spawn(move || {
    ///     tx1.unbounded_send(Ok(1)).unwrap();
    /// });
    /// thread::spawn(move || {
    ///     tx2.unbounded_send(Ok(2)).unwrap();
    ///     tx2.unbounded_send(Err(3)).unwrap();
    ///     tx2.unbounded_send(Ok(4)).unwrap();
    /// });
    /// thread::spawn(move || {
    ///     tx3.unbounded_send(Ok(rx1)).unwrap();
    ///     tx3.unbounded_send(Ok(rx2)).unwrap();
    ///     tx3.unbounded_send(Err(5)).unwrap();
    /// });
    ///
    /// let mut stream = rx3.try_flatten();
    /// assert_eq!(stream.next().await, Some(Ok(1)));
    /// assert_eq!(stream.next().await, Some(Ok(2)));
    /// assert_eq!(stream.next().await, Some(Err(3)));
    /// assert_eq!(stream.next().await, Some(Ok(4)));
    /// assert_eq!(stream.next().await, Some(Err(5)));
    /// assert_eq!(stream.next().await, None);
    /// # });
    /// ```
    fn try_flatten(self) -> TryFlatten<Self>
    where
        Self::Ok: TryStream,
        <Self::Ok as TryStream>::Error: From<Self::Error>,
        Self: Sized,
    {
        assert_stream::<Result<<Self::Ok as TryStream>::Ok, <Self::Ok as TryStream>::Error>, _>(
            TryFlatten::new(self),
        )
    }

    /// Attempt to execute an accumulating asynchronous computation over a
    /// stream, collecting all the values into one final result.
    ///
    /// This combinator will accumulate all values returned by this stream
    /// according to the closure provided. The initial state is also provided to
    /// this method and then is returned again by each execution of the closure.
    /// Once the entire stream has been exhausted the returned future will
    /// resolve to this value.
    ///
    /// This method is similar to [`fold`](crate::stream::StreamExt::fold), but will
    /// exit early if an error is encountered in either the stream or the
    /// provided closure.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, TryStreamExt};
    ///
    /// let number_stream = stream::iter(vec![Ok::<i32, i32>(1), Ok(2)]);
    /// let sum = number_stream.try_fold(0, |acc, x| async move { Ok(acc + x) });
    /// assert_eq!(sum.await, Ok(3));
    ///
    /// let number_stream_with_err = stream::iter(vec![Ok::<i32, i32>(1), Err(2), Ok(1)]);
    /// let sum = number_stream_with_err.try_fold(0, |acc, x| async move { Ok(acc + x) });
    /// assert_eq!(sum.await, Err(2));
    /// # })
    /// ```
    fn try_fold<T, Fut, F>(self, init: T, f: F) -> TryFold<Self, Fut, T, F>
    where
        F: FnMut(T, Self::Ok) -> Fut,
        Fut: TryFuture<Ok = T, Error = Self::Error>,
        Self: Sized,
    {
        assert_future::<Result<T, Self::Error>, _>(TryFold::new(self, f, init))
    }

    /// Attempt to concatenate all items of a stream into a single
    /// extendable destination, returning a future representing the end result.
    ///
    /// This combinator will extend the first item with the contents of all
    /// the subsequent successful results of the stream. If the stream is empty,
    /// the default value will be returned.
    ///
    /// Works with all collections that implement the [`Extend`](std::iter::Extend) trait.
    ///
    /// This method is similar to [`concat`](crate::stream::StreamExt::concat), but will
    /// exit early if an error is encountered in the stream.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::mpsc;
    /// use futures::stream::TryStreamExt;
    /// use std::thread;
    ///
    /// let (tx, rx) = mpsc::unbounded::<Result<Vec<i32>, ()>>();
    ///
    /// thread::spawn(move || {
    ///     for i in (0..3).rev() {
    ///         let n = i * 3;
    ///         tx.unbounded_send(Ok(vec![n + 1, n + 2, n + 3])).unwrap();
    ///     }
    /// });
    ///
    /// let result = rx.try_concat().await;
    ///
    /// assert_eq!(result, Ok(vec![7, 8, 9, 4, 5, 6, 1, 2, 3]));
    /// # });
    /// ```
    fn try_concat(self) -> TryConcat<Self>
    where
        Self: Sized,
        Self::Ok: Extend<<<Self as TryStream>::Ok as IntoIterator>::Item> + IntoIterator + Default,
    {
        assert_future::<Result<Self::Ok, Self::Error>, _>(TryConcat::new(self))
    }

    /// Attempt to execute several futures from a stream concurrently (unordered).
    ///
    /// This stream's `Ok` type must be a [`TryFuture`](futures_core::future::TryFuture) with an `Error` type
    /// that matches the stream's `Error` type.
    ///
    /// This adaptor will buffer up to `n` futures and then return their
    /// outputs in the order in which they complete. If the underlying stream
    /// returns an error, it will be immediately propagated.
    ///
    /// The returned stream will be a stream of results, each containing either
    /// an error or a future's output. An error can be produced either by the
    /// underlying stream itself or by one of the futures it yielded.
    ///
    /// This method is only available when the `std` or `alloc` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// Results are returned in the order of completion:
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::oneshot;
    /// use futures::stream::{self, StreamExt, TryStreamExt};
    ///
    /// let (send_one, recv_one) = oneshot::channel();
    /// let (send_two, recv_two) = oneshot::channel();
    ///
    /// let stream_of_futures = stream::iter(vec![Ok(recv_one), Ok(recv_two)]);
    ///
    /// let mut buffered = stream_of_futures.try_buffer_unordered(10);
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
    ///
    /// Errors from the underlying stream itself are propagated:
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::mpsc;
    /// use futures::stream::{StreamExt, TryStreamExt};
    ///
    /// let (sink, stream_of_futures) = mpsc::unbounded();
    /// let mut buffered = stream_of_futures.try_buffer_unordered(10);
    ///
    /// sink.unbounded_send(Ok(async { Ok(7i32) }))?;
    /// assert_eq!(buffered.next().await, Some(Ok(7i32)));
    ///
    /// sink.unbounded_send(Err("error in the stream"))?;
    /// assert_eq!(buffered.next().await, Some(Err("error in the stream")));
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    #[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
    #[cfg(feature = "alloc")]
    fn try_buffer_unordered(self, n: usize) -> TryBufferUnordered<Self>
    where
        Self::Ok: TryFuture<Error = Self::Error>,
        Self: Sized,
    {
        assert_stream::<Result<<Self::Ok as TryFuture>::Ok, Self::Error>, _>(
            TryBufferUnordered::new(self, n),
        )
    }

    /// Attempt to execute several futures from a stream concurrently.
    ///
    /// This stream's `Ok` type must be a [`TryFuture`](futures_core::future::TryFuture) with an `Error` type
    /// that matches the stream's `Error` type.
    ///
    /// This adaptor will buffer up to `n` futures and then return their
    /// outputs in the same order as the underlying stream. If the underlying stream returns an error, it will
    /// be immediately propagated.
    ///
    /// The returned stream will be a stream of results, each containing either
    /// an error or a future's output. An error can be produced either by the
    /// underlying stream itself or by one of the futures it yielded.
    ///
    /// This method is only available when the `std` or `alloc` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// Results are returned in the order of addition:
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::oneshot;
    /// use futures::future::lazy;
    /// use futures::stream::{self, StreamExt, TryStreamExt};
    ///
    /// let (send_one, recv_one) = oneshot::channel();
    /// let (send_two, recv_two) = oneshot::channel();
    ///
    /// let mut buffered = lazy(move |cx| {
    ///     let stream_of_futures = stream::iter(vec![Ok(recv_one), Ok(recv_two)]);
    ///
    ///     let mut buffered = stream_of_futures.try_buffered(10);
    ///
    ///     assert!(buffered.try_poll_next_unpin(cx).is_pending());
    ///
    ///     send_two.send(2i32)?;
    ///     assert!(buffered.try_poll_next_unpin(cx).is_pending());
    ///     Ok::<_, i32>(buffered)
    /// }).await?;
    ///
    /// send_one.send(1i32)?;
    /// assert_eq!(buffered.next().await, Some(Ok(1i32)));
    /// assert_eq!(buffered.next().await, Some(Ok(2i32)));
    ///
    /// assert_eq!(buffered.next().await, None);
    /// # Ok::<(), i32>(()) }).unwrap();
    /// ```
    ///
    /// Errors from the underlying stream itself are propagated:
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::channel::mpsc;
    /// use futures::stream::{StreamExt, TryStreamExt};
    ///
    /// let (sink, stream_of_futures) = mpsc::unbounded();
    /// let mut buffered = stream_of_futures.try_buffered(10);
    ///
    /// sink.unbounded_send(Ok(async { Ok(7i32) }))?;
    /// assert_eq!(buffered.next().await, Some(Ok(7i32)));
    ///
    /// sink.unbounded_send(Err("error in the stream"))?;
    /// assert_eq!(buffered.next().await, Some(Err("error in the stream")));
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    #[cfg_attr(target_os = "none", cfg(target_has_atomic = "ptr"))]
    #[cfg(feature = "alloc")]
    fn try_buffered(self, n: usize) -> TryBuffered<Self>
    where
        Self::Ok: TryFuture<Error = Self::Error>,
        Self: Sized,
    {
        assert_stream::<Result<<Self::Ok as TryFuture>::Ok, Self::Error>, _>(TryBuffered::new(
            self, n,
        ))
    }

    // TODO: false positive warning from rustdoc. Verify once #43466 settles
    //
    /// A convenience method for calling [`TryStream::try_poll_next`] on [`Unpin`]
    /// stream types.
    fn try_poll_next_unpin(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Ok, Self::Error>>>
    where
        Self: Unpin,
    {
        Pin::new(self).try_poll_next(cx)
    }

    /// Wraps a [`TryStream`] into a stream compatible with libraries using
    /// futures 0.1 `Stream`. Requires the `compat` feature to be enabled.
    /// ```
    /// # if cfg!(miri) { return; } // Miri does not support epoll
    /// use futures::future::{FutureExt, TryFutureExt};
    /// # let (tx, rx) = futures::channel::oneshot::channel();
    ///
    /// let future03 = async {
    ///     println!("Running on the pool");
    ///     tx.send(42).unwrap();
    /// };
    ///
    /// let future01 = future03
    ///     .unit_error() // Make it a TryFuture
    ///     .boxed()  // Make it Unpin
    ///     .compat();
    ///
    /// tokio::run(future01);
    /// # assert_eq!(42, futures::executor::block_on(rx).unwrap());
    /// ```
    #[cfg(feature = "compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "compat")))]
    fn compat(self) -> Compat<Self>
    where
        Self: Sized + Unpin,
    {
        Compat::new(self)
    }

    /// Adapter that converts this stream into an [`AsyncBufRead`](crate::io::AsyncBufRead).
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, TryStreamExt};
    /// use futures::io::AsyncReadExt;
    ///
    /// let stream = stream::iter([Ok(vec![1, 2, 3]), Ok(vec![4, 5])]);
    /// let mut reader = stream.into_async_read();
    ///
    /// let mut buf = Vec::new();
    /// reader.read_to_end(&mut buf).await.unwrap();
    /// assert_eq!(buf, [1, 2, 3, 4, 5]);
    /// # })
    /// ```
    #[cfg(feature = "io")]
    #[cfg_attr(docsrs, doc(cfg(feature = "io")))]
    #[cfg(feature = "std")]
    fn into_async_read(self) -> IntoAsyncRead<Self>
    where
        Self: Sized + TryStreamExt<Error = std::io::Error>,
        Self::Ok: AsRef<[u8]>,
    {
        crate::io::assert_read(IntoAsyncRead::new(self))
    }

    /// Attempt to execute a predicate over an asynchronous stream and evaluate if all items
    /// satisfy the predicate. Exits early if an `Err` is encountered or if an `Ok` item is found
    /// that does not satisfy the predicate.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt, TryStreamExt};
    /// use std::convert::Infallible;
    ///
    /// let number_stream = stream::iter(1..10).map(Ok::<_, Infallible>);
    /// let positive = number_stream.try_all(|i| async move { i > 0 });
    /// assert_eq!(positive.await, Ok(true));
    ///
    /// let stream_with_errors = stream::iter([Ok(1), Err("err"), Ok(3)]);
    /// let positive = stream_with_errors.try_all(|i| async move { i > 0 });
    /// assert_eq!(positive.await, Err("err"));
    /// # });
    /// ```
    fn try_all<Fut, F>(self, f: F) -> TryAll<Self, Fut, F>
    where
        Self: Sized,
        F: FnMut(Self::Ok) -> Fut,
        Fut: Future<Output = bool>,
    {
        assert_future::<Result<bool, Self::Error>, _>(TryAll::new(self, f))
    }

    /// Attempt to execute a predicate over an asynchronous stream and evaluate if any items
    /// satisfy the predicate. Exits early if an `Err` is encountered or if an `Ok` item is found
    /// that satisfies the predicate.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt, TryStreamExt};
    /// use std::convert::Infallible;
    ///
    /// let number_stream = stream::iter(0..10).map(Ok::<_, Infallible>);
    /// let contain_three = number_stream.try_any(|i| async move { i == 3 });
    /// assert_eq!(contain_three.await, Ok(true));
    ///
    /// let stream_with_errors = stream::iter([Ok(1), Err("err"), Ok(3)]);
    /// let contain_three = stream_with_errors.try_any(|i| async move { i == 3 });
    /// assert_eq!(contain_three.await, Err("err"));
    /// # });
    /// ```
    fn try_any<Fut, F>(self, f: F) -> TryAny<Self, Fut, F>
    where
        Self: Sized,
        F: FnMut(Self::Ok) -> Fut,
        Fut: Future<Output = bool>,
    {
        assert_future::<Result<bool, Self::Error>, _>(TryAny::new(self, f))
    }
}
