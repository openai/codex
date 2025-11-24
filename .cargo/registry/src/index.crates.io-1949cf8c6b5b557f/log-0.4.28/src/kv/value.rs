//! Structured values.
//!
//! This module defines the [`Value`] type and supporting APIs for
//! capturing and serializing them.

use std::fmt;

pub use crate::kv::Error;

/// A type that can be converted into a [`Value`](struct.Value.html).
pub trait ToValue {
    /// Perform the conversion.
    fn to_value(&self) -> Value;
}

impl<'a, T> ToValue for &'a T
where
    T: ToValue + ?Sized,
{
    fn to_value(&self) -> Value {
        (**self).to_value()
    }
}

impl<'v> ToValue for Value<'v> {
    fn to_value(&self) -> Value {
        Value {
            inner: self.inner.clone(),
        }
    }
}

/// A value in a key-value.
///
/// Values are an anonymous bag containing some structured datum.
///
/// # Capturing values
///
/// There are a few ways to capture a value:
///
/// - Using the `Value::from_*` methods.
/// - Using the `ToValue` trait.
/// - Using the standard `From` trait.
///
/// ## Using the `Value::from_*` methods
///
/// `Value` offers a few constructor methods that capture values of different kinds.
///
/// ```
/// use log::kv::Value;
///
/// let value = Value::from_debug(&42i32);
///
/// assert_eq!(None, value.to_i64());
/// ```
///
/// ## Using the `ToValue` trait
///
/// The `ToValue` trait can be used to capture values generically.
/// It's the bound used by `Source`.
///
/// ```
/// # use log::kv::ToValue;
/// let value = 42i32.to_value();
///
/// assert_eq!(Some(42), value.to_i64());
/// ```
///
/// ## Using the standard `From` trait
///
/// Standard types that implement `ToValue` also implement `From`.
///
/// ```
/// use log::kv::Value;
///
/// let value = Value::from(42i32);
///
/// assert_eq!(Some(42), value.to_i64());
/// ```
///
/// # Data model
///
/// Values can hold one of a number of types:
///
/// - **Null:** The absence of any other meaningful value. Note that
///   `Some(Value::null())` is not the same as `None`. The former is
///   `null` while the latter is `undefined`. This is important to be
///   able to tell the difference between a key-value that was logged,
///   but its value was empty (`Some(Value::null())`) and a key-value
///   that was never logged at all (`None`).
/// - **Strings:** `str`, `char`.
/// - **Booleans:** `bool`.
/// - **Integers:** `u8`-`u128`, `i8`-`i128`, `NonZero*`.
/// - **Floating point numbers:** `f32`-`f64`.
/// - **Errors:** `dyn (Error + 'static)`.
/// - **`serde`:** Any type in `serde`'s data model.
/// - **`sval`:** Any type in `sval`'s data model.
///
/// # Serialization
///
/// Values provide a number of ways to be serialized.
///
/// For basic types the [`Value::visit`] method can be used to extract the
/// underlying typed value. However, this is limited in the amount of types
/// supported (see the [`VisitValue`] trait methods).
///
/// For more complex types one of the following traits can be used:
///  * `sval::Value`, requires the `kv_sval` feature.
///  * `serde::Serialize`, requires the `kv_serde` feature.
///
/// You don't need a visitor to serialize values through `serde` or `sval`.
///
/// A value can always be serialized using any supported framework, regardless
/// of how it was captured. If, for example, a value was captured using its
/// `Display` implementation, it will serialize through `serde` as a string. If it was
/// captured as a struct using `serde`, it will also serialize as a struct
/// through `sval`, or can be formatted using a `Debug`-compatible representation.
#[derive(Clone)]
pub struct Value<'v> {
    inner: inner::Inner<'v>,
}

