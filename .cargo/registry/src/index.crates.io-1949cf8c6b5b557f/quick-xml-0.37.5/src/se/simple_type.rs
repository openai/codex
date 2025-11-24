//! Contains Serde `Serializer` for XML [simple types] [as defined] in the XML Schema.
//!
//! [simple types]: https://www.w3schools.com/xml/el_simpletype.asp
//! [as defined]: https://www.w3.org/TR/xmlschema11-1/#Simple_Type_Definition

use crate::escape::_escape;
use crate::se::{QuoteLevel, SeError};
use serde::ser::{
    Impossible, Serialize, SerializeSeq, SerializeTuple, SerializeTupleStruct,
    SerializeTupleVariant, Serializer,
};
use serde::serde_if_integer128;
use std::borrow::Cow;
use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteTarget {
    /// Escape data for a text content. No additional escape symbols
    Text,
    /// Escape data for a double-quoted attribute. `"` always escaped
    DoubleQAttr,
    /// Escape data for a single-quoted attribute. `'` always escaped
    SingleQAttr,
}

/// Escapes atomic value that could be part of a `xs:list`. All whitespace characters
/// additionally escaped
fn escape_item(value: &str, target: QuoteTarget, level: QuoteLevel) -> Cow<str> {
    use QuoteLevel::*;
    use QuoteTarget::*;

    match (target, level) {
        (_, Full) => _escape(value, |ch| match ch {
            // Spaces used as delimiters of list items, cannot be used in the item
            b' ' | b'\r' | b'\n' | b'\t' => true,
            // Required characters to escape
            b'&' | b'<' | b'>' | b'\'' | b'\"' => true,
            _ => false,
        }),
        //----------------------------------------------------------------------
        (Text, Partial) => _escape(value, |ch| match ch {
            // Spaces used as delimiters of list items, cannot be used in the item
            b' ' | b'\r' | b'\n' | b'\t' => true,
            // Required characters to escape
            b'&' | b'<' | b'>' => true,
            _ => false,
        }),
        (Text, Minimal) => _escape(value, |ch| match ch {
            // Spaces used as delimiters of list items, cannot be used in the item
            b' ' | b'\r' | b'\n' | b'\t' => true,
            // Required characters to escape
            b'&' | b'<' => true,
            _ => false,
        }),
        //----------------------------------------------------------------------
        (DoubleQAttr, Partial) => _escape(value, |ch| match ch {
            // Spaces used as delimiters of list items, cannot be used in the item
            b' ' | b'\r' | b'\n' | b'\t' => true,
            // Required characters to escape
            b'&' | b'<' | b'>' => true,
            // Double quoted attribute should escape quote
            b'"' => true,
            _ => false,
        }),
        (DoubleQAttr, Minimal) => _escape(value, |ch| match ch {
            // Spaces used as delimiters of list items, cannot be used in the item
            b' ' | b'\r' | b'\n' | b'\t' => true,
            // Required characters to escape
            b'&' | b'<' => true,
            // Double quoted attribute should escape quote
            b'"' => true,
            _ => false,
        }),
        //----------------------------------------------------------------------
        (SingleQAttr, Partial) => _escape(value, |ch| match ch {
            // Spaces used as delimiters of list items
            b' ' | b'\r' | b'\n' | b'\t' => true,
            // Required characters to escape
            b'&' | b'<' | b'>' => true,
            // Single quoted attribute should escape quote
            b'\'' => true,
            _ => false,
        }),
        (SingleQAttr, Minimal) => _escape(value, |ch| match ch {
            // Spaces used as delimiters of list items
            b' ' | b'\r' | b'\n' | b'\t' => true,
            // Required characters to escape
            b'&' | b'<' => true,
            // Single quoted attribute should escape quote
            b'\'' => true,
            _ => false,
        }),
    }
}

