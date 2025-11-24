//! Serde `Deserializer` module

use crate::{
    de::key::QNameDeserializer,
    de::resolver::EntityResolver,
    de::simple_type::SimpleTypeDeserializer,
    de::text::TextDeserializer,
    de::{DeEvent, Deserializer, XmlRead, TEXT_KEY, VALUE_KEY},
    encoding::Decoder,
    errors::serialize::DeError,
    errors::Error,
    events::attributes::IterState,
    events::BytesStart,
    name::QName,
    utils::CowRef,
};
use serde::de::value::BorrowedStrDeserializer;
use serde::de::{self, DeserializeSeed, Deserializer as _, MapAccess, SeqAccess, Visitor};
use serde::serde_if_integer128;
use std::borrow::Cow;
use std::ops::Range;

/// Defines a source that should be used to deserialize a value in the next call
/// to [`next_value_seed()`](MapAccess::next_value_seed)
#[derive(Debug, PartialEq)]
enum ValueSource {
    /// Source are not specified, because [`next_key_seed()`] not yet called.
    /// This is an initial state and state after deserializing value
    /// (after call of [`next_value_seed()`]).
    ///
    /// Attempt to call [`next_value_seed()`] while accessor in this state would
    /// return a [`DeError::KeyNotRead`] error.
    ///
    /// [`next_key_seed()`]: MapAccess::next_key_seed
    /// [`next_value_seed()`]: MapAccess::next_value_seed
    Unknown,
    /// Next value should be deserialized from an attribute value; value is located
    /// at specified span.
    Attribute(Range<usize>),
    /// Value should be deserialized from the text content of the XML node, which
    /// represented or by an ordinary text node, or by a CDATA node:
    ///
    /// ```xml
    /// <any-tag>
    ///     <key>text content</key>
    /// <!--     ^^^^^^^^^^^^ - this will be used to deserialize map value -->
    /// </any-tag>
    /// ```
    /// ```xml
    /// <any-tag>
    ///     <key><![CDATA[cdata content]]></key>
    /// <!--              ^^^^^^^^^^^^^ - this will be used to deserialize a map value -->
    /// </any-tag>
    /// ```
    Text,
    /// Next value should be deserialized from an element with an any name, except
    /// elements with a name matching one of the struct fields. Corresponding tag
    /// name will always be associated with a field with name [`VALUE_KEY`].
    ///
    /// That state is set when call to [`peek()`] returns a [`Start`] event, which
    /// [`name()`] is not listed in the [list of known fields] (which for a struct
    /// is a list of field names, and for a map that is an empty list), _and_
    /// struct has a field with a special name [`VALUE_KEY`].
    ///
    /// When in this state, next event, returned by [`next()`], will be a [`Start`],
    /// which represents both a key, and a value. Value would be deserialized from
    /// the whole element and how is will be done determined by the value deserializer.
    /// The [`ElementMapAccess`] do not consume any events in that state.
    ///
    /// Because in that state any encountered `<tag>` is mapped to the [`VALUE_KEY`]
    /// field, it is possible to use tag name as an enum discriminator, so `enum`s
    /// can be deserialized from that XMLs:
    ///
    /// ```xml
    /// <any-tag>
    ///     <variant1>...</variant1>
    /// <!-- ~~~~~~~~               - this data will determine that this is Enum::variant1 -->
    /// <!--^^^^^^^^^^^^^^^^^^^^^^^ - this data will be used to deserialize a map value -->
    /// </any-tag>
    /// ```
    /// ```xml
    /// <any-tag>
    ///     <variant2>...</variant2>
    /// <!-- ~~~~~~~~               - this data will determine that this is Enum::variant2 -->
    /// <!--^^^^^^^^^^^^^^^^^^^^^^^ - this data will be used to deserialize a map value -->
    /// </any-tag>
    /// ```
    ///
    /// both can be deserialized into
    ///
    /// ```ignore
    /// enum Enum {
    ///   variant1,
    ///   variant2,
    /// }
    /// struct AnyName {
    ///   #[serde(rename = "$value")]
    ///   field: Enum,
    /// }
    /// ```
    ///
    /// That is possible, because value deserializer have access to the full content
    /// of a `<variant1>...</variant1>` or `<variant2>...</variant2>` node, including
    /// the tag name.
    ///
    /// [`Start`]: DeEvent::Start
    /// [`peek()`]: Deserializer::peek()
    /// [`next()`]: Deserializer::next()
    /// [`name()`]: BytesStart::name()
    /// [`Text`]: Self::Text
    /// [list of known fields]: ElementMapAccess::fields
    Content,
    /// Next value should be deserialized from an element with a dedicated name.
    /// If deserialized type is a sequence, then that sequence will collect all
    /// elements with the same name until it will be filled. If not all elements
    /// would be consumed, the rest will be ignored.
    ///
    /// That state is set when call to [`peek()`] returns a [`Start`] event, which
    /// [`name()`] represents a field name. That name will be deserialized as a key.
    ///
    /// When in this state, next event, returned by [`next()`], will be a [`Start`],
    /// which represents both a key, and a value. Value would be deserialized from
    /// the whole element and how is will be done determined by the value deserializer.
    /// The [`ElementMapAccess`] do not consume any events in that state.
    ///
    /// An illustration below shows, what data is used to deserialize key and value:
    /// ```xml
    /// <any-tag>
    ///     <key>...</key>
    /// <!-- ~~~           - this data will be used to deserialize a map key -->
    /// <!--^^^^^^^^^^^^^^ - this data will be used to deserialize a map value -->
    /// </any-tag>
    /// ```
    ///
    /// Although value deserializer will have access to the full content of a `<key>`
    /// node (including the tag name), it will not get much benefits from that,
    /// because tag name will always be fixed for a given map field (equal to a
    /// field name). So, if the field type is an `enum`, it cannot select its
    /// variant based on the tag name. If that is needed, then [`Content`] variant
    /// of this enum should be used. Such usage is enabled by annotating a struct
    /// field as "content" field, which implemented as given the field a special
    /// [`VALUE_KEY`] name.
    ///
    /// [`Start`]: DeEvent::Start
    /// [`peek()`]: Deserializer::peek()
    /// [`next()`]: Deserializer::next()
    /// [`name()`]: BytesStart::name()
    /// [`Content`]: Self::Content
    Nested,
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A deserializer that extracts map-like structures from an XML. This deserializer
/// represents a one XML tag:
///
/// ```xml
/// <tag>...</tag>
/// ```
///
/// Name of this tag is stored in a [`Self::start`] property.
///
/// # Lifetimes
///
/// - `'de` lifetime represents a buffer, from which deserialized values can
///   borrow their data. Depending on the underlying reader, there can be an
///   internal buffer of deserializer (i.e. deserializer itself) or an input
///   (in that case it is possible to approach zero-copy deserialization).
///
/// - `'d` lifetime represents a parent deserializer, which could own the data
///   buffer.
pub(crate) struct ElementMapAccess<'de, 'd, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    /// Tag -- owner of attributes
    start: BytesStart<'de>,
    de: &'d mut Deserializer<'de, R, E>,
    /// State of the iterator over attributes. Contains the next position in the
    /// inner `start` slice, from which next attribute should be parsed.
    iter: IterState,
    /// Current state of the accessor that determines what next call to API
    /// methods should return.
    source: ValueSource,
    /// List of field names of the struct. It is empty for maps
    fields: &'static [&'static str],
    /// If `true`, then the deserialized struct has a field with a special name:
    /// [`VALUE_KEY`]. That field should be deserialized from the whole content
    /// of an XML node, including tag name:
    ///
    /// ```xml
    /// <tag>value for VALUE_KEY field<tag>
    /// ```
    has_value_field: bool,
}

