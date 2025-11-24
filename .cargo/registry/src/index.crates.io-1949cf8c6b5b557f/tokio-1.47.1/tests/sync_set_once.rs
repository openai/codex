#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use tokio::sync::SetOnce;

#[derive(Clone)]
struct DropCounter {
    drops: Arc<AtomicU32>,
}

impl DropCounter {
    fn new() -> Self {
        DropCounter {
            drops: Arc::new(AtomicU32::new(0)),
        }
    }

    fn assert_num_drops(&self, value: u32) {
        assert_eq!(value, self.drops.load(Ordering::Relaxed));
    }
}

impl Drop for DropCounter {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn drop_cell() {
    let fooer = DropCounter::new();
    let fooer_cl = fooer.clone();

    {
        let once_cell = SetOnce::new();
        let prev = once_cell.set(fooer_cl);
        assert!(prev.is_ok())
    }

    fooer.assert_num_drops(1);
}

#[test]
fn drop_cell_new_with() {
    let fooer = DropCounter::new();

    {
        let once_cell = SetOnce::new_with(Some(fooer.clone()));
        assert!(once_cell.initialized());
    }

    fooer.assert_num_drops(1);
}

#[test]
fn drop_into_inner() {
    let fooer = DropCounter::new();

    let once_cell = SetOnce::new();
    assert!(once_cell.set(fooer.clone()).is_ok());
    let val = once_cell.into_inner();
    fooer.assert_num_drops(0);
    drop(val);
    fooer.assert_num_drops(1);
}

#[test]
fn drop_into_inner_new_with() {
    let fooer = DropCounter::new();

    let once_cell = SetOnce::new_with(Some(fooer.clone()));
    let val = once_cell.into_inner();
    fooer.assert_num_drops(0);
    drop(val);
    fooer.assert_num_drops(1);
}

#[test]
fn from() {
    let cell = SetOnce::from(2);
    assert_eq!(*cell.get().unwrap(), 2);
}

#[test]
fn set_and_get() {
    static ONCE: SetOnce<u32> = SetOnce::const_new();

    ONCE.set(5).unwrap();
    let value = ONCE.get().unwrap();
    assert_eq!(*value, 5);
}

#[tokio::test]
async fn set_and_wait() {
    static ONCE: SetOnce<u32> = SetOnce::const_new();

    tokio::spawn(async { ONCE.set(5) });

    let value = ONCE.wait().await;
    assert_eq!(*value, 5);
}

#[test]
#[cfg_attr(target_family = "wasm", ignore)]
fn set_and_wait_multiple_threads() {
    static ONCE: SetOnce<u32> = SetOnce::const_new();

    let res1 = std::thread::spawn(|| ONCE.set(4));

    let res2 = std::thread::spawn(|| ONCE.set(3));

    let result_first = res1.join().unwrap().is_err();
    let result_two = res2.join().unwrap().is_err();

    assert!(result_first != result_two);
}

#[tokio::test]
#[cfg_attr(target_family = "wasm", ignore)]
async fn set_and_wait_threads() {
    static ONCE: SetOnce<u32> = SetOnce::const_new();

    let thread = std::thread::spawn(|| {
        ONCE.set(4).unwrap();
    });

    let value = ONCE.wait().await;
    thread.join().unwrap();
    assert_eq!(*value, 4);
}

#[test]
fn get_uninit() {
    static ONCE: SetOnce<u32> = SetOnce::const_new();
    let uninit = ONCE.get();
    assert!(uninit.is_none());
}

#[test]
fn set_twice() {
    static ONCE: SetOnce<u32> = SetOnce::const_new();

    let first = ONCE.set(5);
    assert_eq!(first, Ok(()));
    let second = ONCE.set(6);
    assert!(second.is_err());
}

#[test]
fn is_none_initializing() {
    static ONCE: SetOnce<u32> = SetOnce::const_new();

    assert_eq!(ONCE.get(), None);

    ONCE.set(20).unwrap();

    assert!(ONCE.set(10).is_err());
}

#[tokio::test]
async fn is_some_initializing() {
    static ONCE: SetOnce<u32> = SetOnce::const_new();

    tokio::spawn(async { ONCE.set(20) });

    assert_eq!(*ONCE.wait().await, 20);
}

#[test]
fn into_inner_int_empty_setonce() {
    let once = SetOnce::<u32>::new();

    let val = once.into_inner();

    assert!(val.is_none());
}
