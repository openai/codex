use crate::io::{AsyncRead, ReadBuf};

use pin_project_lite::pin_project;
use std::future::Future;
use std::io;
use std::marker::PhantomPinned;
use std::marker::Unpin;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

/// A future which can be used to easily read exactly enough bytes to fill
/// a buffer.
///
/// Created by the [`AsyncReadExt::read_exact`][read_exact].
/// [`read_exact`]: [`crate::io::AsyncReadExt::read_exact`]
pub(crate) fn read_exact<'a, A>(reader: &'a mut A, buf: &'a mut [u8]) -> ReadExact<'a, A>
where
    A: AsyncRead + Unpin + ?Sized,
{
    ReadExact {
        reader,
        buf: ReadBuf::new(buf),
        _pin: PhantomPinned,
    }
}

pin_project! {
    /// Creates a future which will read exactly enough bytes to fill `buf`,
    /// returning an error if EOF is hit sooner.
    ///
    /// On success the number of bytes is returned
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct ReadExact<'a, A: ?Sized> {
        reader: &'a mut A,
        buf: ReadBuf<'a>,
        // Make this future `!Unpin` for compatibility with async trait methods.
        #[pin]
        _pin: PhantomPinned,
    }
}

fn eof() -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, "early eof")
}

impl<A> Future for ReadExact<'_, A>
where
    A: AsyncRead + Unpin + ?Sized,
{
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<usize>> {
        let me = self.project();

        loop {
            // if our buffer is empty, then we need to read some data to continue.
            let rem = me.buf.remaining();
            if rem != 0 {
                ready!(Pin::new(&mut *me.reader).poll_read(cx, me.buf))?;
                if me.buf.remaining() == rem {
                    return Err(eof()).into();
                }
            } else {
                return Poll::Ready(Ok(me.buf.capacity()));
            }
        }
    }
}
