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

// Possible optimisations:
// Avoid the Box duplication
// Encode Int in the pointer too

// We use pointer tagging on the bottom two bits:
// 00 => this Value pointer is actually a FrozenValue pointer
// 01 => this is a real Value pointer
// 11 => this is a bool (next bit: 1 => true, 0 => false)
// 10 => this is a None
//
// We don't use pointer tagging for Int (although we'd like to), because
// our val_ref requires a pointer to the value. We need to put that pointer
// somewhere. The solution is to have a separate value storage vs vtable.

use std::any;
use std::cmp::Ordering;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;

use allocative::Allocative;
use dupe::Clone_;
use dupe::Copy_;
use dupe::Dupe;
use dupe::Dupe_;
use dupe::IterDupedExt;
use dupe::OptionDupedExt;
use either::Either;
use num_bigint::BigInt;
use serde::Serialize;
use serde::Serializer;
use starlark_map::Equivalent;
use starlark_syntax::value_error;

use crate as starlark;
use crate::any::AnyLifetime;
use crate::any::ProvidesStaticType;
use crate::cast::transmute;
use crate::coerce::Coerce;
use crate::coerce::CoerceKey;
use crate::collections::Hashed;
use crate::collections::StarlarkHashValue;
use crate::collections::StarlarkHasher;
use crate::docs::DocItem;
use crate::eval::compiler::def::Def;
use crate::eval::compiler::def::FrozenDef;
use crate::eval::runtime::arguments::ArgumentsFull;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::eval::Arguments;
use crate::eval::Evaluator;
use crate::eval::ParametersSpec;
use crate::sealed::Sealed;
use crate::typing::ParamIsRequired;
use crate::typing::ParamSpec;
use crate::typing::Ty;
use crate::typing::TyCallable;
use crate::util::ArcStr;
use crate::values::bool::value::VALUE_FALSE_TRUE;
use crate::values::demand::request_value_impl;
use crate::values::dict::value::VALUE_EMPTY_FROZEN_DICT;
use crate::values::dict::FrozenDictRef;
use crate::values::enumeration::EnumType;
use crate::values::enumeration::FrozenEnumValue;
use crate::values::function::FrozenBoundMethod;
use crate::values::function::NativeFunction;
use crate::values::function::FUNCTION_TYPE;
use crate::values::int::pointer_i32::PointerI32;
use crate::values::iter::StarlarkIterator;
use crate::values::layout::avalue::AValue;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::heap::repr::AValueHeader;
use crate::values::layout::heap::repr::AValueOrForwardUnpack;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::layout::pointer::FrozenPointer;
use crate::values::layout::pointer::Pointer;
use crate::values::layout::pointer::RawPointer;
use crate::values::layout::static_string::VALUE_EMPTY_STRING;
use crate::values::layout::typed::string::StringValueLike;
use crate::values::layout::value_lifetimeless::ValueLifetimeless;
use crate::values::layout::vtable::AValueDyn;
use crate::values::layout::vtable::AValueDynFull;
use crate::values::layout::vtable::AValueVTable;
use crate::values::list::value::VALUE_EMPTY_FROZEN_LIST;
use crate::values::none::none_type::VALUE_NONE;
use crate::values::range::Range;
use crate::values::record::instance::FrozenRecord;
use crate::values::record::record_type::RecordType;
use crate::values::recursive_repr_or_json_guard::json_stack_push;
use crate::values::recursive_repr_or_json_guard::repr_stack_push;
use crate::values::stack_guard;
use crate::values::starlark_type_id::StarlarkTypeId;
use crate::values::string::str_type::StarlarkStr;
use crate::values::structs::value::FrozenStruct;
use crate::values::tuple::value::VALUE_EMPTY_TUPLE;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::int::inline_int::InlineInt;
use crate::values::types::int::int_or_big::StarlarkIntRef;
use crate::values::types::list::value::FrozenListData;
use crate::values::types::num::value::NumRef;
use crate::values::types::tuple::value::FrozenTuple;
use crate::values::types::tuple::value::Tuple;
use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenRef;
use crate::values::FrozenStringValue;
use crate::values::FrozenValueTyped;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::StringValue;
use crate::values::Trace;
use crate::values::UnpackValue;
use crate::values::ValueError;
use crate::values::ValueIdentity;

// We already import another `ValueError`, hence the odd name.
#[derive(Debug, thiserror::Error)]
enum ValueValueError {
    #[error("Expected value of type `{0}` but got `{1}`")]
    WrongType(&'static str, String),
}

/// A Starlark value. The lifetime argument `'v` corresponds to the [`Heap`](crate::values::Heap) it is stored on.
///
/// Many of the methods simply forward to the underlying [`StarlarkValue`](crate::values::StarlarkValue).
/// The [`Display`](std::fmt::Display) trait is equivalent to the `repr()` function in Starlark.
#[derive(Clone_, Copy_, Dupe_, ProvidesStaticType, Allocative)]
#[allocative(skip)] // Value is owned by heap.
// One possible change: moving to Forward during GC.
pub struct Value<'v>(pub(crate) Pointer<'v>);

unsafe impl<'v> Coerce<Value<'v>> for Value<'v> {}
unsafe impl<'v> CoerceKey<Value<'v>> for Value<'v> {}
unsafe impl<'v> Coerce<Value<'v>> for FrozenValue {}
unsafe impl<'v> CoerceKey<Value<'v>> for FrozenValue {}

impl Default for Value<'_> {
    fn default() -> Self {
        Self::new_none()
    }
}

impl Default for FrozenValue {
    fn default() -> Self {
        Self::new_none()
    }
}

impl Display for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match repr_stack_push(*self) {
            Ok(_guard) => {
                // We want to reuse Display for `repr`, so that means that
                // strings must display "with quotes", so we get everything consistent.
                Display::fmt(self.get_ref().as_display(), f)
            }
            Err(..) => {
                let mut recursive = String::new();
                self.get_ref().collect_repr_cycle(&mut recursive);
                write!(f, "{}", recursive)
            }
        }
    }
}

impl Display for FrozenValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.to_value(), f)
    }
}

