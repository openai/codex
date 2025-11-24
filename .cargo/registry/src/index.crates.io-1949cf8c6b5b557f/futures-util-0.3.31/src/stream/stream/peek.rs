use crate::fns::FnOnce1;
use crate::stream::{Fuse, StreamExt};
use core::fmt;
use core::marker::PhantomData;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
#[cfg(feature = "sink")]
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// A `Stream` that implements a `peek` method.
    ///
    /// The `peek` method can be used to retrieve a reference
    /// to the next `Stream::Item` if available. A subsequent
    /// call to `poll` will return the owned item.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct Peekable<St: Stream> {
        #[pin]
        stream: Fuse<St>,
        peeked: Option<St::Item>,
    }
}

impl<St: Stream> Peekable<St> {
    pub(super) fn new(stream: St) -> Self {
        Self { stream: stream.fuse(), peeked: None }
    }

    delegate_access_inner!(stream, St, (.));

    /// Produces a future which retrieves a reference to the next item
    /// in the stream, or `None` if the underlying stream terminates.
    pub fn peek(self: Pin<&mut Self>) -> Peek<'_, St> {
        Peek { inner: Some(self) }
    }

    /// Peek retrieves a reference to the next item in the stream.
    ///
    /// This method polls the underlying stream and return either a reference
    /// to the next item if the stream is ready or passes through any errors.
    pub fn poll_peek(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<&St::Item>> {
        let mut this = self.project();

        Poll::Ready(loop {
            if this.peeked.is_some() {
                break this.peeked.as_ref();
            } else if let Some(item) = ready!(this.stream.as_mut().poll_next(cx)) {
                *this.peeked = Some(item);
            } else {
                break None;
            }
        })
    }

    /// Produces a future which retrieves a mutable reference to the next item
    /// in the stream, or `None` if the underlying stream terminates.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    /// use futures::pin_mut;
    ///
    /// let stream = stream::iter(vec![1, 2, 3]).peekable();
    /// pin_mut!(stream);
    ///
    /// assert_eq!(stream.as_mut().peek_mut().await, Some(&mut 1));
    /// assert_eq!(stream.as_mut().next().await, Some(1));
    ///
    /// // Peek into the stream and modify the value which will be returned next
    /// if let Some(p) = stream.as_mut().peek_mut().await {
    ///     if *p == 2 {
    ///         *p = 5;
    ///     }
    /// }
    ///
    /// assert_eq!(stream.collect::<Vec<_>>().await, vec![5, 3]);
    /// # });
    /// ```
    pub fn peek_mut(self: Pin<&mut Self>) -> PeekMut<'_, St> {
        PeekMut { inner: Some(self) }
    }

    /// Peek retrieves a mutable reference to the next item in the stream.
    pub fn poll_peek_mut(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<&mut St::Item>> {
        let mut this = self.project();

        Poll::Ready(loop {
            if this.peeked.is_some() {
                break this.peeked.as_mut();
            } else if let Some(item) = ready!(this.stream.as_mut().poll_next(cx)) {
                *this.peeked = Some(item);
            } else {
                break None;
            }
        })
    }

    /// Creates a future which will consume and return the next value of this
    /// stream if a condition is true.
    ///
    /// If `func` returns `true` for the next value of this stream, consume and
    /// return it. Otherwise, return `None`.
    ///
    /// # Examples
    ///
    /// Consume a number if it's equal to 0.
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    /// use futures::pin_mut;
    ///
    /// let stream = stream::iter(0..5).peekable();
    /// pin_mut!(stream);
    /// // The first item of the stream is 0; consume it.
    /// assert_eq!(stream.as_mut().next_if(|&x| x == 0).await, Some(0));
    /// // The next item returned is now 1, so `consume` will return `false`.
    /// assert_eq!(stream.as_mut().next_if(|&x| x == 0).await, None);
    /// // `next_if` saves the value of the next item if it was not equal to `expected`.
    /// assert_eq!(stream.next().await, Some(1));
    /// # });
    /// ```
    ///
    /// Consume any number less than 10.
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    /// use futures::pin_mut;
    ///
    /// let stream = stream::iter(1..20).peekable();
    /// pin_mut!(stream);
    /// // Consume all numbers less than 10
    /// while stream.as_mut().next_if(|&x| x < 10).await.is_some() {}
    /// // The next value returned will be 10
    /// assert_eq!(stream.next().await, Some(10));
    /// # });
    /// ```
    pub fn next_if<F>(self: Pin<&mut Self>, func: F) -> NextIf<'_, St, F>
    where
        F: FnOnce(&St::Item) -> bool,
    {
        NextIf { inner: Some((self, func)) }
    }

    /// Creates a future which will consume and return the next item if it is
    /// equal to `expected`.
    ///
    /// # Example
    ///
    /// Consume a number if it's equal to 0.
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::stream::{self, StreamExt};
    /// use futures::pin_mut;
    ///
    /// let stream = stream::iter(0..5).peekable();
    /// pin_mut!(stream);
    /// // The first item of the stream is 0; consume it.
    /// assert_eq!(stream.as_mut().next_if_eq(&0).await, Some(0));
    /// // The next item returned is now 1, so `consume` will return `false`.
    /// assert_eq!(stream.as_mut().next_if_eq(&0).await, None);
    /// // `next_if_eq` saves the value of the next item if it was not equal to `expected`.
    /// assert_eq!(stream.next().await, Some(1));
    /// # });
    /// ```
    pub fn next_if_eq<'a, T>(self: Pin<&'a mut Self>, expected: &'a T) -> NextIfEq<'a, St, T>
    where
        T: ?Sized,
        St::Item: PartialEq<T>,
    {
        NextIfEq {
            inner: NextIf { inner: Some((self, NextIfEqFn { expected, _next: PhantomData })) },
        }
    }
}

