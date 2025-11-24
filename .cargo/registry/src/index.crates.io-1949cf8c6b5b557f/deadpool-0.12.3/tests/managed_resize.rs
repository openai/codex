#![cfg(feature = "managed")]

use std::convert::Infallible;

use deadpool::managed::{self, Metrics, Object, RecycleResult};

type Pool = managed::Pool<Manager, Object<Manager>>;

struct Manager {}

impl managed::Manager for Manager {
    type Type = ();
    type Error = Infallible;

    async fn create(&self) -> Result<(), Infallible> {
        Ok(())
    }

    async fn recycle(&self, _conn: &mut (), _: &Metrics) -> RecycleResult<Infallible> {
        Ok(())
    }
}

// Regression test for https://github.com/bikeshedder/deadpool/issues/380
#[tokio::test]
async fn test_grow_reuse_existing() {
    // Shrink doesn't discard objects currently borrowed from the pool but
    // keeps track of them so that repeatedly growing and shrinking will
    // not cause excessive object creation. This logic used to contain a bug
    // causing an overflow.
    let mgr = Manager {};
    let pool = Pool::builder(mgr).max_size(2).build().unwrap();
    let obj1 = pool.get().await.unwrap();
    let obj2 = pool.get().await.unwrap();
    assert!(pool.status().size == 2);
    assert!(pool.status().max_size == 2);
    pool.resize(0);
    // At this point the two objects are still tracked
    assert!(pool.status().size == 2);
    assert!(pool.status().max_size == 0);
    pool.resize(1);
    // Only one of the objects should be returned to the pool
    assert!(pool.status().size == 2);
    assert!(pool.status().max_size == 1);
    drop(obj1);
    // The first drop brings the size to 1.
    assert!(pool.status().size == 1);
    assert!(pool.status().max_size == 1);
    drop(obj2);
    assert!(pool.status().size == 1);
    assert!(pool.status().max_size == 1);
}

#[tokio::test]
async fn resize_pool_shrink() {
    let mgr = Manager {};
    let pool = Pool::builder(mgr).max_size(2).build().unwrap();
    let obj0 = pool.get().await.unwrap();
    let obj1 = pool.get().await.unwrap();
    pool.resize(1);
    assert_eq!(pool.status().max_size, 1);
    assert_eq!(pool.status().size, 2);
    drop(obj1);
    assert_eq!(pool.status().max_size, 1);
    assert_eq!(pool.status().size, 1);
    drop(obj0);
    assert_eq!(pool.status().max_size, 1);
    assert_eq!(pool.status().size, 1);
}

#[tokio::test]
async fn resize_pool_grow() {
    let mgr = Manager {};
    let pool = Pool::builder(mgr).max_size(1).build().unwrap();
    let obj0 = pool.get().await.unwrap();
    pool.resize(2);
    assert_eq!(pool.status().max_size, 2);
    assert_eq!(pool.status().size, 1);
    let obj1 = pool.get().await.unwrap();
    assert_eq!(pool.status().max_size, 2);
    assert_eq!(pool.status().size, 2);
    drop(obj1);
    assert_eq!(pool.status().max_size, 2);
    assert_eq!(pool.status().size, 2);
    drop(obj0);
    assert_eq!(pool.status().max_size, 2);
    assert_eq!(pool.status().size, 2);
}

#[tokio::test]
async fn resize_pool_shrink_grow() {
    let mgr = Manager {};
    let pool = Pool::builder(mgr).max_size(1).build().unwrap();
    let obj0 = pool.get().await.unwrap();
    pool.resize(2);
    pool.resize(0);
    pool.resize(5);
    assert_eq!(pool.status().max_size, 5);
    assert_eq!(pool.status().size, 1);
    drop(obj0);
    assert_eq!(pool.status().max_size, 5);
    assert_eq!(pool.status().size, 1);
}

#[tokio::test]
async fn resize_pool_grow_concurrent() {
    let mgr = Manager {};
    let pool = Pool::builder(mgr).max_size(0).build().unwrap();
    let join_handle = {
        let pool = pool.clone();
        tokio::spawn(async move { pool.get().await })
    };
    tokio::task::yield_now().await;
    assert_eq!(pool.status().max_size, 0);
    assert_eq!(pool.status().size, 0);
    assert_eq!(pool.status().available, 0);
    assert_eq!(pool.status().waiting, 1);
    pool.resize(1);
    assert_eq!(pool.status().max_size, 1);
    assert_eq!(pool.status().size, 0);
    assert_eq!(pool.status().available, 0);
    assert_eq!(pool.status().waiting, 1);
    tokio::task::yield_now().await;
    assert_eq!(pool.status().max_size, 1);
    assert_eq!(pool.status().size, 1);
    assert_eq!(pool.status().available, 0);
    assert_eq!(pool.status().waiting, 0);
    let obj0 = join_handle.await.unwrap().unwrap();
    assert_eq!(pool.status().max_size, 1);
    assert_eq!(pool.status().size, 1);
    assert_eq!(pool.status().available, 0);
    assert_eq!(pool.status().waiting, 0);
    drop(obj0);
    assert_eq!(pool.status().max_size, 1);
    assert_eq!(pool.status().size, 1);
    assert_eq!(pool.status().available, 1);
    assert_eq!(pool.status().waiting, 0);
}

#[tokio::test]
async fn close_resize() {
    let mgr = Manager {};
    let pool = Pool::builder(mgr).max_size(1).build().unwrap();
    pool.close();
    pool.resize(16);
    assert_eq!(pool.status().size, 0);
    assert_eq!(pool.status().max_size, 0);
}