fn debug_value(typ: &str, v: Value, f: &mut fmt::Formatter) -> fmt::Result {
    // When value is being moved during GC or freeze,
    // `Value` pointee is not a proper value, but a GC-related information.
    // Regular operations like `.to_repr()` crash, but `Debug` should work.
    if let Some(x) = v.0.unpack_ptr() {
        if let AValueOrForwardUnpack::Forward(fwd) = x.unpack() {
            return f.debug_tuple(typ).field(&fwd).finish();
        }
    }
    f.debug_tuple(typ).field(v.get_ref().as_debug()).finish()
}

impl Debug for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_value("Value", *self, f)
    }
}

impl Debug for FrozenValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_value("FrozenValue", Value::new_frozen(*self), f)
    }
}

impl<'v> PartialEq for Value<'v> {
    fn eq(&self, other: &Value<'v>) -> bool {
        self.equals(*other).ok() == Some(true)
    }
}

impl PartialEq for FrozenValue {
    fn eq(&self, other: &FrozenValue) -> bool {
        self.to_value().eq(&other.to_value())
    }
}

impl Eq for Value<'_> {}

impl Eq for FrozenValue {}

impl Equivalent<FrozenValue> for Value<'_> {
    fn equivalent(&self, key: &FrozenValue) -> bool {
        key.equals(*self).unwrap()
    }
}

impl Equivalent<Value<'_>> for FrozenValue {
    fn equivalent(&self, key: &Value) -> bool {
        self.equals(*key).unwrap()
    }
}

/// A [`Value`] that can never be changed. Can be converted back to a [`Value`] with [`to_value`](FrozenValue::to_value).
///
/// A [`FrozenValue`] exists on a [`FrozenHeap`](crate::values::FrozenHeap), which in turn can be kept
/// alive by a [`FrozenHeapRef`](crate::values::FrozenHeapRef). If the frozen heap gets dropped
/// while a [`FrozenValue`] from it still exists, the program will probably segfault, so be careful
/// when working directly with [`FrozenValue`]s. See the type [`OwnedFrozenValue`](crate::values::OwnedFrozenValue)
/// for a little bit more safety.
#[derive(Clone, Copy, Dupe, ProvidesStaticType, Allocative)]
// One possible change: moving from Blackhole during GC
pub struct FrozenValue(
    #[allocative(skip)] // Because it is owned by the heap.
    pub(crate)  FrozenPointer<'static>,
);

// These can both be shared, but not obviously, because we hide a fake RefCell in Pointer to stop
// it having variance.
unsafe impl Send for FrozenValue {}
unsafe impl Sync for FrozenValue {}

#[derive(thiserror::Error, Debug)]
#[error("Integer value is too big to fit in {integer_type}: {value}")]
pub(crate) struct IntegerTooBigError {
    pub(crate) integer_type: &'static str,
    pub(crate) value: String,
}

impl<'v> Value<'v> {
    #[inline]
    pub(crate) fn new_ptr(x: &'v AValueHeader, is_str: bool) -> Self {
        Self(Pointer::new_unfrozen(x, is_str))
    }

    #[inline]
    pub(crate) fn new_ptr_query_is_str(x: &'v AValueHeader) -> Self {
        let is_string = x.0.is_str;
        Self::new_ptr(x, is_string)
    }

