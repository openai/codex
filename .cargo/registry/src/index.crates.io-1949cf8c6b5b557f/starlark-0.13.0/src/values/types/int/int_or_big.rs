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
use std::convert::Infallible;
use std::ops::Add;
use std::ops::BitAnd;
use std::ops::BitOr;
use std::ops::BitXor;
use std::ops::Mul;
use std::ops::Neg;
use std::ops::Not;
use std::ops::Sub;
use std::str::FromStr;

use dupe::Dupe;
use num_bigint::BigInt;
use num_bigint::Sign;
use num_traits::FromPrimitive;
use num_traits::Signed;
use num_traits::ToPrimitive;
use num_traits::Zero;
use starlark_syntax::lexer::TokenInt;

use crate as starlark;
use crate::typing::Ty;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::bigint::StarlarkBigInt;
use crate::values::types::int::inline_int::InlineInt;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueLike;

#[derive(Debug, thiserror::Error)]
enum StarlarkIntError {
    #[error("Float `{0}` cannot be represented as exact integer")]
    CannotRepresentAsExact(f64),
    #[error("Floor division by zero: {0} // {1}")]
    FloorDivisionByZero(StarlarkInt, StarlarkInt),
    #[error("Modulo by zero: {0} % {1}")]
    ModuloByZero(StarlarkInt, StarlarkInt),
    #[error("Integer overflow computing left shift")]
    LeftShiftOverflow,
    #[error("Negative left shift")]
    LeftShiftNegative,
    #[error("Negative right shift")]
    RightShiftNegative,
}

#[derive(
    Debug,
    Clone,
    Eq,
    PartialEq,
    derive_more::Display,
    Hash,
    AllocValue,
    AllocFrozenValue
)]
#[doc(hidden)]
pub enum StarlarkInt {
    Small(InlineInt),
    Big(StarlarkBigInt),
}

impl StarlarkTypeRepr for StarlarkInt {
    type Canonical = Self;

    fn starlark_type_repr() -> Ty {
        Ty::int()
    }
}

#[derive(Eq, PartialEq, Copy, Clone, Dupe, Debug, derive_more::Display)]
#[doc(hidden)]
pub enum StarlarkIntRef<'v> {
    Small(InlineInt),
    Big(&'v StarlarkBigInt),
}

impl FromStr for StarlarkInt {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Not very efficient, but only used in tests.
        Ok(StarlarkInt::from(BigInt::from_str(s)?))
    }
}

impl StarlarkInt {
    pub(crate) fn from_str_radix(s: &str, base: u32) -> crate::Result<StarlarkInt> {
        Ok(StarlarkInt::from(TokenInt::from_str_radix(s, base)?))
    }

    pub(crate) fn from_f64_exact(f: f64) -> anyhow::Result<StarlarkInt> {
        let i = InlineInt::try_from(f as i32).unwrap_or(InlineInt::ZERO);
        if i.to_f64() == f {
            Ok(StarlarkInt::Small(i))
        } else {
            if let Some(i) = BigInt::from_f64(f) {
                if i.to_f64() == Some(f) {
                    return Ok(StarlarkInt::from(i));
                }
            }
            Err(StarlarkIntError::CannotRepresentAsExact(f).into())
        }
    }

    pub(crate) fn as_ref(&self) -> StarlarkIntRef {
        match self {
            StarlarkInt::Small(i) => StarlarkIntRef::Small(*i),
            StarlarkInt::Big(i) => StarlarkIntRef::Big(i),
        }
    }

    #[inline]
    fn from_impl<I>(value: I) -> StarlarkInt
    where
        I: Copy,
        InlineInt: TryFrom<I>,
        BigInt: From<I>,
    {
        match InlineInt::try_from(value) {
            Ok(i) => StarlarkInt::Small(i),
            Err(_) => StarlarkInt::Big(StarlarkBigInt::unchecked_new(BigInt::from(value))),
        }
    }
}

