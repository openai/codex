//! This module contains helper traits and types used by the public-facing
//! macros. Most are public so they can be accessed by the expanded macro code,
//! but are not meant to be used by users directly and do not have a stable API.
//!
//! The various `TryCast*` traits in this module are referenced in macro
//! expansions and expose multiple possible implementations of casting with
//! different generic bounds. The compiler chooses which trait to use to fulfill
//! the cast based on the trait bounds using the _autoderef_ trick.

use crate::{
    lifetime_free::LifetimeFree,
    utils::{transmute_unchecked, type_eq, type_eq_non_static},
};
use core::marker::PhantomData;

/// A token struct used to capture a type without taking ownership of any
/// values. Used to select a cast implementation in macros.
pub struct CastToken<T: ?Sized>(PhantomData<T>);

impl<T: ?Sized> CastToken<T> {
    /// Create a cast token for the given type of value.
    pub const fn of_val(_value: &T) -> Self {
        Self::of()
    }

    /// Create a new cast token of the specified type.
    pub const fn of() -> Self {
        Self(PhantomData)
    }
}

/// Supporting trait for autoderef specialization on mutable references to lifetime-free
/// types.
pub trait TryCastMutLifetimeFree<'a, T: ?Sized, U: LifetimeFree + ?Sized> {
    #[inline(always)]
    fn try_cast(&self, value: &'a mut T) -> Result<&'a mut U, &'a mut T> {
        // SAFETY: See comments on safety in `TryCastLifetimeFree`.

        if type_eq_non_static::<T, U>() {
            // Pointer casts are not allowed here since the compiler can't prove
            // that `&mut T` and `&mut U` have the same kind of associated
            // pointer data if they are fat pointers. But we know they are
            // identical, so we use a transmute.
            Ok(unsafe { transmute_unchecked::<&mut T, &mut U>(value) })
        } else {
            Err(value)
        }
    }
}

impl<'a, T: ?Sized, U: LifetimeFree + ?Sized> TryCastMutLifetimeFree<'a, T, U>
    for &&&&&&&(CastToken<&'a mut T>, CastToken<&'a mut U>)
{
}

/// Supporting trait for autoderef specialization on references to lifetime-free
/// types.
pub trait TryCastRefLifetimeFree<'a, T: ?Sized, U: LifetimeFree + ?Sized> {
    #[inline(always)]
    fn try_cast(&self, value: &'a T) -> Result<&'a U, &'a T> {
        // SAFETY: See comments on safety in `TryCastLifetimeFree`.

        if type_eq_non_static::<T, U>() {
            // Pointer casts are not allowed here since the compiler can't prove
            // that `&T` and `&U` have the same kind of associated pointer data if
            // they are fat pointers. But we know they are identical, so we use
            // a transmute.
            Ok(unsafe { transmute_unchecked::<&T, &U>(value) })
        } else {
            Err(value)
        }
    }
}

impl<'a, T: ?Sized, U: LifetimeFree + ?Sized> TryCastRefLifetimeFree<'a, T, U>
    for &&&&&&(CastToken<&'a T>, CastToken<&'a U>)
{
}

/// Supporting trait for autoderef specialization on lifetime-free types.
pub trait TryCastOwnedLifetimeFree<T, U: LifetimeFree> {
    #[inline(always)]
    fn try_cast(&self, value: T) -> Result<U, T> {
        // SAFETY: If `U` is lifetime-free, and the base types of `T` and `U`
        // are equal, then `T` is also lifetime-free. Therefore `T` and `U` are
        // strictly identical and it is safe to cast a `T` into a `U`.
        //
        // We know that `U` is lifetime-free because of the `LifetimeFree` trait
        // checked statically. `LifetimeFree` is an unsafe trait implemented for
        // individual types, so the burden of verifying that a type is indeed
        // lifetime-free is on the implementer.

        if type_eq_non_static::<T, U>() {
            Ok(unsafe { transmute_unchecked::<T, U>(value) })
        } else {
            Err(value)
        }
    }
}

impl<T, U: LifetimeFree> TryCastOwnedLifetimeFree<T, U> for &&&&&(CastToken<T>, CastToken<U>) {}

/// Supporting trait for autoderef specialization on mutable slices.
pub trait TryCastSliceMut<'a, T: 'static, U: 'static> {
    /// Attempt to cast a generic mutable slice to a given type if the types are
    /// equal.
    ///
    /// The reference does not have to be static as long as the item type is
    /// static.
    #[inline(always)]
    fn try_cast(&self, value: &'a mut [T]) -> Result<&'a mut [U], &'a mut [T]> {
        if type_eq::<T, U>() {
            Ok(unsafe { &mut *(value as *mut [T] as *mut [U]) })
        } else {
            Err(value)
        }
    }
}

impl<'a, T: 'static, U: 'static> TryCastSliceMut<'a, T, U>
    for &&&&(CastToken<&'a mut [T]>, CastToken<&'a mut [U]>)
{
}

/// Supporting trait for autoderef specialization on slices.
pub trait TryCastSliceRef<'a, T: 'static, U: 'static> {
    /// Attempt to cast a generic slice to a given type if the types are equal.
    ///
    /// The reference does not have to be static as long as the item type is
    /// static.
    #[inline(always)]
    fn try_cast(&self, value: &'a [T]) -> Result<&'a [U], &'a [T]> {
        if type_eq::<T, U>() {
            Ok(unsafe { &*(value as *const [T] as *const [U]) })
        } else {
            Err(value)
        }
    }
}

impl<'a, T: 'static, U: 'static> TryCastSliceRef<'a, T, U>
    for &&&(CastToken<&'a [T]>, CastToken<&'a [U]>)
{
}

/// Supporting trait for autoderef specialization on mutable references.
pub trait TryCastMut<'a, T: 'static, U: 'static> {
    /// Attempt to cast a generic mutable reference to a given type if the types
    /// are equal.
    ///
    /// The reference does not have to be static as long as the reference target
    /// type is static.
    #[inline(always)]
    fn try_cast(&self, value: &'a mut T) -> Result<&'a mut U, &'a mut T> {
        if type_eq::<T, U>() {
            Ok(unsafe { &mut *(value as *mut T as *mut U) })
        } else {
            Err(value)
        }
    }
}

impl<'a, T: 'static, U: 'static> TryCastMut<'a, T, U>
    for &&(CastToken<&'a mut T>, CastToken<&'a mut U>)
{
}

/// Supporting trait for autoderef specialization on references.
pub trait TryCastRef<'a, T: 'static, U: 'static> {
    /// Attempt to cast a generic reference to a given type if the types are
    /// equal.
    ///
    /// The reference does not have to be static as long as the reference target
    /// type is static.
    #[inline(always)]
    fn try_cast(&self, value: &'a T) -> Result<&'a U, &'a T> {
        if type_eq::<T, U>() {
            Ok(unsafe { &*(value as *const T as *const U) })
        } else {
            Err(value)
        }
    }
}

impl<'a, T: 'static, U: 'static> TryCastRef<'a, T, U> for &(CastToken<&'a T>, CastToken<&'a U>) {}

/// Default trait for autoderef specialization.
pub trait TryCastOwned<T: 'static, U: 'static> {
    /// Attempt to cast a value to a given type if the types are equal.
    #[inline(always)]
    fn try_cast(&self, value: T) -> Result<U, T> {
        if type_eq::<T, U>() {
            Ok(unsafe { transmute_unchecked::<T, U>(value) })
        } else {
            Err(value)
        }
    }
}

impl<T: 'static, U: 'static> TryCastOwned<T, U> for (CastToken<T>, CastToken<U>) {}