    #[inline]
    pub(crate) fn new_repr<T: AValue<'v>>(x: &'v AValueRepr<AValueImpl<'v, T>>) -> Self {
        Self::new_ptr(&x.header, T::IS_STR)
    }

    #[inline]
    pub(crate) unsafe fn new_ptr_usize_with_str_tag(x: usize) -> Self {
        Self(Pointer::new_unfrozen_usize_with_str_tag(x))
    }

    #[inline]
    pub(crate) unsafe fn cast_lifetime<'w>(self) -> Value<'w> {
        Value(self.0.cast_lifetime())
    }

    /// Create a new `None` value.
    #[inline]
    pub fn new_none() -> Self {
        FrozenValue::new_none().to_value()
    }

    /// Create a new boolean.
    #[inline]
    pub fn new_bool(x: bool) -> Self {
        FrozenValue::new_bool(x).to_value()
    }

    /// Create a new integer.
    #[inline]
    pub(crate) fn new_int(x: InlineInt) -> Self {
        FrozenValue::new_int(x).to_value()
    }

    #[cfg(test)]
    pub(crate) fn testing_new_int(x: i32) -> Self {
        FrozenValue::testing_new_int(x).to_value()
    }

    /// Create a new blank string.
    #[inline]
    pub(crate) fn new_empty_string() -> Self {
        FrozenValue::new_empty_string().to_value()
    }

    /// Create a new empty tuple.
    #[inline]
    pub(crate) fn new_empty_tuple() -> Self {
        FrozenValue::new_empty_tuple().to_value()
    }

    /// Turn a [`FrozenValue`] into a [`Value`]. See the safety warnings on
    /// [`OwnedFrozenValue`](crate::values::OwnedFrozenValue).
    #[inline]
    pub fn new_frozen(x: FrozenValue) -> Self {
        // Safe if every FrozenValue must have had a reference added to its heap first.
        // That property is NOT statically checked.
        Self(x.0.to_pointer())
    }

    /// Obtain the underlying [`FrozenValue`] from inside the [`Value`], if it is one.
    #[inline]
    pub fn unpack_frozen(self) -> Option<FrozenValue> {
        if self.0.is_unfrozen() {
            None
        } else {
            // SAFETY: We've just checked the value is frozen.
            unsafe { Some(self.unpack_frozen_unchecked()) }
        }
    }

    #[inline]
    unsafe fn unpack_frozen_unchecked(self) -> FrozenValue {
        debug_assert!(!self.0.is_unfrozen());
        FrozenValue(self.0.cast_lifetime().to_frozen_pointer_unchecked())
    }

    /// Is this value `None`.
    #[inline]
    pub fn is_none(self) -> bool {
        self.ptr_eq(Value::new_none())
    }

    /// Obtain the underlying numerical value, if it is one.
    pub(crate) fn unpack_num(self) -> Option<NumRef<'v>> {
        if let Some(int) = StarlarkIntRef::unpack(self) {
            Some(NumRef::Int(int))
        } else if let Some(float) = self.downcast_ref() {
            Some(NumRef::Float(*float))
        } else {
            None
        }
    }

    pub(crate) fn unpack_integer<I>(self) -> crate::Result<Option<I>>
    where
        I: TryFrom<i32>,
        I: TryFrom<&'v BigInt>,
    {
        let Some(num) = StarlarkIntRef::unpack_value_opt(self) else {
            return Ok(None);
        };
        let option = match num {
            StarlarkIntRef::Small(x) => I::try_from(x.to_i32()).ok(),
            StarlarkIntRef::Big(x) => x.unpack_integer(),
        };
        match option {
            Some(i) => Ok(Some(i)),
            None => Err(crate::Error::new_value(IntegerTooBigError {
                integer_type: any::type_name::<I>(),
                value: num.to_string(),
            })),
        }
    }

    /// Obtain the underlying `bool` if it is a boolean.
    #[inline]
    pub fn unpack_bool(self) -> Option<bool> {
        if self.ptr_eq(Value::new_bool(true)) {
            Some(true)
        } else if self.ptr_eq(Value::new_bool(false)) {
            Some(false)
        } else {
            None
        }
    }

    /// Obtain the underlying integer if it fits in an `i32`.
    /// Note floats are not considered integers, i. e. `unpack_i32` for `1.0` will return `None`.
    #[inline]
    pub fn unpack_i32(self) -> Option<i32> {
        if InlineInt::smaller_than_i32() {
            StarlarkIntRef::unpack(self)?.to_i32()
        } else {
            self.unpack_inline_int().map(|i| i.to_i32())
        }
    }

    #[inline]
    pub(crate) fn unpack_inline_int(self) -> Option<InlineInt> {
        self.0.unpack_int()
    }

    #[inline]
    pub(crate) fn unpack_int_value(self) -> Option<FrozenValueTyped<'static, PointerI32>> {
        if self.unpack_inline_int().is_some() {
            // SAFETY: We've just checked the value is an int.
            unsafe {
                Some(FrozenValueTyped::new_unchecked(
                    self.unpack_frozen_unchecked(),
                ))
            }
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn is_str(self) -> bool {
        self.0.is_str()
    }

    /// Like [`unpack_str`](Value::unpack_str), but gives a pointer to a boxed string.
    /// Mostly useful for when you want to convert the string to a `dyn` trait, but can't
    /// form a `dyn` of an unsized type.
    ///
    /// Unstable and likely to be removed in future, as the presence of the `Box` is
    /// not a guaranteed part of the API.
    #[inline]
    pub fn unpack_starlark_str(self) -> Option<&'v StarlarkStr> {
        if self.is_str() {
            unsafe {
                Some(
                    &self
                        .0
                        .unpack_ptr_no_int_unchecked()
                        .unpack_header_unchecked()
                        .as_repr::<StarlarkStr>()
                        .payload,
                )
            }
        } else {
            None
        }
    }

    /// Obtain the underlying `str` if it is a string.
    #[inline]
    pub fn unpack_str(self) -> Option<&'v str> {
        self.unpack_starlark_str().map(|s| s.as_str())
    }

    /// Get a pointer to a [`AValue`].
    #[inline]
    pub(crate) fn get_ref(self) -> AValueDyn<'v> {
        unsafe {
            match self.0.unpack() {
                Either::Left(x) => x.unpack_header_unchecked().unpack(),
                Either::Right(x) => x.as_avalue_dyn(),
            }
        }
    }

    #[inline]
    fn get_ref_full(self) -> AValueDynFull<'v> {
        unsafe { AValueDynFull::new(self.get_ref(), self) }
    }

    #[inline]
    pub(crate) fn vtable(self) -> &'static AValueVTable {
        unsafe {
            match self.0.unpack() {
                Either::Left(x) => x.unpack_header_unchecked().0,
                Either::Right(_) => PointerI32::vtable(),
            }
        }
    }

    /// Downcast without checking the value type.
    #[inline]
    pub(crate) unsafe fn downcast_ref_unchecked<T: StarlarkValue<'v>>(self) -> &'v T {
        debug_assert!(self.get_ref().downcast_ref::<T>().is_some());
        if PointerI32::type_is_pointer_i32::<T>() {
            transmute!(&PointerI32, &T, self.0.unpack_pointer_i32_unchecked())
        } else {
            self.0
                .unpack_ptr_no_int_unchecked()
                .unpack_header_unchecked()
                .payload()
        }
    }

    pub(crate) fn get_hash(self) -> crate::Result<StarlarkHashValue> {
        self.get_ref().get_hash()
    }

    /// Are two [`Value`]s equal, looking at only their underlying pointer. This function is
    /// low-level and provides two guarantees.
    ///
    /// 1. It is _reflexive_, the same [`Value`] passed as both arguments will result in [`true`].
    /// 2. If this function is [`true`], then [`Value::equals`] will also consider them equal.
    ///
    /// Note that other properties are not guaranteed, and the result is not considered part of the API.
    /// The result can be impacted by optimisations such as hash-consing, copy-on-write, partial
    /// evaluation etc.
    #[inline]
    pub fn ptr_eq(self, other: Value) -> bool {
        self.0.ptr_eq(other.0)
    }

    /// Returns an identity for this [`Value`], derived from its pointer. This function is
    /// low-level and provides two guarantees. Those are valid until the next GC:
    ///
    /// 1. Calling it multiple times on the same [`Value`]  will return [`ValueIdentity`] that
    ///    compare equal.
    /// 2. If two [`Value]` have [`ValueIdentity`]  that compare equal, then [`Value::ptr_eq`] and
    ///    [`Value::equals`]  will also consider them to be equal.
    #[inline]
    pub fn identity(self) -> ValueIdentity<'v> {
        ValueIdentity::new(self)
    }

    /// Get the underlying pointer.
    /// Should be done sparingly as it slightly breaks the abstraction.
    /// Most useful as a hash key based on pointer.
    /// For external users, `Value::identity` returns an opaque `ValueIdentity` that makes fewer
    /// guarantees.
    #[inline]
    pub(crate) fn ptr_value(self) -> RawPointer {
        self.0.raw()
    }

    /// `type(x)`.
    pub fn get_type(self) -> &'static str {
        self.vtable().type_name
    }

    /// `bool(x)`.
    pub fn to_bool(self) -> bool {
        // Fast path for the common case
        if let Some(x) = self.unpack_bool() {
            x
        } else {
            self.get_ref().to_bool()
        }
    }

    /// `x[index]`.
    pub fn at(self, index: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        self.get_ref().at(index, heap)
    }

    /// `x[start:stop:stride]`.
    pub fn slice(
        self,
        start: Option<Value<'v>>,
        stop: Option<Value<'v>>,
        stride: Option<Value<'v>>,
        heap: &'v Heap,
    ) -> crate::Result<Value<'v>> {
        self.get_ref().slice(start, stop, stride, heap)
    }

    /// `len(x)`.
    pub fn length(self) -> crate::Result<i32> {
        self.get_ref().length()
    }

    /// `other in x`.
    pub fn is_in(self, other: Value<'v>) -> crate::Result<bool> {
        self.get_ref().is_in(other)
    }

    /// `+x`.
    pub fn plus(self, heap: &'v Heap) -> crate::Result<Value<'v>> {
        self.get_ref().plus(heap)
    }

    /// `-x`.
    pub fn minus(self, heap: &'v Heap) -> crate::Result<Value<'v>> {
        self.get_ref().minus(heap)
    }

    /// `x - other`.
    pub fn sub(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        self.get_ref().sub(other, heap)
    }

    /// `x * other`.
    pub fn mul(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        if let Some(r) = self.get_ref().mul(other, heap) {
            r
        } else if let Some(r) = other.get_ref().rmul(self, heap) {
            r
        } else {
            ValueError::unsupported_owned(self.get_type(), "*", Some(other.get_type()))
        }
    }

    /// `x % other`.
    pub fn percent(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        self.get_ref().percent(other, heap)
    }

    /// `x / other`.
    pub fn div(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        self.get_ref().div(other, heap)
    }

    /// `x // other`.
    pub fn floor_div(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        self.get_ref().floor_div(other, heap)
    }

    /// `x & other`.
    pub fn bit_and(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        self.get_ref().bit_and(other, heap)
    }

    /// `x | other`.
    pub fn bit_or(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        self.get_ref().bit_or(other, heap)
    }

    /// `x ^ other`.
    pub fn bit_xor(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        self.get_ref().bit_xor(other, heap)
    }

    /// `~x`.
    pub fn bit_not(self, heap: &'v Heap) -> crate::Result<Value<'v>> {
        self.get_ref().bit_not(heap)
    }

    /// `x << other`.
    pub fn left_shift(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        self.get_ref().left_shift(other, heap)
    }

    /// `x >> other`.
    pub fn right_shift(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        self.get_ref().right_shift(other, heap)
    }

    pub(crate) fn invoke_with_loc(
        self,
        location: Option<FrozenRef<'static, FrameSpan>>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        eval.with_call_stack(self, location, |eval| {
            self.get_ref_full().invoke(args, eval)
        })
    }

    /// Callable parameters if known.
    ///
    /// For now it only returns parameter spec for `def` and `lambda`.
    pub fn parameters_spec(self) -> Option<&'v ParametersSpec<Value<'v>>> {
        if let Some(def) = self.downcast_ref::<Def>() {
            Some(&def.parameters)
        } else if let Some(def) = self.downcast_ref::<FrozenDef>() {
            Some(def.parameters.as_value())
        } else {
            None
        }
    }

    /// Invoke self with given arguments.
    pub(crate) fn invoke(
        self,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        self.invoke_with_loc(None, args, eval)
    }

    /// Invoke a function with only positional arguments.
    pub(crate) fn invoke_pos(
        self,
        pos: &[Value<'v>],
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        let params = Arguments(ArgumentsFull {
            pos,
            ..ArgumentsFull::default()
        });
        self.invoke(&params, eval)
    }

    fn check_callable(self) -> crate::Result<()> {
        if !self.vtable().starlark_value.HAS_invoke {
            return Err(value_error!(
                "Value is not callable: {}",
                self.to_string_for_type_error()
            ));
        }
        Ok(())
    }

    /// Check this value can be "called" with given parameter types, and provided return type.
    ///
    /// This check is done optimistically: when it is not known
    /// whether the value is compatible with given arguments, return `Ok(())`.
    ///
    /// This operation is expensive.
    pub fn check_callable_with<'a>(
        self,
        pos: impl IntoIterator<Item = &'a Ty>,
        named: impl IntoIterator<Item = (&'a str, &'a Ty)>,
        args: Option<&Ty>,
        kwargs: Option<&Ty>,
        ret: &Ty,
    ) -> crate::Result<()> {
        let pos = Vec::from_iter(pos);
        let named = Vec::from_iter(named);
        self.check_callable_with_impl(&pos, &named, args, kwargs, ret)
    }

    fn check_callable_with_impl<'a>(
        self,
        pos: &[&Ty],
        named: &[(&'a str, &Ty)],
        args: Option<&Ty>,
        kwargs: Option<&Ty>,
        ret: &Ty,
    ) -> crate::Result<()> {
        // First, provide a good error message when the value is not callable
        // without invoking a typechecker.
        self.check_callable()?;

        let sig = TyCallable::new(
            ParamSpec::new_parts(
                pos.iter().map(|ty| (ParamIsRequired::Yes, (*ty).dupe())),
                [],
                args.duped(),
                named
                    .iter()
                    .map(|(n, ty)| (ArcStr::from(*n), ParamIsRequired::Yes, (*ty).dupe())),
                kwargs.duped(),
            )?,
            ret.dupe(),
        );

        let ty = Ty::of_value(self);
        if !ty.check_call(
            pos.iter().copied().duped(),
            named.iter().map(|(n, ty)| (*n, (*ty).dupe())),
            args.duped(),
            kwargs.duped(),
            ret.dupe(),
        ) {
            return Err(value_error!(
                "Value `{}` is not compatible with the signature `{}`",
                self.to_string_for_type_error(),
                sig
            ));
        }

        Ok(())
    }

    /// `type(x)`.
    pub fn get_type_value(self) -> FrozenStringValue {
        self.vtable().type_value()
    }

    /// See documentation of [`StarlarkTypeId`].
    #[inline]
    pub(crate) fn starlark_type_id(self) -> StarlarkTypeId {
        self.vtable().starlark_type_id
    }

    /// The literal string that a user would need to use this in type annotations.
    pub(crate) fn get_type_starlark_repr(self) -> Ty {
        self.vtable().type_starlark_repr()
    }

    /// Add two [`Value`]s together. Will first try using [`add`](StarlarkValue::add),
    /// before falling back to [`radd`](StarlarkValue::radd).
    pub fn add(self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        // Fast special case for ints.
        if let Some(ls) = self.unpack_inline_int() {
            if let Some(rs) = other.unpack_inline_int() {
                // On overflow take the slow path below.
                if let Some(sum) = ls.checked_add(rs) {
                    return Ok(heap.alloc(sum));
                }
            }
        }

        // Addition of string is super common and pretty cheap, so have a special case for it.
        if let Some(ls) = self.unpack_str() {
            if let Some(rs) = other.unpack_str() {
                if ls.is_empty() {
                    return Ok(other);
                } else if rs.is_empty() {
                    return Ok(self);
                } else {
                    return Ok(heap.alloc_str_concat(ls, rs).to_value());
                }
            }
        }

        if let Some(v) = self.get_ref().add(other, heap) {
            v
        } else if let Some(v) = other.get_ref().radd(self, heap) {
            v
        } else {
            ValueError::unsupported_owned(self.get_type(), "+", Some(other.get_type()))
        }
    }

    /// Convert a value to a [`FrozenValue`] using a supplied [`Freezer`].
    pub fn freeze(self, freezer: &Freezer) -> FreezeResult<FrozenValue> {
        freezer.freeze(self)
    }

    /// Implement the `str()` function - converts a string value to itself,
    /// otherwise uses `repr()`.
    pub fn to_str(self) -> String {
        match self.unpack_str() {
            None => self.to_repr(),
            Some(s) => s.to_owned(),
        }
    }

    /// Implement the `repr()` function.
    pub fn to_repr(self) -> String {
        let mut s = String::new();
        self.collect_repr(&mut s);
        s
    }

    pub(crate) fn name_for_call_stack(self) -> String {
        self.get_ref().name_for_call_stack(self)
    }

    /// Convert the value to JSON.
    ///
    /// Return an error if the value or any contained value does not support conversion to JSON.
    pub fn to_json(self) -> anyhow::Result<String> {
        serde_json::to_string(&self).map_err(|e| anyhow::anyhow!(e))
    }

    /// Convert the value to JSON value.
    pub fn to_json_value(self) -> anyhow::Result<serde_json::Value> {
        serde_json::to_value(self).map_err(|e| anyhow::anyhow!(e))
    }

    /// Forwards to [`StarlarkValue::set_attr`].
    pub fn set_attr(self, attribute: &str, alloc_value: Value<'v>) -> crate::Result<()> {
        self.get_ref().set_attr(attribute, alloc_value)
    }

    /// Forwards to [`StarlarkValue::set_at`].
    pub fn set_at(self, index: Value<'v>, alloc_value: Value<'v>) -> crate::Result<()> {
        self.get_ref().set_at(index, alloc_value)
    }

    /// Forwards to [`StarlarkValue::documentation`].
    pub fn documentation(self) -> DocItem {
        self.get_ref().documentation()
    }

    /// Produce an iterable from a value.
    #[inline]
    pub fn iterate(self, heap: &'v Heap) -> crate::Result<StarlarkIterator<'v>> {
        let iter = self.get_ref().iterate(self, heap)?;
        Ok(StarlarkIterator::new(iter, heap))
    }

    /// Get the [`Hashed`] version of this [`Value`].
    #[inline]
    pub fn get_hashed(self) -> crate::Result<Hashed<Self>> {
        ValueLike::get_hashed(self)
    }

    /// Are two values equal. If the values are of different types it will
    /// return [`false`]. It will only error if there is excessive recursion.
    #[inline]
    pub fn equals(self, other: Value<'v>) -> crate::Result<bool> {
        if self.ptr_eq(other) {
            Ok(true)
        } else {
            // Condition and then branch are cheap, but else branch is not.
            // Split it so the compiler could inline this function
            // without hitting the inlining limit.
            self.equals_not_ptr_eq(other)
        }
    }

    #[inline]
    fn equals_not_ptr_eq(self, other: Value<'v>) -> crate::Result<bool> {
        let _guard = stack_guard::stack_guard()?;
        self.get_ref().equals(other)
    }

    /// How are two values comparable. For values of different types will return [`Err`].
    #[inline]
    pub fn compare(self, other: Value<'v>) -> crate::Result<Ordering> {
        ValueLike::compare(self, other)
    }

    /// Describe the value, in order to get its metadata in a way that could be used
    /// to generate prototypes, help information or whatever other descriptive text
    /// is required.
    /// Plan is to make this return a data type at some point in the future, possibly
    /// move on to `StarlarkValue` and include data from members.
    pub fn describe(self, name: &str) -> String {
        if self.get_type() == FUNCTION_TYPE {
            format!("def {}: pass", self.to_repr().replace(" = ...", " = None"))
        } else {
            format!("# {} = {}", name, self.to_repr())
        }
    }

    /// Call `export_as` on the underlying value, but only if the type is mutable.
    /// Otherwise, does nothing.
    pub fn export_as(
        self,
        variable_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<()> {
        self.get_ref().export_as(variable_name, eval)
    }

    /// Return the attribute with the given name.
    pub fn get_attr(self, attribute: &str, heap: &'v Heap) -> crate::Result<Option<Value<'v>>> {
        let aref = self.get_ref();
        if let Some(methods) = aref.vtable().methods() {
            let attribute = Hashed::new(attribute);
            if let Some(v) = methods.get_hashed(attribute) {
                return Ok(Some(v.bind(self, heap)?));
            }
            Ok(aref.get_attr_hashed(attribute, heap))
        } else {
            Ok(aref.get_attr(attribute, heap))
        }
    }

    /// Like `get_attr` but return an error if the attribute is not available.
    pub fn get_attr_error(self, attribute: &str, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match self.get_attr(attribute, heap)? {
            None => {
                ValueError::unsupported_owned(self.get_type(), &format!(".{}", attribute), None)
            }
            Some(x) => Ok(x),
        }
    }

    /// Query whether an attribute exists on a type. Should be equivalent to whether
    /// [`get_attr`](Value::get_attr) succeeds, but potentially more efficient.
    pub fn has_attr(self, attribute: &str, heap: &'v Heap) -> bool {
        let aref = self.get_ref();
        if let Some(methods) = aref.vtable().methods() {
            if methods.get(attribute).is_some() {
                return true;
            }
        }
        aref.has_attr(attribute, heap)
    }

    /// Get a list of all the attributes this function supports, used to implement the
    /// `dir()` function.
    pub fn dir_attr(self) -> Vec<String> {
        let aref = self.get_ref();
        let mut result = if let Some(methods) = aref.vtable().methods() {
            let mut res = methods.names();
            res.extend(aref.dir_attr());
            res
        } else {
            aref.dir_attr()
        };
        result.sort();
        result
    }

    /// Request a value provided by [`StarlarkValue::provide`].
    pub fn request_value<T: AnyLifetime<'v>>(self) -> Option<T> {
        request_value_impl(self)
    }

    #[cold]
    fn display_for_type_error(self) -> impl Display + 'v {
        fn split_at_safe(s: &str, index: usize) -> (&str, &str) {
            for index in index..s.len() {
                if s.is_char_boundary(index) {
                    return s.split_at(index);
                }
            }
            (s, "")
        }

        struct DisplayWithTypeImpl<'v>(Value<'v>);

        impl<'v> Display for DisplayWithTypeImpl<'v> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let mut repr = self.0.to_repr();

                let max_len = 60usize;

                if repr.len() > max_len && repr.chars().count() > max_len {
                    let truncated = "<<...>>";

                    // 1/3 from back, 2/3 from front, because front is usually more interesting.
                    let take_from_back = max_len.saturating_sub(truncated.len()) / 3;
                    let take_from_front = take_from_back * 2;

                    // Resulting repr is approximately `max_len` long.
                    repr = format!(
                        "{}{}{}",
                        split_at_safe(&repr, take_from_front).0,
                        truncated,
                        split_at_safe(&repr, repr.len().saturating_sub(take_from_back)).1
                    );
                }

                write!(f, "{} (repr: {})", self.0.get_type(), repr)
            }
        }

        DisplayWithTypeImpl(self)
    }

    /// Return a string usable for error messages.
    ///
    /// If the value is too large, it may be truncated.
    #[cold]
    pub fn to_string_for_type_error(self) -> String {
        self.display_for_type_error().to_string()
    }
}

