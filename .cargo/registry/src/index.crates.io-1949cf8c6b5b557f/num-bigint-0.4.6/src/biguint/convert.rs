// This uses stdlib features higher than the MSRV
#![allow(clippy::manual_range_contains)] // 1.35

use super::{biguint_from_vec, BigUint, ToBigUint};

use super::addition::add2;
use super::division::{div_rem_digit, FAST_DIV_WIDE};
use super::multiplication::mac_with_carry;

use crate::big_digit::{self, BigDigit};
use crate::ParseBigIntError;
use crate::TryFromBigIntError;

use alloc::vec::Vec;
use core::cmp::Ordering::{Equal, Greater, Less};
use core::convert::TryFrom;
use core::mem;
use core::str::FromStr;
use num_integer::{Integer, Roots};
use num_traits::float::FloatCore;
use num_traits::{FromPrimitive, Num, One, PrimInt, ToPrimitive, Zero};

/// Find last set bit
/// fls(0) == 0, fls(u32::MAX) == 32
fn fls<T: PrimInt>(v: T) -> u8 {
    mem::size_of::<T>() as u8 * 8 - v.leading_zeros() as u8
}

fn ilog2<T: PrimInt>(v: T) -> u8 {
    fls(v) - 1
}

impl FromStr for BigUint {
    type Err = ParseBigIntError;

    #[inline]
    fn from_str(s: &str) -> Result<BigUint, ParseBigIntError> {
        BigUint::from_str_radix(s, 10)
    }
}

// Convert from a power of two radix (bits == ilog2(radix)) where bits evenly divides
// BigDigit::BITS
pub(super) fn from_bitwise_digits_le(v: &[u8], bits: u8) -> BigUint {
    debug_assert!(!v.is_empty() && bits <= 8 && big_digit::BITS % bits == 0);
    debug_assert!(v.iter().all(|&c| BigDigit::from(c) < (1 << bits)));

    let digits_per_big_digit = big_digit::BITS / bits;

    let data = v
        .chunks(digits_per_big_digit.into())
        .map(|chunk| {
            chunk
                .iter()
                .rev()
                .fold(0, |acc, &c| (acc << bits) | BigDigit::from(c))
        })
        .collect();

    biguint_from_vec(data)
}

// Convert from a power of two radix (bits == ilog2(radix)) where bits doesn't evenly divide
// BigDigit::BITS
fn from_inexact_bitwise_digits_le(v: &[u8], bits: u8) -> BigUint {
    debug_assert!(!v.is_empty() && bits <= 8 && big_digit::BITS % bits != 0);
    debug_assert!(v.iter().all(|&c| BigDigit::from(c) < (1 << bits)));

    let total_bits = (v.len() as u64).saturating_mul(bits.into());
    let big_digits = Integer::div_ceil(&total_bits, &big_digit::BITS.into())
        .to_usize()
        .unwrap_or(usize::MAX);
    let mut data = Vec::with_capacity(big_digits);

    let mut d = 0;
    let mut dbits = 0; // number of bits we currently have in d

    // walk v accumululating bits in d; whenever we accumulate big_digit::BITS in d, spit out a
    // big_digit:
    for &c in v {
        d |= BigDigit::from(c) << dbits;
        dbits += bits;

        if dbits >= big_digit::BITS {
            data.push(d);
            dbits -= big_digit::BITS;
            // if dbits was > big_digit::BITS, we dropped some of the bits in c (they couldn't fit
            // in d) - grab the bits we lost here:
            d = BigDigit::from(c) >> (bits - dbits);
        }
    }

    if dbits > 0 {
        debug_assert!(dbits < big_digit::BITS);
        data.push(d as BigDigit);
    }

    biguint_from_vec(data)
}