impl<'de, 'd, R, E> ElementMapAccess<'de, 'd, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    /// Create a new ElementMapAccess
    pub fn new(
        de: &'d mut Deserializer<'de, R, E>,
        start: BytesStart<'de>,
        fields: &'static [&'static str],
    ) -> Result<Self, DeError> {
        Ok(Self {
            de,
            iter: IterState::new(start.name().as_ref().len(), false),
            start,
            source: ValueSource::Unknown,
            fields,
            has_value_field: fields.contains(&VALUE_KEY),
        })
    }

    /// Determines if subtree started with the specified event shoould be skipped.
    ///
    /// Used to map elements with `xsi:nil` attribute set to true to `None` in optional contexts.
    ///
    /// We need to handle two attributes:
    /// - on parent element: <map xsi:nil="true"><foo/></map>
    /// - on this element:   <map><foo xsi:nil="true"/></map>
    ///
    /// We check parent element too because `xsi:nil` affects only nested elements of the
    /// tag where it is defined. We can map structure with fields mapped to attributes to
    /// the `<map>` element and set to `None` all its optional elements.
    fn should_skip_subtree(&self, start: &BytesStart) -> bool {
        self.de.reader.reader.has_nil_attr(&self.start) || self.de.reader.reader.has_nil_attr(start)
    }
}

