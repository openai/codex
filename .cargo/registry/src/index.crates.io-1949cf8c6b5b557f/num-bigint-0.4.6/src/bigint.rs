// `Add`/`Sub` ops may flip from `BigInt` to its `BigUint` magnitude
#![allow(clippy::suspicious_arithmetic_impl)]

use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ordering::{self, Equal};
use core::default::Default;
use core::fmt;
use core::hash;
use core::ops::{Neg, Not};
use core::str;

use num_integer::{Integer, Roots};
use num_traits::{ConstZero, Num, One, Pow, Signed, Zero};

use self::Sign::{Minus, NoSign, Plus};

use crate::big_digit::BigDigit;
use crate::biguint::to_str_radix_reversed;
use crate::biguint::{BigUint, IntDigits, U32Digits, U64Digits};

mod addition;
mod division;
mod multiplication;
mod subtraction;

mod arbitrary;
mod bits;
mod convert;
mod power;
mod serde;
mod shift;

/// A `Sign` is a [`BigInt`]'s composing element.
#[derive(PartialEq, PartialOrd, Eq, Ord, Copy, Clone, Debug, Hash)]
pub enum Sign {
    Minus,
    NoSign,
    Plus,
}

impl Neg for Sign {
    type Output = Sign;

    /// Negate `Sign` value.
    #[inline]
    fn neg(self) -> Sign {
        match self {
            Minus => Plus,
            NoSign => NoSign,
            Plus => Minus,
        }
    }
}

/// A big signed integer type.
pub struct BigInt {
    sign: Sign,
    data: BigUint,
}

// Note: derived `Clone` doesn't specialize `clone_from`,
// but we want to keep the allocation in `data`.
impl Clone for BigInt {
    #[inline]
    fn clone(&self) -> Self {
        BigInt {
            sign: self.sign,
            data: self.data.clone(),
        }
    }

    #[inline]
    fn clone_from(&mut self, other: &Self) {
        self.sign = other.sign;
        self.data.clone_from(&other.data);
    }
}

impl hash::Hash for BigInt {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        debug_assert!((self.sign != NoSign) ^ self.data.is_zero());
        self.sign.hash(state);
        if self.sign != NoSign {
            self.data.hash(state);
        }
    }
}

impl PartialEq for BigInt {
    #[inline]
    fn eq(&self, other: &BigInt) -> bool {
        debug_assert!((self.sign != NoSign) ^ self.data.is_zero());
        debug_assert!((other.sign != NoSign) ^ other.data.is_zero());
        self.sign == other.sign && (self.sign == NoSign || self.data == other.data)
    }
}

impl Eq for BigInt {}

impl PartialOrd for BigInt {
    #[inline]
    fn partial_cmp(&self, other: &BigInt) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BigInt {
    #[inline]
    fn cmp(&self, other: &BigInt) -> Ordering {
        debug_assert!((self.sign != NoSign) ^ self.data.is_zero());
        debug_assert!((other.sign != NoSign) ^ other.data.is_zero());
        let scmp = self.sign.cmp(&other.sign);
        if scmp != Equal {
            return scmp;
        }

        match self.sign {
            NoSign => Equal,
            Plus => self.data.cmp(&other.data),
            Minus => other.data.cmp(&self.data),
        }
    }
}

impl Default for BigInt {
    #[inline]
    fn default() -> BigInt {
        Self::ZERO
    }
}

impl fmt::Debug for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad_integral(!self.is_negative(), "", &self.data.to_str_radix(10))
    }
}

impl fmt::Binary for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad_integral(!self.is_negative(), "0b", &self.data.to_str_radix(2))
    }
}

impl fmt::Octal for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad_integral(!self.is_negative(), "0o", &self.data.to_str_radix(8))
    }
}

impl fmt::LowerHex for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad_integral(!self.is_negative(), "0x", &self.data.to_str_radix(16))
    }
}

impl fmt::UpperHex for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = self.data.to_str_radix(16);
        s.make_ascii_uppercase();
        f.pad_integral(!self.is_negative(), "0x", &s)
    }
}

// !-2 = !...f fe = ...0 01 = +1
// !-1 = !...f ff = ...0 00 =  0
// ! 0 = !...0 00 = ...f ff = -1
// !+1 = !...0 01 = ...f fe = -2
impl Not for BigInt {
    type Output = BigInt;

    fn not(mut self) -> BigInt {
        match self.sign {
            NoSign | Plus => {
                self.data += 1u32;
                self.sign = Minus;
            }
            Minus => {
                self.data -= 1u32;
                self.sign = if self.data.is_zero() { NoSign } else { Plus };
            }
        }
        self
    }
}

impl Not for &BigInt {
    type Output = BigInt;

    fn not(self) -> BigInt {
        match self.sign {
            NoSign => -BigInt::one(),
            Plus => -BigInt::from(&self.data + 1u32),
            Minus => BigInt::from(&self.data - 1u32),
        }
    }
}

