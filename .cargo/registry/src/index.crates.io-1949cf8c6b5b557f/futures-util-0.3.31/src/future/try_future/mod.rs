//! Futures
//!
//! This module contains a number of functions for working with `Future`s,
//! including the `FutureExt` trait which adds methods to `Future` types.

#[cfg(feature = "compat")]
use crate::compat::Compat;
use core::pin::Pin;
use futures_core::{
    future::TryFuture,
    stream::TryStream,
    task::{Context, Poll},
};
#[cfg(feature = "sink")]
use futures_sink::Sink;

use crate::fns::{
    inspect_err_fn, inspect_ok_fn, into_fn, map_err_fn, map_ok_fn, map_ok_or_else_fn,
    unwrap_or_else_fn, InspectErrFn, InspectOkFn, IntoFn, MapErrFn, MapOkFn, MapOkOrElseFn,
    UnwrapOrElseFn,
};
use crate::future::{assert_future, Inspect, Map};
use crate::stream::assert_stream;

// Combinators
mod into_future;
mod try_flatten;
mod try_flatten_err;

delegate_all!(
    /// Future for the [`try_flatten`](TryFutureExt::try_flatten) method.
    TryFlatten<Fut1, Fut2>(
        try_flatten::TryFlatten<Fut1, Fut2>
    ): Debug + Future + FusedFuture + New[|x: Fut1| try_flatten::TryFlatten::new(x)]
);

delegate_all!(
    /// Future for the [`try_flatten_err`](TryFutureExt::try_flatten_err) method.
    TryFlattenErr<Fut1, Fut2>(
        try_flatten_err::TryFlattenErr<Fut1, Fut2>
    ): Debug + Future + FusedFuture + New[|x: Fut1| try_flatten_err::TryFlattenErr::new(x)]
);

delegate_all!(
    /// Future for the [`try_flatten_stream`](TryFutureExt::try_flatten_stream) method.
    TryFlattenStream<Fut>(
        try_flatten::TryFlatten<Fut, Fut::Ok>
    ): Debug + Sink + Stream + FusedStream + New[|x: Fut| try_flatten::TryFlatten::new(x)]
    where Fut: TryFuture
);

#[cfg(feature = "sink")]
delegate_all!(
    /// Sink for the [`flatten_sink`](TryFutureExt::flatten_sink) method.
    #[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
    FlattenSink<Fut, Si>(
        try_flatten::TryFlatten<Fut, Si>
    ): Debug + Sink + Stream + FusedStream + New[|x: Fut| try_flatten::TryFlatten::new(x)]
);

delegate_all!(
    /// Future for the [`and_then`](TryFutureExt::and_then) method.
    AndThen<Fut1, Fut2, F>(
        TryFlatten<MapOk<Fut1, F>, Fut2>
    ): Debug + Future + FusedFuture + New[|x: Fut1, f: F| TryFlatten::new(MapOk::new(x, f))]
);

delegate_all!(
    /// Future for the [`or_else`](TryFutureExt::or_else) method.
    OrElse<Fut1, Fut2, F>(
        TryFlattenErr<MapErr<Fut1, F>, Fut2>
    ): Debug + Future + FusedFuture + New[|x: Fut1, f: F| TryFlattenErr::new(MapErr::new(x, f))]
);

delegate_all!(
    /// Future for the [`err_into`](TryFutureExt::err_into) method.
    ErrInto<Fut, E>(
        MapErr<Fut, IntoFn<E>>
    ): Debug + Future + FusedFuture + New[|x: Fut| MapErr::new(x, into_fn())]
);

delegate_all!(
    /// Future for the [`ok_into`](TryFutureExt::ok_into) method.
    OkInto<Fut, E>(
        MapOk<Fut, IntoFn<E>>
    ): Debug + Future + FusedFuture + New[|x: Fut| MapOk::new(x, into_fn())]
);

delegate_all!(
    /// Future for the [`inspect_ok`](super::TryFutureExt::inspect_ok) method.
    InspectOk<Fut, F>(
        Inspect<IntoFuture<Fut>, InspectOkFn<F>>
    ): Debug + Future + FusedFuture + New[|x: Fut, f: F| Inspect::new(IntoFuture::new(x), inspect_ok_fn(f))]
);

delegate_all!(
    /// Future for the [`inspect_err`](super::TryFutureExt::inspect_err) method.
    InspectErr<Fut, F>(
        Inspect<IntoFuture<Fut>, InspectErrFn<F>>
    ): Debug + Future + FusedFuture + New[|x: Fut, f: F| Inspect::new(IntoFuture::new(x), inspect_err_fn(f))]
);

