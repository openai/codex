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

use std::cmp::Ordering;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hasher;
use std::mem;
use std::ptr;

use allocative::Allocative;
use num_bigint::BigInt;
use serde::Serialize;
use serde::Serializer;
use starlark_derive::starlark_value;
use starlark_derive::ProvidesStaticType;
use starlark_map::StarlarkHashValue;
use starlark_map::StarlarkHasher;

use crate as starlark;
use crate::any::AnyLifetime;
use crate::cast;
use crate::private::Private;
use crate::typing::Ty;
use crate::typing::TyBasic;
use crate::typing::TypingBinOp;
use crate::values::layout::avalue::AValueBasic;
use crate::values::layout::pointer::RawPointer;
use crate::values::layout::vtable::AValueDyn;
use crate::values::layout::vtable::AValueVTable;
use crate::values::layout::vtable::StarlarkValueRawPtr;
use crate::values::types::bigint::StarlarkBigInt;
use crate::values::types::int::inline_int::InlineInt;
use crate::values::types::int::int_or_big::StarlarkInt;
use crate::values::types::int::int_or_big::StarlarkIntRef;
use crate::values::types::num::typecheck::typecheck_num_bin_op;
use crate::values::types::num::typecheck::NumTy;
use crate::values::types::num::value::NumRef;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueError;

/// The result of calling `type()` on integers.
pub const INT_TYPE: &str = "int";

// WARNING: This type isn't a real type, a pointer to this is secretly an i32.
// Therefore, don't derive stuff on it, since it will be wrong.
// However, `ProvidesStaticType` promises not to peek at its value, so that's fine.
#[derive(ProvidesStaticType, Allocative)]
#[repr(C)]
pub(crate) struct PointerI32 {
    _private: (),
}

impl PartialEq for PointerI32 {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self, other)
    }
}

impl Eq for PointerI32 {}

impl Debug for PointerI32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.get(), f)
    }
}

impl PointerI32 {
    const _ASSERTIONS: () = {
        assert!(mem::align_of::<Self>() == 1);
    };

    #[inline]
    pub(crate) unsafe fn from_raw_pointer_unchecked(
        raw_pointer: RawPointer,
    ) -> &'static PointerI32 {
        debug_assert!(raw_pointer.is_int());
        // UB if the pointer isn't aligned, or it is zero.
        // Alignment is 1, so that's not an issue.
        // And the pointer is not zero because it has `TAG_INT` bit set.
        cast::usize_to_ptr(raw_pointer.ptr_value())
    }

    #[inline]
    pub(crate) fn get(&self) -> InlineInt {
        unsafe { RawPointer::new_unchecked(self as *const Self as usize).unpack_int_unchecked() }
    }

    #[inline]
    pub(crate) fn as_avalue_dyn(&'static self) -> AValueDyn<'static> {
        unsafe { AValueDyn::new(StarlarkValueRawPtr::new_pointer_i32(self), Self::vtable()) }
    }

    #[inline]
    pub(crate) fn vtable() -> &'static AValueVTable {
        AValueVTable::new::<AValueBasic<PointerI32>>()
    }

    /// This operation is expensive, use only if you have to.
    fn to_bigint(&self) -> BigInt {
        self.get().to_bigint()
    }

    pub(crate) fn type_is_pointer_i32<'v, T: StarlarkValue<'v>>() -> bool {
        T::static_type_id() == PointerI32::static_type_id()
    }
}

impl Display for PointerI32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}

/// Define the int type
#[starlark_value(type = INT_TYPE)]
impl<'v> StarlarkValue<'v> for PointerI32 {
    type Canonical = StarlarkBigInt;

    fn is_special(_: Private) -> bool
    where
        Self: Sized,
    {
        true
    }

    fn equals(&self, other: Value) -> crate::Result<bool> {
        Ok(Some(NumRef::Int(StarlarkIntRef::Small(self.get()))) == other.unpack_num())
    }

