//! Tests of `num_traits::cast`.

#![cfg_attr(not(feature = "std"), no_std)]

use num_traits::cast::*;
use num_traits::Bounded;

use core::{f32, f64};
use core::{i128, i16, i32, i64, i8, isize};
use core::{u128, u16, u32, u64, u8, usize};

use core::fmt::Debug;
use core::mem;
use core::num::Wrapping;

#[test]
fn to_primitive_float() {
    let f32_toolarge = 1e39f64;
    assert_eq!(f32_toolarge.to_f32(), Some(f32::INFINITY));
    assert_eq!((-f32_toolarge).to_f32(), Some(f32::NEG_INFINITY));
    assert_eq!((f32::MAX as f64).to_f32(), Some(f32::MAX));
    assert_eq!((-f32::MAX as f64).to_f32(), Some(-f32::MAX));
    assert_eq!(f64::INFINITY.to_f32(), Some(f32::INFINITY));
    assert_eq!((f64::NEG_INFINITY).to_f32(), Some(f32::NEG_INFINITY));
    assert!((f64::NAN).to_f32().map_or(false, |f| f.is_nan()));
}

#[test]
fn wrapping_to_primitive() {
    macro_rules! test_wrapping_to_primitive {
        ($($t:ty)+) => {
            $({
                let i: $t = 0;
                let w = Wrapping(i);
                assert_eq!(i.to_u8(),    w.to_u8());
                assert_eq!(i.to_u16(),   w.to_u16());
                assert_eq!(i.to_u32(),   w.to_u32());
                assert_eq!(i.to_u64(),   w.to_u64());
                assert_eq!(i.to_usize(), w.to_usize());
                assert_eq!(i.to_i8(),    w.to_i8());
                assert_eq!(i.to_i16(),   w.to_i16());
                assert_eq!(i.to_i32(),   w.to_i32());
                assert_eq!(i.to_i64(),   w.to_i64());
                assert_eq!(i.to_isize(), w.to_isize());
                assert_eq!(i.to_f32(),   w.to_f32());
                assert_eq!(i.to_f64(),   w.to_f64());
            })+
        };
    }

    test_wrapping_to_primitive!(usize u8 u16 u32 u64 isize i8 i16 i32 i64);
}

#[test]
fn wrapping_is_toprimitive() {
    fn require_toprimitive<T: ToPrimitive>(_: &T) {}
    require_toprimitive(&Wrapping(42));
}

#[test]
fn wrapping_is_fromprimitive() {
    fn require_fromprimitive<T: FromPrimitive>(_: &T) {}
    require_fromprimitive(&Wrapping(42));
}

#[test]
fn wrapping_is_numcast() {
    fn require_numcast<T: NumCast>(_: &T) {}
    require_numcast(&Wrapping(42));
}

#[test]
fn as_primitive() {
    let x: f32 = (1.625f64).as_();
    assert_eq!(x, 1.625f32);

    let x: f32 = (3.14159265358979323846f64).as_();
    assert_eq!(x, 3.1415927f32);

    let x: u8 = (768i16).as_();
    assert_eq!(x, 0);
}

#[test]
fn float_to_integer_checks_overflow() {
    // This will overflow an i32
    let source: f64 = 1.0e+123f64;

    // Expect the overflow to be caught
    assert_eq!(cast::<f64, i32>(source), None);
}

#[test]
fn cast_to_int_checks_overflow() {
    let big_f: f64 = 1.0e123;
    let normal_f: f64 = 1.0;
    let small_f: f64 = -1.0e123;
    assert_eq!(None, cast::<f64, isize>(big_f));
    assert_eq!(None, cast::<f64, i8>(big_f));
    assert_eq!(None, cast::<f64, i16>(big_f));
    assert_eq!(None, cast::<f64, i32>(big_f));
    assert_eq!(None, cast::<f64, i64>(big_f));

    assert_eq!(Some(normal_f as isize), cast::<f64, isize>(normal_f));
    assert_eq!(Some(normal_f as i8), cast::<f64, i8>(normal_f));
    assert_eq!(Some(normal_f as i16), cast::<f64, i16>(normal_f));
    assert_eq!(Some(normal_f as i32), cast::<f64, i32>(normal_f));
    assert_eq!(Some(normal_f as i64), cast::<f64, i64>(normal_f));

    assert_eq!(None, cast::<f64, isize>(small_f));
    assert_eq!(None, cast::<f64, i8>(small_f));
    assert_eq!(None, cast::<f64, i16>(small_f));
    assert_eq!(None, cast::<f64, i32>(small_f));
    assert_eq!(None, cast::<f64, i64>(small_f));
}

