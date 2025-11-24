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

use std::fmt;
use std::fmt::Display;
use std::ops::Deref;

use allocative::Allocative;
use dupe::Clone_;
use dupe::Dupe;
use dupe::Dupe_;

use crate::cast::transmute;
use crate::typing::Ty;
use crate::values::none::NoneType;
use crate::values::owned_frozen_ref::OwnedFrozenRef;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::AllocFrozenValue;
use crate::values::FrozenHeap;
use crate::values::FrozenHeapRef;
use crate::values::FrozenValue;
use crate::values::FrozenValueTyped;
use crate::values::OwnedRefFrozenRef;
use crate::values::StarlarkValue;
use crate::values::Value;

#[derive(Debug, thiserror::Error)]
enum OwnedError {
    #[error("Expected value of type `{0}` but got `{1}`")]
    WrongType(&'static str, String),
}

/// A [`FrozenValue`] along with a [`FrozenHeapRef`] that ensures it is kept alive.
/// Obtained from [`FrozenModule::get`](crate::environment::FrozenModule::get) or
/// [`OwnedFrozenValue::alloc`].
///
/// While it is possible to obtain the underlying [`FrozenValue`] with
/// [`unchecked_frozen_value`](OwnedFrozenValue::unchecked_frozen_value), that approach
/// is strongly discouraged. See the other methods which unpack the code, access it as a
/// [`Value`] (which has a suitable lifetime) or add references to other heaps.
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct OwnedFrozenValue {
    owner: FrozenHeapRef,
    // Invariant: this FrozenValue must be kept alive by the `owner` field.
    value: FrozenValue,
}

impl Default for OwnedFrozenValue {
    fn default() -> Self {
        OwnedFrozenValue::alloc(NoneType)
    }
}

impl Display for OwnedFrozenValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.value, f)
    }
}

impl StarlarkTypeRepr for OwnedFrozenValue {
    type Canonical = <FrozenValue as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        FrozenValue::starlark_type_repr()
    }
}

impl AllocFrozenValue for OwnedFrozenValue {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        // Safe because this is the standard expectation for alloc_frozen_value
        // - you must keep the heap you allocate it on alive.
        unsafe { self.owned_frozen_value(heap) }
    }
}

impl OwnedFrozenValue {
    /// Create an [`OwnedFrozenValue`] - generally [`OwnedFrozenValue`]s are obtained
    /// from [`FrozenModule::get`](crate::environment::FrozenModule::get).
    /// Safe provided the `value` (and any values it points at) are kept alive by the
    /// `owner`, typically because the value was created on the heap.
    ///
    /// ```
    /// use starlark::values::FrozenHeap;
    /// use starlark::values::OwnedFrozenValue;
    /// let heap = FrozenHeap::new();
    /// let value = heap.alloc("test");
    /// unsafe { OwnedFrozenValue::new(heap.into_ref(), value) };
    /// ```
    pub unsafe fn new(owner: FrozenHeapRef, value: FrozenValue) -> Self {
        Self { owner, value }
    }

    /// Create an [`OwnedFrozenValue`] in a new heap.
    pub fn alloc(x: impl AllocFrozenValue) -> Self {
        let heap = FrozenHeap::new();
        let val = heap.alloc(x);
        // Safe because we just created the value on the heap
        unsafe { Self::new(heap.into_ref(), val) }
    }

    /// Unpack the boolean contained in the underlying value, or [`None`] if it is not a boolean.
    pub fn unpack_bool(&self) -> Option<bool> {
        self.value.unpack_bool()
    }

    /// Obtain the underlying integer if it fits in an `i32`.
    /// Note floats are not considered integers, i. e. `unpack_i32` for `1.0` will return `None`.
    pub fn unpack_i32(&self) -> Option<i32> {
        self.value.unpack_i32()
    }

    /// Unpack the string contained in the underlying value, or [`None`] if it is not an string.
    pub fn unpack_str(&self) -> Option<&str> {
        self.value.unpack_str()
    }

