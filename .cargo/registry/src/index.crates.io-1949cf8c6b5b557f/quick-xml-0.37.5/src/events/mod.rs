//! Defines zero-copy XML events used throughout this library.
//!
//! A XML event often represents part of a XML element.
//! They occur both during reading and writing and are
//! usually used with the stream-oriented API.
//!
//! For example, the XML element
//! ```xml
//! <name attr="value">Inner text</name>
//! ```
//! consists of the three events `Start`, `Text` and `End`.
//! They can also represent other parts in an XML document like the
//! XML declaration. Each Event usually contains further information,
//! like the tag name, the attribute or the inner text.
//!
//! See [`Event`] for a list of all possible events.
//!
//! # Reading
//! When reading a XML stream, the events are emitted by [`Reader::read_event`]
//! and [`Reader::read_event_into`]. You must listen
//! for the different types of events you are interested in.
//!
//! See [`Reader`] for further information.
//!
//! # Writing
//! When writing the XML document, you must create the XML element
//! by constructing the events it consists of and pass them to the writer
//! sequentially.
//!
//! See [`Writer`] for further information.
//!
//! [`Reader::read_event`]: crate::reader::Reader::read_event
//! [`Reader::read_event_into`]: crate::reader::Reader::read_event_into
//! [`Reader`]: crate::reader::Reader
//! [`Writer`]: crate::writer::Writer
//! [`Event`]: crate::events::Event

pub mod attributes;

#[cfg(feature = "encoding")]
use encoding_rs::Encoding;
use std::borrow::Cow;
use std::fmt::{self, Debug, Formatter};
use std::iter::FusedIterator;
use std::mem::replace;
use std::ops::Deref;
use std::str::from_utf8;

use crate::encoding::{Decoder, EncodingError};
use crate::errors::{Error, IllFormedError};
use crate::escape::{
    escape, minimal_escape, partial_escape, resolve_predefined_entity, unescape_with,
};
use crate::name::{LocalName, QName};
#[cfg(feature = "serialize")]
use crate::utils::CowRef;
use crate::utils::{name_len, trim_xml_end, trim_xml_start, write_cow_string, Bytes};
use attributes::{AttrError, Attribute, Attributes};

/// Opening tag data (`Event::Start`), with optional attributes: `<name attr="value">`.
///
/// The name can be accessed using the [`name`] or [`local_name`] methods.
/// An iterator over the attributes is returned by the [`attributes`] method.
///
/// This event implements `Deref<Target = [u8]>`. The `deref()` implementation
/// returns the content of this event between `<` and `>` or `/>`:
///
/// ```
/// # use quick_xml::events::{BytesStart, Event};
/// # use quick_xml::reader::Reader;
/// # use pretty_assertions::assert_eq;
/// // Remember, that \ at the end of string literal strips
/// // all space characters to the first non-space character
/// let mut reader = Reader::from_str("\
///     <element a1 = 'val1' a2=\"val2\" />\
///     <element a1 = 'val1' a2=\"val2\" >"
/// );
/// let content = "element a1 = 'val1' a2=\"val2\" ";
/// let event = BytesStart::from_content(content, 7);
///
/// assert_eq!(reader.read_event().unwrap(), Event::Empty(event.borrow()));
/// assert_eq!(reader.read_event().unwrap(), Event::Start(event.borrow()));
/// // deref coercion of &BytesStart to &[u8]
/// assert_eq!(&event as &[u8], content.as_bytes());
/// // AsRef<[u8]> for &T + deref coercion
/// assert_eq!(event.as_ref(), content.as_bytes());
/// ```
///
/// [`name`]: Self::name
/// [`local_name`]: Self::local_name
/// [`attributes`]: Self::attributes
#[derive(Clone, Eq, PartialEq)]
pub struct BytesStart<'a> {
    /// content of the element, before any utf8 conversion
    pub(crate) buf: Cow<'a, [u8]>,
    /// end of the element name, the name starts at that the start of `buf`
    pub(crate) name_len: usize,
}

impl<'a> BytesStart<'a> {
    /// Internal constructor, used by `Reader`. Supplies data in reader's encoding
    #[inline]
    pub(crate) const fn wrap(content: &'a [u8], name_len: usize) -> Self {
        BytesStart {
            buf: Cow::Borrowed(content),
            name_len,
        }
    }

