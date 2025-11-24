use core::ops::{Deref, DerefMut};

use crate::EitherOrBoth::*;

use either::Either;

/// Value that either holds a single A or B, or both.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum EitherOrBoth<A, B = A> {
    /// Both values are present.
    Both(A, B),
    /// Only the left value of type `A` is present.
    Left(A),
    /// Only the right value of type `B` is present.
    Right(B),
}

impl<A, B> EitherOrBoth<A, B> {
    /// If `Left`, or `Both`, return true. Otherwise, return false.
    pub fn has_left(&self) -> bool {
        self.as_ref().left().is_some()
    }

    /// If `Right`, or `Both`, return true, otherwise, return false.
    pub fn has_right(&self) -> bool {
        self.as_ref().right().is_some()
    }

    /// If `Left`, return true. Otherwise, return false.
    /// Exclusive version of [`has_left`](EitherOrBoth::has_left).
    pub fn is_left(&self) -> bool {
        matches!(self, Left(_))
    }

    /// If `Right`, return true. Otherwise, return false.
    /// Exclusive version of [`has_right`](EitherOrBoth::has_right).
    pub fn is_right(&self) -> bool {
        matches!(self, Right(_))
    }

    /// If `Both`, return true. Otherwise, return false.
    pub fn is_both(&self) -> bool {
        self.as_ref().both().is_some()
    }

    /// If `Left`, or `Both`, return `Some` with the left value. Otherwise, return `None`.
    pub fn left(self) -> Option<A> {
        match self {
            Left(left) | Both(left, _) => Some(left),
            _ => None,
        }
    }

    /// If `Right`, or `Both`, return `Some` with the right value. Otherwise, return `None`.
    pub fn right(self) -> Option<B> {
        match self {
            Right(right) | Both(_, right) => Some(right),
            _ => None,
        }
    }

    /// Return tuple of options corresponding to the left and right value respectively
    ///
    /// If `Left` return `(Some(..), None)`, if `Right` return `(None,Some(..))`, else return
    /// `(Some(..),Some(..))`
    pub fn left_and_right(self) -> (Option<A>, Option<B>) {
        self.map_any(Some, Some).or_default()
    }

    /// If `Left`, return `Some` with the left value. If `Right` or `Both`, return `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// // On the `Left` variant.
    /// # use itertools::{EitherOrBoth, EitherOrBoth::{Left, Right, Both}};
    /// let x: EitherOrBoth<_, ()> = Left("bonjour");
    /// assert_eq!(x.just_left(), Some("bonjour"));
    ///
    /// // On the `Right` variant.
    /// let x: EitherOrBoth<(), _> = Right("hola");
    /// assert_eq!(x.just_left(), None);
    ///
    /// // On the `Both` variant.
    /// let x = Both("bonjour", "hola");
    /// assert_eq!(x.just_left(), None);
    /// ```
    pub fn just_left(self) -> Option<A> {
        match self {
            Left(left) => Some(left),
            _ => None,
        }
    }

    /// If `Right`, return `Some` with the right value. If `Left` or `Both`, return `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// // On the `Left` variant.
    /// # use itertools::{EitherOrBoth::{Left, Right, Both}, EitherOrBoth};
    /// let x: EitherOrBoth<_, ()> = Left("auf wiedersehen");
    /// assert_eq!(x.just_left(), Some("auf wiedersehen"));
    ///
    /// // On the `Right` variant.
    /// let x: EitherOrBoth<(), _> = Right("adios");
    /// assert_eq!(x.just_left(), None);
    ///
    /// // On the `Both` variant.
    /// let x = Both("auf wiedersehen", "adios");
    /// assert_eq!(x.just_left(), None);
    /// ```
    pub fn just_right(self) -> Option<B> {
        match self {
            Right(right) => Some(right),
            _ => None,
        }
    }

    /// If `Both`, return `Some` containing the left and right values. Otherwise, return `None`.
    pub fn both(self) -> Option<(A, B)> {
        match self {
            Both(a, b) => Some((a, b)),
            _ => None,
        }
    }

    /// If `Left` or `Both`, return the left value. Otherwise, convert the right value and return it.
    pub fn into_left(self) -> A
    where
        B: Into<A>,
    {
        match self {
            Left(a) | Both(a, _) => a,
            Right(b) => b.into(),
        }
    }

    /// If `Right` or `Both`, return the right value. Otherwise, convert the left value and return it.
    pub fn into_right(self) -> B
    where
        A: Into<B>,
    {
        match self {
            Right(b) | Both(_, b) => b,
            Left(a) => a.into(),
        }
    }