    /// Check if `self` references `<T>`.
    pub fn downcast<T: StarlarkValue<'static>>(self) -> Result<OwnedFrozenValueTyped<T>, Self> {
        match FrozenValueTyped::new(self.value) {
            Some(typed) => Ok(OwnedFrozenValueTyped {
                owner: self.owner,
                value: typed,
            }),
            None => Err(self),
        }
    }

    /// `downcast`, but return an error for human instead of original value.
    pub fn downcast_starlark<T: StarlarkValue<'static>>(
        self,
    ) -> crate::Result<OwnedFrozenValueTyped<T>> {
        match self.downcast() {
            Ok(v) => Ok(v),
            Err(this) => Err(crate::Error::new_value(OwnedError::WrongType(
                T::TYPE,
                this.value.to_value().to_string_for_type_error(),
            ))),
        }
    }

    /// Obtain the [`Value`] stored inside.
    pub fn value<'v>(&'v self) -> Value<'v> {
        Value::new_frozen(self.value)
    }

    /// Extract a [`Value`] by passing the [`FrozenHeap`] which will promise to keep it alive.
    /// When using with a [`Module`](crate::environment::Module),
    /// see the [`frozen_heap`](crate::environment::Module::frozen_heap) function.
    /// If you don't care about the resulting lifetime the [`value`](OwnedFrozenValue::value) method is easier.
    pub fn owned_value<'v>(&self, heap: &'v FrozenHeap) -> Value<'v> {
        // Safe because we convert it to a value which is tied to the owning heap
        unsafe { self.owned_frozen_value(heap).to_value() }
    }

    /// Operate on the [`FrozenValue`] stored inside.
    /// Safe provided you don't store the argument [`FrozenValue`] after the closure has returned.
    /// Using this function is discouraged when possible.
    pub fn map(&self, f: impl FnOnce(FrozenValue) -> FrozenValue) -> Self {
        Self {
            owner: self.owner.dupe(),
            value: f(self.value),
        }
    }

    /// Same as [`map`](OwnedFrozenValue::map) above but with [`Result`]
    pub fn try_map<E>(
        &self,
        f: impl FnOnce(FrozenValue) -> Result<FrozenValue, E>,
    ) -> Result<Self, E> {
        Ok(Self {
            owner: self.owner.dupe(),
            value: f(self.value)?,
        })
    }

    /// Obtain a reference to the FrozenHeap that owns this value.
    pub fn owner(&self) -> &FrozenHeapRef {
        &self.owner
    }

    /// Obtain direct access to the [`FrozenValue`] that lives inside. If you drop all
    /// references to the [`FrozenHeap`] keeping it alive, any code using the [`FrozenValue`]
    /// is likely to segfault. If possible use [`value`](OwnedFrozenValue::value) or
    /// [`owned_frozen_value`](OwnedFrozenValue::owned_frozen_value).
    pub unsafe fn unchecked_frozen_value(&self) -> FrozenValue {
        self.value
    }

    /// Extract a [`FrozenValue`] by passing the [`FrozenHeap`] which will keep it alive.
    /// Provided the argument heap does indeed stay alive for the lifetime of the result,
    /// all will be fine. Unsafe if you pass the wrong heap, or don't keep the heap alive
    /// long enough. Where possible, use [`value`](OwnedFrozenValue::value) or
    /// [`owned_value`](OwnedFrozenValue::owned_value).
    pub unsafe fn owned_frozen_value(&self, heap: &FrozenHeap) -> FrozenValue {
        heap.add_reference(&self.owner);
        self.value
    }
}

/// Same as [`OwnedFrozenValue`] but it is known to contain `T`.
#[derive(Debug, Clone_, Dupe_, Allocative)]
pub struct OwnedFrozenValueTyped<T: StarlarkValue<'static>> {
    owner: FrozenHeapRef,
    value: FrozenValueTyped<'static, T>,
}

impl<T: StarlarkValue<'static>> Deref for OwnedFrozenValueTyped<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value.as_ref()
    }
}

