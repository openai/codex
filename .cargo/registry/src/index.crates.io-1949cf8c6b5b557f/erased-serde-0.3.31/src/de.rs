use crate::alloc::*;
use crate::any::Any;
use crate::error::Error;
use crate::map::{OptionExt, ResultExt};
use core::fmt::{self, Display};

/// Deserialize a value of type `T` from the given trait object.
///
/// ```rust
/// use erased_serde::Deserializer;
/// use std::collections::BTreeMap as Map;
///
/// fn main() {
///     static JSON: &'static [u8] = br#"{"A": 65, "B": 66}"#;
///     static CBOR: &'static [u8] = &[162, 97, 65, 24, 65, 97, 66, 24, 66];
///
///     // Construct some deserializers.
///     let json = &mut serde_json::Deserializer::from_slice(JSON);
///     let cbor = &mut serde_cbor::Deserializer::from_slice(CBOR);
///
///     // The values in this map are boxed trait objects, which is not possible
///     // with the normal serde::Deserializer because of object safety.
///     let mut formats: Map<&str, Box<dyn Deserializer>> = Map::new();
///     formats.insert("json", Box::new(<dyn Deserializer>::erase(json)));
///     formats.insert("cbor", Box::new(<dyn Deserializer>::erase(cbor)));
///
///     // Pick a Deserializer out of the formats map.
///     let format = formats.get_mut("json").unwrap();
///
///     let data: Map<String, usize> = erased_serde::deserialize(format).unwrap();
///
///     println!("{}", data["A"] + data["B"]);
/// }
/// ```
pub fn deserialize<'de, T>(deserializer: &mut dyn Deserializer<'de>) -> Result<T, Error>
where
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(deserializer)
}

// TRAITS //////////////////////////////////////////////////////////////////////

pub trait DeserializeSeed<'de> {
    fn erased_deserialize_seed(&mut self, d: &mut dyn Deserializer<'de>) -> Result<Out, Error>;
}

