//! Contains serializer for an XML element

use crate::de::{TEXT_KEY, VALUE_KEY};
use crate::se::content::ContentSerializer;
use crate::se::key::QNameSerializer;
use crate::se::simple_type::{QuoteTarget, SimpleSeq, SimpleTypeSerializer};
use crate::se::text::TextSerializer;
use crate::se::{SeError, WriteResult, XmlName};
use serde::ser::{
    Impossible, Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
    SerializeTuple, SerializeTupleStruct, SerializeTupleVariant, Serializer,
};
use serde::serde_if_integer128;
use std::fmt::Write;

/// Writes simple type content between [`ElementSerializer::key`] tags.
macro_rules! write_primitive {
    ($method:ident ( $ty:ty )) => {
        fn $method(self, value: $ty) -> Result<Self::Ok, Self::Error> {
            self.ser.write_wrapped(self.key, |ser| ser.$method(value))
        }
    };
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A serializer used to serialize element with specified name. Unlike the [`ContentSerializer`],
/// this serializer never uses variant names of enum variants, and because of that
/// it is unable to serialize any enum values, except unit variants.
///
/// Returns the classification of the last written type.
///
/// This serializer is used for an ordinary fields in structs, which are not special
/// fields named `$text` ([`TEXT_KEY`]) or `$value` ([`VALUE_KEY`]). `$text` field
/// should be serialized using [`SimpleTypeSerializer`] and `$value` field should be
/// serialized using [`ContentSerializer`].
///
/// This serializer does the following:
/// - numbers converted to a decimal representation and serialized as `<key>value</key>`;
/// - booleans serialized ether as `<key>true</key>` or `<key>false</key>`;
/// - strings and characters are serialized as `<key>value</key>`. In particular,
///   an empty string is serialized as `<key/>`;
/// - `None` is serialized as `<key/>`;
/// - `Some` and newtypes are serialized as an inner type using the same serializer;
/// - units (`()`) and unit structs are serialized as `<key/>`;
/// - sequences, tuples and tuple structs are serialized as repeated `<key>` tag.
///   In particular, empty sequence is serialized to nothing;
/// - structs are serialized as a sequence of fields wrapped in a `<key>` tag. Each
///   field is serialized recursively using either `ElementSerializer`, [`ContentSerializer`]
///   (`$value` fields), or [`SimpleTypeSerializer`] (`$text` fields).
///   In particular, the empty struct is serialized as `<key/>`;
/// - maps are serialized as a sequence of entries wrapped in a `<key>` tag. If key is
///   serialized to a special name, the same rules as for struct fields are applied.
///   In particular, the empty map is serialized as `<key/>`;
/// - enums:
///   - unit variants are serialized as `<key>variant</key>`;
///   - other variants are not supported ([`SeError::Unsupported`] is returned);
///
/// Usage of empty tags depends on the [`ContentSerializer::expand_empty_elements`] setting.
pub struct ElementSerializer<'w, 'k, W: Write> {
    /// The inner serializer that contains the settings and mostly do the actual work
    pub ser: ContentSerializer<'w, 'k, W>,
    /// Tag name used to wrap serialized types except enum variants which uses the variant name
    pub(super) key: XmlName<'k>,
}

impl<'w, 'k, W: Write> Serializer for ElementSerializer<'w, 'k, W> {
    type Ok = WriteResult;
    type Error = SeError;

    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = Map<'w, 'k, W>;
    type SerializeStruct = Struct<'w, 'k, W>;
    type SerializeStructVariant = Struct<'w, 'k, W>;

    write_primitive!(serialize_bool(bool));

    write_primitive!(serialize_i8(i8));
    write_primitive!(serialize_i16(i16));
    write_primitive!(serialize_i32(i32));
    write_primitive!(serialize_i64(i64));

    write_primitive!(serialize_u8(u8));
    write_primitive!(serialize_u16(u16));
    write_primitive!(serialize_u32(u32));
    write_primitive!(serialize_u64(u64));

    serde_if_integer128! {
        write_primitive!(serialize_i128(i128));
        write_primitive!(serialize_u128(u128));
    }

    write_primitive!(serialize_f32(f32));
    write_primitive!(serialize_f64(f64));

    write_primitive!(serialize_char(char));
    write_primitive!(serialize_bytes(&[u8]));

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        if value.is_empty() {
            self.ser.write_empty(self.key)
        } else {
            self.ser
                .write_wrapped(self.key, |ser| ser.serialize_str(value))
        }
    }

    /// By serde contract we should serialize key of [`None`] values. If someone
    /// wants to skip the field entirely, he should use
    /// `#[serde(skip_serializing_if = "Option::is_none")]`.
    ///
    /// In XML when we serialize field, we write field name as:
    /// - element name, or
    /// - attribute name
    ///
    /// and field value as
    /// - content of the element, or
    /// - attribute value
    ///
    /// So serialization of `None` works the same as [serialization of `()`](#method.serialize_unit)
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.ser.write_empty(self.key)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.ser.write_empty(self.key)
    }

    /// Writes a tag with name [`Self::key`] and content of unit variant inside.
    /// If variant is a special `$text` value, then empty tag `<key/>` is written.
    /// Otherwise a `<key>variant</key>` is written.
    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        if variant == TEXT_KEY {
            self.ser.write_empty(self.key)
        } else {
            self.ser.write_wrapped(self.key, |ser| {
                ser.serialize_unit_variant(name, variant_index, variant)
            })
        }
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    /// Always returns [`SeError::Unsupported`]. Newtype variants can be serialized
    /// only in `$value` fields, which is serialized using [`ContentSerializer`].
    #[inline]
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(SeError::Unsupported(
            format!(
                "cannot serialize enum newtype variant `{}::{}`",
                name, variant
            )
            .into(),
        ))
    }

    #[inline]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(self)
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_tuple(len)
    }

    /// Always returns [`SeError::Unsupported`]. Tuple variants can be serialized
    /// only in `$value` fields, which is serialized using [`ContentSerializer`].
    #[inline]
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(SeError::Unsupported(
            format!(
                "cannot serialize enum tuple variant `{}::{}`",
                name, variant
            )
            .into(),
        ))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(Map {
            ser: self.serialize_struct("", 0)?,
            key: None,
        })
    }

    #[inline]
    fn serialize_struct(
        mut self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.ser.write_indent()?;
        self.ser.indent.increase();

        self.ser.writer.write_char('<')?;
        self.ser.writer.write_str(self.key.0)?;
        Ok(Struct {
            ser: self,
            children: String::new(),
            write_indent: true,
        })
    }

    /// Always returns [`SeError::Unsupported`]. Struct variants can be serialized
    /// only in `$value` fields, which is serialized using [`ContentSerializer`].
    #[inline]
    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(SeError::Unsupported(
            format!(
                "cannot serialize enum struct variant `{}::{}`",
                name, variant
            )
            .into(),
        ))
    }
}

impl<'w, 'k, W: Write> SerializeSeq for ElementSerializer<'w, 'k, W> {
    type Ok = WriteResult;
    type Error = SeError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(ElementSerializer {
            ser: self.ser.new_seq_element_serializer(true),
            key: self.key,
        })?;
        // Write indent for the next element
        self.ser.write_indent = true;
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(WriteResult::Element)
    }
}

impl<'w, 'k, W: Write> SerializeTuple for ElementSerializer<'w, 'k, W> {
    type Ok = WriteResult;
    type Error = SeError;

