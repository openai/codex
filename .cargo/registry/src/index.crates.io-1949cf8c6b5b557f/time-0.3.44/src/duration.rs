//! The [`Duration`] struct and its associated `impl`s.

use core::cmp::Ordering;
use core::fmt;
use core::iter::Sum;
use core::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};
use core::time::Duration as StdDuration;
#[cfg(feature = "std")]
use std::time::SystemTime;

use deranged::RangedI32;
use num_conv::prelude::*;

use crate::convert::*;
use crate::error;
use crate::internal_macros::{
    const_try_opt, expect_opt, impl_add_assign, impl_div_assign, impl_mul_assign, impl_sub_assign,
};
#[cfg(feature = "std")]
#[expect(deprecated)]
use crate::Instant;

/// By explicitly inserting this enum where padding is expected, the compiler is able to better
/// perform niche value optimization.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Padding {
    #[allow(clippy::missing_docs_in_private_items)]
    Optimize,
}

/// The type of the `nanosecond` field of `Duration`.
type Nanoseconds =
    RangedI32<{ -Nanosecond::per_t::<i32>(Second) + 1 }, { Nanosecond::per_t::<i32>(Second) - 1 }>;

/// A span of time with nanosecond precision.
///
/// Each `Duration` is composed of a whole number of seconds and a fractional part represented in
/// nanoseconds.
///
/// This implementation allows for negative durations, unlike [`core::time::Duration`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Duration {
    /// Number of whole seconds.
    seconds: i64,
    /// Number of nanoseconds within the second. The sign always matches the `seconds` field.
    // Sign must match that of `seconds` (though this is not a safety requirement).
    nanoseconds: Nanoseconds,
    padding: Padding,
}

impl fmt::Debug for Duration {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Duration")
            .field("seconds", &self.seconds)
            .field("nanoseconds", &self.nanoseconds)
            .finish()
    }
}

impl Default for Duration {
    #[inline]
    fn default() -> Self {
        Self {
            seconds: 0,
            nanoseconds: Nanoseconds::new_static::<0>(),
            padding: Padding::Optimize,
        }
    }
}

/// This is adapted from the [`std` implementation][std], which uses mostly bit
/// operations to ensure the highest precision:
///
/// Changes from `std` are marked and explained below.
///
/// [std]: https://github.com/rust-lang/rust/blob/3a37c2f0523c87147b64f1b8099fc9df22e8c53e/library/core/src/time.rs#L1262-L1340
#[rustfmt::skip] // Skip `rustfmt` because it reformats the arguments of the macro weirdly.
macro_rules! try_from_secs {
    (
        secs = $secs: expr,
        mantissa_bits = $mant_bits: literal,
        exponent_bits = $exp_bits: literal,
        offset = $offset: literal,
        bits_ty = $bits_ty:ty,
        bits_ty_signed = $bits_ty_signed:ty,
        double_ty = $double_ty:ty,
        float_ty = $float_ty:ty,
        is_nan = $is_nan:expr,
        is_overflow = $is_overflow:expr,
    ) => {{
        'value: {
            const MIN_EXP: i16 = 1 - (1i16 << $exp_bits) / 2;
            const MANT_MASK: $bits_ty = (1 << $mant_bits) - 1;
            const EXP_MASK: $bits_ty = (1 << $exp_bits) - 1;

            // Change from std: No error check for negative values necessary.

            let bits = $secs.to_bits();
            let mant = (bits & MANT_MASK) | (MANT_MASK + 1);
            let exp = ((bits >> $mant_bits) & EXP_MASK) as i16 + MIN_EXP;

            let (secs, nanos) = if exp < -31 {
                // the input represents less than 1ns and can not be rounded to it
                (0u64, 0u32)
            } else if exp < 0 {
                // the input is less than 1 second
                let t = <$double_ty>::from(mant) << ($offset + exp);
                let nanos_offset = $mant_bits + $offset;
                let nanos_tmp = Nanosecond::per_t::<u128>(Second) * u128::from(t);
                let nanos = (nanos_tmp >> nanos_offset) as u32;

                let rem_mask = (1 << nanos_offset) - 1;
                let rem_msb_mask = 1 << (nanos_offset - 1);
                let rem = nanos_tmp & rem_mask;
                let is_tie = rem == rem_msb_mask;
                let is_even = (nanos & 1) == 0;
                let rem_msb = nanos_tmp & rem_msb_mask == 0;
                let add_ns = !(rem_msb || (is_even && is_tie));

                // f32 does not have enough precision to trigger the second branch
                // since it can not represent numbers between 0.999_999_940_395 and 1.0.
                let nanos = nanos + add_ns as u32;
                if ($mant_bits == 23) || (nanos != Nanosecond::per_t::<u32>(Second)) {
                    (0, nanos)
                } else {
                    (1, 0)
                }
            } else if exp < $mant_bits {
                let secs = u64::from(mant >> ($mant_bits - exp));
                let t = <$double_ty>::from((mant << exp) & MANT_MASK);
                let nanos_offset = $mant_bits;
                let nanos_tmp = Nanosecond::per_t::<$double_ty>(Second) * t;
                let nanos = (nanos_tmp >> nanos_offset) as u32;

                let rem_mask = (1 << nanos_offset) - 1;
                let rem_msb_mask = 1 << (nanos_offset - 1);
                let rem = nanos_tmp & rem_mask;
                let is_tie = rem == rem_msb_mask;
                let is_even = (nanos & 1) == 0;
                let rem_msb = nanos_tmp & rem_msb_mask == 0;
                let add_ns = !(rem_msb || (is_even && is_tie));

                // f32 does not have enough precision to trigger the second branch.
                // For example, it can not represent numbers between 1.999_999_880...
                // and 2.0. Bigger values result in even smaller precision of the
                // fractional part.
                let nanos = nanos + add_ns as u32;
                if ($mant_bits == 23) || (nanos != Nanosecond::per_t::<u32>(Second)) {
                    (secs, nanos)
                } else {
                    (secs + 1, 0)
                }
            } else if exp < 63 {
                // Change from std: The exponent here is 63 instead of 64,
                // because i64::MAX + 1 is 2^63.

                // the input has no fractional part
                let secs = u64::from(mant) << (exp - $mant_bits);
                (secs, 0)
            } else if bits == (i64::MIN as $float_ty).to_bits() {
                // Change from std: Signed integers are asymmetrical in that
                // iN::MIN is -iN::MAX - 1. So for example i8 covers the
                // following numbers -128..=127. The check above (exp < 63)
                // doesn't cover i64::MIN as that is -2^63, so we have this
                // additional case to handle the asymmetry of iN::MIN.
                break 'value Self::new_ranged_unchecked(i64::MIN, Nanoseconds::new_static::<0>());
            } else if $secs.is_nan() {
                // Change from std: std doesn't differentiate between the error
                // cases.
                $is_nan
            } else {
                $is_overflow
            };

            // Change from std: All the code is mostly unmodified in that it
            // simply calculates an unsigned integer. Here we extract the sign
            // bit and assign it to the number. We basically manually do two's
            // complement here, we could also use an if and just negate the
            // numbers based on the sign, but it turns out to be quite a bit
            // slower.
            let mask = (bits as $bits_ty_signed) >> ($mant_bits + $exp_bits);
            #[allow(trivial_numeric_casts)]
            let secs_signed = ((secs as i64) ^ (mask as i64)) - (mask as i64);
            #[allow(trivial_numeric_casts)]
            let nanos_signed = ((nanos as i32) ^ (mask as i32)) - (mask as i32);
            // Safety: `nanos_signed` is in range.
            unsafe { Self::new_unchecked(secs_signed, nanos_signed) }
        }
    }};
}

