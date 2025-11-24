use tokio_stream::{self as stream, Stream, StreamExt};

#[tokio::test]
async fn basic_usage() {
    let mut stream = stream::empty::<i32>();

    for _ in 0..2 {
        assert_eq!(stream.size_hint(), (0, Some(0)));
        assert_eq!(None, stream.next().await);
    }
}
