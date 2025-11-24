//! The trait [`IntoEither`] provides methods for converting a type `Self`, whose
//! size is constant and known at compile-time, into an [`Either`] variant.

use super::{Either, Left, Right};

/// Provides methods for converting a type `Self` into either a [`Left`] or [`Right`]
/// variant of [`Either<Self, Self>`](Either).
///
/// The [`into_either`](IntoEither::into_either) method takes a [`bool`] to determine
/// whether to convert to [`Left`] or [`Right`].
///
/// The [`into_either_with`](IntoEither::into_either_with) method takes a
/// [predicate function](FnOnce) to determine whether to convert to [`Left`] or [`Right`].
pub trait IntoEither: Sized {
    /// Converts `self` into a [`Left`] variant of [`Either<Self, Self>`](Either)
    /// if `into_left` is `true`.
    /// Converts `self` into a [`Right`] variant of [`Either<Self, Self>`](Either)
    /// otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use either::{IntoEither, Left, Right};
    ///
    /// let x = 0;
    /// assert_eq!(x.into_either(true), Left(x));
    /// assert_eq!(x.into_either(false), Right(x));
    /// ```
    fn into_either(self, into_left: bool) -> Either<Self, Self> {
        if into_left {
            Left(self)
        } else {
            Right(self)
        }
    }

    /// Converts `self` into a [`Left`] variant of [`Either<Self, Self>`](Either)
    /// if `into_left(&self)` returns `true`.
    /// Converts `self` into a [`Right`] variant of [`Either<Self, Self>`](Either)
    /// otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use either::{IntoEither, Left, Right};
    ///
    /// fn is_even(x: &u8) -> bool {
    ///     x % 2 == 0
    /// }
    ///
    /// let x = 0;
    /// assert_eq!(x.into_either_with(is_even), Left(x));
    /// assert_eq!(x.into_either_with(|x| !is_even(x)), Right(x));
    /// ```
    fn into_either_with<F>(self, into_left: F) -> Either<Self, Self>
    where
        F: FnOnce(&Self) -> bool,
    {
        let into_left = into_left(&self);
        self.into_either(into_left)
    }
}

impl<T> IntoEither for T {}
