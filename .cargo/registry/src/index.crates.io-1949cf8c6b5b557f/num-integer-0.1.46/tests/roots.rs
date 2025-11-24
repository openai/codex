use num_integer::Roots;
use num_traits::checked_pow;
use num_traits::{AsPrimitive, PrimInt, Signed};
use std::f64::MANTISSA_DIGITS;
use std::fmt::Debug;
use std::mem;

trait TestInteger: Roots + PrimInt + Debug + AsPrimitive<f64> + 'static {}

impl<T> TestInteger for T where T: Roots + PrimInt + Debug + AsPrimitive<f64> + 'static {}

/// Check that each root is correct
///
/// If `x` is positive, check `rⁿ ≤ x < (r+1)ⁿ`.
/// If `x` is negative, check `(r-1)ⁿ < x ≤ rⁿ`.
fn check<T>(v: &[T], n: u32)
where
    T: TestInteger,
{
    for i in v {
        let rt = i.nth_root(n);
        // println!("nth_root({:?}, {}) = {:?}", i, n, rt);
        if n == 2 {
            assert_eq!(rt, i.sqrt());
        } else if n == 3 {
            assert_eq!(rt, i.cbrt());
        }
        if *i >= T::zero() {
            let rt1 = rt + T::one();
            assert!(rt.pow(n) <= *i);
            if let Some(x) = checked_pow(rt1, n as usize) {
                assert!(*i < x);
            }
        } else {
            let rt1 = rt - T::one();
            assert!(rt < T::zero());
            assert!(*i <= rt.pow(n));
            if let Some(x) = checked_pow(rt1, n as usize) {
                assert!(x < *i);
            }
        };
    }
}

/// Get the maximum value that will round down as `f64` (if any),
/// and its successor that will round up.
///
/// Important because the `std` implementations cast to `f64` to
/// get a close approximation of the roots.
fn mantissa_max<T>() -> Option<(T, T)>
where
    T: TestInteger,
{
    let bits = if T::min_value().is_zero() {
        8 * mem::size_of::<T>()
    } else {
        8 * mem::size_of::<T>() - 1
    };
    if bits > MANTISSA_DIGITS as usize {
        let rounding_bit = T::one() << (bits - MANTISSA_DIGITS as usize - 1);
        let x = T::max_value() - rounding_bit;

        let x1 = x + T::one();
        let x2 = x1 + T::one();
        assert!(x.as_() < x1.as_());
        assert_eq!(x1.as_(), x2.as_());

        Some((x, x1))
    } else {
        None
    }
}

fn extend<T>(v: &mut Vec<T>, start: T, end: T)
where
    T: TestInteger,
{
    let mut i = start;
    while i < end {
        v.push(i);
        i = i + T::one();
    }
    v.push(i);
}

fn extend_shl<T>(v: &mut Vec<T>, start: T, end: T, mask: T)
where
    T: TestInteger,
{
    let mut i = start;
    while i != end {
        v.push(i);
        i = (i << 1) & mask;
    }
}

fn extend_shr<T>(v: &mut Vec<T>, start: T, end: T)
where
    T: TestInteger,
{
    let mut i = start;
    while i != end {
        v.push(i);
        i = i >> 1;
    }
}

fn pos<T>() -> Vec<T>
where
    T: TestInteger,
    i8: AsPrimitive<T>,
{
    let mut v: Vec<T> = vec![];
    if mem::size_of::<T>() == 1 {
        extend(&mut v, T::zero(), T::max_value());
    } else {
        extend(&mut v, T::zero(), i8::max_value().as_());
        extend(
            &mut v,
            T::max_value() - i8::max_value().as_(),
            T::max_value(),
        );
        if let Some((i, j)) = mantissa_max::<T>() {
            v.push(i);
            v.push(j);
        }
        extend_shl(&mut v, T::max_value(), T::zero(), !T::min_value());
        extend_shr(&mut v, T::max_value(), T::zero());
    }
    v
}

fn neg<T>() -> Vec<T>
where
    T: TestInteger + Signed,
    i8: AsPrimitive<T>,
{
    let mut v: Vec<T> = vec![];
    if mem::size_of::<T>() <= 1 {
        extend(&mut v, T::min_value(), T::zero());
    } else {
        extend(&mut v, i8::min_value().as_(), T::zero());
        extend(
            &mut v,
            T::min_value(),
            T::min_value() - i8::min_value().as_(),
        );
        if let Some((i, j)) = mantissa_max::<T>() {
            v.push(-i);
            v.push(-j);
        }
        extend_shl(&mut v, -T::one(), T::min_value(), !T::zero());
        extend_shr(&mut v, T::min_value(), -T::one());
    }
    v
}

macro_rules! test_roots {
    ($I:ident, $U:ident) => {
        mod $I {
            use crate::check;
            use crate::neg;
            use crate::pos;
            use num_integer::Roots;
            use std::mem;

            #[test]
            #[should_panic]
            fn zeroth_root() {
                (123 as $I).nth_root(0);
            }

            #[test]
            fn sqrt() {
                check(&pos::<$I>(), 2);
            }

            #[test]
            #[should_panic]
            fn sqrt_neg() {
                (-123 as $I).sqrt();
            }

            #[test]
            fn cbrt() {
                check(&pos::<$I>(), 3);
            }

            #[test]
            fn cbrt_neg() {
                check(&neg::<$I>(), 3);
            }

            #[test]
            fn nth_root() {
                let bits = 8 * mem::size_of::<$I>() as u32 - 1;
                let pos = pos::<$I>();
                for n in 4..bits {
                    check(&pos, n);
                }
            }

            #[test]
            fn nth_root_neg() {
                let bits = 8 * mem::size_of::<$I>() as u32 - 1;
                let neg = neg::<$I>();
                for n in 2..bits / 2 {
                    check(&neg, 2 * n + 1);
                }
            }

            #[test]
            fn bit_size() {
                let bits = 8 * mem::size_of::<$I>() as u32 - 1;
                assert_eq!($I::max_value().nth_root(bits - 1), 2);
                assert_eq!($I::max_value().nth_root(bits), 1);
                assert_eq!($I::min_value().nth_root(bits), -2);
                assert_eq!(($I::min_value() + 1).nth_root(bits), -1);
            }
        }

        mod $U {
            use crate::check;
            use crate::pos;
            use num_integer::Roots;
            use std::mem;

            #[test]
            #[should_panic]
            fn zeroth_root() {
                (123 as $U).nth_root(0);
            }

            #[test]
            fn sqrt() {
                check(&pos::<$U>(), 2);
            }

            #[test]
            fn cbrt() {
                check(&pos::<$U>(), 3);
            }

            #[test]
            fn nth_root() {
                let bits = 8 * mem::size_of::<$I>() as u32 - 1;
                let pos = pos::<$I>();
                for n in 4..bits {
                    check(&pos, n);
                }
            }

            #[test]
            fn bit_size() {
                let bits = 8 * mem::size_of::<$U>() as u32;
                assert_eq!($U::max_value().nth_root(bits - 1), 2);
                assert_eq!($U::max_value().nth_root(bits), 1);
            }
        }
    };
}

test_roots!(i8, u8);
test_roots!(i16, u16);
test_roots!(i32, u32);
test_roots!(i64, u64);
test_roots!(i128, u128);
test_roots!(isize, usize);
