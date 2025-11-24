use crate::big_digit::{self, BigDigit};

use alloc::string::String;
use alloc::vec::Vec;
use core::cmp;
use core::cmp::Ordering;
use core::default::Default;
use core::fmt;
use core::hash;
use core::mem;
use core::str;

use num_integer::{Integer, Roots};
use num_traits::{ConstZero, Num, One, Pow, ToPrimitive, Unsigned, Zero};

mod addition;
mod division;
mod multiplication;
mod subtraction;

mod arbitrary;
mod bits;
mod convert;
mod iter;
mod monty;
mod power;
mod serde;
mod shift;

pub(crate) use self::convert::to_str_radix_reversed;
pub use self::iter::{U32Digits, U64Digits};

/// A big unsigned integer type.
pub struct BigUint {
    data: Vec<BigDigit>,
}

// Note: derived `Clone` doesn't specialize `clone_from`,
// but we want to keep the allocation in `data`.
impl Clone for BigUint {
    #[inline]
    fn clone(&self) -> Self {
        BigUint {
            data: self.data.clone(),
        }
    }

    #[inline]
    fn clone_from(&mut self, other: &Self) {
        self.data.clone_from(&other.data);
    }
}

impl hash::Hash for BigUint {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        debug_assert!(self.data.last() != Some(&0));
        self.data.hash(state);
    }
}

impl PartialEq for BigUint {
    #[inline]
    fn eq(&self, other: &BigUint) -> bool {
        debug_assert!(self.data.last() != Some(&0));
        debug_assert!(other.data.last() != Some(&0));
        self.data == other.data
    }
}
impl Eq for BigUint {}

impl PartialOrd for BigUint {
    #[inline]
    fn partial_cmp(&self, other: &BigUint) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BigUint {
    #[inline]
    fn cmp(&self, other: &BigUint) -> Ordering {
        cmp_slice(&self.data[..], &other.data[..])
    }
}

#[inline]
fn cmp_slice(a: &[BigDigit], b: &[BigDigit]) -> Ordering {
    debug_assert!(a.last() != Some(&0));
    debug_assert!(b.last() != Some(&0));

    match Ord::cmp(&a.len(), &b.len()) {
        Ordering::Equal => Iterator::cmp(a.iter().rev(), b.iter().rev()),
        other => other,
    }
}

impl Default for BigUint {
    #[inline]
    fn default() -> BigUint {
        Self::ZERO
    }
}

impl fmt::Debug for BigUint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for BigUint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad_integral(true, "", &self.to_str_radix(10))
    }
}

impl fmt::LowerHex for BigUint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad_integral(true, "0x", &self.to_str_radix(16))
    }
}

impl fmt::UpperHex for BigUint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = self.to_str_radix(16);
        s.make_ascii_uppercase();
        f.pad_integral(true, "0x", &s)
    }
}

impl fmt::Binary for BigUint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad_integral(true, "0b", &self.to_str_radix(2))
    }
}

impl fmt::Octal for BigUint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad_integral(true, "0o", &self.to_str_radix(8))
    }
}

impl Zero for BigUint {
    #[inline]
    fn zero() -> BigUint {
        Self::ZERO
    }

    #[inline]
    fn set_zero(&mut self) {
        self.data.clear();
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.data.is_empty()
    }
}

impl ConstZero for BigUint {
    // forward to the inherent const
    const ZERO: Self = Self::ZERO; // BigUint { data: Vec::new() };
}

impl One for BigUint {
    #[inline]
    fn one() -> BigUint {
        BigUint { data: vec![1] }
    }

    #[inline]
    fn set_one(&mut self) {
        self.data.clear();
        self.data.push(1);
    }

    #[inline]
    fn is_one(&self) -> bool {
        self.data[..] == [1]
    }
}

impl Unsigned for BigUint {}

impl Integer for BigUint {
    #[inline]
    fn div_rem(&self, other: &BigUint) -> (BigUint, BigUint) {
        division::div_rem_ref(self, other)
    }

    #[inline]
    fn div_floor(&self, other: &BigUint) -> BigUint {
        let (d, _) = division::div_rem_ref(self, other);
        d
    }

    #[inline]
    fn mod_floor(&self, other: &BigUint) -> BigUint {
        let (_, m) = division::div_rem_ref(self, other);
        m
    }

    #[inline]
    fn div_mod_floor(&self, other: &BigUint) -> (BigUint, BigUint) {
        division::div_rem_ref(self, other)
    }

