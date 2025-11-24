use core::pin::Pin;
use futures_core::ready;
use futures_core::stream::TryStream;
use futures_core::task::{Context, Poll};
use futures_io::{AsyncBufRead, AsyncRead, AsyncWrite};
use pin_project_lite::pin_project;
use std::cmp;
use std::io::{Error, Result};

pin_project! {
    /// Reader for the [`into_async_read`](super::TryStreamExt::into_async_read) method.
    #[derive(Debug)]
    #[must_use = "readers do nothing unless polled"]
    #[cfg_attr(docsrs, doc(cfg(feature = "io")))]
    pub struct IntoAsyncRead<St>
    where
        St: TryStream<Error = Error>,
        St::Ok: AsRef<[u8]>,
    {
        #[pin]
        stream: St,
        state: ReadState<St::Ok>,
    }
}

#[derive(Debug)]
enum ReadState<T: AsRef<[u8]>> {
    Ready { chunk: T, chunk_start: usize },
    PendingChunk,
    Eof,
}

impl<St> IntoAsyncRead<St>
where
    St: TryStream<Error = Error>,
    St::Ok: AsRef<[u8]>,
{
    pub(super) fn new(stream: St) -> Self {
        Self { stream, state: ReadState::PendingChunk }
    }
}

impl<St> AsyncRead for IntoAsyncRead<St>
where
    St: TryStream<Error = Error>,
    St::Ok: AsRef<[u8]>,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        let mut this = self.project();

        loop {
            match this.state {
                ReadState::Ready { chunk, chunk_start } => {
                    let chunk = chunk.as_ref();
                    let len = cmp::min(buf.len(), chunk.len() - *chunk_start);

                    buf[..len].copy_from_slice(&chunk[*chunk_start..*chunk_start + len]);
                    *chunk_start += len;

                    if chunk.len() == *chunk_start {
                        *this.state = ReadState::PendingChunk;
                    }

                    return Poll::Ready(Ok(len));
                }
                ReadState::PendingChunk => match ready!(this.stream.as_mut().try_poll_next(cx)) {
                    Some(Ok(chunk)) => {
                        if !chunk.as_ref().is_empty() {
                            *this.state = ReadState::Ready { chunk, chunk_start: 0 };
                        }
                    }
                    Some(Err(err)) => {
                        *this.state = ReadState::Eof;
                        return Poll::Ready(Err(err));
                    }
                    None => {
                        *this.state = ReadState::Eof;
                        return Poll::Ready(Ok(0));
                    }
                },
                ReadState::Eof => {
                    return Poll::Ready(Ok(0));
                }
            }
        }
    }
}

impl<St> AsyncWrite for IntoAsyncRead<St>
where
    St: TryStream<Error = Error> + AsyncWrite,
    St::Ok: AsRef<[u8]>,
{
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        let this = self.project();
        this.stream.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let this = self.project();
        this.stream.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let this = self.project();
        this.stream.poll_close(cx)
    }
}

impl<St> AsyncBufRead for IntoAsyncRead<St>
where
    St: TryStream<Error = Error>,
    St::Ok: AsRef<[u8]>,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8]>> {
        let mut this = self.project();

        while let ReadState::PendingChunk = this.state {
            match ready!(this.stream.as_mut().try_poll_next(cx)) {
                Some(Ok(chunk)) => {
                    if !chunk.as_ref().is_empty() {
                        *this.state = ReadState::Ready { chunk, chunk_start: 0 };
                    }
                }
                Some(Err(err)) => {
                    *this.state = ReadState::Eof;
                    return Poll::Ready(Err(err));
                }
                None => {
                    *this.state = ReadState::Eof;
                    return Poll::Ready(Ok(&[]));
                }
            }
        }

        if let &mut ReadState::Ready { ref chunk, chunk_start } = this.state {
            let chunk = chunk.as_ref();
            return Poll::Ready(Ok(&chunk[chunk_start..]));
        }

        // To get to this point we must be in ReadState::Eof
        Poll::Ready(Ok(&[]))
    }

    fn consume(self: Pin<&mut Self>, amount: usize) {
        let this = self.project();

        // https://github.com/rust-lang/futures-rs/pull/1556#discussion_r281644295
        if amount == 0 {
            return;
        }
        if let ReadState::Ready { chunk, chunk_start } = this.state {
            *chunk_start += amount;
            debug_assert!(*chunk_start <= chunk.as_ref().len());
            if *chunk_start >= chunk.as_ref().len() {
                *this.state = ReadState::PendingChunk;
            }
        } else {
            debug_assert!(false, "Attempted to consume from IntoAsyncRead without chunk");
        }
    }
}