impl<'de, 'd, R, E> MapAccess<'de> for ElementMapAccess<'de, 'd, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    type Error = DeError;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        debug_assert_eq!(self.source, ValueSource::Unknown);

        // FIXME: There error positions counted from the start of tag name - need global position
        let slice = &self.start.buf;
        let decoder = self.de.reader.decoder();

        if let Some(a) = self.iter.next(slice).transpose()? {
            // try getting map from attributes (key= "value")
            let (key, value) = a.into();
            self.source = ValueSource::Attribute(value.unwrap_or_default());

            let de =
                QNameDeserializer::from_attr(QName(&slice[key]), decoder, &mut self.de.key_buf)?;
            seed.deserialize(de).map(Some)
        } else {
            // try getting from events (<key>value</key>)
            match self.de.peek()? {
                // We shouldn't have both `$value` and `$text` fields in the same
                // struct, so if we have `$value` field, the we should deserialize
                // text content to `$value`
                DeEvent::Text(_) if self.has_value_field => {
                    self.source = ValueSource::Content;
                    // Deserialize `key` from special attribute name which means
                    // that value should be taken from the text content of the
                    // XML node
                    let de = BorrowedStrDeserializer::<DeError>::new(VALUE_KEY);
                    seed.deserialize(de).map(Some)
                }
                DeEvent::Text(_) => {
                    self.source = ValueSource::Text;
                    // Deserialize `key` from special attribute name which means
                    // that value should be taken from the text content of the
                    // XML node
                    let de = BorrowedStrDeserializer::<DeError>::new(TEXT_KEY);
                    seed.deserialize(de).map(Some)
                }
                // Used to deserialize collections of enums, like:
                // <root>
                //   <A/>
                //   <B/>
                //   <C/>
                // </root>
                //
                // into
                //
                // enum Enum { A, B, С }
                // struct Root {
                //     #[serde(rename = "$value")]
                //     items: Vec<Enum>,
                // }
                // TODO: This should be handled by #[serde(flatten)]
                // See https://github.com/serde-rs/serde/issues/1905
                DeEvent::Start(e) if self.has_value_field && not_in(self.fields, e, decoder)? => {
                    self.source = ValueSource::Content;

                    let de = BorrowedStrDeserializer::<DeError>::new(VALUE_KEY);
                    seed.deserialize(de).map(Some)
                }
                DeEvent::Start(e) => {
                    self.source = ValueSource::Nested;

                    let de = QNameDeserializer::from_elem(e.raw_name(), decoder)?;
                    seed.deserialize(de).map(Some)
                }
                // Stop iteration after reaching a closing tag
                // The matching tag name is guaranteed by the reader if our
                // deserializer implementation is correct
                DeEvent::End(e) => {
                    debug_assert_eq!(self.start.name(), e.name());
                    // Consume End
                    self.de.next()?;
                    Ok(None)
                }
                // We cannot get `Eof` legally, because we always inside of the
                // opened tag `self.start`
                DeEvent::Eof => Err(Error::missed_end(self.start.name(), decoder).into()),
            }
        }
    }

    fn next_value_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<K::Value, Self::Error> {
        match std::mem::replace(&mut self.source, ValueSource::Unknown) {
            ValueSource::Attribute(value) => seed.deserialize(SimpleTypeDeserializer::from_part(
                &self.start.buf,
                value,
                true,
                self.de.reader.decoder(),
            )),
            // This arm processes the following XML shape:
            // <any-tag>
            //   text value
            // </any-tag>
            // The whole map represented by an `<any-tag>` element, the map key
            // is implicit and equals to the `TEXT_KEY` constant, and the value
            // is a `Text` event (the value deserializer will see that event)
            // This case are checked by "xml_schema_lists::element" tests in tests/serde-de.rs
            ValueSource::Text => match self.de.next()? {
                DeEvent::Text(e) => seed.deserialize(SimpleTypeDeserializer::from_text_content(e)),
                // SAFETY: We set `Text` only when we seen `Text`
                _ => unreachable!(),
            },
            // This arm processes the following XML shape:
            // <any-tag>
            //   <any>...</any>
            // </any-tag>
            // The whole map represented by an `<any-tag>` element, the map key
            // is implicit and equals to the `VALUE_KEY` constant, and the value
            // is a `Start` event (the value deserializer will see that event)
            ValueSource::Content => seed.deserialize(MapValueDeserializer {
                map: self,
                fixed_name: false,
            }),
            // This arm processes the following XML shape:
            // <any-tag>
            //   <tag>...</tag>
            // </any-tag>
            // The whole map represented by an `<any-tag>` element, the map key
            // is a `tag`, and the value is a `Start` event (the value deserializer
            // will see that event)
            ValueSource::Nested => seed.deserialize(MapValueDeserializer {
                map: self,
                fixed_name: true,
            }),
            ValueSource::Unknown => Err(DeError::KeyNotRead),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A deserializer for a value of map or struct. That deserializer slightly
/// differently processes events for a primitive types and sequences than
/// a [`Deserializer`].
///
/// This deserializer used to deserialize two kinds of fields:
/// - usual fields with a dedicated name, such as `field_one` or `field_two`, in
///   that case field [`Self::fixed_name`] is `true`;
/// - the special `$value` field which represents any tag or a textual content
///   in the XML which would be found in the document, in that case field
///   [`Self::fixed_name`] is `false`.
///
/// This deserializer can see two kind of events at the start:
/// - [`DeEvent::Text`]
/// - [`DeEvent::Start`]
///
/// which represents two possible variants of items:
/// ```xml
/// <item>A tag item</item>
/// A text item
/// <yet another="tag item"/>
/// ```
///
/// This deserializer are very similar to a [`ElementDeserializer`]. The only difference
/// in the `deserialize_seq` method. This deserializer will act as an iterator
/// over tags / text within it's parent tag, whereas the [`ElementDeserializer`]
/// will represent sequences as an `xs:list`.
///
/// This deserializer processes items as following:
/// - primitives (numbers, booleans, strings, characters) are deserialized either
///   from a text content, or unwrapped from a one level of a tag. So, `123` and
///   `<int>123</int>` both can be deserialized into an `u32`;
/// - `Option`:
///   - empty text of [`DeEvent::Text`] is deserialized as `None`;
///   - everything else are deserialized as `Some` using the same deserializer,
///     including `<tag/>` or `<tag></tag>`;
/// - units (`()`) and unit structs consumes the whole text or element subtree;
/// - newtype structs are deserialized by forwarding deserialization of inner type
///   with the same deserializer;
/// - sequences, tuples and tuple structs are deserialized by iterating within the
///   parent tag and deserializing each tag or text content using [`ElementDeserializer`];
/// - structs and maps are deserialized using new instance of [`ElementMapAccess`];
/// - enums:
///   - in case of [`DeEvent::Text`] event the text content is deserialized as
///     a `$text` variant. Enum content is deserialized from the text using
///     [`SimpleTypeDeserializer`];
///   - in case of [`DeEvent::Start`] event the tag name is deserialized as
///     an enum tag, and the content inside are deserialized as an enum content.
///     Depending on a variant kind deserialization is performed as:
///     - unit variants: consuming text content or a subtree;
///     - newtype variants: forward deserialization to the inner type using
///       this deserializer;
///     - tuple variants: call [`deserialize_tuple`] of this deserializer;
///     - struct variants: call [`deserialize_struct`] of this deserializer.
///
/// [`deserialize_tuple`]: #method.deserialize_tuple
/// [`deserialize_struct`]: #method.deserialize_struct
struct MapValueDeserializer<'de, 'd, 'm, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    /// Access to the map that created this deserializer. Gives access to the
    /// context, such as list of fields, that current map known about.
    map: &'m mut ElementMapAccess<'de, 'd, R, E>,
    /// Whether this deserializer was created for deserialization from an element
    /// with fixed name, or the elements with different names or even text are allowed.
    ///
    /// If this field is `true`, we process `<tag>` element in the following XML shape:
    ///
    /// ```xml
    /// <any-tag>
    ///   <tag>...</tag>
    /// </any-tag>
    /// ```
    ///
    /// The whole map represented by an `<any-tag>` element, the map key is a `tag`,
    /// and the value starts with is a `Start("tag")` (the value deserializer will
    /// see that event first) and extended to the matching `End("tag")` event.
    /// In order to deserialize primitives (such as `usize`) we need to allow to
    /// look inside the one levels of tags, so the
    ///
    /// ```xml
    /// <tag>42<tag>
    /// ```
    ///
    /// could be deserialized into `42usize` without problems, and at the same time
    ///
    /// ```xml
    /// <tag>
    ///   <key1/>
    ///   <key2/>
    ///   <!--...-->
    /// <tag>
    /// ```
    /// could be deserialized to a struct.
    ///
    /// If this field is `false`, we processes the one of following XML shapes:
    ///
    /// ```xml
    /// <any-tag>
    ///   text value
    /// </any-tag>
    /// ```
    /// ```xml
    /// <any-tag>
    ///   <![CDATA[cdata value]]>
    /// </any-tag>
    /// ```
    /// ```xml
    /// <any-tag>
    ///   <any>...</any>
    /// </any-tag>
    /// ```
    ///
    /// The whole map represented by an `<any-tag>` element, the map key is
    /// implicit and equals to the [`VALUE_KEY`] constant, and the value is
    /// a [`Text`], or a [`Start`] event (the value deserializer will see one of
    /// those events). In the first two cases the value of this field do not matter
    /// (because we already see the textual event and there no reasons to look
    /// "inside" something), but in the last case the primitives should raise
    /// a deserialization error, because that means that you trying to deserialize
    /// the following struct:
    ///
    /// ```ignore
    /// struct AnyName {
    ///   #[serde(rename = "$value")]
    ///   any_name: String,
    /// }
    /// ```
    /// which means that `any_name` should get a content of the `<any-tag>` element.
    ///
    /// Changing this can be valuable for <https://github.com/tafia/quick-xml/issues/383>,
    /// but those fields should be explicitly marked that they want to get any
    /// possible markup as a `String` and that mark is different from marking them
    /// as accepting "text content" which the currently `$text` means.
    ///
    /// [`Text`]: DeEvent::Text
    /// [`Start`]: DeEvent::Start
    fixed_name: bool,
}

impl<'de, 'd, 'm, R, E> MapValueDeserializer<'de, 'd, 'm, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    /// Returns a next string as concatenated content of consequent [`Text`] and
    /// [`CData`] events, used inside [`deserialize_primitives!()`].
    ///
    /// [`Text`]: crate::events::Event::Text
    /// [`CData`]: crate::events::Event::CData
    #[inline]
    fn read_string(&mut self) -> Result<Cow<'de, str>, DeError> {
        // TODO: Read the whole content to fix https://github.com/tafia/quick-xml/issues/483
        self.map.de.read_string_impl(self.fixed_name)
    }
}

