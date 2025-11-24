use crate::sink::{SinkExt, SinkMapErr};
use futures_core::stream::{FusedStream, Stream};
use futures_sink::Sink;
use pin_project_lite::pin_project;

pin_project! {
    /// Sink for the [`sink_err_into`](super::SinkExt::sink_err_into) method.
    #[derive(Debug)]
    #[must_use = "sinks do nothing unless polled"]
    pub struct SinkErrInto<Si: Sink<Item>, Item, E> {
        #[pin]
        sink: SinkMapErr<Si, fn(Si::Error) -> E>,
    }
}

impl<Si, E, Item> SinkErrInto<Si, Item, E>
where
    Si: Sink<Item>,
    Si::Error: Into<E>,
{
    pub(super) fn new(sink: Si) -> Self {
        Self { sink: SinkExt::sink_map_err(sink, Into::into) }
    }

    delegate_access_inner!(sink, Si, (.));
}

impl<Si, Item, E> Sink<Item> for SinkErrInto<Si, Item, E>
where
    Si: Sink<Item>,
    Si::Error: Into<E>,
{
    type Error = E;

    delegate_sink!(sink, Item);
}

// Forwarding impl of Stream from the underlying sink
impl<S, Item, E> Stream for SinkErrInto<S, Item, E>
where
    S: Sink<Item> + Stream,
    S::Error: Into<E>,
{
    type Item = S::Item;

    delegate_stream!(sink);
}

impl<S, Item, E> FusedStream for SinkErrInto<S, Item, E>
where
    S: Sink<Item> + FusedStream,
    S::Error: Into<E>,
{
    fn is_terminated(&self) -> bool {
        self.sink.is_terminated()
    }
}
