use super::CheckedUnsignedAbs::{Negative, Positive};
use super::Sign::NoSign;
use super::{BigInt, UnsignedAbs};

use crate::{IsizePromotion, UsizePromotion};

use core::ops::{Div, DivAssign, Rem, RemAssign};
use num_integer::Integer;
use num_traits::{CheckedDiv, CheckedEuclid, Euclid, Signed, ToPrimitive, Zero};

forward_all_binop_to_ref_ref!(impl Div for BigInt, div);

impl Div<&BigInt> for &BigInt {
    type Output = BigInt;

    #[inline]
    fn div(self, other: &BigInt) -> BigInt {
        let (q, _) = self.div_rem(other);
        q
    }
}

impl DivAssign<&BigInt> for BigInt {
    #[inline]
    fn div_assign(&mut self, other: &BigInt) {
        *self = &*self / other;
    }
}
forward_val_assign!(impl DivAssign for BigInt, div_assign);

promote_all_scalars!(impl Div for BigInt, div);
promote_all_scalars_assign!(impl DivAssign for BigInt, div_assign);
forward_all_scalar_binop_to_val_val!(impl Div<u32> for BigInt, div);
forward_all_scalar_binop_to_val_val!(impl Div<u64> for BigInt, div);
forward_all_scalar_binop_to_val_val!(impl Div<u128> for BigInt, div);

impl Div<u32> for BigInt {
    type Output = BigInt;

    #[inline]
    fn div(self, other: u32) -> BigInt {
        BigInt::from_biguint(self.sign, self.data / other)
    }
}

impl DivAssign<u32> for BigInt {
    #[inline]
    fn div_assign(&mut self, other: u32) {
        self.data /= other;
        if self.data.is_zero() {
            self.sign = NoSign;
        }
    }
}

impl Div<BigInt> for u32 {
    type Output = BigInt;

    #[inline]
    fn div(self, other: BigInt) -> BigInt {
        BigInt::from_biguint(other.sign, self / other.data)
    }
}

impl Div<u64> for BigInt {
    type Output = BigInt;

    #[inline]
    fn div(self, other: u64) -> BigInt {
        BigInt::from_biguint(self.sign, self.data / other)
    }
}

impl DivAssign<u64> for BigInt {
    #[inline]
    fn div_assign(&mut self, other: u64) {
        self.data /= other;
        if self.data.is_zero() {
            self.sign = NoSign;
        }
    }
}

impl Div<BigInt> for u64 {
    type Output = BigInt;

    #[inline]
    fn div(self, other: BigInt) -> BigInt {
        BigInt::from_biguint(other.sign, self / other.data)
    }
}

impl Div<u128> for BigInt {
    type Output = BigInt;

    #[inline]
    fn div(self, other: u128) -> BigInt {
        BigInt::from_biguint(self.sign, self.data / other)
    }
}

impl DivAssign<u128> for BigInt {
    #[inline]
    fn div_assign(&mut self, other: u128) {
        self.data /= other;
        if self.data.is_zero() {
            self.sign = NoSign;
        }
    }
}

impl Div<BigInt> for u128 {
    type Output = BigInt;

    #[inline]
    fn div(self, other: BigInt) -> BigInt {
        BigInt::from_biguint(other.sign, self / other.data)
    }
}

forward_all_scalar_binop_to_val_val!(impl Div<i32> for BigInt, div);
forward_all_scalar_binop_to_val_val!(impl Div<i64> for BigInt, div);
forward_all_scalar_binop_to_val_val!(impl Div<i128> for BigInt, div);

impl Div<i32> for BigInt {
    type Output = BigInt;

    #[inline]
    fn div(self, other: i32) -> BigInt {
        match other.checked_uabs() {
            Positive(u) => self / u,
            Negative(u) => -self / u,
        }
    }
}

impl DivAssign<i32> for BigInt {
    #[inline]
    fn div_assign(&mut self, other: i32) {
        match other.checked_uabs() {
            Positive(u) => *self /= u,
            Negative(u) => {
                self.sign = -self.sign;
                *self /= u;
            }
        }
    }
}

impl Div<BigInt> for i32 {
    type Output = BigInt;

    #[inline]
    fn div(self, other: BigInt) -> BigInt {
        match self.checked_uabs() {
            Positive(u) => u / other,
            Negative(u) => u / -other,
        }
    }
}

