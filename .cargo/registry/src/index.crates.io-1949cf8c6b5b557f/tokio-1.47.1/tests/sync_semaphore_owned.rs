#![cfg(feature = "sync")]

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::sync::Arc;
use tokio::sync::Semaphore;

#[test]
fn try_acquire() {
    let sem = Arc::new(Semaphore::new(1));
    {
        let p1 = sem.clone().try_acquire_owned();
        assert!(p1.is_ok());
        let p2 = sem.clone().try_acquire_owned();
        assert!(p2.is_err());
    }
    let p3 = sem.try_acquire_owned();
    assert!(p3.is_ok());
}

#[test]
fn try_acquire_many() {
    let sem = Arc::new(Semaphore::new(42));
    {
        let p1 = sem.clone().try_acquire_many_owned(42);
        assert!(p1.is_ok());
        let p2 = sem.clone().try_acquire_owned();
        assert!(p2.is_err());
    }
    let p3 = sem.clone().try_acquire_many_owned(32);
    assert!(p3.is_ok());
    let p4 = sem.clone().try_acquire_many_owned(10);
    assert!(p4.is_ok());
    assert!(sem.try_acquire_owned().is_err());
}

#[tokio::test]
#[cfg(feature = "full")]
async fn acquire() {
    let sem = Arc::new(Semaphore::new(1));
    let p1 = sem.clone().try_acquire_owned().unwrap();
    let sem_clone = sem.clone();
    let j = tokio::spawn(async move {
        let _p2 = sem_clone.acquire_owned().await;
    });
    drop(p1);
    j.await.unwrap();
}

#[tokio::test]
#[cfg(feature = "full")]
async fn acquire_many() {
    let semaphore = Arc::new(Semaphore::new(42));
    let permit32 = semaphore.clone().try_acquire_many_owned(32).unwrap();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let join_handle = tokio::spawn(async move {
        let _permit10 = semaphore.clone().acquire_many_owned(10).await.unwrap();
        sender.send(()).unwrap();
        let _permit32 = semaphore.acquire_many_owned(32).await.unwrap();
    });
    receiver.await.unwrap();
    drop(permit32);
    join_handle.await.unwrap();
}

#[tokio::test]
#[cfg(feature = "full")]
async fn add_permits() {
    let sem = Arc::new(Semaphore::new(0));
    let sem_clone = sem.clone();
    let j = tokio::spawn(async move {
        let _p2 = sem_clone.acquire_owned().await;
    });
    sem.add_permits(1);
    j.await.unwrap();
}

#[test]
fn forget() {
    let sem = Arc::new(Semaphore::new(1));
    {
        let p = sem.clone().try_acquire_owned().unwrap();
        assert_eq!(sem.available_permits(), 0);
        p.forget();
        assert_eq!(sem.available_permits(), 0);
    }
    assert_eq!(sem.available_permits(), 0);
    assert!(sem.try_acquire_owned().is_err());
}

#[test]
fn merge() {
    let sem = Arc::new(Semaphore::new(3));
    {
        let mut p1 = sem.clone().try_acquire_owned().unwrap();
        assert_eq!(sem.available_permits(), 2);
        let p2 = sem.clone().try_acquire_many_owned(2).unwrap();
        assert_eq!(sem.available_permits(), 0);
        p1.merge(p2);
        assert_eq!(sem.available_permits(), 0);
    }
    assert_eq!(sem.available_permits(), 3);
}

#[test]
#[cfg(not(target_family = "wasm"))] // No stack unwinding on wasm targets
#[should_panic]
fn merge_unrelated_permits() {
    let sem1 = Arc::new(Semaphore::new(3));
    let sem2 = Arc::new(Semaphore::new(3));
    let mut p1 = sem1.try_acquire_owned().unwrap();
    let p2 = sem2.try_acquire_owned().unwrap();
    p1.merge(p2)
}

#[test]
fn split() {
    let sem = Arc::new(Semaphore::new(5));
    let mut p1 = sem.clone().try_acquire_many_owned(3).unwrap();
    assert_eq!(sem.available_permits(), 2);
    assert_eq!(p1.num_permits(), 3);
    let mut p2 = p1.split(1).unwrap();
    assert_eq!(sem.available_permits(), 2);
    assert_eq!(p1.num_permits(), 2);
    assert_eq!(p2.num_permits(), 1);
    let p3 = p1.split(0).unwrap();
    assert_eq!(p3.num_permits(), 0);
    drop(p1);
    assert_eq!(sem.available_permits(), 4);
    let p4 = p2.split(1).unwrap();
    assert_eq!(p2.num_permits(), 0);
    assert_eq!(p4.num_permits(), 1);
    assert!(p2.split(1).is_none());
    drop(p2);
    assert_eq!(sem.available_permits(), 4);
    drop(p3);
    assert_eq!(sem.available_permits(), 4);
    drop(p4);
    assert_eq!(sem.available_permits(), 5);
}

#[tokio::test]
#[cfg(feature = "full")]
async fn stress_test() {
    let sem = Arc::new(Semaphore::new(5));
    let mut join_handles = Vec::new();
    for _ in 0..1000 {
        let sem_clone = sem.clone();
        join_handles.push(tokio::spawn(async move {
            let _p = sem_clone.acquire_owned().await;
        }));
    }
    for j in join_handles {
        j.await.unwrap();
    }
    // there should be exactly 5 semaphores available now
    let _p1 = sem.clone().try_acquire_owned().unwrap();
    let _p2 = sem.clone().try_acquire_owned().unwrap();
    let _p3 = sem.clone().try_acquire_owned().unwrap();
    let _p4 = sem.clone().try_acquire_owned().unwrap();
    let _p5 = sem.clone().try_acquire_owned().unwrap();
    assert!(sem.try_acquire_owned().is_err());
}
