// Copyright 2016 Amanieu d'Antras
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use crate::raw_fair_mutex::RawFairMutex;

/// A mutual exclusive primitive that is always fair, useful for protecting shared data
///
/// This mutex will block threads waiting for the lock to become available. The
/// mutex can be statically initialized or created by the `new`
/// constructor. Each mutex has a type parameter which represents the data that
/// it is protecting. The data can only be accessed through the RAII guards
/// returned from `lock` and `try_lock`, which guarantees that the data is only
/// ever accessed when the mutex is locked.
///
/// The regular mutex provided by `parking_lot` uses eventual fairness
/// (after some time it will default to the fair algorithm), but eventual
/// fairness does not provide the same guarantees an always fair method would.
/// Fair mutexes are generally slower, but sometimes needed.
///
/// In a fair mutex the waiters form a queue, and the lock is always granted to
/// the next requester in the queue, in first-in first-out order. This ensures
/// that one thread cannot starve others by quickly re-acquiring the lock after
/// releasing it.
///
/// A fair mutex may not be interesting if threads have different priorities (this is known as
/// priority inversion).
///
/// # Differences from the standard library `Mutex`
///
/// - No poisoning, the lock is released normally on panic.
/// - Only requires 1 byte of space, whereas the standard library boxes the
///   `FairMutex` due to platform limitations.
/// - Can be statically constructed.
/// - Does not require any drop glue when dropped.
/// - Inline fast path for the uncontended case.
/// - Efficient handling of micro-contention using adaptive spinning.
/// - Allows raw locking & unlocking without a guard.
///
/// # Examples
///
/// ```
/// use parking_lot::FairMutex;
/// use std::sync::{Arc, mpsc::channel};
/// use std::thread;
///
/// const N: usize = 10;
///
/// // Spawn a few threads to increment a shared variable (non-atomically), and
/// // let the main thread know once all increments are done.
/// //
/// // Here we're using an Arc to share memory among threads, and the data inside
/// // the Arc is protected with a mutex.
/// let data = Arc::new(FairMutex::new(0));
///
/// let (tx, rx) = channel();
/// for _ in 0..10 {
///     let (data, tx) = (Arc::clone(&data), tx.clone());
///     thread::spawn(move || {
///         // The shared state can only be accessed once the lock is held.
///         // Our non-atomic increment is safe because we're the only thread
///         // which can access the shared state when the lock is held.
///         let mut data = data.lock();
///         *data += 1;
///         if *data == N {
///             tx.send(()).unwrap();
///         }
///         // the lock is unlocked here when `data` goes out of scope.
///     });
/// }
///
/// rx.recv().unwrap();
/// ```
pub type FairMutex<T> = lock_api::Mutex<RawFairMutex, T>;

/// Creates a new fair mutex in an unlocked state ready for use.
///
/// This allows creating a fair mutex in a constant context on stable Rust.
pub const fn const_fair_mutex<T>(val: T) -> FairMutex<T> {
    FairMutex::const_new(<RawFairMutex as lock_api::RawMutex>::INIT, val)
}

/// An RAII implementation of a "scoped lock" of a mutex. When this structure is
/// dropped (falls out of scope), the lock will be unlocked.
///
/// The data protected by the mutex can be accessed through this guard via its
/// `Deref` and `DerefMut` implementations.
pub type FairMutexGuard<'a, T> = lock_api::MutexGuard<'a, RawFairMutex, T>;

/// An RAII mutex guard returned by `FairMutexGuard::map`, which can point to a
/// subfield of the protected data.
///
/// The main difference between `MappedFairMutexGuard` and `FairMutexGuard` is that the
/// former doesn't support temporarily unlocking and re-locking, since that
/// could introduce soundness issues if the locked object is modified by another
/// thread.
pub type MappedFairMutexGuard<'a, T> = lock_api::MappedMutexGuard<'a, RawFairMutex, T>;

#[cfg(test)]
mod tests {
    use crate::FairMutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::thread;

    #[cfg(feature = "serde")]
    use bincode::{deserialize, serialize};

    #[derive(Eq, PartialEq, Debug)]
    struct NonCopy(i32);