#[test]
fn cast_to_unsigned_int_checks_overflow() {
    let big_f: f64 = 1.0e123;
    let normal_f: f64 = 1.0;
    let small_f: f64 = -1.0e123;
    assert_eq!(None, cast::<f64, usize>(big_f));
    assert_eq!(None, cast::<f64, u8>(big_f));
    assert_eq!(None, cast::<f64, u16>(big_f));
    assert_eq!(None, cast::<f64, u32>(big_f));
    assert_eq!(None, cast::<f64, u64>(big_f));

    assert_eq!(Some(normal_f as usize), cast::<f64, usize>(normal_f));
    assert_eq!(Some(normal_f as u8), cast::<f64, u8>(normal_f));
    assert_eq!(Some(normal_f as u16), cast::<f64, u16>(normal_f));
    assert_eq!(Some(normal_f as u32), cast::<f64, u32>(normal_f));
    assert_eq!(Some(normal_f as u64), cast::<f64, u64>(normal_f));

    assert_eq!(None, cast::<f64, usize>(small_f));
    assert_eq!(None, cast::<f64, u8>(small_f));
    assert_eq!(None, cast::<f64, u16>(small_f));
    assert_eq!(None, cast::<f64, u32>(small_f));
    assert_eq!(None, cast::<f64, u64>(small_f));
}

#[test]
fn cast_to_i128_checks_overflow() {
    let big_f: f64 = 1.0e123;
    let normal_f: f64 = 1.0;
    let small_f: f64 = -1.0e123;
    assert_eq!(None, cast::<f64, i128>(big_f));
    assert_eq!(None, cast::<f64, u128>(big_f));

    assert_eq!(Some(normal_f as i128), cast::<f64, i128>(normal_f));
    assert_eq!(Some(normal_f as u128), cast::<f64, u128>(normal_f));

    assert_eq!(None, cast::<f64, i128>(small_f));
    assert_eq!(None, cast::<f64, u128>(small_f));
}

#[cfg(feature = "std")]
fn dbg(args: ::core::fmt::Arguments<'_>) {
    println!("{}", args);
}

#[cfg(not(feature = "std"))]
fn dbg(_: ::core::fmt::Arguments) {}

// Rust 1.8 doesn't handle cfg on macros correctly
macro_rules! dbg { ($($tok:tt)*) => { dbg(format_args!($($tok)*)) } }

macro_rules! float_test_edge {
    ($f:ident -> $($t:ident)+) => { $({
        dbg!("testing cast edge cases for {} -> {}", stringify!($f), stringify!($t));

        let small = if $t::MIN == 0 || mem::size_of::<$t>() < mem::size_of::<$f>() {
            $t::MIN as $f - 1.0
        } else {
            ($t::MIN as $f).raw_inc().floor()
        };
        let fmin = small.raw_dec();
        dbg!("  testing min {}\n\tvs. {:.0}\n\tand {:.0}", $t::MIN, fmin, small);
        assert_eq!(Some($t::MIN), cast::<$f, $t>($t::MIN as $f));
        assert_eq!(Some($t::MIN), cast::<$f, $t>(fmin));
        assert_eq!(None, cast::<$f, $t>(small));

        let (max, large) = if mem::size_of::<$t>() < mem::size_of::<$f>() {
            ($t::MAX, $t::MAX as $f + 1.0)
        } else {
            let large = $t::MAX as $f; // rounds up!
            let max = large.raw_dec() as $t; // the next smallest possible
            assert_eq!(max.count_ones(), $f::MANTISSA_DIGITS);
            (max, large)
        };
        let fmax = large.raw_dec();
        dbg!("  testing max {}\n\tvs. {:.0}\n\tand {:.0}", max, fmax, large);
        assert_eq!(Some(max), cast::<$f, $t>(max as $f));
        assert_eq!(Some(max), cast::<$f, $t>(fmax));
        assert_eq!(None, cast::<$f, $t>(large));

        dbg!("  testing non-finite values");
        assert_eq!(None, cast::<$f, $t>($f::NAN));
        assert_eq!(None, cast::<$f, $t>($f::INFINITY));
        assert_eq!(None, cast::<$f, $t>($f::NEG_INFINITY));
    })+}
}

trait RawOffset: Sized {
    fn raw_inc(self) -> Self;
    fn raw_dec(self) -> Self;
}

impl RawOffset for f32 {
    fn raw_inc(self) -> Self {
        Self::from_bits(self.to_bits() + 1)
    }

    fn raw_dec(self) -> Self {
        Self::from_bits(self.to_bits() - 1)
    }
}

impl RawOffset for f64 {
    fn raw_inc(self) -> Self {
        Self::from_bits(self.to_bits() + 1)
    }

    fn raw_dec(self) -> Self {
        Self::from_bits(self.to_bits() - 1)
    }
}

#[test]
fn cast_float_to_int_edge_cases() {
    float_test_edge!(f32 -> isize i8 i16 i32 i64);
    float_test_edge!(f32 -> usize u8 u16 u32 u64);
    float_test_edge!(f64 -> isize i8 i16 i32 i64);
    float_test_edge!(f64 -> usize u8 u16 u32 u64);
}

#[test]
fn cast_float_to_i128_edge_cases() {
    float_test_edge!(f32 -> i128 u128);
    float_test_edge!(f64 -> i128 u128);
}

