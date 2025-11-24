//! Serialization support for [`Datetime`][crate::Datetime]

/// Check if serializing a [`Datetime`][crate::Datetime]
pub fn is_datetime(name: &'static str) -> bool {
    crate::datetime::is_datetime(name)
}

/// See [`DatetimeSerializer`]
#[derive(Debug)]
#[non_exhaustive]
pub enum SerializerError {
    /// Unsupported datetime format
    InvalidFormat(crate::DatetimeParseError),
    /// Unsupported serialization protocol
    InvalidProtocol,
}

impl serde::ser::Error for SerializerError {
    fn custom<T>(_msg: T) -> Self
    where
        T: core::fmt::Display,
    {
        Self::InvalidProtocol
    }
}

impl core::fmt::Display for SerializerError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidFormat(e) => e.fmt(formatter),
            Self::InvalidProtocol => "invalid serialization protocol".fmt(formatter),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SerializerError {}
#[cfg(all(not(feature = "std"), feature = "serde"))]
impl serde::de::StdError for SerializerError {}

/// Serializer / format support for emitting [`Datetime`][crate::Datetime]
#[derive(Default)]
pub struct DatetimeSerializer {
    value: Option<crate::Datetime>,
}

impl DatetimeSerializer {
    /// Create a serializer to emit [`Datetime`][crate::Datetime]
    pub fn new() -> Self {
        Self { value: None }
    }

    /// See [`serde::ser::SerializeStruct::serialize_field`]
    pub fn serialize_field<T>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), SerializerError>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        if key == crate::datetime::FIELD {
            self.value = Some(value.serialize(DatetimeFieldSerializer::default())?);
        }

        Ok(())
    }

    /// See [`serde::ser::SerializeStruct::end`]
    pub fn end(self) -> Result<crate::Datetime, SerializerError> {
        self.value.ok_or(SerializerError::InvalidProtocol)
    }
}

#[derive(Default)]
struct DatetimeFieldSerializer {}

impl serde::ser::Serializer for DatetimeFieldSerializer {
    type Ok = crate::Datetime;
    type Error = SerializerError;
    type SerializeSeq = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeMap = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = serde::ser::Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, _value: bool) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_i8(self, _value: i8) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_i16(self, _value: i16) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_i32(self, _value: i32) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_i64(self, _value: i64) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_u8(self, _value: u8) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_u16(self, _value: u16) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_u32(self, _value: u32) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_u64(self, _value: u64) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_char(self, _value: char) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        v.parse::<crate::Datetime>()
            .map_err(SerializerError::InvalidFormat)
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_some<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(SerializerError::InvalidProtocol)
    }
}
