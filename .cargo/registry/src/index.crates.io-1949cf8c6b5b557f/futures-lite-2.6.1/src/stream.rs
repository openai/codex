//! Combinators for the [`Stream`] trait.
//!
//! # Examples
//!
//! ```
//! use futures_lite::stream::{self, StreamExt};
//!
//! # spin_on::spin_on(async {
//! let mut s = stream::iter(vec![1, 2, 3]);
//!
//! assert_eq!(s.next().await, Some(1));
//! assert_eq!(s.next().await, Some(2));
//! assert_eq!(s.next().await, Some(3));
//! assert_eq!(s.next().await, None);
//! # });
//! ```

#[doc(no_inline)]
pub use futures_core::stream::Stream;

#[cfg(feature = "alloc")]
use alloc::boxed::Box;

use core::fmt;
use core::future::Future;
use core::marker::PhantomData;
use core::mem;
use core::pin::Pin;
use core::task::{Context, Poll};

#[cfg(feature = "race")]
use fastrand::Rng;

use pin_project_lite::pin_project;

use crate::ready;

/// Converts a stream into a blocking iterator.
///
/// # Examples
///
/// ```
/// use futures_lite::{pin, stream};
///
/// let stream = stream::once(7);
/// pin!(stream);
///
/// let mut iter = stream::block_on(stream);
/// assert_eq!(iter.next(), Some(7));
/// assert_eq!(iter.next(), None);
/// ```
#[cfg(feature = "std")]
pub fn block_on<S: Stream + Unpin>(stream: S) -> BlockOn<S> {
    BlockOn(stream)
}

/// Iterator for the [`block_on()`] function.
#[derive(Debug)]
pub struct BlockOn<S>(S);

#[cfg(feature = "std")]
impl<S: Stream + Unpin> Iterator for BlockOn<S> {
    type Item = S::Item;

    fn next(&mut self) -> Option<Self::Item> {
        crate::future::block_on(self.0.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    fn count(self) -> usize {
        crate::future::block_on(self.0.count())
    }

    fn last(self) -> Option<Self::Item> {
        crate::future::block_on(self.0.last())
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        crate::future::block_on(self.0.nth(n))
    }

    fn fold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        crate::future::block_on(self.0.fold(init, f))
    }

    fn for_each<F>(self, f: F) -> F::Output
    where
        F: FnMut(Self::Item),
    {
        crate::future::block_on(self.0.for_each(f))
    }

    fn all<F>(&mut self, f: F) -> bool
    where
        F: FnMut(Self::Item) -> bool,
    {
        crate::future::block_on(self.0.all(f))
    }

    fn any<F>(&mut self, f: F) -> bool
    where
        F: FnMut(Self::Item) -> bool,
    {
        crate::future::block_on(self.0.any(f))
    }

    fn find<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        crate::future::block_on(self.0.find(predicate))
    }

    fn find_map<B, F>(&mut self, f: F) -> Option<B>
    where
        F: FnMut(Self::Item) -> Option<B>,
    {
        crate::future::block_on(self.0.find_map(f))
    }

    fn position<P>(&mut self, predicate: P) -> Option<usize>
    where
        P: FnMut(Self::Item) -> bool,
    {
        crate::future::block_on(self.0.position(predicate))
    }
}

/// Creates an empty stream.
///
/// # Examples
///
/// ```
/// use futures_lite::stream::{self, StreamExt};
///
/// # spin_on::spin_on(async {
/// let mut s = stream::empty::<i32>();
/// assert_eq!(s.next().await, None);
/// # })
/// ```
pub fn empty<T>() -> Empty<T> {
    Empty {
        _marker: PhantomData,
    }
}

/// Stream for the [`empty()`] function.
#[derive(Clone, Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Empty<T> {
    _marker: PhantomData<T>,
}

impl<T> Unpin for Empty<T> {}

impl<T> Stream for Empty<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(None)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(0))
    }
}

/// Creates a stream from an iterator.
///
/// # Examples
///
/// ```
/// use futures_lite::stream::{self, StreamExt};
///
/// # spin_on::spin_on(async {
/// let mut s = stream::iter(vec![1, 2]);
///
/// assert_eq!(s.next().await, Some(1));
/// assert_eq!(s.next().await, Some(2));
/// assert_eq!(s.next().await, None);
/// # })
/// ```
pub fn iter<I: IntoIterator>(iter: I) -> Iter<I::IntoIter> {
    Iter {
        iter: iter.into_iter(),
    }
}

/// Stream for the [`iter()`] function.
#[derive(Clone, Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Iter<I> {
    iter: I,
}

impl<I> Unpin for Iter<I> {}

impl<I: Iterator> Stream for Iter<I> {
    type Item = I::Item;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.iter.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// Creates a stream that yields a single item.
///
/// # Examples
///
/// ```
/// use futures_lite::stream::{self, StreamExt};
///
/// # spin_on::spin_on(async {
/// let mut s = stream::once(7);
///
/// assert_eq!(s.next().await, Some(7));
/// assert_eq!(s.next().await, None);
/// # })
/// ```
pub fn once<T>(t: T) -> Once<T> {
    Once { value: Some(t) }
}

pin_project! {
    /// Stream for the [`once()`] function.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Once<T> {
        value: Option<T>,
    }
}

impl<T> Stream for Once<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<T>> {
        Poll::Ready(self.project().value.take())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.value.is_some() {
            (1, Some(1))
        } else {
            (0, Some(0))
        }
    }
}

/// Creates a stream that is always pending.
///
/// # Examples
///
/// ```no_run
/// use futures_lite::stream::{self, StreamExt};
///
/// # spin_on::spin_on(async {
/// let mut s = stream::pending::<i32>();
/// s.next().await;
/// unreachable!();
/// # })
/// ```
pub fn pending<T>() -> Pending<T> {
    Pending {
        _marker: PhantomData,
    }
}

/// Stream for the [`pending()`] function.
#[derive(Clone, Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Pending<T> {
    _marker: PhantomData<T>,
}

impl<T> Unpin for Pending<T> {}

impl<T> Stream for Pending<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<T>> {
        Poll::Pending
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(0))
    }
}

/// Creates a stream from a function returning [`Poll`].
///
/// # Examples
///
/// ```
/// use futures_lite::stream::{self, StreamExt};
/// use std::task::{Context, Poll};
///
/// # spin_on::spin_on(async {
/// fn f(_: &mut Context<'_>) -> Poll<Option<i32>> {
///     Poll::Ready(Some(7))
/// }
///
/// assert_eq!(stream::poll_fn(f).next().await, Some(7));
/// # })
/// ```
pub fn poll_fn<T, F>(f: F) -> PollFn<F>
where
    F: FnMut(&mut Context<'_>) -> Poll<Option<T>>,
{
    PollFn { f }
}

/// Stream for the [`poll_fn()`] function.
#[derive(Clone)]
#[must_use = "streams do nothing unless polled"]
pub struct PollFn<F> {
    f: F,
}

impl<F> Unpin for PollFn<F> {}

impl<F> fmt::Debug for PollFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PollFn").finish()
    }
}

impl<T, F> Stream for PollFn<F>
where
    F: FnMut(&mut Context<'_>) -> Poll<Option<T>>,
{
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        (&mut self.f)(cx)
    }
}

/// Creates an infinite stream that yields the same item repeatedly.
///
/// # Examples
///
/// ```
/// use futures_lite::stream::{self, StreamExt};
///
/// # spin_on::spin_on(async {
/// let mut s = stream::repeat(7);
///
/// assert_eq!(s.next().await, Some(7));
/// assert_eq!(s.next().await, Some(7));
/// # })
/// ```
pub fn repeat<T: Clone>(item: T) -> Repeat<T> {
    Repeat { item }
}

/// Stream for the [`repeat()`] function.
#[derive(Clone, Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Repeat<T> {
    item: T,
}

impl<T> Unpin for Repeat<T> {}

impl<T: Clone> Stream for Repeat<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(Some(self.item.clone()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

/// Creates an infinite stream from a closure that generates items.
///
/// # Examples
///
/// ```
/// use futures_lite::stream::{self, StreamExt};
///
/// # spin_on::spin_on(async {
/// let mut s = stream::repeat_with(|| 7);
///
/// assert_eq!(s.next().await, Some(7));
/// assert_eq!(s.next().await, Some(7));
/// # })
/// ```
pub fn repeat_with<T, F>(repeater: F) -> RepeatWith<F>
where
    F: FnMut() -> T,
{
    RepeatWith { f: repeater }
}

/// Stream for the [`repeat_with()`] function.
#[derive(Clone, Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct RepeatWith<F> {
    f: F,
}

impl<F> Unpin for RepeatWith<F> {}

impl<T, F> Stream for RepeatWith<F>
where
    F: FnMut() -> T,
{
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let item = (&mut self.f)();
        Poll::Ready(Some(item))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

/// Creates a stream from a seed value and an async closure operating on it.
///
/// # Examples
///
/// ```
/// use futures_lite::stream::{self, StreamExt};
///
/// # spin_on::spin_on(async {
/// let s = stream::unfold(0, |mut n| async move {
///     if n < 2 {
///         let m = n + 1;
///         Some((n, m))
///     } else {
///         None
///     }
/// });
///
/// let v: Vec<i32> = s.collect().await;
/// assert_eq!(v, [0, 1]);
/// # })
/// ```
pub fn unfold<T, F, Fut, Item>(seed: T, f: F) -> Unfold<T, F, Fut>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Option<(Item, T)>>,
{
    Unfold {
        f,
        state: Some(seed),
        fut: None,
    }
}

pin_project! {
    /// Stream for the [`unfold()`] function.
    #[derive(Clone)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Unfold<T, F, Fut> {
        f: F,
        state: Option<T>,
        #[pin]
        fut: Option<Fut>,
    }
}

impl<T, F, Fut> fmt::Debug for Unfold<T, F, Fut>
where
    T: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Unfold")
            .field("state", &self.state)
            .field("fut", &self.fut)
            .finish()
    }
}

