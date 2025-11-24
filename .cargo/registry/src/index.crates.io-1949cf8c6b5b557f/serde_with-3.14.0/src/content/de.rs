//! Buffer for deserializing data.
//!
//! This is a copy and improvement of the `serde` private type:
//! <https://github.com/serde-rs/serde/blob/f85c4f2fa99c7f3b103e6e2580e75eb9313b00d0/serde/src/private/de.rs#L195>
//! The code is very stable in the `serde` crate, so no maintainability problem is expected.
//!
//! Since the type is private we copy the type here.
//! `serde` is licensed as MIT+Apache2, the same as this crate.
//!
//! This version carries improvements compared to `serde`'s version.
//! The types support 128-bit integers, which is supported for all targets in Rust 1.40+.
//! A value for `is_human_readable` is passed through all types, to preserve the information.
//!
//! In the future this can hopefully be replaced by a public type in `serde` itself.
//! <https://github.com/serde-rs/serde/pull/2348>

use self::utils::{get_unexpected_i128, get_unexpected_u128};
use crate::{
    prelude::*,
    utils::{size_hint_cautious, size_hint_from_bounds},
};

/// Used from generated code to buffer the contents of the Deserializer when
/// deserializing untagged enums and internally tagged enums.
pub(crate) enum Content<'de> {
    Bool(bool),

    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),

    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),

    F32(f32),
    F64(f64),

    Char(char),
    String(String),
    Str(&'de str),
    ByteBuf(Vec<u8>),
    Bytes(&'de [u8]),

    None,
    Some(Box<Content<'de>>),

    Unit,
    Newtype(Box<Content<'de>>),
    Seq(Vec<Content<'de>>),
    Map(Vec<(Content<'de>, Content<'de>)>),
}

impl Content<'_> {
    #[cold]
    fn unexpected<'a>(&'a self, buf: &'a mut [u8; 58]) -> Unexpected<'a> {
        match *self {
            Content::Bool(b) => Unexpected::Bool(b),
            Content::U8(n) => Unexpected::Unsigned(u64::from(n)),
            Content::U16(n) => Unexpected::Unsigned(u64::from(n)),
            Content::U32(n) => Unexpected::Unsigned(u64::from(n)),
            Content::U64(n) => Unexpected::Unsigned(n),
            Content::U128(n) => get_unexpected_u128(n, buf),
            Content::I8(n) => Unexpected::Signed(i64::from(n)),
            Content::I16(n) => Unexpected::Signed(i64::from(n)),
            Content::I32(n) => Unexpected::Signed(i64::from(n)),
            Content::I64(n) => Unexpected::Signed(n),
            Content::I128(n) => get_unexpected_i128(n, buf),
            Content::F32(f) => Unexpected::Float(f64::from(f)),
            Content::F64(f) => Unexpected::Float(f),
            Content::Char(c) => Unexpected::Char(c),
            Content::String(ref s) => Unexpected::Str(s),
            Content::Str(s) => Unexpected::Str(s),
            Content::ByteBuf(ref b) => Unexpected::Bytes(b),
            Content::Bytes(b) => Unexpected::Bytes(b),
            Content::None | Content::Some(_) => Unexpected::Option,
            Content::Unit => Unexpected::Unit,
            Content::Newtype(_) => Unexpected::NewtypeStruct,
            Content::Seq(_) => Unexpected::Seq,
            Content::Map(_) => Unexpected::Map,
        }
    }
}

impl<'de> Deserialize<'de> for Content<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Untagged and internally tagged enums are only supported in
        // self-describing formats.
        let visitor = ContentVisitor { value: PhantomData };
        deserializer.deserialize_any(visitor)
    }
}

struct ContentVisitor<'de> {
    value: PhantomData<Content<'de>>,
}

