use futures::executor::block_on_stream;
use futures::future::{err, ok, TryFutureExt};
use futures::sink::Sink;
use futures::stream::Stream;
use futures::stream::{self, StreamExt};
use futures::task::{Context, Poll};
use std::marker::PhantomData;
use std::pin::Pin;

#[test]
fn successful_future() {
    let stream_items = vec![17, 19];
    let future_of_a_stream = ok::<_, bool>(stream::iter(stream_items).map(Ok));

    let stream = future_of_a_stream.try_flatten_stream();

    let mut iter = block_on_stream(stream);
    assert_eq!(Ok(17), iter.next().unwrap());
    assert_eq!(Ok(19), iter.next().unwrap());
    assert_eq!(None, iter.next());
}

#[test]
fn failed_future() {
    struct PanickingStream<T, E> {
        _marker: PhantomData<(T, E)>,
    }

    impl<T, E> Stream for PanickingStream<T, E> {
        type Item = Result<T, E>;

        fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            panic!()
        }
    }

    let future_of_a_stream = err::<PanickingStream<bool, u32>, _>(10);
    let stream = future_of_a_stream.try_flatten_stream();
    let mut iter = block_on_stream(stream);
    assert_eq!(Err(10), iter.next().unwrap());
    assert_eq!(None, iter.next());
}

#[test]
fn assert_impls() {
    struct StreamSink<T, E, Item>(PhantomData<(T, E, Item)>);

    impl<T, E, Item> Stream for StreamSink<T, E, Item> {
        type Item = Result<T, E>;
        fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            panic!()
        }
    }

    impl<T, E, Item> Sink<Item> for StreamSink<T, E, Item> {
        type Error = E;
        fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            panic!()
        }
        fn start_send(self: Pin<&mut Self>, _: Item) -> Result<(), Self::Error> {
            panic!()
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            panic!()
        }
        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            panic!()
        }
    }

    fn assert_stream<S: Stream>(_: &S) {}
    fn assert_sink<S: Sink<Item>, Item>(_: &S) {}
    fn assert_stream_sink<S: Stream + Sink<Item>, Item>(_: &S) {}

    let s = ok(StreamSink::<(), (), ()>(PhantomData)).try_flatten_stream();
    assert_stream(&s);
    assert_sink(&s);
    assert_stream_sink(&s);
    let s = ok(StreamSink::<(), (), ()>(PhantomData)).flatten_sink();
    assert_stream(&s);
    assert_sink(&s);
    assert_stream_sink(&s);
}
