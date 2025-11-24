// -*- mode: rust; -*-
//
// This file is part of subtle, part of the dalek cryptography project.
// Copyright (c) 2016-2018 isis lovecruft, Henry de Valence
// See LICENSE for licensing information.
//
// Authors:
// - isis agora lovecruft <isis@patternsinthevoid.net>
// - Henry de Valence <hdevalence@hdevalence.ca>

#![no_std]
#![deny(missing_docs)]
#![doc(html_logo_url = "https://doc.dalek.rs/assets/dalek-logo-clear.png")]
#![doc(html_root_url = "https://docs.rs/subtle/2.6.0")]

//! # subtle [![](https://img.shields.io/crates/v/subtle.svg)](https://crates.io/crates/subtle) [![](https://img.shields.io/badge/dynamic/json.svg?label=docs&uri=https%3A%2F%2Fcrates.io%2Fapi%2Fv1%2Fcrates%2Fsubtle%2Fversions&query=%24.versions%5B0%5D.num&colorB=4F74A6)](https://doc.dalek.rs/subtle) [![](https://travis-ci.org/dalek-cryptography/subtle.svg?branch=master)](https://travis-ci.org/dalek-cryptography/subtle)
//!
//! **Pure-Rust traits and utilities for constant-time cryptographic implementations.**
//!
//! It consists of a `Choice` type, and a collection of traits using `Choice`
//! instead of `bool` which are intended to execute in constant-time.  The `Choice`
//! type is a wrapper around a `u8` that holds a `0` or `1`.
//!
//! ```toml
//! subtle = "2.6"
//! ```
//!
//! This crate represents a “best-effort” attempt, since side-channels
//! are ultimately a property of a deployed cryptographic system
//! including the hardware it runs on, not just of software.
//!
//! The traits are implemented using bitwise operations, and should execute in
//! constant time provided that a) the bitwise operations are constant-time and
//! b) the bitwise operations are not recognized as a conditional assignment and
//! optimized back into a branch.
//!
//! For a compiler to recognize that bitwise operations represent a conditional
//! assignment, it needs to know that the value used to generate the bitmasks is
//! really a boolean `i1` rather than an `i8` byte value. In an attempt to
//! prevent this refinement, the crate tries to hide the value of a `Choice`'s
//! inner `u8` by passing it through a volatile read. For more information, see
//! the _About_ section below.
//!
//! Rust versions from 1.51 or higher have const generics support. You may enable
//! `const-generics` feautre to have `subtle` traits implemented for arrays `[T; N]`.
//!
//! Versions prior to `2.2` recommended use of the `nightly` feature to enable an
//! optimization barrier; this is not required in versions `2.2` and above.
//!
//! Note: the `subtle` crate contains `debug_assert`s to check invariants during
//! debug builds. These invariant checks involve secret-dependent branches, and
//! are not present when compiled in release mode. This crate is intended to be
//! used in release mode.
//!
//! ## Documentation
//!
//! Documentation is available [here][docs].
//!
//! ## Minimum Supported Rust Version
//!
//! Rust **1.41** or higher.
//!
//! Minimum supported Rust version can be changed in the future, but it will be done with a minor version bump.
//!
//! ## About
//!
//! This library aims to be the Rust equivalent of Go’s `crypto/subtle` module.
//!
//! Old versions of the optimization barrier in `impl From<u8> for Choice` were
//! based on Tim Maclean's [work on `rust-timing-shield`][rust-timing-shield],
//! which attempts to provide a more comprehensive approach for preventing
//! software side-channels in Rust code.
//! From version `2.2`, it was based on Diane Hosfelt and Amber Sprenkels' work on
//! "Secret Types in Rust".
//!
//! `subtle` is authored by isis agora lovecruft and Henry de Valence.
//!
//! ## Warning
//!
//! This code is a low-level library, intended for specific use-cases implementing
//! cryptographic protocols.  It represents a best-effort attempt to protect
//! against some software side-channels.  Because side-channel resistance is not a
//! property of software alone, but of software together with hardware, any such
//! effort is fundamentally limited.
//!
//! **USE AT YOUR OWN RISK**
//!
//! [docs]: https://docs.rs/subtle
//! [rust-timing-shield]: https://www.chosenplaintext.ca/open-source/rust-timing-shield/security

