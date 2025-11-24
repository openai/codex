//! Minimal support for `rustc-rayon`, not intended for general use.

use crate::vec::Vec;
use crate::{Bucket, Entries, IndexMap, IndexSet};

use rustc_rayon::iter::plumbing::{Consumer, ProducerCallback, UnindexedConsumer};
use rustc_rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};

mod map {
    use super::*;

    impl<K, V, S> IntoParallelIterator for IndexMap<K, V, S>
    where
        K: Send,
        V: Send,
    {
        type Item = (K, V);
        type Iter = IntoParIter<K, V>;

        fn into_par_iter(self) -> Self::Iter {
            IntoParIter {
                entries: self.into_entries(),
            }
        }
    }

    pub struct IntoParIter<K, V> {
        entries: Vec<Bucket<K, V>>,
    }

    impl<K: Send, V: Send> ParallelIterator for IntoParIter<K, V> {
        type Item = (K, V);

        parallel_iterator_methods!(Bucket::key_value);
    }

    impl<K: Send, V: Send> IndexedParallelIterator for IntoParIter<K, V> {
        indexed_parallel_iterator_methods!(Bucket::key_value);
    }

    impl<'a, K, V, S> IntoParallelIterator for &'a IndexMap<K, V, S>
    where
        K: Sync,
        V: Sync,
    {
        type Item = (&'a K, &'a V);
        type Iter = ParIter<'a, K, V>;

        fn into_par_iter(self) -> Self::Iter {
            ParIter {
                entries: self.as_entries(),
            }
        }
    }

    pub struct ParIter<'a, K, V> {
        entries: &'a [Bucket<K, V>],
    }

    impl<'a, K: Sync, V: Sync> ParallelIterator for ParIter<'a, K, V> {
        type Item = (&'a K, &'a V);

        parallel_iterator_methods!(Bucket::refs);
    }

    impl<K: Sync, V: Sync> IndexedParallelIterator for ParIter<'_, K, V> {
        indexed_parallel_iterator_methods!(Bucket::refs);
    }

    impl<'a, K, V, S> IntoParallelIterator for &'a mut IndexMap<K, V, S>
    where
        K: Sync + Send,
        V: Send,
    {
        type Item = (&'a K, &'a mut V);
        type Iter = ParIterMut<'a, K, V>;

        fn into_par_iter(self) -> Self::Iter {
            ParIterMut {
                entries: self.as_entries_mut(),
            }
        }
    }

    pub struct ParIterMut<'a, K, V> {
        entries: &'a mut [Bucket<K, V>],
    }

    impl<'a, K: Sync + Send, V: Send> ParallelIterator for ParIterMut<'a, K, V> {
        type Item = (&'a K, &'a mut V);

        parallel_iterator_methods!(Bucket::ref_mut);
    }

    impl<K: Sync + Send, V: Send> IndexedParallelIterator for ParIterMut<'_, K, V> {
        indexed_parallel_iterator_methods!(Bucket::ref_mut);
    }
}

mod set {
    use super::*;

    impl<T, S> IntoParallelIterator for IndexSet<T, S>
    where
        T: Send,
    {
        type Item = T;
        type Iter = IntoParIter<T>;

        fn into_par_iter(self) -> Self::Iter {
            IntoParIter {
                entries: self.into_entries(),
            }
        }
    }

    pub struct IntoParIter<T> {
        entries: Vec<Bucket<T, ()>>,
    }

    impl<T: Send> ParallelIterator for IntoParIter<T> {
        type Item = T;

        parallel_iterator_methods!(Bucket::key);
    }

    impl<T: Send> IndexedParallelIterator for IntoParIter<T> {
        indexed_parallel_iterator_methods!(Bucket::key);
    }

    impl<'a, T, S> IntoParallelIterator for &'a IndexSet<T, S>
    where
        T: Sync,
    {
        type Item = &'a T;
        type Iter = ParIter<'a, T>;

        fn into_par_iter(self) -> Self::Iter {
            ParIter {
                entries: self.as_entries(),
            }
        }
    }

    pub struct ParIter<'a, T> {
        entries: &'a [Bucket<T, ()>],
    }

    impl<'a, T: Sync> ParallelIterator for ParIter<'a, T> {
        type Item = &'a T;

        parallel_iterator_methods!(Bucket::key_ref);
    }

    impl<T: Sync> IndexedParallelIterator for ParIter<'_, T> {
        indexed_parallel_iterator_methods!(Bucket::key_ref);
    }
}