macro_rules! int_test_edge {
    ($f:ident -> { $($t:ident)+ } with $BigS:ident $BigU:ident ) => { $({
        #[allow(arithmetic_overflow)] // https://github.com/rust-lang/rust/issues/109731
        fn test_edge() {
            dbg!("testing cast edge cases for {} -> {}", stringify!($f), stringify!($t));

            match ($f::MIN as $BigS).cmp(&($t::MIN as $BigS)) {
                Greater => {
                    assert_eq!(Some($f::MIN as $t), cast::<$f, $t>($f::MIN));
                }
                Equal => {
                    assert_eq!(Some($t::MIN), cast::<$f, $t>($f::MIN));
                }
                Less => {
                    let min = $t::MIN as $f;
                    assert_eq!(Some($t::MIN), cast::<$f, $t>(min));
                    assert_eq!(None, cast::<$f, $t>(min - 1));
                }
            }

            match ($f::MAX as $BigU).cmp(&($t::MAX as $BigU)) {
                Greater => {
                    let max = $t::MAX as $f;
                    assert_eq!(Some($t::MAX), cast::<$f, $t>(max));
                    assert_eq!(None, cast::<$f, $t>(max + 1));
                }
                Equal => {
                    assert_eq!(Some($t::MAX), cast::<$f, $t>($f::MAX));
                }
                Less => {
                    assert_eq!(Some($f::MAX as $t), cast::<$f, $t>($f::MAX));
                }
            }
        }
        test_edge();
    })+}
}

#[test]
fn cast_int_to_int_edge_cases() {
    use core::cmp::Ordering::*;

    macro_rules! test_edge {
        ($( $from:ident )+) => { $({
            int_test_edge!($from -> { isize i8 i16 i32 i64 } with i64 u64);
            int_test_edge!($from -> { usize u8 u16 u32 u64 } with i64 u64);
        })+}
    }

    test_edge!(isize i8 i16 i32 i64);
    test_edge!(usize u8 u16 u32 u64);
}

#[test]
fn cast_int_to_128_edge_cases() {
    use core::cmp::Ordering::*;

    macro_rules! test_edge {
        ($( $t:ident )+) => {
            $(
                int_test_edge!($t -> { i128 u128 } with i128 u128);
            )+
            int_test_edge!(i128 -> { $( $t )+ } with i128 u128);
            int_test_edge!(u128 -> { $( $t )+ } with i128 u128);
        }
    }

    test_edge!(isize i8 i16 i32 i64 i128);
    test_edge!(usize u8 u16 u32 u64 u128);
}

#[test]
fn newtype_from_primitive() {
    #[derive(PartialEq, Debug)]
    struct New<T>(T);

    // minimal impl
    impl<T: FromPrimitive> FromPrimitive for New<T> {
        fn from_i64(n: i64) -> Option<Self> {
            T::from_i64(n).map(New)
        }

        fn from_u64(n: u64) -> Option<Self> {
            T::from_u64(n).map(New)
        }
    }

    macro_rules! assert_eq_from {
        ($( $from:ident )+) => {$(
            assert_eq!(T::$from(Bounded::min_value()).map(New),
                       New::<T>::$from(Bounded::min_value()));
            assert_eq!(T::$from(Bounded::max_value()).map(New),
                       New::<T>::$from(Bounded::max_value()));
        )+}
    }

    fn check<T: PartialEq + Debug + FromPrimitive>() {
        assert_eq_from!(from_i8 from_i16 from_i32 from_i64 from_isize);
        assert_eq_from!(from_u8 from_u16 from_u32 from_u64 from_usize);
        assert_eq_from!(from_f32 from_f64);
    }

    macro_rules! check {
        ($( $ty:ty )+) => {$( check::<$ty>(); )+}
    }
    check!(i8 i16 i32 i64 isize);
    check!(u8 u16 u32 u64 usize);
}

#[test]
fn newtype_to_primitive() {
    #[derive(PartialEq, Debug)]
    struct New<T>(T);

    // minimal impl
    impl<T: ToPrimitive> ToPrimitive for New<T> {
        fn to_i64(&self) -> Option<i64> {
            self.0.to_i64()
        }

        fn to_u64(&self) -> Option<u64> {
            self.0.to_u64()
        }
    }

    macro_rules! assert_eq_to {
        ($( $to:ident )+) => {$(
            assert_eq!(T::$to(&Bounded::min_value()),
                       New::<T>::$to(&New(Bounded::min_value())));
            assert_eq!(T::$to(&Bounded::max_value()),
                       New::<T>::$to(&New(Bounded::max_value())));
        )+}
    }

    fn check<T: PartialEq + Debug + Bounded + ToPrimitive>() {
        assert_eq_to!(to_i8 to_i16 to_i32 to_i64 to_isize);
        assert_eq_to!(to_u8 to_u16 to_u32 to_u64 to_usize);
        assert_eq_to!(to_f32 to_f64);
    }

    macro_rules! check {
        ($( $ty:ty )+) => {$( check::<$ty>(); )+}
    }
    check!(i8 i16 i32 i64 isize);
    check!(u8 u16 u32 u64 usize);
}
