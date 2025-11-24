use futures::executor::block_on;
use futures::sink::{Sink, SinkExt};
use futures::stream::{self, Stream, StreamExt};
use futures::task::{Context, Poll};
use pin_project::pin_project;
use std::pin::Pin;

#[test]
fn test_split() {
    #[pin_project]
    struct Join<T, U> {
        #[pin]
        stream: T,
        #[pin]
        sink: U,
    }

    impl<T: Stream, U> Stream for Join<T, U> {
        type Item = T::Item;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T::Item>> {
            self.project().stream.poll_next(cx)
        }
    }

    impl<T, U: Sink<Item>, Item> Sink<Item> for Join<T, U> {
        type Error = U::Error;

        fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.project().sink.poll_ready(cx)
        }

        fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
            self.project().sink.start_send(item)
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.project().sink.poll_flush(cx)
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.project().sink.poll_close(cx)
        }
    }

    let mut dest: Vec<i32> = Vec::new();
    {
        let join = Join { stream: stream::iter(vec![10, 20, 30]), sink: &mut dest };

        let (sink, stream) = join.split();
        let join = sink.reunite(stream).expect("test_split: reunite error");
        let (mut sink, stream) = join.split();
        let mut stream = stream.map(Ok);
        block_on(sink.send_all(&mut stream)).unwrap();
    }
    assert_eq!(dest, vec![10, 20, 30]);
}
