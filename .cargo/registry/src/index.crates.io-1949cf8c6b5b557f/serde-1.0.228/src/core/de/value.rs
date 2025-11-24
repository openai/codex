//! Building blocks for deserializing basic values using the `IntoDeserializer`
//! trait.
//!
//! ```edition2021
//! use serde::de::{value, Deserialize, IntoDeserializer};
//! use serde_derive::Deserialize;
//! use std::str::FromStr;
//!
//! #[derive(Deserialize)]
//! enum Setting {
//!     On,
//!     Off,
//! }
//!
//! impl FromStr for Setting {
//!     type Err = value::Error;
//!
//!     fn from_str(s: &str) -> Result<Self, Self::Err> {
//!         Self::deserialize(s.into_deserializer())
//!     }
//! }
//! ```

use crate::lib::*;

use self::private::{First, Second};
use crate::de::{self, Deserializer, Expected, IntoDeserializer, SeqAccess, Visitor};
use crate::private::size_hint;
use crate::ser;

////////////////////////////////////////////////////////////////////////////////

// For structs that contain a PhantomData. We do not want the trait
// bound `E: Clone` inferred by derive(Clone).
macro_rules! impl_copy_clone {
    ($ty:ident $(<$lifetime:tt>)*) => {
        impl<$($lifetime,)* E> Copy for $ty<$($lifetime,)* E> {}

        impl<$($lifetime,)* E> Clone for $ty<$($lifetime,)* E> {
            fn clone(&self) -> Self {
                *self
            }
        }
    };
}

////////////////////////////////////////////////////////////////////////////////

/// A minimal representation of all possible errors that can occur using the
/// `IntoDeserializer` trait.
#[derive(Clone, PartialEq)]
pub struct Error {
    err: ErrorImpl,
}

#[cfg(any(feature = "std", feature = "alloc"))]
type ErrorImpl = Box<str>;
#[cfg(not(any(feature = "std", feature = "alloc")))]
type ErrorImpl = ();

impl de::Error for Error {
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cold]
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Error {
            err: msg.to_string().into_boxed_str(),
        }
    }

    #[cfg(not(any(feature = "std", feature = "alloc")))]
    #[cold]
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        let _ = msg;
        Error { err: () }
    }
}

impl ser::Error for Error {
    #[cold]
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        de::Error::custom(msg)
    }
}

impl Display for Error {
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&self.err)
    }

    #[cfg(not(any(feature = "std", feature = "alloc")))]
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Serde deserialization error")
    }
}

impl Debug for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let mut debug = formatter.debug_tuple("Error");
        #[cfg(any(feature = "std", feature = "alloc"))]
        debug.field(&self.err);
        debug.finish()
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl error::Error for Error {
    fn description(&self) -> &str {
        &self.err
    }
}

////////////////////////////////////////////////////////////////////////////////

impl<'de, E> IntoDeserializer<'de, E> for ()
where
    E: de::Error,
{
    type Deserializer = UnitDeserializer<E>;

    fn into_deserializer(self) -> UnitDeserializer<E> {
        UnitDeserializer::new()
    }
}

/// A deserializer holding a `()`.
pub struct UnitDeserializer<E> {
    marker: PhantomData<E>,
}

impl_copy_clone!(UnitDeserializer);

impl<E> UnitDeserializer<E> {
    #[allow(missing_docs)]
    pub fn new() -> Self {
        UnitDeserializer {
            marker: PhantomData,
        }
    }
}

impl<'de, E> de::Deserializer<'de> for UnitDeserializer<E>
where
    E: de::Error,
{
    type Error = E;

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple tuple_struct
        map struct enum identifier ignored_any
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_none()
    }
}

impl<'de, E> IntoDeserializer<'de, E> for UnitDeserializer<E>
where
    E: de::Error,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

impl<E> Debug for UnitDeserializer<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.debug_struct("UnitDeserializer").finish()
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A deserializer that cannot be instantiated.
#[cfg(feature = "unstable")]
#[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
pub struct NeverDeserializer<E> {
    never: !,
    marker: PhantomData<E>,
}

