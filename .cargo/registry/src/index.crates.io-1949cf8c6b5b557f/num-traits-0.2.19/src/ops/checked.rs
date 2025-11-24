use core::ops::{Add, Div, Mul, Rem, Shl, Shr, Sub};

/// Performs addition that returns `None` instead of wrapping around on
/// overflow.
pub trait CheckedAdd: Sized + Add<Self, Output = Self> {
    /// Adds two numbers, checking for overflow. If overflow happens, `None` is
    /// returned.
    fn checked_add(&self, v: &Self) -> Option<Self>;
}

macro_rules! checked_impl {
    ($trait_name:ident, $method:ident, $t:ty) => {
        impl $trait_name for $t {
            #[inline]
            fn $method(&self, v: &$t) -> Option<$t> {
                <$t>::$method(*self, *v)
            }
        }
    };
}

checked_impl!(CheckedAdd, checked_add, u8);
checked_impl!(CheckedAdd, checked_add, u16);
checked_impl!(CheckedAdd, checked_add, u32);
checked_impl!(CheckedAdd, checked_add, u64);
checked_impl!(CheckedAdd, checked_add, usize);
checked_impl!(CheckedAdd, checked_add, u128);

checked_impl!(CheckedAdd, checked_add, i8);
checked_impl!(CheckedAdd, checked_add, i16);
checked_impl!(CheckedAdd, checked_add, i32);
checked_impl!(CheckedAdd, checked_add, i64);
checked_impl!(CheckedAdd, checked_add, isize);
checked_impl!(CheckedAdd, checked_add, i128);

/// Performs subtraction that returns `None` instead of wrapping around on underflow.
pub trait CheckedSub: Sized + Sub<Self, Output = Self> {
    /// Subtracts two numbers, checking for underflow. If underflow happens,
    /// `None` is returned.
    fn checked_sub(&self, v: &Self) -> Option<Self>;
}

checked_impl!(CheckedSub, checked_sub, u8);
checked_impl!(CheckedSub, checked_sub, u16);
checked_impl!(CheckedSub, checked_sub, u32);
checked_impl!(CheckedSub, checked_sub, u64);
checked_impl!(CheckedSub, checked_sub, usize);
checked_impl!(CheckedSub, checked_sub, u128);

checked_impl!(CheckedSub, checked_sub, i8);
checked_impl!(CheckedSub, checked_sub, i16);
checked_impl!(CheckedSub, checked_sub, i32);
checked_impl!(CheckedSub, checked_sub, i64);
checked_impl!(CheckedSub, checked_sub, isize);
checked_impl!(CheckedSub, checked_sub, i128);

/// Performs multiplication that returns `None` instead of wrapping around on underflow or
/// overflow.
pub trait CheckedMul: Sized + Mul<Self, Output = Self> {
    /// Multiplies two numbers, checking for underflow or overflow. If underflow
    /// or overflow happens, `None` is returned.
    fn checked_mul(&self, v: &Self) -> Option<Self>;
}

checked_impl!(CheckedMul, checked_mul, u8);
checked_impl!(CheckedMul, checked_mul, u16);
checked_impl!(CheckedMul, checked_mul, u32);
checked_impl!(CheckedMul, checked_mul, u64);
checked_impl!(CheckedMul, checked_mul, usize);
checked_impl!(CheckedMul, checked_mul, u128);

checked_impl!(CheckedMul, checked_mul, i8);
checked_impl!(CheckedMul, checked_mul, i16);
checked_impl!(CheckedMul, checked_mul, i32);
checked_impl!(CheckedMul, checked_mul, i64);
checked_impl!(CheckedMul, checked_mul, isize);
checked_impl!(CheckedMul, checked_mul, i128);

/// Performs division that returns `None` instead of panicking on division by zero and instead of
/// wrapping around on underflow and overflow.
pub trait CheckedDiv: Sized + Div<Self, Output = Self> {
    /// Divides two numbers, checking for underflow, overflow and division by
    /// zero. If any of that happens, `None` is returned.
    fn checked_div(&self, v: &Self) -> Option<Self>;
}

