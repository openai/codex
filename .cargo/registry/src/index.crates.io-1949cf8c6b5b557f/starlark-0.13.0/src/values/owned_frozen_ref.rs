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

use std::convert::Infallible;
use std::fmt;
use std::fmt::Formatter;
use std::mem;
use std::ops::Deref;

use allocative::Allocative;
use dupe::Clone_;
use dupe::Copy_;
use dupe::Dupe;
use dupe::Dupe_;

use crate::values::FrozenHeap;
use crate::values::FrozenHeapRef;
use crate::values::FrozenRef;

/// A reference to a value stored in a frozen heap with a reference to the heap.
#[derive(Copy_, Clone_, Dupe_)]
pub struct OwnedRefFrozenRef<'f, T: ?Sized + 'static> {
    owner: &'f FrozenHeapRef,
    value: FrozenRef<'f, T>,
}

/// Same as a `FrozenRef`, but it keeps itself alive by storing a reference to the owning heap.
///
/// Usually constructed from an `OwnedFrozenValueTyped`.
#[derive(Clone, Dupe, Allocative)]
pub struct OwnedFrozenRef<T: ?Sized + 'static> {
    owner: FrozenHeapRef,
    // Invariant: this FrozenValue must be kept alive by the `owner` field.
    value: FrozenRef<'static, T>,
}

impl<'f, T: ?Sized> OwnedRefFrozenRef<'f, T> {
    /// Create a new `OwnedRefFrozenRef` pointing at the given value.
    pub unsafe fn new_unchecked(
        value: &'f T,
        owner: &'f FrozenHeapRef,
    ) -> OwnedRefFrozenRef<'f, T> {
        OwnedRefFrozenRef {
            owner,
            value: FrozenRef::new(value),
        }
    }

    /// Owner heap.
    pub fn owner(&self) -> &'f FrozenHeapRef {
        self.owner
    }

    /// Return a reference to the underlying value.
    pub fn as_ref(self) -> &'f T {
        self.value.as_ref()
    }

    /// Add a reference to a new heap, and return the pointer with the lifetime of the new heap.
    pub fn add_heap_ref<'v>(self, heap: &'v FrozenHeap) -> &'v T {
        heap.add_reference(self.owner);
        unsafe { mem::transmute::<&'f T, &'v T>(self.value.as_ref()) }
    }

    /// Convert heap pointer to an owned one.
    pub fn to_owned(self) -> OwnedFrozenRef<T> {
        OwnedFrozenRef {
            owner: self.owner.dupe(),
            value: unsafe { mem::transmute::<FrozenRef<'f, T>, FrozenRef<'static, T>>(self.value) },
        }
    }

    /// Fallible map the reference to another one.
    pub fn try_map_result<F, U: ?Sized, E>(self, f: F) -> Result<OwnedRefFrozenRef<'f, U>, E>
    where
        F: FnOnce(&'f T) -> Result<&'f U, E>,
    {
        Ok(OwnedRefFrozenRef {
            owner: self.owner,
            value: FrozenRef::new(f(self.value.as_ref())?),
        })
    }

    /// Apply a function to the underlying value. Projection operation.
    pub fn map<F, U: ?Sized>(self, f: F) -> OwnedRefFrozenRef<'f, U>
    where
        F: FnOnce(&'f T) -> &'f U,
    {
        match self.try_map_result(|x| Ok(f(x))) {
            Ok(x) => x,
            Err(e) => {
                let e: Infallible = e;
                match e {}
            }
        }
    }

    /// Optionally map the reference to another one.
    pub fn try_map_option<F, U: ?Sized>(self, f: F) -> Option<OwnedRefFrozenRef<'f, U>>
    where
        F: FnOnce(&'f T) -> Option<&'f U>,
    {
        match self.try_map_result(|x| f(x).ok_or(())) {
            Ok(x) => Some(x),
            Err(()) => None,
        }
    }
}

impl<T: ?Sized> OwnedFrozenRef<T> {
    /// Creates a new `OwnedFrozenRef` pointing at the given value.
    ///
    /// ## Safety
    ///
    /// The reference must be kept alive by the owning heap
    pub unsafe fn new_unchecked(value: &'static T, owner: FrozenHeapRef) -> OwnedFrozenRef<T> {
        OwnedFrozenRef {
            owner,
            value: FrozenRef::new(value),
        }
    }

    /// Borrow.
    pub fn as_owned_ref_frozen_ref(&self) -> OwnedRefFrozenRef<'_, T> {
        OwnedRefFrozenRef {
            owner: &self.owner,
            value: self.value,
        }
    }

    /// Returns a reference to the underlying value.
    pub fn as_ref<'a>(&'a self) -> &'a T {
        self.as_owned_ref_frozen_ref().as_ref()
    }

    /// Converts `self` into a new reference that points at something reachable from the previous.
    ///
    /// See the caveats on `[starlark::values::OwnedFrozenValue::map]`
    pub fn map<F, U: ?Sized>(self, f: F) -> OwnedFrozenRef<U>
    where
        for<'v> F: FnOnce(&'v T) -> &'v U,
    {
        OwnedFrozenRef {
            owner: self.owner,
            value: self.value.map(f),
        }
    }

    /// Fallible map the reference to another one.
    pub fn try_map_result<F, U: ?Sized, E>(self, f: F) -> Result<OwnedFrozenRef<U>, E>
    where
        for<'v> F: FnOnce(&'v T) -> Result<&'v U, E>,
    {
        Ok(OwnedFrozenRef {
            owner: self.owner,
            value: self.value.try_map_result(f)?,
        })
    }

    /// Optionally map the reference to another one.
    pub fn try_map_option<F, U: ?Sized>(self, f: F) -> Option<OwnedFrozenRef<U>>
    where
        for<'v> F: FnOnce(&'v T) -> Option<&'v U>,
    {
        Some(OwnedFrozenRef {
            owner: self.owner,
            value: self.value.try_map_option(f)?,
        })
    }

    /// Get a reference to the owning frozen heap
    pub fn owner(&self) -> &FrozenHeapRef {
        &self.owner
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for OwnedFrozenRef<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.value, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for OwnedFrozenRef<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.value, f)
    }
}

impl<T: ?Sized> Deref for OwnedFrozenRef<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.as_ref()
    }
}