/// Escapes XSD simple type value
fn escape_list(value: &str, target: QuoteTarget, level: QuoteLevel) -> Cow<str> {
    use QuoteLevel::*;
    use QuoteTarget::*;

    match (target, level) {
        (_, Full) => _escape(value, |ch| match ch {
            // Required characters to escape
            b'&' | b'<' | b'>' | b'\'' | b'\"' => true,
            _ => false,
        }),
        //----------------------------------------------------------------------
        (Text, Partial) => _escape(value, |ch| match ch {
            // Required characters to escape
            b'&' | b'<' | b'>' => true,
            _ => false,
        }),
        (Text, Minimal) => _escape(value, |ch| match ch {
            // Required characters to escape
            b'&' | b'<' => true,
            _ => false,
        }),
        //----------------------------------------------------------------------
        (DoubleQAttr, Partial) => _escape(value, |ch| match ch {
            // Required characters to escape
            b'&' | b'<' | b'>' => true,
            // Double quoted attribute should escape quote
            b'"' => true,
            _ => false,
        }),
        (DoubleQAttr, Minimal) => _escape(value, |ch| match ch {
            // Required characters to escape
            b'&' | b'<' => true,
            // Double quoted attribute should escape quote
            b'"' => true,
            _ => false,
        }),
        //----------------------------------------------------------------------
        (SingleQAttr, Partial) => _escape(value, |ch| match ch {
            // Required characters to escape
            b'&' | b'<' | b'>' => true,
            // Single quoted attribute should escape quote
            b'\'' => true,
            _ => false,
        }),
        (SingleQAttr, Minimal) => _escape(value, |ch| match ch {
            // Required characters to escape
            b'&' | b'<' => true,
            // Single quoted attribute should escape quote
            b'\'' => true,
            _ => false,
        }),
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

macro_rules! write_atomic {
    ($method:ident ( $ty:ty )) => {
        fn $method(mut self, value: $ty) -> Result<Self::Ok, Self::Error> {
            self.write_str(&value.to_string())?;
            Ok(true)
        }
    };
}

/// A serializer that handles ordinary [simple type definition][item] with
/// `{variety} = atomic`, or an ordinary [simple type] definition with
/// `{variety} = union` whose basic members are all atomic.
///
/// This serializer can serialize only primitive types:
/// - numbers
/// - booleans
/// - strings
/// - units
/// - options
/// - unit variants of enums
///
/// Identifiers represented as strings and serialized accordingly.
///
/// Serialization of all other types returns [`Unsupported`][SeError::Unsupported] error.
///
/// This serializer returns `true` if something was written and `false` otherwise.
///
/// [item]: https://www.w3.org/TR/xmlschema11-1/#std-item_type_definition
/// [simple type]: https://www.w3.org/TR/xmlschema11-1/#Simple_Type_Definition
pub struct AtomicSerializer<W: Write> {
    pub writer: W,
    pub target: QuoteTarget,
    /// Defines which XML characters need to be escaped
    pub level: QuoteLevel,
    /// When `true` an `xs:list` delimiter (a space) should be written
    pub(crate) write_delimiter: bool,
}

impl<W: Write> AtomicSerializer<W> {
    fn write_str(&mut self, value: &str) -> Result<(), SeError> {
        if self.write_delimiter {
            // TODO: Customization point -- possible non-XML compatible extension to specify delimiter char
            self.writer.write_char(' ')?;
        }
        Ok(self.writer.write_str(value)?)
    }
}

impl<W: Write> Serializer for AtomicSerializer<W> {
    type Ok = bool;
    type Error = SeError;

    type SerializeSeq = Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(mut self, value: bool) -> Result<Self::Ok, Self::Error> {
        self.write_str(if value { "true" } else { "false" })?;
        Ok(true)
    }

    write_atomic!(serialize_i8(i8));
    write_atomic!(serialize_i16(i16));
    write_atomic!(serialize_i32(i32));
    write_atomic!(serialize_i64(i64));

    write_atomic!(serialize_u8(u8));
    write_atomic!(serialize_u16(u16));
    write_atomic!(serialize_u32(u32));
    write_atomic!(serialize_u64(u64));

    serde_if_integer128! {
        write_atomic!(serialize_i128(i128));
        write_atomic!(serialize_u128(u128));
    }

    write_atomic!(serialize_f32(f32));
    write_atomic!(serialize_f64(f64));

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&value.to_string())
    }

    fn serialize_str(mut self, value: &str) -> Result<Self::Ok, Self::Error> {
        if !value.is_empty() {
            self.write_str(&escape_item(value, self.target, self.level))?;
        }
        Ok(!value.is_empty())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
        //TODO: Customization point - allow user to decide how to encode bytes
        Err(SeError::Unsupported(
            "`serialize_bytes` not supported yet".into(),
        ))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(false)
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    /// We cannot store anything, so the absence of a unit and presence of it
    /// does not differ, so serialization of unit returns `Err(Unsupported)`
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(SeError::Unsupported(
            "cannot serialize unit type `()` as an `xs:list` item".into(),
        ))
    }

    /// We cannot store anything, so the absence of a unit and presence of it
    /// does not differ, so serialization of unit returns `Err(Unsupported)`
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(SeError::Unsupported(
            format!(
                "cannot serialize unit struct `{}` as an `xs:list` item",
                name
            )
            .into(),
        ))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    /// We cannot store both a variant discriminant and a variant value,
    /// so serialization of enum newtype variant returns `Err(Unsupported)`
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, SeError> {
        Err(SeError::Unsupported(
            format!(
                "cannot serialize enum newtype variant `{}::{}` as an `xs:list` item",
                name, variant
            )
            .into(),
        ))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(SeError::Unsupported(
            "cannot serialize sequence as an `xs:list` item".into(),
        ))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(SeError::Unsupported(
            "cannot serialize tuple as an `xs:list` item".into(),
        ))
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(SeError::Unsupported(
            format!(
                "cannot serialize tuple struct `{}` as an `xs:list` item",
                name
            )
            .into(),
        ))
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(SeError::Unsupported(
            format!(
                "cannot serialize enum tuple variant `{}::{}` as an `xs:list` item",
                name, variant
            )
            .into(),
        ))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(SeError::Unsupported(
            "cannot serialize map as an `xs:list` item".into(),
        ))
    }

    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(SeError::Unsupported(
            format!("cannot serialize struct `{}` as an `xs:list` item", name).into(),
        ))
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(SeError::Unsupported(
            format!(
                "cannot serialize enum struct variant `{}::{}` as an `xs:list` item",
                name, variant
            )
            .into(),
        ))
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A serializer for a values representing XSD [simple types], which used in:
/// - attribute values (`<... ...="value" ...>`)
/// - text content (`<...>text</...>`)
/// - CDATA content (`<...><![CDATA[cdata]]></...>`)
///
/// [simple types]: https://www.w3.org/TR/xmlschema11-1/#Simple_Type_Definition
pub struct SimpleTypeSerializer<W: Write> {
    /// Writer to which this serializer writes content
    pub writer: W,
    /// Target for which element is serializing. Affects additional characters to escape.
    pub target: QuoteTarget,
    /// Defines which XML characters need to be escaped
    pub level: QuoteLevel,
}

impl<W: Write> SimpleTypeSerializer<W> {
    fn write_str(&mut self, value: &str) -> Result<(), SeError> {
        Ok(self.writer.write_str(value)?)
    }
}

impl<W: Write> Serializer for SimpleTypeSerializer<W> {
    type Ok = W;
    type Error = SeError;

    type SerializeSeq = SimpleSeq<W>;
    type SerializeTuple = SimpleSeq<W>;
    type SerializeTupleStruct = SimpleSeq<W>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    write_primitive!();

    fn serialize_str(mut self, value: &str) -> Result<Self::Ok, Self::Error> {
        if !value.is_empty() {
            self.write_str(&escape_list(value, self.target, self.level))?;
        }
        Ok(self.writer)
    }

    /// Does not write anything
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.writer)
    }

    /// Does not write anything
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(self.writer)
    }

    /// We cannot store both a variant discriminant and a variant value,
    /// so serialization of enum newtype variant returns `Err(Unsupported)`
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, SeError> {
        Err(SeError::Unsupported(
            format!("cannot serialize enum newtype variant `{}::{}` as an attribute or text content value", name, variant).into(),
        ))
    }

    #[inline]
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SimpleSeq {
            writer: self.writer,
            target: self.target,
            level: self.level,
            is_empty: true,
        })
    }

    #[inline]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(None)
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(None)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(SeError::Unsupported(
            format!("cannot serialize enum tuple variant `{}::{}` as an attribute or text content value", name, variant).into(),
        ))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(SeError::Unsupported(
            "cannot serialize map as an attribute or text content value".into(),
        ))
    }

    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(SeError::Unsupported(
            format!(
                "cannot serialize struct `{}` as an attribute or text content value",
                name
            )
            .into(),
        ))
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(SeError::Unsupported(
            format!("cannot serialize enum struct variant `{}::{}` as an attribute or text content value", name, variant).into(),
        ))
    }
}

