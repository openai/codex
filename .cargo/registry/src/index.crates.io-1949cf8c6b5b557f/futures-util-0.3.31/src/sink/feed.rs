use core::pin::Pin;
use futures_core::future::Future;
use futures_core::ready;
use futures_core::task::{Context, Poll};
use futures_sink::Sink;

/// Future for the [`feed`](super::SinkExt::feed) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Feed<'a, Si: ?Sized, Item> {
    sink: &'a mut Si,
    item: Option<Item>,
}

// Pinning is never projected to children
impl<Si: Unpin + ?Sized, Item> Unpin for Feed<'_, Si, Item> {}

impl<'a, Si: Sink<Item> + Unpin + ?Sized, Item> Feed<'a, Si, Item> {
    pub(super) fn new(sink: &'a mut Si, item: Item) -> Self {
        Feed { sink, item: Some(item) }
    }

    pub(super) fn sink_pin_mut(&mut self) -> Pin<&mut Si> {
        Pin::new(self.sink)
    }

    pub(super) fn is_item_pending(&self) -> bool {
        self.item.is_some()
    }
}

impl<Si: Sink<Item> + Unpin + ?Sized, Item> Future for Feed<'_, Si, Item> {
    type Output = Result<(), Si::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut sink = Pin::new(&mut this.sink);
        ready!(sink.as_mut().poll_ready(cx))?;
        let item = this.item.take().expect("polled Feed after completion");
        sink.as_mut().start_send(item)?;
        Poll::Ready(Ok(()))
    }
}