impl<'de, 'd, 'm, R, E> de::Deserializer<'de> for MapValueDeserializer<'de, 'd, 'm, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    type Error = DeError;

    deserialize_primitives!(mut);

    #[inline]
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.map.de.deserialize_unit(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // We cannot use result of `peek()` directly because of borrow checker
        let _ = self.map.de.peek()?;
        match self.map.de.last_peeked() {
            DeEvent::Text(t) if t.is_empty() => visitor.visit_none(),
            DeEvent::Start(start) if self.map.should_skip_subtree(start) => {
                self.map.de.skip_next_tree()?;
                visitor.visit_none()
            }
            _ => visitor.visit_some(self),
        }
    }

    /// Forwards deserialization of the inner type. Always calls [`Visitor::visit_newtype_struct`]
    /// with the same deserializer.
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

    /// Deserializes each `<tag>` in
    /// ```xml
    /// <any-tag>
    ///   <tag>...</tag>
    ///   <tag>...</tag>
    ///   <tag>...</tag>
    /// </any-tag>
    /// ```
    /// as a sequence item, where `<any-tag>` represents a Map in a [`Self::map`],
    /// and a `<tag>` is a sequential field of that map.
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let filter = if self.fixed_name {
            match self.map.de.peek()? {
                // Clone is cheap if event borrows from the input
                DeEvent::Start(e) => TagFilter::Include(e.clone()),
                // SAFETY: we use that deserializer with `fixed_name == true`
                // only from the `ElementMapAccess::next_value_seed` and only when we
                // peeked `Start` event
                _ => unreachable!(),
            }
        } else {
            TagFilter::Exclude(self.map.fields)
        };
        visitor.visit_seq(MapValueSeqAccess {
            #[cfg(feature = "overlapped-lists")]
            checkpoint: self.map.de.skip_checkpoint(),

            map: self.map,
            filter,
        })
    }

    #[inline]
    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.map.de.deserialize_struct(name, fields, visitor)
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
        if self.fixed_name {
            match self.map.de.next()? {
                // Handles <field>UnitEnumVariant</field>
                DeEvent::Start(e) => {
                    // skip <field>, read text after it and ensure that it is ended by </field>
                    let text = self.map.de.read_text(e.name())?;
                    if text.is_empty() {
                        // Map empty text (<field/>) to a special `$text` variant
                        visitor.visit_enum(SimpleTypeDeserializer::from_text(TEXT_KEY.into()))
                    } else {
                        visitor.visit_enum(SimpleTypeDeserializer::from_text(text))
                    }
                }
                // SAFETY: we use that deserializer with `fixed_name == true`
                // only from the `MapAccess::next_value_seed` and only when we
                // peeked `Start` event
                _ => unreachable!(),
            }
        } else {
            visitor.visit_enum(self)
        }
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.map.de.peek()? {
            DeEvent::Text(_) => self.deserialize_str(visitor),
            _ => self.deserialize_map(visitor),
        }
    }
}

