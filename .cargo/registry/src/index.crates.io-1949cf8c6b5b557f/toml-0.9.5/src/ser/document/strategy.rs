#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum SerializationStrategy {
    Value,
    Table,
    ArrayOfTables,
    Skip,
    Unknown,
}

impl<T> From<&T> for SerializationStrategy
where
    T: serde::ser::Serialize + ?Sized,
{
    fn from(value: &T) -> Self {
        value.serialize(WalkValue).unwrap_err()
    }
}

impl serde::ser::Error for SerializationStrategy {
    fn custom<T>(_msg: T) -> Self
    where
        T: core::fmt::Display,
    {
        Self::Unknown
    }
}

impl core::fmt::Display for SerializationStrategy {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        "error".fmt(f)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SerializationStrategy {}
#[cfg(not(feature = "std"))]
impl serde::de::StdError for SerializationStrategy {}

struct WalkValue;

impl serde::ser::Serializer for WalkValue {
    type Ok = core::convert::Infallible;
    type Error = SerializationStrategy;
    type SerializeSeq = ArrayWalkValue;
    type SerializeTuple = ArrayWalkValue;
    type SerializeTupleStruct = ArrayWalkValue;
    type SerializeTupleVariant = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeMap = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = StructWalkValue;
    type SerializeStructVariant = serde::ser::Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_i128(self, _v: i128) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_u128(self, _v: u128) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_str(self, _v: &str) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Skip)
    }

    fn serialize_some<T>(self, v: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        v.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(SerializationStrategy::Value)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        v: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        v.serialize(self)
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
        Err(SerializationStrategy::Table)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(ArrayWalkValue::new())
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(SerializationStrategy::Table)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(SerializationStrategy::Table)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        if toml_datetime::ser::is_datetime(name) {
            Ok(StructWalkValue)
        } else {
            Err(SerializationStrategy::Table)
        }
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(SerializationStrategy::Table)
    }
}

#[doc(hidden)]
pub(crate) struct ArrayWalkValue {
    is_empty: bool,
}

impl ArrayWalkValue {
    fn new() -> Self {
        Self { is_empty: true }
    }

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), SerializationStrategy>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        self.is_empty = false;
        match SerializationStrategy::from(value) {
            SerializationStrategy::Value
            | SerializationStrategy::ArrayOfTables
            | SerializationStrategy::Unknown
            | SerializationStrategy::Skip => Err(SerializationStrategy::Value),
            SerializationStrategy::Table => Ok(()),
        }
    }

    fn end(self) -> Result<core::convert::Infallible, SerializationStrategy> {
        if self.is_empty {
            Err(SerializationStrategy::Value)
        } else {
            Err(SerializationStrategy::ArrayOfTables)
        }
    }
}

impl serde::ser::SerializeSeq for ArrayWalkValue {
    type Ok = core::convert::Infallible;
    type Error = SerializationStrategy;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        self.serialize_element(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.end()
    }
}

impl serde::ser::SerializeTuple for ArrayWalkValue {
    type Ok = core::convert::Infallible;
    type Error = SerializationStrategy;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        self.serialize_element(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.end()
    }
}

impl serde::ser::SerializeTupleStruct for ArrayWalkValue {
    type Ok = core::convert::Infallible;
    type Error = SerializationStrategy;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        self.serialize_element(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.end()
    }
}

pub(crate) struct StructWalkValue;

impl serde::ser::SerializeMap for StructWalkValue {
    type Ok = core::convert::Infallible;
    type Error = SerializationStrategy;

    fn serialize_key<T>(&mut self, _input: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        Ok(())
    }

    fn serialize_value<T>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        // is date time
        Err(SerializationStrategy::Value)
    }
}

impl serde::ser::SerializeStruct for StructWalkValue {
    type Ok = core::convert::Infallible;
    type Error = SerializationStrategy;

    fn serialize_field<T>(&mut self, _key: &'static str, _value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        // is date time
        Err(SerializationStrategy::Value)
    }
}
