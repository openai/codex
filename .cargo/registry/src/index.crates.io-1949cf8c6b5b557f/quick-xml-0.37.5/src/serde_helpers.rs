//! Provides helper functions to glue an XML with a serde content model.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[macro_export]
#[doc(hidden)]
macro_rules! deserialize_variant {
    // Produce struct enum variant
    ( $de:expr, $enum:tt, $variant:ident {
        $(
            $(#[$meta:meta])*
            $field:ident : $typ:ty
        ),* $(,)?
    } ) => ({
        let var = {
            // Create anonymous type
            #[derive(serde::Deserialize)]
            struct $variant {
                $(
                    $(#[$meta])*
                    $field: $typ,
                )*
            }
            <$variant>::deserialize($de)?
        };
        // Due to https://github.com/rust-lang/rust/issues/86935 we cannot use
        // <$enum> :: $variant
        use $enum :: *;
        $variant {
            $($field: var.$field,)*
        }
    });

    // Produce newtype enum variant
    ( $de:expr, $enum:tt, $variant:ident($typ:ty) ) => ({
        let var = <$typ>::deserialize($de)?;
        <$enum> :: $variant(var)
    });

    // Produce unit enum variant
    ( $de:expr, $enum:tt, $variant:ident ) => ({
        serde::de::IgnoredAny::deserialize($de)?;
        <$enum> :: $variant
    });
}

/// Helper macro that generates different match expressions depending on the presence
/// of default variant
#[macro_export]
#[doc(hidden)]
macro_rules! deserialize_match {
    // Only default variant
    (
        $tag:ident, $de:ident, $enum:ty,
        (_ => $($default_variant:tt)+ )
        $(,)?
    ) => (
        Ok($crate::deserialize_variant!( $de, $enum, $($default_variant)+ ))
    );

    // With default variant
    (
        $tag:ident, $de:ident, $enum:ty,
        $(
            ($variant_tag:literal => $($variant:tt)+ )
        ),*
        , (_ => $($default_variant:tt)+ )
        $(,)?
    ) => (
        match $tag.as_ref() {
            $(
                $variant_tag => Ok($crate::deserialize_variant!( $de, $enum, $($variant)+ )),
            )*
            _ => Ok($crate::deserialize_variant!( $de, $enum, $($default_variant)+ )),
        }
    );

    // Without default variant
    (
        $tag:ident, $de:ident, $enum:ty,
        $(
            ($variant_tag:literal => $($variant:tt)+ )
        ),*
        $(,)?
    ) => (
        match $tag.as_ref() {
            $(
                $variant_tag => Ok($crate::deserialize_variant!( $de, $enum, $($variant)+ )),
            )*
            _ => Err(A::Error::unknown_field(&$tag, &[$($variant_tag),+])),
        }
    );
}

/// A helper to implement [`Deserialize`] for [internally tagged] enums which
/// does not use [`Deserializer::deserialize_any`] that produces wrong results
/// with XML because of [serde#1183].
///
/// In contrast to deriving [`Deserialize`] this macro assumes that a tag will be
/// the first element or attribute in the XML.
///
/// # Example
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use quick_xml::de::from_str;
/// use quick_xml::impl_deserialize_for_internally_tagged_enum;
/// use serde::Deserialize;
///
/// #[derive(Deserialize, Debug, PartialEq)]
/// struct Root {
///     one: InternallyTaggedEnum,
///     two: InternallyTaggedEnum,
///     three: InternallyTaggedEnum,
/// }
///
/// #[derive(Debug, PartialEq)]
/// // #[serde(tag = "@tag")]
/// enum InternallyTaggedEnum {
///     Unit,
///     Newtype(Newtype),
///     Struct {
///         // #[serde(rename = "@attribute")]
///         attribute: u32,
///         element: f32,
///     },
/// }
///
/// #[derive(Deserialize, Debug, PartialEq)]
/// struct Newtype {
///     #[serde(rename = "@attribute")]
///     attribute: u64,
/// }
///
/// // The macro needs the type of the enum, the tag name,
/// // and information about all the variants
/// impl_deserialize_for_internally_tagged_enum!{
///     InternallyTaggedEnum, "@tag",
///     ("Unit"    => Unit),
///     ("Newtype" => Newtype(Newtype)),
///     ("Struct"  => Struct {
///         #[serde(rename = "@attribute")]
///         attribute: u32,
///         element: f32,
///     }),
/// }
///
/// assert_eq!(
///     from_str::<Root>(r#"
///         <root>
///             <one tag="Unit" />
///             <two tag="Newtype" attribute="42" />
///             <three tag="Struct" attribute="42">
///                 <element>4.2</element>
///             </three>
///         </root>
///     "#).unwrap(),
///     Root {
///         one: InternallyTaggedEnum::Unit,
///         two: InternallyTaggedEnum::Newtype(Newtype { attribute: 42 }),
///         three: InternallyTaggedEnum::Struct {
///             attribute: 42,
///             element: 4.2,
///         },
///     },
/// );
/// ```
///
/// You don't necessarily have to provide all the enumeration variants and can use
/// `_` to put every undefined tag into an enumeration variant.
/// This default variant (`_ => ...`) must be the last one to appear in the macro,
/// like `_ => Other` in the example below:
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use quick_xml::de::from_str;
/// use quick_xml::impl_deserialize_for_internally_tagged_enum;
/// use serde::Deserialize;
///
/// #[derive(Deserialize, Debug, PartialEq)]
/// struct Root {
///     one: InternallyTaggedEnum,
///     two: InternallyTaggedEnum,
///     three: InternallyTaggedEnum,
/// }
///
/// #[derive(Debug, PartialEq)]
/// enum InternallyTaggedEnum {
///     NewType(Newtype),
///     Other,
/// }
///
/// #[derive(Deserialize, Debug, PartialEq)]
/// struct Newtype {
///     #[serde(rename = "@attribute")]
///     attribute: u64,
/// }
///
/// // The macro needs the type of the enum, the tag name,
/// // and information about all the variants
/// impl_deserialize_for_internally_tagged_enum!{
///     InternallyTaggedEnum, "@tag",
///     ("NewType" => NewType(Newtype)),
///     (_ => Other),
/// }
///
/// assert_eq!(
///     from_str::<Root>(r#"
///         <root>
///             <one tag="NewType" attribute="42" />
///             <two tag="Something" ignoredAttribute="something" />
///             <three tag="SomethingElse">
///                 <ignoredToo />
///             </three>
///         </root>
///     "#).unwrap(),
///     Root {
///         one: InternallyTaggedEnum::NewType(Newtype { attribute: 42 }),
///         two: InternallyTaggedEnum::Other,
///         three: InternallyTaggedEnum::Other,
///     },
/// );
/// ```
///
/// [internally tagged]: https://serde.rs/enum-representations.html#internally-tagged
/// [serde#1183]: https://github.com/serde-rs/serde/issues/1183
#[macro_export(local_inner_macros)]
macro_rules! impl_deserialize_for_internally_tagged_enum {
    (
        $enum:ty,
        $tag:literal,
        $($cases:tt)*
    ) => {
        impl<'de> serde::de::Deserialize<'de> for $enum {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                use serde::de::{Error, MapAccess, Visitor};

                // The Visitor struct is normally used for state, but none is needed
                struct TheVisitor;
                // The main logic of the deserializing happens in the Visitor trait
                impl<'de> Visitor<'de> for TheVisitor {
                    // The type that is being deserialized
                    type Value = $enum;

                    // Try to give a better error message when this is used wrong
                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str("expecting map with tag in ")?;
                        f.write_str($tag)
                    }

                    // The xml data is provided as an opaque map,
                    // that map is parsed into the type
                    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                    where
                        A: MapAccess<'de>,
                    {
                        // Here the assumption is made that only one attribute
                        // exists and it's the discriminator (enum "tag").
                        let entry: Option<(String, String)> = map.next_entry()?;
                        // If there are more attributes those would need
                        // to be parsed as well.
                        let tag = match entry {
                            // Return an error if the no attributes are found,
                            // and indicate that the @tag attribute is missing.
                            None => Err(A::Error::missing_field($tag)),
                            // Check if the attribute is the tag
                            Some((attribute, value)) => {
                                if attribute == $tag {
                                    // return the value of the tag
                                    Ok(value)
                                } else {
                                    // The attribute is not @tag, return an error
                                    // indicating that there is an unexpected attribute
                                    Err(A::Error::unknown_field(&attribute, &[$tag]))
                                }
                            }
                        }?;

                        let de = serde::de::value::MapAccessDeserializer::new(map);
                        $crate::deserialize_match!( tag, de, $enum, $($cases)* )
                    }
                }
                // Tell the deserializer to deserialize the data as a map,
                // using the TheVisitor as the decoder
                deserializer.deserialize_map(TheVisitor)
            }
        }
    }
}

