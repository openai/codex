use super::Error;
use super::SerializeMap;
use super::SerializeStructVariant;
use super::SerializeTupleVariant;
use super::SerializeValueArray;

/// Serialization for TOML [values][crate::Value].
///
/// This structure implements serialization support for TOML to serialize an
/// arbitrary type to TOML. Note that the TOML format does not support all
/// datatypes in Rust, such as enums, tuples, and tuple structs. These types
/// will generate an error when serialized.
///
/// Currently a serializer always writes its output to an in-memory `String`,
/// which is passed in when creating the serializer itself.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "parse")] {
/// # #[cfg(feature = "display")] {
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Config {
///     database: Database,
/// }
///
/// #[derive(Serialize)]
/// struct Database {
///     ip: String,
///     port: Vec<u16>,
///     connection_max: u32,
///     enabled: bool,
/// }
///
/// let config = Config {
///     database: Database {
///         ip: "192.168.1.1".to_string(),
///         port: vec![8001, 8002, 8003],
///         connection_max: 5000,
///         enabled: false,
///     },
/// };
///
/// let value = serde::Serialize::serialize(
///     &config,
///     toml_edit::ser::ValueSerializer::new()
/// ).unwrap();
/// println!("{}", value)
/// # }
/// # }
/// ```
#[derive(Default)]
#[non_exhaustive]
pub struct ValueSerializer {}

impl ValueSerializer {
    /// Creates a new serializer generate a TOML document.
    pub fn new() -> Self {
        Self {}
    }
}

impl serde::ser::Serializer for ValueSerializer {
    type Ok = crate::Value;
    type Error = Error;
    type SerializeSeq = SerializeValueArray;
    type SerializeTuple = SerializeValueArray;
    type SerializeTupleStruct = SerializeValueArray;
    type SerializeTupleVariant = SerializeTupleVariant;
    type SerializeMap = SerializeMap;
    type SerializeStruct = SerializeMap;
    type SerializeStructVariant = SerializeStructVariant;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        let v: i64 = v
            .try_into()
            .map_err(|_err| Error::out_of_range(Some("u64")))?;
        self.serialize_i64(v)
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(v as f64)
    }

    fn serialize_f64(self, mut v: f64) -> Result<Self::Ok, Self::Error> {
        // Discard sign of NaN when serialized using Serde.
        //
        // In all likelihood the sign of NaNs is not meaningful in the user's
        // program. Ending up with `-nan` in the TOML document would usually be
        // surprising and undesirable, when the sign of the NaN was not
        // intentionally controlled by the caller, or may even be
        // nondeterministic if it comes from arithmetic operations or a cast.
        if v.is_nan() {
            v = v.copysign(1.0);
        }
        Ok(v.into())
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        let mut buf = [0; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(v.into())
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        use serde::ser::Serialize;
        value.serialize(self)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_none())
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("unit")))
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some(name)))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        let value = value.serialize(self)?;
        let mut table = crate::InlineTable::new();
        table.insert(variant, value);
        Ok(table.into())
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SerializeValueArray::seq(len))
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
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(SerializeTupleVariant::tuple(variant, len))
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(SerializeMap::map(len))
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(SerializeMap::struct_(name, Some(len)))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(SerializeStructVariant::struct_(variant, len))
    }
}