impl<'v> Value<'v> {
    /// Get a value from a type implementing `ToValue`.
    pub fn from_any<T>(value: &'v T) -> Self
    where
        T: ToValue,
    {
        value.to_value()
    }

    /// Get a value from a type implementing `std::fmt::Debug`.
    pub fn from_debug<T>(value: &'v T) -> Self
    where
        T: fmt::Debug,
    {
        Value {
            inner: inner::Inner::from_debug(value),
        }
    }

    /// Get a value from a type implementing `std::fmt::Display`.
    pub fn from_display<T>(value: &'v T) -> Self
    where
        T: fmt::Display,
    {
        Value {
            inner: inner::Inner::from_display(value),
        }
    }

    /// Get a value from a type implementing `serde::Serialize`.
    #[cfg(feature = "kv_serde")]
    pub fn from_serde<T>(value: &'v T) -> Self
    where
        T: serde::Serialize,
    {
        Value {
            inner: inner::Inner::from_serde1(value),
        }
    }

    /// Get a value from a type implementing `sval::Value`.
    #[cfg(feature = "kv_sval")]
    pub fn from_sval<T>(value: &'v T) -> Self
    where
        T: sval::Value,
    {
        Value {
            inner: inner::Inner::from_sval2(value),
        }
    }

    /// Get a value from a dynamic `std::fmt::Debug`.
    pub fn from_dyn_debug(value: &'v dyn fmt::Debug) -> Self {
        Value {
            inner: inner::Inner::from_dyn_debug(value),
        }
    }

    /// Get a value from a dynamic `std::fmt::Display`.
    pub fn from_dyn_display(value: &'v dyn fmt::Display) -> Self {
        Value {
            inner: inner::Inner::from_dyn_display(value),
        }
    }

    /// Get a value from a dynamic error.
    #[cfg(feature = "kv_std")]
    pub fn from_dyn_error(err: &'v (dyn std::error::Error + 'static)) -> Self {
        Value {
            inner: inner::Inner::from_dyn_error(err),
        }
    }

    /// Get a `null` value.
    pub fn null() -> Self {
        Value {
            inner: inner::Inner::empty(),
        }
    }

    /// Get a value from an internal primitive.
    fn from_inner<T>(value: T) -> Self
    where
        T: Into<inner::Inner<'v>>,
    {
        Value {
            inner: value.into(),
        }
    }

    /// Inspect this value using a simple visitor.
    ///
    /// When the `kv_serde` or `kv_sval` features are enabled, you can also
    /// serialize a value using its `Serialize` or `Value` implementation.
    pub fn visit(&self, visitor: impl VisitValue<'v>) -> Result<(), Error> {
        inner::visit(&self.inner, visitor)
    }
}

impl<'v> fmt::Debug for Value<'v> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl<'v> fmt::Display for Value<'v> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

#[cfg(feature = "kv_serde")]
impl<'v> serde::Serialize for Value<'v> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.inner.serialize(s)
    }
}

#[cfg(feature = "kv_sval")]
impl<'v> sval::Value for Value<'v> {
    fn stream<'sval, S: sval::Stream<'sval> + ?Sized>(&'sval self, stream: &mut S) -> sval::Result {
        sval::Value::stream(&self.inner, stream)
    }
}

#[cfg(feature = "kv_sval")]
impl<'v> sval_ref::ValueRef<'v> for Value<'v> {
    fn stream_ref<S: sval::Stream<'v> + ?Sized>(&self, stream: &mut S) -> sval::Result {
        sval_ref::ValueRef::stream_ref(&self.inner, stream)
    }
}

impl ToValue for str {
    fn to_value(&self) -> Value {
        Value::from(self)
    }
}

impl<'v> From<&'v str> for Value<'v> {
    fn from(value: &'v str) -> Self {
        Value::from_inner(value)
    }
}

impl ToValue for () {
    fn to_value(&self) -> Value {
        Value::from_inner(())
    }
}

impl<T> ToValue for Option<T>
where
    T: ToValue,
{
    fn to_value(&self) -> Value {
        match *self {
            Some(ref value) => value.to_value(),
            None => Value::from_inner(()),
        }
    }
}

macro_rules! impl_to_value_primitive {
    ($($into_ty:ty,)*) => {
        $(
            impl ToValue for $into_ty {
                fn to_value(&self) -> Value {
                    Value::from(*self)
                }
            }

            impl<'v> From<$into_ty> for Value<'v> {
                fn from(value: $into_ty) -> Self {
                    Value::from_inner(value)
                }
            }

            impl<'v> From<&'v $into_ty> for Value<'v> {
                fn from(value: &'v $into_ty) -> Self {
                    Value::from_inner(*value)
                }
            }
        )*
    };
}

macro_rules! impl_to_value_nonzero_primitive {
    ($($into_ty:ident,)*) => {
        $(
            impl ToValue for std::num::$into_ty {
                fn to_value(&self) -> Value {
                    Value::from(self.get())
                }
            }

            impl<'v> From<std::num::$into_ty> for Value<'v> {
                fn from(value: std::num::$into_ty) -> Self {
                    Value::from(value.get())
                }
            }

            impl<'v> From<&'v std::num::$into_ty> for Value<'v> {
                fn from(value: &'v std::num::$into_ty) -> Self {
                    Value::from(value.get())
                }
            }
        )*
    };
}

macro_rules! impl_value_to_primitive {
    ($(#[doc = $doc:tt] $into_name:ident -> $into_ty:ty,)*) => {
        impl<'v> Value<'v> {
            $(
                #[doc = $doc]
                pub fn $into_name(&self) -> Option<$into_ty> {
                    self.inner.$into_name()
                }
            )*
        }
    }
}

impl_to_value_primitive![
    usize, u8, u16, u32, u64, u128, isize, i8, i16, i32, i64, i128, f32, f64, char, bool,
];

#[rustfmt::skip]
impl_to_value_nonzero_primitive![
    NonZeroUsize, NonZeroU8, NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU128,
    NonZeroIsize, NonZeroI8, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI128,
];

impl_value_to_primitive![
    #[doc = "Try convert this value into a `u64`."]
    to_u64 -> u64,
    #[doc = "Try convert this value into a `i64`."]
    to_i64 -> i64,
    #[doc = "Try convert this value into a `u128`."]
    to_u128 -> u128,
    #[doc = "Try convert this value into a `i128`."]
    to_i128 -> i128,
    #[doc = "Try convert this value into a `f64`."]
    to_f64 -> f64,
    #[doc = "Try convert this value into a `char`."]
    to_char -> char,
    #[doc = "Try convert this value into a `bool`."]
    to_bool -> bool,
];

impl<'v> Value<'v> {
    /// Try to convert this value into an error.
    #[cfg(feature = "kv_std")]
    pub fn to_borrowed_error(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.to_borrowed_error()
    }

    /// Try to convert this value into a borrowed string.
    pub fn to_borrowed_str(&self) -> Option<&'v str> {
        self.inner.to_borrowed_str()
    }
}