impl<T: StarlarkValue<'static>> OwnedFrozenValueTyped<T> {
    /// Create an [`OwnedFrozenValueTyped`] - generally [`OwnedFrozenValueTyped`]s are obtained
    /// from downcasting [`OwnedFrozenValue`].
    ///
    /// Safe provided the `value` (and any values it points at) are kept alive by the
    /// `owner`, typically because the value was created on the heap.
    ///
    /// ```
    /// use starlark::values::FrozenHeap;
    /// use starlark::values::OwnedFrozenValue;
    /// let heap = FrozenHeap::new();
    /// let value = heap.alloc("test");
    /// unsafe { OwnedFrozenValue::new(heap.into_ref(), value) };
    /// ```
    pub unsafe fn new(owner: FrozenHeapRef, value: FrozenValueTyped<'static, T>) -> Self {
        Self { owner, value }
    }

    /// Erase the type.
    ///
    /// This operation is unsafe because returned value is not bound by the heap lifetime.
    /// So if the heap is dropped, the returned value quetly becomes invalid.
    pub unsafe fn to_frozen_value(&self) -> FrozenValue {
        self.value.to_frozen_value()
    }

    /// Get a value reference.
    pub fn to_value<'v>(&'v self) -> Value<'v> {
        // SAFETY: returned value lifetime is tied to self, so
        //   the heap is guaranteed to outlive the returned value.
        unsafe { self.to_frozen_value().to_value() }
    }

    /// Erase the type.
    pub fn to_owned_frozen_value(&self) -> OwnedFrozenValue {
        OwnedFrozenValue {
            owner: self.owner.dupe(),
            value: self.value.to_frozen_value(),
        }
    }

    /// Convert to borrowed ref.
    pub fn as_owned_ref_frozen_ref(&self) -> OwnedRefFrozenRef<'_, T> {
        unsafe { OwnedRefFrozenRef::new_unchecked(self.value.as_ref(), &self.owner) }
    }

    /// Convert to an owned ref.
    pub fn into_owned_frozen_ref(self) -> OwnedFrozenRef<T> {
        // SAFETY: Heap matches the value
        unsafe { OwnedFrozenRef::new_unchecked(self.value.as_ref(), self.owner) }
    }

    /// Obtain a reference to the FrozenHeap that owns this value.
    pub fn owner(&self) -> &FrozenHeapRef {
        &self.owner
    }

    /// Obtain a reference to the value.
    pub fn as_ref(&self) -> &T {
        self.value.as_ref()
    }

    /// Obtain a reference to the value.
    ///
    /// This should return `FrozenValueTyped<'_, T>`, but it is hard to make it work.
    pub unsafe fn value_typed(&self) -> FrozenValueTyped<'static, T> {
        self.value
    }

    /// Extract a [`FrozenValueTyped`] by passing the [`FrozenHeap`] which will keep it alive.
    pub fn owned_frozen_value_typed(&self, heap: &FrozenHeap) -> FrozenValueTyped<'static, T> {
        heap.add_reference(&self.owner);
        self.value
    }

    /// Extract a [`FrozenValue`] by passing the [`FrozenHeap`] which will keep it alive.
    ///
    /// See [`OwnedFrozenValue::owned_frozen_value`].
    pub fn owned_frozen_value(&self, heap: &FrozenHeap) -> FrozenValue {
        self.owned_frozen_value_typed(heap).to_frozen_value()
    }

    /// Extract a [`Value`] by passing the [`FrozenHeap`] which will promise to keep it alive.
    ///
    /// See [`OwnedFrozenValue::owned_value`].
    pub fn owned_value<'v>(&self, heap: &'v FrozenHeap) -> Value<'v> {
        // Safe because we convert it to a value which is tied to the owning heap
        self.owned_frozen_value(heap).to_value()
    }

    /// Extract a reference by passing the [`FrozenHeap`] which will promise to keep it alive.
    ///
    /// See [`OwnedFrozenValue::owned_frozen_value`].
    ///
    /// Not returning `ValueTyped` because of lifetime issues.
    pub fn owned_as_ref<'v>(&self, heap: &'v FrozenHeap) -> &'v T {
        // Keep the reference.
        self.owned_value(heap);

        // SAFETY: we attached the value to the heap, and we return value with a heap lifetime.
        unsafe { transmute!(&T, &T, self.as_ref()) }
    }

    /// Operate on the [`FrozenValue`] stored inside.
    /// Safe provided you don't store the argument [`FrozenValue`] after the closure has returned.
    /// Using this function is discouraged when possible.
    pub fn map<U: StarlarkValue<'static>>(
        &self,
        f: impl FnOnce(FrozenValueTyped<T>) -> FrozenValueTyped<U>,
    ) -> OwnedFrozenValueTyped<U> {
        OwnedFrozenValueTyped {
            owner: self.owner.dupe(),
            value: f(self.value),
        }
    }

    /// Same as [`map`](OwnedFrozenValue::map) above but with [`Result`]
    pub fn try_map<U: StarlarkValue<'static>, E>(
        &self,
        f: impl FnOnce(FrozenValueTyped<T>) -> Result<FrozenValueTyped<U>, E>,
    ) -> Result<OwnedFrozenValueTyped<U>, E> {
        Ok(OwnedFrozenValueTyped {
            owner: self.owner.dupe(),
            value: f(self.value)?,
        })
    }

    /// Same as [`map`](OwnedFrozenValue::map) above but with [`Option`]
    pub fn maybe_map<U: StarlarkValue<'static>>(
        &self,
        f: impl FnOnce(FrozenValueTyped<T>) -> Option<FrozenValueTyped<U>>,
    ) -> Option<OwnedFrozenValueTyped<U>> {
        Some(OwnedFrozenValueTyped {
            owner: self.owner.dupe(),
            value: f(self.value)?,
        })
    }
}