// Read little-endian radix digits
fn from_radix_digits_be(v: &[u8], radix: u32) -> BigUint {
    debug_assert!(!v.is_empty() && !radix.is_power_of_two());
    debug_assert!(v.iter().all(|&c| u32::from(c) < radix));

    // Estimate how big the result will be, so we can pre-allocate it.
    #[cfg(feature = "std")]
    let big_digits = {
        let radix_log2 = f64::from(radix).log2();
        let bits = radix_log2 * v.len() as f64;
        (bits / big_digit::BITS as f64).ceil()
    };
    #[cfg(not(feature = "std"))]
    let big_digits = {
        let radix_log2 = ilog2(radix.next_power_of_two()) as usize;
        let bits = radix_log2 * v.len();
        (bits / big_digit::BITS as usize) + 1
    };

    let mut data = Vec::with_capacity(big_digits.to_usize().unwrap_or(0));

    let (base, power) = get_radix_base(radix);
    let radix = radix as BigDigit;

    let r = v.len() % power;
    let i = if r == 0 { power } else { r };
    let (head, tail) = v.split_at(i);

    let first = head
        .iter()
        .fold(0, |acc, &d| acc * radix + BigDigit::from(d));
    data.push(first);

    debug_assert!(tail.len() % power == 0);
    for chunk in tail.chunks(power) {
        if data.last() != Some(&0) {
            data.push(0);
        }

        let mut carry = 0;
        for d in data.iter_mut() {
            *d = mac_with_carry(0, *d, base, &mut carry);
        }
        debug_assert!(carry == 0);

        let n = chunk
            .iter()
            .fold(0, |acc, &d| acc * radix + BigDigit::from(d));
        add2(&mut data, &[n]);
    }

    biguint_from_vec(data)
}

pub(super) fn from_radix_be(buf: &[u8], radix: u32) -> Option<BigUint> {
    assert!(
        2 <= radix && radix <= 256,
        "The radix must be within 2...256"
    );

    if buf.is_empty() {
        return Some(BigUint::ZERO);
    }

    if radix != 256 && buf.iter().any(|&b| b >= radix as u8) {
        return None;
    }

    let res = if radix.is_power_of_two() {
        // Powers of two can use bitwise masks and shifting instead of multiplication
        let bits = ilog2(radix);
        let mut v = Vec::from(buf);
        v.reverse();
        if big_digit::BITS % bits == 0 {
            from_bitwise_digits_le(&v, bits)
        } else {
            from_inexact_bitwise_digits_le(&v, bits)
        }
    } else {
        from_radix_digits_be(buf, radix)
    };

    Some(res)
}

pub(super) fn from_radix_le(buf: &[u8], radix: u32) -> Option<BigUint> {
    assert!(
        2 <= radix && radix <= 256,
        "The radix must be within 2...256"
    );

    if buf.is_empty() {
        return Some(BigUint::ZERO);
    }

    if radix != 256 && buf.iter().any(|&b| b >= radix as u8) {
        return None;
    }

    let res = if radix.is_power_of_two() {
        // Powers of two can use bitwise masks and shifting instead of multiplication
        let bits = ilog2(radix);
        if big_digit::BITS % bits == 0 {
            from_bitwise_digits_le(buf, bits)
        } else {
            from_inexact_bitwise_digits_le(buf, bits)
        }
    } else {
        let mut v = Vec::from(buf);
        v.reverse();
        from_radix_digits_be(&v, radix)
    };

    Some(res)
}

impl Num for BigUint {
    type FromStrRadixErr = ParseBigIntError;

    /// Creates and initializes a `BigUint`.
    fn from_str_radix(s: &str, radix: u32) -> Result<BigUint, ParseBigIntError> {
        assert!(2 <= radix && radix <= 36, "The radix must be within 2...36");
        let mut s = s;
        if let Some(tail) = s.strip_prefix('+') {
            if !tail.starts_with('+') {
                s = tail
            }
        }

        if s.is_empty() {
            return Err(ParseBigIntError::empty());
        }

        if s.starts_with('_') {
            // Must lead with a real digit!
            return Err(ParseBigIntError::invalid());
        }

        // First normalize all characters to plain digit values
        let mut v = Vec::with_capacity(s.len());
        for b in s.bytes() {
            let d = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'z' => b - b'a' + 10,
                b'A'..=b'Z' => b - b'A' + 10,
                b'_' => continue,
                _ => u8::MAX,
            };
            if d < radix as u8 {
                v.push(d);
            } else {
                return Err(ParseBigIntError::invalid());
            }
        }