impl Duration {
    /// Equivalent to `0.seconds()`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::ZERO, 0.seconds());
    /// ```
    pub const ZERO: Self = Self::seconds(0);

    /// Equivalent to `1.nanoseconds()`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::NANOSECOND, 1.nanoseconds());
    /// ```
    pub const NANOSECOND: Self = Self::nanoseconds(1);

    /// Equivalent to `1.microseconds()`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::MICROSECOND, 1.microseconds());
    /// ```
    pub const MICROSECOND: Self = Self::microseconds(1);

    /// Equivalent to `1.milliseconds()`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::MILLISECOND, 1.milliseconds());
    /// ```
    pub const MILLISECOND: Self = Self::milliseconds(1);

    /// Equivalent to `1.seconds()`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::SECOND, 1.seconds());
    /// ```
    pub const SECOND: Self = Self::seconds(1);

    /// Equivalent to `1.minutes()`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::MINUTE, 1.minutes());
    /// ```
    pub const MINUTE: Self = Self::minutes(1);

    /// Equivalent to `1.hours()`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::HOUR, 1.hours());
    /// ```
    pub const HOUR: Self = Self::hours(1);

    /// Equivalent to `1.days()`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::DAY, 1.days());
    /// ```
    pub const DAY: Self = Self::days(1);

    /// Equivalent to `1.weeks()`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::WEEK, 1.weeks());
    /// ```
    pub const WEEK: Self = Self::weeks(1);

    /// The minimum possible duration. Adding any negative duration to this will cause an overflow.
    pub const MIN: Self = Self::new_ranged(i64::MIN, Nanoseconds::MIN);

    /// The maximum possible duration. Adding any positive duration to this will cause an overflow.
    pub const MAX: Self = Self::new_ranged(i64::MAX, Nanoseconds::MAX);

    /// Check if a duration is exactly zero.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert!(0.seconds().is_zero());
    /// assert!(!1.nanoseconds().is_zero());
    /// ```
    #[inline]
    pub const fn is_zero(self) -> bool {
        self.seconds == 0 && self.nanoseconds.get() == 0
    }

    /// Check if a duration is negative.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert!((-1).seconds().is_negative());
    /// assert!(!0.seconds().is_negative());
    /// assert!(!1.seconds().is_negative());
    /// ```
    #[inline]
    pub const fn is_negative(self) -> bool {
        self.seconds < 0 || self.nanoseconds.get() < 0
    }

    /// Check if a duration is positive.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert!(1.seconds().is_positive());
    /// assert!(!0.seconds().is_positive());
    /// assert!(!(-1).seconds().is_positive());
    /// ```
    #[inline]
    pub const fn is_positive(self) -> bool {
        self.seconds > 0 || self.nanoseconds.get() > 0
    }

    /// Get the absolute value of the duration.
    ///
    /// This method saturates the returned value if it would otherwise overflow.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(1.seconds().abs(), 1.seconds());
    /// assert_eq!(0.seconds().abs(), 0.seconds());
    /// assert_eq!((-1).seconds().abs(), 1.seconds());
    /// ```
    #[inline]
    pub const fn abs(self) -> Self {
        match self.seconds.checked_abs() {
            Some(seconds) => Self::new_ranged_unchecked(seconds, self.nanoseconds.abs()),
            None => Self::MAX,
        }
    }

    /// Convert the existing `Duration` to a `std::time::Duration` and its sign. This returns a
    /// [`std::time::Duration`] and does not saturate the returned value (unlike [`Duration::abs`]).
    ///
    /// ```rust
    /// # use time::ext::{NumericalDuration, NumericalStdDuration};
    /// assert_eq!(1.seconds().unsigned_abs(), 1.std_seconds());
    /// assert_eq!(0.seconds().unsigned_abs(), 0.std_seconds());
    /// assert_eq!((-1).seconds().unsigned_abs(), 1.std_seconds());
    /// ```
    #[inline]
    pub const fn unsigned_abs(self) -> StdDuration {
        StdDuration::new(
            self.seconds.unsigned_abs(),
            self.nanoseconds.get().unsigned_abs(),
        )
    }

    /// Create a new `Duration` without checking the validity of the components.
    ///
    /// # Safety
    ///
    /// - `nanoseconds` must be in the range `-999_999_999..=999_999_999`.
    ///
    /// While the sign of `nanoseconds` is required to be the same as the sign of `seconds`, this is
    /// not a safety invariant.
    #[inline]
    #[track_caller]
    pub(crate) const unsafe fn new_unchecked(seconds: i64, nanoseconds: i32) -> Self {
        Self::new_ranged_unchecked(
            seconds,
            // Safety: The caller must uphold the safety invariants.
            unsafe { Nanoseconds::new_unchecked(nanoseconds) },
        )
    }

    /// Create a new `Duration` without checking the validity of the components.
    #[inline]
    #[track_caller]
    pub(crate) const fn new_ranged_unchecked(seconds: i64, nanoseconds: Nanoseconds) -> Self {
        if seconds < 0 {
            debug_assert!(nanoseconds.get() <= 0);
        } else if seconds > 0 {
            debug_assert!(nanoseconds.get() >= 0);
        }

        Self {
            seconds,
            nanoseconds,
            padding: Padding::Optimize,
        }
    }

    /// Create a new `Duration` with the provided seconds and nanoseconds. If nanoseconds is at
    /// least ±10<sup>9</sup>, it will wrap to the number of seconds.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::new(1, 0), 1.seconds());
    /// assert_eq!(Duration::new(-1, 0), (-1).seconds());
    /// assert_eq!(Duration::new(1, 2_000_000_000), 3.seconds());
    /// ```
    ///
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    pub const fn new(mut seconds: i64, mut nanoseconds: i32) -> Self {
        seconds = expect_opt!(
            seconds.checked_add(nanoseconds as i64 / Nanosecond::per_t::<i64>(Second)),
            "overflow constructing `time::Duration`"
        );
        nanoseconds %= Nanosecond::per_t::<i32>(Second);

        if seconds > 0 && nanoseconds < 0 {
            // `seconds` cannot overflow here because it is positive.
            seconds -= 1;
            nanoseconds += Nanosecond::per_t::<i32>(Second);
        } else if seconds < 0 && nanoseconds > 0 {
            // `seconds` cannot overflow here because it is negative.
            seconds += 1;
            nanoseconds -= Nanosecond::per_t::<i32>(Second);
        }

        // Safety: `nanoseconds` is in range due to the modulus above.
        unsafe { Self::new_unchecked(seconds, nanoseconds) }
    }