impl Div<i64> for BigInt {
    type Output = BigInt;

    #[inline]
    fn div(self, other: i64) -> BigInt {
        match other.checked_uabs() {
            Positive(u) => self / u,
            Negative(u) => -self / u,
        }
    }
}

impl DivAssign<i64> for BigInt {
    #[inline]
    fn div_assign(&mut self, other: i64) {
        match other.checked_uabs() {
            Positive(u) => *self /= u,
            Negative(u) => {
                self.sign = -self.sign;
                *self /= u;
            }
        }
    }
}

impl Div<BigInt> for i64 {
    type Output = BigInt;

    #[inline]
    fn div(self, other: BigInt) -> BigInt {
        match self.checked_uabs() {
            Positive(u) => u / other,
            Negative(u) => u / -other,
        }
    }
}

impl Div<i128> for BigInt {
    type Output = BigInt;

    #[inline]
    fn div(self, other: i128) -> BigInt {
        match other.checked_uabs() {
            Positive(u) => self / u,
            Negative(u) => -self / u,
        }
    }
}

impl DivAssign<i128> for BigInt {
    #[inline]
    fn div_assign(&mut self, other: i128) {
        match other.checked_uabs() {
            Positive(u) => *self /= u,
            Negative(u) => {
                self.sign = -self.sign;
                *self /= u;
            }
        }
    }
}

impl Div<BigInt> for i128 {
    type Output = BigInt;

    #[inline]
    fn div(self, other: BigInt) -> BigInt {
        match self.checked_uabs() {
            Positive(u) => u / other,
            Negative(u) => u / -other,
        }
    }
}

forward_all_binop_to_ref_ref!(impl Rem for BigInt, rem);

impl Rem<&BigInt> for &BigInt {
    type Output = BigInt;

    #[inline]
    fn rem(self, other: &BigInt) -> BigInt {
        if let Some(other) = other.to_u32() {
            self % other
        } else if let Some(other) = other.to_i32() {
            self % other
        } else {
            let (_, r) = self.div_rem(other);
            r
        }
    }
}

impl RemAssign<&BigInt> for BigInt {
    #[inline]
    fn rem_assign(&mut self, other: &BigInt) {
        *self = &*self % other;
    }
}
forward_val_assign!(impl RemAssign for BigInt, rem_assign);

promote_all_scalars!(impl Rem for BigInt, rem);
promote_all_scalars_assign!(impl RemAssign for BigInt, rem_assign);
forward_all_scalar_binop_to_val_val!(impl Rem<u32> for BigInt, rem);
forward_all_scalar_binop_to_val_val!(impl Rem<u64> for BigInt, rem);
forward_all_scalar_binop_to_val_val!(impl Rem<u128> for BigInt, rem);

impl Rem<u32> for BigInt {
    type Output = BigInt;

    #[inline]
    fn rem(self, other: u32) -> BigInt {
        BigInt::from_biguint(self.sign, self.data % other)
    }
}

impl RemAssign<u32> for BigInt {
    #[inline]
    fn rem_assign(&mut self, other: u32) {
        self.data %= other;
        if self.data.is_zero() {
            self.sign = NoSign;
        }
    }
}

impl Rem<BigInt> for u32 {
    type Output = BigInt;

    #[inline]
    fn rem(self, other: BigInt) -> BigInt {
        BigInt::from(self % other.data)
    }
}

impl Rem<u64> for BigInt {
    type Output = BigInt;

    #[inline]
    fn rem(self, other: u64) -> BigInt {
        BigInt::from_biguint(self.sign, self.data % other)
    }
}

impl RemAssign<u64> for BigInt {
    #[inline]
    fn rem_assign(&mut self, other: u64) {
        self.data %= other;
        if self.data.is_zero() {
            self.sign = NoSign;
        }
    }
}

impl Rem<BigInt> for u64 {
    type Output = BigInt;

    #[inline]
    fn rem(self, other: BigInt) -> BigInt {
        BigInt::from(self % other.data)
    }
}

impl Rem<u128> for BigInt {
    type Output = BigInt;

    #[inline]
    fn rem(self, other: u128) -> BigInt {
        BigInt::from_biguint(self.sign, self.data % other)
    }
}

