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

//! This mod defines utilities to easily create Rust values as Starlark values.

use either::Either;

use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::FrozenHeap;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::StringValue;
use crate::values::Value;

/// Trait for things that can be created on a [`Heap`] producing a [`Value`].
///
/// Note, this trait does not represent Starlark types.
/// For example, this trait is implemented for `char`,
/// but there's no Starlark type for `char`, this trait
/// is implemented for `char` to construct Starlark `str`.
///
/// For types that implement [`crate::values::StarlarkValue`] a typical implementation
/// will probably call either [`Heap::alloc_simple`] or [`Heap::alloc_complex`],
/// e.g.
///
/// ```
/// # use allocative::Allocative;
/// # use starlark::any::ProvidesStaticType;
/// # use starlark::values::{AllocValue, Heap, NoSerialize, starlark_value, StarlarkValue, Value};
///
/// #[derive(Debug, derive_more::Display, Allocative, NoSerialize, ProvidesStaticType)]
/// struct MySimpleValue;
///
/// #[starlark_value(type = "MySimpleValue", UnpackValue, StarlarkTypeRepr)]
/// impl<'v> StarlarkValue<'v> for MySimpleValue {}
///
/// impl<'v> AllocValue<'v> for MySimpleValue {
///     fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
///         heap.alloc_simple(self)
///     }
/// }
/// ```
///
/// # Derive
///
/// `AllocValue` can be derived for enums, like this:
///
/// ```
/// use starlark::values::type_repr::StarlarkTypeRepr;
/// use starlark::values::AllocValue;
///
/// #[derive(StarlarkTypeRepr, AllocValue)]
/// enum AllocIntOrStr {
///     Int(i32),
///     Str(String),
/// }
/// ```
pub trait AllocValue<'v>: StarlarkTypeRepr {
    /// Allocate the value on a heap and return a reference to the allocated value.
    ///
    /// Note, for certain values (e.g. empty strings) no allocation is actually performed,
    /// and a reference to the statically allocated object is returned.
    fn alloc_value(self, heap: &'v Heap) -> Value<'v>;
}

/// Type which allocates a string.
pub trait AllocStringValue<'v>: AllocValue<'v> + Sized {
    /// Allocate a string.
    fn alloc_string_value(self, heap: &'v Heap) -> StringValue<'v>;
}

impl<'v> AllocValue<'v> for FrozenValue {
    fn alloc_value(self, _heap: &'v Heap) -> Value<'v> {
        self.to_value()
    }
}

impl<'v> AllocValue<'v> for Value<'v> {
    fn alloc_value(self, _heap: &'v Heap) -> Value<'v> {
        self
    }
}

impl<'v, A: AllocValue<'v>, B: AllocValue<'v>> AllocValue<'v> for Either<A, B> {
    #[inline]
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        match self {
            Either::Left(a) => a.alloc_value(heap),
            Either::Right(b) => b.alloc_value(heap),
        }
    }
}

impl<A: AllocFrozenValue, B: AllocFrozenValue> AllocFrozenValue for Either<A, B> {
    #[inline]
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        match self {
            Either::Left(a) => a.alloc_frozen_value(heap),
            Either::Right(b) => b.alloc_frozen_value(heap),
        }
    }
}

/// Trait for things that can be allocated on a [`FrozenHeap`] producing a [`FrozenValue`].
///
/// # Derive
///
/// `AllocFrozenValue` can be derived for enums, like this:
///
/// ```
/// use starlark::values::type_repr::StarlarkTypeRepr;
/// use starlark::values::AllocFrozenValue;
///
/// #[derive(StarlarkTypeRepr, AllocFrozenValue)]
/// enum AllocIntOrStr {
///     Int(i32),
///     Str(String),
/// }
/// ```
pub trait AllocFrozenValue: StarlarkTypeRepr {
    /// Allocate a value in the frozen heap and return a reference to the allocated value.
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue;
}

/// Type which allocates a string.
pub trait AllocFrozenStringValue: AllocFrozenValue + Sized {
    /// Allocate a string.
    fn alloc_frozen_string_value(self, heap: &FrozenHeap) -> FrozenStringValue;
}

impl AllocFrozenValue for FrozenValue {
    fn alloc_frozen_value(self, _heap: &FrozenHeap) -> FrozenValue {
        self
    }
}
