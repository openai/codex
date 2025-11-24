use std::time::Duration;

use deadpool::managed::{Hook, HookError, Manager, Metrics, Pool, RecycleResult};
use itertools::Itertools;
use tokio::time::{sleep, timeout};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Gate {
    Ok,
    Err,
    Slow,
    Never,
}

impl Gate {
    async fn open(&self) -> Result<(), ()> {
        match self {
            Self::Ok => Ok(()),
            Self::Err => Err(()),
            Self::Never => {
                sleep(Duration::MAX).await;
                unreachable!();
            }
            Self::Slow => {
                sleep(Duration::from_nanos(2)).await;
                Ok(())
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct Gates {
    create: Gate,
    recycle: Gate,
    post_create: Gate,
    pre_recycle: Gate,
    post_recycle: Gate,
}

fn configs() -> impl Iterator<Item = Gates> {
    (0..5)
        .map(|_| &[Gate::Ok, Gate::Err, Gate::Slow, Gate::Never])
        .multi_cartesian_product()
        .map(move |gates| Gates {
            create: *gates[0],
            recycle: *gates[1],
            post_create: *gates[2],
            pre_recycle: *gates[3],
            post_recycle: *gates[4],
        })
}

fn pools(max_size: usize) -> impl Iterator<Item = Pool<GatedManager>> {
    configs().map(move |gates| {
        let manager = GatedManager { gates };
        Pool::builder(manager)
            .max_size(max_size)
            .post_create(Hook::async_fn(move |_, _| {
                Box::pin(async move {
                    gates
                        .post_create
                        .open()
                        .await
                        .map_err(|_| HookError::message("Fail"))?;
                    Ok(())
                })
            }))
            .pre_recycle(Hook::async_fn(move |_, _| {
                Box::pin(async move {
                    gates
                        .pre_recycle
                        .open()
                        .await
                        .map_err(|_| HookError::message("pre_recycle gate set to error"))?;
                    Ok(())
                })
            }))
            .post_recycle(Hook::async_fn(move |_, _| {
                Box::pin(async move {
                    gates
                        .post_recycle
                        .open()
                        .await
                        .map_err(|_| HookError::message("post_recycle gate set to error"))?;
                    Ok(())
                })
            }))
            .build()
            .unwrap()
    })
}

struct GatedManager {
    gates: Gates,
}

impl Manager for GatedManager {
    type Type = ();
    type Error = ();
    async fn create(&self) -> Result<Self::Type, Self::Error> {
        self.gates.create.open().await?;
        Ok(())
    }
    async fn recycle(&self, _conn: &mut Self::Type, _: &Metrics) -> RecycleResult<Self::Error> {
        self.gates.recycle.open().await?;
        Ok(())
    }
}

// This tests various combinations of configurations with
// succeeding, failing, slow and hanging managers and hooks.
// It currently tests 4^5 (=1024) possible combinations and
// therefore takes some time to complete. It is probably not
// neccesary to test all combinations, but doing so doesn't
// hurt either and it is a good stress test of the pool.
#[tokio::test(flavor = "multi_thread", worker_threads = 16)]
async fn test_cancellations() {
    for pool in pools(2) {
        let handles = (0..8)
            .map(|i| {
                let pool = pool.clone();
                tokio::spawn(async move {
                    loop {
                        let _obj = timeout(Duration::from_nanos(i), pool.get()).await;
                        sleep(Duration::from_nanos(i)).await;
                    }
                })
            })
            .collect::<Vec<_>>();
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_millis(1)).await;
            let status = pool.status();
            assert!(
                status.size <= status.max_size,
                "size({}) > max_size({}), gates: {:?}",
                status.size,
                status.max_size,
                pool.manager().gates
            );
        }
        for handle in &handles {
            handle.abort();
        }
        for handle in handles {
            let _ = handle.await;
        }
        let status = pool.status();
        assert!(
            status.size <= status.max_size,
            "size({}) > max_size({}), gates: {:?}",
            status.size,
            status.max_size,
            pool.manager().gates
        );
        assert!(
            status.available <= status.max_size,
            "available({}) > max_size({}), gates: {:?}",
            status.available,
            status.max_size,
            pool.manager().gates
        );
    }
}