#[cfg(feature = "std")]
#[macro_use]
extern crate std;

use core::cmp;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Neg, Not};
use core::option::Option;

#[cfg(feature = "core_hint_black_box")]
use core::hint::black_box;

/// The `Choice` struct represents a choice for use in conditional assignment.
///
/// It is a wrapper around a `u8`, which should have the value either `1` (true)
/// or `0` (false).
///
/// The conversion from `u8` to `Choice` passes the value through an optimization
/// barrier, as a best-effort attempt to prevent the compiler from inferring that
/// the `Choice` value is a boolean. This strategy is based on Tim Maclean's
/// [work on `rust-timing-shield`][rust-timing-shield], which attempts to provide
/// a more comprehensive approach for preventing software side-channels in Rust
/// code.
///
/// The `Choice` struct implements operators for AND, OR, XOR, and NOT, to allow
/// combining `Choice` values. These operations do not short-circuit.
///
/// [rust-timing-shield]:
/// https://www.chosenplaintext.ca/open-source/rust-timing-shield/security
#[derive(Copy, Clone, Debug)]
pub struct Choice(u8);

impl Choice {
    /// Unwrap the `Choice` wrapper to reveal the underlying `u8`.
    ///
    /// # Note
    ///
    /// This function only exists as an **escape hatch** for the rare case
    /// where it's not possible to use one of the `subtle`-provided
    /// trait impls.
    ///
    /// **To convert a `Choice` to a `bool`, use the `From` implementation instead.**
    #[inline]
    pub fn unwrap_u8(&self) -> u8 {
        self.0
    }
}

impl From<Choice> for bool {
    /// Convert the `Choice` wrapper into a `bool`, depending on whether
    /// the underlying `u8` was a `0` or a `1`.
    ///
    /// # Note
    ///
    /// This function exists to avoid having higher-level cryptographic protocol
    /// implementations duplicating this pattern.
    ///
    /// The intended use case for this conversion is at the _end_ of a
    /// higher-level primitive implementation: for example, in checking a keyed
    /// MAC, where the verification should happen in constant-time (and thus use
    /// a `Choice`) but it is safe to return a `bool` at the end of the
    /// verification.
    #[inline]
    fn from(source: Choice) -> bool {
        debug_assert!((source.0 == 0u8) | (source.0 == 1u8));
        source.0 != 0
    }
}

impl BitAnd for Choice {
    type Output = Choice;
    #[inline]
    fn bitand(self, rhs: Choice) -> Choice {
        (self.0 & rhs.0).into()
    }
}

impl BitAndAssign for Choice {
    #[inline]
    fn bitand_assign(&mut self, rhs: Choice) {
        *self = *self & rhs;
    }
}

impl BitOr for Choice {
    type Output = Choice;
    #[inline]
    fn bitor(self, rhs: Choice) -> Choice {
        (self.0 | rhs.0).into()
    }
}

impl BitOrAssign for Choice {
    #[inline]
    fn bitor_assign(&mut self, rhs: Choice) {
        *self = *self | rhs;
    }
}

impl BitXor for Choice {
    type Output = Choice;
    #[inline]
    fn bitxor(self, rhs: Choice) -> Choice {
        (self.0 ^ rhs.0).into()
    }
}

impl BitXorAssign for Choice {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Choice) {
        *self = *self ^ rhs;
    }
}

impl Not for Choice {
    type Output = Choice;
    #[inline]
    fn not(self) -> Choice {
        (1u8 & (!self.0)).into()
    }
}

/// This function is a best-effort attempt to prevent the compiler from knowing
/// anything about the value of the returned `u8`, other than its type.
///
/// Because we want to support stable Rust, we don't have access to inline
/// assembly or test::black_box, so we use the fact that volatile values will
/// never be elided to register values.
///
/// Note: Rust's notion of "volatile" is subject to change over time. While this
/// code may break in a non-destructive way in the future, “constant-time” code
/// is a continually moving target, and this is better than doing nothing.
#[cfg(not(feature = "core_hint_black_box"))]
#[inline(never)]
fn black_box<T: Copy>(input: T) -> T {
    unsafe {
        // Optimization barrier
        //
        // SAFETY:
        //   - &input is not NULL because we own input;
        //   - input is Copy and always live;
        //   - input is always properly aligned.
        core::ptr::read_volatile(&input)
    }
}