    /// Creates a new `BytesStart` from the given name.
    ///
    /// # Warning
    ///
    /// `name` must be a valid name.
    #[inline]
    pub fn new<C: Into<Cow<'a, str>>>(name: C) -> Self {
        let buf = str_cow_to_bytes(name);
        BytesStart {
            name_len: buf.len(),
            buf,
        }
    }

    /// Creates a new `BytesStart` from the given content (name + attributes).
    ///
    /// # Warning
    ///
    /// `&content[..name_len]` must be a valid name, and the remainder of `content`
    /// must be correctly-formed attributes. Neither are checked, it is possible
    /// to generate invalid XML if `content` or `name_len` are incorrect.
    #[inline]
    pub fn from_content<C: Into<Cow<'a, str>>>(content: C, name_len: usize) -> Self {
        BytesStart {
            buf: str_cow_to_bytes(content),
            name_len,
        }
    }

    /// Converts the event into an owned event.
    pub fn into_owned(self) -> BytesStart<'static> {
        BytesStart {
            buf: Cow::Owned(self.buf.into_owned()),
            name_len: self.name_len,
        }
    }

    /// Converts the event into an owned event without taking ownership of Event
    pub fn to_owned(&self) -> BytesStart<'static> {
        BytesStart {
            buf: Cow::Owned(self.buf.clone().into_owned()),
            name_len: self.name_len,
        }
    }

    /// Converts the event into a borrowed event. Most useful when paired with [`to_end`].
    ///
    /// # Example
    ///
    /// ```
    /// use quick_xml::events::{BytesStart, Event};
    /// # use quick_xml::writer::Writer;
    /// # use quick_xml::Error;
    ///
    /// struct SomeStruct<'a> {
    ///     attrs: BytesStart<'a>,
    ///     // ...
    /// }
    /// # impl<'a> SomeStruct<'a> {
    /// # fn example(&self) -> Result<(), Error> {
    /// # let mut writer = Writer::new(Vec::new());
    ///
    /// writer.write_event(Event::Start(self.attrs.borrow()))?;
    /// // ...
    /// writer.write_event(Event::End(self.attrs.to_end()))?;
    /// # Ok(())
    /// # }}
    /// ```
    ///
    /// [`to_end`]: Self::to_end
    pub fn borrow(&self) -> BytesStart {
        BytesStart {
            buf: Cow::Borrowed(&self.buf),
            name_len: self.name_len,
        }
    }

    /// Creates new paired close tag
    #[inline]
    pub fn to_end(&self) -> BytesEnd {
        BytesEnd::from(self.name())
    }

    /// Gets the undecoded raw tag name, as present in the input stream.
    #[inline]
    pub fn name(&self) -> QName {
        QName(&self.buf[..self.name_len])
    }

    /// Gets the undecoded raw local tag name (excluding namespace) as present
    /// in the input stream.
    ///
    /// All content up to and including the first `:` character is removed from the tag name.
    #[inline]
    pub fn local_name(&self) -> LocalName {
        self.name().into()
    }

    /// Edit the name of the BytesStart in-place
    ///
    /// # Warning
    ///
    /// `name` must be a valid name.
    pub fn set_name(&mut self, name: &[u8]) -> &mut BytesStart<'a> {
        let bytes = self.buf.to_mut();
        bytes.splice(..self.name_len, name.iter().cloned());
        self.name_len = name.len();
        self
    }

    /// Gets the undecoded raw tag name, as present in the input stream, which
    /// is borrowed either to the input, or to the event.
    ///
    /// # Lifetimes
    ///
    /// - `'a`: Lifetime of the input data from which this event is borrow
    /// - `'e`: Lifetime of the concrete event instance
    // TODO: We should made this is a part of public API, but with safe wrapped for a name
    #[cfg(feature = "serialize")]
    pub(crate) fn raw_name<'e>(&'e self) -> CowRef<'a, 'e, [u8]> {
        match self.buf {
            Cow::Borrowed(b) => CowRef::Input(&b[..self.name_len]),
            Cow::Owned(ref o) => CowRef::Slice(&o[..self.name_len]),
        }
    }
}

/// Attribute-related methods
impl<'a> BytesStart<'a> {
    /// Consumes `self` and yield a new `BytesStart` with additional attributes from an iterator.
    ///
    /// The yielded items must be convertible to [`Attribute`] using `Into`.
    pub fn with_attributes<'b, I>(mut self, attributes: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Attribute<'b>>,
    {
        self.extend_attributes(attributes);
        self
    }

    /// Add additional attributes to this tag using an iterator.
    ///
    /// The yielded items must be convertible to [`Attribute`] using `Into`.
    pub fn extend_attributes<'b, I>(&mut self, attributes: I) -> &mut BytesStart<'a>
    where
        I: IntoIterator,
        I::Item: Into<Attribute<'b>>,
    {
        for attr in attributes {
            self.push_attribute(attr);
        }
        self
    }

    /// Adds an attribute to this element.
    pub fn push_attribute<'b, A>(&mut self, attr: A)
    where
        A: Into<Attribute<'b>>,
    {
        self.buf.to_mut().push(b' ');
        self.push_attr(attr.into());
    }

    /// Remove all attributes from the ByteStart
    pub fn clear_attributes(&mut self) -> &mut BytesStart<'a> {
        self.buf.to_mut().truncate(self.name_len);
        self
    }

    /// Returns an iterator over the attributes of this tag.
    pub fn attributes(&self) -> Attributes {
        Attributes::wrap(&self.buf, self.name_len, false)
    }

    /// Returns an iterator over the HTML-like attributes of this tag (no mandatory quotes or `=`).
    pub fn html_attributes(&self) -> Attributes {
        Attributes::wrap(&self.buf, self.name_len, true)
    }

    /// Gets the undecoded raw string with the attributes of this tag as a `&[u8]`,
    /// including the whitespace after the tag name if there is any.
    #[inline]
    pub fn attributes_raw(&self) -> &[u8] {
        &self.buf[self.name_len..]
    }

    /// Try to get an attribute
    pub fn try_get_attribute<N: AsRef<[u8]> + Sized>(
        &'a self,
        attr_name: N,
    ) -> Result<Option<Attribute<'a>>, AttrError> {
        for a in self.attributes().with_checks(false) {
            let a = a?;
            if a.key.as_ref() == attr_name.as_ref() {
                return Ok(Some(a));
            }
        }
        Ok(None)
    }

    /// Adds an attribute to this element.
    pub(crate) fn push_attr<'b>(&mut self, attr: Attribute<'b>) {
        let bytes = self.buf.to_mut();
        bytes.extend_from_slice(attr.key.as_ref());
        bytes.extend_from_slice(b"=\"");
        // FIXME: need to escape attribute content
        bytes.extend_from_slice(attr.value.as_ref());
        bytes.push(b'"');
    }

    /// Adds new line in existing element
    pub(crate) fn push_newline(&mut self) {
        self.buf.to_mut().push(b'\n');
    }

    /// Adds indentation bytes in existing element
    pub(crate) fn push_indent(&mut self, indent: &[u8]) {
        self.buf.to_mut().extend_from_slice(indent);
    }
}

impl<'a> Debug for BytesStart<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "BytesStart {{ buf: ")?;
        write_cow_string(f, &self.buf)?;
        write!(f, ", name_len: {} }}", self.name_len)
    }
}

impl<'a> Deref for BytesStart<'a> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.buf
    }
}

impl<'a> From<QName<'a>> for BytesStart<'a> {
    #[inline]
    fn from(name: QName<'a>) -> Self {
        let name = name.into_inner();
        Self::wrap(name, name.len())
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for BytesStart<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let s = <&str>::arbitrary(u)?;
        if s.is_empty() || !s.chars().all(char::is_alphanumeric) {
            return Err(arbitrary::Error::IncorrectFormat);
        }
        let mut result = Self::new(s);
        result.extend_attributes(Vec::<(&str, &str)>::arbitrary(u)?.into_iter());
        Ok(result)
    }

    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        return <&str as arbitrary::Arbitrary>::size_hint(depth);
    }
}
////////////////////////////////////////////////////////////////////////////////////////////////////