/// Serializer for a sequence of atomic values delimited by space
pub struct SimpleSeq<W: Write> {
    writer: W,
    target: QuoteTarget,
    level: QuoteLevel,
    /// If `true`, nothing was written yet to the `writer`
    is_empty: bool,
}

impl<W: Write> SerializeSeq for SimpleSeq<W> {
    type Ok = W;
    type Error = SeError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        if value.serialize(AtomicSerializer {
            writer: &mut self.writer,
            target: self.target,
            level: self.level,
            write_delimiter: !self.is_empty,
        })? {
            self.is_empty = false;
        }
        Ok(())
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.writer)
    }
}

impl<W: Write> SerializeTuple for SimpleSeq<W> {
    type Ok = W;
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

impl<W: Write> SerializeTupleStruct for SimpleSeq<W> {
    type Ok = W;
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

impl<W: Write> SerializeTupleVariant for SimpleSeq<W> {
    type Ok = W;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::Bytes;
    use serde::Serialize;
    use std::collections::BTreeMap;

    #[derive(Debug, Serialize, PartialEq)]
    struct Unit;

    #[derive(Debug, Serialize, PartialEq)]
    struct Newtype(usize);

    #[derive(Debug, Serialize, PartialEq)]
    struct Tuple(&'static str, usize);

    #[derive(Debug, Serialize, PartialEq)]
    struct Struct {
        key: &'static str,
        val: usize,
    }

    #[derive(Debug, Serialize, PartialEq)]
    enum Enum {
        Unit,
        #[serde(rename = "<\"&'>")]
        UnitEscaped,
        Newtype(usize),
        Tuple(&'static str, usize),
        Struct {
            key: &'static str,
            val: usize,
        },
    }

    mod escape_item {
        use super::*;

        mod full {
            use super::*;
            use pretty_assertions::assert_eq;

            #[test]
            fn text() {
                assert_eq!(
                    escape_item("text<\"'&> \t\n\rtext", QuoteTarget::Text, QuoteLevel::Full),
                    "text&lt;&quot;&apos;&amp;&gt;&#32;&#9;&#10;&#13;text"
                );
            }

            #[test]
            fn double_quote_attr() {
                assert_eq!(
                    escape_item(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::DoubleQAttr,
                        QuoteLevel::Full
                    ),
                    "text&lt;&quot;&apos;&amp;&gt;&#32;&#9;&#10;&#13;text"
                );
            }

            #[test]
            fn single_quote_attr() {
                assert_eq!(
                    escape_item(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::SingleQAttr,
                        QuoteLevel::Full
                    ),
                    "text&lt;&quot;&apos;&amp;&gt;&#32;&#9;&#10;&#13;text"
                );
            }
        }

        mod partial {
            use super::*;
            use pretty_assertions::assert_eq;

            #[test]
            fn text() {
                assert_eq!(
                    escape_item(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::Text,
                        QuoteLevel::Partial
                    ),
                    "text&lt;\"'&amp;&gt;&#32;&#9;&#10;&#13;text"
                );
            }

            #[test]
            fn double_quote_attr() {
                assert_eq!(
                    escape_item(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::DoubleQAttr,
                        QuoteLevel::Partial
                    ),
                    "text&lt;&quot;'&amp;&gt;&#32;&#9;&#10;&#13;text"
                );
            }

            #[test]
            fn single_quote_attr() {
                assert_eq!(
                    escape_item(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::SingleQAttr,
                        QuoteLevel::Partial
                    ),
                    "text&lt;\"&apos;&amp;&gt;&#32;&#9;&#10;&#13;text"
                );
            }
        }

        mod minimal {
            use super::*;
            use pretty_assertions::assert_eq;

            #[test]
            fn text() {
                assert_eq!(
                    escape_item(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::Text,
                        QuoteLevel::Minimal
                    ),
                    "text&lt;\"'&amp;>&#32;&#9;&#10;&#13;text"
                );
            }

            #[test]
            fn double_quote_attr() {
                assert_eq!(
                    escape_item(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::DoubleQAttr,
                        QuoteLevel::Minimal
                    ),
                    "text&lt;&quot;'&amp;>&#32;&#9;&#10;&#13;text"
                );
            }

            #[test]
            fn single_quote_attr() {
                assert_eq!(
                    escape_item(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::SingleQAttr,
                        QuoteLevel::Minimal
                    ),
                    "text&lt;\"&apos;&amp;>&#32;&#9;&#10;&#13;text"
                );
            }
        }
    }

    mod escape_list {
        use super::*;

        mod full {
            use super::*;
            use pretty_assertions::assert_eq;

            #[test]
            fn text() {
                assert_eq!(
                    escape_list("text<\"'&> \t\n\rtext", QuoteTarget::Text, QuoteLevel::Full),
                    "text&lt;&quot;&apos;&amp;&gt; \t\n\rtext"
                );
            }

            #[test]
            fn double_quote_attr() {
                assert_eq!(
                    escape_list(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::DoubleQAttr,
                        QuoteLevel::Full
                    ),
                    "text&lt;&quot;&apos;&amp;&gt; \t\n\rtext"
                );
            }

            #[test]
            fn single_quote_attr() {
                assert_eq!(
                    escape_list(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::SingleQAttr,
                        QuoteLevel::Full
                    ),
                    "text&lt;&quot;&apos;&amp;&gt; \t\n\rtext"
                );
            }
        }

        mod partial {
            use super::*;
            use pretty_assertions::assert_eq;

            #[test]
            fn text() {
                assert_eq!(
                    escape_list(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::Text,
                        QuoteLevel::Partial
                    ),
                    "text&lt;\"'&amp;&gt; \t\n\rtext"
                );
            }

            #[test]
            fn double_quote_attr() {
                assert_eq!(
                    escape_list(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::DoubleQAttr,
                        QuoteLevel::Partial
                    ),
                    "text&lt;&quot;'&amp;&gt; \t\n\rtext"
                );
            }

            #[test]
            fn single_quote_attr() {
                assert_eq!(
                    escape_list(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::SingleQAttr,
                        QuoteLevel::Partial
                    ),
                    "text&lt;\"&apos;&amp;&gt; \t\n\rtext"
                );
            }
        }

        mod minimal {
            use super::*;
            use pretty_assertions::assert_eq;

            #[test]
            fn text() {
                assert_eq!(
                    escape_list(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::Text,
                        QuoteLevel::Minimal
                    ),
                    "text&lt;\"'&amp;> \t\n\rtext"
                );
            }

            #[test]
            fn double_quote_attr() {
                assert_eq!(
                    escape_list(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::DoubleQAttr,
                        QuoteLevel::Minimal
                    ),
                    "text&lt;&quot;'&amp;> \t\n\rtext"
                );
            }

            #[test]
            fn single_quote_attr() {
                assert_eq!(
                    escape_list(
                        "text<\"'&> \t\n\rtext",
                        QuoteTarget::SingleQAttr,
                        QuoteLevel::Minimal
                    ),
                    "text&lt;\"&apos;&amp;> \t\n\rtext"
                );
            }
        }
    }

    /// Tests for serialize atomic and union values, as defined in XSD specification
    mod atomic {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Checks that given `$data` successfully serialized as `$expected`
        macro_rules! serialize_as {
            ($name:ident: $data:expr => $expected:literal) => {
                #[test]
                fn $name() {
                    let mut buffer = String::new();
                    let ser = AtomicSerializer {
                        writer: &mut buffer,
                        target: QuoteTarget::Text,
                        level: QuoteLevel::Full,
                        write_delimiter: false,
                    };

                    let has_written = $data.serialize(ser).unwrap();
                    assert_eq!(buffer, $expected);
                    assert_eq!(has_written, !buffer.is_empty());
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
                    let ser = AtomicSerializer {
                        writer: &mut buffer,
                        target: QuoteTarget::Text,
                        level: QuoteLevel::Full,
                        write_delimiter: false,
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
                    assert_eq!(buffer, "");
                }
            };
        }

        serialize_as!(false_: false => "false");
        serialize_as!(true_:  true  => "true");

        serialize_as!(i8_:    -42i8                => "-42");
        serialize_as!(i16_:   -4200i16             => "-4200");
        serialize_as!(i32_:   -42000000i32         => "-42000000");
        serialize_as!(i64_:   -42000000000000i64   => "-42000000000000");
        serialize_as!(isize_: -42000000000000isize => "-42000000000000");

        serialize_as!(u8_:    42u8                => "42");
        serialize_as!(u16_:   4200u16             => "4200");
        serialize_as!(u32_:   42000000u32         => "42000000");
        serialize_as!(u64_:   42000000000000u64   => "42000000000000");
        serialize_as!(usize_: 42000000000000usize => "42000000000000");

        serde_if_integer128! {
            serialize_as!(i128_: -420000000000000000000000000000i128 => "-420000000000000000000000000000");
            serialize_as!(u128_:  420000000000000000000000000000u128 => "420000000000000000000000000000");
        }

        serialize_as!(f32_: 4.2f32 => "4.2");
        serialize_as!(f64_: 4.2f64 => "4.2");

        serialize_as!(char_non_escaped: 'h' => "h");
        serialize_as!(char_lt:   '<' => "&lt;");
        serialize_as!(char_gt:   '>' => "&gt;");
        serialize_as!(char_amp:  '&' => "&amp;");
        serialize_as!(char_apos: '\'' => "&apos;");
        serialize_as!(char_quot: '"' => "&quot;");

        serialize_as!(str_non_escaped: "non-escaped-string" => "non-escaped-string");
        serialize_as!(str_escaped: "<\"escaped & string'>" => "&lt;&quot;escaped&#32;&amp;&#32;string&apos;&gt;");

        err!(bytes: Bytes(b"<\"escaped & bytes'>")
            => Unsupported("`serialize_bytes` not supported yet"));

        serialize_as!(option_none: Option::<&str>::None => "");
        serialize_as!(option_some: Some("non-escaped-string") => "non-escaped-string");

        err!(unit: ()
            => Unsupported("cannot serialize unit type `()` as an `xs:list` item"));
        err!(unit_struct: Unit
            => Unsupported("cannot serialize unit struct `Unit` as an `xs:list` item"));

        serialize_as!(enum_unit: Enum::Unit => "Unit");
        serialize_as!(enum_unit_escaped: Enum::UnitEscaped => "&lt;&quot;&amp;&apos;&gt;");

        serialize_as!(newtype: Newtype(42) => "42");
        err!(enum_newtype: Enum::Newtype(42)
            => Unsupported("cannot serialize enum newtype variant `Enum::Newtype` as an `xs:list` item"));

        err!(seq: vec![1, 2, 3]
            => Unsupported("cannot serialize sequence as an `xs:list` item"));
        err!(tuple: ("<\"&'>", "with\t\n\r spaces", 3usize)
            => Unsupported("cannot serialize tuple as an `xs:list` item"));
        err!(tuple_struct: Tuple("first", 42)
            => Unsupported("cannot serialize tuple struct `Tuple` as an `xs:list` item"));
        err!(enum_tuple: Enum::Tuple("first", 42)
            => Unsupported("cannot serialize enum tuple variant `Enum::Tuple` as an `xs:list` item"));

        err!(map: BTreeMap::from([(1, 2), (3, 4)])
            => Unsupported("cannot serialize map as an `xs:list` item"));
        err!(struct_: Struct { key: "answer", val: 42 }
            => Unsupported("cannot serialize struct `Struct` as an `xs:list` item"));
        err!(enum_struct: Enum::Struct { key: "answer", val: 42 }
            => Unsupported("cannot serialize enum struct variant `Enum::Struct` as an `xs:list` item"));
    }

    mod simple_type {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Checks that given `$data` successfully serialized as `$expected`
        macro_rules! serialize_as {
            ($name:ident: $data:expr => $expected:literal) => {
                #[test]
                fn $name() {
                    let ser = SimpleTypeSerializer {
                        writer: String::new(),
                        target: QuoteTarget::Text,
                        level: QuoteLevel::Full,
                    };

                    let buffer = $data.serialize(ser).unwrap();
                    assert_eq!(buffer, $expected);
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
                    let ser = SimpleTypeSerializer {
                        writer: &mut buffer,
                        target: QuoteTarget::Text,
                        level: QuoteLevel::Full,
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
                    assert_eq!(buffer, "");
                }
            };
        }

        serialize_as!(false_: false => "false");
        serialize_as!(true_:  true  => "true");

        serialize_as!(i8_:    -42i8                => "-42");
        serialize_as!(i16_:   -4200i16             => "-4200");
        serialize_as!(i32_:   -42000000i32         => "-42000000");
        serialize_as!(i64_:   -42000000000000i64   => "-42000000000000");
        serialize_as!(isize_: -42000000000000isize => "-42000000000000");

        serialize_as!(u8_:    42u8                => "42");
        serialize_as!(u16_:   4200u16             => "4200");
        serialize_as!(u32_:   42000000u32         => "42000000");
        serialize_as!(u64_:   42000000000000u64   => "42000000000000");
        serialize_as!(usize_: 42000000000000usize => "42000000000000");

        serde_if_integer128! {
            serialize_as!(i128_: -420000000000000000000000000000i128 => "-420000000000000000000000000000");
            serialize_as!(u128_:  420000000000000000000000000000u128 => "420000000000000000000000000000");
        }

        serialize_as!(f32_: 4.2f32 => "4.2");
        serialize_as!(f64_: 4.2f64 => "4.2");

        serialize_as!(char_non_escaped: 'h' => "h");
        serialize_as!(char_lt:   '<' => "&lt;");
        serialize_as!(char_gt:   '>' => "&gt;");
        serialize_as!(char_amp:  '&' => "&amp;");
        serialize_as!(char_apos: '\'' => "&apos;");
        serialize_as!(char_quot: '"' => "&quot;");

        serialize_as!(str_non_escaped: "non-escaped string" => "non-escaped string");
        serialize_as!(str_escaped: "<\"escaped & string'>" => "&lt;&quot;escaped &amp; string&apos;&gt;");

        err!(bytes: Bytes(b"<\"escaped & bytes'>")
            => Unsupported("`serialize_bytes` not supported yet"));

        serialize_as!(option_none: Option::<&str>::None => "");
        serialize_as!(option_some: Some("non-escaped string") => "non-escaped string");

        serialize_as!(unit: () => "");
        serialize_as!(unit_struct: Unit => "");

        serialize_as!(enum_unit: Enum::Unit => "Unit");
        serialize_as!(enum_unit_escaped: Enum::UnitEscaped => "&lt;&quot;&amp;&apos;&gt;");

        serialize_as!(newtype: Newtype(42) => "42");
        err!(enum_newtype: Enum::Newtype(42)
            => Unsupported("cannot serialize enum newtype variant `Enum::Newtype` as an attribute or text content value"));

        serialize_as!(seq: vec![1, 2, 3] => "1 2 3");
        serialize_as!(seq_empty: Vec::<usize>::new() => "");
        serialize_as!(seq_with_1_empty_str: vec![""] => "");
        serialize_as!(seq_with_2_empty_strs: vec!["", ""] => "");
        serialize_as!(tuple: ("<\"&'>", "with\t\n\r spaces", 3usize)
            => "&lt;&quot;&amp;&apos;&gt; with&#9;&#10;&#13;&#32;spaces 3");
        serialize_as!(tuple_struct: Tuple("first", 42) => "first 42");
        err!(enum_tuple: Enum::Tuple("first", 42)
            => Unsupported("cannot serialize enum tuple variant `Enum::Tuple` as an attribute or text content value"));

        err!(map: BTreeMap::from([(1, 2), (3, 4)])
            => Unsupported("cannot serialize map as an attribute or text content value"));
        err!(struct_: Struct { key: "answer", val: 42 }
            => Unsupported("cannot serialize struct `Struct` as an attribute or text content value"));
        err!(enum_struct: Enum::Struct { key: "answer", val: 42 }
            => Unsupported("cannot serialize enum struct variant `Enum::Struct` as an attribute or text content value"));
    }

    mod simple_seq {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn empty_seq() {
            let mut buffer = String::new();
            let ser = SimpleSeq {
                writer: &mut buffer,
                target: QuoteTarget::Text,
                level: QuoteLevel::Full,
                is_empty: true,
            };

            SerializeSeq::end(ser).unwrap();
            assert_eq!(buffer, "");
        }

        #[test]
        fn all_items_empty() {
            let mut buffer = String::new();
            let mut ser = SimpleSeq {
                writer: &mut buffer,
                target: QuoteTarget::Text,
                level: QuoteLevel::Full,
                is_empty: true,
            };

            SerializeSeq::serialize_element(&mut ser, "").unwrap();
            SerializeSeq::serialize_element(&mut ser, "").unwrap();
            SerializeSeq::serialize_element(&mut ser, "").unwrap();
            SerializeSeq::end(ser).unwrap();
            assert_eq!(buffer, "");
        }

        #[test]
        fn some_items_empty1() {
            let mut buffer = String::new();
            let mut ser = SimpleSeq {
                writer: &mut buffer,
                target: QuoteTarget::Text,
                level: QuoteLevel::Full,
                is_empty: true,
            };

            SerializeSeq::serialize_element(&mut ser, "").unwrap();
            SerializeSeq::serialize_element(&mut ser, &1).unwrap();
            SerializeSeq::serialize_element(&mut ser, "").unwrap();
            SerializeSeq::end(ser).unwrap();
            assert_eq!(buffer, "1");
        }

        #[test]
        fn some_items_empty2() {
            let mut buffer = String::new();
            let mut ser = SimpleSeq {
                writer: &mut buffer,
                target: QuoteTarget::Text,
                level: QuoteLevel::Full,
                is_empty: true,
            };

            SerializeSeq::serialize_element(&mut ser, &1).unwrap();
            SerializeSeq::serialize_element(&mut ser, "").unwrap();
            SerializeSeq::serialize_element(&mut ser, &2).unwrap();
            SerializeSeq::end(ser).unwrap();
            assert_eq!(buffer, "1 2");
        }

        #[test]
        fn items() {
            let mut buffer = String::new();
            let mut ser = SimpleSeq {
                writer: &mut buffer,
                target: QuoteTarget::Text,
                level: QuoteLevel::Full,
                is_empty: true,
            };

            SerializeSeq::serialize_element(&mut ser, &1).unwrap();
            SerializeSeq::serialize_element(&mut ser, &2).unwrap();
            SerializeSeq::serialize_element(&mut ser, &3).unwrap();
            SerializeSeq::end(ser).unwrap();
            assert_eq!(buffer, "1 2 3");
        }
    }
}