impl From<u8> for Choice {
    #[inline]
    fn from(input: u8) -> Choice {
        debug_assert!((input == 0u8) | (input == 1u8));

        // Our goal is to prevent the compiler from inferring that the value held inside the
        // resulting `Choice` struct is really a `bool` instead of a `u8`.
        Choice(black_box(input))
    }
}

/// An `Eq`-like trait that produces a `Choice` instead of a `bool`.
///
/// # Example
///
/// ```
/// use subtle::ConstantTimeEq;
/// let x: u8 = 5;
/// let y: u8 = 13;
///
/// assert_eq!(x.ct_eq(&y).unwrap_u8(), 0);
/// assert_eq!(x.ct_eq(&x).unwrap_u8(), 1);
/// ```
//
// #[inline] is specified on these function prototypes to signify that they
#[allow(unused_attributes)] // should be in the actual implementation
pub trait ConstantTimeEq {
    /// Determine if two items are equal.
    ///
    /// The `ct_eq` function should execute in constant time.
    ///
    /// # Returns
    ///
    /// * `Choice(1u8)` if `self == other`;
    /// * `Choice(0u8)` if `self != other`.
    #[inline]
    #[allow(unused_attributes)]
    fn ct_eq(&self, other: &Self) -> Choice;

    /// Determine if two items are NOT equal.
    ///
    /// The `ct_ne` function should execute in constant time.
    ///
    /// # Returns
    ///
    /// * `Choice(0u8)` if `self == other`;
    /// * `Choice(1u8)` if `self != other`.
    #[inline]
    fn ct_ne(&self, other: &Self) -> Choice {
        !self.ct_eq(other)
    }
}

impl<T: ConstantTimeEq> ConstantTimeEq for [T] {
    /// Check whether two slices of `ConstantTimeEq` types are equal.
    ///
    /// # Note
    ///
    /// This function short-circuits if the lengths of the input slices
    /// are different.  Otherwise, it should execute in time independent
    /// of the slice contents.
    ///
    /// Since arrays coerce to slices, this function works with fixed-size arrays:
    ///
    /// ```
    /// # use subtle::ConstantTimeEq;
    /// #
    /// let a: [u8; 8] = [0,1,2,3,4,5,6,7];
    /// let b: [u8; 8] = [0,1,2,3,0,1,2,3];
    ///
    /// let a_eq_a = a.ct_eq(&a);
    /// let a_eq_b = a.ct_eq(&b);
    ///
    /// assert_eq!(a_eq_a.unwrap_u8(), 1);
    /// assert_eq!(a_eq_b.unwrap_u8(), 0);
    /// ```
    #[inline]
    fn ct_eq(&self, _rhs: &[T]) -> Choice {
        let len = self.len();

        // Short-circuit on the *lengths* of the slices, not their
        // contents.
        if len != _rhs.len() {
            return Choice::from(0);
        }

        // This loop shouldn't be shortcircuitable, since the compiler
        // shouldn't be able to reason about the value of the `u8`
        // unwrapped from the `ct_eq` result.
        let mut x = 1u8;
        for (ai, bi) in self.iter().zip(_rhs.iter()) {
            x &= ai.ct_eq(bi).unwrap_u8();
        }

        x.into()
    }
}

impl ConstantTimeEq for Choice {
    #[inline]
    fn ct_eq(&self, rhs: &Choice) -> Choice {
        !(*self ^ *rhs)
    }
}