/// Closing tag data (`Event::End`): `</name>`.
///
/// The name can be accessed using the [`name`] or [`local_name`] methods.
///
/// This event implements `Deref<Target = [u8]>`. The `deref()` implementation
/// returns the content of this event between `</` and `>`.
///
/// Note, that inner text will not contain `>` character inside:
///
/// ```
/// # use quick_xml::events::{BytesEnd, Event};
/// # use quick_xml::reader::Reader;
/// # use pretty_assertions::assert_eq;
/// let mut reader = Reader::from_str(r#"<element></element a1 = 'val1' a2="val2" >"#);
/// // Note, that this entire string considered as a .name()
/// let content = "element a1 = 'val1' a2=\"val2\" ";
/// let event = BytesEnd::new(content);
///
/// reader.config_mut().trim_markup_names_in_closing_tags = false;
/// reader.config_mut().check_end_names = false;
/// reader.read_event().unwrap(); // Skip `<element>`
///
/// assert_eq!(reader.read_event().unwrap(), Event::End(event.borrow()));
/// assert_eq!(event.name().as_ref(), content.as_bytes());
/// // deref coercion of &BytesEnd to &[u8]
/// assert_eq!(&event as &[u8], content.as_bytes());
/// // AsRef<[u8]> for &T + deref coercion
/// assert_eq!(event.as_ref(), content.as_bytes());
/// ```
///
/// [`name`]: Self::name
/// [`local_name`]: Self::local_name
#[derive(Clone, Eq, PartialEq)]
pub struct BytesEnd<'a> {
    name: Cow<'a, [u8]>,
}

impl<'a> BytesEnd<'a> {
    /// Internal constructor, used by `Reader`. Supplies data in reader's encoding
    #[inline]
    pub(crate) const fn wrap(name: Cow<'a, [u8]>) -> Self {
        BytesEnd { name }
    }

    /// Creates a new `BytesEnd` borrowing a slice.
    ///
    /// # Warning
    ///
    /// `name` must be a valid name.
    #[inline]
    pub fn new<C: Into<Cow<'a, str>>>(name: C) -> Self {
        Self::wrap(str_cow_to_bytes(name))
    }

    /// Converts the event into an owned event.
    pub fn into_owned(self) -> BytesEnd<'static> {
        BytesEnd {
            name: Cow::Owned(self.name.into_owned()),
        }
    }

    /// Converts the event into a borrowed event.
    #[inline]
    pub fn borrow(&self) -> BytesEnd {
        BytesEnd {
            name: Cow::Borrowed(&self.name),
        }
    }

    /// Gets the undecoded raw tag name, as present in the input stream.
    #[inline]
    pub fn name(&self) -> QName {
        QName(&self.name)
    }

    /// Gets the undecoded raw local tag name (excluding namespace) as present
    /// in the input stream.
    ///
    /// All content up to and including the first `:` character is removed from the tag name.
    #[inline]
    pub fn local_name(&self) -> LocalName {
        self.name().into()
    }
}

impl<'a> Debug for BytesEnd<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "BytesEnd {{ name: ")?;
        write_cow_string(f, &self.name)?;
        write!(f, " }}")
    }
}

impl<'a> Deref for BytesEnd<'a> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.name
    }
}

impl<'a> From<QName<'a>> for BytesEnd<'a> {
    #[inline]
    fn from(name: QName<'a>) -> Self {
        Self::wrap(name.into_inner().into())
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for BytesEnd<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self::new(<&str>::arbitrary(u)?))
    }
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        return <&str as arbitrary::Arbitrary>::size_hint(depth);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Data from various events (most notably, `Event::Text`) that stored in XML
/// in escaped form. Internally data is stored in escaped form.
///
/// This event implements `Deref<Target = [u8]>`. The `deref()` implementation
/// returns the content of this event. In case of comment this is everything
/// between `<!--` and `-->` and the text of comment will not contain `-->` inside.
/// In case of DTD this is everything between `<!DOCTYPE` + spaces and closing `>`
/// (i.e. in case of DTD the first character is never space):
///
/// ```
/// # use quick_xml::events::{BytesText, Event};
/// # use quick_xml::reader::Reader;
/// # use pretty_assertions::assert_eq;
/// // Remember, that \ at the end of string literal strips
/// // all space characters to the first non-space character
/// let mut reader = Reader::from_str("\
///     <!DOCTYPE comment or text >\
///     comment or text \
///     <!--comment or text -->"
/// );
/// let content = "comment or text ";
/// let event = BytesText::new(content);
///
/// assert_eq!(reader.read_event().unwrap(), Event::DocType(event.borrow()));
/// assert_eq!(reader.read_event().unwrap(), Event::Text(event.borrow()));
/// assert_eq!(reader.read_event().unwrap(), Event::Comment(event.borrow()));
/// // deref coercion of &BytesText to &[u8]
/// assert_eq!(&event as &[u8], content.as_bytes());
/// // AsRef<[u8]> for &T + deref coercion
/// assert_eq!(event.as_ref(), content.as_bytes());
/// ```
#[derive(Clone, Eq, PartialEq)]
pub struct BytesText<'a> {
    /// Escaped then encoded content of the event. Content is encoded in the XML
    /// document encoding when event comes from the reader and should be in the
    /// document encoding when event passed to the writer
    content: Cow<'a, [u8]>,
    /// Encoding in which the `content` is stored inside the event
    decoder: Decoder,
}

