use crate::Stream;
use pin_project_lite::pin_project;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncBufRead, Lines};

pin_project! {
    /// A wrapper around [`tokio::io::Lines`] that implements [`Stream`].
    ///
    /// [`tokio::io::Lines`]: struct@tokio::io::Lines
    /// [`Stream`]: trait@crate::Stream
    #[derive(Debug)]
    #[cfg_attr(docsrs, doc(cfg(feature = "io-util")))]
    pub struct LinesStream<R> {
        #[pin]
        inner: Lines<R>,
    }
}

impl<R> LinesStream<R> {
    /// Create a new `LinesStream`.
    pub fn new(lines: Lines<R>) -> Self {
        Self { inner: lines }
    }

    /// Get back the inner `Lines`.
    pub fn into_inner(self) -> Lines<R> {
        self.inner
    }

    /// Obtain a pinned reference to the inner `Lines<R>`.
    pub fn as_pin_mut(self: Pin<&mut Self>) -> Pin<&mut Lines<R>> {
        self.project().inner
    }
}

impl<R: AsyncBufRead> Stream for LinesStream<R> {
    type Item = io::Result<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project()
            .inner
            .poll_next_line(cx)
            .map(Result::transpose)
    }
}

impl<R> AsRef<Lines<R>> for LinesStream<R> {
    fn as_ref(&self) -> &Lines<R> {
        &self.inner
    }
}

impl<R> AsMut<Lines<R>> for LinesStream<R> {
    fn as_mut(&mut self) -> &mut Lines<R> {
        &mut self.inner
    }
}
