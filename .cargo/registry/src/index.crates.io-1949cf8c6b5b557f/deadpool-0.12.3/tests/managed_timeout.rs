#![cfg(all(
    feature = "managed",
    any(feature = "rt_tokio_1", feature = "rt_async-std_1")
))]

use std::{convert::Infallible, future::Future, pin::Pin, task, time::Duration};

use deadpool::{
    managed::{self, Metrics, Object, PoolConfig, PoolError, RecycleResult, Timeouts},
    Runtime,
};

type Pool = managed::Pool<Manager, Object<Manager>>;

struct Manager {}

struct Never;

impl Future for Never {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        task::Poll::Pending
    }
}

impl managed::Manager for Manager {
    type Type = usize;
    type Error = Infallible;

    async fn create(&self) -> Result<usize, Infallible> {
        Never.await;
        unreachable!();
    }

    async fn recycle(&self, _conn: &mut usize, _: &Metrics) -> RecycleResult<Infallible> {
        Never.await;
        unreachable!();
    }
}

async fn test_managed_timeout(runtime: Runtime) {
    let mgr = Manager {};
    let cfg = PoolConfig {
        max_size: 16,
        timeouts: Timeouts {
            create: Some(Duration::from_millis(0)),
            wait: Some(Duration::from_millis(0)),
            recycle: Some(Duration::from_millis(0)),
        },
        ..Default::default()
    };
    let pool = Pool::builder(mgr)
        .config(cfg)
        .runtime(runtime)
        .build()
        .unwrap();

    assert!(matches!(pool.get().await, Err(PoolError::Timeout(_))));
}

#[cfg(feature = "rt_tokio_1")]
#[tokio::test]
async fn rt_tokio_1() {
    test_managed_timeout(Runtime::Tokio1).await;
}

#[cfg(feature = "rt_async-std_1")]
#[async_std::test]
async fn rt_async_std_1() {
    test_managed_timeout(Runtime::AsyncStd1).await;
}
