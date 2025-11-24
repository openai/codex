use crate::{
    de::simple_type::SimpleTypeDeserializer,
    de::{Text, TEXT_KEY},
    errors::serialize::DeError,
    utils::CowRef,
};
use serde::de::value::BorrowedStrDeserializer;
use serde::de::{DeserializeSeed, Deserializer, EnumAccess, VariantAccess, Visitor};
use serde::serde_if_integer128;
use std::borrow::Cow;

/// A deserializer for a single text node of a mixed sequence of tags and text.
///
/// This deserializer are very similar to a [`MapValueDeserializer`] (when it
/// processes the [`DeEvent::Text`] event). The only difference in the
/// `deserialize_seq` method. This deserializer will perform deserialization
/// from a textual content, whereas the [`MapValueDeserializer`] will iterate
/// over tags / text within it's parent tag.
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
/// - `Option`:
///   - empty text is deserialized as `None`;
///   - everything else is deserialized as `Some` using the same deserializer;
/// - units (`()`) and unit structs always deserialized successfully, the content is ignored;
/// - newtype structs forwards deserialization to the inner type using the same
///   deserializer;
/// - sequences, tuples and tuple structs are deserialized using [`SimpleTypeDeserializer`]
///   (this is the difference): text content passed to the deserializer directly;
/// - structs and maps calls [`Visitor::visit_borrowed_str`] or [`Visitor::visit_string`],
///   it is responsibility of the type to return an error if it do not able to process
///   this data;
/// - enums:
///   - the variant name is deserialized as `$text`;
///   - the content is deserialized using the same deserializer:
///     - unit variants: just return `()`;
///     - newtype variants forwards deserialization to the inner type using the
///       same deserializer;
///     - tuple and struct variants are deserialized using [`SimpleTypeDeserializer`].
///
/// [`MapValueDeserializer`]: ../map/struct.MapValueDeserializer.html
/// [`DeEvent::Text`]: crate::de::DeEvent::Text
/// [`FromStr`]: std::str::FromStr
/// [specification]: https://www.w3.org/TR/xmlschema11-2/#boolean
pub struct TextDeserializer<'de>(pub Text<'de>);

impl<'de> TextDeserializer<'de> {
    /// Returns a next string as concatenated content of consequent [`Text`] and
    /// [`CData`] events, used inside [`deserialize_primitives!()`].
    ///
    /// [`Text`]: crate::events::Event::Text
    /// [`CData`]: crate::events::Event::CData
    #[inline]
    fn read_string(self) -> Result<Cow<'de, str>, DeError> {
        Ok(self.0.text)
    }
}

impl<'de> Deserializer<'de> for TextDeserializer<'de> {
    type Error = DeError;

    deserialize_primitives!();

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.0.is_empty() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    /// Forwards deserialization of the inner type. Always calls [`Visitor::visit_newtype_struct`]
    /// with this deserializer.
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

    /// This method deserializes a sequence inside of element that itself is a
    /// sequence element:
    ///
    /// ```xml
    /// <>
    ///   ...
    ///   inner sequence as xs:list
    ///   ...
    /// </>
    /// ```
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        SimpleTypeDeserializer::from_text_content(self.0).deserialize_seq(visitor)
    }

    #[inline]
    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // Deserializer methods are only hints, if deserializer could not satisfy
        // request, it should return the data that it has. It is responsibility
        // of a Visitor to return an error if it does not understand the data
        self.deserialize_str(visitor)
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

    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }
}

impl<'de> EnumAccess<'de> for TextDeserializer<'de> {
    type Error = DeError;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let name = seed.deserialize(BorrowedStrDeserializer::<DeError>::new(TEXT_KEY))?;
        Ok((name, self))
    }
}

impl<'de> VariantAccess<'de> for TextDeserializer<'de> {
    type Error = DeError;

    #[inline]
    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self)
    }

    #[inline]
    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_tuple(len, visitor)
    }

    #[inline]
    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_struct("", fields, visitor)
    }
}
