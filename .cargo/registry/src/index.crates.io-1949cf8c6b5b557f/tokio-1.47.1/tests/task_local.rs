#![cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi doesn't support threads

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::oneshot;

#[tokio::test(flavor = "multi_thread")]
async fn local() {
    tokio::task_local! {
        static REQ_ID: u32;
        pub static FOO: bool;
    }

    let j1 = tokio::spawn(REQ_ID.scope(1, async move {
        assert_eq!(REQ_ID.get(), 1);
        assert_eq!(REQ_ID.get(), 1);
    }));

    let j2 = tokio::spawn(REQ_ID.scope(2, async move {
        REQ_ID.with(|v| {
            assert_eq!(REQ_ID.get(), 2);
            assert_eq!(*v, 2);
        });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        assert_eq!(REQ_ID.get(), 2);
    }));

    let j3 = tokio::spawn(FOO.scope(true, async move {
        assert!(FOO.get());
    }));

    j1.await.unwrap();
    j2.await.unwrap();
    j3.await.unwrap();
}

#[tokio::test]
async fn task_local_available_on_abort() {
    tokio::task_local! {
        static KEY: u32;
    }

    struct MyFuture {
        tx_poll: Option<oneshot::Sender<()>>,
        tx_drop: Option<oneshot::Sender<u32>>,
    }
    impl Future for MyFuture {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
            if let Some(tx_poll) = self.tx_poll.take() {
                let _ = tx_poll.send(());
            }
            Poll::Pending
        }
    }
    impl Drop for MyFuture {
        fn drop(&mut self) {
            let _ = self.tx_drop.take().unwrap().send(KEY.get());
        }
    }

    let (tx_drop, rx_drop) = oneshot::channel();
    let (tx_poll, rx_poll) = oneshot::channel();

    let h = tokio::spawn(KEY.scope(
        42,
        MyFuture {
            tx_poll: Some(tx_poll),
            tx_drop: Some(tx_drop),
        },
    ));

    rx_poll.await.unwrap();
    h.abort();
    assert_eq!(rx_drop.await.unwrap(), 42);

    let err = h.await.unwrap_err();
    if !err.is_cancelled() {
        if let Ok(panic) = err.try_into_panic() {
            std::panic::resume_unwind(panic);
        } else {
            panic!();
        }
    }
}

#[tokio::test]
async fn task_local_available_on_completion_drop() {
    tokio::task_local! {
        static KEY: u32;
    }

    struct MyFuture {
        tx: Option<oneshot::Sender<u32>>,
    }
    impl Future for MyFuture {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
            Poll::Ready(())
        }
    }
    impl Drop for MyFuture {
        fn drop(&mut self) {
            let _ = self.tx.take().unwrap().send(KEY.get());
        }
    }

    let (tx, rx) = oneshot::channel();

    let h = tokio::spawn(KEY.scope(42, MyFuture { tx: Some(tx) }));

    assert_eq!(rx.await.unwrap(), 42);
    h.await.unwrap();
}

#[tokio::test]
async fn take_value() {
    tokio::task_local! {
        static KEY: u32
    }
    let fut = KEY.scope(1, async {});
    let mut pinned = Box::pin(fut);
    assert_eq!(pinned.as_mut().take_value(), Some(1));
    assert_eq!(pinned.as_mut().take_value(), None);
}

#[tokio::test]
async fn poll_after_take_value_should_fail() {
    tokio::task_local! {
        static KEY: u32
    }
    let fut = KEY.scope(1, async {
        let result = KEY.try_with(|_| {});
        // The task local value no longer exists.
        assert!(result.is_err());
    });
    let mut fut = Box::pin(fut);
    fut.as_mut().take_value();

    // Poll the future after `take_value` has been called
    fut.await;
}