        let res = if radix.is_power_of_two() {
            // Powers of two can use bitwise masks and shifting instead of multiplication
            let bits = ilog2(radix);
            v.reverse();
            if big_digit::BITS % bits == 0 {
                from_bitwise_digits_le(&v, bits)
            } else {
                from_inexact_bitwise_digits_le(&v, bits)
            }
        } else {
            from_radix_digits_be(&v, radix)
        };
        Ok(res)
    }
}

fn high_bits_to_u64(v: &BigUint) -> u64 {
    match v.data.len() {
        0 => 0,
        1 => {
            // XXX Conversion is useless if already 64-bit.
            #[allow(clippy::useless_conversion)]
            let v0 = u64::from(v.data[0]);
            v0
        }
        _ => {
            let mut bits = v.bits();
            let mut ret = 0u64;
            let mut ret_bits = 0;

            for d in v.data.iter().rev() {
                let digit_bits = (bits - 1) % u64::from(big_digit::BITS) + 1;
                let bits_want = Ord::min(64 - ret_bits, digit_bits);

                if bits_want != 0 {
                    if bits_want != 64 {
                        ret <<= bits_want;
                    }
                    // XXX Conversion is useless if already 64-bit.
                    #[allow(clippy::useless_conversion)]
                    let d0 = u64::from(*d) >> (digit_bits - bits_want);
                    ret |= d0;
                }

                // Implement round-to-odd: If any lower bits are 1, set LSB to 1
                // so that rounding again to floating point value using
                // nearest-ties-to-even is correct.
                //
                // See: https://en.wikipedia.org/wiki/Rounding#Rounding_to_prepare_for_shorter_precision

                if digit_bits - bits_want != 0 {
                    // XXX Conversion is useless if already 64-bit.
                    #[allow(clippy::useless_conversion)]
                    let masked = u64::from(*d) << (64 - (digit_bits - bits_want) as u32);
                    ret |= (masked != 0) as u64;
                }

                ret_bits += bits_want;
                bits -= bits_want;
            }

            ret
        }
    }
}

impl ToPrimitive for BigUint {
    #[inline]
    fn to_i64(&self) -> Option<i64> {
        self.to_u64().as_ref().and_then(u64::to_i64)
    }

    #[inline]
    fn to_i128(&self) -> Option<i128> {
        self.to_u128().as_ref().and_then(u128::to_i128)
    }

    #[allow(clippy::useless_conversion)]
    #[inline]
    fn to_u64(&self) -> Option<u64> {
        let mut ret: u64 = 0;
        let mut bits = 0;

        for i in self.data.iter() {
            if bits >= 64 {
                return None;
            }

            // XXX Conversion is useless if already 64-bit.
            ret += u64::from(*i) << bits;
            bits += big_digit::BITS;
        }

        Some(ret)
    }

    #[inline]
    fn to_u128(&self) -> Option<u128> {
        let mut ret: u128 = 0;
        let mut bits = 0;

        for i in self.data.iter() {
            if bits >= 128 {
                return None;
            }

            ret |= u128::from(*i) << bits;
            bits += big_digit::BITS;
        }

        Some(ret)
    }

    #[inline]
    fn to_f32(&self) -> Option<f32> {
        let mantissa = high_bits_to_u64(self);
        let exponent = self.bits() - u64::from(fls(mantissa));

        if exponent > f32::MAX_EXP as u64 {
            Some(f32::INFINITY)
        } else {
            Some((mantissa as f32) * 2.0f32.powi(exponent as i32))
        }
    }

