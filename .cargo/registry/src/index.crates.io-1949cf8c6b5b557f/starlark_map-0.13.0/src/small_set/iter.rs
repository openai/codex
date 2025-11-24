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

use crate::iter::def_double_ended_iter;
use crate::iter::def_iter;
use crate::small_map;
use crate::Hashed;

/// Iterator over the hashed entries of [`SmallSet`](crate::small_set::SmallSet).
pub struct Iter<'a, T> {
    pub(crate) iter: small_map::Iter<'a, T, ()>,
}

/// Iterator over mutable entries of [`SmallSet`](crate::small_set::SmallSet).
pub struct IterMutUnchecked<'a, T> {
    pub(crate) iter: small_map::IterMutUnchecked<'a, T, ()>,
}

/// Iterator over the hashed entries of [`SmallSet`](crate::small_set::SmallSet).
pub struct IterHashed<'a, T> {
    pub(crate) iter: small_map::IterHashed<'a, T, ()>,
}

/// Iterator that moves entries out of a [`SmallSet`](crate::small_set::SmallSet).
pub struct IntoIter<T> {
    pub(crate) iter: small_map::IntoIter<T, ()>,
}

/// Iterator that moves hashed entries out of a [`SmallSet`](crate::small_set::SmallSet).
pub struct IntoIterHashed<T> {
    pub(crate) iter: small_map::IntoIterHashed<T, ()>,
}

impl<'a, T> Clone for Iter<'a, T> {
    fn clone(&self) -> Iter<'a, T> {
        Iter {
            iter: self.iter.clone(),
        }
    }
}

impl<'a, T> Iter<'a, T> {
    #[inline]
    fn map((k, ()): (&'a T, &'a ())) -> &'a T {
        k
    }
}

impl<'a, T> IterMutUnchecked<'a, T> {
    #[inline]
    fn map((k, ()): (&'a mut T, &'a mut ())) -> &'a mut T {
        k
    }
}

impl<'a, T> IterHashed<'a, T> {
    #[inline]
    fn map((k, ()): (Hashed<&'a T>, &'a ())) -> Hashed<&'a T> {
        k
    }
}

impl<T> IntoIter<T> {
    #[inline]
    fn map((k, ()): (T, ())) -> T {
        k
    }
}

impl<T> IntoIterHashed<T> {
    #[inline]
    fn map((k, ()): (Hashed<T>, ())) -> Hashed<T> {
        k
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    def_iter!();
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    def_double_ended_iter!();
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, T> Iterator for IterMutUnchecked<'a, T> {
    type Item = &'a mut T;

    def_iter!();
}

impl<'a, T> DoubleEndedIterator for IterMutUnchecked<'a, T> {
    def_double_ended_iter!();
}

impl<'a, T> ExactSizeIterator for IterMutUnchecked<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, T> Iterator for IterHashed<'a, T> {
    type Item = Hashed<&'a T>;

    def_iter!();
}

impl<'a, T> DoubleEndedIterator for IterHashed<'a, T> {
    def_double_ended_iter!();
}

impl<'a, T> ExactSizeIterator for IterHashed<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    def_iter!();
}

impl<T> DoubleEndedIterator for IntoIter<T> {
    def_double_ended_iter!();
}

impl<T> ExactSizeIterator for IntoIter<T> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<T> Iterator for IntoIterHashed<T> {
    type Item = Hashed<T>;

    def_iter!();
}

impl<T> DoubleEndedIterator for IntoIterHashed<T> {
    def_double_ended_iter!();
}

impl<T> ExactSizeIterator for IntoIterHashed<T> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}