    #[inline]
    fn div_ceil(&self, other: &BigUint) -> BigUint {
        let (d, m) = division::div_rem_ref(self, other);
        if m.is_zero() {
            d
        } else {
            d + 1u32
        }
    }

    /// Calculates the Greatest Common Divisor (GCD) of the number and `other`.
    ///
    /// The result is always positive.
    #[inline]
    fn gcd(&self, other: &Self) -> Self {
        #[inline]
        fn twos(x: &BigUint) -> u64 {
            x.trailing_zeros().unwrap_or(0)
        }

        // Stein's algorithm
        if self.is_zero() {
            return other.clone();
        }
        if other.is_zero() {
            return self.clone();
        }
        let mut m = self.clone();
        let mut n = other.clone();

        // find common factors of 2
        let shift = cmp::min(twos(&n), twos(&m));

        // divide m and n by 2 until odd
        // m inside loop
        n >>= twos(&n);

        while !m.is_zero() {
            m >>= twos(&m);
            if n > m {
                mem::swap(&mut n, &mut m)
            }
            m -= &n;
        }

        n << shift
    }

    /// Calculates the Lowest Common Multiple (LCM) of the number and `other`.
    #[inline]
    fn lcm(&self, other: &BigUint) -> BigUint {
        if self.is_zero() && other.is_zero() {
            Self::ZERO
        } else {
            self / self.gcd(other) * other
        }
    }

    /// Calculates the Greatest Common Divisor (GCD) and
    /// Lowest Common Multiple (LCM) together.
    #[inline]
    fn gcd_lcm(&self, other: &Self) -> (Self, Self) {
        let gcd = self.gcd(other);
        let lcm = if gcd.is_zero() {
            Self::ZERO
        } else {
            self / &gcd * other
        };
        (gcd, lcm)
    }

    /// Deprecated, use `is_multiple_of` instead.
    #[inline]
    fn divides(&self, other: &BigUint) -> bool {
        self.is_multiple_of(other)
    }

    /// Returns `true` if the number is a multiple of `other`.
    #[inline]
    fn is_multiple_of(&self, other: &BigUint) -> bool {
        if other.is_zero() {
            return self.is_zero();
        }
        (self % other).is_zero()
    }

    /// Returns `true` if the number is divisible by `2`.
    #[inline]
    fn is_even(&self) -> bool {
        // Considering only the last digit.
        match self.data.first() {
            Some(x) => x.is_even(),
            None => true,
        }
    }

