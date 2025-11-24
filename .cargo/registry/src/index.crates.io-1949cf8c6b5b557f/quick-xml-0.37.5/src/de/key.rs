use crate::de::simple_type::UnitOnly;
use crate::encoding::Decoder;
use crate::errors::serialize::DeError;
use crate::name::QName;
use crate::utils::CowRef;
use serde::de::{DeserializeSeed, Deserializer, EnumAccess, Visitor};
use serde::{forward_to_deserialize_any, serde_if_integer128};
use std::borrow::Cow;

macro_rules! deserialize_num {
    ($method:ident, $visit:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            match self.name.parse() {
                Ok(number) => visitor.$visit(number),
                Err(_) => self.name.deserialize_str(visitor),
            }
        }
    };
}

/// Decodes raw bytes using the deserializer encoding.
/// The method will borrow if encoding is UTF-8 compatible and `name` contains
/// only UTF-8 compatible characters (usually only ASCII characters).
#[inline]
fn decode_name<'n>(name: QName<'n>, decoder: Decoder) -> Result<Cow<'n, str>, DeError> {
    let local = name.local_name();
    Ok(decoder.decode(local.into_inner())?)
}

/// A deserializer for xml names of elements and attributes.
///
/// Used for deserializing values from:
/// - attribute names (`<... name="..." ...>`)
/// - element names (`<name>...</name>`)
///
/// Converts a name to an identifier string using the following rules:
///
/// - if it is an [`attribute`] name, put `@` in front of the identifier
/// - if it is a namespace binding (`xmlns` or `xmlns:xxx`) put the decoded name
///   to the identifier
/// - put the decoded [`local_name()`] of a name to the identifier
///
/// The final identifier looks like `[@]local_name`, or `@xmlns`, or `@xmlns:binding`
/// (where `[]` means optional element).
///
/// The deserializer also supports deserializing names as other primitive types:
/// - numbers
/// - booleans
/// - unit (`()`) and unit structs
/// - unit variants of the enumerations
///
/// Because `serde` does not define on which side type conversion should be
/// performed, and because [`Deserialize`] implementation for that primitives
/// in serde does not accept strings, the deserializer will perform conversion
/// by itself.
///
/// The deserializer is able to deserialize unit and unit structs, but any name
/// will be converted to the same unit instance. This is asymmetry with a serializer,
/// which not able to serialize those types, because empty names are impossible
/// in XML.
///
/// `deserialize_any()` returns the same result as `deserialize_identifier()`.
///
/// # Lifetimes
///
/// - `'i`: lifetime of the data that the deserializer borrows from the parsed input
/// - `'d`: lifetime of a deserializer that holds a buffer with content of events
///
/// [`attribute`]: Self::from_attr
/// [`local_name()`]: QName::local_name
/// [`Deserialize`]: serde::Deserialize
pub struct QNameDeserializer<'i, 'd> {
    name: CowRef<'i, 'd, str>,
}

impl<'i, 'd> QNameDeserializer<'i, 'd> {
    /// Creates deserializer from name of an attribute
    pub fn from_attr(
        name: QName<'d>,
        decoder: Decoder,
        key_buf: &'d mut String,
    ) -> Result<Self, DeError> {
        key_buf.clear();
        key_buf.push('@');

        // https://github.com/tafia/quick-xml/issues/537
        // Namespace bindings (xmlns:xxx) map to `@xmlns:xxx` instead of `@xxx`
        if name.as_namespace_binding().is_some() {
            decoder.decode_into(name.into_inner(), key_buf)?;
        } else {
            let local = name.local_name();
            decoder.decode_into(local.into_inner(), key_buf)?;
        };

        Ok(Self {
            name: CowRef::Slice(key_buf),
        })
    }

