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

use std::slice;

use dupe::Clone_;

use crate::iter::def_double_ended_iter;
use crate::iter::def_iter;
use crate::vec2;
use crate::Hashed;
use crate::StarlarkHashValue;

#[derive(Clone_)]
pub(crate) struct Keys<'a, K: 'a, V: 'a> {
    pub(crate) iter: Iter<'a, K, V>,
}

impl<'a, K: 'a, V: 'a> Keys<'a, K, V> {
    #[inline]
    fn map((k, _v): (&'a K, &'a V)) -> <Self as Iterator>::Item {
        k
    }
}

impl<'a, K: 'a, V: 'a> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    def_iter!();
}

impl<'a, K: 'a, V: 'a> DoubleEndedIterator for Keys<'a, K, V> {
    def_double_ended_iter!();
}

impl<'a, K: 'a, V: 'a> ExactSizeIterator for Keys<'a, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

#[derive(Clone_)]
pub(crate) struct Values<'a, K: 'a, V: 'a> {
    pub(crate) iter: Iter<'a, K, V>,
}

impl<'a, K: 'a, V: 'a> Values<'a, K, V> {
    #[inline]
    fn map((_k, v): (&'a K, &'a V)) -> <Self as Iterator>::Item {
        v
    }
}

impl<'a, K: 'a, V: 'a> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    def_iter!();
}

impl<'a, K: 'a, V: 'a> DoubleEndedIterator for Values<'a, K, V> {
    def_double_ended_iter!();
}

impl<'a, K: 'a, V: 'a> ExactSizeIterator for Values<'a, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

pub(crate) struct ValuesMut<'a, K: 'a, V: 'a> {
    pub(crate) iter: IterMut<'a, K, V>,
}

impl<'a, K: 'a, V: 'a> ValuesMut<'a, K, V> {
    #[inline]
    fn map((_k, v): (&'a K, &'a mut V)) -> <Self as Iterator>::Item {
        v
    }
}

impl<'a, K: 'a, V: 'a> Iterator for ValuesMut<'a, K, V> {
    type Item = &'a mut V;

    def_iter!();
}

impl<'a, K: 'a, V: 'a> DoubleEndedIterator for ValuesMut<'a, K, V> {
    def_double_ended_iter!();
}

impl<'a, K: 'a, V: 'a> ExactSizeIterator for ValuesMut<'a, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

#[derive(Clone_)]
pub(crate) struct Iter<'a, K: 'a, V: 'a> {
    pub(crate) iter: slice::Iter<'a, (K, V)>,
}

impl<'a, K: 'a, V: 'a> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    def_iter!();
}

impl<'a, K: 'a, V: 'a> DoubleEndedIterator for Iter<'a, K, V> {
    def_double_ended_iter!();
}

impl<'a, K: 'a, V: 'a> ExactSizeIterator for Iter<'a, K, V> {}

impl<'a, K: 'a, V: 'a> Iter<'a, K, V> {
    #[inline]
    fn map((k, v): &'a (K, V)) -> (&'a K, &'a V) {
        (k, v)
    }
}

#[derive(Clone_)]
pub(crate) struct IterHashed<'a, K: 'a, V: 'a> {
    pub(crate) iter: vec2::Iter<'a, (K, V), StarlarkHashValue>,
}

impl<'a, K: 'a, V: 'a> IterHashed<'a, K, V> {
    #[inline]
    fn map(((k, v), hash): (&'a (K, V), &'a StarlarkHashValue)) -> (Hashed<&'a K>, &'a V) {
        (Hashed::new_unchecked(*hash, k), v)
    }
}

impl<'a, K: 'a, V: 'a> Iterator for IterHashed<'a, K, V> {
    type Item = (Hashed<&'a K>, &'a V);

    def_iter!();
}

impl<'a, K: 'a, V: 'a> DoubleEndedIterator for IterHashed<'a, K, V> {
    def_double_ended_iter!();
}

impl<'a, K: 'a, V: 'a> ExactSizeIterator for IterHashed<'a, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

pub(crate) struct IterMut<'a, K: 'a, V: 'a> {
    pub(crate) iter: slice::IterMut<'a, (K, V)>,
}

pub(crate) struct IterMutUnchecked<'a, K: 'a, V: 'a> {
    pub(crate) iter: slice::IterMut<'a, (K, V)>,
}

impl<'a, K: 'a, V: 'a> IterMut<'a, K, V> {
    #[inline]
    fn map((k, v): &mut (K, V)) -> (&K, &mut V) {
        (k, v)
    }
}

impl<'a, K: 'a, V: 'a> IterMutUnchecked<'a, K, V> {
    #[inline]
    fn map((k, v): &mut (K, V)) -> (&mut K, &mut V) {
        (k, v)
    }
}

impl<'a, K: 'a, V: 'a> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    def_iter!();
}

impl<'a, K: 'a, V: 'a> DoubleEndedIterator for IterMut<'a, K, V> {
    def_double_ended_iter!();
}

impl<'a, K: 'a, V: 'a> ExactSizeIterator for IterMut<'a, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, K: 'a, V: 'a> Iterator for IterMutUnchecked<'a, K, V> {
    type Item = (&'a mut K, &'a mut V);

    def_iter!();
}

impl<'a, K: 'a, V: 'a> DoubleEndedIterator for IterMutUnchecked<'a, K, V> {
    def_double_ended_iter!();
}

impl<'a, K: 'a, V: 'a> ExactSizeIterator for IterMutUnchecked<'a, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

pub(crate) struct IntoIterHashed<K, V> {
    pub(crate) iter: vec2::IntoIter<(K, V), StarlarkHashValue>,
}

impl<K, V> IntoIterHashed<K, V> {
    #[inline]
    fn map(((k, v), hash): ((K, V), StarlarkHashValue)) -> (Hashed<K>, V) {
        (Hashed::new_unchecked(hash, k), v)
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

pub(crate) struct IntoIter<K, V> {
    pub(crate) iter: IntoIterHashed<K, V>,
}

impl<K, V> IntoIter<K, V> {
    #[inline]
    fn map((k, v): (Hashed<K>, V)) -> (K, V) {
        (k.into_key(), v)
    }
}

impl<'a, K: 'a, V: 'a> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    def_iter!();
}

impl<'a, K: 'a, V: 'a> DoubleEndedIterator for IntoIter<K, V> {
    def_double_ended_iter!();
}

impl<'a, K: 'a, V: 'a> ExactSizeIterator for IntoIter<K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}
