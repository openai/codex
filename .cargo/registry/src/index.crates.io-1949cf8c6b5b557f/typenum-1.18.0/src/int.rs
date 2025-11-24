//! Type-level signed integers.
//!
//!
//! Type **operators** implemented:
//!
//! From `core::ops`: `Add`, `Sub`, `Mul`, `Div`, and `Rem`.
//! From `typenum`: `Same`, `Cmp`, and `Pow`.
//!
//! Rather than directly using the structs defined in this module, it is recommended that
//! you import and use the relevant aliases from the [consts](../consts/index.html) module.
//!
//! Note that operators that work on the underlying structure of the number are
//! intentionally not implemented. This is because this implementation of signed integers
//! does *not* use twos-complement, and implementing them would require making arbitrary
//! choices, causing the results of such operators to be difficult to reason about.
//!
//! # Example
//! ```rust
//! use std::ops::{Add, Div, Mul, Rem, Sub};
//! use typenum::{Integer, N3, P2};
//!
//! assert_eq!(<N3 as Add<P2>>::Output::to_i32(), -1);
//! assert_eq!(<N3 as Sub<P2>>::Output::to_i32(), -5);
//! assert_eq!(<N3 as Mul<P2>>::Output::to_i32(), -6);
//! assert_eq!(<N3 as Div<P2>>::Output::to_i32(), -1);
//! assert_eq!(<N3 as Rem<P2>>::Output::to_i32(), -1);
//! ```

pub use crate::marker_traits::Integer;
use crate::{
    bit::{Bit, B0, B1},
    consts::{N1, P1, U0, U1},
    private::{Internal, InternalMarker, PrivateDivInt, PrivateIntegerAdd, PrivateRem},
    uint::{UInt, Unsigned},
    Cmp, Equal, Greater, Less, NonZero, Pow, PowerOfTwo, ToInt, Zero,
};
use core::ops::{Add, Div, Mul, Neg, Rem, Sub};

/// Type-level signed integers with positive sign.
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Default)]
#[cfg_attr(feature = "scale_info", derive(scale_info::TypeInfo))]
pub struct PInt<U: Unsigned + NonZero> {
    pub(crate) n: U,
}

/// Type-level signed integers with negative sign.
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Default)]
#[cfg_attr(feature = "scale_info", derive(scale_info::TypeInfo))]
pub struct NInt<U: Unsigned + NonZero> {
    pub(crate) n: U,
}

impl<U: Unsigned + NonZero> PInt<U> {
    /// Instantiates a singleton representing this strictly positive integer.
    #[inline]
    pub fn new() -> PInt<U> {
        PInt::default()
    }
}

impl<U: Unsigned + NonZero> NInt<U> {
    /// Instantiates a singleton representing this strictly negative integer.
    #[inline]
    pub fn new() -> NInt<U> {
        NInt::default()
    }
}

/// The type-level signed integer 0.
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Default)]
#[cfg_attr(feature = "scale_info", derive(scale_info::TypeInfo))]
pub struct Z0;

impl Z0 {
    /// Instantiates a singleton representing the integer 0.
    #[inline]
    pub fn new() -> Z0 {
        Z0
    }
}

impl<U: Unsigned + NonZero> NonZero for PInt<U> {}
impl<U: Unsigned + NonZero> NonZero for NInt<U> {}
impl Zero for Z0 {}

impl<U: Unsigned + NonZero + PowerOfTwo> PowerOfTwo for PInt<U> {}

impl Integer for Z0 {
    const I8: i8 = 0;
    const I16: i16 = 0;
    const I32: i32 = 0;
    const I64: i64 = 0;
    #[cfg(feature = "i128")]
    const I128: i128 = 0;
    const ISIZE: isize = 0;

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

impl<U: Unsigned + NonZero> Integer for PInt<U> {
    const I8: i8 = U::I8;
    const I16: i16 = U::I16;
    const I32: i32 = U::I32;
    const I64: i64 = U::I64;
    #[cfg(feature = "i128")]
    const I128: i128 = U::I128;
    const ISIZE: isize = U::ISIZE;

    #[inline]
    fn to_i8() -> i8 {
        <U as Unsigned>::to_i8()
    }
    #[inline]
    fn to_i16() -> i16 {
        <U as Unsigned>::to_i16()
    }
    #[inline]
    fn to_i32() -> i32 {
        <U as Unsigned>::to_i32()
    }
    #[inline]
    fn to_i64() -> i64 {
        <U as Unsigned>::to_i64()
    }
    #[cfg(feature = "i128")]
    #[inline]
    fn to_i128() -> i128 {
        <U as Unsigned>::to_i128()
    }
    #[inline]
    fn to_isize() -> isize {
        <U as Unsigned>::to_isize()
    }
}

// Simply negating the result of e.g. `U::I8` will result in overflow for `std::i8::MIN`. Instead,
// we use the fact that `U: NonZero` by subtracting one from the `U::U8` before negating.
impl<U: Unsigned + NonZero> Integer for NInt<U> {
    const I8: i8 = -((U::U8 - 1) as i8) - 1;
    const I16: i16 = -((U::U16 - 1) as i16) - 1;
    const I32: i32 = -((U::U32 - 1) as i32) - 1;
    const I64: i64 = -((U::U64 - 1) as i64) - 1;
    #[cfg(feature = "i128")]
    const I128: i128 = -((U::U128 - 1) as i128) - 1;
    const ISIZE: isize = -((U::USIZE - 1) as isize) - 1;

