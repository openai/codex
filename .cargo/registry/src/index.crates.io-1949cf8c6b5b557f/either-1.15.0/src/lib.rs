//! The enum [`Either`] with variants `Left` and `Right` is a general purpose
//! sum type with two cases.
//!
//! [`Either`]: enum.Either.html
//!
//! **Crate features:**
//!
//! * `"std"`
//!   Enabled by default. Disable to make the library `#![no_std]`.
//!
//! * `"serde"`
//!   Disabled by default. Enable to `#[derive(Serialize, Deserialize)]` for `Either`
//!

#![doc(html_root_url = "https://docs.rs/either/1/")]
#![no_std]

#[cfg(any(test, feature = "std"))]
extern crate std;

#[cfg(feature = "serde")]
pub mod serde_untagged;

#[cfg(feature = "serde")]
pub mod serde_untagged_optional;

use core::convert::{AsMut, AsRef};
use core::fmt;
use core::future::Future;
use core::ops::Deref;
use core::ops::DerefMut;
use core::pin::Pin;

#[cfg(any(test, feature = "std"))]
use std::error::Error;
#[cfg(any(test, feature = "std"))]
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};

pub use crate::Either::{Left, Right};

/// The enum `Either` with variants `Left` and `Right` is a general purpose
/// sum type with two cases.
///
/// The `Either` type is symmetric and treats its variants the same way, without
/// preference.
/// (For representing success or error, use the regular `Result` enum instead.)
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Either<L, R> {
    /// A value of type `L`.
    Left(L),
    /// A value of type `R`.
    Right(R),
}

/// Evaluate the provided expression for both [`Either::Left`] and [`Either::Right`].
///
/// This macro is useful in cases where both sides of [`Either`] can be interacted with
/// in the same way even though the don't share the same type.
///
/// Syntax: `either::for_both!(` *expression* `,` *pattern* `=>` *expression* `)`
///
/// # Example
///
/// ```
/// use either::Either;
///
/// fn length(owned_or_borrowed: Either<String, &'static str>) -> usize {
///     either::for_both!(owned_or_borrowed, s => s.len())
/// }
///
/// fn main() {
///     let borrowed = Either::Right("Hello world!");
///     let owned = Either::Left("Hello world!".to_owned());
///
///     assert_eq!(length(borrowed), 12);
///     assert_eq!(length(owned), 12);
/// }
/// ```
#[macro_export]
macro_rules! for_both {
    ($value:expr, $pattern:pat => $result:expr) => {
        match $value {
            $crate::Either::Left($pattern) => $result,
            $crate::Either::Right($pattern) => $result,
        }
    };
}

/// Macro for unwrapping the left side of an [`Either`], which fails early
/// with the opposite side. Can only be used in functions that return
/// `Either` because of the early return of `Right` that it provides.
///
/// See also [`try_right!`] for its dual, which applies the same just to the
/// right side.
///
/// # Example
///
/// ```
/// use either::{Either, Left, Right};
///
/// fn twice(wrapper: Either<u32, &str>) -> Either<u32, &str> {
///     let value = either::try_left!(wrapper);
///     Left(value * 2)
/// }
///
/// fn main() {
///     assert_eq!(twice(Left(2)), Left(4));
///     assert_eq!(twice(Right("ups")), Right("ups"));
/// }
/// ```
#[macro_export]
macro_rules! try_left {
    ($expr:expr) => {
        match $expr {
            $crate::Left(val) => val,
            $crate::Right(err) => return $crate::Right(::core::convert::From::from(err)),
        }
    };
}

/// Dual to [`try_left!`], see its documentation for more information.
#[macro_export]
macro_rules! try_right {
    ($expr:expr) => {
        match $expr {
            $crate::Left(err) => return $crate::Left(::core::convert::From::from(err)),
            $crate::Right(val) => val,
        }
    };
}

macro_rules! map_either {
    ($value:expr, $pattern:pat => $result:expr) => {
        match $value {
            Left($pattern) => Left($result),
            Right($pattern) => Right($result),
        }
    };
}

mod iterator;
pub use self::iterator::IterEither;

mod into_either;
pub use self::into_either::IntoEither;

impl<L: Clone, R: Clone> Clone for Either<L, R> {
    fn clone(&self) -> Self {
        match self {
            Left(inner) => Left(inner.clone()),
            Right(inner) => Right(inner.clone()),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        match (self, source) {
            (Left(dest), Left(source)) => dest.clone_from(source),
            (Right(dest), Right(source)) => dest.clone_from(source),
            (dest, source) => *dest = source.clone(),
        }
    }
}

impl<L, R> Either<L, R> {
    /// Return true if the value is the `Left` variant.
    ///
    /// ```
    /// use either::*;
    ///
    /// let values = [Left(1), Right("the right value")];
    /// assert_eq!(values[0].is_left(), true);
    /// assert_eq!(values[1].is_left(), false);
    /// ```
    pub fn is_left(&self) -> bool {
        match self {
            Left(_) => true,
            Right(_) => false,
        }
    }

    /// Return true if the value is the `Right` variant.
    ///
    /// ```
    /// use either::*;
    ///
    /// let values = [Left(1), Right("the right value")];
    /// assert_eq!(values[0].is_right(), false);
    /// assert_eq!(values[1].is_right(), true);
    /// ```
    pub fn is_right(&self) -> bool {
        !self.is_left()
    }

    /// Convert the left side of `Either<L, R>` to an `Option<L>`.
    ///
    /// ```
    /// use either::*;
    ///
    /// let left: Either<_, ()> = Left("some value");
    /// assert_eq!(left.left(),  Some("some value"));
    ///
    /// let right: Either<(), _> = Right(321);
    /// assert_eq!(right.left(), None);
    /// ```
    pub fn left(self) -> Option<L> {
        match self {
            Left(l) => Some(l),
            Right(_) => None,
        }
    }