#[cfg(feature = "kv_std")]
mod std_support {
    use std::borrow::Cow;
    use std::rc::Rc;
    use std::sync::Arc;

    use super::*;

    impl<T> ToValue for Box<T>
    where
        T: ToValue + ?Sized,
    {
        fn to_value(&self) -> Value {
            (**self).to_value()
        }
    }

    impl<T> ToValue for Arc<T>
    where
        T: ToValue + ?Sized,
    {
        fn to_value(&self) -> Value {
            (**self).to_value()
        }
    }

    impl<T> ToValue for Rc<T>
    where
        T: ToValue + ?Sized,
    {
        fn to_value(&self) -> Value {
            (**self).to_value()
        }
    }

    impl ToValue for String {
        fn to_value(&self) -> Value {
            Value::from(&**self)
        }
    }

    impl<'v> ToValue for Cow<'v, str> {
        fn to_value(&self) -> Value {
            Value::from(&**self)
        }
    }

    impl<'v> Value<'v> {
        /// Try convert this value into a string.
        pub fn to_cow_str(&self) -> Option<Cow<'v, str>> {
            self.inner.to_str()
        }
    }

    impl<'v> From<&'v String> for Value<'v> {
        fn from(v: &'v String) -> Self {
            Value::from(&**v)
        }
    }
}

/// A visitor for a [`Value`].
///
/// Also see [`Value`'s documentation on serialization]. Value visitors are a simple alternative
/// to a more fully-featured serialization framework like `serde` or `sval`. A value visitor
/// can differentiate primitive types through methods like [`VisitValue::visit_bool`] and
/// [`VisitValue::visit_str`], but more complex types like maps and sequences
/// will fallthrough to [`VisitValue::visit_any`].
///
/// If you're trying to serialize a value to a format like JSON, you can use either `serde`
/// or `sval` directly with the value. You don't need a visitor.
///
/// [`Value`'s documentation on serialization]: Value#serialization
pub trait VisitValue<'v> {
    /// Visit a `Value`.
    ///
    /// This is the only required method on `VisitValue` and acts as a fallback for any
    /// more specific methods that aren't overridden.
    /// The `Value` may be formatted using its `fmt::Debug` or `fmt::Display` implementation,
    /// or serialized using its `sval::Value` or `serde::Serialize` implementation.
    fn visit_any(&mut self, value: Value) -> Result<(), Error>;

    /// Visit an empty value.
    fn visit_null(&mut self) -> Result<(), Error> {
        self.visit_any(Value::null())
    }

    /// Visit an unsigned integer.
    fn visit_u64(&mut self, value: u64) -> Result<(), Error> {
        self.visit_any(value.into())
    }

    /// Visit a signed integer.
    fn visit_i64(&mut self, value: i64) -> Result<(), Error> {
        self.visit_any(value.into())
    }

    /// Visit a big unsigned integer.
    fn visit_u128(&mut self, value: u128) -> Result<(), Error> {
        self.visit_any((value).into())
    }

    /// Visit a big signed integer.
    fn visit_i128(&mut self, value: i128) -> Result<(), Error> {
        self.visit_any((value).into())
    }

    /// Visit a floating point.
    fn visit_f64(&mut self, value: f64) -> Result<(), Error> {
        self.visit_any(value.into())
    }

    /// Visit a boolean.
    fn visit_bool(&mut self, value: bool) -> Result<(), Error> {
        self.visit_any(value.into())
    }

    /// Visit a string.
    fn visit_str(&mut self, value: &str) -> Result<(), Error> {
        self.visit_any(value.into())
    }

    /// Visit a string.
    fn visit_borrowed_str(&mut self, value: &'v str) -> Result<(), Error> {
        self.visit_str(value)
    }

    /// Visit a Unicode character.
    fn visit_char(&mut self, value: char) -> Result<(), Error> {
        let mut b = [0; 4];
        self.visit_str(&*value.encode_utf8(&mut b))
    }

    /// Visit an error.
    #[cfg(feature = "kv_std")]
    fn visit_error(&mut self, err: &(dyn std::error::Error + 'static)) -> Result<(), Error> {
        self.visit_any(Value::from_dyn_error(err))
    }

    /// Visit an error.
    #[cfg(feature = "kv_std")]
    fn visit_borrowed_error(
        &mut self,
        err: &'v (dyn std::error::Error + 'static),
    ) -> Result<(), Error> {
        self.visit_any(Value::from_dyn_error(err))
    }
}