    /// Converts from `&EitherOrBoth<A, B>` to `EitherOrBoth<&A, &B>`.
    pub fn as_ref(&self) -> EitherOrBoth<&A, &B> {
        match *self {
            Left(ref left) => Left(left),
            Right(ref right) => Right(right),
            Both(ref left, ref right) => Both(left, right),
        }
    }

    /// Converts from `&mut EitherOrBoth<A, B>` to `EitherOrBoth<&mut A, &mut B>`.
    pub fn as_mut(&mut self) -> EitherOrBoth<&mut A, &mut B> {
        match *self {
            Left(ref mut left) => Left(left),
            Right(ref mut right) => Right(right),
            Both(ref mut left, ref mut right) => Both(left, right),
        }
    }

    /// Converts from `&EitherOrBoth<A, B>` to `EitherOrBoth<&_, &_>` using the [`Deref`] trait.
    pub fn as_deref(&self) -> EitherOrBoth<&A::Target, &B::Target>
    where
        A: Deref,
        B: Deref,
    {
        match *self {
            Left(ref left) => Left(left),
            Right(ref right) => Right(right),
            Both(ref left, ref right) => Both(left, right),
        }
    }

    /// Converts from `&mut EitherOrBoth<A, B>` to `EitherOrBoth<&mut _, &mut _>` using the [`DerefMut`] trait.
    pub fn as_deref_mut(&mut self) -> EitherOrBoth<&mut A::Target, &mut B::Target>
    where
        A: DerefMut,
        B: DerefMut,
    {
        match *self {
            Left(ref mut left) => Left(left),
            Right(ref mut right) => Right(right),
            Both(ref mut left, ref mut right) => Both(left, right),
        }
    }

    /// Convert `EitherOrBoth<A, B>` to `EitherOrBoth<B, A>`.
    pub fn flip(self) -> EitherOrBoth<B, A> {
        match self {
            Left(a) => Right(a),
            Right(b) => Left(b),
            Both(a, b) => Both(b, a),
        }
    }

    /// Apply the function `f` on the value `a` in `Left(a)` or `Both(a, b)` variants. If it is
    /// present rewrapping the result in `self`'s original variant.
    pub fn map_left<F, M>(self, f: F) -> EitherOrBoth<M, B>
    where
        F: FnOnce(A) -> M,
    {
        match self {
            Both(a, b) => Both(f(a), b),
            Left(a) => Left(f(a)),
            Right(b) => Right(b),
        }
    }

    /// Apply the function `f` on the value `b` in `Right(b)` or `Both(a, b)` variants.
    /// If it is present rewrapping the result in `self`'s original variant.
    pub fn map_right<F, M>(self, f: F) -> EitherOrBoth<A, M>
    where
        F: FnOnce(B) -> M,
    {
        match self {
            Left(a) => Left(a),
            Right(b) => Right(f(b)),
            Both(a, b) => Both(a, f(b)),
        }
    }

    /// Apply the functions `f` and `g` on the value `a` and `b` respectively;
    /// found in `Left(a)`, `Right(b)`, or `Both(a, b)` variants.
    /// The Result is rewrapped `self`'s original variant.
    pub fn map_any<F, L, G, R>(self, f: F, g: G) -> EitherOrBoth<L, R>
    where
        F: FnOnce(A) -> L,
        G: FnOnce(B) -> R,
    {
        match self {
            Left(a) => Left(f(a)),
            Right(b) => Right(g(b)),
            Both(a, b) => Both(f(a), g(b)),
        }
    }

    /// Apply the function `f` on the value `a` in `Left(a)` or `Both(a, _)` variants if it is
    /// present.
    pub fn left_and_then<F, L>(self, f: F) -> EitherOrBoth<L, B>
    where
        F: FnOnce(A) -> EitherOrBoth<L, B>,
    {
        match self {
            Left(a) | Both(a, _) => f(a),
            Right(b) => Right(b),
        }
    }

    /// Apply the function `f` on the value `b`
    /// in `Right(b)` or `Both(_, b)` variants if it is present.
    pub fn right_and_then<F, R>(self, f: F) -> EitherOrBoth<A, R>
    where
        F: FnOnce(B) -> EitherOrBoth<A, R>,
    {
        match self {
            Left(a) => Left(a),
            Right(b) | Both(_, b) => f(b),
        }
    }

