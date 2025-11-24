//! Type-level unsigned integers.
//!
//!
//! **Type operators** implemented:
//!
//! From `::core::ops`: `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `Add`, `Sub`,
//!                 `Mul`, `Div`, and `Rem`.
//! From `typenum`: `Same`, `Cmp`, and `Pow`.
//!
//! Rather than directly using the structs defined in this module, it is recommended that
//! you import and use the relevant aliases from the [consts](../consts/index.html) module.
//!
//! # Example
//! ```rust
//! use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Rem, Shl, Shr, Sub};
//! use typenum::{Unsigned, U1, U2, U3, U4};
//!
//! assert_eq!(<U3 as BitAnd<U2>>::Output::to_u32(), 2);
//! assert_eq!(<U3 as BitOr<U4>>::Output::to_u32(), 7);
//! assert_eq!(<U3 as BitXor<U2>>::Output::to_u32(), 1);
//! assert_eq!(<U3 as Shl<U1>>::Output::to_u32(), 6);
//! assert_eq!(<U3 as Shr<U1>>::Output::to_u32(), 1);
//! assert_eq!(<U3 as Add<U2>>::Output::to_u32(), 5);
//! assert_eq!(<U3 as Sub<U2>>::Output::to_u32(), 1);
//! assert_eq!(<U3 as Mul<U2>>::Output::to_u32(), 6);
//! assert_eq!(<U3 as Div<U2>>::Output::to_u32(), 1);
//! assert_eq!(<U3 as Rem<U2>>::Output::to_u32(), 1);
//! ```

use crate::{
    bit::{Bit, B0, B1},
    consts::{U0, U1},
    private::{
        BitDiff, BitDiffOut, Internal, InternalMarker, PrivateAnd, PrivateAndOut, PrivateCmp,
        PrivateCmpOut, PrivateLogarithm2, PrivatePow, PrivatePowOut, PrivateSquareRoot, PrivateSub,
        PrivateSubOut, PrivateXor, PrivateXorOut, Trim, TrimOut,
    },
    Add1, Cmp, Double, Equal, Gcd, Gcf, GrEq, Greater, IsGreaterOrEqual, Len, Length, Less, Log2,
    Logarithm2, Maximum, Minimum, NonZero, Or, Ord, Pow, Prod, Shleft, Shright, Sqrt, Square,
    SquareRoot, Sub1, Sum, ToInt, Zero,
};
use core::ops::{Add, BitAnd, BitOr, BitXor, Mul, Shl, Shr, Sub};

pub use crate::marker_traits::{PowerOfTwo, Unsigned};

/// The terminating type for `UInt`; it always comes after the most significant
/// bit. `UTerm` by itself represents zero, which is aliased to `U0`.
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Default)]
#[cfg_attr(feature = "scale_info", derive(scale_info::TypeInfo))]
pub struct UTerm;

impl UTerm {
    /// Instantiates a singleton representing this unsigned integer.
    #[inline]
    pub fn new() -> UTerm {
        UTerm
    }
}

impl Unsigned for UTerm {
    const U8: u8 = 0;
    const U16: u16 = 0;
    const U32: u32 = 0;
    const U64: u64 = 0;
    #[cfg(feature = "i128")]
    const U128: u128 = 0;
    const USIZE: usize = 0;

    const I8: i8 = 0;
    const I16: i16 = 0;
    const I32: i32 = 0;
    const I64: i64 = 0;
    #[cfg(feature = "i128")]
    const I128: i128 = 0;
    const ISIZE: isize = 0;

    #[inline]
    fn to_u8() -> u8 {
        0
    }
    #[inline]
    fn to_u16() -> u16 {
        0
    }
    #[inline]
    fn to_u32() -> u32 {
        0
    }
    #[inline]
    fn to_u64() -> u64 {
        0
    }
    #[cfg(feature = "i128")]
    #[inline]
    fn to_u128() -> u128 {
        0
    }
    #[inline]
    fn to_usize() -> usize {
        0
    }

    #[inline]
    fn to_i8() -> i8 {
        0
    }
    #[inline]
    fn to_i16() -> i16 {
        0
    }
    #[inline]
    fn to_i32() -> i32 {
        0
    }
    #[inline]
    fn to_i64() -> i64 {
        0
    }
    #[cfg(feature = "i128")]
    #[inline]
    fn to_i128() -> i128 {
        0
    }
    #[inline]
    fn to_isize() -> isize {
        0
    }
}

/// `UInt` is defined recursively, where `B` is the least significant bit and `U` is the rest
/// of the number. Conceptually, `U` should be bound by the trait `Unsigned` and `B` should
/// be bound by the trait `Bit`, but enforcing these bounds causes linear instead of
/// logrithmic scaling in some places, so they are left off for now. They may be enforced in
/// future.
///
/// In order to keep numbers unique, leading zeros are not allowed, so `UInt<UTerm, B0>` is
/// forbidden.
///
/// # Example
/// ```rust
/// use typenum::{UInt, UTerm, B0, B1};
///
/// # #[allow(dead_code)]
/// type U6 = UInt<UInt<UInt<UTerm, B1>, B1>, B0>;
/// ```
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Default)]
#[cfg_attr(feature = "scale_info", derive(scale_info::TypeInfo))]
pub struct UInt<U, B> {
    /// The more significant bits of `Self`.
    pub(crate) msb: U,
    /// The least significant bit of `Self`.
    pub(crate) lsb: B,
}

impl<U: Unsigned, B: Bit> UInt<U, B> {
    /// Instantiates a singleton representing this unsigned integer.
    #[inline]
    pub fn new() -> UInt<U, B> {
        UInt::default()
    }
}

impl<U: Unsigned, B: Bit> Unsigned for UInt<U, B> {
    const U8: u8 = B::U8 | U::U8 << 1;
    const U16: u16 = B::U8 as u16 | U::U16 << 1;
    const U32: u32 = B::U8 as u32 | U::U32 << 1;
    const U64: u64 = B::U8 as u64 | U::U64 << 1;
    #[cfg(feature = "i128")]
    const U128: u128 = B::U8 as u128 | U::U128 << 1;
    const USIZE: usize = B::U8 as usize | U::USIZE << 1;

    const I8: i8 = B::U8 as i8 | U::I8 << 1;
    const I16: i16 = B::U8 as i16 | U::I16 << 1;
    const I32: i32 = B::U8 as i32 | U::I32 << 1;
    const I64: i64 = B::U8 as i64 | U::I64 << 1;
    #[cfg(feature = "i128")]
    const I128: i128 = B::U8 as i128 | U::I128 << 1;
    const ISIZE: isize = B::U8 as isize | U::ISIZE << 1;

    #[inline]
    fn to_u8() -> u8 {
        B::to_u8() | U::to_u8() << 1
    }
    #[inline]
    fn to_u16() -> u16 {
        u16::from(B::to_u8()) | U::to_u16() << 1
    }
    #[inline]
    fn to_u32() -> u32 {
        u32::from(B::to_u8()) | U::to_u32() << 1
    }
    #[inline]
    fn to_u64() -> u64 {
        u64::from(B::to_u8()) | U::to_u64() << 1
    }
    #[cfg(feature = "i128")]
    #[inline]
    fn to_u128() -> u128 {
        u128::from(B::to_u8()) | U::to_u128() << 1
    }
    #[inline]
    fn to_usize() -> usize {
        usize::from(B::to_u8()) | U::to_usize() << 1
    }

    #[inline]
    fn to_i8() -> i8 {
        B::to_u8() as i8 | U::to_i8() << 1
    }
    #[inline]
    fn to_i16() -> i16 {
        i16::from(B::to_u8()) | U::to_i16() << 1
    }
    #[inline]
    fn to_i32() -> i32 {
        i32::from(B::to_u8()) | U::to_i32() << 1
    }
    #[inline]
    fn to_i64() -> i64 {
        i64::from(B::to_u8()) | U::to_i64() << 1
    }
    #[cfg(feature = "i128")]
    #[inline]
    fn to_i128() -> i128 {
        i128::from(B::to_u8()) | U::to_i128() << 1
    }
    #[inline]
    fn to_isize() -> isize {
        B::to_u8() as isize | U::to_isize() << 1
    }
}

impl<U: Unsigned, B: Bit> NonZero for UInt<U, B> {}
impl Zero for UTerm {}

impl PowerOfTwo for UInt<UTerm, B1> {}
impl<U: Unsigned + PowerOfTwo> PowerOfTwo for UInt<U, B0> {}

// ---------------------------------------------------------------------------------------
// Getting length of unsigned integers, which is defined as the number of bits before `UTerm`

/// Length of `UTerm` by itself is 0
impl Len for UTerm {
    type Output = U0;
    #[inline]
    fn len(&self) -> Self::Output {
        UTerm
    }
}

/// Length of a bit is 1
impl<U: Unsigned, B: Bit> Len for UInt<U, B>
where
    U: Len,
    Length<U>: Add<B1>,
    Add1<Length<U>>: Unsigned,
{
    type Output = Add1<Length<U>>;
    #[inline]
    fn len(&self) -> Self::Output {
        self.msb.len() + B1
    }
}

// ---------------------------------------------------------------------------------------
// Adding bits to unsigned integers

/// `UTerm + B0 = UTerm`
impl Add<B0> for UTerm {
    type Output = UTerm;
    #[inline]
    fn add(self, _: B0) -> Self::Output {
        UTerm
    }
}

/// `U + B0 = U`
impl<U: Unsigned, B: Bit> Add<B0> for UInt<U, B> {
    type Output = UInt<U, B>;
    #[inline]
    fn add(self, _: B0) -> Self::Output {
        UInt::new()
    }
}

/// `UTerm + B1 = UInt<UTerm, B1>`
impl Add<B1> for UTerm {
    type Output = UInt<UTerm, B1>;
    #[inline]
    fn add(self, _: B1) -> Self::Output {
        UInt::new()
    }
}

/// `UInt<U, B0> + B1 = UInt<U + B1>`
impl<U: Unsigned> Add<B1> for UInt<U, B0> {
    type Output = UInt<U, B1>;
    #[inline]
    fn add(self, _: B1) -> Self::Output {
        UInt::new()
    }
}

/// `UInt<U, B1> + B1 = UInt<U + B1, B0>`
impl<U: Unsigned> Add<B1> for UInt<U, B1>
where
    U: Add<B1>,
    Add1<U>: Unsigned,
{
    type Output = UInt<Add1<U>, B0>;
    #[inline]
    fn add(self, _: B1) -> Self::Output {
        UInt::new()
    }
}

// ---------------------------------------------------------------------------------------
// Adding unsigned integers

/// `UTerm + U = U`
impl<U: Unsigned> Add<U> for UTerm {
    type Output = U;
    #[inline]
    fn add(self, rhs: U) -> Self::Output {
        rhs
    }
}

/// `UInt<U, B> + UTerm = UInt<U, B>`
impl<U: Unsigned, B: Bit> Add<UTerm> for UInt<U, B> {
    type Output = UInt<U, B>;
    #[inline]
    fn add(self, _: UTerm) -> Self::Output {
        UInt::new()
    }
}

/// `UInt<Ul, B0> + UInt<Ur, B0> = UInt<Ul + Ur, B0>`
impl<Ul: Unsigned, Ur: Unsigned> Add<UInt<Ur, B0>> for UInt<Ul, B0>
where
    Ul: Add<Ur>,
{
    type Output = UInt<Sum<Ul, Ur>, B0>;
    #[inline]
    fn add(self, rhs: UInt<Ur, B0>) -> Self::Output {
        UInt {
            msb: self.msb + rhs.msb,
            lsb: B0,
        }
    }
}

