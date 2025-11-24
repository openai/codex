use crate::lib::*;

use crate::de::value::{BorrowedBytesDeserializer, BytesDeserializer};
use crate::de::{
    Deserialize, DeserializeSeed, Deserializer, EnumAccess, Error, IntoDeserializer, VariantAccess,
    Visitor,
};

#[cfg(any(feature = "std", feature = "alloc"))]
use crate::de::{MapAccess, Unexpected};

#[cfg(any(feature = "std", feature = "alloc"))]
pub use self::content::{
    content_as_str, Content, ContentDeserializer, ContentRefDeserializer, ContentVisitor,
    EnumDeserializer, InternallyTaggedUnitVisitor, TagContentOtherField,
    TagContentOtherFieldVisitor, TagOrContentField, TagOrContentFieldVisitor, TaggedContentVisitor,
    UntaggedUnitVisitor,
};

pub use crate::serde_core_private::InPlaceSeed;

/// If the missing field is of type `Option<T>` then treat is as `None`,
/// otherwise it is an error.
pub fn missing_field<'de, V, E>(field: &'static str) -> Result<V, E>
where
    V: Deserialize<'de>,
    E: Error,
{
    struct MissingFieldDeserializer<E>(&'static str, PhantomData<E>);

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, E> Deserializer<'de> for MissingFieldDeserializer<E>
    where
        E: Error,
    {
        type Error = E;

        fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, E>
        where
            V: Visitor<'de>,
        {
            Err(Error::missing_field(self.0))
        }

        fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, E>
        where
            V: Visitor<'de>,
        {
            visitor.visit_none()
        }

        serde_core::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
            bytes byte_buf unit unit_struct newtype_struct seq tuple
            tuple_struct map struct enum identifier ignored_any
        }
    }

    let deserializer = MissingFieldDeserializer(field, PhantomData);
    Deserialize::deserialize(deserializer)
}

#[cfg(any(feature = "std", feature = "alloc"))]
pub fn borrow_cow_str<'de: 'a, 'a, D, R>(deserializer: D) -> Result<R, D::Error>
where
    D: Deserializer<'de>,
    R: From<Cow<'a, str>>,
{
    struct CowStrVisitor;

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'a> Visitor<'a> for CowStrVisitor {
        type Value = Cow<'a, str>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Owned(v.to_owned()))
        }

        fn visit_borrowed_str<E>(self, v: &'a str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Borrowed(v))
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Owned(v))
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: Error,
        {
            match str::from_utf8(v) {
                Ok(s) => Ok(Cow::Owned(s.to_owned())),
                Err(_) => Err(Error::invalid_value(Unexpected::Bytes(v), &self)),
            }
        }

        fn visit_borrowed_bytes<E>(self, v: &'a [u8]) -> Result<Self::Value, E>
        where
            E: Error,
        {
            match str::from_utf8(v) {
                Ok(s) => Ok(Cow::Borrowed(s)),
                Err(_) => Err(Error::invalid_value(Unexpected::Bytes(v), &self)),
            }
        }

        fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
        where
            E: Error,
        {
            match String::from_utf8(v) {
                Ok(s) => Ok(Cow::Owned(s)),
                Err(e) => Err(Error::invalid_value(
                    Unexpected::Bytes(&e.into_bytes()),
                    &self,
                )),
            }
        }
    }

    deserializer.deserialize_str(CowStrVisitor).map(From::from)
}

#[cfg(any(feature = "std", feature = "alloc"))]
pub fn borrow_cow_bytes<'de: 'a, 'a, D, R>(deserializer: D) -> Result<R, D::Error>
where
    D: Deserializer<'de>,
    R: From<Cow<'a, [u8]>>,
{
    struct CowBytesVisitor;

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'a> Visitor<'a> for CowBytesVisitor {
        type Value = Cow<'a, [u8]>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a byte array")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Owned(v.as_bytes().to_vec()))
        }

        fn visit_borrowed_str<E>(self, v: &'a str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Borrowed(v.as_bytes()))
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Owned(v.into_bytes()))
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Owned(v.to_vec()))
        }

        fn visit_borrowed_bytes<E>(self, v: &'a [u8]) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Borrowed(v))
        }

        fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Owned(v))
        }
    }

    deserializer
        .deserialize_bytes(CowBytesVisitor)
        .map(From::from)
}

#[cfg(any(feature = "std", feature = "alloc"))]
mod content {
    // This module is private and nothing here should be used outside of
    // generated code.
    //
    // We will iterate on the implementation for a few releases and only have to
    // worry about backward compatibility for the `untagged` and `tag` attributes
    // rather than for this entire mechanism.
    //
    // This issue is tracking making some of this stuff public:
    // https://github.com/serde-rs/serde/issues/741

    use crate::lib::*;

    use crate::de::{
        self, Deserialize, DeserializeSeed, Deserializer, EnumAccess, Expected, IgnoredAny,
        MapAccess, SeqAccess, Unexpected, Visitor,
    };
    use crate::serde_core_private::size_hint;
    pub use crate::serde_core_private::Content;

