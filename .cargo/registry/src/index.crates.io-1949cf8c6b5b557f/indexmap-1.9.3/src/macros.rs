#[cfg(has_std)]
#[macro_export]
/// Create an `IndexMap` from a list of key-value pairs
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
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$($crate::indexmap!(@single $rest)),*]));

    ($($key:expr => $value:expr,)+) => { $crate::indexmap!($($key => $value),+) };
    ($($key:expr => $value:expr),*) => {
        {
            let _cap = $crate::indexmap!(@count $($key),*);
            let mut _map = $crate::IndexMap::with_capacity(_cap);
            $(
                _map.insert($key, $value);
            )*
            _map
        }
    };
}

#[cfg(has_std)]
#[macro_export]
/// Create an `IndexSet` from a list of values
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
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$($crate::indexset!(@single $rest)),*]));

    ($($value:expr,)+) => { $crate::indexset!($($value),+) };
    ($($value:expr),*) => {
        {
            let _cap = $crate::indexset!(@count $($value),*);
            let mut _set = $crate::IndexSet::with_capacity(_cap);
            $(
                _set.insert($value);
            )*
            _set
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
#[cfg(any(feature = "rayon", feature = "rustc-rayon"))]
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
#[cfg(any(feature = "rayon", feature = "rustc-rayon"))]
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