impl<'a, 'v, T: ?Sized> VisitValue<'v> for &'a mut T
where
    T: VisitValue<'v>,
{
    fn visit_any(&mut self, value: Value) -> Result<(), Error> {
        (**self).visit_any(value)
    }

    fn visit_null(&mut self) -> Result<(), Error> {
        (**self).visit_null()
    }

    fn visit_u64(&mut self, value: u64) -> Result<(), Error> {
        (**self).visit_u64(value)
    }

    fn visit_i64(&mut self, value: i64) -> Result<(), Error> {
        (**self).visit_i64(value)
    }

    fn visit_u128(&mut self, value: u128) -> Result<(), Error> {
        (**self).visit_u128(value)
    }

    fn visit_i128(&mut self, value: i128) -> Result<(), Error> {
        (**self).visit_i128(value)
    }

    fn visit_f64(&mut self, value: f64) -> Result<(), Error> {
        (**self).visit_f64(value)
    }

    fn visit_bool(&mut self, value: bool) -> Result<(), Error> {
        (**self).visit_bool(value)
    }

    fn visit_str(&mut self, value: &str) -> Result<(), Error> {
        (**self).visit_str(value)
    }

    fn visit_borrowed_str(&mut self, value: &'v str) -> Result<(), Error> {
        (**self).visit_borrowed_str(value)
    }

    fn visit_char(&mut self, value: char) -> Result<(), Error> {
        (**self).visit_char(value)
    }

    #[cfg(feature = "kv_std")]
    fn visit_error(&mut self, err: &(dyn std::error::Error + 'static)) -> Result<(), Error> {
        (**self).visit_error(err)
    }

    #[cfg(feature = "kv_std")]
    fn visit_borrowed_error(
        &mut self,
        err: &'v (dyn std::error::Error + 'static),
    ) -> Result<(), Error> {
        (**self).visit_borrowed_error(err)
    }
}

#[cfg(feature = "value-bag")]
pub(in crate::kv) mod inner {
    /**
    An implementation of `Value` based on a library called `value_bag`.

    `value_bag` was written specifically for use in `log`'s value, but was split out when it outgrew
    the codebase here. It's a general-purpose type-erasure library that handles mapping between
    more fully-featured serialization frameworks.
    */
    use super::*;

    pub use value_bag::ValueBag as Inner;

    pub use value_bag::Error;

    #[cfg(test)]
    pub use value_bag::test::TestToken as Token;

    pub fn visit<'v>(
        inner: &Inner<'v>,
        visitor: impl VisitValue<'v>,
    ) -> Result<(), crate::kv::Error> {
        struct InnerVisitValue<V>(V);

        impl<'v, V> value_bag::visit::Visit<'v> for InnerVisitValue<V>
        where
            V: VisitValue<'v>,
        {
            fn visit_any(&mut self, value: value_bag::ValueBag) -> Result<(), Error> {
                self.0
                    .visit_any(Value { inner: value })
                    .map_err(crate::kv::Error::into_value)
            }

            fn visit_empty(&mut self) -> Result<(), Error> {
                self.0.visit_null().map_err(crate::kv::Error::into_value)
            }

            fn visit_u64(&mut self, value: u64) -> Result<(), Error> {
                self.0
                    .visit_u64(value)
                    .map_err(crate::kv::Error::into_value)
            }

            fn visit_i64(&mut self, value: i64) -> Result<(), Error> {
                self.0
                    .visit_i64(value)
                    .map_err(crate::kv::Error::into_value)
            }

            fn visit_u128(&mut self, value: u128) -> Result<(), Error> {
                self.0
                    .visit_u128(value)
                    .map_err(crate::kv::Error::into_value)
            }

            fn visit_i128(&mut self, value: i128) -> Result<(), Error> {
                self.0
                    .visit_i128(value)
                    .map_err(crate::kv::Error::into_value)
            }

            fn visit_f64(&mut self, value: f64) -> Result<(), Error> {
                self.0
                    .visit_f64(value)
                    .map_err(crate::kv::Error::into_value)
            }

            fn visit_bool(&mut self, value: bool) -> Result<(), Error> {
                self.0
                    .visit_bool(value)
                    .map_err(crate::kv::Error::into_value)
            }

            fn visit_str(&mut self, value: &str) -> Result<(), Error> {
                self.0
                    .visit_str(value)
                    .map_err(crate::kv::Error::into_value)
            }

            fn visit_borrowed_str(&mut self, value: &'v str) -> Result<(), Error> {
                self.0
                    .visit_borrowed_str(value)
                    .map_err(crate::kv::Error::into_value)
            }

            fn visit_char(&mut self, value: char) -> Result<(), Error> {
                self.0
                    .visit_char(value)
                    .map_err(crate::kv::Error::into_value)
            }

            #[cfg(feature = "kv_std")]
            fn visit_error(
                &mut self,
                err: &(dyn std::error::Error + 'static),
            ) -> Result<(), Error> {
                self.0
                    .visit_error(err)
                    .map_err(crate::kv::Error::into_value)
            }

            #[cfg(feature = "kv_std")]
            fn visit_borrowed_error(
                &mut self,
                err: &'v (dyn std::error::Error + 'static),
            ) -> Result<(), Error> {
                self.0
                    .visit_borrowed_error(err)
                    .map_err(crate::kv::Error::into_value)
            }
        }

        inner
            .visit(&mut InnerVisitValue(visitor))
            .map_err(crate::kv::Error::from_value)
    }
}

