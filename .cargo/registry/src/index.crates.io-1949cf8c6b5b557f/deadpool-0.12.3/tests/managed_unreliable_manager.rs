#![cfg(feature = "managed")]

use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use tokio::time;

use deadpool::managed::{self, Metrics, RecycleError, RecycleResult};

type Pool = managed::Pool<Manager>;

struct Manager {
    create_fail: bool,
    recycle_fail: bool,
    detached: AtomicUsize,
}

impl managed::Manager for Manager {
    type Type = ();
    type Error = ();

    async fn create(&self) -> Result<(), ()> {
        if self.create_fail {
            Err(())
        } else {
            Ok(())
        }
    }
    async fn recycle(&self, _conn: &mut (), _: &Metrics) -> RecycleResult<()> {
        if self.recycle_fail {
            Err(RecycleError::Backend(()))
        } else {
            Ok(())
        }
    }

    fn detach(&self, _obj: &mut Self::Type) {
        self.detached.fetch_add(1, Ordering::Relaxed);
    }
}

#[tokio::test]
async fn create() {
    let manager = Manager {
        create_fail: true,
        recycle_fail: false,
        detached: AtomicUsize::new(0),
    };

    let pool = Pool::builder(manager).max_size(16).build().unwrap();
    {
        assert!(pool.get().await.is_err());
    }

    let status = pool.status();
    assert_eq!(status.available, 0);
    assert_eq!(status.size, 0);
    {
        assert!(time::timeout(Duration::from_millis(10), pool.get())
            .await
            .unwrap()
            .is_err());
    }
    assert_eq!(status.available, 0);
    assert_eq!(status.size, 0);
}

#[tokio::test]
async fn recycle() {
    let manager = Manager {
        create_fail: false,
        recycle_fail: true,
        detached: AtomicUsize::new(0),
    };

    let pool = Pool::builder(manager).max_size(16).build().unwrap();
    {
        let _a = pool.get().await.unwrap();
        let _b = pool.get().await.unwrap();
    }

    let status = pool.status();
    assert_eq!(status.available, 2);
    assert_eq!(status.size, 2);
    assert_eq!(pool.manager().detached.load(Ordering::Relaxed), 0);
    {
        let _a = pool.get().await.unwrap();
        // All connections fail to recycle. Thus reducing the
        // available counter to 0.
        let status = pool.status();
        assert_eq!(status.available, 0);
        assert_eq!(status.size, 1);
        assert_eq!(pool.manager().detached.load(Ordering::Relaxed), 2);
    }
    let status = pool.status();
    assert_eq!(status.available, 1);
    assert_eq!(status.size, 1);
}
