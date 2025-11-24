extern crate std;

use core::hash::Hash;
use std::format;
use std::prelude::rust_2021::*;

use crate::{
    IntErrorKind, OptionRangedI128, OptionRangedI16, OptionRangedI32, OptionRangedI64,
    OptionRangedI8, OptionRangedIsize, OptionRangedU128, OptionRangedU16, OptionRangedU32,
    OptionRangedU64, OptionRangedU8, OptionRangedUsize, ParseIntError, RangedI128, RangedI16,
    RangedI32, RangedI64, RangedI8, RangedIsize, RangedU128, RangedU16, RangedU32, RangedU64,
    RangedU8, RangedUsize, TryFromIntError,
};

#[test]
fn test_ranged_conversion() {
    // equal range
    let _: RangedI8<-5, 5> = RangedI16::<-5, 5>::new_static::<3>().into();

    // wider range
    let _: RangedI8<-6, 6> = RangedI16::<-5, 5>::new_static::<3>().into();

    // the following fails to compile because the source range is wider than the destination range
    // let _ : RangedI8<-4, 4> = RangedI16::<-5, 5>::new_static::<3>().into();
}

macro_rules! if_signed {
    (signed $($x:tt)*) => { $($x)* };
    (unsigned $($x:tt)*) => {};
}

macro_rules! if_unsigned {
    (signed $($x:tt)*) => {};
    (unsigned $($x:tt)*) => { $($x)* };
}

#[test]
fn errors() {
    assert_eq!(
        TryFromIntError.to_string(),
        "out of range integral type conversion attempted"
    );
    assert_eq!(TryFromIntError.clone(), TryFromIntError);
    assert_eq!(format!("{TryFromIntError:?}"), "TryFromIntError");

    assert_eq!(
        ParseIntError {
            kind: IntErrorKind::Empty,
        }
        .to_string(),
        "cannot parse integer from empty string"
    );
    assert_eq!(
        ParseIntError {
            kind: IntErrorKind::InvalidDigit,
        }
        .to_string(),
        "invalid digit found in string"
    );
    assert_eq!(
        ParseIntError {
            kind: IntErrorKind::PosOverflow,
        }
        .to_string(),
        "number too large to fit in target type"
    );
    assert_eq!(
        ParseIntError {
            kind: IntErrorKind::NegOverflow,
        }
        .to_string(),
        "number too small to fit in target type"
    );
    assert_eq!(
        ParseIntError {
            kind: IntErrorKind::Zero,
        }
        .to_string(),
        "number would be zero for non-zero type"
    );
    assert_eq!(
        format!(
            "{:?}",
            ParseIntError {
                kind: IntErrorKind::Empty
            }
        ),
        "ParseIntError { kind: Empty }"
    );
    assert_eq!(
        ParseIntError {
            kind: IntErrorKind::Empty
        }
        .clone(),
        ParseIntError {
            kind: IntErrorKind::Empty
        }
    );
    assert_eq!(
        ParseIntError {
            kind: IntErrorKind::Empty
        }
        .kind(),
        &IntErrorKind::Empty
    );
}

