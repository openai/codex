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

//! Outside of `i32` range int.

mod convert;

use std::cmp::Ordering;
use std::hash::Hash;

use allocative::Allocative;
use num_bigint::BigInt;
use num_bigint::Sign;
use num_traits::cast::ToPrimitive;
use serde::Serialize;
use starlark_derive::starlark_value;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::collections::StarlarkHasher;
use crate::typing::Ty;
use crate::typing::TyBasic;
use crate::typing::TypingBinOp;
use crate::values::types::int::inline_int::InlineInt;
use crate::values::types::int::int_or_big::StarlarkInt;
use crate::values::types::int::int_or_big::StarlarkIntRef;
use crate::values::types::num::typecheck::typecheck_num_bin_op;
use crate::values::types::num::typecheck::NumTy;
use crate::values::types::num::value::NumRef;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueError;

/// `int` implementation for larger integers.
#[derive(
    Clone,
    Debug,
    Default,
    derive_more::Display,
    ProvidesStaticType,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Hash,
    Allocative
)]
#[display("{}", value)]
pub struct StarlarkBigInt {
    /// `value` is strictly either smaller than `i32::MIN` or larger than `i32::MAX`.
    /// Many operation implementations depend on this fact.
    /// For example, `non_zero_int << positive_big_int` is considered to be overflow
    /// without checking the actual value of `positive_big_int`.
    #[allocative(skip)] // TODO(nga): do not skip.
    value: BigInt,
}

impl StarlarkBigInt {
    pub(crate) fn unchecked_new(value: BigInt) -> Self {
        debug_assert!(
            InlineInt::try_from(&value).is_err(),
            "BigInt must be outside of `InlineInt` range"
        );
        Self { value }
    }

    pub(crate) fn get(&self) -> &BigInt {
        &self.value
    }

    pub(crate) fn to_f64(&self) -> f64 {
        // `to_f64` is infallible.
        self.value.to_f64().unwrap()
    }

    pub(crate) fn to_i32(&self) -> Option<i32> {
        // Avoid calling `to_i32` if the value is known to be out of range.
        if InlineInt::smaller_than_i32() {
            let v = self.value.to_i32();
            if let Some(v) = v {
                debug_assert!(InlineInt::try_from(v).is_err());
            }
            v
        } else {
            None
        }
    }

    pub(crate) fn cmp_small_big(a: InlineInt, b: &StarlarkBigInt) -> Ordering {
        let a_sign = a.signum();
        let b_sign = match b.value.sign() {
            Sign::Plus => 2,
            Sign::Minus => -2,
            Sign::NoSign => 0,
        };
        // Sign comparison is enough because `StarlarkBigInt` is out of range of `i32`.
        a_sign.cmp(&b_sign)
    }

    pub(crate) fn cmp_big_small(a: &StarlarkBigInt, b: InlineInt) -> Ordering {
        Self::cmp_small_big(b, a).reverse()
    }

    pub(crate) fn unpack_integer<'v, I: TryFrom<&'v BigInt>>(&'v self) -> Option<I> {
        I::try_from(&self.value).ok()
    }
}

impl PartialEq<i32> for StarlarkBigInt {
    fn eq(&self, _other: &i32) -> bool {
        false
    }
}

impl Serialize for StarlarkBigInt {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.value.to_string())
    }
}

impl<'v> AllocValue<'v> for StarlarkBigInt {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc_simple(self)
    }
}

impl AllocFrozenValue for StarlarkBigInt {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc_simple(self)
    }
}

#[starlark_value(type = "int")]
impl<'v> StarlarkValue<'v> for StarlarkBigInt {
    fn to_bool(&self) -> bool {
        // `StarlarkBigInt` is non-zero.
        true
    }

    fn minus(&self, heap: &'v Heap) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(StarlarkInt::from(-&self.value)))
    }

