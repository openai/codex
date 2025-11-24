use std::hash::Hash;

mod private {
    use std::collections::HashMap;
    use std::hash::Hash;
    use std::fmt;

    #[derive(Clone)]
    #[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
    pub struct DuplicatesBy<I: Iterator, Key, F> {
        pub(crate) iter: I,
        pub(crate) meta: Meta<Key, F>,
    }

    impl<I, V, F> fmt::Debug for DuplicatesBy<I, V, F>
    where
        I: Iterator + fmt::Debug,
        V: fmt::Debug + Hash + Eq,
    {
        debug_fmt_fields!(DuplicatesBy, iter, meta.used);
    }

    impl<I: Iterator, Key: Eq + Hash, F> DuplicatesBy<I, Key, F> {
        pub(crate) fn new(iter: I, key_method: F) -> Self {
            DuplicatesBy {
                iter,
                meta: Meta {
                    used: HashMap::new(),
                    pending: 0,
                    key_method,
                },
            }
        }
    }

    #[derive(Clone)]
    pub struct Meta<Key, F> {
        used: HashMap<Key, bool>,
        pending: usize,
        key_method: F,
    }

    impl<Key, F> Meta<Key, F>
    where
        Key: Eq + Hash,
    {
        /// Takes an item and returns it back to the caller if it's the second time we see it.
        /// Otherwise the item is consumed and None is returned
        #[inline(always)]
        fn filter<I>(&mut self, item: I) -> Option<I>
        where
            F: KeyMethod<Key, I>,
        {
            let kv = self.key_method.make(item);
            match self.used.get_mut(kv.key_ref()) {
                None => {
                    self.used.insert(kv.key(), false);
                    self.pending += 1;
                    None
                }
                Some(true) => None,
                Some(produced) => {
                    *produced = true;
                    self.pending -= 1;
                    Some(kv.value())
                }
            }
        }
    }

    impl<I, Key, F> Iterator for DuplicatesBy<I, Key, F>
    where
        I: Iterator,
        Key: Eq + Hash,
        F: KeyMethod<Key, I::Item>,
    {
        type Item = I::Item;

        fn next(&mut self) -> Option<Self::Item> {
            let DuplicatesBy { iter, meta } = self;
            iter.find_map(|v| meta.filter(v))
        }

        #[inline]
        fn size_hint(&self) -> (usize, Option<usize>) {
            let (_, hi) = self.iter.size_hint();
            let hi = hi.map(|hi| {
                if hi <= self.meta.pending {
                    // fewer or equally many iter-remaining elements than pending elements
                    // => at most, each iter-remaining element is matched
                    hi
                } else {
                    // fewer pending elements than iter-remaining elements
                    // => at most:
                    //    * each pending element is matched
                    //    * the other iter-remaining elements come in pairs
                    self.meta.pending + (hi - self.meta.pending) / 2
                }
            });
            // The lower bound is always 0 since we might only get unique items from now on
            (0, hi)
        }
    }

    impl<I, Key, F> DoubleEndedIterator for DuplicatesBy<I, Key, F>
    where
        I: DoubleEndedIterator,
        Key: Eq + Hash,
        F: KeyMethod<Key, I::Item>,
    {
        fn next_back(&mut self) -> Option<Self::Item> {
            let DuplicatesBy { iter, meta } = self;
            iter.rev().find_map(|v| meta.filter(v))
        }
    }

    /// A keying method for use with `DuplicatesBy`
    pub trait KeyMethod<K, V> {
        type Container: KeyXorValue<K, V>;

        fn make(&mut self, value: V) -> Self::Container;
    }

    /// Apply the identity function to elements before checking them for equality.
    #[derive(Debug)]
    pub struct ById;
    impl<V> KeyMethod<V, V> for ById {
        type Container = JustValue<V>;

        fn make(&mut self, v: V) -> Self::Container {
            JustValue(v)
        }
    }

    /// Apply a user-supplied function to elements before checking them for equality.
    pub struct ByFn<F>(pub(crate) F);
    impl<F> fmt::Debug for ByFn<F> {
        debug_fmt_fields!(ByFn,);
    }
    impl<K, V, F> KeyMethod<K, V> for ByFn<F>
    where
        F: FnMut(&V) -> K,
    {
        type Container = KeyValue<K, V>;

        fn make(&mut self, v: V) -> Self::Container {
            KeyValue((self.0)(&v), v)
        }
    }

    // Implementors of this trait can hold onto a key and a value but only give access to one of them
    // at a time. This allows the key and the value to be the same value internally
    pub trait KeyXorValue<K, V> {
        fn key_ref(&self) -> &K;
        fn key(self) -> K;
        fn value(self) -> V;
    }

    #[derive(Debug)]
    pub struct KeyValue<K, V>(K, V);
    impl<K, V> KeyXorValue<K, V> for KeyValue<K, V> {
        fn key_ref(&self) -> &K {
            &self.0
        }
        fn key(self) -> K {
            self.0
        }
        fn value(self) -> V {
            self.1
        }
    }

    #[derive(Debug)]
    pub struct JustValue<V>(V);
    impl<V> KeyXorValue<V, V> for JustValue<V> {
        fn key_ref(&self) -> &V {
            &self.0
        }
        fn key(self) -> V {
            self.0
        }
        fn value(self) -> V {
            self.0
        }
    }
}

/// An iterator adapter to filter for duplicate elements.
///
/// See [`.duplicates_by()`](crate::Itertools::duplicates_by) for more information.
pub type DuplicatesBy<I, V, F> = private::DuplicatesBy<I, V, private::ByFn<F>>;

/// Create a new `DuplicatesBy` iterator.
pub fn duplicates_by<I, Key, F>(iter: I, f: F) -> DuplicatesBy<I, Key, F>
where
    Key: Eq + Hash,
    F: FnMut(&I::Item) -> Key,
    I: Iterator,
{
    DuplicatesBy::new(iter, private::ByFn(f))
}

/// An iterator adapter to filter out duplicate elements.
///
/// See [`.duplicates()`](crate::Itertools::duplicates) for more information.
pub type Duplicates<I> = private::DuplicatesBy<I, <I as Iterator>::Item, private::ById>;

/// Create a new `Duplicates` iterator.
pub fn duplicates<I>(iter: I) -> Duplicates<I>
where
    I: Iterator,
    I::Item: Eq + Hash,
{
    Duplicates::new(iter, private::ById)
}