    #[test]
    fn smoke() {
        let m = FairMutex::new(());
        drop(m.lock());
        drop(m.lock());
    }

    #[test]
    fn lots_and_lots() {
        const J: u32 = 1000;
        const K: u32 = 3;

        let m = Arc::new(FairMutex::new(0));

        fn inc(m: &FairMutex<u32>) {
            for _ in 0..J {
                *m.lock() += 1;
            }
        }

        let (tx, rx) = channel();
        for _ in 0..K {
            let tx2 = tx.clone();
            let m2 = m.clone();
            thread::spawn(move || {
                inc(&m2);
                tx2.send(()).unwrap();
            });
            let tx2 = tx.clone();
            let m2 = m.clone();
            thread::spawn(move || {
                inc(&m2);
                tx2.send(()).unwrap();
            });
        }

        drop(tx);
        for _ in 0..2 * K {
            rx.recv().unwrap();
        }
        assert_eq!(*m.lock(), J * K * 2);
    }

    #[test]
    fn try_lock() {
        let m = FairMutex::new(());
        *m.try_lock().unwrap() = ();
    }

    #[test]
    fn test_into_inner() {
        let m = FairMutex::new(NonCopy(10));
        assert_eq!(m.into_inner(), NonCopy(10));
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
        let m = FairMutex::new(Foo(num_drops.clone()));
        assert_eq!(num_drops.load(Ordering::SeqCst), 0);
        {
            let _inner = m.into_inner();
            assert_eq!(num_drops.load(Ordering::SeqCst), 0);
        }
        assert_eq!(num_drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_get_mut() {
        let mut m = FairMutex::new(NonCopy(10));
        *m.get_mut() = NonCopy(20);
        assert_eq!(m.into_inner(), NonCopy(20));
    }

    #[test]
    fn test_mutex_arc_nested() {
        // Tests nested mutexes and access
        // to underlying data.
        let arc = Arc::new(FairMutex::new(1));
        let arc2 = Arc::new(FairMutex::new(arc));
        let (tx, rx) = channel();
        let _t = thread::spawn(move || {
            let lock = arc2.lock();
            let lock2 = lock.lock();
            assert_eq!(*lock2, 1);
            tx.send(()).unwrap();
        });
        rx.recv().unwrap();
    }

    #[test]
    fn test_mutex_arc_access_in_unwind() {
        let arc = Arc::new(FairMutex::new(1));
        let arc2 = arc.clone();
        let _ = thread::spawn(move || {
            struct Unwinder {
                i: Arc<FairMutex<i32>>,
            }
            impl Drop for Unwinder {
                fn drop(&mut self) {
                    *self.i.lock() += 1;
                }
            }
            let _u = Unwinder { i: arc2 };
            panic!();
        })
        .join();
        let lock = arc.lock();
        assert_eq!(*lock, 2);
    }

    #[test]
    fn test_mutex_unsized() {
        let mutex: &FairMutex<[i32]> = &FairMutex::new([1, 2, 3]);
        {
            let b = &mut *mutex.lock();
            b[0] = 4;
            b[2] = 5;
        }
        let comp: &[i32] = &[4, 2, 5];
        assert_eq!(&*mutex.lock(), comp);
    }

    #[test]
    fn test_mutexguard_sync() {
        fn sync<T: Sync>(_: T) {}

        let mutex = FairMutex::new(());
        sync(mutex.lock());
    }

    #[test]
    fn test_mutex_debug() {
        let mutex = FairMutex::new(vec![0u8, 10]);

        assert_eq!(format!("{:?}", mutex), "Mutex { data: [0, 10] }");
        let _lock = mutex.lock();
        assert_eq!(format!("{:?}", mutex), "Mutex { data: <locked> }");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_serde() {
        let contents: Vec<u8> = vec![0, 1, 2];
        let mutex = FairMutex::new(contents.clone());

        let serialized = serialize(&mutex).unwrap();
        let deserialized: FairMutex<Vec<u8>> = deserialize(&serialized).unwrap();

        assert_eq!(*(mutex.lock()), *(deserialized.lock()));
        assert_eq!(contents, *(deserialized.lock()));
    }
}