/// Provides helper functions to serialization and deserialization of types
/// (usually enums) as a text content of an element and intended to use with
/// [`#[serde(with = "...")]`][with], [`#[serde(deserialize_with = "...")]`][de-with]
/// and [`#[serde(serialize_with = "...")]`][se-with].
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use quick_xml::de::from_str;
/// use quick_xml::se::to_string;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// enum SomeEnum {
///     // Default implementation serializes enum as an `<EnumValue/>` element
///     EnumValue,
/// # /*
///     ...
/// # */
/// }
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// #[serde(rename = "some-container")]
/// struct SomeContainer {
///     #[serde(with = "quick_xml::serde_helpers::text_content")]
///     field: SomeEnum,
/// }
///
/// let container = SomeContainer {
///     field: SomeEnum::EnumValue,
/// };
/// let xml = "\
///     <some-container>\
///         <field>EnumValue</field>\
///     </some-container>";
///
/// assert_eq!(to_string(&container).unwrap(), xml);
/// assert_eq!(from_str::<SomeContainer>(xml).unwrap(), container);
/// ```
///
/// Using of this module is equivalent to replacing `field`'s type to this:
///
/// ```
/// # use serde::{Deserialize, Serialize};
/// # type SomeEnum = ();
/// #[derive(Serialize, Deserialize)]
/// struct Field {
///     // Use a special name `$text` to map field to the text content
///     #[serde(rename = "$text")]
///     content: SomeEnum,
/// }
///
/// #[derive(Serialize, Deserialize)]
/// #[serde(rename = "some-container")]
/// struct SomeContainer {
///     field: Field,
/// }
/// ```
/// Read about the meaning of a special [`$text`] field.
///
/// In versions of quick-xml before 0.31.0 this module used to represent enum
/// unit variants as `<field>EnumUnitVariant</field>` instead of `<EnumUnitVariant/>`.
/// Since version 0.31.0 this is default representation of enums in normal fields,
/// and `<EnumUnitVariant/>` requires `$value` field:
///
/// ```
/// # use pretty_assertions::assert_eq;
/// use quick_xml::de::from_str;
/// use quick_xml::se::to_string;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// enum SomeEnum {
///     // Default implementation serializes enum as an `<EnumValue/>` element
///     EnumValue,
/// # /*
///     ...
/// # */
/// }
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// #[serde(rename = "some-container")]
/// struct SomeContainer {
///     #[serde(rename = "$value")]
///     field: SomeEnum,
/// }
///
/// let container = SomeContainer {
///     field: SomeEnum::EnumValue,
/// };
/// let xml = "\
///     <some-container>\
///         <EnumValue/>\
///     </some-container>";
///
/// assert_eq!(to_string(&container).unwrap(), xml);
/// assert_eq!(from_str::<SomeContainer>(xml).unwrap(), container);
/// ```
///
/// [with]: https://serde.rs/field-attrs.html#with
/// [de-with]: https://serde.rs/field-attrs.html#deserialize_with
/// [se-with]: https://serde.rs/field-attrs.html#serialize_with
/// [`$text`]: ../../de/index.html#text
pub mod text_content {
    use super::*;

    /// Serializes `value` as an XSD [simple type]. Intended to use with
    /// `#[serde(serialize_with = "...")]`. See example at [`text_content`]
    /// module level.
    ///
    /// [simple type]: https://www.w3.org/TR/xmlschema11-1/#Simple_Type_Definition
    pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        #[derive(Serialize)]
        struct Field<'a, T> {
            #[serde(rename = "$text")]
            value: &'a T,
        }
        Field { value }.serialize(serializer)
    }

    /// Deserializes XSD's [simple type]. Intended to use with
    /// `#[serde(deserialize_with = "...")]`. See example at [`text_content`]
    /// module level.
    ///
    /// [simple type]: https://www.w3.org/TR/xmlschema11-1/#Simple_Type_Definition
    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        #[derive(Deserialize)]
        struct Field<T> {
            #[serde(rename = "$text")]
            value: T,
        }
        Ok(Field::deserialize(deserializer)?.value)
    }
}