    /// Returns a tuple consisting of the `l` and `r` in `Both(l, r)`, if present.
    /// Otherwise, returns the wrapped value for the present element, and the supplied
    /// value for the other. The first (`l`) argument is used for a missing `Left`
    /// value. The second (`r`) argument is used for a missing `Right` value.
    ///
    /// Arguments passed to `or` are eagerly evaluated; if you are passing
    /// the result of a function call, it is recommended to use [`or_else`],
    /// which is lazily evaluated.
    ///
    /// [`or_else`]: EitherOrBoth::or_else
    ///
    /// # Examples
    ///
    /// ```
    /// # use itertools::EitherOrBoth;
    /// assert_eq!(EitherOrBoth::Both("tree", 1).or("stone", 5), ("tree", 1));
    /// assert_eq!(EitherOrBoth::Left("tree").or("stone", 5), ("tree", 5));
    /// assert_eq!(EitherOrBoth::Right(1).or("stone", 5), ("stone", 1));
    /// ```
    pub fn or(self, l: A, r: B) -> (A, B) {
        match self {
            Left(inner_l) => (inner_l, r),
            Right(inner_r) => (l, inner_r),
            Both(inner_l, inner_r) => (inner_l, inner_r),
        }
    }

    /// Returns a tuple consisting of the `l` and `r` in `Both(l, r)`, if present.
    /// Otherwise, returns the wrapped value for the present element, and the [`default`](Default::default)
    /// for the other.
    pub fn or_default(self) -> (A, B)
    where
        A: Default,
        B: Default,
    {
        match self {
            Left(l) => (l, B::default()),
            Right(r) => (A::default(), r),
            Both(l, r) => (l, r),
        }
    }

    /// Returns a tuple consisting of the `l` and `r` in `Both(l, r)`, if present.
    /// Otherwise, returns the wrapped value for the present element, and computes the
    /// missing value with the supplied closure. The first argument (`l`) is used for a
    /// missing `Left` value. The second argument (`r`) is used for a missing `Right` value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use itertools::EitherOrBoth;
    /// let k = 10;
    /// assert_eq!(EitherOrBoth::Both("tree", 1).or_else(|| "stone", || 2 * k), ("tree", 1));
    /// assert_eq!(EitherOrBoth::Left("tree").or_else(|| "stone", || 2 * k), ("tree", 20));
    /// assert_eq!(EitherOrBoth::Right(1).or_else(|| "stone", || 2 * k), ("stone", 1));
    /// ```
    pub fn or_else<L: FnOnce() -> A, R: FnOnce() -> B>(self, l: L, r: R) -> (A, B) {
        match self {
            Left(inner_l) => (inner_l, r()),
            Right(inner_r) => (l(), inner_r),
            Both(inner_l, inner_r) => (inner_l, inner_r),
        }
    }

    /// Returns a mutable reference to the left value. If the left value is not present,
    /// it is replaced with `val`.
    pub fn left_or_insert(&mut self, val: A) -> &mut A {
        self.left_or_insert_with(|| val)
    }

    /// Returns a mutable reference to the right value. If the right value is not present,
    /// it is replaced with `val`.
    pub fn right_or_insert(&mut self, val: B) -> &mut B {
        self.right_or_insert_with(|| val)
    }

    /// If the left value is not present, replace it the value computed by the closure `f`.
    /// Returns a mutable reference to the now-present left value.
    pub fn left_or_insert_with<F>(&mut self, f: F) -> &mut A
    where
        F: FnOnce() -> A,
    {
        match self {
            Left(left) | Both(left, _) => left,
            Right(_) => self.insert_left(f()),
        }
    }

    /// If the right value is not present, replace it the value computed by the closure `f`.
    /// Returns a mutable reference to the now-present right value.
    pub fn right_or_insert_with<F>(&mut self, f: F) -> &mut B
    where
        F: FnOnce() -> B,
    {
        match self {
            Right(right) | Both(_, right) => right,
            Left(_) => self.insert_right(f()),
        }
    }

    /// Sets the `left` value of this instance, and returns a mutable reference to it.
    /// Does not affect the `right` value.
    ///
    /// # Examples
    /// ```
    /// # use itertools::{EitherOrBoth, EitherOrBoth::{Left, Right, Both}};
    ///
    /// // Overwriting a pre-existing value.
    /// let mut either: EitherOrBoth<_, ()> = Left(0_u32);
    /// assert_eq!(*either.insert_left(69), 69);
    ///
    /// // Inserting a second value.
    /// let mut either = Right("no");
    /// assert_eq!(*either.insert_left("yes"), "yes");
    /// assert_eq!(either, Both("yes", "no"));
    /// ```
    pub fn insert_left(&mut self, val: A) -> &mut A {
        match self {
            Left(left) | Both(left, _) => {
                *left = val;
                left
            }
            Right(right) => {
                // This is like a map in place operation. We move out of the reference,
                // change the value, and then move back into the reference.
                unsafe {
                    // SAFETY: We know this pointer is valid for reading since we got it from a reference.
                    let right = std::ptr::read(right as *mut _);
                    // SAFETY: Again, we know the pointer is valid since we got it from a reference.
                    std::ptr::write(self as *mut _, Both(val, right));
                }

                if let Both(left, _) = self {
                    left
                } else {
                    // SAFETY: The above pattern will always match, since we just
                    // set `self` equal to `Both`.
                    unsafe { std::hint::unreachable_unchecked() }
                }
            }
        }
    }