#[cfg(not(feature = "value-bag"))]
pub(in crate::kv) mod inner {
    /**
    This is a dependency-free implementation of `Value` when there's no serialization frameworks involved.
    In these simple cases a more fully featured solution like `value_bag` isn't needed, so we avoid pulling it in.

    There are a few things here that need to remain consistent with the `value_bag`-based implementation:

    1. Conversions should always produce the same results. If a conversion here returns `Some`, then
       the same `value_bag`-based conversion must also. Of particular note here are floats to ints; they're
       based on the standard library's `TryInto` conversions, which need to be converted to `i32` or `u32`,
       and then to `f64`.
    2. VisitValues should always be called in the same way. If a particular type of value calls `visit_i64`,
       then the same `value_bag`-based visitor must also.
    */
    use super::*;

    #[derive(Clone)]
    pub enum Inner<'v> {
        None,
        Bool(bool),
        Str(&'v str),
        Char(char),
        I64(i64),
        U64(u64),
        F64(f64),
        I128(i128),
        U128(u128),
        Debug(&'v dyn fmt::Debug),
        Display(&'v dyn fmt::Display),
    }

    impl<'v> From<()> for Inner<'v> {
        fn from(_: ()) -> Self {
            Inner::None
        }
    }

    impl<'v> From<bool> for Inner<'v> {
        fn from(v: bool) -> Self {
            Inner::Bool(v)
        }
    }

    impl<'v> From<char> for Inner<'v> {
        fn from(v: char) -> Self {
            Inner::Char(v)
        }
    }

    impl<'v> From<f32> for Inner<'v> {
        fn from(v: f32) -> Self {
            Inner::F64(v as f64)
        }
    }

    impl<'v> From<f64> for Inner<'v> {
        fn from(v: f64) -> Self {
            Inner::F64(v)
        }
    }

    impl<'v> From<i8> for Inner<'v> {
        fn from(v: i8) -> Self {
            Inner::I64(v as i64)
        }
    }

    impl<'v> From<i16> for Inner<'v> {
        fn from(v: i16) -> Self {
            Inner::I64(v as i64)
        }
    }

    impl<'v> From<i32> for Inner<'v> {
        fn from(v: i32) -> Self {
            Inner::I64(v as i64)
        }
    }

    impl<'v> From<i64> for Inner<'v> {
        fn from(v: i64) -> Self {
            Inner::I64(v as i64)
        }
    }

    impl<'v> From<isize> for Inner<'v> {
        fn from(v: isize) -> Self {
            Inner::I64(v as i64)
        }
    }

    impl<'v> From<u8> for Inner<'v> {
        fn from(v: u8) -> Self {
            Inner::U64(v as u64)
        }
    }

    impl<'v> From<u16> for Inner<'v> {
        fn from(v: u16) -> Self {
            Inner::U64(v as u64)
        }
    }

    impl<'v> From<u32> for Inner<'v> {
        fn from(v: u32) -> Self {
            Inner::U64(v as u64)
        }
    }

    impl<'v> From<u64> for Inner<'v> {
        fn from(v: u64) -> Self {
            Inner::U64(v as u64)
        }
    }

    impl<'v> From<usize> for Inner<'v> {
        fn from(v: usize) -> Self {
            Inner::U64(v as u64)
        }
    }

    impl<'v> From<i128> for Inner<'v> {
        fn from(v: i128) -> Self {
            Inner::I128(v)
        }
    }

    impl<'v> From<u128> for Inner<'v> {
        fn from(v: u128) -> Self {
            Inner::U128(v)
        }
    }

    impl<'v> From<&'v str> for Inner<'v> {
        fn from(v: &'v str) -> Self {
            Inner::Str(v)
        }
    }

    impl<'v> fmt::Debug for Inner<'v> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                Inner::None => fmt::Debug::fmt(&None::<()>, f),
                Inner::Bool(v) => fmt::Debug::fmt(v, f),
                Inner::Str(v) => fmt::Debug::fmt(v, f),
                Inner::Char(v) => fmt::Debug::fmt(v, f),
                Inner::I64(v) => fmt::Debug::fmt(v, f),
                Inner::U64(v) => fmt::Debug::fmt(v, f),
                Inner::F64(v) => fmt::Debug::fmt(v, f),
                Inner::I128(v) => fmt::Debug::fmt(v, f),
                Inner::U128(v) => fmt::Debug::fmt(v, f),
                Inner::Debug(v) => fmt::Debug::fmt(v, f),
                Inner::Display(v) => fmt::Display::fmt(v, f),
            }
        }
    }

    impl<'v> fmt::Display for Inner<'v> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                Inner::None => fmt::Debug::fmt(&None::<()>, f),
                Inner::Bool(v) => fmt::Display::fmt(v, f),
                Inner::Str(v) => fmt::Display::fmt(v, f),
                Inner::Char(v) => fmt::Display::fmt(v, f),
                Inner::I64(v) => fmt::Display::fmt(v, f),
                Inner::U64(v) => fmt::Display::fmt(v, f),
                Inner::F64(v) => fmt::Display::fmt(v, f),
                Inner::I128(v) => fmt::Display::fmt(v, f),
                Inner::U128(v) => fmt::Display::fmt(v, f),
                Inner::Debug(v) => fmt::Debug::fmt(v, f),
                Inner::Display(v) => fmt::Display::fmt(v, f),
            }
        }
    }

    impl<'v> Inner<'v> {
        pub fn from_debug<T: fmt::Debug>(value: &'v T) -> Self {
            Inner::Debug(value)
        }

        pub fn from_display<T: fmt::Display>(value: &'v T) -> Self {
            Inner::Display(value)
        }

        pub fn from_dyn_debug(value: &'v dyn fmt::Debug) -> Self {
            Inner::Debug(value)
        }

        pub fn from_dyn_display(value: &'v dyn fmt::Display) -> Self {
            Inner::Display(value)
        }

        pub fn empty() -> Self {
            Inner::None
        }

        pub fn to_bool(&self) -> Option<bool> {
            match self {
                Inner::Bool(v) => Some(*v),
                _ => None,
            }
        }

        pub fn to_char(&self) -> Option<char> {
            match self {
                Inner::Char(v) => Some(*v),
                _ => None,
            }
        }

        pub fn to_f64(&self) -> Option<f64> {
            match self {
                Inner::F64(v) => Some(*v),
                Inner::I64(v) => {
                    let v: i32 = (*v).try_into().ok()?;
                    v.try_into().ok()
                }
                Inner::U64(v) => {
                    let v: u32 = (*v).try_into().ok()?;
                    v.try_into().ok()
                }
                Inner::I128(v) => {
                    let v: i32 = (*v).try_into().ok()?;
                    v.try_into().ok()
                }
                Inner::U128(v) => {
                    let v: u32 = (*v).try_into().ok()?;
                    v.try_into().ok()
                }
                _ => None,
            }
        }

        pub fn to_i64(&self) -> Option<i64> {
            match self {
                Inner::I64(v) => Some(*v),
                Inner::U64(v) => (*v).try_into().ok(),
                Inner::I128(v) => (*v).try_into().ok(),
                Inner::U128(v) => (*v).try_into().ok(),
                _ => None,
            }
        }

        pub fn to_u64(&self) -> Option<u64> {
            match self {
                Inner::U64(v) => Some(*v),
                Inner::I64(v) => (*v).try_into().ok(),
                Inner::I128(v) => (*v).try_into().ok(),
                Inner::U128(v) => (*v).try_into().ok(),
                _ => None,
            }
        }

        pub fn to_u128(&self) -> Option<u128> {
            match self {
                Inner::U128(v) => Some(*v),
                Inner::I64(v) => (*v).try_into().ok(),
                Inner::U64(v) => (*v).try_into().ok(),
                Inner::I128(v) => (*v).try_into().ok(),
                _ => None,
            }
        }

        pub fn to_i128(&self) -> Option<i128> {
            match self {
                Inner::I128(v) => Some(*v),
                Inner::I64(v) => (*v).try_into().ok(),
                Inner::U64(v) => (*v).try_into().ok(),
                Inner::U128(v) => (*v).try_into().ok(),
                _ => None,
            }
        }

        pub fn to_borrowed_str(&self) -> Option<&'v str> {
            match self {
                Inner::Str(v) => Some(v),
                _ => None,
            }
        }

        #[cfg(test)]
        pub fn to_test_token(&self) -> Token {
            match self {
                Inner::None => Token::None,
                Inner::Bool(v) => Token::Bool(*v),
                Inner::Str(v) => Token::Str(*v),
                Inner::Char(v) => Token::Char(*v),
                Inner::I64(v) => Token::I64(*v),
                Inner::U64(v) => Token::U64(*v),
                Inner::F64(v) => Token::F64(*v),
                Inner::I128(_) => unimplemented!(),
                Inner::U128(_) => unimplemented!(),
                Inner::Debug(_) => unimplemented!(),
                Inner::Display(_) => unimplemented!(),
            }
        }
    }

    #[cfg(test)]
    #[derive(Debug, PartialEq)]
    pub enum Token<'v> {
        None,
        Bool(bool),
        Char(char),
        Str(&'v str),
        F64(f64),
        I64(i64),
        U64(u64),
    }

    pub fn visit<'v>(
        inner: &Inner<'v>,
        mut visitor: impl VisitValue<'v>,
    ) -> Result<(), crate::kv::Error> {
        match inner {
            Inner::None => visitor.visit_null(),
            Inner::Bool(v) => visitor.visit_bool(*v),
            Inner::Str(v) => visitor.visit_borrowed_str(*v),
            Inner::Char(v) => visitor.visit_char(*v),
            Inner::I64(v) => visitor.visit_i64(*v),
            Inner::U64(v) => visitor.visit_u64(*v),
            Inner::F64(v) => visitor.visit_f64(*v),
            Inner::I128(v) => visitor.visit_i128(*v),
            Inner::U128(v) => visitor.visit_u128(*v),
            Inner::Debug(v) => visitor.visit_any(Value::from_dyn_debug(*v)),
            Inner::Display(v) => visitor.visit_any(Value::from_dyn_display(*v)),
        }
    }
}