    /// Returns `true` if the number is not divisible by `2`.
    #[inline]
    fn is_odd(&self) -> bool {
        !self.is_even()
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

#[inline]
fn fixpoint<F>(mut x: BigUint, max_bits: u64, f: F) -> BigUint
where
    F: Fn(&BigUint) -> BigUint,
{
    let mut xn = f(&x);

    // If the value increased, then the initial guess must have been low.
    // Repeat until we reverse course.
    while x < xn {
        // Sometimes an increase will go way too far, especially with large
        // powers, and then take a long time to walk back.  We know an upper
        // bound based on bit size, so saturate on that.
        x = if xn.bits() > max_bits {
            BigUint::one() << max_bits
        } else {
            xn
        };
        xn = f(&x);
    }

    // Now keep repeating while the estimate is decreasing.
    while x > xn {
        x = xn;
        xn = f(&x);
    }
    x
}

impl Roots for BigUint {
    // nth_root, sqrt and cbrt use Newton's method to compute
    // principal root of a given degree for a given integer.

    // Reference:
    // Brent & Zimmermann, Modern Computer Arithmetic, v0.5.9, Algorithm 1.14
    fn nth_root(&self, n: u32) -> Self {
        assert!(n > 0, "root degree n must be at least 1");

        if self.is_zero() || self.is_one() {
            return self.clone();
        }

        match n {
            // Optimize for small n
            1 => return self.clone(),
            2 => return self.sqrt(),
            3 => return self.cbrt(),
            _ => (),
        }

        // The root of non-zero values less than 2ⁿ can only be 1.
        let bits = self.bits();
        let n64 = u64::from(n);
        if bits <= n64 {
            return BigUint::one();
        }

        // If we fit in `u64`, compute the root that way.
        if let Some(x) = self.to_u64() {
            return x.nth_root(n).into();
        }

        let max_bits = bits / n64 + 1;

        #[cfg(feature = "std")]
        let guess = match self.to_f64() {
            Some(f) if f.is_finite() => {
                use num_traits::FromPrimitive;

                // We fit in `f64` (lossy), so get a better initial guess from that.
                BigUint::from_f64((f.ln() / f64::from(n)).exp()).unwrap()
            }
            _ => {
                // Try to guess by scaling down such that it does fit in `f64`.
                // With some (x * 2ⁿᵏ), its nth root ≈ (ⁿ√x * 2ᵏ)
                let extra_bits = bits - (f64::MAX_EXP as u64 - 1);
                let root_scale = Integer::div_ceil(&extra_bits, &n64);
                let scale = root_scale * n64;
                if scale < bits && bits - scale > n64 {
                    (self >> scale).nth_root(n) << root_scale
                } else {
                    BigUint::one() << max_bits
                }
            }
        };

        #[cfg(not(feature = "std"))]
        let guess = BigUint::one() << max_bits;

        let n_min_1 = n - 1;
        fixpoint(guess, max_bits, move |s| {
            let q = self / s.pow(n_min_1);
            let t = n_min_1 * s + q;
            t / n
        })
    }

    // Reference:
    // Brent & Zimmermann, Modern Computer Arithmetic, v0.5.9, Algorithm 1.13
    fn sqrt(&self) -> Self {
        if self.is_zero() || self.is_one() {
            return self.clone();
        }

        // If we fit in `u64`, compute the root that way.
        if let Some(x) = self.to_u64() {
            return x.sqrt().into();
        }

        let bits = self.bits();
        let max_bits = bits / 2 + 1;

        #[cfg(feature = "std")]
        let guess = match self.to_f64() {
            Some(f) if f.is_finite() => {
                use num_traits::FromPrimitive;

                // We fit in `f64` (lossy), so get a better initial guess from that.
                BigUint::from_f64(f.sqrt()).unwrap()
            }
            _ => {
                // Try to guess by scaling down such that it does fit in `f64`.
                // With some (x * 2²ᵏ), its sqrt ≈ (√x * 2ᵏ)
                let extra_bits = bits - (f64::MAX_EXP as u64 - 1);
                let root_scale = (extra_bits + 1) / 2;
                let scale = root_scale * 2;
                (self >> scale).sqrt() << root_scale
            }
        };

        #[cfg(not(feature = "std"))]
        let guess = BigUint::one() << max_bits;

        fixpoint(guess, max_bits, move |s| {
            let q = self / s;
            let t = s + q;
            t >> 1
        })
    }

    fn cbrt(&self) -> Self {
        if self.is_zero() || self.is_one() {
            return self.clone();
        }

        // If we fit in `u64`, compute the root that way.
        if let Some(x) = self.to_u64() {
            return x.cbrt().into();
        }

        let bits = self.bits();
        let max_bits = bits / 3 + 1;

        #[cfg(feature = "std")]
        let guess = match self.to_f64() {
            Some(f) if f.is_finite() => {
                use num_traits::FromPrimitive;

                // We fit in `f64` (lossy), so get a better initial guess from that.
                BigUint::from_f64(f.cbrt()).unwrap()
            }
            _ => {
                // Try to guess by scaling down such that it does fit in `f64`.
                // With some (x * 2³ᵏ), its cbrt ≈ (∛x * 2ᵏ)
                let extra_bits = bits - (f64::MAX_EXP as u64 - 1);
                let root_scale = (extra_bits + 2) / 3;
                let scale = root_scale * 3;
                (self >> scale).cbrt() << root_scale
            }
        };

        #[cfg(not(feature = "std"))]
        let guess = BigUint::one() << max_bits;

        fixpoint(guess, max_bits, move |s| {
            let q = self / (s * s);
            let t = (s << 1) + q;
            t / 3u32
        })
    }
}

/// A generic trait for converting a value to a [`BigUint`].
pub trait ToBigUint {
    /// Converts the value of `self` to a [`BigUint`].
    fn to_biguint(&self) -> Option<BigUint>;
}

/// Creates and initializes a [`BigUint`].
///
/// The digits are in little-endian base matching `BigDigit`.
#[inline]
pub(crate) fn biguint_from_vec(digits: Vec<BigDigit>) -> BigUint {
    BigUint { data: digits }.normalized()
}

impl BigUint {
    /// A constant `BigUint` with value 0, useful for static initialization.
    pub const ZERO: Self = BigUint { data: Vec::new() };

    /// Creates and initializes a [`BigUint`].
    ///
    /// The base 2<sup>32</sup> digits are ordered least significant digit first.
    #[inline]
    pub fn new(digits: Vec<u32>) -> BigUint {
        let mut big = Self::ZERO;

        cfg_digit_expr!(
            {
                big.data = digits;
                big.normalize();
            },
            big.assign_from_slice(&digits)
        );

        big
    }

    /// Creates and initializes a [`BigUint`].
    ///
    /// The base 2<sup>32</sup> digits are ordered least significant digit first.
    #[inline]
    pub fn from_slice(slice: &[u32]) -> BigUint {
        let mut big = Self::ZERO;
        big.assign_from_slice(slice);
        big
    }

    /// Assign a value to a [`BigUint`].
    ///
    /// The base 2<sup>32</sup> digits are ordered least significant digit first.
    #[inline]
    pub fn assign_from_slice(&mut self, slice: &[u32]) {
        self.data.clear();

        cfg_digit_expr!(
            self.data.extend_from_slice(slice),
            self.data.extend(slice.chunks(2).map(u32_chunk_to_u64))
        );

        self.normalize();
    }

    /// Creates and initializes a [`BigUint`].
    ///
    /// The bytes are in big-endian byte order.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::BigUint;
    ///
    /// assert_eq!(BigUint::from_bytes_be(b"A"),
    ///            BigUint::parse_bytes(b"65", 10).unwrap());
    /// assert_eq!(BigUint::from_bytes_be(b"AA"),
    ///            BigUint::parse_bytes(b"16705", 10).unwrap());
    /// assert_eq!(BigUint::from_bytes_be(b"AB"),
    ///            BigUint::parse_bytes(b"16706", 10).unwrap());
    /// assert_eq!(BigUint::from_bytes_be(b"Hello world!"),
    ///            BigUint::parse_bytes(b"22405534230753963835153736737", 10).unwrap());
    /// ```
    #[inline]
    pub fn from_bytes_be(bytes: &[u8]) -> BigUint {
        if bytes.is_empty() {
            Self::ZERO
        } else {
            let mut v = bytes.to_vec();
            v.reverse();
            BigUint::from_bytes_le(&v)
        }
    }

    /// Creates and initializes a [`BigUint`].
    ///
    /// The bytes are in little-endian byte order.
    #[inline]
    pub fn from_bytes_le(bytes: &[u8]) -> BigUint {
        if bytes.is_empty() {
            Self::ZERO
        } else {
            convert::from_bitwise_digits_le(bytes, 8)
        }
    }

    /// Creates and initializes a [`BigUint`]. The input slice must contain
    /// ascii/utf8 characters in [0-9a-zA-Z].
    /// `radix` must be in the range `2...36`.
    ///
    /// The function `from_str_radix` from the `Num` trait provides the same logic
    /// for `&str` buffers.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{BigUint, ToBigUint};
    ///
    /// assert_eq!(BigUint::parse_bytes(b"1234", 10), ToBigUint::to_biguint(&1234));
    /// assert_eq!(BigUint::parse_bytes(b"ABCD", 16), ToBigUint::to_biguint(&0xABCD));
    /// assert_eq!(BigUint::parse_bytes(b"G", 16), None);
    /// ```
    #[inline]
    pub fn parse_bytes(buf: &[u8], radix: u32) -> Option<BigUint> {
        let s = str::from_utf8(buf).ok()?;
        BigUint::from_str_radix(s, radix).ok()
    }

    /// Creates and initializes a [`BigUint`]. Each `u8` of the input slice is
    /// interpreted as one digit of the number
    /// and must therefore be less than `radix`.
    ///
    /// The bytes are in big-endian byte order.
    /// `radix` must be in the range `2...256`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{BigUint};
    ///
    /// let inbase190 = &[15, 33, 125, 12, 14];
    /// let a = BigUint::from_radix_be(inbase190, 190).unwrap();
    /// assert_eq!(a.to_radix_be(190), inbase190);
    /// ```
    pub fn from_radix_be(buf: &[u8], radix: u32) -> Option<BigUint> {
        convert::from_radix_be(buf, radix)
    }

    /// Creates and initializes a [`BigUint`]. Each `u8` of the input slice is
    /// interpreted as one digit of the number
    /// and must therefore be less than `radix`.
    ///
    /// The bytes are in little-endian byte order.
    /// `radix` must be in the range `2...256`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::{BigUint};
    ///
    /// let inbase190 = &[14, 12, 125, 33, 15];
    /// let a = BigUint::from_radix_be(inbase190, 190).unwrap();
    /// assert_eq!(a.to_radix_be(190), inbase190);
    /// ```
    pub fn from_radix_le(buf: &[u8], radix: u32) -> Option<BigUint> {
        convert::from_radix_le(buf, radix)
    }

    /// Returns the byte representation of the [`BigUint`] in big-endian byte order.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::BigUint;
    ///
    /// let i = BigUint::parse_bytes(b"1125", 10).unwrap();
    /// assert_eq!(i.to_bytes_be(), vec![4, 101]);
    /// ```
    #[inline]
    pub fn to_bytes_be(&self) -> Vec<u8> {
        let mut v = self.to_bytes_le();
        v.reverse();
        v
    }

    /// Returns the byte representation of the [`BigUint`] in little-endian byte order.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::BigUint;
    ///
    /// let i = BigUint::parse_bytes(b"1125", 10).unwrap();
    /// assert_eq!(i.to_bytes_le(), vec![101, 4]);
    /// ```
    #[inline]
    pub fn to_bytes_le(&self) -> Vec<u8> {
        if self.is_zero() {
            vec![0]
        } else {
            convert::to_bitwise_digits_le(self, 8)
        }
    }

    /// Returns the `u32` digits representation of the [`BigUint`] ordered least significant digit
    /// first.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::BigUint;
    ///
    /// assert_eq!(BigUint::from(1125u32).to_u32_digits(), vec![1125]);
    /// assert_eq!(BigUint::from(4294967295u32).to_u32_digits(), vec![4294967295]);
    /// assert_eq!(BigUint::from(4294967296u64).to_u32_digits(), vec![0, 1]);
    /// assert_eq!(BigUint::from(112500000000u64).to_u32_digits(), vec![830850304, 26]);
    /// ```
    #[inline]
    pub fn to_u32_digits(&self) -> Vec<u32> {
        self.iter_u32_digits().collect()
    }

    /// Returns the `u64` digits representation of the [`BigUint`] ordered least significant digit
    /// first.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::BigUint;
    ///
    /// assert_eq!(BigUint::from(1125u32).to_u64_digits(), vec![1125]);
    /// assert_eq!(BigUint::from(4294967295u32).to_u64_digits(), vec![4294967295]);
    /// assert_eq!(BigUint::from(4294967296u64).to_u64_digits(), vec![4294967296]);
    /// assert_eq!(BigUint::from(112500000000u64).to_u64_digits(), vec![112500000000]);
    /// assert_eq!(BigUint::from(1u128 << 64).to_u64_digits(), vec![0, 1]);
    /// ```
    #[inline]
    pub fn to_u64_digits(&self) -> Vec<u64> {
        self.iter_u64_digits().collect()
    }

    /// Returns an iterator of `u32` digits representation of the [`BigUint`] ordered least
    /// significant digit first.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::BigUint;
    ///
    /// assert_eq!(BigUint::from(1125u32).iter_u32_digits().collect::<Vec<u32>>(), vec![1125]);
    /// assert_eq!(BigUint::from(4294967295u32).iter_u32_digits().collect::<Vec<u32>>(), vec![4294967295]);
    /// assert_eq!(BigUint::from(4294967296u64).iter_u32_digits().collect::<Vec<u32>>(), vec![0, 1]);
    /// assert_eq!(BigUint::from(112500000000u64).iter_u32_digits().collect::<Vec<u32>>(), vec![830850304, 26]);
    /// ```
    #[inline]
    pub fn iter_u32_digits(&self) -> U32Digits<'_> {
        U32Digits::new(self.data.as_slice())
    }

    /// Returns an iterator of `u64` digits representation of the [`BigUint`] ordered least
    /// significant digit first.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::BigUint;
    ///
    /// assert_eq!(BigUint::from(1125u32).iter_u64_digits().collect::<Vec<u64>>(), vec![1125]);
    /// assert_eq!(BigUint::from(4294967295u32).iter_u64_digits().collect::<Vec<u64>>(), vec![4294967295]);
    /// assert_eq!(BigUint::from(4294967296u64).iter_u64_digits().collect::<Vec<u64>>(), vec![4294967296]);
    /// assert_eq!(BigUint::from(112500000000u64).iter_u64_digits().collect::<Vec<u64>>(), vec![112500000000]);
    /// assert_eq!(BigUint::from(1u128 << 64).iter_u64_digits().collect::<Vec<u64>>(), vec![0, 1]);
    /// ```
    #[inline]
    pub fn iter_u64_digits(&self) -> U64Digits<'_> {
        U64Digits::new(self.data.as_slice())
    }

