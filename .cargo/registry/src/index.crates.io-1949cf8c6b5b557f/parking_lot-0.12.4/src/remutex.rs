// Copyright 2016 Amanieu d'Antras
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use crate::raw_mutex::RawMutex;
use core::num::NonZeroUsize;
use lock_api::{self, GetThreadId};

/// Implementation of the `GetThreadId` trait for `lock_api::ReentrantMutex`.
pub struct RawThreadId;

unsafe impl GetThreadId for RawThreadId {
    const INIT: RawThreadId = RawThreadId;

    fn nonzero_thread_id(&self) -> NonZeroUsize {
        // The address of a thread-local variable is guaranteed to be unique to the
        // current thread, and is also guaranteed to be non-zero. The variable has to have a
        // non-zero size to guarantee it has a unique address for each thread.
        thread_local!(static KEY: u8 = 0);
        KEY.with(|x| {
            NonZeroUsize::new(x as *const _ as usize)
                .expect("thread-local variable address is null")
        })
    }
}

/// A mutex which can be recursively locked by a single thread.
///
/// This type is identical to `Mutex` except for the following points:
///
/// - Locking multiple times from the same thread will work correctly instead of
///   deadlocking.
/// - `ReentrantMutexGuard` does not give mutable references to the locked data.
///   Use a `RefCell` if you need this.
///
/// See [`Mutex`](crate::Mutex) for more details about the underlying mutex
/// primitive.
pub type ReentrantMutex<T> = lock_api::ReentrantMutex<RawMutex, RawThreadId, T>;

/// Creates a new reentrant mutex in an unlocked state ready for use.
///
/// This allows creating a reentrant mutex in a constant context on stable Rust.
pub const fn const_reentrant_mutex<T>(val: T) -> ReentrantMutex<T> {
    ReentrantMutex::const_new(
        <RawMutex as lock_api::RawMutex>::INIT,
        <RawThreadId as lock_api::GetThreadId>::INIT,
        val,
    )
}

/// An RAII implementation of a "scoped lock" of a reentrant mutex. When this structure
/// is dropped (falls out of scope), the lock will be unlocked.
///
/// The data protected by the mutex can be accessed through this guard via its
/// `Deref` implementation.
pub type ReentrantMutexGuard<'a, T> = lock_api::ReentrantMutexGuard<'a, RawMutex, RawThreadId, T>;

/// An RAII mutex guard returned by `ReentrantMutexGuard::map`, which can point to a
/// subfield of the protected data.
///
/// The main difference between `MappedReentrantMutexGuard` and `ReentrantMutexGuard` is that the
/// former doesn't support temporarily unlocking and re-locking, since that
/// could introduce soundness issues if the locked object is modified by another
/// thread.
pub type MappedReentrantMutexGuard<'a, T> =
    lock_api::MappedReentrantMutexGuard<'a, RawMutex, RawThreadId, T>;

#[cfg(test)]
mod tests {
    use crate::ReentrantMutex;
    use crate::ReentrantMutexGuard;
    use std::cell::RefCell;
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::thread;

    #[cfg(feature = "serde")]
    use bincode::{deserialize, serialize};

    #[test]
    fn smoke() {
        let m = ReentrantMutex::new(2);
        {
            let a = m.lock();
            {
                let b = m.lock();
                {
                    let c = m.lock();
                    assert_eq!(*c, 2);
                }
                assert_eq!(*b, 2);
            }
            assert_eq!(*a, 2);
        }
    }

    #[test]
    fn is_mutex() {
        let m = Arc::new(ReentrantMutex::new(RefCell::new(0)));
        let m2 = m.clone();
        let lock = m.lock();
        let child = thread::spawn(move || {
            let lock = m2.lock();
            assert_eq!(*lock.borrow(), 4950);
        });
        for i in 0..100 {
            let lock = m.lock();
            *lock.borrow_mut() += i;
        }
        drop(lock);
        child.join().unwrap();
    }

    #[test]
    fn trylock_works() {
        let m = Arc::new(ReentrantMutex::new(()));
        let m2 = m.clone();
        let _lock = m.try_lock();
        let _lock2 = m.try_lock();
        thread::spawn(move || {
            let lock = m2.try_lock();
            assert!(lock.is_none());
        })
        .join()
        .unwrap();
        let _lock3 = m.try_lock();
    }

    #[test]
    fn test_reentrant_mutex_debug() {
        let mutex = ReentrantMutex::new(vec![0u8, 10]);

        assert_eq!(format!("{:?}", mutex), "ReentrantMutex { data: [0, 10] }");
    }

    #[test]
    fn test_reentrant_mutex_bump() {
        let mutex = Arc::new(ReentrantMutex::new(()));
        let mutex2 = mutex.clone();

        let mut guard = mutex.lock();

        let (tx, rx) = channel();

        thread::spawn(move || {
            let _guard = mutex2.lock();
            tx.send(()).unwrap();
        });

        // `bump()` repeatedly until the thread starts up and requests the lock
        while rx.try_recv().is_err() {
            ReentrantMutexGuard::bump(&mut guard);
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_serde() {
        let contents: Vec<u8> = vec![0, 1, 2];
        let mutex = ReentrantMutex::new(contents.clone());

        let serialized = serialize(&mutex).unwrap();
        let deserialized: ReentrantMutex<Vec<u8>> = deserialize(&serialized).unwrap();

        assert_eq!(*(mutex.lock()), *(deserialized.lock()));
        assert_eq!(contents, *(deserialized.lock()));
    }
}
