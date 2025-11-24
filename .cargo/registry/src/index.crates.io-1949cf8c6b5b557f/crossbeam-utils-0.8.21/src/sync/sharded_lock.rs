use std::boxed::Box;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::{LockResult, PoisonError, TryLockError, TryLockResult};
use std::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::thread::{self, ThreadId};
use std::vec::Vec;

use crate::sync::once_lock::OnceLock;
use crate::CachePadded;

/// The number of shards per sharded lock. Must be a power of two.
const NUM_SHARDS: usize = 8;

/// A shard containing a single reader-writer lock.
struct Shard {
    /// The inner reader-writer lock.
    lock: RwLock<()>,

    /// The write-guard keeping this shard locked.
    ///
    /// Write operations will lock each shard and store the guard here. These guards get dropped at
    /// the same time the big guard is dropped.
    write_guard: UnsafeCell<Option<RwLockWriteGuard<'static, ()>>>,
}

/// A sharded reader-writer lock.
///
/// This lock is equivalent to [`RwLock`], except read operations are faster and write operations
/// are slower.
///
/// A `ShardedLock` is internally made of a list of *shards*, each being a [`RwLock`] occupying a
/// single cache line. Read operations will pick one of the shards depending on the current thread
/// and lock it. Write operations need to lock all shards in succession.
///
/// By splitting the lock into shards, concurrent read operations will in most cases choose
/// different shards and thus update different cache lines, which is good for scalability. However,
/// write operations need to do more work and are therefore slower than usual.
///
/// The priority policy of the lock is dependent on the underlying operating system's
/// implementation, and this type does not guarantee that any particular policy will be used.
///
/// # Poisoning
///
/// A `ShardedLock`, like [`RwLock`], will become poisoned on a panic. Note that it may only be
/// poisoned if a panic occurs while a write operation is in progress. If a panic occurs in any
/// read operation, the lock will not be poisoned.
///
/// # Examples
///
/// ```
/// use crossbeam_utils::sync::ShardedLock;
///
/// let lock = ShardedLock::new(5);
///
/// // Any number of read locks can be held at once.
/// {
///     let r1 = lock.read().unwrap();
///     let r2 = lock.read().unwrap();
///     assert_eq!(*r1, 5);
///     assert_eq!(*r2, 5);
/// } // Read locks are dropped at this point.
///
/// // However, only one write lock may be held.
/// {
///     let mut w = lock.write().unwrap();
///     *w += 1;
///     assert_eq!(*w, 6);
/// } // Write lock is dropped here.
/// ```
///
/// [`RwLock`]: std::sync::RwLock
pub struct ShardedLock<T: ?Sized> {
    /// A list of locks protecting the internal data.
    shards: Box<[CachePadded<Shard>]>,

    /// The internal data.
    value: UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Send for ShardedLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for ShardedLock<T> {}

impl<T: ?Sized> UnwindSafe for ShardedLock<T> {}
impl<T: ?Sized> RefUnwindSafe for ShardedLock<T> {}

impl<T> ShardedLock<T> {
    /// Creates a new sharded reader-writer lock.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::ShardedLock;
    ///
    /// let lock = ShardedLock::new(5);
    /// ```
    pub fn new(value: T) -> ShardedLock<T> {
        ShardedLock {
            shards: (0..NUM_SHARDS)
                .map(|_| {
                    CachePadded::new(Shard {
                        lock: RwLock::new(()),
                        write_guard: UnsafeCell::new(None),
                    })
                })
                .collect::<Box<[_]>>(),
            value: UnsafeCell::new(value),
        }
    }