impl Zero for BigInt {
    #[inline]
    fn zero() -> BigInt {
        Self::ZERO
    }

    #[inline]
    fn set_zero(&mut self) {
        self.data.set_zero();
        self.sign = NoSign;
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.sign == NoSign
    }
}

impl ConstZero for BigInt {
    // forward to the inherent const
    const ZERO: Self = Self::ZERO;
}

impl One for BigInt {
    #[inline]
    fn one() -> BigInt {
        BigInt {
            sign: Plus,
            data: BigUint::one(),
        }
    }

    #[inline]
    fn set_one(&mut self) {
        self.data.set_one();
        self.sign = Plus;
    }

    #[inline]
    fn is_one(&self) -> bool {
        self.sign == Plus && self.data.is_one()
    }
}

impl Signed for BigInt {
    #[inline]
    fn abs(&self) -> BigInt {
        match self.sign {
            Plus | NoSign => self.clone(),
            Minus => BigInt::from(self.data.clone()),
        }
    }

    #[inline]
    fn abs_sub(&self, other: &BigInt) -> BigInt {
        if *self <= *other {
            Self::ZERO
        } else {
            self - other
        }
    }

    #[inline]
    fn signum(&self) -> BigInt {
        match self.sign {
            Plus => BigInt::one(),
            Minus => -BigInt::one(),
            NoSign => Self::ZERO,
        }
    }

    #[inline]
    fn is_positive(&self) -> bool {
        self.sign == Plus
    }

    #[inline]
    fn is_negative(&self) -> bool {
        self.sign == Minus
    }
}

trait UnsignedAbs {
    type Unsigned;

    fn checked_uabs(self) -> CheckedUnsignedAbs<Self::Unsigned>;
}

enum CheckedUnsignedAbs<T> {
    Positive(T),
    Negative(T),
}
use self::CheckedUnsignedAbs::{Negative, Positive};

macro_rules! impl_unsigned_abs {
    ($Signed:ty, $Unsigned:ty) => {
        impl UnsignedAbs for $Signed {
            type Unsigned = $Unsigned;

            #[inline]
            fn checked_uabs(self) -> CheckedUnsignedAbs<Self::Unsigned> {
                if self >= 0 {
                    Positive(self as $Unsigned)
                } else {
                    Negative(self.wrapping_neg() as $Unsigned)
                }
            }
        }
    };
}
impl_unsigned_abs!(i8, u8);
impl_unsigned_abs!(i16, u16);
impl_unsigned_abs!(i32, u32);
impl_unsigned_abs!(i64, u64);
impl_unsigned_abs!(i128, u128);
impl_unsigned_abs!(isize, usize);

impl Neg for BigInt {
    type Output = BigInt;

    #[inline]
    fn neg(mut self) -> BigInt {
        self.sign = -self.sign;
        self
    }
}

impl Neg for &BigInt {
    type Output = BigInt;

    #[inline]
    fn neg(self) -> BigInt {
        -self.clone()
    }
}

impl Integer for BigInt {
    #[inline]
    fn div_rem(&self, other: &BigInt) -> (BigInt, BigInt) {
        // r.sign == self.sign
        let (d_ui, r_ui) = self.data.div_rem(&other.data);
        let d = BigInt::from_biguint(self.sign, d_ui);
        let r = BigInt::from_biguint(self.sign, r_ui);
        if other.is_negative() {
            (-d, r)
        } else {
            (d, r)
        }
    }

    #[inline]
    fn div_floor(&self, other: &BigInt) -> BigInt {
        let (d_ui, m) = self.data.div_mod_floor(&other.data);
        let d = BigInt::from(d_ui);
        match (self.sign, other.sign) {
            (Plus, Plus) | (NoSign, Plus) | (Minus, Minus) => d,
            (Plus, Minus) | (NoSign, Minus) | (Minus, Plus) => {
                if m.is_zero() {
                    -d
                } else {
                    -d - 1u32
                }
            }
            (_, NoSign) => unreachable!(),
        }
    }

    #[inline]
    fn mod_floor(&self, other: &BigInt) -> BigInt {
        // m.sign == other.sign
        let m_ui = self.data.mod_floor(&other.data);
        let m = BigInt::from_biguint(other.sign, m_ui);
        match (self.sign, other.sign) {
            (Plus, Plus) | (NoSign, Plus) | (Minus, Minus) => m,
            (Plus, Minus) | (NoSign, Minus) | (Minus, Plus) => {
                if m.is_zero() {
                    m
                } else {
                    other - m
                }
            }
            (_, NoSign) => unreachable!(),
        }
    }

    fn div_mod_floor(&self, other: &BigInt) -> (BigInt, BigInt) {
        // m.sign == other.sign
        let (d_ui, m_ui) = self.data.div_mod_floor(&other.data);
        let d = BigInt::from(d_ui);
        let m = BigInt::from_biguint(other.sign, m_ui);
        match (self.sign, other.sign) {
            (Plus, Plus) | (NoSign, Plus) | (Minus, Minus) => (d, m),
            (Plus, Minus) | (NoSign, Minus) | (Minus, Plus) => {
                if m.is_zero() {
                    (-d, m)
                } else {
                    (-d - 1u32, other - m)
                }
            }
            (_, NoSign) => unreachable!(),
        }
    }