impl FrozenValue {
    #[inline]
    pub(crate) fn new_ptr(x: &'static AValueHeader, is_str: bool) -> Self {
        Self(FrozenPointer::new_frozen(x, is_str))
    }

    #[inline]
    pub(crate) fn new_ptr_query_is_str(x: &'static AValueHeader) -> Self {
        let is_string = x.0.is_str;
        Self::new_ptr(x, is_string)
    }

    #[inline]
    pub(crate) fn new_repr<'a, T: AValue<'a>>(x: &'static AValueRepr<AValueImpl<'a, T>>) -> Self {
        Self::new_ptr(&x.header, T::IS_STR)
    }

    #[inline]
    pub(crate) fn new_ptr_usize_with_str_tag(x: usize) -> Self {
        Self(FrozenPointer::new_frozen_usize_with_str_tag(x))
    }

    /// Create a new value representing `None` in Starlark.
    #[inline]
    pub fn new_none() -> Self {
        Self::new_repr(&VALUE_NONE)
    }

    /// Create a new boolean in Starlark.
    #[inline]
    pub fn new_bool(x: bool) -> Self {
        // Implemented by indexing into a static so that
        // the compiler makes this function branchless.
        Self::new_repr(&VALUE_FALSE_TRUE[x as usize])
    }