/// An object-safe equivalent of Serde's `Deserializer` trait.
///
/// Any implementation of Serde's `Deserializer` can be converted to an
/// `&erased_serde::Deserializer` or `Box<erased_serde::Deserializer>` trait
/// object using `erased_serde::Deserializer::erase`.
///
/// ```rust
/// use erased_serde::Deserializer;
/// use std::collections::BTreeMap as Map;
///
/// fn main() {
///     static JSON: &'static [u8] = br#"{"A": 65, "B": 66}"#;
///     static CBOR: &'static [u8] = &[162, 97, 65, 24, 65, 97, 66, 24, 66];
///
///     // Construct some deserializers.
///     let json = &mut serde_json::Deserializer::from_slice(JSON);
///     let cbor = &mut serde_cbor::Deserializer::from_slice(CBOR);
///
///     // The values in this map are boxed trait objects, which is not possible
///     // with the normal serde::Deserializer because of object safety.
///     let mut formats: Map<&str, Box<dyn Deserializer>> = Map::new();
///     formats.insert("json", Box::new(<dyn Deserializer>::erase(json)));
///     formats.insert("cbor", Box::new(<dyn Deserializer>::erase(cbor)));
///
///     // Pick a Deserializer out of the formats map.
///     let format = formats.get_mut("json").unwrap();
///
///     let data: Map<String, usize> = erased_serde::deserialize(format).unwrap();
///
///     println!("{}", data["A"] + data["B"]);
/// }
/// ```
pub trait Deserializer<'de> {
    fn erased_deserialize_any(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_bool(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_i8(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_i16(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_i32(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_i64(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_i128(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_u8(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_u16(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_u32(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_u64(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_u128(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_f32(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_f64(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_char(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_str(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_string(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_bytes(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_byte_buf(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_option(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_unit(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_unit_struct(
        &mut self,
        name: &'static str,
        v: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error>;
    fn erased_deserialize_newtype_struct(
        &mut self,
        name: &'static str,
        v: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error>;
    fn erased_deserialize_seq(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_tuple(
        &mut self,
        len: usize,
        v: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error>;
    fn erased_deserialize_tuple_struct(
        &mut self,
        name: &'static str,
        len: usize,
        v: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error>;
    fn erased_deserialize_map(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_struct(
        &mut self,
        name: &'static str,
        fields: &'static [&'static str],
        v: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error>;
    fn erased_deserialize_identifier(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_deserialize_enum(
        &mut self,
        name: &'static str,
        variants: &'static [&'static str],
        v: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error>;
    fn erased_deserialize_ignored_any(&mut self, v: &mut dyn Visitor<'de>) -> Result<Out, Error>;
    fn erased_is_human_readable(&self) -> bool;
}

pub trait Visitor<'de> {
    fn erased_expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result;
    fn erased_visit_bool(&mut self, v: bool) -> Result<Out, Error>;
    fn erased_visit_i8(&mut self, v: i8) -> Result<Out, Error>;
    fn erased_visit_i16(&mut self, v: i16) -> Result<Out, Error>;
    fn erased_visit_i32(&mut self, v: i32) -> Result<Out, Error>;
    fn erased_visit_i64(&mut self, v: i64) -> Result<Out, Error>;
    fn erased_visit_i128(&mut self, v: i128) -> Result<Out, Error>;
    fn erased_visit_u8(&mut self, v: u8) -> Result<Out, Error>;
    fn erased_visit_u16(&mut self, v: u16) -> Result<Out, Error>;
    fn erased_visit_u32(&mut self, v: u32) -> Result<Out, Error>;
    fn erased_visit_u64(&mut self, v: u64) -> Result<Out, Error>;
    fn erased_visit_u128(&mut self, v: u128) -> Result<Out, Error>;
    fn erased_visit_f32(&mut self, v: f32) -> Result<Out, Error>;
    fn erased_visit_f64(&mut self, v: f64) -> Result<Out, Error>;
    fn erased_visit_char(&mut self, v: char) -> Result<Out, Error>;
    fn erased_visit_str(&mut self, v: &str) -> Result<Out, Error>;
    fn erased_visit_borrowed_str(&mut self, v: &'de str) -> Result<Out, Error>;
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn erased_visit_string(&mut self, v: String) -> Result<Out, Error>;
    fn erased_visit_bytes(&mut self, v: &[u8]) -> Result<Out, Error>;
    fn erased_visit_borrowed_bytes(&mut self, v: &'de [u8]) -> Result<Out, Error>;
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn erased_visit_byte_buf(&mut self, v: Vec<u8>) -> Result<Out, Error>;
    fn erased_visit_none(&mut self) -> Result<Out, Error>;
    fn erased_visit_some(&mut self, d: &mut dyn Deserializer<'de>) -> Result<Out, Error>;
    fn erased_visit_unit(&mut self) -> Result<Out, Error>;
    fn erased_visit_newtype_struct(&mut self, d: &mut dyn Deserializer<'de>) -> Result<Out, Error>;
    fn erased_visit_seq(&mut self, s: &mut dyn SeqAccess<'de>) -> Result<Out, Error>;
    fn erased_visit_map(&mut self, m: &mut dyn MapAccess<'de>) -> Result<Out, Error>;
    fn erased_visit_enum(&mut self, e: &mut dyn EnumAccess<'de>) -> Result<Out, Error>;
}

pub trait SeqAccess<'de> {
    fn erased_next_element(
        &mut self,
        d: &mut dyn DeserializeSeed<'de>,
    ) -> Result<Option<Out>, Error>;
    fn erased_size_hint(&self) -> Option<usize>;
}

pub trait MapAccess<'de> {
    fn erased_next_key(&mut self, d: &mut dyn DeserializeSeed<'de>) -> Result<Option<Out>, Error>;
    fn erased_next_value(&mut self, d: &mut dyn DeserializeSeed<'de>) -> Result<Out, Error>;
    fn erased_next_entry(
        &mut self,
        key: &mut dyn DeserializeSeed<'de>,
        value: &mut dyn DeserializeSeed<'de>,
    ) -> Result<Option<(Out, Out)>, Error>;
    fn erased_size_hint(&self) -> Option<usize>;
}

pub trait EnumAccess<'de> {
    fn erased_variant_seed(
        &mut self,
        d: &mut dyn DeserializeSeed<'de>,
    ) -> Result<(Out, Variant<'de>), Error>;
}

impl<'de> dyn Deserializer<'de> {
    /// Convert any Serde `Deserializer` to a trait object.
    ///
    /// ```rust
    /// use erased_serde::Deserializer;
    /// use std::collections::BTreeMap as Map;
    ///
    /// fn main() {
    ///     static JSON: &'static [u8] = br#"{"A": 65, "B": 66}"#;
    ///     static CBOR: &'static [u8] = &[162, 97, 65, 24, 65, 97, 66, 24, 66];
    ///
    ///     // Construct some deserializers.
    ///     let json = &mut serde_json::Deserializer::from_slice(JSON);
    ///     let cbor = &mut serde_cbor::Deserializer::from_slice(CBOR);
    ///
    ///     // The values in this map are boxed trait objects, which is not possible
    ///     // with the normal serde::Deserializer because of object safety.
    ///     let mut formats: Map<&str, Box<dyn Deserializer>> = Map::new();
    ///     formats.insert("json", Box::new(<dyn Deserializer>::erase(json)));
    ///     formats.insert("cbor", Box::new(<dyn Deserializer>::erase(cbor)));
    ///
    ///     // Pick a Deserializer out of the formats map.
    ///     let format = formats.get_mut("json").unwrap();
    ///
    ///     let data: Map<String, usize> = erased_serde::deserialize(format).unwrap();
    ///
    ///     println!("{}", data["A"] + data["B"]);
    /// }
    /// ```
    pub fn erase<D>(deserializer: D) -> erase::Deserializer<D>
    where
        D: serde::Deserializer<'de>,
    {
        erase::Deserializer {
            state: Some(deserializer),
        }
    }
}

// OUT /////////////////////////////////////////////////////////////////////////

pub struct Out(Any);

impl Out {
    unsafe fn new<T>(t: T) -> Self {
        Out(unsafe { Any::new(t) })
    }

    unsafe fn take<T>(self) -> T {
        unsafe { self.0.take() }
    }
}

// IMPL ERASED SERDE FOR SERDE /////////////////////////////////////////////////

mod erase {
    pub struct DeserializeSeed<D> {
        pub(crate) state: Option<D>,
    }

    impl<D> DeserializeSeed<D> {
        pub(crate) fn take(&mut self) -> D {
            self.state.take().unwrap()
        }
    }

    pub struct Deserializer<D> {
        pub(crate) state: Option<D>,
    }

    impl<D> Deserializer<D> {
        pub(crate) fn take(&mut self) -> D {
            self.state.take().unwrap()
        }
        pub(crate) fn as_ref(&self) -> &D {
            self.state.as_ref().unwrap()
        }
    }

    pub struct Visitor<D> {
        pub(crate) state: Option<D>,
    }

    impl<D> Visitor<D> {
        pub(crate) fn take(&mut self) -> D {
            self.state.take().unwrap()
        }
        pub(crate) fn as_ref(&self) -> &D {
            self.state.as_ref().unwrap()
        }
    }

    pub struct SeqAccess<D> {
        pub(crate) state: D,
    }

    impl<D> SeqAccess<D> {
        pub(crate) fn as_ref(&self) -> &D {
            &self.state
        }
        pub(crate) fn as_mut(&mut self) -> &mut D {
            &mut self.state
        }
    }

    pub struct MapAccess<D> {
        pub(crate) state: D,
    }

    impl<D> MapAccess<D> {
        pub(crate) fn as_ref(&self) -> &D {
            &self.state
        }
        pub(crate) fn as_mut(&mut self) -> &mut D {
            &mut self.state
        }
    }

    pub struct EnumAccess<D> {
        pub(crate) state: Option<D>,
    }

    impl<D> EnumAccess<D> {
        pub(crate) fn take(&mut self) -> D {
            self.state.take().unwrap()
        }
    }
}

impl<'de, T> DeserializeSeed<'de> for erase::DeserializeSeed<T>
where
    T: serde::de::DeserializeSeed<'de>,
{
    fn erased_deserialize_seed(
        &mut self,
        deserializer: &mut dyn Deserializer<'de>,
    ) -> Result<Out, Error> {
        unsafe { self.take().deserialize(deserializer).unsafe_map(Out::new) }
    }
}

impl<'de, T> Deserializer<'de> for erase::Deserializer<T>
where
    T: serde::Deserializer<'de>,
{
    fn erased_deserialize_any(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_any(visitor).map_err(erase)
    }

    fn erased_deserialize_bool(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_bool(visitor).map_err(erase)
    }

    fn erased_deserialize_i8(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_i8(visitor).map_err(erase)
    }

    fn erased_deserialize_i16(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_i16(visitor).map_err(erase)
    }

    fn erased_deserialize_i32(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_i32(visitor).map_err(erase)
    }

    fn erased_deserialize_i64(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_i64(visitor).map_err(erase)
    }

    fn erased_deserialize_i128(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_i128(visitor).map_err(erase)
    }

    fn erased_deserialize_u8(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_u8(visitor).map_err(erase)
    }

    fn erased_deserialize_u16(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_u16(visitor).map_err(erase)
    }

    fn erased_deserialize_u32(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_u32(visitor).map_err(erase)
    }

    fn erased_deserialize_u64(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_u64(visitor).map_err(erase)
    }

    fn erased_deserialize_u128(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_u128(visitor).map_err(erase)
    }

    fn erased_deserialize_f32(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_f32(visitor).map_err(erase)
    }

    fn erased_deserialize_f64(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_f64(visitor).map_err(erase)
    }

    fn erased_deserialize_char(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_char(visitor).map_err(erase)
    }

    fn erased_deserialize_str(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_str(visitor).map_err(erase)
    }

    fn erased_deserialize_string(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_string(visitor).map_err(erase)
    }

    fn erased_deserialize_bytes(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_bytes(visitor).map_err(erase)
    }

    fn erased_deserialize_byte_buf(
        &mut self,
        visitor: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error> {
        self.take().deserialize_byte_buf(visitor).map_err(erase)
    }

    fn erased_deserialize_option(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_option(visitor).map_err(erase)
    }

    fn erased_deserialize_unit(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_unit(visitor).map_err(erase)
    }

    fn erased_deserialize_unit_struct(
        &mut self,
        name: &'static str,
        visitor: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error> {
        self.take()
            .deserialize_unit_struct(name, visitor)
            .map_err(erase)
    }

    fn erased_deserialize_newtype_struct(
        &mut self,
        name: &'static str,
        visitor: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error> {
        self.take()
            .deserialize_newtype_struct(name, visitor)
            .map_err(erase)
    }

    fn erased_deserialize_seq(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_seq(visitor).map_err(erase)
    }

    fn erased_deserialize_tuple(
        &mut self,
        len: usize,
        visitor: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error> {
        self.take().deserialize_tuple(len, visitor).map_err(erase)
    }

    fn erased_deserialize_tuple_struct(
        &mut self,
        name: &'static str,
        len: usize,
        visitor: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error> {
        self.take()
            .deserialize_tuple_struct(name, len, visitor)
            .map_err(erase)
    }

    fn erased_deserialize_map(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
        self.take().deserialize_map(visitor).map_err(erase)
    }

    fn erased_deserialize_struct(
        &mut self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error> {
        self.take()
            .deserialize_struct(name, fields, visitor)
            .map_err(erase)
    }

    fn erased_deserialize_identifier(
        &mut self,
        visitor: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error> {
        self.take().deserialize_identifier(visitor).map_err(erase)
    }

    fn erased_deserialize_enum(
        &mut self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error> {
        self.take()
            .deserialize_enum(name, variants, visitor)
            .map_err(erase)
    }

    fn erased_deserialize_ignored_any(
        &mut self,
        visitor: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error> {
        self.take().deserialize_ignored_any(visitor).map_err(erase)
    }

    fn erased_is_human_readable(&self) -> bool {
        self.as_ref().is_human_readable()
    }
}

impl<'de, T> Visitor<'de> for erase::Visitor<T>
where
    T: serde::de::Visitor<'de>,
{
    fn erased_expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.as_ref().expecting(formatter)
    }

    fn erased_visit_bool(&mut self, v: bool) -> Result<Out, Error> {
        unsafe { self.take().visit_bool(v).unsafe_map(Out::new) }
    }

    fn erased_visit_i8(&mut self, v: i8) -> Result<Out, Error> {
        unsafe { self.take().visit_i8(v).unsafe_map(Out::new) }
    }

    fn erased_visit_i16(&mut self, v: i16) -> Result<Out, Error> {
        unsafe { self.take().visit_i16(v).unsafe_map(Out::new) }
    }

    fn erased_visit_i32(&mut self, v: i32) -> Result<Out, Error> {
        unsafe { self.take().visit_i32(v).unsafe_map(Out::new) }
    }

    fn erased_visit_i64(&mut self, v: i64) -> Result<Out, Error> {
        unsafe { self.take().visit_i64(v).unsafe_map(Out::new) }
    }

    fn erased_visit_i128(&mut self, v: i128) -> Result<Out, Error> {
        unsafe { self.take().visit_i128(v).unsafe_map(Out::new) }
    }

    fn erased_visit_u8(&mut self, v: u8) -> Result<Out, Error> {
        unsafe { self.take().visit_u8(v).unsafe_map(Out::new) }
    }

    fn erased_visit_u16(&mut self, v: u16) -> Result<Out, Error> {
        unsafe { self.take().visit_u16(v).unsafe_map(Out::new) }
    }

    fn erased_visit_u32(&mut self, v: u32) -> Result<Out, Error> {
        unsafe { self.take().visit_u32(v).unsafe_map(Out::new) }
    }

    fn erased_visit_u64(&mut self, v: u64) -> Result<Out, Error> {
        unsafe { self.take().visit_u64(v).unsafe_map(Out::new) }
    }

    fn erased_visit_u128(&mut self, v: u128) -> Result<Out, Error> {
        unsafe { self.take().visit_u128(v).unsafe_map(Out::new) }
    }

    fn erased_visit_f32(&mut self, v: f32) -> Result<Out, Error> {
        unsafe { self.take().visit_f32(v).unsafe_map(Out::new) }
    }

    fn erased_visit_f64(&mut self, v: f64) -> Result<Out, Error> {
        unsafe { self.take().visit_f64(v).unsafe_map(Out::new) }
    }

    fn erased_visit_char(&mut self, v: char) -> Result<Out, Error> {
        unsafe { self.take().visit_char(v).unsafe_map(Out::new) }
    }

    fn erased_visit_str(&mut self, v: &str) -> Result<Out, Error> {
        unsafe { self.take().visit_str(v).unsafe_map(Out::new) }
    }

    fn erased_visit_borrowed_str(&mut self, v: &'de str) -> Result<Out, Error> {
        unsafe { self.take().visit_borrowed_str(v).unsafe_map(Out::new) }
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    fn erased_visit_string(&mut self, v: String) -> Result<Out, Error> {
        unsafe { self.take().visit_string(v).unsafe_map(Out::new) }
    }

    fn erased_visit_bytes(&mut self, v: &[u8]) -> Result<Out, Error> {
        unsafe { self.take().visit_bytes(v).unsafe_map(Out::new) }
    }

    fn erased_visit_borrowed_bytes(&mut self, v: &'de [u8]) -> Result<Out, Error> {
        unsafe { self.take().visit_borrowed_bytes(v).unsafe_map(Out::new) }
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    fn erased_visit_byte_buf(&mut self, v: Vec<u8>) -> Result<Out, Error> {
        unsafe { self.take().visit_byte_buf(v).unsafe_map(Out::new) }
    }

    fn erased_visit_none(&mut self) -> Result<Out, Error> {
        unsafe { self.take().visit_none().unsafe_map(Out::new) }
    }

    fn erased_visit_some(
        &mut self,
        deserializer: &mut dyn Deserializer<'de>,
    ) -> Result<Out, Error> {
        unsafe { self.take().visit_some(deserializer).unsafe_map(Out::new) }
    }

    fn erased_visit_unit(&mut self) -> Result<Out, Error> {
        unsafe { self.take().visit_unit().unsafe_map(Out::new) }
    }

    fn erased_visit_newtype_struct(
        &mut self,
        deserializer: &mut dyn Deserializer<'de>,
    ) -> Result<Out, Error> {
        unsafe {
            self.take()
                .visit_newtype_struct(deserializer)
                .unsafe_map(Out::new)
        }
    }

    fn erased_visit_seq(&mut self, seq: &mut dyn SeqAccess<'de>) -> Result<Out, Error> {
        unsafe { self.take().visit_seq(seq).unsafe_map(Out::new) }
    }

    fn erased_visit_map(&mut self, map: &mut dyn MapAccess<'de>) -> Result<Out, Error> {
        unsafe { self.take().visit_map(map).unsafe_map(Out::new) }
    }

    fn erased_visit_enum(&mut self, data: &mut dyn EnumAccess<'de>) -> Result<Out, Error> {
        unsafe { self.take().visit_enum(data).unsafe_map(Out::new) }
    }
}

impl<'de, T> SeqAccess<'de> for erase::SeqAccess<T>
where
    T: serde::de::SeqAccess<'de>,
{
    fn erased_next_element(
        &mut self,
        seed: &mut dyn DeserializeSeed<'de>,
    ) -> Result<Option<Out>, Error> {
        self.as_mut().next_element_seed(seed).map_err(erase)
    }

    fn erased_size_hint(&self) -> Option<usize> {
        self.as_ref().size_hint()
    }
}

impl<'de, T> MapAccess<'de> for erase::MapAccess<T>
where
    T: serde::de::MapAccess<'de>,
{
    fn erased_next_key(
        &mut self,
        seed: &mut dyn DeserializeSeed<'de>,
    ) -> Result<Option<Out>, Error> {
        self.as_mut().next_key_seed(seed).map_err(erase)
    }

    fn erased_next_value(&mut self, seed: &mut dyn DeserializeSeed<'de>) -> Result<Out, Error> {
        self.as_mut().next_value_seed(seed).map_err(erase)
    }

    fn erased_next_entry(
        &mut self,
        k: &mut dyn DeserializeSeed<'de>,
        v: &mut dyn DeserializeSeed<'de>,
    ) -> Result<Option<(Out, Out)>, Error> {
        self.as_mut().next_entry_seed(k, v).map_err(erase)
    }

    fn erased_size_hint(&self) -> Option<usize> {
        self.as_ref().size_hint()
    }
}

impl<'de, T> EnumAccess<'de> for erase::EnumAccess<T>
where
    T: serde::de::EnumAccess<'de>,
{
    fn erased_variant_seed(
        &mut self,
        seed: &mut dyn DeserializeSeed<'de>,
    ) -> Result<(Out, Variant<'de>), Error> {
        self.take()
            .variant_seed(seed)
            .map(|(out, variant)| {
                use serde::de::VariantAccess;
                let erased = Variant {
                    data: unsafe { Any::new(variant) },
                    unit_variant: {
                        unsafe fn unit_variant<'de, T>(a: Any) -> Result<(), Error>
                        where
                            T: serde::de::EnumAccess<'de>,
                        {
                            unsafe { a.take::<T::Variant>().unit_variant().map_err(erase) }
                        }
                        unit_variant::<T>
                    },
                    visit_newtype: {
                        unsafe fn visit_newtype<'de, T>(
                            a: Any,
                            seed: &mut dyn DeserializeSeed<'de>,
                        ) -> Result<Out, Error>
                        where
                            T: serde::de::EnumAccess<'de>,
                        {
                            unsafe {
                                a.take::<T::Variant>()
                                    .newtype_variant_seed(seed)
                                    .map_err(erase)
                            }
                        }
                        visit_newtype::<T>
                    },
                    tuple_variant: {
                        unsafe fn tuple_variant<'de, T>(
                            a: Any,
                            len: usize,
                            visitor: &mut dyn Visitor<'de>,
                        ) -> Result<Out, Error>
                        where
                            T: serde::de::EnumAccess<'de>,
                        {
                            unsafe {
                                a.take::<T::Variant>()
                                    .tuple_variant(len, visitor)
                                    .map_err(erase)
                            }
                        }
                        tuple_variant::<T>
                    },
                    struct_variant: {
                        unsafe fn struct_variant<'de, T>(
                            a: Any,
                            fields: &'static [&'static str],
                            visitor: &mut dyn Visitor<'de>,
                        ) -> Result<Out, Error>
                        where
                            T: serde::de::EnumAccess<'de>,
                        {
                            unsafe {
                                a.take::<T::Variant>()
                                    .struct_variant(fields, visitor)
                                    .map_err(erase)
                            }
                        }
                        struct_variant::<T>
                    },
                };
                (out, erased)
            })
            .map_err(erase)
    }
}

// IMPL SERDE FOR ERASED SERDE /////////////////////////////////////////////////

impl<'de> serde::de::DeserializeSeed<'de> for &mut (dyn DeserializeSeed<'de> + '_) {
    type Value = Out;
    fn deserialize<D>(self, deserializer: D) -> Result<Out, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut erased = erase::Deserializer {
            state: Some(deserializer),
        };
        self.erased_deserialize_seed(&mut erased).map_err(unerase)
    }
}

macro_rules! impl_deserializer_for_trait_object {
    ({$($mut:tt)*} $ty:ty) => {
        impl<'de> serde::Deserializer<'de> for $ty {
            type Error = Error;

            fn deserialize_any<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_any(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_bool<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_bool(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_i8<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_i8(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_i16<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_i16(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_i32<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_i32(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_i64<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_i64(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_i128<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_i128(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_u8<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_u8(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_u16<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_u16(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_u32<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_u32(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_u64<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_u64(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_u128<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_u128(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_f32<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_f32(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_f64<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_f64(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_char<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_char(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_str<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_str(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_string<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_string(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_bytes<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_bytes(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_byte_buf<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_byte_buf(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_option<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_option(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_unit<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_unit(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_unit_struct<V>($($mut)* self, name: &'static str, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_unit_struct(name, &mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_newtype_struct<V>($($mut)* self, name: &'static str, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_newtype_struct(name, &mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_seq<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_seq(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_tuple<V>($($mut)* self, len: usize, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_tuple(len, &mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_tuple_struct<V>($($mut)* self, name: &'static str, len: usize, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_tuple_struct(name, len, &mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_map<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_map(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_struct<V>($($mut)* self, name: &'static str, fields: &'static [&'static str], visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_struct(name, fields, &mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_identifier<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_identifier(&mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_enum<V>($($mut)* self, name: &'static str, variants: &'static [&'static str], visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_enum(name, variants, &mut erased).unsafe_map(Out::take) }
            }

            fn deserialize_ignored_any<V>($($mut)* self, visitor: V) -> Result<V::Value, Error>
            where
                V: serde::de::Visitor<'de>,
            {
                let mut erased = erase::Visitor { state: Some(visitor) };
                unsafe { self.erased_deserialize_ignored_any(&mut erased).unsafe_map(Out::take) }
            }

            fn is_human_readable(&self) -> bool {
                self.erased_is_human_readable()
            }
        }
    };
}

impl_deserializer_for_trait_object!({} &mut (dyn Deserializer<'de> + '_));
impl_deserializer_for_trait_object!({} &mut (dyn Deserializer<'de> + Send + '_));
impl_deserializer_for_trait_object!({} &mut (dyn Deserializer<'de> + Sync + '_));
impl_deserializer_for_trait_object!({} &mut (dyn Deserializer<'de> + Send + Sync + '_));
impl_deserializer_for_trait_object!({mut} Box<dyn Deserializer<'de> + '_>);
impl_deserializer_for_trait_object!({mut} Box<dyn Deserializer<'de> + Send + '_>);
impl_deserializer_for_trait_object!({mut} Box<dyn Deserializer<'de> + Sync + '_>);
impl_deserializer_for_trait_object!({mut} Box<dyn Deserializer<'de> + Send + Sync + '_>);

impl<'de> serde::de::Visitor<'de> for &mut (dyn Visitor<'de> + '_) {
    type Value = Out;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        (**self).erased_expecting(formatter)
    }

    fn visit_bool<E>(self, v: bool) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_bool(v).map_err(unerase)
    }

    fn visit_i8<E>(self, v: i8) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_i8(v).map_err(unerase)
    }

    fn visit_i16<E>(self, v: i16) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_i16(v).map_err(unerase)
    }

    fn visit_i32<E>(self, v: i32) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_i32(v).map_err(unerase)
    }

    fn visit_i64<E>(self, v: i64) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_i64(v).map_err(unerase)
    }

    fn visit_i128<E>(self, v: i128) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_i128(v).map_err(unerase)
    }

    fn visit_u8<E>(self, v: u8) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_u8(v).map_err(unerase)
    }

    fn visit_u16<E>(self, v: u16) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_u16(v).map_err(unerase)
    }

    fn visit_u32<E>(self, v: u32) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_u32(v).map_err(unerase)
    }

    fn visit_u64<E>(self, v: u64) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_u64(v).map_err(unerase)
    }

    fn visit_u128<E>(self, v: u128) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_u128(v).map_err(unerase)
    }

    fn visit_f32<E>(self, v: f32) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_f32(v).map_err(unerase)
    }

    fn visit_f64<E>(self, v: f64) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_f64(v).map_err(unerase)
    }

    fn visit_char<E>(self, v: char) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_char(v).map_err(unerase)
    }

    fn visit_str<E>(self, v: &str) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_str(v).map_err(unerase)
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_borrowed_str(v).map_err(unerase)
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    fn visit_string<E>(self, v: String) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_string(v).map_err(unerase)
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_bytes(v).map_err(unerase)
    }

    fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_borrowed_bytes(v).map_err(unerase)
    }

    #[cfg(any(feature = "std", feature = "alloc"))]
    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_byte_buf(v).map_err(unerase)
    }

    fn visit_none<E>(self) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_none().map_err(unerase)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Out, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut erased = erase::Deserializer {
            state: Some(deserializer),
        };
        self.erased_visit_some(&mut erased).map_err(unerase)
    }

    fn visit_unit<E>(self) -> Result<Out, E>
    where
        E: serde::de::Error,
    {
        self.erased_visit_unit().map_err(unerase)
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Out, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut erased = erase::Deserializer {
            state: Some(deserializer),
        };
        self.erased_visit_newtype_struct(&mut erased)
            .map_err(unerase)
    }

    fn visit_seq<V>(self, seq: V) -> Result<Out, V::Error>
    where
        V: serde::de::SeqAccess<'de>,
    {
        let mut erased = erase::SeqAccess { state: seq };
        self.erased_visit_seq(&mut erased).map_err(unerase)
    }

    fn visit_map<V>(self, map: V) -> Result<Out, V::Error>
    where
        V: serde::de::MapAccess<'de>,
    {
        let mut erased = erase::MapAccess { state: map };
        self.erased_visit_map(&mut erased).map_err(unerase)
    }

    fn visit_enum<V>(self, data: V) -> Result<Out, V::Error>
    where
        V: serde::de::EnumAccess<'de>,
    {
        let mut erased = erase::EnumAccess { state: Some(data) };
        self.erased_visit_enum(&mut erased).map_err(unerase)
    }
}

impl<'de> serde::de::SeqAccess<'de> for &mut (dyn SeqAccess<'de> + '_) {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        let mut seed = erase::DeserializeSeed { state: Some(seed) };
        unsafe {
            (**self)
                .erased_next_element(&mut seed)
                .map(|opt| opt.unsafe_map(Out::take))
        }
    }

    fn size_hint(&self) -> Option<usize> {
        (**self).erased_size_hint()
    }
}

impl<'de> serde::de::MapAccess<'de> for &mut (dyn MapAccess<'de> + '_) {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        let mut erased = erase::DeserializeSeed { state: Some(seed) };
        unsafe {
            (**self)
                .erased_next_key(&mut erased)
                .map(|opt| opt.unsafe_map(Out::take))
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let mut erased = erase::DeserializeSeed { state: Some(seed) };
        unsafe {
            (**self)
                .erased_next_value(&mut erased)
                .unsafe_map(Out::take)
        }
    }

    fn size_hint(&self) -> Option<usize> {
        (**self).erased_size_hint()
    }
}

impl<'de> serde::de::EnumAccess<'de> for &mut (dyn EnumAccess<'de> + '_) {
    type Error = Error;
    type Variant = Variant<'de>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let mut erased = erase::DeserializeSeed { state: Some(seed) };
        match self.erased_variant_seed(&mut erased) {
            Ok((out, variant)) => Ok((unsafe { out.take() }, variant)),
            Err(err) => Err(err),
        }
    }
}

pub struct Variant<'de> {
    data: Any,
    unit_variant: unsafe fn(Any) -> Result<(), Error>,
    visit_newtype: unsafe fn(Any, seed: &mut dyn DeserializeSeed<'de>) -> Result<Out, Error>,
    tuple_variant: unsafe fn(Any, len: usize, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error>,
    struct_variant: unsafe fn(
        Any,
        fields: &'static [&'static str],
        visitor: &mut dyn Visitor<'de>,
    ) -> Result<Out, Error>,
}

impl<'de> serde::de::VariantAccess<'de> for Variant<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        unsafe { (self.unit_variant)(self.data) }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        let mut erased = erase::DeserializeSeed { state: Some(seed) };
        unsafe { (self.visit_newtype)(self.data, &mut erased).unsafe_map(Out::take) }
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let mut erased = erase::Visitor {
            state: Some(visitor),
        };
        unsafe { (self.tuple_variant)(self.data, len, &mut erased).unsafe_map(Out::take) }
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let mut erased = erase::Visitor {
            state: Some(visitor),
        };
        unsafe { (self.struct_variant)(self.data, fields, &mut erased).unsafe_map(Out::take) }
    }
}

// IMPL ERASED SERDE FOR ERASED SERDE //////////////////////////////////////////

macro_rules! deref_erased_deserializer {
    ($($imp:tt)+) => {
        impl $($imp)+ {
            fn erased_deserialize_any(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_any(visitor)
            }

            fn erased_deserialize_bool(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_bool(visitor)
            }

            fn erased_deserialize_i8(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_i8(visitor)
            }

            fn erased_deserialize_i16(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_i16(visitor)
            }

            fn erased_deserialize_i32(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_i32(visitor)
            }

            fn erased_deserialize_i64(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_i64(visitor)
            }

            fn erased_deserialize_i128(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_i128(visitor)
            }

            fn erased_deserialize_u8(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_u8(visitor)
            }

            fn erased_deserialize_u16(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_u16(visitor)
            }

            fn erased_deserialize_u32(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_u32(visitor)
            }

            fn erased_deserialize_u64(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_u64(visitor)
            }

            fn erased_deserialize_u128(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_u128(visitor)
            }

            fn erased_deserialize_f32(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_f32(visitor)
            }

            fn erased_deserialize_f64(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_f64(visitor)
            }

            fn erased_deserialize_char(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_char(visitor)
            }

            fn erased_deserialize_str(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_str(visitor)
            }

            fn erased_deserialize_string(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_string(visitor)
            }

            fn erased_deserialize_bytes(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_bytes(visitor)
            }

            fn erased_deserialize_byte_buf(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_byte_buf(visitor)
            }

            fn erased_deserialize_option(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_option(visitor)
            }

            fn erased_deserialize_unit(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_unit(visitor)
            }

            fn erased_deserialize_unit_struct(&mut self, name: &'static str, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_unit_struct(name, visitor)
            }

            fn erased_deserialize_newtype_struct(&mut self, name: &'static str, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_newtype_struct(name, visitor)
            }

            fn erased_deserialize_seq(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_seq(visitor)
            }

            fn erased_deserialize_tuple(&mut self, len: usize, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_tuple(len, visitor)
            }

            fn erased_deserialize_tuple_struct(&mut self, name: &'static str, len: usize, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_tuple_struct(name, len, visitor)
            }

            fn erased_deserialize_map(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_map(visitor)
            }

            fn erased_deserialize_struct(&mut self, name: &'static str, fields: &'static [&'static str], visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_struct(name, fields, visitor)
            }

            fn erased_deserialize_identifier(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_identifier(visitor)
            }

            fn erased_deserialize_enum(&mut self, name: &'static str, variants: &'static [&'static str], visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_enum(name, variants, visitor)
            }

            fn erased_deserialize_ignored_any(&mut self, visitor: &mut dyn Visitor<'de>) -> Result<Out, Error> {
                (**self).erased_deserialize_ignored_any(visitor)
            }

            fn erased_is_human_readable(&self) -> bool {
                (**self).erased_is_human_readable()
            }
        }
    };
}

deref_erased_deserializer!(<'de> Deserializer<'de> for Box<dyn Deserializer<'de> + '_>);
deref_erased_deserializer!(<'de> Deserializer<'de> for Box<dyn Deserializer<'de> + Send + '_>);
deref_erased_deserializer!(<'de> Deserializer<'de> for Box<dyn Deserializer<'de> + Sync + '_>);
deref_erased_deserializer!(<'de> Deserializer<'de> for Box<dyn Deserializer<'de> + Send + Sync + '_>);
deref_erased_deserializer!(<'de, T: ?Sized + Deserializer<'de>> Deserializer<'de> for &mut T);

// ERROR ///////////////////////////////////////////////////////////////////////

fn erase<E>(e: E) -> Error
where
    E: Display,
{
    serde::de::Error::custom(e)
}

fn unerase<E>(e: Error) -> E
where
    E: serde::de::Error,
{
    E::custom(e)
}

// TEST ////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc::ToOwned;
    use core::fmt::Debug;
    use serde_derive::Deserialize;

    fn test_json<'de, T>(json: &'de [u8])
    where
        T: serde::Deserialize<'de> + PartialEq + Debug,
    {
        let expected: T = serde_json::from_slice(json).unwrap();

        // test borrowed trait object
        {
            let mut de = serde_json::Deserializer::from_slice(json);
            let de: &mut dyn Deserializer = &mut <dyn Deserializer>::erase(&mut de);
            assert_eq!(expected, deserialize::<T>(de).unwrap());
        }

        // test boxed trait object
        {
            let mut de = serde_json::Deserializer::from_slice(json);
            let mut de: Box<dyn Deserializer> = Box::new(<dyn Deserializer>::erase(&mut de));
            assert_eq!(expected, deserialize::<T>(&mut de).unwrap());
        }
    }

    #[test]
    fn test_value() {
        test_json::<serde_json::Value>(br#"["a", 1, [true], {"a": 1}]"#);
    }

    #[test]
    fn test_struct() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct S {
            f: usize,
        }

        test_json::<S>(br#"{"f":256}"#);
    }

    #[test]
    fn test_enum() {
        #[derive(Deserialize, PartialEq, Debug)]
        enum E {
            Unit,
            Newtype(bool),
            Tuple(bool, bool),
            Struct { t: bool, f: bool },
        }

        test_json::<E>(br#""Unit""#);
        test_json::<E>(br#"{"Newtype":true}"#);
        test_json::<E>(br#"{"Tuple":[true,false]}"#);
        test_json::<E>(br#"{"Struct":{"t":true,"f":false}}"#);
    }

    #[test]
    fn test_borrowed() {
        let bytes = br#""borrowed""#.to_owned();
        test_json::<&str>(&bytes);
    }

    #[test]
    fn assert_deserializer() {
        fn assert<'de, T: serde::Deserializer<'de>>() {}

        assert::<&mut dyn Deserializer>();
        assert::<&mut (dyn Deserializer + Send)>();
        assert::<&mut (dyn Deserializer + Sync)>();
        assert::<&mut (dyn Deserializer + Send + Sync)>();
        assert::<&mut (dyn Deserializer + Sync + Send)>();

        assert::<Box<dyn Deserializer>>();
        assert::<Box<dyn Deserializer + Send>>();
        assert::<Box<dyn Deserializer + Sync>>();
        assert::<Box<dyn Deserializer + Send + Sync>>();
        assert::<Box<dyn Deserializer + Sync + Send>>();
    }
}
