//! Rayon extensions for `HashMap`.

use super::raw::{RawIntoParIter, RawParDrain, RawParIter};
use crate::hash_map::HashMap;
use crate::raw::{Allocator, Global};
use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::marker::PhantomData;
use rayon::iter::plumbing::UnindexedConsumer;
use rayon::iter::{FromParallelIterator, IntoParallelIterator, ParallelExtend, ParallelIterator};

/// Parallel iterator over shared references to entries in a map.
///
/// This iterator is created by the [`par_iter`] method on [`HashMap`]
/// (provided by the [`IntoParallelRefIterator`] trait).
/// See its documentation for more.
///
/// [`par_iter`]: /hashbrown/struct.HashMap.html#method.par_iter
/// [`HashMap`]: /hashbrown/struct.HashMap.html
/// [`IntoParallelRefIterator`]: https://docs.rs/rayon/1.0/rayon/iter/trait.IntoParallelRefIterator.html
pub struct ParIter<'a, K, V> {
    inner: RawParIter<(K, V)>,
    marker: PhantomData<(&'a K, &'a V)>,
}

impl<'a, K: Sync, V: Sync> ParallelIterator for ParIter<'a, K, V> {
    type Item = (&'a K, &'a V);

    #[cfg_attr(feature = "inline-more", inline)]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.inner
            .map(|x| unsafe {
                let r = x.as_ref();
                (&r.0, &r.1)
            })
            .drive_unindexed(consumer)
    }
}

impl<K, V> Clone for ParIter<'_, K, V> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}

impl<K: fmt::Debug + Eq + Hash, V: fmt::Debug> fmt::Debug for ParIter<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = unsafe { self.inner.iter() }.map(|x| unsafe {
            let r = x.as_ref();
            (&r.0, &r.1)
        });
        f.debug_list().entries(iter).finish()
    }
}

/// Parallel iterator over shared references to keys in a map.
///
/// This iterator is created by the [`par_keys`] method on [`HashMap`].
/// See its documentation for more.
///
/// [`par_keys`]: /hashbrown/struct.HashMap.html#method.par_keys
/// [`HashMap`]: /hashbrown/struct.HashMap.html
pub struct ParKeys<'a, K, V> {
    inner: RawParIter<(K, V)>,
    marker: PhantomData<(&'a K, &'a V)>,
}

impl<'a, K: Sync, V: Sync> ParallelIterator for ParKeys<'a, K, V> {
    type Item = &'a K;

    #[cfg_attr(feature = "inline-more", inline)]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.inner
            .map(|x| unsafe { &x.as_ref().0 })
            .drive_unindexed(consumer)
    }
}

impl<K, V> Clone for ParKeys<'_, K, V> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}

impl<K: fmt::Debug + Eq + Hash, V> fmt::Debug for ParKeys<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = unsafe { self.inner.iter() }.map(|x| unsafe { &x.as_ref().0 });
        f.debug_list().entries(iter).finish()
    }
}

/// Parallel iterator over shared references to values in a map.
///
/// This iterator is created by the [`par_values`] method on [`HashMap`].
/// See its documentation for more.
///
/// [`par_values`]: /hashbrown/struct.HashMap.html#method.par_values
/// [`HashMap`]: /hashbrown/struct.HashMap.html
pub struct ParValues<'a, K, V> {
    inner: RawParIter<(K, V)>,
    marker: PhantomData<(&'a K, &'a V)>,
}

impl<'a, K: Sync, V: Sync> ParallelIterator for ParValues<'a, K, V> {
    type Item = &'a V;

    #[cfg_attr(feature = "inline-more", inline)]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.inner
            .map(|x| unsafe { &x.as_ref().1 })
            .drive_unindexed(consumer)
    }
}

impl<K, V> Clone for ParValues<'_, K, V> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}

impl<K: Eq + Hash, V: fmt::Debug> fmt::Debug for ParValues<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = unsafe { self.inner.iter() }.map(|x| unsafe { &x.as_ref().1 });
        f.debug_list().entries(iter).finish()
    }
}