#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::into_future::IntoFuture;

delegate_all!(
    /// Future for the [`map_ok`](TryFutureExt::map_ok) method.
    MapOk<Fut, F>(
        Map<IntoFuture<Fut>, MapOkFn<F>>
    ): Debug + Future + FusedFuture + New[|x: Fut, f: F| Map::new(IntoFuture::new(x), map_ok_fn(f))]
);

delegate_all!(
    /// Future for the [`map_err`](TryFutureExt::map_err) method.
    MapErr<Fut, F>(
        Map<IntoFuture<Fut>, MapErrFn<F>>
    ): Debug + Future + FusedFuture + New[|x: Fut, f: F| Map::new(IntoFuture::new(x), map_err_fn(f))]
);

delegate_all!(
    /// Future for the [`map_ok_or_else`](TryFutureExt::map_ok_or_else) method.
    MapOkOrElse<Fut, F, G>(
        Map<IntoFuture<Fut>, MapOkOrElseFn<F, G>>
    ): Debug + Future + FusedFuture + New[|x: Fut, f: F, g: G| Map::new(IntoFuture::new(x), map_ok_or_else_fn(f, g))]
);

delegate_all!(
    /// Future for the [`unwrap_or_else`](TryFutureExt::unwrap_or_else) method.
    UnwrapOrElse<Fut, F>(
        Map<IntoFuture<Fut>, UnwrapOrElseFn<F>>
    ): Debug + Future + FusedFuture + New[|x: Fut, f: F| Map::new(IntoFuture::new(x), unwrap_or_else_fn(f))]
);

impl<Fut: ?Sized + TryFuture> TryFutureExt for Fut {}

/// Adapters specific to [`Result`]-returning futures
pub trait TryFutureExt: TryFuture {
    /// Flattens the execution of this future when the successful result of this
    /// future is a [`Sink`].
    ///
    /// This can be useful when sink initialization is deferred, and it is
    /// convenient to work with that sink as if the sink was available at the
    /// call site.
    ///
    /// Note that this function consumes this future and returns a wrapped
    /// version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::future::{Future, TryFutureExt};
    /// use futures::sink::Sink;
    /// # use futures::channel::mpsc::{self, SendError};
    /// # type T = i32;
    /// # type E = SendError;
    ///
    /// fn make_sink_async() -> impl Future<Output = Result<
    ///     impl Sink<T, Error = E>,
    ///     E,
    /// >> { // ... }
    /// # let (tx, _rx) = mpsc::unbounded::<i32>();
    /// # futures::future::ready(Ok(tx))
    /// # }
    /// fn take_sink(sink: impl Sink<T, Error = E>) { /* ... */ }
    ///
    /// let fut = make_sink_async();
    /// take_sink(fut.flatten_sink())
    /// ```
    #[cfg(feature = "sink")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
    fn flatten_sink<Item>(self) -> FlattenSink<Self, Self::Ok>
    where
        Self::Ok: Sink<Item, Error = Self::Error>,
        Self: Sized,
    {
        crate::sink::assert_sink::<Item, Self::Error, _>(FlattenSink::new(self))
    }

    /// Maps this future's success value to a different value.
    ///
    /// This method can be used to change the [`Ok`](TryFuture::Ok) type of the
    /// future into a different type. It is similar to the [`Result::map`]
    /// method. You can use this method to chain along a computation once the
    /// future has been resolved.
    ///
    /// The provided closure `f` will only be called if this future is resolved
    /// to an [`Ok`]. If it resolves to an [`Err`], panics, or is dropped, then
    /// the provided closure will never be invoked.
    ///
    /// Note that this method consumes the future it is called on and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::future::TryFutureExt;
    ///
    /// # futures::executor::block_on(async {
    /// let future = async { Ok::<i32, i32>(1) };
    /// let future = future.map_ok(|x| x + 3);
    /// assert_eq!(future.await, Ok(4));
    /// # });
    /// ```
    ///
    /// Calling [`map_ok`](TryFutureExt::map_ok) on an errored future has no
    /// effect:
    ///
    /// ```
    /// use futures::future::TryFutureExt;
    ///
    /// # futures::executor::block_on(async {
    /// let future = async { Err::<i32, i32>(1) };
    /// let future = future.map_ok(|x| x + 3);
    /// assert_eq!(future.await, Err(1));
    /// # });
    /// ```
    fn map_ok<T, F>(self, f: F) -> MapOk<Self, F>
    where
        F: FnOnce(Self::Ok) -> T,
        Self: Sized,
    {
        assert_future::<Result<T, Self::Error>, _>(MapOk::new(self, f))
    }