/// Given the bit-width `$bit_width` and the corresponding primitive
/// unsigned and signed types `$t_u` and `$t_i` respectively, generate
/// an `ConstantTimeEq` implementation.
macro_rules! generate_integer_equal {
    ($t_u:ty, $t_i:ty, $bit_width:expr) => {
        impl ConstantTimeEq for $t_u {
            #[inline]
            fn ct_eq(&self, other: &$t_u) -> Choice {
                // x == 0 if and only if self == other
                let x: $t_u = self ^ other;

                // If x == 0, then x and -x are both equal to zero;
                // otherwise, one or both will have its high bit set.
                let y: $t_u = (x | x.wrapping_neg()) >> ($bit_width - 1);

                // Result is the opposite of the high bit (now shifted to low).
                ((y ^ (1 as $t_u)) as u8).into()
            }
        }
        impl ConstantTimeEq for $t_i {
            #[inline]
            fn ct_eq(&self, other: &$t_i) -> Choice {
                // Bitcast to unsigned and call that implementation.
                (*self as $t_u).ct_eq(&(*other as $t_u))
            }
        }
    };
}

generate_integer_equal!(u8, i8, 8);
generate_integer_equal!(u16, i16, 16);
generate_integer_equal!(u32, i32, 32);
generate_integer_equal!(u64, i64, 64);
#[cfg(feature = "i128")]
generate_integer_equal!(u128, i128, 128);
generate_integer_equal!(usize, isize, ::core::mem::size_of::<usize>() * 8);

/// `Ordering` is `#[repr(i8)]` making it possible to leverage `i8::ct_eq`.
impl ConstantTimeEq for cmp::Ordering {
    #[inline]
    fn ct_eq(&self, other: &Self) -> Choice {
        (*self as i8).ct_eq(&(*other as i8))
    }
}

/// A type which can be conditionally selected in constant time.
///
/// This trait also provides generic implementations of conditional
/// assignment and conditional swaps.
//
// #[inline] is specified on these function prototypes to signify that they
#[allow(unused_attributes)] // should be in the actual implementation
pub trait ConditionallySelectable: Copy {
    /// Select `a` or `b` according to `choice`.
    ///
    /// # Returns
    ///
    /// * `a` if `choice == Choice(0)`;
    /// * `b` if `choice == Choice(1)`.
    ///
    /// This function should execute in constant time.
    ///
    /// # Example
    ///
    /// ```
    /// use subtle::ConditionallySelectable;
    /// #
    /// # fn main() {
    /// let x: u8 = 13;
    /// let y: u8 = 42;
    ///
    /// let z = u8::conditional_select(&x, &y, 0.into());
    /// assert_eq!(z, x);
    /// let z = u8::conditional_select(&x, &y, 1.into());
    /// assert_eq!(z, y);
    /// # }
    /// ```
    #[inline]
    #[allow(unused_attributes)]
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self;

    /// Conditionally assign `other` to `self`, according to `choice`.
    ///
    /// This function should execute in constant time.
    ///
    /// # Example
    ///
    /// ```
    /// use subtle::ConditionallySelectable;
    /// #
    /// # fn main() {
    /// let mut x: u8 = 13;
    /// let mut y: u8 = 42;
    ///
    /// x.conditional_assign(&y, 0.into());
    /// assert_eq!(x, 13);
    /// x.conditional_assign(&y, 1.into());
    /// assert_eq!(x, 42);
    /// # }
    /// ```
    #[inline]
    fn conditional_assign(&mut self, other: &Self, choice: Choice) {
        *self = Self::conditional_select(self, other, choice);
    }

    /// Conditionally swap `self` and `other` if `choice == 1`; otherwise,
    /// reassign both unto themselves.
    ///
    /// This function should execute in constant time.
    ///
    /// # Example
    ///
    /// ```
    /// use subtle::ConditionallySelectable;
    /// #
    /// # fn main() {
    /// let mut x: u8 = 13;
    /// let mut y: u8 = 42;
    ///
    /// u8::conditional_swap(&mut x, &mut y, 0.into());
    /// assert_eq!(x, 13);
    /// assert_eq!(y, 42);
    /// u8::conditional_swap(&mut x, &mut y, 1.into());
    /// assert_eq!(x, 42);
    /// assert_eq!(y, 13);
    /// # }
    /// ```
    #[inline]
    fn conditional_swap(a: &mut Self, b: &mut Self, choice: Choice) {
        let t: Self = *a;
        a.conditional_assign(&b, choice);
        b.conditional_assign(&t, choice);
    }
}