    /// Create a new `Duration` with the provided seconds and nanoseconds.
    #[inline]
    pub(crate) const fn new_ranged(mut seconds: i64, mut nanoseconds: Nanoseconds) -> Self {
        if seconds > 0 && nanoseconds.get() < 0 {
            // `seconds` cannot overflow here because it is positive.
            seconds -= 1;
            // Safety: `nanoseconds` is negative with a maximum of 999,999,999, so adding a billion
            // to it is guaranteed to result in an in-range value.
            nanoseconds = unsafe {
                Nanoseconds::new_unchecked(nanoseconds.get() + Nanosecond::per_t::<i32>(Second))
            };
        } else if seconds < 0 && nanoseconds.get() > 0 {
            // `seconds` cannot overflow here because it is negative.
            seconds += 1;
            // Safety: `nanoseconds` is positive with a minimum of -999,999,999, so subtracting a
            // billion from it is guaranteed to result in an in-range value.
            nanoseconds = unsafe {
                Nanoseconds::new_unchecked(nanoseconds.get() - Nanosecond::per_t::<i32>(Second))
            };
        }

        Self::new_ranged_unchecked(seconds, nanoseconds)
    }

    /// Create a new `Duration` with the given number of weeks. Equivalent to
    /// `Duration::seconds(weeks * 604_800)`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::weeks(1), 604_800.seconds());
    /// ```
    ///
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    pub const fn weeks(weeks: i64) -> Self {
        Self::seconds(expect_opt!(
            weeks.checked_mul(Second::per_t(Week)),
            "overflow constructing `time::Duration`"
        ))
    }

    /// Create a new `Duration` with the given number of days. Equivalent to
    /// `Duration::seconds(days * 86_400)`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::days(1), 86_400.seconds());
    /// ```
    ///
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    pub const fn days(days: i64) -> Self {
        Self::seconds(expect_opt!(
            days.checked_mul(Second::per_t(Day)),
            "overflow constructing `time::Duration`"
        ))
    }

    /// Create a new `Duration` with the given number of hours. Equivalent to
    /// `Duration::seconds(hours * 3_600)`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::hours(1), 3_600.seconds());
    /// ```
    ///
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    pub const fn hours(hours: i64) -> Self {
        Self::seconds(expect_opt!(
            hours.checked_mul(Second::per_t(Hour)),
            "overflow constructing `time::Duration`"
        ))
    }

    /// Create a new `Duration` with the given number of minutes. Equivalent to
    /// `Duration::seconds(minutes * 60)`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::minutes(1), 60.seconds());
    /// ```
    ///
    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    pub const fn minutes(minutes: i64) -> Self {
        Self::seconds(expect_opt!(
            minutes.checked_mul(Second::per_t(Minute)),
            "overflow constructing `time::Duration`"
        ))
    }

    /// Create a new `Duration` with the given number of seconds.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::seconds(1), 1_000.milliseconds());
    /// ```
    #[inline]
    pub const fn seconds(seconds: i64) -> Self {
        Self::new_ranged_unchecked(seconds, Nanoseconds::new_static::<0>())
    }

    /// Creates a new `Duration` from the specified number of seconds represented as `f64`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::seconds_f64(0.5), 0.5.seconds());
    /// assert_eq!(Duration::seconds_f64(-0.5), (-0.5).seconds());
    /// ```
    #[inline]
    #[track_caller]
    pub fn seconds_f64(seconds: f64) -> Self {
        try_from_secs!(
            secs = seconds,
            mantissa_bits = 52,
            exponent_bits = 11,
            offset = 44,
            bits_ty = u64,
            bits_ty_signed = i64,
            double_ty = u128,
            float_ty = f64,
            is_nan = crate::expect_failed("passed NaN to `time::Duration::seconds_f64`"),
            is_overflow = crate::expect_failed("overflow constructing `time::Duration`"),
        )
    }

    /// Creates a new `Duration` from the specified number of seconds represented as `f32`.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::seconds_f32(0.5), 0.5.seconds());
    /// assert_eq!(Duration::seconds_f32(-0.5), (-0.5).seconds());
    /// ```
    #[inline]
    #[track_caller]
    pub fn seconds_f32(seconds: f32) -> Self {
        try_from_secs!(
            secs = seconds,
            mantissa_bits = 23,
            exponent_bits = 8,
            offset = 41,
            bits_ty = u32,
            bits_ty_signed = i32,
            double_ty = u64,
            float_ty = f32,
            is_nan = crate::expect_failed("passed NaN to `time::Duration::seconds_f32`"),
            is_overflow = crate::expect_failed("overflow constructing `time::Duration`"),
        )
    }

    /// Creates a new `Duration` from the specified number of seconds
    /// represented as `f64`. Any values that are out of bounds are saturated at
    /// the minimum or maximum respectively. `NaN` gets turned into a `Duration`
    /// of 0 seconds.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::saturating_seconds_f64(0.5), 0.5.seconds());
    /// assert_eq!(Duration::saturating_seconds_f64(-0.5), (-0.5).seconds());
    /// assert_eq!(
    ///     Duration::saturating_seconds_f64(f64::NAN),
    ///     Duration::new(0, 0),
    /// );
    /// assert_eq!(
    ///     Duration::saturating_seconds_f64(f64::NEG_INFINITY),
    ///     Duration::MIN,
    /// );
    /// assert_eq!(
    ///     Duration::saturating_seconds_f64(f64::INFINITY),
    ///     Duration::MAX,
    /// );
    /// ```
    #[inline]
    pub fn saturating_seconds_f64(seconds: f64) -> Self {
        try_from_secs!(
            secs = seconds,
            mantissa_bits = 52,
            exponent_bits = 11,
            offset = 44,
            bits_ty = u64,
            bits_ty_signed = i64,
            double_ty = u128,
            float_ty = f64,
            is_nan = return Self::ZERO,
            is_overflow = return if seconds < 0.0 { Self::MIN } else { Self::MAX },
        )
    }

    /// Creates a new `Duration` from the specified number of seconds
    /// represented as `f32`. Any values that are out of bounds are saturated at
    /// the minimum or maximum respectively. `NaN` gets turned into a `Duration`
    /// of 0 seconds.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::saturating_seconds_f32(0.5), 0.5.seconds());
    /// assert_eq!(Duration::saturating_seconds_f32(-0.5), (-0.5).seconds());
    /// assert_eq!(
    ///     Duration::saturating_seconds_f32(f32::NAN),
    ///     Duration::new(0, 0),
    /// );
    /// assert_eq!(
    ///     Duration::saturating_seconds_f32(f32::NEG_INFINITY),
    ///     Duration::MIN,
    /// );
    /// assert_eq!(
    ///     Duration::saturating_seconds_f32(f32::INFINITY),
    ///     Duration::MAX,
    /// );
    /// ```
    #[inline]
    pub fn saturating_seconds_f32(seconds: f32) -> Self {
        try_from_secs!(
            secs = seconds,
            mantissa_bits = 23,
            exponent_bits = 8,
            offset = 41,
            bits_ty = u32,
            bits_ty_signed = i32,
            double_ty = u64,
            float_ty = f32,
            is_nan = return Self::ZERO,
            is_overflow = return if seconds < 0.0 { Self::MIN } else { Self::MAX },
        )
    }