impl<'de, 'd, 'm, R, E> de::EnumAccess<'de> for MapValueDeserializer<'de, 'd, 'm, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    type Error = DeError;
    type Variant = MapValueVariantAccess<'de, 'd, 'm, R, E>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let decoder = self.map.de.reader.decoder();
        let (name, is_text) = match self.map.de.peek()? {
            DeEvent::Start(e) => (
                seed.deserialize(QNameDeserializer::from_elem(e.raw_name(), decoder)?)?,
                false,
            ),
            DeEvent::Text(_) => (
                seed.deserialize(BorrowedStrDeserializer::<DeError>::new(TEXT_KEY))?,
                true,
            ),
            // SAFETY: we use that deserializer only when we peeked `Start` or `Text` event
            _ => unreachable!(),
        };
        Ok((
            name,
            MapValueVariantAccess {
                map: self.map,
                is_text,
            },
        ))
    }
}

struct MapValueVariantAccess<'de, 'd, 'm, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    /// Access to the map that created this enum accessor. Gives access to the
    /// context, such as list of fields, that current map known about.
    map: &'m mut ElementMapAccess<'de, 'd, R, E>,
    /// `true` if variant should be deserialized from a textual content
    /// and `false` if from tag
    is_text: bool,
}

impl<'de, 'd, 'm, R, E> de::VariantAccess<'de> for MapValueVariantAccess<'de, 'd, 'm, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    type Error = DeError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        match self.map.de.next()? {
            // Consume subtree
            DeEvent::Start(e) => self.map.de.read_to_end(e.name()),
            // Does not needed to deserialize using SimpleTypeDeserializer, because
            // it returns `()` when `deserialize_unit()` is requested
            DeEvent::Text(_) => Ok(()),
            // SAFETY: the other events are filtered in `variant_seed()`
            _ => unreachable!("Only `Start` or `Text` events are possible here"),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        if self.is_text {
            match self.map.de.next()? {
                DeEvent::Text(e) => seed.deserialize(SimpleTypeDeserializer::from_text_content(e)),
                // SAFETY: the other events are filtered in `variant_seed()`
                _ => unreachable!("Only `Text` events are possible here"),
            }
        } else {
            seed.deserialize(MapValueDeserializer {
                map: self.map,
                // Because element name already was either mapped to a field name,
                // or to a variant name, we should not treat it as variable
                fixed_name: true,
            })
        }
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.is_text {
            match self.map.de.next()? {
                DeEvent::Text(e) => {
                    SimpleTypeDeserializer::from_text_content(e).deserialize_tuple(len, visitor)
                }
                // SAFETY: the other events are filtered in `variant_seed()`
                _ => unreachable!("Only `Text` events are possible here"),
            }
        } else {
            MapValueDeserializer {
                map: self.map,
                // Because element name already was either mapped to a field name,
                // or to a variant name, we should not treat it as variable
                fixed_name: true,
            }
            .deserialize_tuple(len, visitor)
        }
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.map.de.next()? {
            DeEvent::Start(e) => visitor.visit_map(ElementMapAccess::new(self.map.de, e, fields)?),
            DeEvent::Text(e) => {
                SimpleTypeDeserializer::from_text_content(e).deserialize_struct("", fields, visitor)
            }
            // SAFETY: the other events are filtered in `variant_seed()`
            _ => unreachable!("Only `Start` or `Text` events are possible here"),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Check if tag `start` is included in the `fields` list. `decoder` is used to
/// get a string representation of a tag.
///
/// Returns `true`, if `start` is not in the `fields` list and `false` otherwise.
fn not_in(
    fields: &'static [&'static str],
    start: &BytesStart,
    decoder: Decoder,
) -> Result<bool, DeError> {
    let tag = decoder.decode(start.local_name().into_inner())?;

    Ok(fields.iter().all(|&field| field != tag.as_ref()))
}

