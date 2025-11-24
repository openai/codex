//! Contains Serde `Deserializer` for XML [simple types] [as defined] in the XML Schema.
//!
//! [simple types]: https://www.w3schools.com/xml/el_simpletype.asp
//! [as defined]: https://www.w3.org/TR/xmlschema11-1/#Simple_Type_Definition

use crate::de::Text;
use crate::encoding::Decoder;
use crate::errors::serialize::DeError;
use crate::escape::unescape;
use crate::utils::CowRef;
use memchr::memchr;
use serde::de::value::UnitDeserializer;
use serde::de::{
    DeserializeSeed, Deserializer, EnumAccess, IntoDeserializer, SeqAccess, VariantAccess, Visitor,
};
use serde::serde_if_integer128;
use std::borrow::Cow;
use std::ops::Range;

macro_rules! deserialize_num {
    ($method:ident => $visit:ident) => {
        #[inline]
        fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            let text: &str = self.content.as_ref();
            match text.parse() {
                Ok(number) => visitor.$visit(number),
                Err(_) => self.content.deserialize_str(visitor),
            }
        }
    };
}

macro_rules! deserialize_primitive {
    ($method:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            let de = AtomicDeserializer {
                content: self.decode()?,
                escaped: self.escaped,
            };
            de.$method(visitor)
        }
    };
}

macro_rules! unsupported {
    (
        $deserialize:ident
        $(
            ($($type:ty),*)
        )?
    ) => {
        #[inline]
        fn $deserialize<V: Visitor<'de>>(
            self,
            $($(_: $type,)*)?
            visitor: V
        ) -> Result<V::Value, Self::Error> {
            // Deserializer methods are only hints, if deserializer could not satisfy
            // request, it should return the data that it has. It is responsibility
            // of a Visitor to return an error if it does not understand the data
            self.deserialize_str(visitor)
        }
    };
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A version of [`Cow`] that can borrow from two different buffers, one of them
/// is a deserializer input, and conceptually contains only part of owned data.
///
/// # Lifetimes
/// - `'de` -- lifetime of the data that deserializer borrow from the parsed input
/// - `'a` -- lifetime of the data that owned by a deserializer
enum Content<'de, 'a> {
    /// An input borrowed from the parsed data
    Input(&'de str),
    /// An input borrowed from the buffer owned by another deserializer
    Slice(&'a str),
    /// An input taken from an external deserializer, owned by that deserializer.
    /// Only part of this data, located after offset represented by `usize`, used
    /// to deserialize data, the other is a garbage that can't be dropped because
    /// we do not want to make reallocations if they will not required.
    Owned(String, usize),
}
impl<'de, 'a> Content<'de, 'a> {
    /// Returns string representation of the content
    fn as_str(&self) -> &str {
        match self {
            Content::Input(s) => s,
            Content::Slice(s) => s,
            Content::Owned(s, offset) => s.split_at(*offset).1,
        }
    }
}

/// A deserializer that handles ordinary [simple type definition][item] with
/// `{variety} = atomic`, or an ordinary [simple type] definition with
/// `{variety} = union` whose basic members are all atomic.
///
/// This deserializer can deserialize only primitive types:
/// - numbers
/// - booleans
/// - strings
/// - units
/// - options
/// - unit variants of enums
///
/// Identifiers represented as strings and deserialized accordingly.
///
/// Deserialization of all other types will provide a string and in most cases
/// the deserialization will fail because visitor does not expect that.
///
/// The `Owned` variant of the content acts as a storage for data, allocated by
/// an external deserializer that pass it via [`ListIter`].
///
/// [item]: https://www.w3.org/TR/xmlschema11-1/#std-item_type_definition
/// [simple type]: https://www.w3.org/TR/xmlschema11-1/#Simple_Type_Definition
struct AtomicDeserializer<'de, 'a> {
    /// Content of the attribute value, text content or CDATA content
    content: CowRef<'de, 'a, str>,
    /// If `true`, `content` in an escaped form and should be unescaped before use
    escaped: bool,
}

impl<'de, 'a> Deserializer<'de> for AtomicDeserializer<'de, 'a> {
    type Error = DeError;

    /// Forwards deserialization to the [`Self::deserialize_str`]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    /// According to the <https://www.w3.org/TR/xmlschema11-2/#boolean>,
    /// valid boolean representations are only `"true"`, `"false"`, `"1"`,
    /// and `"0"`.
    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.content.deserialize_bool(visitor)
    }

    deserialize_num!(deserialize_i8  => visit_i8);
    deserialize_num!(deserialize_i16 => visit_i16);
    deserialize_num!(deserialize_i32 => visit_i32);
    deserialize_num!(deserialize_i64 => visit_i64);

    deserialize_num!(deserialize_u8  => visit_u8);
    deserialize_num!(deserialize_u16 => visit_u16);
    deserialize_num!(deserialize_u32 => visit_u32);
    deserialize_num!(deserialize_u64 => visit_u64);

