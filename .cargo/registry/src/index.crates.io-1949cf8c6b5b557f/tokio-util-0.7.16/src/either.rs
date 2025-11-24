//! Module defining an Either type.
use std::{
    future::Future,
    io::SeekFrom,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncBufRead, AsyncRead, AsyncSeek, AsyncWrite, ReadBuf, Result};

/// Combines two different futures, streams, or sinks having the same associated types into a single type.
///
/// This type implements common asynchronous traits such as [`Future`] and those in Tokio.
///
/// [`Future`]: std::future::Future
///
/// # Example
///
/// The following code will not work:
///
/// ```compile_fail
/// # fn some_condition() -> bool { true }
/// # async fn some_async_function() -> u32 { 10 }
/// # async fn other_async_function() -> u32 { 20 }
/// #[tokio::main]
/// async fn main() {
///     let result = if some_condition() {
///         some_async_function()
///     } else {
///         other_async_function() // <- Will print: "`if` and `else` have incompatible types"
///     };
///
///     println!("Result is {}", result.await);
/// }
/// ```
///
// This is because although the output types for both futures is the same, the exact future
// types are different, but the compiler must be able to choose a single type for the
// `result` variable.
///
/// When the output type is the same, we can wrap each future in `Either` to avoid the
/// issue:
///
/// ```
/// use tokio_util::either::Either;
/// # fn some_condition() -> bool { true }
/// # async fn some_async_function() -> u32 { 10 }
/// # async fn other_async_function() -> u32 { 20 }
///
/// #[tokio::main]
/// async fn main() {
///     let result = if some_condition() {
///         Either::Left(some_async_function())
///     } else {
///         Either::Right(other_async_function())
///     };
///
///     let value = result.await;
///     println!("Result is {}", value);
///     # assert_eq!(value, 10);
/// }
/// ```
#[allow(missing_docs)] // Doc-comments for variants in this particular case don't make much sense.
#[derive(Debug, Clone)]
pub enum Either<L, R> {
    Left(L),
    Right(R),
}

/// A small helper macro which reduces amount of boilerplate in the actual trait method implementation.
/// It takes an invocation of method as an argument (e.g. `self.poll(cx)`), and redirects it to either
/// enum variant held in `self`.
macro_rules! delegate_call {
    ($self:ident.$method:ident($($args:ident),+)) => {
        unsafe {
            match $self.get_unchecked_mut() {
                Self::Left(l) => Pin::new_unchecked(l).$method($($args),+),
                Self::Right(r) => Pin::new_unchecked(r).$method($($args),+),
            }
        }
    }
}

impl<L, R, O> Future for Either<L, R>
where
    L: Future<Output = O>,
    R: Future<Output = O>,
{
    type Output = O;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        delegate_call!(self.poll(cx))
    }
}

impl<L, R> AsyncRead for Either<L, R>
where
    L: AsyncRead,
    R: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        delegate_call!(self.poll_read(cx, buf))
    }
}

impl<L, R> AsyncBufRead for Either<L, R>
where
    L: AsyncBufRead,
    R: AsyncBufRead,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8]>> {
        delegate_call!(self.poll_fill_buf(cx))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        delegate_call!(self.consume(amt));
    }
}

impl<L, R> AsyncSeek for Either<L, R>
where
    L: AsyncSeek,
    R: AsyncSeek,
{
    fn start_seek(self: Pin<&mut Self>, position: SeekFrom) -> Result<()> {
        delegate_call!(self.start_seek(position))
    }

    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<u64>> {
        delegate_call!(self.poll_complete(cx))
    }
}

impl<L, R> AsyncWrite for Either<L, R>
where
    L: AsyncWrite,
    R: AsyncWrite,
{
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        delegate_call!(self.poll_write(cx, buf))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<tokio::io::Result<()>> {
        delegate_call!(self.poll_flush(cx))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<tokio::io::Result<()>> {
        delegate_call!(self.poll_shutdown(cx))
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<std::result::Result<usize, std::io::Error>> {
        delegate_call!(self.poll_write_vectored(cx, bufs))
    }

    fn is_write_vectored(&self) -> bool {
        match self {
            Self::Left(l) => l.is_write_vectored(),
            Self::Right(r) => r.is_write_vectored(),
        }
    }
}

impl<L, R> futures_core::stream::Stream for Either<L, R>
where
    L: futures_core::stream::Stream,
    R: futures_core::stream::Stream<Item = L::Item>,
{
    type Item = L::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        delegate_call!(self.poll_next(cx))
    }
}

impl<L, R, Item, Error> futures_sink::Sink<Item> for Either<L, R>
where
    L: futures_sink::Sink<Item, Error = Error>,
    R: futures_sink::Sink<Item, Error = Error>,
{
    type Error = Error;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::result::Result<(), Self::Error>> {
        delegate_call!(self.poll_ready(cx))
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> std::result::Result<(), Self::Error> {
        delegate_call!(self.start_send(item))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::result::Result<(), Self::Error>> {
        delegate_call!(self.poll_flush(cx))
    }

    fn poll_close(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::result::Result<(), Self::Error>> {
        delegate_call!(self.poll_close(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{repeat, AsyncReadExt, Repeat};
    use tokio_stream::{once, Once, StreamExt};

    #[tokio::test]
    async fn either_is_stream() {
        let mut either: Either<Once<u32>, Once<u32>> = Either::Left(once(1));

        assert_eq!(Some(1u32), either.next().await);
    }

    #[tokio::test]
    async fn either_is_async_read() {
        let mut buffer = [0; 3];
        let mut either: Either<Repeat, Repeat> = Either::Right(repeat(0b101));

        either.read_exact(&mut buffer).await.unwrap();
        assert_eq!(buffer, [0b101, 0b101, 0b101]);
    }
}