    #[inline]
    fn div_ceil(&self, other: &Self) -> Self {
        let (d_ui, m) = self.data.div_mod_floor(&other.data);
        let d = BigInt::from(d_ui);
        match (self.sign, other.sign) {
            (Plus, Minus) | (NoSign, Minus) | (Minus, Plus) => -d,
            (Plus, Plus) | (NoSign, Plus) | (Minus, Minus) => {
                if m.is_zero() {
                    d
                } else {
                    d + 1u32
                }
            }
            (_, NoSign) => unreachable!(),
        }
    }

    /// Calculates the Greatest Common Divisor (GCD) of the number and `other`.
    ///
    /// The result is always positive.
    #[inline]
    fn gcd(&self, other: &BigInt) -> BigInt {
        BigInt::from(self.data.gcd(&other.data))
    }

    /// Calculates the Lowest Common Multiple (LCM) of the number and `other`.
    #[inline]
    fn lcm(&self, other: &BigInt) -> BigInt {
        BigInt::from(self.data.lcm(&other.data))
    }

    /// Calculates the Greatest Common Divisor (GCD) and
    /// Lowest Common Multiple (LCM) together.
    #[inline]
    fn gcd_lcm(&self, other: &BigInt) -> (BigInt, BigInt) {
        let (gcd, lcm) = self.data.gcd_lcm(&other.data);
        (BigInt::from(gcd), BigInt::from(lcm))
    }

    /// Greatest common divisor, least common multiple, and Bézout coefficients.
    #[inline]
    fn extended_gcd_lcm(&self, other: &BigInt) -> (num_integer::ExtendedGcd<BigInt>, BigInt) {
        let egcd = self.extended_gcd(other);
        let lcm = if egcd.gcd.is_zero() {
            Self::ZERO
        } else {
            BigInt::from(&self.data / &egcd.gcd.data * &other.data)
        };
        (egcd, lcm)
    }

    /// Deprecated, use `is_multiple_of` instead.
    #[inline]
    fn divides(&self, other: &BigInt) -> bool {
        self.is_multiple_of(other)
    }

    /// Returns `true` if the number is a multiple of `other`.
    #[inline]
    fn is_multiple_of(&self, other: &BigInt) -> bool {
        self.data.is_multiple_of(&other.data)
    }

    /// Returns `true` if the number is divisible by `2`.
    #[inline]
    fn is_even(&self) -> bool {
        self.data.is_even()
    }

    /// Returns `true` if the number is not divisible by `2`.
    #[inline]
    fn is_odd(&self) -> bool {
        self.data.is_odd()
    }

    /// Rounds up to nearest multiple of argument.
    #[inline]
    fn next_multiple_of(&self, other: &Self) -> Self {
        let m = self.mod_floor(other);
        if m.is_zero() {
            self.clone()
        } else {
            self + (other - m)
        }
    }
    /// Rounds down to nearest multiple of argument.
    #[inline]
    fn prev_multiple_of(&self, other: &Self) -> Self {
        self - self.mod_floor(other)
    }

    fn dec(&mut self) {
        *self -= 1u32;
    }

    fn inc(&mut self) {
        *self += 1u32;
    }
}

impl Roots for BigInt {
    fn nth_root(&self, n: u32) -> Self {
        assert!(
            !(self.is_negative() && n.is_even()),
            "root of degree {} is imaginary",
            n
        );

        BigInt::from_biguint(self.sign, self.data.nth_root(n))
    }

    fn sqrt(&self) -> Self {
        assert!(!self.is_negative(), "square root is imaginary");

        BigInt::from_biguint(self.sign, self.data.sqrt())
    }

    fn cbrt(&self) -> Self {
        BigInt::from_biguint(self.sign, self.data.cbrt())
    }
}

impl IntDigits for BigInt {
    #[inline]
    fn digits(&self) -> &[BigDigit] {
        self.data.digits()
    }
    #[inline]
    fn digits_mut(&mut self) -> &mut Vec<BigDigit> {
        self.data.digits_mut()
    }
    #[inline]
    fn normalize(&mut self) {
        self.data.normalize();
        if self.data.is_zero() {
            self.sign = NoSign;
        }
    }
    #[inline]
    fn capacity(&self) -> usize {
        self.data.capacity()
    }
    #[inline]
    fn len(&self) -> usize {
        self.data.len()
    }
}

/// A generic trait for converting a value to a [`BigInt`]. This may return
/// `None` when converting from `f32` or `f64`, and will always succeed
/// when converting from any integer or unsigned primitive, or [`BigUint`].
pub trait ToBigInt {
    /// Converts the value of `self` to a [`BigInt`].
    fn to_bigint(&self) -> Option<BigInt>;
}