    fn to_bool(&self) -> bool {
        self.get().to_i32() != 0
    }
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        hasher.write_u64(NumRef::Int(StarlarkIntRef::Small(self.get())).get_hash_64());
        Ok(())
    }

    fn get_hash(&self, _private: Private) -> crate::Result<StarlarkHashValue> {
        Ok(NumRef::Int(StarlarkIntRef::Small(self.get())).get_hash())
    }

    fn plus(&self, _heap: &'v Heap) -> crate::Result<Value<'v>> {
        Ok(Value::new_int(self.get()))
    }
    fn minus(&self, heap: &'v Heap) -> crate::Result<Value<'v>> {
        Ok(heap.alloc(-StarlarkIntRef::Small(self.get())))
    }
    fn add(&self, other: Value<'v>, heap: &'v Heap) -> Option<crate::Result<Value<'v>>> {
        Some(Ok(heap.alloc(
            NumRef::Int(StarlarkIntRef::Small(self.get())) + other.unpack_num()?,
        )))
    }
    fn sub(&self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match other.unpack_num() {
            Some(other) => Ok(heap.alloc(NumRef::Int(StarlarkIntRef::Small(self.get())) - other)),
            None => ValueError::unsupported_with(self, "-", other),
        }
    }
    fn mul(&self, other: Value<'v>, heap: &'v Heap) -> Option<crate::Result<Value<'v>>> {
        Some(Ok(heap.alloc(
            NumRef::Int(StarlarkIntRef::Small(self.get())) * other.unpack_num()?,
        )))
    }
    fn div(&self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match other.unpack_num() {
            Some(other) => {
                Ok(heap.alloc(NumRef::Int(StarlarkIntRef::Small(self.get())).div(other)?))
            }
            None => ValueError::unsupported_with(self, "/", other),
        }
    }
    fn percent(&self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match other.unpack_num() {
            Some(other) => {
                Ok(heap.alloc(NumRef::Int(StarlarkIntRef::Small(self.get())).percent(other)?))
            }
            None => ValueError::unsupported_with(self, "%", other),
        }
    }
    fn floor_div(&self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match other.unpack_num() {
            Some(other) => {
                Ok(heap.alloc(NumRef::Int(StarlarkIntRef::Small(self.get())).floor_div(other)?))
            }
            None => ValueError::unsupported_with(self, "//", other),
        }
    }

    fn compare(&self, other: Value) -> crate::Result<Ordering> {
        match other.unpack_num() {
            None => ValueError::unsupported_with(self, "compare", other),
            Some(other) => Ok(NumRef::Int(StarlarkIntRef::Small(self.get())).cmp(&other)),
        }
    }

    fn bit_and(&self, other: Value, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match StarlarkIntRef::unpack(other) {
            None => ValueError::unsupported_with(self, "&", other),
            Some(StarlarkIntRef::Small(i)) => Ok(Value::new_int(self.get() & i)),
            Some(StarlarkIntRef::Big(b)) => {
                Ok(heap.alloc(StarlarkInt::from(&self.to_bigint() & b.get())))
            }
        }
    }

    fn bit_or(&self, other: Value, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match StarlarkIntRef::unpack(other) {
            None => ValueError::unsupported_with(self, "|", other),
            Some(StarlarkIntRef::Small(i)) => Ok(Value::new_int(self.get() | i)),
            Some(StarlarkIntRef::Big(b)) => {
                Ok(heap.alloc(StarlarkInt::from(&self.to_bigint() | b.get())))
            }
        }
    }

    fn bit_xor(&self, other: Value, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match StarlarkIntRef::unpack(other) {
            None => ValueError::unsupported_with(self, "^", other),
            Some(StarlarkIntRef::Small(i)) => Ok(Value::new_int(self.get() ^ i)),
            Some(StarlarkIntRef::Big(b)) => {
                Ok(heap.alloc(StarlarkInt::from(&self.to_bigint() ^ b.get())))
            }
        }
    }

    fn bit_not(&self, _heap: &'v Heap) -> crate::Result<Value<'v>> {
        Ok(Value::new_int(!self.get()))
    }

    fn left_shift(&self, other: Value, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match StarlarkIntRef::unpack(other) {
            None => ValueError::unsupported_with(self, "<<", other),
            Some(other) => Ok(heap.alloc(StarlarkIntRef::Small(self.get()).left_shift(other)?)),
        }
    }

    fn right_shift(&self, other: Value, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match StarlarkIntRef::unpack(other) {
            None => ValueError::unsupported_with(self, ">>", other),
            Some(other) => Ok(heap.alloc(StarlarkIntRef::Small(self.get()).right_shift(other)?)),
        }
    }

    fn bin_op_ty(op: TypingBinOp, rhs: &TyBasic) -> Option<Ty> {
        // This is dead code, because canonical int type is `StarlarkBigInt`,
        // but keep for consistency.
        typecheck_num_bin_op(NumTy::Int, op, rhs)
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(Ty::int())
    }
}

impl Serialize for PointerI32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.get().serialize(serializer)
    }
}