impl<T, F, Fut, Item> Stream for Unfold<T, F, Fut>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Option<(Item, T)>>,
{
    type Item = Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if let Some(state) = this.state.take() {
            this.fut.set(Some((this.f)(state)));
        }

        let step = ready!(this
            .fut
            .as_mut()
            .as_pin_mut()
            .expect("`Unfold` must not be polled after it returned `Poll::Ready(None)`")
            .poll(cx));
        this.fut.set(None);

        if let Some((item, next_state)) = step {
            *this.state = Some(next_state);
            Poll::Ready(Some(item))
        } else {
            Poll::Ready(None)
        }
    }
}

/// Creates a stream from a seed value and a fallible async closure operating on it.
///
/// # Examples
///
/// ```
/// use futures_lite::stream::{self, StreamExt};
///
/// # spin_on::spin_on(async {
/// let s = stream::try_unfold(0, |mut n| async move {
///     if n < 2 {
///         let m = n + 1;
///         Ok(Some((n, m)))
///     } else {
///         std::io::Result::Ok(None)
///     }
/// });
///
/// let v: Vec<i32> = s.try_collect().await?;
/// assert_eq!(v, [0, 1]);
/// # std::io::Result::Ok(()) });
/// ```
pub fn try_unfold<T, E, F, Fut, Item>(init: T, f: F) -> TryUnfold<T, F, Fut>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Result<Option<(Item, T)>, E>>,
{
    TryUnfold {
        f,
        state: Some(init),
        fut: None,
    }
}

pin_project! {
    /// Stream for the [`try_unfold()`] function.
    #[derive(Clone)]
    #[must_use = "streams do nothing unless polled"]
    pub struct TryUnfold<T, F, Fut> {
        f: F,
        state: Option<T>,
        #[pin]
        fut: Option<Fut>,
    }
}

impl<T, F, Fut> fmt::Debug for TryUnfold<T, F, Fut>
where
    T: fmt::Debug,
    Fut: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TryUnfold")
            .field("state", &self.state)
            .field("fut", &self.fut)
            .finish()
    }
}

impl<T, E, F, Fut, Item> Stream for TryUnfold<T, F, Fut>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = Result<Option<(Item, T)>, E>>,
{
    type Item = Result<Item, E>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if let Some(state) = this.state.take() {
            this.fut.set(Some((this.f)(state)));
        }

        match this.fut.as_mut().as_pin_mut() {
            None => {
                // The future previously errored
                Poll::Ready(None)
            }
            Some(future) => {
                let step = ready!(future.poll(cx));
                this.fut.set(None);

                match step {
                    Ok(Some((item, next_state))) => {
                        *this.state = Some(next_state);
                        Poll::Ready(Some(Ok(item)))
                    }
                    Ok(None) => Poll::Ready(None),
                    Err(e) => Poll::Ready(Some(Err(e))),
                }
            }
        }
    }
}

/// Creates a stream that invokes the given future as its first item, and then
/// produces no more items.
///
/// # Example
///
/// ```
/// use futures_lite::{stream, prelude::*};
///
/// # spin_on::spin_on(async {
/// let mut stream = Box::pin(stream::once_future(async { 1 }));
/// assert_eq!(stream.next().await, Some(1));
/// assert_eq!(stream.next().await, None);
/// # });
/// ```
pub fn once_future<F: Future>(future: F) -> OnceFuture<F> {
    OnceFuture {
        future: Some(future),
    }
}

pin_project! {
    /// Stream for the [`once_future()`] function.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct OnceFuture<F> {
        #[pin]
        future: Option<F>,
    }
}

impl<F: Future> Stream for OnceFuture<F> {
    type Item = F::Output;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        match this.future.as_mut().as_pin_mut().map(|f| f.poll(cx)) {
            Some(Poll::Ready(t)) => {
                this.future.set(None);
                Poll::Ready(Some(t))
            }
            Some(Poll::Pending) => Poll::Pending,
            None => Poll::Ready(None),
        }
    }
}

/// Take elements from this stream until the provided future resolves.
///
/// This function will take elements from the stream until the provided
/// stopping future `fut` resolves. Once the `fut` future becomes ready,
/// this stream combinator will always return that the stream is done.
///
/// The stopping future may return any type. Once the stream is stopped
/// the result of the stopping future may be accessed with `StopAfterFuture::take_result()`.
/// The stream may also be resumed with `StopAfterFuture::take_future()`.
/// See the documentation of [`StopAfterFuture`] for more information.
///
/// ```
/// use futures_lite::stream::{self, StreamExt, stop_after_future};
/// use futures_lite::future;
/// use std::task::Poll;
///
/// let stream = stream::iter(1..=10);
///
/// # spin_on::spin_on(async {
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
/// let stream = stop_after_future(stream, stop_fut);
///
/// assert_eq!(vec![1, 2, 3, 4, 5], stream.collect::<Vec<_>>().await);
/// # });
pub fn stop_after_future<S, F>(stream: S, future: F) -> StopAfterFuture<S, F>
where
    S: Sized + Stream,
    F: Future,
{
    StopAfterFuture {
        stream,
        fut: Some(future),
        fut_result: None,
        free: false,
    }
}

pin_project! {
    /// Stream for the [`stop_after_future()`] function.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct StopAfterFuture<S: Stream, Fut: Future> {
        #[pin]
        stream: S,
        // Contains the inner Future on start and None once the inner Future is resolved
        // or taken out by the user.
        #[pin]
        fut: Option<Fut>,
        // Contains fut's return value once fut is resolved
        fut_result: Option<Fut::Output>,
        // Whether the future was taken out by the user.
        free: bool,
    }
}

impl<St, Fut> StopAfterFuture<St, Fut>
where
    St: Stream,
    Fut: Future,
{
    /// Extract the stopping future out of the combinator.
    ///
    /// The future is returned only if it isn't resolved yet, ie. if the stream isn't stopped yet.
    /// Taking out the future means the combinator will be yielding
    /// elements from the wrapped stream without ever stopping it.
    pub fn take_future(&mut self) -> Option<Fut> {
        if self.fut.is_some() {
            self.free = true;
        }

        self.fut.take()
    }

    /// Once the stopping future is resolved, this method can be used
    /// to extract the value returned by the stopping future.
    ///
    /// This may be used to retrieve arbitrary data from the stopping
    /// future, for example a reason why the stream was stopped.
    ///
    /// This method will return `None` if the future isn't resolved yet,
    /// or if the result was already taken out.
    ///
    /// # Examples
    ///
    /// ```
    /// # spin_on::spin_on(async {
    /// use futures_lite::stream::{self, StreamExt, stop_after_future};
    /// use futures_lite::future;
    /// use std::task::Poll;
    ///
    /// let stream = stream::iter(1..=10);
    ///
    /// let mut i = 0;
    /// let stop_fut = future::poll_fn(|_cx| {
    ///     i += 1;
    ///     if i <= 5 {
    ///         Poll::Pending
    ///     } else {
    ///         Poll::Ready("reason")
    ///     }
    /// });
    ///
    /// let mut stream = stop_after_future(stream, stop_fut);
    /// let _ = (&mut stream).collect::<Vec<_>>().await;
    ///
    /// let result = stream.take_result().unwrap();
    /// assert_eq!(result, "reason");
    /// # });
    /// ```
    pub fn take_result(&mut self) -> Option<Fut::Output> {
        self.fut_result.take()
    }

    /// Whether the stream was stopped yet by the stopping future
    /// being resolved.
    pub fn is_stopped(&self) -> bool {
        !self.free && self.fut.is_none()
    }
}

impl<St, Fut> Stream for StopAfterFuture<St, Fut>
where
    St: Stream,
    Fut: Future,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<St::Item>> {
        let mut this = self.project();

        if let Some(f) = this.fut.as_mut().as_pin_mut() {
            if let Poll::Ready(result) = f.poll(cx) {
                this.fut.set(None);
                *this.fut_result = Some(result);
            }
        }

        if !*this.free && this.fut.is_none() {
            // Future resolved, inner stream stopped
            Poll::Ready(None)
        } else {
            // Future either not resolved yet or taken out by the user
            let item = ready!(this.stream.poll_next(cx));
            if item.is_none() {
                this.fut.set(None);
            }
            Poll::Ready(item)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.is_stopped() {
            return (0, Some(0));
        }

        // Original stream can be truncated at any moment, so the lower bound isn't reliable.
        let (_, upper_bound) = self.stream.size_hint();
        (0, upper_bound)
    }
}

/// Extension trait for [`Stream`].
pub trait StreamExt: Stream {
    /// A convenience for calling [`Stream::poll_next()`] on `!`[`Unpin`] types.
    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>
    where
        Self: Unpin,
    {
        Stream::poll_next(Pin::new(self), cx)
    }

