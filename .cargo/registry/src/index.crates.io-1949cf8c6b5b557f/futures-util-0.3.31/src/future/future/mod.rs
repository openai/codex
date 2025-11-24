//! Futures
//!
//! This module contains a number of functions for working with `Future`s,
//! including the `FutureExt` trait which adds methods to `Future` types.

#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use core::pin::Pin;

use crate::fns::{inspect_fn, into_fn, ok_fn, InspectFn, IntoFn, OkFn};
use crate::future::{assert_future, Either};
use crate::never::Never;
use crate::stream::assert_stream;
#[cfg(feature = "alloc")]
use futures_core::future::{BoxFuture, LocalBoxFuture};
use futures_core::{
    future::Future,
    stream::Stream,
    task::{Context, Poll},
};
use pin_utils::pin_mut;

// Combinators

mod flatten;
mod fuse;
mod map;

delegate_all!(
    /// Future for the [`flatten`](super::FutureExt::flatten) method.
    Flatten<F>(
        flatten::Flatten<F, <F as Future>::Output>
    ): Debug + Future + FusedFuture + New[|x: F| flatten::Flatten::new(x)]
    where F: Future
);

delegate_all!(
    /// Stream for the [`flatten_stream`](FutureExt::flatten_stream) method.
    FlattenStream<F>(
        flatten::Flatten<F, <F as Future>::Output>
    ): Debug + Sink + Stream + FusedStream + New[|x: F| flatten::Flatten::new(x)]
    where F: Future
);

#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use fuse::Fuse;

delegate_all!(
    /// Future for the [`map`](super::FutureExt::map) method.
    Map<Fut, F>(
        map::Map<Fut, F>
    ): Debug + Future + FusedFuture + New[|x: Fut, f: F| map::Map::new(x, f)]
);

delegate_all!(
    /// Stream for the [`into_stream`](FutureExt::into_stream) method.
    IntoStream<F>(
        crate::stream::Once<F>
    ): Debug + Stream + FusedStream + New[|x: F| crate::stream::Once::new(x)]
);

delegate_all!(
    /// Future for the [`map_into`](FutureExt::map_into) combinator.
    MapInto<Fut, T>(
        Map<Fut, IntoFn<T>>
    ): Debug + Future + FusedFuture + New[|x: Fut| Map::new(x, into_fn())]
);

delegate_all!(
    /// Future for the [`then`](FutureExt::then) method.
    Then<Fut1, Fut2, F>(
        flatten::Flatten<Map<Fut1, F>, Fut2>
    ): Debug + Future + FusedFuture + New[|x: Fut1, y: F| flatten::Flatten::new(Map::new(x, y))]
);

delegate_all!(
    /// Future for the [`inspect`](FutureExt::inspect) method.
    Inspect<Fut, F>(
        map::Map<Fut, InspectFn<F>>
    ): Debug + Future + FusedFuture + New[|x: Fut, f: F| map::Map::new(x, inspect_fn(f))]
);

delegate_all!(
    /// Future for the [`never_error`](super::FutureExt::never_error) combinator.
    NeverError<Fut>(
        Map<Fut, OkFn<Never>>
    ): Debug + Future + FusedFuture + New[|x: Fut| Map::new(x, ok_fn())]
);

delegate_all!(
    /// Future for the [`unit_error`](super::FutureExt::unit_error) combinator.
    UnitError<Fut>(
        Map<Fut, OkFn<()>>
    ): Debug + Future + FusedFuture + New[|x: Fut| Map::new(x, ok_fn())]
);

#[cfg(feature = "std")]
mod catch_unwind;
#[cfg(feature = "std")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::catch_unwind::CatchUnwind;

#[cfg(feature = "channel")]
#[cfg_attr(docsrs, doc(cfg(feature = "channel")))]
#[cfg(feature = "std")]
mod remote_handle;
#[cfg(feature = "channel")]
#[cfg_attr(docsrs, doc(cfg(feature = "channel")))]
#[cfg(feature = "std")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::remote_handle::{Remote, RemoteHandle};

#[cfg(feature = "std")]
mod shared;
#[cfg(feature = "std")]
#[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
pub use self::shared::{Shared, WeakShared};

impl<T: ?Sized> FutureExt for T where T: Future {}

