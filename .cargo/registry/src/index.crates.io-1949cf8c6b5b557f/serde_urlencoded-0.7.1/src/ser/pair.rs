use crate::ser::key::KeySink;
use crate::ser::part::PartSerializer;
use crate::ser::value::ValueSink;
use crate::ser::Error;
use form_urlencoded::Serializer as UrlEncodedSerializer;
use form_urlencoded::Target as UrlEncodedTarget;
use serde::ser;
use std::borrow::Cow;
use std::mem;

pub struct PairSerializer<'input, 'target, Target: UrlEncodedTarget> {
    urlencoder: &'target mut UrlEncodedSerializer<'input, Target>,
    state: PairState,
}

impl<'input, 'target, Target> PairSerializer<'input, 'target, Target>
where
    Target: 'target + UrlEncodedTarget,
{
    pub fn new(
        urlencoder: &'target mut UrlEncodedSerializer<'input, Target>,
    ) -> Self {
        PairSerializer {
            urlencoder,
            state: PairState::WaitingForKey,
        }
    }
}

impl<'input, 'target, Target> ser::Serializer
    for PairSerializer<'input, 'target, Target>
where
    Target: 'target + UrlEncodedTarget,
{
    type Ok = ();
    type Error = Error;
    type SerializeSeq = ser::Impossible<(), Error>;
    type SerializeTuple = Self;
    type SerializeTupleStruct = ser::Impossible<(), Error>;
    type SerializeTupleVariant = ser::Impossible<(), Error>;
    type SerializeMap = ser::Impossible<(), Error>;
    type SerializeStruct = ser::Impossible<(), Error>;
    type SerializeStructVariant = ser::Impossible<(), Error>;

    fn serialize_bool(self, _v: bool) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_i8(self, _v: i8) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_i16(self, _v: i16) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_i32(self, _v: i32) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_i64(self, _v: i64) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_u8(self, _v: u8) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_u16(self, _v: u16) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_u32(self, _v: u32) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_u64(self, _v: u64) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_f32(self, _v: f32) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_f64(self, _v: f64) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_char(self, _v: char) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_str(self, _value: &str) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_unit(self) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_newtype_struct<T: ?Sized + ser::Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + ser::Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<(), Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_none(self) -> Result<(), Error> {
        Ok(())
    }

    fn serialize_some<T: ?Sized + ser::Serialize>(
        self,
        value: &T,
    ) -> Result<(), Error> {
        value.serialize(self)
    }

    fn serialize_seq(
        self,
        _len: Option<usize>,
    ) -> Result<Self::SerializeSeq, Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_tuple(self, len: usize) -> Result<Self, Error> {
        if len == 2 {
            Ok(self)
        } else {
            Err(Error::unsupported_pair())
        }
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_map(
        self,
        _len: Option<usize>,
    ) -> Result<Self::SerializeMap, Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        Err(Error::unsupported_pair())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Err(Error::unsupported_pair())
    }
}

impl<'input, 'target, Target> ser::SerializeTuple
    for PairSerializer<'input, 'target, Target>
where
    Target: 'target + UrlEncodedTarget,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + ser::Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), Error> {
        match mem::replace(&mut self.state, PairState::Done) {
            PairState::WaitingForKey => {
                let key_sink = KeySink::new(|key| Ok(key.into()));
                let key_serializer = PartSerializer::new(key_sink);
                self.state = PairState::WaitingForValue {
                    key: value.serialize(key_serializer)?,
                };
                Ok(())
            }
            PairState::WaitingForValue { key } => {
                let result = {
                    let value_sink = ValueSink::new(self.urlencoder, &key);
                    let value_serializer = PartSerializer::new(value_sink);
                    value.serialize(value_serializer)
                };
                if result.is_ok() {
                    self.state = PairState::Done;
                } else {
                    self.state = PairState::WaitingForValue { key };
                }
                result
            }
            PairState::Done => Err(Error::done()),
        }
    }

    fn end(self) -> Result<(), Error> {
        if let PairState::Done = self.state {
            Ok(())
        } else {
            Err(Error::not_done())
        }
    }
}

enum PairState {
    WaitingForKey,
    WaitingForValue { key: Cow<'static, str> },
    Done,
}

impl Error {
    fn done() -> Self {
        Error::Custom("this pair has already been serialized".into())
    }

    fn not_done() -> Self {
        Error::Custom("this pair has not yet been serialized".into())
    }

    fn unsupported_pair() -> Self {
        Error::Custom("unsupported pair".into())
    }
}