    /// Maps this future's success value to a different value, and permits for error handling resulting in the same type.
    ///
    /// This method can be used to coalesce your [`Ok`](TryFuture::Ok) type and [`Error`](TryFuture::Error) into another type,
    /// where that type is the same for both outcomes.
    ///
    /// The provided closure `f` will only be called if this future is resolved
    /// to an [`Ok`]. If it resolves to an [`Err`], panics, or is dropped, then
    /// the provided closure will never be invoked.
    ///
    /// The provided closure `e` will only be called if this future is resolved
    /// to an [`Err`]. If it resolves to an [`Ok`], panics, or is dropped, then
    /// the provided closure will never be invoked.
    ///
    /// Note that this method consumes the future it is called on and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::future::TryFutureExt;
    ///
    /// # futures::executor::block_on(async {
    /// let future = async { Ok::<i32, i32>(5) };
    /// let future = future.map_ok_or_else(|x| x * 2, |x| x + 3);
    /// assert_eq!(future.await, 8);
    ///
    /// let future = async { Err::<i32, i32>(5) };
    /// let future = future.map_ok_or_else(|x| x * 2, |x| x + 3);
    /// assert_eq!(future.await, 10);
    /// # });
    /// ```
    ///
    fn map_ok_or_else<T, E, F>(self, e: E, f: F) -> MapOkOrElse<Self, F, E>
    where
        F: FnOnce(Self::Ok) -> T,
        E: FnOnce(Self::Error) -> T,
        Self: Sized,
    {
        assert_future::<T, _>(MapOkOrElse::new(self, f, e))
    }

    /// Maps this future's error value to a different value.
    ///
    /// This method can be used to change the [`Error`](TryFuture::Error) type
    /// of the future into a different type. It is similar to the
    /// [`Result::map_err`] method. You can use this method for example to
    /// ensure that futures have the same [`Error`](TryFuture::Error) type when
    /// using [`select!`] or [`join!`].
    ///
    /// The provided closure `f` will only be called if this future is resolved
    /// to an [`Err`]. If it resolves to an [`Ok`], panics, or is dropped, then
    /// the provided closure will never be invoked.
    ///
    /// Note that this method consumes the future it is called on and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::future::TryFutureExt;
    ///
    /// # futures::executor::block_on(async {
    /// let future = async { Err::<i32, i32>(1) };
    /// let future = future.map_err(|x| x + 3);
    /// assert_eq!(future.await, Err(4));
    /// # });
    /// ```
    ///
    /// Calling [`map_err`](TryFutureExt::map_err) on a successful future has
    /// no effect:
    ///
    /// ```
    /// use futures::future::TryFutureExt;
    ///
    /// # futures::executor::block_on(async {
    /// let future = async { Ok::<i32, i32>(1) };
    /// let future = future.map_err(|x| x + 3);
    /// assert_eq!(future.await, Ok(1));
    /// # });
    /// ```
    ///
    /// [`join!`]: crate::join
    /// [`select!`]: crate::select
    fn map_err<E, F>(self, f: F) -> MapErr<Self, F>
    where
        F: FnOnce(Self::Error) -> E,
        Self: Sized,
    {
        assert_future::<Result<Self::Ok, E>, _>(MapErr::new(self, f))
    }

    /// Maps this future's [`Error`](TryFuture::Error) to a new error type
    /// using the [`Into`](std::convert::Into) trait.
    ///
    /// This method does for futures what the `?`-operator does for
    /// [`Result`]: It lets the compiler infer the type of the resulting
    /// error. Just as [`map_err`](TryFutureExt::map_err), this is useful for
    /// example to ensure that futures have the same [`Error`](TryFuture::Error)
    /// type when using [`select!`] or [`join!`].
    ///
    /// Note that this method consumes the future it is called on and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::future::TryFutureExt;
    ///
    /// # futures::executor::block_on(async {
    /// let future_err_u8 = async { Err::<(), u8>(1) };
    /// let future_err_i32 = future_err_u8.err_into::<i32>();
    /// # });
    /// ```
    ///
    /// [`join!`]: crate::join
    /// [`select!`]: crate::select
    fn err_into<E>(self) -> ErrInto<Self, E>
    where
        Self: Sized,
        Self::Error: Into<E>,
    {
        assert_future::<Result<Self::Ok, E>, _>(ErrInto::new(self))
    }

