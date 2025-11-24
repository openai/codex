#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::runtime::Runtime;
use tokio::sync::oneshot;
use tokio::task::{self, Id, LocalSet};

mod support {
    pub mod panic;
}
use support::panic::test_panic;

#[tokio::test(flavor = "current_thread")]
async fn task_id_spawn() {
    tokio::spawn(async { println!("task id: {}", task::id()) })
        .await
        .unwrap();
}

#[cfg(not(target_os = "wasi"))]
#[tokio::test(flavor = "current_thread")]
async fn task_id_spawn_blocking() {
    task::spawn_blocking(|| println!("task id: {}", task::id()))
        .await
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn task_id_collision_current_thread() {
    let handle1 = tokio::spawn(async { task::id() });
    let handle2 = tokio::spawn(async { task::id() });

    let (id1, id2) = tokio::join!(handle1, handle2);
    assert_ne!(id1.unwrap(), id2.unwrap());
}

#[cfg(not(target_os = "wasi"))]
#[tokio::test(flavor = "multi_thread")]
async fn task_id_collision_multi_thread() {
    let handle1 = tokio::spawn(async { task::id() });
    let handle2 = tokio::spawn(async { task::id() });

    let (id1, id2) = tokio::join!(handle1, handle2);
    assert_ne!(id1.unwrap(), id2.unwrap());
}

#[tokio::test(flavor = "current_thread")]
async fn task_ids_match_current_thread() {
    let (tx, rx) = oneshot::channel();
    let handle = tokio::spawn(async {
        let id = rx.await.unwrap();
        assert_eq!(id, task::id());
    });
    tx.send(handle.id()).unwrap();
    handle.await.unwrap();
}

#[cfg(not(target_os = "wasi"))]
#[tokio::test(flavor = "multi_thread")]
async fn task_ids_match_multi_thread() {
    let (tx, rx) = oneshot::channel();
    let handle = tokio::spawn(async {
        let id = rx.await.unwrap();
        assert_eq!(id, task::id());
    });
    tx.send(handle.id()).unwrap();
    handle.await.unwrap();
}

#[cfg(not(target_os = "wasi"))]
#[tokio::test(flavor = "multi_thread")]
async fn task_id_future_destructor_completion() {
    struct MyFuture {
        tx: Option<oneshot::Sender<Id>>,
    }

    impl Future for MyFuture {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
            Poll::Ready(())
        }
    }

    impl Drop for MyFuture {
        fn drop(&mut self) {
            let _ = self.tx.take().unwrap().send(task::id());
        }
    }

    let (tx, rx) = oneshot::channel();
    let handle = tokio::spawn(MyFuture { tx: Some(tx) });
    let id = handle.id();
    handle.await.unwrap();
    assert_eq!(rx.await.unwrap(), id);
}

#[cfg(not(target_os = "wasi"))]
#[tokio::test(flavor = "multi_thread")]
async fn task_id_future_destructor_abort() {
    struct MyFuture {
        tx: Option<oneshot::Sender<Id>>,
    }

    impl Future for MyFuture {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
            Poll::Pending
        }
    }
    impl Drop for MyFuture {
        fn drop(&mut self) {
            let _ = self.tx.take().unwrap().send(task::id());
        }
    }

    let (tx, rx) = oneshot::channel();
    let handle = tokio::spawn(MyFuture { tx: Some(tx) });
    let id = handle.id();
    handle.abort();
    assert!(handle.await.unwrap_err().is_cancelled());
    assert_eq!(rx.await.unwrap(), id);
}

#[tokio::test(flavor = "current_thread")]
async fn task_id_output_destructor_handle_dropped_before_completion() {
    struct MyOutput {
        tx: Option<oneshot::Sender<Id>>,
    }

    impl Drop for MyOutput {
        fn drop(&mut self) {
            let _ = self.tx.take().unwrap().send(task::id());
        }
    }

    struct MyFuture {
        tx: Option<oneshot::Sender<Id>>,
    }

    impl Future for MyFuture {
        type Output = MyOutput;

        fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Ready(MyOutput { tx: self.tx.take() })
        }
    }

    let (tx, mut rx) = oneshot::channel();
    let handle = tokio::spawn(MyFuture { tx: Some(tx) });
    let id = handle.id();
    drop(handle);
    assert!(rx.try_recv().is_err());
    assert_eq!(rx.await.unwrap(), id);
}

#[tokio::test(flavor = "current_thread")]
async fn task_id_output_destructor_handle_dropped_after_completion() {
    struct MyOutput {
        tx: Option<oneshot::Sender<Id>>,
    }

    impl Drop for MyOutput {
        fn drop(&mut self) {
            let _ = self.tx.take().unwrap().send(task::id());
        }
    }

    struct MyFuture {
        tx_output: Option<oneshot::Sender<Id>>,
        tx_future: Option<oneshot::Sender<()>>,
    }

    impl Future for MyFuture {
        type Output = MyOutput;

        fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            let _ = self.tx_future.take().unwrap().send(());
            Poll::Ready(MyOutput {
                tx: self.tx_output.take(),
            })
        }
    }

    let (tx_output, mut rx_output) = oneshot::channel();
    let (tx_future, rx_future) = oneshot::channel();
    let handle = tokio::spawn(MyFuture {
        tx_output: Some(tx_output),
        tx_future: Some(tx_future),
    });
    let id = handle.id();
    rx_future.await.unwrap();
    assert!(rx_output.try_recv().is_err());
    drop(handle);
    assert_eq!(rx_output.await.unwrap(), id);
}

#[test]
fn task_try_id_outside_task() {
    assert_eq!(None, task::try_id());
}

#[cfg(not(target_os = "wasi"))]
#[test]
fn task_try_id_inside_block_on() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        assert_eq!(None, task::try_id());
    });
}

#[tokio::test(flavor = "current_thread")]
async fn task_id_spawn_local() {
    LocalSet::new()
        .run_until(async {
            task::spawn_local(async { println!("task id: {}", task::id()) })
                .await
                .unwrap();
        })
        .await
}

#[tokio::test(flavor = "current_thread")]
async fn task_id_nested_spawn_local() {
    LocalSet::new()
        .run_until(async {
            task::spawn_local(async {
                let parent_id = task::id();
                LocalSet::new()
                    .run_until(async {
                        task::spawn_local(async move {
                            assert_ne!(parent_id, task::id());
                        })
                        .await
                        .unwrap();
                    })
                    .await;
                assert_eq!(parent_id, task::id());
            })
            .await
            .unwrap();
        })
        .await;
}

#[cfg(not(target_os = "wasi"))]
#[tokio::test(flavor = "multi_thread")]
async fn task_id_block_in_place_block_on_spawn() {
    use tokio::runtime::Builder;

    task::spawn(async {
        let parent_id = task::id();

        task::block_in_place(move || {
            let rt = Builder::new_current_thread().build().unwrap();
            rt.block_on(rt.spawn(async move {
                assert_ne!(parent_id, task::id());
            }))
            .unwrap();
        });

        assert_eq!(parent_id, task::id());
    })
    .await
    .unwrap();
}

#[test]
#[cfg_attr(not(panic = "unwind"), ignore)]
fn task_id_outside_task_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let _ = task::id();
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
#[cfg_attr(not(panic = "unwind"), ignore)]
fn task_id_inside_block_on_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            task::id();
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}
