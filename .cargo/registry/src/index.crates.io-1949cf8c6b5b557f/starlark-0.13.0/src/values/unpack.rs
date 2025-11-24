/*
 * Copyright 2018 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! Parameter conversion utilities for `starlark_module` macros.

use std::convert::Infallible;
use std::fmt::Debug;

use anyhow::Context;
use either::Either;
use starlark_syntax::StarlarkResultExt;

use crate::typing::Ty;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::Value;

/// Error that can be returned by [`UnpackValue`].
pub trait UnpackValueError: Debug + Send + Sync + 'static {
    /// Convert into a crate error.
    fn into_error(this: Self) -> crate::Error;
}

impl UnpackValueError for crate::Error {
    #[cold]
    fn into_error(this: Self) -> crate::Error {
        this
    }
}

impl UnpackValueError for anyhow::Error {
    #[cold]
    fn into_error(this: Self) -> crate::Error {
        crate::Error::new_value(this)
    }
}

impl UnpackValueError for Infallible {
    #[cold]
    fn into_error(this: Self) -> crate::Error {
        match this {}
    }
}

impl<A: UnpackValueError, B: UnpackValueError> UnpackValueError for Either<A, B> {
    #[cold]
    fn into_error(this: Self) -> crate::Error {
        match this {
            Either::Left(a) => UnpackValueError::into_error(a),
            Either::Right(b) => UnpackValueError::into_error(b),
        }
    }
}

/// Never error.
pub trait UnpackValueErrorInfallible: UnpackValueError {
    /// Convert into a never type.
    fn into_infallible(this: Self) -> !;
}

impl UnpackValueErrorInfallible for Infallible {
    fn into_infallible(this: Self) -> ! {
        match this {}
    }
}

impl<A: UnpackValueErrorInfallible, B: UnpackValueErrorInfallible> UnpackValueErrorInfallible
    for Either<A, B>
{
    fn into_infallible(this: Self) -> ! {
        match this {
            Either::Left(a) => UnpackValueErrorInfallible::into_infallible(a),
            Either::Right(b) => UnpackValueErrorInfallible::into_infallible(b),
        }
    }
}

/// How to convert a [`Value`] to a Rust type. Required for all arguments in
/// a [`#[starlark_module]`](macro@crate::starlark_module) definition.
///
/// Note for simple references it often can be implemented with `#[starlark_value(UnpackValue)]`,
/// for example:
///
/// ```
/// # use allocative::Allocative;
/// # use starlark::any::ProvidesStaticType;
/// # use starlark::values::{NoSerialize, StarlarkValue, starlark_value};
///
/// #[derive(
///     Debug,
///     derive_more::Display,
///     Allocative,
///     NoSerialize,
///     ProvidesStaticType
/// )]
/// struct MySimpleValue;
///
/// #[starlark_value(type = "MySimpleValue", UnpackValue, StarlarkTypeRepr)]
/// impl<'v> StarlarkValue<'v> for MySimpleValue {}
/// ```
///
/// Whereas for types that aren't also [`StarlarkValue`](crate::values::StarlarkValue) you can define:
///
/// ```
/// # use either::Either;
/// # use starlark::typing::Ty;
/// # use starlark::values::{UnpackValue, Value};
/// # use starlark::values::type_repr::StarlarkTypeRepr;
///
/// struct BoolOrInt(i32);
///
/// impl StarlarkTypeRepr for BoolOrInt {
///     type Canonical = <Either<bool, i32> as StarlarkTypeRepr>::Canonical;
///
///     fn starlark_type_repr() -> Ty {
///         Either::<bool, i32>::starlark_type_repr()
///     }
/// }
///
/// impl<'v> UnpackValue<'v> for BoolOrInt {
///     type Error = starlark::Error;
///
///     fn unpack_value_impl(value: Value<'v>) -> starlark::Result<Option<Self>> {
///         if let Some(x) = value.unpack_bool() {
///             Ok(Some(BoolOrInt(x as i32)))
///         } else {
///             let Some(x) = i32::unpack_value(value)? else {
///                 return Ok(None);
///             };
///             Ok(Some(BoolOrInt(x)))
///         }
///     }
/// }
/// ```
pub trait UnpackValue<'v>: Sized + StarlarkTypeRepr {
    /// Error returned when type matches, but conversion fails.
    ///
    /// Typically [`starlark::Error`](crate::Error), [`anyhow::Error`], or [`Infallible`].
    type Error: UnpackValueError;

    /// Given a [`Value`], try and unpack it into the given type,
    /// which may involve some element of conversion.
    ///
    /// Return `None` if the value is not of expected type (as described by [`StarlarkTypeRepr`],
    /// and return `Err` if the value is of expected type, but conversion cannot be performed.
    /// For example, when unpacking an integer to `String`, return `None`,
    /// and when unpacking a large integer to `i32`, return `Err`.
    ///
    /// This function is needs to be implemented, but usually not meant to be called directly.
    /// Consider using [`unpack_value`](UnpackValue::unpack_value),
    /// [`unpack_value_err`](UnpackValue::unpack_value_err),
    /// [`unpack_value_opt`](UnpackValue::unpack_value_opt) instead.
    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error>;

    /// Given a [`Value`], try and unpack it into the given type,
    /// which may involve some element of conversion.
    ///
    /// Return `None` if the value is not of expected type (as described by [`StarlarkTypeRepr`],
    /// and return `Err` if the value is of expected type, but conversion cannot be performed.
    /// For example, when unpacking an integer to `String`, return `None`,
    /// and when unpacking a large integer to `i32`, return `Err`.
    fn unpack_value(value: Value<'v>) -> Result<Option<Self>, crate::Error> {
        Self::unpack_value_impl(value).map_err(Self::Error::into_error)
    }

    /// Unpack a value if unpacking is infallible.
    fn unpack_value_opt(value: Value<'v>) -> Option<Self>
    where
        Self::Error: UnpackValueErrorInfallible,
    {
        match Self::unpack_value_impl(value) {
            Ok(x) => x,
            Err(e) => Self::Error::into_infallible(e),
        }
    }

    /// Unpack a value, but return error instead of `None` if unpacking fails.
    #[inline]
    fn unpack_value_err(value: Value<'v>) -> anyhow::Result<Self> {
        #[cold]
        fn error<'v>(value: Value<'v>, ty: fn() -> Ty) -> anyhow::Error {
            #[derive(thiserror::Error, Debug)]
            #[error("Expected `{0}`, but got `{1}`")]
            struct IncorrectType(Ty, String);

            crate::Error::new_value(IncorrectType(ty(), value.to_string_for_type_error()))
                .into_anyhow()
        }

        Self::unpack_value(value)
            .into_anyhow_result()?
            .ok_or_else(|| error(value, Self::starlark_type_repr))
    }

    /// Unpack value, but instead of `None` return error about incorrect argument type.
    #[inline]
    fn unpack_param(value: Value<'v>) -> anyhow::Result<Self> {
        #[cold]
        fn error<'v>(value: Value<'v>, ty: fn() -> Ty) -> anyhow::Error {
            #[derive(thiserror::Error, Debug)]
            #[error("Type of parameters mismatch, expected `{0}`, actual `{1}`")]
            struct IncorrectParameterTypeWithExpected(Ty, String);

            crate::Error::new_value(IncorrectParameterTypeWithExpected(
                ty(),
                value.to_string_for_type_error(),
            ))
            .into_anyhow()
        }

        Self::unpack_value(value)
            .into_anyhow_result()?
            .ok_or_else(|| error(value, Self::starlark_type_repr))
    }

    /// Unpack value, but instead of `None` return error about incorrect named argument type.
    #[inline]
    fn unpack_named_param(value: Value<'v>, param_name: &str) -> anyhow::Result<Self> {
        #[cold]
        fn error<'v>(value: Value<'v>, param_name: &str, ty: fn() -> Ty) -> anyhow::Error {
            #[derive(thiserror::Error, Debug)]
            #[error("Type of parameter `{0}` doesn't match, expected `{1}`, actual `{2}`")]
            struct IncorrectParameterTypeNamedWithExpected(String, Ty, String);

            crate::Error::new_value(IncorrectParameterTypeNamedWithExpected(
                param_name.to_owned(),
                ty(),
                value.to_string_for_type_error(),
            ))
            .into_anyhow()
        }

        Self::unpack_value(value)
            .into_anyhow_result()
            .with_context(|| {
                format!(
                    "Error unpacking value for parameter `{}` of type `{}",
                    param_name,
                    Self::starlark_type_repr()
                )
            })?
            .ok_or_else(|| error(value, param_name, Self::starlark_type_repr))
    }
}

impl<'v> UnpackValue<'v> for Value<'v> {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        Ok(Some(value))
    }
}

impl<'v, TLeft: UnpackValue<'v>, TRight: UnpackValue<'v>> UnpackValue<'v>
    for Either<TLeft, TRight>
{
    type Error = Either<TLeft::Error, TRight::Error>;

    // Only implemented for types that implement [`UnpackValue`]. Nonsensical for other types.
    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        if let Some(left) = TLeft::unpack_value_impl(value).map_err(Either::Left)? {
            Ok(Some(Self::Left(left)))
        } else {
            Ok(TRight::unpack_value_impl(value)
                .map_err(Either::Right)?
                .map(Self::Right))
        }
    }
}