    fn plus(&self, heap: &'v Heap) -> starlark::Result<Value<'v>> {
        // This unnecessarily allocates, could return `self`.
        // But practically people rarely write `+NNN` except in constants,
        // and in constants we fold `+NNN` into `NNN`.
        Ok(heap.alloc(StarlarkInt::from(self.value.clone())))
    }

    fn equals(&self, other: Value<'v>) -> crate::Result<bool> {
        Ok(Some(NumRef::Int(StarlarkIntRef::Big(self))) == other.unpack_num())
    }

    fn compare(&self, other: Value<'v>) -> crate::Result<Ordering> {
        match other.unpack_num() {
            None => ValueError::unsupported_with(self, "compare", other),
            Some(other) => Ok(NumRef::Int(StarlarkIntRef::Big(self)).cmp(&other)),
        }
    }

    fn add(&self, rhs: Value<'v>, heap: &'v Heap) -> Option<crate::Result<Value<'v>>> {
        Some(Ok(heap.alloc(
            NumRef::Int(StarlarkIntRef::Big(self)) + rhs.unpack_num()?,
        )))
    }

    fn sub(&self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match other.unpack_num() {
            Some(other) => Ok(heap.alloc(NumRef::Int(StarlarkIntRef::Big(self)) - other)),
            None => ValueError::unsupported_with(self, "-", other),
        }
    }

    fn mul(&self, other: Value<'v>, heap: &'v Heap) -> Option<crate::Result<Value<'v>>> {
        Some(Ok(heap.alloc(
            NumRef::Int(StarlarkIntRef::Big(self)) * other.unpack_num()?,
        )))
    }

    fn div(&self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match other.unpack_num() {
            Some(other) => Ok(heap.alloc(NumRef::Int(StarlarkIntRef::Big(self)).div(other)?)),
            None => ValueError::unsupported_with(self, "/", other),
        }
    }

    fn floor_div(&self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match other.unpack_num() {
            Some(rhs) => Ok(heap.alloc(NumRef::Int(StarlarkIntRef::Big(self)).floor_div(rhs)?)),
            None => ValueError::unsupported_with(self, "//", other),
        }
    }

    fn percent(&self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match other.unpack_num() {
            Some(rhs) => Ok(heap.alloc(NumRef::Int(StarlarkIntRef::Big(self)).percent(rhs)?)),
            None => ValueError::unsupported_with(self, "%", other),
        }
    }

    fn bit_and(&self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        let rhs = match StarlarkIntRef::unpack_value_opt(other) {
            Some(rhs) => rhs,
            None => return ValueError::unsupported_with(self, "&", other),
        };
        Ok(heap.alloc(StarlarkIntRef::Big(self) & rhs))
    }

    fn bit_xor(&self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        let rhs = match StarlarkIntRef::unpack_value_opt(other) {
            Some(rhs) => rhs,
            None => return ValueError::unsupported_with(self, "^", other),
        };
        Ok(heap.alloc(StarlarkIntRef::Big(self) ^ rhs))
    }

    fn bit_or(&self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        let rhs = match StarlarkIntRef::unpack_value_opt(other) {
            Some(rhs) => rhs,
            None => return ValueError::unsupported_with(self, "|", other),
        };
        Ok(heap.alloc(StarlarkIntRef::Big(self) | rhs))
    }

    fn bit_not(&self, heap: &'v Heap) -> crate::Result<Value<'v>> {
        Ok(heap.alloc(!StarlarkIntRef::Big(self)))
    }

    fn left_shift(&self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match StarlarkIntRef::unpack_value_opt(other) {
            None => ValueError::unsupported_with(self, "<<", other),
            Some(other) => Ok(heap.alloc(StarlarkIntRef::Big(self).left_shift(other)?)),
        }
    }

    fn right_shift(&self, other: Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match StarlarkIntRef::unpack_value_opt(other) {
            None => ValueError::unsupported_with(self, ">>", other),
            Some(other) => Ok(heap.alloc(StarlarkIntRef::Big(self).right_shift(other)?)),
        }
    }

    fn bin_op_ty(op: TypingBinOp, rhs: &TyBasic) -> Option<Ty> {
        typecheck_num_bin_op(NumTy::Int, op, rhs)
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        NumRef::Int(StarlarkIntRef::Big(self))
            .get_hash_64()
            .hash(hasher);
        Ok(())
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(Ty::int())
    }
}

#[cfg(test)]
mod tests {
    use std::hash::Hasher;

    use num_bigint::BigInt;

    use crate::assert;
    use crate::collections::StarlarkHasher;
    use crate::values::float::StarlarkFloat;
    use crate::values::types::bigint::StarlarkBigInt;
    use crate::values::StarlarkValue;