impl<St: Stream> FusedStream for Peekable<St> {
    fn is_terminated(&self) -> bool {
        self.peeked.is_none() && self.stream.is_terminated()
    }
}

impl<S: Stream> Stream for Peekable<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        if let Some(item) = this.peeked.take() {
            return Poll::Ready(Some(item));
        }
        this.stream.poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let peek_len = usize::from(self.peeked.is_some());
        let (lower, upper) = self.stream.size_hint();
        let lower = lower.saturating_add(peek_len);
        let upper = match upper {
            Some(x) => x.checked_add(peek_len),
            None => None,
        };
        (lower, upper)
    }
}

// Forwarding impl of Sink from the underlying stream
#[cfg(feature = "sink")]
impl<S, Item> Sink<Item> for Peekable<S>
where
    S: Sink<Item> + Stream,
{
    type Error = S::Error;

    delegate_sink!(stream, Item);
}

pin_project! {
    /// Future for the [`Peekable::peek`](self::Peekable::peek) method.
    #[must_use = "futures do nothing unless polled"]
    pub struct Peek<'a, St: Stream> {
        inner: Option<Pin<&'a mut Peekable<St>>>,
    }
}

impl<St> fmt::Debug for Peek<'_, St>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Peek").field("inner", &self.inner).finish()
    }
}

impl<St: Stream> FusedFuture for Peek<'_, St> {
    fn is_terminated(&self) -> bool {
        self.inner.is_none()
    }
}

impl<'a, St> Future for Peek<'a, St>
where
    St: Stream,
{
    type Output = Option<&'a St::Item>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = self.project().inner;
        if let Some(peekable) = inner {
            ready!(peekable.as_mut().poll_peek(cx));

            inner.take().unwrap().poll_peek(cx)
        } else {
            panic!("Peek polled after completion")
        }
    }
}

pin_project! {
    /// Future for the [`Peekable::peek_mut`](self::Peekable::peek_mut) method.
    #[must_use = "futures do nothing unless polled"]
    pub struct PeekMut<'a, St: Stream> {
        inner: Option<Pin<&'a mut Peekable<St>>>,
    }
}