    pub fn content_as_str<'a, 'de>(content: &'a Content<'de>) -> Option<&'a str> {
        match *content {
            Content::Str(x) => Some(x),
            Content::String(ref x) => Some(x),
            Content::Bytes(x) => str::from_utf8(x).ok(),
            Content::ByteBuf(ref x) => str::from_utf8(x).ok(),
            _ => None,
        }
    }

    fn content_clone<'de>(content: &Content<'de>) -> Content<'de> {
        match content {
            Content::Bool(b) => Content::Bool(*b),
            Content::U8(n) => Content::U8(*n),
            Content::U16(n) => Content::U16(*n),
            Content::U32(n) => Content::U32(*n),
            Content::U64(n) => Content::U64(*n),
            Content::I8(n) => Content::I8(*n),
            Content::I16(n) => Content::I16(*n),
            Content::I32(n) => Content::I32(*n),
            Content::I64(n) => Content::I64(*n),
            Content::F32(f) => Content::F32(*f),
            Content::F64(f) => Content::F64(*f),
            Content::Char(c) => Content::Char(*c),
            Content::String(s) => Content::String(s.clone()),
            Content::Str(s) => Content::Str(*s),
            Content::ByteBuf(b) => Content::ByteBuf(b.clone()),
            Content::Bytes(b) => Content::Bytes(b),
            Content::None => Content::None,
            Content::Some(content) => Content::Some(Box::new(content_clone(content))),
            Content::Unit => Content::Unit,
            Content::Newtype(content) => Content::Newtype(Box::new(content_clone(content))),
            Content::Seq(seq) => Content::Seq(seq.iter().map(content_clone).collect()),
            Content::Map(map) => Content::Map(
                map.iter()
                    .map(|(k, v)| (content_clone(k), content_clone(v)))
                    .collect(),
            ),
        }
    }

    #[cold]
    fn content_unexpected<'a, 'de>(content: &'a Content<'de>) -> Unexpected<'a> {
        match *content {
            Content::Bool(b) => Unexpected::Bool(b),
            Content::U8(n) => Unexpected::Unsigned(n as u64),
            Content::U16(n) => Unexpected::Unsigned(n as u64),
            Content::U32(n) => Unexpected::Unsigned(n as u64),
            Content::U64(n) => Unexpected::Unsigned(n),
            Content::I8(n) => Unexpected::Signed(n as i64),
            Content::I16(n) => Unexpected::Signed(n as i64),
            Content::I32(n) => Unexpected::Signed(n as i64),
            Content::I64(n) => Unexpected::Signed(n),
            Content::F32(f) => Unexpected::Float(f as f64),
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

    pub struct ContentVisitor<'de> {
        value: PhantomData<Content<'de>>,
    }

    impl<'de> ContentVisitor<'de> {
        pub fn new() -> Self {
            ContentVisitor { value: PhantomData }
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de> DeserializeSeed<'de> for ContentVisitor<'de> {
        type Value = Content<'de>;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.__deserialize_content_v1(self)
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de> Visitor<'de> for ContentVisitor<'de> {
        type Value = Content<'de>;

        fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
            fmt.write_str("any value")
        }

        fn visit_bool<F>(self, value: bool) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::Bool(value))
        }

        fn visit_i8<F>(self, value: i8) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::I8(value))
        }

        fn visit_i16<F>(self, value: i16) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::I16(value))
        }

        fn visit_i32<F>(self, value: i32) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::I32(value))
        }

        fn visit_i64<F>(self, value: i64) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::I64(value))
        }

        fn visit_u8<F>(self, value: u8) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::U8(value))
        }

        fn visit_u16<F>(self, value: u16) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::U16(value))
        }

        fn visit_u32<F>(self, value: u32) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::U32(value))
        }

        fn visit_u64<F>(self, value: u64) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::U64(value))
        }

        fn visit_f32<F>(self, value: f32) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::F32(value))
        }

        fn visit_f64<F>(self, value: f64) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::F64(value))
        }

        fn visit_char<F>(self, value: char) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::Char(value))
        }

        fn visit_str<F>(self, value: &str) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::String(value.into()))
        }

        fn visit_borrowed_str<F>(self, value: &'de str) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::Str(value))
        }

        fn visit_string<F>(self, value: String) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::String(value))
        }

        fn visit_bytes<F>(self, value: &[u8]) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::ByteBuf(value.into()))
        }

        fn visit_borrowed_bytes<F>(self, value: &'de [u8]) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::Bytes(value))
        }

        fn visit_byte_buf<F>(self, value: Vec<u8>) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::ByteBuf(value))
        }

        fn visit_unit<F>(self) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::Unit)
        }

        fn visit_none<F>(self) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            Ok(Content::None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            let v = tri!(ContentVisitor::new().deserialize(deserializer));
            Ok(Content::Some(Box::new(v)))
        }

        fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            let v = tri!(ContentVisitor::new().deserialize(deserializer));
            Ok(Content::Newtype(Box::new(v)))
        }

        fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
        where
            V: SeqAccess<'de>,
        {
            let mut vec =
                Vec::<Content>::with_capacity(size_hint::cautious::<Content>(visitor.size_hint()));
            while let Some(e) = tri!(visitor.next_element_seed(ContentVisitor::new())) {
                vec.push(e);
            }
            Ok(Content::Seq(vec))
        }

        fn visit_map<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
        where
            V: MapAccess<'de>,
        {
            let mut vec =
                Vec::<(Content, Content)>::with_capacity(
                    size_hint::cautious::<(Content, Content)>(visitor.size_hint()),
                );
            while let Some(kv) =
                tri!(visitor.next_entry_seed(ContentVisitor::new(), ContentVisitor::new()))
            {
                vec.push(kv);
            }
            Ok(Content::Map(vec))
        }

        fn visit_enum<V>(self, _visitor: V) -> Result<Self::Value, V::Error>
        where
            V: EnumAccess<'de>,
        {
            Err(de::Error::custom(
                "untagged and internally tagged enums do not support enum input",
            ))
        }
    }

    /// This is the type of the map keys in an internally tagged enum.
    ///
    /// Not public API.
    pub enum TagOrContent<'de> {
        Tag,
        Content(Content<'de>),
    }

    /// Serves as a seed for deserializing a key of internally tagged enum.
    /// Cannot capture externally tagged enums, `i128` and `u128`.
    struct TagOrContentVisitor<'de> {
        name: &'static str,
        value: PhantomData<TagOrContent<'de>>,
    }

    impl<'de> TagOrContentVisitor<'de> {
        fn new(name: &'static str) -> Self {
            TagOrContentVisitor {
                name,
                value: PhantomData,
            }
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de> DeserializeSeed<'de> for TagOrContentVisitor<'de> {
        type Value = TagOrContent<'de>;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            // Internally tagged enums are only supported in self-describing
            // formats.
            deserializer.deserialize_any(self)
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de> Visitor<'de> for TagOrContentVisitor<'de> {
        type Value = TagOrContent<'de>;

        fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
            write!(fmt, "a type tag `{}` or any other value", self.name)
        }

        fn visit_bool<F>(self, value: bool) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            ContentVisitor::new()
                .visit_bool(value)
                .map(TagOrContent::Content)
        }

        fn visit_i8<F>(self, value: i8) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            ContentVisitor::new()
                .visit_i8(value)
                .map(TagOrContent::Content)
        }

        fn visit_i16<F>(self, value: i16) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            ContentVisitor::new()
                .visit_i16(value)
                .map(TagOrContent::Content)
        }

        fn visit_i32<F>(self, value: i32) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            ContentVisitor::new()
                .visit_i32(value)
                .map(TagOrContent::Content)
        }

        fn visit_i64<F>(self, value: i64) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            ContentVisitor::new()
                .visit_i64(value)
                .map(TagOrContent::Content)
        }

        fn visit_u8<F>(self, value: u8) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            ContentVisitor::new()
                .visit_u8(value)
                .map(TagOrContent::Content)
        }

        fn visit_u16<F>(self, value: u16) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            ContentVisitor::new()
                .visit_u16(value)
                .map(TagOrContent::Content)
        }

        fn visit_u32<F>(self, value: u32) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            ContentVisitor::new()
                .visit_u32(value)
                .map(TagOrContent::Content)
        }

        fn visit_u64<F>(self, value: u64) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            ContentVisitor::new()
                .visit_u64(value)
                .map(TagOrContent::Content)
        }

        fn visit_f32<F>(self, value: f32) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            ContentVisitor::new()
                .visit_f32(value)
                .map(TagOrContent::Content)
        }

        fn visit_f64<F>(self, value: f64) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            ContentVisitor::new()
                .visit_f64(value)
                .map(TagOrContent::Content)
        }

        fn visit_char<F>(self, value: char) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            ContentVisitor::new()
                .visit_char(value)
                .map(TagOrContent::Content)
        }

        fn visit_str<F>(self, value: &str) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            if value == self.name {
                Ok(TagOrContent::Tag)
            } else {
                ContentVisitor::new()
                    .visit_str(value)
                    .map(TagOrContent::Content)
            }
        }

        fn visit_borrowed_str<F>(self, value: &'de str) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            if value == self.name {
                Ok(TagOrContent::Tag)
            } else {
                ContentVisitor::new()
                    .visit_borrowed_str(value)
                    .map(TagOrContent::Content)
            }
        }

        fn visit_string<F>(self, value: String) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            if value == self.name {
                Ok(TagOrContent::Tag)
            } else {
                ContentVisitor::new()
                    .visit_string(value)
                    .map(TagOrContent::Content)
            }
        }

        fn visit_bytes<F>(self, value: &[u8]) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            if value == self.name.as_bytes() {
                Ok(TagOrContent::Tag)
            } else {
                ContentVisitor::new()
                    .visit_bytes(value)
                    .map(TagOrContent::Content)
            }
        }

        fn visit_borrowed_bytes<F>(self, value: &'de [u8]) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            if value == self.name.as_bytes() {
                Ok(TagOrContent::Tag)
            } else {
                ContentVisitor::new()
                    .visit_borrowed_bytes(value)
                    .map(TagOrContent::Content)
            }
        }

        fn visit_byte_buf<F>(self, value: Vec<u8>) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            if value == self.name.as_bytes() {
                Ok(TagOrContent::Tag)
            } else {
                ContentVisitor::new()
                    .visit_byte_buf(value)
                    .map(TagOrContent::Content)
            }
        }

        fn visit_unit<F>(self) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            ContentVisitor::new()
                .visit_unit()
                .map(TagOrContent::Content)
        }

        fn visit_none<F>(self) -> Result<Self::Value, F>
        where
            F: de::Error,
        {
            ContentVisitor::new()
                .visit_none()
                .map(TagOrContent::Content)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            ContentVisitor::new()
                .visit_some(deserializer)
                .map(TagOrContent::Content)
        }

        fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            ContentVisitor::new()
                .visit_newtype_struct(deserializer)
                .map(TagOrContent::Content)
        }

        fn visit_seq<V>(self, visitor: V) -> Result<Self::Value, V::Error>
        where
            V: SeqAccess<'de>,
        {
            ContentVisitor::new()
                .visit_seq(visitor)
                .map(TagOrContent::Content)
        }

        fn visit_map<V>(self, visitor: V) -> Result<Self::Value, V::Error>
        where
            V: MapAccess<'de>,
        {
            ContentVisitor::new()
                .visit_map(visitor)
                .map(TagOrContent::Content)
        }

        fn visit_enum<V>(self, visitor: V) -> Result<Self::Value, V::Error>
        where
            V: EnumAccess<'de>,
        {
            ContentVisitor::new()
                .visit_enum(visitor)
                .map(TagOrContent::Content)
        }
    }

    /// Used by generated code to deserialize an internally tagged enum.
    ///
    /// Captures map or sequence from the original deserializer and searches
    /// a tag in it (in case of sequence, tag is the first element of sequence).
    ///
    /// Not public API.
    pub struct TaggedContentVisitor<T> {
        tag_name: &'static str,
        expecting: &'static str,
        value: PhantomData<T>,
    }

    impl<T> TaggedContentVisitor<T> {
        /// Visitor for the content of an internally tagged enum with the given
        /// tag name.
        pub fn new(name: &'static str, expecting: &'static str) -> Self {
            TaggedContentVisitor {
                tag_name: name,
                expecting,
                value: PhantomData,
            }
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, T> Visitor<'de> for TaggedContentVisitor<T>
    where
        T: Deserialize<'de>,
    {
        type Value = (T, Content<'de>);

        fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
            fmt.write_str(self.expecting)
        }

        fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
        where
            S: SeqAccess<'de>,
        {
            let tag = match tri!(seq.next_element()) {
                Some(tag) => tag,
                None => {
                    return Err(de::Error::missing_field(self.tag_name));
                }
            };
            let rest = de::value::SeqAccessDeserializer::new(seq);
            Ok((tag, tri!(ContentVisitor::new().deserialize(rest))))
        }

        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let mut tag = None;
            let mut vec = Vec::<(Content, Content)>::with_capacity(size_hint::cautious::<(
                Content,
                Content,
            )>(map.size_hint()));
            while let Some(k) = tri!(map.next_key_seed(TagOrContentVisitor::new(self.tag_name))) {
                match k {
                    TagOrContent::Tag => {
                        if tag.is_some() {
                            return Err(de::Error::duplicate_field(self.tag_name));
                        }
                        tag = Some(tri!(map.next_value()));
                    }
                    TagOrContent::Content(k) => {
                        let v = tri!(map.next_value_seed(ContentVisitor::new()));
                        vec.push((k, v));
                    }
                }
            }
            match tag {
                None => Err(de::Error::missing_field(self.tag_name)),
                Some(tag) => Ok((tag, Content::Map(vec))),
            }
        }
    }

    /// Used by generated code to deserialize an adjacently tagged enum.
    ///
    /// Not public API.
    pub enum TagOrContentField {
        Tag,
        Content,
    }

    /// Not public API.
    pub struct TagOrContentFieldVisitor {
        /// Name of the tag field of the adjacently tagged enum
        pub tag: &'static str,
        /// Name of the content field of the adjacently tagged enum
        pub content: &'static str,
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de> DeserializeSeed<'de> for TagOrContentFieldVisitor {
        type Value = TagOrContentField;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_identifier(self)
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de> Visitor<'de> for TagOrContentFieldVisitor {
        type Value = TagOrContentField;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "{:?} or {:?}", self.tag, self.content)
        }

        fn visit_u64<E>(self, field_index: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match field_index {
                0 => Ok(TagOrContentField::Tag),
                1 => Ok(TagOrContentField::Content),
                _ => Err(de::Error::invalid_value(
                    Unexpected::Unsigned(field_index),
                    &self,
                )),
            }
        }

        fn visit_str<E>(self, field: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if field == self.tag {
                Ok(TagOrContentField::Tag)
            } else if field == self.content {
                Ok(TagOrContentField::Content)
            } else {
                Err(de::Error::invalid_value(Unexpected::Str(field), &self))
            }
        }

        fn visit_bytes<E>(self, field: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if field == self.tag.as_bytes() {
                Ok(TagOrContentField::Tag)
            } else if field == self.content.as_bytes() {
                Ok(TagOrContentField::Content)
            } else {
                Err(de::Error::invalid_value(Unexpected::Bytes(field), &self))
            }
        }
    }

    /// Used by generated code to deserialize an adjacently tagged enum when
    /// ignoring unrelated fields is allowed.
    ///
    /// Not public API.
    pub enum TagContentOtherField {
        Tag,
        Content,
        Other,
    }

    /// Not public API.
    pub struct TagContentOtherFieldVisitor {
        /// Name of the tag field of the adjacently tagged enum
        pub tag: &'static str,
        /// Name of the content field of the adjacently tagged enum
        pub content: &'static str,
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de> DeserializeSeed<'de> for TagContentOtherFieldVisitor {
        type Value = TagContentOtherField;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_identifier(self)
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de> Visitor<'de> for TagContentOtherFieldVisitor {
        type Value = TagContentOtherField;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(
                formatter,
                "{:?}, {:?}, or other ignored fields",
                self.tag, self.content
            )
        }

        fn visit_u64<E>(self, field_index: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match field_index {
                0 => Ok(TagContentOtherField::Tag),
                1 => Ok(TagContentOtherField::Content),
                _ => Ok(TagContentOtherField::Other),
            }
        }

        fn visit_str<E>(self, field: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_bytes(field.as_bytes())
        }

        fn visit_bytes<E>(self, field: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if field == self.tag.as_bytes() {
                Ok(TagContentOtherField::Tag)
            } else if field == self.content.as_bytes() {
                Ok(TagContentOtherField::Content)
            } else {
                Ok(TagContentOtherField::Other)
            }
        }
    }

    /// Not public API
    pub struct ContentDeserializer<'de, E> {
        content: Content<'de>,
        err: PhantomData<E>,
    }

    impl<'de, E> ContentDeserializer<'de, E>
    where
        E: de::Error,
    {
        #[cold]
        fn invalid_type(self, exp: &dyn Expected) -> E {
            de::Error::invalid_type(content_unexpected(&self.content), exp)
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
                Content::I8(v) => visitor.visit_i8(v),
                Content::I16(v) => visitor.visit_i16(v),
                Content::I32(v) => visitor.visit_i32(v),
                Content::I64(v) => visitor.visit_i64(v),
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
                Content::I8(v) => visitor.visit_i8(v),
                Content::I16(v) => visitor.visit_i16(v),
                Content::I32(v) => visitor.visit_i32(v),
                Content::I64(v) => visitor.visit_i64(v),
                _ => Err(self.invalid_type(&visitor)),
            }
        }
    }

    fn visit_content_seq<'de, V, E>(content: Vec<Content<'de>>, visitor: V) -> Result<V::Value, E>
    where
        V: Visitor<'de>,
        E: de::Error,
    {
        let mut seq_visitor = SeqDeserializer::new(content);
        let value = tri!(visitor.visit_seq(&mut seq_visitor));
        tri!(seq_visitor.end());
        Ok(value)
    }

    fn visit_content_map<'de, V, E>(
        content: Vec<(Content<'de>, Content<'de>)>,
        visitor: V,
    ) -> Result<V::Value, E>
    where
        V: Visitor<'de>,
        E: de::Error,
    {
        let mut map_visitor = MapDeserializer::new(content);
        let value = tri!(visitor.visit_map(&mut map_visitor));
        tri!(map_visitor.end());
        Ok(value)
    }

    /// Used when deserializing an internally tagged enum because the content
    /// will be used exactly once.
    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, E> Deserializer<'de> for ContentDeserializer<'de, E>
    where
        E: de::Error,
    {
        type Error = E;

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
                Content::I8(v) => visitor.visit_i8(v),
                Content::I16(v) => visitor.visit_i16(v),
                Content::I32(v) => visitor.visit_i32(v),
                Content::I64(v) => visitor.visit_i64(v),
                Content::F32(v) => visitor.visit_f32(v),
                Content::F64(v) => visitor.visit_f64(v),
                Content::Char(v) => visitor.visit_char(v),
                Content::String(v) => visitor.visit_string(v),
                Content::Str(v) => visitor.visit_borrowed_str(v),
                Content::ByteBuf(v) => visitor.visit_byte_buf(v),
                Content::Bytes(v) => visitor.visit_borrowed_bytes(v),
                Content::Unit => visitor.visit_unit(),
                Content::None => visitor.visit_none(),
                Content::Some(v) => visitor.visit_some(ContentDeserializer::new(*v)),
                Content::Newtype(v) => visitor.visit_newtype_struct(ContentDeserializer::new(*v)),
                Content::Seq(v) => visit_content_seq(v, visitor),
                Content::Map(v) => visit_content_map(v, visitor),
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
                Content::Seq(v) => visit_content_seq(v, visitor),
                _ => Err(self.invalid_type(&visitor)),
            }
        }

        fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            match self.content {
                Content::None => visitor.visit_none(),
                Content::Some(v) => visitor.visit_some(ContentDeserializer::new(*v)),
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

        fn deserialize_newtype_struct<V>(
            self,
            _name: &str,
            visitor: V,
        ) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            match self.content {
                Content::Newtype(v) => visitor.visit_newtype_struct(ContentDeserializer::new(*v)),
                _ => visitor.visit_newtype_struct(self),
            }
        }

        fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            match self.content {
                Content::Seq(v) => visit_content_seq(v, visitor),
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
                Content::Map(v) => visit_content_map(v, visitor),
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
                Content::Seq(v) => visit_content_seq(v, visitor),
                Content::Map(v) => visit_content_map(v, visitor),
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
                            return Err(de::Error::invalid_value(
                                de::Unexpected::Map,
                                &"map with a single key",
                            ));
                        }
                    };
                    // enums are encoded in json as maps with a single key:value pair
                    if iter.next().is_some() {
                        return Err(de::Error::invalid_value(
                            de::Unexpected::Map,
                            &"map with a single key",
                        ));
                    }
                    (variant, Some(value))
                }
                s @ Content::String(_) | s @ Content::Str(_) => (s, None),
                other => {
                    return Err(de::Error::invalid_type(
                        content_unexpected(&other),
                        &"string or map",
                    ));
                }
            };

            visitor.visit_enum(EnumDeserializer::new(variant, value))
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

        fn __deserialize_content_v1<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de, Value = Content<'de>>,
        {
            let _ = visitor;
            Ok(self.content)
        }
    }

    impl<'de, E> ContentDeserializer<'de, E> {
        /// private API, don't use
        pub fn new(content: Content<'de>) -> Self {
            ContentDeserializer {
                content,
                err: PhantomData,
            }
        }
    }

    struct SeqDeserializer<'de, E> {
        iter: <Vec<Content<'de>> as IntoIterator>::IntoIter,
        count: usize,
        marker: PhantomData<E>,
    }

    impl<'de, E> SeqDeserializer<'de, E> {
        fn new(content: Vec<Content<'de>>) -> Self {
            SeqDeserializer {
                iter: content.into_iter(),
                count: 0,
                marker: PhantomData,
            }
        }
    }

    impl<'de, E> SeqDeserializer<'de, E>
    where
        E: de::Error,
    {
        fn end(self) -> Result<(), E> {
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

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, E> Deserializer<'de> for SeqDeserializer<'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            let v = tri!(visitor.visit_seq(&mut self));
            tri!(self.end());
            Ok(v)
        }

        serde_core::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
            bytes byte_buf option unit unit_struct newtype_struct seq tuple
            tuple_struct map struct enum identifier ignored_any
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, E> SeqAccess<'de> for SeqDeserializer<'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        fn next_element_seed<V>(&mut self, seed: V) -> Result<Option<V::Value>, Self::Error>
        where
            V: DeserializeSeed<'de>,
        {
            match self.iter.next() {
                Some(value) => {
                    self.count += 1;
                    seed.deserialize(ContentDeserializer::new(value)).map(Some)
                }
                None => Ok(None),
            }
        }

        fn size_hint(&self) -> Option<usize> {
            size_hint::from_bounds(&self.iter)
        }
    }

    struct ExpectedInSeq(usize);

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl Expected for ExpectedInSeq {
        fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            if self.0 == 1 {
                formatter.write_str("1 element in sequence")
            } else {
                write!(formatter, "{} elements in sequence", self.0)
            }
        }
    }

    struct MapDeserializer<'de, E> {
        iter: <Vec<(Content<'de>, Content<'de>)> as IntoIterator>::IntoIter,
        value: Option<Content<'de>>,
        count: usize,
        error: PhantomData<E>,
    }

    impl<'de, E> MapDeserializer<'de, E> {
        fn new(content: Vec<(Content<'de>, Content<'de>)>) -> Self {
            MapDeserializer {
                iter: content.into_iter(),
                value: None,
                count: 0,
                error: PhantomData,
            }
        }
    }

    impl<'de, E> MapDeserializer<'de, E>
    where
        E: de::Error,
    {
        fn end(self) -> Result<(), E> {
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

    impl<'de, E> MapDeserializer<'de, E> {
        fn next_pair(&mut self) -> Option<(Content<'de>, Content<'de>)> {
            match self.iter.next() {
                Some((k, v)) => {
                    self.count += 1;
                    Some((k, v))
                }
                None => None,
            }
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, E> Deserializer<'de> for MapDeserializer<'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            let value = tri!(visitor.visit_map(&mut self));
            tri!(self.end());
            Ok(value)
        }

        fn deserialize_seq<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            let value = tri!(visitor.visit_seq(&mut self));
            tri!(self.end());
            Ok(value)
        }

        fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            let _ = len;
            self.deserialize_seq(visitor)
        }

        serde_core::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
            bytes byte_buf option unit unit_struct newtype_struct tuple_struct map
            struct enum identifier ignored_any
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, E> MapAccess<'de> for MapDeserializer<'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        fn next_key_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
        where
            T: DeserializeSeed<'de>,
        {
            match self.next_pair() {
                Some((key, value)) => {
                    self.value = Some(value);
                    seed.deserialize(ContentDeserializer::new(key)).map(Some)
                }
                None => Ok(None),
            }
        }

        fn next_value_seed<T>(&mut self, seed: T) -> Result<T::Value, Self::Error>
        where
            T: DeserializeSeed<'de>,
        {
            let value = self.value.take();
            // Panic because this indicates a bug in the program rather than an
            // expected failure.
            let value = value.expect("MapAccess::next_value called before next_key");
            seed.deserialize(ContentDeserializer::new(value))
        }

        fn next_entry_seed<TK, TV>(
            &mut self,
            kseed: TK,
            vseed: TV,
        ) -> Result<Option<(TK::Value, TV::Value)>, Self::Error>
        where
            TK: DeserializeSeed<'de>,
            TV: DeserializeSeed<'de>,
        {
            match self.next_pair() {
                Some((key, value)) => {
                    let key = tri!(kseed.deserialize(ContentDeserializer::new(key)));
                    let value = tri!(vseed.deserialize(ContentDeserializer::new(value)));
                    Ok(Some((key, value)))
                }
                None => Ok(None),
            }
        }

        fn size_hint(&self) -> Option<usize> {
            size_hint::from_bounds(&self.iter)
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, E> SeqAccess<'de> for MapDeserializer<'de, E>
    where
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

    struct PairDeserializer<'de, E>(Content<'de>, Content<'de>, PhantomData<E>);

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, E> Deserializer<'de> for PairDeserializer<'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        serde_core::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
            bytes byte_buf option unit unit_struct newtype_struct tuple_struct map
            struct enum identifier ignored_any
        }

        fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_seq(visitor)
        }

        fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
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

    struct PairVisitor<'de, E>(Option<Content<'de>>, Option<Content<'de>>, PhantomData<E>);

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, E> SeqAccess<'de> for PairVisitor<'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
        where
            T: DeserializeSeed<'de>,
        {
            if let Some(k) = self.0.take() {
                seed.deserialize(ContentDeserializer::new(k)).map(Some)
            } else if let Some(v) = self.1.take() {
                seed.deserialize(ContentDeserializer::new(v)).map(Some)
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

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl Expected for ExpectedInMap {
        fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            if self.0 == 1 {
                formatter.write_str("1 element in map")
            } else {
                write!(formatter, "{} elements in map", self.0)
            }
        }
    }

    pub struct EnumDeserializer<'de, E>
    where
        E: de::Error,
    {
        variant: Content<'de>,
        value: Option<Content<'de>>,
        err: PhantomData<E>,
    }

    impl<'de, E> EnumDeserializer<'de, E>
    where
        E: de::Error,
    {
        pub fn new(variant: Content<'de>, value: Option<Content<'de>>) -> EnumDeserializer<'de, E> {
            EnumDeserializer {
                variant,
                value,
                err: PhantomData,
            }
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, E> de::EnumAccess<'de> for EnumDeserializer<'de, E>
    where
        E: de::Error,
    {
        type Error = E;
        type Variant = VariantDeserializer<'de, Self::Error>;

        fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), E>
        where
            V: de::DeserializeSeed<'de>,
        {
            let visitor = VariantDeserializer {
                value: self.value,
                err: PhantomData,
            };
            seed.deserialize(ContentDeserializer::new(self.variant))
                .map(|v| (v, visitor))
        }
    }

    pub struct VariantDeserializer<'de, E>
    where
        E: de::Error,
    {
        value: Option<Content<'de>>,
        err: PhantomData<E>,
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, E> de::VariantAccess<'de> for VariantDeserializer<'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        fn unit_variant(self) -> Result<(), E> {
            match self.value {
                Some(value) => de::Deserialize::deserialize(ContentDeserializer::new(value)),
                None => Ok(()),
            }
        }

        fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, E>
        where
            T: de::DeserializeSeed<'de>,
        {
            match self.value {
                Some(value) => seed.deserialize(ContentDeserializer::new(value)),
                None => Err(de::Error::invalid_type(
                    de::Unexpected::UnitVariant,
                    &"newtype variant",
                )),
            }
        }

        fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: de::Visitor<'de>,
        {
            match self.value {
                Some(Content::Seq(v)) => {
                    de::Deserializer::deserialize_any(SeqDeserializer::new(v), visitor)
                }
                Some(other) => Err(de::Error::invalid_type(
                    content_unexpected(&other),
                    &"tuple variant",
                )),
                None => Err(de::Error::invalid_type(
                    de::Unexpected::UnitVariant,
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
            V: de::Visitor<'de>,
        {
            match self.value {
                Some(Content::Map(v)) => {
                    de::Deserializer::deserialize_any(MapDeserializer::new(v), visitor)
                }
                Some(Content::Seq(v)) => {
                    de::Deserializer::deserialize_any(SeqDeserializer::new(v), visitor)
                }
                Some(other) => Err(de::Error::invalid_type(
                    content_unexpected(&other),
                    &"struct variant",
                )),
                None => Err(de::Error::invalid_type(
                    de::Unexpected::UnitVariant,
                    &"struct variant",
                )),
            }
        }
    }

    /// Not public API.
    pub struct ContentRefDeserializer<'a, 'de: 'a, E> {
        content: &'a Content<'de>,
        err: PhantomData<E>,
    }

    impl<'a, 'de, E> ContentRefDeserializer<'a, 'de, E>
    where
        E: de::Error,
    {
        #[cold]
        fn invalid_type(self, exp: &dyn Expected) -> E {
            de::Error::invalid_type(content_unexpected(self.content), exp)
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
                Content::I8(v) => visitor.visit_i8(v),
                Content::I16(v) => visitor.visit_i16(v),
                Content::I32(v) => visitor.visit_i32(v),
                Content::I64(v) => visitor.visit_i64(v),
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
                Content::I8(v) => visitor.visit_i8(v),
                Content::I16(v) => visitor.visit_i16(v),
                Content::I32(v) => visitor.visit_i32(v),
                Content::I64(v) => visitor.visit_i64(v),
                _ => Err(self.invalid_type(&visitor)),
            }
        }
    }

    fn visit_content_seq_ref<'a, 'de, V, E>(
        content: &'a [Content<'de>],
        visitor: V,
    ) -> Result<V::Value, E>
    where
        V: Visitor<'de>,
        E: de::Error,
    {
        let mut seq_visitor = SeqRefDeserializer::new(content);
        let value = tri!(visitor.visit_seq(&mut seq_visitor));
        tri!(seq_visitor.end());
        Ok(value)
    }

    fn visit_content_map_ref<'a, 'de, V, E>(
        content: &'a [(Content<'de>, Content<'de>)],
        visitor: V,
    ) -> Result<V::Value, E>
    where
        V: Visitor<'de>,
        E: de::Error,
    {
        let mut map_visitor = MapRefDeserializer::new(content);
        let value = tri!(visitor.visit_map(&mut map_visitor));
        tri!(map_visitor.end());
        Ok(value)
    }

    /// Used when deserializing an untagged enum because the content may need
    /// to be used more than once.
    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, 'a, E> Deserializer<'de> for ContentRefDeserializer<'a, 'de, E>
    where
        E: de::Error,
    {
        type Error = E;

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
                Content::I8(v) => visitor.visit_i8(v),
                Content::I16(v) => visitor.visit_i16(v),
                Content::I32(v) => visitor.visit_i32(v),
                Content::I64(v) => visitor.visit_i64(v),
                Content::F32(v) => visitor.visit_f32(v),
                Content::F64(v) => visitor.visit_f64(v),
                Content::Char(v) => visitor.visit_char(v),
                Content::String(ref v) => visitor.visit_str(v),
                Content::Str(v) => visitor.visit_borrowed_str(v),
                Content::ByteBuf(ref v) => visitor.visit_bytes(v),
                Content::Bytes(v) => visitor.visit_borrowed_bytes(v),
                Content::Unit => visitor.visit_unit(),
                Content::None => visitor.visit_none(),
                Content::Some(ref v) => visitor.visit_some(ContentRefDeserializer::new(v)),
                Content::Newtype(ref v) => {
                    visitor.visit_newtype_struct(ContentRefDeserializer::new(v))
                }
                Content::Seq(ref v) => visit_content_seq_ref(v, visitor),
                Content::Map(ref v) => visit_content_map_ref(v, visitor),
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
                Content::Seq(ref v) => visit_content_seq_ref(v, visitor),
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
            // Covered by tests/test_enum_untagged.rs
            //      with_optional_field::*
            match *self.content {
                Content::None => visitor.visit_none(),
                Content::Some(ref v) => visitor.visit_some(ContentRefDeserializer::new(v)),
                Content::Unit => visitor.visit_unit(),
                // This case is to support data formats which do not encode an
                // indication whether a value is optional. An example of such a
                // format is JSON, and a counterexample is RON. When requesting
                // `deserialize_any` in JSON, the data format never performs
                // `Visitor::visit_some` but we still must be able to
                // deserialize the resulting Content into data structures with
                // optional fields.
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
            // Covered by tests/test_enum_untagged.rs
            //      newtype_struct
            match *self.content {
                Content::Newtype(ref v) => {
                    visitor.visit_newtype_struct(ContentRefDeserializer::new(v))
                }
                // This case is to support data formats that encode newtype
                // structs and their underlying data the same, with no
                // indication whether a newtype wrapper was present. For example
                // JSON does this, while RON does not. In RON a newtype's name
                // is included in the serialized representation and it knows to
                // call `Visitor::visit_newtype_struct` from `deserialize_any`.
                // JSON's `deserialize_any` never calls `visit_newtype_struct`
                // but in this code we still must be able to deserialize the
                // resulting Content into newtypes.
                _ => visitor.visit_newtype_struct(self),
            }
        }

        fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            match *self.content {
                Content::Seq(ref v) => visit_content_seq_ref(v, visitor),
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
                Content::Map(ref v) => visit_content_map_ref(v, visitor),
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
                Content::Seq(ref v) => visit_content_seq_ref(v, visitor),
                Content::Map(ref v) => visit_content_map_ref(v, visitor),
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
                            return Err(de::Error::invalid_value(
                                de::Unexpected::Map,
                                &"map with a single key",
                            ));
                        }
                    };
                    // enums are encoded in json as maps with a single key:value pair
                    if iter.next().is_some() {
                        return Err(de::Error::invalid_value(
                            de::Unexpected::Map,
                            &"map with a single key",
                        ));
                    }
                    (variant, Some(value))
                }
                ref s @ Content::String(_) | ref s @ Content::Str(_) => (s, None),
                ref other => {
                    return Err(de::Error::invalid_type(
                        content_unexpected(other),
                        &"string or map",
                    ));
                }
            };

            visitor.visit_enum(EnumRefDeserializer {
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

        fn __deserialize_content_v1<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de, Value = Content<'de>>,
        {
            let _ = visitor;
            Ok(content_clone(self.content))
        }
    }

    impl<'a, 'de, E> ContentRefDeserializer<'a, 'de, E> {
        /// private API, don't use
        pub fn new(content: &'a Content<'de>) -> Self {
            ContentRefDeserializer {
                content,
                err: PhantomData,
            }
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'a, 'de: 'a, E> Copy for ContentRefDeserializer<'a, 'de, E> {}

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'a, 'de: 'a, E> Clone for ContentRefDeserializer<'a, 'de, E> {
        fn clone(&self) -> Self {
            *self
        }
    }

    struct SeqRefDeserializer<'a, 'de, E> {
        iter: <&'a [Content<'de>] as IntoIterator>::IntoIter,
        count: usize,
        marker: PhantomData<E>,
    }

    impl<'a, 'de, E> SeqRefDeserializer<'a, 'de, E> {
        fn new(content: &'a [Content<'de>]) -> Self {
            SeqRefDeserializer {
                iter: content.iter(),
                count: 0,
                marker: PhantomData,
            }
        }
    }

    impl<'a, 'de, E> SeqRefDeserializer<'a, 'de, E>
    where
        E: de::Error,
    {
        fn end(self) -> Result<(), E> {
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

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'a, 'de, E> Deserializer<'de> for SeqRefDeserializer<'a, 'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            let v = tri!(visitor.visit_seq(&mut self));
            tri!(self.end());
            Ok(v)
        }

        serde_core::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
            bytes byte_buf option unit unit_struct newtype_struct seq tuple
            tuple_struct map struct enum identifier ignored_any
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'a, 'de, E> SeqAccess<'de> for SeqRefDeserializer<'a, 'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        fn next_element_seed<V>(&mut self, seed: V) -> Result<Option<V::Value>, Self::Error>
        where
            V: DeserializeSeed<'de>,
        {
            match self.iter.next() {
                Some(value) => {
                    self.count += 1;
                    seed.deserialize(ContentRefDeserializer::new(value))
                        .map(Some)
                }
                None => Ok(None),
            }
        }

        fn size_hint(&self) -> Option<usize> {
            size_hint::from_bounds(&self.iter)
        }
    }

    struct MapRefDeserializer<'a, 'de, E> {
        iter: <&'a [(Content<'de>, Content<'de>)] as IntoIterator>::IntoIter,
        value: Option<&'a Content<'de>>,
        count: usize,
        error: PhantomData<E>,
    }

    impl<'a, 'de, E> MapRefDeserializer<'a, 'de, E> {
        fn new(content: &'a [(Content<'de>, Content<'de>)]) -> Self {
            MapRefDeserializer {
                iter: content.iter(),
                value: None,
                count: 0,
                error: PhantomData,
            }
        }
    }

    impl<'a, 'de, E> MapRefDeserializer<'a, 'de, E>
    where
        E: de::Error,
    {
        fn end(self) -> Result<(), E> {
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

    impl<'a, 'de, E> MapRefDeserializer<'a, 'de, E> {
        fn next_pair(&mut self) -> Option<(&'a Content<'de>, &'a Content<'de>)> {
            match self.iter.next() {
                Some((k, v)) => {
                    self.count += 1;
                    Some((k, v))
                }
                None => None,
            }
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'a, 'de, E> Deserializer<'de> for MapRefDeserializer<'a, 'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            let value = tri!(visitor.visit_map(&mut self));
            tri!(self.end());
            Ok(value)
        }

        fn deserialize_seq<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            let value = tri!(visitor.visit_seq(&mut self));
            tri!(self.end());
            Ok(value)
        }

        fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            let _ = len;
            self.deserialize_seq(visitor)
        }

        serde_core::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
            bytes byte_buf option unit unit_struct newtype_struct tuple_struct map
            struct enum identifier ignored_any
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'a, 'de, E> MapAccess<'de> for MapRefDeserializer<'a, 'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        fn next_key_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
        where
            T: DeserializeSeed<'de>,
        {
            match self.next_pair() {
                Some((key, value)) => {
                    self.value = Some(value);
                    seed.deserialize(ContentRefDeserializer::new(key)).map(Some)
                }
                None => Ok(None),
            }
        }

        fn next_value_seed<T>(&mut self, seed: T) -> Result<T::Value, Self::Error>
        where
            T: DeserializeSeed<'de>,
        {
            let value = self.value.take();
            // Panic because this indicates a bug in the program rather than an
            // expected failure.
            let value = value.expect("MapAccess::next_value called before next_key");
            seed.deserialize(ContentRefDeserializer::new(value))
        }

        fn next_entry_seed<TK, TV>(
            &mut self,
            kseed: TK,
            vseed: TV,
        ) -> Result<Option<(TK::Value, TV::Value)>, Self::Error>
        where
            TK: DeserializeSeed<'de>,
            TV: DeserializeSeed<'de>,
        {
            match self.next_pair() {
                Some((key, value)) => {
                    let key = tri!(kseed.deserialize(ContentRefDeserializer::new(key)));
                    let value = tri!(vseed.deserialize(ContentRefDeserializer::new(value)));
                    Ok(Some((key, value)))
                }
                None => Ok(None),
            }
        }

        fn size_hint(&self) -> Option<usize> {
            size_hint::from_bounds(&self.iter)
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'a, 'de, E> SeqAccess<'de> for MapRefDeserializer<'a, 'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
        where
            T: de::DeserializeSeed<'de>,
        {
            match self.next_pair() {
                Some((k, v)) => {
                    let de = PairRefDeserializer(k, v, PhantomData);
                    seed.deserialize(de).map(Some)
                }
                None => Ok(None),
            }
        }

        fn size_hint(&self) -> Option<usize> {
            size_hint::from_bounds(&self.iter)
        }
    }

    struct PairRefDeserializer<'a, 'de, E>(&'a Content<'de>, &'a Content<'de>, PhantomData<E>);

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'a, 'de, E> Deserializer<'de> for PairRefDeserializer<'a, 'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        serde_core::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
            bytes byte_buf option unit unit_struct newtype_struct tuple_struct map
            struct enum identifier ignored_any
        }

        fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_seq(visitor)
        }

        fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            let mut pair_visitor = PairRefVisitor(Some(self.0), Some(self.1), PhantomData);
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

    struct PairRefVisitor<'a, 'de, E>(
        Option<&'a Content<'de>>,
        Option<&'a Content<'de>>,
        PhantomData<E>,
    );

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'a, 'de, E> SeqAccess<'de> for PairRefVisitor<'a, 'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
        where
            T: DeserializeSeed<'de>,
        {
            if let Some(k) = self.0.take() {
                seed.deserialize(ContentRefDeserializer::new(k)).map(Some)
            } else if let Some(v) = self.1.take() {
                seed.deserialize(ContentRefDeserializer::new(v)).map(Some)
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

    struct EnumRefDeserializer<'a, 'de: 'a, E>
    where
        E: de::Error,
    {
        variant: &'a Content<'de>,
        value: Option<&'a Content<'de>>,
        err: PhantomData<E>,
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, 'a, E> de::EnumAccess<'de> for EnumRefDeserializer<'a, 'de, E>
    where
        E: de::Error,
    {
        type Error = E;
        type Variant = VariantRefDeserializer<'a, 'de, Self::Error>;

        fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
        where
            V: de::DeserializeSeed<'de>,
        {
            let visitor = VariantRefDeserializer {
                value: self.value,
                err: PhantomData,
            };
            seed.deserialize(ContentRefDeserializer::new(self.variant))
                .map(|v| (v, visitor))
        }
    }

    struct VariantRefDeserializer<'a, 'de: 'a, E>
    where
        E: de::Error,
    {
        value: Option<&'a Content<'de>>,
        err: PhantomData<E>,
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, 'a, E> de::VariantAccess<'de> for VariantRefDeserializer<'a, 'de, E>
    where
        E: de::Error,
    {
        type Error = E;

        fn unit_variant(self) -> Result<(), E> {
            match self.value {
                Some(value) => de::Deserialize::deserialize(ContentRefDeserializer::new(value)),
                // Covered by tests/test_annotations.rs
                //      test_partially_untagged_adjacently_tagged_enum
                // Covered by tests/test_enum_untagged.rs
                //      newtype_enum::unit
                None => Ok(()),
            }
        }

        fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, E>
        where
            T: de::DeserializeSeed<'de>,
        {
            match self.value {
                // Covered by tests/test_annotations.rs
                //      test_partially_untagged_enum_desugared
                //      test_partially_untagged_enum_generic
                // Covered by tests/test_enum_untagged.rs
                //      newtype_enum::newtype
                Some(value) => seed.deserialize(ContentRefDeserializer::new(value)),
                None => Err(de::Error::invalid_type(
                    de::Unexpected::UnitVariant,
                    &"newtype variant",
                )),
            }
        }

        fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: de::Visitor<'de>,
        {
            match self.value {
                // Covered by tests/test_annotations.rs
                //      test_partially_untagged_enum
                //      test_partially_untagged_enum_desugared
                // Covered by tests/test_enum_untagged.rs
                //      newtype_enum::tuple0
                //      newtype_enum::tuple2
                Some(Content::Seq(v)) => visit_content_seq_ref(v, visitor),
                Some(other) => Err(de::Error::invalid_type(
                    content_unexpected(other),
                    &"tuple variant",
                )),
                None => Err(de::Error::invalid_type(
                    de::Unexpected::UnitVariant,
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
            V: de::Visitor<'de>,
        {
            match self.value {
                // Covered by tests/test_enum_untagged.rs
                //      newtype_enum::struct_from_map
                Some(Content::Map(v)) => visit_content_map_ref(v, visitor),
                // Covered by tests/test_enum_untagged.rs
                //      newtype_enum::struct_from_seq
                //      newtype_enum::empty_struct_from_seq
                Some(Content::Seq(v)) => visit_content_seq_ref(v, visitor),
                Some(other) => Err(de::Error::invalid_type(
                    content_unexpected(other),
                    &"struct variant",
                )),
                None => Err(de::Error::invalid_type(
                    de::Unexpected::UnitVariant,
                    &"struct variant",
                )),
            }
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, E> de::IntoDeserializer<'de, E> for ContentDeserializer<'de, E>
    where
        E: de::Error,
    {
        type Deserializer = Self;

        fn into_deserializer(self) -> Self {
            self
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, 'a, E> de::IntoDeserializer<'de, E> for ContentRefDeserializer<'a, 'de, E>
    where
        E: de::Error,
    {
        type Deserializer = Self;

        fn into_deserializer(self) -> Self {
            self
        }
    }

    /// Visitor for deserializing an internally tagged unit variant.
    ///
    /// Not public API.
    pub struct InternallyTaggedUnitVisitor<'a> {
        type_name: &'a str,
        variant_name: &'a str,
    }

    impl<'a> InternallyTaggedUnitVisitor<'a> {
        /// Not public API.
        pub fn new(type_name: &'a str, variant_name: &'a str) -> Self {
            InternallyTaggedUnitVisitor {
                type_name,
                variant_name,
            }
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, 'a> Visitor<'de> for InternallyTaggedUnitVisitor<'a> {
        type Value = ();

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(
                formatter,
                "unit variant {}::{}",
                self.type_name, self.variant_name
            )
        }

        fn visit_seq<S>(self, _: S) -> Result<(), S::Error>
        where
            S: SeqAccess<'de>,
        {
            Ok(())
        }

        fn visit_map<M>(self, mut access: M) -> Result<(), M::Error>
        where
            M: MapAccess<'de>,
        {
            while tri!(access.next_entry::<IgnoredAny, IgnoredAny>()).is_some() {}
            Ok(())
        }
    }

    /// Visitor for deserializing an untagged unit variant.
    ///
    /// Not public API.
    pub struct UntaggedUnitVisitor<'a> {
        type_name: &'a str,
        variant_name: &'a str,
    }

    impl<'a> UntaggedUnitVisitor<'a> {
        /// Not public API.
        pub fn new(type_name: &'a str, variant_name: &'a str) -> Self {
            UntaggedUnitVisitor {
                type_name,
                variant_name,
            }
        }
    }

    #[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
    impl<'de, 'a> Visitor<'de> for UntaggedUnitVisitor<'a> {
        type Value = ();

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(
                formatter,
                "unit variant {}::{}",
                self.type_name, self.variant_name
            )
        }

        fn visit_unit<E>(self) -> Result<(), E>
        where
            E: de::Error,
        {
            Ok(())
        }

        fn visit_none<E>(self) -> Result<(), E>
        where
            E: de::Error,
        {
            Ok(())
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

// Like `IntoDeserializer` but also implemented for `&[u8]`. This is used for
// the newtype fallthrough case of `field_identifier`.
//
//    #[derive(Deserialize)]
//    #[serde(field_identifier)]
//    enum F {
//        A,
//        B,
//        Other(String), // deserialized using IdentifierDeserializer
//    }
pub trait IdentifierDeserializer<'de, E: Error> {
    type Deserializer: Deserializer<'de, Error = E>;

    fn from(self) -> Self::Deserializer;
}

pub struct Borrowed<'de, T: 'de + ?Sized>(pub &'de T);

#[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
impl<'de, E> IdentifierDeserializer<'de, E> for u64
where
    E: Error,
{
    type Deserializer = <u64 as IntoDeserializer<'de, E>>::Deserializer;

    fn from(self) -> Self::Deserializer {
        self.into_deserializer()
    }
}

pub struct StrDeserializer<'a, E> {
    value: &'a str,
    marker: PhantomData<E>,
}

#[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
impl<'de, 'a, E> Deserializer<'de> for StrDeserializer<'a, E>
where
    E: Error,
{
    type Error = E;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.value)
    }

    serde_core::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

pub struct BorrowedStrDeserializer<'de, E> {
    value: &'de str,
    marker: PhantomData<E>,
}

#[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
impl<'de, E> Deserializer<'de> for BorrowedStrDeserializer<'de, E>
where
    E: Error,
{
    type Error = E;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.value)
    }

    serde_core::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

#[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
impl<'a, E> IdentifierDeserializer<'a, E> for &'a str
where
    E: Error,
{
    type Deserializer = StrDeserializer<'a, E>;

    fn from(self) -> Self::Deserializer {
        StrDeserializer {
            value: self,
            marker: PhantomData,
        }
    }
}

#[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
impl<'de, E> IdentifierDeserializer<'de, E> for Borrowed<'de, str>
where
    E: Error,
{
    type Deserializer = BorrowedStrDeserializer<'de, E>;

    fn from(self) -> Self::Deserializer {
        BorrowedStrDeserializer {
            value: self.0,
            marker: PhantomData,
        }
    }
}

#[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
impl<'a, E> IdentifierDeserializer<'a, E> for &'a [u8]
where
    E: Error,
{
    type Deserializer = BytesDeserializer<'a, E>;

    fn from(self) -> Self::Deserializer {
        BytesDeserializer::new(self)
    }
}

#[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
impl<'de, E> IdentifierDeserializer<'de, E> for Borrowed<'de, [u8]>
where
    E: Error,
{
    type Deserializer = BorrowedBytesDeserializer<'de, E>;

    fn from(self) -> Self::Deserializer {
        BorrowedBytesDeserializer::new(self.0)
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
pub struct FlatMapDeserializer<'a, 'de: 'a, E>(
    pub &'a mut Vec<Option<(Content<'de>, Content<'de>)>>,
    pub PhantomData<E>,
);

#[cfg(any(feature = "std", feature = "alloc"))]
impl<'a, 'de, E> FlatMapDeserializer<'a, 'de, E>
where
    E: Error,
{
    fn deserialize_other<V>() -> Result<V, E> {
        Err(Error::custom("can only flatten structs and maps"))
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
macro_rules! forward_to_deserialize_other {
    ($($func:ident ($($arg:ty),*))*) => {
        $(
            fn $func<V>(self, $(_: $arg,)* _visitor: V) -> Result<V::Value, Self::Error>
            where
                V: Visitor<'de>,
            {
                Self::deserialize_other()
            }
        )*
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
impl<'a, 'de, E> Deserializer<'de> for FlatMapDeserializer<'a, 'de, E>
where
    E: Error,
{
    type Error = E;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        for entry in self.0 {
            if let Some((key, value)) = flat_map_take_entry(entry, variants) {
                return visitor.visit_enum(EnumDeserializer::new(key, Some(value)));
            }
        }

        Err(Error::custom(format_args!(
            "no variant of enum {} found in flattened data",
            name
        )))
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(FlatMapAccess {
            iter: self.0.iter(),
            pending_content: None,
            _marker: PhantomData,
        })
    }

    fn deserialize_struct<V>(
        self,
        _: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(FlatStructAccess {
            iter: self.0.iter_mut(),
            pending_content: None,
            fields,
            _marker: PhantomData,
        })
    }

    fn deserialize_newtype_struct<V>(self, _name: &str, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match visitor.__private_visit_untagged_option(self) {
            Ok(value) => Ok(value),
            Err(()) => Self::deserialize_other(),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    forward_to_deserialize_other! {
        deserialize_bool()
        deserialize_i8()
        deserialize_i16()
        deserialize_i32()
        deserialize_i64()
        deserialize_u8()
        deserialize_u16()
        deserialize_u32()
        deserialize_u64()
        deserialize_f32()
        deserialize_f64()
        deserialize_char()
        deserialize_str()
        deserialize_string()
        deserialize_bytes()
        deserialize_byte_buf()
        deserialize_seq()
        deserialize_tuple(usize)
        deserialize_tuple_struct(&'static str, usize)
        deserialize_identifier()
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
struct FlatMapAccess<'a, 'de: 'a, E> {
    iter: slice::Iter<'a, Option<(Content<'de>, Content<'de>)>>,
    pending_content: Option<&'a Content<'de>>,
    _marker: PhantomData<E>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
impl<'a, 'de, E> MapAccess<'de> for FlatMapAccess<'a, 'de, E>
where
    E: Error,
{
    type Error = E;

    fn next_key_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        for item in &mut self.iter {
            // Items in the vector are nulled out when used by a struct.
            if let Some((ref key, ref content)) = *item {
                // Do not take(), instead borrow this entry. The internally tagged
                // enum does its own buffering so we can't tell whether this entry
                // is going to be consumed. Borrowing here leaves the entry
                // available for later flattened fields.
                self.pending_content = Some(content);
                return seed.deserialize(ContentRefDeserializer::new(key)).map(Some);
            }
        }
        Ok(None)
    }

    fn next_value_seed<T>(&mut self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.pending_content.take() {
            Some(value) => seed.deserialize(ContentRefDeserializer::new(value)),
            None => Err(Error::custom("value is missing")),
        }
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
struct FlatStructAccess<'a, 'de: 'a, E> {
    iter: slice::IterMut<'a, Option<(Content<'de>, Content<'de>)>>,
    pending_content: Option<Content<'de>>,
    fields: &'static [&'static str],
    _marker: PhantomData<E>,
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
impl<'a, 'de, E> MapAccess<'de> for FlatStructAccess<'a, 'de, E>
where
    E: Error,
{
    type Error = E;

    fn next_key_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        for entry in self.iter.by_ref() {
            if let Some((key, content)) = flat_map_take_entry(entry, self.fields) {
                self.pending_content = Some(content);
                return seed.deserialize(ContentDeserializer::new(key)).map(Some);
            }
        }
        Ok(None)
    }

    fn next_value_seed<T>(&mut self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.pending_content.take() {
            Some(value) => seed.deserialize(ContentDeserializer::new(value)),
            None => Err(Error::custom("value is missing")),
        }
    }
}

/// Claims one key-value pair from a FlatMapDeserializer's field buffer if the
/// field name matches any of the recognized ones.
#[cfg(any(feature = "std", feature = "alloc"))]
fn flat_map_take_entry<'de>(
    entry: &mut Option<(Content<'de>, Content<'de>)>,
    recognized: &[&str],
) -> Option<(Content<'de>, Content<'de>)> {
    // Entries in the FlatMapDeserializer buffer are nulled out as they get
    // claimed for deserialization. We only use an entry if it is still present
    // and if the field is one recognized by the current data structure.
    let is_recognized = match entry {
        None => false,
        Some((k, _v)) => content_as_str(k).map_or(false, |name| recognized.contains(&name)),
    };

    if is_recognized {
        entry.take()
    } else {
        None
    }
}

pub struct AdjacentlyTaggedEnumVariantSeed<F> {
    pub enum_name: &'static str,
    pub variants: &'static [&'static str],
    pub fields_enum: PhantomData<F>,
}

pub struct AdjacentlyTaggedEnumVariantVisitor<F> {
    enum_name: &'static str,
    fields_enum: PhantomData<F>,
}

#[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
impl<'de, F> Visitor<'de> for AdjacentlyTaggedEnumVariantVisitor<F>
where
    F: Deserialize<'de>,
{
    type Value = F;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "variant of enum {}", self.enum_name)
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: EnumAccess<'de>,
    {
        let (variant, variant_access) = tri!(data.variant());
        tri!(variant_access.unit_variant());
        Ok(variant)
    }
}

#[cfg_attr(not(no_diagnostic_namespace), diagnostic::do_not_recommend)]
impl<'de, F> DeserializeSeed<'de> for AdjacentlyTaggedEnumVariantSeed<F>
where
    F: Deserialize<'de>,
{
    type Value = F;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_enum(
            self.enum_name,
            self.variants,
            AdjacentlyTaggedEnumVariantVisitor {
                enum_name: self.enum_name,
                fields_enum: PhantomData,
            },
        )
    }
}