impl<'a> BytesText<'a> {
    /// Creates a new `BytesText` from an escaped byte sequence in the specified encoding.
    #[inline]
    pub(crate) fn wrap<C: Into<Cow<'a, [u8]>>>(content: C, decoder: Decoder) -> Self {
        Self {
            content: content.into(),
            decoder,
        }
    }

    /// Creates a new `BytesText` from an escaped string.
    #[inline]
    pub fn from_escaped<C: Into<Cow<'a, str>>>(content: C) -> Self {
        Self::wrap(str_cow_to_bytes(content), Decoder::utf8())
    }

    /// Creates a new `BytesText` from a string. The string is expected not to
    /// be escaped.
    #[inline]
    pub fn new(content: &'a str) -> Self {
        Self::from_escaped(escape(content))
    }

    /// Ensures that all data is owned to extend the object's lifetime if
    /// necessary.
    #[inline]
    pub fn into_owned(self) -> BytesText<'static> {
        BytesText {
            content: self.content.into_owned().into(),
            decoder: self.decoder,
        }
    }

    /// Extracts the inner `Cow` from the `BytesText` event container.
    #[inline]
    pub fn into_inner(self) -> Cow<'a, [u8]> {
        self.content
    }

    /// Converts the event into a borrowed event.
    #[inline]
    pub fn borrow(&self) -> BytesText {
        BytesText {
            content: Cow::Borrowed(&self.content),
            decoder: self.decoder,
        }
    }

    /// Decodes then unescapes the content of the event.
    ///
    /// This will allocate if the value contains any escape sequences or in
    /// non-UTF-8 encoding.
    pub fn unescape(&self) -> Result<Cow<'a, str>, Error> {
        self.unescape_with(resolve_predefined_entity)
    }

    /// Decodes then unescapes the content of the event with custom entities.
    ///
    /// This will allocate if the value contains any escape sequences or in
    /// non-UTF-8 encoding.
    pub fn unescape_with<'entity>(
        &self,
        resolve_entity: impl FnMut(&str) -> Option<&'entity str>,
    ) -> Result<Cow<'a, str>, Error> {
        let decoded = self.decoder.decode_cow(&self.content)?;

        match unescape_with(&decoded, resolve_entity)? {
            // Because result is borrowed, no replacements was done and we can use original string
            Cow::Borrowed(_) => Ok(decoded),
            Cow::Owned(s) => Ok(s.into()),
        }
    }

    /// Removes leading XML whitespace bytes from text content.
    ///
    /// Returns `true` if content is empty after that
    pub fn inplace_trim_start(&mut self) -> bool {
        self.content = trim_cow(
            replace(&mut self.content, Cow::Borrowed(b"")),
            trim_xml_start,
        );
        self.content.is_empty()
    }

    /// Removes trailing XML whitespace bytes from text content.
    ///
    /// Returns `true` if content is empty after that
    pub fn inplace_trim_end(&mut self) -> bool {
        self.content = trim_cow(replace(&mut self.content, Cow::Borrowed(b"")), trim_xml_end);
        self.content.is_empty()
    }
}

impl<'a> Debug for BytesText<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "BytesText {{ content: ")?;
        write_cow_string(f, &self.content)?;
        write!(f, " }}")
    }
}

impl<'a> Deref for BytesText<'a> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.content
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for BytesText<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let s = <&str>::arbitrary(u)?;
        if !s.chars().all(char::is_alphanumeric) {
            return Err(arbitrary::Error::IncorrectFormat);
        }
        Ok(Self::new(s))
    }

    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        return <&str as arbitrary::Arbitrary>::size_hint(depth);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// CDATA content contains unescaped data from the reader. If you want to write them as a text,
/// [convert](Self::escape) it to [`BytesText`].
///
/// This event implements `Deref<Target = [u8]>`. The `deref()` implementation
/// returns the content of this event between `<![CDATA[` and `]]>`.
///
/// Note, that inner text will not contain `]]>` sequence inside:
///
/// ```
/// # use quick_xml::events::{BytesCData, Event};
/// # use quick_xml::reader::Reader;
/// # use pretty_assertions::assert_eq;
/// let mut reader = Reader::from_str("<![CDATA[ CDATA section ]]>");
/// let content = " CDATA section ";
/// let event = BytesCData::new(content);
///
/// assert_eq!(reader.read_event().unwrap(), Event::CData(event.borrow()));
/// // deref coercion of &BytesCData to &[u8]
/// assert_eq!(&event as &[u8], content.as_bytes());
/// // AsRef<[u8]> for &T + deref coercion
/// assert_eq!(event.as_ref(), content.as_bytes());
/// ```
#[derive(Clone, Eq, PartialEq)]
pub struct BytesCData<'a> {
    content: Cow<'a, [u8]>,
    /// Encoding in which the `content` is stored inside the event
    decoder: Decoder,
}