    #[inline]
    fn to_f64(&self) -> Option<f64> {
        let mantissa = high_bits_to_u64(self);
        let exponent = self.bits() - u64::from(fls(mantissa));

        if exponent > f64::MAX_EXP as u64 {
            Some(f64::INFINITY)
        } else {
            Some((mantissa as f64) * 2.0f64.powi(exponent as i32))
        }
    }
}

macro_rules! impl_try_from_biguint {
    ($T:ty, $to_ty:path) => {
        impl TryFrom<&BigUint> for $T {
            type Error = TryFromBigIntError<()>;

            #[inline]
            fn try_from(value: &BigUint) -> Result<$T, TryFromBigIntError<()>> {
                $to_ty(value).ok_or(TryFromBigIntError::new(()))
            }
        }

        impl TryFrom<BigUint> for $T {
            type Error = TryFromBigIntError<BigUint>;

            #[inline]
            fn try_from(value: BigUint) -> Result<$T, TryFromBigIntError<BigUint>> {
                <$T>::try_from(&value).map_err(|_| TryFromBigIntError::new(value))
            }
        }
    };
}

impl_try_from_biguint!(u8, ToPrimitive::to_u8);
impl_try_from_biguint!(u16, ToPrimitive::to_u16);
impl_try_from_biguint!(u32, ToPrimitive::to_u32);
impl_try_from_biguint!(u64, ToPrimitive::to_u64);
impl_try_from_biguint!(usize, ToPrimitive::to_usize);
impl_try_from_biguint!(u128, ToPrimitive::to_u128);

impl_try_from_biguint!(i8, ToPrimitive::to_i8);
impl_try_from_biguint!(i16, ToPrimitive::to_i16);
impl_try_from_biguint!(i32, ToPrimitive::to_i32);
impl_try_from_biguint!(i64, ToPrimitive::to_i64);
impl_try_from_biguint!(isize, ToPrimitive::to_isize);
impl_try_from_biguint!(i128, ToPrimitive::to_i128);

impl FromPrimitive for BigUint {
    #[inline]
    fn from_i64(n: i64) -> Option<BigUint> {
        if n >= 0 {
            Some(BigUint::from(n as u64))
        } else {
            None
        }
    }

    #[inline]
    fn from_i128(n: i128) -> Option<BigUint> {
        if n >= 0 {
            Some(BigUint::from(n as u128))
        } else {
            None
        }
    }

    #[inline]
    fn from_u64(n: u64) -> Option<BigUint> {
        Some(BigUint::from(n))
    }

    #[inline]
    fn from_u128(n: u128) -> Option<BigUint> {
        Some(BigUint::from(n))
    }

    #[inline]
    fn from_f64(mut n: f64) -> Option<BigUint> {
        // handle NAN, INFINITY, NEG_INFINITY
        if !n.is_finite() {
            return None;
        }

        // match the rounding of casting from float to int
        n = n.trunc();

        // handle 0.x, -0.x
        if n.is_zero() {
            return Some(Self::ZERO);
        }

        let (mantissa, exponent, sign) = FloatCore::integer_decode(n);

        if sign == -1 {
            return None;
        }

        let mut ret = BigUint::from(mantissa);
        match exponent.cmp(&0) {
            Greater => ret <<= exponent as usize,
            Equal => {}
            Less => ret >>= (-exponent) as usize,
        }
        Some(ret)
    }
}

impl From<u64> for BigUint {
    #[inline]
    fn from(mut n: u64) -> Self {
        let mut ret: BigUint = Self::ZERO;

        while n != 0 {
            ret.data.push(n as BigDigit);
            // don't overflow if BITS is 64:
            n = (n >> 1) >> (big_digit::BITS - 1);
        }

        ret
    }
}