    /// Creates a new `Duration` from the specified number of seconds
    /// represented as `f64`. Returns `None` if the `Duration` can't be
    /// represented.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::checked_seconds_f64(0.5), Some(0.5.seconds()));
    /// assert_eq!(Duration::checked_seconds_f64(-0.5), Some((-0.5).seconds()));
    /// assert_eq!(Duration::checked_seconds_f64(f64::NAN), None);
    /// assert_eq!(Duration::checked_seconds_f64(f64::NEG_INFINITY), None);
    /// assert_eq!(Duration::checked_seconds_f64(f64::INFINITY), None);
    /// ```
    #[inline]
    pub fn checked_seconds_f64(seconds: f64) -> Option<Self> {
        Some(try_from_secs!(
            secs = seconds,
            mantissa_bits = 52,
            exponent_bits = 11,
            offset = 44,
            bits_ty = u64,
            bits_ty_signed = i64,
            double_ty = u128,
            float_ty = f64,
            is_nan = return None,
            is_overflow = return None,
        ))
    }

    /// Creates a new `Duration` from the specified number of seconds
    /// represented as `f32`. Returns `None` if the `Duration` can't be
    /// represented.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::checked_seconds_f32(0.5), Some(0.5.seconds()));
    /// assert_eq!(Duration::checked_seconds_f32(-0.5), Some((-0.5).seconds()));
    /// assert_eq!(Duration::checked_seconds_f32(f32::NAN), None);
    /// assert_eq!(Duration::checked_seconds_f32(f32::NEG_INFINITY), None);
    /// assert_eq!(Duration::checked_seconds_f32(f32::INFINITY), None);
    /// ```
    #[inline]
    pub fn checked_seconds_f32(seconds: f32) -> Option<Self> {
        Some(try_from_secs!(
            secs = seconds,
            mantissa_bits = 23,
            exponent_bits = 8,
            offset = 41,
            bits_ty = u32,
            bits_ty_signed = i32,
            double_ty = u64,
            float_ty = f32,
            is_nan = return None,
            is_overflow = return None,
        ))
    }

    /// Create a new `Duration` with the given number of milliseconds.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::milliseconds(1), 1_000.microseconds());
    /// assert_eq!(Duration::milliseconds(-1), (-1_000).microseconds());
    /// ```
    #[inline]
    pub const fn milliseconds(milliseconds: i64) -> Self {
        // Safety: `nanoseconds` is guaranteed to be in range because of the modulus.
        unsafe {
            Self::new_unchecked(
                milliseconds / Millisecond::per_t::<i64>(Second),
                (milliseconds % Millisecond::per_t::<i64>(Second)
                    * Nanosecond::per_t::<i64>(Millisecond)) as i32,
            )
        }
    }

    /// Create a new `Duration` with the given number of microseconds.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::microseconds(1), 1_000.nanoseconds());
    /// assert_eq!(Duration::microseconds(-1), (-1_000).nanoseconds());
    /// ```
    #[inline]
    pub const fn microseconds(microseconds: i64) -> Self {
        // Safety: `nanoseconds` is guaranteed to be in range because of the modulus.
        unsafe {
            Self::new_unchecked(
                microseconds / Microsecond::per_t::<i64>(Second),
                (microseconds % Microsecond::per_t::<i64>(Second)
                    * Nanosecond::per_t::<i64>(Microsecond)) as i32,
            )
        }
    }

    /// Create a new `Duration` with the given number of nanoseconds.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(Duration::nanoseconds(1), 1.microseconds() / 1_000);
    /// assert_eq!(Duration::nanoseconds(-1), (-1).microseconds() / 1_000);
    /// ```
    #[inline]
    pub const fn nanoseconds(nanoseconds: i64) -> Self {
        // Safety: `nanoseconds` is guaranteed to be in range because of the modulus.
        unsafe {
            Self::new_unchecked(
                nanoseconds / Nanosecond::per_t::<i64>(Second),
                (nanoseconds % Nanosecond::per_t::<i64>(Second)) as i32,
            )
        }
    }

    /// Create a new `Duration` with the given number of nanoseconds.
    ///
    /// As the input range cannot be fully mapped to the output, this should only be used where it's
    /// known to result in a valid value.
    #[inline]
    #[track_caller]
    pub(crate) const fn nanoseconds_i128(nanoseconds: i128) -> Self {
        let seconds = nanoseconds / Nanosecond::per_t::<i128>(Second);
        let nanoseconds = nanoseconds % Nanosecond::per_t::<i128>(Second);

        if seconds > i64::MAX as i128 || seconds < i64::MIN as i128 {
            crate::expect_failed("overflow constructing `time::Duration`");
        }

        // Safety: `nanoseconds` is guaranteed to be in range because of the modulus above.
        unsafe { Self::new_unchecked(seconds as i64, nanoseconds as i32) }
    }

    /// Get the number of whole weeks in the duration.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(1.weeks().whole_weeks(), 1);
    /// assert_eq!((-1).weeks().whole_weeks(), -1);
    /// assert_eq!(6.days().whole_weeks(), 0);
    /// assert_eq!((-6).days().whole_weeks(), 0);
    /// ```
    #[inline]
    pub const fn whole_weeks(self) -> i64 {
        self.whole_seconds() / Second::per_t::<i64>(Week)
    }

    /// Get the number of whole days in the duration.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(1.days().whole_days(), 1);
    /// assert_eq!((-1).days().whole_days(), -1);
    /// assert_eq!(23.hours().whole_days(), 0);
    /// assert_eq!((-23).hours().whole_days(), 0);
    /// ```
    #[inline]
    pub const fn whole_days(self) -> i64 {
        self.whole_seconds() / Second::per_t::<i64>(Day)
    }

    /// Get the number of whole hours in the duration.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(1.hours().whole_hours(), 1);
    /// assert_eq!((-1).hours().whole_hours(), -1);
    /// assert_eq!(59.minutes().whole_hours(), 0);
    /// assert_eq!((-59).minutes().whole_hours(), 0);
    /// ```
    #[inline]
    pub const fn whole_hours(self) -> i64 {
        self.whole_seconds() / Second::per_t::<i64>(Hour)
    }

    /// Get the number of whole minutes in the duration.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(1.minutes().whole_minutes(), 1);
    /// assert_eq!((-1).minutes().whole_minutes(), -1);
    /// assert_eq!(59.seconds().whole_minutes(), 0);
    /// assert_eq!((-59).seconds().whole_minutes(), 0);
    /// ```
    #[inline]
    pub const fn whole_minutes(self) -> i64 {
        self.whole_seconds() / Second::per_t::<i64>(Minute)
    }

