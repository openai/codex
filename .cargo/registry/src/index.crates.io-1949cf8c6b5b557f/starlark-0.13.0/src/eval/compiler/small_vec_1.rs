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

//! Small `Vec`.

use std::fmt::Debug;
use std::hash::Hash;
use std::hash::Hasher;
use std::iter;
use std::mem;
use std::ops::Deref;
use std::slice;
use std::vec;

use allocative::Allocative;
use either::Either;

/// A small vector.
#[derive(Clone, Allocative)]
pub(crate) enum SmallVec1<T> {
    One(T),
    Vec(Vec<T>),
}

impl<T: Debug> Debug for SmallVec1<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.as_slice()).finish()
    }
}

impl<T: PartialEq> PartialEq for SmallVec1<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Eq> Eq for SmallVec1<T> {}

impl<T: Hash> Hash for SmallVec1<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state)
    }
}

impl<T: PartialOrd> PartialOrd for SmallVec1<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl<T: Ord> Ord for SmallVec1<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl<T> Deref for SmallVec1<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> IntoIterator for SmallVec1<T> {
    type Item = T;
    type IntoIter = Either<vec::IntoIter<T>, iter::Once<T>>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            SmallVec1::One(x) => Either::Right(iter::once(x)),
            SmallVec1::Vec(xs) => Either::Left(xs.into_iter()),
        }
    }
}

impl<T> SmallVec1<T> {
    pub(crate) const fn new() -> SmallVec1<T> {
        SmallVec1::Vec(Vec::new())
    }

    pub(crate) fn as_slice(&self) -> &[T] {
        match self {
            SmallVec1::One(x) => slice::from_ref(x),
            SmallVec1::Vec(xs) => xs,
        }
    }

    pub(crate) fn extend(&mut self, that: SmallVec1<T>) {
        *self = match (mem::replace(self, SmallVec1::Vec(Vec::new())), that) {
            (SmallVec1::Vec(vec), right) if vec.is_empty() => right,
            (left, SmallVec1::Vec(vec)) if vec.is_empty() => left,
            (SmallVec1::One(left), SmallVec1::One(right)) => SmallVec1::Vec(vec![left, right]),
            (SmallVec1::One(left), SmallVec1::Vec(mut right)) => {
                right.insert(0, left);
                SmallVec1::Vec(right)
            }
            (SmallVec1::Vec(mut left), SmallVec1::One(right)) => {
                left.push(right);
                SmallVec1::Vec(left)
            }
            (SmallVec1::Vec(mut left), SmallVec1::Vec(right)) => {
                left.extend(right);
                SmallVec1::Vec(left)
            }
        }
    }

    pub(crate) fn push(&mut self, value: T) {
        self.extend(SmallVec1::One(value))
    }
}
