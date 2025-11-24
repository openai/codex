#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))]
#![cfg(panic = "unwind")]

use std::{error::Error, sync::Arc};
use tokio::{
    runtime::{Builder, Runtime},
    sync::{broadcast, mpsc, oneshot, Mutex, RwLock, Semaphore},
};

mod support {
    pub mod panic;
}
use support::panic::test_panic;

#[test]
fn broadcast_channel_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let (_, _) = broadcast::channel::<u32>(0);
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn mutex_blocking_lock_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = current_thread();
        rt.block_on(async {
            let mutex = Mutex::new(5_u32);
            let _g = mutex.blocking_lock();
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn oneshot_blocking_recv_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = current_thread();
        rt.block_on(async {
            let (_tx, rx) = oneshot::channel::<u8>();
            let _ = rx.blocking_recv();
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn rwlock_with_max_readers_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let _ = RwLock::<u8>::with_max_readers(0, (u32::MAX >> 3) + 1);
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn rwlock_blocking_read_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = current_thread();
        rt.block_on(async {
            let lock = RwLock::<u8>::new(0);
            let _ = lock.blocking_read();
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn rwlock_blocking_write_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = current_thread();
        rt.block_on(async {
            let lock = RwLock::<u8>::new(0);
            let _ = lock.blocking_write();
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn mpsc_bounded_channel_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let (_, _) = mpsc::channel::<u8>(0);
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn mpsc_bounded_receiver_blocking_recv_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = current_thread();
        let (_tx, mut rx) = mpsc::channel::<u8>(1);
        rt.block_on(async {
            let _ = rx.blocking_recv();
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn mpsc_bounded_receiver_blocking_recv_many_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = current_thread();
        let (_tx, mut rx) = mpsc::channel::<u8>(1);
        rt.block_on(async {
            let _ = rx.blocking_recv();
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn mpsc_bounded_sender_blocking_send_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = current_thread();
        let (tx, _rx) = mpsc::channel::<u8>(1);
        rt.block_on(async {
            let _ = tx.blocking_send(3);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn mpsc_unbounded_receiver_blocking_recv_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = current_thread();
        let (_tx, mut rx) = mpsc::unbounded_channel::<u8>();
        rt.block_on(async {
            let _ = rx.blocking_recv();
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn mpsc_unbounded_receiver_blocking_recv_many_panic_caller() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let rt = current_thread();
        let (_tx, mut rx) = mpsc::unbounded_channel::<u8>();
        let mut vec = vec![];
        rt.block_on(async {
            let _ = rx.blocking_recv_many(&mut vec, 1);
        });
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn semaphore_merge_unrelated_owned_permits() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let sem1 = Arc::new(Semaphore::new(42));
        let sem2 = Arc::new(Semaphore::new(42));
        let mut p1 = sem1.try_acquire_owned().unwrap();
        let p2 = sem2.try_acquire_owned().unwrap();
        p1.merge(p2);
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

#[test]
fn semaphore_merge_unrelated_permits() -> Result<(), Box<dyn Error>> {
    let panic_location_file = test_panic(|| {
        let sem1 = Semaphore::new(42);
        let sem2 = Semaphore::new(42);
        let mut p1 = sem1.try_acquire().unwrap();
        let p2 = sem2.try_acquire().unwrap();
        p1.merge(p2);
    });

    // The panic location should be in this file
    assert_eq!(&panic_location_file.unwrap(), file!());

    Ok(())
}

fn current_thread() -> Runtime {
    Builder::new_current_thread().enable_all().build().unwrap()
}