impl<'v> StarlarkIntRef<'v> {
    #[inline]
    pub(crate) fn unpack(value: Value<'v>) -> Option<StarlarkIntRef<'v>> {
        if let Some(int) = value.unpack_inline_int() {
            Some(StarlarkIntRef::Small(int))
        } else if let Some(int) = value.downcast_ref() {
            Some(StarlarkIntRef::Big(int))
        } else {
            None
        }
    }

    pub(crate) fn to_owned(self) -> StarlarkInt {
        match self {
            StarlarkIntRef::Small(i) => StarlarkInt::Small(i),
            StarlarkIntRef::Big(i) => StarlarkInt::Big(i.clone()),
        }
    }

    fn to_big(self) -> BigInt {
        match self {
            StarlarkIntRef::Small(i) => i.to_bigint(),
            StarlarkIntRef::Big(i) => i.get().clone(),
        }
    }

    pub(crate) fn to_f64(self) -> f64 {
        match self {
            StarlarkIntRef::Small(i) => i.to_f64(),
            StarlarkIntRef::Big(i) => i.to_f64(),
        }
    }

    pub(crate) fn to_i32(self) -> Option<i32> {
        match self {
            StarlarkIntRef::Small(i) => Some(i.to_i32()),
            StarlarkIntRef::Big(i) => i.to_i32(),
        }
    }

    pub(crate) fn to_u64(self) -> Option<u64> {
        match self {
            StarlarkIntRef::Small(i) => i.to_u64(),
            StarlarkIntRef::Big(i) => i.get().to_u64(),
        }
    }

    fn floor_div_small_small(a: InlineInt, b: InlineInt) -> anyhow::Result<StarlarkInt> {
        if b == 0 {
            return Err(StarlarkIntError::FloorDivisionByZero(
                StarlarkInt::Small(a),
                StarlarkInt::Small(b),
            )
            .into());
        }
        let sig = b.signum() * a.signum();
        let offset = if sig < 0 && a % b != 0 { 1 } else { 0 };
        match a.checked_div(b) {
            Some(div) => Ok(StarlarkInt::Small(
                div.checked_sub_i32(offset)
                    .ok_or_else(|| anyhow::anyhow!("unreachable"))?,
            )),
            None => Self::floor_div_big_big(&a.to_bigint(), &b.to_bigint()),
        }
    }

    fn is_negative(self) -> bool {
        match self {
            StarlarkIntRef::Small(i) => i < 0,
            StarlarkIntRef::Big(i) => i.get().is_negative(),
        }
    }

    fn is_zero(self) -> bool {
        match self {
            StarlarkIntRef::Small(i) => i == 0,
            StarlarkIntRef::Big(_) => false,
        }
    }

    fn signum_big(b: &BigInt) -> i32 {
        match b.sign() {
            Sign::Plus => 1,
            Sign::Minus => -1,
            Sign::NoSign => 0,
        }
    }

    fn floor_div_big_big(a: &BigInt, b: &BigInt) -> anyhow::Result<StarlarkInt> {
        if b.is_zero() {
            return Err(StarlarkIntError::FloorDivisionByZero(
                StarlarkInt::from(a.clone()),
                StarlarkInt::from(b.clone()),
            )
            .into());
        }
        let sig = Self::signum_big(b) * Self::signum_big(a);
        // TODO(nga): optimize.
        let offset = if sig < 0 && (a % b).is_zero().not() {
            1
        } else {
            0
        };
        Ok(StarlarkInt::from((a / b) - offset))
    }

    /// `//`.
    pub(crate) fn floor_div(self, other: StarlarkIntRef) -> anyhow::Result<StarlarkInt> {
        match (self, other) {
            (StarlarkIntRef::Small(a), StarlarkIntRef::Small(b)) => {
                Self::floor_div_small_small(a, b)
            }
            (StarlarkIntRef::Small(a), StarlarkIntRef::Big(b)) => {
                Self::floor_div_big_big(&a.to_bigint(), b.get())
            }
            (StarlarkIntRef::Big(a), StarlarkIntRef::Small(b)) => {
                Self::floor_div_big_big(a.get(), &b.to_bigint())
            }
            (StarlarkIntRef::Big(a), StarlarkIntRef::Big(b)) => {
                Self::floor_div_big_big(a.get(), b.get())
            }
        }
    }

    fn percent_small(a: InlineInt, b: InlineInt) -> anyhow::Result<InlineInt> {
        if b == 0 {
            return Err(StarlarkIntError::ModuloByZero(
                StarlarkInt::Small(a),
                StarlarkInt::Small(b),
            )
            .into());
        }
        // In Rust `i32::min_value() % -1` is overflow, but we should eval it to zero.
        if a == i32::MIN && b == -1 {
            return Ok(InlineInt::ZERO);
        }
        let r = a % b;
        if r == 0 {
            Ok(InlineInt::ZERO)
        } else {
            Ok(if b.signum() != r.signum() {
                r.checked_add(b)
                    .ok_or_else(|| anyhow::anyhow!("unreachable"))?
            } else {
                r
            })
        }
    }

    fn percent_big(a: &BigInt, b: &BigInt) -> anyhow::Result<StarlarkInt> {
        if b.is_zero() {
            return Err(StarlarkIntError::ModuloByZero(
                StarlarkInt::from(a.clone()),
                StarlarkInt::from(b.clone()),
            )
            .into());
        }
        let r = a % b;
        if r.is_zero() {
            Ok(StarlarkInt::Small(InlineInt::ZERO))
        } else {
            Ok(StarlarkInt::from(if b.sign() != r.sign() {
                r + b
            } else {
                r
            }))
        }
    }

    /// `%`.
    pub(crate) fn percent(self, other: StarlarkIntRef) -> anyhow::Result<StarlarkInt> {
        match (self, other) {
            (StarlarkIntRef::Small(a), StarlarkIntRef::Small(b)) => {
                Ok(StarlarkInt::Small(Self::percent_small(a, b)?))
            }
            (StarlarkIntRef::Small(a), StarlarkIntRef::Big(b)) => {
                Self::percent_big(&a.to_bigint(), b.get())
            }
            (StarlarkIntRef::Big(a), StarlarkIntRef::Small(b)) => {
                Self::percent_big(a.get(), &b.to_bigint())
            }
            (StarlarkIntRef::Big(a), StarlarkIntRef::Big(b)) => Self::percent_big(a.get(), b.get()),
        }
    }

    /// `<<`.
    pub(crate) fn left_shift(self, other: StarlarkIntRef) -> anyhow::Result<StarlarkInt> {
        // Handle the most common case first.
        if let (StarlarkIntRef::Small(a), StarlarkIntRef::Small(b)) = (self, other) {
            if let Some(b) = b.to_u32() {
                if let Some(r) = a.checked_shl(b) {
                    return Ok(StarlarkInt::Small(r));
                }
            }
        }

        if other.is_negative() {
            return Err(StarlarkIntError::LeftShiftNegative.into());
        }
        if self.is_zero() || other.is_zero() {
            return Ok(self.to_owned());
        }
        if other > 100_000 {
            // Limit the size of the BigInt to avoid accidentally consuming
            // too much memory. 100_000 is practically enough for most use cases.
            return Err(StarlarkIntError::LeftShiftOverflow.into());
        }

        match other {
            StarlarkIntRef::Big(_) => Err(StarlarkIntError::LeftShiftOverflow.into()),
            StarlarkIntRef::Small(b) => {
                // No overflow, checked above.
                let b = b.to_u64().unwrap();
                Ok(StarlarkInt::from(self.to_big() << b))
            }
        }
    }

    /// `>>`.
    pub(crate) fn right_shift(self, other: StarlarkIntRef) -> anyhow::Result<StarlarkInt> {
        // Handle the most common case first.
        if let (StarlarkIntRef::Small(a), StarlarkIntRef::Small(b)) = (self, other) {
            if let Some(b) = b.to_u32() {
                if let Some(r) = a.checked_shr(b) {
                    return Ok(StarlarkInt::Small(r));
                }
            }
        }

        if other.is_negative() {
            return Err(StarlarkIntError::RightShiftNegative.into());
        }
        if self.is_zero() || other.is_zero() {
            return Ok(self.to_owned());
        }
        let Some(other) = other.to_u64() else {
            return if self.is_negative() {
                Ok(StarlarkInt::Small(InlineInt::MINUS_ONE))
            } else {
                Ok(StarlarkInt::Small(InlineInt::ZERO))
            };
        };

        match self {
            StarlarkIntRef::Small(a) => {
                if a < 0 {
                    Ok(StarlarkInt::Small(InlineInt::MINUS_ONE))
                } else {
                    Ok(StarlarkInt::Small(InlineInt::ZERO))
                }
            }
            StarlarkIntRef::Big(a) => Ok(StarlarkInt::from(a.get() >> other)),
        }
    }

    pub(crate) fn abs(self) -> StarlarkInt {
        match self {
            StarlarkIntRef::Small(i) => i.abs(),
            StarlarkIntRef::Big(i) => StarlarkInt::from(i.get().abs()),
        }
    }
}

impl<'v> StarlarkTypeRepr for StarlarkIntRef<'v> {
    type Canonical = <StarlarkInt as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        StarlarkInt::starlark_type_repr()
    }
}

