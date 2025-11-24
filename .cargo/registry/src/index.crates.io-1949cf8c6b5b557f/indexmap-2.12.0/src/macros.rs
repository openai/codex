/// Create an [`IndexMap`][crate::IndexMap] from a list of key-value pairs
/// and a [`BuildHasherDefault`][core::hash::BuildHasherDefault]-wrapped custom hasher.
///
/// ## Example
///
/// ```
/// use indexmap::indexmap_with_default;
/// use fnv::FnvHasher;
///
/// let map = indexmap_with_default!{
///     FnvHasher;
///     "a" => 1,
///     "b" => 2,
/// };
/// assert_eq!(map["a"], 1);
/// assert_eq!(map["b"], 2);
/// assert_eq!(map.get("c"), None);
///
/// // "a" is the first key
/// assert_eq!(map.keys().next(), Some(&"a"));
/// ```
#[macro_export]
macro_rules! indexmap_with_default {
    ($H:ty; $($key:expr => $value:expr,)+) => { $crate::indexmap_with_default!($H; $($key => $value),+) };
    ($H:ty; $($key:expr => $value:expr),*) => {{
        let builder = ::core::hash::BuildHasherDefault::<$H>::default();
        const CAP: usize = <[()]>::len(&[$({ stringify!($key); }),*]);
        #[allow(unused_mut)]
        // Specify your custom `H` (must implement Default + Hasher) as the hasher:
        let mut map = $crate::IndexMap::with_capacity_and_hasher(CAP, builder);
        $(
            map.insert($key, $value);
        )*
        map
    }};
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[macro_export]
/// Create an [`IndexMap`][crate::IndexMap] from a list of key-value pairs
///
/// ## Example
///
/// ```
/// use indexmap::indexmap;
///
/// let map = indexmap!{
///     "a" => 1,
///     "b" => 2,
/// };
/// assert_eq!(map["a"], 1);
/// assert_eq!(map["b"], 2);
/// assert_eq!(map.get("c"), None);
///
/// // "a" is the first key
/// assert_eq!(map.keys().next(), Some(&"a"));
/// ```
macro_rules! indexmap {
    ($($key:expr => $value:expr,)+) => { $crate::indexmap!($($key => $value),+) };
    ($($key:expr => $value:expr),*) => {
        {
            // Note: `stringify!($key)` is just here to consume the repetition,
            // but we throw away that string literal during constant evaluation.
            const CAP: usize = <[()]>::len(&[$({ stringify!($key); }),*]);
            let mut map = $crate::IndexMap::with_capacity(CAP);
            $(
                map.insert($key, $value);
            )*
            map
        }
    };
}

/// Create an [`IndexSet`][crate::IndexSet] from a list of values
/// and a [`BuildHasherDefault`][core::hash::BuildHasherDefault]-wrapped custom hasher.
///
/// ## Example
///
/// ```
/// use indexmap::indexset_with_default;
/// use fnv::FnvHasher;
///
/// let set = indexset_with_default!{
///     FnvHasher;
///     "a",
///     "b",
/// };
/// assert!(set.contains("a"));
/// assert!(set.contains("b"));
/// assert!(!set.contains("c"));
///
/// // "a" is the first value
/// assert_eq!(set.iter().next(), Some(&"a"));
/// ```
#[macro_export]
macro_rules! indexset_with_default {
    ($H:ty; $($value:expr,)+) => { $crate::indexset_with_default!($H; $($value),+) };
    ($H:ty; $($value:expr),*) => {{
        let builder = ::core::hash::BuildHasherDefault::<$H>::default();
        const CAP: usize = <[()]>::len(&[$({ stringify!($value); }),*]);
        #[allow(unused_mut)]
        // Specify your custom `H` (must implement Default + Hash) as the hasher:
        let mut set = $crate::IndexSet::with_capacity_and_hasher(CAP, builder);
        $(
            set.insert($value);
        )*
        set
    }};
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[macro_export]
/// Create an [`IndexSet`][crate::IndexSet] from a list of values
///
/// ## Example
///
/// ```
/// use indexmap::indexset;
///
/// let set = indexset!{
///     "a",
///     "b",
/// };
/// assert!(set.contains("a"));
/// assert!(set.contains("b"));
/// assert!(!set.contains("c"));
///
/// // "a" is the first value
/// assert_eq!(set.iter().next(), Some(&"a"));
/// ```
macro_rules! indexset {
    ($($value:expr,)+) => { $crate::indexset!($($value),+) };
    ($($value:expr),*) => {
        {
            // Note: `stringify!($value)` is just here to consume the repetition,
            // but we throw away that string literal during constant evaluation.
            const CAP: usize = <[()]>::len(&[$({ stringify!($value); }),*]);
            let mut set = $crate::IndexSet::with_capacity(CAP);
            $(
                set.insert($value);
            )*
            set
        }
    };
}

// generate all the Iterator methods by just forwarding to the underlying
// self.iter and mapping its element.
macro_rules! iterator_methods {
    // $map_elt is the mapping function from the underlying iterator's element
    // same mapping function for both options and iterators
    ($map_elt:expr) => {
        fn next(&mut self) -> Option<Self::Item> {
            self.iter.next().map($map_elt)
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.iter.size_hint()
        }

        fn count(self) -> usize {
            self.iter.len()
        }

        fn nth(&mut self, n: usize) -> Option<Self::Item> {
            self.iter.nth(n).map($map_elt)
        }

        fn last(mut self) -> Option<Self::Item> {
            self.next_back()
        }

        fn collect<C>(self) -> C
        where
            C: FromIterator<Self::Item>,
        {
            // NB: forwarding this directly to standard iterators will
            // allow it to leverage unstable traits like `TrustedLen`.
            self.iter.map($map_elt).collect()
        }
    };
}

macro_rules! double_ended_iterator_methods {
    // $map_elt is the mapping function from the underlying iterator's element
    // same mapping function for both options and iterators
    ($map_elt:expr) => {
        fn next_back(&mut self) -> Option<Self::Item> {
            self.iter.next_back().map($map_elt)
        }

        fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
            self.iter.nth_back(n).map($map_elt)
        }
    };
}

// generate `ParallelIterator` methods by just forwarding to the underlying
// self.entries and mapping its elements.
#[cfg(feature = "rayon")]
macro_rules! parallel_iterator_methods {
    // $map_elt is the mapping function from the underlying iterator's element
    ($map_elt:expr) => {
        fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where
            C: UnindexedConsumer<Self::Item>,
        {
            self.entries
                .into_par_iter()
                .map($map_elt)
                .drive_unindexed(consumer)
        }

        // NB: This allows indexed collection, e.g. directly into a `Vec`, but the
        // underlying iterator must really be indexed.  We should remove this if we
        // start having tombstones that must be filtered out.
        fn opt_len(&self) -> Option<usize> {
            Some(self.entries.len())
        }
    };
}

// generate `IndexedParallelIterator` methods by just forwarding to the underlying
// self.entries and mapping its elements.
#[cfg(feature = "rayon")]
macro_rules! indexed_parallel_iterator_methods {
    // $map_elt is the mapping function from the underlying iterator's element
    ($map_elt:expr) => {
        fn drive<C>(self, consumer: C) -> C::Result
        where
            C: Consumer<Self::Item>,
        {
            self.entries.into_par_iter().map($map_elt).drive(consumer)
        }

        fn len(&self) -> usize {
            self.entries.len()
        }

        fn with_producer<CB>(self, callback: CB) -> CB::Output
        where
            CB: ProducerCallback<Self::Item>,
        {
            self.entries
                .into_par_iter()
                .map($map_elt)
                .with_producer(callback)
        }
    };
}