    /// Retrieves the next item in the stream.
    ///
    /// Returns [`None`] when iteration is finished. Stream implementations may choose to or not to
    /// resume iteration after that.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(1..=3);
    ///
    /// assert_eq!(s.next().await, Some(1));
    /// assert_eq!(s.next().await, Some(2));
    /// assert_eq!(s.next().await, Some(3));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn next(&mut self) -> NextFuture<'_, Self>
    where
        Self: Unpin,
    {
        NextFuture { stream: self }
    }

    /// Retrieves the next item in the stream.
    ///
    /// This is similar to the [`next()`][`StreamExt::next()`] method, but returns
    /// `Result<Option<T>, E>` rather than `Option<Result<T, E>>`.
    ///
    /// Note that `s.try_next().await` is equivalent to `s.next().await.transpose()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(vec![Ok(1), Ok(2), Err("error")]);
    ///
    /// assert_eq!(s.try_next().await, Ok(Some(1)));
    /// assert_eq!(s.try_next().await, Ok(Some(2)));
    /// assert_eq!(s.try_next().await, Err("error"));
    /// assert_eq!(s.try_next().await, Ok(None));
    /// # });
    /// ```
    fn try_next<T, E>(&mut self) -> TryNextFuture<'_, Self>
    where
        Self: Stream<Item = Result<T, E>> + Unpin,
    {
        TryNextFuture { stream: self }
    }

    /// Counts the number of items in the stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s1 = stream::iter(vec![0]);
    /// let s2 = stream::iter(vec![1, 2, 3]);
    ///
    /// assert_eq!(s1.count().await, 1);
    /// assert_eq!(s2.count().await, 3);
    /// # });
    /// ```
    fn count(self) -> CountFuture<Self>
    where
        Self: Sized,
    {
        CountFuture {
            stream: self,
            count: 0,
        }
    }

    /// Maps items of the stream to new values using a closure.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![1, 2, 3]);
    /// let mut s = s.map(|x| 2 * x);
    ///
    /// assert_eq!(s.next().await, Some(2));
    /// assert_eq!(s.next().await, Some(4));
    /// assert_eq!(s.next().await, Some(6));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn map<T, F>(self, f: F) -> Map<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> T,
    {
        Map { stream: self, f }
    }

    /// Maps items to streams and then concatenates them.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let words = stream::iter(vec!["one", "two"]);
    ///
    /// let s: String = words
    ///     .flat_map(|s| stream::iter(s.chars()))
    ///     .collect()
    ///     .await;
    ///
    /// assert_eq!(s, "onetwo");
    /// # });
    /// ```
    fn flat_map<U, F>(self, f: F) -> FlatMap<Self, U, F>
    where
        Self: Sized,
        U: Stream,
        F: FnMut(Self::Item) -> U,
    {
        FlatMap {
            stream: self.map(f),
            inner_stream: None,
        }
    }

    /// Concatenates inner streams.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s1 = stream::iter(vec![1, 2, 3]);
    /// let s2 = stream::iter(vec![4, 5]);
    ///
    /// let s = stream::iter(vec![s1, s2]);
    /// let v: Vec<_> = s.flatten().collect().await;
    /// assert_eq!(v, [1, 2, 3, 4, 5]);
    /// # });
    /// ```
    fn flatten(self) -> Flatten<Self>
    where
        Self: Sized,
        Self::Item: Stream,
    {
        Flatten {
            stream: self,
            inner_stream: None,
        }
    }

