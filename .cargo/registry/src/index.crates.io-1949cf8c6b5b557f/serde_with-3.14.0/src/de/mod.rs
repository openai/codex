//! Module for [`DeserializeAs`][] implementations
//!
//! The module contains the [`DeserializeAs`][] trait and helper code.
//! Additionally, it contains implementations of [`DeserializeAs`][] for types defined in the Rust Standard Library or this crate.
//!
//! You can find more details on how to implement this trait for your types in the documentation of the [`DeserializeAs`][] trait and details about the usage in the [user guide][].
//!
//! [user guide]: crate::guide

#[cfg(feature = "alloc")]
mod duplicates;
mod impls;
#[cfg(feature = "alloc")]
mod skip_error;

use crate::prelude::*;

/// A **data structure** that can be deserialized from any data format supported by Serde, analogue to [`Deserialize`].
///
/// The trait is analogue to the [`serde::Deserialize`][`Deserialize`] trait, with the same meaning of input and output arguments.
/// It can and should be implemented using the same code structure as the [`Deserialize`] trait.
/// As such, the same advice for [implementing `Deserialize`][impl-deserialize] applies here.
///
/// # Differences to [`Deserialize`]
///
/// The trait is only required for container-like types or types implementing specific conversion functions.
/// Container-like types are [`Vec`], [`BTreeMap`], but also [`Option`] and [`Box`].
/// Conversion types deserialize into a different Rust type.
/// For example, [`DisplayFromStr`] uses the [`FromStr`] trait after deserializing a string and [`DurationSeconds`] creates a [`Duration`] from either String or integer values.
///
/// This code shows how to implement [`Deserialize`] for [`Box`]:
///
/// ```rust,ignore
/// impl<'de, T: Deserialize<'de>> Deserialize<'de> for Box<T> {
///     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
///     where
///         D: Deserializer<'de>,
///     {
///         Ok(Box::new(Deserialize::deserialize(deserializer)?))
///     }
/// }
/// ```
///
/// and this code shows how to do the same using [`DeserializeAs`][]:
///
/// ```rust,ignore
/// impl<'de, T, U> DeserializeAs<'de, Box<T>> for Box<U>
/// where
///     U: DeserializeAs<'de, T>,
/// {
///     fn deserialize_as<D>(deserializer: D) -> Result<Box<T>, D::Error>
///     where
///         D: Deserializer<'de>,
///     {
///         Ok(Box::new(
///             DeserializeAsWrap::<T, U>::deserialize(deserializer)?.into_inner(),
///         ))
///     }
/// }
/// ```
///
/// It uses two type parameters, `T` and `U` instead of only one and performs the deserialization step using the `DeserializeAsWrap` type.
/// The `T` type is the type on the Rust side after deserialization, whereas the `U` type determines how the value will be deserialized.
/// These two changes are usually enough to make a container type implement [`DeserializeAs`][].
///
///
/// [`DeserializeAsWrap`] is a piece of glue code which turns [`DeserializeAs`] into a serde compatible datatype, by converting all calls to `deserialize` into `deserialize_as`.
/// This allows us to implement [`DeserializeAs`] such that it can be applied recursively throughout the whole data structure.
/// This is mostly important for container types, such as `Vec` or `BTreeMap`.
/// In a `BTreeMap` this allows us to specify two different serialization behaviors, one for key and one for value, using the [`DeserializeAs`] trait.
///
/// ## Implementing a converter Type
///
/// This shows a simplified implementation for [`DisplayFromStr`].
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::Deserialize;
/// # use serde::de::Error;
/// # use serde_with::{serde_as, DeserializeAs};
/// # use std::str::FromStr;
/// # use std::fmt::Display;
/// struct DisplayFromStr;
///
/// impl<'de, T> DeserializeAs<'de, T> for DisplayFromStr
/// where
///     T: FromStr,
///     T::Err: Display,
/// {
///     fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
///     where
///         D: serde::Deserializer<'de>,
///     {
///         let s = String::deserialize(deserializer).map_err(Error::custom)?;
///         s.parse().map_err(Error::custom)
///     }
/// }
/// #
/// # #[serde_as]
/// # #[derive(serde::Deserialize)]
/// # struct S (#[serde_as(as = "DisplayFromStr")] bool);
/// #
/// # assert!(!serde_json::from_str::<S>(r#""false""#).unwrap().0);
/// # }
/// ```
/// [`Box`]: std::boxed::Box
/// [`BTreeMap`]: std::collections::BTreeMap
/// [`Duration`]: std::time::Duration
/// [`FromStr`]: std::str::FromStr
/// [`Vec`]: std::vec::Vec
/// [impl-deserialize]: https://serde.rs/impl-deserialize.html
pub trait DeserializeAs<'de, T>: Sized {
    /// Deserialize this value from the given Serde deserializer.
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>;
}

/// Helper type to implement [`DeserializeAs`] for container-like types.
pub struct DeserializeAsWrap<T, U> {
    value: T,
    marker: PhantomData<U>,
}

impl<T, U> DeserializeAsWrap<T, U> {
    /// Return the inner value of type `T`.
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<'de, T, U> Deserialize<'de> for DeserializeAsWrap<T, U>
where
    U: DeserializeAs<'de, T>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        U::deserialize_as(deserializer).map(|value| Self {
            value,
            marker: PhantomData,
        })
    }
}

impl<T: ?Sized> As<T> {
    /// Deserialize type `T` using [`DeserializeAs`][]
    ///
    /// The function signature is compatible with [serde's `with` annotation][with-annotation].
    ///
    /// [with-annotation]: https://serde.rs/field-attrs.html#with
    pub fn deserialize<'de, D, I>(deserializer: D) -> Result<I, D::Error>
    where
        T: DeserializeAs<'de, I>,
        D: Deserializer<'de>,
    {
        T::deserialize_as(deserializer)
    }
}