impl<'de> Visitor<'de> for ContentVisitor<'de> {
    type Value = Content<'de>;

    fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str("any value")
    }

    fn visit_bool<F>(self, value: bool) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::Bool(value))
    }

    fn visit_i8<F>(self, value: i8) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::I8(value))
    }

    fn visit_i16<F>(self, value: i16) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::I16(value))
    }

    fn visit_i32<F>(self, value: i32) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::I32(value))
    }

    fn visit_i64<F>(self, value: i64) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::I64(value))
    }

    fn visit_i128<F>(self, value: i128) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::I128(value))
    }

    fn visit_u8<F>(self, value: u8) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::U8(value))
    }

    fn visit_u16<F>(self, value: u16) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::U16(value))
    }

    fn visit_u32<F>(self, value: u32) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::U32(value))
    }

    fn visit_u64<F>(self, value: u64) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::U64(value))
    }

    fn visit_u128<F>(self, value: u128) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::U128(value))
    }

    fn visit_f32<F>(self, value: f32) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::F32(value))
    }

    fn visit_f64<F>(self, value: f64) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::F64(value))
    }

    fn visit_char<F>(self, value: char) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::Char(value))
    }

    fn visit_str<F>(self, value: &str) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::String(value.into()))
    }

    fn visit_borrowed_str<F>(self, value: &'de str) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::Str(value))
    }

    fn visit_string<F>(self, value: String) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::String(value))
    }

    fn visit_bytes<F>(self, value: &[u8]) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::ByteBuf(value.into()))
    }

    fn visit_borrowed_bytes<F>(self, value: &'de [u8]) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::Bytes(value))
    }

    fn visit_byte_buf<F>(self, value: Vec<u8>) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::ByteBuf(value))
    }

    fn visit_unit<F>(self) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::Unit)
    }

    fn visit_none<F>(self) -> Result<Self::Value, F>
    where
        F: DeError,
    {
        Ok(Content::None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(|v| Content::Some(Box::new(v)))
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(|v| Content::Newtype(Box::new(v)))
    }

    fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut vec = Vec::with_capacity(size_hint_cautious::<Content<'_>>(visitor.size_hint()));
        while let Some(e) = visitor.next_element()? {
            vec.push(e);
        }
        Ok(Content::Seq(vec))
    }

    fn visit_map<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
    where
        V: MapAccess<'de>,
    {
        let mut vec = Vec::with_capacity(size_hint_cautious::<(Content<'_>, Content<'_>)>(
            visitor.size_hint(),
        ));
        while let Some(kv) = visitor.next_entry()? {
            vec.push(kv);
        }
        Ok(Content::Map(vec))
    }

    fn visit_enum<V>(self, _visitor: V) -> Result<Self::Value, V::Error>
    where
        V: EnumAccess<'de>,
    {
        Err(DeError::custom(
            "untagged and internally tagged enums do not support enum input",
        ))
    }
}

pub(crate) struct ContentDeserializer<'de, E> {
    is_human_readable: bool,
    content: Content<'de>,
    err: PhantomData<E>,
}

impl<'de, E> ContentDeserializer<'de, E>
where
    E: DeError,
{
    #[cold]
    fn invalid_type(self, exp: &dyn Expected) -> E {
        let mut buf = [0; 58];
        DeError::invalid_type(self.content.unexpected(&mut buf), exp)
    }

    fn deserialize_integer<V>(self, visitor: V) -> Result<V::Value, E>
    where
        V: Visitor<'de>,
    {
        match self.content {
            Content::U8(v) => visitor.visit_u8(v),
            Content::U16(v) => visitor.visit_u16(v),
            Content::U32(v) => visitor.visit_u32(v),
            Content::U64(v) => visitor.visit_u64(v),
            Content::U128(v) => visitor.visit_u128(v),
            Content::I8(v) => visitor.visit_i8(v),
            Content::I16(v) => visitor.visit_i16(v),
            Content::I32(v) => visitor.visit_i32(v),
            Content::I64(v) => visitor.visit_i64(v),
            Content::I128(v) => visitor.visit_i128(v),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_float<V>(self, visitor: V) -> Result<V::Value, E>
    where
        V: Visitor<'de>,
    {
        match self.content {
            Content::F32(v) => visitor.visit_f32(v),
            Content::F64(v) => visitor.visit_f64(v),
            Content::U8(v) => visitor.visit_u8(v),
            Content::U16(v) => visitor.visit_u16(v),
            Content::U32(v) => visitor.visit_u32(v),
            Content::U64(v) => visitor.visit_u64(v),
            Content::U128(v) => visitor.visit_u128(v),
            Content::I8(v) => visitor.visit_i8(v),
            Content::I16(v) => visitor.visit_i16(v),
            Content::I32(v) => visitor.visit_i32(v),
            Content::I64(v) => visitor.visit_i64(v),
            Content::I128(v) => visitor.visit_i128(v),
            _ => Err(self.invalid_type(&visitor)),
        }
    }
}

fn visit_content_seq<'de, V, E>(
    content: Vec<Content<'de>>,
    visitor: V,
    is_human_readable: bool,
) -> Result<V::Value, E>
where
    V: Visitor<'de>,
    E: DeError,
{
    let seq = content
        .into_iter()
        .map(|x| ContentDeserializer::new(x, is_human_readable));
    let mut seq_visitor = serde::de::value::SeqDeserializer::new(seq);
    let value = visitor.visit_seq(&mut seq_visitor)?;
    seq_visitor.end()?;
    Ok(value)
}

fn visit_content_map<'de, V, E>(
    content: Vec<(Content<'de>, Content<'de>)>,
    visitor: V,
    is_human_readable: bool,
) -> Result<V::Value, E>
where
    V: Visitor<'de>,
    E: DeError,
{
    let map = content.into_iter().map(|(k, v)| {
        (
            ContentDeserializer::new(k, is_human_readable),
            ContentDeserializer::new(v, is_human_readable),
        )
    });
    let mut map_visitor = serde::de::value::MapDeserializer::new(map);
    let value = visitor.visit_map(&mut map_visitor)?;
    map_visitor.end()?;
    Ok(value)
}

