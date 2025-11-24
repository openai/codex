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

pub(crate) mod string;

use std::convert::Infallible;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::marker;
use std::ops::Deref;

use allocative::Allocative;
use dupe::Clone_;
use dupe::Copy_;
use dupe::Dupe_;
use serde::Serialize;
use starlark_map::Hashed;

use crate as starlark;
use crate::any::AnyLifetime;
use crate::any::ProvidesStaticType;
use crate::cast;
use crate::cast::transmute;
use crate::coerce::Coerce;
use crate::coerce::CoerceKey;
use crate::typing::Ty;
use crate::values::alloc_value::AllocFrozenStringValue;
use crate::values::alloc_value::AllocStringValue;
use crate::values::int::pointer_i32::PointerI32;
use crate::values::layout::avalue::AValue;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::starlark_type_id::StarlarkTypeId;
use crate::values::string::str_type::StarlarkStr;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::Freeze;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenHeap;
use crate::values::FrozenRef;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::FrozenValueOfUnchecked;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::StringValue;
use crate::values::StringValueLike;
use crate::values::Trace;
use crate::values::Tracer;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::ValueOfUnchecked;

/// [`Value`] wrapper which asserts contained value is of type `<T>`.
#[derive(Copy_, Clone_, Dupe_, ProvidesStaticType, Allocative)]
#[allocative(skip)] // Heap owns the value.
pub struct ValueTyped<'v, T: StarlarkValue<'v>>(Value<'v>, marker::PhantomData<&'v T>);
/// [`FrozenValue`] wrapper which asserts contained value is of type `<T>`.
#[derive(Copy_, Clone_, Dupe_, ProvidesStaticType, Allocative)]
#[allocative(skip)] // Heap owns the value.
pub struct FrozenValueTyped<'v, T: StarlarkValue<'v>>(FrozenValue, marker::PhantomData<&'v T>);

unsafe impl<'v, T: StarlarkValue<'v>> Coerce<ValueTyped<'v, T>> for ValueTyped<'v, T> {}
unsafe impl<'v, T: StarlarkValue<'v>> CoerceKey<ValueTyped<'v, T>> for ValueTyped<'v, T> {}
unsafe impl<'v, T: StarlarkValue<'v>> Coerce<Value<'v>> for ValueTyped<'v, T> {}
unsafe impl<'v, T: StarlarkValue<'v>> CoerceKey<Value<'v>> for ValueTyped<'v, T> {}
unsafe impl<'v, T: StarlarkValue<'v>> Coerce<FrozenValueTyped<'v, T>> for FrozenValueTyped<'v, T> {}
unsafe impl<'v, T: StarlarkValue<'v>> CoerceKey<FrozenValueTyped<'v, T>>
    for FrozenValueTyped<'v, T>
{
}
unsafe impl<'v, T: StarlarkValue<'v>> Coerce<Value<'v>> for FrozenValueTyped<'v, T> {}
unsafe impl<'v, T: StarlarkValue<'v>> CoerceKey<Value<'v>> for FrozenValueTyped<'v, T> {}

unsafe impl<'v, 'f, T: StarlarkValue<'f>> Trace<'v> for FrozenValueTyped<'f, T> {
    fn trace(&mut self, _tracer: &Tracer<'v>) {}
}

impl<T: StarlarkValue<'static>> Freeze for FrozenValueTyped<'static, T> {
    type Frozen = Self;

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(self)
    }
}

impl<'v, T: StarlarkValue<'v>> Debug for ValueTyped<'v, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ValueTyped").field(&self.0).finish()
    }
}

impl<'v, T: StarlarkValue<'v>> Debug for FrozenValueTyped<'v, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("FrozenValueTyped").field(&self.0).finish()
    }
}

