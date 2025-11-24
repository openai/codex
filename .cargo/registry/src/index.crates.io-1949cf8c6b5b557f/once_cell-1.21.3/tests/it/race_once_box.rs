#[cfg(feature = "std")]
use std::sync::Barrier;
use std::sync::{
    atomic::{AtomicUsize, Ordering::SeqCst},
    Arc,
};

use once_cell::race::OnceBox;

#[derive(Default)]
struct Heap {
    total: Arc<AtomicUsize>,
}

#[derive(Debug)]
struct Pebble<T> {
    val: T,
    total: Arc<AtomicUsize>,
}

impl<T> Drop for Pebble<T> {
    fn drop(&mut self) {
        self.total.fetch_sub(1, SeqCst);
    }
}

impl Heap {
    fn total(&self) -> usize {
        self.total.load(SeqCst)
    }
    fn new_pebble<T>(&self, val: T) -> Pebble<T> {
        self.total.fetch_add(1, SeqCst);
        Pebble { val, total: Arc::clone(&self.total) }
    }
}

#[cfg(feature = "std")]
#[test]
fn once_box_smoke_test() {
    use std::thread::scope;

    let heap = Heap::default();
    let global_cnt = AtomicUsize::new(0);
    let cell = OnceBox::new();
    let b = Barrier::new(128);
    scope(|s| {
        for _ in 0..128 {
            s.spawn(|| {
                let local_cnt = AtomicUsize::new(0);
                cell.get_or_init(|| {
                    global_cnt.fetch_add(1, SeqCst);
                    local_cnt.fetch_add(1, SeqCst);
                    b.wait();
                    Box::new(heap.new_pebble(()))
                });
                assert_eq!(local_cnt.load(SeqCst), 1);

                cell.get_or_init(|| {
                    global_cnt.fetch_add(1, SeqCst);
                    local_cnt.fetch_add(1, SeqCst);
                    Box::new(heap.new_pebble(()))
                });
                assert_eq!(local_cnt.load(SeqCst), 1);
            });
        }
    });
    assert!(cell.get().is_some());
    assert!(global_cnt.load(SeqCst) > 10);

    assert_eq!(heap.total(), 1);
    drop(cell);
    assert_eq!(heap.total(), 0);
}

#[test]
fn once_box_set() {
    let heap = Heap::default();
    let cell = OnceBox::new();
    assert!(cell.get().is_none());

    assert!(cell.set(Box::new(heap.new_pebble("hello"))).is_ok());
    assert_eq!(cell.get().unwrap().val, "hello");
    assert_eq!(heap.total(), 1);

    assert!(cell.set(Box::new(heap.new_pebble("world"))).is_err());
    assert_eq!(cell.get().unwrap().val, "hello");
    assert_eq!(heap.total(), 1);

    drop(cell);
    assert_eq!(heap.total(), 0);
}

#[cfg(feature = "std")]
#[test]
fn once_box_first_wins() {
    use std::thread::scope;

    let cell = OnceBox::new();
    let val1 = 92;
    let val2 = 62;

    let b1 = Barrier::new(2);
    let b2 = Barrier::new(2);
    let b3 = Barrier::new(2);
    scope(|s| {
        s.spawn(|| {
            let r1 = cell.get_or_init(|| {
                b1.wait();
                b2.wait();
                Box::new(val1)
            });
            assert_eq!(*r1, val1);
            b3.wait();
        });
        b1.wait();
        s.spawn(|| {
            let r2 = cell.get_or_init(|| {
                b2.wait();
                b3.wait();
                Box::new(val2)
            });
            assert_eq!(*r2, val1);
        });
    });

    assert_eq!(cell.get(), Some(&val1));
}

#[test]
fn once_box_reentrant() {
    let cell = OnceBox::new();
    let res = cell.get_or_init(|| {
        cell.get_or_init(|| Box::new("hello".to_string()));
        Box::new("world".to_string())
    });
    assert_eq!(res, "hello");
}

#[test]
fn once_box_default() {
    struct Foo;

    let cell: OnceBox<Foo> = Default::default();
    assert!(cell.get().is_none());
}

#[test]
fn onece_box_with_value() {
    let cell = OnceBox::with_value(Box::new(92));
    assert_eq!(cell.get(), Some(&92));
}

#[test]
fn onece_box_clone() {
    let cell1 = OnceBox::new();
    let cell2 = cell1.clone();
    cell1.set(Box::new(92)).unwrap();
    let cell3 = cell1.clone();
    assert_eq!(cell1.get(), Some(&92));
    assert_eq!(cell2.get(), None);
    assert_eq!(cell3.get(), Some(&92));
}
