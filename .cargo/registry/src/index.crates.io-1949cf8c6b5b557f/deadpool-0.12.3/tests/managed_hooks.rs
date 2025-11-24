#![cfg(feature = "managed")]

use std::sync::atomic::{AtomicUsize, Ordering};

use deadpool::managed::{Hook, HookError, Manager, Metrics, Pool, RecycleResult};

struct Computer {
    next_id: AtomicUsize,
}

impl Computer {
    pub fn new(start: usize) -> Self {
        Self {
            next_id: AtomicUsize::new(start),
        }
    }
}

impl Manager for Computer {
    type Type = usize;
    type Error = ();

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        Ok(self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    async fn recycle(&self, _: &mut Self::Type, _: &Metrics) -> RecycleResult<Self::Error> {
        Ok(())
    }
}

#[tokio::test]
async fn post_create_ok() {
    let manager = Computer::new(42);
    let pool = Pool::<Computer>::builder(manager)
        .max_size(1)
        .post_create(Hook::sync_fn(|obj, _| {
            *obj += 1;
            Ok(())
        }))
        .build()
        .unwrap();
    assert!(*pool.get().await.unwrap() == 43);
}

#[tokio::test]
async fn post_create_ok_async() {
    let manager = Computer::new(42);
    let pool = Pool::<Computer>::builder(manager)
        .max_size(1)
        .post_create(Hook::async_fn(|obj, _| {
            Box::pin(async move {
                *obj += 1;
                Ok(())
            })
        }))
        .build()
        .unwrap();
    assert!(*pool.get().await.unwrap() == 43);
}

#[tokio::test]
async fn post_create_err_abort() {
    let manager = Computer::new(0);
    let pool = Pool::<Computer>::builder(manager)
        .max_size(3)
        .post_create(Hook::sync_fn(|obj, _| {
            (*obj % 2 == 0)
                .then_some(())
                .ok_or(HookError::message("odd creation"))
        }))
        .build()
        .unwrap();
    let obj1 = pool.get().await.unwrap();
    assert_eq!(*obj1, 0);
    assert!(pool.get().await.is_err());
    let obj2 = pool.get().await.unwrap();
    assert_eq!(*obj2, 2);
    assert!(pool.get().await.is_err());
    let obj2 = pool.get().await.unwrap();
    assert_eq!(*obj2, 4);
}

#[tokio::test]
async fn pre_recycle_ok() {
    let manager = Computer::new(42);
    let pool = Pool::<Computer>::builder(manager)
        .max_size(1)
        .pre_recycle(Hook::sync_fn(|obj, _| {
            *obj += 1;
            Ok(())
        }))
        .build()
        .unwrap();
    assert!(*pool.get().await.unwrap() == 42);
    assert!(*pool.get().await.unwrap() == 43);
    assert!(*pool.get().await.unwrap() == 44);
    assert!(*pool.get().await.unwrap() == 45);
}

#[tokio::test]
async fn pre_recycle_err_continue() {
    let manager = Computer::new(0);
    let pool = Pool::<Computer>::builder(manager)
        .max_size(1)
        .pre_recycle(Hook::sync_fn(|_, metrics| {
            if metrics.recycle_count > 0 {
                Err(HookError::message("Fail!"))
            } else {
                Ok(())
            }
        }))
        .build()
        .unwrap();
    assert_eq!(*pool.get().await.unwrap(), 0);
    assert_eq!(pool.status().available, 1);
    assert_eq!(pool.status().size, 1);
    assert_eq!(*pool.get().await.unwrap(), 0);
    assert_eq!(pool.status().available, 1);
    assert_eq!(pool.status().size, 1);
    assert_eq!(*pool.get().await.unwrap(), 1);
    assert_eq!(pool.status().available, 1);
    assert_eq!(pool.status().size, 1);
    assert_eq!(*pool.get().await.unwrap(), 1);
    assert_eq!(pool.status().available, 1);
    assert_eq!(pool.status().size, 1);
    assert_eq!(*pool.get().await.unwrap(), 2);
    assert_eq!(pool.status().available, 1);
    assert_eq!(pool.status().size, 1);
    assert_eq!(*pool.get().await.unwrap(), 2);
    assert_eq!(pool.status().available, 1);
    assert_eq!(pool.status().size, 1);
}

#[tokio::test]
async fn post_recycle_ok() {
    let manager = Computer::new(42);
    let pool = Pool::<Computer>::builder(manager)
        .max_size(1)
        .post_recycle(Hook::sync_fn(|obj, _| {
            *obj += 1;
            Ok(())
        }))
        .build()
        .unwrap();
    assert!(*pool.get().await.unwrap() == 42);
    assert!(*pool.get().await.unwrap() == 43);
    assert!(*pool.get().await.unwrap() == 44);
    assert!(*pool.get().await.unwrap() == 45);
}

#[tokio::test]
async fn post_recycle_err_continue() {
    let manager = Computer::new(0);
    let pool = Pool::<Computer>::builder(manager)
        .max_size(1)
        .post_recycle(Hook::sync_fn(|_, metrics| {
            if metrics.recycle_count > 0 {
                Err(HookError::message("Fail!"))
            } else {
                Ok(())
            }
        }))
        .build()
        .unwrap();
    assert_eq!(*pool.get().await.unwrap(), 0);
    assert_eq!(pool.status().available, 1);
    assert_eq!(pool.status().size, 1);
    assert_eq!(*pool.get().await.unwrap(), 0);
    assert_eq!(pool.status().available, 1);
    assert_eq!(pool.status().size, 1);
    assert_eq!(*pool.get().await.unwrap(), 1);
    assert_eq!(pool.status().available, 1);
    assert_eq!(pool.status().size, 1);
    assert_eq!(*pool.get().await.unwrap(), 1);
    assert_eq!(pool.status().available, 1);
    assert_eq!(pool.status().size, 1);
    assert_eq!(*pool.get().await.unwrap(), 2);
    assert_eq!(pool.status().available, 1);
    assert_eq!(pool.status().size, 1);
    assert_eq!(*pool.get().await.unwrap(), 2);
    assert_eq!(pool.status().available, 1);
    assert_eq!(pool.status().size, 1);
}