macro_rules! to_signed_int {
    (u8) => {
        i8
    };
    (u16) => {
        i16
    };
    (u32) => {
        i32
    };
    (u64) => {
        i64
    };
    (u128) => {
        i128
    };
    (i8) => {
        i8
    };
    (i16) => {
        i16
    };
    (i32) => {
        i32
    };
    (i64) => {
        i64
    };
    (i128) => {
        i128
    };
}

macro_rules! generate_integer_conditional_select {
    ($($t:tt)*) => ($(
        impl ConditionallySelectable for $t {
            #[inline]
            fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
                // if choice = 0, mask = (-0) = 0000...0000
                // if choice = 1, mask = (-1) = 1111...1111
                let mask = -(choice.unwrap_u8() as to_signed_int!($t)) as $t;
                a ^ (mask & (a ^ b))
            }

            #[inline]
            fn conditional_assign(&mut self, other: &Self, choice: Choice) {
                // if choice = 0, mask = (-0) = 0000...0000
                // if choice = 1, mask = (-1) = 1111...1111
                let mask = -(choice.unwrap_u8() as to_signed_int!($t)) as $t;
                *self ^= mask & (*self ^ *other);
            }

            #[inline]
            fn conditional_swap(a: &mut Self, b: &mut Self, choice: Choice) {
                // if choice = 0, mask = (-0) = 0000...0000
                // if choice = 1, mask = (-1) = 1111...1111
                let mask = -(choice.unwrap_u8() as to_signed_int!($t)) as $t;
                let t = mask & (*a ^ *b);
                *a ^= t;
                *b ^= t;
            }
         }
    )*)
}

generate_integer_conditional_select!(  u8   i8);
generate_integer_conditional_select!( u16  i16);
generate_integer_conditional_select!( u32  i32);
generate_integer_conditional_select!( u64  i64);
#[cfg(feature = "i128")]
generate_integer_conditional_select!(u128 i128);

/// `Ordering` is `#[repr(i8)]` where:
///
/// - `Less` => -1
/// - `Equal` => 0
/// - `Greater` => 1
///
/// Given this, it's possible to operate on orderings as if they're integers,
/// which allows leveraging conditional masking for predication.
impl ConditionallySelectable for cmp::Ordering {
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        let a = *a as i8;
        let b = *b as i8;
        let ret = i8::conditional_select(&a, &b, choice);

        // SAFETY: `Ordering` is `#[repr(i8)]` and `ret` has been assigned to
        // a value which was originally a valid `Ordering` then cast to `i8`
        unsafe { *((&ret as *const _) as *const cmp::Ordering) }
    }
}

impl ConditionallySelectable for Choice {
    #[inline]
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        Choice(u8::conditional_select(&a.0, &b.0, choice))
    }
}

#[cfg(feature = "const-generics")]
impl<T, const N: usize> ConditionallySelectable for [T; N]
where
    T: ConditionallySelectable,
{
    #[inline]
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        let mut output = *a;
        output.conditional_assign(b, choice);
        output
    }

    fn conditional_assign(&mut self, other: &Self, choice: Choice) {
        for (a_i, b_i) in self.iter_mut().zip(other) {
            a_i.conditional_assign(b_i, choice)
        }
    }
}

/// A type which can be conditionally negated in constant time.
///
/// # Note
///
/// A generic implementation of `ConditionallyNegatable` is provided
/// for types `T` which are `ConditionallySelectable` and have `Neg`
/// implemented on `&T`.
//
// #[inline] is specified on these function prototypes to signify that they
#[allow(unused_attributes)] // should be in the actual implementation
pub trait ConditionallyNegatable {
    /// Negate `self` if `choice == Choice(1)`; otherwise, leave it
    /// unchanged.
    ///
    /// This function should execute in constant time.
    #[inline]
    #[allow(unused_attributes)]
    fn conditional_negate(&mut self, choice: Choice);
}

impl<T> ConditionallyNegatable for T
where
    T: ConditionallySelectable,
    for<'a> &'a T: Neg<Output = T>,
{
    #[inline]
    fn conditional_negate(&mut self, choice: Choice) {
        // Need to cast to eliminate mutability
        let self_neg: T = -(self as &T);
        self.conditional_assign(&self_neg, choice);
    }
}

