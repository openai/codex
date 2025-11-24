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
use std::fmt::Debug;
use std::fmt::Formatter;
use std::ops::BitAnd;
use std::ops::BitOr;
use std::ops::BitXor;
use std::ops::Not;
use std::ops::Rem;

use dupe::Dupe;
use num_bigint::BigInt;
use num_traits::Signed;
use serde::Serialize;

use crate::hint;
use crate::typing::Ty;
use crate::values::int::pointer_i32::PointerI32;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::int::int_or_big::StarlarkInt;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::UnpackValue;
use crate::values::Value;

/// Integer which is stored inline in `RawPointer`.
#[derive(
    Clone,
    Copy,
    Dupe,
    derive_more::Display,
    Eq,
    PartialEq,
    Hash,
    Ord,
    PartialOrd,
    Serialize
)]
#[serde(transparent)]
#[doc(hidden)]
pub struct InlineInt(i32);

impl Debug for InlineInt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl InlineInt {
    const fn min_max_for_bits(bits: usize) -> (i32, i32) {
        let max = ((1u64 << (bits - 1)) - 1) as i32;
        let min = -max - 1;
        (min, max)
    }

    /// Number of bits in the integer.
    #[cfg(target_pointer_width = "64")]
    pub(crate) const BITS: usize = 32;
    #[cfg(target_pointer_width = "32")]
    pub(crate) const BITS: usize = 29;

    pub(crate) const ZERO: InlineInt = InlineInt(0);
    pub(crate) const MINUS_ONE: InlineInt = InlineInt(-1);
    pub(crate) const MIN: InlineInt = InlineInt(Self::min_max_for_bits(Self::BITS).0);
    pub(crate) const MAX: InlineInt = InlineInt(Self::min_max_for_bits(Self::BITS).1);

    /// This type does not contain full range of `i32`.
    pub(crate) fn smaller_than_i32() -> bool {
        Self::BITS < 32
    }

    #[inline]
    pub(crate) fn new_unchecked(i: i32) -> InlineInt {
        InlineInt(i)
    }

    #[cfg(test)]
    pub(crate) fn testing_new(i: i32) -> InlineInt {
        InlineInt::try_from(i).ok().unwrap()
    }

    #[inline]
    pub(crate) fn to_i32(self) -> i32 {
        self.0
    }

    #[inline]
    pub(crate) fn to_u64(self) -> Option<u64> {
        u64::try_from(self.0).ok()
    }

    #[inline]
    pub(crate) fn to_u32(self) -> Option<u32> {
        u32::try_from(self.0).ok()
    }

    #[inline]
    pub(crate) fn to_f64(self) -> f64 {
        self.0 as f64
    }

    #[inline]
    pub(crate) fn signum(self) -> i32 {
        self.0.signum()
    }

    #[inline]
    pub(crate) fn checked_add(self, rhs: InlineInt) -> Option<InlineInt> {
        self.0
            .checked_add(rhs.0)
            .and_then(|i| InlineInt::try_from(i).ok())
    }

    #[inline]
    pub(crate) fn checked_sub(self, rhs: InlineInt) -> Option<InlineInt> {
        self.checked_sub_i32(rhs.0)
    }

    #[inline]
    pub(crate) fn checked_sub_i32(self, rhs: i32) -> Option<InlineInt> {
        self.0
            .checked_sub(rhs)
            .and_then(|i| InlineInt::try_from(i).ok())
    }

    #[inline]
    pub(crate) fn checked_neg(self) -> Option<InlineInt> {
        self.0
            .checked_neg()
            .and_then(|i| InlineInt::try_from(i).ok())
    }

    #[inline]
    pub(crate) fn checked_div(self, rhs: InlineInt) -> Option<InlineInt> {
        self.0
            .checked_div(rhs.0)
            .and_then(|i| InlineInt::try_from(i).ok())
    }

    #[inline]
    pub(crate) fn checked_mul_i32(self, rhs: i32) -> Option<InlineInt> {
        self.0
            .checked_mul(rhs)
            .and_then(|i| InlineInt::try_from(i).ok())
    }

    #[inline]
    pub(crate) fn checked_shr(self, rhs: u32) -> Option<InlineInt> {
        self.0
            .checked_shr(rhs)
            .and_then(|i| InlineInt::try_from(i).ok())
    }

    #[inline]
    pub(crate) fn checked_shl(self, rhs: u32) -> Option<InlineInt> {
        self.0
            .checked_shl(rhs)
            .and_then(|i| InlineInt::try_from(i).ok())
    }

    pub(crate) fn to_bigint(self) -> BigInt {
        BigInt::from(self.0)
    }

    pub(crate) fn abs(self) -> StarlarkInt {
        match self.0.checked_abs() {
            Some(i) => StarlarkInt::from(i),
            None => StarlarkInt::from(self.to_bigint().abs()),
        }
    }

