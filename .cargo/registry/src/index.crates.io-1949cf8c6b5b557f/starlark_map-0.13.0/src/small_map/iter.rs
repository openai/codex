/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use dupe::Clone_;

use crate::iter::def_double_ended_iter;
use crate::iter::def_iter;
use crate::vec_map;
use crate::Hashed;

/// Iterator over the hashed entries of [`SmallMap`](crate::small_map::SmallMap).
#[derive(Clone_)]
pub struct IterHashed<'a, K, V> {
    pub(crate) iter: vec_map::IterHashed<'a, K, V>,
}

impl<'a, K, V> IterHashed<'a, K, V> {
    #[inline]
    fn map((k, v): (Hashed<&'a K>, &'a V)) -> <Self as Iterator>::Item {
        (k, v)
    }
}

impl<'a, K, V> Iterator for IterHashed<'a, K, V> {
    type Item = (Hashed<&'a K>, &'a V);

    def_iter!();
}

impl<'a, K, V> ExactSizeIterator for IterHashed<'a, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, K, V> DoubleEndedIterator for IterHashed<'a, K, V> {
    def_double_ended_iter!();
}

/// Iterator over a small map entry references.
#[derive(Clone_)]
pub struct Iter<'a, K, V> {
    pub(crate) iter: vec_map::Iter<'a, K, V>,
}

impl<'a, K, V> Iter<'a, K, V> {
    #[inline]
    fn map((k, v): (&'a K, &'a V)) -> <Self as Iterator>::Item {
        (k, v)
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    def_iter!();
}

impl<'a, K, V> ExactSizeIterator for Iter<'a, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, K, V> DoubleEndedIterator for Iter<'a, K, V> {
    def_double_ended_iter!();
}

/// Iterator over a small map mutable entry references.
pub struct IterMut<'a, K, V> {
    pub(crate) iter: vec_map::IterMut<'a, K, V>,
}

/// Iterator over a small map mutable entry references.
///
/// This iterator provides mutable references to keys, not just to values.
pub struct IterMutUnchecked<'a, K, V> {
    pub(crate) iter: vec_map::IterMutUnchecked<'a, K, V>,
}

impl<'a, K, V> IterMut<'a, K, V> {
    #[inline]
    fn map((k, v): (&'a K, &'a mut V)) -> <Self as Iterator>::Item {
        (k, v)
    }
}

impl<'a, K, V> IterMutUnchecked<'a, K, V> {
    #[inline]
    fn map((k, v): (&'a mut K, &'a mut V)) -> <Self as Iterator>::Item {
        (k, v)
    }
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    def_iter!();
}

impl<'a, K, V> ExactSizeIterator for IterMut<'a, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, K, V> DoubleEndedIterator for IterMut<'a, K, V> {
    def_double_ended_iter!();
}

impl<'a, K, V> Iterator for IterMutUnchecked<'a, K, V> {
    type Item = (&'a mut K, &'a mut V);

    def_iter!();
}

impl<'a, K, V> ExactSizeIterator for IterMutUnchecked<'a, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, K, V> DoubleEndedIterator for IterMutUnchecked<'a, K, V> {
    def_double_ended_iter!();
}

/// Iterator that moves hashed entries out of a [`SmallMap`](crate::small_map::SmallMap).
pub struct IntoIterHashed<K, V> {
    pub(crate) iter: vec_map::IntoIterHashed<K, V>,
}

impl<K, V> IntoIterHashed<K, V> {
    #[inline]
    fn map((k, v): (Hashed<K>, V)) -> <Self as Iterator>::Item {
        (k, v)
    }
}

impl<K, V> Iterator for IntoIterHashed<K, V> {
    type Item = (Hashed<K>, V);

    def_iter!();
}

impl<K, V> ExactSizeIterator for IntoIterHashed<K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> DoubleEndedIterator for IntoIterHashed<K, V> {
    def_double_ended_iter!();
}

/// Iterator that moves entries out of a [`SmallMap`](crate::small_map::SmallMap).
pub struct IntoIter<K, V> {
    pub(crate) iter: vec_map::IntoIter<K, V>,
}

impl<K, V> IntoIter<K, V> {
    #[inline]
    fn map((k, v): (K, V)) -> <Self as Iterator>::Item {
        (k, v)
    }
}

impl<K, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    def_iter!();
}