    /// Convert the right side of `Either<L, R>` to an `Option<R>`.
    ///
    /// ```
    /// use either::*;
    ///
    /// let left: Either<_, ()> = Left("some value");
    /// assert_eq!(left.right(),  None);
    ///
    /// let right: Either<(), _> = Right(321);
    /// assert_eq!(right.right(), Some(321));
    /// ```
    pub fn right(self) -> Option<R> {
        match self {
            Left(_) => None,
            Right(r) => Some(r),
        }
    }

    /// Convert `&Either<L, R>` to `Either<&L, &R>`.
    ///
    /// ```
    /// use either::*;
    ///
    /// let left: Either<_, ()> = Left("some value");
    /// assert_eq!(left.as_ref(), Left(&"some value"));
    ///
    /// let right: Either<(), _> = Right("some value");
    /// assert_eq!(right.as_ref(), Right(&"some value"));
    /// ```
    pub fn as_ref(&self) -> Either<&L, &R> {
        map_either!(self, inner => inner)
    }

    /// Convert `&mut Either<L, R>` to `Either<&mut L, &mut R>`.
    ///
    /// ```
    /// use either::*;
    ///
    /// fn mutate_left(value: &mut Either<u32, u32>) {
    ///     if let Some(l) = value.as_mut().left() {
    ///         *l = 999;
    ///     }
    /// }
    ///
    /// let mut left = Left(123);
    /// let mut right = Right(123);
    /// mutate_left(&mut left);
    /// mutate_left(&mut right);
    /// assert_eq!(left, Left(999));
    /// assert_eq!(right, Right(123));
    /// ```
    pub fn as_mut(&mut self) -> Either<&mut L, &mut R> {
        map_either!(self, inner => inner)
    }

    /// Convert `Pin<&Either<L, R>>` to `Either<Pin<&L>, Pin<&R>>`,
    /// pinned projections of the inner variants.
    pub fn as_pin_ref(self: Pin<&Self>) -> Either<Pin<&L>, Pin<&R>> {
        // SAFETY: We can use `new_unchecked` because the `inner` parts are
        // guaranteed to be pinned, as they come from `self` which is pinned.
        unsafe { map_either!(Pin::get_ref(self), inner => Pin::new_unchecked(inner)) }
    }

    /// Convert `Pin<&mut Either<L, R>>` to `Either<Pin<&mut L>, Pin<&mut R>>`,
    /// pinned projections of the inner variants.
    pub fn as_pin_mut(self: Pin<&mut Self>) -> Either<Pin<&mut L>, Pin<&mut R>> {
        // SAFETY: `get_unchecked_mut` is fine because we don't move anything.
        // We can use `new_unchecked` because the `inner` parts are guaranteed
        // to be pinned, as they come from `self` which is pinned, and we never
        // offer an unpinned `&mut L` or `&mut R` through `Pin<&mut Self>`. We
        // also don't have an implementation of `Drop`, nor manual `Unpin`.
        unsafe { map_either!(Pin::get_unchecked_mut(self), inner => Pin::new_unchecked(inner)) }
    }

    /// Convert `Either<L, R>` to `Either<R, L>`.
    ///
    /// ```
    /// use either::*;
    ///
    /// let left: Either<_, ()> = Left(123);
    /// assert_eq!(left.flip(), Right(123));
    ///
    /// let right: Either<(), _> = Right("some value");
    /// assert_eq!(right.flip(), Left("some value"));
    /// ```
    pub fn flip(self) -> Either<R, L> {
        match self {
            Left(l) => Right(l),
            Right(r) => Left(r),
        }
    }

    /// Apply the function `f` on the value in the `Left` variant if it is present rewrapping the
    /// result in `Left`.
    ///
    /// ```
    /// use either::*;
    ///
    /// let left: Either<_, u32> = Left(123);
    /// assert_eq!(left.map_left(|x| x * 2), Left(246));
    ///
    /// let right: Either<u32, _> = Right(123);
    /// assert_eq!(right.map_left(|x| x * 2), Right(123));
    /// ```
    pub fn map_left<F, M>(self, f: F) -> Either<M, R>
    where
        F: FnOnce(L) -> M,
    {
        match self {
            Left(l) => Left(f(l)),
            Right(r) => Right(r),
        }
    }

    /// Apply the function `f` on the value in the `Right` variant if it is present rewrapping the
    /// result in `Right`.
    ///
    /// ```
    /// use either::*;
    ///
    /// let left: Either<_, u32> = Left(123);
    /// assert_eq!(left.map_right(|x| x * 2), Left(123));
    ///
    /// let right: Either<u32, _> = Right(123);
    /// assert_eq!(right.map_right(|x| x * 2), Right(246));
    /// ```
    pub fn map_right<F, S>(self, f: F) -> Either<L, S>
    where
        F: FnOnce(R) -> S,
    {
        match self {
            Left(l) => Left(l),
            Right(r) => Right(f(r)),
        }
    }

    /// Apply the functions `f` and `g` to the `Left` and `Right` variants
    /// respectively. This is equivalent to
    /// [bimap](https://hackage.haskell.org/package/bifunctors-5/docs/Data-Bifunctor.html)
    /// in functional programming.
    ///
    /// ```
    /// use either::*;
    ///
    /// let f = |s: String| s.len();
    /// let g = |u: u8| u.to_string();
    ///
    /// let left: Either<String, u8> = Left("loopy".into());
    /// assert_eq!(left.map_either(f, g), Left(5));
    ///
    /// let right: Either<String, u8> = Right(42);
    /// assert_eq!(right.map_either(f, g), Right("42".into()));
    /// ```
    pub fn map_either<F, G, M, S>(self, f: F, g: G) -> Either<M, S>
    where
        F: FnOnce(L) -> M,
        G: FnOnce(R) -> S,
    {
        match self {
            Left(l) => Left(f(l)),
            Right(r) => Right(g(r)),
        }
    }