impl<'v> UnpackValue<'v> for StarlarkIntRef<'v> {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        Ok(StarlarkIntRef::unpack(value))
    }
}

impl From<i32> for StarlarkInt {
    #[inline]
    fn from(value: i32) -> Self {
        StarlarkInt::from_impl(value)
    }
}

impl From<BigInt> for StarlarkInt {
    fn from(value: BigInt) -> Self {
        match InlineInt::try_from(&value) {
            Ok(i) => StarlarkInt::Small(i),
            Err(_) => StarlarkInt::Big(StarlarkBigInt::unchecked_new(value)),
        }
    }
}

impl From<TokenInt> for StarlarkInt {
    fn from(value: TokenInt) -> Self {
        match value {
            TokenInt::I32(i) => StarlarkInt::from(i),
            TokenInt::BigInt(i) => StarlarkInt::from(i),
        }
    }
}

impl From<u32> for StarlarkInt {
    #[inline]
    fn from(value: u32) -> Self {
        StarlarkInt::from_impl(value)
    }
}

impl From<i64> for StarlarkInt {
    #[inline]
    fn from(value: i64) -> Self {
        StarlarkInt::from_impl(value)
    }
}

impl From<u64> for StarlarkInt {
    #[inline]
    fn from(value: u64) -> Self {
        StarlarkInt::from_impl(value)
    }
}