    serde_if_integer128! {
        deserialize_num!(deserialize_i128 => visit_i128);
        deserialize_num!(deserialize_u128 => visit_u128);
    }

    deserialize_num!(deserialize_f32 => visit_f32);
    deserialize_num!(deserialize_f64 => visit_f64);

    /// Forwards deserialization to the [`Self::deserialize_str`]
    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    /// Supply to the visitor borrowed string, string slice, or owned string
    /// depending on the kind of input and presence of the escaped data.
    ///
    /// If string requires unescaping, then calls [`Visitor::visit_string`] with
    /// new allocated buffer with unescaped data.
    ///
    /// Otherwise calls
    /// - [`Visitor::visit_borrowed_str`] if data borrowed from the input
    /// - [`Visitor::visit_str`] if data borrowed from other deserializer
    /// - [`Visitor::visit_string`] if data owned by this deserializer
    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.escaped {
            match unescape(self.content.as_ref())? {
                Cow::Borrowed(_) => self.content.deserialize_str(visitor),
                Cow::Owned(s) => visitor.visit_string(s),
            }
        } else {
            self.content.deserialize_str(visitor)
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    /// If `content` is an empty string then calls [`Visitor::visit_none`],
    /// otherwise calls [`Visitor::visit_some`] with itself
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let text: &str = self.content.as_ref();
        if text.is_empty() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    /// Forwards deserialization to the [`Self::deserialize_unit`]
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

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(self)
    }

