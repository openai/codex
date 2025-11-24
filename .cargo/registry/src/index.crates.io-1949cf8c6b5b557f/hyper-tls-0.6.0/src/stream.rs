use std::fmt;
use std::io;
use std::io::IoSlice;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::rt::{Read, ReadBufCursor, Write};

use hyper_util::{
    client::legacy::connect::{Connected, Connection},
    rt::TokioIo,
};
pub use tokio_native_tls::TlsStream;

/// A stream that might be protected with TLS.
pub enum MaybeHttpsStream<T> {
    /// A stream over plain text.
    Http(T),
    /// A stream protected with TLS.
    Https(TokioIo<TlsStream<TokioIo<T>>>),
}

// ===== impl MaybeHttpsStream =====

impl<T: fmt::Debug> fmt::Debug for MaybeHttpsStream<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MaybeHttpsStream::Http(s) => f.debug_tuple("Http").field(s).finish(),
            MaybeHttpsStream::Https(s) => f.debug_tuple("Https").field(s).finish(),
        }
    }
}

impl<T> From<T> for MaybeHttpsStream<T> {
    fn from(inner: T) -> Self {
        MaybeHttpsStream::Http(inner)
    }
}

impl<T> From<TlsStream<TokioIo<T>>> for MaybeHttpsStream<T> {
    fn from(inner: TlsStream<TokioIo<T>>) -> Self {
        MaybeHttpsStream::Https(TokioIo::new(inner))
    }
}

impl<T> From<TokioIo<TlsStream<TokioIo<T>>>> for MaybeHttpsStream<T> {
    fn from(inner: TokioIo<TlsStream<TokioIo<T>>>) -> Self {
        MaybeHttpsStream::Https(inner)
    }
}

impl<T: Read + Write + Unpin> Read for MaybeHttpsStream<T> {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: ReadBufCursor<'_>,
    ) -> Poll<Result<(), io::Error>> {
        match Pin::get_mut(self) {
            MaybeHttpsStream::Http(s) => Pin::new(s).poll_read(cx, buf),
            MaybeHttpsStream::Https(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl<T: Write + Read + Unpin> Write for MaybeHttpsStream<T> {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        match Pin::get_mut(self) {
            MaybeHttpsStream::Http(s) => Pin::new(s).poll_write(cx, buf),
            MaybeHttpsStream::Https(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        match Pin::get_mut(self) {
            MaybeHttpsStream::Http(s) => Pin::new(s).poll_write_vectored(cx, bufs),
            MaybeHttpsStream::Https(s) => Pin::new(s).poll_write_vectored(cx, bufs),
        }
    }

    fn is_write_vectored(&self) -> bool {
        match self {
            MaybeHttpsStream::Http(s) => s.is_write_vectored(),
            MaybeHttpsStream::Https(s) => s.is_write_vectored(),
        }
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match Pin::get_mut(self) {
            MaybeHttpsStream::Http(s) => Pin::new(s).poll_flush(cx),
            MaybeHttpsStream::Https(s) => Pin::new(s).poll_flush(cx),
        }
    }

    #[inline]
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match Pin::get_mut(self) {
            MaybeHttpsStream::Http(s) => Pin::new(s).poll_shutdown(cx),
            MaybeHttpsStream::Https(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

impl<T: Write + Read + Connection + Unpin> Connection for MaybeHttpsStream<T> {
    fn connected(&self) -> Connected {
        match self {
            MaybeHttpsStream::Http(s) => s.connected(),
            MaybeHttpsStream::Https(s) => {
                let c = s.inner().get_ref().get_ref().get_ref().inner().connected();
                #[cfg(feature = "alpn")]
                {
                    if negotiated_h2(s.inner().get_ref()) {
                        return c.negotiated_h2();
                    }
                }
                c
            }
        }
    }
}

#[cfg(feature = "alpn")]
fn negotiated_h2<T: std::io::Read + std::io::Write>(s: &native_tls::TlsStream<T>) -> bool {
    s.negotiated_alpn()
        .unwrap_or(None)
        .map(|list| list == &b"h2"[..])
        .unwrap_or(false)
}
