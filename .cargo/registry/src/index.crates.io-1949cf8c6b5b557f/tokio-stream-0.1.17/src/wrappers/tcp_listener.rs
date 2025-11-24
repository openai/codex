use crate::Stream;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::net::{TcpListener, TcpStream};

/// A wrapper around [`TcpListener`] that implements [`Stream`].
///
/// [`TcpListener`]: struct@tokio::net::TcpListener
/// [`Stream`]: trait@crate::Stream
#[derive(Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = "net")))]
pub struct TcpListenerStream {
    inner: TcpListener,
}

impl TcpListenerStream {
    /// Create a new `TcpListenerStream`.
    pub fn new(listener: TcpListener) -> Self {
        Self { inner: listener }
    }

    /// Get back the inner `TcpListener`.
    pub fn into_inner(self) -> TcpListener {
        self.inner
    }
}

impl Stream for TcpListenerStream {
    type Item = io::Result<TcpStream>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<io::Result<TcpStream>>> {
        match self.inner.poll_accept(cx) {
            Poll::Ready(Ok((stream, _))) => Poll::Ready(Some(Ok(stream))),
            Poll::Ready(Err(err)) => Poll::Ready(Some(Err(err))),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsRef<TcpListener> for TcpListenerStream {
    fn as_ref(&self) -> &TcpListener {
        &self.inner
    }
}

impl AsMut<TcpListener> for TcpListenerStream {
    fn as_mut(&mut self) -> &mut TcpListener {
        &mut self.inner
    }
}
