use super::CheckedUnsignedAbs::{Negative, Positive};
use super::Sign::{Minus, NoSign, Plus};
use super::{BigInt, UnsignedAbs};

use crate::{IsizePromotion, UsizePromotion};

use core::cmp::Ordering::{Equal, Greater, Less};
use core::mem;
use core::ops::{Sub, SubAssign};
use num_traits::CheckedSub;

// We want to forward to BigUint::sub, but it's not clear how that will go until
// we compare both sign and magnitude.  So we duplicate this body for every
// val/ref combination, deferring that decision to BigUint's own forwarding.
macro_rules! bigint_sub {
    ($a:expr, $a_owned:expr, $a_data:expr, $b:expr, $b_owned:expr, $b_data:expr) => {
        match ($a.sign, $b.sign) {
            (_, NoSign) => $a_owned,
            (NoSign, _) => -$b_owned,
            // opposite signs => keep the sign of the left with the sum of magnitudes
            (Plus, Minus) | (Minus, Plus) => BigInt::from_biguint($a.sign, $a_data + $b_data),
            // same sign => keep or toggle the sign of the left with the difference of magnitudes
            (Plus, Plus) | (Minus, Minus) => match $a.data.cmp(&$b.data) {
                Less => BigInt::from_biguint(-$a.sign, $b_data - $a_data),
                Greater => BigInt::from_biguint($a.sign, $a_data - $b_data),
                Equal => BigInt::ZERO,
            },
        }
    };
}

impl Sub<&BigInt> for &BigInt {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: &BigInt) -> BigInt {
        bigint_sub!(
            self,
            self.clone(),
            &self.data,
            other,
            other.clone(),
            &other.data
        )
    }
}

impl Sub<BigInt> for &BigInt {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: BigInt) -> BigInt {
        bigint_sub!(self, self.clone(), &self.data, other, other, other.data)
    }
}

impl Sub<&BigInt> for BigInt {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: &BigInt) -> BigInt {
        bigint_sub!(self, self, self.data, other, other.clone(), &other.data)
    }
}

impl Sub<BigInt> for BigInt {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: BigInt) -> BigInt {
        bigint_sub!(self, self, self.data, other, other, other.data)
    }
}

impl SubAssign<&BigInt> for BigInt {
    #[inline]
    fn sub_assign(&mut self, other: &BigInt) {
        let n = mem::replace(self, Self::ZERO);
        *self = n - other;
    }
}
forward_val_assign!(impl SubAssign for BigInt, sub_assign);

promote_all_scalars!(impl Sub for BigInt, sub);
promote_all_scalars_assign!(impl SubAssign for BigInt, sub_assign);
forward_all_scalar_binop_to_val_val!(impl Sub<u32> for BigInt, sub);
forward_all_scalar_binop_to_val_val!(impl Sub<u64> for BigInt, sub);
forward_all_scalar_binop_to_val_val!(impl Sub<u128> for BigInt, sub);

impl Sub<u32> for BigInt {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: u32) -> BigInt {
        match self.sign {
            NoSign => -BigInt::from(other),
            Minus => -BigInt::from(self.data + other),
            Plus => match self.data.cmp(&From::from(other)) {
                Equal => Self::ZERO,
                Greater => BigInt::from(self.data - other),
                Less => -BigInt::from(other - self.data),
            },
        }
    }
}
impl SubAssign<u32> for BigInt {
    #[inline]
    fn sub_assign(&mut self, other: u32) {
        let n = mem::replace(self, Self::ZERO);
        *self = n - other;
    }
}

impl Sub<BigInt> for u32 {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: BigInt) -> BigInt {
        -(other - self)
    }
}

impl Sub<BigInt> for u64 {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: BigInt) -> BigInt {
        -(other - self)
    }
}

impl Sub<BigInt> for u128 {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: BigInt) -> BigInt {
        -(other - self)
    }
}

impl Sub<u64> for BigInt {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: u64) -> BigInt {
        match self.sign {
            NoSign => -BigInt::from(other),
            Minus => -BigInt::from(self.data + other),
            Plus => match self.data.cmp(&From::from(other)) {
                Equal => Self::ZERO,
                Greater => BigInt::from(self.data - other),
                Less => -BigInt::from(other - self.data),
            },
        }
    }
}

impl SubAssign<u64> for BigInt {
    #[inline]
    fn sub_assign(&mut self, other: u64) {
        let n = mem::replace(self, Self::ZERO);
        *self = n - other;
    }
}

impl Sub<u128> for BigInt {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: u128) -> BigInt {
        match self.sign {
            NoSign => -BigInt::from(other),
            Minus => -BigInt::from(self.data + other),
            Plus => match self.data.cmp(&From::from(other)) {
                Equal => Self::ZERO,
                Greater => BigInt::from(self.data - other),
                Less => -BigInt::from(other - self.data),
            },
        }
    }
}

impl SubAssign<u128> for BigInt {
    #[inline]
    fn sub_assign(&mut self, other: u128) {
        let n = mem::replace(self, Self::ZERO);
        *self = n - other;
    }
}

forward_all_scalar_binop_to_val_val!(impl Sub<i32> for BigInt, sub);
forward_all_scalar_binop_to_val_val!(impl Sub<i64> for BigInt, sub);
forward_all_scalar_binop_to_val_val!(impl Sub<i128> for BigInt, sub);

impl Sub<i32> for BigInt {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: i32) -> BigInt {
        match other.checked_uabs() {
            Positive(u) => self - u,
            Negative(u) => self + u,
        }
    }
}
impl SubAssign<i32> for BigInt {
    #[inline]
    fn sub_assign(&mut self, other: i32) {
        match other.checked_uabs() {
            Positive(u) => *self -= u,
            Negative(u) => *self += u,
        }
    }
}

impl Sub<BigInt> for i32 {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: BigInt) -> BigInt {
        match self.checked_uabs() {
            Positive(u) => u - other,
            Negative(u) => -other - u,
        }
    }
}

impl Sub<i64> for BigInt {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: i64) -> BigInt {
        match other.checked_uabs() {
            Positive(u) => self - u,
            Negative(u) => self + u,
        }
    }
}
impl SubAssign<i64> for BigInt {
    #[inline]
    fn sub_assign(&mut self, other: i64) {
        match other.checked_uabs() {
            Positive(u) => *self -= u,
            Negative(u) => *self += u,
        }
    }
}

impl Sub<BigInt> for i64 {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: BigInt) -> BigInt {
        match self.checked_uabs() {
            Positive(u) => u - other,
            Negative(u) => -other - u,
        }
    }
}

impl Sub<i128> for BigInt {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: i128) -> BigInt {
        match other.checked_uabs() {
            Positive(u) => self - u,
            Negative(u) => self + u,
        }
    }
}

impl SubAssign<i128> for BigInt {
    #[inline]
    fn sub_assign(&mut self, other: i128) {
        match other.checked_uabs() {
            Positive(u) => *self -= u,
            Negative(u) => *self += u,
        }
    }
}

impl Sub<BigInt> for i128 {
    type Output = BigInt;

    #[inline]
    fn sub(self, other: BigInt) -> BigInt {
        match self.checked_uabs() {
            Positive(u) => u - other,
            Negative(u) => -other - u,
        }
    }
}

impl CheckedSub for BigInt {
    #[inline]
    fn checked_sub(&self, v: &BigInt) -> Option<BigInt> {
        Some(self.sub(v))
    }
}
