use crate::wrap::Wrap;
use crate::{Chain, Error, Track};
use alloc::borrow::ToOwned as _;
use alloc::string::{String, ToString as _};
use core::cell::Cell;
use core::fmt::Display;
use serde::ser::{self, Serialize};

/// Entry point for tracking path to Serialize error.
///
/// # Example
///
/// ```
/// # use serde_derive::Serialize;
/// #
/// use serde::Serialize;
/// use std::cell::RefCell;
///
/// #[derive(Serialize)]
/// struct Outer<'a> {
///     k: Inner<'a>,
/// }
///
/// #[derive(Serialize)]
/// struct Inner<'a> {
///     refcell: &'a RefCell<String>,
/// }
///
/// let refcell = RefCell::new(String::new());
/// let value = Outer {
///     k: Inner { refcell: &refcell },
/// };
///
/// // A RefCell cannot be serialized while it is still mutably borrowed.
/// let _borrowed = refcell.borrow_mut();
///
/// // Some Serializer.
/// let mut out = Vec::new();
/// let jser = &mut serde_json::Serializer::new(&mut out);
///
/// let result = serde_path_to_error::serialize(&value, jser);
/// match result {
///     Ok(_) => panic!("expected failure to serialize RefCell"),
///     Err(err) => {
///         let path = err.path().to_string();
///         assert_eq!(path, "k.refcell");
///     }
/// }
/// ```
pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, Error<S::Error>>
where
    T: ?Sized + Serialize,
    S: ser::Serializer,
{
    let mut track = Track::new();
    match T::serialize(value, Serializer::new(serializer, &mut track)) {
        Ok(ok) => Ok(ok),
        Err(err) => Err(Error {
            path: track.path(),
            original: err,
        }),
    }
}

/// Serializer adapter that records path to serialization errors.
///
/// # Example
///
/// ```
/// # use serde_derive::Serialize;
/// #
/// use serde::Serialize;
/// use std::collections::BTreeMap;
///
/// // Maps with a non-string key are not valid in JSON.
/// let mut inner_map = BTreeMap::new();
/// inner_map.insert(vec!['w', 'a', 't'], 0);
///
/// let mut outer_map = BTreeMap::new();
/// outer_map.insert("k", inner_map);
///
/// // Some Serializer.
/// let mut out = Vec::new();
/// let jser = &mut serde_json::Serializer::new(&mut out);
///
/// let mut track = serde_path_to_error::Track::new();
/// let ps = serde_path_to_error::Serializer::new(jser, &mut track);
///
/// match outer_map.serialize(ps) {
///     Ok(_) => panic!("expected failure to serialize non-string key"),
///     Err(_) => {
///         let path = track.path().to_string();
///         assert_eq!(path, "k");
///     }
/// }
/// ```
pub struct Serializer<'a, 'b, S> {
    ser: S,
    chain: &'a Chain<'a>,
    track: &'b Track,
}

impl<'a, 'b, S> Serializer<'a, 'b, S> {
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn new(ser: S, track: &'b mut Track) -> Self {
        Serializer {
            ser,
            chain: &Chain::Root,
            track,
        }
    }
}