    /// Create a new int in Starlark.
    #[inline]
    pub(crate) fn new_int(x: InlineInt) -> Self {
        Self(FrozenPointer::new_int(x))
    }

    #[cfg(test)]
    pub(crate) fn testing_new_int(x: i32) -> Self {
        Self::new_int(InlineInt::try_from(x).ok().unwrap())
    }

    /// Create a new empty string.
    #[inline]
    pub(crate) fn new_empty_string() -> Self {
        VALUE_EMPTY_STRING.unpack()
    }

    /// Create a new empty tuple.
    #[inline]
    pub(crate) fn new_empty_tuple() -> Self {
        FrozenValue::new_repr(&VALUE_EMPTY_TUPLE)
    }

    /// Create a new empty list.
    #[inline]
    pub fn new_empty_list() -> Self {
        FrozenValue::new_repr(&VALUE_EMPTY_FROZEN_LIST)
    }

    /// Create a new empty dict.
    #[inline]
    pub fn new_empty_dict() -> Self {
        FrozenValue::new_repr(&VALUE_EMPTY_FROZEN_DICT)
    }

    #[inline]
    pub(crate) fn ptr_value(self) -> RawPointer {
        self.0.raw()
    }

    /// Is a value a Starlark `None`.
    #[inline]
    pub fn is_none(self) -> bool {
        self.to_value().is_none()
    }