#[cfg(feature = "unstable")]
#[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
impl<'de, E> IntoDeserializer<'de, E> for !
where
    E: de::Error,
{
    type Deserializer = NeverDeserializer<E>;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

#[cfg(feature = "unstable")]
impl<'de, E> de::Deserializer<'de> for NeverDeserializer<E>
where
    E: de::Error,
{
    type Error = E;

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.never
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

#[cfg(feature = "unstable")]
impl<'de, E> IntoDeserializer<'de, E> for NeverDeserializer<E>
where
    E: de::Error,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

////////////////////////////////////////////////////////////////////////////////

macro_rules! primitive_deserializer {
    ($ty:ty, $doc:tt, $name:ident, $method:ident $($cast:tt)*) => {
        #[doc = "A deserializer holding"]
        #[doc = $doc]
        pub struct $name<E> {
            value: $ty,
            marker: PhantomData<E>
        }

        impl_copy_clone!($name);

        impl<'de, E> IntoDeserializer<'de, E> for $ty
        where
            E: de::Error,
        {
            type Deserializer = $name<E>;

            fn into_deserializer(self) -> $name<E> {
                $name::new(self)
            }
        }

        impl<E> $name<E> {
            #[allow(missing_docs)]
            pub fn new(value: $ty) -> Self {
                $name {
                    value,
                    marker: PhantomData,
                }
            }
        }

        impl<'de, E> de::Deserializer<'de> for $name<E>
        where
            E: de::Error,
        {
            type Error = E;

            forward_to_deserialize_any! {
                bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str
                string bytes byte_buf option unit unit_struct newtype_struct seq
                tuple tuple_struct map struct enum identifier ignored_any
            }

            fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: de::Visitor<'de>,
            {
                visitor.$method(self.value $($cast)*)
            }
        }

        impl<'de, E> IntoDeserializer<'de, E> for $name<E>
        where
            E: de::Error,
        {
            type Deserializer = Self;

            fn into_deserializer(self) -> Self {
                self
            }
        }

        impl<E> Debug for $name<E> {
            fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($name))
                    .field("value", &self.value)
                    .finish()
            }
        }
    }
}

primitive_deserializer!(bool, "a `bool`.", BoolDeserializer, visit_bool);
primitive_deserializer!(i8, "an `i8`.", I8Deserializer, visit_i8);
primitive_deserializer!(i16, "an `i16`.", I16Deserializer, visit_i16);
primitive_deserializer!(i32, "an `i32`.", I32Deserializer, visit_i32);
primitive_deserializer!(i64, "an `i64`.", I64Deserializer, visit_i64);
primitive_deserializer!(i128, "an `i128`.", I128Deserializer, visit_i128);
primitive_deserializer!(isize, "an `isize`.", IsizeDeserializer, visit_i64 as i64);
primitive_deserializer!(u8, "a `u8`.", U8Deserializer, visit_u8);
primitive_deserializer!(u16, "a `u16`.", U16Deserializer, visit_u16);
primitive_deserializer!(u64, "a `u64`.", U64Deserializer, visit_u64);
primitive_deserializer!(u128, "a `u128`.", U128Deserializer, visit_u128);
primitive_deserializer!(usize, "a `usize`.", UsizeDeserializer, visit_u64 as u64);
primitive_deserializer!(f32, "an `f32`.", F32Deserializer, visit_f32);
primitive_deserializer!(f64, "an `f64`.", F64Deserializer, visit_f64);
primitive_deserializer!(char, "a `char`.", CharDeserializer, visit_char);

/// A deserializer holding a `u32`.
pub struct U32Deserializer<E> {
    value: u32,
    marker: PhantomData<E>,
}

impl_copy_clone!(U32Deserializer);

impl<'de, E> IntoDeserializer<'de, E> for u32
where
    E: de::Error,
{
    type Deserializer = U32Deserializer<E>;

    fn into_deserializer(self) -> U32Deserializer<E> {
        U32Deserializer::new(self)
    }
}

impl<E> U32Deserializer<E> {
    #[allow(missing_docs)]
    pub fn new(value: u32) -> Self {
        U32Deserializer {
            value,
            marker: PhantomData,
        }
    }
}

impl<'de, E> de::Deserializer<'de> for U32Deserializer<E>
where
    E: de::Error,
{
    type Error = E;

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_u32(self.value)
    }

    fn deserialize_enum<V>(
        self,
        name: &str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let _ = name;
        let _ = variants;
        visitor.visit_enum(self)
    }
}

impl<'de, E> IntoDeserializer<'de, E> for U32Deserializer<E>
where
    E: de::Error,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

impl<'de, E> de::EnumAccess<'de> for U32Deserializer<E>
where
    E: de::Error,
{
    type Error = E;
    type Variant = private::UnitOnly<E>;

    fn variant_seed<T>(self, seed: T) -> Result<(T::Value, Self::Variant), Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self).map(private::unit_only)
    }
}

impl<E> Debug for U32Deserializer<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("U32Deserializer")
            .field("value", &self.value)
            .finish()
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A deserializer holding a `&str`.
pub struct StrDeserializer<'a, E> {
    value: &'a str,
    marker: PhantomData<E>,
}