impl<'a> BytesCData<'a> {
    /// Creates a new `BytesCData` from a byte sequence in the specified encoding.
    #[inline]
    pub(crate) fn wrap<C: Into<Cow<'a, [u8]>>>(content: C, decoder: Decoder) -> Self {
        Self {
            content: content.into(),
            decoder,
        }
    }

    /// Creates a new `BytesCData` from a string.
    ///
    /// # Warning
    ///
    /// `content` must not contain the `]]>` sequence. You can use
    /// [`BytesCData::escaped`] to escape the content instead.
    #[inline]
    pub fn new<C: Into<Cow<'a, str>>>(content: C) -> Self {
        Self::wrap(str_cow_to_bytes(content), Decoder::utf8())
    }

    /// Creates an iterator of `BytesCData` from a string.
    ///
    /// If a string contains `]]>`, it needs to be split into multiple `CDATA`
    /// sections, splitting the `]]` and `>` characters, because the CDATA closing
    /// sequence cannot be escaped. This iterator yields a `BytesCData` instance
    /// for each of those sections.
    ///
    /// # Examples
    ///
    /// ```
    /// # use quick_xml::events::BytesCData;
    /// # use pretty_assertions::assert_eq;
    /// let content = "";
    /// let cdata = BytesCData::escaped(content).collect::<Vec<_>>();
    /// assert_eq!(cdata, &[BytesCData::new("")]);
    ///
    /// let content = "Certain tokens like ]]> can be difficult and <invalid>";
    /// let cdata = BytesCData::escaped(content).collect::<Vec<_>>();
    /// assert_eq!(cdata, &[
    ///     BytesCData::new("Certain tokens like ]]"),
    ///     BytesCData::new("> can be difficult and <invalid>"),
    /// ]);
    ///
    /// let content = "foo]]>bar]]>baz]]>quux";
    /// let cdata = BytesCData::escaped(content).collect::<Vec<_>>();
    /// assert_eq!(cdata, &[
    ///     BytesCData::new("foo]]"),
    ///     BytesCData::new(">bar]]"),
    ///     BytesCData::new(">baz]]"),
    ///     BytesCData::new(">quux"),
    /// ]);
    /// ```
    #[inline]
    pub fn escaped(content: &'a str) -> CDataIterator<'a> {
        CDataIterator {
            unprocessed: content.as_bytes(),
            finished: false,
        }
    }

    /// Ensures that all data is owned to extend the object's lifetime if
    /// necessary.
    #[inline]
    pub fn into_owned(self) -> BytesCData<'static> {
        BytesCData {
            content: self.content.into_owned().into(),
            decoder: self.decoder,
        }
    }

    /// Extracts the inner `Cow` from the `BytesCData` event container.
    #[inline]
    pub fn into_inner(self) -> Cow<'a, [u8]> {
        self.content
    }

    /// Converts the event into a borrowed event.
    #[inline]
    pub fn borrow(&self) -> BytesCData {
        BytesCData {
            content: Cow::Borrowed(&self.content),
            decoder: self.decoder,
        }
    }

    /// Converts this CDATA content to an escaped version, that can be written
    /// as an usual text in XML.
    ///
    /// This function performs following replacements:
    ///
    /// | Character | Replacement
    /// |-----------|------------
    /// | `<`       | `&lt;`
    /// | `>`       | `&gt;`
    /// | `&`       | `&amp;`
    /// | `'`       | `&apos;`
    /// | `"`       | `&quot;`
    pub fn escape(self) -> Result<BytesText<'a>, EncodingError> {
        let decoded = self.decode()?;
        Ok(BytesText::wrap(
            match escape(decoded) {
                Cow::Borrowed(escaped) => Cow::Borrowed(escaped.as_bytes()),
                Cow::Owned(escaped) => Cow::Owned(escaped.into_bytes()),
            },
            Decoder::utf8(),
        ))
    }

    /// Converts this CDATA content to an escaped version, that can be written
    /// as an usual text in XML.
    ///
    /// In XML text content, it is allowed (though not recommended) to leave
    /// the quote special characters `"` and `'` unescaped.
    ///
    /// This function performs following replacements:
    ///
    /// | Character | Replacement
    /// |-----------|------------
    /// | `<`       | `&lt;`
    /// | `>`       | `&gt;`
    /// | `&`       | `&amp;`
    pub fn partial_escape(self) -> Result<BytesText<'a>, EncodingError> {
        let decoded = self.decode()?;
        Ok(BytesText::wrap(
            match partial_escape(decoded) {
                Cow::Borrowed(escaped) => Cow::Borrowed(escaped.as_bytes()),
                Cow::Owned(escaped) => Cow::Owned(escaped.into_bytes()),
            },
            Decoder::utf8(),
        ))
    }

    /// Converts this CDATA content to an escaped version, that can be written
    /// as an usual text in XML. This method escapes only those characters that
    /// must be escaped according to the [specification].
    ///
    /// This function performs following replacements:
    ///
    /// | Character | Replacement
    /// |-----------|------------
    /// | `<`       | `&lt;`
    /// | `&`       | `&amp;`
    ///
    /// [specification]: https://www.w3.org/TR/xml11/#syntax
    pub fn minimal_escape(self) -> Result<BytesText<'a>, EncodingError> {
        let decoded = self.decode()?;
        Ok(BytesText::wrap(
            match minimal_escape(decoded) {
                Cow::Borrowed(escaped) => Cow::Borrowed(escaped.as_bytes()),
                Cow::Owned(escaped) => Cow::Owned(escaped.into_bytes()),
            },
            Decoder::utf8(),
        ))
    }

    /// Decodes the raw input byte content of the CDATA section into a string,
    /// without performing XML entity escaping.
    ///
    /// When this event produced by the XML reader, it uses the encoding information
    /// associated with that reader to interpret the raw bytes contained within this
    /// CDATA event.
    pub fn decode(&self) -> Result<Cow<'a, str>, EncodingError> {
        Ok(self.decoder.decode_cow(&self.content)?)
    }
}

impl<'a> Debug for BytesCData<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "BytesCData {{ content: ")?;
        write_cow_string(f, &self.content)?;
        write!(f, " }}")
    }
}

impl<'a> Deref for BytesCData<'a> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.content
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for BytesCData<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self::new(<&str>::arbitrary(u)?))
    }
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        return <&str as arbitrary::Arbitrary>::size_hint(depth);
    }
}

/// Iterator over `CDATA` sections in a string.
///
/// This iterator is created by the [`BytesCData::escaped`] method.
#[derive(Clone)]
pub struct CDataIterator<'a> {
    /// The unprocessed data which should be emitted as `BytesCData` events.
    /// At each iteration, the processed data is cut from this slice.
    unprocessed: &'a [u8],
    finished: bool,
}

impl<'a> Debug for CDataIterator<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("CDataIterator")
            .field("unprocessed", &Bytes(self.unprocessed))
            .field("finished", &self.finished)
            .finish()
    }
}

impl<'a> Iterator for CDataIterator<'a> {
    type Item = BytesCData<'a>;

    fn next(&mut self) -> Option<BytesCData<'a>> {
        if self.finished {
            return None;
        }

        for gt in memchr::memchr_iter(b'>', self.unprocessed) {
            if self.unprocessed[..gt].ends_with(b"]]") {
                let (slice, rest) = self.unprocessed.split_at(gt);
                self.unprocessed = rest;
                return Some(BytesCData::wrap(slice, Decoder::utf8()));
            }
        }

        self.finished = true;
        Some(BytesCData::wrap(self.unprocessed, Decoder::utf8()))
    }
}

impl FusedIterator for CDataIterator<'_> {}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// [Processing instructions][PI] (PIs) allow documents to contain instructions for applications.
///
/// This event implements `Deref<Target = [u8]>`. The `deref()` implementation
/// returns the content of this event between `<?` and `?>`.
///
/// Note, that inner text will not contain `?>` sequence inside:
///
/// ```
/// # use quick_xml::events::{BytesPI, Event};
/// # use quick_xml::reader::Reader;
/// # use pretty_assertions::assert_eq;
/// let mut reader = Reader::from_str("<?processing instruction >:-<~ ?>");
/// let content = "processing instruction >:-<~ ";
/// let event = BytesPI::new(content);
///
/// assert_eq!(reader.read_event().unwrap(), Event::PI(event.borrow()));
/// // deref coercion of &BytesPI to &[u8]
/// assert_eq!(&event as &[u8], content.as_bytes());
/// // AsRef<[u8]> for &T + deref coercion
/// assert_eq!(event.as_ref(), content.as_bytes());
/// ```
///
/// [PI]: https://www.w3.org/TR/xml11/#sec-pi
#[derive(Clone, Eq, PartialEq)]
pub struct BytesPI<'a> {
    content: BytesStart<'a>,
}

