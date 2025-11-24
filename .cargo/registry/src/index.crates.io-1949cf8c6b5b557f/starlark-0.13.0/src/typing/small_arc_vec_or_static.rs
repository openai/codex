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

use std::hash::Hash;
use std::ops::Deref;
use std::slice;

use allocative::Allocative;
use dupe::Dupe;

use crate::typing::small_arc_vec::SmallArcVec1;

#[derive(Debug, Clone, Dupe, Allocative)]
enum SmallArcVec1OrStaticImpl<T: 'static> {
    Arc(SmallArcVec1<T>),
    Static(&'static [T]),
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub(crate) struct SmallArcVec1OrStatic<T: 'static>(SmallArcVec1OrStaticImpl<T>);

impl<T: 'static> Default for SmallArcVec1OrStatic<T> {
    fn default() -> Self {
        Self::new_static(&[])
    }
}

impl<T: 'static> Deref for SmallArcVec1OrStatic<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T: PartialEq + 'static> PartialEq for SmallArcVec1OrStatic<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Eq + 'static> Eq for SmallArcVec1OrStatic<T> {}

impl<T: Hash + 'static> Hash for SmallArcVec1OrStatic<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state)
    }
}

impl<T: PartialOrd + 'static> PartialOrd for SmallArcVec1OrStatic<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl<T: Ord + 'static> Ord for SmallArcVec1OrStatic<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl<T: 'static> SmallArcVec1OrStatic<T> {
    pub(crate) fn new_static(x: &'static [T]) -> Self {
        SmallArcVec1OrStatic(SmallArcVec1OrStaticImpl::Static(x))
    }

    pub(crate) fn clone_from_slice(x: &[T]) -> Self
    where
        T: Clone,
    {
        if x.is_empty() {
            Self::new_static(&[])
        } else {
            SmallArcVec1OrStatic(SmallArcVec1OrStaticImpl::Arc(
                SmallArcVec1::clone_from_slice(x),
            ))
        }
    }

    pub(crate) fn as_slice(&self) -> &[T] {
        match &self.0 {
            SmallArcVec1OrStaticImpl::Arc(x) => x,
            SmallArcVec1OrStaticImpl::Static(x) => x,
        }
    }
}

impl<'a, T> IntoIterator for &'a SmallArcVec1OrStatic<T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}