/// The `CtOption<T>` type represents an optional value similar to the
/// [`Option<T>`](core::option::Option) type but is intended for
/// use in constant time APIs.
///
/// Any given `CtOption<T>` is either `Some` or `None`, but unlike
/// `Option<T>` these variants are not exposed. The
/// [`is_some()`](CtOption::is_some) method is used to determine if
/// the value is `Some`, and [`unwrap_or()`](CtOption::unwrap_or) and
/// [`unwrap_or_else()`](CtOption::unwrap_or_else) methods are
/// provided to access the underlying value. The value can also be
/// obtained with [`unwrap()`](CtOption::unwrap) but this will panic
/// if it is `None`.
///
/// Functions that are intended to be constant time may not produce
/// valid results for all inputs, such as square root and inversion
/// operations in finite field arithmetic. Returning an `Option<T>`
/// from these functions makes it difficult for the caller to reason
/// about the result in constant time, and returning an incorrect
/// value burdens the caller and increases the chance of bugs.
#[derive(Clone, Copy, Debug)]
pub struct CtOption<T> {
    value: T,
    is_some: Choice,
}

impl<T> From<CtOption<T>> for Option<T> {
    /// Convert the `CtOption<T>` wrapper into an `Option<T>`, depending on whether
    /// the underlying `is_some` `Choice` was a `0` or a `1` once unwrapped.
    ///
    /// # Note
    ///
    /// This function exists to avoid ending up with ugly, verbose and/or bad handled
    /// conversions from the `CtOption<T>` wraps to an `Option<T>` or `Result<T, E>`.
    /// This implementation doesn't intend to be constant-time nor try to protect the
    /// leakage of the `T` since the `Option<T>` will do it anyways.
    fn from(source: CtOption<T>) -> Option<T> {
        if source.is_some().unwrap_u8() == 1u8 {
            Option::Some(source.value)
        } else {
            None
        }
    }
}

impl<T> CtOption<T> {
    /// This method is used to construct a new `CtOption<T>` and takes
    /// a value of type `T`, and a `Choice` that determines whether
    /// the optional value should be `Some` or not. If `is_some` is
    /// false, the value will still be stored but its value is never
    /// exposed.
    #[inline]
    pub fn new(value: T, is_some: Choice) -> CtOption<T> {
        CtOption {
            value: value,
            is_some: is_some,
        }
    }

    /// Returns the contained value, consuming the `self` value.
    ///
    /// # Panics
    ///
    /// Panics if the value is none with a custom panic message provided by
    /// `msg`.
    pub fn expect(self, msg: &str) -> T {
        assert_eq!(self.is_some.unwrap_u8(), 1, "{}", msg);

        self.value
    }

    /// This returns the underlying value but panics if it
    /// is not `Some`.
    #[inline]
    pub fn unwrap(self) -> T {
        assert_eq!(self.is_some.unwrap_u8(), 1);

        self.value
    }

    /// This returns the underlying value if it is `Some`
    /// or the provided value otherwise.
    #[inline]
    pub fn unwrap_or(self, def: T) -> T
    where
        T: ConditionallySelectable,
    {
        T::conditional_select(&def, &self.value, self.is_some)
    }

    /// This returns the underlying value if it is `Some`
    /// or the value produced by the provided closure otherwise.
    ///
    /// This operates in constant time, because the provided closure
    /// is always called.
    #[inline]
    pub fn unwrap_or_else<F>(self, f: F) -> T
    where
        T: ConditionallySelectable,
        F: FnOnce() -> T,
    {
        T::conditional_select(&f(), &self.value, self.is_some)
    }

    /// Returns a true `Choice` if this value is `Some`.
    #[inline]
    pub fn is_some(&self) -> Choice {
        self.is_some
    }

    /// Returns a true `Choice` if this value is `None`.
    #[inline]
    pub fn is_none(&self) -> Choice {
        !self.is_some
    }

    /// Returns a `None` value if the option is `None`, otherwise
    /// returns a `CtOption` enclosing the value of the provided closure.
    /// The closure is given the enclosed value or, if the option is
    /// `None`, it is provided a dummy value computed using
    /// `Default::default()`.
    ///
    /// This operates in constant time, because the provided closure
    /// is always called.
    #[inline]
    pub fn map<U, F>(self, f: F) -> CtOption<U>
    where
        T: Default + ConditionallySelectable,
        F: FnOnce(T) -> U,
    {
        CtOption::new(
            f(T::conditional_select(
                &T::default(),
                &self.value,
                self.is_some,
            )),
            self.is_some,
        )
    }