impl From<isize> for StarlarkInt {
    #[inline]
    fn from(value: isize) -> Self {
        StarlarkInt::from_impl(value)
    }
}

impl From<usize> for StarlarkInt {
    #[inline]
    fn from(value: usize) -> Self {
        StarlarkInt::from_impl(value)
    }
}

impl<'v> BitAnd for StarlarkIntRef<'v> {
    type Output = StarlarkInt;

    fn bitand(self, other: Self) -> StarlarkInt {
        match (self, other) {
            (StarlarkIntRef::Small(a), StarlarkIntRef::Small(b)) => StarlarkInt::Small(a & b),
            (a, b) => StarlarkInt::from(a.to_big() & b.to_big()),
        }
    }
}

impl<'v> BitOr for StarlarkIntRef<'v> {
    type Output = StarlarkInt;

    fn bitor(self, other: Self) -> StarlarkInt {
        match (self, other) {
            (StarlarkIntRef::Small(a), StarlarkIntRef::Small(b)) => StarlarkInt::Small(a | b),
            (a, b) => StarlarkInt::from(a.to_big() | b.to_big()),
        }
    }
}

impl<'v> BitXor for StarlarkIntRef<'v> {
    type Output = StarlarkInt;

    fn bitxor(self, other: Self) -> StarlarkInt {
        match (self, other) {
            (StarlarkIntRef::Small(a), StarlarkIntRef::Small(b)) => StarlarkInt::Small(a ^ b),
            (a, b) => StarlarkInt::from(a.to_big() ^ b.to_big()),
        }
    }
}

impl<'v> Not for StarlarkIntRef<'v> {
    type Output = StarlarkInt;

    fn not(self) -> StarlarkInt {
        match self {
            StarlarkIntRef::Small(a) => StarlarkInt::Small(!a),
            a => StarlarkInt::from(!a.to_big()),
        }
    }
}