    #[inline]
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        SerializeSeq::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl<'w, 'k, W: Write> SerializeTupleStruct for ElementSerializer<'w, 'k, W> {
    type Ok = WriteResult;
    type Error = SeError;

    #[inline]
    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        SerializeSeq::serialize_element(self, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeSeq::end(self)
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A serializer for tuple variants. Tuples can be serialized in two modes:
/// - wrapping each tuple field into a tag
/// - without wrapping, fields are delimited by a space
pub enum Tuple<'w, 'k, W: Write> {
    /// Serialize each tuple field as an element
    Element(ElementSerializer<'w, 'k, W>),
    /// Serialize tuple as an `xs:list`: space-delimited content of fields
    Text(SimpleSeq<&'w mut W>),
}

impl<'w, 'k, W: Write> SerializeTupleVariant for Tuple<'w, 'k, W> {
    type Ok = WriteResult;
    type Error = SeError;

    #[inline]
    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        match self {
            Self::Element(ser) => SerializeTuple::serialize_element(ser, value),
            Self::Text(ser) => SerializeTuple::serialize_element(ser, value),
        }
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self {
            Self::Element(ser) => SerializeTuple::end(ser),
            // Do not write indent after `$text` fields because it may be interpreted as
            // part of content when deserialize
            Self::Text(ser) => SerializeTuple::end(ser).map(|_| WriteResult::SensitiveText),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A serializer for struct variants, which serializes the struct contents inside
/// of wrapping tags (`<${tag}>...</${tag}>`).
///
/// Returns the classification of the last written type.
///
/// Serialization of each field depends on it representation:
/// - attributes written directly to the higher serializer
/// - elements buffered into internal buffer and at the end written into higher
///   serializer
pub struct Struct<'w, 'k, W: Write> {
    ser: ElementSerializer<'w, 'k, W>,
    /// Buffer to store serialized elements
    // TODO: Customization point: allow direct writing of elements, but all
    // attributes should be listed first. Fail, if attribute encountered after
    // element. Use feature to configure
    children: String,
    /// Whether need to write indent after the last written field
    write_indent: bool,
}

impl<'w, 'k, W: Write> Struct<'w, 'k, W> {
    #[inline]
    fn write_field<T>(&mut self, key: &str, value: &T) -> Result<(), SeError>
    where
        T: ?Sized + Serialize,
    {
        //TODO: Customization point: allow user to determine if field is attribute or not
        if let Some(key) = key.strip_prefix('@') {
            let key = XmlName::try_from(key)?;
            self.write_attribute(key, value)
        } else {
            self.write_element(key, value)
        }
    }

    /// Writes `value` as an attribute
    #[inline]
    fn write_attribute<T>(&mut self, key: XmlName, value: &T) -> Result<(), SeError>
    where
        T: ?Sized + Serialize,
    {
        //TODO: Customization point: each attribute on new line
        self.ser.ser.writer.write_char(' ')?;
        self.ser.ser.writer.write_str(key.0)?;
        self.ser.ser.writer.write_char('=')?;

        //TODO: Customization point: preferred quote style
        self.ser.ser.writer.write_char('"')?;
        value.serialize(SimpleTypeSerializer {
            writer: &mut self.ser.ser.writer,
            target: QuoteTarget::DoubleQAttr,
            level: self.ser.ser.level,
        })?;
        self.ser.ser.writer.write_char('"')?;

        Ok(())
    }

    /// Writes `value` either as a text content, or as an element.
    ///
    /// If `key` has a magic value [`TEXT_KEY`], then `value` serialized as a
    /// [simple type].
    ///
    /// If `key` has a magic value [`VALUE_KEY`], then `value` serialized as a
    /// [content] without wrapping in tags, otherwise it is wrapped in
    /// `<${key}>...</${key}>`.
    ///
    /// [simple type]: SimpleTypeSerializer
    /// [content]: ContentSerializer
    fn write_element<T>(&mut self, key: &str, value: &T) -> Result<(), SeError>
    where
        T: ?Sized + Serialize,
    {
        let ser = ContentSerializer {
            writer: &mut self.children,
            level: self.ser.ser.level,
            indent: self.ser.ser.indent.borrow(),
            // If previous field does not require indent, do not write it
            write_indent: self.write_indent,
            allow_primitive: true,
            expand_empty_elements: self.ser.ser.expand_empty_elements,
        };

        if key == TEXT_KEY {
            value.serialize(TextSerializer(ser.into_simple_type_serializer()?))?;
            // Text was written so we don't need to indent next field
            self.write_indent = false;
        } else if key == VALUE_KEY {
            // If element was written then we need to indent next field unless it is a text field
            self.write_indent = value.serialize(ser)?.allow_indent();
        } else {
            value.serialize(ElementSerializer {
                key: XmlName::try_from(key)?,
                ser,
            })?;
            // Element was written so we need to indent next field unless it is a text field
            self.write_indent = true;
        }
        Ok(())
    }
}

impl<'w, 'k, W: Write> SerializeStruct for Struct<'w, 'k, W> {
    type Ok = WriteResult;
    type Error = SeError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.write_field(key, value)
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.ser.ser.indent.decrease();

        if self.children.is_empty() {
            if self.ser.ser.expand_empty_elements {
                self.ser.ser.writer.write_str("></")?;
                self.ser.ser.writer.write_str(self.ser.key.0)?;
                self.ser.ser.writer.write_char('>')?;
            } else {
                self.ser.ser.writer.write_str("/>")?;
            }
        } else {
            self.ser.ser.writer.write_char('>')?;
            self.ser.ser.writer.write_str(&self.children)?;

            if self.write_indent {
                self.ser.ser.indent.write_indent(&mut self.ser.ser.writer)?;
            }

            self.ser.ser.writer.write_str("</")?;
            self.ser.ser.writer.write_str(self.ser.key.0)?;
            self.ser.ser.writer.write_char('>')?;
        }
        Ok(WriteResult::Element)
    }
}

impl<'w, 'k, W: Write> SerializeStructVariant for Struct<'w, 'k, W> {
    type Ok = WriteResult;
    type Error = SeError;

    #[inline]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        SerializeStruct::serialize_field(self, key, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeStruct::end(self)
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct Map<'w, 'k, W: Write> {
    ser: Struct<'w, 'k, W>,
    /// Key, serialized by `QNameSerializer` if consumer uses `serialize_key` +
    /// `serialize_value` calls instead of `serialize_entry`
    key: Option<String>,
}

impl<'w, 'k, W: Write> Map<'w, 'k, W> {
    fn make_key<T>(&mut self, key: &T) -> Result<String, SeError>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(QNameSerializer {
            writer: String::new(),
        })
    }
}

impl<'w, 'k, W: Write> SerializeMap for Map<'w, 'k, W> {
    type Ok = WriteResult;
    type Error = SeError;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        if let Some(_) = self.key.take() {
            return Err(SeError::Custom(
                "calling `serialize_key` twice without `serialize_value`".to_string(),
            ));
        }
        self.key = Some(self.make_key(key)?);
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        if let Some(key) = self.key.take() {
            return self.ser.write_field(&key, value);
        }
        Err(SeError::Custom(
            "calling `serialize_value` without call of `serialize_key`".to_string(),
        ))
    }

    fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<(), Self::Error>
    where
        K: ?Sized + Serialize,
        V: ?Sized + Serialize,
    {
        let key = self.make_key(key)?;
        self.ser.write_field(&key, value)
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        if let Some(key) = self.key.take() {
            return Err(SeError::Custom(format!(
                "calling `end` without call of `serialize_value` for key `{key}`"
            )));
        }
        SerializeStruct::end(self.ser)
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use crate::se::content::tests::*;
    use crate::se::{Indent, QuoteLevel};
    use crate::utils::Bytes;
    use serde::Serialize;
    use std::collections::BTreeMap;

    #[derive(Debug, Serialize, PartialEq)]
    struct OptionalElements {
        a: Option<&'static str>,

        #[serde(skip_serializing_if = "Option::is_none")]
        b: Option<&'static str>,
    }
    #[derive(Debug, Serialize, PartialEq)]
    struct OptionalAttributes {
        #[serde(rename = "@a")]
        a: Option<&'static str>,

        #[serde(rename = "@b")]
        #[serde(skip_serializing_if = "Option::is_none")]
        b: Option<&'static str>,
    }

    mod without_indent {
        use super::*;
        use crate::se::content::tests::Struct;
        use pretty_assertions::assert_eq;

        /// Checks that given `$data` successfully serialized as `$expected`
        macro_rules! serialize_as {
            ($name:ident: $data:expr => $expected:expr) => {
                #[test]
                fn $name() {
                    let mut buffer = String::new();
                    let ser = ElementSerializer {
                        ser: ContentSerializer {
                            writer: &mut buffer,
                            level: QuoteLevel::Full,
                            indent: Indent::None,
                            write_indent: false,
                            allow_primitive: true,
                            expand_empty_elements: false,
                        },
                        key: XmlName("root"),
                    };

                    let result = $data.serialize(ser).unwrap();
                    assert_eq!(buffer, $expected);
                    assert_eq!(result, WriteResult::Element);
                }
            };
        }

        /// Checks that attempt to serialize given `$data` results to a
        /// serialization error `$kind` with `$reason`
        macro_rules! err {
            ($name:ident: $data:expr => $kind:ident($reason:literal)) => {
                #[test]
                fn $name() {
                    let mut buffer = String::new();
                    let ser = ElementSerializer {
                        ser: ContentSerializer {
                            writer: &mut buffer,
                            level: QuoteLevel::Full,
                            indent: Indent::None,
                            write_indent: false,
                            allow_primitive: true,
                            expand_empty_elements: false,
                        },
                        key: XmlName("root"),
                    };

                    match $data.serialize(ser).unwrap_err() {
                        SeError::$kind(e) => assert_eq!(e, $reason),
                        e => panic!(
                            "Expected `Err({}({}))`, but got `{:?}`",
                            stringify!($kind),
                            $reason,
                            e
                        ),
                    }
                    // We can write something before fail
                    // assert_eq!(buffer, "");
                }
            };
        }

        serialize_as!(false_: false => "<root>false</root>");
        serialize_as!(true_:  true  => "<root>true</root>");

        serialize_as!(i8_:    -42i8                => "<root>-42</root>");
        serialize_as!(i16_:   -4200i16             => "<root>-4200</root>");
        serialize_as!(i32_:   -42000000i32         => "<root>-42000000</root>");
        serialize_as!(i64_:   -42000000000000i64   => "<root>-42000000000000</root>");
        serialize_as!(isize_: -42000000000000isize => "<root>-42000000000000</root>");

        serialize_as!(u8_:    42u8                => "<root>42</root>");
        serialize_as!(u16_:   4200u16             => "<root>4200</root>");
        serialize_as!(u32_:   42000000u32         => "<root>42000000</root>");
        serialize_as!(u64_:   42000000000000u64   => "<root>42000000000000</root>");
        serialize_as!(usize_: 42000000000000usize => "<root>42000000000000</root>");

        serde_if_integer128! {
            serialize_as!(i128_: -420000000000000000000000000000i128 => "<root>-420000000000000000000000000000</root>");
            serialize_as!(u128_:  420000000000000000000000000000u128 => "<root>420000000000000000000000000000</root>");
        }

        serialize_as!(f32_: 4.2f32 => "<root>4.2</root>");
        serialize_as!(f64_: 4.2f64 => "<root>4.2</root>");

        serialize_as!(char_non_escaped: 'h' => "<root>h</root>");
        serialize_as!(char_lt:   '<' => "<root>&lt;</root>");
        serialize_as!(char_gt:   '>' => "<root>&gt;</root>");
        serialize_as!(char_amp:  '&' => "<root>&amp;</root>");
        serialize_as!(char_apos: '\'' => "<root>&apos;</root>");
        serialize_as!(char_quot: '"' => "<root>&quot;</root>");

        serialize_as!(str_non_escaped: "non-escaped string" => "<root>non-escaped string</root>");
        serialize_as!(str_escaped: "<\"escaped & string'>" => "<root>&lt;&quot;escaped &amp; string&apos;&gt;</root>");

        err!(bytes: Bytes(b"<\"escaped & bytes'>") => Unsupported("`serialize_bytes` not supported yet"));

        serialize_as!(option_none: Option::<&str>::None => "<root/>");
        serialize_as!(option_some: Some("non-escaped string") => "<root>non-escaped string</root>");
        serialize_as!(option_some_empty_str: Some("") => "<root/>");

        serialize_as!(unit: () => "<root/>");
        serialize_as!(unit_struct: Unit => "<root/>");
        serialize_as!(unit_struct_escaped: UnitEscaped => "<root/>");

        serialize_as!(enum_unit: Enum::Unit => "<root>Unit</root>");
        serialize_as!(enum_unit_escaped: Enum::UnitEscaped => "<root>&lt;&quot;&amp;&apos;&gt;</root>");

        serialize_as!(newtype: Newtype(42) => "<root>42</root>");
        err!(enum_newtype: Enum::Newtype(42)
            => Unsupported("cannot serialize enum newtype variant `Enum::Newtype`"));

        serialize_as!(seq: vec![1, 2, 3]
            => "<root>1</root>\
                <root>2</root>\
                <root>3</root>");
        serialize_as!(seq_empty: Vec::<usize>::new() => "");
        serialize_as!(tuple: ("<\"&'>", "with\t\n\r spaces", 3usize)
            => "<root>&lt;&quot;&amp;&apos;&gt;</root>\
                <root>with\t\n\r spaces</root>\
                <root>3</root>");
        serialize_as!(tuple_struct: Tuple("first", 42)
            => "<root>first</root>\
                <root>42</root>");
        err!(enum_tuple: Enum::Tuple("first", 42)
            => Unsupported("cannot serialize enum tuple variant `Enum::Tuple`"));

        serialize_as!(map: BTreeMap::from([("_1", 2), ("_3", 4)])
            => "<root>\
                    <_1>2</_1>\
                    <_3>4</_3>\
                </root>");
        serialize_as!(struct_: Struct { key: "answer", val: (42, 42) }
            => "<root>\
                    <key>answer</key>\
                    <val>42</val>\
                    <val>42</val>\
                </root>");
        err!(enum_struct: Enum::Struct { key: "answer", val: (42, 42) }
            => Unsupported("cannot serialize enum struct variant `Enum::Struct`"));

        /// Special field name `$text` should be serialized as text content.
        /// Sequences serialized as an `xs:list` content
        mod text_field {
            use super::*;

            /// `$text` key in a map
            mod map {
                use super::*;
                use pretty_assertions::assert_eq;

                macro_rules! text {
                    ($name:ident: $data:expr) => {
                        serialize_as!($name:
                            BTreeMap::from([("$text", $data)])
                            => "<root/>");
                    };
                    ($name:ident: $data:expr => $expected:literal) => {
                        serialize_as!($name:
                            BTreeMap::from([("$text", $data)])
                            => concat!("<root>", $expected,"</root>"));
                    };
                }

                text!(false_: false => "false");
                text!(true_:  true  => "true");

                text!(i8_:    -42i8                => "-42");
                text!(i16_:   -4200i16             => "-4200");
                text!(i32_:   -42000000i32         => "-42000000");
                text!(i64_:   -42000000000000i64   => "-42000000000000");
                text!(isize_: -42000000000000isize => "-42000000000000");

                text!(u8_:    42u8                => "42");
                text!(u16_:   4200u16             => "4200");
                text!(u32_:   42000000u32         => "42000000");
                text!(u64_:   42000000000000u64   => "42000000000000");
                text!(usize_: 42000000000000usize => "42000000000000");

                serde_if_integer128! {
                    text!(i128_: -420000000000000000000000000000i128 => "-420000000000000000000000000000");
                    text!(u128_:  420000000000000000000000000000u128 => "420000000000000000000000000000");
                }

                text!(f32_: 4.2f32 => "4.2");
                text!(f64_: 4.2f64 => "4.2");

                text!(char_non_escaped: 'h' => "h");
                text!(char_lt:   '<' => "&lt;");
                text!(char_gt:   '>' => "&gt;");
                text!(char_amp:  '&' => "&amp;");
                text!(char_apos: '\'' => "&apos;");
                text!(char_quot: '"' => "&quot;");
                text!(char_space: ' ' => " ");

                text!(str_non_escaped: "non-escaped string" => "non-escaped string");
                text!(str_escaped: "<\"escaped & string'>" => "&lt;&quot;escaped &amp; string&apos;&gt;");

                err!(bytes:
                    Text {
                        before: "answer",
                        content: Bytes(b"<\"escaped & bytes'>"),
                        after: "answer",
                    }
                    => Unsupported("`serialize_bytes` not supported yet"));

                text!(option_none: Option::<&str>::None);
                text!(option_some: Some("non-escaped string") => "non-escaped string");
                text!(option_some_empty_str: Some(""));

                text!(unit: ());
                text!(unit_struct: Unit);
                text!(unit_struct_escaped: UnitEscaped);

                text!(enum_unit: Enum::Unit => "Unit");
                text!(enum_unit_escaped: Enum::UnitEscaped => "&lt;&quot;&amp;&apos;&gt;");

                text!(newtype: Newtype(42) => "42");
                // We have no space where name of a variant can be stored
                err!(enum_newtype:
                    Text {
                        before: "answer",
                        content: Enum::Newtype(42),
                        after: "answer",
                    }
                    => Unsupported("cannot serialize enum newtype variant `Enum::Newtype` as text content value"));

                // Sequences are serialized separated by spaces, all spaces inside are escaped
                text!(seq: vec![1, 2, 3] => "1 2 3");
                text!(seq_empty: Vec::<usize>::new());
                text!(tuple: ("<\"&'>", "with\t\n\r spaces", 3usize)
                    => "&lt;&quot;&amp;&apos;&gt; \
                        with&#9;&#10;&#13;&#32;spaces \
                        3");
                text!(tuple_struct: Tuple("first", 42) => "first 42");
                // We have no space where name of a variant can be stored
                err!(enum_tuple:
                    Text {
                        before: "answer",
                        content: Enum::Tuple("first", 42),
                        after: "answer",
                    }
                    => Unsupported("cannot serialize enum tuple variant `Enum::Tuple` as text content value"));

                // Complex types cannot be serialized in `$text` field
                err!(map:
                    Text {
                        before: "answer",
                        content: BTreeMap::from([("_1", 2), ("_3", 4)]),
                        after: "answer",
                    }
                    => Unsupported("cannot serialize map as text content value"));
                err!(struct_:
                    Text {
                        before: "answer",
                        content: Struct { key: "answer", val: (42, 42) },
                        after: "answer",
                    }
                    => Unsupported("cannot serialize struct `Struct` as text content value"));
                err!(enum_struct:
                    Text {
                        before: "answer",
                        content: Enum::Struct { key: "answer", val: (42, 42) },
                        after: "answer",
                    }
                    => Unsupported("cannot serialize enum struct variant `Enum::Struct` as text content value"));
            }

            /// `$text` field inside a struct
            mod struct_ {
                use super::*;
                use pretty_assertions::assert_eq;

                macro_rules! text {
                    ($name:ident: $data:expr => $expected:literal) => {
                        serialize_as!($name:
                            Text {
                                before: "answer",
                                content: $data,
                                after: "answer",
                            }
                            => concat!(
                                "<root><before>answer</before>",
                                $expected,
                                "<after>answer</after></root>",
                            ));
                    };
                }

                text!(false_: false => "false");
                text!(true_:  true  => "true");

                text!(i8_:    -42i8                => "-42");
                text!(i16_:   -4200i16             => "-4200");
                text!(i32_:   -42000000i32         => "-42000000");
                text!(i64_:   -42000000000000i64   => "-42000000000000");
                text!(isize_: -42000000000000isize => "-42000000000000");

                text!(u8_:    42u8                => "42");
                text!(u16_:   4200u16             => "4200");
                text!(u32_:   42000000u32         => "42000000");
                text!(u64_:   42000000000000u64   => "42000000000000");
                text!(usize_: 42000000000000usize => "42000000000000");

                serde_if_integer128! {
                    text!(i128_: -420000000000000000000000000000i128 => "-420000000000000000000000000000");
                    text!(u128_:  420000000000000000000000000000u128 => "420000000000000000000000000000");
                }

                text!(f32_: 4.2f32 => "4.2");
                text!(f64_: 4.2f64 => "4.2");

                text!(char_non_escaped: 'h' => "h");
                text!(char_lt:   '<' => "&lt;");
                text!(char_gt:   '>' => "&gt;");
                text!(char_amp:  '&' => "&amp;");
                text!(char_apos: '\'' => "&apos;");
                text!(char_quot: '"' => "&quot;");
                text!(char_space: ' ' => " ");

                text!(str_non_escaped: "non-escaped string" => "non-escaped string");
                text!(str_escaped: "<\"escaped & string'>" => "&lt;&quot;escaped &amp; string&apos;&gt;");

                err!(bytes:
                    Text {
                        before: "answer",
                        content: Bytes(b"<\"escaped & bytes'>"),
                        after: "answer",
                    }
                    => Unsupported("`serialize_bytes` not supported yet"));

                text!(option_none: Option::<&str>::None => "");
                text!(option_some: Some("non-escaped string") => "non-escaped string");
                text!(option_some_empty_str: Some("") => "");

                text!(unit: () => "");
                text!(unit_struct: Unit => "");
                text!(unit_struct_escaped: UnitEscaped => "");

                text!(enum_unit: Enum::Unit => "Unit");
                text!(enum_unit_escaped: Enum::UnitEscaped => "&lt;&quot;&amp;&apos;&gt;");

                text!(newtype: Newtype(42) => "42");
                // We have no space where name of a variant can be stored
                err!(enum_newtype:
                    Text {
                        before: "answer",
                        content: Enum::Newtype(42),
                        after: "answer",
                    }
                    => Unsupported("cannot serialize enum newtype variant `Enum::Newtype` as text content value"));

                // Sequences are serialized separated by spaces, all spaces inside are escaped
                text!(seq: vec![1, 2, 3] => "1 2 3");
                text!(seq_empty: Vec::<usize>::new() => "");
                text!(tuple: ("<\"&'>", "with\t\n\r spaces", 3usize)
                    => "&lt;&quot;&amp;&apos;&gt; \
                        with&#9;&#10;&#13;&#32;spaces \
                        3");
                text!(tuple_struct: Tuple("first", 42) => "first 42");
                // We have no space where name of a variant can be stored
                err!(enum_tuple:
                    Text {
                        before: "answer",
                        content: Enum::Tuple("first", 42),
                        after: "answer",
                    }
                    => Unsupported("cannot serialize enum tuple variant `Enum::Tuple` as text content value"));

                // Complex types cannot be serialized in `$text` field
                err!(map:
                    Text {
                        before: "answer",
                        content: BTreeMap::from([("_1", 2), ("_3", 4)]),
                        after: "answer",
                    }
                    => Unsupported("cannot serialize map as text content value"));
                err!(struct_:
                    Text {
                        before: "answer",
                        content: Struct { key: "answer", val: (42, 42) },
                        after: "answer",
                    }
                    => Unsupported("cannot serialize struct `Struct` as text content value"));
                err!(enum_struct:
                    Text {
                        before: "answer",
                        content: Enum::Struct { key: "answer", val: (42, 42) },
                        after: "answer",
                    }
                    => Unsupported("cannot serialize enum struct variant `Enum::Struct` as text content value"));
            }
        }

        /// Special field name `$value` should be serialized using name, provided
        /// by the type of value instead of a key. Sequences serialized as a list
        /// of tags with that name (each element can have their own name)
        mod value_field {
            use super::*;

            /// `$value` key in a map
            mod map {
                use super::*;
                use pretty_assertions::assert_eq;

                macro_rules! value {
                    ($name:ident: $data:expr) => {
                        serialize_as!($name:
                            BTreeMap::from([("$value", $data)])
                            => "<root/>");
                    };
                    ($name:ident: $data:expr => $expected:literal) => {
                        serialize_as!($name:
                            BTreeMap::from([("$value", $data)])
                            => concat!("<root>", $expected,"</root>"));
                    };
                }

                value!(false_: false => "false");
                value!(true_:  true  => "true");

                value!(i8_:    -42i8                => "-42");
                value!(i16_:   -4200i16             => "-4200");
                value!(i32_:   -42000000i32         => "-42000000");
                value!(i64_:   -42000000000000i64   => "-42000000000000");
                value!(isize_: -42000000000000isize => "-42000000000000");

                value!(u8_:    42u8                => "42");
                value!(u16_:   4200u16             => "4200");
                value!(u32_:   42000000u32         => "42000000");
                value!(u64_:   42000000000000u64   => "42000000000000");
                value!(usize_: 42000000000000usize => "42000000000000");

                serde_if_integer128! {
                    value!(i128_: -420000000000000000000000000000i128 => "-420000000000000000000000000000");
                    value!(u128_:  420000000000000000000000000000u128 => "420000000000000000000000000000");
                }

                value!(f32_: 4.2f32 => "4.2");
                value!(f64_: 4.2f64 => "4.2");

                value!(char_non_escaped: 'h' => "h");
                value!(char_lt:   '<' => "&lt;");
                value!(char_gt:   '>' => "&gt;");
                value!(char_amp:  '&' => "&amp;");
                value!(char_apos: '\'' => "&apos;");
                value!(char_quot: '"' => "&quot;");
                value!(char_space: ' ' => " ");

                value!(str_non_escaped: "non-escaped string" => "non-escaped string");
                value!(str_escaped: "<\"escaped & string'>" => "&lt;&quot;escaped &amp; string&apos;&gt;");

                err!(bytes:
                    BTreeMap::from([("$value", Bytes(b"<\"escaped & bytes'>"))])
                    => Unsupported("`serialize_bytes` not supported yet"));

                value!(option_none: Option::<&str>::None);
                value!(option_some: Some("non-escaped string") => "non-escaped string");
                value!(option_some_empty_str: Some(""));

                value!(unit: ());
                value!(unit_struct: Unit);
                value!(unit_struct_escaped: UnitEscaped);

                value!(enum_unit: Enum::Unit => "<Unit/>");
                err!(enum_unit_escaped:
                    BTreeMap::from([("$value", Enum::UnitEscaped)])
                    => Unsupported("character `<` is not allowed at the start of an XML name `<\"&'>`"));

                value!(newtype: Newtype(42) => "42");
                value!(enum_newtype: Enum::Newtype(42) => "<Newtype>42</Newtype>");

                // Note that sequences of primitives serialized without delimiters!
                err!(seq:
                    BTreeMap::from([("$value", vec![1, 2, 3])])
                    => Unsupported("consequent primitives would be serialized without delimiter and cannot be deserialized back"));
                value!(seq_empty: Vec::<usize>::new());
                err!(tuple:
                    BTreeMap::from([("$value", ("<\"&'>", "with\t\n\r spaces", 3usize))])
                    => Unsupported("consequent primitives would be serialized without delimiter and cannot be deserialized back"));
                err!(tuple_struct:
                    BTreeMap::from([("$value", Tuple("first", 42))])
                    => Unsupported("consequent primitives would be serialized without delimiter and cannot be deserialized back"));
                value!(enum_tuple: Enum::Tuple("first", 42)
                    => "<Tuple>first</Tuple>\
                        <Tuple>42</Tuple>");

                // We cannot wrap map or struct in any container and should not
                // flatten it, so it is impossible to serialize maps and structs
                err!(map:
                    BTreeMap::from([("$value", BTreeMap::from([("_1", 2), ("_3", 4)]))])
                    => Unsupported("serialization of map types is not supported in `$value` field"));
                err!(struct_:
                    BTreeMap::from([("$value", Struct { key: "answer", val: (42, 42) })])
                    => Unsupported("serialization of struct `Struct` is not supported in `$value` field"));
                value!(enum_struct:
                    Enum::Struct { key: "answer", val: (42, 42) }
                    => "<Struct>\
                            <key>answer</key>\
                            <val>42</val>\
                            <val>42</val>\
                        </Struct>");
            }

            /// `$value` field inside a struct
            mod struct_ {
                use super::*;
                use pretty_assertions::assert_eq;

                macro_rules! value {
                    ($name:ident: $data:expr => $expected:literal) => {
                        serialize_as!($name:
                            Value {
                                before: "answer",
                                content: $data,
                                after: "answer",
                            }
                            => concat!(
                                "<root><before>answer</before>",
                                $expected,
                                "<after>answer</after></root>",
                            ));
                    };
                }

                value!(false_: false => "false");
                value!(true_:  true  => "true");

                value!(i8_:    -42i8                => "-42");
                value!(i16_:   -4200i16             => "-4200");
                value!(i32_:   -42000000i32         => "-42000000");
                value!(i64_:   -42000000000000i64   => "-42000000000000");
                value!(isize_: -42000000000000isize => "-42000000000000");

                value!(u8_:    42u8                => "42");
                value!(u16_:   4200u16             => "4200");
                value!(u32_:   42000000u32         => "42000000");
                value!(u64_:   42000000000000u64   => "42000000000000");
                value!(usize_: 42000000000000usize => "42000000000000");

                serde_if_integer128! {
                    value!(i128_: -420000000000000000000000000000i128 => "-420000000000000000000000000000");
                    value!(u128_:  420000000000000000000000000000u128 => "420000000000000000000000000000");
                }

                value!(f32_: 4.2f32 => "4.2");
                value!(f64_: 4.2f64 => "4.2");

                value!(char_non_escaped: 'h' => "h");
                value!(char_lt:   '<' => "&lt;");
                value!(char_gt:   '>' => "&gt;");
                value!(char_amp:  '&' => "&amp;");
                value!(char_apos: '\'' => "&apos;");
                value!(char_quot: '"' => "&quot;");
                value!(char_space: ' ' => " ");

                value!(str_non_escaped: "non-escaped string" => "non-escaped string");
                value!(str_escaped: "<\"escaped & string'>" => "&lt;&quot;escaped &amp; string&apos;&gt;");

                err!(bytes:
                    Value {
                        before: "answer",
                        content: Bytes(b"<\"escaped & bytes'>"),
                        after: "answer",
                    }
                    => Unsupported("`serialize_bytes` not supported yet"));

                value!(option_none: Option::<&str>::None => "");
                value!(option_some: Some("non-escaped string") => "non-escaped string");
                value!(option_some_empty_str: Some("") => "");

                value!(unit: () => "");
                value!(unit_struct: Unit => "");
                value!(unit_struct_escaped: UnitEscaped => "");

                value!(enum_unit: Enum::Unit => "<Unit/>");
                err!(enum_unit_escaped:
                    Value {
                        before: "answer",
                        content: Enum::UnitEscaped,
                        after: "answer",
                    }
                    => Unsupported("character `<` is not allowed at the start of an XML name `<\"&'>`"));

                value!(newtype: Newtype(42) => "42");
                value!(enum_newtype: Enum::Newtype(42) => "<Newtype>42</Newtype>");

                // Note that sequences of primitives serialized without delimiters!
                err!(seq:
                    Value {
                        before: "answer",
                        content: vec![1, 2, 3],
                        after: "answer",
                    }
                    => Unsupported("consequent primitives would be serialized without delimiter and cannot be deserialized back"));
                value!(seq_empty: Vec::<usize>::new() => "");
                err!(tuple:
                    Value {
                        before: "answer",
                        content: ("<\"&'>", "with\t\n\r spaces", 3usize),
                        after: "answer",
                    }
                    => Unsupported("consequent primitives would be serialized without delimiter and cannot be deserialized back"));
                err!(tuple_struct:
                    Value {
                        before: "answer",
                        content: Tuple("first", 42),
                        after: "answer",
                    }
                    => Unsupported("consequent primitives would be serialized without delimiter and cannot be deserialized back"));
                value!(enum_tuple: Enum::Tuple("first", 42)
                    => "<Tuple>first</Tuple>\
                        <Tuple>42</Tuple>");

                // We cannot wrap map or struct in any container and should not
                // flatten it, so it is impossible to serialize maps and structs
                err!(map:
                    Value {
                        before: "answer",
                        content: BTreeMap::from([("_1", 2), ("_3", 4)]),
                        after: "answer",
                    }
                    => Unsupported("serialization of map types is not supported in `$value` field"));
                err!(struct_:
                    Value {
                        before: "answer",
                        content: Struct { key: "answer", val: (42, 42) },
                        after: "answer",
                    }
                    => Unsupported("serialization of struct `Struct` is not supported in `$value` field"));
                value!(enum_struct:
                    Enum::Struct { key: "answer", val: (42, 42) }
                    => "<Struct>\
                            <key>answer</key>\
                            <val>42</val>\
                            <val>42</val>\
                        </Struct>");
            }
        }

        mod attributes {
            use super::*;
            use pretty_assertions::assert_eq;

            serialize_as!(map_attr: BTreeMap::from([("@key1", 1), ("@key2", 2)])
                => r#"<root key1="1" key2="2"/>"#);
            serialize_as!(map_mixed: BTreeMap::from([("@key1", 1), ("key2", 2)])
                => r#"<root key1="1"><key2>2</key2></root>"#);

            serialize_as!(struct_: Attributes { key: "answer", val: (42, 42) }
                => r#"<root key="answer" val="42 42"/>"#);
            serialize_as!(struct_before: AttributesBefore { key: "answer", val: 42 }
                => r#"<root key="answer"><val>42</val></root>"#);
            serialize_as!(struct_after: AttributesAfter { key: "answer", val: 42 }
                => r#"<root val="42"><key>answer</key></root>"#);

            err!(enum_: Enum::Attributes { key: "answer", val: (42, 42) }
                => Unsupported("cannot serialize enum struct variant `Enum::Attributes`"));

            /// Test for https://github.com/tafia/quick-xml/issues/252
            mod optional {
                use super::*;
                use pretty_assertions::assert_eq;

                serialize_as!(none:
                    OptionalAttributes { a: None, b: None }
                    => r#"<root a=""/>"#);
                serialize_as!(some_empty_str:
                    OptionalAttributes {
                        a: Some(""),
                        b: Some(""),
                    }
                    => r#"<root a="" b=""/>"#);
                serialize_as!(some_non_empty:
                    OptionalAttributes {
                        a: Some("1"),
                        b: Some("2"),
                    }
                    => r#"<root a="1" b="2"/>"#);
            }
        }

        /// Test for https://github.com/tafia/quick-xml/issues/252
        mod optional {
            use super::*;
            use pretty_assertions::assert_eq;

            serialize_as!(none:
                OptionalElements { a: None, b: None }
                => "<root>\
                        <a/>\
                    </root>");
            serialize_as!(some_empty_str:
                OptionalElements {
                    a: Some(""),
                    b: Some(""),
                }
                => "<root>\
                        <a/>\
                        <b/>\
                    </root>");
            serialize_as!(some_non_empty:
                OptionalElements {
                    a: Some("1"),
                    b: Some("2"),
                }
                => "<root>\
                        <a>1</a>\
                        <b>2</b>\
                    </root>");
        }
    }

    mod with_indent {
        use super::*;
        use crate::se::content::tests::Struct;
        use crate::writer::Indentation;
        use pretty_assertions::assert_eq;

        /// Checks that given `$data` successfully serialized as `$expected`.
        /// Writes `$data` using [`ElementSerializer`] with indent of two spaces.
        macro_rules! serialize_as {
            ($name:ident: $data:expr => $expected:expr) => {
                #[test]
                fn $name() {
                    let mut buffer = String::new();
                    let ser = ElementSerializer {
                        ser: ContentSerializer {
                            writer: &mut buffer,
                            level: QuoteLevel::Full,
                            indent: Indent::Owned(Indentation::new(b' ', 2)),
                            write_indent: false,
                            allow_primitive: true,
                            expand_empty_elements: false,
                        },
                        key: XmlName("root"),
                    };

                    let result = $data.serialize(ser).unwrap();
                    assert_eq!(buffer, $expected);
                    assert_eq!(result, WriteResult::Element);
                }
            };
        }

        /// Checks that attempt to serialize given `$data` results to a
        /// serialization error `$kind` with `$reason`
        macro_rules! err {
            ($name:ident: $data:expr => $kind:ident($reason:literal)) => {
                #[test]
                fn $name() {
                    let mut buffer = String::new();
                    let ser = ElementSerializer {
                        ser: ContentSerializer {
                            writer: &mut buffer,
                            level: QuoteLevel::Full,
                            indent: Indent::Owned(Indentation::new(b' ', 2)),
                            write_indent: false,
                            allow_primitive: true,
                            expand_empty_elements: false,
                        },
                        key: XmlName("root"),
                    };

                    match $data.serialize(ser).unwrap_err() {
                        SeError::$kind(e) => assert_eq!(e, $reason),
                        e => panic!(
                            "Expected `Err({}({}))`, but got `{:?}`",
                            stringify!($kind),
                            $reason,
                            e
                        ),
                    }
                    // We can write something before fail
                    // assert_eq!(buffer, "");
                }
            };
        }

        serialize_as!(false_: false => "<root>false</root>");
        serialize_as!(true_:  true  => "<root>true</root>");

        serialize_as!(i8_:    -42i8                => "<root>-42</root>");
        serialize_as!(i16_:   -4200i16             => "<root>-4200</root>");
        serialize_as!(i32_:   -42000000i32         => "<root>-42000000</root>");
        serialize_as!(i64_:   -42000000000000i64   => "<root>-42000000000000</root>");
        serialize_as!(isize_: -42000000000000isize => "<root>-42000000000000</root>");

        serialize_as!(u8_:    42u8                => "<root>42</root>");
        serialize_as!(u16_:   4200u16             => "<root>4200</root>");
        serialize_as!(u32_:   42000000u32         => "<root>42000000</root>");
        serialize_as!(u64_:   42000000000000u64   => "<root>42000000000000</root>");
        serialize_as!(usize_: 42000000000000usize => "<root>42000000000000</root>");

        serde_if_integer128! {
            serialize_as!(i128_: -420000000000000000000000000000i128 => "<root>-420000000000000000000000000000</root>");
            serialize_as!(u128_:  420000000000000000000000000000u128 => "<root>420000000000000000000000000000</root>");
        }

        serialize_as!(f32_: 4.2f32 => "<root>4.2</root>");
        serialize_as!(f64_: 4.2f64 => "<root>4.2</root>");

        serialize_as!(char_non_escaped: 'h' => "<root>h</root>");
        serialize_as!(char_lt:   '<' => "<root>&lt;</root>");
        serialize_as!(char_gt:   '>' => "<root>&gt;</root>");
        serialize_as!(char_amp:  '&' => "<root>&amp;</root>");
        serialize_as!(char_apos: '\'' => "<root>&apos;</root>");
        serialize_as!(char_quot: '"' => "<root>&quot;</root>");
        serialize_as!(char_space: ' ' => "<root> </root>");

        serialize_as!(str_non_escaped: "non-escaped string" => "<root>non-escaped string</root>");
        serialize_as!(str_escaped: "<\"escaped & string'>" => "<root>&lt;&quot;escaped &amp; string&apos;&gt;</root>");

        err!(bytes: Bytes(b"<\"escaped & bytes'>") => Unsupported("`serialize_bytes` not supported yet"));

        serialize_as!(option_none: Option::<&str>::None => "<root/>");
        serialize_as!(option_some: Some("non-escaped string") => "<root>non-escaped string</root>");
        serialize_as!(option_some_empty: Some("") => "<root/>");

        serialize_as!(unit: () => "<root/>");
        serialize_as!(unit_struct: Unit => "<root/>");
        serialize_as!(unit_struct_escaped: UnitEscaped => "<root/>");

        serialize_as!(enum_unit: Enum::Unit => "<root>Unit</root>");
        serialize_as!(enum_unit_escaped: Enum::UnitEscaped => "<root>&lt;&quot;&amp;&apos;&gt;</root>");

        serialize_as!(newtype: Newtype(42) => "<root>42</root>");
        err!(enum_newtype: Enum::Newtype(42)
            => Unsupported("cannot serialize enum newtype variant `Enum::Newtype`"));

        serialize_as!(seq: vec![1, 2, 3]
            => "<root>1</root>\n\
                <root>2</root>\n\
                <root>3</root>");
        serialize_as!(seq_empty: Vec::<usize>::new() => "");
        serialize_as!(tuple: ("<\"&'>", "with\t\n\r spaces", 3usize)
            => "<root>&lt;&quot;&amp;&apos;&gt;</root>\n\
                <root>with\t\n\r spaces</root>\n\
                <root>3</root>");
        serialize_as!(tuple_struct: Tuple("first", 42)
            => "<root>first</root>\n\
                <root>42</root>");
        err!(enum_tuple: Enum::Tuple("first", 42)
            => Unsupported("cannot serialize enum tuple variant `Enum::Tuple`"));

        serialize_as!(map: BTreeMap::from([("_1", 2), ("_3", 4)])
            => "<root>\n  \
                    <_1>2</_1>\n  \
                    <_3>4</_3>\n\
                </root>");
        serialize_as!(struct_: Struct { key: "answer", val: (42, 42) }
            => "<root>\n  \
                    <key>answer</key>\n  \
                    <val>42</val>\n  \
                    <val>42</val>\n\
                </root>");
        err!(enum_struct: Enum::Struct { key: "answer", val: (42, 42) }
            => Unsupported("cannot serialize enum struct variant `Enum::Struct`"));

        /// Special field name `$text` should be serialized as text content.
        /// Sequences serialized as an `xs:list` content
        mod text_field {
            use super::*;

            /// `$text` key in a map
            mod map {
                use super::*;
                use pretty_assertions::assert_eq;

                macro_rules! text {
                    ($name:ident: $data:expr) => {
                        serialize_as!($name:
                            // Serialization started from ElementSerializer::serialize_map
                            BTreeMap::from([("$text", $data)])
                            => "<root/>");
                    };
                    ($name:ident: $data:expr => $expected:literal) => {
                        serialize_as!($name:
                            // Serialization started from ElementSerializer::serialize_map
                            BTreeMap::from([("$text", $data)])
                            => concat!("<root>", $expected,"</root>"));
                    };
                }

                text!(false_: false => "false");
                text!(true_:  true  => "true");

                text!(i8_:    -42i8                => "-42");
                text!(i16_:   -4200i16             => "-4200");
                text!(i32_:   -42000000i32         => "-42000000");
                text!(i64_:   -42000000000000i64   => "-42000000000000");
                text!(isize_: -42000000000000isize => "-42000000000000");

                text!(u8_:    42u8                => "42");
                text!(u16_:   4200u16             => "4200");
                text!(u32_:   42000000u32         => "42000000");
                text!(u64_:   42000000000000u64   => "42000000000000");
                text!(usize_: 42000000000000usize => "42000000000000");

                serde_if_integer128! {
                    text!(i128_: -420000000000000000000000000000i128 => "-420000000000000000000000000000");
                    text!(u128_:  420000000000000000000000000000u128 => "420000000000000000000000000000");
                }

                text!(f32_: 4.2f32 => "4.2");
                text!(f64_: 4.2f64 => "4.2");

                text!(char_non_escaped: 'h' => "h");
                text!(char_lt:   '<' => "&lt;");
                text!(char_gt:   '>' => "&gt;");
                text!(char_amp:  '&' => "&amp;");
                text!(char_apos: '\'' => "&apos;");
                text!(char_quot: '"' => "&quot;");
                text!(char_space: ' ' => " ");

                text!(str_non_escaped: "non-escaped string" => "non-escaped string");
                text!(str_escaped: "<\"escaped & string'>" => "&lt;&quot;escaped &amp; string&apos;&gt;");

                err!(bytes:
                    Text {
                        before: "answer",
                        content: Bytes(b"<\"escaped & bytes'>"),
                        after: "answer",
                    }
                    => Unsupported("`serialize_bytes` not supported yet"));

                text!(option_none: Option::<&str>::None);
                text!(option_some: Some("non-escaped string") => "non-escaped string");
                text!(option_some_empty_str: Some(""));

                text!(unit: ());
                text!(unit_struct: Unit);
                text!(unit_struct_escaped: UnitEscaped);

                text!(enum_unit: Enum::Unit => "Unit");
                text!(enum_unit_escaped: Enum::UnitEscaped => "&lt;&quot;&amp;&apos;&gt;");

                text!(newtype: Newtype(42) => "42");
                // We have no space where name of a variant can be stored
                err!(enum_newtype:
                    Text {
                        before: "answer",
                        content: Enum::Newtype(42),
                        after: "answer",
                    }
                    => Unsupported("cannot serialize enum newtype variant `Enum::Newtype` as text content value"));

                // Sequences are serialized separated by spaces, all spaces inside are escaped
                text!(seq: vec![1, 2, 3] => "1 2 3");
                text!(seq_empty: Vec::<usize>::new());
                text!(tuple: ("<\"&'>", "with\t\n\r spaces", 3usize)
                    => "&lt;&quot;&amp;&apos;&gt; \
                        with&#9;&#10;&#13;&#32;spaces \
                        3");
                text!(tuple_struct: Tuple("first", 42) => "first 42");
                // We have no space where name of a variant can be stored
                err!(enum_tuple:
                    Text {
                        before: "answer",
                        content: Enum::Tuple("first", 42),
                        after: "answer",
                    }
                    => Unsupported("cannot serialize enum tuple variant `Enum::Tuple` as text content value"));

                // Complex types cannot be serialized in `$text` field
                err!(map:
                    Text {
                        before: "answer",
                        content: BTreeMap::from([("_1", 2), ("_3", 4)]),
                        after: "answer",
                    }
                    => Unsupported("cannot serialize map as text content value"));
                err!(struct_:
                    Text {
                        before: "answer",
                        content: Struct { key: "answer", val: (42, 42) },
                        after: "answer",
                    }
                    => Unsupported("cannot serialize struct `Struct` as text content value"));
                err!(enum_struct:
                    Text {
                        before: "answer",
                        content: Enum::Struct { key: "answer", val: (42, 42) },
                        after: "answer",
                    }
                    => Unsupported("cannot serialize enum struct variant `Enum::Struct` as text content value"));
            }

            /// `$text` field inside a struct
            mod struct_ {
                use super::*;
                use pretty_assertions::assert_eq;

                macro_rules! text {
                    ($name:ident: $data:expr => $expected:literal) => {
                        serialize_as!($name:
                            // Serialization started from ElementSerializer::serialize_struct
                            Text {
                                before: "answer",
                                content: $data,
                                after: "answer",
                            }
                            => concat!(
                                "<root>\n  <before>answer</before>",
                                $expected,
                                "<after>answer</after>\n</root>",
                            ));
                    };
                }

                text!(false_: false => "false");
                text!(true_:  true  => "true");

                text!(i8_:    -42i8                => "-42");
                text!(i16_:   -4200i16             => "-4200");
                text!(i32_:   -42000000i32         => "-42000000");
                text!(i64_:   -42000000000000i64   => "-42000000000000");
                text!(isize_: -42000000000000isize => "-42000000000000");

                text!(u8_:    42u8                => "42");
                text!(u16_:   4200u16             => "4200");
                text!(u32_:   42000000u32         => "42000000");
                text!(u64_:   42000000000000u64   => "42000000000000");
                text!(usize_: 42000000000000usize => "42000000000000");

                serde_if_integer128! {
                    text!(i128_: -420000000000000000000000000000i128 => "-420000000000000000000000000000");
                    text!(u128_:  420000000000000000000000000000u128 => "420000000000000000000000000000");
                }

                text!(f32_: 4.2f32 => "4.2");
                text!(f64_: 4.2f64 => "4.2");

                text!(char_non_escaped: 'h' => "h");
                text!(char_lt:   '<' => "&lt;");
                text!(char_gt:   '>' => "&gt;");
                text!(char_amp:  '&' => "&amp;");
                text!(char_apos: '\'' => "&apos;");
                text!(char_quot: '"' => "&quot;");
                text!(char_space: ' ' => " ");

                text!(str_non_escaped: "non-escaped string" => "non-escaped string");
                text!(str_escaped: "<\"escaped & string'>" => "&lt;&quot;escaped &amp; string&apos;&gt;");

                err!(bytes:
                    Text {
                        before: "answer",
                        content: Bytes(b"<\"escaped & bytes'>"),
                        after: "answer",
                    }
                    => Unsupported("`serialize_bytes` not supported yet"));

                text!(option_none: Option::<&str>::None => "");
                text!(option_some: Some("non-escaped string") => "non-escaped string");
                text!(option_some_empty_str: Some("") => "");

                text!(unit: () => "");
                text!(unit_struct: Unit => "");
                text!(unit_struct_escaped: UnitEscaped => "");

                text!(enum_unit: Enum::Unit => "Unit");
                text!(enum_unit_escaped: Enum::UnitEscaped => "&lt;&quot;&amp;&apos;&gt;");

                text!(newtype: Newtype(42) => "42");
                // We have no space where name of a variant can be stored
                err!(enum_newtype:
                    Text {
                        before: "answer",
                        content: Enum::Newtype(42),
                        after: "answer",
                    }
                    => Unsupported("cannot serialize enum newtype variant `Enum::Newtype` as text content value"));

                // Sequences are serialized separated by spaces, all spaces inside are escaped
                text!(seq: vec![1, 2, 3] => "1 2 3");
                text!(seq_empty: Vec::<usize>::new() => "");
                text!(tuple: ("<\"&'>", "with\t\n\r spaces", 3usize)
                    => "&lt;&quot;&amp;&apos;&gt; \
                        with&#9;&#10;&#13;&#32;spaces \
                        3");
                text!(tuple_struct: Tuple("first", 42) => "first 42");
                // We have no space where name of a variant can be stored
                err!(enum_tuple:
                    Text {
                        before: "answer",
                        content: Enum::Tuple("first", 42),
                        after: "answer",
                    }
                    => Unsupported("cannot serialize enum tuple variant `Enum::Tuple` as text content value"));

                // Complex types cannot be serialized in `$text` field
                err!(map:
                    Text {
                        before: "answer",
                        content: BTreeMap::from([("_1", 2), ("_3", 4)]),
                        after: "answer",
                    }
                    => Unsupported("cannot serialize map as text content value"));
                err!(struct_:
                    Text {
                        before: "answer",
                        content: Struct { key: "answer", val: (42, 42) },
                        after: "answer",
                    }
                    => Unsupported("cannot serialize struct `Struct` as text content value"));
                err!(enum_struct:
                    Text {
                        before: "answer",
                        content: Enum::Struct { key: "answer", val: (42, 42) },
                        after: "answer",
                    }
                    => Unsupported("cannot serialize enum struct variant `Enum::Struct` as text content value"));
            }
        }

        /// Special field name `$value` should be serialized using name, provided
        /// by the type of value instead of a key. Sequences serialized as a list
        /// of tags with that name (each element can have their own name)
        mod value_field {
            use super::*;

            /// `$value` key in a map
            mod map {
                use super::*;
                use pretty_assertions::assert_eq;

                macro_rules! value {
                    ($name:ident: $data:expr) => {
                        serialize_as!($name:
                            // Serialization started from ElementSerializer::serialize_map
                            BTreeMap::from([("$value", $data)])
                            => "<root/>");
                    };
                    ($name:ident: $data:expr => $expected:literal) => {
                        serialize_as!($name:
                            // Serialization started from ElementSerializer::serialize_map
                            BTreeMap::from([("$value", $data)])
                            => concat!("<root>", $expected,"</root>"));
                    };
                }

                value!(false_: false => "false");
                value!(true_:  true  => "true");

                value!(i8_:    -42i8                => "-42");
                value!(i16_:   -4200i16             => "-4200");
                value!(i32_:   -42000000i32         => "-42000000");
                value!(i64_:   -42000000000000i64   => "-42000000000000");
                value!(isize_: -42000000000000isize => "-42000000000000");

                value!(u8_:    42u8                => "42");
                value!(u16_:   4200u16             => "4200");
                value!(u32_:   42000000u32         => "42000000");
                value!(u64_:   42000000000000u64   => "42000000000000");
                value!(usize_: 42000000000000usize => "42000000000000");

                serde_if_integer128! {
                    value!(i128_: -420000000000000000000000000000i128 => "-420000000000000000000000000000");
                    value!(u128_:  420000000000000000000000000000u128 => "420000000000000000000000000000");
                }

                value!(f32_: 4.2f32 => "4.2");
                value!(f64_: 4.2f64 => "4.2");

                value!(char_non_escaped: 'h' => "h");
                value!(char_lt:   '<' => "&lt;");
                value!(char_gt:   '>' => "&gt;");
                value!(char_amp:  '&' => "&amp;");
                value!(char_apos: '\'' => "&apos;");
                value!(char_quot: '"' => "&quot;");
                value!(char_space: ' ' => " ");

                value!(str_non_escaped: "non-escaped string" => "non-escaped string");
                value!(str_escaped: "<\"escaped & string'>" => "&lt;&quot;escaped &amp; string&apos;&gt;");

                err!(bytes:
                    BTreeMap::from([("$value", Bytes(b"<\"escaped & bytes'>"))])
                    => Unsupported("`serialize_bytes` not supported yet"));

                value!(option_none: Option::<&str>::None);
                value!(option_some: Some("non-escaped string") => "non-escaped string");
                value!(option_some_empty_str: Some(""));

                value!(unit: ());
                value!(unit_struct: Unit);
                value!(unit_struct_escaped: UnitEscaped);

                value!(enum_unit: Enum::Unit => "\n  <Unit/>\n");
                err!(enum_unit_escaped:
                    BTreeMap::from([("$value", Enum::UnitEscaped)])
                    => Unsupported("character `<` is not allowed at the start of an XML name `<\"&'>`"));

                value!(newtype: Newtype(42) => "42");
                value!(enum_newtype: Enum::Newtype(42) => "\n  <Newtype>42</Newtype>\n");

                err!(seq:
                    BTreeMap::from([("$value", vec![1, 2, 3])])
                    => Unsupported("consequent primitives would be serialized without delimiter and cannot be deserialized back"));
                value!(seq_empty: Vec::<usize>::new());
                err!(tuple:
                    BTreeMap::from([("$value", ("<\"&'>", "with\t\n\r spaces", 3usize))])
                    => Unsupported("consequent primitives would be serialized without delimiter and cannot be deserialized back"));
                err!(tuple_struct:
                    BTreeMap::from([("$value", Tuple("first", 42))])
                    => Unsupported("consequent primitives would be serialized without delimiter and cannot be deserialized back"));
                value!(enum_tuple: Enum::Tuple("first", 42)
                    => "\n  \
                        <Tuple>first</Tuple>\n  \
                        <Tuple>42</Tuple>\n");

                // We cannot wrap map or struct in any container and should not
                // flatten it, so it is impossible to serialize maps and structs
                err!(map:
                    BTreeMap::from([("$value", BTreeMap::from([("_1", 2), ("_3", 4)]))])
                    => Unsupported("serialization of map types is not supported in `$value` field"));
                err!(struct_:
                    BTreeMap::from([("$value", Struct { key: "answer", val: (42, 42) })])
                    => Unsupported("serialization of struct `Struct` is not supported in `$value` field"));
                value!(enum_struct:
                    Enum::Struct { key: "answer", val: (42, 42) }
                    => "\n  \
                        <Struct>\n    \
                            <key>answer</key>\n    \
                            <val>42</val>\n    \
                            <val>42</val>\n  \
                        </Struct>\n");
            }

            /// `$value` field inside a struct
            mod struct_ {
                use super::*;
                use pretty_assertions::assert_eq;

                macro_rules! value {
                    ($name:ident: $data:expr => $expected:literal) => {
                        serialize_as!($name:
                            // Serialization started from ElementSerializer::serialize_struct
                            Value {
                                before: "answer",
                                content: $data,
                                after: "answer",
                            }
                            => concat!(
                                "<root>\n  <before>answer</before>",
                                $expected,
                                "<after>answer</after>\n</root>",
                            ));
                    };
                }

                value!(false_: false => "false");
                value!(true_:  true  => "true");

                value!(i8_:    -42i8                => "-42");
                value!(i16_:   -4200i16             => "-4200");
                value!(i32_:   -42000000i32         => "-42000000");
                value!(i64_:   -42000000000000i64   => "-42000000000000");
                value!(isize_: -42000000000000isize => "-42000000000000");

                value!(u8_:    42u8                => "42");
                value!(u16_:   4200u16             => "4200");
                value!(u32_:   42000000u32         => "42000000");
                value!(u64_:   42000000000000u64   => "42000000000000");
                value!(usize_: 42000000000000usize => "42000000000000");

                serde_if_integer128! {
                    value!(i128_: -420000000000000000000000000000i128 => "-420000000000000000000000000000");
                    value!(u128_:  420000000000000000000000000000u128 => "420000000000000000000000000000");
                }

                value!(f32_: 4.2f32 => "4.2");
                value!(f64_: 4.2f64 => "4.2");

                value!(char_non_escaped: 'h' => "h");
                value!(char_lt:   '<' => "&lt;");
                value!(char_gt:   '>' => "&gt;");
                value!(char_amp:  '&' => "&amp;");
                value!(char_apos: '\'' => "&apos;");
                value!(char_quot: '"' => "&quot;");
                value!(char_space: ' ' => " ");

                value!(str_non_escaped: "non-escaped string" => "non-escaped string");
                value!(str_escaped: "<\"escaped & string'>" => "&lt;&quot;escaped &amp; string&apos;&gt;");

                err!(bytes:
                    Value {
                        before: "answer",
                        content: Bytes(b"<\"escaped & bytes'>"),
                        after: "answer",
                    }
                    => Unsupported("`serialize_bytes` not supported yet"));

                value!(option_none: Option::<&str>::None => "");
                value!(option_some: Some("non-escaped string") => "non-escaped string");
                value!(option_some_empty_str: Some("") => "");

                value!(unit: () => "\n  ");
                value!(unit_struct: Unit => "\n  ");
                value!(unit_struct_escaped: UnitEscaped => "\n  ");

                value!(enum_unit: Enum::Unit => "\n  <Unit/>\n  ");
                err!(enum_unit_escaped:
                    Value {
                        before: "answer",
                        content: Enum::UnitEscaped,
                        after: "answer",
                    }
                    => Unsupported("character `<` is not allowed at the start of an XML name `<\"&'>`"));

                value!(newtype: Newtype(42) => "42");
                value!(enum_newtype: Enum::Newtype(42) => "\n  <Newtype>42</Newtype>\n  ");

                err!(seq:
                    Value {
                        before: "answer",
                        content: vec![1, 2, 3],
                        after: "answer",
                    }
                    => Unsupported("consequent primitives would be serialized without delimiter and cannot be deserialized back"));
                value!(seq_empty: Vec::<usize>::new() => "");
                err!(tuple:
                    Value {
                        before: "answer",
                        content: ("<\"&'>", "with\t\n\r spaces", 3usize),
                        after: "answer",
                    }
                    => Unsupported("consequent primitives would be serialized without delimiter and cannot be deserialized back"));
                err!(tuple_struct:
                    Value {
                        before: "answer",
                        content: Tuple("first", 42),
                        after: "answer",
                    }
                    => Unsupported("consequent primitives would be serialized without delimiter and cannot be deserialized back"));
                value!(enum_tuple: Enum::Tuple("first", 42)
                    => "\n  \
                        <Tuple>first</Tuple>\n  \
                        <Tuple>42</Tuple>\n  ");

                // We cannot wrap map or struct in any container and should not
                // flatten it, so it is impossible to serialize maps and structs
                err!(map:
                    Value {
                        before: "answer",
                        content: BTreeMap::from([("_1", 2), ("_3", 4)]),
                        after: "answer",
                    }
                    => Unsupported("serialization of map types is not supported in `$value` field"));
                err!(struct_:
                    Value {
                        before: "answer",
                        content: Struct { key: "answer", val: (42, 42) },
                        after: "answer",
                    }
                    => Unsupported("serialization of struct `Struct` is not supported in `$value` field"));
                value!(enum_struct:
                    Enum::Struct { key: "answer", val: (42, 42) }
                    => "\n  \
                        <Struct>\n    \
                            <key>answer</key>\n    \
                            <val>42</val>\n    \
                            <val>42</val>\n  \
                        </Struct>\n  ");
            }
        }

        mod attributes {
            use super::*;
            use pretty_assertions::assert_eq;

            serialize_as!(map_attr: BTreeMap::from([("@key1", 1), ("@key2", 2)])
                => r#"<root key1="1" key2="2"/>"#);
            serialize_as!(map_mixed: BTreeMap::from([("@key1", 1), ("key2", 2)])
                => "<root key1=\"1\">\n  \
                        <key2>2</key2>\n\
                    </root>");

            serialize_as!(struct_: Attributes { key: "answer", val: (42, 42) }
                => r#"<root key="answer" val="42 42"/>"#);
            serialize_as!(struct_before: AttributesBefore { key: "answer", val: 42 }
                => "<root key=\"answer\">\n  \
                        <val>42</val>\n\
                    </root>");
            serialize_as!(struct_after: AttributesAfter { key: "answer", val: 42 }
                => "<root val=\"42\">\n  \
                        <key>answer</key>\n\
                    </root>");

            err!(enum_: Enum::Attributes { key: "answer", val: (42, 42) }
                => Unsupported("cannot serialize enum struct variant `Enum::Attributes`"));

            /// Test for https://github.com/tafia/quick-xml/issues/252
            mod optional {
                use super::*;
                use pretty_assertions::assert_eq;

                serialize_as!(none:
                    OptionalAttributes { a: None, b: None }
                    => r#"<root a=""/>"#);
                serialize_as!(some_empty_str:
                    OptionalAttributes {
                        a: Some(""),
                        b: Some("")
                    }
                    => r#"<root a="" b=""/>"#);
                serialize_as!(some_non_empty:
                    OptionalAttributes {
                        a: Some("a"),
                        b: Some("b")
                    }
                    => r#"<root a="a" b="b"/>"#);
            }
        }

        /// Test for https://github.com/tafia/quick-xml/issues/252
        mod optional {
            use super::*;
            use pretty_assertions::assert_eq;

            serialize_as!(none:
                OptionalElements { a: None, b: None }
                => "<root>\n  \
                        <a/>\n\
                    </root>");
            serialize_as!(some_empty_str:
                OptionalElements {
                    a: Some(""),
                    b: Some("")
                }
                => "<root>\n  \
                        <a/>\n  \
                        <b/>\n\
                    </root>");
            serialize_as!(some_non_empty:
                OptionalElements {
                    a: Some("a"),
                    b: Some("b")
                }
                => "<root>\n  \
                        <a>a</a>\n  \
                        <b>b</b>\n\
                    </root>");
        }
    }

    mod expand_empty_elements {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Checks that given `$data` successfully serialized as `$expected`
        macro_rules! serialize_as {
            ($name:ident: $data:expr => $expected:expr) => {
                #[test]
                fn $name() {
                    let mut buffer = String::new();
                    let ser = ElementSerializer {
                        ser: ContentSerializer {
                            writer: &mut buffer,
                            level: QuoteLevel::Full,
                            indent: Indent::None,
                            write_indent: false,
                            allow_primitive: true,
                            expand_empty_elements: true,
                        },
                        key: XmlName("root"),
                    };

                    let result = $data.serialize(ser).unwrap();
                    assert_eq!(buffer, $expected);
                    assert_eq!(result, WriteResult::Element);
                }
            };
        }

        serialize_as!(option_some_empty: Some("") => "<root></root>");
        serialize_as!(option_some_empty_str: Some("") => "<root></root>");

        serialize_as!(unit: () => "<root></root>");
        serialize_as!(unit_struct: Unit => "<root></root>");
        serialize_as!(unit_struct_escaped: UnitEscaped => "<root></root>");

        serialize_as!(enum_unit: Enum::Unit => "<root>Unit</root>");
        serialize_as!(enum_unit_escaped: Enum::UnitEscaped => "<root>&lt;&quot;&amp;&apos;&gt;</root>");
    }
}