    /// Get the number of whole seconds in the duration.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(1.seconds().whole_seconds(), 1);
    /// assert_eq!((-1).seconds().whole_seconds(), -1);
    /// assert_eq!(1.minutes().whole_seconds(), 60);
    /// assert_eq!((-1).minutes().whole_seconds(), -60);
    /// ```
    #[inline]
    pub const fn whole_seconds(self) -> i64 {
        self.seconds
    }

    /// Get the number of fractional seconds in the duration.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(1.5.seconds().as_seconds_f64(), 1.5);
    /// assert_eq!((-1.5).seconds().as_seconds_f64(), -1.5);
    /// ```
    #[inline]
    pub fn as_seconds_f64(self) -> f64 {
        self.seconds as f64 + self.nanoseconds.get() as f64 / Nanosecond::per_t::<f64>(Second)
    }

    /// Get the number of fractional seconds in the duration.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(1.5.seconds().as_seconds_f32(), 1.5);
    /// assert_eq!((-1.5).seconds().as_seconds_f32(), -1.5);
    /// ```
    #[inline]
    pub fn as_seconds_f32(self) -> f32 {
        self.seconds as f32 + self.nanoseconds.get() as f32 / Nanosecond::per_t::<f32>(Second)
    }

    /// Get the number of whole milliseconds in the duration.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(1.seconds().whole_milliseconds(), 1_000);
    /// assert_eq!((-1).seconds().whole_milliseconds(), -1_000);
    /// assert_eq!(1.milliseconds().whole_milliseconds(), 1);
    /// assert_eq!((-1).milliseconds().whole_milliseconds(), -1);
    /// ```
    #[inline]
    pub const fn whole_milliseconds(self) -> i128 {
        self.seconds as i128 * Millisecond::per_t::<i128>(Second)
            + self.nanoseconds.get() as i128 / Nanosecond::per_t::<i128>(Millisecond)
    }

    /// Get the number of milliseconds past the number of whole seconds.
    ///
    /// Always in the range `-999..=999`.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(1.4.seconds().subsec_milliseconds(), 400);
    /// assert_eq!((-1.4).seconds().subsec_milliseconds(), -400);
    /// ```
    #[inline]
    pub const fn subsec_milliseconds(self) -> i16 {
        (self.nanoseconds.get() / Nanosecond::per_t::<i32>(Millisecond)) as i16
    }

    /// Get the number of whole microseconds in the duration.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(1.milliseconds().whole_microseconds(), 1_000);
    /// assert_eq!((-1).milliseconds().whole_microseconds(), -1_000);
    /// assert_eq!(1.microseconds().whole_microseconds(), 1);
    /// assert_eq!((-1).microseconds().whole_microseconds(), -1);
    /// ```
    #[inline]
    pub const fn whole_microseconds(self) -> i128 {
        self.seconds as i128 * Microsecond::per_t::<i128>(Second)
            + self.nanoseconds.get() as i128 / Nanosecond::per_t::<i128>(Microsecond)
    }

    /// Get the number of microseconds past the number of whole seconds.
    ///
    /// Always in the range `-999_999..=999_999`.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(1.0004.seconds().subsec_microseconds(), 400);
    /// assert_eq!((-1.0004).seconds().subsec_microseconds(), -400);
    /// ```
    #[inline]
    pub const fn subsec_microseconds(self) -> i32 {
        self.nanoseconds.get() / Nanosecond::per_t::<i32>(Microsecond)
    }

    /// Get the number of nanoseconds in the duration.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(1.microseconds().whole_nanoseconds(), 1_000);
    /// assert_eq!((-1).microseconds().whole_nanoseconds(), -1_000);
    /// assert_eq!(1.nanoseconds().whole_nanoseconds(), 1);
    /// assert_eq!((-1).nanoseconds().whole_nanoseconds(), -1);
    /// ```
    #[inline]
    pub const fn whole_nanoseconds(self) -> i128 {
        self.seconds as i128 * Nanosecond::per_t::<i128>(Second) + self.nanoseconds.get() as i128
    }

    /// Get the number of nanoseconds past the number of whole seconds.
    ///
    /// The returned value will always be in the range `-999_999_999..=999_999_999`.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(1.000_000_400.seconds().subsec_nanoseconds(), 400);
    /// assert_eq!((-1.000_000_400).seconds().subsec_nanoseconds(), -400);
    /// ```
    #[inline]
    pub const fn subsec_nanoseconds(self) -> i32 {
        self.nanoseconds.get()
    }

    /// Get the number of nanoseconds past the number of whole seconds.
    #[cfg(feature = "quickcheck")]
    #[inline]
    pub(crate) const fn subsec_nanoseconds_ranged(self) -> Nanoseconds {
        self.nanoseconds
    }

    /// Computes `self + rhs`, returning `None` if an overflow occurred.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(5.seconds().checked_add(5.seconds()), Some(10.seconds()));
    /// assert_eq!(Duration::MAX.checked_add(1.nanoseconds()), None);
    /// assert_eq!((-5).seconds().checked_add(5.seconds()), Some(0.seconds()));
    /// ```
    #[inline]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        let mut seconds = const_try_opt!(self.seconds.checked_add(rhs.seconds));
        let mut nanoseconds = self.nanoseconds.get() + rhs.nanoseconds.get();

        if nanoseconds >= Nanosecond::per_t(Second) || seconds < 0 && nanoseconds > 0 {
            nanoseconds -= Nanosecond::per_t::<i32>(Second);
            seconds = const_try_opt!(seconds.checked_add(1));
        } else if nanoseconds <= -Nanosecond::per_t::<i32>(Second) || seconds > 0 && nanoseconds < 0
        {
            nanoseconds += Nanosecond::per_t::<i32>(Second);
            seconds = const_try_opt!(seconds.checked_sub(1));
        }

