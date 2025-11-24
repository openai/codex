#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", tokio_unstable))]

use tokio::runtime::LocalOptions;
use tokio::task::spawn_local;

#[test]
fn test_spawn_local_in_runtime() {
    let rt = rt();

    let res = rt.block_on(async move {
        let (tx, rx) = tokio::sync::oneshot::channel();

        spawn_local(async {
            tokio::task::yield_now().await;
            tx.send(5).unwrap();
        });

        rx.await.unwrap()
    });

    assert_eq!(res, 5);
}

#[test]
fn test_spawn_from_handle() {
    let rt = rt();

    let (tx, rx) = tokio::sync::oneshot::channel();

    rt.handle().spawn(async {
        tokio::task::yield_now().await;
        tx.send(5).unwrap();
    });

    let res = rt.block_on(async move { rx.await.unwrap() });

    assert_eq!(res, 5);
}

#[test]
fn test_spawn_local_on_runtime_object() {
    let rt = rt();

    let (tx, rx) = tokio::sync::oneshot::channel();

    rt.spawn_local(async {
        tokio::task::yield_now().await;
        tx.send(5).unwrap();
    });

    let res = rt.block_on(async move { rx.await.unwrap() });

    assert_eq!(res, 5);
}

#[test]
fn test_spawn_local_from_guard() {
    let rt = rt();

    let (tx, rx) = tokio::sync::oneshot::channel();

    let _guard = rt.enter();

    spawn_local(async {
        tokio::task::yield_now().await;
        tx.send(5).unwrap();
    });

    let res = rt.block_on(async move { rx.await.unwrap() });

    assert_eq!(res, 5);
}

#[test]
#[cfg_attr(target_family = "wasm", ignore)] // threads not supported
fn test_spawn_from_guard_other_thread() {
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let rt = rt();
        let handle = rt.handle().clone();

        tx.send(handle).unwrap();
    });

    let handle = rx.recv().unwrap();

    let _guard = handle.enter();

    tokio::spawn(async {});
}

#[test]
#[should_panic = "Local tasks can only be spawned on a LocalRuntime from the thread the runtime was created on"]
#[cfg_attr(target_family = "wasm", ignore)] // threads not supported
fn test_spawn_local_from_guard_other_thread() {
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let rt = rt();
        let handle = rt.handle().clone();

        tx.send(handle).unwrap();
    });

    let handle = rx.recv().unwrap();

    let _guard = handle.enter();

    spawn_local(async {});
}

fn rt() -> tokio::runtime::LocalRuntime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build_local(LocalOptions::default())
        .unwrap()
}