impl<'v, T: StarlarkValue<'v>> Display for ValueTyped<'v, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<'v, T: StarlarkValue<'v>> Display for FrozenValueTyped<'v, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<'v, T: StarlarkValue<'v>> Serialize for ValueTyped<'v, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'v, T: StarlarkValue<'v>> Serialize for FrozenValueTyped<'v, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'v, T: StarlarkValue<'v>> PartialEq for ValueTyped<'v, T> {
    fn eq(&self, other: &Self) -> bool {
        // Poor man specialization.
        if T::static_type_id() == StarlarkStr::static_type_id() {
            // SAFETY: just checked type ids.
            let (this, other) = unsafe {
                (
                    StringValue::new_unchecked(self.0),
                    StringValue::new_unchecked(other.0),
                )
            };
            this.0.ptr_eq(other.0) || StarlarkStr::eq(this.as_ref(), other.as_ref())
        } else {
            // Slow comparison with virtual call.
            self.0 == other.0
        }
    }
}

impl<'v, T: StarlarkValue<'v>> Eq for ValueTyped<'v, T> {}

impl<'v, T: StarlarkValue<'v>> PartialEq for FrozenValueTyped<'v, T> {
    fn eq(&self, other: &Self) -> bool {
        self.to_value_typed() == other.to_value_typed()
    }
}

impl<'v, T: StarlarkValue<'v>> Eq for FrozenValueTyped<'v, T> {}

impl<'v, T: StarlarkValue<'v>> ValueTyped<'v, T> {
    /// Downcast.
    #[inline]
    pub fn new(value: Value<'v>) -> Option<ValueTyped<'v, T>> {
        value.downcast_ref::<T>()?;
        Some(ValueTyped(value, marker::PhantomData))
    }

    /// Downcast.
    #[inline]
    pub fn new_err(value: Value<'v>) -> anyhow::Result<ValueTyped<'v, T>> {
        value.downcast_ref_err::<T>()?;
        Ok(ValueTyped(value, marker::PhantomData))
    }

    /// Construct typed value without checking the value is of type `<T>`.
    #[inline]
    pub unsafe fn new_unchecked(value: Value<'v>) -> ValueTyped<'v, T> {
        debug_assert!(value.downcast_ref::<T>().is_some());
        ValueTyped(value, marker::PhantomData)
    }

    #[inline]
    pub(crate) fn new_repr<A: AValue<'v, StarlarkValue = T>>(
        repr: &'v AValueRepr<AValueImpl<'v, A>>,
    ) -> ValueTyped<'v, T> {
        ValueTyped(Value::new_repr(repr), marker::PhantomData)
    }

    /// Erase the type.
    #[inline]
    pub fn to_value(self) -> Value<'v> {
        self.0
    }

    /// Get the reference to the pointed value.
    #[inline]
    pub fn as_ref(self) -> &'v T {
        // SAFETY: type is checked in constructor.
        unsafe { self.0.downcast_ref_unchecked() }
    }

    /// Compute the hash value.
    pub fn hashed(self) -> crate::Result<Hashed<Self>> {
        let hash = if let Some(s) = self.to_value().unpack_starlark_str() {
            s.get_hash()
        } else {
            self.to_value().get_hash()?
        };
        Ok(Hashed::new_unchecked(hash, self))
    }

    /// Convert to another `Value` wrapper.
    #[inline]
    pub fn to_value_of_unchecked(self) -> ValueOfUnchecked<'v, T> {
        ValueOfUnchecked::new(self.to_value())
    }
}