    /// Consumes this lock, returning the underlying data.
    ///
    /// # Errors
    ///
    /// This method will return an error if the lock is poisoned. A lock gets poisoned when a write
    /// operation panics.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::ShardedLock;
    ///
    /// let lock = ShardedLock::new(String::new());
    /// {
    ///     let mut s = lock.write().unwrap();
    ///     *s = "modified".to_owned();
    /// }
    /// assert_eq!(lock.into_inner().unwrap(), "modified");
    /// ```
    pub fn into_inner(self) -> LockResult<T> {
        let is_poisoned = self.is_poisoned();
        let inner = self.value.into_inner();

        if is_poisoned {
            Err(PoisonError::new(inner))
        } else {
            Ok(inner)
        }
    }
}

impl<T: ?Sized> ShardedLock<T> {
    /// Returns `true` if the lock is poisoned.
    ///
    /// If another thread can still access the lock, it may become poisoned at any time. A `false`
    /// result should not be trusted without additional synchronization.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::ShardedLock;
    /// use std::sync::Arc;
    /// use std::thread;
    ///
    /// let lock = Arc::new(ShardedLock::new(0));
    /// let c_lock = lock.clone();
    ///
    /// let _ = thread::spawn(move || {
    ///     let _lock = c_lock.write().unwrap();
    ///     panic!(); // the lock gets poisoned
    /// }).join();
    /// assert_eq!(lock.is_poisoned(), true);
    /// ```
    pub fn is_poisoned(&self) -> bool {
        self.shards[0].lock.is_poisoned()
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the lock mutably, no actual locking needs to take place.
    ///
    /// # Errors
    ///
    /// This method will return an error if the lock is poisoned. A lock gets poisoned when a write
    /// operation panics.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::ShardedLock;
    ///
    /// let mut lock = ShardedLock::new(0);
    /// *lock.get_mut().unwrap() = 10;
    /// assert_eq!(*lock.read().unwrap(), 10);
    /// ```
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        let is_poisoned = self.is_poisoned();
        let inner = unsafe { &mut *self.value.get() };

        if is_poisoned {
            Err(PoisonError::new(inner))
        } else {
            Ok(inner)
        }
    }

    /// Attempts to acquire this lock with shared read access.
    ///
    /// If the access could not be granted at this time, an error is returned. Otherwise, a guard
    /// is returned which will release the shared access when it is dropped. This method does not
    /// provide any guarantees with respect to the ordering of whether contentious readers or
    /// writers will acquire the lock first.
    ///
    /// # Errors
    ///
    /// This method will return an error if the lock is poisoned. A lock gets poisoned when a write
    /// operation panics.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::ShardedLock;
    ///
    /// let lock = ShardedLock::new(1);
    ///
    /// match lock.try_read() {
    ///     Ok(n) => assert_eq!(*n, 1),
    ///     Err(_) => unreachable!(),
    /// };
    /// ```
    pub fn try_read(&self) -> TryLockResult<ShardedLockReadGuard<'_, T>> {
        // Take the current thread index and map it to a shard index. Thread indices will tend to
        // distribute shards among threads equally, thus reducing contention due to read-locking.
        let current_index = current_index().unwrap_or(0);
        let shard_index = current_index & (self.shards.len() - 1);

        match self.shards[shard_index].lock.try_read() {
            Ok(guard) => Ok(ShardedLockReadGuard {
                lock: self,
                _guard: guard,
                _marker: PhantomData,
            }),
            Err(TryLockError::Poisoned(err)) => {
                let guard = ShardedLockReadGuard {
                    lock: self,
                    _guard: err.into_inner(),
                    _marker: PhantomData,
                };
                Err(TryLockError::Poisoned(PoisonError::new(guard)))
            }
            Err(TryLockError::WouldBlock) => Err(TryLockError::WouldBlock),
        }
    }

    /// Locks with shared read access, blocking the current thread until it can be acquired.
    ///
    /// The calling thread will be blocked until there are no more writers which hold the lock.
    /// There may be other readers currently inside the lock when this method returns. This method
    /// does not provide any guarantees with respect to the ordering of whether contentious readers
    /// or writers will acquire the lock first.
    ///
    /// Returns a guard which will release the shared access when dropped.
    ///
    /// # Errors
    ///
    /// This method will return an error if the lock is poisoned. A lock gets poisoned when a write
    /// operation panics.
    ///
    /// # Panics
    ///
    /// This method might panic when called if the lock is already held by the current thread.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::ShardedLock;
    /// use std::sync::Arc;
    /// use std::thread;
    ///
    /// let lock = Arc::new(ShardedLock::new(1));
    /// let c_lock = lock.clone();
    ///
    /// let n = lock.read().unwrap();
    /// assert_eq!(*n, 1);
    ///
    /// thread::spawn(move || {
    ///     let r = c_lock.read();
    ///     assert!(r.is_ok());
    /// }).join().unwrap();
    /// ```
    pub fn read(&self) -> LockResult<ShardedLockReadGuard<'_, T>> {
        // Take the current thread index and map it to a shard index. Thread indices will tend to
        // distribute shards among threads equally, thus reducing contention due to read-locking.
        let current_index = current_index().unwrap_or(0);
        let shard_index = current_index & (self.shards.len() - 1);