impl BigInt {
    /// A constant `BigInt` with value 0, useful for static initialization.
    pub const ZERO: Self = BigInt {
        sign: NoSign,
        data: BigUint::ZERO,
    };

    /// Creates and initializes a [`BigInt`].
    ///
    /// The base 2<sup>32</sup> digits are ordered least significant digit first.
    #[inline]
    pub fn new(sign: Sign, digits: Vec<u32>) -> BigInt {
        BigInt::from_biguint(sign, BigUint::new(digits))
    }

    /// Creates and initializes a [`BigInt`].
    ///
    /// The base 2<sup>32</sup> digits are ordered least significant digit first.
    #[inline]
    pub fn from_biguint(mut sign: Sign, mut data: BigUint) -> BigInt {
        if sign == NoSign {
            data.assign_from_slice(&[]);
        } else if data.is_zero() {
            sign = NoSign;
        }

        BigInt { sign, data }
    }

    /// Creates and initializes a [`BigInt`].
    ///
    /// The base 2<sup>32</sup> digits are ordered least significant digit first.
    #[inline]
    pub fn from_slice(sign: Sign, slice: &[u32]) -> BigInt {
        BigInt::from_biguint(sign, BigUint::from_slice(slice))
    }

    /// Reinitializes a [`BigInt`].
    ///
    /// The base 2<sup>32</sup> digits are ordered least significant digit first.
    #[inline]
    pub fn assign_from_slice(&mut self, sign: Sign, slice: &[u32]) {
        if sign == NoSign {
            self.set_zero();
        } else {
            self.data.assign_from_slice(slice);
            self.sign = if self.data.is_zero() { NoSign } else { sign };
        }
    }

    /// Creates and initializes a [`BigInt`].
    ///
    /// The bytes are in big-endian byte order.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{BigInt, Sign};
    ///
    /// assert_eq!(BigInt::from_bytes_be(Sign::Plus, b"A"),
    ///            BigInt::parse_bytes(b"65", 10).unwrap());
    /// assert_eq!(BigInt::from_bytes_be(Sign::Plus, b"AA"),
    ///            BigInt::parse_bytes(b"16705", 10).unwrap());
    /// assert_eq!(BigInt::from_bytes_be(Sign::Plus, b"AB"),
    ///            BigInt::parse_bytes(b"16706", 10).unwrap());
    /// assert_eq!(BigInt::from_bytes_be(Sign::Plus, b"Hello world!"),
    ///            BigInt::parse_bytes(b"22405534230753963835153736737", 10).unwrap());
    /// ```
    #[inline]
    pub fn from_bytes_be(sign: Sign, bytes: &[u8]) -> BigInt {
        BigInt::from_biguint(sign, BigUint::from_bytes_be(bytes))
    }

    /// Creates and initializes a [`BigInt`].
    ///
    /// The bytes are in little-endian byte order.
    #[inline]
    pub fn from_bytes_le(sign: Sign, bytes: &[u8]) -> BigInt {
        BigInt::from_biguint(sign, BigUint::from_bytes_le(bytes))
    }

    /// Creates and initializes a [`BigInt`] from an array of bytes in
    /// two's complement binary representation.
    ///
    /// The digits are in big-endian base 2<sup>8</sup>.
    #[inline]
    pub fn from_signed_bytes_be(digits: &[u8]) -> BigInt {
        convert::from_signed_bytes_be(digits)
    }

    /// Creates and initializes a [`BigInt`] from an array of bytes in two's complement.
    ///
    /// The digits are in little-endian base 2<sup>8</sup>.
    #[inline]
    pub fn from_signed_bytes_le(digits: &[u8]) -> BigInt {
        convert::from_signed_bytes_le(digits)
    }

    /// Creates and initializes a [`BigInt`].
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{BigInt, ToBigInt};
    ///
    /// assert_eq!(BigInt::parse_bytes(b"1234", 10), ToBigInt::to_bigint(&1234));
    /// assert_eq!(BigInt::parse_bytes(b"ABCD", 16), ToBigInt::to_bigint(&0xABCD));
    /// assert_eq!(BigInt::parse_bytes(b"G", 16), None);
    /// ```
    #[inline]
    pub fn parse_bytes(buf: &[u8], radix: u32) -> Option<BigInt> {
        let s = str::from_utf8(buf).ok()?;
        BigInt::from_str_radix(s, radix).ok()
    }

    /// Creates and initializes a [`BigInt`]. Each `u8` of the input slice is
    /// interpreted as one digit of the number
    /// and must therefore be less than `radix`.
    ///
    /// The bytes are in big-endian byte order.
    /// `radix` must be in the range `2...256`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{BigInt, Sign};
    ///
    /// let inbase190 = vec![15, 33, 125, 12, 14];
    /// let a = BigInt::from_radix_be(Sign::Minus, &inbase190, 190).unwrap();
    /// assert_eq!(a.to_radix_be(190), (Sign:: Minus, inbase190));
    /// ```
    pub fn from_radix_be(sign: Sign, buf: &[u8], radix: u32) -> Option<BigInt> {
        let u = BigUint::from_radix_be(buf, radix)?;
        Some(BigInt::from_biguint(sign, u))
    }