/// A filter that determines, what tags should form a sequence.
///
/// There are two types of sequences:
/// - sequence where each element represented by tags with the same name
/// - sequence where each element can have a different tag
///
/// The first variant could represent a collection of structs, the second --
/// a collection of enum variants.
///
/// In the second case we don't know what tag name should be expected as a
/// sequence element, so we accept any element. Since the sequence are flattened
/// into maps, we skip elements which have dedicated fields in a struct by using an
/// `Exclude` filter that filters out elements with names matching field names
/// from the struct.
///
/// # Lifetimes
///
/// `'de` represents a lifetime of the XML input, when filter stores the
/// dedicated tag name
#[derive(Debug)]
enum TagFilter<'de> {
    /// A `SeqAccess` interested only in tags with specified name to deserialize
    /// an XML like this:
    ///
    /// ```xml
    /// <...>
    ///   <tag/>
    ///   <tag/>
    ///   <tag/>
    ///   ...
    /// </...>
    /// ```
    ///
    /// The tag name is stored inside (`b"tag"` for that example)
    Include(BytesStart<'de>), //TODO: Need to store only name instead of a whole tag
    /// A `SeqAccess` interested in tags with any name, except explicitly listed.
    /// Excluded tags are used as struct field names and therefore should not
    /// fall into a `$value` category
    Exclude(&'static [&'static str]),
}