impl<'v> Value<'v> {
    /// Get a value from a type implementing `std::fmt::Debug`.
    #[cfg(feature = "kv_unstable")]
    #[deprecated(note = "use `from_debug` instead")]
    pub fn capture_debug<T>(value: &'v T) -> Self
    where
        T: fmt::Debug + 'static,
    {
        Value::from_debug(value)
    }

    /// Get a value from a type implementing `std::fmt::Display`.
    #[cfg(feature = "kv_unstable")]
    #[deprecated(note = "use `from_display` instead")]
    pub fn capture_display<T>(value: &'v T) -> Self
    where
        T: fmt::Display + 'static,
    {
        Value::from_display(value)
    }

    /// Get a value from an error.
    #[cfg(feature = "kv_unstable_std")]
    #[deprecated(note = "use `from_dyn_error` instead")]
    pub fn capture_error<T>(err: &'v T) -> Self
    where
        T: std::error::Error + 'static,
    {
        Value::from_dyn_error(err)
    }

    /// Get a value from a type implementing `serde::Serialize`.
    #[cfg(feature = "kv_unstable_serde")]
    #[deprecated(note = "use `from_serde` instead")]
    pub fn capture_serde<T>(value: &'v T) -> Self
    where
        T: serde::Serialize + 'static,
    {
        Value::from_serde(value)
    }

    /// Get a value from a type implementing `sval::Value`.
    #[cfg(feature = "kv_unstable_sval")]
    #[deprecated(note = "use `from_sval` instead")]
    pub fn capture_sval<T>(value: &'v T) -> Self
    where
        T: sval::Value + 'static,
    {
        Value::from_sval(value)
    }

    /// Check whether this value can be downcast to `T`.
    #[cfg(feature = "kv_unstable")]
    #[deprecated(
        note = "downcasting has been removed; log an issue at https://github.com/rust-lang/log/issues if this is something you rely on"
    )]
    pub fn is<T: 'static>(&self) -> bool {
        false
    }

    /// Try downcast this value to `T`.
    #[cfg(feature = "kv_unstable")]
    #[deprecated(
        note = "downcasting has been removed; log an issue at https://github.com/rust-lang/log/issues if this is something you rely on"
    )]
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        None
    }
}

