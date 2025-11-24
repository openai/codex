use std::{sync::mpsc, thread::sleep, time::Duration};

use async_broadcast::*;
use futures_util::{future::join, stream::StreamExt};

use easy_parallel::Parallel;
use futures_lite::future::block_on;

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
fn basic_sync() {
    let (s, mut r1) = broadcast(10);
    let mut r2 = r1.clone();

    s.try_broadcast(7).unwrap();
    assert_eq!(r1.try_recv().unwrap(), 7);
    assert_eq!(r2.try_recv().unwrap(), 7);

    let mut r3 = r1.clone();
    s.try_broadcast(8).unwrap();
    assert_eq!(r1.try_recv().unwrap(), 8);
    assert_eq!(r2.try_recv().unwrap(), 8);
    assert_eq!(r3.try_recv().unwrap(), 8);
}

#[test]
fn basic_async() {
    block_on(async {
        let (s, mut r1) = broadcast(10);
        let mut r2 = r1.clone();

        s.broadcast(7).await.unwrap();
        assert_eq!(r1.recv().await.unwrap(), 7);
        assert_eq!(r2.recv().await.unwrap(), 7);

        // Now let's try the Stream impl.
        let mut r3 = r1.clone();
        s.broadcast(8).await.unwrap();
        assert_eq!(r1.next().await.unwrap(), 8);
        assert_eq!(r2.next().await.unwrap(), 8);
        assert_eq!(r3.next().await.unwrap(), 8);
    });
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn basic_blocking() {
    let (s, mut r) = broadcast(1);

    s.broadcast_blocking(7).unwrap();
    assert_eq!(r.try_recv(), Ok(7));

    s.broadcast_blocking(8).unwrap();
    assert_eq!(block_on(r.recv()), Ok(8));

    block_on(s.broadcast(9)).unwrap();
    assert_eq!(r.recv_blocking(), Ok(9));

    assert_eq!(r.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn parallel() {
    let (s1, mut r1) = broadcast(2);
    let s2 = s1.clone();
    let mut r2 = r1.clone();

    let (sender_sync_send, sender_sync_recv) = mpsc::channel();
    let (receiver_sync_send, receiver_sync_recv) = mpsc::channel();

    Parallel::new()
        .add(move || {
            sender_sync_recv.recv().unwrap();

            s1.try_broadcast(7).unwrap();
            s2.try_broadcast(8).unwrap();
            assert!(s2.try_broadcast(9).unwrap_err().is_full());
            assert!(s1.try_broadcast(10).unwrap_err().is_full());
            receiver_sync_send.send(()).unwrap();

            drop(s1);
            drop(s2);
            receiver_sync_send.send(()).unwrap();
        })
        .add(move || {
            assert_eq!(r1.try_recv(), Err(TryRecvError::Empty));
            assert_eq!(r2.try_recv(), Err(TryRecvError::Empty));
            sender_sync_send.send(()).unwrap();

            receiver_sync_recv.recv().unwrap();
            assert_eq!(r1.try_recv().unwrap(), 7);
            assert_eq!(r1.try_recv().unwrap(), 8);
            assert_eq!(r2.try_recv().unwrap(), 7);
            assert_eq!(r2.try_recv().unwrap(), 8);

            receiver_sync_recv.recv().unwrap();
            assert_eq!(r1.try_recv(), Err(TryRecvError::Closed));
            assert_eq!(r2.try_recv(), Err(TryRecvError::Closed));
        })
        .run();
}

#[test]
fn parallel_async() {
    let (s1, mut r1) = broadcast(2);
    let s2 = s1.clone();
    let mut r2 = r1.clone();

    let (sender_sync_send, sender_sync_recv) = mpsc::channel();
    let (receiver_sync_send, receiver_sync_recv) = mpsc::channel();

    Parallel::new()
        .add(move || {
            block_on(async move {
                sender_sync_recv.recv().unwrap();
                sleep(ms(5));

                s1.broadcast(7).await.unwrap();
                s2.broadcast(8).await.unwrap();
                assert!(s2.try_broadcast(9).unwrap_err().is_full());
                assert!(s1.try_broadcast(10).unwrap_err().is_full());
                receiver_sync_send.send(()).unwrap();

                s1.broadcast(9).await.unwrap();
                s2.broadcast(10).await.unwrap();

                drop(s1);
                drop(s2);
                receiver_sync_send.send(()).unwrap();
            })
        })
        .add(move || {
            block_on(async move {
                assert_eq!(r1.try_recv(), Err(TryRecvError::Empty));
                assert_eq!(r2.try_recv(), Err(TryRecvError::Empty));
                sender_sync_send.send(()).unwrap();

                receiver_sync_recv.recv().unwrap();
                assert_eq!(r1.next().await.unwrap(), 7);
                assert_eq!(r2.next().await.unwrap(), 7);
                assert_eq!(r1.recv().await.unwrap(), 8);
                assert_eq!(r2.recv().await.unwrap(), 8);

                receiver_sync_recv.recv().unwrap();
                sleep(ms(5));
                assert_eq!(r1.next().await.unwrap(), 9);
                assert_eq!(r2.next().await.unwrap(), 9);

                assert_eq!(r1.recv().await.unwrap(), 10);
                assert_eq!(r2.recv().await.unwrap(), 10);

                assert_eq!(r1.recv().await, Err(RecvError::Closed));
                assert_eq!(r2.recv().await, Err(RecvError::Closed));
            })
        })
        .run();
}

#[test]
fn channel_shrink() {
    let (s1, mut r1) = broadcast(4);
    let mut r2 = r1.clone();
    let mut r3 = r1.clone();
    let mut r4 = r1.clone();

    s1.try_broadcast(1).unwrap();
    s1.try_broadcast(2).unwrap();
    s1.try_broadcast(3).unwrap();
    s1.try_broadcast(4).unwrap();

    assert_eq!(r2.try_recv().unwrap(), 1);
    assert_eq!(r2.try_recv().unwrap(), 2);

    assert_eq!(r3.try_recv().unwrap(), 1);
    assert_eq!(r3.try_recv().unwrap(), 2);
    assert_eq!(r3.try_recv().unwrap(), 3);

    assert_eq!(r4.try_recv().unwrap(), 1);
    assert_eq!(r4.try_recv().unwrap(), 2);
    assert_eq!(r4.try_recv().unwrap(), 3);
    assert_eq!(r4.try_recv().unwrap(), 4);

    r1.set_capacity(2);

    assert_eq!(r1.try_recv(), Err(TryRecvError::Overflowed(2)));
    assert_eq!(r1.try_recv().unwrap(), 3);
    assert_eq!(r1.try_recv().unwrap(), 4);
    assert_eq!(r1.try_recv(), Err(TryRecvError::Empty));

    assert_eq!(r2.try_recv().unwrap(), 3);
    assert_eq!(r2.try_recv().unwrap(), 4);
    assert_eq!(r2.try_recv(), Err(TryRecvError::Empty));

    assert_eq!(r3.try_recv().unwrap(), 4);
    assert_eq!(r3.try_recv(), Err(TryRecvError::Empty));

    assert_eq!(r4.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn overflow() {
    let (s1, mut r1) = broadcast(2);
    r1.set_overflow(true);
    // We'll keep r1 as the lagging receiver.
    let mut r2 = r1.clone();
    let mut r3 = r1.clone();

    let (sender_sync_send, sender_sync_recv) = mpsc::channel();

    Parallel::new()
        .add(move || {
            block_on(async move {
                s1.broadcast(7).await.unwrap();
                s1.broadcast(8).await.unwrap();
                sender_sync_recv.recv().unwrap();
                sleep(ms(5));

                s1.broadcast(9).await.unwrap();
                sender_sync_recv.recv().unwrap();
            })
        })
        .add(move || {
            block_on(async move {
                assert_eq!(r2.next().await.unwrap(), 7);
                assert_eq!(r2.recv().await.unwrap(), 8);

                sender_sync_send.send(()).unwrap();
                assert_eq!(r2.next().await.unwrap(), 9);
                sender_sync_send.send(()).unwrap();
            })
        })
        .add(move || {
            block_on(async move {
                assert_eq!(r3.next().await.unwrap(), 7);
                assert_eq!(r3.recv().await.unwrap(), 8);
                assert_eq!(r3.next().await.unwrap(), 9);
            })
        })
        .run();

    assert_eq!(r1.try_recv(), Err(TryRecvError::Overflowed(1)));
    assert_eq!(r1.try_recv().unwrap(), 8);
    assert_eq!(r1.try_recv().unwrap(), 9);
}

#[test]
fn open_channel() {
    let (s1, r) = broadcast(2);
    let inactive = r.deactivate();
    let s2 = s1.clone();

    let (receiver_sync_send, receiver_sync_recv) = mpsc::channel();
    let (sender_sync_send, sender_sync_recv) = mpsc::channel();

    Parallel::new()
        .add(move || {
            block_on(async move {
                receiver_sync_send.send(()).unwrap();

                let (result1, result2) = join(s1.broadcast(7), s2.broadcast(8)).await;
                result1.unwrap();
                result2.unwrap();

                sender_sync_recv.recv().unwrap();
                assert_eq!(s1.try_broadcast(9), Err(TrySendError::Inactive(9)));
                assert_eq!(s2.try_broadcast(10), Err(TrySendError::Inactive(10)));
                receiver_sync_send.send(()).unwrap();
                sleep(ms(5));

                s1.broadcast(9).await.unwrap();
                s2.broadcast(10).await.unwrap();
            })
        })
        .add(move || {
            block_on(async move {
                receiver_sync_recv.recv().unwrap();
                sleep(ms(5));

                let mut r = inactive.activate_cloned();
                assert_eq!(r.next().await.unwrap(), 7);
                assert_eq!(r.recv().await.unwrap(), 8);
                drop(r);

                sender_sync_send.send(()).unwrap();
                receiver_sync_recv.recv().unwrap();

                let mut r = inactive.activate();
                assert_eq!(r.recv().await.unwrap(), 9);
                assert_eq!(r.recv().await.unwrap(), 10);
            })
        })
        .run();
}

#[test]
fn inactive_drop() {
    let (s, active_receiver) = broadcast::<()>(1);
    let inactive = active_receiver.deactivate();
    let inactive2 = inactive.clone();
    drop(inactive);
    drop(inactive2);

    assert!(s.is_closed())
}

#[test]
fn poll_recv() {
    let (s, mut r) = broadcast::<i32>(2);
    r.set_overflow(true);

    // A quick custom stream impl to demonstrate/test `poll_recv`.
    struct MyStream(Receiver<i32>);
    impl futures_core::Stream for MyStream {
        type Item = Result<i32, RecvError>;
        fn poll_next(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Self::Item>> {
            std::pin::Pin::new(&mut self.0).poll_recv(cx)
        }
    }

    block_on(async move {
        let mut stream = MyStream(r);

        s.broadcast(1).await.unwrap();
        s.broadcast(2).await.unwrap();
        s.broadcast(3).await.unwrap();
        s.broadcast(4).await.unwrap();

        assert_eq!(stream.next().await.unwrap(), Err(RecvError::Overflowed(2)));
        assert_eq!(stream.next().await.unwrap(), Ok(3));
        assert_eq!(stream.next().await.unwrap(), Ok(4));

        drop(s);

        assert_eq!(stream.next().await, None);
    })
}