        // Safety: `nanoseconds` is guaranteed to be in range because of the overflow handling.
        unsafe { Some(Self::new_unchecked(seconds, nanoseconds)) }
    }

    /// Computes `self - rhs`, returning `None` if an overflow occurred.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(5.seconds().checked_sub(5.seconds()), Some(Duration::ZERO));
    /// assert_eq!(Duration::MIN.checked_sub(1.nanoseconds()), None);
    /// assert_eq!(5.seconds().checked_sub(10.seconds()), Some((-5).seconds()));
    /// ```
    #[inline]
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        let mut seconds = const_try_opt!(self.seconds.checked_sub(rhs.seconds));
        let mut nanoseconds = self.nanoseconds.get() - rhs.nanoseconds.get();

        if nanoseconds >= Nanosecond::per_t(Second) || seconds < 0 && nanoseconds > 0 {
            nanoseconds -= Nanosecond::per_t::<i32>(Second);
            seconds = const_try_opt!(seconds.checked_add(1));
        } else if nanoseconds <= -Nanosecond::per_t::<i32>(Second) || seconds > 0 && nanoseconds < 0
        {
            nanoseconds += Nanosecond::per_t::<i32>(Second);
            seconds = const_try_opt!(seconds.checked_sub(1));
        }

        // Safety: `nanoseconds` is guaranteed to be in range because of the overflow handling.
        unsafe { Some(Self::new_unchecked(seconds, nanoseconds)) }
    }

    /// Computes `self * rhs`, returning `None` if an overflow occurred.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(5.seconds().checked_mul(2), Some(10.seconds()));
    /// assert_eq!(5.seconds().checked_mul(-2), Some((-10).seconds()));
    /// assert_eq!(5.seconds().checked_mul(0), Some(0.seconds()));
    /// assert_eq!(Duration::MAX.checked_mul(2), None);
    /// assert_eq!(Duration::MIN.checked_mul(2), None);
    /// ```
    #[inline]
    pub const fn checked_mul(self, rhs: i32) -> Option<Self> {
        // Multiply nanoseconds as i64, because it cannot overflow that way.
        let total_nanos = self.nanoseconds.get() as i64 * rhs as i64;
        let extra_secs = total_nanos / Nanosecond::per_t::<i64>(Second);
        let nanoseconds = (total_nanos % Nanosecond::per_t::<i64>(Second)) as i32;
        let seconds = const_try_opt!(
            const_try_opt!(self.seconds.checked_mul(rhs as i64)).checked_add(extra_secs)
        );

        // Safety: `nanoseconds` is guaranteed to be in range because of the modulus above.
        unsafe { Some(Self::new_unchecked(seconds, nanoseconds)) }
    }

    /// Computes `self / rhs`, returning `None` if `rhs == 0` or if the result would overflow.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// assert_eq!(10.seconds().checked_div(2), Some(5.seconds()));
    /// assert_eq!(10.seconds().checked_div(-2), Some((-5).seconds()));
    /// assert_eq!(1.seconds().checked_div(0), None);
    /// ```
    #[inline]
    pub const fn checked_div(self, rhs: i32) -> Option<Self> {
        let (secs, extra_secs) = (
            const_try_opt!(self.seconds.checked_div(rhs as i64)),
            self.seconds % (rhs as i64),
        );
        let (mut nanos, extra_nanos) = (self.nanoseconds.get() / rhs, self.nanoseconds.get() % rhs);
        nanos += ((extra_secs * (Nanosecond::per_t::<i64>(Second)) + extra_nanos as i64)
            / (rhs as i64)) as i32;

        // Safety: `nanoseconds` is in range.
        unsafe { Some(Self::new_unchecked(secs, nanos)) }
    }

    /// Computes `-self`, returning `None` if the result would overflow.
    ///
    /// ```rust
    /// # use time::ext::NumericalDuration;
    /// # use time::Duration;
    /// assert_eq!(5.seconds().checked_neg(), Some((-5).seconds()));
    /// assert_eq!(Duration::MIN.checked_neg(), None);
    /// ```
    #[inline]
    pub const fn checked_neg(self) -> Option<Self> {
        if self.seconds == i64::MIN {
            None
        } else {
            Some(Self::new_ranged_unchecked(
                -self.seconds,
                self.nanoseconds.neg(),
            ))
        }
    }

    /// Computes `self + rhs`, saturating if an overflow occurred.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(5.seconds().saturating_add(5.seconds()), 10.seconds());
    /// assert_eq!(Duration::MAX.saturating_add(1.nanoseconds()), Duration::MAX);
    /// assert_eq!(
    ///     Duration::MIN.saturating_add((-1).nanoseconds()),
    ///     Duration::MIN
    /// );
    /// assert_eq!((-5).seconds().saturating_add(5.seconds()), Duration::ZERO);
    /// ```
    #[inline]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        let (mut seconds, overflow) = self.seconds.overflowing_add(rhs.seconds);
        if overflow {
            if self.seconds > 0 {
                return Self::MAX;
            }
            return Self::MIN;
        }
        let mut nanoseconds = self.nanoseconds.get() + rhs.nanoseconds.get();

        if nanoseconds >= Nanosecond::per_t(Second) || seconds < 0 && nanoseconds > 0 {
            nanoseconds -= Nanosecond::per_t::<i32>(Second);
            seconds = match seconds.checked_add(1) {
                Some(seconds) => seconds,
                None => return Self::MAX,
            };
        } else if nanoseconds <= -Nanosecond::per_t::<i32>(Second) || seconds > 0 && nanoseconds < 0
        {
            nanoseconds += Nanosecond::per_t::<i32>(Second);
            seconds = match seconds.checked_sub(1) {
                Some(seconds) => seconds,
                None => return Self::MIN,
            };
        }

        // Safety: `nanoseconds` is guaranteed to be in range because of the overflow handling.
        unsafe { Self::new_unchecked(seconds, nanoseconds) }
    }

    /// Computes `self - rhs`, saturating if an overflow occurred.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(5.seconds().saturating_sub(5.seconds()), Duration::ZERO);
    /// assert_eq!(Duration::MIN.saturating_sub(1.nanoseconds()), Duration::MIN);
    /// assert_eq!(
    ///     Duration::MAX.saturating_sub((-1).nanoseconds()),
    ///     Duration::MAX
    /// );
    /// assert_eq!(5.seconds().saturating_sub(10.seconds()), (-5).seconds());
    /// ```
    #[inline]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        let (mut seconds, overflow) = self.seconds.overflowing_sub(rhs.seconds);
        if overflow {
            if self.seconds > 0 {
                return Self::MAX;
            }
            return Self::MIN;
        }
        let mut nanoseconds = self.nanoseconds.get() - rhs.nanoseconds.get();

        if nanoseconds >= Nanosecond::per_t(Second) || seconds < 0 && nanoseconds > 0 {
            nanoseconds -= Nanosecond::per_t::<i32>(Second);
            seconds = match seconds.checked_add(1) {
                Some(seconds) => seconds,
                None => return Self::MAX,
            };
        } else if nanoseconds <= -Nanosecond::per_t::<i32>(Second) || seconds > 0 && nanoseconds < 0
        {
            nanoseconds += Nanosecond::per_t::<i32>(Second);
            seconds = match seconds.checked_sub(1) {
                Some(seconds) => seconds,
                None => return Self::MIN,
            };
        }

        // Safety: `nanoseconds` is guaranteed to be in range because of the overflow handling.
        unsafe { Self::new_unchecked(seconds, nanoseconds) }
    }

    /// Computes `self * rhs`, saturating if an overflow occurred.
    ///
    /// ```rust
    /// # use time::{Duration, ext::NumericalDuration};
    /// assert_eq!(5.seconds().saturating_mul(2), 10.seconds());
    /// assert_eq!(5.seconds().saturating_mul(-2), (-10).seconds());
    /// assert_eq!(5.seconds().saturating_mul(0), Duration::ZERO);
    /// assert_eq!(Duration::MAX.saturating_mul(2), Duration::MAX);
    /// assert_eq!(Duration::MIN.saturating_mul(2), Duration::MIN);
    /// assert_eq!(Duration::MAX.saturating_mul(-2), Duration::MIN);
    /// assert_eq!(Duration::MIN.saturating_mul(-2), Duration::MAX);
    /// ```
    #[inline]
    pub const fn saturating_mul(self, rhs: i32) -> Self {
        // Multiply nanoseconds as i64, because it cannot overflow that way.
        let total_nanos = self.nanoseconds.get() as i64 * rhs as i64;
        let extra_secs = total_nanos / Nanosecond::per_t::<i64>(Second);
        let nanoseconds = (total_nanos % Nanosecond::per_t::<i64>(Second)) as i32;
        let (seconds, overflow1) = self.seconds.overflowing_mul(rhs as i64);
        if overflow1 {
            if self.seconds > 0 && rhs > 0 || self.seconds < 0 && rhs < 0 {
                return Self::MAX;
            }
            return Self::MIN;
        }
        let (seconds, overflow2) = seconds.overflowing_add(extra_secs);
        if overflow2 {
            if self.seconds > 0 && rhs > 0 {
                return Self::MAX;
            }
            return Self::MIN;
        }

        // Safety: `nanoseconds` is guaranteed to be in range because of to the modulus above.
        unsafe { Self::new_unchecked(seconds, nanoseconds) }
    }

    /// Runs a closure, returning the duration of time it took to run. The return value of the
    /// closure is provided in the second part of the tuple.
    #[cfg(feature = "std")]
    #[doc(hidden)]
    #[inline]
    #[track_caller]
    #[deprecated(
        since = "0.3.32",
        note = "extremely limited use case, not intended for benchmarking"
    )]
    #[expect(deprecated)]
    pub fn time_fn<T>(f: impl FnOnce() -> T) -> (Self, T) {
        let start = Instant::now();
        let return_value = f();
        let end = Instant::now();

        (end - start, return_value)
    }
}

