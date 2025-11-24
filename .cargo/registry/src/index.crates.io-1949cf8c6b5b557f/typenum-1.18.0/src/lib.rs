//! This crate provides type-level numbers evaluated at compile time. It depends only on libcore.
//!
//! The traits defined or used in this crate are used in a typical manner. They can be divided into
//! two categories: **marker traits** and **type operators**.
//!
//! Many of the marker traits have functions defined, but they all do essentially the same thing:
//! convert a type into its runtime counterpart, and are really just there for debugging. For
//! example,
//!
//! ```rust
//! use typenum::{Integer, N4};
//!
//! assert_eq!(N4::to_i32(), -4);
//! ```
//!
//! **Type operators** are traits that behave as functions at the type level. These are the meat of
//! this library. Where possible, traits defined in libcore have been used, but their attached
//! functions have not been implemented.
//!
//! For example, the `Add` trait is implemented for both unsigned and signed integers, but the
//! `add` function is not. As there are never any objects of the types defined here, it wouldn't
//! make sense to implement it. What is important is its associated type `Output`, which is where
//! the addition happens.
//!
//! ```rust
//! use std::ops::Add;
//! use typenum::{Integer, P3, P4};
//!
//! type X = <P3 as Add<P4>>::Output;
//! assert_eq!(<X as Integer>::to_i32(), 7);
//! ```
//!
//! In addition, helper aliases are defined for type operators. For example, the above snippet
//! could be replaced with
//!
//! ```rust
//! use typenum::{Integer, Sum, P3, P4};
//!
//! type X = Sum<P3, P4>;
//! assert_eq!(<X as Integer>::to_i32(), 7);
//! ```
//!
//! Documented in each module is the full list of type operators implemented.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(feature = "strict", deny(missing_docs))]
#![cfg_attr(feature = "strict", deny(warnings))]
#![doc(html_root_url = "https://docs.rs/typenum/1.18.0")]
#![cfg_attr(docsrs, feature(doc_auto_cfg, doc_cfg))]

// For debugging macros:
// #![feature(trace_macros)]
// trace_macros!(true);

use core::cmp::Ordering;

pub mod bit;
mod gen;
pub mod int;
pub mod marker_traits;
pub mod operator_aliases;
pub mod private;
pub mod type_operators;
pub mod uint;

pub mod array;

pub use crate::{
    array::{ATerm, TArr},
    gen::consts,
    int::{NInt, PInt},
    marker_traits::*,
    operator_aliases::*,
    type_operators::*,
    uint::{UInt, UTerm},
};

#[doc(no_inline)]
#[rustfmt::skip]
pub use consts::*;

#[cfg(feature = "const-generics")]
pub use crate::gen::generic_const_mappings;

#[cfg(feature = "const-generics")]
#[doc(no_inline)]
pub use generic_const_mappings::{Const, ToUInt, U};

/// A potential output from `Cmp`, this is the type equivalent to the enum variant
/// `core::cmp::Ordering::Greater`.
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Default)]
#[cfg_attr(feature = "scale_info", derive(scale_info::TypeInfo))]
pub struct Greater;

/// A potential output from `Cmp`, this is the type equivalent to the enum variant
/// `core::cmp::Ordering::Less`.
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Default)]
#[cfg_attr(feature = "scale_info", derive(scale_info::TypeInfo))]
pub struct Less;

/// A potential output from `Cmp`, this is the type equivalent to the enum variant
/// `core::cmp::Ordering::Equal`.
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Hash, Debug, Default)]
#[cfg_attr(feature = "scale_info", derive(scale_info::TypeInfo))]
pub struct Equal;

/// Returns `core::cmp::Ordering::Greater`
impl Ord for Greater {
    #[inline]
    fn to_ordering() -> Ordering {
        Ordering::Greater
    }
}

/// Returns `core::cmp::Ordering::Less`
impl Ord for Less {
    #[inline]
    fn to_ordering() -> Ordering {
        Ordering::Less
    }
}

/// Returns `core::cmp::Ordering::Equal`
impl Ord for Equal {
    #[inline]
    fn to_ordering() -> Ordering {
        Ordering::Equal
    }
}

/// Asserts that two types are the same.
#[macro_export]
macro_rules! assert_type_eq {
    ($a:ty, $b:ty) => {
        const _: core::marker::PhantomData<<$a as $crate::Same<$b>>::Output> =
            core::marker::PhantomData;
    };
}

/// Asserts that a type is `True`, aka `B1`.
#[macro_export]
macro_rules! assert_type {
    ($a:ty) => {
        const _: core::marker::PhantomData<<$a as $crate::Same<True>>::Output> =
            core::marker::PhantomData;
    };
}

mod sealed {
    use crate::{
        ATerm, Bit, Equal, Greater, Less, NInt, NonZero, PInt, TArr, UInt, UTerm, Unsigned, B0, B1,
        Z0,
    };

    pub trait Sealed {}

    impl Sealed for B0 {}
    impl Sealed for B1 {}

    impl Sealed for UTerm {}
    impl<U: Unsigned, B: Bit> Sealed for UInt<U, B> {}

    impl Sealed for Z0 {}
    impl<U: Unsigned + NonZero> Sealed for PInt<U> {}
    impl<U: Unsigned + NonZero> Sealed for NInt<U> {}

    impl Sealed for Less {}
    impl Sealed for Equal {}
    impl Sealed for Greater {}

    impl Sealed for ATerm {}
    impl<V, A> Sealed for TArr<V, A> {}
}
