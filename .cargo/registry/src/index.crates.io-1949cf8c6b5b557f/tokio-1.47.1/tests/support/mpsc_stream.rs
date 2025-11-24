#![allow(dead_code)]

use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc::{self, Receiver, Sender, UnboundedReceiver, UnboundedSender};
use tokio_stream::Stream;

struct UnboundedStream<T> {
    recv: UnboundedReceiver<T>,
}
impl<T> Stream for UnboundedStream<T> {
    type Item = T;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        Pin::into_inner(self).recv.poll_recv(cx)
    }
}

pub fn unbounded_channel_stream<T: Unpin>() -> (UnboundedSender<T>, impl Stream<Item = T>) {
    let (tx, rx) = mpsc::unbounded_channel();

    let stream = UnboundedStream { recv: rx };

    (tx, stream)
}

struct BoundedStream<T> {
    recv: Receiver<T>,
}
impl<T> Stream for BoundedStream<T> {
    type Item = T;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        Pin::into_inner(self).recv.poll_recv(cx)
    }
}

pub fn channel_stream<T: Unpin>(size: usize) -> (Sender<T>, impl Stream<Item = T>) {
    let (tx, rx) = mpsc::channel(size);

    let stream = BoundedStream { recv: rx };

    (tx, stream)
}
