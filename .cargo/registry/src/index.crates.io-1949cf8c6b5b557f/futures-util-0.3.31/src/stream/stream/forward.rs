use crate::stream::Fuse;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::ready;
use futures_core::stream::Stream;
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Future for the [`forward`](super::StreamExt::forward) method.
    #[project = ForwardProj]
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct Forward<St, Si, Item> {
        #[pin]
        sink: Option<Si>,
        #[pin]
        stream: Fuse<St>,
        buffered_item: Option<Item>,
    }
}

impl<St, Si, Item> Forward<St, Si, Item> {
    pub(crate) fn new(stream: St, sink: Si) -> Self {
        Self { sink: Some(sink), stream: Fuse::new(stream), buffered_item: None }
    }
}

impl<St, Si, Item, E> FusedFuture for Forward<St, Si, Item>
where
    Si: Sink<Item, Error = E>,
    St: Stream<Item = Result<Item, E>>,
{
    fn is_terminated(&self) -> bool {
        self.sink.is_none()
    }
}

impl<St, Si, Item, E> Future for Forward<St, Si, Item>
where
    Si: Sink<Item, Error = E>,
    St: Stream<Item = Result<Item, E>>,
{
    type Output = Result<(), E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let ForwardProj { mut sink, mut stream, buffered_item } = self.project();
        let mut si = sink.as_mut().as_pin_mut().expect("polled `Forward` after completion");

        loop {
            // If we've got an item buffered already, we need to write it to the
            // sink before we can do anything else
            if buffered_item.is_some() {
                ready!(si.as_mut().poll_ready(cx))?;
                si.as_mut().start_send(buffered_item.take().unwrap())?;
            }

            match stream.as_mut().poll_next(cx)? {
                Poll::Ready(Some(item)) => {
                    *buffered_item = Some(item);
                }
                Poll::Ready(None) => {
                    ready!(si.poll_close(cx))?;
                    sink.set(None);
                    return Poll::Ready(Ok(()));
                }
                Poll::Pending => {
                    ready!(si.poll_flush(cx))?;
                    return Poll::Pending;
                }
            }
        }
    }
}