    /// Return the [`bool`] if the value is a boolean, otherwise [`None`].
    #[inline]
    pub fn unpack_bool(self) -> Option<bool> {
        self.to_value().unpack_bool()
    }

    /// Obtain the underlying integer if it fits in an `i32`.
    /// Note floats are not considered integers, i. e. `unpack_i32` for `1.0` will return `None`.
    #[inline]
    pub fn unpack_i32(self) -> Option<i32> {
        self.to_value().unpack_i32()
    }

    #[inline]
    pub(crate) fn unpack_inline_int(self) -> Option<InlineInt> {
        self.to_value().unpack_inline_int()
    }

    #[inline]
    pub(crate) fn is_str(self) -> bool {
        self.to_value().is_str()
    }

    // The resulting `str` is alive as long as the `FrozenHeap` is,
    // but we don't have that lifetime available to us. Therefore,
    // we cheat a little, and use the lifetime of the `FrozenValue`.
    // Because of this cheating, we don't expose it outside Starlark.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[inline]
    pub(crate) fn unpack_str<'v>(&'v self) -> Option<&'v str> {
        self.to_value().unpack_str()
    }

    /// Convert a [`FrozenValue`] back to a [`Value`].
    #[inline]
    pub fn to_value<'v>(self) -> Value<'v> {
        Value::new_frozen(self)
    }

    /// Is this type builtin? We perform certain optimizations only on builtin types
    /// because we know they have well defined semantics.
    pub(crate) fn is_builtin(self) -> bool {
        // The list is not comprehensive, this is fine.
        // If some type is not listed here, some optimizations won't work for this type.
        self.is_none()
            || self.is_str()
            || self.unpack_bool().is_some()
            || NumRef::unpack_value(self.to_value()).is_ok_and(|n| n.is_some())
            || FrozenListData::from_frozen_value(&self).is_some()
            || FrozenDictRef::from_frozen_value(self).is_some()
            || FrozenValueTyped::<FrozenTuple>::new(self).is_some()
            || FrozenValueTyped::<Range>::new(self).is_some()
            || FrozenValueTyped::<FrozenDef>::new(self).is_some()
            || FrozenValueTyped::<NativeFunction>::new(self).is_some()
            || FrozenValueTyped::<FrozenStruct>::new(self).is_some()
            || FrozenValueTyped::<RecordType>::new(self).is_some()
            || FrozenValueTyped::<FrozenRecord>::new(self).is_some()
            || FrozenValueTyped::<EnumType>::new(self).is_some()
            || FrozenValueTyped::<FrozenEnumValue>::new(self).is_some()
    }

    /// Can `invoke` be called on this object speculatively?
    /// (E. g. at compiled time when all the arguments are known.)
    pub(crate) fn speculative_exec_safe(self) -> bool {
        if let Some(v) = FrozenValueTyped::<NativeFunction>::new(self) {
            v.speculative_exec_safe
        } else if let Some(v) = FrozenValueTyped::<FrozenBoundMethod>::new(self) {
            v.method.speculative_exec_safe
        } else {
            false
        }
    }

    /// `self == b` is `ptr_eq`.
    pub(crate) fn eq_is_ptr_eq(self) -> bool {
        // Note `int` is not `ptr_eq` because `int` can be equal to `float`.

        // If a value does not override equality, it is `ptr_eq`.
        !self.to_value().get_ref().vtable().starlark_value.HAS_equals
            // Strings of length <= 1 are statically allocated.
            || matches!(self.unpack_str(), Some(s) if s.len() <= 1)
            // Empty tuple is statically allocated.
            || matches!(Tuple::from_value(self.to_value()), Some(t) if t.len() == 0)
    }

    /// Downcast to given type.
    #[inline]
    pub fn downcast_frozen_ref<T: StarlarkValue<'static>>(self) -> Option<FrozenRef<'static, T>> {
        self.downcast_ref::<T>().map(|value| FrozenRef { value })
    }

    /// Downcast to string.
    #[inline]
    pub fn downcast_frozen_str(self) -> Option<FrozenRef<'static, str>> {
        self.to_value()
            .unpack_str()
            .map(|value| FrozenRef { value })
    }

    /// Note: see docs about ['Value::unpack_box_str'] about instability
    #[inline]
    pub fn downcast_frozen_starlark_str(self) -> Option<FrozenRef<'static, StarlarkStr>> {
        self.to_value()
            .unpack_starlark_str()
            .map(|value| FrozenRef { value })
    }
}