impl From<u128> for BigUint {
    #[inline]
    fn from(mut n: u128) -> Self {
        let mut ret: BigUint = Self::ZERO;

        while n != 0 {
            ret.data.push(n as BigDigit);
            n >>= big_digit::BITS;
        }

        ret
    }
}

macro_rules! impl_biguint_from_uint {
    ($T:ty) => {
        impl From<$T> for BigUint {
            #[inline]
            fn from(n: $T) -> Self {
                BigUint::from(n as u64)
            }
        }
    };
}

impl_biguint_from_uint!(u8);
impl_biguint_from_uint!(u16);
impl_biguint_from_uint!(u32);
impl_biguint_from_uint!(usize);

macro_rules! impl_biguint_try_from_int {
    ($T:ty, $from_ty:path) => {
        impl TryFrom<$T> for BigUint {
            type Error = TryFromBigIntError<()>;

            #[inline]
            fn try_from(value: $T) -> Result<BigUint, TryFromBigIntError<()>> {
                $from_ty(value).ok_or(TryFromBigIntError::new(()))
            }
        }
    };
}

impl_biguint_try_from_int!(i8, FromPrimitive::from_i8);
impl_biguint_try_from_int!(i16, FromPrimitive::from_i16);
impl_biguint_try_from_int!(i32, FromPrimitive::from_i32);
impl_biguint_try_from_int!(i64, FromPrimitive::from_i64);
impl_biguint_try_from_int!(isize, FromPrimitive::from_isize);
impl_biguint_try_from_int!(i128, FromPrimitive::from_i128);

impl ToBigUint for BigUint {
    #[inline]
    fn to_biguint(&self) -> Option<BigUint> {
        Some(self.clone())
    }
}

macro_rules! impl_to_biguint {
    ($T:ty, $from_ty:path) => {
        impl ToBigUint for $T {
            #[inline]
            fn to_biguint(&self) -> Option<BigUint> {
                $from_ty(*self)
            }
        }
    };
}

impl_to_biguint!(isize, FromPrimitive::from_isize);
impl_to_biguint!(i8, FromPrimitive::from_i8);
impl_to_biguint!(i16, FromPrimitive::from_i16);
impl_to_biguint!(i32, FromPrimitive::from_i32);
impl_to_biguint!(i64, FromPrimitive::from_i64);
impl_to_biguint!(i128, FromPrimitive::from_i128);

impl_to_biguint!(usize, FromPrimitive::from_usize);
impl_to_biguint!(u8, FromPrimitive::from_u8);
impl_to_biguint!(u16, FromPrimitive::from_u16);
impl_to_biguint!(u32, FromPrimitive::from_u32);
impl_to_biguint!(u64, FromPrimitive::from_u64);
impl_to_biguint!(u128, FromPrimitive::from_u128);

impl_to_biguint!(f32, FromPrimitive::from_f32);
impl_to_biguint!(f64, FromPrimitive::from_f64);

impl From<bool> for BigUint {
    fn from(x: bool) -> Self {
        if x {
            One::one()
        } else {
            Self::ZERO
        }
    }
}

// Extract bitwise digits that evenly divide BigDigit
pub(super) fn to_bitwise_digits_le(u: &BigUint, bits: u8) -> Vec<u8> {
    debug_assert!(!u.is_zero() && bits <= 8 && big_digit::BITS % bits == 0);

    let last_i = u.data.len() - 1;
    let mask: BigDigit = (1 << bits) - 1;
    let digits_per_big_digit = big_digit::BITS / bits;
    let digits = Integer::div_ceil(&u.bits(), &u64::from(bits))
        .to_usize()
        .unwrap_or(usize::MAX);
    let mut res = Vec::with_capacity(digits);

    for mut r in u.data[..last_i].iter().cloned() {
        for _ in 0..digits_per_big_digit {
            res.push((r & mask) as u8);
            r >>= bits;
        }
    }

    let mut r = u.data[last_i];
    while r != 0 {
        res.push((r & mask) as u8);
        r >>= bits;
    }

    res
}