    /// Maps items of the stream to new values using an async closure.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::pin;
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![1, 2, 3]);
    /// let mut s = s.then(|x| async move { 2 * x });
    ///
    /// pin!(s);
    /// assert_eq!(s.next().await, Some(2));
    /// assert_eq!(s.next().await, Some(4));
    /// assert_eq!(s.next().await, Some(6));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn then<F, Fut>(self, f: F) -> Then<Self, F, Fut>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> Fut,
        Fut: Future,
    {
        Then {
            stream: self,
            future: None,
            f,
        }
    }

    /// Keeps items of the stream for which `predicate` returns `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![1, 2, 3, 4]);
    /// let mut s = s.filter(|i| i % 2 == 0);
    ///
    /// assert_eq!(s.next().await, Some(2));
    /// assert_eq!(s.next().await, Some(4));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn filter<P>(self, predicate: P) -> Filter<Self, P>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        Filter {
            stream: self,
            predicate,
        }
    }

    /// Filters and maps items of the stream using a closure.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec!["1", "lol", "3", "NaN", "5"]);
    /// let mut s = s.filter_map(|a| a.parse::<u32>().ok());
    ///
    /// assert_eq!(s.next().await, Some(1));
    /// assert_eq!(s.next().await, Some(3));
    /// assert_eq!(s.next().await, Some(5));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn filter_map<T, F>(self, f: F) -> FilterMap<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> Option<T>,
    {
        FilterMap { stream: self, f }
    }

    /// Takes only the first `n` items of the stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::repeat(7).take(2);
    ///
    /// assert_eq!(s.next().await, Some(7));
    /// assert_eq!(s.next().await, Some(7));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn take(self, n: usize) -> Take<Self>
    where
        Self: Sized,
    {
        Take { stream: self, n }
    }

    /// Takes items while `predicate` returns `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![1, 2, 3, 4]);
    /// let mut s = s.take_while(|x| *x < 3);
    ///
    /// assert_eq!(s.next().await, Some(1));
    /// assert_eq!(s.next().await, Some(2));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn take_while<P>(self, predicate: P) -> TakeWhile<Self, P>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        TakeWhile {
            stream: self,
            predicate,
        }
    }

    /// Maps items while `predicate` returns [`Some`].
    ///
    /// This stream is not fused. After the predicate returns [`None`] the stream still
    /// contains remaining items that can be obtained by subsequent `next` calls.
    /// You can [`fuse`](StreamExt::fuse) the stream if this behavior is undesirable.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![1, 2, 0, 3]);
    /// let mut s = s.map_while(|x: u32| x.checked_sub(1));
    ///
    /// assert_eq!(s.next().await, Some(0));
    /// assert_eq!(s.next().await, Some(1));
    /// assert_eq!(s.next().await, None);
    ///
    /// // Continue to iterate the stream.
    /// assert_eq!(s.next().await, Some(2));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn map_while<B, P>(self, predicate: P) -> MapWhile<Self, P>
    where
        Self: Sized,
        P: FnMut(Self::Item) -> Option<B>,
    {
        MapWhile {
            stream: self,
            predicate,
        }
    }

    /// Skips the first `n` items of the stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![1, 2, 3]);
    /// let mut s = s.skip(2);
    ///
    /// assert_eq!(s.next().await, Some(3));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn skip(self, n: usize) -> Skip<Self>
    where
        Self: Sized,
    {
        Skip { stream: self, n }
    }

    /// Skips items while `predicate` returns `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![-1i32, 0, 1]);
    /// let mut s = s.skip_while(|x| x.is_negative());
    ///
    /// assert_eq!(s.next().await, Some(0));
    /// assert_eq!(s.next().await, Some(1));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn skip_while<P>(self, predicate: P) -> SkipWhile<Self, P>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        SkipWhile {
            stream: self,
            predicate: Some(predicate),
        }
    }

    /// Yields every `step`th item.
    ///
    /// # Panics
    ///
    /// This method will panic if the `step` is 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![0, 1, 2, 3, 4]);
    /// let mut s = s.step_by(2);
    ///
    /// assert_eq!(s.next().await, Some(0));
    /// assert_eq!(s.next().await, Some(2));
    /// assert_eq!(s.next().await, Some(4));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn step_by(self, step: usize) -> StepBy<Self>
    where
        Self: Sized,
    {
        assert!(step > 0, "`step` must be greater than zero");
        StepBy {
            stream: self,
            step,
            i: 0,
        }
    }

    /// Appends another stream to the end of this one.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s1 = stream::iter(vec![1, 2]);
    /// let s2 = stream::iter(vec![7, 8]);
    /// let mut s = s1.chain(s2);
    ///
    /// assert_eq!(s.next().await, Some(1));
    /// assert_eq!(s.next().await, Some(2));
    /// assert_eq!(s.next().await, Some(7));
    /// assert_eq!(s.next().await, Some(8));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn chain<U>(self, other: U) -> Chain<Self, U>
    where
        Self: Sized,
        U: Stream<Item = Self::Item> + Sized,
    {
        Chain {
            first: self.fuse(),
            second: other.fuse(),
        }
    }

    /// Clones all items.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![&1, &2]);
    /// let mut s = s.cloned();
    ///
    /// assert_eq!(s.next().await, Some(1));
    /// assert_eq!(s.next().await, Some(2));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn cloned<'a, T>(self) -> Cloned<Self>
    where
        Self: Stream<Item = &'a T> + Sized,
        T: Clone + 'a,
    {
        Cloned { stream: self }
    }

    /// Copies all items.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![&1, &2]);
    /// let mut s = s.copied();
    ///
    /// assert_eq!(s.next().await, Some(1));
    /// assert_eq!(s.next().await, Some(2));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn copied<'a, T>(self) -> Copied<Self>
    where
        Self: Stream<Item = &'a T> + Sized,
        T: Copy + 'a,
    {
        Copied { stream: self }
    }

    /// Collects all items in the stream into a collection.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(1..=3);
    ///
    /// let items: Vec<_> = s.collect().await;
    /// assert_eq!(items, [1, 2, 3]);
    /// # });
    /// ```
    fn collect<C>(self) -> CollectFuture<Self, C>
    where
        Self: Sized,
        C: Default + Extend<Self::Item>,
    {
        CollectFuture {
            stream: self,
            collection: Default::default(),
        }
    }

    /// Collects all items in the fallible stream into a collection.
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![Ok(1), Err(2), Ok(3)]);
    /// let res: Result<Vec<i32>, i32> = s.try_collect().await;
    /// assert_eq!(res, Err(2));
    ///
    /// let s = stream::iter(vec![Ok(1), Ok(2), Ok(3)]);
    /// let res: Result<Vec<i32>, i32> = s.try_collect().await;
    /// assert_eq!(res, Ok(vec![1, 2, 3]));
    /// # })
    /// ```
    fn try_collect<T, E, C>(self) -> TryCollectFuture<Self, C>
    where
        Self: Stream<Item = Result<T, E>> + Sized,
        C: Default + Extend<T>,
    {
        TryCollectFuture {
            stream: self,
            items: Default::default(),
        }
    }

    /// Partitions items into those for which `predicate` is `true` and those for which it is
    /// `false`, and then collects them into two collections.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![1, 2, 3]);
    /// let (even, odd): (Vec<_>, Vec<_>) = s.partition(|&n| n % 2 == 0).await;
    ///
    /// assert_eq!(even, &[2]);
    /// assert_eq!(odd, &[1, 3]);
    /// # })
    /// ```
    fn partition<B, P>(self, predicate: P) -> PartitionFuture<Self, P, B>
    where
        Self: Sized,
        B: Default + Extend<Self::Item>,
        P: FnMut(&Self::Item) -> bool,
    {
        PartitionFuture {
            stream: self,
            predicate,
            res: Some(Default::default()),
        }
    }

    /// Accumulates a computation over the stream.
    ///
    /// The computation begins with the accumulator value set to `init`, and then applies `f` to
    /// the accumulator and each item in the stream. The final accumulator value is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![1, 2, 3]);
    /// let sum = s.fold(0, |acc, x| acc + x).await;
    ///
    /// assert_eq!(sum, 6);
    /// # })
    /// ```
    fn fold<T, F>(self, init: T, f: F) -> FoldFuture<Self, F, T>
    where
        Self: Sized,
        F: FnMut(T, Self::Item) -> T,
    {
        FoldFuture {
            stream: self,
            f,
            acc: Some(init),
        }
    }

    /// Accumulates a fallible computation over the stream.
    ///
    /// The computation begins with the accumulator value set to `init`, and then applies `f` to
    /// the accumulator and each item in the stream. The final accumulator value is returned, or an
    /// error if `f` failed the computation.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(vec![Ok(1), Ok(2), Ok(3)]);
    ///
    /// let sum = s.try_fold(0, |acc, v| {
    ///     if (acc + v) % 2 == 1 {
    ///         Ok(acc + v)
    ///     } else {
    ///         Err("fail")
    ///     }
    /// })
    /// .await;
    ///
    /// assert_eq!(sum, Err("fail"));
    /// # })
    /// ```
    fn try_fold<T, E, F, B>(&mut self, init: B, f: F) -> TryFoldFuture<'_, Self, F, B>
    where
        Self: Stream<Item = Result<T, E>> + Unpin + Sized,
        F: FnMut(B, T) -> Result<B, E>,
    {
        TryFoldFuture {
            stream: self,
            f,
            acc: Some(init),
        }
    }

    /// Maps items of the stream to new values using a state value and a closure.
    ///
    /// Scanning begins with the initial state set to `initial_state`, and then applies `f` to the
    /// state and each item in the stream. The stream stops when `f` returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![1, 2, 3]);
    /// let mut s = s.scan(1, |state, x| {
    ///     *state = *state * x;
    ///     Some(-*state)
    /// });
    ///
    /// assert_eq!(s.next().await, Some(-1));
    /// assert_eq!(s.next().await, Some(-2));
    /// assert_eq!(s.next().await, Some(-6));
    /// assert_eq!(s.next().await, None);
    /// # })
    /// ```
    fn scan<St, B, F>(self, initial_state: St, f: F) -> Scan<Self, St, F>
    where
        Self: Sized,
        F: FnMut(&mut St, Self::Item) -> Option<B>,
    {
        Scan {
            stream: self,
            state_f: (initial_state, f),
        }
    }

    /// Fuses the stream so that it stops yielding items after the first [`None`].
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::once(1).fuse();
    ///
    /// assert_eq!(s.next().await, Some(1));
    /// assert_eq!(s.next().await, None);
    /// assert_eq!(s.next().await, None);
    /// # })
    /// ```
    fn fuse(self) -> Fuse<Self>
    where
        Self: Sized,
    {
        Fuse {
            stream: self,
            done: false,
        }
    }

    /// Repeats the stream from beginning to end, forever.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(vec![1, 2]).cycle();
    ///
    /// assert_eq!(s.next().await, Some(1));
    /// assert_eq!(s.next().await, Some(2));
    /// assert_eq!(s.next().await, Some(1));
    /// assert_eq!(s.next().await, Some(2));
    /// # });
    /// ```
    fn cycle(self) -> Cycle<Self>
    where
        Self: Clone + Sized,
    {
        Cycle {
            orig: self.clone(),
            stream: self,
        }
    }

    /// Enumerates items, mapping them to `(index, item)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec!['a', 'b', 'c']);
    /// let mut s = s.enumerate();
    ///
    /// assert_eq!(s.next().await, Some((0, 'a')));
    /// assert_eq!(s.next().await, Some((1, 'b')));
    /// assert_eq!(s.next().await, Some((2, 'c')));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn enumerate(self) -> Enumerate<Self>
    where
        Self: Sized,
    {
        Enumerate { stream: self, i: 0 }
    }

    /// Calls a closure on each item and passes it on.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![1, 2, 3, 4, 5]);
    ///
    /// let sum = s
    ///    .inspect(|x| println!("about to filter {}", x))
    ///    .filter(|x| x % 2 == 0)
    ///    .inspect(|x| println!("made it through filter: {}", x))
    ///    .fold(0, |sum, i| sum + i)
    ///    .await;
    /// # });
    /// ```
    fn inspect<F>(self, f: F) -> Inspect<Self, F>
    where
        Self: Sized,
        F: FnMut(&Self::Item),
    {
        Inspect { stream: self, f }
    }

    /// Gets the `n`th item of the stream.
    ///
    /// In the end, `n+1` items of the stream will be consumed.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(vec![0, 1, 2, 3, 4, 5, 6, 7]);
    ///
    /// assert_eq!(s.nth(2).await, Some(2));
    /// assert_eq!(s.nth(2).await, Some(5));
    /// assert_eq!(s.nth(2).await, None);
    /// # });
    /// ```
    fn nth(&mut self, n: usize) -> NthFuture<'_, Self>
    where
        Self: Unpin,
    {
        NthFuture { stream: self, n }
    }

    /// Returns the last item in the stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![1, 2, 3, 4]);
    /// assert_eq!(s.last().await, Some(4));
    ///
    /// let s = stream::empty::<i32>();
    /// assert_eq!(s.last().await, None);
    /// # });
    /// ```
    fn last(self) -> LastFuture<Self>
    where
        Self: Sized,
    {
        LastFuture {
            stream: self,
            last: None,
        }
    }

    /// Finds the first item of the stream for which `predicate` returns `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(vec![11, 12, 13, 14]);
    ///
    /// assert_eq!(s.find(|x| *x % 2 == 0).await, Some(12));
    /// assert_eq!(s.next().await, Some(13));
    /// # });
    /// ```
    fn find<P>(&mut self, predicate: P) -> FindFuture<'_, Self, P>
    where
        Self: Unpin,
        P: FnMut(&Self::Item) -> bool,
    {
        FindFuture {
            stream: self,
            predicate,
        }
    }

    /// Applies a closure to items in the stream and returns the first [`Some`] result.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(vec!["lol", "NaN", "2", "5"]);
    /// let number = s.find_map(|s| s.parse().ok()).await;
    ///
    /// assert_eq!(number, Some(2));
    /// # });
    /// ```
    fn find_map<F, B>(&mut self, f: F) -> FindMapFuture<'_, Self, F>
    where
        Self: Unpin,
        F: FnMut(Self::Item) -> Option<B>,
    {
        FindMapFuture { stream: self, f }
    }

    /// Finds the index of the first item of the stream for which `predicate` returns `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(vec![0, 1, 2, 3, 4, 5]);
    ///
    /// assert_eq!(s.position(|x| x == 2).await, Some(2));
    /// assert_eq!(s.position(|x| x == 3).await, Some(0));
    /// assert_eq!(s.position(|x| x == 9).await, None);
    /// # });
    /// ```
    fn position<P>(&mut self, predicate: P) -> PositionFuture<'_, Self, P>
    where
        Self: Unpin,
        P: FnMut(Self::Item) -> bool,
    {
        PositionFuture {
            stream: self,
            predicate,
            index: 0,
        }
    }

    /// Tests if `predicate` returns `true` for all items in the stream.
    ///
    /// The result is `true` for an empty stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(vec![1, 2, 3]);
    /// assert!(!s.all(|x| x % 2 == 0).await);
    ///
    /// let mut s = stream::iter(vec![2, 4, 6, 8]);
    /// assert!(s.all(|x| x % 2 == 0).await);
    ///
    /// let mut s = stream::empty::<i32>();
    /// assert!(s.all(|x| x % 2 == 0).await);
    /// # });
    /// ```
    fn all<P>(&mut self, predicate: P) -> AllFuture<'_, Self, P>
    where
        Self: Unpin,
        P: FnMut(Self::Item) -> bool,
    {
        AllFuture {
            stream: self,
            predicate,
        }
    }

    /// Tests if `predicate` returns `true` for any item in the stream.
    ///
    /// The result is `false` for an empty stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(vec![1, 3, 5, 7]);
    /// assert!(!s.any(|x| x % 2 == 0).await);
    ///
    /// let mut s = stream::iter(vec![1, 2, 3]);
    /// assert!(s.any(|x| x % 2 == 0).await);
    ///
    /// let mut s = stream::empty::<i32>();
    /// assert!(!s.any(|x| x % 2 == 0).await);
    /// # });
    /// ```
    fn any<P>(&mut self, predicate: P) -> AnyFuture<'_, Self, P>
    where
        Self: Unpin,
        P: FnMut(Self::Item) -> bool,
    {
        AnyFuture {
            stream: self,
            predicate,
        }
    }

    /// Calls a closure on each item of the stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(vec![1, 2, 3]);
    /// s.for_each(|s| println!("{}", s)).await;
    /// # });
    /// ```
    fn for_each<F>(self, f: F) -> ForEachFuture<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Item),
    {
        ForEachFuture { stream: self, f }
    }

    /// Calls a fallible closure on each item of the stream, stopping on first error.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(vec![0, 1, 2, 3]);
    ///
    /// let mut v = vec![];
    /// let res = s
    ///     .try_for_each(|n| {
    ///         if n < 2 {
    ///             v.push(n);
    ///             Ok(())
    ///         } else {
    ///             Err("too big")
    ///         }
    ///     })
    ///     .await;
    ///
    /// assert_eq!(v, &[0, 1]);
    /// assert_eq!(res, Err("too big"));
    /// # });
    /// ```
    fn try_for_each<F, E>(&mut self, f: F) -> TryForEachFuture<'_, Self, F>
    where
        Self: Unpin,
        F: FnMut(Self::Item) -> Result<(), E>,
    {
        TryForEachFuture { stream: self, f }
    }

    /// Zips up two streams into a single stream of pairs.
    ///
    /// The stream of pairs stops when either of the original two streams is exhausted.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let l = stream::iter(vec![1, 2, 3]);
    /// let r = stream::iter(vec![4, 5, 6, 7]);
    /// let mut s = l.zip(r);
    ///
    /// assert_eq!(s.next().await, Some((1, 4)));
    /// assert_eq!(s.next().await, Some((2, 5)));
    /// assert_eq!(s.next().await, Some((3, 6)));
    /// assert_eq!(s.next().await, None);
    /// # });
    /// ```
    fn zip<U>(self, other: U) -> Zip<Self, U>
    where
        Self: Sized,
        U: Stream,
    {
        Zip {
            item_slot: None,
            first: self,
            second: other,
        }
    }

    /// Collects a stream of pairs into a pair of collections.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let s = stream::iter(vec![(1, 2), (3, 4)]);
    /// let (left, right): (Vec<_>, Vec<_>) = s.unzip().await;
    ///
    /// assert_eq!(left, [1, 3]);
    /// assert_eq!(right, [2, 4]);
    /// # });
    /// ```
    fn unzip<A, B, FromA, FromB>(self) -> UnzipFuture<Self, FromA, FromB>
    where
        FromA: Default + Extend<A>,
        FromB: Default + Extend<B>,
        Self: Stream<Item = (A, B)> + Sized,
    {
        UnzipFuture {
            stream: self,
            res: Some(Default::default()),
        }
    }

    /// Merges with `other` stream, preferring items from `self` whenever both streams are ready.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    /// use futures_lite::stream::{once, pending};
    ///
    /// # spin_on::spin_on(async {
    /// assert_eq!(once(1).or(pending()).next().await, Some(1));
    /// assert_eq!(pending().or(once(2)).next().await, Some(2));
    ///
    /// // The first future wins.
    /// assert_eq!(once(1).or(once(2)).next().await, Some(1));
    /// # })
    /// ```
    fn or<S>(self, other: S) -> Or<Self, S>
    where
        Self: Sized,
        S: Stream<Item = Self::Item>,
    {
        Or {
            stream1: self,
            stream2: other,
        }
    }

    /// Merges with `other` stream, with no preference for either stream when both are ready.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    /// use futures_lite::stream::{once, pending};
    ///
    /// # spin_on::spin_on(async {
    /// assert_eq!(once(1).race(pending()).next().await, Some(1));
    /// assert_eq!(pending().race(once(2)).next().await, Some(2));
    ///
    /// // One of the two stream is randomly chosen as the winner.
    /// let res = once(1).race(once(2)).next().await;
    /// # })
    /// ```
    #[cfg(all(feature = "std", feature = "race"))]
    fn race<S>(self, other: S) -> Race<Self, S>
    where
        Self: Sized,
        S: Stream<Item = Self::Item>,
    {
        Race {
            stream1: self,
            stream2: other,
            rng: Rng::new(),
        }
    }

    /// Yields all immediately available values from a stream.
    ///
    /// This is intended to be used as a way of polling a stream without waiting, similar to the
    /// [`try_iter`] function on [`std::sync::mpsc::Receiver`]. For instance, running this stream
    /// on an [`async_channel::Receiver`] will return all messages that are currently in the
    /// channel, but will not wait for new messages.
    ///
    /// This returns a [`Stream`] instead of an [`Iterator`] because it still needs access to the
    /// polling context in order to poll the underlying stream. Since this stream will never return
    /// `Poll::Pending`, wrapping it in [`block_on`] will allow it to be effectively used as an
    /// [`Iterator`].
    ///
    /// This stream is not necessarily fused. After it returns `None`, it can return `Some(x)` in
    /// the future if it is polled again.
    ///
    /// [`try_iter`]: std::sync::mpsc::Receiver::try_iter
    /// [`async_channel::Receiver`]: https://docs.rs/async-channel/latest/async_channel/struct.Receiver.html
    /// [`Stream`]: crate::stream::Stream
    /// [`Iterator`]: std::iter::Iterator
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::{future, pin};
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # #[cfg(feature = "std")] {
    /// // A stream that yields two values, returns `Pending`, and then yields one more value.
    /// let pend_once = stream::once_future(async {
    ///     future::yield_now().await;
    ///     3
    /// });
    /// let s = stream::iter(vec![1, 2]).chain(pend_once);
    /// pin!(s);
    ///
    /// // This will return the first two values, and then `None` because the stream returns
    /// // `Pending` after that.
    /// let mut iter = stream::block_on(s.drain());
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), None);
    ///
    /// // This will return the last value, because the stream returns `Ready` when polled.
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), None);
    /// # }
    /// ```
    fn drain(&mut self) -> Drain<'_, Self> {
        Drain { stream: self }
    }

    /// Boxes the stream and changes its type to `dyn Stream + Send + 'a`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let a = stream::once(1);
    /// let b = stream::empty();
    ///
    /// // Streams of different types can be stored in
    /// // the same collection when they are boxed:
    /// let streams = vec![a.boxed(), b.boxed()];
    /// # })
    /// ```
    #[cfg(feature = "alloc")]
    fn boxed<'a>(self) -> Pin<Box<dyn Stream<Item = Self::Item> + Send + 'a>>
    where
        Self: Send + Sized + 'a,
    {
        Box::pin(self)
    }

    /// Boxes the stream and changes its type to `dyn Stream + 'a`.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::stream::{self, StreamExt};
    ///
    /// # spin_on::spin_on(async {
    /// let a = stream::once(1);
    /// let b = stream::empty();
    ///
    /// // Streams of different types can be stored in
    /// // the same collection when they are boxed:
    /// let streams = vec![a.boxed_local(), b.boxed_local()];
    /// # })
    /// ```
    #[cfg(feature = "alloc")]
    fn boxed_local<'a>(self) -> Pin<Box<dyn Stream<Item = Self::Item> + 'a>>
    where
        Self: Sized + 'a,
    {
        Box::pin(self)
    }
}

