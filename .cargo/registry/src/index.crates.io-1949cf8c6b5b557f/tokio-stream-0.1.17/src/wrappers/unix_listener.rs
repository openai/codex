use crate::Stream;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::net::{UnixListener, UnixStream};

/// A wrapper around [`UnixListener`] that implements [`Stream`].
///
/// [`UnixListener`]: struct@tokio::net::UnixListener
/// [`Stream`]: trait@crate::Stream
#[derive(Debug)]
#[cfg_attr(docsrs, doc(cfg(all(unix, feature = "net"))))]
pub struct UnixListenerStream {
    inner: UnixListener,
}

impl UnixListenerStream {
    /// Create a new `UnixListenerStream`.
    pub fn new(listener: UnixListener) -> Self {
        Self { inner: listener }
    }

    /// Get back the inner `UnixListener`.
    pub fn into_inner(self) -> UnixListener {
        self.inner
    }
}

impl Stream for UnixListenerStream {
    type Item = io::Result<UnixStream>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<io::Result<UnixStream>>> {
        match self.inner.poll_accept(cx) {
            Poll::Ready(Ok((stream, _))) => Poll::Ready(Some(Ok(stream))),
            Poll::Ready(Err(err)) => Poll::Ready(Some(Err(err))),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsRef<UnixListener> for UnixListenerStream {
    fn as_ref(&self) -> &UnixListener {
        &self.inner
    }
}

impl AsMut<UnixListener> for UnixListenerStream {
    fn as_mut(&mut self) -> &mut UnixListener {
        &mut self.inner
    }
}
