use tokio_stream::{self as stream, Stream, StreamExt};

#[tokio::test]
async fn basic_usage() {
    let mut one = stream::once(1);

    assert_eq!(one.size_hint(), (1, Some(1)));
    assert_eq!(Some(1), one.next().await);

    assert_eq!(one.size_hint(), (0, Some(0)));
    assert_eq!(None, one.next().await);
}
