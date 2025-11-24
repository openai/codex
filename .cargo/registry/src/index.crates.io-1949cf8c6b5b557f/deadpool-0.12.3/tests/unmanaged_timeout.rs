#![cfg(feature = "unmanaged")]

use std::time::Duration;

use deadpool::{
    unmanaged::{self, PoolConfig, PoolError},
    Runtime,
};

type Pool = unmanaged::Pool<()>;

#[tokio::test]
async fn no_runtime() {
    let pool = Pool::default();
    assert!(matches!(
        pool.timeout_get(Some(Duration::from_millis(1))).await,
        Err(PoolError::NoRuntimeSpecified)
    ));
    assert!(matches!(
        pool.timeout_get(Some(Duration::from_millis(0))).await,
        Err(PoolError::Timeout)
    ));
}

async fn _test_get(runtime: Runtime) {
    let cfg = PoolConfig {
        max_size: 16,
        timeout: None,
        runtime: Some(runtime),
    };
    let pool = Pool::from_config(&cfg);
    assert!(matches!(
        pool.timeout_get(Some(Duration::from_millis(1))).await,
        Err(PoolError::Timeout),
    ));
}

async fn _test_config(runtime: Runtime) {
    let cfg = PoolConfig {
        max_size: 16,
        timeout: Some(Duration::from_millis(1)),
        runtime: Some(runtime),
    };
    let pool = Pool::from_config(&cfg);
    assert!(matches!(pool.get().await, Err(PoolError::Timeout)));
}

#[cfg(feature = "rt_tokio_1")]
#[tokio::test]
async fn rt_tokio_1() {
    _test_get(Runtime::Tokio1).await;
    _test_config(Runtime::Tokio1).await;
}

#[cfg(feature = "rt_async-std_1")]
#[async_std::test]
async fn rt_async_std_1() {
    _test_get(Runtime::AsyncStd1).await;
    _test_config(Runtime::AsyncStd1).await;
}
