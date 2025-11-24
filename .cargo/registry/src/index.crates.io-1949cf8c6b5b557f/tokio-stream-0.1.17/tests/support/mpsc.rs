use async_stream::stream;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_stream::Stream;

pub fn unbounded_channel_stream<T: Unpin>() -> (UnboundedSender<T>, impl Stream<Item = T>) {
    let (tx, mut rx) = mpsc::unbounded_channel();

    let stream = stream! {
        while let Some(item) = rx.recv().await {
            yield item;
        }
    };

    (tx, stream)
}
