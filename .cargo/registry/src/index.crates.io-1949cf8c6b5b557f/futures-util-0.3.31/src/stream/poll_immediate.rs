use core::pin::Pin;
use futures_core::task::{Context, Poll};
use futures_core::Stream;
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [poll_immediate](poll_immediate()) function.
    ///
    /// It will never return [Poll::Pending](core::task::Poll::Pending)
    #[derive(Debug, Clone)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct PollImmediate<S> {
        #[pin]
        stream: Option<S>
    }
}

impl<T, S> Stream for PollImmediate<S>
where
    S: Stream<Item = T>,
{
    type Item = Poll<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        let stream = match this.stream.as_mut().as_pin_mut() {
            // inner is gone, so we can continue to signal that the stream is closed.
            None => return Poll::Ready(None),
            Some(inner) => inner,
        };

        match stream.poll_next(cx) {
            Poll::Ready(Some(t)) => Poll::Ready(Some(Poll::Ready(t))),
            Poll::Ready(None) => {
                this.stream.set(None);
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Ready(Some(Poll::Pending)),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.as_ref().map_or((0, Some(0)), Stream::size_hint)
    }
}

impl<S: Stream> super::FusedStream for PollImmediate<S> {
    fn is_terminated(&self) -> bool {
        self.stream.is_none()
    }
}

/// Creates a new stream that always immediately returns [Poll::Ready](core::task::Poll::Ready) when awaiting it.
///
/// This is useful when immediacy is more important than waiting for the next item to be ready.
///
/// # Examples
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::stream::{self, StreamExt};
/// use futures::task::Poll;
///
/// let mut r = stream::poll_immediate(Box::pin(stream::iter(1_u32..3)));
/// assert_eq!(r.next().await, Some(Poll::Ready(1)));
/// assert_eq!(r.next().await, Some(Poll::Ready(2)));
/// assert_eq!(r.next().await, None);
///
/// let mut p = stream::poll_immediate(Box::pin(stream::once(async {
///     futures::pending!();
///     42_u8
/// })));
/// assert_eq!(p.next().await, Some(Poll::Pending));
/// assert_eq!(p.next().await, Some(Poll::Ready(42)));
/// assert_eq!(p.next().await, None);
/// # });
/// ```
pub fn poll_immediate<S: Stream>(s: S) -> PollImmediate<S> {
    super::assert_stream::<Poll<S::Item>, PollImmediate<S>>(PollImmediate { stream: Some(s) })
}