impl<'v, T: StarlarkValue<'v>> FrozenValueTyped<'v, T> {
    pub(crate) fn is_str() -> bool {
        T::static_type_id() == StarlarkStr::static_type_id()
    }

    pub(crate) fn is_pointer_i32() -> bool {
        PointerI32::type_is_pointer_i32::<T>()
    }

    /// Construct `FrozenValueTyped` without checking that the value is of correct type.
    #[inline]
    pub unsafe fn new_unchecked(value: FrozenValue) -> FrozenValueTyped<'v, T> {
        debug_assert!(value.downcast_ref::<T>().is_some());
        FrozenValueTyped(value, marker::PhantomData)
    }

    /// Downcast.
    #[inline]
    pub fn new(value: FrozenValue) -> Option<FrozenValueTyped<'v, T>> {
        value.downcast_ref::<T>()?;
        Some(FrozenValueTyped(value, marker::PhantomData))
    }

    /// Downcast.
    #[inline]
    pub fn new_err(value: FrozenValue) -> anyhow::Result<FrozenValueTyped<'v, T>> {
        value.downcast_ref_err::<T>()?;
        Ok(FrozenValueTyped(value, marker::PhantomData))
    }

    #[inline]
    pub(crate) fn new_repr<A: AValue<'v, StarlarkValue = T>>(
        repr: &'v AValueRepr<AValueImpl<'v, A>>,
    ) -> FrozenValueTyped<'v, T> {
        // drop lifetime: `FrozenValue` is not (yet) parameterized with lifetime.
        let header = unsafe { cast::ptr_lifetime(&repr.header) };
        FrozenValueTyped(FrozenValue::new_ptr(header, A::IS_STR), marker::PhantomData)
    }

    /// Erase the type.
    #[inline]
    pub fn to_frozen_value(self) -> FrozenValue {
        self.0
    }

    /// Convert to the value.
    #[inline]
    pub fn to_value(self) -> Value<'v> {
        self.0.to_value()
    }

    /// Convert to the value.
    #[inline]
    pub fn to_value_typed(self) -> ValueTyped<'v, T> {
        unsafe { ValueTyped::new_unchecked(self.0.to_value()) }
    }

    /// Get the reference to the pointed value.
    #[inline]
    pub fn as_ref(self) -> &'v T {
        if Self::is_pointer_i32() {
            unsafe { transmute!(&PointerI32, &T, self.0.0.unpack_pointer_i32_unchecked()) }
        } else if Self::is_str() {
            unsafe {
                self.0
                    .0
                    .unpack_ptr_no_int_unchecked()
                    .unpack_header_unchecked()
                    .payload::<T>()
            }
        } else {
            // When a frozen pointer is not str and not int,
            // unpack is does not need untagging.
            // This generates slightly more efficient machine code.
            unsafe {
                self.0
                    .0
                    .unpack_ptr_no_int_no_str_unchecked()
                    .unpack_header_unchecked()
                    .payload::<T>()
            }
        }
    }

    #[inline]
    pub(crate) fn as_frozen_ref(self) -> FrozenRef<'v, T> {
        FrozenRef::new(self.as_ref())
    }

    /// Convert to another `FrozenValue` wrapper.
    #[inline]
    pub fn to_value_of_unchecked(self) -> FrozenValueOfUnchecked<'v, T> {
        FrozenValueOfUnchecked::new(self.to_frozen_value())
    }
}

impl<'v> ValueTyped<'v, StarlarkStr> {
    /// Get the Rust string reference.
    #[inline]
    pub fn as_str(self) -> &'v str {
        self.as_ref().as_str()
    }
}

impl<'v> FrozenValueTyped<'v, StarlarkStr> {
    /// Get the Rust string reference.
    #[inline]
    pub fn as_str(self) -> &'v str {
        self.as_ref().as_str()
    }
}

unsafe impl<'v, T: StarlarkValue<'v>> Trace<'v> for ValueTyped<'v, T> {
    fn trace(&mut self, tracer: &Tracer<'v>) {
        tracer.trace(&mut self.0);
        // If type of value changed, dereference will produce the wrong object type.
        debug_assert!(self.0.downcast_ref::<T>().is_some());
    }
}

impl<'v, T: StarlarkValue<'v>> Deref for FrozenValueTyped<'v, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.as_ref()
    }
}

impl<'v, T: StarlarkValue<'v>> Deref for ValueTyped<'v, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.as_ref()
    }
}

impl<'v, T: StarlarkValue<'v>> StarlarkTypeRepr for ValueTyped<'v, T> {
    type Canonical = <T as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        T::starlark_type_repr()
    }
}

impl<'v, T: StarlarkValue<'v>> UnpackValue<'v> for ValueTyped<'v, T> {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        Ok(ValueTyped::new(value))
    }
}