    /// Creates and initializes a [`BigInt`]. Each `u8` of the input slice is
    /// interpreted as one digit of the number
    /// and must therefore be less than `radix`.
    ///
    /// The bytes are in little-endian byte order.
    /// `radix` must be in the range `2...256`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{BigInt, Sign};
    ///
    /// let inbase190 = vec![14, 12, 125, 33, 15];
    /// let a = BigInt::from_radix_be(Sign::Minus, &inbase190, 190).unwrap();
    /// assert_eq!(a.to_radix_be(190), (Sign::Minus, inbase190));
    /// ```
    pub fn from_radix_le(sign: Sign, buf: &[u8], radix: u32) -> Option<BigInt> {
        let u = BigUint::from_radix_le(buf, radix)?;
        Some(BigInt::from_biguint(sign, u))
    }

    /// Returns the sign and the byte representation of the [`BigInt`] in big-endian byte order.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{ToBigInt, Sign};
    ///
    /// let i = -1125.to_bigint().unwrap();
    /// assert_eq!(i.to_bytes_be(), (Sign::Minus, vec![4, 101]));
    /// ```
    #[inline]
    pub fn to_bytes_be(&self) -> (Sign, Vec<u8>) {
        (self.sign, self.data.to_bytes_be())
    }

    /// Returns the sign and the byte representation of the [`BigInt`] in little-endian byte order.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{ToBigInt, Sign};
    ///
    /// let i = -1125.to_bigint().unwrap();
    /// assert_eq!(i.to_bytes_le(), (Sign::Minus, vec![101, 4]));
    /// ```
    #[inline]
    pub fn to_bytes_le(&self) -> (Sign, Vec<u8>) {
        (self.sign, self.data.to_bytes_le())
    }

    /// Returns the sign and the `u32` digits representation of the [`BigInt`] ordered least
    /// significant digit first.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{BigInt, Sign};
    ///
    /// assert_eq!(BigInt::from(-1125).to_u32_digits(), (Sign::Minus, vec![1125]));
    /// assert_eq!(BigInt::from(4294967295u32).to_u32_digits(), (Sign::Plus, vec![4294967295]));
    /// assert_eq!(BigInt::from(4294967296u64).to_u32_digits(), (Sign::Plus, vec![0, 1]));
    /// assert_eq!(BigInt::from(-112500000000i64).to_u32_digits(), (Sign::Minus, vec![830850304, 26]));
    /// assert_eq!(BigInt::from(112500000000i64).to_u32_digits(), (Sign::Plus, vec![830850304, 26]));
    /// ```
    #[inline]
    pub fn to_u32_digits(&self) -> (Sign, Vec<u32>) {
        (self.sign, self.data.to_u32_digits())
    }

    /// Returns the sign and the `u64` digits representation of the [`BigInt`] ordered least
    /// significant digit first.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{BigInt, Sign};
    ///
    /// assert_eq!(BigInt::from(-1125).to_u64_digits(), (Sign::Minus, vec![1125]));
    /// assert_eq!(BigInt::from(4294967295u32).to_u64_digits(), (Sign::Plus, vec![4294967295]));
    /// assert_eq!(BigInt::from(4294967296u64).to_u64_digits(), (Sign::Plus, vec![4294967296]));
    /// assert_eq!(BigInt::from(-112500000000i64).to_u64_digits(), (Sign::Minus, vec![112500000000]));
    /// assert_eq!(BigInt::from(112500000000i64).to_u64_digits(), (Sign::Plus, vec![112500000000]));
    /// assert_eq!(BigInt::from(1u128 << 64).to_u64_digits(), (Sign::Plus, vec![0, 1]));
    /// ```
    #[inline]
    pub fn to_u64_digits(&self) -> (Sign, Vec<u64>) {
        (self.sign, self.data.to_u64_digits())
    }