macro_rules! tests {
    ($($signed:ident $opt:ident $t:ident $inner:ident),* $(,)?) => {
        #[test]
        fn derives() {$(
            assert_eq!($t::<5, 10>::MIN.clone(), $t::<5, 10>::MIN);
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            $t::<5, 10>::MIN.hash(&mut hasher);
            assert_eq!(
                $t::<5, 10>::MIN.cmp(&$t::<5, 10>::MAX),
                core::cmp::Ordering::Less
            );

            assert_eq!($opt::<5, 10>::None.clone(), $opt::<5, 10>::None);
            $opt::<5, 10>::None.hash(&mut hasher);
        )*}

        #[test]
        fn expand() {$(
            let expanded: $t::<0, 20> = $t::<5, 10>::MAX.expand();
            assert_eq!(expanded, $t::<0, 20>::new_static::<10>());
        )*}

        #[test]
        fn narrow() {$(
            let narrowed: Option<$t::<10, 20>> = $t::<0, 20>::new_static::<10>().narrow();
            assert_eq!(narrowed, Some($t::<10, 20>::MIN));
        )*}

        #[test]
        fn new() {$(
            assert!($t::<5, 10>::new(10).is_some());
            assert!($t::<5, 10>::new(11).is_none());
        )*}

        #[test]
        fn new_static() {$(
            let six: $t::<5, 10> = $t::<5, 10>::new_static::<6>();
            assert_eq!(Some(six), $t::<5, 10>::new(6));
        )*}

        #[test]
        fn some_unchecked() {$(
            // Safety: The value is in range.
            unsafe {
                assert_eq!($opt::<5, 10>::some_unchecked(10), $opt::Some($t::<5, 10>::MAX));
            }
        )*}

        #[test]
        fn is_some() {$(
            assert!($opt::<5, 10>::Some($t::<5, 10>::MAX).is_some());
        )*}

        #[test]
        fn is_none() {$(
            assert!($opt::<5, 10>::None.is_none());
        )*}

        #[test]
        fn is_some_by_ref() {$(
            let value = $opt::<5, 10>::Some($t::<5, 10>::MAX);
            assert!($opt::is_some(&value));
        )*}

        #[test]
        fn is_none_by_ref() {$(
            let value = $opt::<5, 10>::None;
            assert!($opt::is_none(&value));
        )*}

        #[test]
        fn default() {$(
            assert_eq!($opt::<5, 10>::default(), $opt::<5, 10>::None);
        )*}

        #[test]
        fn get() {$(
            assert_eq!($t::<5, 10>::MAX.get(), 10);
            assert_eq!($opt::<5, 10>::None.get(), None);
            assert_eq!($opt::Some($t::<5, 10>::MAX).get(), Some($t::<5, 10>::MAX));
        )*}

        #[test]
        fn get_primitive() {$(
            assert_eq!($opt::Some($t::<5, 10>::MAX).get_primitive(), Some(10));
            assert_eq!($opt::<5, 10>::None.get_primitive(), None);
        )*}

        #[test]
        fn get_ref() {$(
            assert_eq!($t::<5, 10>::MAX.get_ref(), &10);
        )*}

        #[test]
        fn new_saturating() {$(
            assert_eq!($t::<5, 10>::new_saturating(11), $t::<5, 10>::MAX);
            assert_eq!($t::<5, 10>::new_saturating(0), $t::<5, 10>::MIN);
            assert_eq!($t::<5, 10>::new_saturating(9), $t::<5, 10>::new_static::<9>());
        )*}

        #[test]
        fn from_str_radix() {$(
            assert_eq!($t::<5, 10>::from_str_radix("10", 10), Ok($t::<5, 10>::MAX));
            assert_eq!($t::<5, 10>::from_str_radix("5", 10), Ok($t::<5, 10>::MIN));
            assert_eq!(
                $t::<5, 10>::from_str_radix("4", 10),
                Err(ParseIntError { kind: IntErrorKind::NegOverflow }),
            );
            assert_eq!(
                $t::<5, 10>::from_str_radix("11", 10),
                Err(ParseIntError { kind: IntErrorKind::PosOverflow }),
            );
            assert_eq!(
                $t::<5, 10>::from_str_radix("", 10),
                Err(ParseIntError { kind: IntErrorKind::Empty }),
            );
        )*}

        #[test]
        fn checked_add() {$(
            assert_eq!($t::<5, 10>::MAX.checked_add(1), None);
            assert_eq!($t::<5, 10>::MAX.checked_add(0), Some($t::<5, 10>::MAX));
        )*}

        #[test]
        fn unchecked_add() {$(
            // Safety: The result is in range.
            unsafe {
                assert_eq!($t::<5, 10>::MIN.unchecked_add(5), $t::<5, 10>::MAX);
            }
        )*}

        #[test]
        fn checked_sub() {$(
            assert_eq!($t::<5, 10>::MIN.checked_sub(1), None);
            assert_eq!($t::<5, 10>::MIN.checked_sub(0), Some($t::<5, 10>::MIN));
        )*}

        #[test]
        fn unchecked_sub() {$(
            // Safety: The result is in range.
            unsafe {
                assert_eq!($t::<5, 10>::MAX.unchecked_sub(5), $t::<5, 10>::MIN);
            }
        )*}

        #[test]
        fn checked_mul() {$(
            assert_eq!($t::<5, 10>::MAX.checked_mul(2), None);
            assert_eq!($t::<5, 10>::MAX.checked_mul(1), Some($t::<5, 10>::MAX));
        )*}

        #[test]
        fn unchecked_mul() {$(
            // Safety: The result is in range.
            unsafe {
                assert_eq!($t::<5, 10>::MAX.unchecked_mul(1), $t::<5, 10>::MAX);
            }
        )*}

        #[test]
        fn checked_div() {$(
            assert_eq!($t::<5, 10>::MAX.checked_div(3), None);
            assert_eq!($t::<5, 10>::MAX.checked_div(2), $t::<5, 10>::new(5));
            assert_eq!($t::<5, 10>::MAX.checked_div(1), Some($t::<5, 10>::MAX));
            assert_eq!($t::<5, 10>::MAX.checked_div(0), None);
        )*}

        #[test]
        fn unchecked_div() {$(
            // Safety: The result is in range.
            unsafe {
                assert_eq!($t::<5, 10>::MAX.unchecked_div(1), $t::<5, 10>::MAX);
            }
        )*}

        #[test]
        fn checked_div_euclid() {$(
            assert_eq!($t::<5, 10>::MAX.checked_div_euclid(3), None);
            assert_eq!($t::<5, 10>::MAX.checked_div_euclid(2), $t::<5, 10>::new(5));
            assert_eq!($t::<5, 10>::MAX.checked_div_euclid(1), Some($t::<5, 10>::MAX));
            assert_eq!($t::<5, 10>::MAX.checked_div_euclid(0), None);
        )*}

        #[test]
        fn unchecked_div_euclid() {$(
            // Safety: The result is in range.
            unsafe {
                assert_eq!($t::<5, 10>::MAX.unchecked_div_euclid(1), $t::<5, 10>::MAX);
            }
        )*}

        #[test]
        fn rem() {$(if_unsigned! { $signed
            assert_eq!($t::<5, 10>::MAX.rem($t::exact::<3>()), $t::<0, 3>::new_static::<1>());
            assert_eq!($t::<5, 10>::MAX.rem($t::exact::<5>()), $t::<0, 5>::MIN);
        })*}

        #[test]
        fn checked_rem() {$(
            assert_eq!($t::<5, 10>::MAX.checked_rem(11), Some($t::<5, 10>::MAX));
            assert_eq!($t::<5, 10>::MAX.checked_rem(5), None);
        )*}

        #[test]
        fn unchecked_rem() {$(
            // Safety: The result is in range.
            unsafe {
                assert_eq!($t::<5, 10>::MAX.unchecked_rem(11), $t::<5, 10>::MAX);
            }
        )*}

        #[test]
        fn checked_rem_euclid() {$(
            assert_eq!($t::<5, 10>::MAX.checked_rem_euclid(11), Some($t::<5, 10>::MAX));
            assert_eq!($t::<5, 10>::MAX.checked_rem_euclid(5), None);
        )*}

        #[test]
        fn unchecked_rem_euclid() {$(
            // Safety: The result is in range.
            unsafe {
                assert_eq!($t::<5, 10>::MAX.unchecked_rem_euclid(11), $t::<5, 10>::MAX);
            }
        )*}

        #[test]
        fn checked_neg() {$(
            assert_eq!($t::<5, 10>::MIN.checked_neg(), None);
            assert_eq!($t::<0, 10>::MIN.checked_neg(), Some($t::<0, 10>::MIN));
        )*}

        #[test]
        fn unchecked_neg() {$(
            // Safety: The result is in range.
            unsafe {
                assert_eq!($t::<0, 10>::MIN.unchecked_neg(), $t::<0, 10>::MIN);
            }
        )*}

        #[test]
        fn neg() {$( if_signed! { $signed
            assert_eq!($t::<-10, 10>::MIN.neg(), $t::<-10, 10>::MAX);
        })*}

        #[test]
        fn checked_shl() {$(
            assert_eq!($t::<5, 10>::MAX.checked_shl(1), None);
            assert_eq!($t::<5, 10>::MAX.checked_shl(0), Some($t::<5, 10>::MAX));
            assert_eq!($t::<5, 10>::MIN.checked_shl(1), Some($t::<5, 10>::MAX));
        )*}

        #[test]
        fn unchecked_shl() {$(
            // Safety: The result is in range.
            unsafe {
                assert_eq!($t::<5, 10>::MAX.unchecked_shl(0), $t::<5, 10>::MAX);
                assert_eq!($t::<5, 10>::MIN.unchecked_shl(1), $t::<5, 10>::MAX);
            }
        )*}

        #[test]
        fn checked_shr() {$(
            assert_eq!($t::<5, 10>::MAX.checked_shr(2), None);
            assert_eq!($t::<5, 10>::MAX.checked_shr(1), Some($t::<5, 10>::MIN));
            assert_eq!($t::<5, 10>::MAX.checked_shr(0), Some($t::<5, 10>::MAX));
        )*}

        #[test]
        fn unchecked_shr() {$(
            // Safety: The result is in range.
            unsafe {
                assert_eq!($t::<5, 10>::MAX.unchecked_shr(1), $t::<5, 10>::MIN);
                assert_eq!($t::<5, 10>::MAX.unchecked_shr(0), $t::<5, 10>::MAX);
            }
        )*}

        #[test]
        fn checked_abs() {$( if_signed! { $signed
            assert_eq!($t::<5, 10>::MAX.checked_abs(), Some($t::<5, 10>::MAX));
            assert_eq!($t::<-10, 10>::MIN.checked_abs(), Some($t::<-10, 10>::MAX));
            assert_eq!($t::<-10, 0>::MIN.checked_abs(), None);
        })*}

        #[test]
        fn unchecked_abs() { $(if_signed! { $signed
            // Safety: The result is in range.
            unsafe {
                assert_eq!($t::<5, 10>::MAX.unchecked_abs(), $t::<5, 10>::MAX);
                assert_eq!($t::<-10, 10>::MIN.unchecked_abs(), $t::<-10, 10>::MAX);
            }
        })*}

        #[test]
        fn abs() { $(if_signed! { $signed
            assert_eq!($t::<-5, 10>::MIN.abs().get(), 5);
        })*}

        #[test]
        fn checked_pow() {$(
            assert_eq!($t::<5, 10>::MAX.checked_pow(0), None);
            assert_eq!($t::<5, 10>::MAX.checked_pow(1), Some($t::<5, 10>::MAX));
            assert_eq!($t::<5, 10>::MAX.checked_pow(2), None);
        )*}

        #[test]
        fn unchecked_pow() {$(
            // Safety: The result is in range.
            unsafe {
                assert_eq!($t::<5, 10>::MAX.unchecked_pow(1), $t::<5, 10>::MAX);
            }
        )*}

        #[test]
        fn saturating_add() {$(
            assert_eq!($t::<5, 10>::MAX.saturating_add(0), $t::<5, 10>::MAX);
            assert_eq!($t::<5, 10>::MAX.saturating_add(1), $t::<5, 10>::MAX);
        )*}

        #[test]
        fn wrapping_add() {
            $(
                assert_eq!($t::<5, 10>::MAX.wrapping_add(0), $t::<5, 10>::MAX);
                assert_eq!($t::<5, 10>::MAX.wrapping_add(1), $t::<5, 10>::MIN);
                assert_eq!($t::<{ $inner::MIN }, { $inner::MAX }>::MAX.wrapping_add(1),
                           $t::<{ $inner::MIN }, { $inner::MAX }>::MIN);
                for i in 1..127 {
                    assert_eq!(
                        $t::<{ $inner::MIN}, { $inner::MAX - 1 }>::MAX.wrapping_add(i),
                        $t::<{ $inner::MIN}, { $inner::MAX - 1 }>::new($inner::MIN + i - 1).unwrap_or_else(|| panic!("adding {i}+{} does not yield {}", $inner::MIN, $inner::MAX + i ))
                    );
                }
            )*
            $(if_signed! { $signed
                for i in 1..=127 {
                    assert_eq!($t::<-5, 126>::MIN.wrapping_add(-i), $t::<-5,126>::new(126-i+1).unwrap_or_else(|| panic!("adding {i}+{} does not yield {}", $inner::MIN, 126-i+1)));
                    assert_eq!($t::<-5, 126>::MIN.wrapping_add(i), $t::<-5,126>::new(-5+i).unwrap_or_else(|| panic!("adding {i}+{} does not yield {}", $inner::MIN, 126-i+1)));
                }
                for i in -127..=-1 {
                    assert_eq!($t::<-5, 126>::MIN.wrapping_add(i), $t::<-5,126>::new(126+i+1).unwrap_or_else(|| panic!("adding {i}+{} does not yield {}", $inner::MIN, 126-i+1)));
                    assert_eq!($t::<-5, 126>::MIN.wrapping_add(-i), $t::<-5,126>::new(-5-i).unwrap_or_else(|| panic!("adding {i}+{} does not yield {}", $inner::MIN, 126-i+1)));
                }
                assert_eq!($t::<-5, 126>::MIN.wrapping_add(-128), $t::<-5,126>::new(-1).unwrap_or_else(|| panic!("adding 128+{} does not yield -1", $inner::MIN)));
                assert_eq!($t::<-5, 10>::MAX.wrapping_add(0), $t::<-5, 10>::MAX);
                assert_eq!($t::<-5, -3>::MIN.wrapping_add(-1-3), $t::<-5, -3>::MAX);
                assert_eq!($t::<-5, -3>::MIN.wrapping_add(-1-30), $t::<-5, -3>::MAX);
                assert_eq!($t::<-5, -3>::MIN.wrapping_add(30), $t::<-5, -3>::MIN);
                assert_eq!($t::<-5, -3>::MIN.wrapping_add(-30), $t::<-5, -3>::MIN);
                assert_eq!($t::<-5, 10>::MAX.wrapping_add(25), $t::<-5, 10>::MIN.wrapping_add(24));
                assert_eq!($t::<-5, 10>::MIN.wrapping_add(24), $t::<-5, 10>::MIN.wrapping_add(8));
                assert_eq!($t::<-5, 10>::MAX.wrapping_add(1), $t::<-5, 10>::MIN);
                assert_eq!($t::<-5, 10>::MIN.wrapping_add(-1), $t::<-5, 10>::MAX);
                assert_eq!($t::<-5, 127>::MIN.wrapping_add(-1), $t::<-5, 127>::MAX);
                assert_eq!($t::<-127, 126>::MIN.wrapping_add(-1), $t::<-127, 126>::MAX);
                assert_eq!($t::<{ $inner::MIN }, { $inner::MAX }>::MIN.wrapping_add(-1),
                           $t::<{ $inner::MIN }, { $inner::MAX }>::MAX);
            })*
        }

        #[test]
        fn wrapping_sub() {
            $(
                assert_eq!($t::<5, 10>::MIN.wrapping_sub(0), $t::<5, 10>::MIN);
                assert_eq!($t::<5, 10>::MIN.wrapping_sub(1), $t::<5, 10>::MAX);
                assert_eq!($t::<5, 10>::new(5 + 1).unwrap().wrapping_sub(1), $t::<5, 10>::MIN);
                assert_eq!($t::<5, 10>::MAX.wrapping_sub(1), $t::<5, 10>::new(10 - 1).unwrap());
                assert_eq!($t::<{ $inner::MIN }, { $inner::MAX }>::MIN.wrapping_sub(1),
                           $t::<{ $inner::MIN }, { $inner::MAX }>::MAX);
                for i in 1..127 {
                    assert_eq!(
                        $t::<{ $inner::MIN + 1 }, { $inner::MAX }>::MIN.wrapping_sub(i),
                        $t::<{ $inner::MIN + 1 }, { $inner::MAX }>::new($inner::MAX - i + 1).unwrap_or_else(|| panic!("failed test at iteration {i}"))
                    );
                }
            )*
            $(if_signed! { $signed
                for i in -127..=127 {
                    assert_eq!($t::<-5, 126>::MIN.wrapping_add(i), $t::<-5,126>::MIN.wrapping_sub(-i), "failed test at {i}");
                    assert_eq!($t::<-5, 126>::MIN.wrapping_add(-i), $t::<-5,126>::MIN.wrapping_sub(i), "failed test at {i}");
                }
                assert_eq!(
                    $t::<-5, 126>::MIN.wrapping_add(127).wrapping_add(1),
                    $t::<-5,126>::MIN.wrapping_sub(-128)
                );
                assert_eq!(
                    $t::<-5, 126>::MIN.wrapping_add(-128),
                    $t::<-5,126>::MIN.wrapping_sub(127).wrapping_sub(1)
                );
            })*
        }

        #[test]
        fn saturating_sub() {$(
            assert_eq!($t::<5, 10>::MIN.saturating_sub(0), $t::<5, 10>::MIN);
            assert_eq!($t::<5, 10>::MIN.saturating_sub(1), $t::<5, 10>::MIN);
        )*}

        #[test]
        fn saturating_neg() {$(if_signed! { $signed
            assert_eq!($t::<5, 10>::MIN.saturating_neg(), $t::<5, 10>::MIN);
            assert_eq!($t::<5, 10>::MAX.saturating_neg(), $t::<5, 10>::MIN);
            assert_eq!($t::<-10, 0>::MIN.saturating_neg(), $t::<-10, 0>::MAX);
            assert_eq!($t::<-10, 0>::MAX.saturating_neg(), $t::<-10, 0>::MAX);
        })*}

        #[test]
        fn saturating_abs() {$(if_signed! { $signed
            assert_eq!($t::<5, 10>::MIN.saturating_abs(), $t::<5, 10>::MIN);
            assert_eq!($t::<5, 10>::MAX.saturating_abs(), $t::<5, 10>::MAX);
            assert_eq!($t::<-10, 0>::MIN.saturating_abs(), $t::<-10, 0>::MAX);
            assert_eq!($t::<-10, 0>::MAX.saturating_abs(), $t::<-10, 0>::MAX);
        })*}

        #[test]
        fn saturating_mul() {$(
            assert_eq!($t::<5, 10>::MIN.saturating_mul(0), $t::<5, 10>::MIN);
            assert_eq!($t::<5, 10>::MIN.saturating_mul(1), $t::<5, 10>::MIN);
            assert_eq!($t::<5, 10>::MIN.saturating_mul(2), $t::<5, 10>::MAX);
            assert_eq!($t::<5, 10>::MIN.saturating_mul(3), $t::<5, 10>::MAX);
        )*}

        #[test]
        fn saturating_pow() {$(
            assert_eq!($t::<5, 10>::MIN.saturating_pow(0), $t::<5, 10>::MIN);
            assert_eq!($t::<5, 10>::MIN.saturating_pow(1), $t::<5, 10>::MIN);
            assert_eq!($t::<5, 10>::MIN.saturating_pow(2), $t::<5, 10>::MAX);
            assert_eq!($t::<5, 10>::MIN.saturating_pow(3), $t::<5, 10>::MAX);
        )*}

        #[test]
        fn as_ref() {$(
            assert_eq!($t::<5, 10>::MIN.as_ref(), &5);
            assert_eq!($t::<5, 10>::MAX.as_ref(), &10);
        )*}

        #[test]
        fn borrow() {
            use core::borrow::Borrow;
            $(
            assert_eq!(Borrow::<$inner>::borrow(&$t::<5, 10>::MIN), &5);
            assert_eq!(Borrow::<$inner>::borrow(&$t::<5, 10>::MAX), &10);
            )*
        }

        #[test]
        fn formatting() {$(
            let val = $t::<5, 10>::MAX;
            assert_eq!(format!("{}", val), "10");
            assert_eq!(format!("{:?}", val), "10");
            assert_eq!(format!("{:b}", val), "1010");
            assert_eq!(format!("{:o}", val), "12");
            assert_eq!(format!("{:x}", val), "a");
            assert_eq!(format!("{:X}", val), "A");
            assert_eq!(format!("{:e}", val), "1e1");
            assert_eq!(format!("{:E}", val), "1E1");

            assert_eq!(format!("{:?}", $opt::Some($t::<5, 10>::MAX)), "Some(10)");
            assert_eq!(format!("{:?}", $opt::<5, 10>::None), "None");
        )*}

        #[test]
        fn ord() {$(
            assert!($t::<5, 10>::MIN < $t::<5, 10>::MAX);
            assert!($t::<5, 10>::MIN <= $t::<5, 10>::MAX);
            assert!($t::<5, 10>::MAX > $t::<5, 10>::MIN);
            assert!($t::<5, 10>::MAX >= $t::<5, 10>::MIN);

            let none = $opt::<5, 10>::None;
            let five = $opt::Some($t::<5, 10>::MIN);
            let ten = $opt::Some($t::<5, 10>::MAX);

            assert_eq!(none.cmp(&none), core::cmp::Ordering::Equal);
            assert_eq!(five.cmp(&five), core::cmp::Ordering::Equal);
            assert_eq!(ten.cmp(&ten), core::cmp::Ordering::Equal);
            assert_eq!(none.cmp(&five), core::cmp::Ordering::Less);
            assert_eq!(five.cmp(&ten), core::cmp::Ordering::Less);
            assert_eq!(none.cmp(&ten), core::cmp::Ordering::Less);
            assert_eq!(ten.cmp(&none), core::cmp::Ordering::Greater);

            let none = $opt::<0, 10>::None;
            let zero = $opt::Some($t::<0, 10>::MIN);
            let ten = $opt::Some($t::<0, 10>::MAX);

            assert_eq!(none.partial_cmp(&none), Some(core::cmp::Ordering::Equal));
            assert_eq!(none.partial_cmp(&zero), Some(core::cmp::Ordering::Less));
            assert_eq!(zero.partial_cmp(&ten), Some(core::cmp::Ordering::Less));
            assert_eq!(none.partial_cmp(&ten), Some(core::cmp::Ordering::Less));
            assert_eq!(ten.partial_cmp(&none), Some(core::cmp::Ordering::Greater));
        )*}

        #[test]
        fn from() {$(
            assert_eq!($inner::from($t::<5, 10>::MAX), 10);
            assert_eq!($inner::from($t::<5, 10>::MIN), 5);

            assert_eq!($opt::from($t::<5, 10>::MAX), $opt::Some($t::<5, 10>::MAX));
            assert_eq!($opt::from(Some($t::<5, 10>::MAX)), $opt::Some($t::<5, 10>::MAX));
            assert_eq!($opt::<5, 10>::from(None), $opt::<5, 10>::None);
            assert_eq!(Option::from($opt::Some($t::<5, 10>::MAX)), Some($t::<5, 10>::MAX));
            assert_eq!(Option::<$t<5, 10>>::from($opt::<5, 10>::None), None);
        )*}

        #[test]
        fn try_from() {$(
            assert_eq!($t::<5, 10>::try_from(10), Ok($t::<5, 10>::MAX));
            assert_eq!($t::<5, 10>::try_from(5), Ok($t::<5, 10>::MIN));
            assert_eq!($t::<5, 10>::try_from(4), Err(TryFromIntError));
            assert_eq!($t::<5, 10>::try_from(11), Err(TryFromIntError));
        )*}

        #[test]
        fn from_str() {$(
            assert_eq!("10".parse::<$t<5, 10>>(), Ok($t::<5, 10>::MAX));
            assert_eq!("5".parse::<$t<5, 10>>(), Ok($t::<5, 10>::MIN));
            assert_eq!("4".parse::<$t<5, 10>>(), Err(ParseIntError { kind: IntErrorKind::NegOverflow }));
            assert_eq!("11".parse::<$t<5, 10>>(), Err(ParseIntError { kind: IntErrorKind::PosOverflow }));
            assert_eq!("".parse::<$t<5, 10>>(), Err(ParseIntError { kind: IntErrorKind::Empty }));
        )*}

        #[cfg(feature = "serde")]
        #[test]
        fn serde() -> serde_json::Result<()> {
            $(
            let val = $t::<5, 10>::MAX;
            let serialized = serde_json::to_string(&val)?;
            assert_eq!(serialized, "10");
            let deserialized: $t<5, 10> = serde_json::from_str(&serialized)?;
            assert_eq!(deserialized, val);

            assert!(serde_json::from_str::<$t<5, 10>>("").is_err());
            assert!(serde_json::from_str::<$t<5, 10>>("4").is_err());
            assert!(serde_json::from_str::<$t<5, 10>>("11").is_err());

            let val = $opt::<5, 10>::Some($t::<5, 10>::MAX);
            let serialized = serde_json::to_string(&val)?;
            assert_eq!(serialized, "10");
            let deserialized: $opt<5, 10> = serde_json::from_str(&serialized)?;
            assert_eq!(deserialized, val);

            assert!(serde_json::from_str::<$opt<5, 10>>("").is_err());
            assert!(serde_json::from_str::<$opt<5, 10>>("4").is_err());
            assert!(serde_json::from_str::<$opt<5, 10>>("11").is_err());

            let val = $opt::<5, 10>::None;
            let serialized = serde_json::to_string(&val)?;
            assert_eq!(serialized, "null");

            assert!(serde_json::from_str::<$opt<5, 10>>("").is_err());
            assert!(serde_json::from_str::<$opt<5, 10>>("4").is_err());
            assert!(serde_json::from_str::<$opt<5, 10>>("11").is_err());
            )*
            Ok(())
        }

        #[cfg(feature = "rand08")]
        #[test]
        fn rand08() {$(
            let rand_val: $t<5, 10> = rand08::random();
            assert!(rand_val >= $t::<5, 10>::MIN);
            assert!(rand_val <= $t::<5, 10>::MAX);

            let rand: $opt<5, 10> = rand08::random();
            if let Some(rand) = rand.get() {
                assert!(rand >= $t::<5, 10>::MIN);
                assert!(rand <= $t::<5, 10>::MAX);
            }
        )*}

        #[cfg(feature = "rand09")]
        #[test]
        fn rand09() {$(
            let rand_val: $t<5, 10> = rand09::random();
            assert!(rand_val >= $t::<5, 10>::MIN);
            assert!(rand_val <= $t::<5, 10>::MAX);

            let rand: $opt<5, 10> = rand09::random();
            if let Some(rand) = rand.get() {
                assert!(rand >= $t::<5, 10>::MIN);
                assert!(rand <= $t::<5, 10>::MAX);
            }
        )*}

        #[cfg(feature = "num")]
        #[test]
        fn num() {$(
            assert_eq!(<$t<5, 10> as num_traits::Bounded>::min_value(), $t::<5, 10>::MIN);
            assert_eq!(<$t<5, 10> as num_traits::Bounded>::max_value(), $t::<5, 10>::MAX);
        )*}

        #[cfg(feature = "quickcheck")]
        #[test]
        fn quickcheck() {$(
            #[allow(trivial_casts)]
            quickcheck::quickcheck((|val| {
                val >= $t::<5, 10>::MIN && val <= $t::<5, 10>::MAX
            }) as fn($t<5, 10>) -> bool);

            #[allow(trivial_casts)]
            quickcheck::quickcheck((|val| {
                if let Some(val) = val.get() {
                    val >= $t::<5, 10>::MIN && val <= $t::<5, 10>::MAX
                } else {
                    true
                }
            }) as fn($opt<5, 10>) -> bool);
        )*}
    };
}

tests![
    signed OptionRangedI8 RangedI8 i8,
    signed OptionRangedI16 RangedI16 i16,
    signed OptionRangedI32 RangedI32 i32,
    signed OptionRangedI64 RangedI64 i64,
    signed OptionRangedI128 RangedI128 i128,
    signed OptionRangedIsize RangedIsize isize,
    unsigned OptionRangedU8 RangedU8 u8,
    unsigned OptionRangedU16 RangedU16 u16,
    unsigned OptionRangedU32 RangedU32 u32,
    unsigned OptionRangedU64 RangedU64 u64,
    unsigned OptionRangedU128 RangedU128 u128,
    unsigned OptionRangedUsize RangedUsize usize,
];
