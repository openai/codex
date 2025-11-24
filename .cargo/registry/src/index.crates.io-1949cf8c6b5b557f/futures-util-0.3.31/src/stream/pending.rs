use super::assert_stream;
use core::marker;
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};

/// Stream for the [`pending()`] function.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Pending<T> {
    _data: marker::PhantomData<T>,
}

/// Creates a stream which never returns any elements.
///
/// The returned stream will always return `Pending` when polled.
pub fn pending<T>() -> Pending<T> {
    assert_stream::<T, _>(Pending { _data: marker::PhantomData })
}

impl<T> Unpin for Pending<T> {}

impl<T> FusedStream for Pending<T> {
    fn is_terminated(&self) -> bool {
        true
    }
}

impl<T> Stream for Pending<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Pending
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(0))
    }
}

impl<T> Clone for Pending<T> {
    fn clone(&self) -> Self {
        pending()
    }
}