impl<'de> TagFilter<'de> {
    fn is_suitable(&self, start: &BytesStart, decoder: Decoder) -> Result<bool, DeError> {
        match self {
            Self::Include(n) => Ok(n.name() == start.name()),
            Self::Exclude(fields) => not_in(fields, start, decoder),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// An accessor to sequence elements forming a value for struct field.
/// Technically, this sequence is flattened out into structure and sequence
/// elements are overlapped with other fields of a structure. Each call to
/// [`Self::next_element_seed`] consumes a next sub-tree or consequent list
/// of [`Text`] and [`CData`] events.
///
/// ```xml
/// <>
///   ...
///   <item>The is the one item</item>
///   This is <![CDATA[one another]]> item<!-- even when--> it splitted by comments
///   <tag>...and that is the third!</tag>
///   ...
/// </>
/// ```
///
/// Depending on [`Self::filter`], only some of that possible constructs would be
/// an element.
///
/// [`Text`]: crate::events::Event::Text
/// [`CData`]: crate::events::Event::CData
struct MapValueSeqAccess<'de, 'd, 'm, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    /// Accessor to a map that creates this accessor and to a deserializer for
    /// a sequence items.
    map: &'m mut ElementMapAccess<'de, 'd, R, E>,
    /// Filter that determines whether a tag is a part of this sequence.
    ///
    /// When feature [`overlapped-lists`] is not activated, iteration will stop
    /// when found a tag that does not pass this filter.
    ///
    /// When feature [`overlapped-lists`] is activated, all tags, that not pass
    /// this check, will be skipped.
    ///
    /// [`overlapped-lists`]: ../../index.html#overlapped-lists
    filter: TagFilter<'de>,

    /// Checkpoint after which all skipped events should be returned. All events,
    /// that was skipped before creating this checkpoint, will still stay buffered
    /// and will not be returned
    #[cfg(feature = "overlapped-lists")]
    checkpoint: usize,
}

#[cfg(feature = "overlapped-lists")]
impl<'de, 'd, 'm, R, E> Drop for MapValueSeqAccess<'de, 'd, 'm, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    fn drop(&mut self) {
        self.map.de.start_replay(self.checkpoint);
    }
}

impl<'de, 'd, 'm, R, E> SeqAccess<'de> for MapValueSeqAccess<'de, 'd, 'm, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    type Error = DeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, DeError>
    where
        T: DeserializeSeed<'de>,
    {
        let decoder = self.map.de.reader.decoder();
        loop {
            break match self.map.de.peek()? {
                // If we see a tag that we not interested, skip it
                #[cfg(feature = "overlapped-lists")]
                DeEvent::Start(e) if !self.filter.is_suitable(e, decoder)? => {
                    self.map.de.skip()?;
                    continue;
                }
                // Stop iteration when list elements ends
                #[cfg(not(feature = "overlapped-lists"))]
                DeEvent::Start(e) if !self.filter.is_suitable(e, decoder)? => Ok(None),

                // Stop iteration after reaching a closing tag
                // The matching tag name is guaranteed by the reader
                DeEvent::End(e) => {
                    debug_assert_eq!(self.map.start.name(), e.name());
                    Ok(None)
                }
                // We cannot get `Eof` legally, because we always inside of the
                // opened tag `self.map.start`
                DeEvent::Eof => Err(Error::missed_end(self.map.start.name(), decoder).into()),

                DeEvent::Text(_) => match self.map.de.next()? {
                    DeEvent::Text(e) => seed.deserialize(TextDeserializer(e)).map(Some),
                    // SAFETY: we just checked that the next event is Text
                    _ => unreachable!(),
                },
                DeEvent::Start(_) => match self.map.de.next()? {
                    DeEvent::Start(start) => seed
                        .deserialize(ElementDeserializer {
                            start,
                            de: self.map.de,
                        })
                        .map(Some),
                    // SAFETY: we just checked that the next event is Start
                    _ => unreachable!(),
                },
            };
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A deserializer for a single tag item of a mixed sequence of tags and text.
///
/// This deserializer are very similar to a [`MapValueDeserializer`] (when it
/// processes the [`DeEvent::Start`] event). The only difference in the
/// [`deserialize_seq`] method. This deserializer will perform deserialization
/// from the textual content between start and end events, whereas the
/// [`MapValueDeserializer`] will iterate over tags / text within it's parent tag.
///
/// This deserializer processes items as following:
/// - numbers are parsed from a text content between tags using [`FromStr`]. So,
///   `<int>123</int>` can be deserialized into an `u32`;
/// - booleans converted from a text content between tags according to the XML
///   [specification]:
///   - `"true"` and `"1"` converted to `true`;
///   - `"false"` and `"0"` converted to `false`;
/// - strings returned as a text content between tags;
/// - characters also returned as strings. If string contain more than one character
///   or empty, it is responsibility of a type to return an error;
/// - `Option` are always deserialized as `Some` using the same deserializer,
///   including `<tag/>` or `<tag></tag>`;
/// - units (`()`) and unit structs consumes the whole element subtree;
/// - newtype structs forwards deserialization to the inner type using
///   [`SimpleTypeDeserializer`];
/// - sequences, tuples and tuple structs are deserialized using [`SimpleTypeDeserializer`]
///   (this is the difference): text content between tags is passed to
///   [`SimpleTypeDeserializer`];
/// - structs and maps are deserialized using new instance of [`ElementMapAccess`];
/// - enums:
///   - the variant name is deserialized using [`QNameDeserializer`] from the element name;
///   - the content is deserialized using the same deserializer:
///     - unit variants: consuming a subtree and return `()`;
///     - newtype variants forwards deserialization to the inner type using
///       this deserializer;
///     - tuple variants: call [`deserialize_tuple`] of this deserializer;
///     - struct variants: call [`deserialize_struct`] of this deserializer.
///
/// [`deserialize_seq`]: #method.deserialize_seq
/// [`FromStr`]: std::str::FromStr
/// [specification]: https://www.w3.org/TR/xmlschema11-2/#boolean
/// [`deserialize_tuple`]: #method.deserialize_tuple
/// [`deserialize_struct`]: #method.deserialize_struct
struct ElementDeserializer<'de, 'd, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    start: BytesStart<'de>,
    de: &'d mut Deserializer<'de, R, E>,
}

impl<'de, 'd, R, E> ElementDeserializer<'de, 'd, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    /// Returns a next string as concatenated content of consequent [`Text`] and
    /// [`CData`] events, used inside [`deserialize_primitives!()`].
    ///
    /// [`Text`]: crate::events::Event::Text
    /// [`CData`]: crate::events::Event::CData
    #[inline]
    fn read_string(&mut self) -> Result<Cow<'de, str>, DeError> {
        self.de.read_text(self.start.name())
    }
}