impl_copy_clone!(StrDeserializer<'de>);

impl<'de, 'a, E> IntoDeserializer<'de, E> for &'a str
where
    E: de::Error,
{
    type Deserializer = StrDeserializer<'a, E>;

    fn into_deserializer(self) -> StrDeserializer<'a, E> {
        StrDeserializer::new(self)
    }
}

impl<'a, E> StrDeserializer<'a, E> {
    #[allow(missing_docs)]
    pub fn new(value: &'a str) -> Self {
        StrDeserializer {
            value,
            marker: PhantomData,
        }
    }
}

impl<'de, 'a, E> de::Deserializer<'de> for StrDeserializer<'a, E>
where
    E: de::Error,
{
    type Error = E;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_str(self.value)
    }

    fn deserialize_enum<V>(
        self,
        name: &str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let _ = name;
        let _ = variants;
        visitor.visit_enum(self)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }
}

impl<'de, 'a, E> IntoDeserializer<'de, E> for StrDeserializer<'a, E>
where
    E: de::Error,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

impl<'de, 'a, E> de::EnumAccess<'de> for StrDeserializer<'a, E>
where
    E: de::Error,
{
    type Error = E;
    type Variant = private::UnitOnly<E>;

    fn variant_seed<T>(self, seed: T) -> Result<(T::Value, Self::Variant), Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self).map(private::unit_only)
    }
}

impl<'a, E> Debug for StrDeserializer<'a, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("StrDeserializer")
            .field("value", &self.value)
            .finish()
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A deserializer holding a `&str` with a lifetime tied to another
/// deserializer.
pub struct BorrowedStrDeserializer<'de, E> {
    value: &'de str,
    marker: PhantomData<E>,
}

impl_copy_clone!(BorrowedStrDeserializer<'de>);

impl<'de, E> BorrowedStrDeserializer<'de, E> {
    /// Create a new borrowed deserializer from the given string.
    pub fn new(value: &'de str) -> BorrowedStrDeserializer<'de, E> {
        BorrowedStrDeserializer {
            value,
            marker: PhantomData,
        }
    }
}

impl<'de, E> de::Deserializer<'de> for BorrowedStrDeserializer<'de, E>
where
    E: de::Error,
{
    type Error = E;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.value)
    }

    fn deserialize_enum<V>(
        self,
        name: &str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let _ = name;
        let _ = variants;
        visitor.visit_enum(self)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }
}

impl<'de, E> IntoDeserializer<'de, E> for BorrowedStrDeserializer<'de, E>
where
    E: de::Error,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

impl<'de, E> de::EnumAccess<'de> for BorrowedStrDeserializer<'de, E>
where
    E: de::Error,
{
    type Error = E;
    type Variant = private::UnitOnly<E>;

    fn variant_seed<T>(self, seed: T) -> Result<(T::Value, Self::Variant), Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self).map(private::unit_only)
    }
}

