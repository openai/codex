use super::CheckedUnsignedAbs::{Negative, Positive};
use super::Sign::{Minus, NoSign, Plus};
use super::{BigInt, UnsignedAbs};

use crate::{IsizePromotion, UsizePromotion};

use core::cmp::Ordering::{Equal, Greater, Less};
use core::iter::Sum;
use core::mem;
use core::ops::{Add, AddAssign};
use num_traits::CheckedAdd;

// We want to forward to BigUint::add, but it's not clear how that will go until
// we compare both sign and magnitude.  So we duplicate this body for every
// val/ref combination, deferring that decision to BigUint's own forwarding.
macro_rules! bigint_add {
    ($a:expr, $a_owned:expr, $a_data:expr, $b:expr, $b_owned:expr, $b_data:expr) => {
        match ($a.sign, $b.sign) {
            (_, NoSign) => $a_owned,
            (NoSign, _) => $b_owned,
            // same sign => keep the sign with the sum of magnitudes
            (Plus, Plus) | (Minus, Minus) => BigInt::from_biguint($a.sign, $a_data + $b_data),
            // opposite signs => keep the sign of the larger with the difference of magnitudes
            (Plus, Minus) | (Minus, Plus) => match $a.data.cmp(&$b.data) {
                Less => BigInt::from_biguint($b.sign, $b_data - $a_data),
                Greater => BigInt::from_biguint($a.sign, $a_data - $b_data),
                Equal => BigInt::ZERO,
            },
        }
    };
}

impl Add<&BigInt> for &BigInt {
    type Output = BigInt;

    #[inline]
    fn add(self, other: &BigInt) -> BigInt {
        bigint_add!(
            self,
            self.clone(),
            &self.data,
            other,
            other.clone(),
            &other.data
        )
    }
}

impl Add<BigInt> for &BigInt {
    type Output = BigInt;

    #[inline]
    fn add(self, other: BigInt) -> BigInt {
        bigint_add!(self, self.clone(), &self.data, other, other, other.data)
    }
}

impl Add<&BigInt> for BigInt {
    type Output = BigInt;

    #[inline]
    fn add(self, other: &BigInt) -> BigInt {
        bigint_add!(self, self, self.data, other, other.clone(), &other.data)
    }
}

impl Add<BigInt> for BigInt {
    type Output = BigInt;

    #[inline]
    fn add(self, other: BigInt) -> BigInt {
        bigint_add!(self, self, self.data, other, other, other.data)
    }
}

impl AddAssign<&BigInt> for BigInt {
    #[inline]
    fn add_assign(&mut self, other: &BigInt) {
        let n = mem::replace(self, Self::ZERO);
        *self = n + other;
    }
}
forward_val_assign!(impl AddAssign for BigInt, add_assign);

promote_all_scalars!(impl Add for BigInt, add);
promote_all_scalars_assign!(impl AddAssign for BigInt, add_assign);
forward_all_scalar_binop_to_val_val_commutative!(impl Add<u32> for BigInt, add);
forward_all_scalar_binop_to_val_val_commutative!(impl Add<u64> for BigInt, add);
forward_all_scalar_binop_to_val_val_commutative!(impl Add<u128> for BigInt, add);

impl Add<u32> for BigInt {
    type Output = BigInt;

    #[inline]
    fn add(self, other: u32) -> BigInt {
        match self.sign {
            NoSign => From::from(other),
            Plus => BigInt::from(self.data + other),
            Minus => match self.data.cmp(&From::from(other)) {
                Equal => Self::ZERO,
                Less => BigInt::from(other - self.data),
                Greater => -BigInt::from(self.data - other),
            },
        }
    }
}

impl AddAssign<u32> for BigInt {
    #[inline]
    fn add_assign(&mut self, other: u32) {
        let n = mem::replace(self, Self::ZERO);
        *self = n + other;
    }
}

impl Add<u64> for BigInt {
    type Output = BigInt;

    #[inline]
    fn add(self, other: u64) -> BigInt {
        match self.sign {
            NoSign => From::from(other),
            Plus => BigInt::from(self.data + other),
            Minus => match self.data.cmp(&From::from(other)) {
                Equal => Self::ZERO,
                Less => BigInt::from(other - self.data),
                Greater => -BigInt::from(self.data - other),
            },
        }
    }
}

impl AddAssign<u64> for BigInt {
    #[inline]
    fn add_assign(&mut self, other: u64) {
        let n = mem::replace(self, Self::ZERO);
        *self = n + other;
    }
}

impl Add<u128> for BigInt {
    type Output = BigInt;

    #[inline]
    fn add(self, other: u128) -> BigInt {
        match self.sign {
            NoSign => BigInt::from(other),
            Plus => BigInt::from(self.data + other),
            Minus => match self.data.cmp(&From::from(other)) {
                Equal => Self::ZERO,
                Less => BigInt::from(other - self.data),
                Greater => -BigInt::from(self.data - other),
            },
        }
    }
}
impl AddAssign<u128> for BigInt {
    #[inline]
    fn add_assign(&mut self, other: u128) {
        let n = mem::replace(self, Self::ZERO);
        *self = n + other;
    }
}

forward_all_scalar_binop_to_val_val_commutative!(impl Add<i32> for BigInt, add);
forward_all_scalar_binop_to_val_val_commutative!(impl Add<i64> for BigInt, add);
forward_all_scalar_binop_to_val_val_commutative!(impl Add<i128> for BigInt, add);

impl Add<i32> for BigInt {
    type Output = BigInt;

    #[inline]
    fn add(self, other: i32) -> BigInt {
        match other.checked_uabs() {
            Positive(u) => self + u,
            Negative(u) => self - u,
        }
    }
}
impl AddAssign<i32> for BigInt {
    #[inline]
    fn add_assign(&mut self, other: i32) {
        match other.checked_uabs() {
            Positive(u) => *self += u,
            Negative(u) => *self -= u,
        }
    }
}

impl Add<i64> for BigInt {
    type Output = BigInt;

    #[inline]
    fn add(self, other: i64) -> BigInt {
        match other.checked_uabs() {
            Positive(u) => self + u,
            Negative(u) => self - u,
        }
    }
}
impl AddAssign<i64> for BigInt {
    #[inline]
    fn add_assign(&mut self, other: i64) {
        match other.checked_uabs() {
            Positive(u) => *self += u,
            Negative(u) => *self -= u,
        }
    }
}

impl Add<i128> for BigInt {
    type Output = BigInt;

    #[inline]
    fn add(self, other: i128) -> BigInt {
        match other.checked_uabs() {
            Positive(u) => self + u,
            Negative(u) => self - u,
        }
    }
}
impl AddAssign<i128> for BigInt {
    #[inline]
    fn add_assign(&mut self, other: i128) {
        match other.checked_uabs() {
            Positive(u) => *self += u,
            Negative(u) => *self -= u,
        }
    }
}

impl CheckedAdd for BigInt {
    #[inline]
    fn checked_add(&self, v: &BigInt) -> Option<BigInt> {
        Some(self.add(v))
    }
}

impl_sum_iter_type!(BigInt);
