use std::borrow::Borrow;
use std::hash::Hash;
use std::num::NonZeroUsize;

use lru::LruCache;
use tokio::sync::Mutex;
use tokio::sync::MutexGuard;

/// A minimal LRU cache protected by a tokio mutex (requires tokio runtime).
pub struct BlockingLruCache<K, V> {
    inner: Mutex<LruCache<K, V>>,
}

impl<K, V> BlockingLruCache<K, V>
where
    K: Eq + Hash,
{
    /// Creates a cache with the provided non-zero capacity.
    #[must_use]
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            inner: Mutex::new(LruCache::new(capacity)),
        }
    }

    /// Returns a clone of the cached value for `key`, or computes and inserts it.
    pub fn get_or_insert_with(&self, key: K, value: impl FnOnce() -> V) -> V
    where
        V: Clone,
    {
        let mut guard = self.inner.blocking_lock();
        if let Some(v) = guard.get(&key) {
            return v.clone();
        }
        let v = value();
        // Insert and return a clone to keep ownership in the cache.
        guard.put(key, v.clone());
        v
    }

    /// Like `get_or_insert_with`, but the value factory may fail.
    pub fn get_or_try_insert_with<E>(
        &self,
        key: K,
        value: impl FnOnce() -> Result<V, E>,
    ) -> Result<V, E>
    where
        V: Clone,
    {
        let mut guard = self.inner.blocking_lock();
        if let Some(v) = guard.get(&key) {
            return Ok(v.clone());
        }
        let v = value()?;
        guard.put(key, v.clone());
        Ok(v)
    }

    /// Builds a cache if `capacity` is non-zero, returning `None` otherwise.
    #[must_use]
    pub fn try_with_capacity(capacity: usize) -> Option<Self> {
        NonZeroUsize::new(capacity).map(Self::new)
    }

    /// Returns a clone of the cached value corresponding to `key`, if present.
    pub fn get<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
        V: Clone,
    {
        self.inner.blocking_lock().get(key).cloned()
    }

    /// Inserts `value` for `key`, returning the previous entry if it existed.
    pub fn insert(&self, key: K, value: V) -> Option<V> {
        self.inner.blocking_lock().put(key, value)
    }

    /// Removes the entry for `key` if it exists, returning it.
    pub fn remove<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.blocking_lock().pop(key)
    }

    /// Clears all entries from the cache.
    pub fn clear(&self) {
        self.inner.blocking_lock().clear();
    }

    /// Executes `callback` with a mutable reference to the underlying cache.
    pub fn with_mut<R>(&self, callback: impl FnOnce(&mut LruCache<K, V>) -> R) -> R {
        let mut guard = self.inner.blocking_lock();
        callback(&mut guard)
    }

    /// Provides direct access to the cache guard for advanced use cases.
    pub fn blocking_lock(&self) -> MutexGuard<'_, LruCache<K, V>> {
        self.inner.blocking_lock()
    }
}

#[cfg(test)]
mod tests {
    use super::BlockingLruCache;
    use std::num::NonZeroUsize;

    #[test]
    fn stores_and_retrieves_values() {
        let cache = BlockingLruCache::new(NonZeroUsize::new(2).expect("capacity"));

        assert!(cache.get(&"first").is_none());
        cache.insert("first", 1);
        assert_eq!(cache.get(&"first"), Some(1));
    }

    #[test]
    fn evicts_least_recently_used() {
        let cache = BlockingLruCache::new(NonZeroUsize::new(2).expect("capacity"));
        cache.insert("a", 1);
        cache.insert("b", 2);
        assert_eq!(cache.get(&"a"), Some(1));

        cache.insert("c", 3);

        assert!(cache.get(&"b").is_none());
        assert_eq!(cache.get(&"a"), Some(1));
        assert_eq!(cache.get(&"c"), Some(3));
    }
}