    /// Returns the integer formatted as a string in the given radix.
    /// `radix` must be in the range `2...36`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::BigUint;
    ///
    /// let i = BigUint::parse_bytes(b"ff", 16).unwrap();
    /// assert_eq!(i.to_str_radix(16), "ff");
    /// ```
    #[inline]
    pub fn to_str_radix(&self, radix: u32) -> String {
        let mut v = to_str_radix_reversed(self, radix);
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
    /// use num_bigint::BigUint;
    ///
    /// assert_eq!(BigUint::from(0xFFFFu64).to_radix_be(159),
    ///            vec![2, 94, 27]);
    /// // 0xFFFF = 65535 = 2*(159^2) + 94*159 + 27
    /// ```
    #[inline]
    pub fn to_radix_be(&self, radix: u32) -> Vec<u8> {
        let mut v = convert::to_radix_le(self, radix);
        v.reverse();
        v
    }

    /// Returns the integer in the requested base in little-endian digit order.
    /// The output is not given in a human readable alphabet but as a zero
    /// based u8 number.
    /// `radix` must be in the range `2...256`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_bigint::BigUint;
    ///
    /// assert_eq!(BigUint::from(0xFFFFu64).to_radix_le(159),
    ///            vec![27, 94, 2]);
    /// // 0xFFFF = 65535 = 27 + 94*159 + 2*(159^2)
    /// ```
    #[inline]
    pub fn to_radix_le(&self, radix: u32) -> Vec<u8> {
        convert::to_radix_le(self, radix)
    }

    /// Determines the fewest bits necessary to express the [`BigUint`].
    #[inline]
    pub fn bits(&self) -> u64 {
        if self.is_zero() {
            return 0;
        }
        let zeros: u64 = self.data.last().unwrap().leading_zeros().into();
        self.data.len() as u64 * u64::from(big_digit::BITS) - zeros
    }

    /// Strips off trailing zero bigdigits - comparisons require the last element in the vector to
    /// be nonzero.
    #[inline]
    fn normalize(&mut self) {
        if let Some(&0) = self.data.last() {
            let len = self.data.iter().rposition(|&d| d != 0).map_or(0, |i| i + 1);
            self.data.truncate(len);
        }
        if self.data.len() < self.data.capacity() / 4 {
            self.data.shrink_to_fit();
        }
    }

    /// Returns a normalized [`BigUint`].
    #[inline]
    fn normalized(mut self) -> BigUint {
        self.normalize();
        self
    }

    /// Returns `self ^ exponent`.
    pub fn pow(&self, exponent: u32) -> Self {
        Pow::pow(self, exponent)
    }

    /// Returns `(self ^ exponent) % modulus`.
    ///
    /// Panics if the modulus is zero.
    pub fn modpow(&self, exponent: &Self, modulus: &Self) -> Self {
        power::modpow(self, exponent, modulus)
    }

    /// Returns the modular multiplicative inverse if it exists, otherwise `None`.
    ///
    /// This solves for `x` in the interval `[0, modulus)` such that `self * x ≡ 1 (mod modulus)`.
    /// The solution exists if and only if `gcd(self, modulus) == 1`.
    ///
    /// ```
    /// use num_bigint::BigUint;
    /// use num_traits::{One, Zero};
    ///
    /// let m = BigUint::from(383_u32);
    ///
    /// // Trivial cases
    /// assert_eq!(BigUint::zero().modinv(&m), None);
    /// assert_eq!(BigUint::one().modinv(&m), Some(BigUint::one()));
    /// let neg1 = &m - 1u32;
    /// assert_eq!(neg1.modinv(&m), Some(neg1));
    ///
    /// let a = BigUint::from(271_u32);
    /// let x = a.modinv(&m).unwrap();
    /// assert_eq!(x, BigUint::from(106_u32));
    /// assert_eq!(x.modinv(&m).unwrap(), a);
    /// assert!((a * x % m).is_one());
    /// ```
    pub fn modinv(&self, modulus: &Self) -> Option<Self> {
        // Based on the inverse pseudocode listed here:
        // https://en.wikipedia.org/wiki/Extended_Euclidean_algorithm#Modular_integers
        // TODO: consider Binary or Lehmer's GCD algorithms for optimization.

        assert!(
            !modulus.is_zero(),
            "attempt to calculate with zero modulus!"
        );
        if modulus.is_one() {
            return Some(Self::zero());
        }

        let mut r0; // = modulus.clone();
        let mut r1 = self % modulus;
        let mut t0; // = Self::zero();
        let mut t1; // = Self::one();

        // Lift and simplify the first iteration to avoid some initial allocations.
        if r1.is_zero() {
            return None;
        } else if r1.is_one() {
            return Some(r1);
        } else {
            let (q, r2) = modulus.div_rem(&r1);
            if r2.is_zero() {
                return None;
            }
            r0 = r1;
            r1 = r2;
            t0 = Self::one();
            t1 = modulus - q;
        }

        while !r1.is_zero() {
            let (q, r2) = r0.div_rem(&r1);
            r0 = r1;
            r1 = r2;

            // let t2 = (t0 - q * t1) % modulus;
            let qt1 = q * &t1 % modulus;
            let t2 = if t0 < qt1 {
                t0 + (modulus - qt1)
            } else {
                t0 - qt1
            };
            t0 = t1;
            t1 = t2;
        }

        if r0.is_one() {
            Some(t0)
        } else {
            None
        }
    }

    /// Returns the truncated principal square root of `self` --
    /// see [Roots::sqrt](https://docs.rs/num-integer/0.1/num_integer/trait.Roots.html#method.sqrt)
    pub fn sqrt(&self) -> Self {
        Roots::sqrt(self)
    }

    /// Returns the truncated principal cube root of `self` --
    /// see [Roots::cbrt](https://docs.rs/num-integer/0.1/num_integer/trait.Roots.html#method.cbrt).
    pub fn cbrt(&self) -> Self {
        Roots::cbrt(self)
    }

    /// Returns the truncated principal `n`th root of `self` --
    /// see [Roots::nth_root](https://docs.rs/num-integer/0.1/num_integer/trait.Roots.html#tymethod.nth_root).
    pub fn nth_root(&self, n: u32) -> Self {
        Roots::nth_root(self, n)
    }

    /// Returns the number of least-significant bits that are zero,
    /// or `None` if the entire number is zero.
    pub fn trailing_zeros(&self) -> Option<u64> {
        let i = self.data.iter().position(|&digit| digit != 0)?;
        let zeros: u64 = self.data[i].trailing_zeros().into();
        Some(i as u64 * u64::from(big_digit::BITS) + zeros)
    }

    /// Returns the number of least-significant bits that are ones.
    pub fn trailing_ones(&self) -> u64 {
        if let Some(i) = self.data.iter().position(|&digit| !digit != 0) {
            let ones: u64 = self.data[i].trailing_ones().into();
            i as u64 * u64::from(big_digit::BITS) + ones
        } else {
            self.data.len() as u64 * u64::from(big_digit::BITS)
        }
    }

    /// Returns the number of one bits.
    pub fn count_ones(&self) -> u64 {
        self.data.iter().map(|&d| u64::from(d.count_ones())).sum()
    }

    /// Returns whether the bit in the given position is set
    pub fn bit(&self, bit: u64) -> bool {
        let bits_per_digit = u64::from(big_digit::BITS);
        if let Some(digit_index) = (bit / bits_per_digit).to_usize() {
            if let Some(digit) = self.data.get(digit_index) {
                let bit_mask = (1 as BigDigit) << (bit % bits_per_digit);
                return (digit & bit_mask) != 0;
            }
        }
        false
    }

    /// Sets or clears the bit in the given position
    ///
    /// Note that setting a bit greater than the current bit length, a reallocation may be needed
    /// to store the new digits
    pub fn set_bit(&mut self, bit: u64, value: bool) {
        // Note: we're saturating `digit_index` and `new_len` -- any such case is guaranteed to
        // fail allocation, and that's more consistent than adding our own overflow panics.
        let bits_per_digit = u64::from(big_digit::BITS);
        let digit_index = (bit / bits_per_digit).to_usize().unwrap_or(usize::MAX);
        let bit_mask = (1 as BigDigit) << (bit % bits_per_digit);
        if value {
            if digit_index >= self.data.len() {
                let new_len = digit_index.saturating_add(1);
                self.data.resize(new_len, 0);
            }
            self.data[digit_index] |= bit_mask;
        } else if digit_index < self.data.len() {
            self.data[digit_index] &= !bit_mask;
            // the top bit may have been cleared, so normalize
            self.normalize();
        }
    }
}

impl num_traits::FromBytes for BigUint {
    type Bytes = [u8];