// Extract bitwise digits that don't evenly divide BigDigit
fn to_inexact_bitwise_digits_le(u: &BigUint, bits: u8) -> Vec<u8> {
    debug_assert!(!u.is_zero() && bits <= 8 && big_digit::BITS % bits != 0);

    let mask: BigDigit = (1 << bits) - 1;
    let digits = Integer::div_ceil(&u.bits(), &u64::from(bits))
        .to_usize()
        .unwrap_or(usize::MAX);
    let mut res = Vec::with_capacity(digits);

    let mut r = 0;
    let mut rbits = 0;

    for c in &u.data {
        r |= *c << rbits;
        rbits += big_digit::BITS;

        while rbits >= bits {
            res.push((r & mask) as u8);
            r >>= bits;

            // r had more bits than it could fit - grab the bits we lost
            if rbits > big_digit::BITS {
                r = *c >> (big_digit::BITS - (rbits - bits));
            }

            rbits -= bits;
        }
    }

    if rbits != 0 {
        res.push(r as u8);
    }

    while let Some(&0) = res.last() {
        res.pop();
    }

    res
}

// Extract little-endian radix digits
#[inline(always)] // forced inline to get const-prop for radix=10
pub(super) fn to_radix_digits_le(u: &BigUint, radix: u32) -> Vec<u8> {
    debug_assert!(!u.is_zero() && !radix.is_power_of_two());

    #[cfg(feature = "std")]
    let radix_digits = {
        let radix_log2 = f64::from(radix).log2();
        ((u.bits() as f64) / radix_log2).ceil()
    };
    #[cfg(not(feature = "std"))]
    let radix_digits = {
        let radix_log2 = ilog2(radix) as usize;
        ((u.bits() as usize) / radix_log2) + 1
    };

    // Estimate how big the result will be, so we can pre-allocate it.
    let mut res = Vec::with_capacity(radix_digits.to_usize().unwrap_or(0));

    let mut digits = u.clone();

    // X86 DIV can quickly divide by a full digit, otherwise we choose a divisor
    // that's suitable for `div_half` to avoid slow `DoubleBigDigit` division.
    let (base, power) = if FAST_DIV_WIDE {
        get_radix_base(radix)
    } else {
        get_half_radix_base(radix)
    };
    let radix = radix as BigDigit;

    // For very large numbers, the O(n²) loop of repeated `div_rem_digit` dominates the
    // performance. We can mitigate this by dividing into chunks of a larger base first.
    // The threshold for this was chosen by anecdotal performance measurements to
    // approximate where this starts to make a noticeable difference.
    if digits.data.len() >= 64 {
        let mut big_base = BigUint::from(base);
        let mut big_power = 1usize;

        // Choose a target base length near √n.
        let target_len = digits.data.len().sqrt();
        while big_base.data.len() < target_len {
            big_base = &big_base * &big_base;
            big_power *= 2;
        }

        // This outer loop will run approximately √n times.
        while digits > big_base {
            // This is still the dominating factor, with n digits divided by √n digits.
            let (q, mut big_r) = digits.div_rem(&big_base);
            digits = q;

            // This inner loop now has O(√n²)=O(n) behavior altogether.
            for _ in 0..big_power {
                let (q, mut r) = div_rem_digit(big_r, base);
                big_r = q;
                for _ in 0..power {
                    res.push((r % radix) as u8);
                    r /= radix;
                }
            }
        }
    }

    while digits.data.len() > 1 {
        let (q, mut r) = div_rem_digit(digits, base);
        for _ in 0..power {
            res.push((r % radix) as u8);
            r /= radix;
        }
        digits = q;
    }

    let mut r = digits.data[0];
    while r != 0 {
        res.push((r % radix) as u8);
        r /= radix;
    }

    res
}

