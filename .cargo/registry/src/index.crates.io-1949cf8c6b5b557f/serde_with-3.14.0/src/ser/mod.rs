//! Module for [`SerializeAs`][] implementations
//!
//! The module contains the [`SerializeAs`][] trait and helper code.
//! Additionally, it contains implementations of [`SerializeAs`][] for types defined in the Rust Standard Library or this crate.
//!
//! You can find more details on how to implement this trait for your types in the documentation of the [`SerializeAs`][] trait and details about the usage in the [user guide][].
//!
//! [user guide]: crate::guide

#[cfg(feature = "alloc")]
mod duplicates;
mod impls;
#[cfg(feature = "alloc")]
mod skip_error;

use crate::prelude::*;

/// A **data structure** that can be serialized into any data format supported by Serde, analogue to [`Serialize`].
///
/// The trait is analogue to the [`serde::Serialize`][`Serialize`] trait, with the same meaning of input and output arguments.
/// It can and should be implemented using the same code structure as the [`Serialize`] trait.
/// As such, the same advice for [implementing `Serialize`][impl-serialize] applies here.
///
/// # Differences to [`Serialize`]
///
/// The trait is only required for container-like types or types implementing specific conversion functions.
/// Container-like types are [`Vec`], [`BTreeMap`], but also [`Option`] and [`Box`].
/// Conversion types serialize into a different serde data type.
/// For example, [`DisplayFromStr`] uses the [`Display`] trait to serialize a String and [`DurationSeconds`] converts a [`Duration`] into either String or integer values.
///
/// This code shows how to implement [`Serialize`] for [`Box`]:
///
/// ```rust,ignore
/// impl<T> Serialize for Box<T>
/// where
///     T: Serialize,
/// {
///     #[inline]
///     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
///     where
///         S: Serializer,
///     {
///         (**self).serialize(serializer)
///     }
/// }
/// ```
///
/// and this code shows how to do the same using [`SerializeAs`][]:
///
/// ```rust,ignore
/// impl<T, U> SerializeAs<Box<T>> for Box<U>
/// where
///     U: SerializeAs<T>,
/// {
///     fn serialize_as<S>(source: &Box<T>, serializer: S) -> Result<S::Ok, S::Error>
///     where
///         S: Serializer,
///     {
///         SerializeAsWrap::<T, U>::new(source).serialize(serializer)
///     }
/// }
/// ```
///
/// It uses two type parameters, `T` and `U` instead of only one and performs the serialization step using the `SerializeAsWrap` type.
/// The `T` type is the type on the Rust side before serialization, whereas the `U` type determines how the value will be serialized.
/// These two changes are usually enough to make a container type implement [`SerializeAs`][].
///
/// [`SerializeAsWrap`] is a piece of glue code which turns [`SerializeAs`] into a serde compatible datatype, by converting all calls to `serialize` into `serialize_as`.
/// This allows us to implement [`SerializeAs`] such that it can be applied recursively throughout the whole data structure.
/// This is mostly important for container types, such as `Vec` or `BTreeMap`.
/// In a `BTreeMap` this allows us to specify two different serialization behaviors, one for key and one for value, using the [`SerializeAs`] trait.
///
/// ## Implementing a converter Type
///
/// This shows a simplified implementation for [`DisplayFromStr`].
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde_with::{serde_as, SerializeAs};
/// # use std::fmt::Display;
/// struct DisplayFromStr;
///
/// impl<T> SerializeAs<T> for DisplayFromStr
/// where
///     T: Display,
/// {
///     fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
///     where
///         S: serde::Serializer,
///     {
///         serializer.collect_str(&source)
///     }
/// }
/// #
/// # #[serde_as]
/// # #[derive(serde::Serialize)]
/// # struct S (#[serde_as(as = "DisplayFromStr")] bool);
/// #
/// # assert_eq!(r#""false""#, serde_json::to_string(&S(false)).unwrap());
/// # }
/// ```
///
/// [`Box`]: std::boxed::Box
/// [`BTreeMap`]: std::collections::BTreeMap
/// [`Display`]: std::fmt::Display
/// [`Duration`]: std::time::Duration
/// [`Vec`]: std::vec::Vec
/// [impl-serialize]: https://serde.rs/impl-serialize.html
pub trait SerializeAs<T: ?Sized> {
    /// Serialize this value into the given Serde serializer.
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer;
}

/// Helper type to implement [`SerializeAs`] for container-like types.
pub struct SerializeAsWrap<'a, T: ?Sized, U: ?Sized> {
    value: &'a T,
    marker: PhantomData<U>,
}

impl<'a, T, U> SerializeAsWrap<'a, T, U>
where
    T: ?Sized,
    U: ?Sized,
{
    /// Create new instance with provided value.
    pub fn new(value: &'a T) -> Self {
        Self {
            value,
            marker: PhantomData,
        }
    }
}

impl<T, U> Serialize for SerializeAsWrap<'_, T, U>
where
    T: ?Sized,
    U: ?Sized,
    U: SerializeAs<T>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        U::serialize_as(self.value, serializer)
    }
}

impl<'a, T, U> From<&'a T> for SerializeAsWrap<'a, T, U>
where
    T: ?Sized,
    U: ?Sized,
    U: SerializeAs<T>,
{
    fn from(value: &'a T) -> Self {
        Self::new(value)
    }
}

impl<T: ?Sized> As<T> {
    /// Serialize type `T` using [`SerializeAs`][]
    ///
    /// The function signature is compatible with [serde's `with` annotation][with-annotation].
    ///
    /// [with-annotation]: https://serde.rs/field-attrs.html#with
    pub fn serialize<S, I>(value: &I, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: SerializeAs<I>,
        I: ?Sized,
    {
        T::serialize_as(value, serializer)
    }
}
