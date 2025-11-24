use futures_core::Stream;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

// This is equivalent to the `futures::StreamExt::next` method.
// But we want to make this crate dependency as small as possible, so we define our `next` function.
#[doc(hidden)]
pub fn next<S>(stream: &mut S) -> impl Future<Output = Option<S::Item>> + '_
where
    S: Stream + Unpin,
{
    Next { stream }
}

#[derive(Debug)]
struct Next<'a, S> {
    stream: &'a mut S,
}

impl<S> Unpin for Next<'_, S> where S: Unpin {}

impl<S> Future for Next<'_, S>
where
    S: Stream + Unpin,
{
    type Output = Option<S::Item>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.stream).poll_next(cx)
    }
}