    fn from_be_bytes(bytes: &Self::Bytes) -> Self {
        Self::from_bytes_be(bytes)
    }

    fn from_le_bytes(bytes: &Self::Bytes) -> Self {
        Self::from_bytes_le(bytes)
    }
}

impl num_traits::ToBytes for BigUint {
    type Bytes = Vec<u8>;

    fn to_be_bytes(&self) -> Self::Bytes {
        self.to_bytes_be()
    }

    fn to_le_bytes(&self) -> Self::Bytes {
        self.to_bytes_le()
    }
}

pub(crate) trait IntDigits {
    fn digits(&self) -> &[BigDigit];
    fn digits_mut(&mut self) -> &mut Vec<BigDigit>;
    fn normalize(&mut self);
    fn capacity(&self) -> usize;
    fn len(&self) -> usize;
}

impl IntDigits for BigUint {
    #[inline]
    fn digits(&self) -> &[BigDigit] {
        &self.data
    }
    #[inline]
    fn digits_mut(&mut self) -> &mut Vec<BigDigit> {
        &mut self.data
    }
    #[inline]
    fn normalize(&mut self) {
        self.normalize();
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

/// Convert a `u32` chunk (len is either 1 or 2) to a single `u64` digit
#[inline]
fn u32_chunk_to_u64(chunk: &[u32]) -> u64 {
    // raw could have odd length
    let mut digit = chunk[0] as u64;
    if let Some(&hi) = chunk.get(1) {
        digit |= (hi as u64) << 32;
    }
    digit
}

cfg_32_or_test!(
    /// Combine four `u32`s into a single `u128`.
    #[inline]
    fn u32_to_u128(a: u32, b: u32, c: u32, d: u32) -> u128 {
        u128::from(d) | (u128::from(c) << 32) | (u128::from(b) << 64) | (u128::from(a) << 96)
    }
);

cfg_32_or_test!(
    /// Split a single `u128` into four `u32`.
    #[inline]
    fn u32_from_u128(n: u128) -> (u32, u32, u32, u32) {
        (
            (n >> 96) as u32,
            (n >> 64) as u32,
            (n >> 32) as u32,
            n as u32,
        )
    }
);

cfg_digit!(
    #[test]
    fn test_from_slice() {
        fn check(slice: &[u32], data: &[BigDigit]) {
            assert_eq!(BigUint::from_slice(slice).data, data);
        }
        check(&[1], &[1]);
        check(&[0, 0, 0], &[]);
        check(&[1, 2, 0, 0], &[1, 2]);
        check(&[0, 0, 1, 2], &[0, 0, 1, 2]);
        check(&[0, 0, 1, 2, 0, 0], &[0, 0, 1, 2]);
        check(&[-1i32 as u32], &[-1i32 as BigDigit]);
    }

    #[test]
    fn test_from_slice() {
        fn check(slice: &[u32], data: &[BigDigit]) {
            assert_eq!(
                BigUint::from_slice(slice).data,
                data,
                "from {:?}, to {:?}",
                slice,
                data
            );
        }
        check(&[1], &[1]);
        check(&[0, 0, 0], &[]);
        check(&[1, 2], &[8_589_934_593]);
        check(&[1, 2, 0, 0], &[8_589_934_593]);
        check(&[0, 0, 1, 2], &[0, 8_589_934_593]);
        check(&[0, 0, 1, 2, 0, 0], &[0, 8_589_934_593]);
        check(&[-1i32 as u32], &[(-1i32 as u32) as BigDigit]);
    }
);

#[test]
fn test_u32_u128() {
    assert_eq!(u32_from_u128(0u128), (0, 0, 0, 0));
    assert_eq!(
        u32_from_u128(u128::MAX),
        (u32::MAX, u32::MAX, u32::MAX, u32::MAX)
    );

    assert_eq!(u32_from_u128(u32::MAX as u128), (0, 0, 0, u32::MAX));

    assert_eq!(u32_from_u128(u64::MAX as u128), (0, 0, u32::MAX, u32::MAX));

    assert_eq!(
        u32_from_u128((u64::MAX as u128) + u32::MAX as u128),
        (0, 1, 0, u32::MAX - 1)
    );

    assert_eq!(u32_from_u128(36_893_488_151_714_070_528), (0, 2, 1, 0));
}

#[test]
fn test_u128_u32_roundtrip() {
    // roundtrips
    let values = vec![
        0u128,
        1u128,
        u64::MAX as u128 * 3,
        u32::MAX as u128,
        u64::MAX as u128,
        (u64::MAX as u128) + u32::MAX as u128,
        u128::MAX,
    ];

    for val in &values {
        let (a, b, c, d) = u32_from_u128(*val);
        assert_eq!(u32_to_u128(a, b, c, d), *val);
    }
}