impl<S: Stream + ?Sized> StreamExt for S {}

/// Type alias for `Pin<Box<dyn Stream<Item = T> + Send + 'static>>`.
///
/// # Examples
///
/// ```
/// use futures_lite::stream::{self, StreamExt};
///
/// // These two lines are equivalent:
/// let s1: stream::Boxed<i32> = stream::once(7).boxed();
/// let s2: stream::Boxed<i32> = Box::pin(stream::once(7));
/// ```
#[cfg(feature = "alloc")]
pub type Boxed<T> = Pin<Box<dyn Stream<Item = T> + Send + 'static>>;

/// Type alias for `Pin<Box<dyn Stream<Item = T> + 'static>>`.
///
/// # Examples
///
/// ```
/// use futures_lite::stream::{self, StreamExt};
///
/// // These two lines are equivalent:
/// let s1: stream::BoxedLocal<i32> = stream::once(7).boxed_local();
/// let s2: stream::BoxedLocal<i32> = Box::pin(stream::once(7));
/// ```
#[cfg(feature = "alloc")]
pub type BoxedLocal<T> = Pin<Box<dyn Stream<Item = T> + 'static>>;

/// Future for the [`StreamExt::next()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct NextFuture<'a, S: ?Sized> {
    stream: &'a mut S,
}