impl<'de, E> Debug for BorrowedStrDeserializer<'de, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("BorrowedStrDeserializer")
            .field("value", &self.value)
            .finish()
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A deserializer holding a `String`.
#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub struct StringDeserializer<E> {
    value: String,
    marker: PhantomData<E>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<E> Clone for StringDeserializer<E> {
    fn clone(&self) -> Self {
        StringDeserializer {
            value: self.value.clone(),
            marker: PhantomData,
        }
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
impl<'de, E> IntoDeserializer<'de, E> for String
where
    E: de::Error,
{
    type Deserializer = StringDeserializer<E>;

    fn into_deserializer(self) -> StringDeserializer<E> {
        StringDeserializer::new(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<E> StringDeserializer<E> {
    #[allow(missing_docs)]
    pub fn new(value: String) -> Self {
        StringDeserializer {
            value,
            marker: PhantomData,
        }
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<'de, E> de::Deserializer<'de> for StringDeserializer<E>
where
    E: de::Error,
{
    type Error = E;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_string(self.value)
    }

    fn deserialize_enum<V>(
        self,
        name: &str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let _ = name;
        let _ = variants;
        visitor.visit_enum(self)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<'de, E> IntoDeserializer<'de, E> for StringDeserializer<E>
where
    E: de::Error,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<'de, E> de::EnumAccess<'de> for StringDeserializer<E>
where
    E: de::Error,
{
    type Error = E;
    type Variant = private::UnitOnly<E>;

    fn variant_seed<T>(self, seed: T) -> Result<(T::Value, Self::Variant), Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self).map(private::unit_only)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<E> Debug for StringDeserializer<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("StringDeserializer")
            .field("value", &self.value)
            .finish()
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A deserializer holding a `Cow<str>`.
#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
pub struct CowStrDeserializer<'a, E> {
    value: Cow<'a, str>,
    marker: PhantomData<E>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<'a, E> Clone for CowStrDeserializer<'a, E> {
    fn clone(&self) -> Self {
        CowStrDeserializer {
            value: self.value.clone(),
            marker: PhantomData,
        }
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
impl<'de, 'a, E> IntoDeserializer<'de, E> for Cow<'a, str>
where
    E: de::Error,
{
    type Deserializer = CowStrDeserializer<'a, E>;

    fn into_deserializer(self) -> CowStrDeserializer<'a, E> {
        CowStrDeserializer::new(self)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<'a, E> CowStrDeserializer<'a, E> {
    #[allow(missing_docs)]
    pub fn new(value: Cow<'a, str>) -> Self {
        CowStrDeserializer {
            value,
            marker: PhantomData,
        }
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<'de, 'a, E> de::Deserializer<'de> for CowStrDeserializer<'a, E>
where
    E: de::Error,
{
    type Error = E;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self.value {
            Cow::Borrowed(string) => visitor.visit_str(string),
            Cow::Owned(string) => visitor.visit_string(string),
        }
    }

    fn deserialize_enum<V>(
        self,
        name: &str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let _ = name;
        let _ = variants;
        visitor.visit_enum(self)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<'de, 'a, E> IntoDeserializer<'de, E> for CowStrDeserializer<'a, E>
where
    E: de::Error,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<'de, 'a, E> de::EnumAccess<'de> for CowStrDeserializer<'a, E>
where
    E: de::Error,
{
    type Error = E;
    type Variant = private::UnitOnly<E>;

    fn variant_seed<T>(self, seed: T) -> Result<(T::Value, Self::Variant), Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self).map(private::unit_only)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<'a, E> Debug for CowStrDeserializer<'a, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("CowStrDeserializer")
            .field("value", &self.value)
            .finish()
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A deserializer holding a `&[u8]`. Always calls [`Visitor::visit_bytes`].
pub struct BytesDeserializer<'a, E> {
    value: &'a [u8],
    marker: PhantomData<E>,
}

impl<'a, E> BytesDeserializer<'a, E> {
    /// Create a new deserializer from the given bytes.
    pub fn new(value: &'a [u8]) -> Self {
        BytesDeserializer {
            value,
            marker: PhantomData,
        }
    }
}

impl_copy_clone!(BytesDeserializer<'a>);

impl<'de, 'a, E> IntoDeserializer<'de, E> for &'a [u8]
where
    E: de::Error,
{
    type Deserializer = BytesDeserializer<'a, E>;

    fn into_deserializer(self) -> BytesDeserializer<'a, E> {
        BytesDeserializer::new(self)
    }
}

impl<'de, 'a, E> Deserializer<'de> for BytesDeserializer<'a, E>
where
    E: de::Error,
{
    type Error = E;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bytes(self.value)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de, 'a, E> IntoDeserializer<'de, E> for BytesDeserializer<'a, E>
where
    E: de::Error,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

impl<'a, E> Debug for BytesDeserializer<'a, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("BytesDeserializer")
            .field("value", &self.value)
            .finish()
    }
}

/// A deserializer holding a `&[u8]` with a lifetime tied to another
/// deserializer. Always calls [`Visitor::visit_borrowed_bytes`].
pub struct BorrowedBytesDeserializer<'de, E> {
    value: &'de [u8],
    marker: PhantomData<E>,
}

impl<'de, E> BorrowedBytesDeserializer<'de, E> {
    /// Create a new borrowed deserializer from the given borrowed bytes.
    pub fn new(value: &'de [u8]) -> Self {
        BorrowedBytesDeserializer {
            value,
            marker: PhantomData,
        }
    }
}

impl_copy_clone!(BorrowedBytesDeserializer<'de>);

impl<'de, E> Deserializer<'de> for BorrowedBytesDeserializer<'de, E>
where
    E: de::Error,
{
    type Error = E;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_bytes(self.value)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de, E> IntoDeserializer<'de, E> for BorrowedBytesDeserializer<'de, E>
where
    E: de::Error,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

impl<'de, E> Debug for BorrowedBytesDeserializer<'de, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("BorrowedBytesDeserializer")
            .field("value", &self.value)
            .finish()
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A deserializer that iterates over a sequence.
#[derive(Clone)]
pub struct SeqDeserializer<I, E> {
    iter: iter::Fuse<I>,
    count: usize,
    marker: PhantomData<E>,
}

impl<I, E> SeqDeserializer<I, E>
where
    I: Iterator,
{
    /// Construct a new `SeqDeserializer<I, E>`.
    pub fn new(iter: I) -> Self {
        SeqDeserializer {
            iter: iter.fuse(),
            count: 0,
            marker: PhantomData,
        }
    }
}

impl<I, E> SeqDeserializer<I, E>
where
    I: Iterator,
    E: de::Error,
{
    /// Check for remaining elements after passing a `SeqDeserializer` to
    /// `Visitor::visit_seq`.
    pub fn end(self) -> Result<(), E> {
        let remaining = self.iter.count();
        if remaining == 0 {
            Ok(())
        } else {
            // First argument is the number of elements in the data, second
            // argument is the number of elements expected by the Deserialize.
            Err(de::Error::invalid_length(
                self.count + remaining,
                &ExpectedInSeq(self.count),
            ))
        }
    }
}

impl<'de, I, T, E> de::Deserializer<'de> for SeqDeserializer<I, E>
where
    I: Iterator<Item = T>,
    T: IntoDeserializer<'de, E>,
    E: de::Error,
{
    type Error = E;

    fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let v = tri!(visitor.visit_seq(&mut self));
        tri!(self.end());
        Ok(v)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de, I, T, E> IntoDeserializer<'de, E> for SeqDeserializer<I, E>
where
    I: Iterator<Item = T>,
    T: IntoDeserializer<'de, E>,
    E: de::Error,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

impl<'de, I, T, E> de::SeqAccess<'de> for SeqDeserializer<I, E>
where
    I: Iterator<Item = T>,
    T: IntoDeserializer<'de, E>,
    E: de::Error,
{
    type Error = E;

    fn next_element_seed<V>(&mut self, seed: V) -> Result<Option<V::Value>, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(value) => {
                self.count += 1;
                seed.deserialize(value.into_deserializer()).map(Some)
            }
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        size_hint::from_bounds(&self.iter)
    }
}

struct ExpectedInSeq(usize);

impl Expected for ExpectedInSeq {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        if self.0 == 1 {
            formatter.write_str("1 element in sequence")
        } else {
            write!(formatter, "{} elements in sequence", self.0)
        }
    }
}

impl<I, E> Debug for SeqDeserializer<I, E>
where
    I: Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("SeqDeserializer")
            .field("iter", &self.iter)
            .field("count", &self.count)
            .finish()
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
impl<'de, T, E> IntoDeserializer<'de, E> for Vec<T>
where
    T: IntoDeserializer<'de, E>,
    E: de::Error,
{
    type Deserializer = SeqDeserializer<<Self as IntoIterator>::IntoIter, E>;

    fn into_deserializer(self) -> Self::Deserializer {
        SeqDeserializer::new(self.into_iter())
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
impl<'de, T, E> IntoDeserializer<'de, E> for BTreeSet<T>
where
    T: IntoDeserializer<'de, E> + Eq + Ord,
    E: de::Error,
{
    type Deserializer = SeqDeserializer<<Self as IntoIterator>::IntoIter, E>;

    fn into_deserializer(self) -> Self::Deserializer {
        SeqDeserializer::new(self.into_iter())
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<'de, T, S, E> IntoDeserializer<'de, E> for HashSet<T, S>
where
    T: IntoDeserializer<'de, E> + Eq + Hash,
    S: BuildHasher,
    E: de::Error,
{
    type Deserializer = SeqDeserializer<<Self as IntoIterator>::IntoIter, E>;

    fn into_deserializer(self) -> Self::Deserializer {
        SeqDeserializer::new(self.into_iter())
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A deserializer holding a `SeqAccess`.
#[derive(Clone, Debug)]
pub struct SeqAccessDeserializer<A> {
    seq: A,
}

impl<A> SeqAccessDeserializer<A> {
    /// Construct a new `SeqAccessDeserializer<A>`.
    pub fn new(seq: A) -> Self {
        SeqAccessDeserializer { seq }
    }
}

impl<'de, A> de::Deserializer<'de> for SeqAccessDeserializer<A>
where
    A: de::SeqAccess<'de>,
{
    type Error = A::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(self.seq)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de, A> IntoDeserializer<'de, A::Error> for SeqAccessDeserializer<A>
where
    A: de::SeqAccess<'de>,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A deserializer that iterates over a map.
pub struct MapDeserializer<'de, I, E>
where
    I: Iterator,
    I::Item: private::Pair,
{
    iter: iter::Fuse<I>,
    value: Option<Second<I::Item>>,
    count: usize,
    lifetime: PhantomData<&'de ()>,
    error: PhantomData<E>,
}

impl<'de, I, E> MapDeserializer<'de, I, E>
where
    I: Iterator,
    I::Item: private::Pair,
{
    /// Construct a new `MapDeserializer<I, E>`.
    pub fn new(iter: I) -> Self {
        MapDeserializer {
            iter: iter.fuse(),
            value: None,
            count: 0,
            lifetime: PhantomData,
            error: PhantomData,
        }
    }
}

impl<'de, I, E> MapDeserializer<'de, I, E>
where
    I: Iterator,
    I::Item: private::Pair,
    E: de::Error,
{
    /// Check for remaining elements after passing a `MapDeserializer` to
    /// `Visitor::visit_map`.
    pub fn end(self) -> Result<(), E> {
        let remaining = self.iter.count();
        if remaining == 0 {
            Ok(())
        } else {
            // First argument is the number of elements in the data, second
            // argument is the number of elements expected by the Deserialize.
            Err(de::Error::invalid_length(
                self.count + remaining,
                &ExpectedInMap(self.count),
            ))
        }
    }
}

impl<'de, I, E> MapDeserializer<'de, I, E>
where
    I: Iterator,
    I::Item: private::Pair,
{
    fn next_pair(&mut self) -> Option<(First<I::Item>, Second<I::Item>)> {
        match self.iter.next() {
            Some(kv) => {
                self.count += 1;
                Some(private::Pair::split(kv))
            }
            None => None,
        }
    }
}

impl<'de, I, E> de::Deserializer<'de> for MapDeserializer<'de, I, E>
where
    I: Iterator,
    I::Item: private::Pair,
    First<I::Item>: IntoDeserializer<'de, E>,
    Second<I::Item>: IntoDeserializer<'de, E>,
    E: de::Error,
{
    type Error = E;

    fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let value = tri!(visitor.visit_map(&mut self));
        tri!(self.end());
        Ok(value)
    }

    fn deserialize_seq<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let value = tri!(visitor.visit_seq(&mut self));
        tri!(self.end());
        Ok(value)
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let _ = len;
        self.deserialize_seq(visitor)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct tuple_struct map
        struct enum identifier ignored_any
    }
}

impl<'de, I, E> IntoDeserializer<'de, E> for MapDeserializer<'de, I, E>
where
    I: Iterator,
    I::Item: private::Pair,
    First<I::Item>: IntoDeserializer<'de, E>,
    Second<I::Item>: IntoDeserializer<'de, E>,
    E: de::Error,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

impl<'de, I, E> de::MapAccess<'de> for MapDeserializer<'de, I, E>
where
    I: Iterator,
    I::Item: private::Pair,
    First<I::Item>: IntoDeserializer<'de, E>,
    Second<I::Item>: IntoDeserializer<'de, E>,
    E: de::Error,
{
    type Error = E;

    fn next_key_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.next_pair() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(key.into_deserializer()).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<T>(&mut self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        let value = self.value.take();
        // Panic because this indicates a bug in the program rather than an
        // expected failure.
        let value = value.expect("MapAccess::next_value called before next_key");
        seed.deserialize(value.into_deserializer())
    }

    fn next_entry_seed<TK, TV>(
        &mut self,
        kseed: TK,
        vseed: TV,
    ) -> Result<Option<(TK::Value, TV::Value)>, Self::Error>
    where
        TK: de::DeserializeSeed<'de>,
        TV: de::DeserializeSeed<'de>,
    {
        match self.next_pair() {
            Some((key, value)) => {
                let key = tri!(kseed.deserialize(key.into_deserializer()));
                let value = tri!(vseed.deserialize(value.into_deserializer()));
                Ok(Some((key, value)))
            }
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        size_hint::from_bounds(&self.iter)
    }
}

impl<'de, I, E> de::SeqAccess<'de> for MapDeserializer<'de, I, E>
where
    I: Iterator,
    I::Item: private::Pair,
    First<I::Item>: IntoDeserializer<'de, E>,
    Second<I::Item>: IntoDeserializer<'de, E>,
    E: de::Error,
{
    type Error = E;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.next_pair() {
            Some((k, v)) => {
                let de = PairDeserializer(k, v, PhantomData);
                seed.deserialize(de).map(Some)
            }
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        size_hint::from_bounds(&self.iter)
    }
}

// Cannot #[derive(Clone)] because of the bound `Second<I::Item>: Clone`.
impl<'de, I, E> Clone for MapDeserializer<'de, I, E>
where
    I: Iterator + Clone,
    I::Item: private::Pair,
    Second<I::Item>: Clone,
{
    fn clone(&self) -> Self {
        MapDeserializer {
            iter: self.iter.clone(),
            value: self.value.clone(),
            count: self.count,
            lifetime: self.lifetime,
            error: self.error,
        }
    }
}

impl<'de, I, E> Debug for MapDeserializer<'de, I, E>
where
    I: Iterator + Debug,
    I::Item: private::Pair,
    Second<I::Item>: Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("MapDeserializer")
            .field("iter", &self.iter)
            .field("value", &self.value)
            .field("count", &self.count)
            .finish()
    }
}

// Used in the `impl SeqAccess for MapDeserializer` to visit the map as a
// sequence of pairs.
struct PairDeserializer<A, B, E>(A, B, PhantomData<E>);

impl<'de, A, B, E> de::Deserializer<'de> for PairDeserializer<A, B, E>
where
    A: IntoDeserializer<'de, E>,
    B: IntoDeserializer<'de, E>,
    E: de::Error,
{
    type Error = E;

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct tuple_struct map
        struct enum identifier ignored_any
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let mut pair_visitor = PairVisitor(Some(self.0), Some(self.1), PhantomData);
        let pair = tri!(visitor.visit_seq(&mut pair_visitor));
        if pair_visitor.1.is_none() {
            Ok(pair)
        } else {
            let remaining = pair_visitor.size_hint().unwrap();
            // First argument is the number of elements in the data, second
            // argument is the number of elements expected by the Deserialize.
            Err(de::Error::invalid_length(2, &ExpectedInSeq(2 - remaining)))
        }
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        if len == 2 {
            self.deserialize_seq(visitor)
        } else {
            // First argument is the number of elements in the data, second
            // argument is the number of elements expected by the Deserialize.
            Err(de::Error::invalid_length(2, &ExpectedInSeq(len)))
        }
    }
}

struct PairVisitor<A, B, E>(Option<A>, Option<B>, PhantomData<E>);

impl<'de, A, B, E> de::SeqAccess<'de> for PairVisitor<A, B, E>
where
    A: IntoDeserializer<'de, E>,
    B: IntoDeserializer<'de, E>,
    E: de::Error,
{
    type Error = E;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        if let Some(k) = self.0.take() {
            seed.deserialize(k.into_deserializer()).map(Some)
        } else if let Some(v) = self.1.take() {
            seed.deserialize(v.into_deserializer()).map(Some)
        } else {
            Ok(None)
        }
    }

    fn size_hint(&self) -> Option<usize> {
        if self.0.is_some() {
            Some(2)
        } else if self.1.is_some() {
            Some(1)
        } else {
            Some(0)
        }
    }
}

struct ExpectedInMap(usize);

impl Expected for ExpectedInMap {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        if self.0 == 1 {
            formatter.write_str("1 element in map")
        } else {
            write!(formatter, "{} elements in map", self.0)
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
impl<'de, K, V, E> IntoDeserializer<'de, E> for BTreeMap<K, V>
where
    K: IntoDeserializer<'de, E> + Eq + Ord,
    V: IntoDeserializer<'de, E>,
    E: de::Error,
{
    type Deserializer = MapDeserializer<'de, <Self as IntoIterator>::IntoIter, E>;

    fn into_deserializer(self) -> Self::Deserializer {
        MapDeserializer::new(self.into_iter())
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<'de, K, V, S, E> IntoDeserializer<'de, E> for HashMap<K, V, S>
where
    K: IntoDeserializer<'de, E> + Eq + Hash,
    V: IntoDeserializer<'de, E>,
    S: BuildHasher,
    E: de::Error,
{
    type Deserializer = MapDeserializer<'de, <Self as IntoIterator>::IntoIter, E>;

    fn into_deserializer(self) -> Self::Deserializer {
        MapDeserializer::new(self.into_iter())
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A deserializer holding a `MapAccess`.
#[derive(Clone, Debug)]
pub struct MapAccessDeserializer<A> {
    map: A,
}

impl<A> MapAccessDeserializer<A> {
    /// Construct a new `MapAccessDeserializer<A>`.
    pub fn new(map: A) -> Self {
        MapAccessDeserializer { map }
    }
}

impl<'de, A> de::Deserializer<'de> for MapAccessDeserializer<A>
where
    A: de::MapAccess<'de>,
{
    type Error = A::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_map(self.map)
    }

    fn deserialize_enum<V>(
        self,
        _name: &str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_enum(self)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }
}

impl<'de, A> IntoDeserializer<'de, A::Error> for MapAccessDeserializer<A>
where
    A: de::MapAccess<'de>,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

impl<'de, A> de::EnumAccess<'de> for MapAccessDeserializer<A>
where
    A: de::MapAccess<'de>,
{
    type Error = A::Error;
    type Variant = private::MapAsEnum<A>;

    fn variant_seed<T>(mut self, seed: T) -> Result<(T::Value, Self::Variant), Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match tri!(self.map.next_key_seed(seed)) {
            Some(key) => Ok((key, private::map_as_enum(self.map))),
            None => Err(de::Error::invalid_type(de::Unexpected::Map, &"enum")),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A deserializer holding an `EnumAccess`.
#[derive(Clone, Debug)]
pub struct EnumAccessDeserializer<A> {
    access: A,
}

impl<A> EnumAccessDeserializer<A> {
    /// Construct a new `EnumAccessDeserializer<A>`.
    pub fn new(access: A) -> Self {
        EnumAccessDeserializer { access }
    }
}

impl<'de, A> de::Deserializer<'de> for EnumAccessDeserializer<A>
where
    A: de::EnumAccess<'de>,
{
    type Error = A::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_enum(self.access)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de, A> IntoDeserializer<'de, A::Error> for EnumAccessDeserializer<A>
where
    A: de::EnumAccess<'de>,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

////////////////////////////////////////////////////////////////////////////////

mod private {
    use crate::lib::*;

    use crate::de::{
        self, DeserializeSeed, Deserializer, MapAccess, Unexpected, VariantAccess, Visitor,
    };

    pub struct UnitOnly<E> {
        marker: PhantomData<E>,
    }

    pub fn unit_only<T, E>(t: T) -> (T, UnitOnly<E>) {
        (
            t,
            UnitOnly {
                marker: PhantomData,
            },
        )
    }

    impl<'de, E> de::VariantAccess<'de> for UnitOnly<E>
    where
        E: de::Error,
    {
        type Error = E;

        fn unit_variant(self) -> Result<(), Self::Error> {
            Ok(())
        }

        fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value, Self::Error>
        where
            T: de::DeserializeSeed<'de>,
        {
            Err(de::Error::invalid_type(
                Unexpected::UnitVariant,
                &"newtype variant",
            ))
        }

        fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
        where
            V: de::Visitor<'de>,
        {
            Err(de::Error::invalid_type(
                Unexpected::UnitVariant,
                &"tuple variant",
            ))
        }

        fn struct_variant<V>(
            self,
            _fields: &'static [&'static str],
            _visitor: V,
        ) -> Result<V::Value, Self::Error>
        where
            V: de::Visitor<'de>,
        {
            Err(de::Error::invalid_type(
                Unexpected::UnitVariant,
                &"struct variant",
            ))
        }
    }

    pub struct MapAsEnum<A> {
        map: A,
    }

    pub fn map_as_enum<A>(map: A) -> MapAsEnum<A> {
        MapAsEnum { map }
    }

    impl<'de, A> VariantAccess<'de> for MapAsEnum<A>
    where
        A: MapAccess<'de>,
    {
        type Error = A::Error;

        fn unit_variant(mut self) -> Result<(), Self::Error> {
            self.map.next_value()
        }

        fn newtype_variant_seed<T>(mut self, seed: T) -> Result<T::Value, Self::Error>
        where
            T: DeserializeSeed<'de>,
        {
            self.map.next_value_seed(seed)
        }

        fn tuple_variant<V>(mut self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            self.map.next_value_seed(SeedTupleVariant { len, visitor })
        }

        fn struct_variant<V>(
            mut self,
            _fields: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            self.map.next_value_seed(SeedStructVariant { visitor })
        }
    }

    struct SeedTupleVariant<V> {
        len: usize,
        visitor: V,
    }

    impl<'de, V> DeserializeSeed<'de> for SeedTupleVariant<V>
    where
        V: Visitor<'de>,
    {
        type Value = V::Value;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_tuple(self.len, self.visitor)
        }
    }

    struct SeedStructVariant<V> {
        visitor: V,
    }

    impl<'de, V> DeserializeSeed<'de> for SeedStructVariant<V>
    where
        V: Visitor<'de>,
    {
        type Value = V::Value;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_map(self.visitor)
        }
    }

    /// Avoid having to restate the generic types on `MapDeserializer`. The
    /// `Iterator::Item` contains enough information to figure out K and V.
    pub trait Pair {
        type First;
        type Second;
        fn split(self) -> (Self::First, Self::Second);
    }

    impl<A, B> Pair for (A, B) {
        type First = A;
        type Second = B;
        fn split(self) -> (A, B) {
            self
        }
    }

    pub type First<T> = <T as Pair>::First;
    pub type Second<T> = <T as Pair>::Second;
}