/// Parallel iterator over mutable references to entries in a map.
///
/// This iterator is created by the [`par_iter_mut`] method on [`HashMap`]
/// (provided by the [`IntoParallelRefMutIterator`] trait).
/// See its documentation for more.
///
/// [`par_iter_mut`]: /hashbrown/struct.HashMap.html#method.par_iter_mut
/// [`HashMap`]: /hashbrown/struct.HashMap.html
/// [`IntoParallelRefMutIterator`]: https://docs.rs/rayon/1.0/rayon/iter/trait.IntoParallelRefMutIterator.html
pub struct ParIterMut<'a, K, V> {
    inner: RawParIter<(K, V)>,
    marker: PhantomData<(&'a K, &'a mut V)>,
}

impl<'a, K: Sync, V: Send> ParallelIterator for ParIterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    #[cfg_attr(feature = "inline-more", inline)]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.inner
            .map(|x| unsafe {
                let r = x.as_mut();
                (&r.0, &mut r.1)
            })
            .drive_unindexed(consumer)
    }
}

impl<K: fmt::Debug + Eq + Hash, V: fmt::Debug> fmt::Debug for ParIterMut<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ParIter {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
        .fmt(f)
    }
}

/// Parallel iterator over mutable references to values in a map.
///
/// This iterator is created by the [`par_values_mut`] method on [`HashMap`].
/// See its documentation for more.
///
/// [`par_values_mut`]: /hashbrown/struct.HashMap.html#method.par_values_mut
/// [`HashMap`]: /hashbrown/struct.HashMap.html
pub struct ParValuesMut<'a, K, V> {
    inner: RawParIter<(K, V)>,
    marker: PhantomData<(&'a K, &'a mut V)>,
}

impl<'a, K: Sync, V: Send> ParallelIterator for ParValuesMut<'a, K, V> {
    type Item = &'a mut V;

    #[cfg_attr(feature = "inline-more", inline)]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.inner
            .map(|x| unsafe { &mut x.as_mut().1 })
            .drive_unindexed(consumer)
    }
}

impl<K: Eq + Hash, V: fmt::Debug> fmt::Debug for ParValuesMut<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ParValues {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
        .fmt(f)
    }
}

/// Parallel iterator over entries of a consumed map.
///
/// This iterator is created by the [`into_par_iter`] method on [`HashMap`]
/// (provided by the [`IntoParallelIterator`] trait).
/// See its documentation for more.
///
/// [`into_par_iter`]: /hashbrown/struct.HashMap.html#method.into_par_iter
/// [`HashMap`]: /hashbrown/struct.HashMap.html
/// [`IntoParallelIterator`]: https://docs.rs/rayon/1.0/rayon/iter/trait.IntoParallelIterator.html
pub struct IntoParIter<K, V, A: Allocator + Clone = Global> {
    inner: RawIntoParIter<(K, V), A>,
}

impl<K: Send, V: Send, A: Allocator + Clone + Send> ParallelIterator for IntoParIter<K, V, A> {
    type Item = (K, V);

    #[cfg_attr(feature = "inline-more", inline)]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.inner.drive_unindexed(consumer)
    }
}

impl<K: fmt::Debug + Eq + Hash, V: fmt::Debug, A: Allocator + Clone> fmt::Debug
    for IntoParIter<K, V, A>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ParIter {
            inner: unsafe { self.inner.par_iter() },
            marker: PhantomData,
        }
        .fmt(f)
    }
}

/// Parallel draining iterator over entries of a map.
///
/// This iterator is created by the [`par_drain`] method on [`HashMap`].
/// See its documentation for more.
///
/// [`par_drain`]: /hashbrown/struct.HashMap.html#method.par_drain
/// [`HashMap`]: /hashbrown/struct.HashMap.html
pub struct ParDrain<'a, K, V, A: Allocator + Clone = Global> {
    inner: RawParDrain<'a, (K, V), A>,
}

impl<K: Send, V: Send, A: Allocator + Clone + Sync> ParallelIterator for ParDrain<'_, K, V, A> {
    type Item = (K, V);

    #[cfg_attr(feature = "inline-more", inline)]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.inner.drive_unindexed(consumer)
    }
}

impl<K: fmt::Debug + Eq + Hash, V: fmt::Debug, A: Allocator + Clone> fmt::Debug
    for ParDrain<'_, K, V, A>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ParIter {
            inner: unsafe { self.inner.par_iter() },
            marker: PhantomData,
        }
        .fmt(f)
    }
}

