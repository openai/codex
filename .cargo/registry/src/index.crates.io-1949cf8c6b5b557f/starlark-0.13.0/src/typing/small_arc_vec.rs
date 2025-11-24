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

use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;
use std::slice;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;

#[derive(Clone, Dupe, Eq, PartialEq, Allocative)]
enum SmallArcVec1Impl<T> {
    Zero,
    One(T),
    Arc(Arc<[T]>),
}

#[derive(Clone, Dupe, Eq, PartialEq, Allocative)]
pub(crate) struct SmallArcVec1<T>(SmallArcVec1Impl<T>);

impl<T> SmallArcVec1<T> {
    pub(crate) const fn empty() -> Self {
        SmallArcVec1(SmallArcVec1Impl::Zero)
    }

    pub(crate) const fn one(x: T) -> Self {
        SmallArcVec1(SmallArcVec1Impl::One(x))
    }

    pub(crate) fn clone_from_slice(slice: &[T]) -> SmallArcVec1<T>
    where
        T: Clone,
    {
        match slice {
            [] => SmallArcVec1::empty(),
            [x] => SmallArcVec1::one(x.clone()),
            xs => SmallArcVec1(SmallArcVec1Impl::Arc(xs.into())),
        }
    }

    pub(crate) fn as_slice(&self) -> &[T] {
        match &self.0 {
            SmallArcVec1Impl::Zero => &[],
            SmallArcVec1Impl::One(x) => slice::from_ref(x),
            SmallArcVec1Impl::Arc(x) => {
                debug_assert!(x.len() >= 2);
                x
            }
        }
    }
}

impl<T> Deref for SmallArcVec1<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> Default for SmallArcVec1<T> {
    fn default() -> Self {
        SmallArcVec1(SmallArcVec1Impl::Zero)
    }
}

impl<T: PartialOrd> PartialOrd for SmallArcVec1<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl<T: Ord> Ord for SmallArcVec1<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl<T: Hash> Hash for SmallArcVec1<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state)
    }
}

impl<T: Debug> Debug for SmallArcVec1<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.as_slice(), f)
    }
}

impl<T> FromIterator<T> for SmallArcVec1<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut iter = iter.into_iter();
        let Some(i0) = iter.next() else {
            return SmallArcVec1::default();
        };
        let Some(i1) = iter.next() else {
            return SmallArcVec1(SmallArcVec1Impl::One(i0));
        };
        let vec: Vec<T> = [i0, i1].into_iter().chain(iter).collect();
        SmallArcVec1(SmallArcVec1Impl::Arc(vec.into()))
    }
}