impl<'v> Neg for StarlarkIntRef<'v> {
    type Output = StarlarkInt;

    fn neg(self) -> StarlarkInt {
        if let StarlarkIntRef::Small(i) = self {
            if let Some(n) = i.checked_neg() {
                return StarlarkInt::Small(n);
            }
        }
        StarlarkInt::from(-self.to_big())
    }
}

impl Neg for StarlarkInt {
    type Output = StarlarkInt;

    fn neg(self) -> StarlarkInt {
        // TODO(nga): can negate without allocating in most cases.
        -self.as_ref()
    }
}

impl<'v> Add for StarlarkIntRef<'v> {
    type Output = StarlarkInt;

    fn add(self, other: Self) -> StarlarkInt {
        if let (StarlarkIntRef::Small(a), StarlarkIntRef::Small(b)) = (self, other) {
            if let Some(c) = a.checked_add(b) {
                return StarlarkInt::Small(c);
            }
        }
        StarlarkInt::from(self.to_big() + other.to_big())
    }
}

impl<'v> Sub for StarlarkIntRef<'v> {
    type Output = StarlarkInt;

    fn sub(self, other: Self) -> StarlarkInt {
        if let (StarlarkIntRef::Small(a), StarlarkIntRef::Small(b)) = (self, other) {
            if let Some(c) = a.checked_sub(b) {
                return StarlarkInt::Small(c);
            }
        }
        StarlarkInt::from(self.to_big() - other.to_big())
    }
}

impl<'v> Mul<i32> for StarlarkIntRef<'v> {
    type Output = StarlarkInt;

    fn mul(self, rhs: i32) -> Self::Output {
        match self {
            StarlarkIntRef::Small(a) => {
                if let Some(c) = a.checked_mul_i32(rhs) {
                    return StarlarkInt::Small(c);
                }
                StarlarkInt::from(a.to_bigint() * rhs)
            }
            StarlarkIntRef::Big(b) => StarlarkInt::from(b.get() * rhs),
        }
    }
}

impl<'v> Mul<StarlarkIntRef<'v>> for i32 {
    type Output = StarlarkInt;

    fn mul(self, rhs: StarlarkIntRef<'v>) -> Self::Output {
        rhs * self
    }
}

impl<'v> Mul for StarlarkIntRef<'v> {
    type Output = StarlarkInt;

    fn mul(self, other: Self) -> StarlarkInt {
        match (self, other) {
            (StarlarkIntRef::Small(a), b) => a.to_i32() * b,
            (a, StarlarkIntRef::Small(b)) => a * b.to_i32(),
            (StarlarkIntRef::Big(a), StarlarkIntRef::Big(b)) => {
                StarlarkInt::from(a.get() * b.get())
            }
        }
    }
}

impl<'v> PartialOrd for StarlarkIntRef<'v> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'v> Ord for StarlarkIntRef<'v> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (StarlarkIntRef::Small(a), StarlarkIntRef::Small(b)) => a.cmp(b),
            (StarlarkIntRef::Big(a), StarlarkIntRef::Big(b)) => a.cmp(b),
            (StarlarkIntRef::Small(a), StarlarkIntRef::Big(b)) => {
                StarlarkBigInt::cmp_small_big(*a, b)
            }
            (StarlarkIntRef::Big(a), StarlarkIntRef::Small(b)) => {
                StarlarkBigInt::cmp_big_small(a, *b)
            }
        }
    }
}

impl<'v> PartialEq<i32> for StarlarkIntRef<'v> {
    fn eq(&self, other: &i32) -> bool {
        *self == StarlarkInt::from(*other).as_ref()
    }
}

impl<'v> PartialOrd<i32> for StarlarkIntRef<'v> {
    fn partial_cmp(&self, other: &i32) -> Option<Ordering> {
        // TODO(nga): this is inefficient if `i32` cannot fit in `InlineInt`.
        self.partial_cmp(&StarlarkInt::from(*other).as_ref())
    }
}

impl<'v> PartialEq<StarlarkIntRef<'v>> for i32 {
    fn eq(&self, other: &StarlarkIntRef<'v>) -> bool {
        StarlarkInt::from(*self).as_ref() == *other
    }
}