impl<S: Unpin + ?Sized> Unpin for NextFuture<'_, S> {}

impl<S: Stream + Unpin + ?Sized> Future for NextFuture<'_, S> {
    type Output = Option<S::Item>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.stream.poll_next(cx)
    }
}

/// Future for the [`StreamExt::try_next()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct TryNextFuture<'a, S: ?Sized> {
    stream: &'a mut S,
}

impl<S: Unpin + ?Sized> Unpin for TryNextFuture<'_, S> {}

impl<T, E, S> Future for TryNextFuture<'_, S>
where
    S: Stream<Item = Result<T, E>> + Unpin + ?Sized,
{
    type Output = Result<Option<T>, E>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = ready!(self.stream.poll_next(cx));
        Poll::Ready(res.transpose())
    }
}

pin_project! {
    /// Future for the [`StreamExt::count()`] method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct CountFuture<S: ?Sized> {
        count: usize,
        #[pin]
        stream: S,
    }
}

impl<S: Stream + ?Sized> Future for CountFuture<S> {
    type Output = usize;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match ready!(self.as_mut().project().stream.poll_next(cx)) {
                None => return Poll::Ready(self.count),
                Some(_) => *self.as_mut().project().count += 1,
            }
        }
    }
}

pin_project! {
    /// Future for the [`StreamExt::collect()`] method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct CollectFuture<S, C> {
        #[pin]
        stream: S,
        collection: C,
    }
}

impl<S, C> Future for CollectFuture<S, C>
where
    S: Stream,
    C: Default + Extend<S::Item>,
{
    type Output = C;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<C> {
        let mut this = self.as_mut().project();
        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(e) => this.collection.extend(Some(e)),
                None => return Poll::Ready(mem::take(self.project().collection)),
            }
        }
    }
}

pin_project! {
    /// Future for the [`StreamExt::try_collect()`] method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct TryCollectFuture<S, C> {
        #[pin]
        stream: S,
        items: C,
    }
}

impl<T, E, S, C> Future for TryCollectFuture<S, C>
where
    S: Stream<Item = Result<T, E>>,
    C: Default + Extend<T>,
{
    type Output = Result<C, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        Poll::Ready(Ok(loop {
            match ready!(this.stream.as_mut().poll_next(cx)?) {
                Some(x) => this.items.extend(Some(x)),
                None => break mem::take(this.items),
            }
        }))
    }
}

pin_project! {
    /// Future for the [`StreamExt::partition()`] method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct PartitionFuture<S, P, B> {
        #[pin]
        stream: S,
        predicate: P,
        res: Option<(B, B)>,
    }
}

impl<S, P, B> Future for PartitionFuture<S, P, B>
where
    S: Stream + Sized,
    P: FnMut(&S::Item) -> bool,
    B: Default + Extend<S::Item>,
{
    type Output = (B, B);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(v) => {
                    let res = this.res.as_mut().unwrap();
                    if (this.predicate)(&v) {
                        res.0.extend(Some(v))
                    } else {
                        res.1.extend(Some(v))
                    }
                }
                None => return Poll::Ready(this.res.take().unwrap()),
            }
        }
    }
}

pin_project! {
    /// Future for the [`StreamExt::fold()`] method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct FoldFuture<S, F, T> {
        #[pin]
        stream: S,
        f: F,
        acc: Option<T>,
    }
}

impl<S, F, T> Future for FoldFuture<S, F, T>
where
    S: Stream,
    F: FnMut(T, S::Item) -> T,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(v) => {
                    let old = this.acc.take().unwrap();
                    let new = (this.f)(old, v);
                    *this.acc = Some(new);
                }
                None => return Poll::Ready(this.acc.take().unwrap()),
            }
        }
    }
}

/// Future for the [`StreamExt::try_fold()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct TryFoldFuture<'a, S, F, B> {
    stream: &'a mut S,
    f: F,
    acc: Option<B>,
}

impl<S, F, B> Unpin for TryFoldFuture<'_, S, F, B> {}

impl<T, E, S, F, B> Future for TryFoldFuture<'_, S, F, B>
where
    S: Stream<Item = Result<T, E>> + Unpin,
    F: FnMut(B, T) -> Result<B, E>,
{
    type Output = Result<B, E>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match ready!(self.stream.poll_next(cx)) {
                Some(Err(e)) => return Poll::Ready(Err(e)),
                Some(Ok(t)) => {
                    let old = self.acc.take().unwrap();
                    let new = (&mut self.f)(old, t);

                    match new {
                        Ok(t) => self.acc = Some(t),
                        Err(e) => return Poll::Ready(Err(e)),
                    }
                }
                None => return Poll::Ready(Ok(self.acc.take().unwrap())),
            }
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::scan()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Scan<S, St, F> {
        #[pin]
        stream: S,
        state_f: (St, F),
    }
}

impl<S, St, F, B> Stream for Scan<S, St, F>
where
    S: Stream,
    F: FnMut(&mut St, S::Item) -> Option<B>,
{
    type Item = B;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<B>> {
        let mut this = self.project();
        this.stream.as_mut().poll_next(cx).map(|item| {
            item.and_then(|item| {
                let (state, f) = this.state_f;
                f(state, item)
            })
        })
    }
}

pin_project! {
    /// Stream for the [`StreamExt::fuse()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Fuse<S> {
        #[pin]
        stream: S,
        done: bool,
    }
}

impl<S: Stream> Stream for Fuse<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        let this = self.project();

        if *this.done {
            Poll::Ready(None)
        } else {
            let next = ready!(this.stream.poll_next(cx));
            if next.is_none() {
                *this.done = true;
            }
            Poll::Ready(next)
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::map()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Map<S, F> {
        #[pin]
        stream: S,
        f: F,
    }
}

impl<S, F, T> Stream for Map<S, F>
where
    S: Stream,
    F: FnMut(S::Item) -> T,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let next = ready!(this.stream.poll_next(cx));
        Poll::Ready(next.map(this.f))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

pin_project! {
    /// Stream for the [`StreamExt::flat_map()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct FlatMap<S, U, F> {
        #[pin]
        stream: Map<S, F>,
        #[pin]
        inner_stream: Option<U>,
    }
}

impl<S, U, F> Stream for FlatMap<S, U, F>
where
    S: Stream,
    U: Stream,
    F: FnMut(S::Item) -> U,
{
    type Item = U::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        loop {
            if let Some(inner) = this.inner_stream.as_mut().as_pin_mut() {
                match ready!(inner.poll_next(cx)) {
                    Some(item) => return Poll::Ready(Some(item)),
                    None => this.inner_stream.set(None),
                }
            }

            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(stream) => this.inner_stream.set(Some(stream)),
                None => return Poll::Ready(None),
            }
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::flatten()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Flatten<S: Stream> {
        #[pin]
        stream: S,
        #[pin]
        inner_stream: Option<S::Item>,
    }
}

impl<S, U> Stream for Flatten<S>
where
    S: Stream<Item = U>,
    U: Stream,
{
    type Item = U::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        loop {
            if let Some(inner) = this.inner_stream.as_mut().as_pin_mut() {
                match ready!(inner.poll_next(cx)) {
                    Some(item) => return Poll::Ready(Some(item)),
                    None => this.inner_stream.set(None),
                }
            }

            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(inner) => this.inner_stream.set(Some(inner)),
                None => return Poll::Ready(None),
            }
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::then()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Then<S, F, Fut> {
        #[pin]
        stream: S,
        #[pin]
        future: Option<Fut>,
        f: F,
    }
}

impl<S, F, Fut> Stream for Then<S, F, Fut>
where
    S: Stream,
    F: FnMut(S::Item) -> Fut,
    Fut: Future,
{
    type Item = Fut::Output;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            if let Some(fut) = this.future.as_mut().as_pin_mut() {
                let item = ready!(fut.poll(cx));
                this.future.set(None);
                return Poll::Ready(Some(item));
            } else if let Some(item) = ready!(this.stream.as_mut().poll_next(cx)) {
                this.future.set(Some((this.f)(item)));
            } else {
                return Poll::Ready(None);
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let future_len = self.future.is_some() as usize;
        let (lower, upper) = self.stream.size_hint();
        let lower = lower.saturating_add(future_len);
        let upper = upper.and_then(|u| u.checked_add(future_len));
        (lower, upper)
    }
}

pin_project! {
    /// Stream for the [`StreamExt::filter()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Filter<S, P> {
        #[pin]
        stream: S,
        predicate: P,
    }
}

impl<S, P> Stream for Filter<S, P>
where
    S: Stream,
    P: FnMut(&S::Item) -> bool,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                None => return Poll::Ready(None),
                Some(v) if (this.predicate)(&v) => return Poll::Ready(Some(v)),
                Some(_) => {}
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, hi) = self.stream.size_hint();

        // If the filter matches all of the elements, it will match the stream's upper bound.
        // If the filter matches none of the elements, there will be zero returned values.
        (0, hi)
    }
}