    /// Similar to [`map_either`][Self::map_either], with an added context `ctx` accessible to
    /// both functions.
    ///
    /// ```
    /// use either::*;
    ///
    /// let mut sum = 0;
    ///
    /// // Both closures want to update the same value, so pass it as context.
    /// let mut f = |sum: &mut usize, s: String| { *sum += s.len(); s.to_uppercase() };
    /// let mut g = |sum: &mut usize, u: usize| { *sum += u; u.to_string() };
    ///
    /// let left: Either<String, usize> = Left("loopy".into());
    /// assert_eq!(left.map_either_with(&mut sum, &mut f, &mut g), Left("LOOPY".into()));
    ///
    /// let right: Either<String, usize> = Right(42);
    /// assert_eq!(right.map_either_with(&mut sum, &mut f, &mut g), Right("42".into()));
    ///
    /// assert_eq!(sum, 47);
    /// ```
    pub fn map_either_with<Ctx, F, G, M, S>(self, ctx: Ctx, f: F, g: G) -> Either<M, S>
    where
        F: FnOnce(Ctx, L) -> M,
        G: FnOnce(Ctx, R) -> S,
    {
        match self {
            Left(l) => Left(f(ctx, l)),
            Right(r) => Right(g(ctx, r)),
        }
    }

    /// Apply one of two functions depending on contents, unifying their result. If the value is
    /// `Left(L)` then the first function `f` is applied; if it is `Right(R)` then the second
    /// function `g` is applied.
    ///
    /// ```
    /// use either::*;
    ///
    /// fn square(n: u32) -> i32 { (n * n) as i32 }
    /// fn negate(n: i32) -> i32 { -n }
    ///
    /// let left: Either<u32, i32> = Left(4);
    /// assert_eq!(left.either(square, negate), 16);
    ///
    /// let right: Either<u32, i32> = Right(-4);
    /// assert_eq!(right.either(square, negate), 4);
    /// ```
    pub fn either<F, G, T>(self, f: F, g: G) -> T
    where
        F: FnOnce(L) -> T,
        G: FnOnce(R) -> T,
    {
        match self {
            Left(l) => f(l),
            Right(r) => g(r),
        }
    }

    /// Like [`either`][Self::either], but provide some context to whichever of the
    /// functions ends up being called.
    ///
    /// ```
    /// // In this example, the context is a mutable reference
    /// use either::*;
    ///
    /// let mut result = Vec::new();
    ///
    /// let values = vec![Left(2), Right(2.7)];
    ///
    /// for value in values {
    ///     value.either_with(&mut result,
    ///                       |ctx, integer| ctx.push(integer),
    ///                       |ctx, real| ctx.push(f64::round(real) as i32));
    /// }
    ///
    /// assert_eq!(result, vec![2, 3]);
    /// ```
    pub fn either_with<Ctx, F, G, T>(self, ctx: Ctx, f: F, g: G) -> T
    where
        F: FnOnce(Ctx, L) -> T,
        G: FnOnce(Ctx, R) -> T,
    {
        match self {
            Left(l) => f(ctx, l),
            Right(r) => g(ctx, r),
        }
    }

    /// Apply the function `f` on the value in the `Left` variant if it is present.
    ///
    /// ```
    /// use either::*;
    ///
    /// let left: Either<_, u32> = Left(123);
    /// assert_eq!(left.left_and_then::<_,()>(|x| Right(x * 2)), Right(246));
    ///
    /// let right: Either<u32, _> = Right(123);
    /// assert_eq!(right.left_and_then(|x| Right::<(), _>(x * 2)), Right(123));
    /// ```
    pub fn left_and_then<F, S>(self, f: F) -> Either<S, R>
    where
        F: FnOnce(L) -> Either<S, R>,
    {
        match self {
            Left(l) => f(l),
            Right(r) => Right(r),
        }
    }

    /// Apply the function `f` on the value in the `Right` variant if it is present.
    ///
    /// ```
    /// use either::*;
    ///
    /// let left: Either<_, u32> = Left(123);
    /// assert_eq!(left.right_and_then(|x| Right(x * 2)), Left(123));
    ///
    /// let right: Either<u32, _> = Right(123);
    /// assert_eq!(right.right_and_then(|x| Right(x * 2)), Right(246));
    /// ```
    pub fn right_and_then<F, S>(self, f: F) -> Either<L, S>
    where
        F: FnOnce(R) -> Either<L, S>,
    {
        match self {
            Left(l) => Left(l),
            Right(r) => f(r),
        }
    }

    /// Convert the inner value to an iterator.
    ///
    /// This requires the `Left` and `Right` iterators to have the same item type.
    /// See [`factor_into_iter`][Either::factor_into_iter] to iterate different types.
    ///
    /// ```
    /// use either::*;
    ///
    /// let left: Either<_, Vec<u32>> = Left(vec![1, 2, 3, 4, 5]);
    /// let mut right: Either<Vec<u32>, _> = Right(vec![]);
    /// right.extend(left.into_iter());
    /// assert_eq!(right, Right(vec![1, 2, 3, 4, 5]));
    /// ```
    #[allow(clippy::should_implement_trait)]
    pub fn into_iter(self) -> Either<L::IntoIter, R::IntoIter>
    where
        L: IntoIterator,
        R: IntoIterator<Item = L::Item>,
    {
        map_either!(self, inner => inner.into_iter())
    }

    /// Borrow the inner value as an iterator.
    ///
    /// This requires the `Left` and `Right` iterators to have the same item type.
    /// See [`factor_iter`][Either::factor_iter] to iterate different types.
    ///
    /// ```
    /// use either::*;
    ///
    /// let left: Either<_, &[u32]> = Left(vec![2, 3]);
    /// let mut right: Either<Vec<u32>, _> = Right(&[4, 5][..]);
    /// let mut all = vec![1];
    /// all.extend(left.iter());
    /// all.extend(right.iter());
    /// assert_eq!(all, vec![1, 2, 3, 4, 5]);
    /// ```
    pub fn iter(&self) -> Either<<&L as IntoIterator>::IntoIter, <&R as IntoIterator>::IntoIter>
    where
        for<'a> &'a L: IntoIterator,
        for<'a> &'a R: IntoIterator<Item = <&'a L as IntoIterator>::Item>,
    {
        map_either!(self, inner => inner.into_iter())
    }