impl<K: Sync, V: Sync, S, A: Allocator + Clone> HashMap<K, V, S, A> {
    /// Visits (potentially in parallel) immutably borrowed keys in an arbitrary order.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn par_keys(&self) -> ParKeys<'_, K, V> {
        ParKeys {
            inner: unsafe { self.table.par_iter() },
            marker: PhantomData,
        }
    }

    /// Visits (potentially in parallel) immutably borrowed values in an arbitrary order.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn par_values(&self) -> ParValues<'_, K, V> {
        ParValues {
            inner: unsafe { self.table.par_iter() },
            marker: PhantomData,
        }
    }
}

impl<K: Send, V: Send, S, A: Allocator + Clone> HashMap<K, V, S, A> {
    /// Visits (potentially in parallel) mutably borrowed values in an arbitrary order.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn par_values_mut(&mut self) -> ParValuesMut<'_, K, V> {
        ParValuesMut {
            inner: unsafe { self.table.par_iter() },
            marker: PhantomData,
        }
    }

    /// Consumes (potentially in parallel) all values in an arbitrary order,
    /// while preserving the map's allocated memory for reuse.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn par_drain(&mut self) -> ParDrain<'_, K, V, A> {
        ParDrain {
            inner: self.table.par_drain(),
        }
    }
}

impl<K, V, S, A> HashMap<K, V, S, A>
where
    K: Eq + Hash + Sync,
    V: PartialEq + Sync,
    S: BuildHasher + Sync,
    A: Allocator + Clone + Sync,
{
    /// Returns `true` if the map is equal to another,
    /// i.e. both maps contain the same keys mapped to the same values.
    ///
    /// This method runs in a potentially parallel fashion.
    pub fn par_eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .into_par_iter()
                .all(|(key, value)| other.get(key).map_or(false, |v| *value == *v))
    }
}

impl<K: Send, V: Send, S, A: Allocator + Clone + Send> IntoParallelIterator
    for HashMap<K, V, S, A>
{
    type Item = (K, V);
    type Iter = IntoParIter<K, V, A>;

    #[cfg_attr(feature = "inline-more", inline)]
    fn into_par_iter(self) -> Self::Iter {
        IntoParIter {
            inner: self.table.into_par_iter(),
        }
    }
}

impl<'a, K: Sync, V: Sync, S, A: Allocator + Clone> IntoParallelIterator
    for &'a HashMap<K, V, S, A>
{
    type Item = (&'a K, &'a V);
    type Iter = ParIter<'a, K, V>;

    #[cfg_attr(feature = "inline-more", inline)]
    fn into_par_iter(self) -> Self::Iter {
        ParIter {
            inner: unsafe { self.table.par_iter() },
            marker: PhantomData,
        }
    }
}

impl<'a, K: Sync, V: Send, S, A: Allocator + Clone> IntoParallelIterator
    for &'a mut HashMap<K, V, S, A>
{
    type Item = (&'a K, &'a mut V);
    type Iter = ParIterMut<'a, K, V>;

    #[cfg_attr(feature = "inline-more", inline)]
    fn into_par_iter(self) -> Self::Iter {
        ParIterMut {
            inner: unsafe { self.table.par_iter() },
            marker: PhantomData,
        }
    }
}

/// Collect (key, value) pairs from a parallel iterator into a
/// hashmap. If multiple pairs correspond to the same key, then the
/// ones produced earlier in the parallel iterator will be
/// overwritten, just as with a sequential iterator.
impl<K, V, S> FromParallelIterator<(K, V)> for HashMap<K, V, S, Global>
where
    K: Eq + Hash + Send,
    V: Send,
    S: BuildHasher + Default,
{
    fn from_par_iter<P>(par_iter: P) -> Self
    where
        P: IntoParallelIterator<Item = (K, V)>,
    {
        let mut map = HashMap::default();
        map.par_extend(par_iter);
        map
    }
}

/// Extend a hash map with items from a parallel iterator.
impl<K, V, S, A> ParallelExtend<(K, V)> for HashMap<K, V, S, A>
where
    K: Eq + Hash + Send,
    V: Send,
    S: BuildHasher,
    A: Allocator + Clone,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = (K, V)>,
    {
        extend(self, par_iter);
    }
}

