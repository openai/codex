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

use allocative::Allocative;
use dupe::Dupe;
use either::Either;

use crate::typing::Ty;
use crate::values::none::NoneType;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::UnpackValue;
use crate::values::Value;

/// Equivalent of a Rust [`Option`], where `None`
/// is encoded as [`NoneType`](crate::values::none::NoneType).
/// Useful for its [`UnpackValue`] instance.
#[derive(Debug, Eq, PartialEq, Copy, Clone, Dupe, Allocative)]
pub enum NoneOr<T> {
    /// Starlark `None`.
    None,
    /// Not `None`.
    Other(T),
}

impl<T> NoneOr<T> {
    /// Convert the [`NoneOr`] to a real Rust [`Option`].
    #[inline]
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::None => None,
            Self::Other(x) => Some(x),
        }
    }

    /// Convert a Rust [`Option`] to a [`NoneOr`].
    #[inline]
    pub fn from_option(option: Option<T>) -> Self {
        match option {
            None => NoneOr::None,
            Some(x) => NoneOr::Other(x),
        }
    }

    /// Is the value a [`NoneOr::None`].
    pub fn is_none(&self) -> bool {
        matches!(self, NoneOr::None)
    }
}

impl<T: StarlarkTypeRepr> StarlarkTypeRepr for NoneOr<T> {
    type Canonical = <Either<NoneType, T> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        Either::<NoneType, T>::starlark_type_repr()
    }
}

impl<'v, T: UnpackValue<'v>> UnpackValue<'v> for NoneOr<T> {
    type Error = <T as UnpackValue<'v>>::Error;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        if value.is_none() {
            Ok(Some(NoneOr::None))
        } else {
            Ok(T::unpack_value_impl(value)?.map(NoneOr::Other))
        }
    }
}

impl<'v, T: AllocValue<'v>> AllocValue<'v> for NoneOr<T> {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        match self {
            NoneOr::None => Value::new_none(),
            NoneOr::Other(x) => x.alloc_value(heap),
        }
    }
}

impl<T: AllocFrozenValue> AllocFrozenValue for NoneOr<T> {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        match self {
            NoneOr::None => FrozenValue::new_none(),
            NoneOr::Other(x) => x.alloc_frozen_value(heap),
        }
    }
}
