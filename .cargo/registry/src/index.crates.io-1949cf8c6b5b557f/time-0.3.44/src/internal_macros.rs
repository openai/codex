//! Macros for use within the library. They are not publicly available.

/// Helper macro for easily implementing `OpAssign`.
macro_rules! __impl_assign {
    ($sym:tt $op:ident $fn:ident $target:ty : $($(#[$attr:meta])* $t:ty),+) => {$(
        #[allow(unused_qualifications)]
        $(#[$attr])*
        impl core::ops::$op<$t> for $target {
            #[inline]
            fn $fn(&mut self, rhs: $t) {
                *self = *self $sym rhs;
            }
        }
    )+};
}

/// Implement `AddAssign` for the provided types.
macro_rules! impl_add_assign {
    ($target:ty : $($(#[$attr:meta])* $t:ty),+ $(,)?) => {
        $crate::internal_macros::__impl_assign!(
            + AddAssign add_assign $target : $($(#[$attr])* $t),+
        );
    };
}

/// Implement `SubAssign` for the provided types.
macro_rules! impl_sub_assign {
    ($target:ty : $($(#[$attr:meta])* $t:ty),+ $(,)?) => {
        $crate::internal_macros::__impl_assign!(
            - SubAssign sub_assign $target : $($(#[$attr])* $t),+
        );
    };
}

/// Implement `MulAssign` for the provided types.
macro_rules! impl_mul_assign {
    ($target:ty : $($(#[$attr:meta])* $t:ty),+ $(,)?) => {
        $crate::internal_macros::__impl_assign!(
            * MulAssign mul_assign $target : $($(#[$attr])* $t),+
        );
    };
}

/// Implement `DivAssign` for the provided types.
macro_rules! impl_div_assign {
    ($target:ty : $($(#[$attr:meta])* $t:ty),+ $(,)?) => {
        $crate::internal_macros::__impl_assign!(
            / DivAssign div_assign $target : $($(#[$attr])* $t),+
        );
    };
}

/// Division of integers, rounding the resulting value towards negative infinity.
macro_rules! div_floor {
    ($self:expr, $rhs:expr) => {
        match ($self, $rhs) {
            (this, rhs) => {
                let d = this / rhs;
                let r = this % rhs;

                // If the remainder is non-zero, we need to subtract one if the
                // signs of self and rhs differ, as this means we rounded upwards
                // instead of downwards. We do this branchlessly by creating a mask
                // which is all-ones iff the signs differ, and 0 otherwise. Then by
                // adding this mask (which corresponds to the signed value -1), we
                // get our correction.
                let correction = (this ^ rhs) >> ($crate::size_of_val(&this) * 8 - 1);
                if r != 0 {
                    d + correction
                } else {
                    d
                }
            }
        }
    };
}

/// Similar to `overflowing_add`, but returning the number of times that it overflowed. Contained to
/// a certain range and only overflows a maximum number of times.
macro_rules! carry {
    (@most_once $value:expr, $min:literal.. $max:expr) => {
        match ($value, $min, $max) {
            (value, min, max) => {
                if crate::hint::likely(value >= min) {
                    if crate::hint::likely(value < max) {
                        (value, 0)
                    } else {
                        (value - (max - min), 1)
                    }
                } else {
                    (value + (max - min), -1)
                }
            }
        }
    };
    (@most_twice $value:expr, $min:literal.. $max:expr) => {
        match ($value, $min, $max) {
            (value, min, max) => {
                if crate::hint::likely(value >= min) {
                    if crate::hint::likely(value < max) {
                        (value, 0)
                    } else if value < 2 * max - min {
                        (value - (max - min), 1)
                    } else {
                        (value - 2 * (max - min), 2)
                    }
                } else {
                    if value >= min - max {
                        (value + (max - min), -1)
                    } else {
                        (value + 2 * (max - min), -2)
                    }
                }
            }
        }
    };
    (@most_thrice $value:expr, $min:literal.. $max:expr) => {
        match ($value, $min, $max) {
            (value, min, max) => {
                if crate::hint::likely(value >= min) {
                    if crate::hint::likely(value < max) {
                        (value, 0)
                    } else if value < 2 * max - min {
                        (value - (max - min), 1)
                    } else if value < 3 * max - 2 * min {
                        (value - 2 * (max - min), 2)
                    } else {
                        (value - 3 * (max - min), 3)
                    }
                } else {
                    if value >= min - max {
                        (value + (max - min), -1)
                    } else if value >= 2 * (min - max) {
                        (value + 2 * (max - min), -2)
                    } else {
                        (value + 3 * (max - min), -3)
                    }
                }
            }
        }
    };
}

/// Cascade an out-of-bounds value.
macro_rules! cascade {
    (@ordinal ordinal) => {};
    (@year year) => {};

    // Cascade an out-of-bounds value from "from" to "to".
    ($from:ident in $min:literal.. $max:expr => $to:tt) => {
        #[allow(unused_comparisons, unused_assignments)]
        let min = $min;
        let max = $max;
        if crate::hint::unlikely($from >= max) {
            $from -= max - min;
            $to += 1;
        } else if crate::hint::unlikely($from < min) {
            $from += max - min;
            $to -= 1;
        }
    };

    // Special case the ordinal-to-year cascade, as it has different behavior.
    ($ordinal:ident => $year:ident) => {
        // We need to actually capture the idents. Without this, macro hygiene causes errors.
        cascade!(@ordinal $ordinal);
        cascade!(@year $year);

        let days_in_year = crate::util::days_in_year($year) as i16;
        #[allow(unused_assignments)]
        if crate::hint::unlikely($ordinal > days_in_year) {
            $ordinal -= days_in_year;
            $year += 1;
        } else if crate::hint::unlikely($ordinal < 1) {
            $year -= 1;
            $ordinal += crate::util::days_in_year($year) as i16;
        }
    };
}

/// Constructs a ranged integer, returning a `ComponentRange` error if the value is out of range.
macro_rules! ensure_ranged {
    ($type:ty : $value:ident) => {
        match <$type>::new($value) {
            Some(val) => val,
            None => {
                $crate::hint::cold_path();
                #[allow(trivial_numeric_casts)]
                return Err(crate::error::ComponentRange {
                    name: stringify!($value),
                    minimum: <$type>::MIN.get() as i64,
                    maximum: <$type>::MAX.get() as i64,
                    value: $value as i64,
                    conditional_message: None,
                });
            }
        }
    };

    ($type:ty : $value:ident $(as $as_type:ident)? * $factor:expr) => {
        match ($value $(as $as_type)?).checked_mul($factor) {
            Some(val) => match <$type>::new(val) {
                Some(val) => val,
                None => {
                    $crate::hint::cold_path();
                    #[allow(trivial_numeric_casts)]
                    return Err(crate::error::ComponentRange {
                        name: stringify!($value),
                        minimum: <$type>::MIN.get() as i64 / $factor as i64,
                        maximum: <$type>::MAX.get() as i64 / $factor as i64,
                        value: $value as i64,
                        conditional_message: None,
                    });
                }
            },
            None => {
                $crate::hint::cold_path();
                return Err(crate::error::ComponentRange {
                    name: stringify!($value),
                    minimum: <$type>::MIN.get() as i64 / $factor as i64,
                    maximum: <$type>::MAX.get() as i64 / $factor as i64,
                    value: $value as i64,
                    conditional_message: None,
                });
            }
        }
    };
}

/// Try to unwrap an expression, returning if not possible.
///
/// This is similar to the `?` operator, but does not perform `.into()`. Because of this, it is
/// usable in `const` contexts.
macro_rules! const_try {
    ($e:expr) => {
        match $e {
            Ok(value) => value,
            Err(error) => return Err(error),
        }
    };
}

/// Try to unwrap an expression, returning if not possible.
///
/// This is similar to the `?` operator, but is usable in `const` contexts.
macro_rules! const_try_opt {
    ($e:expr) => {
        match $e {
            Some(value) => value,
            None => return None,
        }
    };
}

/// Try to unwrap an expression, panicking if not possible.
///
/// This is similar to `$e.expect($message)`, but is usable in `const` contexts.
macro_rules! expect_opt {
    ($e:expr, $message:literal) => {
        match $e {
            Some(value) => value,
            None => crate::expect_failed($message),
        }
    };
}

/// `unreachable!()`, but better.
#[cfg(any(feature = "formatting", feature = "parsing"))]
macro_rules! bug {
    () => { compile_error!("provide an error message to help fix a possible bug") };
    ($descr:literal $($rest:tt)?) => {
        panic!(concat!("internal error: ", $descr) $($rest)?)
    }
}

#[cfg(any(feature = "formatting", feature = "parsing"))]
pub(crate) use bug;
pub(crate) use {
    __impl_assign, carry, cascade, const_try, const_try_opt, div_floor, ensure_ranged, expect_opt,
    impl_add_assign, impl_div_assign, impl_mul_assign, impl_sub_assign,
};