/// Merges two streams, preferring items from `stream1` whenever both streams are ready.
///
/// # Examples
///
/// ```
/// use futures_lite::stream::{self, once, pending, StreamExt};
///
/// # spin_on::spin_on(async {
/// assert_eq!(stream::or(once(1), pending()).next().await, Some(1));
/// assert_eq!(stream::or(pending(), once(2)).next().await, Some(2));
///
/// // The first stream wins.
/// assert_eq!(stream::or(once(1), once(2)).next().await, Some(1));
/// # })
/// ```
pub fn or<T, S1, S2>(stream1: S1, stream2: S2) -> Or<S1, S2>
where
    S1: Stream<Item = T>,
    S2: Stream<Item = T>,
{
    Or { stream1, stream2 }
}

pin_project! {
    /// Stream for the [`or()`] function and the [`StreamExt::or()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Or<S1, S2> {
        #[pin]
        stream1: S1,
        #[pin]
        stream2: S2,
    }
}

impl<T, S1, S2> Stream for Or<S1, S2>
where
    S1: Stream<Item = T>,
    S2: Stream<Item = T>,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if let Poll::Ready(Some(t)) = this.stream1.as_mut().poll_next(cx) {
            return Poll::Ready(Some(t));
        }
        this.stream2.as_mut().poll_next(cx)
    }
}

/// Merges two streams, with no preference for either stream when both are ready.
///
/// # Examples
///
/// ```
/// use futures_lite::stream::{self, once, pending, StreamExt};
///
/// # spin_on::spin_on(async {
/// assert_eq!(stream::race(once(1), pending()).next().await, Some(1));
/// assert_eq!(stream::race(pending(), once(2)).next().await, Some(2));
///
/// // One of the two stream is randomly chosen as the winner.
/// let res = stream::race(once(1), once(2)).next().await;
/// # })
/// ```
#[cfg(all(feature = "std", feature = "race"))]
pub fn race<T, S1, S2>(stream1: S1, stream2: S2) -> Race<S1, S2>
where
    S1: Stream<Item = T>,
    S2: Stream<Item = T>,
{
    Race {
        stream1,
        stream2,
        rng: Rng::new(),
    }
}

/// Races two streams, but with a user-provided seed for randomness.
///
/// # Examples
///
/// ```
/// use futures_lite::stream::{self, once, pending, StreamExt};
///
/// // A fixed seed is used for reproducibility.
/// const SEED: u64 = 123;
///
/// # spin_on::spin_on(async {
/// assert_eq!(stream::race_with_seed(once(1), pending(), SEED).next().await, Some(1));
/// assert_eq!(stream::race_with_seed(pending(), once(2), SEED).next().await, Some(2));
///
/// // One of the two stream is randomly chosen as the winner.
/// let res = stream::race_with_seed(once(1), once(2), SEED).next().await;
/// # })
/// ```
#[cfg(feature = "race")]
pub fn race_with_seed<T, S1, S2>(stream1: S1, stream2: S2, seed: u64) -> Race<S1, S2>
where
    S1: Stream<Item = T>,
    S2: Stream<Item = T>,
{
    Race {
        stream1,
        stream2,
        rng: Rng::with_seed(seed),
    }
}

#[cfg(feature = "race")]
pin_project! {
    /// Stream for the [`race()`] function and the [`StreamExt::race()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Race<S1, S2> {
        #[pin]
        stream1: S1,
        #[pin]
        stream2: S2,
        rng: Rng,
    }
}

#[cfg(feature = "race")]
impl<T, S1, S2> Stream for Race<S1, S2>
where
    S1: Stream<Item = T>,
    S2: Stream<Item = T>,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if this.rng.bool() {
            if let Poll::Ready(Some(t)) = this.stream1.as_mut().poll_next(cx) {
                return Poll::Ready(Some(t));
            }
            if let Poll::Ready(Some(t)) = this.stream2.as_mut().poll_next(cx) {
                return Poll::Ready(Some(t));
            }
        } else {
            if let Poll::Ready(Some(t)) = this.stream2.as_mut().poll_next(cx) {
                return Poll::Ready(Some(t));
            }
            if let Poll::Ready(Some(t)) = this.stream1.as_mut().poll_next(cx) {
                return Poll::Ready(Some(t));
            }
        }
        Poll::Pending
    }
}

pin_project! {
    /// Stream for the [`StreamExt::filter_map()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct FilterMap<S, F> {
        #[pin]
        stream: S,
        f: F,
    }
}

impl<S, F, T> Stream for FilterMap<S, F>
where
    S: Stream,
    F: FnMut(S::Item) -> Option<T>,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                None => return Poll::Ready(None),
                Some(v) => {
                    if let Some(t) = (this.f)(v) {
                        return Poll::Ready(Some(t));
                    }
                }
            }
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::take()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Take<S> {
        #[pin]
        stream: S,
        n: usize,
    }
}

impl<S: Stream> Stream for Take<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        let this = self.project();

        if *this.n == 0 {
            Poll::Ready(None)
        } else {
            let next = ready!(this.stream.poll_next(cx));
            match next {
                Some(_) => *this.n -= 1,
                None => *this.n = 0,
            }
            Poll::Ready(next)
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::take_while()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct TakeWhile<S, P> {
        #[pin]
        stream: S,
        predicate: P,
    }
}

impl<S, P> Stream for TakeWhile<S, P>
where
    S: Stream,
    P: FnMut(&S::Item) -> bool,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match ready!(this.stream.poll_next(cx)) {
            Some(v) => {
                if (this.predicate)(&v) {
                    Poll::Ready(Some(v))
                } else {
                    Poll::Ready(None)
                }
            }
            None => Poll::Ready(None),
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::map_while()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct MapWhile<S, P> {
        #[pin]
        stream: S,
        predicate: P,
    }
}

impl<B, S, P> Stream for MapWhile<S, P>
where
    S: Stream,
    P: FnMut(S::Item) -> Option<B>,
{
    type Item = B;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match ready!(this.stream.poll_next(cx)) {
            Some(v) => Poll::Ready((this.predicate)(v)),
            None => Poll::Ready(None),
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::skip()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Skip<S> {
        #[pin]
        stream: S,
        n: usize,
    }
}

impl<S: Stream> Stream for Skip<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(v) => match *this.n {
                    0 => return Poll::Ready(Some(v)),
                    _ => *this.n -= 1,
                },
                None => return Poll::Ready(None),
            }
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::skip_while()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct SkipWhile<S, P> {
        #[pin]
        stream: S,
        predicate: Option<P>,
    }
}

impl<S, P> Stream for SkipWhile<S, P>
where
    S: Stream,
    P: FnMut(&S::Item) -> bool,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(v) => match this.predicate {
                    Some(p) => {
                        if !p(&v) {
                            *this.predicate = None;
                            return Poll::Ready(Some(v));
                        }
                    }
                    None => return Poll::Ready(Some(v)),
                },
                None => return Poll::Ready(None),
            }
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::step_by()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct StepBy<S> {
        #[pin]
        stream: S,
        step: usize,
        i: usize,
    }
}

impl<S: Stream> Stream for StepBy<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(v) => {
                    if *this.i == 0 {
                        *this.i = *this.step - 1;
                        return Poll::Ready(Some(v));
                    } else {
                        *this.i -= 1;
                    }
                }
                None => return Poll::Ready(None),
            }
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::chain()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Chain<S, U> {
        #[pin]
        first: Fuse<S>,
        #[pin]
        second: Fuse<U>,
    }
}

impl<S: Stream, U: Stream<Item = S::Item>> Stream for Chain<S, U> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if !this.first.done {
            let next = ready!(this.first.as_mut().poll_next(cx));
            if let Some(next) = next {
                return Poll::Ready(Some(next));
            }
        }

        if !this.second.done {
            let next = ready!(this.second.as_mut().poll_next(cx));
            if let Some(next) = next {
                return Poll::Ready(Some(next));
            }
        }

        if this.first.done && this.second.done {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::cloned()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Cloned<S> {
        #[pin]
        stream: S,
    }
}

impl<'a, S, T: 'a> Stream for Cloned<S>
where
    S: Stream<Item = &'a T>,
    T: Clone,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let next = ready!(this.stream.poll_next(cx));
        Poll::Ready(next.cloned())
    }
}

pin_project! {
    /// Stream for the [`StreamExt::copied()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Copied<S> {
        #[pin]
        stream: S,
    }
}

impl<'a, S, T: 'a> Stream for Copied<S>
where
    S: Stream<Item = &'a T>,
    T: Copy,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let next = ready!(this.stream.poll_next(cx));
        Poll::Ready(next.copied())
    }
}

pin_project! {
    /// Stream for the [`StreamExt::cycle()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Cycle<S> {
        orig: S,
        #[pin]
        stream: S,
    }
}

impl<S> Stream for Cycle<S>
where
    S: Stream + Clone,
{
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match ready!(self.as_mut().project().stream.as_mut().poll_next(cx)) {
            Some(item) => Poll::Ready(Some(item)),
            None => {
                let new = self.as_mut().orig.clone();
                self.as_mut().project().stream.set(new);
                self.project().stream.poll_next(cx)
            }
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::enumerate()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Enumerate<S> {
        #[pin]
        stream: S,
        i: usize,
    }
}

impl<S> Stream for Enumerate<S>
where
    S: Stream,
{
    type Item = (usize, S::Item);

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match ready!(this.stream.poll_next(cx)) {
            Some(v) => {
                let ret = (*this.i, v);
                *this.i += 1;
                Poll::Ready(Some(ret))
            }
            None => Poll::Ready(None),
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::inspect()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Inspect<S, F> {
        #[pin]
        stream: S,
        f: F,
    }
}

impl<S, F> Stream for Inspect<S, F>
where
    S: Stream,
    F: FnMut(&S::Item),
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        let next = ready!(this.stream.as_mut().poll_next(cx));
        if let Some(x) = &next {
            (this.f)(x);
        }
        Poll::Ready(next)
    }
}