/// `UInt<Ul, B0> + UInt<Ur, B1> = UInt<Ul + Ur, B1>`
impl<Ul: Unsigned, Ur: Unsigned> Add<UInt<Ur, B1>> for UInt<Ul, B0>
where
    Ul: Add<Ur>,
{
    type Output = UInt<Sum<Ul, Ur>, B1>;
    #[inline]
    fn add(self, rhs: UInt<Ur, B1>) -> Self::Output {
        UInt {
            msb: self.msb + rhs.msb,
            lsb: B1,
        }
    }
}

/// `UInt<Ul, B1> + UInt<Ur, B0> = UInt<Ul + Ur, B1>`
impl<Ul: Unsigned, Ur: Unsigned> Add<UInt<Ur, B0>> for UInt<Ul, B1>
where
    Ul: Add<Ur>,
{
    type Output = UInt<Sum<Ul, Ur>, B1>;
    #[inline]
    fn add(self, rhs: UInt<Ur, B0>) -> Self::Output {
        UInt {
            msb: self.msb + rhs.msb,
            lsb: B1,
        }
    }
}

/// `UInt<Ul, B1> + UInt<Ur, B1> = UInt<(Ul + Ur) + B1, B0>`
impl<Ul: Unsigned, Ur: Unsigned> Add<UInt<Ur, B1>> for UInt<Ul, B1>
where
    Ul: Add<Ur>,
    Sum<Ul, Ur>: Add<B1>,
{
    type Output = UInt<Add1<Sum<Ul, Ur>>, B0>;
    #[inline]
    fn add(self, rhs: UInt<Ur, B1>) -> Self::Output {
        UInt {
            msb: self.msb + rhs.msb + B1,
            lsb: B0,
        }
    }
}

// ---------------------------------------------------------------------------------------
// Subtracting bits from unsigned integers

/// `UTerm - B0 = Term`
impl Sub<B0> for UTerm {
    type Output = UTerm;
    #[inline]
    fn sub(self, _: B0) -> Self::Output {
        UTerm
    }
}

/// `UInt - B0 = UInt`
impl<U: Unsigned, B: Bit> Sub<B0> for UInt<U, B> {
    type Output = UInt<U, B>;
    #[inline]
    fn sub(self, _: B0) -> Self::Output {
        UInt::new()
    }
}

/// `UInt<U, B1> - B1 = UInt<U, B0>`
impl<U: Unsigned, B: Bit> Sub<B1> for UInt<UInt<U, B>, B1> {
    type Output = UInt<UInt<U, B>, B0>;
    #[inline]
    fn sub(self, _: B1) -> Self::Output {
        UInt::new()
    }
}

/// `UInt<UTerm, B1> - B1 = UTerm`
impl Sub<B1> for UInt<UTerm, B1> {
    type Output = UTerm;
    #[inline]
    fn sub(self, _: B1) -> Self::Output {
        UTerm
    }
}

/// `UInt<U, B0> - B1 = UInt<U - B1, B1>`
impl<U: Unsigned> Sub<B1> for UInt<U, B0>
where
    U: Sub<B1>,
    Sub1<U>: Unsigned,
{
    type Output = UInt<Sub1<U>, B1>;
    #[inline]
    fn sub(self, _: B1) -> Self::Output {
        UInt::new()
    }
}

// ---------------------------------------------------------------------------------------
// Subtracting unsigned integers

/// `UTerm - UTerm = UTerm`
impl Sub<UTerm> for UTerm {
    type Output = UTerm;
    #[inline]
    fn sub(self, _: UTerm) -> Self::Output {
        UTerm
    }
}

/// Subtracting unsigned integers. We just do our `PrivateSub` and then `Trim` the output.
impl<Ul: Unsigned, Bl: Bit, Ur: Unsigned> Sub<Ur> for UInt<Ul, Bl>
where
    UInt<Ul, Bl>: PrivateSub<Ur>,
    PrivateSubOut<UInt<Ul, Bl>, Ur>: Trim,
{
    type Output = TrimOut<PrivateSubOut<UInt<Ul, Bl>, Ur>>;
    #[inline]
    fn sub(self, rhs: Ur) -> Self::Output {
        self.private_sub(rhs).trim()
    }
}

/// `U - UTerm = U`
impl<U: Unsigned> PrivateSub<UTerm> for U {
    type Output = U;

    #[inline]
    fn private_sub(self, _: UTerm) -> Self::Output {
        self
    }
}

/// `UInt<Ul, B0> - UInt<Ur, B0> = UInt<Ul - Ur, B0>`
impl<Ul: Unsigned, Ur: Unsigned> PrivateSub<UInt<Ur, B0>> for UInt<Ul, B0>
where
    Ul: PrivateSub<Ur>,
{
    type Output = UInt<PrivateSubOut<Ul, Ur>, B0>;

    #[inline]
    fn private_sub(self, rhs: UInt<Ur, B0>) -> Self::Output {
        UInt {
            msb: self.msb.private_sub(rhs.msb),
            lsb: B0,
        }
    }
}

/// `UInt<Ul, B0> - UInt<Ur, B1> = UInt<(Ul - Ur) - B1, B1>`
impl<Ul: Unsigned, Ur: Unsigned> PrivateSub<UInt<Ur, B1>> for UInt<Ul, B0>
where
    Ul: PrivateSub<Ur>,
    PrivateSubOut<Ul, Ur>: Sub<B1>,
{
    type Output = UInt<Sub1<PrivateSubOut<Ul, Ur>>, B1>;

    #[inline]
    fn private_sub(self, rhs: UInt<Ur, B1>) -> Self::Output {
        UInt {
            msb: self.msb.private_sub(rhs.msb) - B1,
            lsb: B1,
        }
    }
}

/// `UInt<Ul, B1> - UInt<Ur, B0> = UInt<Ul - Ur, B1>`
impl<Ul: Unsigned, Ur: Unsigned> PrivateSub<UInt<Ur, B0>> for UInt<Ul, B1>
where
    Ul: PrivateSub<Ur>,
{
    type Output = UInt<PrivateSubOut<Ul, Ur>, B1>;

    #[inline]
    fn private_sub(self, rhs: UInt<Ur, B0>) -> Self::Output {
        UInt {
            msb: self.msb.private_sub(rhs.msb),
            lsb: B1,
        }
    }
}

/// `UInt<Ul, B1> - UInt<Ur, B1> = UInt<Ul - Ur, B0>`
impl<Ul: Unsigned, Ur: Unsigned> PrivateSub<UInt<Ur, B1>> for UInt<Ul, B1>
where
    Ul: PrivateSub<Ur>,
{
    type Output = UInt<PrivateSubOut<Ul, Ur>, B0>;

    #[inline]
    fn private_sub(self, rhs: UInt<Ur, B1>) -> Self::Output {
        UInt {
            msb: self.msb.private_sub(rhs.msb),
            lsb: B0,
        }
    }
}

// ---------------------------------------------------------------------------------------
// And unsigned integers

/// 0 & X = 0
impl<Ur: Unsigned> BitAnd<Ur> for UTerm {
    type Output = UTerm;
    #[inline]
    fn bitand(self, _: Ur) -> Self::Output {
        UTerm
    }
}

/// Anding unsigned integers.
/// We use our `PrivateAnd` operator and then `Trim` the output.
impl<Ul: Unsigned, Bl: Bit, Ur: Unsigned> BitAnd<Ur> for UInt<Ul, Bl>
where
    UInt<Ul, Bl>: PrivateAnd<Ur>,
    PrivateAndOut<UInt<Ul, Bl>, Ur>: Trim,
{
    type Output = TrimOut<PrivateAndOut<UInt<Ul, Bl>, Ur>>;
    #[inline]
    fn bitand(self, rhs: Ur) -> Self::Output {
        self.private_and(rhs).trim()
    }
}

/// `UTerm & X = UTerm`
impl<U: Unsigned> PrivateAnd<U> for UTerm {
    type Output = UTerm;

    #[inline]
    fn private_and(self, _: U) -> Self::Output {
        UTerm
    }
}

/// `X & UTerm = UTerm`
impl<B: Bit, U: Unsigned> PrivateAnd<UTerm> for UInt<U, B> {
    type Output = UTerm;

    #[inline]
    fn private_and(self, _: UTerm) -> Self::Output {
        UTerm
    }
}

/// `UInt<Ul, B0> & UInt<Ur, B0> = UInt<Ul & Ur, B0>`
impl<Ul: Unsigned, Ur: Unsigned> PrivateAnd<UInt<Ur, B0>> for UInt<Ul, B0>
where
    Ul: PrivateAnd<Ur>,
{
    type Output = UInt<PrivateAndOut<Ul, Ur>, B0>;

    #[inline]
    fn private_and(self, rhs: UInt<Ur, B0>) -> Self::Output {
        UInt {
            msb: self.msb.private_and(rhs.msb),
            lsb: B0,
        }
    }
}

/// `UInt<Ul, B0> & UInt<Ur, B1> = UInt<Ul & Ur, B0>`
impl<Ul: Unsigned, Ur: Unsigned> PrivateAnd<UInt<Ur, B1>> for UInt<Ul, B0>
where
    Ul: PrivateAnd<Ur>,
{
    type Output = UInt<PrivateAndOut<Ul, Ur>, B0>;

    #[inline]
    fn private_and(self, rhs: UInt<Ur, B1>) -> Self::Output {
        UInt {
            msb: self.msb.private_and(rhs.msb),
            lsb: B0,
        }
    }
}

/// `UInt<Ul, B1> & UInt<Ur, B0> = UInt<Ul & Ur, B0>`
impl<Ul: Unsigned, Ur: Unsigned> PrivateAnd<UInt<Ur, B0>> for UInt<Ul, B1>
where
    Ul: PrivateAnd<Ur>,
{
    type Output = UInt<PrivateAndOut<Ul, Ur>, B0>;

    #[inline]
    fn private_and(self, rhs: UInt<Ur, B0>) -> Self::Output {
        UInt {
            msb: self.msb.private_and(rhs.msb),
            lsb: B0,
        }
    }
}

/// `UInt<Ul, B1> & UInt<Ur, B1> = UInt<Ul & Ur, B1>`
impl<Ul: Unsigned, Ur: Unsigned> PrivateAnd<UInt<Ur, B1>> for UInt<Ul, B1>
where
    Ul: PrivateAnd<Ur>,
{
    type Output = UInt<PrivateAndOut<Ul, Ur>, B1>;

    #[inline]
    fn private_and(self, rhs: UInt<Ur, B1>) -> Self::Output {
        UInt {
            msb: self.msb.private_and(rhs.msb),
            lsb: B1,
        }
    }
}

// ---------------------------------------------------------------------------------------
// Or unsigned integers

/// `UTerm | X = X`
impl<U: Unsigned> BitOr<U> for UTerm {
    type Output = U;
    #[inline]
    fn bitor(self, rhs: U) -> Self::Output {
        rhs
    }
}

///  `X | UTerm = X`
impl<B: Bit, U: Unsigned> BitOr<UTerm> for UInt<U, B> {
    type Output = Self;
    #[inline]
    fn bitor(self, _: UTerm) -> Self::Output {
        UInt::new()
    }
}