    #[inline]
    fn to_i8() -> i8 {
        Self::I8
    }
    #[inline]
    fn to_i16() -> i16 {
        Self::I16
    }
    #[inline]
    fn to_i32() -> i32 {
        Self::I32
    }
    #[inline]
    fn to_i64() -> i64 {
        Self::I64
    }
    #[cfg(feature = "i128")]
    #[inline]
    fn to_i128() -> i128 {
        Self::I128
    }
    #[inline]
    fn to_isize() -> isize {
        Self::ISIZE
    }
}

// ---------------------------------------------------------------------------------------
// Neg

/// `-Z0 = Z0`
impl Neg for Z0 {
    type Output = Z0;
    #[inline]
    fn neg(self) -> Self::Output {
        Z0
    }
}

/// `-PInt = NInt`
impl<U: Unsigned + NonZero> Neg for PInt<U> {
    type Output = NInt<U>;
    #[inline]
    fn neg(self) -> Self::Output {
        NInt::new()
    }
}

/// `-NInt = PInt`
impl<U: Unsigned + NonZero> Neg for NInt<U> {
    type Output = PInt<U>;
    #[inline]
    fn neg(self) -> Self::Output {
        PInt::new()
    }
}

// ---------------------------------------------------------------------------------------
// Add

/// `Z0 + I = I`
impl<I: Integer> Add<I> for Z0 {
    type Output = I;
    #[inline]
    fn add(self, rhs: I) -> Self::Output {
        rhs
    }
}

/// `PInt + Z0 = PInt`
impl<U: Unsigned + NonZero> Add<Z0> for PInt<U> {
    type Output = PInt<U>;
    #[inline]
    fn add(self, _: Z0) -> Self::Output {
        PInt::new()
    }
}

/// `NInt + Z0 = NInt`
impl<U: Unsigned + NonZero> Add<Z0> for NInt<U> {
    type Output = NInt<U>;
    #[inline]
    fn add(self, _: Z0) -> Self::Output {
        NInt::new()
    }
}

/// `P(Ul) + P(Ur) = P(Ul + Ur)`
impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Add<PInt<Ur>> for PInt<Ul>
where
    Ul: Add<Ur>,
    <Ul as Add<Ur>>::Output: Unsigned + NonZero,
{
    type Output = PInt<<Ul as Add<Ur>>::Output>;
    #[inline]
    fn add(self, _: PInt<Ur>) -> Self::Output {
        PInt::new()
    }
}

/// `N(Ul) + N(Ur) = N(Ul + Ur)`
impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Add<NInt<Ur>> for NInt<Ul>
where
    Ul: Add<Ur>,
    <Ul as Add<Ur>>::Output: Unsigned + NonZero,
{
    type Output = NInt<<Ul as Add<Ur>>::Output>;
    #[inline]
    fn add(self, _: NInt<Ur>) -> Self::Output {
        NInt::new()
    }
}

/// `P(Ul) + N(Ur)`: We resolve this with our `PrivateAdd`
impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Add<NInt<Ur>> for PInt<Ul>
where
    Ul: Cmp<Ur> + PrivateIntegerAdd<<Ul as Cmp<Ur>>::Output, Ur>,
{
    type Output = <Ul as PrivateIntegerAdd<<Ul as Cmp<Ur>>::Output, Ur>>::Output;
    #[inline]
    fn add(self, rhs: NInt<Ur>) -> Self::Output {
        let lhs = self.n;
        let rhs = rhs.n;
        let lhs_cmp_rhs = lhs.compare::<Internal>(&rhs);
        lhs.private_integer_add(lhs_cmp_rhs, rhs)
    }
}

/// `N(Ul) + P(Ur)`: We resolve this with our `PrivateAdd`
// We just do the same thing as above, swapping Lhs and Rhs
impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Add<PInt<Ur>> for NInt<Ul>
where
    Ur: Cmp<Ul> + PrivateIntegerAdd<<Ur as Cmp<Ul>>::Output, Ul>,
{
    type Output = <Ur as PrivateIntegerAdd<<Ur as Cmp<Ul>>::Output, Ul>>::Output;
    #[inline]
    fn add(self, rhs: PInt<Ur>) -> Self::Output {
        let lhs = self.n;
        let rhs = rhs.n;
        let rhs_cmp_lhs = rhs.compare::<Internal>(&lhs);
        rhs.private_integer_add(rhs_cmp_lhs, lhs)
    }
}

/// `P + N = 0` where `P == N`
impl<N: Unsigned, P: Unsigned> PrivateIntegerAdd<Equal, N> for P {
    type Output = Z0;