impl<'v> Serialize for Value<'v> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match json_stack_push(*self) {
            Ok(_guard) => erased_serde::serialize(self.get_ref().as_serialize(), s),
            Err(..) => Err(serde::ser::Error::custom(ToJsonCycleError(self.get_type()))),
        }
    }
}

impl Serialize for FrozenValue {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_value().serialize(s)
    }
}

impl<'v> StarlarkTypeRepr for Value<'v> {
    type Canonical = <FrozenValue as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        FrozenValue::starlark_type_repr()
    }
}

impl StarlarkTypeRepr for FrozenValue {
    type Canonical = Self;

    fn starlark_type_repr() -> Ty {
        Ty::any()
    }
}

/// Abstract over [`Value`] and [`FrozenValue`].
///
/// The methods on this trait are those required to implement containers,
/// allowing implementations of [`ComplexValue`](crate::values::ComplexValue)
/// to be agnostic of their contained type.
/// For details about each function, see the documentation for [`Value`],
/// which provides the same functions (and more).
pub trait ValueLike<'v>:
    ValueLifetimeless + Trace<'v> + CoerceKey<Value<'v>> + ProvidesStaticType<'v> + 'v
{
    /// `StringValue` or `FrozenStringValue`.
    type String: StringValueLike<'v>;

    /// Produce a [`Value`] regardless of the type you are starting with.
    fn to_value(self) -> Value<'v>;

    /// Convert from [`FrozenValue`].
    fn from_frozen_value(v: FrozenValue) -> Self;

    /// Call this value as a function with given arguments.
    fn invoke(
        self,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> crate::Result<Value<'v>> {
        self.to_value().invoke(args, eval)
    }

    /// Hash the value.
    fn write_hash(self, hasher: &mut StarlarkHasher) -> crate::Result<()>;

    /// Get hash value.
    fn get_hashed(self) -> crate::Result<Hashed<Self>> {
        let hash = if let Some(s) = self.to_value().unpack_starlark_str() {
            s.get_hash()
        } else {
            self.to_value().get_hash()?
        };
        Ok(Hashed::new_unchecked(hash, self))
    }

    /// `repr(x)`.
    fn collect_repr(self, collector: &mut String);

    /// `str(x)`.
    fn collect_str(self, collector: &mut String) {
        if let Some(s) = self.to_value().unpack_str() {
            collector.push_str(s);
        } else {
            self.collect_repr(collector);
        }
    }

    /// `x == other`.
    ///
    /// This operation can only return error on stack overflow.
    fn equals(self, other: Value<'v>) -> crate::Result<bool>;

    /// `x <=> other`.
    fn compare(self, other: Value<'v>) -> crate::Result<Ordering>;

    /// Get a reference to underlying data or [`None`]
    /// if contained object has different type than requested.
    fn downcast_ref<T: StarlarkValue<'v>>(self) -> Option<&'v T>;

    /// Get a reference to underlying data or [`Err`]
    /// if contained object has different type than requested.
    fn downcast_ref_err<T: StarlarkValue<'v>>(self) -> anyhow::Result<&'v T> {
        match self.downcast_ref() {
            Some(v) => Ok(v),
            None => Err(ValueValueError::WrongType(
                T::TYPE,
                self.to_value().to_string_for_type_error(),
            )
            .into()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Cycle detected when serializing value of type `{0}` to JSON")]
struct ToJsonCycleError(&'static str);

impl<'v> Sealed for Value<'v> {}

impl<'v> ValueLifetimeless for Value<'v> {}

impl<'v> ValueLike<'v> for Value<'v> {
    type String = StringValue<'v>;

    #[inline]
    fn to_value(self) -> Value<'v> {
        self
    }

    #[inline]
    fn from_frozen_value(v: FrozenValue) -> Self {
        v.to_value()
    }

    fn downcast_ref<T: StarlarkValue<'v>>(self) -> Option<&'v T> {
        if T::static_type_id() == StarlarkStr::static_type_id() {
            if self.is_str() {
                // SAFETY: we just checked this is string, and requested type is string.
                Some(unsafe { self.downcast_ref_unchecked() })
            } else {
                None
            }
        } else if PointerI32::type_is_pointer_i32::<T>() {
            if self.unpack_inline_int().is_some() {
                // SAFETY: we just checked this is int, and requested type is int.
                Some(unsafe { self.downcast_ref_unchecked() })
            } else {
                None
            }
        } else {
            self.get_ref().downcast_ref::<T>()
        }
    }

    fn collect_repr(self, collector: &mut String) {
        match repr_stack_push(self) {
            Ok(_guard) => {
                self.get_ref().collect_repr(collector);
            }
            Err(..) => {
                self.get_ref().collect_repr_cycle(collector);
            }
        }
    }

    fn write_hash(self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        self.get_ref().write_hash(hasher)
    }

    #[inline]
    fn equals(self, other: Value<'v>) -> crate::Result<bool> {
        self.equals(other)
    }

    fn compare(self, other: Value<'v>) -> crate::Result<Ordering> {
        let _guard = stack_guard::stack_guard()?;
        self.get_ref().compare(other)
    }
}

impl Sealed for FrozenValue {}

impl ValueLifetimeless for FrozenValue {}

impl<'v> ValueLike<'v> for FrozenValue {
    type String = FrozenStringValue;

    #[inline]
    fn to_value(self) -> Value<'v> {
        Value::new_frozen(self)
    }

    #[inline]
    fn from_frozen_value(v: FrozenValue) -> Self {
        v
    }

    #[inline]
    fn downcast_ref<T: StarlarkValue<'v>>(self) -> Option<&'v T> {
        self.to_value().downcast_ref()
    }

    #[inline]
    fn collect_repr(self, collector: &mut String) {
        self.to_value().collect_repr(collector)
    }

    #[inline]
    fn write_hash(self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        self.to_value().write_hash(hasher)
    }

    #[inline]
    fn equals(self, other: Value<'v>) -> crate::Result<bool> {
        self.to_value().equals(other)
    }

    #[inline]
    fn compare(self, other: Value<'v>) -> crate::Result<Ordering> {
        self.to_value().compare(other)
    }
}

fn _test_send_sync()
where
    FrozenValue: Send + Sync,
{
}

#[cfg(test)]
mod tests {
    use num_bigint::BigInt;

    use crate::assert;
    use crate::environment::Globals;
    use crate::typing::Ty;
    use crate::values::int::pointer_i32::PointerI32;
    use crate::values::list::AllocList;
    use crate::values::none::NoneType;
    use crate::values::string::str_type::StarlarkStr;
    use crate::values::unpack::UnpackValue;
    use crate::values::Heap;
    use crate::values::Value;
    use crate::values::ValueLike;

    #[test]
    fn test_downcast_ref() {
        let heap = Heap::new();
        let string = heap.alloc_str("asd").to_value();
        let none = Value::new_none();
        let integer = Value::testing_new_int(17);

        assert!(string.downcast_ref::<NoneType>().is_none());
        assert!(integer.downcast_ref::<NoneType>().is_none());
        assert!(none.downcast_ref::<NoneType>().is_some());

        assert_eq!(
            "asd",
            string.downcast_ref::<StarlarkStr>().unwrap().as_str()
        );
        assert!(integer.downcast_ref::<StarlarkStr>().is_none());
        assert!(none.downcast_ref::<StarlarkStr>().is_none());

        assert!(string.downcast_ref::<PointerI32>().is_none());
        assert_eq!(17, integer.downcast_ref::<PointerI32>().unwrap().get());
        assert!(none.downcast_ref::<PointerI32>().is_none());
    }

    #[test]
    fn test_unpack_i32() {
        let heap = Heap::new();
        let value = heap.alloc(i32::MAX);
        assert_eq!(Some(i32::MAX), value.unpack_i32());
    }

    #[test]
    fn test_unpack_frozen() {
        assert!(Value::new_none().unpack_frozen().is_some());
        assert!(Value::testing_new_int(10).unpack_frozen().is_some());
    }

    #[test]
    fn test_unpack_bigint() {
        let heap = Heap::new();
        let value = heap.alloc(BigInt::from(i64::MAX));
        assert_eq!(None, value.unpack_i32());
        assert_eq!(
            Some(BigInt::from(i64::MAX)),
            BigInt::unpack_value(value).unwrap()
        );
    }

    #[test]
    fn test_to_json_value() {
        let value = assert::pass("{'a': 10}");
        assert_eq!(
            serde_json::json!({"a": 10}),
            value.value().to_json_value().unwrap()
        );
    }

    #[test]
    fn test_display_for_type_error() {
        assert_eq!(
            "NoneType (repr: None)",
            Value::new_none().to_string_for_type_error(),
        );

        let heap = Heap::new();
        let list = heap.alloc(AllocList(0..12345));
        assert_eq!(
            "list (repr: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,<<...>>42, 12343, 12344])",
            list.to_string_for_type_error(),
        );
    }

    #[test]
    fn test_check_callable_with_none() {
        let e = Value::new_none()
            .check_callable_with([], [], None, None, &Ty::int())
            .unwrap_err();
        assert!(
            e.to_string().contains("Value is not callable: NoneType"),
            "{e}"
        );
    }

    #[test]
    fn test_check_callable_with_good_function() {
        let g = Globals::standard();
        let f = g.get("bool").unwrap();

        // Positional.
        f.check_callable_with([&Ty::any_list()], [], None, None, &Ty::bool())
            .unwrap();

        // Named.
        let e = f
            .check_callable_with([], [("x", &Ty::any_list())], None, None, &Ty::bool())
            .unwrap_err();
        assert!(
            e.to_string()
                .contains("Value `function (repr: bool)` is not compatible with"),
            "{e}"
        );

        // Return type.
        let e = f
            .check_callable_with([&Ty::any_list()], [], None, None, &Ty::string())
            .unwrap_err();
        assert!(
            e.to_string()
                .contains("Value `function (repr: bool)` is not compatible with"),
            "{e}"
        );
    }
}