    #[inline]
    fn try_from_impl<I>(i: I) -> Result<InlineInt, InlineIntOverflow>
    where
        i32: TryFrom<I>,
    {
        let i = i32::try_from(i).ok().ok_or(InlineIntOverflow)?;
        // Only absurd for certain bit widths
        #[allow(clippy::absurd_extreme_comparisons)]
        if hint::likely(i >= Self::MIN.0 && i <= Self::MAX.0) {
            Ok(InlineInt(i))
        } else {
            Err(InlineIntOverflow)
        }
    }
}

#[doc(hidden)]
pub struct InlineIntOverflow;

impl TryFrom<u32> for InlineInt {
    type Error = InlineIntOverflow;

    #[inline]
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        i32::try_from(value)
            .ok()
            .and_then(|i| InlineInt::try_from(i).ok())
            .ok_or(InlineIntOverflow)
    }
}

impl TryFrom<i32> for InlineInt {
    type Error = InlineIntOverflow;

    #[inline]
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        InlineInt::try_from_impl(value)
    }
}

impl TryFrom<u64> for InlineInt {
    type Error = InlineIntOverflow;

    #[inline]
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        InlineInt::try_from_impl(value)
    }
}

impl TryFrom<i64> for InlineInt {
    type Error = InlineIntOverflow;

    #[inline]
    fn try_from(value: i64) -> Result<Self, Self::Error> {
        InlineInt::try_from_impl(value)
    }
}

impl TryFrom<usize> for InlineInt {
    type Error = InlineIntOverflow;

    #[inline]
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        InlineInt::try_from_impl(value)
    }
}

impl TryFrom<isize> for InlineInt {
    type Error = InlineIntOverflow;

    fn try_from(value: isize) -> Result<Self, Self::Error> {
        InlineInt::try_from_impl(value)
    }
}

impl<'v> TryFrom<&'v BigInt> for InlineInt {
    type Error = InlineIntOverflow;

    fn try_from(value: &'v BigInt) -> Result<Self, Self::Error> {
        InlineInt::try_from_impl(value)
    }
}

impl BitAnd for InlineInt {
    type Output = InlineInt;

    fn bitand(self, rhs: Self) -> InlineInt {
        InlineInt(self.0 & rhs.0)
    }
}

impl BitOr for InlineInt {
    type Output = InlineInt;

    fn bitor(self, rhs: Self) -> InlineInt {
        InlineInt(self.0 | rhs.0)
    }
}

impl BitXor for InlineInt {
    type Output = InlineInt;

    fn bitxor(self, rhs: Self) -> InlineInt {
        InlineInt(self.0 ^ rhs.0)
    }
}

impl Not for InlineInt {
    type Output = InlineInt;

    fn not(self) -> InlineInt {
        InlineInt(!self.0)
    }
}

impl PartialEq<i32> for InlineInt {
    #[inline]
    fn eq(&self, other: &i32) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<i32> for InlineInt {
    #[inline]
    fn partial_cmp(&self, other: &i32) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl PartialEq<InlineInt> for i32 {
    #[inline]
    fn eq(&self, other: &InlineInt) -> bool {
        *self == other.0
    }
}

impl PartialOrd<InlineInt> for i32 {
    #[inline]
    fn partial_cmp(&self, other: &InlineInt) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl Rem for InlineInt {
    type Output = InlineInt;

    fn rem(self, rhs: Self) -> InlineInt {
        InlineInt(self.0 % rhs.0)
    }
}

impl StarlarkTypeRepr for InlineInt {
    type Canonical = <StarlarkInt as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        PointerI32::starlark_type_repr()
    }
}

impl<'v> UnpackValue<'v> for InlineInt {
    type Error = crate::Error;

    fn unpack_value_impl(value: Value<'v>) -> crate::Result<Option<Self>> {
        // TODO(nga): return error on too big integer.
        Ok(value.0.unpack_int())
    }
}

impl<'v> AllocValue<'v> for InlineInt {
    fn alloc_value(self, _heap: &'v Heap) -> Value<'v> {
        Value::new_int(self)
    }
}

impl AllocFrozenValue for InlineInt {
    fn alloc_frozen_value(self, _heap: &FrozenHeap) -> FrozenValue {
        FrozenValue::new_int(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::values::types::int::inline_int::InlineInt;

    #[test]
    fn test_min_max_for_bits() {
        assert_eq!((-1, 0), InlineInt::min_max_for_bits(1));
        assert_eq!((-2, 1), InlineInt::min_max_for_bits(2));
        assert_eq!((-4, 3), InlineInt::min_max_for_bits(3));
        assert_eq!((i32::MIN, i32::MAX), InlineInt::min_max_for_bits(32));
    }
}
