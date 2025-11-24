use core::pin::Pin;
use core::task::{Context, Poll};
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, Stream};
#[cfg(feature = "sink")]
use futures_sink::Sink;

/// Combines two different futures, streams, or sinks having the same associated types into a single type.
///
/// This is useful when conditionally choosing between two distinct future types:
///
/// ```rust
/// use futures::future::Either;
///
/// # futures::executor::block_on(async {
/// let cond = true;
///
/// let fut = if cond {
///     Either::Left(async move { 12 })
/// } else {
///     Either::Right(async move { 44 })
/// };
///
/// assert_eq!(fut.await, 12);
/// # })
/// ```
#[derive(Debug, Clone)]
pub enum Either<A, B> {
    /// First branch of the type
    Left(/* #[pin] */ A),
    /// Second branch of the type
    Right(/* #[pin] */ B),
}

impl<A, B> Either<A, B> {
    /// Convert `Pin<&Either<A, B>>` to `Either<Pin<&A>, Pin<&B>>`,
    /// pinned projections of the inner variants.
    pub fn as_pin_ref(self: Pin<&Self>) -> Either<Pin<&A>, Pin<&B>> {
        // SAFETY: We can use `new_unchecked` because the `inner` parts are
        // guaranteed to be pinned, as they come from `self` which is pinned.
        unsafe {
            match self.get_ref() {
                Self::Left(inner) => Either::Left(Pin::new_unchecked(inner)),
                Self::Right(inner) => Either::Right(Pin::new_unchecked(inner)),
            }
        }
    }

    /// Convert `Pin<&mut Either<A, B>>` to `Either<Pin<&mut A>, Pin<&mut B>>`,
    /// pinned projections of the inner variants.
    pub fn as_pin_mut(self: Pin<&mut Self>) -> Either<Pin<&mut A>, Pin<&mut B>> {
        // SAFETY: `get_unchecked_mut` is fine because we don't move anything.
        // We can use `new_unchecked` because the `inner` parts are guaranteed
        // to be pinned, as they come from `self` which is pinned, and we never
        // offer an unpinned `&mut A` or `&mut B` through `Pin<&mut Self>`. We
        // also don't have an implementation of `Drop`, nor manual `Unpin`.
        unsafe {
            match self.get_unchecked_mut() {
                Self::Left(inner) => Either::Left(Pin::new_unchecked(inner)),
                Self::Right(inner) => Either::Right(Pin::new_unchecked(inner)),
            }
        }
    }
}

impl<A, B, T> Either<(T, A), (T, B)> {
    /// Factor out a homogeneous type from an either of pairs.
    ///
    /// Here, the homogeneous type is the first element of the pairs.
    pub fn factor_first(self) -> (T, Either<A, B>) {
        match self {
            Self::Left((x, a)) => (x, Either::Left(a)),
            Self::Right((x, b)) => (x, Either::Right(b)),
        }
    }
}

impl<A, B, T> Either<(A, T), (B, T)> {
    /// Factor out a homogeneous type from an either of pairs.
    ///
    /// Here, the homogeneous type is the second element of the pairs.
    pub fn factor_second(self) -> (Either<A, B>, T) {
        match self {
            Self::Left((a, x)) => (Either::Left(a), x),
            Self::Right((b, x)) => (Either::Right(b), x),
        }
    }
}

impl<T> Either<T, T> {
    /// Extract the value of an either over two equivalent types.
    pub fn into_inner(self) -> T {
        match self {
            Self::Left(x) | Self::Right(x) => x,
        }
    }
}

impl<A, B> Future for Either<A, B>
where
    A: Future,
    B: Future<Output = A::Output>,
{
    type Output = A::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.as_pin_mut() {
            Either::Left(x) => x.poll(cx),
            Either::Right(x) => x.poll(cx),
        }
    }
}

impl<A, B> FusedFuture for Either<A, B>
where
    A: FusedFuture,
    B: FusedFuture<Output = A::Output>,
{
    fn is_terminated(&self) -> bool {
        match self {
            Self::Left(x) => x.is_terminated(),
            Self::Right(x) => x.is_terminated(),
        }
    }
}