    /// Sets the `right` value of this instance, and returns a mutable reference to it.
    /// Does not affect the `left` value.
    ///
    /// # Examples
    /// ```
    /// # use itertools::{EitherOrBoth, EitherOrBoth::{Left, Both}};
    /// // Overwriting a pre-existing value.
    /// let mut either: EitherOrBoth<_, ()> = Left(0_u32);
    /// assert_eq!(*either.insert_left(69), 69);
    ///
    /// // Inserting a second value.
    /// let mut either = Left("what's");
    /// assert_eq!(*either.insert_right(9 + 10), 21 - 2);
    /// assert_eq!(either, Both("what's", 9+10));
    /// ```
    pub fn insert_right(&mut self, val: B) -> &mut B {
        match self {
            Right(right) | Both(_, right) => {
                *right = val;
                right
            }
            Left(left) => {
                // This is like a map in place operation. We move out of the reference,
                // change the value, and then move back into the reference.
                unsafe {
                    // SAFETY: We know this pointer is valid for reading since we got it from a reference.
                    let left = std::ptr::read(left as *mut _);
                    // SAFETY: Again, we know the pointer is valid since we got it from a reference.
                    std::ptr::write(self as *mut _, Both(left, val));
                }
                if let Both(_, right) = self {
                    right
                } else {
                    // SAFETY: The above pattern will always match, since we just
                    // set `self` equal to `Both`.
                    unsafe { std::hint::unreachable_unchecked() }
                }
            }
        }
    }

    /// Set `self` to `Both(..)`, containing the specified left and right values,
    /// and returns a mutable reference to those values.
    pub fn insert_both(&mut self, left: A, right: B) -> (&mut A, &mut B) {
        *self = Both(left, right);
        if let Both(left, right) = self {
            (left, right)
        } else {
            // SAFETY: The above pattern will always match, since we just
            // set `self` equal to `Both`.
            unsafe { std::hint::unreachable_unchecked() }
        }
    }
}

impl<T> EitherOrBoth<T, T> {
    /// Return either value of left, right, or apply a function `f` to both values if both are present.
    /// The input function has to return the same type as both Right and Left carry.
    ///
    /// This function can be used to preferrably extract the left resp. right value,
    /// but fall back to the other (i.e. right resp. left) if the preferred one is not present.
    ///
    /// # Examples
    /// ```
    /// # use itertools::EitherOrBoth;
    /// assert_eq!(EitherOrBoth::Both(3, 7).reduce(u32::max), 7);
    /// assert_eq!(EitherOrBoth::Left(3).reduce(u32::max), 3);
    /// assert_eq!(EitherOrBoth::Right(7).reduce(u32::max), 7);
    ///
    /// // Extract the left value if present, fall back to the right otherwise.
    /// assert_eq!(EitherOrBoth::Left("left").reduce(|l, _r| l), "left");
    /// assert_eq!(EitherOrBoth::Right("right").reduce(|l, _r| l), "right");
    /// assert_eq!(EitherOrBoth::Both("left", "right").reduce(|l, _r| l), "left");
    /// ```
    pub fn reduce<F>(self, f: F) -> T
    where
        F: FnOnce(T, T) -> T,
    {
        match self {
            Left(a) => a,
            Right(b) => b,
            Both(a, b) => f(a, b),
        }
    }
}

impl<A, B> From<EitherOrBoth<A, B>> for Option<Either<A, B>> {
    fn from(value: EitherOrBoth<A, B>) -> Self {
        match value {
            Left(l) => Some(Either::Left(l)),
            Right(r) => Some(Either::Right(r)),
            Both(..) => None,
        }
    }
}

impl<A, B> From<Either<A, B>> for EitherOrBoth<A, B> {
    fn from(either: Either<A, B>) -> Self {
        match either {
            Either::Left(l) => Left(l),
            Either::Right(l) => Right(l),
        }
    }
}
