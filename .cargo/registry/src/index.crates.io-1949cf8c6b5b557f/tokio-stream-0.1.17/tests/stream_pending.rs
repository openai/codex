use tokio_stream::{self as stream, Stream, StreamExt};
use tokio_test::{assert_pending, task};

#[tokio::test]
async fn basic_usage() {
    let mut stream = stream::pending::<i32>();

    for _ in 0..2 {
        assert_eq!(stream.size_hint(), (0, None));

        let mut next = task::spawn(async { stream.next().await });
        assert_pending!(next.poll());
    }
}
