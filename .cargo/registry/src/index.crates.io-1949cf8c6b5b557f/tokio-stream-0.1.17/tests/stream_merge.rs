use tokio_stream::{self as stream, Stream, StreamExt};
use tokio_test::task;
use tokio_test::{assert_pending, assert_ready};

mod support {
    pub(crate) mod mpsc;
}

use support::mpsc;

#[tokio::test]
async fn merge_sync_streams() {
    let mut s = stream::iter(vec![0, 2, 4, 6]).merge(stream::iter(vec![1, 3, 5]));

    for i in 0..7 {
        let rem = 7 - i;
        assert_eq!(s.size_hint(), (rem, Some(rem)));
        assert_eq!(Some(i), s.next().await);
    }

    assert!(s.next().await.is_none());
}

#[tokio::test]
async fn merge_async_streams() {
    let (tx1, rx1) = mpsc::unbounded_channel_stream();
    let (tx2, rx2) = mpsc::unbounded_channel_stream();

    let mut rx = task::spawn(rx1.merge(rx2));

    assert_eq!(rx.size_hint(), (0, None));

    assert_pending!(rx.poll_next());

    tx1.send(1).unwrap();

    assert!(rx.is_woken());
    assert_eq!(Some(1), assert_ready!(rx.poll_next()));

    assert_pending!(rx.poll_next());
    tx2.send(2).unwrap();

    assert!(rx.is_woken());
    assert_eq!(Some(2), assert_ready!(rx.poll_next()));
    assert_pending!(rx.poll_next());

    drop(tx1);
    assert!(rx.is_woken());
    assert_pending!(rx.poll_next());

    tx2.send(3).unwrap();
    assert!(rx.is_woken());
    assert_eq!(Some(3), assert_ready!(rx.poll_next()));
    assert_pending!(rx.poll_next());

    drop(tx2);
    assert!(rx.is_woken());
    assert_eq!(None, assert_ready!(rx.poll_next()));
}

#[test]
fn size_overflow() {
    struct Monster;

    impl tokio_stream::Stream for Monster {
        type Item = ();
        fn poll_next(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<()>> {
            panic!()
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            (usize::MAX, Some(usize::MAX))
        }
    }

    let m1 = Monster;
    let m2 = Monster;
    let m = m1.merge(m2);
    assert_eq!(m.size_hint(), (usize::MAX, None));
}
