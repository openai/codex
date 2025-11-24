use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, TryLockError};
use std::thread;

use crossbeam_utils::sync::ShardedLock;
use rand::Rng;

#[derive(Eq, PartialEq, Debug)]
struct NonCopy(i32);

#[test]
fn smoke() {
    let l = ShardedLock::new(());
    drop(l.read().unwrap());
    drop(l.write().unwrap());
    drop((l.read().unwrap(), l.read().unwrap()));
    drop(l.write().unwrap());
}

#[test]
fn frob() {
    const N: u32 = 10;
    #[cfg(miri)]
    const M: usize = 50;
    #[cfg(not(miri))]
    const M: usize = 1000;

    let r = Arc::new(ShardedLock::new(()));

    let (tx, rx) = channel::<()>();
    for _ in 0..N {
        let tx = tx.clone();
        let r = r.clone();
        thread::spawn(move || {
            let mut rng = rand::thread_rng();
            for _ in 0..M {
                if rng.gen_bool(1.0 / (N as f64)) {
                    drop(r.write().unwrap());
                } else {
                    drop(r.read().unwrap());
                }
            }
            drop(tx);
        });
    }
    drop(tx);
    let _ = rx.recv();
}

#[test]
fn arc_poison_wr() {
    let arc = Arc::new(ShardedLock::new(1));
    let arc2 = arc.clone();
    let _: Result<(), _> = thread::spawn(move || {
        let _lock = arc2.write().unwrap();
        panic!();
    })
    .join();
    assert!(arc.read().is_err());
}

#[test]
fn arc_poison_ww() {
    let arc = Arc::new(ShardedLock::new(1));
    assert!(!arc.is_poisoned());
    let arc2 = arc.clone();
    let _: Result<(), _> = thread::spawn(move || {
        let _lock = arc2.write().unwrap();
        panic!();
    })
    .join();
    assert!(arc.write().is_err());
    assert!(arc.is_poisoned());
}

#[test]
fn arc_no_poison_rr() {
    let arc = Arc::new(ShardedLock::new(1));
    let arc2 = arc.clone();
    let _: Result<(), _> = thread::spawn(move || {
        let _lock = arc2.read().unwrap();
        panic!();
    })
    .join();
    let lock = arc.read().unwrap();
    assert_eq!(*lock, 1);
}
#[test]
fn arc_no_poison_sl() {
    let arc = Arc::new(ShardedLock::new(1));
    let arc2 = arc.clone();
    let _: Result<(), _> = thread::spawn(move || {
        let _lock = arc2.read().unwrap();
        panic!()
    })
    .join();
    let lock = arc.write().unwrap();
    assert_eq!(*lock, 1);
}

#[test]
fn arc() {
    let arc = Arc::new(ShardedLock::new(0));
    let arc2 = arc.clone();
    let (tx, rx) = channel();

    thread::spawn(move || {
        let mut lock = arc2.write().unwrap();
        for _ in 0..10 {
            let tmp = *lock;
            *lock = -1;
            thread::yield_now();
            *lock = tmp + 1;
        }
        tx.send(()).unwrap();
    });

    // Readers try to catch the writer in the act
    let mut children = Vec::new();
    for _ in 0..5 {
        let arc3 = arc.clone();
        children.push(thread::spawn(move || {
            let lock = arc3.read().unwrap();
            assert!(*lock >= 0);
        }));
    }

    // Wait for children to pass their asserts
    for r in children {
        assert!(r.join().is_ok());
    }

    // Wait for writer to finish
    rx.recv().unwrap();
    let lock = arc.read().unwrap();
    assert_eq!(*lock, 10);
}

#[test]
fn arc_access_in_unwind() {
    let arc = Arc::new(ShardedLock::new(1));
    let arc2 = arc.clone();
    let _ = thread::spawn(move || {
        struct Unwinder {
            i: Arc<ShardedLock<isize>>,
        }
        impl Drop for Unwinder {
            fn drop(&mut self) {
                let mut lock = self.i.write().unwrap();
                *lock += 1;
            }
        }
        let _u = Unwinder { i: arc2 };
        panic!();
    })
    .join();
    let lock = arc.read().unwrap();
    assert_eq!(*lock, 2);
}

#[test]
fn unsized_type() {
    let sl: &ShardedLock<[i32]> = &ShardedLock::new([1, 2, 3]);
    {
        let b = &mut *sl.write().unwrap();
        b[0] = 4;
        b[2] = 5;
    }
    let comp: &[i32] = &[4, 2, 5];
    assert_eq!(&*sl.read().unwrap(), comp);
}

#[test]
fn try_write() {
    let lock = ShardedLock::new(0isize);
    let read_guard = lock.read().unwrap();

    let write_result = lock.try_write();
    match write_result {
        Err(TryLockError::WouldBlock) => (),
        Ok(_) => panic!("try_write should not succeed while read_guard is in scope"),
        Err(_) => panic!("unexpected error"),
    }

    drop(read_guard);
}

#[test]
fn test_into_inner() {
    let m = ShardedLock::new(NonCopy(10));
    assert_eq!(m.into_inner().unwrap(), NonCopy(10));
}

#[test]
fn test_into_inner_drop() {
    struct Foo(Arc<AtomicUsize>);
    impl Drop for Foo {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let num_drops = Arc::new(AtomicUsize::new(0));
    let m = ShardedLock::new(Foo(num_drops.clone()));
    assert_eq!(num_drops.load(Ordering::SeqCst), 0);
    {
        let _inner = m.into_inner().unwrap();
        assert_eq!(num_drops.load(Ordering::SeqCst), 0);
    }
    assert_eq!(num_drops.load(Ordering::SeqCst), 1);
}

#[test]
fn test_into_inner_poison() {
    let m = Arc::new(ShardedLock::new(NonCopy(10)));
    let m2 = m.clone();
    let _ = thread::spawn(move || {
        let _lock = m2.write().unwrap();
        panic!("test panic in inner thread to poison ShardedLock");
    })
    .join();

    assert!(m.is_poisoned());
    match Arc::try_unwrap(m).unwrap().into_inner() {
        Err(e) => assert_eq!(e.into_inner(), NonCopy(10)),
        Ok(x) => panic!("into_inner of poisoned ShardedLock is Ok: {:?}", x),
    }
}

#[test]
fn test_get_mut() {
    let mut m = ShardedLock::new(NonCopy(10));
    *m.get_mut().unwrap() = NonCopy(20);
    assert_eq!(m.into_inner().unwrap(), NonCopy(20));
}

#[test]
fn test_get_mut_poison() {
    let m = Arc::new(ShardedLock::new(NonCopy(10)));
    let m2 = m.clone();
    let _ = thread::spawn(move || {
        let _lock = m2.write().unwrap();
        panic!("test panic in inner thread to poison ShardedLock");
    })
    .join();

    assert!(m.is_poisoned());
    match Arc::try_unwrap(m).unwrap().get_mut() {
        Err(e) => assert_eq!(*e.into_inner(), NonCopy(10)),
        Ok(x) => panic!("get_mut of poisoned ShardedLock is Ok: {:?}", x),
    }
}