    /// Mutably borrow the inner value as an iterator.
    ///
    /// This requires the `Left` and `Right` iterators to have the same item type.
    /// See [`factor_iter_mut`][Either::factor_iter_mut] to iterate different types.
    ///
    /// ```
    /// use either::*;
    ///
    /// let mut left: Either<_, &mut [u32]> = Left(vec![2, 3]);
    /// for l in left.iter_mut() {
    ///     *l *= *l
    /// }
    /// assert_eq!(left, Left(vec![4, 9]));
    ///
    /// let mut inner = [4, 5];
    /// let mut right: Either<Vec<u32>, _> = Right(&mut inner[..]);
    /// for r in right.iter_mut() {
    ///     *r *= *r
    /// }
    /// assert_eq!(inner, [16, 25]);
    /// ```
    pub fn iter_mut(
        &mut self,
    ) -> Either<<&mut L as IntoIterator>::IntoIter, <&mut R as IntoIterator>::IntoIter>
    where
        for<'a> &'a mut L: IntoIterator,
        for<'a> &'a mut R: IntoIterator<Item = <&'a mut L as IntoIterator>::Item>,
    {
        map_either!(self, inner => inner.into_iter())
    }

    /// Converts an `Either` of `Iterator`s to be an `Iterator` of `Either`s
    ///
    /// Unlike [`into_iter`][Either::into_iter], this does not require the
    /// `Left` and `Right` iterators to have the same item type.
    ///
    /// ```
    /// use either::*;
    /// let left: Either<_, Vec<u8>> = Left(&["hello"]);
    /// assert_eq!(left.factor_into_iter().next(), Some(Left(&"hello")));
    ///
    /// let right: Either<&[&str], _> = Right(vec![0, 1]);
    /// assert_eq!(right.factor_into_iter().collect::<Vec<_>>(), vec![Right(0), Right(1)]);
    ///
    /// ```
    // TODO(MSRV): doc(alias) was stabilized in Rust 1.48
    // #[doc(alias = "transpose")]
    pub fn factor_into_iter(self) -> IterEither<L::IntoIter, R::IntoIter>
    where
        L: IntoIterator,
        R: IntoIterator,
    {
        IterEither::new(map_either!(self, inner => inner.into_iter()))
    }

    /// Borrows an `Either` of `Iterator`s to be an `Iterator` of `Either`s
    ///
    /// Unlike [`iter`][Either::iter], this does not require the
    /// `Left` and `Right` iterators to have the same item type.
    ///
    /// ```
    /// use either::*;
    /// let left: Either<_, Vec<u8>> = Left(["hello"]);
    /// assert_eq!(left.factor_iter().next(), Some(Left(&"hello")));
    ///
    /// let right: Either<[&str; 2], _> = Right(vec![0, 1]);
    /// assert_eq!(right.factor_iter().collect::<Vec<_>>(), vec![Right(&0), Right(&1)]);
    ///
    /// ```
    pub fn factor_iter(
        &self,
    ) -> IterEither<<&L as IntoIterator>::IntoIter, <&R as IntoIterator>::IntoIter>
    where
        for<'a> &'a L: IntoIterator,
        for<'a> &'a R: IntoIterator,
    {
        IterEither::new(map_either!(self, inner => inner.into_iter()))
    }

    /// Mutably borrows an `Either` of `Iterator`s to be an `Iterator` of `Either`s
    ///
    /// Unlike [`iter_mut`][Either::iter_mut], this does not require the
    /// `Left` and `Right` iterators to have the same item type.
    ///
    /// ```
    /// use either::*;
    /// let mut left: Either<_, Vec<u8>> = Left(["hello"]);
    /// left.factor_iter_mut().for_each(|x| *x.unwrap_left() = "goodbye");
    /// assert_eq!(left, Left(["goodbye"]));
    ///
    /// let mut right: Either<[&str; 2], _> = Right(vec![0, 1, 2]);
    /// right.factor_iter_mut().for_each(|x| if let Right(r) = x { *r = -*r; });
    /// assert_eq!(right, Right(vec![0, -1, -2]));
    ///
    /// ```
    pub fn factor_iter_mut(
        &mut self,
    ) -> IterEither<<&mut L as IntoIterator>::IntoIter, <&mut R as IntoIterator>::IntoIter>
    where
        for<'a> &'a mut L: IntoIterator,
        for<'a> &'a mut R: IntoIterator,
    {
        IterEither::new(map_either!(self, inner => inner.into_iter()))
    }

    /// Return left value or given value
    ///
    /// Arguments passed to `left_or` are eagerly evaluated; if you are passing
    /// the result of a function call, it is recommended to use
    /// [`left_or_else`][Self::left_or_else], which is lazily evaluated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use either::*;
    /// let left: Either<&str, &str> = Left("left");
    /// assert_eq!(left.left_or("foo"), "left");
    ///
    /// let right: Either<&str, &str> = Right("right");
    /// assert_eq!(right.left_or("left"), "left");
    /// ```
    pub fn left_or(self, other: L) -> L {
        match self {
            Either::Left(l) => l,
            Either::Right(_) => other,
        }
    }

    /// Return left or a default
    ///
    /// # Examples
    ///
    /// ```
    /// # use either::*;
    /// let left: Either<String, u32> = Left("left".to_string());
    /// assert_eq!(left.left_or_default(), "left");
    ///
    /// let right: Either<String, u32> = Right(42);
    /// assert_eq!(right.left_or_default(), String::default());
    /// ```
    pub fn left_or_default(self) -> L
    where
        L: Default,
    {
        match self {
            Either::Left(l) => l,
            Either::Right(_) => L::default(),
        }
    }