impl<K, V> ExactSizeIterator for IntoIter<K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> DoubleEndedIterator for IntoIter<K, V> {
    def_double_ended_iter!();
}

/// Iterator over a [`SmallMap`](crate::small_map::SmallMap) keys.
#[derive(Clone_)]
pub struct Keys<'a, K, V> {
    pub(crate) iter: vec_map::Keys<'a, K, V>,
}

impl<'a, K, V> Keys<'a, K, V> {
    #[inline]
    fn map(k: &'a K) -> <Self as Iterator>::Item {
        k
    }
}

impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    def_iter!();
}

impl<'a, K, V> ExactSizeIterator for Keys<'a, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, K, V> DoubleEndedIterator for Keys<'a, K, V> {
    def_double_ended_iter!();
}

/// Iterator over a [`SmallMap`](crate::small_map::SmallMap) values.
#[derive(Clone_)]
pub struct Values<'a, K, V> {
    pub(crate) iter: vec_map::Values<'a, K, V>,
}

impl<'a, K, V> Values<'a, K, V> {
    #[inline]
    fn map(v: &'a V) -> <Self as Iterator>::Item {
        v
    }
}

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    def_iter!();
}

impl<'a, K, V> ExactSizeIterator for Values<'a, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, K, V> DoubleEndedIterator for Values<'a, K, V> {
    def_double_ended_iter!();
}

/// Iterator that moves keys out of [`SmallMap`](crate::small_map::SmallMap).
pub struct IntoKeys<K, V> {
    pub(crate) iter: vec_map::IntoIter<K, V>,
}

impl<K, V> IntoKeys<K, V> {
    #[inline]
    fn map((k, _): (K, V)) -> <Self as Iterator>::Item {
        k
    }
}

impl<K, V> Iterator for IntoKeys<K, V> {
    type Item = K;

    def_iter!();
}

impl<K, V> ExactSizeIterator for IntoKeys<K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> DoubleEndedIterator for IntoKeys<K, V> {
    def_double_ended_iter!();
}

/// Iterator that moves values out of [`SmallMap`](crate::small_map::SmallMap).
pub struct IntoValues<K, V> {
    pub(crate) iter: vec_map::IntoIter<K, V>,
}

impl<K, V> IntoValues<K, V> {
    #[inline]
    fn map((_, v): (K, V)) -> <Self as Iterator>::Item {
        v
    }
}

impl<K, V> Iterator for IntoValues<K, V> {
    type Item = V;

    def_iter!();
}

impl<K, V> ExactSizeIterator for IntoValues<K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> DoubleEndedIterator for IntoValues<K, V> {
    def_double_ended_iter!();
}

/// Iterator over a [`SmallMap`](crate::small_map::SmallMap) mutable values.
pub struct ValuesMut<'a, K, V> {
    pub(crate) iter: vec_map::ValuesMut<'a, K, V>,
}

impl<'a, K, V> ValuesMut<'a, K, V> {
    #[inline]
    fn map(v: &'a mut V) -> <Self as Iterator>::Item {
        v
    }
}

impl<'a, K, V> Iterator for ValuesMut<'a, K, V> {
    type Item = &'a mut V;

    def_iter!();
}

impl<'a, K, V> ExactSizeIterator for ValuesMut<'a, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, K, V> DoubleEndedIterator for ValuesMut<'a, K, V> {
    def_double_ended_iter!();
}

fn _assert_iterators_sync_send() {
    fn assert_sync_send<T: Sync + Send>(_: T) {}
    fn test_iter_hashed(iter: IterHashed<String, u32>) {
        assert_sync_send(iter);
    }
    fn test_iter(iter: Iter<String, u32>) {
        assert_sync_send(iter);
    }
    fn test_into_iter_hashed(iter: IntoIterHashed<String, u32>) {
        assert_sync_send(iter);
    }
    fn test_into_iter(iter: IntoIter<String, u32>) {
        assert_sync_send(iter);
    }
    fn test_keys(iter: Keys<String, u32>) {
        assert_sync_send(iter);
    }
    fn test_values(iter: Values<String, u32>) {
        assert_sync_send(iter);
    }
    fn test_into_keys(iter: IntoKeys<String, u32>) {
        assert_sync_send(iter);
    }
    fn test_into_values(iter: IntoValues<String, u32>) {
        assert_sync_send(iter);
    }
}