/// The format returned by this implementation is not stable and must not be relied upon.
///
/// By default this produces an exact, full-precision printout of the duration.
/// For a concise, rounded printout instead, you can use the `.N` format specifier:
///
/// ```
/// # use time::Duration;
/// #
/// let duration = Duration::new(123456, 789011223);
/// println!("{duration:.3}");
/// ```
///
/// For the purposes of this implementation, a day is exactly 24 hours and a minute is exactly 60
/// seconds.
impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_negative() {
            f.write_str("-")?;
        }

        if let Some(_precision) = f.precision() {
            // Concise, rounded representation.

            if self.is_zero() {
                // Write a zero value with the requested precision.
                return (0.).fmt(f).and_then(|_| f.write_str("s"));
            }

            /// Format the first item that produces a value greater than 1 and then break.
            macro_rules! item {
                ($name:literal, $value:expr) => {
                    let value = $value;
                    if value >= 1.0 {
                        return value.fmt(f).and_then(|_| f.write_str($name));
                    }
                };
            }

            // Even if this produces a de-normal float, because we're rounding we don't really care.
            let seconds = self.unsigned_abs().as_secs_f64();

            item!("d", seconds / Second::per_t::<f64>(Day));
            item!("h", seconds / Second::per_t::<f64>(Hour));
            item!("m", seconds / Second::per_t::<f64>(Minute));
            item!("s", seconds);
            item!("ms", seconds * Millisecond::per_t::<f64>(Second));
            item!("µs", seconds * Microsecond::per_t::<f64>(Second));
            item!("ns", seconds * Nanosecond::per_t::<f64>(Second));
        } else {
            // Precise, but verbose representation.

            if self.is_zero() {
                return f.write_str("0s");
            }

            /// Format a single item.
            macro_rules! item {
                ($name:literal, $value:expr) => {
                    match $value {
                        0 => Ok(()),
                        value => value.fmt(f).and_then(|_| f.write_str($name)),
                    }
                };
            }

            let seconds = self.seconds.unsigned_abs();
            let nanoseconds = self.nanoseconds.get().unsigned_abs();

            item!("d", seconds / Second::per_t::<u64>(Day))?;
            item!(
                "h",
                seconds / Second::per_t::<u64>(Hour) % Hour::per_t::<u64>(Day)
            )?;
            item!(
                "m",
                seconds / Second::per_t::<u64>(Minute) % Minute::per_t::<u64>(Hour)
            )?;
            item!("s", seconds % Second::per_t::<u64>(Minute))?;
            item!("ms", nanoseconds / Nanosecond::per_t::<u32>(Millisecond))?;
            item!(
                "µs",
                nanoseconds / Nanosecond::per_t::<u32>(Microsecond)
                    % Microsecond::per_t::<u32>(Millisecond)
            )?;
            item!("ns", nanoseconds % Nanosecond::per_t::<u32>(Microsecond))?;
        }

        Ok(())
    }
}

impl TryFrom<StdDuration> for Duration {
    type Error = error::ConversionRange;

    #[inline]
    fn try_from(original: StdDuration) -> Result<Self, error::ConversionRange> {
        Ok(Self::new(
            original
                .as_secs()
                .try_into()
                .map_err(|_| error::ConversionRange)?,
            original.subsec_nanos().cast_signed(),
        ))
    }
}

impl TryFrom<Duration> for StdDuration {
    type Error = error::ConversionRange;

    #[inline]
    fn try_from(duration: Duration) -> Result<Self, error::ConversionRange> {
        Ok(Self::new(
            duration
                .seconds
                .try_into()
                .map_err(|_| error::ConversionRange)?,
            duration
                .nanoseconds
                .get()
                .try_into()
                .map_err(|_| error::ConversionRange)?,
        ))
    }
}

impl Add for Duration {
    type Output = Self;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn add(self, rhs: Self) -> Self::Output {
        self.checked_add(rhs)
            .expect("overflow when adding durations")
    }
}

impl Add<StdDuration> for Duration {
    type Output = Self;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn add(self, std_duration: StdDuration) -> Self::Output {
        self + Self::try_from(std_duration)
            .expect("overflow converting `std::time::Duration` to `time::Duration`")
    }
}

impl Add<Duration> for StdDuration {
    type Output = Duration;

    #[inline]
    #[track_caller]
    fn add(self, rhs: Duration) -> Self::Output {
        rhs + self
    }
}

impl_add_assign!(Duration: Self, StdDuration);

impl AddAssign<Duration> for StdDuration {
    /// # Panics
    ///
    /// This may panic if the resulting addition cannot be represented.
    #[inline]
    #[track_caller]
    fn add_assign(&mut self, rhs: Duration) {
        *self = (*self + rhs).try_into().expect(
            "Cannot represent a resulting duration in std. Try `let x = x + rhs;`, which will \
             change the type.",
        );
    }
}

impl Neg for Duration {
    type Output = Self;

    #[inline]
    #[track_caller]
    fn neg(self) -> Self::Output {
        self.checked_neg().expect("overflow when negating duration")
    }
}

