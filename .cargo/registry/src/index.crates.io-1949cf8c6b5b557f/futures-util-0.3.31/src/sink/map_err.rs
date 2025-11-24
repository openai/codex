use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Context, Poll};
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Sink for the [`sink_map_err`](super::SinkExt::sink_map_err) method.
    #[derive(Debug, Clone)]
    #[must_use = "sinks do nothing unless polled"]
    pub struct SinkMapErr<Si, F> {
        #[pin]
        sink: Si,
        f: Option<F>,
    }
}

impl<Si, F> SinkMapErr<Si, F> {
    pub(super) fn new(sink: Si, f: F) -> Self {
        Self { sink, f: Some(f) }
    }

    delegate_access_inner!(sink, Si, ());

    fn take_f(self: Pin<&mut Self>) -> F {
        self.project().f.take().expect("polled MapErr after completion")
    }
}

impl<Si, F, E, Item> Sink<Item> for SinkMapErr<Si, F>
where
    Si: Sink<Item>,
    F: FnOnce(Si::Error) -> E,
{
    type Error = E;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.as_mut().project().sink.poll_ready(cx).map_err(|e| self.as_mut().take_f()(e))
    }

    fn start_send(mut self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        self.as_mut().project().sink.start_send(item).map_err(|e| self.as_mut().take_f()(e))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.as_mut().project().sink.poll_flush(cx).map_err(|e| self.as_mut().take_f()(e))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.as_mut().project().sink.poll_close(cx).map_err(|e| self.as_mut().take_f()(e))
    }
}

// Forwarding impl of Stream from the underlying sink
impl<S: Stream, F> Stream for SinkMapErr<S, F> {
    type Item = S::Item;

    delegate_stream!(sink);
}

impl<S: FusedStream, F> FusedStream for SinkMapErr<S, F> {
    fn is_terminated(&self) -> bool {
        self.sink.is_terminated()
    }
}