    /// Returns left value or computes it from a closure
    ///
    /// # Examples
    ///
    /// ```
    /// # use either::*;
    /// let left: Either<String, u32> = Left("3".to_string());
    /// assert_eq!(left.left_or_else(|_| unreachable!()), "3");
    ///
    /// let right: Either<String, u32> = Right(3);
    /// assert_eq!(right.left_or_else(|x| x.to_string()), "3");
    /// ```
    pub fn left_or_else<F>(self, f: F) -> L
    where
        F: FnOnce(R) -> L,
    {
        match self {
            Either::Left(l) => l,
            Either::Right(r) => f(r),
        }
    }

    /// Return right value or given value
    ///
    /// Arguments passed to `right_or` are eagerly evaluated; if you are passing
    /// the result of a function call, it is recommended to use
    /// [`right_or_else`][Self::right_or_else], which is lazily evaluated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use either::*;
    /// let right: Either<&str, &str> = Right("right");
    /// assert_eq!(right.right_or("foo"), "right");
    ///
    /// let left: Either<&str, &str> = Left("left");
    /// assert_eq!(left.right_or("right"), "right");
    /// ```
    pub fn right_or(self, other: R) -> R {
        match self {
            Either::Left(_) => other,
            Either::Right(r) => r,
        }
    }

    /// Return right or a default
    ///
    /// # Examples
    ///
    /// ```
    /// # use either::*;
    /// let left: Either<String, u32> = Left("left".to_string());
    /// assert_eq!(left.right_or_default(), u32::default());
    ///
    /// let right: Either<String, u32> = Right(42);
    /// assert_eq!(right.right_or_default(), 42);
    /// ```
    pub fn right_or_default(self) -> R
    where
        R: Default,
    {
        match self {
            Either::Left(_) => R::default(),
            Either::Right(r) => r,
        }
    }

    /// Returns right value or computes it from a closure
    ///
    /// # Examples
    ///
    /// ```
    /// # use either::*;
    /// let left: Either<String, u32> = Left("3".to_string());
    /// assert_eq!(left.right_or_else(|x| x.parse().unwrap()), 3);
    ///
    /// let right: Either<String, u32> = Right(3);
    /// assert_eq!(right.right_or_else(|_| unreachable!()), 3);
    /// ```
    pub fn right_or_else<F>(self, f: F) -> R
    where
        F: FnOnce(L) -> R,
    {
        match self {
            Either::Left(l) => f(l),
            Either::Right(r) => r,
        }
    }

    /// Returns the left value
    ///
    /// # Examples
    ///
    /// ```
    /// # use either::*;
    /// let left: Either<_, ()> = Left(3);
    /// assert_eq!(left.unwrap_left(), 3);
    /// ```
    ///
    /// # Panics
    ///
    /// When `Either` is a `Right` value
    ///
    /// ```should_panic
    /// # use either::*;
    /// let right: Either<(), _> = Right(3);
    /// right.unwrap_left();
    /// ```
    pub fn unwrap_left(self) -> L
    where
        R: core::fmt::Debug,
    {
        match self {
            Either::Left(l) => l,
            Either::Right(r) => {
                panic!("called `Either::unwrap_left()` on a `Right` value: {:?}", r)
            }
        }
    }

    /// Returns the right value
    ///
    /// # Examples
    ///
    /// ```
    /// # use either::*;
    /// let right: Either<(), _> = Right(3);
    /// assert_eq!(right.unwrap_right(), 3);
    /// ```
    ///
    /// # Panics
    ///
    /// When `Either` is a `Left` value
    ///
    /// ```should_panic
    /// # use either::*;
    /// let left: Either<_, ()> = Left(3);
    /// left.unwrap_right();
    /// ```
    pub fn unwrap_right(self) -> R
    where
        L: core::fmt::Debug,
    {
        match self {
            Either::Right(r) => r,
            Either::Left(l) => panic!("called `Either::unwrap_right()` on a `Left` value: {:?}", l),
        }
    }

    /// Returns the left value
    ///
    /// # Examples
    ///
    /// ```
    /// # use either::*;
    /// let left: Either<_, ()> = Left(3);
    /// assert_eq!(left.expect_left("value was Right"), 3);
    /// ```
    ///
    /// # Panics
    ///
    /// When `Either` is a `Right` value
    ///
    /// ```should_panic
    /// # use either::*;
    /// let right: Either<(), _> = Right(3);
    /// right.expect_left("value was Right");
    /// ```
    pub fn expect_left(self, msg: &str) -> L
    where
        R: core::fmt::Debug,
    {
        match self {
            Either::Left(l) => l,
            Either::Right(r) => panic!("{}: {:?}", msg, r),
        }
    }

    /// Returns the right value
    ///
    /// # Examples
    ///
    /// ```
    /// # use either::*;
    /// let right: Either<(), _> = Right(3);
    /// assert_eq!(right.expect_right("value was Left"), 3);
    /// ```
    ///
    /// # Panics
    ///
    /// When `Either` is a `Left` value
    ///
    /// ```should_panic
    /// # use either::*;
    /// let left: Either<_, ()> = Left(3);
    /// left.expect_right("value was Right");
    /// ```
    pub fn expect_right(self, msg: &str) -> R
    where
        L: core::fmt::Debug,
    {
        match self {
            Either::Right(r) => r,
            Either::Left(l) => panic!("{}: {:?}", msg, l),
        }
    }