    #[inline]
    fn private_integer_add(self, _: Equal, _: N) -> Self::Output {
        Z0
    }
}

/// `P + N = Positive` where `P > N`
impl<N: Unsigned, P: Unsigned> PrivateIntegerAdd<Greater, N> for P
where
    P: Sub<N>,
    <P as Sub<N>>::Output: Unsigned + NonZero,
{
    type Output = PInt<<P as Sub<N>>::Output>;

    #[inline]
    fn private_integer_add(self, _: Greater, n: N) -> Self::Output {
        PInt { n: self - n }
    }
}

/// `P + N = Negative` where `P < N`
impl<N: Unsigned, P: Unsigned> PrivateIntegerAdd<Less, N> for P
where
    N: Sub<P>,
    <N as Sub<P>>::Output: Unsigned + NonZero,
{
    type Output = NInt<<N as Sub<P>>::Output>;

    #[inline]
    fn private_integer_add(self, _: Less, n: N) -> Self::Output {
        NInt { n: n - self }
    }
}

// ---------------------------------------------------------------------------------------
// Sub

/// `Z0 - Z0 = Z0`
impl Sub<Z0> for Z0 {
    type Output = Z0;
    #[inline]
    fn sub(self, _: Z0) -> Self::Output {
        Z0
    }
}

/// `Z0 - P = N`
impl<U: Unsigned + NonZero> Sub<PInt<U>> for Z0 {
    type Output = NInt<U>;
    #[inline]
    fn sub(self, _: PInt<U>) -> Self::Output {
        NInt::new()
    }
}

/// `Z0 - N = P`
impl<U: Unsigned + NonZero> Sub<NInt<U>> for Z0 {
    type Output = PInt<U>;
    #[inline]
    fn sub(self, _: NInt<U>) -> Self::Output {
        PInt::new()
    }
}

/// `PInt - Z0 = PInt`
impl<U: Unsigned + NonZero> Sub<Z0> for PInt<U> {
    type Output = PInt<U>;
    #[inline]
    fn sub(self, _: Z0) -> Self::Output {
        PInt::new()
    }
}

/// `NInt - Z0 = NInt`
impl<U: Unsigned + NonZero> Sub<Z0> for NInt<U> {
    type Output = NInt<U>;
    #[inline]
    fn sub(self, _: Z0) -> Self::Output {
        NInt::new()
    }
}

/// `P(Ul) - N(Ur) = P(Ul + Ur)`
impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Sub<NInt<Ur>> for PInt<Ul>
where
    Ul: Add<Ur>,
    <Ul as Add<Ur>>::Output: Unsigned + NonZero,
{
    type Output = PInt<<Ul as Add<Ur>>::Output>;
    #[inline]
    fn sub(self, _: NInt<Ur>) -> Self::Output {
        PInt::new()
    }
}

/// `N(Ul) - P(Ur) = N(Ul + Ur)`
impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Sub<PInt<Ur>> for NInt<Ul>
where
    Ul: Add<Ur>,
    <Ul as Add<Ur>>::Output: Unsigned + NonZero,
{
    type Output = NInt<<Ul as Add<Ur>>::Output>;
    #[inline]
    fn sub(self, _: PInt<Ur>) -> Self::Output {
        NInt::new()
    }
}

/// `P(Ul) - P(Ur)`: We resolve this with our `PrivateAdd`
impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Sub<PInt<Ur>> for PInt<Ul>
where
    Ul: Cmp<Ur> + PrivateIntegerAdd<<Ul as Cmp<Ur>>::Output, Ur>,
{
    type Output = <Ul as PrivateIntegerAdd<<Ul as Cmp<Ur>>::Output, Ur>>::Output;
    #[inline]
    fn sub(self, rhs: PInt<Ur>) -> Self::Output {
        let lhs = self.n;
        let rhs = rhs.n;
        let lhs_cmp_rhs = lhs.compare::<Internal>(&rhs);
        lhs.private_integer_add(lhs_cmp_rhs, rhs)
    }
}

/// `N(Ul) - N(Ur)`: We resolve this with our `PrivateAdd`
// We just do the same thing as above, swapping Lhs and Rhs
impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Sub<NInt<Ur>> for NInt<Ul>
where
    Ur: Cmp<Ul> + PrivateIntegerAdd<<Ur as Cmp<Ul>>::Output, Ul>,
{
    type Output = <Ur as PrivateIntegerAdd<<Ur as Cmp<Ul>>::Output, Ul>>::Output;
    #[inline]
    fn sub(self, rhs: NInt<Ur>) -> Self::Output {
        let lhs = self.n;
        let rhs = rhs.n;
        let rhs_cmp_lhs = rhs.compare::<Internal>(&lhs);
        rhs.private_integer_add(rhs_cmp_lhs, lhs)
    }
}

// ---------------------------------------------------------------------------------------
// Mul

/// `Z0 * I = Z0`
impl<I: Integer> Mul<I> for Z0 {
    type Output = Z0;
    #[inline]
    fn mul(self, _: I) -> Self::Output {
        Z0
    }
}

/// `P * Z0 = Z0`
impl<U: Unsigned + NonZero> Mul<Z0> for PInt<U> {
    type Output = Z0;
    #[inline]
    fn mul(self, _: Z0) -> Self::Output {
        Z0
    }
}

/// `N * Z0 = Z0`
impl<U: Unsigned + NonZero> Mul<Z0> for NInt<U> {
    type Output = Z0;
    #[inline]
    fn mul(self, _: Z0) -> Self::Output {
        Z0
    }
}

/// P(Ul) * P(Ur) = P(Ul * Ur)
impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Mul<PInt<Ur>> for PInt<Ul>
where
    Ul: Mul<Ur>,
    <Ul as Mul<Ur>>::Output: Unsigned + NonZero,
{
    type Output = PInt<<Ul as Mul<Ur>>::Output>;
    #[inline]
    fn mul(self, _: PInt<Ur>) -> Self::Output {
        PInt::new()
    }
}

/// N(Ul) * N(Ur) = P(Ul * Ur)
impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Mul<NInt<Ur>> for NInt<Ul>
where
    Ul: Mul<Ur>,
    <Ul as Mul<Ur>>::Output: Unsigned + NonZero,
{
    type Output = PInt<<Ul as Mul<Ur>>::Output>;
    #[inline]
    fn mul(self, _: NInt<Ur>) -> Self::Output {
        PInt::new()
    }
}

/// P(Ul) * N(Ur) = N(Ul * Ur)
impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Mul<NInt<Ur>> for PInt<Ul>
where
    Ul: Mul<Ur>,
    <Ul as Mul<Ur>>::Output: Unsigned + NonZero,
{
    type Output = NInt<<Ul as Mul<Ur>>::Output>;
    #[inline]
    fn mul(self, _: NInt<Ur>) -> Self::Output {
        NInt::new()
    }
}

/// N(Ul) * P(Ur) = N(Ul * Ur)
impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Mul<PInt<Ur>> for NInt<Ul>
where
    Ul: Mul<Ur>,
    <Ul as Mul<Ur>>::Output: Unsigned + NonZero,
{
    type Output = NInt<<Ul as Mul<Ur>>::Output>;
    #[inline]
    fn mul(self, _: PInt<Ur>) -> Self::Output {
        NInt::new()
    }
}

// ---------------------------------------------------------------------------------------
// Div

/// `Z0 / I = Z0` where `I != 0`
impl<I: Integer + NonZero> Div<I> for Z0 {
    type Output = Z0;
    #[inline]
    fn div(self, _: I) -> Self::Output {
        Z0
    }
}

macro_rules! impl_int_div {
    ($A:ident, $B:ident, $R:ident) => {
        /// `$A<Ul> / $B<Ur> = $R<Ul / Ur>`
        impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Div<$B<Ur>> for $A<Ul>
        where
            Ul: Cmp<Ur>,
            $A<Ul>: PrivateDivInt<<Ul as Cmp<Ur>>::Output, $B<Ur>>,
        {
            type Output = <$A<Ul> as PrivateDivInt<<Ul as Cmp<Ur>>::Output, $B<Ur>>>::Output;
            #[inline]
            fn div(self, rhs: $B<Ur>) -> Self::Output {
                let lhs_cmp_rhs = self.n.compare::<Internal>(&rhs.n);
                self.private_div_int(lhs_cmp_rhs, rhs)
            }
        }
        impl<Ul, Ur> PrivateDivInt<Less, $B<Ur>> for $A<Ul>
        where
            Ul: Unsigned + NonZero,
            Ur: Unsigned + NonZero,
        {
            type Output = Z0;

            #[inline]
            fn private_div_int(self, _: Less, _: $B<Ur>) -> Self::Output {
                Z0
            }
        }
        impl<Ul, Ur> PrivateDivInt<Equal, $B<Ur>> for $A<Ul>
        where
            Ul: Unsigned + NonZero,
            Ur: Unsigned + NonZero,
        {
            type Output = $R<U1>;

            #[inline]
            fn private_div_int(self, _: Equal, _: $B<Ur>) -> Self::Output {
                $R { n: U1::new() }
            }
        }
        impl<Ul, Ur> PrivateDivInt<Greater, $B<Ur>> for $A<Ul>
        where
            Ul: Unsigned + NonZero + Div<Ur>,
            Ur: Unsigned + NonZero,
            <Ul as Div<Ur>>::Output: Unsigned + NonZero,
        {
            type Output = $R<<Ul as Div<Ur>>::Output>;

            #[inline]
            fn private_div_int(self, _: Greater, d: $B<Ur>) -> Self::Output {
                $R { n: self.n / d.n }
            }
        }
    };
}

impl_int_div!(PInt, PInt, PInt);
impl_int_div!(PInt, NInt, NInt);
impl_int_div!(NInt, PInt, NInt);
impl_int_div!(NInt, NInt, PInt);

// ---------------------------------------------------------------------------------------
// PartialDiv

use crate::{PartialDiv, Quot};

impl<M, N> PartialDiv<N> for M
where
    M: Integer + Div<N> + Rem<N, Output = Z0>,
{
    type Output = Quot<M, N>;
    #[inline]
    fn partial_div(self, rhs: N) -> Self::Output {
        self / rhs
    }
}

// ---------------------------------------------------------------------------------------
// Cmp

/// 0 == 0
impl Cmp<Z0> for Z0 {
    type Output = Equal;