/// An extension trait for `Future`s that provides a variety of convenient
/// adapters.
pub trait FutureExt: Future {
    /// Map this future's output to a different type, returning a new future of
    /// the resulting type.
    ///
    /// This function is similar to the `Option::map` or `Iterator::map` where
    /// it will change the type of the underlying future. This is useful to
    /// chain along a computation once a future has been resolved.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it, similar to the existing `map` methods in the
    /// standard library.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::FutureExt;
    ///
    /// let future = async { 1 };
    /// let new_future = future.map(|x| x + 3);
    /// assert_eq!(new_future.await, 4);
    /// # });
    /// ```
    fn map<U, F>(self, f: F) -> Map<Self, F>
    where
        F: FnOnce(Self::Output) -> U,
        Self: Sized,
    {
        assert_future::<U, _>(Map::new(self, f))
    }

    /// Map this future's output to a different type, returning a new future of
    /// the resulting type.
    ///
    /// This function is equivalent to calling `map(Into::into)` but allows naming
    /// the return type.
    fn map_into<U>(self) -> MapInto<Self, U>
    where
        Self::Output: Into<U>,
        Self: Sized,
    {
        assert_future::<U, _>(MapInto::new(self))
    }

    /// Chain on a computation for when a future finished, passing the result of
    /// the future to the provided closure `f`.
    ///
    /// The returned value of the closure must implement the `Future` trait
    /// and can represent some more work to be done before the composed future
    /// is finished.
    ///
    /// The closure `f` is only run *after* successful completion of the `self`
    /// future.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::FutureExt;
    ///
    /// let future_of_1 = async { 1 };
    /// let future_of_4 = future_of_1.then(|x| async move { x + 3 });
    /// assert_eq!(future_of_4.await, 4);
    /// # });
    /// ```
    fn then<Fut, F>(self, f: F) -> Then<Self, Fut, F>
    where
        F: FnOnce(Self::Output) -> Fut,
        Fut: Future,
        Self: Sized,
    {
        assert_future::<Fut::Output, _>(Then::new(self, f))
    }

    /// Wrap this future in an `Either` future, making it the left-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `right_future` method to write `if`
    /// statements that evaluate to different futures in different branches.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::FutureExt;
    ///
    /// let x = 6;
    /// let future = if x < 10 {
    ///     async { true }.left_future()
    /// } else {
    ///     async { false }.right_future()
    /// };
    ///
    /// assert_eq!(future.await, true);
    /// # });
    /// ```
    fn left_future<B>(self) -> Either<Self, B>
    where
        B: Future<Output = Self::Output>,
        Self: Sized,
    {
        assert_future::<Self::Output, _>(Either::Left(self))
    }

    /// Wrap this future in an `Either` future, making it the right-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `left_future` method to write `if`
    /// statements that evaluate to different futures in different branches.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::FutureExt;
    ///
    /// let x = 6;
    /// let future = if x > 10 {
    ///     async { true }.left_future()
    /// } else {
    ///     async { false }.right_future()
    /// };
    ///
    /// assert_eq!(future.await, false);
    /// # });
    /// ```
    fn right_future<A>(self) -> Either<A, Self>
    where
        A: Future<Output = Self::Output>,
        Self: Sized,
    {
        assert_future::<Self::Output, _>(Either::Right(self))
    }

    /// Convert this future into a single element stream.
    ///
    /// The returned stream contains single success if this future resolves to
    /// success or single error if this future resolves into error.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::FutureExt;
    /// use futures::stream::StreamExt;
    ///
    /// let future = async { 17 };
    /// let stream = future.into_stream();
    /// let collected: Vec<_> = stream.collect().await;
    /// assert_eq!(collected, vec![17]);
    /// # });
    /// ```
    fn into_stream(self) -> IntoStream<Self>
    where
        Self: Sized,
    {
        assert_stream::<Self::Output, _>(IntoStream::new(self))
    }