impl<'v, T: StarlarkValue<'v>> AllocValue<'v> for ValueTyped<'v, T> {
    fn alloc_value(self, _heap: &'v Heap) -> Value<'v> {
        self.0
    }
}

impl<'v> AllocStringValue<'v> for StringValue<'v> {
    fn alloc_string_value(self, _heap: &'v Heap) -> StringValue<'v> {
        self
    }
}

impl<'v, T: StarlarkValue<'v>> StarlarkTypeRepr for FrozenValueTyped<'v, T> {
    type Canonical = <T as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        T::starlark_type_repr()
    }
}

impl<'v, T: StarlarkValue<'v>> UnpackValue<'v> for FrozenValueTyped<'v, T> {
    type Error = crate::Error;

    fn unpack_value_impl(value: Value<'v>) -> crate::Result<Option<Self>> {
        if let Some(value) = value.unpack_frozen() {
            if let Some(value) = FrozenValueTyped::new(value) {
                return Ok(Some(value));
            }
        } else if StarlarkTypeId::of::<T>() == value.vtable().starlark_type_id {
            #[derive(thiserror::Error, Debug)]
            #[error("Expected frozen value of type `{expected}`, got unfrozen: `{value}`")]
            struct NotFrozenError {
                expected: Ty,
                value: String,
            }

            return Err(crate::Error::new_value(NotFrozenError {
                expected: T::starlark_type_repr(),
                value: value.to_string_for_type_error(),
            }));
        }

        Ok(None)
    }
}

impl<'v, 'f, T: StarlarkValue<'f>> AllocValue<'v> for FrozenValueTyped<'f, T> {
    fn alloc_value(self, _heap: &'v Heap) -> Value<'v> {
        self.0.to_value()
    }
}

impl<'v> AllocStringValue<'v> for FrozenStringValue {
    fn alloc_string_value(self, _heap: &'v Heap) -> StringValue<'v> {
        self.to_string_value()
    }
}

impl<'v, T: StarlarkValue<'v>> AllocFrozenValue for FrozenValueTyped<'v, T> {
    fn alloc_frozen_value(self, _heap: &FrozenHeap) -> FrozenValue {
        self.0
    }
}

impl AllocFrozenStringValue for FrozenStringValue {
    fn alloc_frozen_string_value(self, _heap: &FrozenHeap) -> FrozenStringValue {
        self
    }
}

#[cfg(test)]
mod tests {
    use starlark_derive::starlark_module;

    use crate as starlark;
    use crate::assert::Assert;
    use crate::environment::GlobalsBuilder;
    use crate::tests::util::TestComplexValue;
    use crate::values::int::pointer_i32::PointerI32;
    use crate::values::none::NoneType;
    use crate::values::FrozenValue;
    use crate::values::FrozenValueTyped;
    use crate::values::Value;

    #[test]
    fn int() {
        let v = FrozenValueTyped::<PointerI32>::new(FrozenValue::testing_new_int(17)).unwrap();
        assert_eq!(17, v.as_ref().get().to_i32());
    }

    #[test]
    fn test_unpack_value_for_frozen_value_typed() {
        #[starlark_module]
        fn module(globals: &mut GlobalsBuilder) {
            fn mutable<'v>() -> anyhow::Result<TestComplexValue<Value<'v>>> {
                Ok(TestComplexValue(Value::new_none()))
            }

            const FROZEN: TestComplexValue<FrozenValue> = TestComplexValue(FrozenValue::new_none());

            fn takes_frozen_value_typed<'v>(
                value: FrozenValueTyped<'v, TestComplexValue<FrozenValue>>,
            ) -> anyhow::Result<NoneType> {
                let _ = value;
                Ok(NoneType)
            }
        }

        let mut a = Assert::new();
        a.globals_add(module);

        a.pass("takes_frozen_value_typed(FROZEN)");
        a.fail("takes_frozen_value_typed(1)", "Type of parameter `value` doesn't match, expected `TestComplexValue`, actual `int (repr: 1)`");
        a.fail(
            "takes_frozen_value_typed(mutable())",
            "Expected frozen value",
        );
    }
}
