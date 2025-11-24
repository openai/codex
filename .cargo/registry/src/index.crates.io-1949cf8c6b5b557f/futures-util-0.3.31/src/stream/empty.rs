use super::assert_stream;
use core::marker::PhantomData;
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};

/// Stream for the [`empty`] function.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Empty<T> {
    _phantom: PhantomData<T>,
}

/// Creates a stream which contains no elements.
///
/// The returned stream will always return `Ready(None)` when polled.
pub fn empty<T>() -> Empty<T> {
    assert_stream::<T, _>(Empty { _phantom: PhantomData })
}

impl<T> Unpin for Empty<T> {}

impl<T> FusedStream for Empty<T> {
    fn is_terminated(&self) -> bool {
        true
    }
}

impl<T> Stream for Empty<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(None)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(0))
    }
}

impl<T> Clone for Empty<T> {
    fn clone(&self) -> Self {
        empty()
    }
}
