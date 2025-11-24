use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use pin_project_lite::pin_project;
use std::any::Any;
use std::boxed::Box;
use std::panic::{catch_unwind, AssertUnwindSafe, UnwindSafe};
use std::pin::Pin;

pin_project! {
    /// Stream for the [`catch_unwind`](super::StreamExt::catch_unwind) method.
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled"]
    pub struct CatchUnwind<St> {
        #[pin]
        stream: St,
        caught_unwind: bool,
    }
}

impl<St: Stream + UnwindSafe> CatchUnwind<St> {
    pub(super) fn new(stream: St) -> Self {
        Self { stream, caught_unwind: false }
    }

    delegate_access_inner!(stream, St, ());
}

impl<St: Stream + UnwindSafe> Stream for CatchUnwind<St> {
    type Item = Result<St::Item, Box<dyn Any + Send>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if *this.caught_unwind {
            Poll::Ready(None)
        } else {
            let res = catch_unwind(AssertUnwindSafe(|| this.stream.as_mut().poll_next(cx)));

            match res {
                Ok(poll) => poll.map(|opt| opt.map(Ok)),
                Err(e) => {
                    *this.caught_unwind = true;
                    Poll::Ready(Some(Err(e)))
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.caught_unwind {
            (0, Some(0))
        } else {
            self.stream.size_hint()
        }
    }
}

impl<St: FusedStream + UnwindSafe> FusedStream for CatchUnwind<St> {
    fn is_terminated(&self) -> bool {
        self.caught_unwind || self.stream.is_terminated()
    }
}