    /// Creates deserializer from name of an element
    pub fn from_elem(name: CowRef<'i, 'd, [u8]>, decoder: Decoder) -> Result<Self, DeError> {
        let local = match name {
            CowRef::Input(borrowed) => match decode_name(QName(borrowed), decoder)? {
                Cow::Borrowed(borrowed) => CowRef::Input(borrowed),
                Cow::Owned(owned) => CowRef::Owned(owned),
            },
            CowRef::Slice(borrowed) => match decode_name(QName(borrowed), decoder)? {
                Cow::Borrowed(borrowed) => CowRef::Slice(borrowed),
                Cow::Owned(owned) => CowRef::Owned(owned),
            },
            CowRef::Owned(owned) => match decode_name(QName(&owned), decoder)? {
                // SAFETY: Because result is borrowed, no changes was done
                // and we can safely unwrap here
                Cow::Borrowed(_) => CowRef::Owned(String::from_utf8(owned).unwrap()),
                Cow::Owned(owned) => CowRef::Owned(owned),
            },
        };

        Ok(Self { name: local })
    }
}

impl<'de, 'd> Deserializer<'de> for QNameDeserializer<'de, 'd> {
    type Error = DeError;

    forward_to_deserialize_any! {
        char str string
        bytes byte_buf
        seq tuple tuple_struct
        map struct
        ignored_any
    }

    /// According to the <https://www.w3.org/TR/xmlschema11-2/#boolean>,
    /// valid boolean representations are only `"true"`, `"false"`, `"1"`,
    /// and `"0"`.
    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.name.deserialize_bool(visitor)
    }

    deserialize_num!(deserialize_i8, visit_i8);
    deserialize_num!(deserialize_i16, visit_i16);
    deserialize_num!(deserialize_i32, visit_i32);
    deserialize_num!(deserialize_i64, visit_i64);

    deserialize_num!(deserialize_u8, visit_u8);
    deserialize_num!(deserialize_u16, visit_u16);
    deserialize_num!(deserialize_u32, visit_u32);
    deserialize_num!(deserialize_u64, visit_u64);

    serde_if_integer128! {
        deserialize_num!(deserialize_i128, visit_i128);
        deserialize_num!(deserialize_u128, visit_u128);
    }

    deserialize_num!(deserialize_f32, visit_f32);
    deserialize_num!(deserialize_f64, visit_f64);

    /// Calls [`Visitor::visit_unit`]
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

    /// Forwards deserialization to the [`Self::deserialize_identifier`]
    #[inline]
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_identifier(visitor)
    }

    /// If `name` is an empty string then calls [`Visitor::visit_none`],
    /// otherwise calls [`Visitor::visit_some`] with itself
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.name.is_empty() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
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

    /// Calls a [`Visitor::visit_str`] if [`name`] contains only UTF-8
    /// compatible encoded characters and represents an element name and
    /// a [`Visitor::visit_string`] in all other cases.
    ///
    /// [`name`]: Self::name
    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.name {
            CowRef::Input(name) => visitor.visit_borrowed_str(name),
            CowRef::Slice(name) => visitor.visit_str(name),
            CowRef::Owned(name) => visitor.visit_string(name),
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
        visitor.visit_enum(self)
    }
}

