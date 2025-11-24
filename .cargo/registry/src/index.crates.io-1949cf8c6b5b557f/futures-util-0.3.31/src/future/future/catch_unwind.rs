use core::any::Any;
use core::pin::Pin;
use std::boxed::Box;
use std::panic::{catch_unwind, AssertUnwindSafe, UnwindSafe};

use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`catch_unwind`](super::FutureExt::catch_unwind) method.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct CatchUnwind<Fut> {
        #[pin]
        future: Fut,
    }
}

impl<Fut> CatchUnwind<Fut>
where
    Fut: Future + UnwindSafe,
{
    pub(super) fn new(future: Fut) -> Self {
        Self { future }
    }
}

impl<Fut> Future for CatchUnwind<Fut>
where
    Fut: Future + UnwindSafe,
{
    type Output = Result<Fut::Output, Box<dyn Any + Send>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let f = self.project().future;
        catch_unwind(AssertUnwindSafe(|| f.poll(cx)))?.map(Ok)
    }
}
