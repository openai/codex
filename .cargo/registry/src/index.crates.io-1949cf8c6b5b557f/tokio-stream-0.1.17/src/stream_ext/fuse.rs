use crate::Stream;

use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

pin_project! {
    /// Stream returned by [`fuse()`][super::StreamExt::fuse].
    #[derive(Debug)]
    pub struct Fuse<T> {
        #[pin]
        stream: Option<T>,
    }
}

impl<T> Fuse<T>
where
    T: Stream,
{
    pub(crate) fn new(stream: T) -> Fuse<T> {
        Fuse {
            stream: Some(stream),
        }
    }
}

impl<T> Stream for Fuse<T>
where
    T: Stream,
{
    type Item = T::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T::Item>> {
        let res = match Option::as_pin_mut(self.as_mut().project().stream) {
            Some(stream) => ready!(stream.poll_next(cx)),
            None => return Poll::Ready(None),
        };

        if res.is_none() {
            // Do not poll the stream anymore
            self.as_mut().project().stream.set(None);
        }

        Poll::Ready(res)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.stream {
            Some(ref stream) => stream.size_hint(),
            None => (0, Some(0)),
        }
    }
}
