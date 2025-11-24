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

//! Utility to make stateful debug assertions easier.

use std::cmp::Ordering;
use std::marker;

use dupe::Dupe;

/// Store `T` is debug assertions enabled, ZST otherwise.
///
/// This object is easier to work with than explicitly marking code with
/// `#[cfg(debug_assertions)]`: cfg attributes are not easy to write
/// without mistakes where code works, and no warnings in both debug and release.
///
/// In other words, this type converts cfg to types.
///
/// This type implements [`Eq`] which always returns `true`,
/// and [`Ord`] which always returns [`Ordering::Equal`], so
/// * this type is easy to include in structs which do derives
/// * behavior of this object does not depend on `<T>`
/// * behavior does not depend on whether debugging assertions enabled or not
#[derive(Debug, Default, Copy, Clone, Dupe)]
// In release build this structure is DST,
// so gazebo suggests implementing `Dupe` for any `<T>`. T102920913.
pub(crate) struct IfDebug<T> {
    #[cfg(debug_assertions)]
    value: T,
    _marker: marker::PhantomData<T>,
}

impl<T> IfDebug<T> {
    /// Store a value if debug assertions enabled, drop otherwise.
    pub(crate) fn new(value: T) -> IfDebug<T> {
        Self::new_if_debug(|| value)
    }

    /// Store a value if debug assertions enabled, drop otherwise.
    pub(crate) fn new_if_debug(init: impl FnOnce() -> T) -> IfDebug<T> {
        #[cfg(not(debug_assertions))]
        drop(init);
        IfDebug {
            #[cfg(debug_assertions)]
            value: { init() },
            _marker: marker::PhantomData,
        }
    }

    /// Get a reference to stored value is assertions enabled, `None` otherwise.
    pub(crate) fn get_ref(&self) -> Option<&T> {
        #[cfg(debug_assertions)]
        return Some(&self.value);
        #[cfg(not(debug_assertions))]
        return None;
    }

    /// Get a reference to stored value is assertions enabled, panic otherwise.
    pub(crate) fn get_ref_if_debug(&self) -> &T {
        self.get_ref().expect("assertions disabled")
    }

    /// Invoke a function if debug enabled.
    pub(crate) fn if_debug(&self, f: impl FnOnce(&T)) {
        if let Some(value) = self.get_ref() {
            f(value);
        }
    }
}

impl<T> PartialEq for IfDebug<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T> Eq for IfDebug<T> {}

impl<T> PartialOrd for IfDebug<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for IfDebug<T> {
    fn cmp(&self, _other: &Self) -> Ordering {
        Ordering::Equal
    }
}
