#![cfg(all(tokio_unstable, feature = "tracing"))]

use std::rc::Rc;
use tokio::{
    task::{Builder, LocalSet},
    test,
};

#[test]
async fn spawn_with_name() {
    let result = Builder::new()
        .name("name")
        .spawn(async { "task executed" })
        .unwrap()
        .await;

    assert_eq!(result.unwrap(), "task executed");
}

#[test]
async fn spawn_blocking_with_name() {
    let result = Builder::new()
        .name("name")
        .spawn_blocking(|| "task executed")
        .unwrap()
        .await;

    assert_eq!(result.unwrap(), "task executed");
}

#[test]
async fn spawn_local_with_name() {
    let unsend_data = Rc::new("task executed");
    let result = LocalSet::new()
        .run_until(async move {
            Builder::new()
                .name("name")
                .spawn_local(async move { unsend_data })
                .unwrap()
                .await
        })
        .await;

    assert_eq!(*result.unwrap(), "task executed");
}

#[test]
async fn spawn_without_name() {
    let result = Builder::new()
        .spawn(async { "task executed" })
        .unwrap()
        .await;

    assert_eq!(result.unwrap(), "task executed");
}

#[test]
async fn spawn_blocking_without_name() {
    let result = Builder::new()
        .spawn_blocking(|| "task executed")
        .unwrap()
        .await;

    assert_eq!(result.unwrap(), "task executed");
}

#[test]
async fn spawn_local_without_name() {
    let unsend_data = Rc::new("task executed");
    let result = LocalSet::new()
        .run_until(async move {
            Builder::new()
                .spawn_local(async move { unsend_data })
                .unwrap()
                .await
        })
        .await;

    assert_eq!(*result.unwrap(), "task executed");
}
