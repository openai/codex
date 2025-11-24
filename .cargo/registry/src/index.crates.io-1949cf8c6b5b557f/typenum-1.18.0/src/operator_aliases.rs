//! Aliases for the type operators used in this crate.

//! Their purpose is to increase the ergonomics of performing operations on the types defined
//! here. For even more ergonomics, consider using the `op!` macro instead.
//!
//! For example, type `X` and type `Y` are the same here:
//!
//! ```rust
//! # #[macro_use] extern crate typenum;
//! use std::ops::Mul;
//! use typenum::{Prod, P5, P7};
//!
//! type X = <P7 as Mul<P5>>::Output;
//! type Y = Prod<P7, P5>;
//!
//! assert_type_eq!(X, Y);
//! ```

// Aliases!!!
use crate::type_operators::{
    Abs, Cmp, FoldAdd, FoldMul, Gcd, Len, Logarithm2, Max, Min, PartialDiv, Pow, SquareRoot,
};
use core::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Rem, Shl, Shr, Sub};

/// Alias for the associated type of `BitAnd`: `And<A, B> = <A as BitAnd<B>>::Output`
pub type And<A, B> = <A as BitAnd<B>>::Output;
/// Alias for the associated type of `BitOr`: `Or<A, B> = <A as BitOr<B>>::Output`
pub type Or<A, B> = <A as BitOr<B>>::Output;
/// Alias for the associated type of `BitXor`: `Xor<A, B> = <A as BitXor<B>>::Output`
pub type Xor<A, B> = <A as BitXor<B>>::Output;

/// Alias for the associated type of `Shl`: `Shleft<A, B> = <A as Shl<B>>::Output`
pub type Shleft<A, B> = <A as Shl<B>>::Output;
/// Alias for the associated type of `Shr`: `Shright<A, B> = <A as Shr<B>>::Output`
pub type Shright<A, B> = <A as Shr<B>>::Output;

/// Alias for the associated type of `Add`: `Sum<A, B> = <A as Add<B>>::Output`
pub type Sum<A, B> = <A as Add<B>>::Output;
/// Alias for the associated type of `Sub`: `Diff<A, B> = <A as Sub<B>>::Output`
pub type Diff<A, B> = <A as Sub<B>>::Output;
/// Alias for the associated type of `Mul`: `Prod<A, B> = <A as Mul<B>>::Output`
pub type Prod<A, B> = <A as Mul<B>>::Output;
/// Alias for the associated type of `Div`: `Quot<A, B> = <A as Div<B>>::Output`
pub type Quot<A, B> = <A as Div<B>>::Output;
/// Alias for the associated type of `Rem`: `Mod<A, B> = <A as Rem<B>>::Output`
pub type Mod<A, B> = <A as Rem<B>>::Output;

/// Alias for the associated type of
/// `PartialDiv`: `PartialQuot<A, B> = <A as PartialDiv<B>>::Output`
pub type PartialQuot<A, B> = <A as PartialDiv<B>>::Output;

/// Alias for the associated type of `Neg`: `Negate<A> = <A as Neg>::Output`
pub type Negate<A> = <A as Neg>::Output;

/// Alias for the associated type of `Abs`: `AbsVal<A> = <A as Abs>::Output`
pub type AbsVal<A> = <A as Abs>::Output;

/// Alias for the associated type of `Pow`: `Exp<A, B> = <A as Pow<B>>::Output`
pub type Exp<A, B> = <A as Pow<B>>::Output;

/// Alias for the associated type of `Gcd`: `Gcf<A, B> = <A as Gcd<B>>::Output>`
pub type Gcf<A, B> = <A as Gcd<B>>::Output;

/// Alias to make it easy to add 1: `Add1<A> = <A as Add<B1>>::Output`
pub type Add1<A> = <A as Add<crate::bit::B1>>::Output;
/// Alias to make it easy to subtract 1: `Sub1<A> = <A as Sub<B1>>::Output`
pub type Sub1<A> = <A as Sub<crate::bit::B1>>::Output;

/// Alias to make it easy to multiply by 2. `Double<A> = Shleft<A, B1>`
pub type Double<A> = Shleft<A, crate::bit::B1>;

/// Alias to make it easy to square. `Square<A> = <A as Mul<A>>::Output`
pub type Square<A> = <A as Mul>::Output;
/// Alias to make it easy to cube. `Cube<A> = <Square<A> as Mul<A>>::Output`
pub type Cube<A> = <Square<A> as Mul<A>>::Output;

/// Alias for the associated type of `SquareRoot`: `Sqrt<A> = <A as SquareRoot>::Output`
pub type Sqrt<A> = <A as SquareRoot>::Output;

/// Alias for the associated type of `Cmp`: `Compare<A, B> = <A as Cmp<B>>::Output`
pub type Compare<A, B> = <A as Cmp<B>>::Output;

/// Alias for the associated type of `Len`: `Length<A> = <A as Len>::Output`
pub type Length<T> = <T as Len>::Output;

/// Alias for the associated type of `FoldAdd`: `FoldSum<A> = <A as FoldAdd>::Output`
pub type FoldSum<A> = <A as FoldAdd>::Output;

/// Alias for the associated type of `FoldMul`: `FoldProd<A> = <A as FoldMul>::Output`
pub type FoldProd<A> = <A as FoldMul>::Output;

/// Alias for the associated type of `Min`: `Minimum<A, B> = <A as Min<B>>::Output`
pub type Minimum<A, B> = <A as Min<B>>::Output;

/// Alias for the associated type of `Max`: `Maximum<A, B> = <A as Max<B>>::Output`
pub type Maximum<A, B> = <A as Max<B>>::Output;

use crate::type_operators::{
    IsEqual, IsGreater, IsGreaterOrEqual, IsLess, IsLessOrEqual, IsNotEqual,
};
/// Alias for the associated type of `IsLess`: `Le<A, B> = <A as IsLess<B>>::Output`
pub type Le<A, B> = <A as IsLess<B>>::Output;
/// Alias for the associated type of `IsEqual`: `Eq<A, B> = <A as IsEqual<B>>::Output`
pub type Eq<A, B> = <A as IsEqual<B>>::Output;
/// Alias for the associated type of `IsGreater`: `Gr<A, B> = <A as IsGreater<B>>::Output`
pub type Gr<A, B> = <A as IsGreater<B>>::Output;
/// Alias for the associated type of `IsGreaterOrEqual`:
/// `GrEq<A, B> = <A as IsGreaterOrEqual<B>>::Output`
pub type GrEq<A, B> = <A as IsGreaterOrEqual<B>>::Output;
/// Alias for the associated type of `IsLessOrEqual`: `LeEq<A, B> = <A as IsLessOrEqual<B>>::Output`
pub type LeEq<A, B> = <A as IsLessOrEqual<B>>::Output;
/// Alias for the associated type of `IsNotEqual`: `NotEq<A, B> = <A as IsNotEqual<B>>::Output`
pub type NotEq<A, B> = <A as IsNotEqual<B>>::Output;
/// Alias for the associated type of `Logarithm2`: `Log2<A> = <A as Logarithm2>::Output`
pub type Log2<A> = <A as Logarithm2>::Output;