impl<'de, 'd, R, E> de::Deserializer<'de> for ElementDeserializer<'de, 'd, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    type Error = DeError;

    deserialize_primitives!(mut);

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // Consume subtree
        self.de.read_to_end(self.start.name())?;
        visitor.visit_unit()
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
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
    ///   <self>inner sequence</self>
    ///   <self>inner sequence</self>
    ///   <self>inner sequence</self>
    ///   ...
    /// </>
    /// ```
    fn deserialize_seq<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let text = self.read_string()?;
        SimpleTypeDeserializer::from_text(text).deserialize_seq(visitor)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(ElementMapAccess::new(self.de, self.start, fields)?)
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
        self.deserialize_map(visitor)
    }
}

impl<'de, 'd, R, E> de::EnumAccess<'de> for ElementDeserializer<'de, 'd, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    type Error = DeError;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let name = seed.deserialize(QNameDeserializer::from_elem(
            self.start.raw_name(),
            self.de.reader.decoder(),
        )?)?;
        Ok((name, self))
    }
}

impl<'de, 'd, R, E> de::VariantAccess<'de> for ElementDeserializer<'de, 'd, R, E>
where
    R: XmlRead<'de>,
    E: EntityResolver,
{
    type Error = DeError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        // Consume subtree
        self.de.read_to_end(self.start.name())
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

////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_not_in() {
    use pretty_assertions::assert_eq;

    let tag = BytesStart::new("tag");

    assert_eq!(not_in(&[], &tag, Decoder::utf8()).unwrap(), true);
    assert_eq!(
        not_in(&["no", "such", "tags"], &tag, Decoder::utf8()).unwrap(),
        true
    );
    assert_eq!(
        not_in(&["some", "tag", "included"], &tag, Decoder::utf8()).unwrap(),
        false
    );

    let tag_ns = BytesStart::new("ns1:tag");
    assert_eq!(
        not_in(&["no", "such", "tags"], &tag_ns, Decoder::utf8()).unwrap(),
        true
    );
    assert_eq!(
        not_in(&["some", "tag", "included"], &tag_ns, Decoder::utf8()).unwrap(),
        false
    );
    assert_eq!(
        not_in(&["some", "namespace", "ns1:tag"], &tag_ns, Decoder::utf8()).unwrap(),
        true
    );
}