/// `UInt<Ul, B0> | UInt<Ur, B0> = UInt<Ul | Ur, B0>`
impl<Ul: Unsigned, Ur: Unsigned> BitOr<UInt<Ur, B0>> for UInt<Ul, B0>
where
    Ul: BitOr<Ur>,
{
    type Output = UInt<<Ul as BitOr<Ur>>::Output, B0>;
    #[inline]
    fn bitor(self, rhs: UInt<Ur, B0>) -> Self::Output {
        UInt {
            msb: self.msb.bitor(rhs.msb),
            lsb: B0,
        }
    }
}

/// `UInt<Ul, B0> | UInt<Ur, B1> = UInt<Ul | Ur, B1>`
impl<Ul: Unsigned, Ur: Unsigned> BitOr<UInt<Ur, B1>> for UInt<Ul, B0>
where
    Ul: BitOr<Ur>,
{
    type Output = UInt<Or<Ul, Ur>, B1>;
    #[inline]
    fn bitor(self, rhs: UInt<Ur, B1>) -> Self::Output {
        UInt {
            msb: self.msb.bitor(rhs.msb),
            lsb: self.lsb.bitor(rhs.lsb),
        }
    }
}

/// `UInt<Ul, B1> | UInt<Ur, B0> = UInt<Ul | Ur, B1>`
impl<Ul: Unsigned, Ur: Unsigned> BitOr<UInt<Ur, B0>> for UInt<Ul, B1>
where
    Ul: BitOr<Ur>,
{
    type Output = UInt<Or<Ul, Ur>, B1>;
    #[inline]
    fn bitor(self, rhs: UInt<Ur, B0>) -> Self::Output {
        UInt {
            msb: self.msb.bitor(rhs.msb),
            lsb: self.lsb.bitor(rhs.lsb),
        }
    }
}

