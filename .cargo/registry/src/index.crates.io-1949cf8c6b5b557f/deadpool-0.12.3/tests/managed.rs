#![cfg(feature = "managed")]

use std::{convert::Infallible, time::Duration};

use tokio::time;

use deadpool::managed::{self, Metrics, Object, PoolError, RecycleResult, Timeouts};

type Pool = managed::Pool<Manager>;

struct Manager {}

impl managed::Manager for Manager {
    type Type = usize;
    type Error = Infallible;

    async fn create(&self) -> Result<usize, Infallible> {
        Ok(0)
    }

    async fn recycle(&self, _conn: &mut usize, _: &Metrics) -> RecycleResult<Infallible> {
        Ok(())
    }
}

#[tokio::test]
async fn basic() {
    let mgr = Manager {};
    let pool = Pool::builder(mgr).max_size(16).build().unwrap();

    let status = pool.status();
    assert_eq!(status.size, 0);
    assert_eq!(status.available, 0);
    assert_eq!(status.waiting, 0);

    let obj0 = pool.get().await.unwrap();
    let status = pool.status();
    assert_eq!(status.size, 1);
    assert_eq!(status.available, 0);
    assert_eq!(status.waiting, 0);

    let obj1 = pool.get().await.unwrap();
    let status = pool.status();
    assert_eq!(status.size, 2);
    assert_eq!(status.available, 0);
    assert_eq!(status.waiting, 0);

    let obj2 = pool.get().await.unwrap();
    let status = pool.status();
    assert_eq!(status.size, 3);
    assert_eq!(status.available, 0);
    assert_eq!(status.waiting, 0);

    drop(obj0);
    let status = pool.status();
    assert_eq!(status.size, 3);
    assert_eq!(status.available, 1);
    assert_eq!(status.waiting, 0);

    drop(obj1);
    let status = pool.status();
    assert_eq!(status.size, 3);
    assert_eq!(status.available, 2);
    assert_eq!(status.waiting, 0);

    drop(obj2);
    let status = pool.status();
    assert_eq!(status.size, 3);
    assert_eq!(status.available, 3);
    assert_eq!(status.waiting, 0);
}

#[tokio::test]
async fn closing() {
    let mgr = Manager {};
    let pool = Pool::builder(mgr).max_size(1).build().unwrap();

    // fetch the only object from the pool
    let obj = pool.get().await;
    let join_handle = {
        let pool = pool.clone();
        tokio::spawn(async move { pool.get().await })
    };

    tokio::task::yield_now().await;
    assert_eq!(pool.status().available, 0);
    assert_eq!(pool.status().waiting, 1);

    pool.close();
    tokio::task::yield_now().await;
    assert_eq!(pool.status().available, 0);
    assert_eq!(pool.status().waiting, 0);

    assert!(matches!(join_handle.await.unwrap(), Err(PoolError::Closed)));
    assert!(matches!(pool.get().await, Err(PoolError::Closed)));
    assert!(matches!(
        pool.timeout_get(&Timeouts {
            wait: Some(Duration::ZERO),
            ..pool.timeouts()
        })
        .await,
        Err(PoolError::Closed)
    ));

    drop(obj);
    tokio::task::yield_now().await;
    assert_eq!(pool.status().available, 0);
    assert_eq!(pool.status().waiting, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent() {
    let mgr = Manager {};
    let pool = Pool::builder(mgr).max_size(3).build().unwrap();

    // Spawn tasks
    let futures = (0..100)
        .map(|_| {
            let pool = pool.clone();
            tokio::spawn(async move {
                let mut obj = pool.get().await.unwrap();
                *obj += 1;
                time::sleep(Duration::from_millis(1)).await;
            })
        })
        .collect::<Vec<_>>();

    // Await tasks to finish
    for future in futures {
        future.await.unwrap();
    }

    // Verify
    let status = pool.status();
    assert_eq!(status.size, 3);
    assert_eq!(status.available, 3);
    assert_eq!(status.waiting, 0);

    let values = [
        pool.get().await.unwrap(),
        pool.get().await.unwrap(),
        pool.get().await.unwrap(),
    ];

    assert_eq!(values.iter().map(|obj| **obj).sum::<usize>(), 100);
}

#[tokio::test(flavor = "multi_thread")]
async fn object_take() {
    let mgr = Manager {};
    let pool = Pool::builder(mgr).max_size(2).build().unwrap();
    let obj0 = pool.get().await.unwrap();
    let obj1 = pool.get().await.unwrap();

    let status = pool.status();
    assert_eq!(status.size, 2);
    assert_eq!(status.available, 0);
    assert_eq!(status.waiting, 0);

    let _ = Object::take(obj0);
    let status = pool.status();
    assert_eq!(status.size, 1);
    assert_eq!(status.available, 0);
    assert_eq!(status.waiting, 0);

    let _ = Object::take(obj1);
    let status = pool.status();
    assert_eq!(status.size, 0);
    assert_eq!(status.available, 0);

    let obj0 = pool.get().await.unwrap();
    let obj1 = pool.get().await.unwrap();
    let status = pool.status();
    assert_eq!(status.size, 2);
    assert_eq!(status.available, 0);
    assert_eq!(status.waiting, 0);

    drop(obj0);
    drop(obj1);
    let status = pool.status();
    assert_eq!(status.size, 2);
    assert_eq!(status.available, 2);
    assert_eq!(status.waiting, 0);
}
#[tokio::test]
async fn retain() {
    let mgr = Manager {};
    let pool = Pool::builder(mgr).max_size(4).build().unwrap();
    {
        let _a = pool.get().await;
        let _b = pool.get().await;
        tokio::time::sleep(Duration::from_millis(5)).await;
        let _c = pool.get().await;
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(pool.status().size, 3);
    let retain_result = pool.retain(|_, metrics| metrics.age() <= Duration::from_millis(10));
    assert_eq!(retain_result.retained, 1);
    assert_eq!(retain_result.removed.len(), 2);
    assert_eq!(pool.status().size, 1);
    tokio::time::sleep(Duration::from_millis(5)).await;
    let retain_result = pool.retain(|_, metrics| metrics.age() <= Duration::from_millis(10));
    assert_eq!(retain_result.retained, 0);
    assert_eq!(retain_result.removed.len(), 1);
    assert_eq!(pool.status().size, 0);
}

#[tokio::test]
async fn retain_fnmut() {
    let mgr = Manager {};
    let pool = Pool::builder(mgr).max_size(4).build().unwrap();
    {
        let _a = pool.get().await;
        let _b = pool.get().await;
        let _c = pool.get().await;
        let _c = pool.get().await;
    }
    let mut removed = 0;
    {
        let retain_result = pool.retain(|_, _| {
            removed += 1;
            false
        });
        assert_eq!(retain_result.retained, 0);
        assert_eq!(retain_result.removed.len(), 4);
    }
    assert_eq!(pool.status().size, 0);
}
