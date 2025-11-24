use tokio_stream::{self as stream, Stream, StreamExt};
use tokio_test::{assert_pending, assert_ready, task};

mod support {
    pub(crate) mod mpsc;
}

use support::mpsc;
use tokio_stream::adapters::Chain;

#[tokio::test]
async fn basic_usage() {
    let one = stream::iter(vec![1, 2, 3]);
    let two = stream::iter(vec![4, 5, 6]);

    let mut stream = visibility_test(one, two);

    assert_eq!(stream.size_hint(), (6, Some(6)));
    assert_eq!(stream.next().await, Some(1));

    assert_eq!(stream.size_hint(), (5, Some(5)));
    assert_eq!(stream.next().await, Some(2));

    assert_eq!(stream.size_hint(), (4, Some(4)));
    assert_eq!(stream.next().await, Some(3));

    assert_eq!(stream.size_hint(), (3, Some(3)));
    assert_eq!(stream.next().await, Some(4));

    assert_eq!(stream.size_hint(), (2, Some(2)));
    assert_eq!(stream.next().await, Some(5));

    assert_eq!(stream.size_hint(), (1, Some(1)));
    assert_eq!(stream.next().await, Some(6));

    assert_eq!(stream.size_hint(), (0, Some(0)));
    assert_eq!(stream.next().await, None);

    assert_eq!(stream.size_hint(), (0, Some(0)));
    assert_eq!(stream.next().await, None);
}

fn visibility_test<I, S1, S2>(s1: S1, s2: S2) -> Chain<S1, S2>
where
    S1: Stream<Item = I>,
    S2: Stream<Item = I>,
{
    s1.chain(s2)
}

#[tokio::test]
#[cfg_attr(miri, ignore)] // Block on https://github.com/tokio-rs/tokio/issues/6860
async fn pending_first() {
    let (tx1, rx1) = mpsc::unbounded_channel_stream();
    let (tx2, rx2) = mpsc::unbounded_channel_stream();

    let mut stream = task::spawn(rx1.chain(rx2));
    assert_eq!(stream.size_hint(), (0, None));

    assert_pending!(stream.poll_next());

    tx2.send(2).unwrap();
    assert!(!stream.is_woken());

    assert_pending!(stream.poll_next());

    tx1.send(1).unwrap();
    assert!(stream.is_woken());
    assert_eq!(Some(1), assert_ready!(stream.poll_next()));

    assert_pending!(stream.poll_next());

    drop(tx1);

    assert_eq!(stream.size_hint(), (0, None));

    assert!(stream.is_woken());
    assert_eq!(Some(2), assert_ready!(stream.poll_next()));

    assert_eq!(stream.size_hint(), (0, None));

    drop(tx2);

    assert_eq!(stream.size_hint(), (0, None));
    assert_eq!(None, assert_ready!(stream.poll_next()));
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
    let m = m1.chain(m2);
    assert_eq!(m.size_hint(), (usize::MAX, None));
}