impl Sub for Duration {
    type Output = Self;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, rhs: Self) -> Self::Output {
        self.checked_sub(rhs)
            .expect("overflow when subtracting durations")
    }
}

impl Sub<StdDuration> for Duration {
    type Output = Self;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, rhs: StdDuration) -> Self::Output {
        self - Self::try_from(rhs)
            .expect("overflow converting `std::time::Duration` to `time::Duration`")
    }
}

impl Sub<Duration> for StdDuration {
    type Output = Duration;

    /// # Panics
    ///
    /// This may panic if an overflow occurs.
    #[inline]
    #[track_caller]
    fn sub(self, rhs: Duration) -> Self::Output {
        Duration::try_from(self)
            .expect("overflow converting `std::time::Duration` to `time::Duration`")
            - rhs
    }
}

impl_sub_assign!(Duration: Self, StdDuration);

impl SubAssign<Duration> for StdDuration {
    /// # Panics
    ///
    /// This may panic if the resulting subtraction can not be represented.
    #[inline]
    #[track_caller]
    fn sub_assign(&mut self, rhs: Duration) {
        *self = (*self - rhs).try_into().expect(
            "Cannot represent a resulting duration in std. Try `let x = x - rhs;`, which will \
             change the type.",
        );
    }
}

/// Implement `Mul` (reflexively) and `Div` for `Duration` for various types.
macro_rules! duration_mul_div_int {
    ($($type:ty),+) => {$(
        impl Mul<$type> for Duration {
            type Output = Self;

            #[inline]
            #[track_caller]
            fn mul(self, rhs: $type) -> Self::Output {
                Self::nanoseconds_i128(
                    self.whole_nanoseconds()
                        .checked_mul(rhs.cast_signed().extend::<i128>())
                        .expect("overflow when multiplying duration")
                )
            }
        }

        impl Mul<Duration> for $type {
            type Output = Duration;

            #[inline]
            #[track_caller]
            fn mul(self, rhs: Duration) -> Self::Output {
                rhs * self
            }
        }

        impl Div<$type> for Duration {
            type Output = Self;

            #[inline]
            #[track_caller]
            fn div(self, rhs: $type) -> Self::Output {
                Self::nanoseconds_i128(
                    self.whole_nanoseconds() / rhs.cast_signed().extend::<i128>()
                )
            }
        }
    )+};
}
duration_mul_div_int![i8, i16, i32, u8, u16, u32];

impl Mul<f32> for Duration {
    type Output = Self;

    #[inline]
    #[track_caller]
    fn mul(self, rhs: f32) -> Self::Output {
        Self::seconds_f32(self.as_seconds_f32() * rhs)
    }
}

impl Mul<Duration> for f32 {
    type Output = Duration;

    #[inline]
    #[track_caller]
    fn mul(self, rhs: Duration) -> Self::Output {
        rhs * self
    }
}

impl Mul<f64> for Duration {
    type Output = Self;

    #[inline]
    #[track_caller]
    fn mul(self, rhs: f64) -> Self::Output {
        Self::seconds_f64(self.as_seconds_f64() * rhs)
    }
}

impl Mul<Duration> for f64 {
    type Output = Duration;

    #[inline]
    #[track_caller]
    fn mul(self, rhs: Duration) -> Self::Output {
        rhs * self
    }
}

impl_mul_assign!(Duration: i8, i16, i32, u8, u16, u32, f32, f64);

impl Div<f32> for Duration {
    type Output = Self;

    #[inline]
    #[track_caller]
    fn div(self, rhs: f32) -> Self::Output {
        Self::seconds_f32(self.as_seconds_f32() / rhs)
    }
}

impl Div<f64> for Duration {
    type Output = Self;

    #[inline]
    #[track_caller]
    fn div(self, rhs: f64) -> Self::Output {
        Self::seconds_f64(self.as_seconds_f64() / rhs)
    }
}

impl_div_assign!(Duration: i8, i16, i32, u8, u16, u32, f32, f64);

impl Div for Duration {
    type Output = f64;

    #[inline]
    #[track_caller]
    fn div(self, rhs: Self) -> Self::Output {
        self.as_seconds_f64() / rhs.as_seconds_f64()
    }
}

impl Div<StdDuration> for Duration {
    type Output = f64;

    #[inline]
    #[track_caller]
    fn div(self, rhs: StdDuration) -> Self::Output {
        self.as_seconds_f64() / rhs.as_secs_f64()
    }
}

impl Div<Duration> for StdDuration {
    type Output = f64;

    #[inline]
    #[track_caller]
    fn div(self, rhs: Duration) -> Self::Output {
        self.as_secs_f64() / rhs.as_seconds_f64()
    }
}

impl PartialEq<StdDuration> for Duration {
    #[inline]
    fn eq(&self, rhs: &StdDuration) -> bool {
        Ok(*self) == Self::try_from(*rhs)
    }
}

impl PartialEq<Duration> for StdDuration {
    #[inline]
    fn eq(&self, rhs: &Duration) -> bool {
        rhs == self
    }
}

impl PartialOrd<StdDuration> for Duration {
    #[inline]
    fn partial_cmp(&self, rhs: &StdDuration) -> Option<Ordering> {
        if rhs.as_secs() > i64::MAX.cast_unsigned() {
            return Some(Ordering::Less);
        }

        Some(
            self.seconds
                .cmp(&rhs.as_secs().cast_signed())
                .then_with(|| {
                    self.nanoseconds
                        .get()
                        .cmp(&rhs.subsec_nanos().cast_signed())
                }),
        )
    }
}

impl PartialOrd<Duration> for StdDuration {
    #[inline]
    fn partial_cmp(&self, rhs: &Duration) -> Option<Ordering> {
        rhs.partial_cmp(self).map(Ordering::reverse)
    }
}

impl Sum for Duration {
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.reduce(|a, b| a + b).unwrap_or_default()
    }
}

impl<'a> Sum<&'a Self> for Duration {
    #[inline]
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.copied().sum()
    }
}

#[cfg(feature = "std")]
impl Add<Duration> for SystemTime {
    type Output = Self;

    #[inline]
    #[track_caller]
    fn add(self, duration: Duration) -> Self::Output {
        if duration.is_zero() {
            self
        } else if duration.is_positive() {
            self + duration.unsigned_abs()
        } else {
            debug_assert!(duration.is_negative());
            self - duration.unsigned_abs()
        }
    }
}

impl_add_assign!(SystemTime: #[cfg(feature = "std")] Duration);

#[cfg(feature = "std")]
impl Sub<Duration> for SystemTime {
    type Output = Self;

    #[inline]
    #[track_caller]
    fn sub(self, duration: Duration) -> Self::Output {
        if duration.is_zero() {
            self
        } else if duration.is_positive() {
            self - duration.unsigned_abs()
        } else {
            debug_assert!(duration.is_negative());
            self + duration.unsigned_abs()
        }
    }
}

impl_sub_assign!(SystemTime: #[cfg(feature = "std")] Duration);
