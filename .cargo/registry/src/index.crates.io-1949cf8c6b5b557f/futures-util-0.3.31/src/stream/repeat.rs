use super::assert_stream;
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};

/// Stream for the [`repeat`] function.
#[derive(Debug, Clone)]
#[must_use = "streams do nothing unless polled"]
pub struct Repeat<T> {
    item: T,
}

/// Create a stream which produces the same item repeatedly.
///
/// The stream never terminates. Note that you likely want to avoid
/// usage of `collect` or such on the returned stream as it will exhaust
/// available memory as it tries to just fill up all RAM.
///
/// ```
/// # futures::executor::block_on(async {
/// use futures::stream::{self, StreamExt};
///
/// let stream = stream::repeat(9);
/// assert_eq!(vec![9, 9, 9], stream.take(3).collect::<Vec<i32>>().await);
/// # });
/// ```
pub fn repeat<T>(item: T) -> Repeat<T>
where
    T: Clone,
{
    assert_stream::<T, _>(Repeat { item })
}

impl<T> Unpin for Repeat<T> {}

impl<T> Stream for Repeat<T>
where
    T: Clone,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(Some(self.item.clone()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

impl<T> FusedStream for Repeat<T>
where
    T: Clone,
{
    fn is_terminated(&self) -> bool {
        false
    }
}
