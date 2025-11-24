use crate::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::signal::unix::Signal;

/// A wrapper around [`Signal`] that implements [`Stream`].
///
/// [`Signal`]: struct@tokio::signal::unix::Signal
/// [`Stream`]: trait@crate::Stream
#[derive(Debug)]
#[cfg_attr(docsrs, doc(cfg(all(unix, feature = "signal"))))]
pub struct SignalStream {
    inner: Signal,
}

impl SignalStream {
    /// Create a new `SignalStream`.
    pub fn new(signal: Signal) -> Self {
        Self { inner: signal }
    }

    /// Get back the inner `Signal`.
    pub fn into_inner(self) -> Signal {
        self.inner
    }
}

impl Stream for SignalStream {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<()>> {
        self.inner.poll_recv(cx)
    }
}

impl AsRef<Signal> for SignalStream {
    fn as_ref(&self) -> &Signal {
        &self.inner
    }
}

impl AsMut<Signal> for SignalStream {
    fn as_mut(&mut self) -> &mut Signal {
        &mut self.inner
    }
}
