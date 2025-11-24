use super::fast::{FixedState, RandomState};

/// Type alias for [`std::collections::HashMap<K, V, foldhash::fast::RandomState>`].
pub type HashMap<K, V> = std::collections::HashMap<K, V, RandomState>;

/// Type alias for [`std::collections::HashSet<T, foldhash::fast::RandomState>`].
pub type HashSet<T> = std::collections::HashSet<T, RandomState>;

/// A convenience extension trait to enable [`HashMap::new`] for hash maps that use `foldhash`.
pub trait HashMapExt {
    /// Creates an empty `HashMap`.
    fn new() -> Self;

    /// Creates an empty `HashMap` with at least the specified capacity.
    fn with_capacity(capacity: usize) -> Self;
}

impl<K, V> HashMapExt for std::collections::HashMap<K, V, RandomState> {
    #[inline(always)]
    fn new() -> Self {
        Self::with_hasher(RandomState::default())
    }

    #[inline(always)]
    fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_hasher(capacity, RandomState::default())
    }
}

impl<K, V> HashMapExt for std::collections::HashMap<K, V, FixedState> {
    #[inline(always)]
    fn new() -> Self {
        Self::with_hasher(FixedState::default())
    }

    #[inline(always)]
    fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_hasher(capacity, FixedState::default())
    }
}

/// A convenience extension trait to enable [`HashSet::new`] for hash sets that use `foldhash`.
pub trait HashSetExt {
    /// Creates an empty `HashSet`.
    fn new() -> Self;

    /// Creates an empty `HashSet` with at least the specified capacity.
    fn with_capacity(capacity: usize) -> Self;
}

impl<T> HashSetExt for std::collections::HashSet<T, RandomState> {
    #[inline(always)]
    fn new() -> Self {
        Self::with_hasher(RandomState::default())
    }

    #[inline(always)]
    fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_hasher(capacity, RandomState::default())
    }
}

impl<T> HashSetExt for std::collections::HashSet<T, FixedState> {
    #[inline(always)]
    fn new() -> Self {
        Self::with_hasher(FixedState::default())
    }

    #[inline(always)]
    fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_hasher(capacity, FixedState::default())
    }
}
