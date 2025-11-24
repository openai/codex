use core::panic::{RefUnwindSafe, UnwindSafe};

use alloc::{boxed::Box, vec, vec::Vec};

use crate::pikevm;

// Literally the only reason that this crate requires 'std' currently.
//
// In regex-automata, we support the no-std use case by rolling our own
// spin-lock based Mutex. That's questionable on its own, but it's not clear if
// we should be doing that here. It will require introducing non-safe code in a
// crate that is otherwise safe. But maybe it's worth doing?
use std::sync::Mutex;

/// A type alias for our pool of meta::Cache that fixes the type parameters to
/// what we use for the meta regex below.
pub(crate) type CachePool = Pool<pikevm::Cache, CachePoolFn>;

/// Same as above, but for the guard returned by a pool.
pub(crate) type CachePoolGuard<'a> = PoolGuard<'a, pikevm::Cache, CachePoolFn>;

/// The type of the closure we use to create new caches. We need to spell out
/// all of the marker traits or else we risk leaking !MARKER impls.
pub(crate) type CachePoolFn =
    Box<dyn Fn() -> pikevm::Cache + Send + Sync + UnwindSafe + RefUnwindSafe>;

/// A thread safe pool utilizing alloc-only features.
///
/// Unlike the pool in regex-automata, this has no "fast path." We could add
/// it, but it's more code and requires reasoning about safety.
pub(crate) struct Pool<T, F> {
    /// A stack of T values to hand out. These are used when a Pool is
    /// accessed by a thread that didn't create it.
    stack: Mutex<Vec<Box<T>>>,
    /// A function to create more T values when stack is empty and a caller
    /// has requested a T.
    create: F,
}

// If T is UnwindSafe, then since we provide exclusive access to any
// particular value in the pool, it should therefore also be considered
// RefUnwindSafe.
impl<T: UnwindSafe, F: UnwindSafe> RefUnwindSafe for Pool<T, F> {}

impl<T, F> Pool<T, F> {
    /// Create a new pool. The given closure is used to create values in
    /// the pool when necessary.
    pub(crate) const fn new(create: F) -> Pool<T, F> {
        Pool { stack: Mutex::new(vec![]), create }
    }
}

impl<T: Send, F: Fn() -> T> Pool<T, F> {
    /// Get a value from the pool. This may block if another thread is also
    /// attempting to retrieve a value from the pool.
    pub(crate) fn get(&self) -> PoolGuard<'_, T, F> {
        let mut stack = self.stack.lock().unwrap();
        let value = match stack.pop() {
            None => Box::new((self.create)()),
            Some(value) => value,
        };
        PoolGuard { pool: self, value: Some(value) }
    }

    /// Puts a value back into the pool. Callers don't need to call this.
    /// Once the guard that's returned by 'get' is dropped, it is put back
    /// into the pool automatically.
    fn put_value(&self, value: Box<T>) {
        let mut stack = self.stack.lock().unwrap();
        stack.push(value);
    }
}

impl<T: core::fmt::Debug, F> core::fmt::Debug for Pool<T, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Pool").field("stack", &self.stack).finish()
    }
}

/// A guard that is returned when a caller requests a value from the pool.
pub(crate) struct PoolGuard<'a, T: Send, F: Fn() -> T> {
    /// The pool that this guard is attached to.
    pool: &'a Pool<T, F>,
    /// This is None after the guard has been put back into the pool.
    value: Option<Box<T>>,
}

impl<'a, T: Send, F: Fn() -> T> Drop for PoolGuard<'a, T, F> {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            self.pool.put_value(value);
        }
    }
}

impl<'a, T: Send, F: Fn() -> T> core::ops::Deref for PoolGuard<'a, T, F> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value.as_deref().unwrap()
    }
}

impl<'a, T: Send, F: Fn() -> T> core::ops::DerefMut for PoolGuard<'a, T, F> {
    fn deref_mut(&mut self) -> &mut T {
        self.value.as_deref_mut().unwrap()
    }
}

impl<'a, T: Send + core::fmt::Debug, F: Fn() -> T> core::fmt::Debug
    for PoolGuard<'a, T, F>
{
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("PoolGuard")
            .field("pool", &self.pool)
            .field("value", &self.value)
            .finish()
    }
}