impl<A, B> Stream for Either<A, B>
where
    A: Stream,
    B: Stream<Item = A::Item>,
{
    type Item = A::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.as_pin_mut() {
            Either::Left(x) => x.poll_next(cx),
            Either::Right(x) => x.poll_next(cx),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Left(x) => x.size_hint(),
            Self::Right(x) => x.size_hint(),
        }
    }
}

impl<A, B> FusedStream for Either<A, B>
where
    A: FusedStream,
    B: FusedStream<Item = A::Item>,
{
    fn is_terminated(&self) -> bool {
        match self {
            Self::Left(x) => x.is_terminated(),
            Self::Right(x) => x.is_terminated(),
        }
    }
}

#[cfg(feature = "sink")]
impl<A, B, Item> Sink<Item> for Either<A, B>
where
    A: Sink<Item>,
    B: Sink<Item, Error = A::Error>,
{
    type Error = A::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.as_pin_mut() {
            Either::Left(x) => x.poll_ready(cx),
            Either::Right(x) => x.poll_ready(cx),
        }
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        match self.as_pin_mut() {
            Either::Left(x) => x.start_send(item),
            Either::Right(x) => x.start_send(item),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.as_pin_mut() {
            Either::Left(x) => x.poll_flush(cx),
            Either::Right(x) => x.poll_flush(cx),
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.as_pin_mut() {
            Either::Left(x) => x.poll_close(cx),
            Either::Right(x) => x.poll_close(cx),
        }
    }
}

#[cfg(feature = "io")]
#[cfg(feature = "std")]
mod if_std {
    use super::*;

    use futures_io::{
        AsyncBufRead, AsyncRead, AsyncSeek, AsyncWrite, IoSlice, IoSliceMut, Result, SeekFrom,
    };

    impl<A, B> AsyncRead for Either<A, B>
    where
        A: AsyncRead,
        B: AsyncRead,
    {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<Result<usize>> {
            match self.as_pin_mut() {
                Either::Left(x) => x.poll_read(cx, buf),
                Either::Right(x) => x.poll_read(cx, buf),
            }
        }

        fn poll_read_vectored(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &mut [IoSliceMut<'_>],
        ) -> Poll<Result<usize>> {
            match self.as_pin_mut() {
                Either::Left(x) => x.poll_read_vectored(cx, bufs),
                Either::Right(x) => x.poll_read_vectored(cx, bufs),
            }
        }
    }

    impl<A, B> AsyncWrite for Either<A, B>
    where
        A: AsyncWrite,
        B: AsyncWrite,
    {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize>> {
            match self.as_pin_mut() {
                Either::Left(x) => x.poll_write(cx, buf),
                Either::Right(x) => x.poll_write(cx, buf),
            }
        }

        fn poll_write_vectored(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<Result<usize>> {
            match self.as_pin_mut() {
                Either::Left(x) => x.poll_write_vectored(cx, bufs),
                Either::Right(x) => x.poll_write_vectored(cx, bufs),
            }
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
            match self.as_pin_mut() {
                Either::Left(x) => x.poll_flush(cx),
                Either::Right(x) => x.poll_flush(cx),
            }
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
            match self.as_pin_mut() {
                Either::Left(x) => x.poll_close(cx),
                Either::Right(x) => x.poll_close(cx),
            }
        }
    }

    impl<A, B> AsyncSeek for Either<A, B>
    where
        A: AsyncSeek,
        B: AsyncSeek,
    {
        fn poll_seek(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            pos: SeekFrom,
        ) -> Poll<Result<u64>> {
            match self.as_pin_mut() {
                Either::Left(x) => x.poll_seek(cx, pos),
                Either::Right(x) => x.poll_seek(cx, pos),
            }
        }
    }

    impl<A, B> AsyncBufRead for Either<A, B>
    where
        A: AsyncBufRead,
        B: AsyncBufRead,
    {
        fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8]>> {
            match self.as_pin_mut() {
                Either::Left(x) => x.poll_fill_buf(cx),
                Either::Right(x) => x.poll_fill_buf(cx),
            }
        }

        fn consume(self: Pin<&mut Self>, amt: usize) {
            match self.as_pin_mut() {
                Either::Left(x) => x.consume(amt),
                Either::Right(x) => x.consume(amt),
            }
        }
    }
}