impl RemAssign<u128> for BigInt {
    #[inline]
    fn rem_assign(&mut self, other: u128) {
        self.data %= other;
        if self.data.is_zero() {
            self.sign = NoSign;
        }
    }
}

impl Rem<BigInt> for u128 {
    type Output = BigInt;

    #[inline]
    fn rem(self, other: BigInt) -> BigInt {
        BigInt::from(self % other.data)
    }
}

forward_all_scalar_binop_to_val_val!(impl Rem<i32> for BigInt, rem);
forward_all_scalar_binop_to_val_val!(impl Rem<i64> for BigInt, rem);
forward_all_scalar_binop_to_val_val!(impl Rem<i128> for BigInt, rem);

impl Rem<i32> for BigInt {
    type Output = BigInt;

    #[inline]
    fn rem(self, other: i32) -> BigInt {
        self % other.unsigned_abs()
    }
}

impl RemAssign<i32> for BigInt {
    #[inline]
    fn rem_assign(&mut self, other: i32) {
        *self %= other.unsigned_abs();
    }
}

impl Rem<BigInt> for i32 {
    type Output = BigInt;

    #[inline]
    fn rem(self, other: BigInt) -> BigInt {
        match self.checked_uabs() {
            Positive(u) => u % other,
            Negative(u) => -(u % other),
        }
    }
}

impl Rem<i64> for BigInt {
    type Output = BigInt;

    #[inline]
    fn rem(self, other: i64) -> BigInt {
        self % other.unsigned_abs()
    }
}

impl RemAssign<i64> for BigInt {
    #[inline]
    fn rem_assign(&mut self, other: i64) {
        *self %= other.unsigned_abs();
    }
}

impl Rem<BigInt> for i64 {
    type Output = BigInt;

    #[inline]
    fn rem(self, other: BigInt) -> BigInt {
        match self.checked_uabs() {
            Positive(u) => u % other,
            Negative(u) => -(u % other),
        }
    }
}

impl Rem<i128> for BigInt {
    type Output = BigInt;

    #[inline]
    fn rem(self, other: i128) -> BigInt {
        self % other.unsigned_abs()
    }
}

impl RemAssign<i128> for BigInt {
    #[inline]
    fn rem_assign(&mut self, other: i128) {
        *self %= other.unsigned_abs();
    }
}

impl Rem<BigInt> for i128 {
    type Output = BigInt;

    #[inline]
    fn rem(self, other: BigInt) -> BigInt {
        match self.checked_uabs() {
            Positive(u) => u % other,
            Negative(u) => -(u % other),
        }
    }
}

impl CheckedDiv for BigInt {
    #[inline]
    fn checked_div(&self, v: &BigInt) -> Option<BigInt> {
        if v.is_zero() {
            return None;
        }
        Some(self.div(v))
    }
}

impl CheckedEuclid for BigInt {
    #[inline]
    fn checked_div_euclid(&self, v: &BigInt) -> Option<BigInt> {
        if v.is_zero() {
            return None;
        }
        Some(self.div_euclid(v))
    }

    #[inline]
    fn checked_rem_euclid(&self, v: &BigInt) -> Option<BigInt> {
        if v.is_zero() {
            return None;
        }
        Some(self.rem_euclid(v))
    }

    fn checked_div_rem_euclid(&self, v: &Self) -> Option<(Self, Self)> {
        Some(self.div_rem_euclid(v))
    }
}

impl Euclid for BigInt {
    #[inline]
    fn div_euclid(&self, v: &BigInt) -> BigInt {
        let (q, r) = self.div_rem(v);
        if r.is_negative() {
            if v.is_positive() {
                q - 1
            } else {
                q + 1
            }
        } else {
            q
        }
    }

    #[inline]
    fn rem_euclid(&self, v: &BigInt) -> BigInt {
        let r = self % v;
        if r.is_negative() {
            if v.is_positive() {
                r + v
            } else {
                r - v
            }
        } else {
            r
        }
    }

    fn div_rem_euclid(&self, v: &Self) -> (Self, Self) {
        let (q, r) = self.div_rem(v);
        if r.is_negative() {
            if v.is_positive() {
                (q - 1, r + v)
            } else {
                (q + 1, r - v)
            }
        } else {
            (q, r)
        }
    }
}