pub(super) fn to_radix_le(u: &BigUint, radix: u32) -> Vec<u8> {
    if u.is_zero() {
        vec![0]
    } else if radix.is_power_of_two() {
        // Powers of two can use bitwise masks and shifting instead of division
        let bits = ilog2(radix);
        if big_digit::BITS % bits == 0 {
            to_bitwise_digits_le(u, bits)
        } else {
            to_inexact_bitwise_digits_le(u, bits)
        }
    } else if radix == 10 {
        // 10 is so common that it's worth separating out for const-propagation.
        // Optimizers can often turn constant division into a faster multiplication.
        to_radix_digits_le(u, 10)
    } else {
        to_radix_digits_le(u, radix)
    }
}

pub(crate) fn to_str_radix_reversed(u: &BigUint, radix: u32) -> Vec<u8> {
    assert!(2 <= radix && radix <= 36, "The radix must be within 2...36");

    if u.is_zero() {
        return vec![b'0'];
    }

    let mut res = to_radix_le(u, radix);

    // Now convert everything to ASCII digits.
    for r in &mut res {
        debug_assert!(u32::from(*r) < radix);
        if *r < 10 {
            *r += b'0';
        } else {
            *r += b'a' - 10;
        }
    }
    res
}

/// Returns the greatest power of the radix for the `BigDigit` bit size
#[inline]
fn get_radix_base(radix: u32) -> (BigDigit, usize) {
    static BASES: [(BigDigit, usize); 257] = generate_radix_bases(big_digit::MAX);
    debug_assert!(!radix.is_power_of_two());
    debug_assert!((3..256).contains(&radix));
    BASES[radix as usize]
}

/// Returns the greatest power of the radix for half the `BigDigit` bit size
#[inline]
fn get_half_radix_base(radix: u32) -> (BigDigit, usize) {
    static BASES: [(BigDigit, usize); 257] = generate_radix_bases(big_digit::HALF);
    debug_assert!(!radix.is_power_of_two());
    debug_assert!((3..256).contains(&radix));
    BASES[radix as usize]
}

/// Generate tables of the greatest power of each radix that is less that the given maximum. These
/// are returned from `get_radix_base` to batch the multiplication/division of radix conversions on
/// full `BigUint` values, operating on primitive integers as much as possible.
///
/// e.g. BASES_16[3] = (59049, 10) // 3¹⁰ fits in u16, but 3¹¹ is too big
///      BASES_32[3] = (3486784401, 20)
///      BASES_64[3] = (12157665459056928801, 40)
///
/// Powers of two are not included, just zeroed, as they're implemented with shifts.
const fn generate_radix_bases(max: BigDigit) -> [(BigDigit, usize); 257] {
    let mut bases = [(0, 0); 257];

    let mut radix: BigDigit = 3;
    while radix < 256 {
        if !radix.is_power_of_two() {
            let mut power = 1;
            let mut base = radix;

            while let Some(b) = base.checked_mul(radix) {
                if b > max {
                    break;
                }
                base = b;
                power += 1;
            }
            bases[radix as usize] = (base, power)
        }
        radix += 1;
    }

    bases
}

#[test]
fn test_radix_bases() {
    for radix in 3u32..256 {
        if !radix.is_power_of_two() {
            let (base, power) = get_radix_base(radix);
            let radix = BigDigit::from(radix);
            let power = u32::try_from(power).unwrap();
            assert_eq!(base, radix.pow(power));
            assert!(radix.checked_pow(power + 1).is_none());
        }
    }
}

#[test]
fn test_half_radix_bases() {
    for radix in 3u32..256 {
        if !radix.is_power_of_two() {
            let (base, power) = get_half_radix_base(radix);
            let radix = BigDigit::from(radix);
            let power = u32::try_from(power).unwrap();
            assert_eq!(base, radix.pow(power));
            assert!(radix.pow(power + 1) > big_digit::HALF);
        }
    }
}