    #[test]
    fn test_parse() {
        assert::eq(
            "'1234567890112233445566778899'",
            "str(1234567890112233445566778899)",
        );
        assert::eq(
            "'1234567890112233445566778899'",
            "str(0x3fd35eb6d519aff76f50e13)",
        );
        assert::eq(
            "'1234567890112233445566778899'",
            "str(0o776465726665214657756675207023)",
        );
        assert::eq(
            "'1234567890112233445566778899'",
            "str(0b11111111010011010111101011011011010101000\
                1100110101111111101110110111101010000111000010011)",
        );
    }

    #[test]
    fn test_str() {
        assert::eq(
            "'1234567890112233445566778899'",
            "str(1234567890112233445566778899)",
        );
    }

    #[test]
    fn test_repr() {
        assert::eq(
            "'1234567890112233445566778899'",
            "repr(1234567890112233445566778899)",
        );
    }

    #[test]
    fn test_equals() {
        assert::eq("10000000000000000000000", "10000000000000000000000");
        assert::eq("10000000000000000000000", "10000000000000000000000.0");
        assert::eq("10000000000000000000000.0", "10000000000000000000000");
    }

    #[test]
    fn test_plus() {
        assert::eq("10000000000000000000000", "+10000000000000000000000");
    }

    #[test]
    fn test_compare_big_big() {
        assert::is_true("10000000000000000000000 < 20000000000000000000000");
        assert::is_true("-20000000000000000000000 < -10000000000000000000000");
        assert::is_true("20000000000000000000000 > 10000000000000000000000");
        assert::is_true("-10000000000000000000000 > -20000000000000000000000");
    }

    #[test]
    fn test_compare_big_small() {
        assert::is_true("1 < 10000000000000000000000");
        assert::is_true("-1 < 10000000000000000000000");
        assert::is_true("1 > -10000000000000000000000");
        assert::is_true("-1 > -10000000000000000000000");
        assert::is_true("10000000000000000000000 > 1");
        assert::is_true("10000000000000000000000 > -1");
        assert::is_true("-10000000000000000000000 < 1");
        assert::is_true("-10000000000000000000000 < -1");
    }

    #[test]
    fn test_compare_big_float() {
        assert::is_true("1.0 < 10000000000000000000000");
        assert::is_true("-1.0 < 10000000000000000000000");
        assert::is_true("1.0 > -10000000000000000000000");
        assert::is_true("-1.0 > -10000000000000000000000");
        assert::is_true("10000000000000000000000 > 1.0");
        assert::is_true("10000000000000000000000 > -1.0");
        assert::is_true("-10000000000000000000000 < 1.0");
        assert::is_true("-10000000000000000000000 < -1.0");
    }

    #[test]
    fn test_add_big() {
        assert::eq(
            "300000000000000000009",
            "100000000000000000004 + 200000000000000000005",
        );
        assert::eq("7", "100000000000000000007 + -100000000000000000000");
        assert::eq(
            "200000000000000000005",
            "300000000000000000009 - 100000000000000000004",
        );
        assert::eq("7", "100000000000000000007 - 100000000000000000000");
    }

    #[test]
    fn test_add_big_small() {
        assert::eq("100000000000000000017", "100000000000000000000 + 17");
        assert::eq("100000000000000000017", "17 + 100000000000000000000");
        assert::eq("100000000000000000000", "100000000000000000017 - 17");
        assert::eq("-100000000000000000017", "17 - 100000000000000000034");
    }

    #[test]
    fn test_add_big_float() {
        assert::eq("2e20", "100000000000000000000 + 1e20");
        assert::eq("2e20", "1e20 + 100000000000000000000");
        assert::eq("2e20", "300000000000000000000 - 1e20");
        assert::eq("2e20", "3e20 - 100000000000000000000");
    }

    #[test]
    fn test_mul_big() {
        assert::eq(
            "60000000000000000000000000000000000000000",
            "200000000000000000000 * 300000000000000000000",
        );
    }

    #[test]
    fn test_mul_big_small() {
        assert::eq("600000000000000000000", "200000000000000000000 * 3");
        assert::eq("600000000000000000000", "3 * 200000000000000000000");
    }