// NOTE: Deprecated; but aliases can't carry this attribute
#[cfg(feature = "kv_unstable")]
pub use VisitValue as Visit;

/// Get a value from a type implementing `std::fmt::Debug`.
#[cfg(feature = "kv_unstable")]
#[deprecated(note = "use the `key:? = value` macro syntax instead")]
#[macro_export]
macro_rules! as_debug {
    ($capture:expr) => {
        $crate::kv::Value::from_debug(&$capture)
    };
}

/// Get a value from a type implementing `std::fmt::Display`.
#[cfg(feature = "kv_unstable")]
#[deprecated(note = "use the `key:% = value` macro syntax instead")]
#[macro_export]
macro_rules! as_display {
    ($capture:expr) => {
        $crate::kv::Value::from_display(&$capture)
    };
}

/// Get a value from an error.
#[cfg(feature = "kv_unstable_std")]
#[deprecated(note = "use the `key:err = value` macro syntax instead")]
#[macro_export]
macro_rules! as_error {
    ($capture:expr) => {
        $crate::kv::Value::from_dyn_error(&$capture)
    };
}

#[cfg(feature = "kv_unstable_serde")]
#[deprecated(note = "use the `key:serde = value` macro syntax instead")]
/// Get a value from a type implementing `serde::Serialize`.
#[macro_export]
macro_rules! as_serde {
    ($capture:expr) => {
        $crate::kv::Value::from_serde(&$capture)
    };
}