    /// Returns an iterator of `u32` digits representation of the [`BigInt`] ordered least
    /// significant digit first.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::BigInt;
    ///
    /// assert_eq!(BigInt::from(-1125).iter_u32_digits().collect::<Vec<u32>>(), vec![1125]);
    /// assert_eq!(BigInt::from(4294967295u32).iter_u32_digits().collect::<Vec<u32>>(), vec![4294967295]);
    /// assert_eq!(BigInt::from(4294967296u64).iter_u32_digits().collect::<Vec<u32>>(), vec![0, 1]);
    /// assert_eq!(BigInt::from(-112500000000i64).iter_u32_digits().collect::<Vec<u32>>(), vec![830850304, 26]);
    /// assert_eq!(BigInt::from(112500000000i64).iter_u32_digits().collect::<Vec<u32>>(), vec![830850304, 26]);
    /// ```
    #[inline]
    pub fn iter_u32_digits(&self) -> U32Digits<'_> {
        self.data.iter_u32_digits()
    }

    /// Returns an iterator of `u64` digits representation of the [`BigInt`] ordered least
    /// significant digit first.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::BigInt;
    ///
    /// assert_eq!(BigInt::from(-1125).iter_u64_digits().collect::<Vec<u64>>(), vec![1125u64]);
    /// assert_eq!(BigInt::from(4294967295u32).iter_u64_digits().collect::<Vec<u64>>(), vec![4294967295u64]);
    /// assert_eq!(BigInt::from(4294967296u64).iter_u64_digits().collect::<Vec<u64>>(), vec![4294967296u64]);
    /// assert_eq!(BigInt::from(-112500000000i64).iter_u64_digits().collect::<Vec<u64>>(), vec![112500000000u64]);
    /// assert_eq!(BigInt::from(112500000000i64).iter_u64_digits().collect::<Vec<u64>>(), vec![112500000000u64]);
    /// assert_eq!(BigInt::from(1u128 << 64).iter_u64_digits().collect::<Vec<u64>>(), vec![0, 1]);
    /// ```
    #[inline]
    pub fn iter_u64_digits(&self) -> U64Digits<'_> {
        self.data.iter_u64_digits()
    }

    /// Returns the two's-complement byte representation of the [`BigInt`] in big-endian byte order.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::ToBigInt;
    ///
    /// let i = -1125.to_bigint().unwrap();
    /// assert_eq!(i.to_signed_bytes_be(), vec![251, 155]);
    /// ```
    #[inline]
    pub fn to_signed_bytes_be(&self) -> Vec<u8> {
        convert::to_signed_bytes_be(self)
    }

    /// Returns the two's-complement byte representation of the [`BigInt`] in little-endian byte order.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::ToBigInt;
    ///
    /// let i = -1125.to_bigint().unwrap();
    /// assert_eq!(i.to_signed_bytes_le(), vec![155, 251]);
    /// ```
    #[inline]
    pub fn to_signed_bytes_le(&self) -> Vec<u8> {
        convert::to_signed_bytes_le(self)
    }

    /// Returns the integer formatted as a string in the given radix.
    /// `radix` must be in the range `2...36`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::BigInt;
    ///
    /// let i = BigInt::parse_bytes(b"ff", 16).unwrap();
    /// assert_eq!(i.to_str_radix(16), "ff");
    /// ```
    #[inline]
    pub fn to_str_radix(&self, radix: u32) -> String {
        let mut v = to_str_radix_reversed(&self.data, radix);

        if self.is_negative() {
            v.push(b'-');
        }

        v.reverse();
        unsafe { String::from_utf8_unchecked(v) }
    }

    /// Returns the integer in the requested base in big-endian digit order.
    /// The output is not given in a human readable alphabet but as a zero
    /// based `u8` number.
    /// `radix` must be in the range `2...256`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{BigInt, Sign};
    ///
    /// assert_eq!(BigInt::from(-0xFFFFi64).to_radix_be(159),
    ///            (Sign::Minus, vec![2, 94, 27]));
    /// // 0xFFFF = 65535 = 2*(159^2) + 94*159 + 27
    /// ```
    #[inline]
    pub fn to_radix_be(&self, radix: u32) -> (Sign, Vec<u8>) {
        (self.sign, self.data.to_radix_be(radix))
    }

    /// Returns the integer in the requested base in little-endian digit order.
    /// The output is not given in a human readable alphabet but as a zero
    /// based `u8` number.
    /// `radix` must be in the range `2...256`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{BigInt, Sign};
    ///
    /// assert_eq!(BigInt::from(-0xFFFFi64).to_radix_le(159),
    ///            (Sign::Minus, vec![27, 94, 2]));
    /// // 0xFFFF = 65535 = 27 + 94*159 + 2*(159^2)
    /// ```
    #[inline]
    pub fn to_radix_le(&self, radix: u32) -> (Sign, Vec<u8>) {
        (self.sign, self.data.to_radix_le(radix))
    }

    /// Returns the sign of the [`BigInt`] as a [`Sign`].
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{BigInt, Sign};
    ///
    /// assert_eq!(BigInt::from(1234).sign(), Sign::Plus);
    /// assert_eq!(BigInt::from(-4321).sign(), Sign::Minus);
    /// assert_eq!(BigInt::ZERO.sign(), Sign::NoSign);
    /// ```
    #[inline]
    pub fn sign(&self) -> Sign {
        self.sign
    }

    /// Returns the magnitude of the [`BigInt`] as a [`BigUint`].
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{BigInt, BigUint};
    /// use num_traits::Zero;
    ///
    /// assert_eq!(BigInt::from(1234).magnitude(), &BigUint::from(1234u32));
    /// assert_eq!(BigInt::from(-4321).magnitude(), &BigUint::from(4321u32));
    /// assert!(BigInt::ZERO.magnitude().is_zero());
    /// ```
    #[inline]
    pub fn magnitude(&self) -> &BigUint {
        &self.data
    }

    /// Convert this [`BigInt`] into its [`Sign`] and [`BigUint`] magnitude,
    /// the reverse of [`BigInt::from_biguint()`].
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{BigInt, BigUint, Sign};
    ///
    /// assert_eq!(BigInt::from(1234).into_parts(), (Sign::Plus, BigUint::from(1234u32)));
    /// assert_eq!(BigInt::from(-4321).into_parts(), (Sign::Minus, BigUint::from(4321u32)));
    /// assert_eq!(BigInt::ZERO.into_parts(), (Sign::NoSign, BigUint::ZERO));
    /// ```
    #[inline]
    pub fn into_parts(self) -> (Sign, BigUint) {
        (self.sign, self.data)
    }

    /// Determines the fewest bits necessary to express the [`BigInt`],
    /// not including the sign.
    #[inline]
    pub fn bits(&self) -> u64 {
        self.data.bits()
    }

    /// Converts this [`BigInt`] into a [`BigUint`], if it's not negative.
    #[inline]
    pub fn to_biguint(&self) -> Option<BigUint> {
        match self.sign {
            Plus => Some(self.data.clone()),
            NoSign => Some(BigUint::ZERO),
            Minus => None,
        }
    }

    #[inline]
    pub fn checked_add(&self, v: &BigInt) -> Option<BigInt> {
        Some(self + v)
    }

    #[inline]
    pub fn checked_sub(&self, v: &BigInt) -> Option<BigInt> {
        Some(self - v)
    }

    #[inline]
    pub fn checked_mul(&self, v: &BigInt) -> Option<BigInt> {
        Some(self * v)
    }

    #[inline]
    pub fn checked_div(&self, v: &BigInt) -> Option<BigInt> {
        if v.is_zero() {
            return None;
        }
        Some(self / v)
    }

    /// Returns `self ^ exponent`.
    pub fn pow(&self, exponent: u32) -> Self {
        Pow::pow(self, exponent)
    }

    /// Returns `(self ^ exponent) mod modulus`
    ///
    /// Note that this rounds like `mod_floor`, not like the `%` operator,
    /// which makes a difference when given a negative `self` or `modulus`.
    /// The result will be in the interval `[0, modulus)` for `modulus > 0`,
    /// or in the interval `(modulus, 0]` for `modulus < 0`
    ///
    /// Panics if the exponent is negative or the modulus is zero.
    pub fn modpow(&self, exponent: &Self, modulus: &Self) -> Self {
        power::modpow(self, exponent, modulus)
    }

    /// Returns the modular multiplicative inverse if it exists, otherwise `None`.
    ///
    /// This solves for `x` such that `self * x ≡ 1 (mod modulus)`.
    /// Note that this rounds like `mod_floor`, not like the `%` operator,
    /// which makes a difference when given a negative `self` or `modulus`.
    /// The solution will be in the interval `[0, modulus)` for `modulus > 0`,
    /// or in the interval `(modulus, 0]` for `modulus < 0`,
    /// and it exists if and only if `gcd(self, modulus) == 1`.
    ///
    /// ```
    /// use num_bigint::BigInt;
    /// use num_integer::Integer;
    /// use num_traits::{One, Zero};
    ///
    /// let m = BigInt::from(383);
    ///
    /// // Trivial cases
    /// assert_eq!(BigInt::zero().modinv(&m), None);
    /// assert_eq!(BigInt::one().modinv(&m), Some(BigInt::one()));
    /// let neg1 = &m - 1u32;
    /// assert_eq!(neg1.modinv(&m), Some(neg1));
    ///
    /// // Positive self and modulus
    /// let a = BigInt::from(271);
    /// let x = a.modinv(&m).unwrap();
    /// assert_eq!(x, BigInt::from(106));
    /// assert_eq!(x.modinv(&m).unwrap(), a);
    /// assert_eq!((&a * x).mod_floor(&m), BigInt::one());
    ///
    /// // Negative self and positive modulus
    /// let b = -&a;
    /// let x = b.modinv(&m).unwrap();
    /// assert_eq!(x, BigInt::from(277));
    /// assert_eq!((&b * x).mod_floor(&m), BigInt::one());
    ///
    /// // Positive self and negative modulus
    /// let n = -&m;
    /// let x = a.modinv(&n).unwrap();
    /// assert_eq!(x, BigInt::from(-277));
    /// assert_eq!((&a * x).mod_floor(&n), &n + 1);
    ///
    /// // Negative self and modulus
    /// let x = b.modinv(&n).unwrap();
    /// assert_eq!(x, BigInt::from(-106));
    /// assert_eq!((&b * x).mod_floor(&n), &n + 1);
    /// ```
    pub fn modinv(&self, modulus: &Self) -> Option<Self> {
        let result = self.data.modinv(&modulus.data)?;
        // The sign of the result follows the modulus, like `mod_floor`.
        let (sign, mag) = match (self.is_negative(), modulus.is_negative()) {
            (false, false) => (Plus, result),
            (true, false) => (Plus, &modulus.data - result),
            (false, true) => (Minus, &modulus.data - result),
            (true, true) => (Minus, result),
        };
        Some(BigInt::from_biguint(sign, mag))
    }

    /// Returns the truncated principal square root of `self` --
    /// see [`num_integer::Roots::sqrt()`].
    pub fn sqrt(&self) -> Self {
        Roots::sqrt(self)
    }

    /// Returns the truncated principal cube root of `self` --
    /// see [`num_integer::Roots::cbrt()`].
    pub fn cbrt(&self) -> Self {
        Roots::cbrt(self)
    }

    /// Returns the truncated principal `n`th root of `self` --
    /// See [`num_integer::Roots::nth_root()`].
    pub fn nth_root(&self, n: u32) -> Self {
        Roots::nth_root(self, n)
    }

    /// Returns the number of least-significant bits that are zero,
    /// or `None` if the entire number is zero.
    pub fn trailing_zeros(&self) -> Option<u64> {
        self.data.trailing_zeros()
    }

    /// Returns whether the bit in position `bit` is set,
    /// using the two's complement for negative numbers
    pub fn bit(&self, bit: u64) -> bool {
        if self.is_negative() {
            // Let the binary representation of a number be
            //   ... 0  x 1 0 ... 0
            // Then the two's complement is
            //   ... 1 !x 1 0 ... 0
            // where !x is obtained from x by flipping each bit
            if bit >= u64::from(crate::big_digit::BITS) * self.len() as u64 {
                true
            } else {
                let trailing_zeros = self.data.trailing_zeros().unwrap();
                match Ord::cmp(&bit, &trailing_zeros) {
                    Ordering::Less => false,
                    Ordering::Equal => true,
                    Ordering::Greater => !self.data.bit(bit),
                }
            }
        } else {
            self.data.bit(bit)
        }
    }

    /// Sets or clears the bit in the given position,
    /// using the two's complement for negative numbers
    ///
    /// Note that setting/clearing a bit (for positive/negative numbers,
    /// respectively) greater than the current bit length, a reallocation
    /// may be needed to store the new digits
    pub fn set_bit(&mut self, bit: u64, value: bool) {
        match self.sign {
            Sign::Plus => self.data.set_bit(bit, value),
            Sign::Minus => bits::set_negative_bit(self, bit, value),
            Sign::NoSign => {
                if value {
                    self.data.set_bit(bit, true);
                    self.sign = Sign::Plus;
                } else {
                    // Clearing a bit for zero is a no-op
                }
            }
        }
        // The top bit may have been cleared, so normalize
        self.normalize();
    }
}