impl<'a, 'b, S> ser::Serializer for Serializer<'a, 'b, S>
where
    S: ser::Serializer,
{
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = WrapSeq<'a, 'b, S::SerializeSeq>;
    type SerializeTuple = WrapSeq<'a, 'b, S::SerializeTuple>;
    type SerializeTupleStruct = WrapSeq<'a, 'b, S::SerializeTupleStruct>;
    type SerializeTupleVariant = WrapSeq<'a, 'b, S::SerializeTupleVariant>;
    type SerializeMap = WrapMap<'a, 'b, S::SerializeMap>;
    type SerializeStruct = Wrap<'a, 'b, S::SerializeStruct>;
    type SerializeStructVariant = Wrap<'a, 'b, S::SerializeStructVariant>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_bool(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_i8(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_i16(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_i32(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_i64(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_i128(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_u8(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_u16(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_u32(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_u64(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_u128(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_f32(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_f64(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_char(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_str(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_bytes(v)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_none()
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_some(value)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_unit()
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_unit_struct(name)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_unit_variant(name, variant_index, variant)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_newtype_struct(name, value)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .serialize_newtype_variant(name, variant_index, variant, value)
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        match self.ser.serialize_seq(len) {
            Ok(delegate) => Ok(WrapSeq::new(delegate, chain, track)),
            Err(err) => Err(track.trigger(chain, err)),
        }
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        match self.ser.serialize_tuple(len) {
            Ok(delegate) => Ok(WrapSeq::new(delegate, chain, track)),
            Err(err) => Err(track.trigger(chain, err)),
        }
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        match self.ser.serialize_tuple_struct(name, len) {
            Ok(delegate) => Ok(WrapSeq::new(delegate, chain, track)),
            Err(err) => Err(track.trigger(chain, err)),
        }
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        match self
            .ser
            .serialize_tuple_variant(name, variant_index, variant, len)
        {
            Ok(delegate) => Ok(WrapSeq::new(delegate, chain, track)),
            Err(err) => Err(track.trigger(chain, err)),
        }
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        match self.ser.serialize_map(len) {
            Ok(delegate) => Ok(WrapMap::new(delegate, chain, track)),
            Err(err) => Err(track.trigger(chain, err)),
        }
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        match self.ser.serialize_struct(name, len) {
            Ok(delegate) => Ok(Wrap::new(delegate, chain, track)),
            Err(err) => Err(track.trigger(chain, err)),
        }
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        match self
            .ser
            .serialize_struct_variant(name, variant_index, variant, len)
        {
            Ok(delegate) => Ok(Wrap::new(delegate, chain, track)),
            Err(err) => Err(track.trigger(chain, err)),
        }
    }

    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Display,
    {
        let chain = self.chain;
        let track = self.track;
        self.ser
            .collect_str(value)
            .map_err(|err| track.trigger(chain, err))
    }

    fn is_human_readable(&self) -> bool {
        self.ser.is_human_readable()
    }
}

struct TrackedValue<'a, 'b, X> {
    value: X,
    chain: &'a Chain<'a>,
    track: &'b Track,
}

impl<'a, 'b, X> TrackedValue<'a, 'b, X> {
    fn new(value: X, chain: &'a Chain<'a>, track: &'b Track) -> Self {
        TrackedValue {
            value,
            chain,
            track,
        }
    }
}

impl<'a, 'b, X> Serialize for TrackedValue<'a, 'b, X>
where
    X: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let chain = self.chain;
        let track = self.track;
        self.value
            .serialize(Serializer {
                ser: serializer,
                chain,
                track,
            })
            .map_err(|err| track.trigger(chain, err))
    }
}

pub struct WrapSeq<'a, 'b, S> {
    delegate: S,
    chain: &'a Chain<'a>,
    index: usize,
    track: &'b Track,
}

impl<'a, 'b, S> WrapSeq<'a, 'b, S> {
    fn new(delegate: S, chain: &'a Chain<'a>, track: &'b Track) -> Self {
        WrapSeq {
            delegate,
            chain,
            index: 0,
            track,
        }
    }
}

impl<'a, 'b, S> ser::SerializeSeq for WrapSeq<'a, 'b, S>
where
    S: ser::SerializeSeq,
{
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let parent = self.chain;
        let chain = Chain::Seq {
            parent,
            index: self.index,
        };
        let track = self.track;
        self.index += 1;
        self.delegate
            .serialize_element(&TrackedValue::new(value, &chain, track))
            .map_err(|err| track.trigger(parent, err))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.delegate.end().map_err(|err| track.trigger(chain, err))
    }
}

impl<'a, 'b, S> ser::SerializeTuple for WrapSeq<'a, 'b, S>
where
    S: ser::SerializeTuple,
{
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let parent = self.chain;
        let chain = Chain::Seq {
            parent,
            index: self.index,
        };
        let track = self.track;
        self.index += 1;
        self.delegate
            .serialize_element(&TrackedValue::new(value, &chain, track))
            .map_err(|err| track.trigger(parent, err))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.delegate.end().map_err(|err| track.trigger(chain, err))
    }
}

impl<'a, 'b, S> ser::SerializeTupleStruct for WrapSeq<'a, 'b, S>
where
    S: ser::SerializeTupleStruct,
{
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let parent = self.chain;
        let chain = Chain::Seq {
            parent,
            index: self.index,
        };
        let track = self.track;
        self.index += 1;
        self.delegate
            .serialize_field(&TrackedValue::new(value, &chain, track))
            .map_err(|err| track.trigger(parent, err))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.delegate.end().map_err(|err| track.trigger(chain, err))
    }
}

impl<'a, 'b, S> ser::SerializeTupleVariant for WrapSeq<'a, 'b, S>
where
    S: ser::SerializeTupleVariant,
{
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let parent = self.chain;
        let chain = Chain::Seq {
            parent,
            index: self.index,
        };
        let track = self.track;
        self.index += 1;
        self.delegate
            .serialize_field(&TrackedValue::new(value, &chain, track))
            .map_err(|err| track.trigger(parent, err))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.delegate.end().map_err(|err| track.trigger(chain, err))
    }
}

pub struct WrapMap<'a, 'b, S> {
    delegate: S,
    chain: &'a Chain<'a>,
    key: Cell<Option<String>>,
    track: &'b Track,
}

impl<'a, 'b, S> WrapMap<'a, 'b, S> {
    fn new(delegate: S, chain: &'a Chain<'a>, track: &'b Track) -> Self {
        WrapMap {
            delegate,
            chain,
            key: Cell::new(None),
            track,
        }
    }
}

impl<'a, 'b, S> ser::SerializeMap for WrapMap<'a, 'b, S>
where
    S: ser::SerializeMap,
{
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let chain = self.chain;
        let track = self.track;
        self.key.set(None);
        self.delegate
            .serialize_key(&CaptureKey::new(&self.key, key))
            .map_err(|err| track.trigger(chain, err))
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let parent = self.chain;
        let chain = match self.key.take() {
            Some(key) => Chain::Map { parent, key },
            None => Chain::NonStringKey { parent },
        };
        let track = self.track;
        self.delegate
            .serialize_value(&TrackedValue::new(value, &chain, track))
            .map_err(|err| track.trigger(parent, err))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.delegate.end().map_err(|err| track.trigger(chain, err))
    }
}

impl<'a, 'b, S> ser::SerializeStruct for Wrap<'a, 'b, S>
where
    S: ser::SerializeStruct,
{
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let parent = self.chain;
        let chain = Chain::Struct { parent, key };
        let track = self.track;
        self.delegate
            .serialize_field(key, &TrackedValue::new(value, &chain, track))
            .map_err(|err| track.trigger(parent, err))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.delegate.end().map_err(|err| track.trigger(chain, err))
    }

    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.delegate
            .skip_field(key)
            .map_err(|err| track.trigger(chain, err))
    }
}

impl<'a, 'b, S> ser::SerializeStructVariant for Wrap<'a, 'b, S>
where
    S: ser::SerializeStructVariant,
{
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let parent = self.chain;
        let chain = Chain::Struct { parent, key };
        let track = self.track;
        self.delegate
            .serialize_field(key, &TrackedValue::new(value, &chain, track))
            .map_err(|err| track.trigger(parent, err))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.delegate.end().map_err(|err| track.trigger(chain, err))
    }

    fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
        let chain = self.chain;
        let track = self.track;
        self.delegate
            .skip_field(key)
            .map_err(|err| track.trigger(chain, err))
    }
}

struct CaptureKey<'a, T> {
    out: &'a Cell<Option<String>>,
    delegate: T,
}

impl<'a, T> CaptureKey<'a, T> {
    fn new(out: &'a Cell<Option<String>>, delegate: T) -> Self {
        CaptureKey { out, delegate }
    }
}

impl<'a, T> Serialize for CaptureKey<'a, T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.delegate
            .serialize(CaptureKey::new(self.out, serializer))
    }
}