/// `UInt<Ul, B1> | UInt<Ur, B1> = UInt<Ul | Ur, B1>`
impl<Ul: Unsigned, Ur: Unsigned> BitOr<UInt<Ur, B1>> for UInt<Ul, B1>
where
    Ul: BitOr<Ur>,
{
    type Output = UInt<Or<Ul, Ur>, B1>;
    #[inline]
    fn bitor(self, rhs: UInt<Ur, B1>) -> Self::Output {
        UInt {
            msb: self.msb.bitor(rhs.msb),
            lsb: self.lsb.bitor(rhs.lsb),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Xor unsigned integers

/// 0 ^ X = X
impl<Ur: Unsigned> BitXor<Ur> for UTerm {
    type Output = Ur;
    #[inline]
    fn bitxor(self, rhs: Ur) -> Self::Output {
        rhs
    }
}

/// Xoring unsigned integers.
/// We use our `PrivateXor` operator and then `Trim` the output.
impl<Ul: Unsigned, Bl: Bit, Ur: Unsigned> BitXor<Ur> for UInt<Ul, Bl>
where
    UInt<Ul, Bl>: PrivateXor<Ur>,
    PrivateXorOut<UInt<Ul, Bl>, Ur>: Trim,
{
    type Output = TrimOut<PrivateXorOut<UInt<Ul, Bl>, Ur>>;
    #[inline]
    fn bitxor(self, rhs: Ur) -> Self::Output {
        self.private_xor(rhs).trim()
    }
}

/// `UTerm ^ X = X`
impl<U: Unsigned> PrivateXor<U> for UTerm {
    type Output = U;

    #[inline]
    fn private_xor(self, rhs: U) -> Self::Output {
        rhs
    }
}

/// `X ^ UTerm = X`
impl<B: Bit, U: Unsigned> PrivateXor<UTerm> for UInt<U, B> {
    type Output = Self;

    #[inline]
    fn private_xor(self, _: UTerm) -> Self::Output {
        self
    }
}

/// `UInt<Ul, B0> ^ UInt<Ur, B0> = UInt<Ul ^ Ur, B0>`
impl<Ul: Unsigned, Ur: Unsigned> PrivateXor<UInt<Ur, B0>> for UInt<Ul, B0>
where
    Ul: PrivateXor<Ur>,
{
    type Output = UInt<PrivateXorOut<Ul, Ur>, B0>;

    #[inline]
    fn private_xor(self, rhs: UInt<Ur, B0>) -> Self::Output {
        UInt {
            msb: self.msb.private_xor(rhs.msb),
            lsb: B0,
        }
    }
}

/// `UInt<Ul, B0> ^ UInt<Ur, B1> = UInt<Ul ^ Ur, B1>`
impl<Ul: Unsigned, Ur: Unsigned> PrivateXor<UInt<Ur, B1>> for UInt<Ul, B0>
where
    Ul: PrivateXor<Ur>,
{
    type Output = UInt<PrivateXorOut<Ul, Ur>, B1>;

    #[inline]
    fn private_xor(self, rhs: UInt<Ur, B1>) -> Self::Output {
        UInt {
            msb: self.msb.private_xor(rhs.msb),
            lsb: B1,
        }
    }
}

/// `UInt<Ul, B1> ^ UInt<Ur, B0> = UInt<Ul ^ Ur, B1>`
impl<Ul: Unsigned, Ur: Unsigned> PrivateXor<UInt<Ur, B0>> for UInt<Ul, B1>
where
    Ul: PrivateXor<Ur>,
{
    type Output = UInt<PrivateXorOut<Ul, Ur>, B1>;

    #[inline]
    fn private_xor(self, rhs: UInt<Ur, B0>) -> Self::Output {
        UInt {
            msb: self.msb.private_xor(rhs.msb),
            lsb: B1,
        }
    }
}

/// `UInt<Ul, B1> ^ UInt<Ur, B1> = UInt<Ul ^ Ur, B0>`
impl<Ul: Unsigned, Ur: Unsigned> PrivateXor<UInt<Ur, B1>> for UInt<Ul, B1>
where
    Ul: PrivateXor<Ur>,
{
    type Output = UInt<PrivateXorOut<Ul, Ur>, B0>;

    #[inline]
    fn private_xor(self, rhs: UInt<Ur, B1>) -> Self::Output {
        UInt {
            msb: self.msb.private_xor(rhs.msb),
            lsb: B0,
        }
    }
}

// ---------------------------------------------------------------------------------------
// Shl unsigned integers

/// Shifting `UTerm` by a 0 bit: `UTerm << B0 = UTerm`
impl Shl<B0> for UTerm {
    type Output = UTerm;
    #[inline]
    fn shl(self, _: B0) -> Self::Output {
        UTerm
    }
}

/// Shifting `UTerm` by a 1 bit: `UTerm << B1 = UTerm`
impl Shl<B1> for UTerm {
    type Output = UTerm;
    #[inline]
    fn shl(self, _: B1) -> Self::Output {
        UTerm
    }
}

/// Shifting left any unsigned by a zero bit: `U << B0 = U`
impl<U: Unsigned, B: Bit> Shl<B0> for UInt<U, B> {
    type Output = UInt<U, B>;
    #[inline]
    fn shl(self, _: B0) -> Self::Output {
        UInt::new()
    }
}

/// Shifting left a `UInt` by a one bit: `UInt<U, B> << B1 = UInt<UInt<U, B>, B0>`
impl<U: Unsigned, B: Bit> Shl<B1> for UInt<U, B> {
    type Output = UInt<UInt<U, B>, B0>;
    #[inline]
    fn shl(self, _: B1) -> Self::Output {
        UInt::new()
    }
}

/// Shifting left `UInt` by `UTerm`: `UInt<U, B> << UTerm = UInt<U, B>`
impl<U: Unsigned, B: Bit> Shl<UTerm> for UInt<U, B> {
    type Output = UInt<U, B>;
    #[inline]
    fn shl(self, _: UTerm) -> Self::Output {
        UInt::new()
    }
}

/// Shifting left `UTerm` by an unsigned integer: `UTerm << U = UTerm`
impl<U: Unsigned> Shl<U> for UTerm {
    type Output = UTerm;
    #[inline]
    fn shl(self, _: U) -> Self::Output {
        UTerm
    }
}

/// Shifting left `UInt` by `UInt`: `X << Y` = `UInt(X, B0) << (Y - 1)`
impl<U: Unsigned, B: Bit, Ur: Unsigned, Br: Bit> Shl<UInt<Ur, Br>> for UInt<U, B>
where
    UInt<Ur, Br>: Sub<B1>,
    UInt<UInt<U, B>, B0>: Shl<Sub1<UInt<Ur, Br>>>,
{
    type Output = Shleft<UInt<UInt<U, B>, B0>, Sub1<UInt<Ur, Br>>>;
    #[inline]
    fn shl(self, rhs: UInt<Ur, Br>) -> Self::Output {
        #[allow(clippy::suspicious_arithmetic_impl)]
        (UInt { msb: self, lsb: B0 }).shl(rhs - B1)
    }
}

// ---------------------------------------------------------------------------------------
// Shr unsigned integers

/// Shifting right a `UTerm` by an unsigned integer: `UTerm >> U = UTerm`
impl<U: Unsigned> Shr<U> for UTerm {
    type Output = UTerm;
    #[inline]
    fn shr(self, _: U) -> Self::Output {
        UTerm
    }
}

/// Shifting right `UInt` by `UTerm`: `UInt<U, B> >> UTerm = UInt<U, B>`
impl<U: Unsigned, B: Bit> Shr<UTerm> for UInt<U, B> {
    type Output = UInt<U, B>;
    #[inline]
    fn shr(self, _: UTerm) -> Self::Output {
        UInt::new()
    }
}

/// Shifting right `UTerm` by a 0 bit: `UTerm >> B0 = UTerm`
impl Shr<B0> for UTerm {
    type Output = UTerm;
    #[inline]
    fn shr(self, _: B0) -> Self::Output {
        UTerm
    }
}

/// Shifting right `UTerm` by a 1 bit: `UTerm >> B1 = UTerm`
impl Shr<B1> for UTerm {
    type Output = UTerm;
    #[inline]
    fn shr(self, _: B1) -> Self::Output {
        UTerm
    }
}

/// Shifting right any unsigned by a zero bit: `U >> B0 = U`
impl<U: Unsigned, B: Bit> Shr<B0> for UInt<U, B> {
    type Output = UInt<U, B>;
    #[inline]
    fn shr(self, _: B0) -> Self::Output {
        UInt::new()
    }
}

/// Shifting right a `UInt` by a 1 bit: `UInt<U, B> >> B1 = U`
impl<U: Unsigned, B: Bit> Shr<B1> for UInt<U, B> {
    type Output = U;
    #[inline]
    fn shr(self, _: B1) -> Self::Output {
        self.msb
    }
}

/// Shifting right `UInt` by `UInt`: `UInt(U, B) >> Y` = `U >> (Y - 1)`
impl<U: Unsigned, B: Bit, Ur: Unsigned, Br: Bit> Shr<UInt<Ur, Br>> for UInt<U, B>
where
    UInt<Ur, Br>: Sub<B1>,
    U: Shr<Sub1<UInt<Ur, Br>>>,
{
    type Output = Shright<U, Sub1<UInt<Ur, Br>>>;
    #[inline]
    fn shr(self, rhs: UInt<Ur, Br>) -> Self::Output {
        #[allow(clippy::suspicious_arithmetic_impl)]
        self.msb.shr(rhs - B1)
    }
}

// ---------------------------------------------------------------------------------------
// Multiply unsigned integers

/// `UInt * B0 = UTerm`
impl<U: Unsigned, B: Bit> Mul<B0> for UInt<U, B> {
    type Output = UTerm;
    #[inline]
    fn mul(self, _: B0) -> Self::Output {
        UTerm
    }
}

/// `UTerm * B0 = UTerm`
impl Mul<B0> for UTerm {
    type Output = UTerm;
    #[inline]
    fn mul(self, _: B0) -> Self::Output {
        UTerm
    }
}

/// `UTerm * B1 = UTerm`
impl Mul<B1> for UTerm {
    type Output = UTerm;
    #[inline]
    fn mul(self, _: B1) -> Self::Output {
        UTerm
    }
}

/// `UInt * B1 = UInt`
impl<U: Unsigned, B: Bit> Mul<B1> for UInt<U, B> {
    type Output = UInt<U, B>;
    #[inline]
    fn mul(self, _: B1) -> Self::Output {
        UInt::new()
    }
}

/// `UInt<U, B> * UTerm = UTerm`
impl<U: Unsigned, B: Bit> Mul<UTerm> for UInt<U, B> {
    type Output = UTerm;
    #[inline]
    fn mul(self, _: UTerm) -> Self::Output {
        UTerm
    }
}

/// `UTerm * U = UTerm`
impl<U: Unsigned> Mul<U> for UTerm {
    type Output = UTerm;
    #[inline]
    fn mul(self, _: U) -> Self::Output {
        UTerm
    }
}

/// `UInt<Ul, B0> * UInt<Ur, B> = UInt<(Ul * UInt<Ur, B>), B0>`
impl<Ul: Unsigned, B: Bit, Ur: Unsigned> Mul<UInt<Ur, B>> for UInt<Ul, B0>
where
    Ul: Mul<UInt<Ur, B>>,
{
    type Output = UInt<Prod<Ul, UInt<Ur, B>>, B0>;
    #[inline]
    fn mul(self, rhs: UInt<Ur, B>) -> Self::Output {
        UInt {
            msb: self.msb * rhs,
            lsb: B0,
        }
    }
}

/// `UInt<Ul, B1> * UInt<Ur, B> = UInt<(Ul * UInt<Ur, B>), B0> + UInt<Ur, B>`
impl<Ul: Unsigned, B: Bit, Ur: Unsigned> Mul<UInt<Ur, B>> for UInt<Ul, B1>
where
    Ul: Mul<UInt<Ur, B>>,
    UInt<Prod<Ul, UInt<Ur, B>>, B0>: Add<UInt<Ur, B>>,
{
    type Output = Sum<UInt<Prod<Ul, UInt<Ur, B>>, B0>, UInt<Ur, B>>;
    #[inline]
    fn mul(self, rhs: UInt<Ur, B>) -> Self::Output {
        UInt {
            msb: self.msb * rhs,
            lsb: B0,
        } + rhs
    }
}

// ---------------------------------------------------------------------------------------
// Compare unsigned integers

/// Zero == Zero
impl Cmp<UTerm> for UTerm {
    type Output = Equal;

    #[inline]
    fn compare<IM: InternalMarker>(&self, _: &UTerm) -> Self::Output {
        Equal
    }
}

/// Nonzero > Zero
impl<U: Unsigned, B: Bit> Cmp<UTerm> for UInt<U, B> {
    type Output = Greater;

    #[inline]
    fn compare<IM: InternalMarker>(&self, _: &UTerm) -> Self::Output {
        Greater
    }
}

/// Zero < Nonzero
impl<U: Unsigned, B: Bit> Cmp<UInt<U, B>> for UTerm {
    type Output = Less;

    #[inline]
    fn compare<IM: InternalMarker>(&self, _: &UInt<U, B>) -> Self::Output {
        Less
    }
}

/// `UInt<Ul, B0>` cmp with `UInt<Ur, B0>`: `SoFar` is `Equal`
impl<Ul: Unsigned, Ur: Unsigned> Cmp<UInt<Ur, B0>> for UInt<Ul, B0>
where
    Ul: PrivateCmp<Ur, Equal>,
{
    type Output = PrivateCmpOut<Ul, Ur, Equal>;

    #[inline]
    fn compare<IM: InternalMarker>(&self, rhs: &UInt<Ur, B0>) -> Self::Output {
        self.msb.private_cmp(&rhs.msb, Equal)
    }
}

/// `UInt<Ul, B1>` cmp with `UInt<Ur, B1>`: `SoFar` is `Equal`
impl<Ul: Unsigned, Ur: Unsigned> Cmp<UInt<Ur, B1>> for UInt<Ul, B1>
where
    Ul: PrivateCmp<Ur, Equal>,
{
    type Output = PrivateCmpOut<Ul, Ur, Equal>;

    #[inline]
    fn compare<IM: InternalMarker>(&self, rhs: &UInt<Ur, B1>) -> Self::Output {
        self.msb.private_cmp(&rhs.msb, Equal)
    }
}

/// `UInt<Ul, B0>` cmp with `UInt<Ur, B1>`: `SoFar` is `Less`
impl<Ul: Unsigned, Ur: Unsigned> Cmp<UInt<Ur, B1>> for UInt<Ul, B0>
where
    Ul: PrivateCmp<Ur, Less>,
{
    type Output = PrivateCmpOut<Ul, Ur, Less>;

    #[inline]
    fn compare<IM: InternalMarker>(&self, rhs: &UInt<Ur, B1>) -> Self::Output {
        self.msb.private_cmp(&rhs.msb, Less)
    }
}

/// `UInt<Ul, B1>` cmp with `UInt<Ur, B0>`: `SoFar` is `Greater`
impl<Ul: Unsigned, Ur: Unsigned> Cmp<UInt<Ur, B0>> for UInt<Ul, B1>
where
    Ul: PrivateCmp<Ur, Greater>,
{
    type Output = PrivateCmpOut<Ul, Ur, Greater>;

    #[inline]
    fn compare<IM: InternalMarker>(&self, rhs: &UInt<Ur, B0>) -> Self::Output {
        self.msb.private_cmp(&rhs.msb, Greater)
    }
}

/// Comparing non-terimal bits, with both having bit `B0`.
/// These are `Equal`, so we propagate `SoFar`.
impl<Ul, Ur, SoFar> PrivateCmp<UInt<Ur, B0>, SoFar> for UInt<Ul, B0>
where
    Ul: Unsigned,
    Ur: Unsigned,
    SoFar: Ord,
    Ul: PrivateCmp<Ur, SoFar>,
{
    type Output = PrivateCmpOut<Ul, Ur, SoFar>;

    #[inline]
    fn private_cmp(&self, rhs: &UInt<Ur, B0>, so_far: SoFar) -> Self::Output {
        self.msb.private_cmp(&rhs.msb, so_far)
    }
}

/// Comparing non-terimal bits, with both having bit `B1`.
/// These are `Equal`, so we propagate `SoFar`.
impl<Ul, Ur, SoFar> PrivateCmp<UInt<Ur, B1>, SoFar> for UInt<Ul, B1>
where
    Ul: Unsigned,
    Ur: Unsigned,
    SoFar: Ord,
    Ul: PrivateCmp<Ur, SoFar>,
{
    type Output = PrivateCmpOut<Ul, Ur, SoFar>;

    #[inline]
    fn private_cmp(&self, rhs: &UInt<Ur, B1>, so_far: SoFar) -> Self::Output {
        self.msb.private_cmp(&rhs.msb, so_far)
    }
}

/// Comparing non-terimal bits, with `Lhs` having bit `B0` and `Rhs` having bit `B1`.
/// `SoFar`, Lhs is `Less`.
impl<Ul, Ur, SoFar> PrivateCmp<UInt<Ur, B1>, SoFar> for UInt<Ul, B0>
where
    Ul: Unsigned,
    Ur: Unsigned,
    SoFar: Ord,
    Ul: PrivateCmp<Ur, Less>,
{
    type Output = PrivateCmpOut<Ul, Ur, Less>;

    #[inline]
    fn private_cmp(&self, rhs: &UInt<Ur, B1>, _: SoFar) -> Self::Output {
        self.msb.private_cmp(&rhs.msb, Less)
    }
}

/// Comparing non-terimal bits, with `Lhs` having bit `B1` and `Rhs` having bit `B0`.
/// `SoFar`, Lhs is `Greater`.
impl<Ul, Ur, SoFar> PrivateCmp<UInt<Ur, B0>, SoFar> for UInt<Ul, B1>
where
    Ul: Unsigned,
    Ur: Unsigned,
    SoFar: Ord,
    Ul: PrivateCmp<Ur, Greater>,
{
    type Output = PrivateCmpOut<Ul, Ur, Greater>;

    #[inline]
    fn private_cmp(&self, rhs: &UInt<Ur, B0>, _: SoFar) -> Self::Output {
        self.msb.private_cmp(&rhs.msb, Greater)
    }
}

/// Got to the end of just the `Lhs`. It's `Less`.
impl<U: Unsigned, B: Bit, SoFar: Ord> PrivateCmp<UInt<U, B>, SoFar> for UTerm {
    type Output = Less;

    #[inline]
    fn private_cmp(&self, _: &UInt<U, B>, _: SoFar) -> Self::Output {
        Less
    }
}

/// Got to the end of just the `Rhs`. `Lhs` is `Greater`.
impl<U: Unsigned, B: Bit, SoFar: Ord> PrivateCmp<UTerm, SoFar> for UInt<U, B> {
    type Output = Greater;

    #[inline]
    fn private_cmp(&self, _: &UTerm, _: SoFar) -> Self::Output {
        Greater
    }
}

/// Got to the end of both! Return `SoFar`
impl<SoFar: Ord> PrivateCmp<UTerm, SoFar> for UTerm {
    type Output = SoFar;

    #[inline]
    fn private_cmp(&self, _: &UTerm, so_far: SoFar) -> Self::Output {
        so_far
    }
}

// ---------------------------------------------------------------------------------------
// Getting difference in number of bits

impl<Ul, Bl, Ur, Br> BitDiff<UInt<Ur, Br>> for UInt<Ul, Bl>
where
    Ul: Unsigned,
    Bl: Bit,
    Ur: Unsigned,
    Br: Bit,
    Ul: BitDiff<Ur>,
{
    type Output = BitDiffOut<Ul, Ur>;
}

impl<Ul> BitDiff<UTerm> for Ul
where
    Ul: Unsigned + Len,
{
    type Output = Length<Ul>;
}

// ---------------------------------------------------------------------------------------
// Shifting one number until it's the size of another
use crate::private::ShiftDiff;
impl<Ul: Unsigned, Ur: Unsigned> ShiftDiff<Ur> for Ul
where
    Ur: BitDiff<Ul>,
    Ul: Shl<BitDiffOut<Ur, Ul>>,
{
    type Output = Shleft<Ul, BitDiffOut<Ur, Ul>>;
}

// ---------------------------------------------------------------------------------------
// Powers of unsigned integers

/// X^N
impl<X: Unsigned, N: Unsigned> Pow<N> for X
where
    X: PrivatePow<U1, N>,
{
    type Output = PrivatePowOut<X, U1, N>;
    #[inline]
    fn powi(self, n: N) -> Self::Output {
        self.private_pow(U1::new(), n)
    }
}

impl<Y: Unsigned, X: Unsigned> PrivatePow<Y, U0> for X {
    type Output = Y;

    #[inline]
    fn private_pow(self, y: Y, _: U0) -> Self::Output {
        y
    }
}

impl<Y: Unsigned, X: Unsigned> PrivatePow<Y, U1> for X
where
    X: Mul<Y>,
{
    type Output = Prod<X, Y>;

    #[inline]
    fn private_pow(self, y: Y, _: U1) -> Self::Output {
        self * y
    }
}

/// N is even
impl<Y: Unsigned, U: Unsigned, B: Bit, X: Unsigned> PrivatePow<Y, UInt<UInt<U, B>, B0>> for X
where
    X: Mul,
    Square<X>: PrivatePow<Y, UInt<U, B>>,
{
    type Output = PrivatePowOut<Square<X>, Y, UInt<U, B>>;

    #[inline]
    fn private_pow(self, y: Y, n: UInt<UInt<U, B>, B0>) -> Self::Output {
        (self * self).private_pow(y, n.msb)
    }
}

/// N is odd
impl<Y: Unsigned, U: Unsigned, B: Bit, X: Unsigned> PrivatePow<Y, UInt<UInt<U, B>, B1>> for X
where
    X: Mul + Mul<Y>,
    Square<X>: PrivatePow<Prod<X, Y>, UInt<U, B>>,
{
    type Output = PrivatePowOut<Square<X>, Prod<X, Y>, UInt<U, B>>;

    #[inline]
    fn private_pow(self, y: Y, n: UInt<UInt<U, B>, B1>) -> Self::Output {
        (self * self).private_pow(self * y, n.msb)
    }
}

//------------------------------------------
// Greatest Common Divisor

/// The even number 2*N
#[allow(unused)] // Silence spurious warning on older versions of rust
type Even<N> = UInt<N, B0>;

/// The odd number 2*N + 1
type Odd<N> = UInt<N, B1>;

/// gcd(0, 0) = 0
impl Gcd<U0> for U0 {
    type Output = U0;
}

/// gcd(x, 0) = x
impl<X> Gcd<U0> for X
where
    X: Unsigned + NonZero,
{
    type Output = X;
}

/// gcd(0, y) = y
impl<Y> Gcd<Y> for U0
where
    Y: Unsigned + NonZero,
{
    type Output = Y;
}

/// gcd(x, y) = 2*gcd(x/2, y/2) if both x and y even
impl<Xp, Yp> Gcd<Even<Yp>> for Even<Xp>
where
    Xp: Gcd<Yp>,
    Even<Xp>: NonZero,
    Even<Yp>: NonZero,
{
    type Output = UInt<Gcf<Xp, Yp>, B0>;
}

/// gcd(x, y) = gcd(x, y/2) if x odd and y even
impl<Xp, Yp> Gcd<Even<Yp>> for Odd<Xp>
where
    Odd<Xp>: Gcd<Yp>,
    Even<Yp>: NonZero,
{
    type Output = Gcf<Odd<Xp>, Yp>;
}

/// gcd(x, y) = gcd(x/2, y) if x even and y odd
impl<Xp, Yp> Gcd<Odd<Yp>> for Even<Xp>
where
    Xp: Gcd<Odd<Yp>>,
    Even<Xp>: NonZero,
{
    type Output = Gcf<Xp, Odd<Yp>>;
}

/// gcd(x, y) = gcd([max(x, y) - min(x, y)], min(x, y)) if both x and y odd
///
/// This will immediately invoke the case for x even and y odd because the difference of two odd
/// numbers is an even number.
impl<Xp, Yp> Gcd<Odd<Yp>> for Odd<Xp>
where
    Odd<Xp>: Max<Odd<Yp>> + Min<Odd<Yp>>,
    Odd<Yp>: Max<Odd<Xp>> + Min<Odd<Xp>>,
    Maximum<Odd<Xp>, Odd<Yp>>: Sub<Minimum<Odd<Xp>, Odd<Yp>>>,
    Diff<Maximum<Odd<Xp>, Odd<Yp>>, Minimum<Odd<Xp>, Odd<Yp>>>: Gcd<Minimum<Odd<Xp>, Odd<Yp>>>,
{
    type Output =
        Gcf<Diff<Maximum<Odd<Xp>, Odd<Yp>>, Minimum<Odd<Xp>, Odd<Yp>>>, Minimum<Odd<Xp>, Odd<Yp>>>;
}

#[cfg(test)]
mod gcd_tests {
    use super::*;
    use crate::consts::*;

    macro_rules! gcd_test {
        (
            $( $a:ident, $b:ident => $c:ident ),* $(,)*
        ) => {
            $(
                assert_eq!(<Gcf<$a, $b> as Unsigned>::to_usize(), $c::to_usize());
                assert_eq!(<Gcf<$b, $a> as Unsigned>::to_usize(), $c::to_usize());
             )*
        }
    }

    #[test]
    fn gcd() {
        gcd_test! {
            U0,   U0    => U0,
            U0,   U42   => U42,
            U12,  U8    => U4,
            U13,  U1013 => U1,  // Two primes
            U9,   U26   => U1,  // Not prime but coprime
            U143, U273  => U13,
            U117, U273  => U39,
        }
    }
}

// -----------------------------------------
// GetBit

#[allow(missing_docs)]
pub trait GetBit<I> {
    #[allow(missing_docs)]
    type Output;

    #[doc(hidden)]
    fn get_bit<IM: InternalMarker>(&self, _: &I) -> Self::Output;
}

#[allow(missing_docs)]
pub type GetBitOut<N, I> = <N as GetBit<I>>::Output;

// Base case
impl<Un, Bn> GetBit<U0> for UInt<Un, Bn>
where
    Bn: Copy,
{
    type Output = Bn;

    #[inline]
    fn get_bit<IM: InternalMarker>(&self, _: &U0) -> Self::Output {
        self.lsb
    }
}

// Recursion case
impl<Un, Bn, Ui, Bi> GetBit<UInt<Ui, Bi>> for UInt<Un, Bn>
where
    UInt<Ui, Bi>: Copy + Sub<B1>,
    Un: GetBit<Sub1<UInt<Ui, Bi>>>,
{
    type Output = GetBitOut<Un, Sub1<UInt<Ui, Bi>>>;

    #[inline]
    fn get_bit<IM: InternalMarker>(&self, i: &UInt<Ui, Bi>) -> Self::Output {
        self.msb.get_bit::<Internal>(&(*i - B1))
    }
}

// Ran out of bits
impl<I> GetBit<I> for UTerm {
    type Output = B0;

    #[inline]
    fn get_bit<IM: InternalMarker>(&self, _: &I) -> Self::Output {
        B0
    }
}

#[test]
fn test_get_bit() {
    use crate::consts::*;
    use crate::Same;
    type T1 = <GetBitOut<U2, U0> as Same<B0>>::Output;
    type T2 = <GetBitOut<U2, U1> as Same<B1>>::Output;
    type T3 = <GetBitOut<U2, U2> as Same<B0>>::Output;

    <T1 as Bit>::to_bool();
    <T2 as Bit>::to_bool();
    <T3 as Bit>::to_bool();
}

// -----------------------------------------
// SetBit

/// A **type operator** that, when implemented for unsigned integer `N`, sets the bit at position
/// `I` to `B`.
pub trait SetBit<I, B> {
    #[allow(missing_docs)]
    type Output;

    #[doc(hidden)]
    fn set_bit<IM: InternalMarker>(self, _: I, _: B) -> Self::Output;
}
/// Alias for the result of calling `SetBit`: `SetBitOut<N, I, B> = <N as SetBit<I, B>>::Output`.
pub type SetBitOut<N, I, B> = <N as SetBit<I, B>>::Output;

use crate::private::{PrivateSetBit, PrivateSetBitOut};

// Call private one then trim it
impl<N, I, B> SetBit<I, B> for N
where
    N: PrivateSetBit<I, B>,
    PrivateSetBitOut<N, I, B>: Trim,
{
    type Output = TrimOut<PrivateSetBitOut<N, I, B>>;

    #[inline]
    fn set_bit<IM: InternalMarker>(self, i: I, b: B) -> Self::Output {
        self.private_set_bit(i, b).trim()
    }
}

// Base case
impl<Un, Bn, B> PrivateSetBit<U0, B> for UInt<Un, Bn> {
    type Output = UInt<Un, B>;

    #[inline]
    fn private_set_bit(self, _: U0, b: B) -> Self::Output {
        UInt {
            msb: self.msb,
            lsb: b,
        }
    }
}

// Recursion case
impl<Un, Bn, Ui, Bi, B> PrivateSetBit<UInt<Ui, Bi>, B> for UInt<Un, Bn>
where
    UInt<Ui, Bi>: Sub<B1>,
    Un: PrivateSetBit<Sub1<UInt<Ui, Bi>>, B>,
{
    type Output = UInt<PrivateSetBitOut<Un, Sub1<UInt<Ui, Bi>>, B>, Bn>;

    #[inline]
    fn private_set_bit(self, i: UInt<Ui, Bi>, b: B) -> Self::Output {
        UInt {
            msb: self.msb.private_set_bit(i - B1, b),
            lsb: self.lsb,
        }
    }
}

// Ran out of bits, setting B0
impl<I> PrivateSetBit<I, B0> for UTerm {
    type Output = UTerm;

    #[inline]
    fn private_set_bit(self, _: I, _: B0) -> Self::Output {
        UTerm
    }
}

// Ran out of bits, setting B1
impl<I> PrivateSetBit<I, B1> for UTerm
where
    U1: Shl<I>,
{
    type Output = Shleft<U1, I>;

    #[inline]
    fn private_set_bit(self, i: I, _: B1) -> Self::Output {
        <U1 as Shl<I>>::shl(U1::new(), i)
    }
}

#[test]
fn test_set_bit() {
    use crate::consts::*;
    use crate::Same;
    type T1 = <SetBitOut<U2, U0, B0> as Same<U2>>::Output;
    type T2 = <SetBitOut<U2, U0, B1> as Same<U3>>::Output;
    type T3 = <SetBitOut<U2, U1, B0> as Same<U0>>::Output;
    type T4 = <SetBitOut<U2, U1, B1> as Same<U2>>::Output;
    type T5 = <SetBitOut<U2, U2, B0> as Same<U2>>::Output;
    type T6 = <SetBitOut<U2, U2, B1> as Same<U6>>::Output;
    type T7 = <SetBitOut<U2, U3, B0> as Same<U2>>::Output;
    type T8 = <SetBitOut<U2, U3, B1> as Same<U10>>::Output;
    type T9 = <SetBitOut<U2, U4, B0> as Same<U2>>::Output;
    type T10 = <SetBitOut<U2, U4, B1> as Same<U18>>::Output;

    type T11 = <SetBitOut<U3, U0, B0> as Same<U2>>::Output;

    <T1 as Unsigned>::to_u32();
    <T2 as Unsigned>::to_u32();
    <T3 as Unsigned>::to_u32();
    <T4 as Unsigned>::to_u32();
    <T5 as Unsigned>::to_u32();
    <T6 as Unsigned>::to_u32();
    <T7 as Unsigned>::to_u32();
    <T8 as Unsigned>::to_u32();
    <T9 as Unsigned>::to_u32();
    <T10 as Unsigned>::to_u32();
    <T11 as Unsigned>::to_u32();
}

// -----------------------------------------

// Division algorithm:
// We have N / D:
// let Q = 0, R = 0
// NBits = len(N)
// for I in NBits-1..0:
//   R <<=1
//   R[0] = N[i]
//   let C = R.cmp(D)
//   if C == Equal or Greater:
//     R -= D
//     Q[i] = 1

#[cfg(test)]
mod div_tests {
    use crate::Unsigned;

    use super::SetBitOut;

    macro_rules! test_div {
        ($a:ident / $b:ident = $c:ident) => {{
            type R = Quot<$a, $b>;
            assert_eq!(<R as Unsigned>::to_usize(), $c::to_usize());
        }};
    }
    #[test]
    fn test_div() {
        use crate::consts::*;
        use crate::{Quot, Same};

        test_div!(U0 / U1 = U0);
        test_div!(U1 / U1 = U1);
        test_div!(U2 / U1 = U2);
        test_div!(U3 / U1 = U3);
        test_div!(U4 / U1 = U4);

        test_div!(U0 / U2 = U0);
        test_div!(U1 / U2 = U0);
        test_div!(U2 / U2 = U1);
        test_div!(U3 / U2 = U1);
        test_div!(U4 / U2 = U2);
        test_div!(U6 / U2 = U3);
        test_div!(U7 / U2 = U3);

        type T = <SetBitOut<U0, U1, B1> as Same<U2>>::Output;
        <T as Unsigned>::to_u32();
    }
}
// -----------------------------------------
// Div
use core::ops::Div;

// 0 // N
impl<Ur: Unsigned, Br: Bit> Div<UInt<Ur, Br>> for UTerm {
    type Output = UTerm;
    #[inline]
    fn div(self, _: UInt<Ur, Br>) -> Self::Output {
        UTerm
    }
}

// M // N
impl<Ul: Unsigned, Bl: Bit, Ur: Unsigned, Br: Bit> Div<UInt<Ur, Br>> for UInt<Ul, Bl>
where
    UInt<Ul, Bl>: Len,
    Length<UInt<Ul, Bl>>: Sub<B1>,
    (): PrivateDiv<UInt<Ul, Bl>, UInt<Ur, Br>, U0, U0, Sub1<Length<UInt<Ul, Bl>>>>,
{
    type Output = PrivateDivQuot<UInt<Ul, Bl>, UInt<Ur, Br>, U0, U0, Sub1<Length<UInt<Ul, Bl>>>>;
    #[inline]
    fn div(self, rhs: UInt<Ur, Br>) -> Self::Output {
        #[allow(clippy::suspicious_arithmetic_impl)]
        ().private_div_quotient(self, rhs, U0::new(), U0::new(), self.len() - B1)
    }
}

// -----------------------------------------
// Rem
use core::ops::Rem;

// 0 % N
impl<Ur: Unsigned, Br: Bit> Rem<UInt<Ur, Br>> for UTerm {
    type Output = UTerm;
    #[inline]
    fn rem(self, _: UInt<Ur, Br>) -> Self::Output {
        UTerm
    }
}

// M % N
impl<Ul: Unsigned, Bl: Bit, Ur: Unsigned, Br: Bit> Rem<UInt<Ur, Br>> for UInt<Ul, Bl>
where
    UInt<Ul, Bl>: Len,
    Length<UInt<Ul, Bl>>: Sub<B1>,
    (): PrivateDiv<UInt<Ul, Bl>, UInt<Ur, Br>, U0, U0, Sub1<Length<UInt<Ul, Bl>>>>,
{
    type Output = PrivateDivRem<UInt<Ul, Bl>, UInt<Ur, Br>, U0, U0, Sub1<Length<UInt<Ul, Bl>>>>;
    #[inline]
    fn rem(self, rhs: UInt<Ur, Br>) -> Self::Output {
        #[allow(clippy::suspicious_arithmetic_impl)]
        ().private_div_remainder(self, rhs, UTerm, UTerm, self.len() - B1)
    }
}

// -----------------------------------------
// PrivateDiv
use crate::private::{PrivateDiv, PrivateDivQuot, PrivateDivRem};

use crate::Compare;
// R == 0: We set R = UInt<UTerm, N[i]>, then call out to PrivateDivIf for the if statement
impl<N, D, Q, I> PrivateDiv<N, D, Q, U0, I> for ()
where
    N: GetBit<I>,
    UInt<UTerm, GetBitOut<N, I>>: Trim,
    TrimOut<UInt<UTerm, GetBitOut<N, I>>>: Cmp<D>,
    (): PrivateDivIf<
        N,
        D,
        Q,
        TrimOut<UInt<UTerm, GetBitOut<N, I>>>,
        I,
        Compare<TrimOut<UInt<UTerm, GetBitOut<N, I>>>, D>,
    >,
{
    type Quotient = PrivateDivIfQuot<
        N,
        D,
        Q,
        TrimOut<UInt<UTerm, GetBitOut<N, I>>>,
        I,
        Compare<TrimOut<UInt<UTerm, GetBitOut<N, I>>>, D>,
    >;
    type Remainder = PrivateDivIfRem<
        N,
        D,
        Q,
        TrimOut<UInt<UTerm, GetBitOut<N, I>>>,
        I,
        Compare<TrimOut<UInt<UTerm, GetBitOut<N, I>>>, D>,
    >;

    #[inline]
    fn private_div_quotient(self, n: N, d: D, q: Q, _: U0, i: I) -> Self::Quotient
where {
        let r = (UInt {
            msb: UTerm,
            lsb: n.get_bit::<Internal>(&i),
        })
        .trim();
        let r_cmp_d = r.compare::<Internal>(&d);
        ().private_div_if_quotient(n, d, q, r, i, r_cmp_d)
    }

    #[inline]
    fn private_div_remainder(self, n: N, d: D, q: Q, _: U0, i: I) -> Self::Remainder {
        let r = (UInt {
            msb: UTerm,
            lsb: n.get_bit::<Internal>(&i),
        })
        .trim();
        let r_cmp_d = r.compare::<Internal>(&d);
        ().private_div_if_remainder(n, d, q, r, i, r_cmp_d)
    }
}

// R > 0: We perform R <<= 1 and R[0] = N[i], then call out to PrivateDivIf for the if statement
impl<N, D, Q, Ur, Br, I> PrivateDiv<N, D, Q, UInt<Ur, Br>, I> for ()
where
    N: GetBit<I>,
    UInt<UInt<Ur, Br>, GetBitOut<N, I>>: Cmp<D>,
    (): PrivateDivIf<
        N,
        D,
        Q,
        UInt<UInt<Ur, Br>, GetBitOut<N, I>>,
        I,
        Compare<UInt<UInt<Ur, Br>, GetBitOut<N, I>>, D>,
    >,
{
    type Quotient = PrivateDivIfQuot<
        N,
        D,
        Q,
        UInt<UInt<Ur, Br>, GetBitOut<N, I>>,
        I,
        Compare<UInt<UInt<Ur, Br>, GetBitOut<N, I>>, D>,
    >;
    type Remainder = PrivateDivIfRem<
        N,
        D,
        Q,
        UInt<UInt<Ur, Br>, GetBitOut<N, I>>,
        I,
        Compare<UInt<UInt<Ur, Br>, GetBitOut<N, I>>, D>,
    >;

    #[inline]
    fn private_div_quotient(self, n: N, d: D, q: Q, r: UInt<Ur, Br>, i: I) -> Self::Quotient {
        let r = UInt {
            msb: r,
            lsb: n.get_bit::<Internal>(&i),
        };
        let r_cmp_d = r.compare::<Internal>(&d);
        ().private_div_if_quotient(n, d, q, r, i, r_cmp_d)
    }

    #[inline]
    fn private_div_remainder(self, n: N, d: D, q: Q, r: UInt<Ur, Br>, i: I) -> Self::Remainder {
        let r = UInt {
            msb: r,
            lsb: n.get_bit::<Internal>(&i),
        };
        let r_cmp_d = r.compare::<Internal>(&d);
        ().private_div_if_remainder(n, d, q, r, i, r_cmp_d)
    }
}

// -----------------------------------------
// PrivateDivIf

use crate::private::{PrivateDivIf, PrivateDivIfQuot, PrivateDivIfRem};

// R < D, I > 0, we do nothing and recurse
impl<N, D, Q, R, Ui, Bi> PrivateDivIf<N, D, Q, R, UInt<Ui, Bi>, Less> for ()
where
    UInt<Ui, Bi>: Sub<B1>,
    (): PrivateDiv<N, D, Q, R, Sub1<UInt<Ui, Bi>>>,
{
    type Quotient = PrivateDivQuot<N, D, Q, R, Sub1<UInt<Ui, Bi>>>;
    type Remainder = PrivateDivRem<N, D, Q, R, Sub1<UInt<Ui, Bi>>>;

    #[inline]
    fn private_div_if_quotient(
        self,
        n: N,
        d: D,
        q: Q,
        r: R,
        i: UInt<Ui, Bi>,
        _: Less,
    ) -> Self::Quotient
where {
        ().private_div_quotient(n, d, q, r, i - B1)
    }

    #[inline]
    fn private_div_if_remainder(
        self,
        n: N,
        d: D,
        q: Q,
        r: R,
        i: UInt<Ui, Bi>,
        _: Less,
    ) -> Self::Remainder
where {
        ().private_div_remainder(n, d, q, r, i - B1)
    }
}

// R == D, I > 0, we set R = 0, Q[I] = 1 and recurse
impl<N, D, Q, R, Ui, Bi> PrivateDivIf<N, D, Q, R, UInt<Ui, Bi>, Equal> for ()
where
    UInt<Ui, Bi>: Copy + Sub<B1>,
    Q: SetBit<UInt<Ui, Bi>, B1>,
    (): PrivateDiv<N, D, SetBitOut<Q, UInt<Ui, Bi>, B1>, U0, Sub1<UInt<Ui, Bi>>>,
{
    type Quotient = PrivateDivQuot<N, D, SetBitOut<Q, UInt<Ui, Bi>, B1>, U0, Sub1<UInt<Ui, Bi>>>;
    type Remainder = PrivateDivRem<N, D, SetBitOut<Q, UInt<Ui, Bi>, B1>, U0, Sub1<UInt<Ui, Bi>>>;

    #[inline]
    fn private_div_if_quotient(
        self,
        n: N,
        d: D,
        q: Q,
        _: R,
        i: UInt<Ui, Bi>,
        _: Equal,
    ) -> Self::Quotient
where {
        ().private_div_quotient(n, d, q.set_bit::<Internal>(i, B1), U0::new(), i - B1)
    }

    #[inline]
    fn private_div_if_remainder(
        self,
        n: N,
        d: D,
        q: Q,
        _: R,
        i: UInt<Ui, Bi>,
        _: Equal,
    ) -> Self::Remainder
where {
        ().private_div_remainder(n, d, q.set_bit::<Internal>(i, B1), U0::new(), i - B1)
    }
}

use crate::Diff;
// R > D, I > 0, we set R -= D, Q[I] = 1 and recurse
impl<N, D, Q, R, Ui, Bi> PrivateDivIf<N, D, Q, R, UInt<Ui, Bi>, Greater> for ()
where
    D: Copy,
    UInt<Ui, Bi>: Copy + Sub<B1>,
    R: Sub<D>,
    Q: SetBit<UInt<Ui, Bi>, B1>,
    (): PrivateDiv<N, D, SetBitOut<Q, UInt<Ui, Bi>, B1>, Diff<R, D>, Sub1<UInt<Ui, Bi>>>,
{
    type Quotient =
        PrivateDivQuot<N, D, SetBitOut<Q, UInt<Ui, Bi>, B1>, Diff<R, D>, Sub1<UInt<Ui, Bi>>>;
    type Remainder =
        PrivateDivRem<N, D, SetBitOut<Q, UInt<Ui, Bi>, B1>, Diff<R, D>, Sub1<UInt<Ui, Bi>>>;

    #[inline]
    fn private_div_if_quotient(
        self,
        n: N,
        d: D,
        q: Q,
        r: R,
        i: UInt<Ui, Bi>,
        _: Greater,
    ) -> Self::Quotient
where {
        ().private_div_quotient(n, d, q.set_bit::<Internal>(i, B1), r - d, i - B1)
    }

    #[inline]
    fn private_div_if_remainder(
        self,
        n: N,
        d: D,
        q: Q,
        r: R,
        i: UInt<Ui, Bi>,
        _: Greater,
    ) -> Self::Remainder
where {
        ().private_div_remainder(n, d, q.set_bit::<Internal>(i, B1), r - d, i - B1)
    }
}

// R < D, I == 0: we do nothing, and return
impl<N, D, Q, R> PrivateDivIf<N, D, Q, R, U0, Less> for () {
    type Quotient = Q;
    type Remainder = R;

    #[inline]
    fn private_div_if_quotient(self, _: N, _: D, q: Q, _: R, _: U0, _: Less) -> Self::Quotient {
        q
    }

    #[inline]
    fn private_div_if_remainder(self, _: N, _: D, _: Q, r: R, _: U0, _: Less) -> Self::Remainder {
        r
    }
}

// R == D, I == 0: we set R = 0, Q[I] = 1, and return
impl<N, D, Q, R> PrivateDivIf<N, D, Q, R, U0, Equal> for ()
where
    Q: SetBit<U0, B1>,
{
    type Quotient = SetBitOut<Q, U0, B1>;
    type Remainder = U0;

    #[inline]
    fn private_div_if_quotient(self, _: N, _: D, q: Q, _: R, i: U0, _: Equal) -> Self::Quotient {
        q.set_bit::<Internal>(i, B1)
    }

    #[inline]
    fn private_div_if_remainder(self, _: N, _: D, _: Q, _: R, i: U0, _: Equal) -> Self::Remainder {
        i
    }
}

// R > D, I == 0: We set R -= D, Q[I] = 1, and return
impl<N, D, Q, R> PrivateDivIf<N, D, Q, R, U0, Greater> for ()
where
    R: Sub<D>,
    Q: SetBit<U0, B1>,
{
    type Quotient = SetBitOut<Q, U0, B1>;
    type Remainder = Diff<R, D>;

    #[inline]
    fn private_div_if_quotient(self, _: N, _: D, q: Q, _: R, i: U0, _: Greater) -> Self::Quotient {
        q.set_bit::<Internal>(i, B1)
    }

    #[inline]
    fn private_div_if_remainder(
        self,
        _: N,
        d: D,
        _: Q,
        r: R,
        _: U0,
        _: Greater,
    ) -> Self::Remainder {
        r - d
    }
}

// -----------------------------------------
// PartialDiv
use crate::{PartialDiv, Quot};
impl<Ur: Unsigned, Br: Bit> PartialDiv<UInt<Ur, Br>> for UTerm {
    type Output = UTerm;
    #[inline]
    fn partial_div(self, _: UInt<Ur, Br>) -> Self::Output {
        UTerm
    }
}

// M / N
impl<Ul: Unsigned, Bl: Bit, Ur: Unsigned, Br: Bit> PartialDiv<UInt<Ur, Br>> for UInt<Ul, Bl>
where
    UInt<Ul, Bl>: Div<UInt<Ur, Br>> + Rem<UInt<Ur, Br>, Output = U0>,
{
    type Output = Quot<UInt<Ul, Bl>, UInt<Ur, Br>>;
    #[inline]
    fn partial_div(self, rhs: UInt<Ur, Br>) -> Self::Output {
        self / rhs
    }
}

// -----------------------------------------
// PrivateMin
use crate::private::{PrivateMin, PrivateMinOut};

impl<U, B, Ur> PrivateMin<Ur, Equal> for UInt<U, B>
where
    Ur: Unsigned,
    U: Unsigned,
    B: Bit,
{
    type Output = UInt<U, B>;
    #[inline]
    fn private_min(self, _: Ur) -> Self::Output {
        self
    }
}

impl<U, B, Ur> PrivateMin<Ur, Less> for UInt<U, B>
where
    Ur: Unsigned,
    U: Unsigned,
    B: Bit,
{
    type Output = UInt<U, B>;
    #[inline]
    fn private_min(self, _: Ur) -> Self::Output {
        self
    }
}

impl<U, B, Ur> PrivateMin<Ur, Greater> for UInt<U, B>
where
    Ur: Unsigned,
    U: Unsigned,
    B: Bit,
{
    type Output = Ur;
    #[inline]
    fn private_min(self, rhs: Ur) -> Self::Output {
        rhs
    }
}

// -----------------------------------------
// Min
use crate::Min;

impl<U> Min<U> for UTerm
where
    U: Unsigned,
{
    type Output = UTerm;
    #[inline]
    fn min(self, _: U) -> Self::Output {
        self
    }
}

impl<U, B, Ur> Min<Ur> for UInt<U, B>
where
    U: Unsigned,
    B: Bit,
    Ur: Unsigned,
    UInt<U, B>: Cmp<Ur> + PrivateMin<Ur, Compare<UInt<U, B>, Ur>>,
{
    type Output = PrivateMinOut<UInt<U, B>, Ur, Compare<UInt<U, B>, Ur>>;
    #[inline]
    fn min(self, rhs: Ur) -> Self::Output {
        self.private_min(rhs)
    }
}

// -----------------------------------------
// PrivateMax
use crate::private::{PrivateMax, PrivateMaxOut};

impl<U, B, Ur> PrivateMax<Ur, Equal> for UInt<U, B>
where
    Ur: Unsigned,
    U: Unsigned,
    B: Bit,
{
    type Output = UInt<U, B>;
    #[inline]
    fn private_max(self, _: Ur) -> Self::Output {
        self
    }
}

impl<U, B, Ur> PrivateMax<Ur, Less> for UInt<U, B>
where
    Ur: Unsigned,
    U: Unsigned,
    B: Bit,
{
    type Output = Ur;
    #[inline]
    fn private_max(self, rhs: Ur) -> Self::Output {
        rhs
    }
}

impl<U, B, Ur> PrivateMax<Ur, Greater> for UInt<U, B>
where
    Ur: Unsigned,
    U: Unsigned,
    B: Bit,
{
    type Output = UInt<U, B>;
    #[inline]
    fn private_max(self, _: Ur) -> Self::Output {
        self
    }
}

// -----------------------------------------
// Max
use crate::Max;

impl<U> Max<U> for UTerm
where
    U: Unsigned,
{
    type Output = U;
    #[inline]
    fn max(self, rhs: U) -> Self::Output {
        rhs
    }
}

impl<U, B, Ur> Max<Ur> for UInt<U, B>
where
    U: Unsigned,
    B: Bit,
    Ur: Unsigned,
    UInt<U, B>: Cmp<Ur> + PrivateMax<Ur, Compare<UInt<U, B>, Ur>>,
{
    type Output = PrivateMaxOut<UInt<U, B>, Ur, Compare<UInt<U, B>, Ur>>;
    #[inline]
    fn max(self, rhs: Ur) -> Self::Output {
        self.private_max(rhs)
    }
}

// -----------------------------------------
// SquareRoot

impl<N> SquareRoot for N
where
    N: PrivateSquareRoot,
{
    type Output = <Self as PrivateSquareRoot>::Output;
}

// sqrt(0) = 0.
impl PrivateSquareRoot for UTerm {
    type Output = UTerm;
}

// sqrt(1) = 1.
impl PrivateSquareRoot for UInt<UTerm, B1> {
    type Output = UInt<UTerm, B1>;
}

// General case of sqrt(Self) where Self >= 2. If a and b are
// bit-valued and Self = 4*u + 2*a + b, then the integer-valued
// (fractional part truncated) square root of Self is either 2*sqrt(u)
// or 2*sqrt(u)+1. Guess and check by comparing (2*sqrt(u)+1)^2
// against Self. Since the `typenum` result of that comparison is a
// bit, directly add that bit to 2*sqrt(u).
//
// Use `Sum<Double<Sqrt<U>>, GrEq<...>>` instead of `UInt<Sqrt<U>,
// GrEq<...>>` because `Sqrt<U>` can turn out to be `UTerm` and
// `GrEq<...>` can turn out to be `B0`, which would not be a valid
// UInt as leading zeros are disallowed.
impl<U, Ba, Bb> PrivateSquareRoot for UInt<UInt<U, Ba>, Bb>
where
    U: Unsigned,
    Ba: Bit,
    Bb: Bit,
    U: SquareRoot,
    Sqrt<U>: Shl<B1>,
    Double<Sqrt<U>>: Add<B1>,
    Add1<Double<Sqrt<U>>>: Mul,
    Self: IsGreaterOrEqual<Square<Add1<Double<Sqrt<U>>>>>,
    Double<Sqrt<U>>: Add<GrEq<Self, Square<Add1<Double<Sqrt<U>>>>>>,
{
    type Output = Sum<Double<Sqrt<U>>, GrEq<Self, Square<Add1<Double<Sqrt<U>>>>>>;
}

#[test]
fn sqrt_test() {
    use crate::consts::*;

    assert_eq!(0, <Sqrt<U0>>::to_u32());

    assert_eq!(1, <Sqrt<U1>>::to_u32());
    assert_eq!(1, <Sqrt<U2>>::to_u32());
    assert_eq!(1, <Sqrt<U3>>::to_u32());

    assert_eq!(2, <Sqrt<U4>>::to_u32());
    assert_eq!(2, <Sqrt<U5>>::to_u32());
    assert_eq!(2, <Sqrt<U6>>::to_u32());
    assert_eq!(2, <Sqrt<U7>>::to_u32());
    assert_eq!(2, <Sqrt<U8>>::to_u32());

    assert_eq!(3, <Sqrt<U9>>::to_u32());
    assert_eq!(3, <Sqrt<U10>>::to_u32());
    assert_eq!(3, <Sqrt<U11>>::to_u32());
    assert_eq!(3, <Sqrt<U12>>::to_u32());
    assert_eq!(3, <Sqrt<U13>>::to_u32());
    assert_eq!(3, <Sqrt<U14>>::to_u32());
    assert_eq!(3, <Sqrt<U15>>::to_u32());

    assert_eq!(4, <Sqrt<U16>>::to_u32());
    assert_eq!(4, <Sqrt<U17>>::to_u32());
    assert_eq!(4, <Sqrt<U18>>::to_u32());
    assert_eq!(4, <Sqrt<U19>>::to_u32());
    assert_eq!(4, <Sqrt<U20>>::to_u32());
    assert_eq!(4, <Sqrt<U21>>::to_u32());
    assert_eq!(4, <Sqrt<U22>>::to_u32());
    assert_eq!(4, <Sqrt<U23>>::to_u32());
    assert_eq!(4, <Sqrt<U24>>::to_u32());

    assert_eq!(5, <Sqrt<U25>>::to_u32());
    assert_eq!(5, <Sqrt<U26>>::to_u32());
    // ...
}

// -----------------------------------------
// Logarithm2

impl<N> Logarithm2 for N
where
    N: PrivateLogarithm2,
{
    type Output = <Self as PrivateLogarithm2>::Output;
}

// log2(1) = 0.
impl PrivateLogarithm2 for UInt<UTerm, B1> {
    type Output = U0;
}

// General case of log2(Self) where Self >= 2.
impl<U, B> PrivateLogarithm2 for UInt<U, B>
where
    U: Unsigned + Logarithm2,
    B: Bit,
    Log2<U>: Add<B1>,
{
    type Output = Add1<Log2<U>>;
}

// -----------------------------------------
// ToInt

impl ToInt<i8> for UTerm {
    #[inline]
    fn to_int() -> i8 {
        Self::I8
    }
    const INT: i8 = Self::I8;
}

impl ToInt<i16> for UTerm {
    #[inline]
    fn to_int() -> i16 {
        Self::I16
    }
    const INT: i16 = Self::I16;
}

impl ToInt<i32> for UTerm {
    #[inline]
    fn to_int() -> i32 {
        Self::I32
    }
    const INT: i32 = Self::I32;
}

impl ToInt<i64> for UTerm {
    #[inline]
    fn to_int() -> i64 {
        Self::I64
    }
    const INT: i64 = Self::I64;
}

impl ToInt<u8> for UTerm {
    #[inline]
    fn to_int() -> u8 {
        Self::U8
    }
    const INT: u8 = Self::U8;
}

impl ToInt<u16> for UTerm {
    #[inline]
    fn to_int() -> u16 {
        Self::U16
    }
    const INT: u16 = Self::U16;
}

impl ToInt<u32> for UTerm {
    #[inline]
    fn to_int() -> u32 {
        Self::U32
    }
    const INT: u32 = Self::U32;
}

impl ToInt<u64> for UTerm {
    #[inline]
    fn to_int() -> u64 {
        Self::U64
    }
    const INT: u64 = Self::U64;
}

impl ToInt<usize> for UTerm {
    #[inline]
    fn to_int() -> usize {
        Self::USIZE
    }
    const INT: usize = Self::USIZE;
}

impl<U, B> ToInt<i8> for UInt<U, B>
where
    U: Unsigned,
    B: Bit,
{
    #[inline]
    fn to_int() -> i8 {
        Self::I8
    }
    const INT: i8 = Self::I8;
}

impl<U, B> ToInt<i16> for UInt<U, B>
where
    U: Unsigned,
    B: Bit,
{
    #[inline]
    fn to_int() -> i16 {
        Self::I16
    }
    const INT: i16 = Self::I16;
}

impl<U, B> ToInt<i32> for UInt<U, B>
where
    U: Unsigned,
    B: Bit,
{
    #[inline]
    fn to_int() -> i32 {
        Self::I32
    }
    const INT: i32 = Self::I32;
}

impl<U, B> ToInt<i64> for UInt<U, B>
where
    U: Unsigned,
    B: Bit,
{
    #[inline]
    fn to_int() -> i64 {
        Self::I64
    }
    const INT: i64 = Self::I64;
}

impl<U, B> ToInt<u8> for UInt<U, B>
where
    U: Unsigned,
    B: Bit,
{
    #[inline]
    fn to_int() -> u8 {
        Self::U8
    }
    const INT: u8 = Self::U8;
}

impl<U, B> ToInt<u16> for UInt<U, B>
where
    U: Unsigned,
    B: Bit,
{
    #[inline]
    fn to_int() -> u16 {
        Self::U16
    }
    const INT: u16 = Self::U16;
}

impl<U, B> ToInt<u32> for UInt<U, B>
where
    U: Unsigned,
    B: Bit,
{
    #[inline]
    fn to_int() -> u32 {
        Self::U32
    }
    const INT: u32 = Self::U32;
}

impl<U, B> ToInt<u64> for UInt<U, B>
where
    U: Unsigned,
    B: Bit,
{
    #[inline]
    fn to_int() -> u64 {
        Self::U64
    }
    const INT: u64 = Self::U64;
}

impl<U, B> ToInt<usize> for UInt<U, B>
where
    U: Unsigned,
    B: Bit,
{
    #[inline]
    fn to_int() -> usize {
        Self::USIZE
    }
    const INT: usize = Self::USIZE;
}

#[cfg(test)]
mod tests {
    use crate::consts::*;
    use crate::{Log2, ToInt, Unsigned};

    #[test]
    fn log2_test() {
        assert_eq!(0, <Log2<U1>>::to_u32());

        assert_eq!(1, <Log2<U2>>::to_u32());
        assert_eq!(1, <Log2<U3>>::to_u32());

        assert_eq!(2, <Log2<U4>>::to_u32());
        assert_eq!(2, <Log2<U5>>::to_u32());
        assert_eq!(2, <Log2<U6>>::to_u32());
        assert_eq!(2, <Log2<U7>>::to_u32());

        assert_eq!(3, <Log2<U8>>::to_u32());
        assert_eq!(3, <Log2<U9>>::to_u32());
        assert_eq!(3, <Log2<U10>>::to_u32());
        assert_eq!(3, <Log2<U11>>::to_u32());
        assert_eq!(3, <Log2<U12>>::to_u32());
        assert_eq!(3, <Log2<U13>>::to_u32());
        assert_eq!(3, <Log2<U14>>::to_u32());
        assert_eq!(3, <Log2<U15>>::to_u32());

        assert_eq!(4, <Log2<U16>>::to_u32());
        assert_eq!(4, <Log2<U17>>::to_u32());
        assert_eq!(4, <Log2<U18>>::to_u32());
        assert_eq!(4, <Log2<U19>>::to_u32());
        assert_eq!(4, <Log2<U20>>::to_u32());
        assert_eq!(4, <Log2<U21>>::to_u32());
        assert_eq!(4, <Log2<U22>>::to_u32());
        assert_eq!(4, <Log2<U23>>::to_u32());
        assert_eq!(4, <Log2<U24>>::to_u32());
        assert_eq!(4, <Log2<U25>>::to_u32());
        assert_eq!(4, <Log2<U26>>::to_u32());
        assert_eq!(4, <Log2<U27>>::to_u32());
        assert_eq!(4, <Log2<U28>>::to_u32());
        assert_eq!(4, <Log2<U29>>::to_u32());
        assert_eq!(4, <Log2<U30>>::to_u32());
        assert_eq!(4, <Log2<U31>>::to_u32());

        assert_eq!(5, <Log2<U32>>::to_u32());
        assert_eq!(5, <Log2<U33>>::to_u32());

        // ...
    }

    #[test]
    fn uint_toint_test() {
        // i8
        assert_eq!(0_i8, U0::to_int());
        assert_eq!(1_i8, U1::to_int());
        assert_eq!(2_i8, U2::to_int());
        assert_eq!(3_i8, U3::to_int());
        assert_eq!(4_i8, U4::to_int());
        assert_eq!(0_i8, U0::INT);
        assert_eq!(1_i8, U1::INT);
        assert_eq!(2_i8, U2::INT);
        assert_eq!(3_i8, U3::INT);
        assert_eq!(4_i8, U4::INT);

        // i16
        assert_eq!(0_i16, U0::to_int());
        assert_eq!(1_i16, U1::to_int());
        assert_eq!(2_i16, U2::to_int());
        assert_eq!(3_i16, U3::to_int());
        assert_eq!(4_i16, U4::to_int());
        assert_eq!(0_i16, U0::INT);
        assert_eq!(1_i16, U1::INT);
        assert_eq!(2_i16, U2::INT);
        assert_eq!(3_i16, U3::INT);
        assert_eq!(4_i16, U4::INT);

        // i32
        assert_eq!(0_i32, U0::to_int());
        assert_eq!(1_i32, U1::to_int());
        assert_eq!(2_i32, U2::to_int());
        assert_eq!(3_i32, U3::to_int());
        assert_eq!(4_i32, U4::to_int());
        assert_eq!(0_i32, U0::INT);
        assert_eq!(1_i32, U1::INT);
        assert_eq!(2_i32, U2::INT);
        assert_eq!(3_i32, U3::INT);
        assert_eq!(4_i32, U4::INT);

        // i64
        assert_eq!(0_i64, U0::to_int());
        assert_eq!(1_i64, U1::to_int());
        assert_eq!(2_i64, U2::to_int());
        assert_eq!(3_i64, U3::to_int());
        assert_eq!(4_i64, U4::to_int());
        assert_eq!(0_i64, U0::INT);
        assert_eq!(1_i64, U1::INT);
        assert_eq!(2_i64, U2::INT);
        assert_eq!(3_i64, U3::INT);
        assert_eq!(4_i64, U4::INT);

        // u8
        assert_eq!(0_u8, U0::to_int());
        assert_eq!(1_u8, U1::to_int());
        assert_eq!(2_u8, U2::to_int());
        assert_eq!(3_u8, U3::to_int());
        assert_eq!(4_u8, U4::to_int());
        assert_eq!(0_u8, U0::INT);
        assert_eq!(1_u8, U1::INT);
        assert_eq!(2_u8, U2::INT);
        assert_eq!(3_u8, U3::INT);
        assert_eq!(4_u8, U4::INT);

        // u16
        assert_eq!(0_u16, U0::to_int());
        assert_eq!(1_u16, U1::to_int());
        assert_eq!(2_u16, U2::to_int());
        assert_eq!(3_u16, U3::to_int());
        assert_eq!(4_u16, U4::to_int());
        assert_eq!(0_u16, U0::INT);
        assert_eq!(1_u16, U1::INT);
        assert_eq!(2_u16, U2::INT);
        assert_eq!(3_u16, U3::INT);
        assert_eq!(4_u16, U4::INT);

        // u32
        assert_eq!(0_u32, U0::to_int());
        assert_eq!(1_u32, U1::to_int());
        assert_eq!(2_u32, U2::to_int());
        assert_eq!(3_u32, U3::to_int());
        assert_eq!(4_u32, U4::to_int());
        assert_eq!(0_u32, U0::INT);
        assert_eq!(1_u32, U1::INT);
        assert_eq!(2_u32, U2::INT);
        assert_eq!(3_u32, U3::INT);
        assert_eq!(4_u32, U4::INT);

        // u64
        assert_eq!(0_u64, U0::to_int());
        assert_eq!(1_u64, U1::to_int());
        assert_eq!(2_u64, U2::to_int());
        assert_eq!(3_u64, U3::to_int());
        assert_eq!(4_u64, U4::to_int());
        assert_eq!(0_u64, U0::INT);
        assert_eq!(1_u64, U1::INT);
        assert_eq!(2_u64, U2::INT);
        assert_eq!(3_u64, U3::INT);
        assert_eq!(4_u64, U4::INT);

        // usize
        assert_eq!(0_usize, U0::to_int());
        assert_eq!(1_usize, U1::to_int());
        assert_eq!(2_usize, U2::to_int());
        assert_eq!(3_usize, U3::to_int());
        assert_eq!(4_usize, U4::to_int());
        assert_eq!(0_usize, U0::INT);
        assert_eq!(1_usize, U1::INT);
        assert_eq!(2_usize, U2::INT);
        assert_eq!(3_usize, U3::INT);
        assert_eq!(4_usize, U4::INT);
    }
}
