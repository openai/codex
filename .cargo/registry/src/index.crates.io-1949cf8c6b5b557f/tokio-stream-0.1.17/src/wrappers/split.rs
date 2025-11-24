use crate::Stream;
use pin_project_lite::pin_project;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncBufRead, Split};

pin_project! {
    /// A wrapper around [`tokio::io::Split`] that implements [`Stream`].
    ///
    /// [`tokio::io::Split`]: struct@tokio::io::Split
    /// [`Stream`]: trait@crate::Stream
    #[derive(Debug)]
    #[cfg_attr(docsrs, doc(cfg(feature = "io-util")))]
    pub struct SplitStream<R> {
        #[pin]
        inner: Split<R>,
    }
}

impl<R> SplitStream<R> {
    /// Create a new `SplitStream`.
    pub fn new(split: Split<R>) -> Self {
        Self { inner: split }
    }

    /// Get back the inner `Split`.
    pub fn into_inner(self) -> Split<R> {
        self.inner
    }

    /// Obtain a pinned reference to the inner `Split<R>`.
    pub fn as_pin_mut(self: Pin<&mut Self>) -> Pin<&mut Split<R>> {
        self.project().inner
    }
}

impl<R: AsyncBufRead> Stream for SplitStream<R> {
    type Item = io::Result<Vec<u8>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project()
            .inner
            .poll_next_segment(cx)
            .map(Result::transpose)
    }
}

impl<R> AsRef<Split<R>> for SplitStream<R> {
    fn as_ref(&self) -> &Split<R> {
        &self.inner
    }
}

impl<R> AsMut<Split<R>> for SplitStream<R> {
    fn as_mut(&mut self) -> &mut Split<R> {
        &mut self.inner
    }
}
