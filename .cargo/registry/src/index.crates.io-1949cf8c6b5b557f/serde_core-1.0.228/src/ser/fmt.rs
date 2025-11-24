use crate::lib::*;
use crate::ser::{Error, Impossible, Serialize, Serializer};

impl Error for fmt::Error {
    fn custom<T: Display>(_msg: T) -> Self {
        fmt::Error
    }
}

macro_rules! fmt_primitives {
    ($($f:ident: $t:ty,)*) => {
        $(
            fn $f(self, v: $t) -> fmt::Result {
                Display::fmt(&v, self)
            }
        )*
    };
}

/// ```edition2021
/// use serde::ser::Serialize;
/// use serde_derive::Serialize;
/// use std::fmt::{self, Display};
///
/// #[derive(Serialize)]
/// #[serde(rename_all = "kebab-case")]
/// pub enum MessageType {
///     StartRequest,
///     EndRequest,
/// }
///
/// impl Display for MessageType {
///     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
///         self.serialize(f)
///     }
/// }
/// ```
impl<'a> Serializer for &mut fmt::Formatter<'a> {
    type Ok = ();
    type Error = fmt::Error;
    type SerializeSeq = Impossible<(), fmt::Error>;
    type SerializeTuple = Impossible<(), fmt::Error>;
    type SerializeTupleStruct = Impossible<(), fmt::Error>;
    type SerializeTupleVariant = Impossible<(), fmt::Error>;
    type SerializeMap = Impossible<(), fmt::Error>;
    type SerializeStruct = Impossible<(), fmt::Error>;
    type SerializeStructVariant = Impossible<(), fmt::Error>;

    fmt_primitives! {
        serialize_bool: bool,
        serialize_i8: i8,
        serialize_i16: i16,
        serialize_i32: i32,
        serialize_i64: i64,
        serialize_i128: i128,
        serialize_u8: u8,
        serialize_u16: u16,
        serialize_u32: u32,
        serialize_u64: u64,
        serialize_u128: u128,
        serialize_f32: f32,
        serialize_f64: f64,
        serialize_char: char,
        serialize_str: &str,
        serialize_unit_struct: &'static str,
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> fmt::Result {
        Display::fmt(variant, self)
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> fmt::Result
    where
        T: ?Sized + Serialize,
    {
        Serialize::serialize(value, self)
    }

    fn serialize_bytes(self, _v: &[u8]) -> fmt::Result {
        Err(fmt::Error)
    }

    fn serialize_none(self) -> fmt::Result {
        Err(fmt::Error)
    }

    fn serialize_some<T>(self, _value: &T) -> fmt::Result
    where
        T: ?Sized + Serialize,
    {
        Err(fmt::Error)
    }

    fn serialize_unit(self) -> fmt::Result {
        Err(fmt::Error)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> fmt::Result
    where
        T: ?Sized + Serialize,
    {
        Err(fmt::Error)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, fmt::Error> {
        Err(fmt::Error)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, fmt::Error> {
        Err(fmt::Error)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, fmt::Error> {
        Err(fmt::Error)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, fmt::Error> {
        Err(fmt::Error)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, fmt::Error> {
        Err(fmt::Error)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, fmt::Error> {
        Err(fmt::Error)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, fmt::Error> {
        Err(fmt::Error)
    }

    fn collect_str<T>(self, value: &T) -> fmt::Result
    where
        T: ?Sized + Display,
    {
        Display::fmt(value, self)
    }
}
