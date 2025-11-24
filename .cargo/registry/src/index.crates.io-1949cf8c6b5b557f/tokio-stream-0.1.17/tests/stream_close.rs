use tokio_stream::{StreamExt, StreamNotifyClose};

#[tokio::test]
async fn basic_usage() {
    let mut stream = StreamNotifyClose::new(tokio_stream::iter(vec![0, 1]));

    assert_eq!(stream.next().await, Some(Some(0)));
    assert_eq!(stream.next().await, Some(Some(1)));
    assert_eq!(stream.next().await, Some(None));
    assert_eq!(stream.next().await, None);
}