        match self.shards[shard_index].lock.read() {
            Ok(guard) => Ok(ShardedLockReadGuard {
                lock: self,
                _guard: guard,
                _marker: PhantomData,
            }),
            Err(err) => Err(PoisonError::new(ShardedLockReadGuard {
                lock: self,
                _guard: err.into_inner(),
                _marker: PhantomData,
            })),
        }
    }

    /// Attempts to acquire this lock with exclusive write access.
    ///
    /// If the access could not be granted at this time, an error is returned. Otherwise, a guard
    /// is returned which will release the exclusive access when it is dropped. This method does
    /// not provide any guarantees with respect to the ordering of whether contentious readers or
    /// writers will acquire the lock first.
    ///
    /// # Errors
    ///
    /// This method will return an error if the lock is poisoned. A lock gets poisoned when a write
    /// operation panics.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::ShardedLock;
    ///
    /// let lock = ShardedLock::new(1);
    ///
    /// let n = lock.read().unwrap();
    /// assert_eq!(*n, 1);
    ///
    /// assert!(lock.try_write().is_err());
    /// ```
    pub fn try_write(&self) -> TryLockResult<ShardedLockWriteGuard<'_, T>> {
        let mut poisoned = false;
        let mut blocked = None;

        // Write-lock each shard in succession.
        for (i, shard) in self.shards.iter().enumerate() {
            let guard = match shard.lock.try_write() {
                Ok(guard) => guard,
                Err(TryLockError::Poisoned(err)) => {
                    poisoned = true;
                    err.into_inner()
                }
                Err(TryLockError::WouldBlock) => {
                    blocked = Some(i);
                    break;
                }
            };

            // Store the guard into the shard.
            unsafe {
                let guard: RwLockWriteGuard<'static, ()> = mem::transmute(guard);
                let dest: *mut _ = shard.write_guard.get();
                *dest = Some(guard);
            }
        }

        if let Some(i) = blocked {
            // Unlock the shards in reverse order of locking.
            for shard in self.shards[0..i].iter().rev() {
                unsafe {
                    let dest: *mut _ = shard.write_guard.get();
                    let guard = (*dest).take();
                    drop(guard);
                }
            }
            Err(TryLockError::WouldBlock)
        } else if poisoned {
            let guard = ShardedLockWriteGuard {
                lock: self,
                _marker: PhantomData,
            };
            Err(TryLockError::Poisoned(PoisonError::new(guard)))
        } else {
            Ok(ShardedLockWriteGuard {
                lock: self,
                _marker: PhantomData,
            })
        }
    }

    /// Locks with exclusive write access, blocking the current thread until it can be acquired.
    ///
    /// The calling thread will be blocked until there are no more writers which hold the lock.
    /// There may be other readers currently inside the lock when this method returns. This method
    /// does not provide any guarantees with respect to the ordering of whether contentious readers
    /// or writers will acquire the lock first.
    ///
    /// Returns a guard which will release the exclusive access when dropped.
    ///
    /// # Errors
    ///
    /// This method will return an error if the lock is poisoned. A lock gets poisoned when a write
    /// operation panics.
    ///
    /// # Panics
    ///
    /// This method might panic when called if the lock is already held by the current thread.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::ShardedLock;
    ///
    /// let lock = ShardedLock::new(1);
    ///
    /// let mut n = lock.write().unwrap();
    /// *n = 2;
    ///
    /// assert!(lock.try_read().is_err());
    /// ```
    pub fn write(&self) -> LockResult<ShardedLockWriteGuard<'_, T>> {
        let mut poisoned = false;

        // Write-lock each shard in succession.
        for shard in self.shards.iter() {
            let guard = match shard.lock.write() {
                Ok(guard) => guard,
                Err(err) => {
                    poisoned = true;
                    err.into_inner()
                }
            };

            // Store the guard into the shard.
            unsafe {
                let guard: RwLockWriteGuard<'_, ()> = guard;
                let guard: RwLockWriteGuard<'static, ()> = mem::transmute(guard);
                let dest: *mut _ = shard.write_guard.get();
                *dest = Some(guard);
            }
        }

        if poisoned {
            Err(PoisonError::new(ShardedLockWriteGuard {
                lock: self,
                _marker: PhantomData,
            }))
        } else {
            Ok(ShardedLockWriteGuard {
                lock: self,
                _marker: PhantomData,
            })
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for ShardedLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_read() {
            Ok(guard) => f
                .debug_struct("ShardedLock")
                .field("data", &&*guard)
                .finish(),
            Err(TryLockError::Poisoned(err)) => f
                .debug_struct("ShardedLock")
                .field("data", &&**err.get_ref())
                .finish(),
            Err(TryLockError::WouldBlock) => {
                struct LockedPlaceholder;
                impl fmt::Debug for LockedPlaceholder {
                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("<locked>")
                    }
                }
                f.debug_struct("ShardedLock")
                    .field("data", &LockedPlaceholder)
                    .finish()
            }
        }
    }
}