impl<'a> BytesPI<'a> {
    /// Creates a new `BytesPI` from a byte sequence in the specified encoding.
    #[inline]
    pub(crate) const fn wrap(content: &'a [u8], target_len: usize) -> Self {
        Self {
            content: BytesStart::wrap(content, target_len),
        }
    }

    /// Creates a new `BytesPI` from a string.
    ///
    /// # Warning
    ///
    /// `content` must not contain the `?>` sequence.
    #[inline]
    pub fn new<C: Into<Cow<'a, str>>>(content: C) -> Self {
        let buf = str_cow_to_bytes(content);
        let name_len = name_len(&buf);
        Self {
            content: BytesStart { buf, name_len },
        }
    }

    /// Ensures that all data is owned to extend the object's lifetime if
    /// necessary.
    #[inline]
    pub fn into_owned(self) -> BytesPI<'static> {
        BytesPI {
            content: self.content.into_owned().into(),
        }
    }

    /// Extracts the inner `Cow` from the `BytesPI` event container.
    #[inline]
    pub fn into_inner(self) -> Cow<'a, [u8]> {
        self.content.buf
    }

    /// Converts the event into a borrowed event.
    #[inline]
    pub fn borrow(&self) -> BytesPI {
        BytesPI {
            content: self.content.borrow(),
        }
    }

    /// A target used to identify the application to which the instruction is directed.
    ///
    /// # Example
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use quick_xml::events::BytesPI;
    ///
    /// let instruction = BytesPI::new(r#"xml-stylesheet href="style.css""#);
    /// assert_eq!(instruction.target(), b"xml-stylesheet");
    /// ```
    #[inline]
    pub fn target(&self) -> &[u8] {
        self.content.name().0
    }

    /// Content of the processing instruction. Contains everything between target
    /// name and the end of the instruction. A direct consequence is that the first
    /// character is always a space character.
    ///
    /// # Example
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use quick_xml::events::BytesPI;
    ///
    /// let instruction = BytesPI::new(r#"xml-stylesheet href="style.css""#);
    /// assert_eq!(instruction.content(), br#" href="style.css""#);
    /// ```
    #[inline]
    pub fn content(&self) -> &[u8] {
        self.content.attributes_raw()
    }

    /// A view of the processing instructions' content as a list of key-value pairs.
    ///
    /// Key-value pairs are used in some processing instructions, for example in
    /// `<?xml-stylesheet?>`.
    ///
    /// Returned iterator does not validate attribute values as may required by
    /// target's rules. For example, it doesn't check that substring `?>` is not
    /// present in the attribute value. That shouldn't be the problem when event
    /// is produced by the reader, because reader detects end of processing instruction
    /// by the first `?>` sequence, as required by the specification, and therefore
    /// this sequence cannot appear inside it.
    ///
    /// # Example
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use std::borrow::Cow;
    /// use quick_xml::events::attributes::Attribute;
    /// use quick_xml::events::BytesPI;
    /// use quick_xml::name::QName;
    ///
    /// let instruction = BytesPI::new(r#"xml-stylesheet href="style.css""#);
    /// for attr in instruction.attributes() {
    ///     assert_eq!(attr, Ok(Attribute {
    ///         key: QName(b"href"),
    ///         value: Cow::Borrowed(b"style.css"),
    ///     }));
    /// }
    /// ```
    #[inline]
    pub fn attributes(&self) -> Attributes {
        self.content.attributes()
    }
}

impl<'a> Debug for BytesPI<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "BytesPI {{ content: ")?;
        write_cow_string(f, &self.content.buf)?;
        write!(f, " }}")
    }
}

impl<'a> Deref for BytesPI<'a> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.content
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for BytesPI<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self::new(<&str>::arbitrary(u)?))
    }
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        return <&str as arbitrary::Arbitrary>::size_hint(depth);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// An XML declaration (`Event::Decl`).
///
/// [W3C XML 1.1 Prolog and Document Type Declaration](http://w3.org/TR/xml11/#sec-prolog-dtd)
///
/// This event implements `Deref<Target = [u8]>`. The `deref()` implementation
/// returns the content of this event between `<?` and `?>`.
///
/// Note, that inner text will not contain `?>` sequence inside:
///
/// ```
/// # use quick_xml::events::{BytesDecl, BytesStart, Event};
/// # use quick_xml::reader::Reader;
/// # use pretty_assertions::assert_eq;
/// let mut reader = Reader::from_str("<?xml version = '1.0' ?>");
/// let content = "xml version = '1.0' ";
/// let event = BytesDecl::from_start(BytesStart::from_content(content, 3));
///
/// assert_eq!(reader.read_event().unwrap(), Event::Decl(event.borrow()));
/// // deref coercion of &BytesDecl to &[u8]
/// assert_eq!(&event as &[u8], content.as_bytes());
/// // AsRef<[u8]> for &T + deref coercion
/// assert_eq!(event.as_ref(), content.as_bytes());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BytesDecl<'a> {
    content: BytesStart<'a>,
}

impl<'a> BytesDecl<'a> {
    /// Constructs a new `XmlDecl` from the (mandatory) _version_ (should be `1.0` or `1.1`),
    /// the optional _encoding_ (e.g., `UTF-8`) and the optional _standalone_ (`yes` or `no`)
    /// attribute.
    ///
    /// Does not escape any of its inputs. Always uses double quotes to wrap the attribute values.
    /// The caller is responsible for escaping attribute values. Shouldn't usually be relevant since
    /// the double quote character is not allowed in any of the attribute values.
    pub fn new(
        version: &str,
        encoding: Option<&str>,
        standalone: Option<&str>,
    ) -> BytesDecl<'static> {
        // Compute length of the buffer based on supplied attributes
        // ' encoding=""'   => 12
        let encoding_attr_len = if let Some(xs) = encoding {
            12 + xs.len()
        } else {
            0
        };
        // ' standalone=""' => 14
        let standalone_attr_len = if let Some(xs) = standalone {
            14 + xs.len()
        } else {
            0
        };
        // 'xml version=""' => 14
        let mut buf = String::with_capacity(14 + encoding_attr_len + standalone_attr_len);

        buf.push_str("xml version=\"");
        buf.push_str(version);

        if let Some(encoding_val) = encoding {
            buf.push_str("\" encoding=\"");
            buf.push_str(encoding_val);
        }