/// Used when deserializing an internally tagged enum because the content
/// will be used exactly once.
impl<'de, E> Deserializer<'de> for ContentDeserializer<'de, E>
where
    E: DeError,
{
    type Error = E;

    #[inline]
    fn is_human_readable(&self) -> bool {
        self.is_human_readable
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.content {
            Content::Bool(v) => visitor.visit_bool(v),
            Content::U8(v) => visitor.visit_u8(v),
            Content::U16(v) => visitor.visit_u16(v),
            Content::U32(v) => visitor.visit_u32(v),
            Content::U64(v) => visitor.visit_u64(v),
            Content::U128(v) => visitor.visit_u128(v),
            Content::I8(v) => visitor.visit_i8(v),
            Content::I16(v) => visitor.visit_i16(v),
            Content::I32(v) => visitor.visit_i32(v),
            Content::I64(v) => visitor.visit_i64(v),
            Content::I128(v) => visitor.visit_i128(v),
            Content::F32(v) => visitor.visit_f32(v),
            Content::F64(v) => visitor.visit_f64(v),
            Content::Char(v) => visitor.visit_char(v),
            Content::String(v) => visitor.visit_string(v),
            Content::Str(v) => visitor.visit_borrowed_str(v),
            Content::ByteBuf(v) => visitor.visit_byte_buf(v),
            Content::Bytes(v) => visitor.visit_borrowed_bytes(v),
            Content::Unit => visitor.visit_unit(),
            Content::None => visitor.visit_none(),
            Content::Some(v) => {
                visitor.visit_some(ContentDeserializer::new(*v, self.is_human_readable))
            }
            Content::Newtype(v) => {
                visitor.visit_newtype_struct(ContentDeserializer::new(*v, self.is_human_readable))
            }
            Content::Seq(v) => visit_content_seq(v, visitor, self.is_human_readable),
            Content::Map(v) => visit_content_map(v, visitor, self.is_human_readable),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.content {
            Content::Bool(v) => visitor.visit_bool(v),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_i128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_u128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_float(visitor)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_float(visitor)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.content {
            Content::Char(v) => visitor.visit_char(v),
            Content::String(v) => visitor.visit_string(v),
            Content::Str(v) => visitor.visit_borrowed_str(v),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_string(visitor)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.content {
            Content::String(v) => visitor.visit_string(v),
            Content::Str(v) => visitor.visit_borrowed_str(v),
            Content::ByteBuf(v) => visitor.visit_byte_buf(v),
            Content::Bytes(v) => visitor.visit_borrowed_bytes(v),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_byte_buf(visitor)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.content {
            Content::String(v) => visitor.visit_string(v),
            Content::Str(v) => visitor.visit_borrowed_str(v),
            Content::ByteBuf(v) => visitor.visit_byte_buf(v),
            Content::Bytes(v) => visitor.visit_borrowed_bytes(v),
            Content::Seq(v) => visit_content_seq(v, visitor, self.is_human_readable),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.content {
            Content::None => visitor.visit_none(),
            Content::Some(v) => {
                visitor.visit_some(ContentDeserializer::new(*v, self.is_human_readable))
            }
            Content::Unit => visitor.visit_unit(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.content {
            Content::Unit => visitor.visit_unit(),

            // Allow deserializing newtype variant containing unit.
            //
            //     #[derive(Deserialize)]
            //     #[serde(tag = "result")]
            //     enum Response<T> {
            //         Success(T),
            //     }
            //
            // We want {"result":"Success"} to deserialize into Response<()>.
            Content::Map(ref v) if v.is_empty() => visitor.visit_unit(),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.content {
            // As a special case, allow deserializing untagged newtype
            // variant containing unit struct.
            //
            //     #[derive(Deserialize)]
            //     struct Info;
            //
            //     #[derive(Deserialize)]
            //     #[serde(tag = "topic")]
            //     enum Message {
            //         Info(Info),
            //     }
            //
            // We want {"topic":"Info"} to deserialize even though
            // ordinarily unit structs do not deserialize from empty map/seq.
            Content::Map(ref v) if v.is_empty() => visitor.visit_unit(),
            Content::Seq(ref v) if v.is_empty() => visitor.visit_unit(),
            _ => self.deserialize_any(visitor),
        }
    }

    fn deserialize_newtype_struct<V>(self, _name: &str, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.content {
            Content::Newtype(v) => {
                visitor.visit_newtype_struct(ContentDeserializer::new(*v, self.is_human_readable))
            }
            _ => visitor.visit_newtype_struct(self),
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.content {
            Content::Seq(v) => visit_content_seq(v, visitor, self.is_human_readable),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.content {
            Content::Map(v) => visit_content_map(v, visitor, self.is_human_readable),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.content {
            Content::Seq(v) => visit_content_seq(v, visitor, self.is_human_readable),
            Content::Map(v) => visit_content_map(v, visitor, self.is_human_readable),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_enum<V>(
        self,
        _name: &str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let (variant, value) = match self.content {
            Content::Map(value) => {
                let mut iter = value.into_iter();
                let (variant, value) = match iter.next() {
                    Some(v) => v,
                    None => {
                        return Err(DeError::invalid_value(
                            Unexpected::Map,
                            &"map with a single key",
                        ));
                    }
                };
                // enums are encoded in json as maps with a single key:value pair
                if iter.next().is_some() {
                    return Err(DeError::invalid_value(
                        Unexpected::Map,
                        &"map with a single key",
                    ));
                }
                (variant, Some(value))
            }
            s @ Content::String(_) | s @ Content::Str(_) => (s, None),
            other => {
                let mut buf = [0; 58];
                return Err(DeError::invalid_type(
                    other.unexpected(&mut buf),
                    &"string or map",
                ));
            }
        };

        visitor.visit_enum(EnumDeserializer::new(
            variant,
            value,
            self.is_human_readable,
        ))
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.content {
            Content::String(v) => visitor.visit_string(v),
            Content::Str(v) => visitor.visit_borrowed_str(v),
            Content::ByteBuf(v) => visitor.visit_byte_buf(v),
            Content::Bytes(v) => visitor.visit_borrowed_bytes(v),
            Content::U8(v) => visitor.visit_u8(v),
            Content::U64(v) => visitor.visit_u64(v),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        drop(self);
        visitor.visit_unit()
    }
}

impl<'de, E> ContentDeserializer<'de, E> {
    /// private API, don't use
    pub(crate) fn new(content: Content<'de>, is_human_readable: bool) -> Self {
        ContentDeserializer {
            is_human_readable,
            content,
            err: PhantomData,
        }
    }
}

struct EnumDeserializer<'de, E>
where
    E: DeError,
{
    is_human_readable: bool,
    variant: Content<'de>,
    value: Option<Content<'de>>,
    err: PhantomData<E>,
}

impl<'de, E> EnumDeserializer<'de, E>
where
    E: DeError,
{
    pub fn new(
        variant: Content<'de>,
        value: Option<Content<'de>>,
        is_human_readable: bool,
    ) -> EnumDeserializer<'de, E> {
        EnumDeserializer {
            is_human_readable,
            variant,
            value,
            err: PhantomData,
        }
    }
}

impl<'de, E> EnumAccess<'de> for EnumDeserializer<'de, E>
where
    E: DeError,
{
    type Error = E;
    type Variant = VariantDeserializer<'de, Self::Error>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), E>
    where
        V: DeserializeSeed<'de>,
    {
        let visitor = VariantDeserializer {
            is_human_readable: self.is_human_readable,
            value: self.value,
            err: PhantomData,
        };
        seed.deserialize(ContentDeserializer::new(
            self.variant,
            self.is_human_readable,
        ))
        .map(|v| (v, visitor))
    }
}

pub struct VariantDeserializer<'de, E>
where
    E: DeError,
{
    is_human_readable: bool,
    value: Option<Content<'de>>,
    err: PhantomData<E>,
}

impl<'de, E> VariantAccess<'de> for VariantDeserializer<'de, E>
where
    E: DeError,
{
    type Error = E;

    fn unit_variant(self) -> Result<(), E> {
        match self.value {
            Some(value) => {
                Deserialize::deserialize(ContentDeserializer::new(value, self.is_human_readable))
            }
            None => Ok(()),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, E>
    where
        T: DeserializeSeed<'de>,
    {
        match self.value {
            Some(value) => {
                seed.deserialize(ContentDeserializer::new(value, self.is_human_readable))
            }
            None => Err(DeError::invalid_type(
                Unexpected::UnitVariant,
                &"newtype variant",
            )),
        }
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Some(Content::Seq(v)) => Deserializer::deserialize_any(
                SeqDeserializer::new(v, self.is_human_readable),
                visitor,
            ),
            Some(other) => {
                let mut buf = [0; 58];
                Err(DeError::invalid_type(
                    other.unexpected(&mut buf),
                    &"tuple variant",
                ))
            }
            None => Err(DeError::invalid_type(
                Unexpected::UnitVariant,
                &"tuple variant",
            )),
        }
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Some(Content::Map(v)) => Deserializer::deserialize_any(
                MapDeserializer::new(v, self.is_human_readable),
                visitor,
            ),
            Some(Content::Seq(v)) => Deserializer::deserialize_any(
                SeqDeserializer::new(v, self.is_human_readable),
                visitor,
            ),
            Some(other) => {
                let mut buf = [0; 58];
                Err(DeError::invalid_type(
                    other.unexpected(&mut buf),
                    &"struct variant",
                ))
            }
            None => Err(DeError::invalid_type(
                Unexpected::UnitVariant,
                &"struct variant",
            )),
        }
    }
}

struct SeqDeserializer<'de, E>
where
    E: DeError,
{
    is_human_readable: bool,
    iter: <Vec<Content<'de>> as IntoIterator>::IntoIter,
    err: PhantomData<E>,
}

impl<'de, E> SeqDeserializer<'de, E>
where
    E: DeError,
{
    fn new(vec: Vec<Content<'de>>, is_human_readable: bool) -> Self {
        SeqDeserializer {
            is_human_readable,
            iter: vec.into_iter(),
            err: PhantomData,
        }
    }
}

impl<'de, E> Deserializer<'de> for SeqDeserializer<'de, E>
where
    E: DeError,
{
    type Error = E;

    #[inline]
    fn is_human_readable(&self) -> bool {
        self.is_human_readable
    }

    #[inline]
    fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let len = self.iter.len();
        if len == 0 {
            visitor.visit_unit()
        } else {
            let ret = visitor.visit_seq(&mut self)?;
            let remaining = self.iter.len();
            if remaining == 0 {
                Ok(ret)
            } else {
                Err(DeError::invalid_length(len, &"fewer elements in array"))
            }
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de, E> SeqAccess<'de> for SeqDeserializer<'de, E>
where
    E: DeError,
{
    type Error = E;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(value) => seed
                .deserialize(ContentDeserializer::new(value, self.is_human_readable))
                .map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        size_hint_from_bounds(&self.iter)
    }
}

struct MapDeserializer<'de, E>
where
    E: DeError,
{
    is_human_readable: bool,
    iter: <Vec<(Content<'de>, Content<'de>)> as IntoIterator>::IntoIter,
    value: Option<Content<'de>>,
    err: PhantomData<E>,
}

impl<'de, E> MapDeserializer<'de, E>
where
    E: DeError,
{
    fn new(map: Vec<(Content<'de>, Content<'de>)>, is_human_readable: bool) -> Self {
        MapDeserializer {
            is_human_readable,
            iter: map.into_iter(),
            value: None,
            err: PhantomData,
        }
    }
}

impl<'de, E> MapAccess<'de> for MapDeserializer<'de, E>
where
    E: DeError,
{
    type Error = E;

    fn next_key_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(ContentDeserializer::new(key, self.is_human_readable))
                    .map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<T>(&mut self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.value.take() {
            Some(value) => {
                seed.deserialize(ContentDeserializer::new(value, self.is_human_readable))
            }
            None => Err(DeError::custom("value is missing")),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        size_hint_from_bounds(&self.iter)
    }
}

impl<'de, E> Deserializer<'de> for MapDeserializer<'de, E>
where
    E: DeError,
{
    type Error = E;

    #[inline]
    fn is_human_readable(&self) -> bool {
        self.is_human_readable
    }

    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(self)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

/// Not public API.
pub struct ContentRefDeserializer<'a, 'de, E> {
    is_human_readable: bool,
    content: &'a Content<'de>,
    err: PhantomData<E>,
}

impl<'de, E> ContentRefDeserializer<'_, 'de, E>
where
    E: DeError,
{
    #[cold]
    fn invalid_type(self, exp: &dyn Expected) -> E {
        let mut buf = [0; 58];
        DeError::invalid_type(self.content.unexpected(&mut buf), exp)
    }

    fn deserialize_integer<V>(self, visitor: V) -> Result<V::Value, E>
    where
        V: Visitor<'de>,
    {
        match *self.content {
            Content::U8(v) => visitor.visit_u8(v),
            Content::U16(v) => visitor.visit_u16(v),
            Content::U32(v) => visitor.visit_u32(v),
            Content::U64(v) => visitor.visit_u64(v),
            Content::U128(v) => visitor.visit_u128(v),
            Content::I8(v) => visitor.visit_i8(v),
            Content::I16(v) => visitor.visit_i16(v),
            Content::I32(v) => visitor.visit_i32(v),
            Content::I64(v) => visitor.visit_i64(v),
            Content::I128(v) => visitor.visit_i128(v),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_float<V>(self, visitor: V) -> Result<V::Value, E>
    where
        V: Visitor<'de>,
    {
        match *self.content {
            Content::F32(v) => visitor.visit_f32(v),
            Content::F64(v) => visitor.visit_f64(v),
            Content::U8(v) => visitor.visit_u8(v),
            Content::U16(v) => visitor.visit_u16(v),
            Content::U32(v) => visitor.visit_u32(v),
            Content::U64(v) => visitor.visit_u64(v),
            Content::U128(v) => visitor.visit_u128(v),
            Content::I8(v) => visitor.visit_i8(v),
            Content::I16(v) => visitor.visit_i16(v),
            Content::I32(v) => visitor.visit_i32(v),
            Content::I64(v) => visitor.visit_i64(v),
            Content::I128(v) => visitor.visit_i128(v),
            _ => Err(self.invalid_type(&visitor)),
        }
    }
}

fn visit_content_seq_ref<'a, 'de, V, E>(
    content: &'a [Content<'de>],
    visitor: V,
    is_human_readable: bool,
) -> Result<V::Value, E>
where
    V: Visitor<'de>,
    E: DeError,
{
    let seq = content
        .iter()
        .map(|x| ContentRefDeserializer::new(x, is_human_readable));
    let mut seq_visitor = serde::de::value::SeqDeserializer::new(seq);
    let value = visitor.visit_seq(&mut seq_visitor)?;
    seq_visitor.end()?;
    Ok(value)
}

fn visit_content_map_ref<'a, 'de, V, E>(
    content: &'a [(Content<'de>, Content<'de>)],
    visitor: V,
    is_human_readable: bool,
) -> Result<V::Value, E>
where
    V: Visitor<'de>,
    E: DeError,
{
    let map = content.iter().map(|(k, v)| {
        (
            ContentRefDeserializer::new(k, is_human_readable),
            ContentRefDeserializer::new(v, is_human_readable),
        )
    });
    let mut map_visitor = serde::de::value::MapDeserializer::new(map);
    let value = visitor.visit_map(&mut map_visitor)?;
    map_visitor.end()?;
    Ok(value)
}

/// Used when deserializing an untagged enum because the content may need
/// to be used more than once.
impl<'de, E> Deserializer<'de> for ContentRefDeserializer<'_, 'de, E>
where
    E: DeError,
{
    type Error = E;

    #[inline]
    fn is_human_readable(&self) -> bool {
        self.is_human_readable
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, E>
    where
        V: Visitor<'de>,
    {
        match *self.content {
            Content::Bool(v) => visitor.visit_bool(v),
            Content::U8(v) => visitor.visit_u8(v),
            Content::U16(v) => visitor.visit_u16(v),
            Content::U32(v) => visitor.visit_u32(v),
            Content::U64(v) => visitor.visit_u64(v),
            Content::U128(v) => visitor.visit_u128(v),
            Content::I8(v) => visitor.visit_i8(v),
            Content::I16(v) => visitor.visit_i16(v),
            Content::I32(v) => visitor.visit_i32(v),
            Content::I64(v) => visitor.visit_i64(v),
            Content::I128(v) => visitor.visit_i128(v),
            Content::F32(v) => visitor.visit_f32(v),
            Content::F64(v) => visitor.visit_f64(v),
            Content::Char(v) => visitor.visit_char(v),
            Content::String(ref v) => visitor.visit_str(v),
            Content::Str(v) => visitor.visit_borrowed_str(v),
            Content::ByteBuf(ref v) => visitor.visit_bytes(v),
            Content::Bytes(v) => visitor.visit_borrowed_bytes(v),
            Content::Unit => visitor.visit_unit(),
            Content::None => visitor.visit_none(),
            Content::Some(ref v) => {
                visitor.visit_some(ContentRefDeserializer::new(v, self.is_human_readable))
            }
            Content::Newtype(ref v) => {
                visitor.visit_newtype_struct(ContentRefDeserializer::new(v, self.is_human_readable))
            }
            Content::Seq(ref v) => visit_content_seq_ref(v, visitor, self.is_human_readable),
            Content::Map(ref v) => visit_content_map_ref(v, visitor, self.is_human_readable),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.content {
            Content::Bool(v) => visitor.visit_bool(v),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_i128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_u128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_integer(visitor)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_float(visitor)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_float(visitor)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.content {
            Content::Char(v) => visitor.visit_char(v),
            Content::String(ref v) => visitor.visit_str(v),
            Content::Str(v) => visitor.visit_borrowed_str(v),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.content {
            Content::String(ref v) => visitor.visit_str(v),
            Content::Str(v) => visitor.visit_borrowed_str(v),
            Content::ByteBuf(ref v) => visitor.visit_bytes(v),
            Content::Bytes(v) => visitor.visit_borrowed_bytes(v),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.content {
            Content::String(ref v) => visitor.visit_str(v),
            Content::Str(v) => visitor.visit_borrowed_str(v),
            Content::ByteBuf(ref v) => visitor.visit_bytes(v),
            Content::Bytes(v) => visitor.visit_borrowed_bytes(v),
            Content::Seq(ref v) => visit_content_seq_ref(v, visitor, self.is_human_readable),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, E>
    where
        V: Visitor<'de>,
    {
        match *self.content {
            Content::None => visitor.visit_none(),
            Content::Some(ref v) => {
                visitor.visit_some(ContentRefDeserializer::new(v, self.is_human_readable))
            }
            Content::Unit => visitor.visit_unit(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.content {
            Content::Unit => visitor.visit_unit(),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(self, _name: &str, visitor: V) -> Result<V::Value, E>
    where
        V: Visitor<'de>,
    {
        match *self.content {
            Content::Newtype(ref v) => {
                visitor.visit_newtype_struct(ContentRefDeserializer::new(v, self.is_human_readable))
            }
            _ => visitor.visit_newtype_struct(self),
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.content {
            Content::Seq(ref v) => visit_content_seq_ref(v, visitor, self.is_human_readable),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.content {
            Content::Map(ref v) => visit_content_map_ref(v, visitor, self.is_human_readable),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.content {
            Content::Seq(ref v) => visit_content_seq_ref(v, visitor, self.is_human_readable),
            Content::Map(ref v) => visit_content_map_ref(v, visitor, self.is_human_readable),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_enum<V>(
        self,
        _name: &str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let (variant, value) = match *self.content {
            Content::Map(ref value) => {
                let mut iter = value.iter();
                let (variant, value) = match iter.next() {
                    Some(v) => v,
                    None => {
                        return Err(DeError::invalid_value(
                            Unexpected::Map,
                            &"map with a single key",
                        ));
                    }
                };
                // enums are encoded in json as maps with a single key:value pair
                if iter.next().is_some() {
                    return Err(DeError::invalid_value(
                        Unexpected::Map,
                        &"map with a single key",
                    ));
                }
                (variant, Some(value))
            }
            ref s @ Content::String(_) | ref s @ Content::Str(_) => (s, None),
            ref other => {
                let mut buf = [0; 58];
                return Err(DeError::invalid_type(
                    other.unexpected(&mut buf),
                    &"string or map",
                ));
            }
        };

        visitor.visit_enum(EnumRefDeserializer {
            is_human_readable: self.is_human_readable,
            variant,
            value,
            err: PhantomData,
        })
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.content {
            Content::String(ref v) => visitor.visit_str(v),
            Content::Str(v) => visitor.visit_borrowed_str(v),
            Content::ByteBuf(ref v) => visitor.visit_bytes(v),
            Content::Bytes(v) => visitor.visit_borrowed_bytes(v),
            Content::U8(v) => visitor.visit_u8(v),
            Content::U64(v) => visitor.visit_u64(v),
            _ => Err(self.invalid_type(&visitor)),
        }
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

impl<'a, 'de, E> ContentRefDeserializer<'a, 'de, E> {
    /// private API, don't use
    pub(crate) fn new(content: &'a Content<'de>, is_human_readable: bool) -> Self {
        ContentRefDeserializer {
            is_human_readable,
            content,
            err: PhantomData,
        }
    }
}

struct EnumRefDeserializer<'a, 'de, E>
where
    E: DeError,
{
    is_human_readable: bool,
    variant: &'a Content<'de>,
    value: Option<&'a Content<'de>>,
    err: PhantomData<E>,
}

impl<'de, 'a, E> EnumAccess<'de> for EnumRefDeserializer<'a, 'de, E>
where
    E: DeError,
{
    type Error = E;
    type Variant = VariantRefDeserializer<'a, 'de, Self::Error>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let visitor = VariantRefDeserializer {
            is_human_readable: self.is_human_readable,
            value: self.value,
            err: PhantomData,
        };
        seed.deserialize(ContentRefDeserializer::new(
            self.variant,
            self.is_human_readable,
        ))
        .map(|v| (v, visitor))
    }
}

struct VariantRefDeserializer<'a, 'de, E>
where
    E: DeError,
{
    is_human_readable: bool,
    value: Option<&'a Content<'de>>,
    err: PhantomData<E>,
}

impl<'de, E> VariantAccess<'de> for VariantRefDeserializer<'_, 'de, E>
where
    E: DeError,
{
    type Error = E;

    fn unit_variant(self) -> Result<(), E> {
        match self.value {
            Some(value) => {
                Deserialize::deserialize(ContentRefDeserializer::new(value, self.is_human_readable))
            }
            None => Ok(()),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, E>
    where
        T: DeserializeSeed<'de>,
    {
        match self.value {
            Some(value) => {
                seed.deserialize(ContentRefDeserializer::new(value, self.is_human_readable))
            }
            None => Err(DeError::invalid_type(
                Unexpected::UnitVariant,
                &"newtype variant",
            )),
        }
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Some(Content::Seq(v)) => Deserializer::deserialize_any(
                SeqRefDeserializer::new(v, self.is_human_readable),
                visitor,
            ),
            Some(other) => {
                let mut buf = [0; 58];
                Err(DeError::invalid_type(
                    other.unexpected(&mut buf),
                    &"tuple variant",
                ))
            }
            None => Err(DeError::invalid_type(
                Unexpected::UnitVariant,
                &"tuple variant",
            )),
        }
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Some(Content::Map(v)) => Deserializer::deserialize_any(
                MapRefDeserializer::new(v, self.is_human_readable),
                visitor,
            ),
            Some(Content::Seq(v)) => Deserializer::deserialize_any(
                SeqRefDeserializer::new(v, self.is_human_readable),
                visitor,
            ),
            Some(other) => {
                let mut buf = [0; 58];
                Err(DeError::invalid_type(
                    other.unexpected(&mut buf),
                    &"struct variant",
                ))
            }
            None => Err(DeError::invalid_type(
                Unexpected::UnitVariant,
                &"struct variant",
            )),
        }
    }
}

struct SeqRefDeserializer<'a, 'de, E>
where
    E: DeError,
{
    is_human_readable: bool,
    iter: <&'a [Content<'de>] as IntoIterator>::IntoIter,
    err: PhantomData<E>,
}

impl<'a, 'de, E> SeqRefDeserializer<'a, 'de, E>
where
    E: DeError,
{
    fn new(slice: &'a [Content<'de>], is_human_readable: bool) -> Self {
        SeqRefDeserializer {
            is_human_readable,
            iter: slice.iter(),
            err: PhantomData,
        }
    }
}

impl<'de, E> Deserializer<'de> for SeqRefDeserializer<'_, 'de, E>
where
    E: DeError,
{
    type Error = E;

    #[inline]
    fn is_human_readable(&self) -> bool {
        self.is_human_readable
    }

    #[inline]
    fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let len = self.iter.len();
        if len == 0 {
            visitor.visit_unit()
        } else {
            let ret = visitor.visit_seq(&mut self)?;
            let remaining = self.iter.len();
            if remaining == 0 {
                Ok(ret)
            } else {
                Err(DeError::invalid_length(len, &"fewer elements in array"))
            }
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de, E> SeqAccess<'de> for SeqRefDeserializer<'_, 'de, E>
where
    E: DeError,
{
    type Error = E;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(value) => seed
                .deserialize(ContentRefDeserializer::new(value, self.is_human_readable))
                .map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        size_hint_from_bounds(&self.iter)
    }
}

struct MapRefDeserializer<'a, 'de, E>
where
    E: DeError,
{
    is_human_readable: bool,
    iter: <&'a [(Content<'de>, Content<'de>)] as IntoIterator>::IntoIter,
    value: Option<&'a Content<'de>>,
    err: PhantomData<E>,
}

impl<'a, 'de, E> MapRefDeserializer<'a, 'de, E>
where
    E: DeError,
{
    fn new(map: &'a [(Content<'de>, Content<'de>)], is_human_readable: bool) -> Self {
        MapRefDeserializer {
            is_human_readable,
            iter: map.iter(),
            value: None,
            err: PhantomData,
        }
    }
}

impl<'de, E> MapAccess<'de> for MapRefDeserializer<'_, 'de, E>
where
    E: DeError,
{
    type Error = E;

    fn next_key_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(ContentRefDeserializer::new(key, self.is_human_readable))
                    .map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<T>(&mut self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.value.take() {
            Some(value) => {
                seed.deserialize(ContentRefDeserializer::new(value, self.is_human_readable))
            }
            None => Err(DeError::custom("value is missing")),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        size_hint_from_bounds(&self.iter)
    }
}

impl<'de, E> Deserializer<'de> for MapRefDeserializer<'_, 'de, E>
where
    E: DeError,
{
    type Error = E;

    #[inline]
    fn is_human_readable(&self) -> bool {
        self.is_human_readable
    }

    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(self)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de, E> IntoDeserializer<'de, E> for ContentDeserializer<'de, E>
where
    E: DeError,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

impl<'de, E> IntoDeserializer<'de, E> for ContentRefDeserializer<'_, 'de, E>
where
    E: DeError,
{
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}
