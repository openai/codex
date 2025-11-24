use super::BigInt;
use super::Sign::{self, Minus, Plus};

use crate::BigUint;

use num_integer::Integer;
use num_traits::{Pow, Signed, Zero};

/// Help function for pow
///
/// Computes the effect of the exponent on the sign.
#[inline]
fn powsign<T: Integer>(sign: Sign, other: &T) -> Sign {
    if other.is_zero() {
        Plus
    } else if sign != Minus || other.is_odd() {
        sign
    } else {
        -sign
    }
}

macro_rules! pow_impl {
    ($T:ty) => {
        impl Pow<$T> for BigInt {
            type Output = BigInt;

            #[inline]
            fn pow(self, rhs: $T) -> BigInt {
                BigInt::from_biguint(powsign(self.sign, &rhs), self.data.pow(rhs))
            }
        }

        impl Pow<&$T> for BigInt {
            type Output = BigInt;

            #[inline]
            fn pow(self, rhs: &$T) -> BigInt {
                BigInt::from_biguint(powsign(self.sign, rhs), self.data.pow(rhs))
            }
        }

        impl Pow<$T> for &BigInt {
            type Output = BigInt;

            #[inline]
            fn pow(self, rhs: $T) -> BigInt {
                BigInt::from_biguint(powsign(self.sign, &rhs), Pow::pow(&self.data, rhs))
            }
        }

        impl Pow<&$T> for &BigInt {
            type Output = BigInt;

            #[inline]
            fn pow(self, rhs: &$T) -> BigInt {
                BigInt::from_biguint(powsign(self.sign, rhs), Pow::pow(&self.data, rhs))
            }
        }
    };
}

pow_impl!(u8);
pow_impl!(u16);
pow_impl!(u32);
pow_impl!(u64);
pow_impl!(usize);
pow_impl!(u128);
pow_impl!(BigUint);

pub(super) fn modpow(x: &BigInt, exponent: &BigInt, modulus: &BigInt) -> BigInt {
    assert!(
        !exponent.is_negative(),
        "negative exponentiation is not supported!"
    );
    assert!(
        !modulus.is_zero(),
        "attempt to calculate with zero modulus!"
    );

    let result = x.data.modpow(&exponent.data, &modulus.data);
    if result.is_zero() {
        return BigInt::ZERO;
    }

    // The sign of the result follows the modulus, like `mod_floor`.
    let (sign, mag) = match (x.is_negative() && exponent.is_odd(), modulus.is_negative()) {
        (false, false) => (Plus, result),
        (true, false) => (Plus, &modulus.data - result),
        (false, true) => (Minus, &modulus.data - result),
        (true, true) => (Minus, result),
    };
    BigInt::from_biguint(sign, mag)
}