    /// Returns a `None` value if the option is `None`, otherwise
    /// returns the result of the provided closure. The closure is
    /// given the enclosed value or, if the option is `None`, it
    /// is provided a dummy value computed using `Default::default()`.
    ///
    /// This operates in constant time, because the provided closure
    /// is always called.
    #[inline]
    pub fn and_then<U, F>(self, f: F) -> CtOption<U>
    where
        T: Default + ConditionallySelectable,
        F: FnOnce(T) -> CtOption<U>,
    {
        let mut tmp = f(T::conditional_select(
            &T::default(),
            &self.value,
            self.is_some,
        ));
        tmp.is_some &= self.is_some;

        tmp
    }

    /// Returns `self` if it contains a value, and otherwise returns the result of
    /// calling `f`. The provided function `f` is always called.
    #[inline]
    pub fn or_else<F>(self, f: F) -> CtOption<T>
    where
        T: ConditionallySelectable,
        F: FnOnce() -> CtOption<T>,
    {
        let is_none = self.is_none();
        let f = f();

        Self::conditional_select(&self, &f, is_none)
    }

    /// Convert the `CtOption<T>` wrapper into an `Option<T>`, depending on whether
    /// the underlying `is_some` `Choice` was a `0` or a `1` once unwrapped.
    ///
    /// # Note
    ///
    /// This function exists to avoid ending up with ugly, verbose and/or bad handled
    /// conversions from the `CtOption<T>` wraps to an `Option<T>` or `Result<T, E>`.
    /// This implementation doesn't intend to be constant-time nor try to protect the
    /// leakage of the `T` since the `Option<T>` will do it anyways.
    ///
    /// It's equivalent to the corresponding `From` impl, however this version is
    /// friendlier for type inference.
    pub fn into_option(self) -> Option<T> {
        self.into()
    }
}

impl<T: ConditionallySelectable> ConditionallySelectable for CtOption<T> {
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        CtOption::new(
            T::conditional_select(&a.value, &b.value, choice),
            Choice::conditional_select(&a.is_some, &b.is_some, choice),
        )
    }
}

impl<T: ConstantTimeEq> ConstantTimeEq for CtOption<T> {
    /// Two `CtOption<T>`s are equal if they are both `Some` and
    /// their values are equal, or both `None`.
    #[inline]
    fn ct_eq(&self, rhs: &CtOption<T>) -> Choice {
        let a = self.is_some();
        let b = rhs.is_some();

        (a & b & self.value.ct_eq(&rhs.value)) | (!a & !b)
    }
}

/// A type which can be compared in some manner and be determined to be greater
/// than another of the same type.
pub trait ConstantTimeGreater {
    /// Determine whether `self > other`.
    ///
    /// The bitwise-NOT of the return value of this function should be usable to
    /// determine if `self <= other`.
    ///
    /// This function should execute in constant time.
    ///
    /// # Returns
    ///
    /// A `Choice` with a set bit if `self > other`, and with no set bits
    /// otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use subtle::ConstantTimeGreater;
    ///
    /// let x: u8 = 13;
    /// let y: u8 = 42;
    ///
    /// let x_gt_y = x.ct_gt(&y);
    ///
    /// assert_eq!(x_gt_y.unwrap_u8(), 0);
    ///
    /// let y_gt_x = y.ct_gt(&x);
    ///
    /// assert_eq!(y_gt_x.unwrap_u8(), 1);
    ///
    /// let x_gt_x = x.ct_gt(&x);
    ///
    /// assert_eq!(x_gt_x.unwrap_u8(), 0);
    /// ```
    fn ct_gt(&self, other: &Self) -> Choice;
}