    /// Convert the contained value into `T`
    ///
    /// # Examples
    ///
    /// ```
    /// # use either::*;
    /// // Both u16 and u32 can be converted to u64.
    /// let left: Either<u16, u32> = Left(3u16);
    /// assert_eq!(left.either_into::<u64>(), 3u64);
    /// let right: Either<u16, u32> = Right(7u32);
    /// assert_eq!(right.either_into::<u64>(), 7u64);
    /// ```
    pub fn either_into<T>(self) -> T
    where
        L: Into<T>,
        R: Into<T>,
    {
        for_both!(self, inner => inner.into())
    }
}

impl<L, R> Either<Option<L>, Option<R>> {
    /// Factors out `None` from an `Either` of [`Option`].
    ///
    /// ```
    /// use either::*;
    /// let left: Either<_, Option<String>> = Left(Some(vec![0]));
    /// assert_eq!(left.factor_none(), Some(Left(vec![0])));
    ///
    /// let right: Either<Option<Vec<u8>>, _> = Right(Some(String::new()));
    /// assert_eq!(right.factor_none(), Some(Right(String::new())));
    /// ```
    // TODO(MSRV): doc(alias) was stabilized in Rust 1.48
    // #[doc(alias = "transpose")]
    pub fn factor_none(self) -> Option<Either<L, R>> {
        match self {
            Left(l) => l.map(Either::Left),
            Right(r) => r.map(Either::Right),
        }
    }
}

impl<L, R, E> Either<Result<L, E>, Result<R, E>> {
    /// Factors out a homogenous type from an `Either` of [`Result`].
    ///
    /// Here, the homogeneous type is the `Err` type of the [`Result`].
    ///
    /// ```
    /// use either::*;
    /// let left: Either<_, Result<String, u32>> = Left(Ok(vec![0]));
    /// assert_eq!(left.factor_err(), Ok(Left(vec![0])));
    ///
    /// let right: Either<Result<Vec<u8>, u32>, _> = Right(Ok(String::new()));
    /// assert_eq!(right.factor_err(), Ok(Right(String::new())));
    /// ```
    // TODO(MSRV): doc(alias) was stabilized in Rust 1.48
    // #[doc(alias = "transpose")]
    pub fn factor_err(self) -> Result<Either<L, R>, E> {
        match self {
            Left(l) => l.map(Either::Left),
            Right(r) => r.map(Either::Right),
        }
    }
}

impl<T, L, R> Either<Result<T, L>, Result<T, R>> {
    /// Factors out a homogenous type from an `Either` of [`Result`].
    ///
    /// Here, the homogeneous type is the `Ok` type of the [`Result`].
    ///
    /// ```
    /// use either::*;
    /// let left: Either<_, Result<u32, String>> = Left(Err(vec![0]));
    /// assert_eq!(left.factor_ok(), Err(Left(vec![0])));
    ///
    /// let right: Either<Result<u32, Vec<u8>>, _> = Right(Err(String::new()));
    /// assert_eq!(right.factor_ok(), Err(Right(String::new())));
    /// ```
    // TODO(MSRV): doc(alias) was stabilized in Rust 1.48
    // #[doc(alias = "transpose")]
    pub fn factor_ok(self) -> Result<T, Either<L, R>> {
        match self {
            Left(l) => l.map_err(Either::Left),
            Right(r) => r.map_err(Either::Right),
        }
    }
}

impl<T, L, R> Either<(T, L), (T, R)> {
    /// Factor out a homogeneous type from an either of pairs.
    ///
    /// Here, the homogeneous type is the first element of the pairs.
    ///
    /// ```
    /// use either::*;
    /// let left: Either<_, (u32, String)> = Left((123, vec![0]));
    /// assert_eq!(left.factor_first().0, 123);
    ///
    /// let right: Either<(u32, Vec<u8>), _> = Right((123, String::new()));
    /// assert_eq!(right.factor_first().0, 123);
    /// ```
    pub fn factor_first(self) -> (T, Either<L, R>) {
        match self {
            Left((t, l)) => (t, Left(l)),
            Right((t, r)) => (t, Right(r)),
        }
    }
}

impl<T, L, R> Either<(L, T), (R, T)> {
    /// Factor out a homogeneous type from an either of pairs.
    ///
    /// Here, the homogeneous type is the second element of the pairs.
    ///
    /// ```
    /// use either::*;
    /// let left: Either<_, (String, u32)> = Left((vec![0], 123));
    /// assert_eq!(left.factor_second().1, 123);
    ///
    /// let right: Either<(Vec<u8>, u32), _> = Right((String::new(), 123));
    /// assert_eq!(right.factor_second().1, 123);
    /// ```
    pub fn factor_second(self) -> (Either<L, R>, T) {
        match self {
            Left((l, t)) => (Left(l), t),
            Right((r, t)) => (Right(r), t),
        }
    }
}

impl<T> Either<T, T> {
    /// Extract the value of an either over two equivalent types.
    ///
    /// ```
    /// use either::*;
    ///
    /// let left: Either<_, u32> = Left(123);
    /// assert_eq!(left.into_inner(), 123);
    ///
    /// let right: Either<u32, _> = Right(123);
    /// assert_eq!(right.into_inner(), 123);
    /// ```
    pub fn into_inner(self) -> T {
        for_both!(self, inner => inner)
    }

    /// Map `f` over the contained value and return the result in the
    /// corresponding variant.
    ///
    /// ```
    /// use either::*;
    ///
    /// let value: Either<_, i32> = Right(42);
    ///
    /// let other = value.map(|x| x * 2);
    /// assert_eq!(other, Right(84));
    /// ```
    pub fn map<F, M>(self, f: F) -> Either<M, M>
    where
        F: FnOnce(T) -> M,
    {
        match self {
            Left(l) => Left(f(l)),
            Right(r) => Right(f(r)),
        }
    }
}

impl<L, R> Either<&L, &R> {
    /// Maps an `Either<&L, &R>` to an `Either<L, R>` by cloning the contents of
    /// either branch.
    pub fn cloned(self) -> Either<L, R>
    where
        L: Clone,
        R: Clone,
    {
        map_either!(self, inner => inner.clone())
    }

    /// Maps an `Either<&L, &R>` to an `Either<L, R>` by copying the contents of
    /// either branch.
    pub fn copied(self) -> Either<L, R>
    where
        L: Copy,
        R: Copy,
    {
        map_either!(self, inner => *inner)
    }
}

impl<L, R> Either<&mut L, &mut R> {
    /// Maps an `Either<&mut L, &mut R>` to an `Either<L, R>` by cloning the contents of
    /// either branch.
    pub fn cloned(self) -> Either<L, R>
    where
        L: Clone,
        R: Clone,
    {
        map_either!(self, inner => inner.clone())
    }

