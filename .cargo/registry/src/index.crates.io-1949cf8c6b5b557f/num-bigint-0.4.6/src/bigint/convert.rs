use super::Sign::{self, Minus, NoSign, Plus};
use super::{BigInt, ToBigInt};

use crate::TryFromBigIntError;
use crate::{BigUint, ParseBigIntError, ToBigUint};

use alloc::vec::Vec;
use core::cmp::Ordering::{Equal, Greater, Less};
use core::convert::TryFrom;
use core::str::{self, FromStr};
use num_traits::{FromPrimitive, Num, One, ToPrimitive, Zero};

impl FromStr for BigInt {
    type Err = ParseBigIntError;

    #[inline]
    fn from_str(s: &str) -> Result<BigInt, ParseBigIntError> {
        BigInt::from_str_radix(s, 10)
    }
}

impl Num for BigInt {
    type FromStrRadixErr = ParseBigIntError;

    /// Creates and initializes a [`BigInt`].
    #[inline]
    fn from_str_radix(mut s: &str, radix: u32) -> Result<BigInt, ParseBigIntError> {
        let sign = if let Some(tail) = s.strip_prefix('-') {
            if !tail.starts_with('+') {
                s = tail
            }
            Minus
        } else {
            Plus
        };
        let bu = BigUint::from_str_radix(s, radix)?;
        Ok(BigInt::from_biguint(sign, bu))
    }
}

impl ToPrimitive for BigInt {
    #[inline]
    fn to_i64(&self) -> Option<i64> {
        match self.sign {
            Plus => self.data.to_i64(),
            NoSign => Some(0),
            Minus => {
                let n = self.data.to_u64()?;
                let m: u64 = 1 << 63;
                match n.cmp(&m) {
                    Less => Some(-(n as i64)),
                    Equal => Some(i64::MIN),
                    Greater => None,
                }
            }
        }
    }

    #[inline]
    fn to_i128(&self) -> Option<i128> {
        match self.sign {
            Plus => self.data.to_i128(),
            NoSign => Some(0),
            Minus => {
                let n = self.data.to_u128()?;
                let m: u128 = 1 << 127;
                match n.cmp(&m) {
                    Less => Some(-(n as i128)),
                    Equal => Some(i128::MIN),
                    Greater => None,
                }
            }
        }
    }

    #[inline]
    fn to_u64(&self) -> Option<u64> {
        match self.sign {
            Plus => self.data.to_u64(),
            NoSign => Some(0),
            Minus => None,
        }
    }

    #[inline]
    fn to_u128(&self) -> Option<u128> {
        match self.sign {
            Plus => self.data.to_u128(),
            NoSign => Some(0),
            Minus => None,
        }
    }

    #[inline]
    fn to_f32(&self) -> Option<f32> {
        let n = self.data.to_f32()?;
        Some(if self.sign == Minus { -n } else { n })
    }

    #[inline]
    fn to_f64(&self) -> Option<f64> {
        let n = self.data.to_f64()?;
        Some(if self.sign == Minus { -n } else { n })
    }
}

macro_rules! impl_try_from_bigint {
    ($T:ty, $to_ty:path) => {
        impl TryFrom<&BigInt> for $T {
            type Error = TryFromBigIntError<()>;

            #[inline]
            fn try_from(value: &BigInt) -> Result<$T, TryFromBigIntError<()>> {
                $to_ty(value).ok_or(TryFromBigIntError::new(()))
            }
        }

        impl TryFrom<BigInt> for $T {
            type Error = TryFromBigIntError<BigInt>;

            #[inline]
            fn try_from(value: BigInt) -> Result<$T, TryFromBigIntError<BigInt>> {
                <$T>::try_from(&value).map_err(|_| TryFromBigIntError::new(value))
            }
        }
    };
}

impl_try_from_bigint!(u8, ToPrimitive::to_u8);
impl_try_from_bigint!(u16, ToPrimitive::to_u16);
impl_try_from_bigint!(u32, ToPrimitive::to_u32);
impl_try_from_bigint!(u64, ToPrimitive::to_u64);
impl_try_from_bigint!(usize, ToPrimitive::to_usize);
impl_try_from_bigint!(u128, ToPrimitive::to_u128);

impl_try_from_bigint!(i8, ToPrimitive::to_i8);
impl_try_from_bigint!(i16, ToPrimitive::to_i16);
impl_try_from_bigint!(i32, ToPrimitive::to_i32);
impl_try_from_bigint!(i64, ToPrimitive::to_i64);
impl_try_from_bigint!(isize, ToPrimitive::to_isize);
impl_try_from_bigint!(i128, ToPrimitive::to_i128);