/// Extend a hash map with copied items from a parallel iterator.
impl<'a, K, V, S, A> ParallelExtend<(&'a K, &'a V)> for HashMap<K, V, S, A>
where
    K: Copy + Eq + Hash + Sync,
    V: Copy + Sync,
    S: BuildHasher,
    A: Allocator + Clone,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = (&'a K, &'a V)>,
    {
        extend(self, par_iter);
    }
}

// This is equal to the normal `HashMap` -- no custom advantage.
fn extend<K, V, S, A, I>(map: &mut HashMap<K, V, S, A>, par_iter: I)
where
    K: Eq + Hash,
    S: BuildHasher,
    I: IntoParallelIterator,
    A: Allocator + Clone,
    HashMap<K, V, S, A>: Extend<I::Item>,
{
    let (list, len) = super::helpers::collect(par_iter);

    // Keys may be already present or show multiple times in the iterator.
    // Reserve the entire length if the map is empty.
    // Otherwise reserve half the length (rounded up), so the map
    // will only resize twice in the worst case.
    let reserve = if map.is_empty() { len } else { (len + 1) / 2 };
    map.reserve(reserve);
    for vec in list {
        map.extend(vec);
    }
}

#[cfg(test)]
mod test_par_map {
    use alloc::vec::Vec;
    use core::hash::{Hash, Hasher};
    use core::sync::atomic::{AtomicUsize, Ordering};

    use rayon::prelude::*;

    use crate::hash_map::HashMap;

