use crate::sync::SetOnce;

use loom::future::block_on;
use loom::sync::atomic::AtomicU32;
use loom::thread;
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[derive(Clone)]
struct DropCounter {
    pub drops: Arc<AtomicU32>,
}

impl DropCounter {
    pub fn new() -> Self {
        Self {
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
fn set_once_drop_test() {
    loom::model(|| {
        let set_once = Arc::new(SetOnce::new());
        let set_once_clone = Arc::clone(&set_once);

        let drop_counter = DropCounter::new();
        let counter_cl = drop_counter.clone();

        let thread = thread::spawn(move || set_once_clone.set(counter_cl).is_ok());

        let foo = drop_counter.clone();

        let set = set_once.set(foo).is_ok();
        let res = thread.join().unwrap();

        drop(set_once);

        drop_counter.assert_num_drops(2);
        assert!(res != set);
    });
}

#[test]
fn set_once_wait_test() {
    loom::model(|| {
        let tx = Arc::new(SetOnce::new());
        let rx_one = tx.clone();
        let rx_two = tx.clone();

        let thread = thread::spawn(move || {
            assert!(rx_one.set(2).is_ok());
        });

        block_on(async {
            assert_eq!(*rx_two.wait().await, 2);
        });

        thread.join().unwrap();
    });
}