impl<T: Default> Default for ShardedLock<T> {
    fn default() -> ShardedLock<T> {
        ShardedLock::new(Default::default())
    }
}

impl<T> From<T> for ShardedLock<T> {
    fn from(t: T) -> Self {
        ShardedLock::new(t)
    }
}

/// A guard used to release the shared read access of a [`ShardedLock`] when dropped.
#[clippy::has_significant_drop]
pub struct ShardedLockReadGuard<'a, T: ?Sized> {
    lock: &'a ShardedLock<T>,
    _guard: RwLockReadGuard<'a, ()>,
    _marker: PhantomData<RwLockReadGuard<'a, T>>,
}

unsafe impl<T: ?Sized + Sync> Sync for ShardedLockReadGuard<'_, T> {}

impl<T: ?Sized> Deref for ShardedLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.value.get() }
    }
}

impl<T: fmt::Debug> fmt::Debug for ShardedLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShardedLockReadGuard")
            .field("lock", &self.lock)
            .finish()
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for ShardedLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

/// A guard used to release the exclusive write access of a [`ShardedLock`] when dropped.
#[clippy::has_significant_drop]
pub struct ShardedLockWriteGuard<'a, T: ?Sized> {
    lock: &'a ShardedLock<T>,
    _marker: PhantomData<RwLockWriteGuard<'a, T>>,
}

unsafe impl<T: ?Sized + Sync> Sync for ShardedLockWriteGuard<'_, T> {}

impl<T: ?Sized> Drop for ShardedLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        // Unlock the shards in reverse order of locking.
        for shard in self.lock.shards.iter().rev() {
            unsafe {
                let dest: *mut _ = shard.write_guard.get();
                let guard = (*dest).take();
                drop(guard);
            }
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for ShardedLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShardedLockWriteGuard")
            .field("lock", &self.lock)
            .finish()
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for ShardedLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: ?Sized> Deref for ShardedLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.value.get() }
    }
}

impl<T: ?Sized> DerefMut for ShardedLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.value.get() }
    }
}

/// Returns a `usize` that identifies the current thread.
///
/// Each thread is associated with an 'index'. While there are no particular guarantees, indices
/// usually tend to be consecutive numbers between 0 and the number of running threads.
///
/// Since this function accesses TLS, `None` might be returned if the current thread's TLS is
/// tearing down.
#[inline]
fn current_index() -> Option<usize> {
    REGISTRATION.try_with(|reg| reg.index).ok()
}

/// The global registry keeping track of registered threads and indices.
struct ThreadIndices {
    /// Mapping from `ThreadId` to thread index.
    mapping: HashMap<ThreadId, usize>,

    /// A list of free indices.
    free_list: Vec<usize>,

    /// The next index to allocate if the free list is empty.
    next_index: usize,
}

fn thread_indices() -> &'static Mutex<ThreadIndices> {
    static THREAD_INDICES: OnceLock<Mutex<ThreadIndices>> = OnceLock::new();
    fn init() -> Mutex<ThreadIndices> {
        Mutex::new(ThreadIndices {
            mapping: HashMap::new(),
            free_list: Vec::new(),
            next_index: 0,
        })
    }
    THREAD_INDICES.get_or_init(init)
}

/// A registration of a thread with an index.
///
/// When dropped, unregisters the thread and frees the reserved index.
struct Registration {
    index: usize,
    thread_id: ThreadId,
}

impl Drop for Registration {
    fn drop(&mut self) {
        let mut indices = thread_indices().lock().unwrap();
        indices.mapping.remove(&self.thread_id);
        indices.free_list.push(self.index);
    }
}

std::thread_local! {
    static REGISTRATION: Registration = {
        let thread_id = thread::current().id();
        let mut indices = thread_indices().lock().unwrap();

        let index = match indices.free_list.pop() {
            Some(i) => i,
            None => {
                let i = indices.next_index;
                indices.next_index += 1;
                i
            }
        };
        indices.mapping.insert(thread_id, index);

        Registration {
            index,
            thread_id,
        }
    };
}