impl<'de, 'd> EnumAccess<'de> for QNameDeserializer<'de, 'd> {
    type Error = DeError;
    type Variant = UnitOnly;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let name = seed.deserialize(self)?;
        Ok((name, UnitOnly))
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    use crate::se::key::QNameSerializer;
    use crate::utils::{ByteBuf, Bytes};
    use pretty_assertions::assert_eq;
    use serde::de::IgnoredAny;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Unit;

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Newtype(String);

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Tuple((), ());

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Struct {
        key: String,
        val: usize,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    enum Enum {
        Unit,
        #[serde(rename = "@Attr")]
        Attr,
        Newtype(String),
        Tuple(String, usize),
        Struct {
            key: String,
            val: usize,
        },
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

    /// Checks that given `$input` successfully deserializing into given `$result`
    macro_rules! deserialized_to_only {
        ($name:ident: $type:ty = $input:literal => $result:expr) => {
            #[test]
            fn $name() {
                let de = QNameDeserializer {
                    name: CowRef::Input($input),
                };
                let data: $type = Deserialize::deserialize(de).unwrap();

                assert_eq!(data, $result);
            }
        };
    }

    /// Checks that given `$input` successfully deserializing into given `$result`
    macro_rules! deserialized_to {
        ($name:ident: $type:ty = $input:literal => $result:expr) => {
            #[test]
            fn $name() {
                let de = QNameDeserializer {
                    name: CowRef::Input($input),
                };
                let data: $type = Deserialize::deserialize(de).unwrap();

                assert_eq!(data, $result);

                // Roundtrip to ensure that serializer corresponds to deserializer
                assert_eq!(
                    data.serialize(QNameSerializer {
                        writer: String::new()
                    })
                    .unwrap(),
                    $input
                );
            }
        };
    }

    /// Checks that attempt to deserialize given `$input` as a `$type` results to a
    /// deserialization error `$kind` with `$reason`
    macro_rules! err {
        ($name:ident: $type:ty = $input:literal => $kind:ident($reason:literal)) => {
            #[test]
            fn $name() {
                let de = QNameDeserializer {
                    name: CowRef::Input($input),
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
    err!(char_escaped: char = "&lt;"
        => Custom("invalid value: string \"&lt;\", expected a character"));

    deserialized_to!(string: String = "&lt;escaped&#x20;string" => "&lt;escaped&#x20;string");
    deserialized_to!(borrowed_str: &str = "name" => "name");

    err!(byte_buf: ByteBuf = "&lt;escaped&#x20;string"
        => Custom("invalid type: string \"&lt;escaped&#x20;string\", expected byte data"));
    err!(borrowed_bytes: Bytes = "name"
        => Custom("invalid type: string \"name\", expected borrowed bytes"));

    deserialized_to!(option_none: Option<String> = "" => None);
    deserialized_to!(option_some: Option<String> = "name" => Some("name".into()));

    // Unit structs cannot be represented in some meaningful way, but it meaningful
    // to use them as a placeholder when we want to deserialize _something_
    deserialized_to_only!(unit: () = "anything" => ());
    deserialized_to_only!(unit_struct: Unit = "anything" => Unit);

    deserialized_to!(newtype: Newtype = "&lt;escaped&#x20;string" => Newtype("&lt;escaped&#x20;string".into()));

    err!(seq: Vec<()> = "name"
        => Custom("invalid type: string \"name\", expected a sequence"));
    err!(tuple: ((), ()) = "name"
        => Custom("invalid type: string \"name\", expected a tuple of size 2"));
    err!(tuple_struct: Tuple = "name"
        => Custom("invalid type: string \"name\", expected tuple struct Tuple"));

    err!(map: HashMap<(), ()> = "name"
        => Custom("invalid type: string \"name\", expected a map"));
    err!(struct_: Struct = "name"
        => Custom("invalid type: string \"name\", expected struct Struct"));

    deserialized_to!(enum_unit: Enum = "Unit" => Enum::Unit);
    deserialized_to!(enum_unit_for_attr: Enum = "@Attr" => Enum::Attr);
    err!(enum_newtype: Enum = "Newtype"
        => Custom("invalid type: unit value, expected a string"));
    err!(enum_tuple: Enum = "Tuple"
        => Custom("invalid type: unit value, expected tuple variant Enum::Tuple"));
    err!(enum_struct: Enum = "Struct"
        => Custom("invalid type: unit value, expected struct variant Enum::Struct"));

    // Field identifiers cannot be serialized, and IgnoredAny represented _something_
    // which is not concrete
    deserialized_to_only!(identifier: Id = "Field" => Id::Field);
    deserialized_to_only!(ignored_any: Any = "any-name" => Any(IgnoredAny));
}