    /// Flatten the execution of this future when the output of this
    /// future is itself another future.
    ///
    /// This can be useful when combining futures together to flatten the
    /// computation out the final result.
    ///
    /// This method is roughly equivalent to `self.then(|x| x)`.
    ///
    /// Note that this function consumes the receiving future and returns a
    /// wrapped version of it.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::FutureExt;
    ///
    /// let nested_future = async { async { 1 } };
    /// let future = nested_future.flatten();
    /// assert_eq!(future.await, 1);
    /// # });
    /// ```
    fn flatten(self) -> Flatten<Self>
    where
        Self::Output: Future,
        Self: Sized,
    {
        let f = Flatten::new(self);
        assert_future::<<<Self as Future>::Output as Future>::Output, _>(f)
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
    /// use futures::future::FutureExt;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream_items = vec![17, 18, 19];
    /// let future_of_a_stream = async { stream::iter(stream_items) };
    ///
    /// let stream = future_of_a_stream.flatten_stream();
    /// let list: Vec<_> = stream.collect().await;
    /// assert_eq!(list, vec![17, 18, 19]);
    /// # });
    /// ```
    fn flatten_stream(self) -> FlattenStream<Self>
    where
        Self::Output: Stream,
        Self: Sized,
    {
        assert_stream::<<Self::Output as Stream>::Item, _>(FlattenStream::new(self))
    }

    /// Fuse a future such that `poll` will never again be called once it has
    /// completed. This method can be used to turn any `Future` into a
    /// `FusedFuture`.
    ///
    /// Normally, once a future has returned `Poll::Ready` from `poll`,
    /// any further calls could exhibit bad behavior such as blocking
    /// forever, panicking, never returning, etc. If it is known that `poll`
    /// may be called too often then this method can be used to ensure that it
    /// has defined semantics.
    ///
    /// If a `fuse`d future is `poll`ed after having returned `Poll::Ready`
    /// previously, it will return `Poll::Pending`, from `poll` again (and will
    /// continue to do so for all future calls to `poll`).
    ///
    /// This combinator will drop the underlying future as soon as it has been
    /// completed to ensure resources are reclaimed as soon as possible.
    fn fuse(self) -> Fuse<Self>
    where
        Self: Sized,
    {
        let f = Fuse::new(self);
        assert_future::<Self::Output, _>(f)
    }

    /// Do something with the output of a future before passing it on.
    ///
    /// When using futures, you'll often chain several of them together.  While
    /// working on such code, you might want to check out what's happening at
    /// various parts in the pipeline, without consuming the intermediate
    /// value. To do that, insert a call to `inspect`.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::FutureExt;
    ///
    /// let future = async { 1 };
    /// let new_future = future.inspect(|&x| println!("about to resolve: {}", x));
    /// assert_eq!(new_future.await, 1);
    /// # });
    /// ```
    fn inspect<F>(self, f: F) -> Inspect<Self, F>
    where
        F: FnOnce(&Self::Output),
        Self: Sized,
    {
        assert_future::<Self::Output, _>(Inspect::new(self, f))
    }

    /// Catches unwinding panics while polling the future.
    ///
    /// In general, panics within a future can propagate all the way out to the
    /// task level. This combinator makes it possible to halt unwinding within
    /// the future itself. It's most commonly used within task executors. It's
    /// not recommended to use this for error handling.
    ///
    /// Note that this method requires the `UnwindSafe` bound from the standard
    /// library. This isn't always applied automatically, and the standard
    /// library provides an `AssertUnwindSafe` wrapper type to apply it
    /// after-the fact. To assist using this method, the `Future` trait is also
    /// implemented for `AssertUnwindSafe<F>` where `F` implements `Future`.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::{self, FutureExt, Ready};
    ///
    /// let future = future::ready(2);
    /// assert!(future.catch_unwind().await.is_ok());
    ///
    /// let future = future::lazy(|_| -> Ready<i32> {
    ///     unimplemented!()
    /// });
    /// assert!(future.catch_unwind().await.is_err());
    /// # });
    /// ```
    #[cfg(feature = "std")]
    fn catch_unwind(self) -> CatchUnwind<Self>
    where
        Self: Sized + ::std::panic::UnwindSafe,
    {
        assert_future::<Result<Self::Output, Box<dyn std::any::Any + Send>>, _>(CatchUnwind::new(
            self,
        ))
    }

    /// Create a cloneable handle to this future where all handles will resolve
    /// to the same result.
    ///
    /// The `shared` combinator method provides a method to convert any future
    /// into a cloneable future. It enables a future to be polled by multiple
    /// threads.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::FutureExt;
    ///
    /// let future = async { 6 };
    /// let shared1 = future.shared();
    /// let shared2 = shared1.clone();
    ///
    /// assert_eq!(6, shared1.await);
    /// assert_eq!(6, shared2.await);
    /// # });
    /// ```
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::future::FutureExt;
    /// use futures::executor::block_on;
    /// use std::thread;
    ///
    /// let future = async { 6 };
    /// let shared1 = future.shared();
    /// let shared2 = shared1.clone();
    /// let join_handle = thread::spawn(move || {
    ///     assert_eq!(6, block_on(shared2));
    /// });
    /// assert_eq!(6, shared1.await);
    /// join_handle.join().unwrap();
    /// # });
    /// ```
    #[cfg(feature = "std")]
    fn shared(self) -> Shared<Self>
    where
        Self: Sized,
        Self::Output: Clone,
    {
        assert_future::<Self::Output, _>(Shared::new(self))
    }

    /// Turn this future into a future that yields `()` on completion and sends
    /// its output to another future on a separate task.
    ///
    /// This can be used with spawning executors to easily retrieve the result
    /// of a future executing on a separate task or thread.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    #[cfg(feature = "channel")]
    #[cfg_attr(docsrs, doc(cfg(feature = "channel")))]
    #[cfg(feature = "std")]
    fn remote_handle(self) -> (Remote<Self>, RemoteHandle<Self::Output>)
    where
        Self: Sized,
    {
        let (wrapped, handle) = remote_handle::remote_handle(self);
        (assert_future::<(), _>(wrapped), handle)
    }

    /// Wrap the future in a Box, pinning it.
    ///
    /// This method is only available when the `std` or `alloc` feature of this
    /// library is activated, and it is activated by default.
    #[cfg(feature = "alloc")]
    fn boxed<'a>(self) -> BoxFuture<'a, Self::Output>
    where
        Self: Sized + Send + 'a,
    {
        assert_future::<Self::Output, _>(Box::pin(self))
    }

    /// Wrap the future in a Box, pinning it.
    ///
    /// Similar to `boxed`, but without the `Send` requirement.
    ///
    /// This method is only available when the `std` or `alloc` feature of this
    /// library is activated, and it is activated by default.
    #[cfg(feature = "alloc")]
    fn boxed_local<'a>(self) -> LocalBoxFuture<'a, Self::Output>
    where
        Self: Sized + 'a,
    {
        assert_future::<Self::Output, _>(Box::pin(self))
    }

    /// Turns a [`Future<Output = T>`](Future) into a
    /// [`TryFuture<Ok = T, Error = ()`>](futures_core::future::TryFuture).
    fn unit_error(self) -> UnitError<Self>
    where
        Self: Sized,
    {
        assert_future::<Result<Self::Output, ()>, _>(UnitError::new(self))
    }

    /// Turns a [`Future<Output = T>`](Future) into a
    /// [`TryFuture<Ok = T, Error = Never`>](futures_core::future::TryFuture).
    fn never_error(self) -> NeverError<Self>
    where
        Self: Sized,
    {
        assert_future::<Result<Self::Output, Never>, _>(NeverError::new(self))
    }

    /// A convenience for calling `Future::poll` on `Unpin` future types.
    fn poll_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Self::Output>
    where
        Self: Unpin,
    {
        Pin::new(self).poll(cx)
    }

    /// Evaluates and consumes the future, returning the resulting output if
    /// the future is ready after the first call to `Future::poll`.
    ///
    /// If `poll` instead returns `Poll::Pending`, `None` is returned.
    ///
    /// This method is useful in cases where immediacy is more important than
    /// waiting for a result. It is also convenient for quickly obtaining
    /// the value of a future that is known to always resolve immediately.
    ///
    /// # Examples
    ///
    /// ```
    /// # use futures::prelude::*;
    /// use futures::{future::ready, future::pending};
    /// let future_ready = ready("foobar");
    /// let future_pending = pending::<&'static str>();
    ///
    /// assert_eq!(future_ready.now_or_never(), Some("foobar"));
    /// assert_eq!(future_pending.now_or_never(), None);
    /// ```
    ///
    /// In cases where it is absolutely known that a future should always
    /// resolve immediately and never return `Poll::Pending`, this method can
    /// be combined with `expect()`:
    ///
    /// ```
    /// # use futures::{prelude::*, future::ready};
    /// let future_ready = ready("foobar");
    ///
    /// assert_eq!(future_ready.now_or_never().expect("Future not ready"), "foobar");
    /// ```
    fn now_or_never(self) -> Option<Self::Output>
    where
        Self: Sized,
    {
        let noop_waker = crate::task::noop_waker();
        let mut cx = Context::from_waker(&noop_waker);

        let this = self;
        pin_mut!(this);
        match this.poll(&mut cx) {
            Poll::Ready(x) => Some(x),
            _ => None,
        }
    }
}
