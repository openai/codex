use crate::Stream;
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// A `Stream` that wraps the values in an `Option`.
    ///
    /// Whenever the wrapped stream yields an item, this stream yields that item
    /// wrapped in `Some`. When the inner stream ends, then this stream first
    /// yields a `None` item, and then this stream will also end.
    ///
    /// # Example
    ///
    /// Using `StreamNotifyClose` to handle closed streams with `StreamMap`.
    ///
    /// ```
    /// use tokio_stream::{StreamExt, StreamMap, StreamNotifyClose};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut map = StreamMap::new();
    ///     let stream = StreamNotifyClose::new(tokio_stream::iter(vec![0, 1]));
    ///     let stream2 = StreamNotifyClose::new(tokio_stream::iter(vec![0, 1]));
    ///     map.insert(0, stream);
    ///     map.insert(1, stream2);
    ///     while let Some((key, val)) = map.next().await {
    ///         match val {
    ///             Some(val) => println!("got {val:?} from stream {key:?}"),
    ///             None => println!("stream {key:?} closed"),
    ///         }
    ///     }
    /// }
    /// ```
    #[must_use = "streams do nothing unless polled"]
    pub struct StreamNotifyClose<S> {
        #[pin]
        inner: Option<S>,
    }
}

impl<S> StreamNotifyClose<S> {
    /// Create a new `StreamNotifyClose`.
    pub fn new(stream: S) -> Self {
        Self {
            inner: Some(stream),
        }
    }

    /// Get back the inner `Stream`.
    ///
    /// Returns `None` if the stream has reached its end.
    pub fn into_inner(self) -> Option<S> {
        self.inner
    }
}

impl<S> Stream for StreamNotifyClose<S>
where
    S: Stream,
{
    type Item = Option<S::Item>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // We can't invoke poll_next after it ended, so we unset the inner stream as a marker.
        match self
            .as_mut()
            .project()
            .inner
            .as_pin_mut()
            .map(|stream| S::poll_next(stream, cx))
        {
            Some(Poll::Ready(Some(item))) => Poll::Ready(Some(Some(item))),
            Some(Poll::Ready(None)) => {
                self.project().inner.set(None);
                Poll::Ready(Some(None))
            }
            Some(Poll::Pending) => Poll::Pending,
            None => Poll::Ready(None),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if let Some(inner) = &self.inner {
            // We always return +1 because when there's stream there's atleast one more item.
            let (l, u) = inner.size_hint();
            (l.saturating_add(1), u.and_then(|u| u.checked_add(1)))
        } else {
            (0, Some(0))
        }
    }
}