    /// Maps this future's [`Ok`](TryFuture::Ok) to a new type
    /// using the [`Into`](std::convert::Into) trait.
    fn ok_into<U>(self) -> OkInto<Self, U>
    where
        Self: Sized,
        Self::Ok: Into<U>,
    {
        assert_future::<Result<U, Self::Error>, _>(OkInto::new(self))
    }

    /// Executes another future after this one resolves successfully. The
    /// success value is passed to a closure to create this subsequent future.
    ///
    /// The provided closure `f` will only be called if this future is resolved
    /// to an [`Ok`]. If this future resolves to an [`Err`], panics, or is
    /// dropped, then the provided closure will never be invoked. The
    /// [`Error`](TryFuture::Error) type of this future and the future
    /// returned by `f` have to match.
    ///
    /// Note that this method consumes the future it is called on and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::future::TryFutureExt;
    ///
    /// # futures::executor::block_on(async {
    /// let future = async { Ok::<i32, i32>(1) };
    /// let future = future.and_then(|x| async move { Ok::<i32, i32>(x + 3) });
    /// assert_eq!(future.await, Ok(4));
    /// # });
    /// ```
    ///
    /// Calling [`and_then`](TryFutureExt::and_then) on an errored future has no
    /// effect:
    ///
    /// ```
    /// use futures::future::TryFutureExt;
    ///
    /// # futures::executor::block_on(async {
    /// let future = async { Err::<i32, i32>(1) };
    /// let future = future.and_then(|x| async move { Err::<i32, i32>(x + 3) });
    /// assert_eq!(future.await, Err(1));
    /// # });
    /// ```
    fn and_then<Fut, F>(self, f: F) -> AndThen<Self, Fut, F>
    where
        F: FnOnce(Self::Ok) -> Fut,
        Fut: TryFuture<Error = Self::Error>,
        Self: Sized,
    {
        assert_future::<Result<Fut::Ok, Fut::Error>, _>(AndThen::new(self, f))
    }

    /// Executes another future if this one resolves to an error. The
    /// error value is passed to a closure to create this subsequent future.
    ///
    /// The provided closure `f` will only be called if this future is resolved
    /// to an [`Err`]. If this future resolves to an [`Ok`], panics, or is
    /// dropped, then the provided closure will never be invoked. The
    /// [`Ok`](TryFuture::Ok) type of this future and the future returned by `f`
    /// have to match.
    ///
    /// Note that this method consumes the future it is called on and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::future::TryFutureExt;
    ///
    /// # futures::executor::block_on(async {
    /// let future = async { Err::<i32, i32>(1) };
    /// let future = future.or_else(|x| async move { Err::<i32, i32>(x + 3) });
    /// assert_eq!(future.await, Err(4));
    /// # });
    /// ```
    ///
    /// Calling [`or_else`](TryFutureExt::or_else) on a successful future has
    /// no effect:
    ///
    /// ```
    /// use futures::future::TryFutureExt;
    ///
    /// # futures::executor::block_on(async {
    /// let future = async { Ok::<i32, i32>(1) };
    /// let future = future.or_else(|x| async move { Ok::<i32, i32>(x + 3) });
    /// assert_eq!(future.await, Ok(1));
    /// # });
    /// ```
    fn or_else<Fut, F>(self, f: F) -> OrElse<Self, Fut, F>
    where
        F: FnOnce(Self::Error) -> Fut,
        Fut: TryFuture<Ok = Self::Ok>,
        Self: Sized,
    {
        assert_future::<Result<Fut::Ok, Fut::Error>, _>(OrElse::new(self, f))
    }

    /// Do something with the success value of a future before passing it on.
    ///
    /// When using futures, you'll often chain several of them together.  While
    /// working on such code, you might want to check out what's happening at
    /// various parts in the pipeline, without consuming the intermediate
    /// value. To do that, insert a call to `inspect_ok`.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::TryFutureExt;
    ///
    /// let future = async { Ok::<_, ()>(1) };
    /// let new_future = future.inspect_ok(|&x| println!("about to resolve: {}", x));
    /// assert_eq!(new_future.await, Ok(1));
    /// # });
    /// ```
    fn inspect_ok<F>(self, f: F) -> InspectOk<Self, F>
    where
        F: FnOnce(&Self::Ok),
        Self: Sized,
    {
        assert_future::<Result<Self::Ok, Self::Error>, _>(InspectOk::new(self, f))
    }