impl FromPrimitive for BigInt {
    #[inline]
    fn from_i64(n: i64) -> Option<BigInt> {
        Some(BigInt::from(n))
    }

    #[inline]
    fn from_i128(n: i128) -> Option<BigInt> {
        Some(BigInt::from(n))
    }

    #[inline]
    fn from_u64(n: u64) -> Option<BigInt> {
        Some(BigInt::from(n))
    }

    #[inline]
    fn from_u128(n: u128) -> Option<BigInt> {
        Some(BigInt::from(n))
    }

    #[inline]
    fn from_f64(n: f64) -> Option<BigInt> {
        if n >= 0.0 {
            BigUint::from_f64(n).map(BigInt::from)
        } else {
            let x = BigUint::from_f64(-n)?;
            Some(-BigInt::from(x))
        }
    }
}

impl From<i64> for BigInt {
    #[inline]
    fn from(n: i64) -> Self {
        if n >= 0 {
            BigInt::from(n as u64)
        } else {
            let u = u64::MAX - (n as u64) + 1;
            BigInt {
                sign: Minus,
                data: BigUint::from(u),
            }
        }
    }
}

impl From<i128> for BigInt {
    #[inline]
    fn from(n: i128) -> Self {
        if n >= 0 {
            BigInt::from(n as u128)
        } else {
            let u = u128::MAX - (n as u128) + 1;
            BigInt {
                sign: Minus,
                data: BigUint::from(u),
            }
        }
    }
}

macro_rules! impl_bigint_from_int {
    ($T:ty) => {
        impl From<$T> for BigInt {
            #[inline]
            fn from(n: $T) -> Self {
                BigInt::from(n as i64)
            }
        }
    };
}

impl_bigint_from_int!(i8);
impl_bigint_from_int!(i16);
impl_bigint_from_int!(i32);
impl_bigint_from_int!(isize);

impl From<u64> for BigInt {
    #[inline]
    fn from(n: u64) -> Self {
        if n > 0 {
            BigInt {
                sign: Plus,
                data: BigUint::from(n),
            }
        } else {
            Self::ZERO
        }
    }
}

impl From<u128> for BigInt {
    #[inline]
    fn from(n: u128) -> Self {
        if n > 0 {
            BigInt {
                sign: Plus,
                data: BigUint::from(n),
            }
        } else {
            Self::ZERO
        }
    }
}

macro_rules! impl_bigint_from_uint {
    ($T:ty) => {
        impl From<$T> for BigInt {
            #[inline]
            fn from(n: $T) -> Self {
                BigInt::from(n as u64)
            }
        }
    };
}

impl_bigint_from_uint!(u8);
impl_bigint_from_uint!(u16);
impl_bigint_from_uint!(u32);
impl_bigint_from_uint!(usize);

impl From<BigUint> for BigInt {
    #[inline]
    fn from(n: BigUint) -> Self {
        if n.is_zero() {
            Self::ZERO
        } else {
            BigInt {
                sign: Plus,
                data: n,
            }
        }
    }
}

impl ToBigInt for BigInt {
    #[inline]
    fn to_bigint(&self) -> Option<BigInt> {
        Some(self.clone())
    }
}

impl ToBigInt for BigUint {
    #[inline]
    fn to_bigint(&self) -> Option<BigInt> {
        if self.is_zero() {
            Some(BigInt::ZERO)
        } else {
            Some(BigInt {
                sign: Plus,
                data: self.clone(),
            })
        }
    }
}

impl ToBigUint for BigInt {
    #[inline]
    fn to_biguint(&self) -> Option<BigUint> {
        match self.sign() {
            Plus => Some(self.data.clone()),
            NoSign => Some(BigUint::ZERO),
            Minus => None,
        }
    }
}

impl TryFrom<&BigInt> for BigUint {
    type Error = TryFromBigIntError<()>;

    #[inline]
    fn try_from(value: &BigInt) -> Result<BigUint, TryFromBigIntError<()>> {
        value
            .to_biguint()
            .ok_or_else(|| TryFromBigIntError::new(()))
    }
}

impl TryFrom<BigInt> for BigUint {
    type Error = TryFromBigIntError<BigInt>;

    #[inline]
    fn try_from(value: BigInt) -> Result<BigUint, TryFromBigIntError<BigInt>> {
        if value.sign() == Sign::Minus {
            Err(TryFromBigIntError::new(value))
        } else {
            Ok(value.data)
        }
    }
}