macro_rules! generate_unsigned_integer_greater {
    ($t_u: ty, $bit_width: expr) => {
        impl ConstantTimeGreater for $t_u {
            /// Returns Choice::from(1) iff x > y, and Choice::from(0) iff x <= y.
            ///
            /// # Note
            ///
            /// This algoritm would also work for signed integers if we first
            /// flip the top bit, e.g. `let x: u8 = x ^ 0x80`, etc.
            #[inline]
            fn ct_gt(&self, other: &$t_u) -> Choice {
                let gtb = self & !other; // All the bits in self that are greater than their corresponding bits in other.
                let mut ltb = !self & other; // All the bits in self that are less than their corresponding bits in other.
                let mut pow = 1;

                // Less-than operator is okay here because it's dependent on the bit-width.
                while pow < $bit_width {
                    ltb |= ltb >> pow; // Bit-smear the highest set bit to the right.
                    pow += pow;
                }
                let mut bit = gtb & !ltb; // Select the highest set bit.
                let mut pow = 1;

                while pow < $bit_width {
                    bit |= bit >> pow; // Shift it to the right until we end up with either 0 or 1.
                    pow += pow;
                }
                // XXX We should possibly do the above flattening to 0 or 1 in the
                //     Choice constructor rather than making it a debug error?
                Choice::from((bit & 1) as u8)
            }
        }
    };
}

generate_unsigned_integer_greater!(u8, 8);
generate_unsigned_integer_greater!(u16, 16);
generate_unsigned_integer_greater!(u32, 32);
generate_unsigned_integer_greater!(u64, 64);
#[cfg(feature = "i128")]
generate_unsigned_integer_greater!(u128, 128);

impl ConstantTimeGreater for cmp::Ordering {
    #[inline]
    fn ct_gt(&self, other: &Self) -> Choice {
        // No impl of `ConstantTimeGreater` for `i8`, so use `u8`
        let a = (*self as i8) + 1;
        let b = (*other as i8) + 1;
        (a as u8).ct_gt(&(b as u8))
    }
}

/// A type which can be compared in some manner and be determined to be less
/// than another of the same type.
pub trait ConstantTimeLess: ConstantTimeEq + ConstantTimeGreater {
    /// Determine whether `self < other`.
    ///
    /// The bitwise-NOT of the return value of this function should be usable to
    /// determine if `self >= other`.
    ///
    /// A default implementation is provided and implemented for the unsigned
    /// integer types.
    ///
    /// This function should execute in constant time.
    ///
    /// # Returns
    ///
    /// A `Choice` with a set bit if `self < other`, and with no set bits
    /// otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use subtle::ConstantTimeLess;
    ///
    /// let x: u8 = 13;
    /// let y: u8 = 42;
    ///
    /// let x_lt_y = x.ct_lt(&y);
    ///
    /// assert_eq!(x_lt_y.unwrap_u8(), 1);
    ///
    /// let y_lt_x = y.ct_lt(&x);
    ///
    /// assert_eq!(y_lt_x.unwrap_u8(), 0);
    ///
    /// let x_lt_x = x.ct_lt(&x);
    ///
    /// assert_eq!(x_lt_x.unwrap_u8(), 0);
    /// ```
    #[inline]
    fn ct_lt(&self, other: &Self) -> Choice {
        !self.ct_gt(other) & !self.ct_eq(other)
    }
}

impl ConstantTimeLess for u8 {}
impl ConstantTimeLess for u16 {}
impl ConstantTimeLess for u32 {}
impl ConstantTimeLess for u64 {}
#[cfg(feature = "i128")]
impl ConstantTimeLess for u128 {}

impl ConstantTimeLess for cmp::Ordering {
    #[inline]
    fn ct_lt(&self, other: &Self) -> Choice {
        // No impl of `ConstantTimeLess` for `i8`, so use `u8`
        let a = (*self as i8) + 1;
        let b = (*other as i8) + 1;
        (a as u8).ct_lt(&(b as u8))
    }
}

/// Wrapper type which implements an optimization barrier for all accesses.
#[derive(Clone, Copy, Debug)]
pub struct BlackBox<T: Copy>(T);

impl<T: Copy> BlackBox<T> {
    /// Constructs a new instance of `BlackBox` which will wrap the specified value.
    ///
    /// All access to the inner value will be mediated by a `black_box` optimization barrier.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Read the inner value, applying an optimization barrier on access.
    pub fn get(self) -> T {
        black_box(self.0)
    }
}