impl<'v> PartialOrd<StarlarkIntRef<'v>> for i32 {
    // TODO(nga): this is inefficient if `i32` cannot fit in `InlineInt`.
    fn partial_cmp(&self, other: &StarlarkIntRef<'v>) -> Option<Ordering> {
        StarlarkInt::from(*self).as_ref().partial_cmp(other)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::values::types::int::int_or_big::StarlarkInt;

    fn int(s: &str) -> StarlarkInt {
        StarlarkInt::from_str(s).unwrap()
    }

    fn floor_div(a: &str, b: &str) -> String {
        int(a)
            .as_ref()
            .floor_div(int(b).as_ref())
            .unwrap()
            .to_string()
    }

    fn percent(a: &str, b: &str) -> String {
        int(a)
            .as_ref()
            .percent(int(b).as_ref())
            .unwrap()
            .to_string()
    }

    #[test]
    fn test_floor_div_big() {
        assert_eq!(
            "2",
            floor_div("600000000000000000005", "300000000000000000000")
        );
        assert_eq!(
            "-3",
            floor_div("600000000000000000005", "-300000000000000000000")
        );
        assert_eq!(
            "-3",
            floor_div("-600000000000000000005", "300000000000000000000")
        );
        assert_eq!(
            "2",
            floor_div("-600000000000000000005", "-300000000000000000000")
        );
    }

    #[test]
    fn test_floor_div_big_small() {
        assert_eq!(
            "200000000000000000001",
            floor_div("600000000000000000005", "3"),
        );
        assert_eq!(
            "-200000000000000000002",
            floor_div("600000000000000000005", "-3"),
        );
        assert_eq!(
            "-200000000000000000002",
            floor_div("-600000000000000000005", "3"),
        );
        assert_eq!(
            "200000000000000000001",
            floor_div("-600000000000000000005", "-3"),
        );
    }

    #[test]
    fn test_floor_div_small_big() {
        assert_eq!("0", floor_div("3", "600000000000000000000"));
        assert_eq!("0", floor_div("-3", "-600000000000000000000"));
        assert_eq!("-1", floor_div("3", "-600000000000000000000"));
        assert_eq!("-1", floor_div("-3", "600000000000000000000"));
    }

    #[test]
    fn test_floor_div_small() {
        assert_eq!("4", floor_div("13", "3"));
        assert_eq!("-5", floor_div("13", "-3"));
        assert_eq!("-5", floor_div("-13", "3"));
        assert_eq!("4", floor_div("-13", "-3"));
    }

    #[test]
    fn test_percent_big() {
        assert_eq!(
            "7",
            percent("600000000000000000007", "200000000000000000000")
        );
        assert_eq!(
            "-199999999999999999993",
            percent("600000000000000000007", "-200000000000000000000")
        );
        assert_eq!(
            "199999999999999999993",
            percent("-600000000000000000007", "200000000000000000000")
        );
        assert_eq!(
            "-7",
            percent("-600000000000000000007", "-200000000000000000000")
        );
    }

    #[test]
    fn test_percent_big_small() {
        assert_eq!("7", percent("600000000000000000007", "20"));
        assert_eq!("-13", percent("600000000000000000007", "-20"));
        assert_eq!("13", percent("-600000000000000000007", "20"));
        assert_eq!("-7", percent("-600000000000000000007", "-20"));
    }

    #[test]
    fn test_percent_small_big() {
        assert_eq!("3", percent("3", "600000000000000000001"));
        assert_eq!(
            "-599999999999999999998",
            percent("3", "-600000000000000000001")
        );
        assert_eq!(
            "599999999999999999998",
            percent("-3", "600000000000000000001")
        );
        assert_eq!("-3", percent("-3", "-600000000000000000001"));
    }

    #[test]
    fn test_percent_small() {
        assert_eq!("2", percent("5", "3"));
        assert_eq!("-1", percent("5", "-3"));
        assert_eq!("1", percent("-5", "3"));
        assert_eq!("-2", percent("-5", "-3"));
    }
}
