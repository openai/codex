//! Rayon extensions for `HashTable`.

use super::raw::{RawIntoParIter, RawParDrain, RawParIter};
use crate::hash_table::HashTable;
use crate::raw::{Allocator, Global};
use core::fmt;
use core::marker::PhantomData;
use rayon::iter::plumbing::UnindexedConsumer;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

/// Parallel iterator over shared references to entries in a map.
///
/// This iterator is created by the [`par_iter`] method on [`HashTable`]
/// (provided by the [`IntoParallelRefIterator`] trait).
/// See its documentation for more.
///
/// [`par_iter`]: /hashbrown/struct.HashTable.html#method.par_iter
/// [`HashTable`]: /hashbrown/struct.HashTable.html
/// [`IntoParallelRefIterator`]: https://docs.rs/rayon/1.0/rayon/iter/trait.IntoParallelRefIterator.html
pub struct ParIter<'a, T> {
    inner: RawParIter<T>,
    marker: PhantomData<&'a T>,
}

impl<'a, T: Sync> ParallelIterator for ParIter<'a, T> {
    type Item = &'a T;

    #[cfg_attr(feature = "inline-more", inline)]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.inner
            .map(|x| unsafe { x.as_ref() })
            .drive_unindexed(consumer)
    }
}

impl<T> Clone for ParIter<'_, T> {
    #[cfg_attr(feature = "inline-more", inline)]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for ParIter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = unsafe { self.inner.iter() }.map(|x| unsafe { x.as_ref() });
        f.debug_list().entries(iter).finish()
    }
}

/// Parallel iterator over mutable references to entries in a map.
///
/// This iterator is created by the [`par_iter_mut`] method on [`HashTable`]
/// (provided by the [`IntoParallelRefMutIterator`] trait).
/// See its documentation for more.
///
/// [`par_iter_mut`]: /hashbrown/struct.HashTable.html#method.par_iter_mut
/// [`HashTable`]: /hashbrown/struct.HashTable.html
/// [`IntoParallelRefMutIterator`]: https://docs.rs/rayon/1.0/rayon/iter/trait.IntoParallelRefMutIterator.html
pub struct ParIterMut<'a, T> {
    inner: RawParIter<T>,
    marker: PhantomData<&'a mut T>,
}

impl<'a, T: Send> ParallelIterator for ParIterMut<'a, T> {
    type Item = &'a mut T;

    #[cfg_attr(feature = "inline-more", inline)]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.inner
            .map(|x| unsafe { x.as_mut() })
            .drive_unindexed(consumer)
    }
}

impl<T: fmt::Debug> fmt::Debug for ParIterMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ParIter {
            inner: self.inner.clone(),
            marker: PhantomData,
        }
        .fmt(f)
    }
}

/// Parallel iterator over entries of a consumed map.
///
/// This iterator is created by the [`into_par_iter`] method on [`HashTable`]
/// (provided by the [`IntoParallelIterator`] trait).
/// See its documentation for more.
///
/// [`into_par_iter`]: /hashbrown/struct.HashTable.html#method.into_par_iter
/// [`HashTable`]: /hashbrown/struct.HashTable.html
/// [`IntoParallelIterator`]: https://docs.rs/rayon/1.0/rayon/iter/trait.IntoParallelIterator.html
pub struct IntoParIter<T, A: Allocator = Global> {
    inner: RawIntoParIter<T, A>,
}

impl<T: Send, A: Allocator + Send> ParallelIterator for IntoParIter<T, A> {
    type Item = T;

    #[cfg_attr(feature = "inline-more", inline)]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.inner.drive_unindexed(consumer)
    }
}

impl<T: fmt::Debug, A: Allocator> fmt::Debug for IntoParIter<T, A> {
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
/// This iterator is created by the [`par_drain`] method on [`HashTable`].
/// See its documentation for more.
///
/// [`par_drain`]: /hashbrown/struct.HashTable.html#method.par_drain
/// [`HashTable`]: /hashbrown/struct.HashTable.html
pub struct ParDrain<'a, T, A: Allocator = Global> {
    inner: RawParDrain<'a, T, A>,
}

impl<T: Send, A: Allocator + Sync> ParallelIterator for ParDrain<'_, T, A> {
    type Item = T;

    #[cfg_attr(feature = "inline-more", inline)]
    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.inner.drive_unindexed(consumer)
    }
}

impl<T: fmt::Debug, A: Allocator> fmt::Debug for ParDrain<'_, T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ParIter {
            inner: unsafe { self.inner.par_iter() },
            marker: PhantomData,
        }
        .fmt(f)
    }
}

impl<T: Send, A: Allocator> HashTable<T, A> {
    /// Consumes (potentially in parallel) all values in an arbitrary order,
    /// while preserving the map's allocated memory for reuse.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn par_drain(&mut self) -> ParDrain<'_, T, A> {
        ParDrain {
            inner: self.raw.par_drain(),
        }
    }
}

impl<T: Send, A: Allocator + Send> IntoParallelIterator for HashTable<T, A> {
    type Item = T;
    type Iter = IntoParIter<T, A>;

    #[cfg_attr(feature = "inline-more", inline)]
    fn into_par_iter(self) -> Self::Iter {
        IntoParIter {
            inner: self.raw.into_par_iter(),
        }
    }
}

impl<'a, T: Sync, A: Allocator> IntoParallelIterator for &'a HashTable<T, A> {
    type Item = &'a T;
    type Iter = ParIter<'a, T>;

    #[cfg_attr(feature = "inline-more", inline)]
    fn into_par_iter(self) -> Self::Iter {
        ParIter {
            inner: unsafe { self.raw.par_iter() },
            marker: PhantomData,
        }
    }
}

impl<'a, T: Send, A: Allocator> IntoParallelIterator for &'a mut HashTable<T, A> {
    type Item = &'a mut T;
    type Iter = ParIterMut<'a, T>;

    #[cfg_attr(feature = "inline-more", inline)]
    fn into_par_iter(self) -> Self::Iter {
        ParIterMut {
            inner: unsafe { self.raw.par_iter() },
            marker: PhantomData,
        }
    }
}

#[cfg(test)]
mod test_par_table {
    use alloc::vec::Vec;
    use core::sync::atomic::{AtomicUsize, Ordering};

    use rayon::prelude::*;

    use crate::{
        hash_map::{make_hash, DefaultHashBuilder},
        hash_table::HashTable,
    };

    #[test]
    fn test_iterate() {
        let hasher = DefaultHashBuilder::default();
        let mut a = HashTable::new();
        for i in 0..32 {
            a.insert_unique(make_hash(&hasher, &i), i, |x| make_hash(&hasher, x));
        }
        let observed = AtomicUsize::new(0);
        a.par_iter().for_each(|k| {
            observed.fetch_or(1 << *k, Ordering::Relaxed);
        });
        assert_eq!(observed.into_inner(), 0xFFFF_FFFF);
    }

    #[test]
    fn test_move_iter() {
        let hasher = DefaultHashBuilder::default();
        let hs = {
            let mut hs = HashTable::new();

            hs.insert_unique(make_hash(&hasher, &'a'), 'a', |x| make_hash(&hasher, x));
            hs.insert_unique(make_hash(&hasher, &'b'), 'b', |x| make_hash(&hasher, x));

            hs
        };

        let v = hs.into_par_iter().collect::<Vec<char>>();
        assert!(v == ['a', 'b'] || v == ['b', 'a']);
    }
}