    /// Maps an `Either<&mut L, &mut R>` to an `Either<L, R>` by copying the contents of
    /// either branch.
    pub fn copied(self) -> Either<L, R>
    where
        L: Copy,
        R: Copy,
    {
        map_either!(self, inner => *inner)
    }
}

/// Convert from `Result` to `Either` with `Ok => Right` and `Err => Left`.
impl<L, R> From<Result<R, L>> for Either<L, R> {
    fn from(r: Result<R, L>) -> Self {
        match r {
            Err(e) => Left(e),
            Ok(o) => Right(o),
        }
    }
}

/// Convert from `Either` to `Result` with `Right => Ok` and `Left => Err`.
impl<L, R> From<Either<L, R>> for Result<R, L> {
    fn from(val: Either<L, R>) -> Self {
        match val {
            Left(l) => Err(l),
            Right(r) => Ok(r),
        }
    }
}

/// `Either<L, R>` is a future if both `L` and `R` are futures.
impl<L, R> Future for Either<L, R>
where
    L: Future,
    R: Future<Output = L::Output>,
{
    type Output = L::Output;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        for_both!(self.as_pin_mut(), inner => inner.poll(cx))
    }
}

#[cfg(any(test, feature = "std"))]
/// `Either<L, R>` implements `Read` if both `L` and `R` do.
///
/// Requires crate feature `"std"`
impl<L, R> Read for Either<L, R>
where
    L: Read,
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        for_both!(self, inner => inner.read(buf))
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        for_both!(self, inner => inner.read_exact(buf))
    }

    fn read_to_end(&mut self, buf: &mut std::vec::Vec<u8>) -> io::Result<usize> {
        for_both!(self, inner => inner.read_to_end(buf))
    }

    fn read_to_string(&mut self, buf: &mut std::string::String) -> io::Result<usize> {
        for_both!(self, inner => inner.read_to_string(buf))
    }
}

#[cfg(any(test, feature = "std"))]
/// `Either<L, R>` implements `Seek` if both `L` and `R` do.
///
/// Requires crate feature `"std"`
impl<L, R> Seek for Either<L, R>
where
    L: Seek,
    R: Seek,
{
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        for_both!(self, inner => inner.seek(pos))
    }
}

#[cfg(any(test, feature = "std"))]
/// Requires crate feature `"std"`
impl<L, R> BufRead for Either<L, R>
where
    L: BufRead,
    R: BufRead,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        for_both!(self, inner => inner.fill_buf())
    }

    fn consume(&mut self, amt: usize) {
        for_both!(self, inner => inner.consume(amt))
    }

    fn read_until(&mut self, byte: u8, buf: &mut std::vec::Vec<u8>) -> io::Result<usize> {
        for_both!(self, inner => inner.read_until(byte, buf))
    }

    fn read_line(&mut self, buf: &mut std::string::String) -> io::Result<usize> {
        for_both!(self, inner => inner.read_line(buf))
    }
}

#[cfg(any(test, feature = "std"))]
/// `Either<L, R>` implements `Write` if both `L` and `R` do.
///
/// Requires crate feature `"std"`
impl<L, R> Write for Either<L, R>
where
    L: Write,
    R: Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for_both!(self, inner => inner.write(buf))
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        for_both!(self, inner => inner.write_all(buf))
    }

    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> io::Result<()> {
        for_both!(self, inner => inner.write_fmt(fmt))
    }

    fn flush(&mut self) -> io::Result<()> {
        for_both!(self, inner => inner.flush())
    }
}

impl<L, R, Target> AsRef<Target> for Either<L, R>
where
    L: AsRef<Target>,
    R: AsRef<Target>,
{
    fn as_ref(&self) -> &Target {
        for_both!(self, inner => inner.as_ref())
    }
}

macro_rules! impl_specific_ref_and_mut {
    ($t:ty, $($attr:meta),* ) => {
        $(#[$attr])*
        impl<L, R> AsRef<$t> for Either<L, R>
            where L: AsRef<$t>, R: AsRef<$t>
        {
            fn as_ref(&self) -> &$t {
                for_both!(self, inner => inner.as_ref())
            }
        }

        $(#[$attr])*
        impl<L, R> AsMut<$t> for Either<L, R>
            where L: AsMut<$t>, R: AsMut<$t>
        {
            fn as_mut(&mut self) -> &mut $t {
                for_both!(self, inner => inner.as_mut())
            }
        }
    };
}

impl_specific_ref_and_mut!(str,);
impl_specific_ref_and_mut!(
    ::std::path::Path,
    cfg(feature = "std"),
    doc = "Requires crate feature `std`."
);
impl_specific_ref_and_mut!(
    ::std::ffi::OsStr,
    cfg(feature = "std"),
    doc = "Requires crate feature `std`."
);
impl_specific_ref_and_mut!(
    ::std::ffi::CStr,
    cfg(feature = "std"),
    doc = "Requires crate feature `std`."
);

impl<L, R, Target> AsRef<[Target]> for Either<L, R>
where
    L: AsRef<[Target]>,
    R: AsRef<[Target]>,
{
    fn as_ref(&self) -> &[Target] {
        for_both!(self, inner => inner.as_ref())
    }
}

impl<L, R, Target> AsMut<Target> for Either<L, R>
where
    L: AsMut<Target>,
    R: AsMut<Target>,
{
    fn as_mut(&mut self) -> &mut Target {
        for_both!(self, inner => inner.as_mut())
    }
}

impl<L, R, Target> AsMut<[Target]> for Either<L, R>
where
    L: AsMut<[Target]>,
    R: AsMut<[Target]>,
{
    fn as_mut(&mut self) -> &mut [Target] {
        for_both!(self, inner => inner.as_mut())
    }
}

impl<L, R> Deref for Either<L, R>
where
    L: Deref,
    R: Deref<Target = L::Target>,
{
    type Target = L::Target;

    fn deref(&self) -> &Self::Target {
        for_both!(self, inner => &**inner)
    }
}

impl<L, R> DerefMut for Either<L, R>
where
    L: DerefMut,
    R: DerefMut<Target = L::Target>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        for_both!(self, inner => &mut *inner)
    }
}