        if let Some(standalone_val) = standalone {
            buf.push_str("\" standalone=\"");
            buf.push_str(standalone_val);
        }
        buf.push('"');

        BytesDecl {
            content: BytesStart::from_content(buf, 3),
        }
    }

    /// Creates a `BytesDecl` from a `BytesStart`
    pub const fn from_start(start: BytesStart<'a>) -> Self {
        Self { content: start }
    }

    /// Gets xml version, excluding quotes (`'` or `"`).
    ///
    /// According to the [grammar], the version *must* be the first thing in the declaration.
    /// This method tries to extract the first thing in the declaration and return it.
    /// In case of multiple attributes value of the first one is returned.
    ///
    /// If version is missed in the declaration, or the first thing is not a version,
    /// [`IllFormedError::MissingDeclVersion`] will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use quick_xml::errors::{Error, IllFormedError};
    /// use quick_xml::events::{BytesDecl, BytesStart};
    ///
    /// // <?xml version='1.1'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" version='1.1'", 0));
    /// assert_eq!(decl.version().unwrap(), b"1.1".as_ref());
    ///
    /// // <?xml version='1.0' version='1.1'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" version='1.0' version='1.1'", 0));
    /// assert_eq!(decl.version().unwrap(), b"1.0".as_ref());
    ///
    /// // <?xml encoding='utf-8'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" encoding='utf-8'", 0));
    /// match decl.version() {
    ///     Err(Error::IllFormed(IllFormedError::MissingDeclVersion(Some(key)))) => assert_eq!(key, "encoding"),
    ///     _ => assert!(false),
    /// }
    ///
    /// // <?xml encoding='utf-8' version='1.1'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" encoding='utf-8' version='1.1'", 0));
    /// match decl.version() {
    ///     Err(Error::IllFormed(IllFormedError::MissingDeclVersion(Some(key)))) => assert_eq!(key, "encoding"),
    ///     _ => assert!(false),
    /// }
    ///
    /// // <?xml?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content("", 0));
    /// match decl.version() {
    ///     Err(Error::IllFormed(IllFormedError::MissingDeclVersion(None))) => {},
    ///     _ => assert!(false),
    /// }
    /// ```
    ///
    /// [grammar]: https://www.w3.org/TR/xml11/#NT-XMLDecl
    pub fn version(&self) -> Result<Cow<[u8]>, Error> {
        // The version *must* be the first thing in the declaration.
        match self.content.attributes().with_checks(false).next() {
            Some(Ok(a)) if a.key.as_ref() == b"version" => Ok(a.value),
            // first attribute was not "version"
            Some(Ok(a)) => {
                let found = from_utf8(a.key.as_ref())
                    .map_err(|_| IllFormedError::MissingDeclVersion(None))?
                    .to_string();
                Err(Error::IllFormed(IllFormedError::MissingDeclVersion(Some(
                    found,
                ))))
            }
            // error parsing attributes
            Some(Err(e)) => Err(e.into()),
            // no attributes
            None => Err(Error::IllFormed(IllFormedError::MissingDeclVersion(None))),
        }
    }

    /// Gets xml encoding, excluding quotes (`'` or `"`).
    ///
    /// Although according to the [grammar] encoding must appear before `"standalone"`
    /// and after `"version"`, this method does not check that. The first occurrence
    /// of the attribute will be returned even if there are several. Also, method does
    /// not restrict symbols that can forming the encoding, so the returned encoding
    /// name may not correspond to the grammar.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::borrow::Cow;
    /// use quick_xml::Error;
    /// use quick_xml::events::{BytesDecl, BytesStart};
    ///
    /// // <?xml version='1.1'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" version='1.1'", 0));
    /// assert!(decl.encoding().is_none());
    ///
    /// // <?xml encoding='utf-8'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" encoding='utf-8'", 0));
    /// match decl.encoding() {
    ///     Some(Ok(Cow::Borrowed(encoding))) => assert_eq!(encoding, b"utf-8"),
    ///     _ => assert!(false),
    /// }
    ///
    /// // <?xml encoding='something_WRONG' encoding='utf-8'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" encoding='something_WRONG' encoding='utf-8'", 0));
    /// match decl.encoding() {
    ///     Some(Ok(Cow::Borrowed(encoding))) => assert_eq!(encoding, b"something_WRONG"),
    ///     _ => assert!(false),
    /// }
    /// ```
    ///
    /// [grammar]: https://www.w3.org/TR/xml11/#NT-XMLDecl
    pub fn encoding(&self) -> Option<Result<Cow<[u8]>, AttrError>> {
        self.content
            .try_get_attribute("encoding")
            .map(|a| a.map(|a| a.value))
            .transpose()
    }

    /// Gets xml standalone, excluding quotes (`'` or `"`).
    ///
    /// Although according to the [grammar] standalone flag must appear after `"version"`
    /// and `"encoding"`, this method does not check that. The first occurrence of the
    /// attribute will be returned even if there are several. Also, method does not
    /// restrict symbols that can forming the value, so the returned flag name may not
    /// correspond to the grammar.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::borrow::Cow;
    /// use quick_xml::Error;
    /// use quick_xml::events::{BytesDecl, BytesStart};
    ///
    /// // <?xml version='1.1'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" version='1.1'", 0));
    /// assert!(decl.standalone().is_none());
    ///
    /// // <?xml standalone='yes'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" standalone='yes'", 0));
    /// match decl.standalone() {
    ///     Some(Ok(Cow::Borrowed(encoding))) => assert_eq!(encoding, b"yes"),
    ///     _ => assert!(false),
    /// }
    ///
    /// // <?xml standalone='something_WRONG' encoding='utf-8'?>
    /// let decl = BytesDecl::from_start(BytesStart::from_content(" standalone='something_WRONG' encoding='utf-8'", 0));
    /// match decl.standalone() {
    ///     Some(Ok(Cow::Borrowed(flag))) => assert_eq!(flag, b"something_WRONG"),
    ///     _ => assert!(false),
    /// }
    /// ```
    ///
    /// [grammar]: https://www.w3.org/TR/xml11/#NT-XMLDecl
    pub fn standalone(&self) -> Option<Result<Cow<[u8]>, AttrError>> {
        self.content
            .try_get_attribute("standalone")
            .map(|a| a.map(|a| a.value))
            .transpose()
    }

    /// Gets the actual encoding using [_get an encoding_](https://encoding.spec.whatwg.org/#concept-encoding-get)
    /// algorithm.
    ///
    /// If encoding in not known, or `encoding` key was not found, returns `None`.
    /// In case of duplicated `encoding` key, encoding, corresponding to the first
    /// one, is returned.
    #[cfg(feature = "encoding")]
    pub fn encoder(&self) -> Option<&'static Encoding> {
        self.encoding()
            .and_then(|e| e.ok())
            .and_then(|e| Encoding::for_label(&e))
    }

    /// Converts the event into an owned event.
    pub fn into_owned(self) -> BytesDecl<'static> {
        BytesDecl {
            content: self.content.into_owned(),
        }
    }

    /// Converts the event into a borrowed event.
    #[inline]
    pub fn borrow(&self) -> BytesDecl {
        BytesDecl {
            content: self.content.borrow(),
        }
    }
}