/// Future for the [`StreamExt::nth()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct NthFuture<'a, S: ?Sized> {
    stream: &'a mut S,
    n: usize,
}

impl<S: Unpin + ?Sized> Unpin for NthFuture<'_, S> {}

impl<S> Future for NthFuture<'_, S>
where
    S: Stream + Unpin + ?Sized,
{
    type Output = Option<S::Item>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match ready!(self.stream.poll_next(cx)) {
                Some(v) => match self.n {
                    0 => return Poll::Ready(Some(v)),
                    _ => self.n -= 1,
                },
                None => return Poll::Ready(None),
            }
        }
    }
}

pin_project! {
    /// Future for the [`StreamExt::last()`] method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct LastFuture<S: Stream> {
        #[pin]
        stream: S,
        last: Option<S::Item>,
    }
}

impl<S: Stream> Future for LastFuture<S> {
    type Output = Option<S::Item>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(new) => *this.last = Some(new),
                None => return Poll::Ready(this.last.take()),
            }
        }
    }
}

/// Future for the [`StreamExt::find()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct FindFuture<'a, S: ?Sized, P> {
    stream: &'a mut S,
    predicate: P,
}

impl<S: Unpin + ?Sized, P> Unpin for FindFuture<'_, S, P> {}

impl<S, P> Future for FindFuture<'_, S, P>
where
    S: Stream + Unpin + ?Sized,
    P: FnMut(&S::Item) -> bool,
{
    type Output = Option<S::Item>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match ready!(self.stream.poll_next(cx)) {
                Some(v) if (&mut self.predicate)(&v) => return Poll::Ready(Some(v)),
                Some(_) => {}
                None => return Poll::Ready(None),
            }
        }
    }
}

/// Future for the [`StreamExt::find_map()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct FindMapFuture<'a, S: ?Sized, F> {
    stream: &'a mut S,
    f: F,
}

impl<S: Unpin + ?Sized, F> Unpin for FindMapFuture<'_, S, F> {}

impl<S, B, F> Future for FindMapFuture<'_, S, F>
where
    S: Stream + Unpin + ?Sized,
    F: FnMut(S::Item) -> Option<B>,
{
    type Output = Option<B>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match ready!(self.stream.poll_next(cx)) {
                Some(v) => {
                    if let Some(v) = (&mut self.f)(v) {
                        return Poll::Ready(Some(v));
                    }
                }
                None => return Poll::Ready(None),
            }
        }
    }
}

/// Future for the [`StreamExt::position()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct PositionFuture<'a, S: ?Sized, P> {
    stream: &'a mut S,
    predicate: P,
    index: usize,
}

impl<S: Unpin + ?Sized, P> Unpin for PositionFuture<'_, S, P> {}

impl<S, P> Future for PositionFuture<'_, S, P>
where
    S: Stream + Unpin + ?Sized,
    P: FnMut(S::Item) -> bool,
{
    type Output = Option<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match ready!(self.stream.poll_next(cx)) {
                Some(v) => {
                    if (&mut self.predicate)(v) {
                        return Poll::Ready(Some(self.index));
                    } else {
                        self.index += 1;
                    }
                }
                None => return Poll::Ready(None),
            }
        }
    }
}

/// Future for the [`StreamExt::all()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct AllFuture<'a, S: ?Sized, P> {
    stream: &'a mut S,
    predicate: P,
}

impl<S: Unpin + ?Sized, P> Unpin for AllFuture<'_, S, P> {}

impl<S, P> Future for AllFuture<'_, S, P>
where
    S: Stream + Unpin + ?Sized,
    P: FnMut(S::Item) -> bool,
{
    type Output = bool;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match ready!(self.stream.poll_next(cx)) {
                Some(v) => {
                    if !(&mut self.predicate)(v) {
                        return Poll::Ready(false);
                    }
                }
                None => return Poll::Ready(true),
            }
        }
    }
}

/// Future for the [`StreamExt::any()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct AnyFuture<'a, S: ?Sized, P> {
    stream: &'a mut S,
    predicate: P,
}

impl<S: Unpin + ?Sized, P> Unpin for AnyFuture<'_, S, P> {}

impl<S, P> Future for AnyFuture<'_, S, P>
where
    S: Stream + Unpin + ?Sized,
    P: FnMut(S::Item) -> bool,
{
    type Output = bool;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match ready!(self.stream.poll_next(cx)) {
                Some(v) => {
                    if (&mut self.predicate)(v) {
                        return Poll::Ready(true);
                    }
                }
                None => return Poll::Ready(false),
            }
        }
    }
}

pin_project! {
    /// Future for the [`StreamExt::for_each()`] method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct ForEachFuture<S, F> {
        #[pin]
        stream: S,
        f: F,
    }
}

impl<S, F> Future for ForEachFuture<S, F>
where
    S: Stream,
    F: FnMut(S::Item),
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some(v) => (this.f)(v),
                None => return Poll::Ready(()),
            }
        }
    }
}

/// Future for the [`StreamExt::try_for_each()`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct TryForEachFuture<'a, S: ?Sized, F> {
    stream: &'a mut S,
    f: F,
}

impl<S: Unpin + ?Sized, F> Unpin for TryForEachFuture<'_, S, F> {}

impl<S, F, E> Future for TryForEachFuture<'_, S, F>
where
    S: Stream + Unpin + ?Sized,
    F: FnMut(S::Item) -> Result<(), E>,
{
    type Output = Result<(), E>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match ready!(self.stream.poll_next(cx)) {
                None => return Poll::Ready(Ok(())),
                Some(v) => (&mut self.f)(v)?,
            }
        }
    }
}

pin_project! {
    /// Stream for the [`StreamExt::zip()`] method.
    #[derive(Clone, Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Zip<A: Stream, B> {
        item_slot: Option<A::Item>,
        #[pin]
        first: A,
        #[pin]
        second: B,
    }
}

impl<A: Stream, B: Stream> Stream for Zip<A, B> {
    type Item = (A::Item, B::Item);

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        if this.item_slot.is_none() {
            match this.first.poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(item)) => *this.item_slot = Some(item),
            }
        }

        let second_item = ready!(this.second.poll_next(cx));
        let first_item = this.item_slot.take().unwrap();
        Poll::Ready(second_item.map(|second_item| (first_item, second_item)))
    }
}

pin_project! {
    /// Future for the [`StreamExt::unzip()`] method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct UnzipFuture<S, FromA, FromB> {
        #[pin]
        stream: S,
        res: Option<(FromA, FromB)>,
    }
}

impl<S, A, B, FromA, FromB> Future for UnzipFuture<S, FromA, FromB>
where
    S: Stream<Item = (A, B)>,
    FromA: Default + Extend<A>,
    FromB: Default + Extend<B>,
{
    type Output = (FromA, FromB);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            match ready!(this.stream.as_mut().poll_next(cx)) {
                Some((a, b)) => {
                    let res = this.res.as_mut().unwrap();
                    res.0.extend(Some(a));
                    res.1.extend(Some(b));
                }
                None => return Poll::Ready(this.res.take().unwrap()),
            }
        }
    }
}

/// Stream for the [`StreamExt::drain()`] method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Drain<'a, S: ?Sized> {
    stream: &'a mut S,
}

impl<S: Unpin + ?Sized> Unpin for Drain<'_, S> {}

impl<'a, S: Unpin + ?Sized> Drain<'a, S> {
    /// Get a reference to the underlying stream.
    ///
    /// ## Examples
    ///
    /// ```
    /// use futures_lite::{prelude::*, stream};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(vec![1, 2, 3]);
    /// let s2 = s.drain();
    ///
    /// let inner = s2.get_ref();
    /// // s and inner are the same.
    /// # });
    /// ```
    pub fn get_ref(&self) -> &S {
        &self.stream
    }

    /// Get a mutable reference to the underlying stream.
    ///
    /// ## Examples
    ///
    /// ```
    /// use futures_lite::{prelude::*, stream};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(vec![1, 2, 3]);
    /// let mut s2 = s.drain();
    ///
    /// let inner = s2.get_mut();
    /// assert_eq!(inner.collect::<Vec<_>>().await, vec![1, 2, 3]);
    /// # });
    /// ```
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.stream
    }

    /// Consume this stream and get the underlying stream.
    ///
    /// ## Examples
    ///
    /// ```
    /// use futures_lite::{prelude::*, stream};
    ///
    /// # spin_on::spin_on(async {
    /// let mut s = stream::iter(vec![1, 2, 3]);
    /// let mut s2 = s.drain();
    ///
    /// let inner = s2.into_inner();
    /// assert_eq!(inner.collect::<Vec<_>>().await, vec![1, 2, 3]);
    /// # });
    /// ```
    pub fn into_inner(self) -> &'a mut S {
        self.stream
    }
}

impl<S: Stream + Unpin + ?Sized> Stream for Drain<'_, S> {
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.stream.poll_next(cx) {
            Poll::Ready(x) => Poll::Ready(x),
            Poll::Pending => Poll::Ready(None),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, hi) = self.stream.size_hint();
        (0, hi)
    }
}