/// Get a value from a type implementing `sval::Value`.
#[cfg(feature = "kv_unstable_sval")]
#[deprecated(note = "use the `key:sval = value` macro syntax instead")]
#[macro_export]
macro_rules! as_sval {
    ($capture:expr) => {
        $crate::kv::Value::from_sval(&$capture)
    };
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    impl<'v> Value<'v> {
        pub(crate) fn to_token(&self) -> inner::Token {
            self.inner.to_test_token()
        }
    }

    fn unsigned() -> impl Iterator<Item = Value<'static>> {
        vec![
            Value::from(8u8),
            Value::from(16u16),
            Value::from(32u32),
            Value::from(64u64),
            Value::from(1usize),
            Value::from(std::num::NonZeroU8::new(8).unwrap()),
            Value::from(std::num::NonZeroU16::new(16).unwrap()),
            Value::from(std::num::NonZeroU32::new(32).unwrap()),
            Value::from(std::num::NonZeroU64::new(64).unwrap()),
            Value::from(std::num::NonZeroUsize::new(1).unwrap()),
        ]
        .into_iter()
    }

    fn signed() -> impl Iterator<Item = Value<'static>> {
        vec![
            Value::from(-8i8),
            Value::from(-16i16),
            Value::from(-32i32),
            Value::from(-64i64),
            Value::from(-1isize),
            Value::from(std::num::NonZeroI8::new(-8).unwrap()),
            Value::from(std::num::NonZeroI16::new(-16).unwrap()),
            Value::from(std::num::NonZeroI32::new(-32).unwrap()),
            Value::from(std::num::NonZeroI64::new(-64).unwrap()),
            Value::from(std::num::NonZeroIsize::new(-1).unwrap()),
        ]
        .into_iter()
    }

    fn float() -> impl Iterator<Item = Value<'static>> {
        vec![Value::from(32.32f32), Value::from(64.64f64)].into_iter()
    }

    fn bool() -> impl Iterator<Item = Value<'static>> {
        vec![Value::from(true), Value::from(false)].into_iter()
    }

    fn str() -> impl Iterator<Item = Value<'static>> {
        vec![Value::from("a string"), Value::from("a loong string")].into_iter()
    }

    fn char() -> impl Iterator<Item = Value<'static>> {
        vec![Value::from('a'), Value::from('⛰')].into_iter()
    }

    #[test]
    fn test_to_value_display() {
        assert_eq!(42u64.to_value().to_string(), "42");
        assert_eq!(42i64.to_value().to_string(), "42");
        assert_eq!(42.01f64.to_value().to_string(), "42.01");
        assert_eq!(true.to_value().to_string(), "true");
        assert_eq!('a'.to_value().to_string(), "a");
        assert_eq!("a loong string".to_value().to_string(), "a loong string");
        assert_eq!(Some(true).to_value().to_string(), "true");
        assert_eq!(().to_value().to_string(), "None");
        assert_eq!(None::<bool>.to_value().to_string(), "None");
    }

    #[test]
    fn test_to_value_structured() {
        assert_eq!(42u64.to_value().to_token(), inner::Token::U64(42));
        assert_eq!(42i64.to_value().to_token(), inner::Token::I64(42));
        assert_eq!(42.01f64.to_value().to_token(), inner::Token::F64(42.01));
        assert_eq!(true.to_value().to_token(), inner::Token::Bool(true));
        assert_eq!('a'.to_value().to_token(), inner::Token::Char('a'));
        assert_eq!(
            "a loong string".to_value().to_token(),
            inner::Token::Str("a loong string".into())
        );
        assert_eq!(Some(true).to_value().to_token(), inner::Token::Bool(true));
        assert_eq!(().to_value().to_token(), inner::Token::None);
        assert_eq!(None::<bool>.to_value().to_token(), inner::Token::None);
    }

    #[test]
    fn test_to_number() {
        for v in unsigned() {
            assert!(v.to_u64().is_some());
            assert!(v.to_i64().is_some());
        }

        for v in signed() {
            assert!(v.to_i64().is_some());
        }

        for v in unsigned().chain(signed()).chain(float()) {
            assert!(v.to_f64().is_some());
        }

        for v in bool().chain(str()).chain(char()) {
            assert!(v.to_u64().is_none());
            assert!(v.to_i64().is_none());
            assert!(v.to_f64().is_none());
        }
    }

    #[test]
    fn test_to_float() {
        // Only integers from i32::MIN..=u32::MAX can be converted into floats
        assert!(Value::from(i32::MIN).to_f64().is_some());
        assert!(Value::from(u32::MAX).to_f64().is_some());

        assert!(Value::from((i32::MIN as i64) - 1).to_f64().is_none());
        assert!(Value::from((u32::MAX as u64) + 1).to_f64().is_none());
    }

    #[test]
    fn test_to_cow_str() {
        for v in str() {
            assert!(v.to_borrowed_str().is_some());

            #[cfg(feature = "kv_std")]
            assert!(v.to_cow_str().is_some());
        }

        let short_lived = String::from("short lived");
        let v = Value::from(&*short_lived);

        assert!(v.to_borrowed_str().is_some());

        #[cfg(feature = "kv_std")]
        assert!(v.to_cow_str().is_some());

        for v in unsigned().chain(signed()).chain(float()).chain(bool()) {
            assert!(v.to_borrowed_str().is_none());

            #[cfg(feature = "kv_std")]
            assert!(v.to_cow_str().is_none());
        }
    }

    #[test]
    fn test_to_bool() {
        for v in bool() {
            assert!(v.to_bool().is_some());
        }

        for v in unsigned()
            .chain(signed())
            .chain(float())
            .chain(str())
            .chain(char())
        {
            assert!(v.to_bool().is_none());
        }
    }

    #[test]
    fn test_to_char() {
        for v in char() {
            assert!(v.to_char().is_some());
        }

        for v in unsigned()
            .chain(signed())
            .chain(float())
            .chain(str())
            .chain(bool())
        {
            assert!(v.to_char().is_none());
        }
    }

    #[test]
    fn test_visit_integer() {
        struct Extract(Option<u64>);

        impl<'v> VisitValue<'v> for Extract {
            fn visit_any(&mut self, value: Value) -> Result<(), Error> {
                unimplemented!("unexpected value: {value:?}")
            }

            fn visit_u64(&mut self, value: u64) -> Result<(), Error> {
                self.0 = Some(value);

                Ok(())
            }
        }

        let mut extract = Extract(None);
        Value::from(42u64).visit(&mut extract).unwrap();

        assert_eq!(Some(42), extract.0);
    }

    #[test]
    fn test_visit_borrowed_str() {
        struct Extract<'v>(Option<&'v str>);

        impl<'v> VisitValue<'v> for Extract<'v> {
            fn visit_any(&mut self, value: Value) -> Result<(), Error> {
                unimplemented!("unexpected value: {value:?}")
            }

            fn visit_borrowed_str(&mut self, value: &'v str) -> Result<(), Error> {
                self.0 = Some(value);

                Ok(())
            }
        }

        let mut extract = Extract(None);

        let short_lived = String::from("A short-lived string");
        Value::from(&*short_lived).visit(&mut extract).unwrap();

        assert_eq!(Some("A short-lived string"), extract.0);
    }
}