    /// Do something with the error value of a future before passing it on.
    ///
    /// When using futures, you'll often chain several of them together.  While
    /// working on such code, you might want to check out what's happening at
    /// various parts in the pipeline, without consuming the intermediate
    /// value. To do that, insert a call to `inspect_err`.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::TryFutureExt;
    ///
    /// let future = async { Err::<(), _>(1) };
    /// let new_future = future.inspect_err(|&x| println!("about to error: {}", x));
    /// assert_eq!(new_future.await, Err(1));
    /// # });
    /// ```
    fn inspect_err<F>(self, f: F) -> InspectErr<Self, F>
    where
        F: FnOnce(&Self::Error),
        Self: Sized,
    {
        assert_future::<Result<Self::Ok, Self::Error>, _>(InspectErr::new(self, f))
    }

    /// Flatten the execution of this future when the successful result of this
    /// future is another future.
    ///
    /// This is equivalent to `future.and_then(|x| x)`.
    fn try_flatten(self) -> TryFlatten<Self, Self::Ok>
    where
        Self::Ok: TryFuture<Error = Self::Error>,
        Self: Sized,
    {
        assert_future::<Result<<Self::Ok as TryFuture>::Ok, Self::Error>, _>(TryFlatten::new(self))
    }

    /// Flatten the execution of this future when the successful result of this
    /// future is a stream.
    ///
    /// This can be useful when stream initialization is deferred, and it is
    /// convenient to work with that stream as if stream was available at the
    /// call site.
    ///
    /// Note that this function consumes this future and returns a wrapped
    /// version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::TryFutureExt;
    /// use futures::stream::{self, TryStreamExt};
    ///
    /// let stream_items = vec![17, 18, 19].into_iter().map(Ok);
    /// let future_of_a_stream = async { Ok::<_, ()>(stream::iter(stream_items)) };
    ///
    /// let stream = future_of_a_stream.try_flatten_stream();
    /// let list = stream.try_collect::<Vec<_>>().await;
    /// assert_eq!(list, Ok(vec![17, 18, 19]));
    /// # });
    /// ```
    fn try_flatten_stream(self) -> TryFlattenStream<Self>
    where
        Self::Ok: TryStream<Error = Self::Error>,
        Self: Sized,
    {
        assert_stream::<Result<<Self::Ok as TryStream>::Ok, Self::Error>, _>(TryFlattenStream::new(
            self,
        ))
    }

    /// Unwraps this future's output, producing a future with this future's
    /// [`Ok`](TryFuture::Ok) type as its
    /// [`Output`](std::future::Future::Output) type.
    ///
    /// If this future is resolved successfully, the returned future will
    /// contain the original future's success value as output. Otherwise, the
    /// closure `f` is called with the error value to produce an alternate
    /// success value.
    ///
    /// This method is similar to the [`Result::unwrap_or_else`] method.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::future::TryFutureExt;
    ///
    /// # futures::executor::block_on(async {
    /// let future = async { Err::<(), &str>("Boom!") };
    /// let future = future.unwrap_or_else(|_| ());
    /// assert_eq!(future.await, ());
    /// # });
    /// ```
    fn unwrap_or_else<F>(self, f: F) -> UnwrapOrElse<Self, F>
    where
        Self: Sized,
        F: FnOnce(Self::Error) -> Self::Ok,
    {
        assert_future::<Self::Ok, _>(UnwrapOrElse::new(self, f))
    }

    /// Wraps a [`TryFuture`] into a future compatible with libraries using
    /// futures 0.1 future definitions. Requires the `compat` feature to enable.
    #[cfg(feature = "compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "compat")))]
    fn compat(self) -> Compat<Self>
    where
        Self: Sized + Unpin,
    {
        Compat::new(self)
    }

    /// Wraps a [`TryFuture`] into a type that implements
    /// [`Future`](std::future::Future).
    ///
    /// [`TryFuture`]s currently do not implement the
    /// [`Future`](std::future::Future) trait due to limitations of the
    /// compiler.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::future::{Future, TryFuture, TryFutureExt};
    ///
    /// # type T = i32;
    /// # type E = ();
    /// fn make_try_future() -> impl TryFuture<Ok = T, Error = E> { // ... }
    /// # async { Ok::<i32, ()>(1) }
    /// # }
    /// fn take_future(future: impl Future<Output = Result<T, E>>) { /* ... */ }
    ///
    /// take_future(make_try_future().into_future());
    /// ```
    fn into_future(self) -> IntoFuture<Self>
    where
        Self: Sized,
    {
        assert_future::<Result<Self::Ok, Self::Error>, _>(IntoFuture::new(self))
    }

    /// A convenience method for calling [`TryFuture::try_poll`] on [`Unpin`]
    /// future types.
    fn try_poll_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Result<Self::Ok, Self::Error>>
    where
        Self: Unpin,
    {
        Pin::new(self).try_poll(cx)
    }
}