macro_rules! impl_to_bigint {
    ($T:ty, $from_ty:path) => {
        impl ToBigInt for $T {
            #[inline]
            fn to_bigint(&self) -> Option<BigInt> {
                $from_ty(*self)
            }
        }
    };
}

impl_to_bigint!(isize, FromPrimitive::from_isize);
impl_to_bigint!(i8, FromPrimitive::from_i8);
impl_to_bigint!(i16, FromPrimitive::from_i16);
impl_to_bigint!(i32, FromPrimitive::from_i32);
impl_to_bigint!(i64, FromPrimitive::from_i64);
impl_to_bigint!(i128, FromPrimitive::from_i128);

impl_to_bigint!(usize, FromPrimitive::from_usize);
impl_to_bigint!(u8, FromPrimitive::from_u8);
impl_to_bigint!(u16, FromPrimitive::from_u16);
impl_to_bigint!(u32, FromPrimitive::from_u32);
impl_to_bigint!(u64, FromPrimitive::from_u64);
impl_to_bigint!(u128, FromPrimitive::from_u128);

impl_to_bigint!(f32, FromPrimitive::from_f32);
impl_to_bigint!(f64, FromPrimitive::from_f64);

impl From<bool> for BigInt {
    fn from(x: bool) -> Self {
        if x {
            One::one()
        } else {
            Self::ZERO
        }
    }
}

#[inline]
pub(super) fn from_signed_bytes_be(digits: &[u8]) -> BigInt {
    let sign = match digits.first() {
        Some(v) if *v > 0x7f => Sign::Minus,
        Some(_) => Sign::Plus,
        None => return BigInt::ZERO,
    };

    if sign == Sign::Minus {
        // two's-complement the content to retrieve the magnitude
        let mut digits = Vec::from(digits);
        twos_complement_be(&mut digits);
        BigInt::from_biguint(sign, BigUint::from_bytes_be(&digits))
    } else {
        BigInt::from_biguint(sign, BigUint::from_bytes_be(digits))
    }
}

#[inline]
pub(super) fn from_signed_bytes_le(digits: &[u8]) -> BigInt {
    let sign = match digits.last() {
        Some(v) if *v > 0x7f => Sign::Minus,
        Some(_) => Sign::Plus,
        None => return BigInt::ZERO,
    };

    if sign == Sign::Minus {
        // two's-complement the content to retrieve the magnitude
        let mut digits = Vec::from(digits);
        twos_complement_le(&mut digits);
        BigInt::from_biguint(sign, BigUint::from_bytes_le(&digits))
    } else {
        BigInt::from_biguint(sign, BigUint::from_bytes_le(digits))
    }
}

#[inline]
pub(super) fn to_signed_bytes_be(x: &BigInt) -> Vec<u8> {
    let mut bytes = x.data.to_bytes_be();
    let first_byte = bytes.first().cloned().unwrap_or(0);
    if first_byte > 0x7f
        && !(first_byte == 0x80 && bytes.iter().skip(1).all(Zero::is_zero) && x.sign == Sign::Minus)
    {
        // msb used by magnitude, extend by 1 byte
        bytes.insert(0, 0);
    }
    if x.sign == Sign::Minus {
        twos_complement_be(&mut bytes);
    }
    bytes
}

#[inline]
pub(super) fn to_signed_bytes_le(x: &BigInt) -> Vec<u8> {
    let mut bytes = x.data.to_bytes_le();
    let last_byte = bytes.last().cloned().unwrap_or(0);
    if last_byte > 0x7f
        && !(last_byte == 0x80
            && bytes.iter().rev().skip(1).all(Zero::is_zero)
            && x.sign == Sign::Minus)
    {
        // msb used by magnitude, extend by 1 byte
        bytes.push(0);
    }
    if x.sign == Sign::Minus {
        twos_complement_le(&mut bytes);
    }
    bytes
}

/// Perform in-place two's complement of the given binary representation,
/// in little-endian byte order.
#[inline]
fn twos_complement_le(digits: &mut [u8]) {
    twos_complement(digits)
}

/// Perform in-place two's complement of the given binary representation
/// in big-endian byte order.
#[inline]
fn twos_complement_be(digits: &mut [u8]) {
    twos_complement(digits.iter_mut().rev())
}

/// Perform in-place two's complement of the given digit iterator
/// starting from the least significant byte.
#[inline]
fn twos_complement<'a, I>(digits: I)
where
    I: IntoIterator<Item = &'a mut u8>,
{
    let mut carry = true;
    for d in digits {
        *d = !*d;
        if carry {
            *d = d.wrapping_add(1);
            carry = d.is_zero();
        }
    }
}