checked_impl!(CheckedDiv, checked_div, u8);
checked_impl!(CheckedDiv, checked_div, u16);
checked_impl!(CheckedDiv, checked_div, u32);
checked_impl!(CheckedDiv, checked_div, u64);
checked_impl!(CheckedDiv, checked_div, usize);
checked_impl!(CheckedDiv, checked_div, u128);

checked_impl!(CheckedDiv, checked_div, i8);
checked_impl!(CheckedDiv, checked_div, i16);
checked_impl!(CheckedDiv, checked_div, i32);
checked_impl!(CheckedDiv, checked_div, i64);
checked_impl!(CheckedDiv, checked_div, isize);
checked_impl!(CheckedDiv, checked_div, i128);

/// Performs an integral remainder that returns `None` instead of panicking on division by zero and
/// instead of wrapping around on underflow and overflow.
pub trait CheckedRem: Sized + Rem<Self, Output = Self> {
    /// Finds the remainder of dividing two numbers, checking for underflow, overflow and division
    /// by zero. If any of that happens, `None` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::CheckedRem;
    /// use std::i32::MIN;
    ///
    /// assert_eq!(CheckedRem::checked_rem(&10, &7), Some(3));
    /// assert_eq!(CheckedRem::checked_rem(&10, &-7), Some(3));
    /// assert_eq!(CheckedRem::checked_rem(&-10, &7), Some(-3));
    /// assert_eq!(CheckedRem::checked_rem(&-10, &-7), Some(-3));
    ///
    /// assert_eq!(CheckedRem::checked_rem(&10, &0), None);
    ///
    /// assert_eq!(CheckedRem::checked_rem(&MIN, &1), Some(0));
    /// assert_eq!(CheckedRem::checked_rem(&MIN, &-1), None);
    /// ```
    fn checked_rem(&self, v: &Self) -> Option<Self>;
}

checked_impl!(CheckedRem, checked_rem, u8);
checked_impl!(CheckedRem, checked_rem, u16);
checked_impl!(CheckedRem, checked_rem, u32);
checked_impl!(CheckedRem, checked_rem, u64);
checked_impl!(CheckedRem, checked_rem, usize);
checked_impl!(CheckedRem, checked_rem, u128);

checked_impl!(CheckedRem, checked_rem, i8);
checked_impl!(CheckedRem, checked_rem, i16);
checked_impl!(CheckedRem, checked_rem, i32);
checked_impl!(CheckedRem, checked_rem, i64);
checked_impl!(CheckedRem, checked_rem, isize);
checked_impl!(CheckedRem, checked_rem, i128);

macro_rules! checked_impl_unary {
    ($trait_name:ident, $method:ident, $t:ty) => {
        impl $trait_name for $t {
            #[inline]
            fn $method(&self) -> Option<$t> {
                <$t>::$method(*self)
            }
        }
    };
}

/// Performs negation that returns `None` if the result can't be represented.
pub trait CheckedNeg: Sized {
    /// Negates a number, returning `None` for results that can't be represented, like signed `MIN`
    /// values that can't be positive, or non-zero unsigned values that can't be negative.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::CheckedNeg;
    /// use std::i32::MIN;
    ///
    /// assert_eq!(CheckedNeg::checked_neg(&1_i32), Some(-1));
    /// assert_eq!(CheckedNeg::checked_neg(&-1_i32), Some(1));
    /// assert_eq!(CheckedNeg::checked_neg(&MIN), None);
    ///
    /// assert_eq!(CheckedNeg::checked_neg(&0_u32), Some(0));
    /// assert_eq!(CheckedNeg::checked_neg(&1_u32), None);
    /// ```
    fn checked_neg(&self) -> Option<Self>;
}

checked_impl_unary!(CheckedNeg, checked_neg, u8);
checked_impl_unary!(CheckedNeg, checked_neg, u16);
checked_impl_unary!(CheckedNeg, checked_neg, u32);
checked_impl_unary!(CheckedNeg, checked_neg, u64);
checked_impl_unary!(CheckedNeg, checked_neg, usize);
checked_impl_unary!(CheckedNeg, checked_neg, u128);