    /// Forwards deserialization to the [`Self::deserialize_str`]
    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    unsupported!(deserialize_bytes);
    unsupported!(deserialize_byte_buf);
    unsupported!(deserialize_seq);
    unsupported!(deserialize_tuple(usize));
    unsupported!(deserialize_tuple_struct(&'static str, usize));
    unsupported!(deserialize_map);
    unsupported!(deserialize_struct(&'static str, &'static [&'static str]));
}

impl<'de, 'a> EnumAccess<'de> for AtomicDeserializer<'de, 'a> {
    type Error = DeError;
    type Variant = UnitOnly;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), DeError>
    where
        V: DeserializeSeed<'de>,
    {
        let name = seed.deserialize(self)?;
        Ok((name, UnitOnly))
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Deserializer of variant data, that supports only unit variants.
/// Attempt to deserialize newtype will provide [`UnitDeserializer`].
/// Attempt to deserialize tuple or struct variant will result to call of
/// [`Visitor::visit_unit`].
pub struct UnitOnly;
impl<'de> VariantAccess<'de> for UnitOnly {
    type Error = DeError;

    #[inline]
    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(UnitDeserializer::<Self::Error>::new())
    }

    #[inline]
    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    #[inline]
    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Iterator over string sub-slices delimited by one or several spaces.
/// Contains decoded value of the `simpleType`.
/// Iteration ends when list contains `None`.
struct ListIter<'de, 'a> {
    /// If `Some`, contains unconsumed data of the list
    content: Option<Content<'de, 'a>>,
    /// If `true`, `content` in escaped form and should be unescaped before use
    escaped: bool,
}
impl<'de, 'a> SeqAccess<'de> for ListIter<'de, 'a> {
    type Error = DeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, DeError>
    where
        T: DeserializeSeed<'de>,
    {
        if let Some(mut content) = self.content.take() {
            const DELIMITER: u8 = b' ';

            loop {
                let string = content.as_str();
                if string.is_empty() {
                    return Ok(None);
                }
                return match memchr(DELIMITER, string.as_bytes()) {
                    // No delimiters in the `content`, deserialize it as a whole atomic
                    None => match content {
                        Content::Input(s) => seed.deserialize(AtomicDeserializer {
                            content: CowRef::Input(s),
                            escaped: self.escaped,
                        }),
                        Content::Slice(s) => seed.deserialize(AtomicDeserializer {
                            content: CowRef::Slice(s),
                            escaped: self.escaped,
                        }),
                        Content::Owned(s, 0) => seed.deserialize(AtomicDeserializer {
                            content: CowRef::Owned(s),
                            escaped: self.escaped,
                        }),
                        Content::Owned(s, offset) => seed.deserialize(AtomicDeserializer {
                            content: CowRef::Slice(s.split_at(offset).1),
                            escaped: self.escaped,
                        }),
                    },
                    // `content` started with a space, skip them all
                    Some(0) => {
                        // Skip all spaces
                        let start = string.as_bytes().iter().position(|ch| *ch != DELIMITER);
                        content = match (start, content) {
                            // We cannot find any non-space character, so string contains only spaces
                            (None, _) => return Ok(None),
                            // Borrow result from input or deserializer depending on the initial borrowing
                            (Some(start), Content::Input(s)) => Content::Input(s.split_at(start).1),
                            (Some(start), Content::Slice(s)) => Content::Slice(s.split_at(start).1),
                            // Skip additional bytes if we own data
                            (Some(start), Content::Owned(s, skip)) => {
                                Content::Owned(s, skip + start)
                            }
                        };
                        continue;
                    }
                    // `content` started from an atomic
                    Some(end) => match content {
                        // Borrow for the next iteration from input or deserializer depending on
                        // the initial borrowing
                        Content::Input(s) => {
                            let (item, rest) = s.split_at(end);
                            self.content = Some(Content::Input(rest));

                            seed.deserialize(AtomicDeserializer {
                                content: CowRef::Input(item),
                                escaped: self.escaped,
                            })
                        }
                        Content::Slice(s) => {
                            let (item, rest) = s.split_at(end);
                            self.content = Some(Content::Slice(rest));

                            seed.deserialize(AtomicDeserializer {
                                content: CowRef::Slice(item),
                                escaped: self.escaped,
                            })
                        }
                        // Skip additional bytes if we own data for next iteration, but deserialize from
                        // the borrowed data from our buffer
                        Content::Owned(s, skip) => {
                            let item = s.split_at(skip + end).0;
                            let result = seed.deserialize(AtomicDeserializer {
                                content: CowRef::Slice(item),
                                escaped: self.escaped,
                            });

                            self.content = Some(Content::Owned(s, skip + end));

                            result
                        }
                    },
                }
                .map(Some);
            }
        }
        Ok(None)
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A deserializer for an xml probably escaped and encoded value of XSD [simple types].
/// This deserializer will borrow from the input as much as possible.
///
/// `deserialize_any()` returns the whole string that deserializer contains.
///
/// Escaping the value is actually not always necessary, for instance when
/// converting to a float, we don't expect any escapable character anyway.
/// In that cases deserializer skips unescaping step.
///
/// Used for deserialize values from:
/// - attribute values (`<... ...="value" ...>`)
/// - mixed text / CDATA content (`<...>text<![CDATA[cdata]]></...>`)
///
/// This deserializer processes items as following:
/// - numbers are parsed from a text content using [`FromStr`]; in case of error
///   [`Visitor::visit_borrowed_str`], [`Visitor::visit_str`], or [`Visitor::visit_string`]
///   is called; it is responsibility of the type to return an error if it does
///   not able to process passed data;
/// - booleans converted from the text according to the XML [specification]:
///   - `"true"` and `"1"` converted to `true`;
///   - `"false"` and `"0"` converted to `false`;
///   - everything else calls [`Visitor::visit_borrowed_str`], [`Visitor::visit_str`],
///     or [`Visitor::visit_string`]; it is responsibility of the type to return
///     an error if it does not able to process passed data;
/// - strings returned as is;
/// - characters also returned as strings. If string contain more than one character
///   or empty, it is responsibility of a type to return an error;
/// - `Option` always deserialized as `Some` using the same deserializer.
///   If attribute or text content is missed, then the deserializer even wouldn't
///   be used, so if it is used, then the value should be;
/// - units (`()`) and unit structs always deserialized successfully, the content is ignored;
/// - newtype structs forwards deserialization to the inner type using the same
///   deserializer;
/// - sequences, tuples and tuple structs are deserialized as `xs:list`s. Only
///   sequences of primitive types is possible to deserialize this way and they
///   should be delimited by a space (` `, `\t`, `\r`, or `\n`);
/// - structs and maps delegates to [`Self::deserialize_str`] which calls
///   [`Visitor::visit_borrowed_str`] or [`Visitor::visit_string`]; it is responsibility
///   of the type to return an error if it does not able to process passed data;
/// - enums:
///   - the variant name is deserialized using the same deserializer;
///   - the content is deserialized using the deserializer that always returns unit (`()`):
///     - unit variants: just return `()`;
///     - newtype variants: deserialize from [`UnitDeserializer`];
///     - tuple and struct variants: call [`Visitor::visit_unit`];
/// - identifiers are deserialized as strings.
///
/// [simple types]: https://www.w3.org/TR/xmlschema11-1/#Simple_Type_Definition
/// [`FromStr`]: std::str::FromStr
/// [specification]: https://www.w3.org/TR/xmlschema11-2/#boolean
pub struct SimpleTypeDeserializer<'de, 'a> {
    /// - In case of attribute contains escaped attribute value
    /// - In case of text contains unescaped text value
    content: CowRef<'de, 'a, [u8]>,
    /// If `true`, `content` in escaped form and should be unescaped before use
    escaped: bool,
    /// Decoder used to deserialize string data, numeric and boolean data.
    /// Not used for deserializing raw byte buffers
    decoder: Decoder,
}

impl<'de, 'a> SimpleTypeDeserializer<'de, 'a> {
    /// Creates a deserializer from a value, that possible borrowed from input.
    ///
    /// It is assumed that `text` does not have entities.
    pub fn from_text(text: Cow<'de, str>) -> Self {
        let content = match text {
            Cow::Borrowed(slice) => CowRef::Input(slice.as_bytes()),
            Cow::Owned(content) => CowRef::Owned(content.into_bytes()),
        };
        Self::new(content, false, Decoder::utf8())
    }
    /// Creates a deserializer from an XML text node, that possible borrowed from input.
    ///
    /// It is assumed that `text` does not have entities.
    ///
    /// This constructor used internally to deserialize from text nodes.
    pub fn from_text_content(value: Text<'de>) -> Self {
        Self::from_text(value.text)
    }

    /// Creates a deserializer from a part of value at specified range.
    ///
    /// This constructor used internally to deserialize from attribute values.
    #[allow(clippy::ptr_arg)]
    pub(crate) fn from_part(
        value: &'a Cow<'de, [u8]>,
        range: Range<usize>,
        escaped: bool,
        decoder: Decoder,
    ) -> Self {
        let content = match value {
            Cow::Borrowed(slice) => CowRef::Input(&slice[range]),
            Cow::Owned(slice) => CowRef::Slice(&slice[range]),
        };
        Self::new(content, escaped, decoder)
    }

    /// Constructor for tests
    #[inline]
    const fn new(content: CowRef<'de, 'a, [u8]>, escaped: bool, decoder: Decoder) -> Self {
        Self {
            content,
            escaped,
            decoder,
        }
    }

    /// Decodes raw bytes using the encoding specified.
    /// The method will borrow if has the UTF-8 compatible representation.
    #[inline]
    fn decode<'b>(&'b self) -> Result<CowRef<'de, 'b, str>, DeError> {
        Ok(match self.content {
            CowRef::Input(content) => match self.decoder.decode(content)? {
                Cow::Borrowed(content) => CowRef::Input(content),
                Cow::Owned(content) => CowRef::Owned(content),
            },
            CowRef::Slice(content) => match self.decoder.decode(content)? {
                Cow::Borrowed(content) => CowRef::Slice(content),
                Cow::Owned(content) => CowRef::Owned(content),
            },
            CowRef::Owned(ref content) => match self.decoder.decode(content)? {
                Cow::Borrowed(content) => CowRef::Slice(content),
                Cow::Owned(content) => CowRef::Owned(content),
            },
        })
    }
}

impl<'de, 'a> Deserializer<'de> for SimpleTypeDeserializer<'de, 'a> {
    type Error = DeError;

    /// Forwards deserialization to the [`Self::deserialize_str`]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    deserialize_primitive!(deserialize_bool);

    deserialize_primitive!(deserialize_i8);
    deserialize_primitive!(deserialize_i16);
    deserialize_primitive!(deserialize_i32);
    deserialize_primitive!(deserialize_i64);

    deserialize_primitive!(deserialize_u8);
    deserialize_primitive!(deserialize_u16);
    deserialize_primitive!(deserialize_u32);
    deserialize_primitive!(deserialize_u64);

    serde_if_integer128! {
        deserialize_primitive!(deserialize_i128);
        deserialize_primitive!(deserialize_u128);
    }

    deserialize_primitive!(deserialize_f32);
    deserialize_primitive!(deserialize_f64);

    deserialize_primitive!(deserialize_str);

    /// Forwards deserialization to the [`Self::deserialize_str`]
    #[inline]
    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    /// Forwards deserialization to the [`Self::deserialize_str`]
    #[inline]
    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    /// Forwards deserialization to the [`Self::deserialize_str`]
    #[inline]
    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    /// Forwards deserialization to the [`Self::deserialize_str`]
    #[inline]
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    #[inline]
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    /// Forwards deserialization to the [`Self::deserialize_unit`]
    #[inline]
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

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let content = match self.decode()? {
            CowRef::Input(s) => Content::Input(s),
            CowRef::Slice(s) => Content::Slice(s),
            CowRef::Owned(s) => Content::Owned(s, 0),
        };
        visitor.visit_seq(ListIter {
            content: Some(content),
            escaped: self.escaped,
        })
    }

    /// Representation of tuples the same as [sequences][Self::deserialize_seq].
    #[inline]
    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    /// Representation of named tuples the same as [unnamed tuples][Self::deserialize_tuple].
    #[inline]
    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, DeError>
    where
        V: Visitor<'de>,
    {
        self.deserialize_tuple(len, visitor)
    }

    unsupported!(deserialize_map);
    unsupported!(deserialize_struct(&'static str, &'static [&'static str]));

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(self)
    }

    /// Forwards deserialization to the [`Self::deserialize_str`]
    #[inline]
    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    #[inline]
    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

impl<'de, 'a> EnumAccess<'de> for SimpleTypeDeserializer<'de, 'a> {
    type Error = DeError;
    type Variant = UnitOnly;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), DeError>
    where
        V: DeserializeSeed<'de>,
    {
        let name = seed.deserialize(self)?;
        Ok((name, UnitOnly))
    }
}

impl<'de, 'a> IntoDeserializer<'de, DeError> for SimpleTypeDeserializer<'de, 'a> {
    type Deserializer = Self;

    #[inline]
    fn into_deserializer(self) -> Self {
        self
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use crate::se::simple_type::{QuoteTarget, SimpleTypeSerializer};
    use crate::se::QuoteLevel;
    use crate::utils::{ByteBuf, Bytes};
    use serde::de::IgnoredAny;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    macro_rules! simple_only {
        ($encoding:ident, $name:ident: $type:ty = $xml:expr => $result:expr) => {
            #[test]
            fn $name() {
                let decoder = Decoder::$encoding();
                let xml = $xml;
                let de = SimpleTypeDeserializer::new(CowRef::Input(xml.as_ref()), true, decoder);
                let data: $type = Deserialize::deserialize(de).unwrap();

                assert_eq!(data, $result);
            }
        };
    }

    macro_rules! simple {
        ($encoding:ident, $name:ident: $type:ty = $xml:expr => $result:expr) => {
            #[test]
            fn $name() {
                let decoder = Decoder::$encoding();
                let xml = $xml;
                let de = SimpleTypeDeserializer::new(CowRef::Input(xml.as_ref()), true, decoder);
                let data: $type = Deserialize::deserialize(de).unwrap();

                assert_eq!(data, $result);

                // Roundtrip to ensure that serializer corresponds to deserializer
                assert_eq!(
                    data.serialize(SimpleTypeSerializer {
                        writer: String::new(),
                        target: QuoteTarget::Text,
                        level: QuoteLevel::Full,
                    })
                    .unwrap(),
                    xml
                );
            }
        };
    }

    macro_rules! err {
        ($encoding:ident, $name:ident: $type:ty = $xml:expr => $kind:ident($reason:literal)) => {
            #[test]
            fn $name() {
                let decoder = Decoder::$encoding();
                let xml = $xml;
                let de = SimpleTypeDeserializer::new(CowRef::Input(xml.as_ref()), true, decoder);
                let err = <$type as Deserialize>::deserialize(de).unwrap_err();

                match err {
                    DeError::$kind(e) => assert_eq!(e, $reason),
                    _ => panic!(
                        "Expected `Err({}({}))`, but got `{:?}`",
                        stringify!($kind),
                        $reason,
                        err
                    ),
                }
            }
        };
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Unit;

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Newtype(String);

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Tuple((), ());

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct BorrowedNewtype<'a>(&'a str);

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Struct {
        key: String,
        val: usize,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    enum Enum {
        Unit,
        Newtype(String),
        Tuple(String, usize),
        Struct { key: String, val: usize },
    }

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(field_identifier)]
    enum Id {
        Field,
    }

    #[derive(Debug, Deserialize)]
    #[serde(transparent)]
    struct Any(IgnoredAny);
    impl PartialEq for Any {
        fn eq(&self, _other: &Any) -> bool {
            true
        }
    }

    /// Tests for deserialize atomic and union values, as defined in XSD specification
    mod atomic {
        use super::*;
        use crate::se::simple_type::AtomicSerializer;
        use pretty_assertions::assert_eq;
        use std::ops::Deref;

        /// Checks that given `$input` successfully deserializing into given `$result`
        macro_rules! deserialized_to_only {
            ($name:ident: $type:ty = $input:literal => $result:expr) => {
                #[test]
                fn $name() {
                    let de = AtomicDeserializer {
                        content: CowRef::Input($input),
                        escaped: true,
                    };
                    let data: $type = Deserialize::deserialize(de).unwrap();

                    assert_eq!(data, $result);
                }
            };
        }

        /// Checks that given `$input` successfully deserializing into given `$result`
        /// and the result is serialized back to the `$input`
        macro_rules! deserialized_to {
            ($name:ident: $type:ty = $input:literal => $result:expr) => {
                #[test]
                fn $name() {
                    let de = AtomicDeserializer {
                        content: CowRef::Input($input),
                        escaped: true,
                    };
                    let data: $type = Deserialize::deserialize(de).unwrap();

                    assert_eq!(data, $result);

                    // Roundtrip to ensure that serializer corresponds to deserializer
                    let mut buffer = String::new();
                    let has_written = data
                        .serialize(AtomicSerializer {
                            writer: &mut buffer,
                            target: QuoteTarget::Text,
                            level: QuoteLevel::Full,
                            write_delimiter: false,
                        })
                        .unwrap();
                    assert_eq!(buffer, $input);
                    assert_eq!(has_written, !buffer.is_empty());
                }
            };
        }

        /// Checks that attempt to deserialize given `$input` as a `$type` results to a
        /// deserialization error `$kind` with `$reason`
        macro_rules! err {
            ($name:ident: $type:ty = $input:literal => $kind:ident($reason:literal)) => {
                #[test]
                fn $name() {
                    let de = AtomicDeserializer {
                        content: CowRef::Input($input),
                        escaped: true,
                    };
                    let err = <$type as Deserialize>::deserialize(de).unwrap_err();

                    match err {
                        DeError::$kind(e) => assert_eq!(e, $reason),
                        _ => panic!(
                            "Expected `Err({}({}))`, but got `{:?}`",
                            stringify!($kind),
                            $reason,
                            err
                        ),
                    }
                }
            };
        }

        deserialized_to!(false_: bool = "false" => false);
        deserialized_to!(true_: bool  = "true" => true);

        deserialized_to!(i8_:  i8  = "-2" => -2);
        deserialized_to!(i16_: i16 = "-2" => -2);
        deserialized_to!(i32_: i32 = "-2" => -2);
        deserialized_to!(i64_: i64 = "-2" => -2);

        deserialized_to!(u8_:  u8  = "3" => 3);
        deserialized_to!(u16_: u16 = "3" => 3);
        deserialized_to!(u32_: u32 = "3" => 3);
        deserialized_to!(u64_: u64 = "3" => 3);

        serde_if_integer128! {
            deserialized_to!(i128_: i128 = "-2" => -2);
            deserialized_to!(u128_: u128 = "2" => 2);
        }

        deserialized_to!(f32_: f32 = "1.23" => 1.23);
        deserialized_to!(f64_: f64 = "1.23" => 1.23);

        deserialized_to!(char_unescaped: char = "h" => 'h');
        deserialized_to!(char_escaped: char = "&lt;" => '<');

        deserialized_to!(string: String = "&lt;escaped&#32;string" => "<escaped string");
        // Serializer will escape space. Because borrowing has meaning only for deserializer,
        // no need to test roundtrip here, it is already tested with non-borrowing version
        deserialized_to_only!(borrowed_str: &str = "non-escaped string" => "non-escaped string");
        err!(escaped_str: &str = "escaped&#32;string"
                => Custom("invalid type: string \"escaped string\", expected a borrowed string"));

        err!(byte_buf: ByteBuf = "&lt;escaped&#32;string"
                => Custom("invalid type: string \"<escaped string\", expected byte data"));
        err!(borrowed_bytes: Bytes = "non-escaped string"
                => Custom("invalid type: string \"non-escaped string\", expected borrowed bytes"));

        deserialized_to!(option_none: Option<&str> = "" => None);
        deserialized_to!(option_some: Option<&str> = "non-escaped-string" => Some("non-escaped-string"));

        deserialized_to_only!(unit: () = "<root>anything</root>" => ());
        deserialized_to_only!(unit_struct: Unit = "<root>anything</root>" => Unit);

        deserialized_to!(newtype_owned: Newtype = "&lt;escaped&#32;string" => Newtype("<escaped string".into()));
        // Serializer will escape space. Because borrowing has meaning only for deserializer,
        // no need to test roundtrip here, it is already tested with non-borrowing version
        deserialized_to_only!(newtype_borrowed: BorrowedNewtype = "non-escaped string"
                => BorrowedNewtype("non-escaped string"));

        err!(seq: Vec<()> = "non-escaped string"
                => Custom("invalid type: string \"non-escaped string\", expected a sequence"));
        err!(tuple: ((), ()) = "non-escaped string"
                => Custom("invalid type: string \"non-escaped string\", expected a tuple of size 2"));
        err!(tuple_struct: Tuple = "non-escaped string"
                => Custom("invalid type: string \"non-escaped string\", expected tuple struct Tuple"));

        err!(map: HashMap<(), ()> = "non-escaped string"
                => Custom("invalid type: string \"non-escaped string\", expected a map"));
        err!(struct_: Struct = "non-escaped string"
                => Custom("invalid type: string \"non-escaped string\", expected struct Struct"));

        deserialized_to!(enum_unit: Enum = "Unit" => Enum::Unit);
        err!(enum_newtype: Enum = "Newtype"
                => Custom("invalid type: unit value, expected a string"));
        err!(enum_tuple: Enum = "Tuple"
                => Custom("invalid type: unit value, expected tuple variant Enum::Tuple"));
        err!(enum_struct: Enum = "Struct"
                => Custom("invalid type: unit value, expected struct variant Enum::Struct"));
        err!(enum_other: Enum = "any data"
                => Custom("unknown variant `any data`, expected one of `Unit`, `Newtype`, `Tuple`, `Struct`"));

        deserialized_to_only!(identifier: Id = "Field" => Id::Field);
        deserialized_to_only!(ignored_any: Any = "any data" => Any(IgnoredAny));

        /// Checks that deserialization from an owned content is working
        #[test]
        #[cfg(feature = "encoding")]
        fn owned_data() {
            let de = AtomicDeserializer {
                content: CowRef::Owned("string slice".into()),
                escaped: true,
            };
            assert_eq!(de.content.deref(), "string slice");

            let data: String = Deserialize::deserialize(de).unwrap();
            assert_eq!(data, "string slice");
        }

        /// Checks that deserialization from a content borrowed from some
        /// buffer other that input is working
        #[test]
        fn borrowed_from_deserializer() {
            let de = AtomicDeserializer {
                content: CowRef::Slice("string slice"),
                escaped: true,
            };
            assert_eq!(de.content.deref(), "string slice");

            let data: String = Deserialize::deserialize(de).unwrap();
            assert_eq!(data, "string slice");
        }
    }

    /// Module for testing list accessor
    mod list {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn empty() {
            let mut seq = ListIter {
                content: Some(Content::Input("")),
                escaped: true,
            };

            assert_eq!(seq.next_element::<&str>().unwrap(), None);
            assert_eq!(seq.next_element::<&str>().unwrap(), None);
        }

        #[test]
        fn only_spaces() {
            let mut seq = ListIter {
                content: Some(Content::Input("  ")),
                escaped: true,
            };

            assert_eq!(seq.next_element::<&str>().unwrap(), None);
            assert_eq!(seq.next_element::<&str>().unwrap(), None);
        }

        #[test]
        fn one_item() {
            let mut seq = ListIter {
                content: Some(Content::Input("abc")),
                escaped: true,
            };

            assert_eq!(seq.next_element::<&str>().unwrap(), Some("abc"));
            assert_eq!(seq.next_element::<&str>().unwrap(), None);
            assert_eq!(seq.next_element::<&str>().unwrap(), None);
        }

        #[test]
        fn two_items() {
            let mut seq = ListIter {
                content: Some(Content::Input("abc def")),
                escaped: true,
            };

            assert_eq!(seq.next_element::<&str>().unwrap(), Some("abc"));
            assert_eq!(seq.next_element::<&str>().unwrap(), Some("def"));
            assert_eq!(seq.next_element::<&str>().unwrap(), None);
            assert_eq!(seq.next_element::<&str>().unwrap(), None);
        }

        #[test]
        fn leading_spaces() {
            let mut seq = ListIter {
                content: Some(Content::Input("  def")),
                escaped: true,
            };

            assert_eq!(seq.next_element::<&str>().unwrap(), Some("def"));
            assert_eq!(seq.next_element::<&str>().unwrap(), None);
            assert_eq!(seq.next_element::<&str>().unwrap(), None);
        }

        #[test]
        fn trailing_spaces() {
            let mut seq = ListIter {
                content: Some(Content::Input("abc  ")),
                escaped: true,
            };

            assert_eq!(seq.next_element::<&str>().unwrap(), Some("abc"));
            assert_eq!(seq.next_element::<&str>().unwrap(), None);
            assert_eq!(seq.next_element::<&str>().unwrap(), None);
        }

        #[test]
        fn mixed_types() {
            let mut seq = ListIter {
                content: Some(Content::Input("string 1.23 42 true false h Unit")),
                escaped: true,
            };

            assert_eq!(seq.next_element::<&str>().unwrap(), Some("string"));
            assert_eq!(seq.next_element::<f32>().unwrap(), Some(1.23));
            assert_eq!(seq.next_element::<u32>().unwrap(), Some(42));
            assert_eq!(seq.next_element::<bool>().unwrap(), Some(true));
            assert_eq!(seq.next_element::<bool>().unwrap(), Some(false));
            assert_eq!(seq.next_element::<char>().unwrap(), Some('h'));
            assert_eq!(seq.next_element::<Enum>().unwrap(), Some(Enum::Unit));
            assert_eq!(seq.next_element::<()>().unwrap(), None);
            assert_eq!(seq.next_element::<()>().unwrap(), None);
        }
    }

    mod utf8 {
        use super::*;
        use pretty_assertions::assert_eq;

        simple!(utf8, i8_:  i8  = "-2" => -2);
        simple!(utf8, i16_: i16 = "-2" => -2);
        simple!(utf8, i32_: i32 = "-2" => -2);
        simple!(utf8, i64_: i64 = "-2" => -2);

        simple!(utf8, u8_:  u8  = "3" => 3);
        simple!(utf8, u16_: u16 = "3" => 3);
        simple!(utf8, u32_: u32 = "3" => 3);
        simple!(utf8, u64_: u64 = "3" => 3);

        serde_if_integer128! {
            simple!(utf8, i128_: i128 = "-2" => -2);
            simple!(utf8, u128_: u128 = "2" => 2);
        }

        simple!(utf8, f32_: f32 = "1.23" => 1.23);
        simple!(utf8, f64_: f64 = "1.23" => 1.23);

        simple!(utf8, false_: bool = "false" => false);
        simple!(utf8, true_: bool  = "true" => true);
        simple!(utf8, char_unescaped: char = "h" => 'h');
        simple!(utf8, char_escaped: char = "&lt;" => '<');

        simple!(utf8, string: String = "&lt;escaped string" => "<escaped string");
        err!(utf8, byte_buf: ByteBuf = "&lt;escaped&#32;string"
             => Custom("invalid type: string \"<escaped string\", expected byte data"));

        simple!(utf8, borrowed_str: &str = "non-escaped string" => "non-escaped string");
        err!(utf8, borrowed_bytes: Bytes = "&lt;escaped&#32;string"
             => Custom("invalid type: string \"<escaped string\", expected borrowed bytes"));

        simple!(utf8, option_none: Option<&str> = "" => Some(""));
        simple!(utf8, option_some: Option<&str> = "non-escaped string" => Some("non-escaped string"));

        simple_only!(utf8, unit: () = "any data" => ());
        simple_only!(utf8, unit_struct: Unit = "any data" => Unit);

        // Serializer will not escape space because this is unnecessary.
        // Because borrowing has meaning only for deserializer, no need to test
        // roundtrip here, it is already tested for strings where compatible list
        // of escaped characters is used
        simple_only!(utf8, newtype_owned: Newtype = "&lt;escaped&#32;string"
            => Newtype("<escaped string".into()));
        simple_only!(utf8, newtype_borrowed: BorrowedNewtype = "non-escaped string"
            => BorrowedNewtype("non-escaped string"));

        err!(utf8, map: HashMap<(), ()> = "any data"
             => Custom("invalid type: string \"any data\", expected a map"));
        err!(utf8, struct_: Struct = "any data"
             => Custom("invalid type: string \"any data\", expected struct Struct"));

        simple!(utf8, enum_unit: Enum = "Unit" => Enum::Unit);
        err!(utf8, enum_newtype: Enum = "Newtype"
             => Custom("invalid type: unit value, expected a string"));
        err!(utf8, enum_tuple: Enum = "Tuple"
             => Custom("invalid type: unit value, expected tuple variant Enum::Tuple"));
        err!(utf8, enum_struct: Enum = "Struct"
             => Custom("invalid type: unit value, expected struct variant Enum::Struct"));
        err!(utf8, enum_other: Enum = "any data"
             => Custom("unknown variant `any data`, expected one of `Unit`, `Newtype`, `Tuple`, `Struct`"));

        simple_only!(utf8, identifier: Id = "Field" => Id::Field);
        simple_only!(utf8, ignored_any: Any = "any data" => Any(IgnoredAny));
    }

    #[cfg(feature = "encoding")]
    mod utf16 {
        use super::*;
        use pretty_assertions::assert_eq;

        fn to_utf16(string: &str) -> Vec<u8> {
            let mut bytes = Vec::new();
            for ch in string.encode_utf16() {
                bytes.extend_from_slice(&ch.to_le_bytes());
            }
            bytes
        }

        macro_rules! utf16 {
            ($name:ident: $type:ty = $xml:literal => $result:expr) => {
                simple_only!(utf16, $name: $type = to_utf16($xml) => $result);
            };
        }

        macro_rules! unsupported {
            ($name:ident: $type:ty = $xml:literal => $err:literal) => {
                err!(utf16, $name: $type = to_utf16($xml) => Custom($err));
            };
        }

        utf16!(i8_:  i8  = "-2" => -2);
        utf16!(i16_: i16 = "-2" => -2);
        utf16!(i32_: i32 = "-2" => -2);
        utf16!(i64_: i64 = "-2" => -2);

        utf16!(u8_:  u8  = "3" => 3);
        utf16!(u16_: u16 = "3" => 3);
        utf16!(u32_: u32 = "3" => 3);
        utf16!(u64_: u64 = "3" => 3);

        serde_if_integer128! {
            utf16!(i128_: i128 = "-2" => -2);
            utf16!(u128_: u128 = "2" => 2);
        }

        utf16!(f32_: f32 = "1.23" => 1.23);
        utf16!(f64_: f64 = "1.23" => 1.23);

        utf16!(false_: bool = "false" => false);
        utf16!(true_: bool  = "true" => true);
        utf16!(char_unescaped: char = "h" => 'h');
        utf16!(char_escaped: char = "&lt;" => '<');

        utf16!(string: String = "&lt;escaped&#32;string" => "<escaped string");
        unsupported!(borrowed_bytes: Bytes = "&lt;escaped&#32;string"
                    => "invalid type: string \"<escaped string\", expected borrowed bytes");

        utf16!(option_none: Option<()> = "" => Some(()));
        utf16!(option_some: Option<()> = "any data" => Some(()));

        utf16!(unit: () = "any data" => ());
        utf16!(unit_struct: Unit = "any data" => Unit);

        utf16!(newtype_owned: Newtype = "&lt;escaped&#32;string" => Newtype("<escaped string".into()));

        // UTF-16 data never borrow because data was decoded not in-place
        unsupported!(newtype_borrowed: BorrowedNewtype = "non-escaped string"
                    => "invalid type: string \"non-escaped string\", expected a borrowed string");

        unsupported!(map: HashMap<(), ()> = "any data"
                    => "invalid type: string \"any data\", expected a map");
        unsupported!(struct_: Struct = "any data"
                    => "invalid type: string \"any data\", expected struct Struct");

        utf16!(enum_unit: Enum = "Unit" => Enum::Unit);
        unsupported!(enum_newtype: Enum = "Newtype"
                    => "invalid type: unit value, expected a string");
        unsupported!(enum_tuple: Enum = "Tuple"
                    => "invalid type: unit value, expected tuple variant Enum::Tuple");
        unsupported!(enum_struct: Enum = "Struct"
                    => "invalid type: unit value, expected struct variant Enum::Struct");
        unsupported!(enum_other: Enum = "any data"
                    => "unknown variant `any data`, expected one of `Unit`, `Newtype`, `Tuple`, `Struct`");

        utf16!(identifier: Id = "Field" => Id::Field);
        utf16!(ignored_any: Any = "any data" => Any(IgnoredAny));
    }
}