#[cfg(any(test, feature = "std"))]
/// `Either` implements `Error` if *both* `L` and `R` implement it.
///
/// Requires crate feature `"std"`
impl<L, R> Error for Either<L, R>
where
    L: Error,
    R: Error,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        for_both!(self, inner => inner.source())
    }

    #[allow(deprecated)]
    fn description(&self) -> &str {
        for_both!(self, inner => inner.description())
    }

    #[allow(deprecated)]
    fn cause(&self) -> Option<&dyn Error> {
        for_both!(self, inner => inner.cause())
    }
}

impl<L, R> fmt::Display for Either<L, R>
where
    L: fmt::Display,
    R: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for_both!(self, inner => inner.fmt(f))
    }
}

impl<L, R> fmt::Write for Either<L, R>
where
    L: fmt::Write,
    R: fmt::Write,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for_both!(self, inner => inner.write_str(s))
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        for_both!(self, inner => inner.write_char(c))
    }

    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        for_both!(self, inner => inner.write_fmt(args))
    }
}

#[test]
fn basic() {
    let mut e = Left(2);
    let r = Right(2);
    assert_eq!(e, Left(2));
    e = r;
    assert_eq!(e, Right(2));
    assert_eq!(e.left(), None);
    assert_eq!(e.right(), Some(2));
    assert_eq!(e.as_ref().right(), Some(&2));
    assert_eq!(e.as_mut().right(), Some(&mut 2));
}

#[test]
fn macros() {
    use std::string::String;

    fn a() -> Either<u32, u32> {
        let x: u32 = try_left!(Right(1337u32));
        Left(x * 2)
    }
    assert_eq!(a(), Right(1337));

    fn b() -> Either<String, &'static str> {
        Right(try_right!(Left("foo bar")))
    }
    assert_eq!(b(), Left(String::from("foo bar")));
}

#[test]
fn deref() {
    use std::string::String;

    fn is_str(_: &str) {}
    let value: Either<String, &str> = Left(String::from("test"));
    is_str(&value);
}

#[test]
fn iter() {
    let x = 3;
    let mut iter = match x {
        3 => Left(0..10),
        _ => Right(17..),
    };

    assert_eq!(iter.next(), Some(0));
    assert_eq!(iter.count(), 9);
}

#[test]
fn seek() {
    use std::io;

    let use_empty = false;
    let mut mockdata = [0x00; 256];
    for (i, data) in mockdata.iter_mut().enumerate() {
        *data = i as u8;
    }

    let mut reader = if use_empty {
        // Empty didn't impl Seek until Rust 1.51
        Left(io::Cursor::new([]))
    } else {
        Right(io::Cursor::new(&mockdata[..]))
    };

    let mut buf = [0u8; 16];
    assert_eq!(reader.read(&mut buf).unwrap(), buf.len());
    assert_eq!(buf, mockdata[..buf.len()]);

    // the first read should advance the cursor and return the next 16 bytes thus the `ne`
    assert_eq!(reader.read(&mut buf).unwrap(), buf.len());
    assert_ne!(buf, mockdata[..buf.len()]);

    // if the seek operation fails it should read 16..31 instead of 0..15
    reader.seek(io::SeekFrom::Start(0)).unwrap();
    assert_eq!(reader.read(&mut buf).unwrap(), buf.len());
    assert_eq!(buf, mockdata[..buf.len()]);
}

#[test]
fn read_write() {
    use std::io;

    let use_stdio = false;
    let mockdata = [0xff; 256];

    let mut reader = if use_stdio {
        Left(io::stdin())
    } else {
        Right(&mockdata[..])
    };

    let mut buf = [0u8; 16];
    assert_eq!(reader.read(&mut buf).unwrap(), buf.len());
    assert_eq!(&buf, &mockdata[..buf.len()]);

    let mut mockbuf = [0u8; 256];
    let mut writer = if use_stdio {
        Left(io::stdout())
    } else {
        Right(&mut mockbuf[..])
    };

    let buf = [1u8; 16];
    assert_eq!(writer.write(&buf).unwrap(), buf.len());
}

#[test]
fn error() {
    let invalid_utf8 = b"\xff";
    #[allow(invalid_from_utf8)]
    let res = if let Err(error) = ::std::str::from_utf8(invalid_utf8) {
        Err(Left(error))
    } else if let Err(error) = "x".parse::<i32>() {
        Err(Right(error))
    } else {
        Ok(())
    };
    assert!(res.is_err());
    #[allow(deprecated)]
    res.unwrap_err().description(); // make sure this can be called
}

/// A helper macro to check if AsRef and AsMut are implemented for a given type.
macro_rules! check_t {
    ($t:ty) => {{
        fn check_ref<T: AsRef<$t>>() {}
        fn propagate_ref<T1: AsRef<$t>, T2: AsRef<$t>>() {
            check_ref::<Either<T1, T2>>()
        }
        fn check_mut<T: AsMut<$t>>() {}
        fn propagate_mut<T1: AsMut<$t>, T2: AsMut<$t>>() {
            check_mut::<Either<T1, T2>>()
        }
    }};
}

// This "unused" method is here to ensure that compilation doesn't fail on given types.
fn _unsized_ref_propagation() {
    check_t!(str);

    fn check_array_ref<T: AsRef<[Item]>, Item>() {}
    fn check_array_mut<T: AsMut<[Item]>, Item>() {}

    fn propagate_array_ref<T1: AsRef<[Item]>, T2: AsRef<[Item]>, Item>() {
        check_array_ref::<Either<T1, T2>, _>()
    }

    fn propagate_array_mut<T1: AsMut<[Item]>, T2: AsMut<[Item]>, Item>() {
        check_array_mut::<Either<T1, T2>, _>()
    }
}

// This "unused" method is here to ensure that compilation doesn't fail on given types.
#[cfg(feature = "std")]
fn _unsized_std_propagation() {
    check_t!(::std::path::Path);
    check_t!(::std::ffi::OsStr);
    check_t!(::std::ffi::CStr);
}