    struct Dropable<'a> {
        k: usize,
        counter: &'a AtomicUsize,
    }

    impl Dropable<'_> {
        fn new(k: usize, counter: &AtomicUsize) -> Dropable<'_> {
            counter.fetch_add(1, Ordering::Relaxed);

            Dropable { k, counter }
        }
    }

    impl Drop for Dropable<'_> {
        fn drop(&mut self) {
            self.counter.fetch_sub(1, Ordering::Relaxed);
        }
    }

    impl Clone for Dropable<'_> {
        fn clone(&self) -> Self {
            Dropable::new(self.k, self.counter)
        }
    }

    impl Hash for Dropable<'_> {
        fn hash<H>(&self, state: &mut H)
        where
            H: Hasher,
        {
            self.k.hash(state);
        }
    }

    impl PartialEq for Dropable<'_> {
        fn eq(&self, other: &Self) -> bool {
            self.k == other.k
        }
    }

    impl Eq for Dropable<'_> {}

    #[test]
    fn test_into_iter_drops() {
        let key = AtomicUsize::new(0);
        let value = AtomicUsize::new(0);

        let hm = {
            let mut hm = HashMap::new();

            assert_eq!(key.load(Ordering::Relaxed), 0);
            assert_eq!(value.load(Ordering::Relaxed), 0);

            for i in 0..100 {
                let d1 = Dropable::new(i, &key);
                let d2 = Dropable::new(i + 100, &value);
                hm.insert(d1, d2);
            }

            assert_eq!(key.load(Ordering::Relaxed), 100);
            assert_eq!(value.load(Ordering::Relaxed), 100);

            hm
        };

        // By the way, ensure that cloning doesn't screw up the dropping.
        drop(hm.clone());

        assert_eq!(key.load(Ordering::Relaxed), 100);
        assert_eq!(value.load(Ordering::Relaxed), 100);

        // Ensure that dropping the iterator does not leak anything.
        drop(hm.clone().into_par_iter());

        {
            assert_eq!(key.load(Ordering::Relaxed), 100);
            assert_eq!(value.load(Ordering::Relaxed), 100);

            // retain only half
            let _v: Vec<_> = hm
                .into_par_iter()
                .filter(|&(ref key, _)| key.k < 50)
                .collect();

            assert_eq!(key.load(Ordering::Relaxed), 50);
            assert_eq!(value.load(Ordering::Relaxed), 50);
        };

        assert_eq!(key.load(Ordering::Relaxed), 0);
        assert_eq!(value.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_drain_drops() {
        let key = AtomicUsize::new(0);
        let value = AtomicUsize::new(0);

        let mut hm = {
            let mut hm = HashMap::new();

            assert_eq!(key.load(Ordering::Relaxed), 0);
            assert_eq!(value.load(Ordering::Relaxed), 0);

            for i in 0..100 {
                let d1 = Dropable::new(i, &key);
                let d2 = Dropable::new(i + 100, &value);
                hm.insert(d1, d2);
            }

            assert_eq!(key.load(Ordering::Relaxed), 100);
            assert_eq!(value.load(Ordering::Relaxed), 100);

            hm
        };

        // By the way, ensure that cloning doesn't screw up the dropping.
        drop(hm.clone());

        assert_eq!(key.load(Ordering::Relaxed), 100);
        assert_eq!(value.load(Ordering::Relaxed), 100);

        // Ensure that dropping the drain iterator does not leak anything.
        drop(hm.clone().par_drain());

        {
            assert_eq!(key.load(Ordering::Relaxed), 100);
            assert_eq!(value.load(Ordering::Relaxed), 100);

            // retain only half
            let _v: Vec<_> = hm.drain().filter(|&(ref key, _)| key.k < 50).collect();
            assert!(hm.is_empty());

            assert_eq!(key.load(Ordering::Relaxed), 50);
            assert_eq!(value.load(Ordering::Relaxed), 50);
        };

        assert_eq!(key.load(Ordering::Relaxed), 0);
        assert_eq!(value.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_empty_iter() {
        let mut m: HashMap<isize, bool> = HashMap::new();
        assert_eq!(m.par_drain().count(), 0);
        assert_eq!(m.par_keys().count(), 0);
        assert_eq!(m.par_values().count(), 0);
        assert_eq!(m.par_values_mut().count(), 0);
        assert_eq!(m.par_iter().count(), 0);
        assert_eq!(m.par_iter_mut().count(), 0);
        assert_eq!(m.len(), 0);
        assert!(m.is_empty());
        assert_eq!(m.into_par_iter().count(), 0);
    }

    #[test]
    fn test_iterate() {
        let mut m = HashMap::with_capacity(4);
        for i in 0..32 {
            assert!(m.insert(i, i * 2).is_none());
        }
        assert_eq!(m.len(), 32);

        let observed = AtomicUsize::new(0);

        m.par_iter().for_each(|(k, v)| {
            assert_eq!(*v, *k * 2);
            observed.fetch_or(1 << *k, Ordering::Relaxed);
        });
        assert_eq!(observed.into_inner(), 0xFFFF_FFFF);
    }

    #[test]
    fn test_keys() {
        let vec = vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let map: HashMap<_, _> = vec.into_par_iter().collect();
        let keys: Vec<_> = map.par_keys().cloned().collect();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&1));
        assert!(keys.contains(&2));
        assert!(keys.contains(&3));
    }

    #[test]
    fn test_values() {
        let vec = vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let map: HashMap<_, _> = vec.into_par_iter().collect();
        let values: Vec<_> = map.par_values().cloned().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&'a'));
        assert!(values.contains(&'b'));
        assert!(values.contains(&'c'));
    }

    #[test]
    fn test_values_mut() {
        let vec = vec![(1, 1), (2, 2), (3, 3)];
        let mut map: HashMap<_, _> = vec.into_par_iter().collect();
        map.par_values_mut().for_each(|value| *value *= 2);
        let values: Vec<_> = map.par_values().cloned().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&2));
        assert!(values.contains(&4));
        assert!(values.contains(&6));
    }

    #[test]
    fn test_eq() {
        let mut m1 = HashMap::new();
        m1.insert(1, 2);
        m1.insert(2, 3);
        m1.insert(3, 4);

        let mut m2 = HashMap::new();
        m2.insert(1, 2);
        m2.insert(2, 3);

        assert!(!m1.par_eq(&m2));

        m2.insert(3, 4);

        assert!(m1.par_eq(&m2));
    }

    #[test]
    fn test_from_iter() {
        let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        let map: HashMap<_, _> = xs.par_iter().cloned().collect();

        for &(k, v) in &xs {
            assert_eq!(map.get(&k), Some(&v));
        }
    }

    #[test]
    fn test_extend_ref() {
        let mut a = HashMap::new();
        a.insert(1, "one");
        let mut b = HashMap::new();
        b.insert(2, "two");
        b.insert(3, "three");

        a.par_extend(&b);

        assert_eq!(a.len(), 3);
        assert_eq!(a[&1], "one");
        assert_eq!(a[&2], "two");
        assert_eq!(a[&3], "three");
    }
}
