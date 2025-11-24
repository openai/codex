/*
 * Copyright 2018 The Starlark in Rust Authors.
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

use allocative::Allocative;
use dupe::Dupe;

use crate::values::layout::const_type_id::ConstTypeId;
use crate::values::StarlarkValue;

/// Identifier of a starlark type.
///
/// This is different from `TypeId` of `StarlarkValue` implementation:
/// multiple Rust types can share the same `StarlarkTypeId`.
/// For example, mutable and frozen list share the same `StarlarkTypeId`.
#[derive(Eq, PartialEq, Hash, Copy, Clone, Dupe, Debug, Allocative)]
pub(crate) struct StarlarkTypeId(ConstTypeId);

impl StarlarkTypeId {
    #[inline]
    pub(crate) const fn from_type_id(type_id: ConstTypeId) -> StarlarkTypeId {
        StarlarkTypeId(type_id)
    }

    #[inline]
    pub(crate) const fn of<'v, T: StarlarkValue<'v>>() -> StarlarkTypeId {
        StarlarkTypeId::of_canonical::<T::Canonical>()
    }

    #[inline]
    pub(crate) const fn of_canonical<'v, T: StarlarkValue<'v>>() -> StarlarkTypeId {
        #[cfg(rust_nightly)]
        {
            // We should check type ids, but `TypeId` equality is not available even in nightly.
            let type_name = std::any::type_name::<T>();
            let type_name_of_canonical_again = std::any::type_name::<T::Canonical>();
            if type_name.len() != type_name_of_canonical_again.len() {
                panic!("Canonical type of already canonical type `T` is not `T`")
            }
            let mut i = 0;
            while i != type_name.len() {
                if type_name.as_bytes()[i] != type_name_of_canonical_again.as_bytes()[i] {
                    panic!("Canonical type of already canonical type `T` is not `T`")
                }
                i += 1;
            }
        }

        StarlarkTypeId(ConstTypeId::of::<T::StaticType>())
    }
}

/// We require alignment 8 for `StarlarkValue`.
/// `TypeId` is 16 bytes aligned on Rust 1.72 on Apple Silicon.
/// Use this struct to put `ConstTypeId` in a `StarlarkValue`.
// TODO(nga): remove alignment requirement from `Heap`.
#[repr(packed(8))]
#[derive(Allocative, Eq, Clone, Copy, Dupe, Debug)]
#[allocative(skip)] // There are no heap allocations in this struct.
pub(crate) struct StarlarkTypeIdAligned {
    starlark_type_id: StarlarkTypeId,
}

impl PartialEq for StarlarkTypeIdAligned {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl StarlarkTypeIdAligned {
    #[inline]
    pub(crate) const fn new(starlark_type_id: StarlarkTypeId) -> StarlarkTypeIdAligned {
        StarlarkTypeIdAligned { starlark_type_id }
    }

    #[inline]
    pub(crate) const fn get(&self) -> StarlarkTypeId {
        self.starlark_type_id
    }
}