    #[inline]
    fn compare<IM: InternalMarker>(&self, _: &Z0) -> Self::Output {
        Equal
    }
}

/// 0 > -X
impl<U: Unsigned + NonZero> Cmp<NInt<U>> for Z0 {
    type Output = Greater;

    #[inline]
    fn compare<IM: InternalMarker>(&self, _: &NInt<U>) -> Self::Output {
        Greater
    }
}

/// 0 < X
impl<U: Unsigned + NonZero> Cmp<PInt<U>> for Z0 {
    type Output = Less;

    #[inline]
    fn compare<IM: InternalMarker>(&self, _: &PInt<U>) -> Self::Output {
        Less
    }
}

/// X > 0
impl<U: Unsigned + NonZero> Cmp<Z0> for PInt<U> {
    type Output = Greater;

    #[inline]
    fn compare<IM: InternalMarker>(&self, _: &Z0) -> Self::Output {
        Greater
    }
}

/// -X < 0
impl<U: Unsigned + NonZero> Cmp<Z0> for NInt<U> {
    type Output = Less;

    #[inline]
    fn compare<IM: InternalMarker>(&self, _: &Z0) -> Self::Output {
        Less
    }
}

/// -X < Y
impl<P: Unsigned + NonZero, N: Unsigned + NonZero> Cmp<PInt<P>> for NInt<N> {
    type Output = Less;

