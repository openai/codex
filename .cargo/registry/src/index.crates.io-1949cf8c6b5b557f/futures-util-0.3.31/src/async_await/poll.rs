use crate::future::FutureExt;
use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{Context, Poll};

/// A macro which returns the result of polling a future once within the
/// current `async` context.
///
/// This macro is only usable inside of `async` functions, closures, and blocks.
/// It is also gated behind the `async-await` feature of this library, which is
/// activated by default.
///
/// If you need the result of polling a [`Stream`](crate::stream::Stream),
/// you can use this macro with the [`next`](crate::stream::StreamExt::next) method:
/// `poll!(stream.next())`.
#[macro_export]
macro_rules! poll {
    ($x:expr $(,)?) => {
        $crate::__private::async_await::poll($x).await
    };
}

#[doc(hidden)]
pub fn poll<F: Future + Unpin>(future: F) -> PollOnce<F> {
    PollOnce { future }
}

#[allow(missing_debug_implementations)]
#[doc(hidden)]
pub struct PollOnce<F: Future + Unpin> {
    future: F,
}

impl<F: Future + Unpin> Future for PollOnce<F> {
    type Output = Poll<F::Output>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(self.future.poll_unpin(cx))
    }
}