impl<'a> Deref for BytesDecl<'a> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.content
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for BytesDecl<'a> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self::new(
            <&str>::arbitrary(u)?,
            Option::<&str>::arbitrary(u)?,
            Option::<&str>::arbitrary(u)?,
        ))
    }

    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        return <&str as arbitrary::Arbitrary>::size_hint(depth);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Event emitted by [`Reader::read_event_into`].
///
/// [`Reader::read_event_into`]: crate::reader::Reader::read_event_into
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum Event<'a> {
    /// Start tag (with attributes) `<tag attr="value">`.
    Start(BytesStart<'a>),
    /// End tag `</tag>`.
    End(BytesEnd<'a>),
    /// Empty element tag (with attributes) `<tag attr="value" />`.
    Empty(BytesStart<'a>),
    /// Escaped character data between tags.
    Text(BytesText<'a>),
    /// Unescaped character data stored in `<![CDATA[...]]>`.
    CData(BytesCData<'a>),
    /// Comment `<!-- ... -->`.
    Comment(BytesText<'a>),
    /// XML declaration `<?xml ...?>`.
    Decl(BytesDecl<'a>),
    /// Processing instruction `<?...?>`.
    PI(BytesPI<'a>),
    /// Document type definition data (DTD) stored in `<!DOCTYPE ...>`.
    DocType(BytesText<'a>),
    /// End of XML document.
    Eof,
}

impl<'a> Event<'a> {
    /// Converts the event to an owned version, untied to the lifetime of
    /// buffer used when reading but incurring a new, separate allocation.
    pub fn into_owned(self) -> Event<'static> {
        match self {
            Event::Start(e) => Event::Start(e.into_owned()),
            Event::End(e) => Event::End(e.into_owned()),
            Event::Empty(e) => Event::Empty(e.into_owned()),
            Event::Text(e) => Event::Text(e.into_owned()),
            Event::Comment(e) => Event::Comment(e.into_owned()),
            Event::CData(e) => Event::CData(e.into_owned()),
            Event::Decl(e) => Event::Decl(e.into_owned()),
            Event::PI(e) => Event::PI(e.into_owned()),
            Event::DocType(e) => Event::DocType(e.into_owned()),
            Event::Eof => Event::Eof,
        }
    }

    /// Converts the event into a borrowed event.
    #[inline]
    pub fn borrow(&self) -> Event {
        match self {
            Event::Start(e) => Event::Start(e.borrow()),
            Event::End(e) => Event::End(e.borrow()),
            Event::Empty(e) => Event::Empty(e.borrow()),
            Event::Text(e) => Event::Text(e.borrow()),
            Event::Comment(e) => Event::Comment(e.borrow()),
            Event::CData(e) => Event::CData(e.borrow()),
            Event::Decl(e) => Event::Decl(e.borrow()),
            Event::PI(e) => Event::PI(e.borrow()),
            Event::DocType(e) => Event::DocType(e.borrow()),
            Event::Eof => Event::Eof,
        }
    }
}

impl<'a> Deref for Event<'a> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match *self {
            Event::Start(ref e) | Event::Empty(ref e) => e,
            Event::End(ref e) => e,
            Event::Text(ref e) => e,
            Event::Decl(ref e) => e,
            Event::PI(ref e) => e,
            Event::CData(ref e) => e,
            Event::Comment(ref e) => e,
            Event::DocType(ref e) => e,
            Event::Eof => &[],
        }
    }
}

impl<'a> AsRef<Event<'a>> for Event<'a> {
    fn as_ref(&self) -> &Event<'a> {
        self
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

#[inline]
fn str_cow_to_bytes<'a, C: Into<Cow<'a, str>>>(content: C) -> Cow<'a, [u8]> {
    match content.into() {
        Cow::Borrowed(s) => Cow::Borrowed(s.as_bytes()),
        Cow::Owned(s) => Cow::Owned(s.into_bytes()),
    }
}

fn trim_cow<'a, F>(value: Cow<'a, [u8]>, trim: F) -> Cow<'a, [u8]>
where
    F: FnOnce(&[u8]) -> &[u8],
{
    match value {
        Cow::Borrowed(bytes) => Cow::Borrowed(trim(bytes)),
        Cow::Owned(mut bytes) => {
            let trimmed = trim(&bytes);
            if trimmed.len() != bytes.len() {
                bytes = trimmed.to_vec();
            }
            Cow::Owned(bytes)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn bytestart_create() {
        let b = BytesStart::new("test");
        assert_eq!(b.len(), 4);
        assert_eq!(b.name(), QName(b"test"));
    }

    #[test]
    fn bytestart_set_name() {
        let mut b = BytesStart::new("test");
        assert_eq!(b.len(), 4);
        assert_eq!(b.name(), QName(b"test"));
        assert_eq!(b.attributes_raw(), b"");
        b.push_attribute(("x", "a"));
        assert_eq!(b.len(), 10);
        assert_eq!(b.attributes_raw(), b" x=\"a\"");
        b.set_name(b"g");
        assert_eq!(b.len(), 7);
        assert_eq!(b.name(), QName(b"g"));
    }

    #[test]
    fn bytestart_clear_attributes() {
        let mut b = BytesStart::new("test");
        b.push_attribute(("x", "y\"z"));
        b.push_attribute(("x", "y\"z"));
        b.clear_attributes();
        assert!(b.attributes().next().is_none());
        assert_eq!(b.len(), 4);
        assert_eq!(b.name(), QName(b"test"));
    }
}