impl num_traits::FromBytes for BigInt {
    type Bytes = [u8];

    fn from_be_bytes(bytes: &Self::Bytes) -> Self {
        Self::from_signed_bytes_be(bytes)
    }

    fn from_le_bytes(bytes: &Self::Bytes) -> Self {
        Self::from_signed_bytes_le(bytes)
    }
}

impl num_traits::ToBytes for BigInt {
    type Bytes = Vec<u8>;

    fn to_be_bytes(&self) -> Self::Bytes {
        self.to_signed_bytes_be()
    }

    fn to_le_bytes(&self) -> Self::Bytes {
        self.to_signed_bytes_le()
    }
}

#[test]
fn test_from_biguint() {
    fn check(inp_s: Sign, inp_n: usize, ans_s: Sign, ans_n: usize) {
        let inp = BigInt::from_biguint(inp_s, BigUint::from(inp_n));
        let ans = BigInt {
            sign: ans_s,
            data: BigUint::from(ans_n),
        };
        assert_eq!(inp, ans);
    }
    check(Plus, 1, Plus, 1);
    check(Plus, 0, NoSign, 0);
    check(Minus, 1, Minus, 1);
    check(NoSign, 1, NoSign, 0);
}

#[test]
fn test_from_slice() {
    fn check(inp_s: Sign, inp_n: u32, ans_s: Sign, ans_n: u32) {
        let inp = BigInt::from_slice(inp_s, &[inp_n]);
        let ans = BigInt {
            sign: ans_s,
            data: BigUint::from(ans_n),
        };
        assert_eq!(inp, ans);
    }
    check(Plus, 1, Plus, 1);
    check(Plus, 0, NoSign, 0);
    check(Minus, 1, Minus, 1);
    check(NoSign, 1, NoSign, 0);
}

#[test]
fn test_assign_from_slice() {
    fn check(inp_s: Sign, inp_n: u32, ans_s: Sign, ans_n: u32) {
        let mut inp = BigInt::from_slice(Minus, &[2627_u32, 0_u32, 9182_u32, 42_u32]);
        inp.assign_from_slice(inp_s, &[inp_n]);
        let ans = BigInt {
            sign: ans_s,
            data: BigUint::from(ans_n),
        };
        assert_eq!(inp, ans);
    }
    check(Plus, 1, Plus, 1);
    check(Plus, 0, NoSign, 0);
    check(Minus, 1, Minus, 1);
    check(NoSign, 1, NoSign, 0);
}