impl<St> fmt::Debug for PeekMut<'_, St>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeekMut").field("inner", &self.inner).finish()
    }
}

impl<St: Stream> FusedFuture for PeekMut<'_, St> {
    fn is_terminated(&self) -> bool {
        self.inner.is_none()
    }
}

impl<'a, St> Future for PeekMut<'a, St>
where
    St: Stream,
{
    type Output = Option<&'a mut St::Item>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = self.project().inner;
        if let Some(peekable) = inner {
            ready!(peekable.as_mut().poll_peek_mut(cx));

            inner.take().unwrap().poll_peek_mut(cx)
        } else {
            panic!("PeekMut polled after completion")
        }
    }
}

pin_project! {
    /// Future for the [`Peekable::next_if`](self::Peekable::next_if) method.
    #[must_use = "futures do nothing unless polled"]
    pub struct NextIf<'a, St: Stream, F> {
        inner: Option<(Pin<&'a mut Peekable<St>>, F)>,
    }
}

impl<St, F> fmt::Debug for NextIf<'_, St, F>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NextIf").field("inner", &self.inner.as_ref().map(|(s, _f)| s)).finish()
    }
}

#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
impl<St, F> FusedFuture for NextIf<'_, St, F>
where
    St: Stream,
    F: for<'a> FnOnce1<&'a St::Item, Output = bool>,
{
    fn is_terminated(&self) -> bool {
        self.inner.is_none()
    }
}

#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
impl<St, F> Future for NextIf<'_, St, F>
where
    St: Stream,
    F: for<'a> FnOnce1<&'a St::Item, Output = bool>,
{
    type Output = Option<St::Item>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = self.project().inner;
        if let Some((peekable, _)) = inner {
            let res = ready!(peekable.as_mut().poll_next(cx));

            let (peekable, func) = inner.take().unwrap();
            match res {
                Some(ref matched) if func.call_once(matched) => Poll::Ready(res),
                other => {
                    let peekable = peekable.project();
                    // Since we called `self.next()`, we consumed `self.peeked`.
                    assert!(peekable.peeked.is_none());
                    *peekable.peeked = other;
                    Poll::Ready(None)
                }
            }
        } else {
            panic!("NextIf polled after completion")
        }
    }
}

pin_project! {
    /// Future for the [`Peekable::next_if_eq`](self::Peekable::next_if_eq) method.
    #[must_use = "futures do nothing unless polled"]
    pub struct NextIfEq<'a, St: Stream, T: ?Sized> {
        #[pin]
        inner: NextIf<'a, St, NextIfEqFn<'a, T, St::Item>>,
    }
}

impl<St, T> fmt::Debug for NextIfEq<'_, St, T>
where
    St: Stream + fmt::Debug,
    St::Item: fmt::Debug,
    T: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NextIfEq")
            .field("inner", &self.inner.inner.as_ref().map(|(s, _f)| s))
            .finish()
    }
}

impl<St, T> FusedFuture for NextIfEq<'_, St, T>
where
    St: Stream,
    T: ?Sized,
    St::Item: PartialEq<T>,
{
    fn is_terminated(&self) -> bool {
        self.inner.is_terminated()
    }
}

impl<St, T> Future for NextIfEq<'_, St, T>
where
    St: Stream,
    T: ?Sized,
    St::Item: PartialEq<T>,
{
    type Output = Option<St::Item>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

struct NextIfEqFn<'a, T: ?Sized, Item> {
    expected: &'a T,
    _next: PhantomData<Item>,
}

impl<T, Item> FnOnce1<&Item> for NextIfEqFn<'_, T, Item>
where
    T: ?Sized,
    Item: PartialEq<T>,
{
    type Output = bool;

    fn call_once(self, next: &Item) -> Self::Output {
        next == self.expected
    }
}