    #[test]
    fn test_mul_big_float() {
        assert::eq("6e20", "200000000000000000000 * 3.0");
        assert::eq("6e20", "3.0 * 200000000000000000000");
    }

    #[test]
    fn test_div_big() {
        assert::eq(
            "2e20",
            "60000000000000000000000000000000000000000 / 300000000000000000000",
        );
    }

    #[test]
    fn test_div_big_small() {
        assert::eq("2e20", "600000000000000000000 / 3");
        assert::eq("2e-20", "6 / 300000000000000000000");
    }

    #[test]
    fn test_div_big_float() {
        assert::eq("2e20", "600000000000000000000 / 3.0");
        assert::eq("2e-20", "6.0 / 300000000000000000000");
    }

    #[test]
    fn test_floor_div_big() {
        assert::eq("2", "600000000000000000000 // 300000000000000000000");
    }

    #[test]
    fn test_floor_div_big_small() {
        assert::eq("200000000000000000000", "600000000000000000000 // 3");
        assert::eq("0", "3 // 600000000000000000000");
    }

    #[test]
    fn test_floor_div_big_float() {
        assert::eq("2e20", "600000000000000000000 / 3.0");
        assert::eq("2e-20", "6.0 / 300000000000000000000");
    }

    #[test]
    fn test_percent_big() {
        assert::eq("7", "600000000000000000007 % 200000000000000000000");
    }

    #[test]
    fn test_percent_big_small() {
        assert::eq("7", "600000000000000000007 % 20");
        assert::eq("3", "3 % 600000000000000000000");
    }

    #[test]
    fn test_percent_big_float() {
        assert::eq("1e20", "100000000000000000000 % 1e50");
        assert::eq("10.0", "10.0 % 100000000000000000000");
    }

    #[test]
    fn test_bit_and_big() {
        assert::eq(
            "0x10000000000000000000000",
            "0x30000000000000000000000 & 0x90000000000000000000000",
        );
    }

    #[test]
    fn test_bit_and_big_small() {
        assert::eq("1", "0x60000000000000000000003 & 0x9");
        assert::eq("1", "0x9 & 0x60000000000000000000003");
    }

    #[test]
    fn test_bit_and_float() {
        assert::fail_skip_typecheck("0x60000000000000000000000 & 1.0", "not supported");
        assert::fail_skip_typecheck("1.0 & 0x60000000000000000000000", "not supported");
        assert::fail(
            "def f(): 0x60000000000000000000000 & 1.0",
            "is not available on the types",
        );
        assert::fail(
            "def f(): 1.0 & 0x60000000000000000000000",
            "is not available on the types",
        );
    }

    #[test]
    fn test_bit_or_big() {
        assert::eq(
            "0x70000000000000000000000",
            "0x30000000000000000000000 | 0x50000000000000000000000",
        );
    }

    #[test]
    fn test_bit_or_big_small() {
        assert::eq(
            "0x60000000000000000000009",
            "0x60000000000000000000000 | 0x9",
        );
        assert::eq(
            "0x60000000000000000000009",
            "0x9 | 0x60000000000000000000000",
        );
    }

    #[test]
    fn test_bit_or_float() {
        assert::fail_skip_typecheck("0x60000000000000000000000 | 1.0", "not supported");
        assert::fail_skip_typecheck("1.0 | 0x60000000000000000000000", "not supported");
        assert::fail(
            "def f(): 0x60000000000000000000000 | 1.0",
            "is not available on the types",
        );
        assert::fail(
            "def f(): 1.0 | 0x60000000000000000000000",
            "is not available on the types",
        );
    }

    #[test]
    fn test_bit_xor_big() {
        assert::eq(
            "0x60000000000000000000000",
            "0x30000000000000000000000 ^ 0x50000000000000000000000",
        );
    }

    #[test]
    fn test_bit_xor_big_small() {
        assert::eq(
            "0x60000000000000000000000",
            "0x60000000000000000000009 ^ 0x9",
        );
        assert::eq(
            "0x60000000000000000000000",
            "0x9 ^ 0x60000000000000000000009",
        );
    }

