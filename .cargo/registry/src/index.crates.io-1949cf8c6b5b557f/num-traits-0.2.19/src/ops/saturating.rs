use core::ops::{Add, Mul, Sub};

/// Saturating math operations. Deprecated, use `SaturatingAdd`, `SaturatingSub` and
/// `SaturatingMul` instead.
pub trait Saturating {
    /// Saturating addition operator.
    /// Returns a+b, saturating at the numeric bounds instead of overflowing.
    fn saturating_add(self, v: Self) -> Self;

    /// Saturating subtraction operator.
    /// Returns a-b, saturating at the numeric bounds instead of overflowing.
    fn saturating_sub(self, v: Self) -> Self;
}

macro_rules! deprecated_saturating_impl {
    ($trait_name:ident for $($t:ty)*) => {$(
        impl $trait_name for $t {
            #[inline]
            fn saturating_add(self, v: Self) -> Self {
                Self::saturating_add(self, v)
            }

            #[inline]
            fn saturating_sub(self, v: Self) -> Self {
                Self::saturating_sub(self, v)
            }
        }
    )*}
}

deprecated_saturating_impl!(Saturating for isize i8 i16 i32 i64 i128);
deprecated_saturating_impl!(Saturating for usize u8 u16 u32 u64 u128);

macro_rules! saturating_impl {
    ($trait_name:ident, $method:ident, $t:ty) => {
        impl $trait_name for $t {
            #[inline]
            fn $method(&self, v: &Self) -> Self {
                <$t>::$method(*self, *v)
            }
        }
    };
}

/// Performs addition that saturates at the numeric bounds instead of overflowing.
pub trait SaturatingAdd: Sized + Add<Self, Output = Self> {
    /// Saturating addition. Computes `self + other`, saturating at the relevant high or low boundary of
    /// the type.
    fn saturating_add(&self, v: &Self) -> Self;
}

saturating_impl!(SaturatingAdd, saturating_add, u8);
saturating_impl!(SaturatingAdd, saturating_add, u16);
saturating_impl!(SaturatingAdd, saturating_add, u32);
saturating_impl!(SaturatingAdd, saturating_add, u64);
saturating_impl!(SaturatingAdd, saturating_add, usize);
saturating_impl!(SaturatingAdd, saturating_add, u128);

saturating_impl!(SaturatingAdd, saturating_add, i8);
saturating_impl!(SaturatingAdd, saturating_add, i16);
saturating_impl!(SaturatingAdd, saturating_add, i32);
saturating_impl!(SaturatingAdd, saturating_add, i64);
saturating_impl!(SaturatingAdd, saturating_add, isize);
saturating_impl!(SaturatingAdd, saturating_add, i128);

/// Performs subtraction that saturates at the numeric bounds instead of overflowing.
pub trait SaturatingSub: Sized + Sub<Self, Output = Self> {
    /// Saturating subtraction. Computes `self - other`, saturating at the relevant high or low boundary of
    /// the type.
    fn saturating_sub(&self, v: &Self) -> Self;
}

saturating_impl!(SaturatingSub, saturating_sub, u8);
saturating_impl!(SaturatingSub, saturating_sub, u16);
saturating_impl!(SaturatingSub, saturating_sub, u32);
saturating_impl!(SaturatingSub, saturating_sub, u64);
saturating_impl!(SaturatingSub, saturating_sub, usize);
saturating_impl!(SaturatingSub, saturating_sub, u128);

saturating_impl!(SaturatingSub, saturating_sub, i8);
saturating_impl!(SaturatingSub, saturating_sub, i16);
saturating_impl!(SaturatingSub, saturating_sub, i32);
saturating_impl!(SaturatingSub, saturating_sub, i64);
saturating_impl!(SaturatingSub, saturating_sub, isize);
saturating_impl!(SaturatingSub, saturating_sub, i128);

/// Performs multiplication that saturates at the numeric bounds instead of overflowing.
pub trait SaturatingMul: Sized + Mul<Self, Output = Self> {
    /// Saturating multiplication. Computes `self * other`, saturating at the relevant high or low boundary of
    /// the type.
    fn saturating_mul(&self, v: &Self) -> Self;
}

saturating_impl!(SaturatingMul, saturating_mul, u8);
saturating_impl!(SaturatingMul, saturating_mul, u16);
saturating_impl!(SaturatingMul, saturating_mul, u32);
saturating_impl!(SaturatingMul, saturating_mul, u64);
saturating_impl!(SaturatingMul, saturating_mul, usize);
saturating_impl!(SaturatingMul, saturating_mul, u128);

saturating_impl!(SaturatingMul, saturating_mul, i8);
saturating_impl!(SaturatingMul, saturating_mul, i16);
saturating_impl!(SaturatingMul, saturating_mul, i32);
saturating_impl!(SaturatingMul, saturating_mul, i64);
saturating_impl!(SaturatingMul, saturating_mul, isize);
saturating_impl!(SaturatingMul, saturating_mul, i128);

// TODO: add SaturatingNeg for signed integer primitives once the saturating_neg() API is stable.

#[test]
fn test_saturating_traits() {
    fn saturating_add<T: SaturatingAdd>(a: T, b: T) -> T {
        a.saturating_add(&b)
    }
    fn saturating_sub<T: SaturatingSub>(a: T, b: T) -> T {
        a.saturating_sub(&b)
    }
    fn saturating_mul<T: SaturatingMul>(a: T, b: T) -> T {
        a.saturating_mul(&b)
    }
    assert_eq!(saturating_add(255, 1), 255u8);
    assert_eq!(saturating_add(127, 1), 127i8);
    assert_eq!(saturating_add(-128, -1), -128i8);
    assert_eq!(saturating_sub(0, 1), 0u8);
    assert_eq!(saturating_sub(-128, 1), -128i8);
    assert_eq!(saturating_sub(127, -1), 127i8);
    assert_eq!(saturating_mul(255, 2), 255u8);
    assert_eq!(saturating_mul(127, 2), 127i8);
    assert_eq!(saturating_mul(-128, 2), -128i8);
}