impl<'a, S> ser::Serializer for CaptureKey<'a, S>
where
    S: ser::Serializer,
{
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = S::SerializeSeq;
    type SerializeTuple = S::SerializeTuple;
    type SerializeTupleStruct = S::SerializeTupleStruct;
    type SerializeTupleVariant = S::SerializeTupleVariant;
    type SerializeMap = S::SerializeMap;
    type SerializeStruct = S::SerializeStruct;
    type SerializeStructVariant = S::SerializeStructVariant;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        let string = if v { "true" } else { "false" };
        self.out.set(Some(string.to_owned()));
        self.delegate.serialize_bool(v)
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.out.set(Some(itoa::Buffer::new().format(v).to_owned()));
        self.delegate.serialize_i8(v)
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.out.set(Some(itoa::Buffer::new().format(v).to_owned()));
        self.delegate.serialize_i16(v)
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.out.set(Some(itoa::Buffer::new().format(v).to_owned()));
        self.delegate.serialize_i32(v)
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.out.set(Some(itoa::Buffer::new().format(v).to_owned()));
        self.delegate.serialize_i64(v)
    }

    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        self.out.set(Some(itoa::Buffer::new().format(v).to_owned()));
        self.delegate.serialize_i128(v)
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.out.set(Some(itoa::Buffer::new().format(v).to_owned()));
        self.delegate.serialize_u8(v)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.out.set(Some(itoa::Buffer::new().format(v).to_owned()));
        self.delegate.serialize_u16(v)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.out.set(Some(itoa::Buffer::new().format(v).to_owned()));
        self.delegate.serialize_u32(v)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.out.set(Some(itoa::Buffer::new().format(v).to_owned()));
        self.delegate.serialize_u64(v)
    }

    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        self.out.set(Some(itoa::Buffer::new().format(v).to_owned()));
        self.delegate.serialize_u128(v)
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.delegate.serialize_f32(v)
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.delegate.serialize_f64(v)
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.delegate.serialize_char(v)
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.out.set(Some(v.to_owned()));
        self.delegate.serialize_str(v)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.delegate.serialize_bytes(v)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.delegate.serialize_none()
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.delegate
            .serialize_some(&CaptureKey::new(self.out, value))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.delegate.serialize_unit()
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.delegate.serialize_unit_struct(name)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.out.set(Some(variant.to_owned()));
        self.delegate
            .serialize_unit_variant(name, variant_index, variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.delegate
            .serialize_newtype_struct(name, &CaptureKey::new(self.out, value))
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.delegate
            .serialize_newtype_variant(name, variant_index, variant, value)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.delegate.serialize_seq(len)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.delegate.serialize_tuple(len)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.delegate.serialize_tuple_struct(name, len)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.delegate
            .serialize_tuple_variant(name, variant_index, variant, len)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.delegate.serialize_map(len)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.delegate.serialize_struct(name, len)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.delegate
            .serialize_struct_variant(name, variant_index, variant, len)
    }

    fn collect_seq<I>(self, iter: I) -> Result<Self::Ok, Self::Error>
    where
        I: IntoIterator,
        I::Item: Serialize,
    {
        self.delegate.collect_seq(iter)
    }

    fn collect_map<K, V, I>(self, iter: I) -> Result<Self::Ok, Self::Error>
    where
        K: Serialize,
        V: Serialize,
        I: IntoIterator<Item = (K, V)>,
    {
        self.delegate.collect_map(iter)
    }

    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Display,
    {
        self.out.set(Some(value.to_string()));
        self.delegate.collect_str(value)
    }

    fn is_human_readable(&self) -> bool {
        self.delegate.is_human_readable()
    }
}