    #[test]
    fn test_bit_xor_float() {
        assert::fail_skip_typecheck("0x60000000000000000000000 ^ 1.0", "not supported");
        assert::fail_skip_typecheck("1.0 ^ 0x60000000000000000000000", "not supported");
        assert::fail(
            "def f(): 0x60000000000000000000000 ^ 1.0",
            "Binary operator `^` is not available",
        );
        assert::fail(
            "def f(): 1.0 ^ 0x60000000000000000000000",
            "Binary operator `^` is not available",
        );
    }

    #[test]
    fn test_bit_not() {
        assert::eq(
            "-0x10000000000000000000000000000001",
            "~0x10000000000000000000000000000000",
        );
    }

    #[test]
    fn test_left_shift() {
        assert::fail(
            "0x10000000000000000000000000000000 << 0x10000000000000000000000000000000",
            "Integer overflow",
        );
        assert::fail(
            "0x10000000000000000000000000000000 << -0x10000000000000000000000000000000",
            "Negative left shift",
        );
    }

    #[test]
    fn test_left_shift_small() {
        assert::eq(
            "0x20000000000000000000000000000000",
            "0x10000000000000000000000000000000 << 1",
        );
        assert::fail(
            "0x10000000000000000000000000000000 << -1",
            "Negative left shift",
        );
        assert::fail(
            "1 << 0x10000000000000000000000000000000",
            "Integer overflow",
        );
        assert::fail(
            "1 << -0x10000000000000000000000000000000",
            "Negative left shift",
        );
        assert::eq("0", "0 << 0x10000000000000000000000000000000");
        assert::eq("1267650600228229401496703205376", "1 << 100");
        assert::eq("-1267650600228229401496703205376", "-1 << 100");
    }

    #[test]
    fn test_left_shift_float() {
        assert::fail_skip_typecheck("0x10000000000000000000000000000000 << 1.0", "not supported");
        assert::fail_skip_typecheck("1.0 << 0x10000000000000000000000000000000", "not supported");
        assert::fail(
            "def f(): 0x10000000000000000000000000000000 << 1.0",
            "is not available",
        );
        assert::fail(
            "def f(): 1.0 << 0x10000000000000000000000000000000",
            "is not available",
        );
    }

    #[test]
    fn test_right_shift() {
        assert::eq(
            "0",
            "0x20000000000000000000000000000000 >> 0x20000000000000000000000000000000",
        );
        assert::eq(
            "-1",
            "-0x20000000000000000000000000000000 >> 0x20000000000000000000000000000000",
        );
        assert::fail(
            "0x20000000000000000000000000000000 >> -0x20000000000000000000000000000000",
            "Negative right shift",
        );
    }

    #[test]
    fn test_right_shift_small() {
        assert::eq(
            "0x10000000000000000000000000000000",
            "0x20000000000000000000000000000000 >> 1",
        );
        assert::fail(
            "0x20000000000000000000000000000000 >> -1",
            "Negative right shift",
        );
        assert::eq("0", "1 >> 0x20000000000000000000000000000000");
        assert::eq("-1", "-1 >> 0x20000000000000000000000000000000");
        assert::fail(
            "1 >> -0x10000000000000000000000000000000",
            "Negative right shift",
        );
    }

    #[test]
    fn test_right_shift_float() {
        assert::fail_skip_typecheck("0x20000000000000000000000000000000 >> 1.0", "not supported");
        assert::fail_skip_typecheck("1.0 >> 0x20000000000000000000000000000000", "not supported");
        assert::fail(
            "def f(): 0x20000000000000000000000000000000 >> 1.0",
            "is not available",
        );
        assert::fail(
            "def f(): 1.0 >> 0x20000000000000000000000000000000",
            "is not available",
        );
    }

    #[test]
    fn test_int_function() {
        assert::eq(
            "123456789012345678901234567890",
            "int(123456789012345678901234567890)",
        );
    }

    #[test]
    fn test_hash() {
        let mut hash1 = StarlarkHasher::new();
        let mut hash2 = StarlarkHasher::new();
        StarlarkFloat(1e20).write_hash(&mut hash1).unwrap();
        StarlarkBigInt::unchecked_new(BigInt::from(10).pow(20))
            .write_hash(&mut hash2)
            .unwrap();
        assert_eq!(hash1.finish(), hash2.finish());
    }

    #[test]
    fn test_int_type_matches_bigint() {
        assert::is_true("isinstance(1 << 100, int)");
    }
}
