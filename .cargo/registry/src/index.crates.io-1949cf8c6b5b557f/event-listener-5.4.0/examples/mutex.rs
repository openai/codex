//! A simple mutex implementation.
//!
//! This mutex exposes both blocking and async methods for acquiring a lock.

#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod example {
    #![allow(dead_code)]

    use std::ops::{Deref, DerefMut};
    use std::sync::{mpsc, Arc};
    use std::thread;
    use std::time::{Duration, Instant};

    use event_listener::{listener, Event, Listener};
    use try_lock::{Locked, TryLock};

    /// A simple mutex.
    struct Mutex<T> {
        /// Blocked lock operations.
        lock_ops: Event,

        /// The inner non-blocking mutex.
        data: TryLock<T>,
    }

    unsafe impl<T: Send> Send for Mutex<T> {}
    unsafe impl<T: Send> Sync for Mutex<T> {}

    impl<T> Mutex<T> {
        /// Creates a mutex.
        fn new(t: T) -> Mutex<T> {
            Mutex {
                lock_ops: Event::new(),
                data: TryLock::new(t),
            }
        }

        /// Attempts to acquire a lock.
        fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
            self.data.try_lock().map(|l| MutexGuard {
                lock_ops: &self.lock_ops,
                locked: Some(l),
            })
        }

        /// Blocks until a lock is acquired.
        fn lock(&self) -> MutexGuard<'_, T> {
            loop {
                // Attempt grabbing a lock.
                if let Some(guard) = self.try_lock() {
                    return guard;
                }

                // Set up an event listener.
                listener!(self.lock_ops => listener);

                // Try again.
                if let Some(guard) = self.try_lock() {
                    return guard;
                }

                // Wait for a notification.
                listener.wait();
            }
        }

        /// Blocks until a lock is acquired or the timeout is reached.
        fn lock_timeout(&self, timeout: Duration) -> Option<MutexGuard<'_, T>> {
            let deadline = Instant::now() + timeout;

            loop {
                // Attempt grabbing a lock.
                if let Some(guard) = self.try_lock() {
                    return Some(guard);
                }

                // Set up an event listener.
                listener!(self.lock_ops => listener);

                // Try again.
                if let Some(guard) = self.try_lock() {
                    return Some(guard);
                }

                // Wait until a notification is received.
                listener.wait_deadline(deadline)?;
            }
        }

        /// Acquires a lock asynchronously.
        async fn lock_async(&self) -> MutexGuard<'_, T> {
            loop {
                // Attempt grabbing a lock.
                if let Some(guard) = self.try_lock() {
                    return guard;
                }

                // Set up an event listener.
                listener!(self.lock_ops => listener);

                // Try again.
                if let Some(guard) = self.try_lock() {
                    return guard;
                }

                // Wait until a notification is received.
                listener.await;
            }
        }
    }

    /// A guard holding a lock.
    struct MutexGuard<'a, T> {
        lock_ops: &'a Event,
        locked: Option<Locked<'a, T>>,
    }

    impl<T> Deref for MutexGuard<'_, T> {
        type Target = T;

        fn deref(&self) -> &T {
            self.locked.as_deref().unwrap()
        }
    }

    impl<T> DerefMut for MutexGuard<'_, T> {
        fn deref_mut(&mut self) -> &mut T {
            self.locked.as_deref_mut().unwrap()
        }
    }

    impl<T> Drop for MutexGuard<'_, T> {
        fn drop(&mut self) {
            self.locked = None;
            self.lock_ops.notify(1);
        }
    }

    pub(super) fn entry() {
        const N: usize = 10;

        // A shared counter.
        let counter = Arc::new(Mutex::new(0));

        // A channel that signals when all threads are done.
        let (tx, rx) = mpsc::channel();

        // Spawn a bunch of threads incrementing the counter.
        for _ in 0..N {
            let counter = counter.clone();
            let tx = tx.clone();

            thread::spawn(move || {
                let mut counter = counter.lock();
                *counter += 1;

                // If this is the last increment, signal that we're done.
                if *counter == N {
                    tx.send(()).unwrap();
                }
            });
        }

        // Wait until the last thread increments the counter.
        rx.recv().unwrap();

        // The counter must equal the number of threads.
        assert_eq!(*counter.lock(), N);

        println!("Done!");
    }
}

#[cfg(any(target_family = "wasm", not(feature = "std")))]
mod example {
    pub(super) fn entry() {
        println!("This example is not supported on wasm yet.");
    }
}

fn main() {
    example::entry();
}
