#![cfg(feature = "unmanaged")]

use std::time::Duration;

use tokio::{task, time};

use deadpool::unmanaged::{Pool, PoolError};

#[tokio::test]
async fn basic() {
    let pool = Pool::from(vec![(), (), ()]);

    let status = pool.status();
    assert_eq!(status.size, 3);
    assert_eq!(status.available, 3);

    let _val0 = pool.get().await;

    let status = pool.status();
    assert_eq!(status.size, 3);
    assert_eq!(status.available, 2);

    let _val1 = pool.get().await;

    let status = pool.status();
    assert_eq!(status.size, 3);
    assert_eq!(status.available, 1);

    let _val2 = pool.get().await;

    let status = pool.status();
    assert_eq!(status.size, 3);
    assert_eq!(status.available, 0);
}

#[tokio::test]
async fn closing() {
    let pool = Pool::<i64>::new(1);
    pool.try_add(42).unwrap();

    let obj = pool.get().await.unwrap();
    let join_handle = {
        let pool = pool.clone();
        tokio::spawn(async move { pool.get().await })
    };
    assert!(!pool.is_closed());
    assert_eq!(pool.status().available, 0);

    task::yield_now().await;
    pool.close();
    assert!(pool.is_closed());

    task::yield_now().await;
    assert_eq!(pool.status().available, 0);

    assert!(matches!(join_handle.await.unwrap(), Err(PoolError::Closed)));
    assert!(matches!(pool.get().await, Err(PoolError::Closed)));
    assert!(matches!(pool.try_get(), Err(PoolError::Closed)));

    drop(obj);
    assert!(pool.is_closed());
    assert!(matches!(pool.try_get(), Err(PoolError::Closed)));
    assert_eq!(pool.status().available, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent() {
    let pool = Pool::from(vec![0usize, 0, 0]);

    // Spawn tasks
    let futures = (0..100)
        .map(|_| {
            let pool = pool.clone();
            tokio::spawn(async move {
                *pool.get().await.unwrap() += 1;
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

    let values = [pool.get().await, pool.get().await, pool.get().await];

    assert_eq!(
        values
            .iter()
            .map(|obj| **obj.as_ref().unwrap())
            .sum::<usize>(),
        100,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_unmanaged_add_remove() {
    let pool = Pool::new(2);

    pool.add(1).await.unwrap();
    assert_eq!(pool.status().size, 1);

    pool.add(2).await.unwrap();
    assert_eq!(pool.status().size, 2);
    assert!(
        time::timeout(Duration::from_millis(10), pool.add(3))
            .await
            .is_err(),
        "adding a third item should timeout"
    );

    pool.remove().await.unwrap();
    assert_eq!(pool.status().size, 1);
    assert!(
        time::timeout(Duration::from_millis(10), pool.add(3))
            .await
            .is_ok(),
        "adding a third item should not timeout"
    );

    pool.remove().await.unwrap();
    assert_eq!(pool.status().size, 1);

    pool.remove().await.unwrap();
    assert_eq!(pool.status().size, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn try_add_try_remove() {
    let pool = Pool::new(2);

    pool.try_add(1).unwrap();
    assert_eq!(pool.status().size, 1);

    pool.try_add(2).unwrap();
    assert_eq!(pool.status().size, 2);

    assert!(pool.try_add(3).is_err());

    pool.try_remove().unwrap();
    assert_eq!(pool.status().size, 1);

    assert!(pool.try_add(3).is_ok());

    pool.try_remove().unwrap();
    assert_eq!(pool.status().size, 1);

    pool.try_remove().unwrap();
    assert_eq!(pool.status().size, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn add_timeout() {
    let pool = Pool::from(vec![1]);
    let add = {
        let pool = pool.clone();
        tokio::spawn(async move {
            pool.add(2).await.unwrap();
        })
    };

    let mut iv = time::interval(Duration::from_millis(10));
    iv.tick().await;
    iv.tick().await;

    pool.try_remove().unwrap();
    assert!(
        time::timeout(Duration::from_millis(10), add).await.is_ok(),
        "add should not timeout"
    );
    assert_eq!(pool.status().size, 1);

    assert_eq!(pool.try_remove().unwrap(), 2);
}
