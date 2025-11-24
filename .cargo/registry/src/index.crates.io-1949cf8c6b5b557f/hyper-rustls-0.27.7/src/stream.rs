// Copied from hyperium/hyper-tls#62e3376/src/stream.rs
use std::fmt;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::rt;
use hyper_util::client::legacy::connect::{Connected, Connection};

use hyper_util::rt::TokioIo;
use tokio_rustls::client::TlsStream;

/// A stream that might be protected with TLS.
#[allow(clippy::large_enum_variant)]
pub enum MaybeHttpsStream<T> {
    /// A stream over plain text.
    Http(T),
    /// A stream protected with TLS.
    Https(TokioIo<TlsStream<TokioIo<T>>>),
}

impl<T: rt::Read + rt::Write + Connection + Unpin> Connection for MaybeHttpsStream<T> {
    fn connected(&self) -> Connected {
        match self {
            Self::Http(s) => s.connected(),
            Self::Https(s) => {
                let (tcp, tls) = s.inner().get_ref();
                if tls.alpn_protocol() == Some(b"h2") {
                    tcp.inner().connected().negotiated_h2()
                } else {
                    tcp.inner().connected()
                }
            }
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for MaybeHttpsStream<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Http(..) => f.pad("Http(..)"),
            Self::Https(..) => f.pad("Https(..)"),
        }
    }
}

impl<T> From<T> for MaybeHttpsStream<T> {
    fn from(inner: T) -> Self {
        Self::Http(inner)
    }
}

impl<T> From<TlsStream<TokioIo<T>>> for MaybeHttpsStream<T> {
    fn from(inner: TlsStream<TokioIo<T>>) -> Self {
        Self::Https(TokioIo::new(inner))
    }
}

impl<T: rt::Read + rt::Write + Unpin> rt::Read for MaybeHttpsStream<T> {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), io::Error>> {
        match Pin::get_mut(self) {
            Self::Http(s) => Pin::new(s).poll_read(cx, buf),
            Self::Https(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl<T: rt::Write + rt::Read + Unpin> rt::Write for MaybeHttpsStream<T> {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        match Pin::get_mut(self) {
            Self::Http(s) => Pin::new(s).poll_write(cx, buf),
            Self::Https(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match Pin::get_mut(self) {
            Self::Http(s) => Pin::new(s).poll_flush(cx),
            Self::Https(s) => Pin::new(s).poll_flush(cx),
        }
    }

    #[inline]
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match Pin::get_mut(self) {
            Self::Http(s) => Pin::new(s).poll_shutdown(cx),
            Self::Https(s) => Pin::new(s).poll_shutdown(cx),
        }
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        match self {
            Self::Http(s) => s.is_write_vectored(),
            Self::Https(s) => s.is_write_vectored(),
        }
    }

    #[inline]
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        match Pin::get_mut(self) {
            Self::Http(s) => Pin::new(s).poll_write_vectored(cx, bufs),
            Self::Https(s) => Pin::new(s).poll_write_vectored(cx, bufs),
        }
    }
}