    #[inline]
    fn compare<IM: InternalMarker>(&self, _: &PInt<P>) -> Self::Output {
        Less
    }
}

/// X > - Y
impl<P: Unsigned + NonZero, N: Unsigned + NonZero> Cmp<NInt<N>> for PInt<P> {
    type Output = Greater;

    #[inline]
    fn compare<IM: InternalMarker>(&self, _: &NInt<N>) -> Self::Output {
        Greater
    }
}

/// X <==> Y
impl<Pl: Cmp<Pr> + Unsigned + NonZero, Pr: Unsigned + NonZero> Cmp<PInt<Pr>> for PInt<Pl> {
    type Output = <Pl as Cmp<Pr>>::Output;

    #[inline]
    fn compare<IM: InternalMarker>(&self, rhs: &PInt<Pr>) -> Self::Output {
        self.n.compare::<Internal>(&rhs.n)
    }
}

/// -X <==> -Y
impl<Nl: Unsigned + NonZero, Nr: Cmp<Nl> + Unsigned + NonZero> Cmp<NInt<Nr>> for NInt<Nl> {
    type Output = <Nr as Cmp<Nl>>::Output;

    #[inline]
    fn compare<IM: InternalMarker>(&self, rhs: &NInt<Nr>) -> Self::Output {
        rhs.n.compare::<Internal>(&self.n)
    }
}

// ---------------------------------------------------------------------------------------
// Rem

/// `Z0 % I = Z0` where `I != 0`
impl<I: Integer + NonZero> Rem<I> for Z0 {
    type Output = Z0;
    #[inline]
    fn rem(self, _: I) -> Self::Output {
        Z0
    }
}

macro_rules! impl_int_rem {
    ($A:ident, $B:ident, $R:ident) => {
        /// `$A<Ul> % $B<Ur> = $R<Ul % Ur>`
        impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Rem<$B<Ur>> for $A<Ul>
        where
            Ul: Rem<Ur>,
            $A<Ul>: PrivateRem<<Ul as Rem<Ur>>::Output, $B<Ur>>,
        {
            type Output = <$A<Ul> as PrivateRem<<Ul as Rem<Ur>>::Output, $B<Ur>>>::Output;
            #[inline]
            fn rem(self, rhs: $B<Ur>) -> Self::Output {
                self.private_rem(self.n % rhs.n, rhs)
            }
        }
        impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> PrivateRem<U0, $B<Ur>> for $A<Ul> {
            type Output = Z0;

            #[inline]
            fn private_rem(self, _: U0, _: $B<Ur>) -> Self::Output {
                Z0
            }
        }
        impl<Ul, Ur, U, B> PrivateRem<UInt<U, B>, $B<Ur>> for $A<Ul>
        where
            Ul: Unsigned + NonZero,
            Ur: Unsigned + NonZero,
            U: Unsigned,
            B: Bit,
        {
            type Output = $R<UInt<U, B>>;

            #[inline]
            fn private_rem(self, urem: UInt<U, B>, _: $B<Ur>) -> Self::Output {
                $R { n: urem }
            }
        }
    };
}

impl_int_rem!(PInt, PInt, PInt);
impl_int_rem!(PInt, NInt, PInt);
impl_int_rem!(NInt, PInt, NInt);
impl_int_rem!(NInt, NInt, NInt);

// ---------------------------------------------------------------------------------------
// Pow

/// 0^0 = 1
impl Pow<Z0> for Z0 {
    type Output = P1;
    #[inline]
    fn powi(self, _: Z0) -> Self::Output {
        P1::new()
    }
}

/// 0^P = 0
impl<U: Unsigned + NonZero> Pow<PInt<U>> for Z0 {
    type Output = Z0;
    #[inline]
    fn powi(self, _: PInt<U>) -> Self::Output {
        Z0
    }
}

/// 0^N = 0
impl<U: Unsigned + NonZero> Pow<NInt<U>> for Z0 {
    type Output = Z0;
    #[inline]
    fn powi(self, _: NInt<U>) -> Self::Output {
        Z0
    }
}

/// 1^N = 1
impl<U: Unsigned + NonZero> Pow<NInt<U>> for P1 {
    type Output = P1;
    #[inline]
    fn powi(self, _: NInt<U>) -> Self::Output {
        P1::new()
    }
}

/// (-1)^N = 1 if N is even
impl<U: Unsigned> Pow<NInt<UInt<U, B0>>> for N1 {
    type Output = P1;
    #[inline]
    fn powi(self, _: NInt<UInt<U, B0>>) -> Self::Output {
        P1::new()
    }
}

/// (-1)^N = -1 if N is odd
impl<U: Unsigned> Pow<NInt<UInt<U, B1>>> for N1 {
    type Output = N1;
    #[inline]
    fn powi(self, _: NInt<UInt<U, B1>>) -> Self::Output {
        N1::new()
    }
}

/// P^0 = 1
impl<U: Unsigned + NonZero> Pow<Z0> for PInt<U> {
    type Output = P1;
    #[inline]
    fn powi(self, _: Z0) -> Self::Output {
        P1::new()
    }
}

/// N^0 = 1
impl<U: Unsigned + NonZero> Pow<Z0> for NInt<U> {
    type Output = P1;
    #[inline]
    fn powi(self, _: Z0) -> Self::Output {
        P1::new()
    }
}

/// P(Ul)^P(Ur) = P(Ul^Ur)
impl<Ul: Unsigned + NonZero, Ur: Unsigned + NonZero> Pow<PInt<Ur>> for PInt<Ul>
where
    Ul: Pow<Ur>,
    <Ul as Pow<Ur>>::Output: Unsigned + NonZero,
{
    type Output = PInt<<Ul as Pow<Ur>>::Output>;
    #[inline]
    fn powi(self, _: PInt<Ur>) -> Self::Output {
        PInt::new()
    }
}

/// N(Ul)^P(Ur) = P(Ul^Ur) if Ur is even
impl<Ul: Unsigned + NonZero, Ur: Unsigned> Pow<PInt<UInt<Ur, B0>>> for NInt<Ul>
where
    Ul: Pow<UInt<Ur, B0>>,
    <Ul as Pow<UInt<Ur, B0>>>::Output: Unsigned + NonZero,
{
    type Output = PInt<<Ul as Pow<UInt<Ur, B0>>>::Output>;
    #[inline]
    fn powi(self, _: PInt<UInt<Ur, B0>>) -> Self::Output {
        PInt::new()
    }
}

/// N(Ul)^P(Ur) = N(Ul^Ur) if Ur is odd
impl<Ul: Unsigned + NonZero, Ur: Unsigned> Pow<PInt<UInt<Ur, B1>>> for NInt<Ul>
where
    Ul: Pow<UInt<Ur, B1>>,
    <Ul as Pow<UInt<Ur, B1>>>::Output: Unsigned + NonZero,
{
    type Output = NInt<<Ul as Pow<UInt<Ur, B1>>>::Output>;
    #[inline]
    fn powi(self, _: PInt<UInt<Ur, B1>>) -> Self::Output {
        NInt::new()
    }
}

// ---------------------------------------------------------------------------------------
// Gcd
use crate::{Gcd, Gcf};

impl Gcd<Z0> for Z0 {
    type Output = Z0;
}

impl<U> Gcd<PInt<U>> for Z0
where
    U: Unsigned + NonZero,
{
    type Output = PInt<U>;
}

impl<U> Gcd<Z0> for PInt<U>
where
    U: Unsigned + NonZero,
{
    type Output = PInt<U>;
}

impl<U> Gcd<NInt<U>> for Z0
where
    U: Unsigned + NonZero,
{
    type Output = PInt<U>;
}

impl<U> Gcd<Z0> for NInt<U>
where
    U: Unsigned + NonZero,
{
    type Output = PInt<U>;
}

impl<U1, U2> Gcd<PInt<U2>> for PInt<U1>
where
    U1: Unsigned + NonZero + Gcd<U2>,
    U2: Unsigned + NonZero,
    Gcf<U1, U2>: Unsigned + NonZero,
{
    type Output = PInt<Gcf<U1, U2>>;
}

impl<U1, U2> Gcd<PInt<U2>> for NInt<U1>
where
    U1: Unsigned + NonZero + Gcd<U2>,
    U2: Unsigned + NonZero,
    Gcf<U1, U2>: Unsigned + NonZero,
{
    type Output = PInt<Gcf<U1, U2>>;
}

impl<U1, U2> Gcd<NInt<U2>> for PInt<U1>
where
    U1: Unsigned + NonZero + Gcd<U2>,
    U2: Unsigned + NonZero,
    Gcf<U1, U2>: Unsigned + NonZero,
{
    type Output = PInt<Gcf<U1, U2>>;
}

impl<U1, U2> Gcd<NInt<U2>> for NInt<U1>
where
    U1: Unsigned + NonZero + Gcd<U2>,
    U2: Unsigned + NonZero,
    Gcf<U1, U2>: Unsigned + NonZero,
{
    type Output = PInt<Gcf<U1, U2>>;
}

// ---------------------------------------------------------------------------------------
// Min
use crate::{Max, Maximum, Min, Minimum};

impl Min<Z0> for Z0 {
    type Output = Z0;
    #[inline]
    fn min(self, _: Z0) -> Self::Output {
        self
    }
}

impl<U> Min<PInt<U>> for Z0
where
    U: Unsigned + NonZero,
{
    type Output = Z0;
    #[inline]
    fn min(self, _: PInt<U>) -> Self::Output {
        self
    }
}

impl<U> Min<NInt<U>> for Z0
where
    U: Unsigned + NonZero,
{
    type Output = NInt<U>;
    #[inline]
    fn min(self, rhs: NInt<U>) -> Self::Output {
        rhs
    }
}

impl<U> Min<Z0> for PInt<U>
where
    U: Unsigned + NonZero,
{
    type Output = Z0;
    #[inline]
    fn min(self, rhs: Z0) -> Self::Output {
        rhs
    }
}

impl<U> Min<Z0> for NInt<U>
where
    U: Unsigned + NonZero,
{
    type Output = NInt<U>;
    #[inline]
    fn min(self, _: Z0) -> Self::Output {
        self
    }
}

impl<Ul, Ur> Min<PInt<Ur>> for PInt<Ul>
where
    Ul: Unsigned + NonZero + Min<Ur>,
    Ur: Unsigned + NonZero,
    Minimum<Ul, Ur>: Unsigned + NonZero,
{
    type Output = PInt<Minimum<Ul, Ur>>;
    #[inline]
    fn min(self, rhs: PInt<Ur>) -> Self::Output {
        PInt {
            n: self.n.min(rhs.n),
        }
    }
}

impl<Ul, Ur> Min<PInt<Ur>> for NInt<Ul>
where
    Ul: Unsigned + NonZero,
    Ur: Unsigned + NonZero,
{
    type Output = NInt<Ul>;
    #[inline]
    fn min(self, _: PInt<Ur>) -> Self::Output {
        self
    }
}

impl<Ul, Ur> Min<NInt<Ur>> for PInt<Ul>
where
    Ul: Unsigned + NonZero,
    Ur: Unsigned + NonZero,
{
    type Output = NInt<Ur>;
    #[inline]
    fn min(self, rhs: NInt<Ur>) -> Self::Output {
        rhs
    }
}

impl<Ul, Ur> Min<NInt<Ur>> for NInt<Ul>
where
    Ul: Unsigned + NonZero + Max<Ur>,
    Ur: Unsigned + NonZero,
    Maximum<Ul, Ur>: Unsigned + NonZero,
{
    type Output = NInt<Maximum<Ul, Ur>>;
    #[inline]
    fn min(self, rhs: NInt<Ur>) -> Self::Output {
        NInt {
            n: self.n.max(rhs.n),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Max

impl Max<Z0> for Z0 {
    type Output = Z0;
    #[inline]
    fn max(self, _: Z0) -> Self::Output {
        self
    }
}

impl<U> Max<PInt<U>> for Z0
where
    U: Unsigned + NonZero,
{
    type Output = PInt<U>;
    #[inline]
    fn max(self, rhs: PInt<U>) -> Self::Output {
        rhs
    }
}

impl<U> Max<NInt<U>> for Z0
where
    U: Unsigned + NonZero,
{
    type Output = Z0;
    #[inline]
    fn max(self, _: NInt<U>) -> Self::Output {
        self
    }
}

impl<U> Max<Z0> for PInt<U>
where
    U: Unsigned + NonZero,
{
    type Output = PInt<U>;
    #[inline]
    fn max(self, _: Z0) -> Self::Output {
        self
    }
}

impl<U> Max<Z0> for NInt<U>
where
    U: Unsigned + NonZero,
{
    type Output = Z0;
    #[inline]
    fn max(self, rhs: Z0) -> Self::Output {
        rhs
    }
}

impl<Ul, Ur> Max<PInt<Ur>> for PInt<Ul>
where
    Ul: Unsigned + NonZero + Max<Ur>,
    Ur: Unsigned + NonZero,
    Maximum<Ul, Ur>: Unsigned + NonZero,
{
    type Output = PInt<Maximum<Ul, Ur>>;
    #[inline]
    fn max(self, rhs: PInt<Ur>) -> Self::Output {
        PInt {
            n: self.n.max(rhs.n),
        }
    }
}

impl<Ul, Ur> Max<PInt<Ur>> for NInt<Ul>
where
    Ul: Unsigned + NonZero,
    Ur: Unsigned + NonZero,
{
    type Output = PInt<Ur>;
    #[inline]
    fn max(self, rhs: PInt<Ur>) -> Self::Output {
        rhs
    }
}

impl<Ul, Ur> Max<NInt<Ur>> for PInt<Ul>
where
    Ul: Unsigned + NonZero,
    Ur: Unsigned + NonZero,
{
    type Output = PInt<Ul>;
    #[inline]
    fn max(self, _: NInt<Ur>) -> Self::Output {
        self
    }
}

impl<Ul, Ur> Max<NInt<Ur>> for NInt<Ul>
where
    Ul: Unsigned + NonZero + Min<Ur>,
    Ur: Unsigned + NonZero,
    Minimum<Ul, Ur>: Unsigned + NonZero,
{
    type Output = NInt<Minimum<Ul, Ur>>;
    #[inline]
    fn max(self, rhs: NInt<Ur>) -> Self::Output {
        NInt {
            n: self.n.min(rhs.n),
        }
    }
}

// -----------------------------------------
// ToInt

impl ToInt<i8> for Z0 {
    #[inline]
    fn to_int() -> i8 {
        Self::I8
    }
    const INT: i8 = Self::I8;
}

impl ToInt<i16> for Z0 {
    #[inline]
    fn to_int() -> i16 {
        Self::I16
    }
    const INT: i16 = Self::I16;
}

impl ToInt<i32> for Z0 {
    #[inline]
    fn to_int() -> i32 {
        Self::I32
    }
    const INT: i32 = Self::I32;
}

impl ToInt<i64> for Z0 {
    #[inline]
    fn to_int() -> i64 {
        Self::I64
    }
    const INT: i64 = Self::I64;
}

// negative numbers

impl<U> ToInt<i8> for NInt<U>
where
    U: Unsigned + NonZero,
{
    #[inline]
    fn to_int() -> i8 {
        Self::I8
    }
    const INT: i8 = Self::I8;
}

impl<U> ToInt<i16> for NInt<U>
where
    U: Unsigned + NonZero,
{
    #[inline]
    fn to_int() -> i16 {
        Self::I16
    }
    const INT: i16 = Self::I16;
}

impl<U> ToInt<i32> for NInt<U>
where
    U: Unsigned + NonZero,
{
    #[inline]
    fn to_int() -> i32 {
        Self::I32
    }
    const INT: i32 = Self::I32;
}

impl<U> ToInt<i64> for NInt<U>
where
    U: Unsigned + NonZero,
{
    #[inline]
    fn to_int() -> i64 {
        Self::I64
    }
    const INT: i64 = Self::I64;
}

// positive numbers

impl<U> ToInt<i8> for PInt<U>
where
    U: Unsigned + NonZero,
{
    #[inline]
    fn to_int() -> i8 {
        Self::I8
    }
    const INT: i8 = Self::I8;
}

impl<U> ToInt<i16> for PInt<U>
where
    U: Unsigned + NonZero,
{
    #[inline]
    fn to_int() -> i16 {
        Self::I16
    }
    const INT: i16 = Self::I16;
}

impl<U> ToInt<i32> for PInt<U>
where
    U: Unsigned + NonZero,
{
    #[inline]
    fn to_int() -> i32 {
        Self::I32
    }
    const INT: i32 = Self::I32;
}

impl<U> ToInt<i64> for PInt<U>
where
    U: Unsigned + NonZero,
{
    #[inline]
    fn to_int() -> i64 {
        Self::I64
    }
    const INT: i64 = Self::I64;
}

#[cfg(test)]
mod tests {
    use crate::{consts::*, Integer, ToInt};

    #[test]
    fn to_ix_min() {
        assert_eq!(N128::to_i8(), ::core::i8::MIN);
        assert_eq!(N32768::to_i16(), ::core::i16::MIN);
    }

    #[test]
    fn int_toint_test() {
        // i8
        assert_eq!(0_i8, Z0::to_int());
        assert_eq!(1_i8, P1::to_int());
        assert_eq!(2_i8, P2::to_int());
        assert_eq!(3_i8, P3::to_int());
        assert_eq!(4_i8, P4::to_int());
        assert_eq!(-1_i8, N1::to_int());
        assert_eq!(-2_i8, N2::to_int());
        assert_eq!(-3_i8, N3::to_int());
        assert_eq!(-4_i8, N4::to_int());
        assert_eq!(0_i8, Z0::INT);
        assert_eq!(1_i8, P1::INT);
        assert_eq!(2_i8, P2::INT);
        assert_eq!(3_i8, P3::INT);
        assert_eq!(4_i8, P4::INT);
        assert_eq!(-1_i8, N1::INT);
        assert_eq!(-2_i8, N2::INT);
        assert_eq!(-3_i8, N3::INT);
        assert_eq!(-4_i8, N4::INT);

        // i16
        assert_eq!(0_i16, Z0::to_int());
        assert_eq!(1_i16, P1::to_int());
        assert_eq!(2_i16, P2::to_int());
        assert_eq!(3_i16, P3::to_int());
        assert_eq!(4_i16, P4::to_int());
        assert_eq!(-1_i16, N1::to_int());
        assert_eq!(-2_i16, N2::to_int());
        assert_eq!(-3_i16, N3::to_int());
        assert_eq!(-4_i16, N4::to_int());
        assert_eq!(0_i16, Z0::INT);
        assert_eq!(1_i16, P1::INT);
        assert_eq!(2_i16, P2::INT);
        assert_eq!(3_i16, P3::INT);
        assert_eq!(4_i16, P4::INT);
        assert_eq!(-1_i16, N1::INT);
        assert_eq!(-2_i16, N2::INT);
        assert_eq!(-3_i16, N3::INT);
        assert_eq!(-4_i16, N4::INT);

        // i32
        assert_eq!(0_i32, Z0::to_int());
        assert_eq!(1_i32, P1::to_int());
        assert_eq!(2_i32, P2::to_int());
        assert_eq!(3_i32, P3::to_int());
        assert_eq!(4_i32, P4::to_int());
        assert_eq!(-1_i32, N1::to_int());
        assert_eq!(-2_i32, N2::to_int());
        assert_eq!(-3_i32, N3::to_int());
        assert_eq!(-4_i32, N4::to_int());
        assert_eq!(0_i32, Z0::INT);
        assert_eq!(1_i32, P1::INT);
        assert_eq!(2_i32, P2::INT);
        assert_eq!(3_i32, P3::INT);
        assert_eq!(4_i32, P4::INT);
        assert_eq!(-1_i32, N1::INT);
        assert_eq!(-2_i32, N2::INT);
        assert_eq!(-3_i32, N3::INT);
        assert_eq!(-4_i32, N4::INT);

        // i64
        assert_eq!(0_i64, Z0::to_int());
        assert_eq!(1_i64, P1::to_int());
        assert_eq!(2_i64, P2::to_int());
        assert_eq!(3_i64, P3::to_int());
        assert_eq!(4_i64, P4::to_int());
        assert_eq!(-1_i64, N1::to_int());
        assert_eq!(-2_i64, N2::to_int());
        assert_eq!(-3_i64, N3::to_int());
        assert_eq!(-4_i64, N4::to_int());
        assert_eq!(0_i64, Z0::INT);
        assert_eq!(1_i64, P1::INT);
        assert_eq!(2_i64, P2::INT);
        assert_eq!(3_i64, P3::INT);
        assert_eq!(4_i64, P4::INT);
        assert_eq!(-1_i64, N1::INT);
        assert_eq!(-2_i64, N2::INT);
        assert_eq!(-3_i64, N3::INT);
        assert_eq!(-4_i64, N4::INT);
    }
}