checked_impl_unary!(CheckedNeg, checked_neg, i8);
checked_impl_unary!(CheckedNeg, checked_neg, i16);
checked_impl_unary!(CheckedNeg, checked_neg, i32);
checked_impl_unary!(CheckedNeg, checked_neg, i64);
checked_impl_unary!(CheckedNeg, checked_neg, isize);
checked_impl_unary!(CheckedNeg, checked_neg, i128);

/// Performs a left shift that returns `None` on shifts larger than
/// or equal to the type width.
pub trait CheckedShl: Sized + Shl<u32, Output = Self> {
    /// Checked shift left. Computes `self << rhs`, returning `None`
    /// if `rhs` is larger than or equal to the number of bits in `self`.
    ///
    /// ```
    /// use num_traits::CheckedShl;
    ///
    /// let x: u16 = 0x0001;
    ///
    /// assert_eq!(CheckedShl::checked_shl(&x, 0),  Some(0x0001));
    /// assert_eq!(CheckedShl::checked_shl(&x, 1),  Some(0x0002));
    /// assert_eq!(CheckedShl::checked_shl(&x, 15), Some(0x8000));
    /// assert_eq!(CheckedShl::checked_shl(&x, 16), None);
    /// ```
    fn checked_shl(&self, rhs: u32) -> Option<Self>;
}

macro_rules! checked_shift_impl {
    ($trait_name:ident, $method:ident, $t:ty) => {
        impl $trait_name for $t {
            #[inline]
            fn $method(&self, rhs: u32) -> Option<$t> {
                <$t>::$method(*self, rhs)
            }
        }
    };
}

checked_shift_impl!(CheckedShl, checked_shl, u8);
checked_shift_impl!(CheckedShl, checked_shl, u16);
checked_shift_impl!(CheckedShl, checked_shl, u32);
checked_shift_impl!(CheckedShl, checked_shl, u64);
checked_shift_impl!(CheckedShl, checked_shl, usize);
checked_shift_impl!(CheckedShl, checked_shl, u128);

checked_shift_impl!(CheckedShl, checked_shl, i8);
checked_shift_impl!(CheckedShl, checked_shl, i16);
checked_shift_impl!(CheckedShl, checked_shl, i32);
checked_shift_impl!(CheckedShl, checked_shl, i64);
checked_shift_impl!(CheckedShl, checked_shl, isize);
checked_shift_impl!(CheckedShl, checked_shl, i128);

/// Performs a right shift that returns `None` on shifts larger than
/// or equal to the type width.
pub trait CheckedShr: Sized + Shr<u32, Output = Self> {
    /// Checked shift right. Computes `self >> rhs`, returning `None`
    /// if `rhs` is larger than or equal to the number of bits in `self`.
    ///
    /// ```
    /// use num_traits::CheckedShr;
    ///
    /// let x: u16 = 0x8000;
    ///
    /// assert_eq!(CheckedShr::checked_shr(&x, 0),  Some(0x8000));
    /// assert_eq!(CheckedShr::checked_shr(&x, 1),  Some(0x4000));
    /// assert_eq!(CheckedShr::checked_shr(&x, 15), Some(0x0001));
    /// assert_eq!(CheckedShr::checked_shr(&x, 16), None);
    /// ```
    fn checked_shr(&self, rhs: u32) -> Option<Self>;
}

checked_shift_impl!(CheckedShr, checked_shr, u8);
checked_shift_impl!(CheckedShr, checked_shr, u16);
checked_shift_impl!(CheckedShr, checked_shr, u32);
checked_shift_impl!(CheckedShr, checked_shr, u64);
checked_shift_impl!(CheckedShr, checked_shr, usize);
checked_shift_impl!(CheckedShr, checked_shr, u128);

checked_shift_impl!(CheckedShr, checked_shr, i8);
checked_shift_impl!(CheckedShr, checked_shr, i16);
checked_shift_impl!(CheckedShr, checked_shr, i32);
checked_shift_impl!(CheckedShr, checked_shr, i64);
checked_shift_impl!(CheckedShr, checked_shr, isize);
checked_shift_impl!(CheckedShr, checked_shr, i128);
