// We squash dead_code warnings because we define all of our methods for all
// primitive integer types, even though we don't use each method at least once
// on each of the integer types. It would really just be too annoying to do
// anything different. With that said, it is very likely that there is some
// actual dead code below that we're missing because we squash the warning.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use core::{
    cmp::Ordering,
    ops::{
        Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, RemAssign,
        Sub, SubAssign,
    },
};

use crate::{error::Error, util::t::Constant};

macro_rules! define_ranged {
    (
        $name:ident,
        $repr:ty,
        smaller { $($smaller_name:ident $smaller_repr:ty),* },
        bigger { $($bigger_name:ident $bigger_repr:ty),* }
    ) => {
        #[derive(Clone, Copy)]
        pub(crate) struct $name<const MIN: i128, const MAX: i128> {
            /// The actual value of the integer.
            ///
            /// Callers should not access this directly. There are some very
            /// rare cases where algorithms are too difficult to express on
            /// ranged integers, and it's useful to be able to reach inside and
            /// access the raw value directly. (For example, the conversions
            /// between Unix epoch day and Gregorian date.)
            pub(crate) val: $repr,
            /// The minimum possible value computed so far.
            ///
            /// This value is only present when `debug_assertions` are enabled.
            /// In that case, it is used to ensure the minimum possible value
            /// when the integer is actually observed (or converted) is still
            /// within the legal range.
            ///
            /// Callers should not access this directly. There are some very
            /// rare cases where algorithms are too difficult to express on
            /// ranged integers, and it's useful to be able to reach inside and
            /// access the raw value directly. (For example, the conversions
            /// between Unix epoch day and Gregorian date.)
            #[cfg(debug_assertions)]
            pub(crate) min: $repr,
            /// The maximum possible value computed so far.
            ///
            /// This value is only present when `debug_assertions` are enabled.
            /// In that case, it is used to ensure the maximum possible value
            /// when the integer is actually observed (or converted) is still
            /// within the legal range.
            ///
            /// Callers should not access this directly. There are some very
            /// rare cases where algorithms are too difficult to express on
            /// ranged integers, and it's useful to be able to reach inside and
            /// access the raw value directly. (For example, the conversions
            /// between Unix epoch day and Gregorian date.)
            #[cfg(debug_assertions)]
            pub(crate) max: $repr,
        }

        impl<const MIN: i128, const MAX: i128> $name<MIN, MAX> {
            /// These are the absolute min/max values for the integer type
            /// being used.
            const PRIMITIVE_MIN: i128 = <$repr>::MIN as i128;
            const PRIMITIVE_MAX: i128 = <$repr>::MAX as i128;

            /// When true, this range integer has bounds precisely equivalent
            /// to its underlying primitive representation.
            const IS_PRIMITIVE: bool = Self::MIN_REPR == <$repr>::MIN
                && Self::MAX_REPR == <$repr>::MAX;

            /// The min/max values as given by our type parameters.
            pub(crate) const MIN: i128 = MIN;
            pub(crate) const MAX: i128 = MAX;

            /// The number of distinct elements in this type's range.
            pub(crate) const LEN: i128 = {
                assert!(Self::PRIMITIVE_MIN < Self::PRIMITIVE_MAX);
                MAX - MIN + 1
            };

            /// The min/max values of this type, represented in their
            /// primitive form for easy comparisons with incoming values.
            pub(crate) const MIN_REPR: $repr = {
                assert!(
                    Self::PRIMITIVE_MIN <= MIN && MIN <= Self::PRIMITIVE_MAX
                );
                MIN as $repr
            };
            pub(crate) const MAX_REPR: $repr = {
                assert!(
                    Self::PRIMITIVE_MIN <= MAX && MAX <= Self::PRIMITIVE_MAX
                );
                MAX as $repr
            };

            /// The min/max values of this type as a ranged type.
            pub(crate) const MIN_SELF: Self =
                Self::new_unchecked(Self::MIN_REPR);
            pub(crate) const MAX_SELF: Self =
                Self::new_unchecked(Self::MAX_REPR);

            /// The min/max values of this type as a constant.
            pub(crate) const MIN_CONST: Constant =
                Constant(Self::MIN_REPR as i64);
            pub(crate) const MAX_CONST: Constant =
                Constant(Self::MAX_REPR as i64);

            #[inline]
            pub(crate) fn error(
                what: &'static str,
                given: $repr,
            ) -> Error {
                Error::range(what, given, Self::MIN_REPR, Self::MAX_REPR)
            }

            #[inline]
            pub(crate) fn new(val: impl TryInto<$repr>) -> Option<Self> {
                let val = val.try_into().ok()?;
                if !Self::contains(val) {
                    return None;
                }
                #[cfg(not(debug_assertions))]
                {
                    Some(Self { val })
                }
                #[cfg(debug_assertions)]
                {
                    Some(Self {
                        val,
                        min: Self::MIN_REPR,
                        max: Self::MAX_REPR,
                    })
                }
            }

            /// Like `new`, but monomorphic and works in a `const` context.
            #[inline]
            pub(crate) const fn new_const(val: $repr) -> Option<Self> {
                if !Self::contains(val) {
                    return None;
                }
                #[cfg(not(debug_assertions))]
                {
                    Some(Self { val })
                }
                #[cfg(debug_assertions)]
                {
                    Some(Self {
                        val,
                        min: Self::MIN_REPR,
                        max: Self::MAX_REPR,
                    })
                }
            }

            #[inline]
            pub(crate) fn try_new(
                what: &'static str,
                val: impl Into<i64>,
            ) -> Result<Self, Error> {
                let val = val.into();
                #[allow(irrefutable_let_patterns)]
                let Ok(val) = <$repr>::try_from(val) else {
                    return Err(Error::range(
                        what,
                        val,
                        Self::MIN_REPR,
                        Self::MAX_REPR,
                    ));
                };
                Self::new(val).ok_or_else(|| Self::error(what, val))
            }

            #[inline]
            pub(crate) fn try_new128(
                what: &'static str,
                val: impl Into<i128>,
            ) -> Result<Self, Error> {
                let val = val.into();
                #[allow(irrefutable_let_patterns)]
                let Ok(val) = <$repr>::try_from(val) else {
                    return Err(Error::range(
                        what,
                        val,
                        Self::MIN_REPR,
                        Self::MAX_REPR,
                    ));
                };
                Self::new(val).ok_or_else(|| Self::error(what, val))
            }

            #[inline]
            pub(crate) fn constrain(val: impl Into<$repr>) -> Self {
                let val = val.into().clamp(Self::MIN_REPR, Self::MAX_REPR);
                Self::new_unchecked(val)
            }

            #[inline]
            pub(crate) const fn new_unchecked(val: $repr) -> Self {
                #[cfg(not(debug_assertions))]
                {
                    Self { val }
                }
                #[cfg(debug_assertions)]
                {
                    assert!(Self::contains(val), "val is not in range");
                    Self { val, min: Self::MIN_REPR, max: Self::MAX_REPR }
                }
            }

            #[inline]
            pub(crate) const fn N<const VAL: $repr>() -> Self {
                #[cfg(not(debug_assertions))]
                {
                    Self { val: VAL }
                }
                #[cfg(debug_assertions)]
                {
                    Self { val: VAL, min: VAL, max: VAL }
                }
            }

            #[inline]
            pub(crate) const fn N128<const VAL: i128>() -> Self {
                #[cfg(not(debug_assertions))]
                {
                    Self { val: VAL as $repr }
                }
                #[cfg(debug_assertions)]
                {
                    if !(MIN <= VAL && VAL <= MAX) {
                        panic!("constant out of range");
                    }
                    let val = VAL as $repr;
                    Self { val, min: val, max: val }
                }
            }

            #[inline]
            pub(crate) const fn V<
                const VAL: $repr,
                const START: $repr,
                const END: $repr,
            >() -> Self {
                #[cfg(not(debug_assertions))]
                {
                    Self { val: VAL }
                }
                #[cfg(debug_assertions)]
                {
                    Self { val: VAL, min: START, max: END }
                }
            }

            #[inline]
            pub(crate) const fn contains(val: $repr) -> bool {
                Self::MIN_REPR <= val && val <= Self::MAX_REPR
            }

            #[inline]
            pub(crate) fn vary<
                const N: usize,
                const MIN2: i128,
                const MAX2: i128,
            >(
                numbers: [Self; N],
                with: impl Fn([Self; N]) -> $name<MIN2, MAX2>,
            ) -> $name<MIN2, MAX2> {
                let [result] =
                    Self::vary_many(numbers, |numbers| [with(numbers)]);
                result
            }

            #[inline]
            pub(crate) fn vary_many<
                const N: usize,
                const M: usize,
                const MIN2: i128,
                const MAX2: i128,
            >(
                numbers: [Self; N],
                with: impl Fn([Self; N]) -> [$name<MIN2, MAX2>; M],
            ) -> [$name<MIN2, MAX2>; M] {
                #[cfg(not(debug_assertions))]
                {
                    with(numbers)
                }
                #[cfg(debug_assertions)]
                {
                    let vals = with(numbers);
                    let mins = with(numbers.map(|n| Self {
                        val: n.min,
                        min: n.min,
                        max: n.max,
                    }));
                    let maxs = with(numbers.map(|n| Self {
                        val: n.max,
                        min: n.min,
                        max: n.max,
                    }));
                    let mut result = [$name::MIN_SELF; M];
                    let it = vals.into_iter().zip(mins).zip(maxs).enumerate();
                    for (i, ((val, min), max)) in it {
                        result[i] =
                            $name { val: val.val, min: min.val, max: max.val };
                    }
                    result
                }
            }

            #[inline]
            pub(crate) fn get(self) -> $repr {
                #[cfg(not(debug_assertions))]
                {
                    self.val
                }
                #[cfg(debug_assertions)]
                {
                    assert!(
                        Self::contains(self.val),
                        concat!(
                            stringify!($name),
                            " val {val:?} is not in range {MIN:?}..={MAX:?}"
                        ),
                        val = self.val,
                        MIN = MIN,
                        MAX = MAX,
                    );
                    assert!(
                        Self::contains(self.min),
                        concat!(
                            stringify!($name),
                            " min {min:?} is not in range {MIN:?}..={MAX:?}"
                        ),
                        min = self.min,
                        MIN = MIN,
                        MAX = MAX,
                    );
                    assert!(
                        Self::contains(self.max),
                        concat!(
                            stringify!($name),
                            " max {max:?} is not in range {MIN:?}..={MAX:?}"
                        ),
                        max = self.max,
                        MIN = MIN,
                        MAX = MAX,
                    );
                    self.val
                }
            }

            /// Returns the underlying value without checking whether it's
            /// in bounds or not.
            ///
            /// This should generally be avoided as it circumvents the
            /// protections of this type. It is sometimes useful in cases
            /// where the bounds are known not to matter. For example, in
            /// producing an error message for checked arithmetic. It's also
            /// good to use this in `Debug` impls for higher level types,
            /// otherwise printing the debug representation of a type will fail
            /// if a ranged integer is out of bounds. (And this is annoying.)
            #[inline]
            pub(crate) const fn get_unchecked(self) -> $repr {
                self.val
            }

            /// Turns this integer into an error.
            ///
            /// This is useful because it will use the integer's value even if
            /// it falls outside of the bounds of this type.
            ///
            /// Callers can also use this routine to set custom context
            /// dependent bounds. For example, when the day of the month is out
            /// of bounds. The maximum value can vary based on the month (and
            /// year).
            #[inline]
            pub(crate) fn to_error_with_bounds(
                self,
                what: &'static str,
                min: impl Into<i128>,
                max: impl Into<i128>,
            ) -> Error {
                Error::range(
                    what,
                    self.get_unchecked(),
                    min.into(),
                    max.into(),
                )
            }

            #[inline]
            pub(crate) fn abs(self) -> Self {
                #[cfg(not(debug_assertions))]
                {
                    $name { val: self.val.abs() }
                }
                #[cfg(debug_assertions)]
                {
                    let val = self.val.checked_abs().expect(concat!(
                        "absolute value of ",
                        stringify!($name),
                        " value overflowed",
                    ));
                    let min = self.min.checked_abs().expect(concat!(
                        "absolute value of ",
                        stringify!($name),
                        " minimum overflowed",
                    ));
                    let max = self.max.checked_abs().expect(concat!(
                        "absolute value of ",
                        stringify!($name),
                        " maximum overflowed",
                    ));
                    $name { val, min, max }
                }
            }

            #[inline]
            pub(crate) fn signum(self) -> $name<-1, 1> {
                #[cfg(not(debug_assertions))]
                {
                    $name { val: self.val.signum() }
                }
                #[cfg(debug_assertions)]
                {
                    let val = self.val.signum();
                    let min = self.min.signum();
                    let max = self.max.signum();
                    $name { val, min, max }
                }
            }

            #[inline]
            pub(crate) fn min(self, other: impl RInto<Self>) -> Self {
                let other = other.rinto();
                #[cfg(not(debug_assertions))]
                {
                    Self { val: self.val.min(other.val) }
                }
                #[cfg(debug_assertions)]
                {
                    let val = self.val.min(other.val);
                    let min = self.val.min(other.min);
                    let max = self.max.min(other.max);
                    Self { val, min, max }
                }
            }

            #[inline]
            pub(crate) fn max(self, other: impl RInto<Self>) -> Self {
                let other = other.rinto();
                #[cfg(not(debug_assertions))]
                {
                    Self { val: self.val.max(other.val) }
                }
                #[cfg(debug_assertions)]
                {
                    let val = self.val.max(other.val);
                    let min = self.val.max(other.min);
                    let max = self.max.max(other.max);
                    Self { val, min, max }
                }
            }

            #[inline]
            pub(crate) fn clamp(
                self,
                min: impl RInto<Self>,
                max: impl RInto<Self>,
            ) -> Self {
                self.min(max).max(min)
            }

            #[inline]
            pub(crate) fn div_ceil(self, rhs: impl RInto<Self>) -> Self {
                let rhs = rhs.rinto();
                #[cfg(not(debug_assertions))]
                {
                    let val = self.val.wrapping_div(rhs.val);
                    Self { val }
                }
                #[cfg(debug_assertions)]
                {
                    let val = self.val.checked_div(rhs.val).expect(concat!(
                        "dividing(ceil) ",
                        stringify!($name),
                        " values overflowed"
                    ));
                    let min = self.min.checked_div(rhs.min).expect(concat!(
                        "dividing(ceil) ",
                        stringify!($name),
                        " minimums overflowed"
                    ));
                    let max = self.max.checked_div(rhs.max).expect(concat!(
                        "dividing(ceil) ",
                        stringify!($name),
                        " maximums overflowed"
                    ));
                    Self { val, min, max }
                }
            }

            #[inline]
            pub(crate) fn div_floor(self, rhs: impl RInto<Self>) -> Self {
                let rhs = rhs.rinto();
                #[cfg(not(debug_assertions))]
                {
                    let val = self.val.wrapping_div_euclid(rhs.val);
                    Self { val }
                }
                #[cfg(debug_assertions)]
                {
                    let val =
                        self.val.checked_div_euclid(rhs.val).expect(concat!(
                            "dividing(ceil) ",
                            stringify!($name),
                            " values overflowed"
                        ));
                    let min =
                        self.min.checked_div_euclid(rhs.min).expect(concat!(
                            "dividing(ceil) ",
                            stringify!($name),
                            " minimums overflowed"
                        ));
                    let max =
                        self.max.checked_div_euclid(rhs.max).expect(concat!(
                            "dividing(ceil) ",
                            stringify!($name),
                            " maximums overflowed"
                        ));
                    Self { val, min, max }
                }
            }

            #[inline]
            pub(crate) fn rem_ceil(self, rhs: impl RInto<Self>) -> Self {
                let rhs = rhs.rinto();
                #[cfg(not(debug_assertions))]
                {
                    let val = self.val.wrapping_rem(rhs.val);
                    Self { val }
                }
                #[cfg(debug_assertions)]
                {
                    let val = self.val.checked_rem(rhs.val).expect(concat!(
                        "modulo(ceil) ",
                        stringify!($name),
                        " values overflowed"
                    ));
                    let min = self.min.checked_rem(rhs.min).expect(concat!(
                        "modulo(ceil) ",
                        stringify!($name),
                        " minimums overflowed"
                    ));
                    let max = self.max.checked_rem(rhs.max).expect(concat!(
                        "modulo(ceil) ",
                        stringify!($name),
                        " maximums overflowed"
                    ));
                    Self { val, min, max }
                }
            }

            #[inline]
            pub(crate) fn rem_floor(self, rhs: impl RInto<Self>) -> Self {
                let rhs = rhs.rinto();
                #[cfg(not(debug_assertions))]
                {
                    let val = self.val.wrapping_rem_euclid(rhs.val);
                    Self { val }
                }
                #[cfg(debug_assertions)]
                {
                    let val =
                        self.val.checked_rem_euclid(rhs.val).expect(concat!(
                            "modulo(ceil) ",
                            stringify!($name),
                            " values overflowed"
                        ));
                    let min =
                        self.min.checked_rem_euclid(rhs.min).expect(concat!(
                            "modulo(ceil) ",
                            stringify!($name),
                            " minimums overflowed"
                        ));
                    let max =
                        self.max.checked_rem_euclid(rhs.max).expect(concat!(
                            "modulo(ceil) ",
                            stringify!($name),
                            " maximums overflowed"
                        ));
                    Self { val, min, max }
                }
            }

            #[inline]
            pub(crate) fn try_checked_add(
                self,
                what: &'static str,
                rhs: impl RInto<Self>,
            ) -> Result<Self, Error> {
                let rhs = rhs.rinto();
                self.checked_add(rhs)
                    .ok_or_else(|| Self::error(what, rhs.get_unchecked()))
            }

            #[inline]
            pub(crate) fn try_checked_sub(
                self,
                what: &'static str,
                rhs: impl RInto<Self>,
            ) -> Result<Self, Error> {
                let rhs = rhs.rinto();
                self.checked_sub(rhs)
                    .ok_or_else(|| Self::error(what, rhs.get_unchecked()))
            }

            #[inline]
            pub(crate) fn try_checked_mul(
                self,
                what: &'static str,
                rhs: impl RInto<Self>,
            ) -> Result<Self, Error> {
                let rhs = rhs.rinto();
                self.checked_mul(rhs)
                    .ok_or_else(|| Self::error(what, rhs.get_unchecked()))
            }

            #[inline]
            pub(crate) fn checked_add(
                self,
                rhs: impl RInto<Self>,
            ) -> Option<Self> {
                let rhs = rhs.rinto();
                #[cfg(not(debug_assertions))]
                {
                    let val = self.val.checked_add(rhs.val)?;
                    Self::new(val)
                }
                #[cfg(debug_assertions)]
                {
                    let val = self.val.checked_add(rhs.val)?;
                    if !Self::contains(val) {
                        return None;
                    }
                    // We specifically clamp min/max at the boundaries because
                    // the checked arithmetic above implies we will catch
                    // overflow. If we didn't do this, min/max arithmetic
                    // could overflow even when the checked arithmetic above
                    // did not. That is, under normal and expected operation,
                    // we expect the min/max to eventually overflow even when
                    // val does not.
                    let min = self
                        .min
                        .saturating_add(rhs.min)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    let max = self
                        .max
                        .saturating_add(rhs.max)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    Some(Self { val, min, max })
                }
            }

            #[inline]
            pub(crate) fn checked_sub(
                self,
                rhs: impl RInto<Self>,
            ) -> Option<Self> {
                let rhs = rhs.rinto();
                #[cfg(not(debug_assertions))]
                {
                    let val = self.val.checked_sub(rhs.val)?;
                    Self::new(val)
                }
                #[cfg(debug_assertions)]
                {
                    let val = self.val.checked_sub(rhs.val)?;
                    if !Self::contains(val) {
                        return None;
                    }
                    // See comment in `checked_add`.
                    let min = self
                        .min
                        .saturating_sub(rhs.min)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    let max = self
                        .max
                        .saturating_sub(rhs.max)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    Some(Self { val, min, max })
                }
            }

            #[inline]
            pub(crate) fn checked_mul(
                self,
                rhs: impl RInto<Self>,
            ) -> Option<Self> {
                let rhs = rhs.rinto();
                #[cfg(not(debug_assertions))]
                {
                    let val = self.val.checked_mul(rhs.val)?;
                    Self::new(val)
                }
                #[cfg(debug_assertions)]
                {
                    let val = self.val.checked_mul(rhs.val)?;
                    if !Self::contains(val) {
                        return None;
                    }
                    // See comment in `checked_add`.
                    let min = self
                        .min
                        .saturating_mul(rhs.min)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    let max = self
                        .max
                        .saturating_mul(rhs.max)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    Some(Self { val, min, max })
                }
            }

            #[inline]
            pub(crate) fn wrapping_add(self, rhs: impl RInto<Self>) -> Self {
                let rhs = rhs.rinto();
                #[cfg(not(debug_assertions))]
                {
                    // When the min/max bounds match our primitive type, then
                    // standard wrapping arithmetic will work fine. This is
                    // likely a benefit for perf, but it's also required
                    // for correctness since we don't support anything else
                    // at the moment.
                    if Self::IS_PRIMITIVE {
                        Self { val: self.val.wrapping_add(rhs.val) }
                    } else {
                        unimplemented!(
                            "wrapping arithmetic for non-primitive \
                             ranged integers is not implemented yet",
                        );
                    }
                }
                #[cfg(debug_assertions)]
                {
                    if Self::IS_PRIMITIVE {
                        let val = self.val.wrapping_add(rhs.val);
                        let min = self.min.wrapping_add(rhs.min);
                        let max = self.max.wrapping_add(rhs.max);
                        Self { val, min, max }
                    } else {
                        unimplemented!(
                            "wrapping arithmetic for non-primitive \
                             ranged integers is not implemented yet",
                        );
                    }
                }
            }

            #[inline]
            pub(crate) fn wrapping_sub(self, rhs: impl RInto<Self>) -> Self {
                let rhs = rhs.rinto();
                #[cfg(not(debug_assertions))]
                {
                    // When the min/max bounds match our primitive type, then
                    // standard wrapping arithmetic will work fine. This is
                    // likely a benefit for perf, but it's also required
                    // for correctness since we don't support anything else
                    // at the moment.
                    if Self::IS_PRIMITIVE {
                        Self { val: self.val.wrapping_sub(rhs.val) }
                    } else {
                        unimplemented!(
                            "wrapping arithmetic for non-primitive \
                             ranged integers is not implemented yet",
                        );
                    }
                }
                #[cfg(debug_assertions)]
                {
                    if Self::IS_PRIMITIVE {
                        let val = self.val.wrapping_sub(rhs.val);
                        let min = self.min.wrapping_sub(rhs.min);
                        let max = self.max.wrapping_sub(rhs.max);
                        Self { val, min, max }
                    } else {
                        unimplemented!(
                            "wrapping arithmetic for non-primitive \
                             ranged integers is not implemented yet",
                        );
                    }
                }
            }

            #[inline]
            pub(crate) fn wrapping_mul(self, rhs: impl RInto<Self>) -> Self {
                let rhs = rhs.rinto();
                #[cfg(not(debug_assertions))]
                {
                    // When the min/max bounds match our primitive type, then
                    // standard wrapping arithmetic will work fine. This is
                    // likely a benefit for perf, but it's also required
                    // for correctness since we don't support anything else
                    // at the moment.
                    if Self::IS_PRIMITIVE {
                        Self { val: self.val.wrapping_mul(rhs.val) }
                    } else {
                        unimplemented!(
                            "wrapping arithmetic for non-primitive \
                             ranged integers is not implemented yet",
                        );
                    }
                }
                #[cfg(debug_assertions)]
                {
                    if Self::IS_PRIMITIVE {
                        let val = self.val.wrapping_mul(rhs.val);
                        let min = self.min.wrapping_mul(rhs.min);
                        let max = self.max.wrapping_mul(rhs.max);
                        Self { val, min, max }
                    } else {
                        unimplemented!(
                            "wrapping arithmetic for non-primitive \
                             ranged integers is not implemented yet",
                        );
                    }
                }
            }

            #[inline]
            pub(crate) fn saturating_add(self, rhs: impl RInto<Self>) -> Self {
                let rhs = rhs.rinto();
                #[cfg(not(debug_assertions))]
                {
                    let val = self
                        .val
                        .saturating_add(rhs.val)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    Self { val }
                }
                #[cfg(debug_assertions)]
                {
                    let val = self
                        .val
                        .saturating_add(rhs.val)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    let min = self
                        .min
                        .saturating_add(rhs.val)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    let max = self
                        .max
                        .saturating_add(rhs.val)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    Self { val, min, max }
                }
            }

            #[inline]
            pub(crate) fn saturating_sub(self, rhs: impl RInto<Self>) -> Self {
                let rhs = rhs.rinto();
                #[cfg(not(debug_assertions))]
                {
                    let val = self
                        .val
                        .saturating_sub(rhs.val)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    Self { val }
                }
                #[cfg(debug_assertions)]
                {
                    let val = self
                        .val
                        .saturating_sub(rhs.val)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    let min = self
                        .min
                        .saturating_sub(rhs.val)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    let max = self
                        .max
                        .saturating_sub(rhs.val)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    Self { val, min, max }
                }
            }

            #[inline]
            pub(crate) fn saturating_mul(self, rhs: impl RInto<Self>) -> Self {
                let rhs = rhs.rinto();
                #[cfg(not(debug_assertions))]
                {
                    let val = self
                        .val
                        .saturating_mul(rhs.val)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    Self { val }
                }
                #[cfg(debug_assertions)]
                {
                    let val = self
                        .val
                        .saturating_mul(rhs.val)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    let min = self
                        .min
                        .saturating_mul(rhs.val)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    let max = self
                        .max
                        .saturating_mul(rhs.val)
                        .clamp(Self::MIN_REPR, Self::MAX_REPR);
                    Self { val, min, max }
                }
            }

            pub(crate) fn debug(self) -> RangedDebug<MIN, MAX> {
                RangedDebug { rint: self.rinto() }
            }
        }

        // We hand-write the `Hash` impl to avoid the min/max values
        // influencing the hash. Only the actual value should be hashed.
        //
        // See: https://github.com/BurntSushi/jiff/issues/330
        impl<const MIN: i128, const MAX: i128> core::hash::Hash for $name<MIN, MAX> {
            fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
                self.val.hash(state);
            }
        }

        impl<
            const MIN1: i128,
            const MAX1: i128,
            const MIN2: i128,
            const MAX2: i128,
        > RFrom<$name<MIN1, MAX1>> for $name<MIN2, MAX2>
        {
            #[inline]
            fn rfrom(r: $name<MIN1, MAX1>) -> Self {
                #[cfg(not(debug_assertions))]
                {
                    $name { val: r.val }
                }
                #[cfg(debug_assertions)]
                {
                    $name { val: r.val, min: r.min, max: r.max }
                }
            }
        }

        impl<const MIN: i128, const MAX: i128> RFrom<$name<MIN, MAX>>
            for $repr
        {
            #[inline]
            fn rfrom(r: $name<MIN, MAX>) -> $repr {
                r.get()
            }
        }

        impl<const MIN: i128, const MAX: i128> From<$name<MIN, MAX>>
            for $repr
        {
            #[inline]
            fn from(r: $name<MIN, MAX>) -> $repr {
                r.get()
            }
        }

        impl<const MIN: i128, const MAX: i128> RFrom<Constant>
            for $name<MIN, MAX>
        {
            #[inline]
            fn rfrom(c: Constant) -> Self {
                #[cfg(not(debug_assertions))]
                {
                    Self { val: c.value() as $repr }
                }
                #[cfg(debug_assertions)]
                {
                    // We specifically allow constants that don't fit in the
                    // bounds of the integer type, but we don't allow constans
                    // that can't fit in the actual integer representation.
                    // This makes doing things like `number % one-plus-max`
                    // much more convenient.
                    #[allow(irrefutable_let_patterns)]
                    let Ok(val) = <$repr>::try_from(c.value()) else {
                        panic!(
                            "{c:?} does not fit in {name:?}",
                            name = stringify!($name),
                        )
                    };
                    Self { val, min: val, max: val }
                }
            }
        }

        impl<
            const MIN1: i128,
            const MAX1: i128,
            const MIN2: i128,
            const MAX2: i128,
        > TryRFrom<$name<MIN1, MAX1>> for $name<MIN2, MAX2>
        {
            #[inline]
            fn try_rfrom(
                what: &'static str, r: $name<MIN1, MAX1>,
            ) -> Result<Self, Error> {
                #[cfg(not(debug_assertions))]
                {
                    if !Self::contains(r.val) {
                        return Err(Self::error(what, r.val));
                    }
                    Ok($name { val: r.val })
                }
                #[cfg(debug_assertions)]
                {
                    if !Self::contains(r.val) {
                        return Err(Self::error(what, r.val));
                    }
                    Ok($name {
                        val: r.val,
                        min: r.min.clamp(Self::MIN_REPR, Self::MAX_REPR),
                        max: r.max.clamp(Self::MIN_REPR, Self::MAX_REPR),
                    })
                }
            }
        }

        $(
            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > RFrom<$smaller_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn rfrom(r: $smaller_name<MIN1, MAX1>) -> Self {
                    #[cfg(not(debug_assertions))]
                    {
                        Self { val: <$repr>::from(r.val) }
                    }
                    #[cfg(debug_assertions)]
                    {
                        Self {
                            val: <$repr>::from(r.val),
                            min: <$repr>::from(r.min),
                            max: <$repr>::from(r.max),
                        }
                    }
                }
            }

            impl<
                const MIN: i128,
                const MAX: i128,
            > RFrom<$name<MIN, MAX>> for $smaller_repr
            {
                #[inline]
                fn rfrom(r: $name<MIN, MAX>) -> $smaller_repr {
                    #[cfg(not(debug_assertions))]
                    {
                        r.val as $smaller_repr
                    }
                    #[cfg(debug_assertions)]
                    {
                        let Ok(val) = <$smaller_repr>::try_from(r.val) else {
                            panic!(
                                "{from} value {val} does not fit in {to}",
                                from = stringify!($name),
                                val = r.val,
                                to = stringify!($smaller_name),
                            );
                        };
                        if <$smaller_repr>::try_from(r.min).is_err() {
                            panic!(
                                "{from} min value {val} does not fit in {to}",
                                from = stringify!($name),
                                val = r.min,
                                to = stringify!($smaller_name),
                            );
                        }
                        if <$smaller_repr>::try_from(r.max).is_err() {
                            panic!(
                                "{from} max value {val} does not fit in {to}",
                                from = stringify!($name),
                                val = r.max,
                                to = stringify!($smaller_name),
                            );
                        }
                        val
                    }
                }
            }

            impl<
                const MIN: i128,
                const MAX: i128,
            > From<$name<MIN, MAX>> for $smaller_repr
            {
                #[inline]
                fn from(r: $name<MIN, MAX>) -> $smaller_repr {
                    <$smaller_repr>::rfrom(r)
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > TryRFrom<$smaller_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn try_rfrom(
                    what: &'static str, r: $smaller_name<MIN1, MAX1>,
                ) -> Result<Self, Error> {
                    #[cfg(not(debug_assertions))]
                    {
                        let val = <$repr>::from(r.val);
                        if !Self::contains(val) {
                            return Err(Self::error(what, val));
                        }
                        Ok(Self { val })
                    }
                    #[cfg(debug_assertions)]
                    {
                        let val = <$repr>::from(r.val);
                        if !Self::contains(val) {
                            return Err(Self::error(what, val));
                        }
                        Ok(Self {
                            val: val,
                            min: <$repr>::from(r.min)
                                .clamp(Self::MIN_REPR, Self::MAX_REPR),
                            max: <$repr>::from(r.max)
                                .clamp(Self::MIN_REPR, Self::MAX_REPR),
                        })
                    }
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > PartialEq<$smaller_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn eq(&self, other: &$smaller_name<MIN1, MAX1>) -> bool {
                    self.eq(&Self::rfrom(*other))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > PartialOrd<$smaller_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn partial_cmp(
                    &self,
                    other: &$smaller_name<MIN1, MAX1>,
                ) -> Option<Ordering> {
                    self.partial_cmp(&Self::rfrom(*other))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > Add<$smaller_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                type Output = Self;

                #[inline]
                fn add(self, rhs: $smaller_name<MIN1, MAX1>) -> Self::Output {
                    self.add(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > AddAssign<$smaller_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn add_assign(&mut self, rhs: $smaller_name<MIN1, MAX1>) {
                    self.add_assign(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > Sub<$smaller_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                type Output = Self;

                #[inline]
                fn sub(self, rhs: $smaller_name<MIN1, MAX1>) -> Self::Output {
                    self.sub(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > SubAssign<$smaller_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn sub_assign(&mut self, rhs: $smaller_name<MIN1, MAX1>) {
                    self.sub_assign(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > Mul<$smaller_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                type Output = Self;

                #[inline]
                fn mul(self, rhs: $smaller_name<MIN1, MAX1>) -> Self::Output {
                    self.mul(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > MulAssign<$smaller_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn mul_assign(&mut self, rhs: $smaller_name<MIN1, MAX1>) {
                    self.mul_assign(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > Div<$smaller_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                type Output = Self;

                #[inline]
                fn div(self, rhs: $smaller_name<MIN1, MAX1>) -> Self::Output {
                    self.div(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > DivAssign<$smaller_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn div_assign(&mut self, rhs: $smaller_name<MIN1, MAX1>) {
                    self.div_assign(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > Rem<$smaller_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                type Output = Self;

                #[inline]
                fn rem(self, rhs: $smaller_name<MIN1, MAX1>) -> Self::Output {
                    self.rem(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > RemAssign<$smaller_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn rem_assign(&mut self, rhs: $smaller_name<MIN1, MAX1>) {
                    self.rem_assign(Self::rfrom(rhs))
                }
            }
        )*

        $(
            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > RFrom<$bigger_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn rfrom(r: $bigger_name<MIN1, MAX1>) -> Self {
                    #[cfg(not(debug_assertions))]
                    {
                        Self { val: r.val as $repr }
                    }
                    #[cfg(debug_assertions)]
                    {
                        let Ok(val) = <$repr>::try_from(r.val) else {
                            panic!(
                                "{from} value {val} does not fit in {to}",
                                from = stringify!($bigger_name),
                                val = r.val,
                                to = stringify!($name),
                            );
                        };
                        let Ok(min) = <$repr>::try_from(r.min) else {
                            panic!(
                                "{from} min value {val} does not fit in {to}",
                                from = stringify!($bigger_name),
                                val = r.min,
                                to = stringify!($name),
                            );
                        };
                        let Ok(max) = <$repr>::try_from(r.max) else {
                            panic!(
                                "{from} max value {val} does not fit in {to}",
                                from = stringify!($bigger_name),
                                val = r.max,
                                to = stringify!($name),
                            );
                        };
                        Self { val, min, max }
                    }
                }
            }

            impl<
                const MIN: i128,
                const MAX: i128,
            > RFrom<$name<MIN, MAX>> for $bigger_repr
            {
                #[inline]
                fn rfrom(r: $name<MIN, MAX>) -> $bigger_repr {
                    <$bigger_repr>::from(r.get())
                }
            }

            impl<
                const MIN: i128,
                const MAX: i128,
            > From<$name<MIN, MAX>> for $bigger_repr
            {
                #[inline]
                fn from(r: $name<MIN, MAX>) -> $bigger_repr {
                    <$bigger_repr>::rfrom(r)
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > TryRFrom<$bigger_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn try_rfrom(
                    what: &'static str, r: $bigger_name<MIN1, MAX1>,
                ) -> Result<Self, Error> {
                    #[cfg(not(debug_assertions))]
                    {
                        let val = <$repr>::try_from(r.val).map_err(|_| {
                            Error::range(what, r.val, MIN2, MAX2)
                        })?;
                        if !Self::contains(val) {
                            return Err(Self::error(what, val));
                        }
                        Ok(Self { val })
                    }
                    #[cfg(debug_assertions)]
                    {
                        let val = <$repr>::try_from(r.val).map_err(|_| {
                            Error::range(what, r.val, MIN2, MAX2)
                        })?;
                        if !Self::contains(val) {
                            return Err(Self::error(what, val));
                        }
                        let min = <$repr>::try_from(r.min).unwrap_or_else(|_| {
                            if (r.min as i128) < MIN2 {
                                Self::MIN_REPR
                            } else {
                                assert!(r.min as i128 > MAX2);
                                Self::MAX_REPR
                            }
                        });
                        let max = <$repr>::try_from(r.max).unwrap_or_else(|_| {
                            if (r.max as i128) < MIN2 {
                                Self::MIN_REPR
                            } else {
                                assert!(r.max as i128 > MAX2);
                                Self::MAX_REPR
                            }
                        });
                        Ok(Self {
                            val,
                            min: min.clamp(Self::MIN_REPR, Self::MAX_REPR),
                            max: max.clamp(Self::MIN_REPR, Self::MAX_REPR),
                        })
                    }
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > PartialEq<$bigger_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn eq(&self, other: &$bigger_name<MIN1, MAX1>) -> bool {
                    <$bigger_name<MIN1, MAX1>>::rfrom(*self).eq(other)
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > PartialOrd<$bigger_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn partial_cmp(
                    &self,
                    other: &$bigger_name<MIN1, MAX1>,
                ) -> Option<Ordering> {
                    <$bigger_name<MIN1, MAX1>>::rfrom(*self).partial_cmp(other)
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > Add<$bigger_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                type Output = Self;

                #[inline]
                fn add(self, rhs: $bigger_name<MIN1, MAX1>) -> Self::Output {
                    self.add(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > AddAssign<$bigger_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn add_assign(&mut self, rhs: $bigger_name<MIN1, MAX1>) {
                    self.add_assign(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > Sub<$bigger_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                type Output = Self;

                #[inline]
                fn sub(self, rhs: $bigger_name<MIN1, MAX1>) -> Self::Output {
                    self.sub(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > SubAssign<$bigger_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn sub_assign(&mut self, rhs: $bigger_name<MIN1, MAX1>) {
                    self.sub_assign(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > Mul<$bigger_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                type Output = Self;

                #[inline]
                fn mul(self, rhs: $bigger_name<MIN1, MAX1>) -> Self::Output {
                    self.mul(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > MulAssign<$bigger_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn mul_assign(&mut self, rhs: $bigger_name<MIN1, MAX1>) {
                    self.mul_assign(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > Div<$bigger_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                type Output = Self;

                #[inline]
                fn div(self, rhs: $bigger_name<MIN1, MAX1>) -> Self::Output {
                    self.div(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > DivAssign<$bigger_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn div_assign(&mut self, rhs: $bigger_name<MIN1, MAX1>) {
                    self.div_assign(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > Rem<$bigger_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                type Output = Self;

                #[inline]
                fn rem(self, rhs: $bigger_name<MIN1, MAX1>) -> Self::Output {
                    self.rem(Self::rfrom(rhs))
                }
            }

            impl<
                const MIN1: i128,
                const MAX1: i128,
                const MIN2: i128,
                const MAX2: i128,
            > RemAssign<$bigger_name<MIN1, MAX1>> for $name<MIN2, MAX2>
            {
                #[inline]
                fn rem_assign(&mut self, rhs: $bigger_name<MIN1, MAX1>) {
                    self.rem_assign(Self::rfrom(rhs))
                }
            }
        )*

        impl<const MIN: i128, const MAX: i128> Neg for $name<MIN, MAX> {
            type Output = Self;

            #[inline]
            fn neg(self) -> Self {
                #[cfg(not(debug_assertions))]
                {
                    let val = self.val.wrapping_neg();
                    Self { val }
                }
                #[cfg(debug_assertions)]
                {
                    let val = self.val.checked_neg().expect(concat!(
                        "negating ",
                        stringify!($name),
                        " values overflowed"
                    ));
                    let min = self.min.checked_neg().expect(concat!(
                        "negating ",
                        stringify!($name),
                        " minimums overflowed"
                    ));
                    let max = self.max.checked_neg().expect(concat!(
                        "negating ",
                        stringify!($name),
                        " maximums overflowed"
                    ));
                    Self { val, min, max }
                }
            }
        }

        impl<
            const MIN1: i128,
            const MAX1: i128,
            const MIN2: i128,
            const MAX2: i128,
        > Add<$name<MIN2, MAX2>> for $name<MIN1, MAX1> {
            type Output = Self;

            #[inline]
            fn add(self, rhs: $name<MIN2, MAX2>) -> Self::Output {
                #[cfg(not(debug_assertions))]
                {
                    let val = self.val.wrapping_add(rhs.val);
                    Self { val }
                }
                #[cfg(debug_assertions)]
                {
                    let val = self.val.checked_add(rhs.val).expect(concat!(
                        "adding ",
                        stringify!($name),
                        " values overflowed"
                    ));
                    let min = self.min.checked_add(rhs.min).expect(concat!(
                        "adding ",
                        stringify!($name),
                        " minimums overflowed"
                    ));
                    let max = self.max.checked_add(rhs.max).expect(concat!(
                        "adding ",
                        stringify!($name),
                        " maximums overflowed"
                    ));
                    Self { val, min, max }
                }
            }
        }

        impl<
            const MIN1: i128,
            const MAX1: i128,
            const MIN2: i128,
            const MAX2: i128,
        > AddAssign<$name<MIN2, MAX2>> for $name<MIN1, MAX1> {
            #[inline]
            fn add_assign(&mut self, rhs: $name<MIN2, MAX2>) {
                *self = self.add(rhs);
            }
        }

        impl<
            const MIN1: i128,
            const MAX1: i128,
            const MIN2: i128,
            const MAX2: i128,
        > Sub<$name<MIN2, MAX2>> for $name<MIN1, MAX1> {
            type Output = Self;

            #[inline]
            fn sub(self, rhs: $name<MIN2, MAX2>) -> Self::Output {
                #[cfg(not(debug_assertions))]
                {
                    let val = self.val.wrapping_sub(rhs.val);
                    Self { val }
                }
                #[cfg(debug_assertions)]
                {
                    let val = self.val.checked_sub(rhs.val).expect(concat!(
                        "subtracting ",
                        stringify!($name),
                        " values overflowed"
                    ));
                    let min = self.min.checked_sub(rhs.min).expect(concat!(
                        "subtracting ",
                        stringify!($name),
                        " minimums overflowed"
                    ));
                    let max = self.max.checked_sub(rhs.max).expect(concat!(
                        "subtracting ",
                        stringify!($name),
                        " maximums overflowed"
                    ));
                    Self { val, min, max }
                }
            }
        }

        impl<
            const MIN1: i128,
            const MAX1: i128,
            const MIN2: i128,
            const MAX2: i128,
        > SubAssign<$name<MIN2, MAX2>> for $name<MIN1, MAX1> {
            #[inline]
            fn sub_assign(&mut self, rhs: $name<MIN2, MAX2>) {
                *self = self.sub(rhs);
            }
        }

        impl<
            const MIN1: i128,
            const MAX1: i128,
            const MIN2: i128,
            const MAX2: i128,
        > Mul<$name<MIN2, MAX2>> for $name<MIN1, MAX1> {
            type Output = Self;

            #[inline]
            fn mul(self, rhs: $name<MIN2, MAX2>) -> Self::Output {
                #[cfg(not(debug_assertions))]
                {
                    let val = self.val.wrapping_mul(rhs.val);
                    Self { val }
                }
                #[cfg(debug_assertions)]
                {
                    let val = self.val.checked_mul(rhs.val).expect(concat!(
                        "multiplying ",
                        stringify!($name),
                        " values overflowed"
                    ));
                    let min = self.min.checked_mul(rhs.min).expect(concat!(
                        "multiplying ",
                        stringify!($name),
                        " minimums overflowed"
                    ));
                    let max = self.max.checked_mul(rhs.max).expect(concat!(
                        "multiplying ",
                        stringify!($name),
                        " maximums overflowed"
                    ));
                    Self { val, min, max }
                }
            }
        }

        impl<
            const MIN1: i128,
            const MAX1: i128,
            const MIN2: i128,
            const MAX2: i128,
        > MulAssign<$name<MIN2, MAX2>> for $name<MIN1, MAX1> {
            #[inline]
            fn mul_assign(&mut self, rhs: $name<MIN2, MAX2>) {
                *self = self.mul(rhs);
            }
        }

        impl<
            const MIN1: i128,
            const MAX1: i128,
            const MIN2: i128,
            const MAX2: i128,
        > Div<$name<MIN2, MAX2>> for $name<MIN1, MAX1> {
            type Output = Self;

            #[inline]
            fn div(self, rhs: $name<MIN2, MAX2>) -> Self::Output {
                #[cfg(not(debug_assertions))]
                {
                    let val = self.val.wrapping_div_euclid(rhs.val);
                    Self { val }
                }
                #[cfg(debug_assertions)]
                {
                    let val =
                        self.val.checked_div_euclid(rhs.val).expect(concat!(
                            "dividing ",
                            stringify!($name),
                            " values overflowed"
                        ));
                    let min =
                        self.min.checked_div_euclid(rhs.min).expect(concat!(
                            "dividing ",
                            stringify!($name),
                            " minimums overflowed"
                        ));
                    let max =
                        self.max.checked_div_euclid(rhs.max).expect(concat!(
                            "dividing ",
                            stringify!($name),
                            " maximums overflowed"
                        ));
                    Self { val, min, max }
                }
            }
        }

        impl<
            const MIN1: i128,
            const MAX1: i128,
            const MIN2: i128,
            const MAX2: i128,
        > DivAssign<$name<MIN2, MAX2>> for $name<MIN1, MAX1> {
            #[inline]
            fn div_assign(&mut self, rhs: $name<MIN2, MAX2>) {
                *self = self.div(rhs);
            }
        }

        impl<
            const MIN1: i128,
            const MAX1: i128,
            const MIN2: i128,
            const MAX2: i128,
        > Rem<$name<MIN2, MAX2>> for $name<MIN1, MAX1> {
            type Output = Self;

            #[inline]
            fn rem(self, rhs: $name<MIN2, MAX2>) -> Self::Output {
                #[cfg(not(debug_assertions))]
                {
                    let val = self.val.wrapping_rem_euclid(rhs.val);
                    Self { val }
                }
                #[cfg(debug_assertions)]
                {
                    let val =
                        self.val.checked_rem_euclid(rhs.val).expect(concat!(
                            "modulo ",
                            stringify!($name),
                            " values overflowed"
                        ));
                    let min =
                        self.min.checked_rem_euclid(rhs.min).expect(concat!(
                            "modulo ",
                            stringify!($name),
                            " minimums overflowed"
                        ));
                    let max =
                        self.max.checked_rem_euclid(rhs.max).expect(concat!(
                            "modulo ",
                            stringify!($name),
                            " maximums overflowed"
                        ));
                    Self { val, min, max }
                }
            }
        }

        impl<
            const MIN1: i128,
            const MAX1: i128,
            const MIN2: i128,
            const MAX2: i128,
        > RemAssign<$name<MIN2, MAX2>> for $name<MIN1, MAX1> {
            #[inline]
            fn rem_assign(&mut self, rhs: $name<MIN2, MAX2>) {
                *self = self.rem(rhs);
            }
        }

        impl<const MIN: i128, const MAX: i128> Add<$name<MIN, MAX>>
            for Constant
        {
            type Output = $name<MIN, MAX>;

            #[inline]
            fn add(self, rhs: $name<MIN, MAX>) -> Self::Output {
                $name::rfrom(self).add(rhs)
            }
        }

        impl<const MIN: i128, const MAX: i128> Add<Constant> for $name<MIN, MAX> {
            type Output = $name<MIN, MAX>;

            #[inline]
            fn add(self, rhs: Constant) -> Self::Output {
                self.add(Self::rfrom(rhs))
            }
        }

        impl<const MIN: i128, const MAX: i128> AddAssign<Constant> for $name<MIN, MAX> {
            #[inline]
            fn add_assign(&mut self, rhs: Constant) {
                self.add_assign(Self::rfrom(rhs))
            }
        }

        impl<const MIN: i128, const MAX: i128> Sub<$name<MIN, MAX>> for Constant {
            type Output = $name<MIN, MAX>;

            #[inline]
            fn sub(self, rhs: $name<MIN, MAX>) -> Self::Output {
                $name::rfrom(self).sub(rhs)
            }
        }

        impl<const MIN: i128, const MAX: i128> Sub<Constant> for $name<MIN, MAX> {
            type Output = $name<MIN, MAX>;

            #[inline]
            fn sub(self, rhs: Constant) -> Self::Output {
                self.sub(Self::rfrom(rhs))
            }
        }

        impl<const MIN: i128, const MAX: i128> SubAssign<Constant> for $name<MIN, MAX> {
            #[inline]
            fn sub_assign(&mut self, rhs: Constant) {
                self.sub_assign(Self::rfrom(rhs))
            }
        }

        impl<const MIN: i128, const MAX: i128> Mul<$name<MIN, MAX>> for Constant {
            type Output = $name<MIN, MAX>;

            #[inline]
            fn mul(self, rhs: $name<MIN, MAX>) -> Self::Output {
                $name::rfrom(self).mul(rhs)
            }
        }

        impl<const MIN: i128, const MAX: i128> Mul<Constant> for $name<MIN, MAX> {
            type Output = $name<MIN, MAX>;

            #[inline]
            fn mul(self, rhs: Constant) -> Self::Output {
                self.mul(Self::rfrom(rhs))
            }
        }

        impl<const MIN: i128, const MAX: i128> MulAssign<Constant> for $name<MIN, MAX> {
            #[inline]
            fn mul_assign(&mut self, rhs: Constant) {
                self.mul_assign(Self::rfrom(rhs))
            }
        }

        impl<const MIN: i128, const MAX: i128> Div<$name<MIN, MAX>> for Constant {
            type Output = $name<MIN, MAX>;

            #[inline]
            fn div(self, rhs: $name<MIN, MAX>) -> Self::Output {
                $name::rfrom(self).div(rhs)
            }
        }

        impl<const MIN: i128, const MAX: i128> Div<Constant> for $name<MIN, MAX> {
            type Output = $name<MIN, MAX>;

            #[inline]
            fn div(self, rhs: Constant) -> Self::Output {
                self.div(Self::rfrom(rhs))
            }
        }
        impl<const MIN: i128, const MAX: i128> DivAssign<Constant> for $name<MIN, MAX> {
            #[inline]
            fn div_assign(&mut self, rhs: Constant) {
                self.div_assign(Self::rfrom(rhs))
            }
        }

        impl<const MIN: i128, const MAX: i128> Rem<$name<MIN, MAX>> for Constant {
            type Output = $name<MIN, MAX>;

            #[inline]
            fn rem(self, rhs: $name<MIN, MAX>) -> Self::Output {
                $name::rfrom(self).rem(rhs)
            }
        }

        impl<const MIN: i128, const MAX: i128> Rem<Constant> for $name<MIN, MAX> {
            type Output = $name<MIN, MAX>;

            #[inline]
            fn rem(self, rhs: Constant) -> Self::Output {
                self.rem(Self::rfrom(rhs))
            }
        }
        impl<const MIN: i128, const MAX: i128> RemAssign<Constant> for $name<MIN, MAX> {
            #[inline]
            fn rem_assign(&mut self, rhs: Constant) {
                self.rem_assign(Self::rfrom(rhs))
            }
        }

        impl<const MIN: i128, const MAX: i128> Eq for $name<MIN, MAX> {}

        impl<
            const MIN1: i128,
            const MAX1: i128,
            const MIN2: i128,
            const MAX2: i128,
        > PartialEq<$name<MIN2, MAX2>> for $name<MIN1, MAX1> {
            #[inline]
            fn eq(&self, other: &$name<MIN2, MAX2>) -> bool {
                self.val.eq(&other.val)
            }
        }

        impl<const MIN: i128, const MAX: i128> PartialEq<Constant> for $name<MIN, MAX> {
            #[inline]
            fn eq(&self, other: &Constant) -> bool {
                self.val.eq(&<$repr>::from(*other))
            }
        }

        impl<const MIN: i128, const MAX: i128> PartialEq<$name<MIN, MAX>> for Constant {
            #[inline]
            fn eq(&self, other: &$name<MIN, MAX>) -> bool {
                <$repr>::from(*self).eq(&other.val)
            }
        }

        impl<const MIN: i128, const MAX: i128> Ord for $name<MIN, MAX> {
            #[inline]
            fn cmp(&self, other: &Self) -> core::cmp::Ordering {
                self.val.cmp(&other.val)
            }
        }

        impl<
            const MIN1: i128,
            const MAX1: i128,
            const MIN2: i128,
            const MAX2: i128,
        > PartialOrd<$name<MIN2, MAX2>> for $name<MIN1, MAX1> {
            #[inline]
            fn partial_cmp(
                &self,
                other: &$name<MIN2, MAX2>,
            ) -> Option<core::cmp::Ordering> {
                self.val.partial_cmp(&other.val)
            }
        }

        impl<const MIN: i128, const MAX: i128> PartialOrd<Constant> for $name<MIN, MAX> {
            #[inline]
            fn partial_cmp(
                &self,
                other: &Constant,
            ) -> Option<core::cmp::Ordering> {
                self.val.partial_cmp(&<$repr>::from(*other))
            }
        }

        impl<const MIN: i128, const MAX: i128> PartialOrd<$name<MIN, MAX>> for Constant {
            #[inline]
            fn partial_cmp(
                &self,
                other: &$name<MIN, MAX>,
            ) -> Option<core::cmp::Ordering> {
                <$repr>::from(*self).partial_cmp(&other.val)
            }
        }

        impl<const MIN: i128, const MAX: i128> core::fmt::Display for $name<MIN, MAX> {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                // We do this dance here because a Display impl is often used
                // when formatting a panic message, and panicking in this
                // context is supremely annoying because it causes an instant
                // abort. So if this value is not in bounds, then we write out
                // its debug repr which should show some nice output.
                match self.checked_add(Self::N::<0>()) {
                    Some(val) => core::fmt::Display::fmt(&val.get(), f),
                    None => write!(f, "{:?}", self),
                }
            }
        }

        impl<const MIN: i128, const MAX: i128> core::fmt::Debug for $name<MIN, MAX> {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                self.debug().fmt(f)
                /*
                if !f.alternate() {
                    self.debug().fmt(f)
                } else {
                    #[cfg(not(debug_assertions))]
                    {
                        f.debug_struct(stringify!($name))
                            .field("val", &self.val)
                            .field("MIN", &MIN)
                            .field("MAX", &MAX)
                            .finish()
                    }
                    #[cfg(debug_assertions)]
                    {
                        f.debug_struct(stringify!($name))
                            .field("val", &self.val)
                            .field("MIN", &MIN)
                            .field("MAX", &MAX)
                            .field("computed_min", &self.min)
                            .field("computed_max", &self.max)
                            .finish()
                    }
                }
                */
            }
        }

        #[cfg(test)]
        impl<const MIN: i128, const MAX: i128> quickcheck::Arbitrary for $name<MIN, MAX> {
            fn arbitrary(g: &mut quickcheck::Gen) -> Self {
                let mut n: $repr = <$repr>::arbitrary(g);
                if !Self::IS_PRIMITIVE {
                    n = n.wrapping_rem_euclid(Self::LEN as $repr);
                    n += Self::MIN_REPR;
                }
                Self::new(n).unwrap()
            }

            fn shrink(&self) -> alloc::boxed::Box<dyn Iterator<Item = Self>> {
                alloc::boxed::Box::new(self.val.shrink().filter_map(Self::new))
            }
        }
    };
}

define_ranged!(ri8, i8, smaller {}, bigger { ri16 i16, ri32 i32, ri64 i64, ri128 i128 });
define_ranged!(ri16, i16, smaller { ri8 i8 }, bigger { ri32 i32, ri64 i64, ri128 i128 });
define_ranged!(ri32, i32, smaller { ri8 i8, ri16 i16 }, bigger { ri64 i64, ri128 i128 });
define_ranged!(ri64, i64, smaller { ri8 i8, ri16 i16, ri32 i32 }, bigger { ri128 i128 });
define_ranged!(ri128, i128, smaller { ri8 i8, ri16 i16, ri32 i32, ri64 i64 }, bigger {});

impl<const MIN: i128, const MAX: i128> ri8<MIN, MAX> {
    #[inline]
    pub(crate) fn without_bounds(
        self,
    ) -> ri64<{ i64::MIN as i128 }, { i64::MAX as i128 }> {
        ri64::rfrom(self)
    }
}

impl<const MIN: i128, const MAX: i128> ri16<MIN, MAX> {
    #[inline]
    pub(crate) fn without_bounds(
        self,
    ) -> ri64<{ i64::MIN as i128 }, { i64::MAX as i128 }> {
        ri64::rfrom(self)
    }
}

impl<const MIN: i128, const MAX: i128> ri32<MIN, MAX> {
    #[inline]
    pub(crate) fn without_bounds(
        self,
    ) -> ri64<{ i64::MIN as i128 }, { i64::MAX as i128 }> {
        ri64::rfrom(self)
    }
}

impl<const MIN: i128, const MAX: i128> ri64<MIN, MAX> {
    #[inline]
    pub(crate) fn without_bounds(
        self,
    ) -> ri64<{ i64::MIN as i128 }, { i64::MAX as i128 }> {
        ri64::rfrom(self)
    }
}

impl<const MIN: i128, const MAX: i128> ri128<MIN, MAX> {
    #[inline]
    pub(crate) fn without_bounds(self) -> ri128<{ i128::MIN }, { i128::MAX }> {
        ri128::rfrom(self)
    }
}

pub(crate) struct RangedDebug<const MIN: i128, const MAX: i128> {
    rint: ri128<MIN, MAX>,
}

impl<const MIN: i128, const MAX: i128> core::fmt::Debug
    for RangedDebug<MIN, MAX>
{
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        #[cfg(not(debug_assertions))]
        {
            let val = self.rint.get_unchecked();
            if <ri128<MIN, MAX>>::contains(val) {
                val.fmt(f)
            } else {
                write!(f, "#{val:?} [out of range: {MIN}..={MAX}]#")
            }
        }
        #[cfg(debug_assertions)]
        {
            let val = self.rint.get_unchecked();
            let min = self.rint.min;
            let max = self.rint.max;
            if <ri128<MIN, MAX>>::contains(val)
                && <ri128<MIN, MAX>>::contains(min)
                && <ri128<MIN, MAX>>::contains(max)
            {
                val.fmt(f)
            } else {
                write!(
                    f,
                    "#{val:?} \
                     [out of range: {MIN}..={MAX}] \
                     [possible range: {min}..={max}]#",
                )
            }
        }
    }
}

/// A trait for losslessly converting between ranged integers.
///
/// This trait exists despite the fact that the standard library `From` trait
/// is defined in precisely the same way. Indeed, the `From` trait _almost_
/// works for our use case. The problem arises from the fact that we want
/// to be able to write this trait impl:
///
/// ```ignore
/// impl<
///     const MIN1: i128,
///     const MAX1: i128,
///     const MIN2: i128,
///     const MAX2: i128,
/// > From<ri64<MIN1, MAX1>> for ri64<MIN2, MAX2> {
/// // ...
/// }
/// ```
///
/// (We want this impl because we want to be able to freely convert between any
/// kind of ranged integers, including ranged integers with the same primitive
/// representation but different bounds.)
///
/// But this trait impl can't exist because it overlaps with the blanket
/// `impl From<T> for T`. Indeed, here, we do not provide that blanket impl,
/// which lets us add the trait impl above for `RFrom`.
///
/// This would normally be a no-go because it's too important for library
/// crates to provide types that work with `From` as you might expect, but
/// range integers are thankfully a crate internal abstraction. So we just need
/// to write `impl RFrom<T>` and do `t.rinto()` instead of `impl From<T>` and
/// `t.into()`.
pub(crate) trait RFrom<T>: Sized {
    fn rfrom(value: T) -> Self;
}

/// A trait for losslessly converting to ranged integers.
///
/// This goes along with `RFrom` and exists to make things like `t.rinto()`
/// work without the need to do `T::rfrom(..)`. Like the standard library
/// `Into` trait, a blanket impl is provided based on impls of `RFrom`. Callers
/// are not expected to implement this trait directly.
pub(crate) trait RInto<T>: Sized {
    fn rinto(self) -> T;
}

impl<T, U> RInto<U> for T
where
    U: RFrom<T>,
{
    fn rinto(self) -> U {
        RFrom::rfrom(self)
    }
}

pub(crate) trait TryRFrom<T>: Sized {
    fn try_rfrom(what: &'static str, value: T) -> Result<Self, Error>;
}

pub(crate) trait TryRInto<T>: Sized {
    fn try_rinto(self, what: &'static str) -> Result<T, Error>;
}

impl<T, U> TryRInto<U> for T
where
    U: TryRFrom<T>,
{
    #[inline]
    fn try_rinto(self, what: &'static str) -> Result<U, Error> {
        U::try_rfrom(what, self)
    }
}

macro_rules! composite {
    (($($name:ident),* $(,)?) => $with:expr) => {{
        crate::util::rangeint::composite!(($($name = $name),*) => $with)
    }};
    (($($name:ident = $rangeint:expr),* $(,)?) => $with:expr) => {{
        #[cfg(not(debug_assertions))]
        {
            $(
                let $name = $rangeint.val;
            )*
            let val = $with;
            crate::util::rangeint::Composite { val }
        }
        #[cfg(debug_assertions)]
        {
            let val = {
                $(
                    let $name = $rangeint.val;
                )*
                $with
            };
            let min = {
                $(
                    let $name = $rangeint.min;
                )*
                $with
            };
            let max = {
                $(
                    let $name = $rangeint.max;
                )*
                $with
            };
            crate::util::rangeint::Composite { val, min, max }
        }
    }};
}

macro_rules! uncomposite {
    ($composite:expr, $val:ident => ($($get:expr),* $(,)?) $(,)?) => {{
        #[cfg(not(debug_assertions))]
        {
            ($({
                let val = {
                    let $val = $composite.val;
                    $get
                };
                crate::util::rangeint::Composite { val }
            }),*)
        }
        #[cfg(debug_assertions)]
        {
            ($({
                let val = {
                    let $val = $composite.val;
                    $get
                };
                let min = {
                    let $val = $composite.min;
                    $get
                };
                let max = {
                    let $val = $composite.max;
                    $get
                };
                crate::util::rangeint::Composite { val, min, max }
            }),*)
        }
    }};
}

pub(crate) use {composite, uncomposite};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Composite<T> {
    pub(crate) val: T,
    #[cfg(debug_assertions)]
    pub(crate) min: T,
    #[cfg(debug_assertions)]
    pub(crate) max: T,
}

impl<T> Composite<T> {
    #[inline]
    pub(crate) fn map<U>(self, map: impl Fn(T) -> U) -> Composite<U> {
        #[cfg(not(debug_assertions))]
        {
            Composite { val: map(self.val) }
        }
        #[cfg(debug_assertions)]
        {
            Composite {
                val: map(self.val),
                min: map(self.min),
                max: map(self.max),
            }
        }
    }

    #[inline]
    pub(crate) fn zip2<U>(self, other: Composite<U>) -> Composite<(T, U)> {
        #[cfg(not(debug_assertions))]
        {
            Composite { val: (self.val, other.val) }
        }
        #[cfg(debug_assertions)]
        {
            Composite {
                val: (self.val, other.val),
                min: (self.min, other.min),
                max: (self.max, other.max),
            }
        }
    }
}

impl<T, U> Composite<(T, U)> {
    #[inline]
    pub(crate) fn unzip2(self) -> (Composite<T>, Composite<U>) {
        #[cfg(not(debug_assertions))]
        {
            (Composite { val: self.val.0 }, Composite { val: self.val.1 })
        }
        #[cfg(debug_assertions)]
        {
            (
                Composite {
                    val: self.val.0,
                    min: self.min.0,
                    max: self.max.0,
                },
                Composite {
                    val: self.val.1,
                    min: self.min.1,
                    max: self.max.1,
                },
            )
        }
    }
}

impl Composite<i8> {
    pub(crate) const fn to_rint<const MIN: i128, const MAX: i128>(
        self,
    ) -> ri8<MIN, MAX> {
        #[cfg(not(debug_assertions))]
        {
            ri8 { val: self.val }
        }
        #[cfg(debug_assertions)]
        {
            ri8 { val: self.val, min: self.min, max: self.max }
        }
    }
}

impl Composite<i16> {
    pub(crate) const fn to_rint<const MIN: i128, const MAX: i128>(
        self,
    ) -> ri16<MIN, MAX> {
        #[cfg(not(debug_assertions))]
        {
            ri16 { val: self.val }
        }
        #[cfg(debug_assertions)]
        {
            ri16 { val: self.val, min: self.min, max: self.max }
        }
    }
}

impl Composite<i32> {
    pub(crate) const fn to_rint<const MIN: i128, const MAX: i128>(
        self,
    ) -> ri32<MIN, MAX> {
        #[cfg(not(debug_assertions))]
        {
            ri32 { val: self.val }
        }
        #[cfg(debug_assertions)]
        {
            ri32 { val: self.val, min: self.min, max: self.max }
        }
    }
}

impl Composite<i64> {
    pub(crate) const fn to_rint<const MIN: i128, const MAX: i128>(
        self,
    ) -> ri64<MIN, MAX> {
        #[cfg(not(debug_assertions))]
        {
            ri64 { val: self.val }
        }
        #[cfg(debug_assertions)]
        {
            ri64 { val: self.val, min: self.min, max: self.max }
        }
    }

    pub(crate) fn try_to_rint<const MIN: i128, const MAX: i128>(
        self,
        what: &'static str,
    ) -> Result<ri64<MIN, MAX>, Error> {
        #[cfg(not(debug_assertions))]
        {
            if !ri64::<MIN, MAX>::contains(self.val) {
                return Err(ri64::<MIN, MAX>::error(what, self.val));
            }
            Ok(ri64 { val: self.val })
        }
        #[cfg(debug_assertions)]
        {
            if !ri64::<MIN, MAX>::contains(self.val) {
                return Err(ri64::<MIN, MAX>::error(what, self.val));
            }
            Ok(ri64 {
                val: self.val,
                min: self.min.clamp(MIN as i64, MAX as i64),
                max: self.max.clamp(MIN as i64, MAX as i64),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    // use super::*;

    // What follows below are some tests I wrote for my attempt at implementing
    // Ada-style modular/cyclic arithmetic on ranged integers. I found it to
    // be incredibly challenging. I decided that I could make do with only
    // using wrapping arithmetic on primitive-ranged integers and gave up.
    //
    // I did briefly look at GNAT to see if it could be of help, but I found
    // the source overwhelming and didn't find anything. I also could find any
    // help on the broader web for how to implement this correctly.
    //
    // Probably the next step here is to sit down with a pen & paper and work
    // out how this should be done, assuming we need/want it. One thing I ran
    // into was doing modular arithmetic when the range was bigger than the
    // underlying primitive representation. I could see how to do it if we
    // allowed casting up to a bigger integer representation, but I really
    // wanted to avoid doing that.
    /*
    type PrimitiveInt = ri8<{ i8::MIN as i128 }, { i8::MAX as i128 }>;
    type SmallInt = ri8<-20, 20>;
    type AlmostPrimitiveInt =
        ri8<{ i8::MIN as i128 }, { (i8::MAX - 1) as i128 }>;

    #[test]
    fn wrapping_add_small() {
        let int = |n| SmallInt::new(n).unwrap();

        assert_eq!(int(15).wrapping_add(int(5)), 20);
        assert_eq!(int(15).wrapping_add(int(6)), -20);
        assert_eq!(int(15).wrapping_add(int(-6)), 9);
        assert_eq!(int(-5).wrapping_add(int(5)), 0);
        assert_eq!(int(-5).wrapping_add(int(6)), 1);
        assert_eq!(int(-5).wrapping_add(int(3)), -2);
        assert_eq!(int(-5).wrapping_add(int(-3)), -8);
        assert_eq!(int(-5).wrapping_add(int(-13)), -18);
        assert_eq!(int(-5).wrapping_add(int(-15)), -20);
        assert_eq!(int(-5).wrapping_add(int(-16)), 20);

        // These tests get SmallInts that are out-of-bounds (which is legal as
        // an intermediate value) and then try to do wrapping arithmetic on
        // them.
        let a: SmallInt = PrimitiveInt::new(127).unwrap();
        assert_eq!(a.wrapping_add(int(1)), 5);
        let a: SmallInt = PrimitiveInt::new(-128).unwrap();
        assert_eq!(a.wrapping_add(int(-1)), -6);

        let a: SmallInt = PrimitiveInt::new(127).unwrap();
        let b: SmallInt = PrimitiveInt::new(127).unwrap();
        assert_eq!(a.wrapping_add(b), 8);

        let a: SmallInt = PrimitiveInt::new(-128).unwrap();
        let b: SmallInt = PrimitiveInt::new(-128).unwrap();
        assert_eq!(a.wrapping_add(b), -10);

        let a: SmallInt = PrimitiveInt::new(127).unwrap();
        let b: SmallInt = PrimitiveInt::new(-128).unwrap();
        assert_eq!(a.wrapping_add(b), -1);

        let a: SmallInt = PrimitiveInt::new(-128).unwrap();
        let b: SmallInt = PrimitiveInt::new(127).unwrap();
        assert_eq!(a.wrapping_add(b), -1);
    }

    #[test]
    fn wrapping_add_almost_primitive() {
        let int = |n| AlmostPrimitiveInt::new(n).unwrap();

        assert_eq!(int(126).wrapping_add(int(126)), 0);
    }

    quickcheck::quickcheck! {
        fn prop_wrapping_add_always_in_bounds(
            n1: SmallInt,
            n2: SmallInt
        ) -> bool {
            let sum = n1.wrapping_add(n2).get();
            SmallInt::contains(sum)
        }

        fn prop_wrapping_add_always_in_bounds_primitive(
            n1: PrimitiveInt,
            n2: PrimitiveInt
        ) -> bool {
            let sum = n1.wrapping_add(n2).get();
            PrimitiveInt::contains(sum)
        }
    }
    */
}
