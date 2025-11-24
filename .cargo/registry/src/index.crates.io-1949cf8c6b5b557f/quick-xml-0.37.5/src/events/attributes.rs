//! Xml Attributes module
//!
//! Provides an iterator over attributes key/value pairs

use crate::encoding::Decoder;
use crate::errors::Result as XmlResult;
use crate::escape::{escape, resolve_predefined_entity, unescape_with};
use crate::name::{LocalName, Namespace, QName};
use crate::reader::NsReader;
use crate::utils::{is_whitespace, Bytes};

use std::fmt::{self, Debug, Display, Formatter};
use std::iter::FusedIterator;
use std::{borrow::Cow, ops::Range};

/// A struct representing a key/value XML attribute.
///
/// Field `value` stores raw bytes, possibly containing escape-sequences. Most users will likely
/// want to access the value using one of the [`unescape_value`] and [`decode_and_unescape_value`]
/// functions.
///
/// [`unescape_value`]: Self::unescape_value
/// [`decode_and_unescape_value`]: Self::decode_and_unescape_value
#[derive(Clone, Eq, PartialEq)]
pub struct Attribute<'a> {
    /// The key to uniquely define the attribute.
    ///
    /// If [`Attributes::with_checks`] is turned off, the key might not be unique.
    pub key: QName<'a>,
    /// The raw value of the attribute.
    pub value: Cow<'a, [u8]>,
}

impl<'a> Attribute<'a> {
    /// Decodes using UTF-8 then unescapes the value.
    ///
    /// This is normally the value you are interested in. Escape sequences such as `&gt;` are
    /// replaced with their unescaped equivalents such as `>`.
    ///
    /// This will allocate if the value contains any escape sequences.
    ///
    /// See also [`unescape_value_with()`](Self::unescape_value_with)
    ///
    /// This method is available only if [`encoding`] feature is **not** enabled.
    ///
    /// [`encoding`]: ../../index.html#encoding
    #[cfg(any(doc, not(feature = "encoding")))]
    pub fn unescape_value(&self) -> XmlResult<Cow<'a, str>> {
        self.unescape_value_with(resolve_predefined_entity)
    }

    /// Decodes using UTF-8 then unescapes the value, using custom entities.
    ///
    /// This is normally the value you are interested in. Escape sequences such as `&gt;` are
    /// replaced with their unescaped equivalents such as `>`.
    /// A fallback resolver for additional custom entities can be provided via
    /// `resolve_entity`.
    ///
    /// This will allocate if the value contains any escape sequences.
    ///
    /// See also [`unescape_value()`](Self::unescape_value)
    ///
    /// This method is available only if [`encoding`] feature is **not** enabled.
    ///
    /// [`encoding`]: ../../index.html#encoding
    #[cfg(any(doc, not(feature = "encoding")))]
    #[inline]
    pub fn unescape_value_with<'entity>(
        &self,
        resolve_entity: impl FnMut(&str) -> Option<&'entity str>,
    ) -> XmlResult<Cow<'a, str>> {
        self.decode_and_unescape_value_with(Decoder::utf8(), resolve_entity)
    }

    /// Decodes then unescapes the value.
    ///
    /// This will allocate if the value contains any escape sequences or in
    /// non-UTF-8 encoding.
    pub fn decode_and_unescape_value(&self, decoder: Decoder) -> XmlResult<Cow<'a, str>> {
        self.decode_and_unescape_value_with(decoder, resolve_predefined_entity)
    }

    /// Decodes then unescapes the value with custom entities.
    ///
    /// This will allocate if the value contains any escape sequences or in
    /// non-UTF-8 encoding.
    pub fn decode_and_unescape_value_with<'entity>(
        &self,
        decoder: Decoder,
        resolve_entity: impl FnMut(&str) -> Option<&'entity str>,
    ) -> XmlResult<Cow<'a, str>> {
        let decoded = decoder.decode_cow(&self.value)?;

        match unescape_with(&decoded, resolve_entity)? {
            // Because result is borrowed, no replacements was done and we can use original string
            Cow::Borrowed(_) => Ok(decoded),
            Cow::Owned(s) => Ok(s.into()),
        }
    }

    /// If attribute value [represents] valid boolean values, returns `Some`, otherwise returns `None`.
    ///
    /// The valid boolean representations are only `"true"`, `"false"`, `"1"`, and `"0"`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use quick_xml::events::attributes::Attribute;
    ///
    /// let attr = Attribute::from(("attr", "false"));
    /// assert_eq!(attr.as_bool(), Some(false));
    ///
    /// let attr = Attribute::from(("attr", "0"));
    /// assert_eq!(attr.as_bool(), Some(false));
    ///
    /// let attr = Attribute::from(("attr", "true"));
    /// assert_eq!(attr.as_bool(), Some(true));
    ///
    /// let attr = Attribute::from(("attr", "1"));
    /// assert_eq!(attr.as_bool(), Some(true));
    ///
    /// let attr = Attribute::from(("attr", "bot bool"));
    /// assert_eq!(attr.as_bool(), None);
    /// ```
    ///
    /// [represents]: https://www.w3.org/TR/xmlschema11-2/#boolean
    #[inline]
    pub fn as_bool(&self) -> Option<bool> {
        match self.value.as_ref() {
            b"1" | b"true" => Some(true),
            b"0" | b"false" => Some(false),
            _ => None,
        }
    }
}

impl<'a> Debug for Attribute<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Attribute")
            .field("key", &Bytes(self.key.as_ref()))
            .field("value", &Bytes(&self.value))
            .finish()
    }
}

impl<'a> From<(&'a [u8], &'a [u8])> for Attribute<'a> {
    /// Creates new attribute from raw bytes.
    /// Does not apply any transformation to both key and value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use quick_xml::events::attributes::Attribute;
    ///
    /// let features = Attribute::from(("features".as_bytes(), "Bells &amp; whistles".as_bytes()));
    /// assert_eq!(features.value, "Bells &amp; whistles".as_bytes());
    /// ```
    fn from(val: (&'a [u8], &'a [u8])) -> Attribute<'a> {
        Attribute {
            key: QName(val.0),
            value: Cow::from(val.1),
        }
    }
}

impl<'a> From<(&'a str, &'a str)> for Attribute<'a> {
    /// Creates new attribute from text representation.
    /// Key is stored as-is, but the value will be escaped.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use quick_xml::events::attributes::Attribute;
    ///
    /// let features = Attribute::from(("features", "Bells & whistles"));
    /// assert_eq!(features.value, "Bells &amp; whistles".as_bytes());
    /// ```
    fn from(val: (&'a str, &'a str)) -> Attribute<'a> {
        Attribute {
            key: QName(val.0.as_bytes()),
            value: match escape(val.1) {
                Cow::Borrowed(s) => Cow::Borrowed(s.as_bytes()),
                Cow::Owned(s) => Cow::Owned(s.into_bytes()),
            },
        }
    }
}

impl<'a> From<(&'a str, Cow<'a, str>)> for Attribute<'a> {
    /// Creates new attribute from text representation.
    /// Key is stored as-is, but the value will be escaped.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::borrow::Cow;
    /// use pretty_assertions::assert_eq;
    /// use quick_xml::events::attributes::Attribute;
    ///
    /// let features = Attribute::from(("features", Cow::Borrowed("Bells & whistles")));
    /// assert_eq!(features.value, "Bells &amp; whistles".as_bytes());
    /// ```
    fn from(val: (&'a str, Cow<'a, str>)) -> Attribute<'a> {
        Attribute {
            key: QName(val.0.as_bytes()),
            value: match escape(val.1) {
                Cow::Borrowed(s) => Cow::Borrowed(s.as_bytes()),
                Cow::Owned(s) => Cow::Owned(s.into_bytes()),
            },
        }
    }
}

impl<'a> From<Attr<&'a [u8]>> for Attribute<'a> {
    #[inline]
    fn from(attr: Attr<&'a [u8]>) -> Self {
        Self {
            key: attr.key(),
            value: Cow::Borrowed(attr.value()),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Iterator over XML attributes.
///
/// Yields `Result<Attribute>`. An `Err` will be yielded if an attribute is malformed or duplicated.
/// The duplicate check can be turned off by calling [`with_checks(false)`].
///
/// [`with_checks(false)`]: Self::with_checks
#[derive(Clone)]
pub struct Attributes<'a> {
    /// Slice of `BytesStart` corresponding to attributes
    bytes: &'a [u8],
    /// Iterator state, independent from the actual source of bytes
    state: IterState,
}

impl<'a> Attributes<'a> {
    /// Internal constructor, used by `BytesStart`. Supplies data in reader's encoding
    #[inline]
    pub(crate) const fn wrap(buf: &'a [u8], pos: usize, html: bool) -> Self {
        Self {
            bytes: buf,
            state: IterState::new(pos, html),
        }
    }

    /// Creates a new attribute iterator from a buffer.
    pub const fn new(buf: &'a str, pos: usize) -> Self {
        Self::wrap(buf.as_bytes(), pos, false)
    }

    /// Creates a new attribute iterator from a buffer, allowing HTML attribute syntax.
    pub const fn html(buf: &'a str, pos: usize) -> Self {
        Self::wrap(buf.as_bytes(), pos, true)
    }

    /// Changes whether attributes should be checked for uniqueness.
    ///
    /// The XML specification requires attribute keys in the same element to be unique. This check
    /// can be disabled to improve performance slightly.
    ///
    /// (`true` by default)
    pub fn with_checks(&mut self, val: bool) -> &mut Attributes<'a> {
        self.state.check_duplicates = val;
        self
    }

    /// Checks if the current tag has a [`xsi:nil`] attribute. This method ignores any errors in
    /// attributes.
    ///
    /// # Examples
    ///
    /// ```
    /// # use pretty_assertions::assert_eq;
    /// use quick_xml::events::Event;
    /// use quick_xml::name::QName;
    /// use quick_xml::reader::NsReader;
    ///
    /// let mut reader = NsReader::from_str("
    ///     <root xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance'>
    ///         <true xsi:nil='true'/>
    ///         <false xsi:nil='false'/>
    ///         <none/>
    ///         <non-xsi xsi:nil='true' xmlns:xsi='namespace'/>
    ///         <unbound-nil nil='true' xmlns='http://www.w3.org/2001/XMLSchema-instance'/>
    ///         <another-xmlns f:nil='true' xmlns:f='http://www.w3.org/2001/XMLSchema-instance'/>
    ///     </root>
    /// ");
    /// reader.config_mut().trim_text(true);
    ///
    /// macro_rules! check {
    ///     ($reader:expr, $name:literal, $value:literal) => {
    ///         let event = match $reader.read_event().unwrap() {
    ///             Event::Empty(e) => e,
    ///             e => panic!("Unexpected event {:?}", e),
    ///         };
    ///         assert_eq!(
    ///             (event.name(), event.attributes().has_nil(&$reader)),
    ///             (QName($name.as_bytes()), $value),
    ///         );
    ///     };
    /// }
    ///
    /// let root = match reader.read_event().unwrap() {
    ///     Event::Start(e) => e,
    ///     e => panic!("Unexpected event {:?}", e),
    /// };
    /// assert_eq!(root.attributes().has_nil(&reader), false);
    ///
    /// // definitely true
    /// check!(reader, "true",          true);
    /// // definitely false
    /// check!(reader, "false",         false);
    /// // absence of the attribute means that attribute is not set
    /// check!(reader, "none",          false);
    /// // attribute not bound to the correct namespace
    /// check!(reader, "non-xsi",       false);
    /// // attributes without prefix not bound to any namespace
    /// check!(reader, "unbound-nil",   false);
    /// // prefix can be any while it is bound to the correct namespace
    /// check!(reader, "another-xmlns", true);
    /// ```
    ///
    /// [`xsi:nil`]: https://www.w3.org/TR/xmlschema-1/#xsi_nil
    pub fn has_nil<R>(&mut self, reader: &NsReader<R>) -> bool {
        use crate::name::ResolveResult::*;

        self.any(|attr| {
            if let Ok(attr) = attr {
                match reader.resolve_attribute(attr.key) {
                    (
                        Bound(Namespace(b"http://www.w3.org/2001/XMLSchema-instance")),
                        LocalName(b"nil"),
                    ) => attr.as_bool().unwrap_or_default(),
                    _ => false,
                }
            } else {
                false
            }
        })
    }
}

impl<'a> Debug for Attributes<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Attributes")
            .field("bytes", &Bytes(&self.bytes))
            .field("state", &self.state)
            .finish()
    }
}

impl<'a> Iterator for Attributes<'a> {
    type Item = Result<Attribute<'a>, AttrError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.state.next(self.bytes) {
            None => None,
            Some(Ok(a)) => Some(Ok(a.map(|range| &self.bytes[range]).into())),
            Some(Err(e)) => Some(Err(e)),
        }
    }
}

impl<'a> FusedIterator for Attributes<'a> {}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Errors that can be raised during parsing attributes.
///
/// Recovery position in examples shows the position from which parsing of the
/// next attribute will be attempted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttrError {
    /// Attribute key was not followed by `=`, position relative to the start of
    /// the owning tag is provided.
    ///
    /// Example of input that raises this error:
    ///
    /// ```xml
    /// <tag key another="attribute"/>
    /// <!--     ^~~ error position, recovery position (8) -->
    /// ```
    ///
    /// This error can be raised only when the iterator is in XML mode.
    ExpectedEq(usize),
    /// Attribute value was not found after `=`, position relative to the start
    /// of the owning tag is provided.
    ///
    /// Example of input that raises this error:
    ///
    /// ```xml
    /// <tag key = />
    /// <!--       ^~~ error position, recovery position (10) -->
    /// ```
    ///
    /// This error can be returned only for the last attribute in the list,
    /// because otherwise any content after `=` will be threated as a value.
    /// The XML
    ///
    /// ```xml
    /// <tag key = another-key = "value"/>
    /// <!--                   ^ ^- recovery position (24) -->
    /// <!--                   '~~ error position (22) -->
    /// ```
    ///
    /// will be treated as `Attribute { key = b"key", value = b"another-key" }`
    /// and or [`Attribute`] is returned, or [`AttrError::UnquotedValue`] is raised,
    /// depending on the parsing mode.
    ExpectedValue(usize),
    /// Attribute value is not quoted, position relative to the start of the
    /// owning tag is provided.
    ///
    /// Example of input that raises this error:
    ///
    /// ```xml
    /// <tag key = value />
    /// <!--       ^    ^~~ recovery position (15) -->
    /// <!--       '~~ error position (10) -->
    /// ```
    ///
    /// This error can be raised only when the iterator is in XML mode.
    UnquotedValue(usize),
    /// Attribute value was not finished with a matching quote, position relative
    /// to the start of owning tag and a quote is provided. That position is always
    /// a last character in the tag content.
    ///
    /// Example of input that raises this error:
    ///
    /// ```xml
    /// <tag key = "value  />
    /// <tag key = 'value  />
    /// <!--               ^~~ error position, recovery position (18) -->
    /// ```
    ///
    /// This error can be returned only for the last attribute in the list,
    /// because all input was consumed during scanning for a quote.
    ExpectedQuote(usize, u8),
    /// An attribute with the same name was already encountered. Two parameters
    /// define (1) the error position relative to the start of the owning tag
    /// for a new attribute and (2) the start position of a previously encountered
    /// attribute with the same name.
    ///
    /// Example of input that raises this error:
    ///
    /// ```xml
    /// <tag key = 'value'  key="value2" attr3='value3' />
    /// <!-- ^              ^            ^~~ recovery position (32) -->
    /// <!-- |              '~~ error position (19) -->
    /// <!-- '~~ previous position (4) -->
    /// ```
    ///
    /// This error is returned only when [`Attributes::with_checks()`] is set
    /// to `true` (that is default behavior).
    Duplicated(usize, usize),
}

impl Display for AttrError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::ExpectedEq(pos) => write!(
                f,
                r#"position {}: attribute key must be directly followed by `=` or space"#,
                pos
            ),
            Self::ExpectedValue(pos) => write!(
                f,
                r#"position {}: `=` must be followed by an attribute value"#,
                pos
            ),
            Self::UnquotedValue(pos) => write!(
                f,
                r#"position {}: attribute value must be enclosed in `"` or `'`"#,
                pos
            ),
            Self::ExpectedQuote(pos, quote) => write!(
                f,
                r#"position {}: missing closing quote `{}` in attribute value"#,
                pos, *quote as char
            ),
            Self::Duplicated(pos1, pos2) => write!(
                f,
                r#"position {}: duplicated attribute, previous declaration at position {}"#,
                pos1, pos2
            ),
        }
    }
}

impl std::error::Error for AttrError {}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A struct representing a key/value XML or HTML [attribute].
///
/// [attribute]: https://www.w3.org/TR/xml11/#NT-Attribute
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Attr<T> {
    /// Attribute with value enclosed in double quotes (`"`). Attribute key and
    /// value provided. This is a canonical XML-style attribute.
    DoubleQ(T, T),
    /// Attribute with value enclosed in single quotes (`'`). Attribute key and
    /// value provided. This is an XML-style attribute.
    SingleQ(T, T),
    /// Attribute with value not enclosed in quotes. Attribute key and value
    /// provided. This is HTML-style attribute, it can be returned in HTML-mode
    /// parsing only. In an XML mode [`AttrError::UnquotedValue`] will be raised
    /// instead.
    ///
    /// Attribute value can be invalid according to the [HTML specification],
    /// in particular, it can contain `"`, `'`, `=`, `<`, and <code>&#96;</code>
    /// characters. The absence of the `>` character is nevertheless guaranteed,
    /// since the parser extracts [events] based on them even before the start
    /// of parsing attributes.
    ///
    /// [HTML specification]: https://html.spec.whatwg.org/#unquoted
    /// [events]: crate::events::Event::Start
    Unquoted(T, T),
    /// Attribute without value. Attribute key provided. This is HTML-style attribute,
    /// it can be returned in HTML-mode parsing only. In XML mode
    /// [`AttrError::ExpectedEq`] will be raised instead.
    Empty(T),
}

impl<T> Attr<T> {
    /// Maps an `Attr<T>` to `Attr<U>` by applying a function to a contained key and value.
    #[inline]
    pub fn map<U, F>(self, mut f: F) -> Attr<U>
    where
        F: FnMut(T) -> U,
    {
        match self {
            Attr::DoubleQ(key, value) => Attr::DoubleQ(f(key), f(value)),
            Attr::SingleQ(key, value) => Attr::SingleQ(f(key), f(value)),
            Attr::Empty(key) => Attr::Empty(f(key)),
            Attr::Unquoted(key, value) => Attr::Unquoted(f(key), f(value)),
        }
    }
}

impl<'a> Attr<&'a [u8]> {
    /// Returns the key value
    #[inline]
    pub const fn key(&self) -> QName<'a> {
        QName(match self {
            Attr::DoubleQ(key, _) => key,
            Attr::SingleQ(key, _) => key,
            Attr::Empty(key) => key,
            Attr::Unquoted(key, _) => key,
        })
    }
    /// Returns the attribute value. For [`Self::Empty`] variant an empty slice
    /// is returned according to the [HTML specification].
    ///
    /// [HTML specification]: https://www.w3.org/TR/2012/WD-html-markup-20120329/syntax.html#syntax-attr-empty
    #[inline]
    pub const fn value(&self) -> &'a [u8] {
        match self {
            Attr::DoubleQ(_, value) => value,
            Attr::SingleQ(_, value) => value,
            Attr::Empty(_) => &[],
            Attr::Unquoted(_, value) => value,
        }
    }
}

impl<T: AsRef<[u8]>> Debug for Attr<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Attr::DoubleQ(key, value) => f
                .debug_tuple("Attr::DoubleQ")
                .field(&Bytes(key.as_ref()))
                .field(&Bytes(value.as_ref()))
                .finish(),
            Attr::SingleQ(key, value) => f
                .debug_tuple("Attr::SingleQ")
                .field(&Bytes(key.as_ref()))
                .field(&Bytes(value.as_ref()))
                .finish(),
            Attr::Empty(key) => f
                .debug_tuple("Attr::Empty")
                // Comment to prevent formatting and keep style consistent
                .field(&Bytes(key.as_ref()))
                .finish(),
            Attr::Unquoted(key, value) => f
                .debug_tuple("Attr::Unquoted")
                .field(&Bytes(key.as_ref()))
                .field(&Bytes(value.as_ref()))
                .finish(),
        }
    }
}

/// Unpacks attribute key and value into tuple of this two elements.
/// `None` value element is returned only for [`Attr::Empty`] variant.
impl<T> From<Attr<T>> for (T, Option<T>) {
    #[inline]
    fn from(attr: Attr<T>) -> Self {
        match attr {
            Attr::DoubleQ(key, value) => (key, Some(value)),
            Attr::SingleQ(key, value) => (key, Some(value)),
            Attr::Empty(key) => (key, None),
            Attr::Unquoted(key, value) => (key, Some(value)),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

type AttrResult = Result<Attr<Range<usize>>, AttrError>;

#[derive(Clone, Copy, Debug)]
enum State {
    /// Iteration finished, iterator will return `None` to all [`IterState::next`]
    /// requests.
    Done,
    /// The last attribute returned was deserialized successfully. Contains an
    /// offset from which next attribute should be searched.
    Next(usize),
    /// The last attribute returns [`AttrError::UnquotedValue`], offset pointed
    /// to the beginning of the value. Recover should skip a value
    SkipValue(usize),
    /// The last attribute returns [`AttrError::Duplicated`], offset pointed to
    /// the equal (`=`) sign. Recover should skip it and a value
    SkipEqValue(usize),
}

/// External iterator over spans of attribute key and value
#[derive(Clone, Debug)]
pub(crate) struct IterState {
    /// Iteration state that determines what actions should be done before the
    /// actual parsing of the next attribute
    state: State,
    /// If `true`, enables ability to parse unquoted values and key-only (empty)
    /// attributes
    html: bool,
    /// If `true`, checks for duplicate names
    check_duplicates: bool,
    /// If `check_duplicates` is set, contains the ranges of already parsed attribute
    /// names. We store a ranges instead of slices to able to report a previous
    /// attribute position
    keys: Vec<Range<usize>>,
}

impl IterState {
    pub const fn new(offset: usize, html: bool) -> Self {
        Self {
            state: State::Next(offset),
            html,
            check_duplicates: true,
            keys: Vec::new(),
        }
    }

    /// Recover from an error that could have been made on a previous step.
    /// Returns an offset from which parsing should continue.
    /// If there no input left, returns `None`.
    fn recover(&self, slice: &[u8]) -> Option<usize> {
        match self.state {
            State::Done => None,
            State::Next(offset) => Some(offset),
            State::SkipValue(offset) => self.skip_value(slice, offset),
            State::SkipEqValue(offset) => self.skip_eq_value(slice, offset),
        }
    }

    /// Skip all characters up to first space symbol or end-of-input
    #[inline]
    #[allow(clippy::manual_map)]
    fn skip_value(&self, slice: &[u8], offset: usize) -> Option<usize> {
        let mut iter = (offset..).zip(slice[offset..].iter());

        match iter.find(|(_, &b)| is_whitespace(b)) {
            // Input: `    key  =  value `
            //                     |    ^
            //                offset    e
            Some((e, _)) => Some(e),
            // Input: `    key  =  value`
            //                     |    ^
            //                offset    e = len()
            None => None,
        }
    }

    /// Skip all characters up to first space symbol or end-of-input
    #[inline]
    fn skip_eq_value(&self, slice: &[u8], offset: usize) -> Option<usize> {
        let mut iter = (offset..).zip(slice[offset..].iter());

        // Skip all up to the quote and get the quote type
        let quote = match iter.find(|(_, &b)| !is_whitespace(b)) {
            // Input: `    key  =  "`
            //                  |  ^
            //             offset
            Some((_, b'"')) => b'"',
            // Input: `    key  =  '`
            //                  |  ^
            //             offset
            Some((_, b'\'')) => b'\'',

            // Input: `    key  =  x`
            //                  |  ^
            //             offset
            Some((offset, _)) => return self.skip_value(slice, offset),
            // Input: `    key  =  `
            //                  |  ^
            //             offset
            None => return None,
        };

        match iter.find(|(_, &b)| b == quote) {
            // Input: `    key  =  "   "`
            //                         ^
            Some((e, b'"')) => Some(e),
            // Input: `    key  =  '   '`
            //                         ^
            Some((e, _)) => Some(e),

            // Input: `    key  =  "   `
            // Input: `    key  =  '   `
            //                         ^
            // Closing quote not found
            None => None,
        }
    }

    #[inline]
    fn check_for_duplicates(
        &mut self,
        slice: &[u8],
        key: Range<usize>,
    ) -> Result<Range<usize>, AttrError> {
        if self.check_duplicates {
            if let Some(prev) = self
                .keys
                .iter()
                .find(|r| slice[(*r).clone()] == slice[key.clone()])
            {
                return Err(AttrError::Duplicated(key.start, prev.start));
            }
            self.keys.push(key.clone());
        }
        Ok(key)
    }

    /// # Parameters
    ///
    /// - `slice`: content of the tag, used for checking for duplicates
    /// - `key`: Range of key in slice, if iterator in HTML mode
    /// - `offset`: Position of error if iterator in XML mode
    #[inline]
    fn key_only(&mut self, slice: &[u8], key: Range<usize>, offset: usize) -> Option<AttrResult> {
        Some(if self.html {
            self.check_for_duplicates(slice, key).map(Attr::Empty)
        } else {
            Err(AttrError::ExpectedEq(offset))
        })
    }

    #[inline]
    fn double_q(&mut self, key: Range<usize>, value: Range<usize>) -> Option<AttrResult> {
        self.state = State::Next(value.end + 1); // +1 for `"`

        Some(Ok(Attr::DoubleQ(key, value)))
    }

    #[inline]
    fn single_q(&mut self, key: Range<usize>, value: Range<usize>) -> Option<AttrResult> {
        self.state = State::Next(value.end + 1); // +1 for `'`

        Some(Ok(Attr::SingleQ(key, value)))
    }

    pub fn next(&mut self, slice: &[u8]) -> Option<AttrResult> {
        let mut iter = match self.recover(slice) {
            Some(offset) => (offset..).zip(slice[offset..].iter()),
            None => return None,
        };

        // Index where next key started
        let start_key = match iter.find(|(_, &b)| !is_whitespace(b)) {
            // Input: `    key`
            //             ^
            Some((s, _)) => s,
            // Input: `    `
            //             ^
            None => {
                // Because we reach end-of-input, stop iteration on next call
                self.state = State::Done;
                return None;
            }
        };
        // Span of a key
        let (key, offset) = match iter.find(|(_, &b)| b == b'=' || is_whitespace(b)) {
            // Input: `    key=`
            //             |  ^
            //             s  e
            Some((e, b'=')) => (start_key..e, e),

            // Input: `    key `
            //                ^
            Some((e, _)) => match iter.find(|(_, &b)| !is_whitespace(b)) {
                // Input: `    key  =`
                //             |  | ^
                //     start_key  e
                Some((offset, b'=')) => (start_key..e, offset),
                // Input: `    key  x`
                //             |  | ^
                //     start_key  e
                // If HTML-like attributes is allowed, this is the result, otherwise error
                Some((offset, _)) => {
                    // In any case, recovering is not required
                    self.state = State::Next(offset);
                    return self.key_only(slice, start_key..e, offset);
                }
                // Input: `    key  `
                //             |  | ^
                //     start_key  e
                // If HTML-like attributes is allowed, this is the result, otherwise error
                None => {
                    // Because we reach end-of-input, stop iteration on next call
                    self.state = State::Done;
                    return self.key_only(slice, start_key..e, slice.len());
                }
            },

            // Input: `    key`
            //             |  ^
            //             s  e = len()
            // If HTML-like attributes is allowed, this is the result, otherwise error
            None => {
                // Because we reach end-of-input, stop iteration on next call
                self.state = State::Done;
                let e = slice.len();
                return self.key_only(slice, start_key..e, e);
            }
        };

        let key = match self.check_for_duplicates(slice, key) {
            Err(e) => {
                self.state = State::SkipEqValue(offset);
                return Some(Err(e));
            }
            Ok(key) => key,
        };

        ////////////////////////////////////////////////////////////////////////

        // Gets the position of quote and quote type
        let (start_value, quote) = match iter.find(|(_, &b)| !is_whitespace(b)) {
            // Input: `    key  =  "`
            //                     ^
            Some((s, b'"')) => (s + 1, b'"'),
            // Input: `    key  =  '`
            //                     ^
            Some((s, b'\'')) => (s + 1, b'\''),

            // Input: `    key  =  x`
            //                     ^
            // If HTML-like attributes is allowed, this is the start of the value
            Some((s, _)) if self.html => {
                // We do not check validity of attribute value characters as required
                // according to https://html.spec.whatwg.org/#unquoted. It can be done
                // during validation phase
                let end = match iter.find(|(_, &b)| is_whitespace(b)) {
                    // Input: `    key  =  value `
                    //                     |    ^
                    //                     s    e
                    Some((e, _)) => e,
                    // Input: `    key  =  value`
                    //                     |    ^
                    //                     s    e = len()
                    None => slice.len(),
                };
                self.state = State::Next(end);
                return Some(Ok(Attr::Unquoted(key, s..end)));
            }
            // Input: `    key  =  x`
            //                     ^
            Some((s, _)) => {
                self.state = State::SkipValue(s);
                return Some(Err(AttrError::UnquotedValue(s)));
            }

            // Input: `    key  =  `
            //                     ^
            None => {
                // Because we reach end-of-input, stop iteration on next call
                self.state = State::Done;
                return Some(Err(AttrError::ExpectedValue(slice.len())));
            }
        };

        match iter.find(|(_, &b)| b == quote) {
            // Input: `    key  =  "   "`
            //                         ^
            Some((e, b'"')) => self.double_q(key, start_value..e),
            // Input: `    key  =  '   '`
            //                         ^
            Some((e, _)) => self.single_q(key, start_value..e),

            // Input: `    key  =  "   `
            // Input: `    key  =  '   `
            //                         ^
            // Closing quote not found
            None => {
                // Because we reach end-of-input, stop iteration on next call
                self.state = State::Done;
                Some(Err(AttrError::ExpectedQuote(slice.len(), quote)))
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// Checks, how parsing of XML-style attributes works. Each attribute should
/// have a value, enclosed in single or double quotes.
#[cfg(test)]
mod xml {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Checked attribute is the single attribute
    mod single {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Attribute have a value enclosed in single quotes
        #[test]
        fn single_quoted() {
            let mut iter = Attributes::new(r#"tag key='value'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value enclosed in double quotes
        #[test]
        fn double_quoted() {
            let mut iter = Attributes::new(r#"tag key="value""#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value, not enclosed in quotes
        #[test]
        fn unquoted() {
            let mut iter = Attributes::new(r#"tag key=value"#, 3);
            //                                0       ^ = 8

            assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(8))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Only attribute key is present
        #[test]
        fn key_only() {
            let mut iter = Attributes::new(r#"tag key"#, 3);
            //                                0      ^ = 7

            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(7))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key is started with an invalid symbol (a single quote in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_start_invalid() {
            let mut iter = Attributes::new(r#"tag 'key'='value'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"'key'"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key contains an invalid symbol (an ampersand in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_contains_invalid() {
            let mut iter = Attributes::new(r#"tag key&jey='value'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key&jey"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute value is missing after `=`
        #[test]
        fn missed_value() {
            let mut iter = Attributes::new(r#"tag key="#, 3);
            //                                0       ^ = 8

            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedValue(8))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }
    }

    /// Checked attribute is the first attribute in the list of many attributes
    mod first {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Attribute have a value enclosed in single quotes
        #[test]
        fn single_quoted() {
            let mut iter = Attributes::new(r#"tag key='value' regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"regular"),
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value enclosed in double quotes
        #[test]
        fn double_quoted() {
            let mut iter = Attributes::new(r#"tag key="value" regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"regular"),
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value, not enclosed in quotes
        #[test]
        fn unquoted() {
            let mut iter = Attributes::new(r#"tag key=value regular='attribute'"#, 3);
            //                                0       ^ = 8

            assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(8))));
            // check error recovery
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"regular"),
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Only attribute key is present
        #[test]
        fn key_only() {
            let mut iter = Attributes::new(r#"tag key regular='attribute'"#, 3);
            //                                0       ^ = 8

            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(8))));
            // check error recovery
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"regular"),
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key is started with an invalid symbol (a single quote in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_start_invalid() {
            let mut iter = Attributes::new(r#"tag 'key'='value' regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"'key'"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"regular"),
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key contains an invalid symbol (an ampersand in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_contains_invalid() {
            let mut iter = Attributes::new(r#"tag key&jey='value' regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key&jey"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"regular"),
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute value is missing after `=`.
        #[test]
        fn missed_value() {
            let mut iter = Attributes::new(r#"tag key= regular='attribute'"#, 3);
            //                                0        ^ = 9

            assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(9))));
            // Because we do not check validity of keys and values during parsing,
            // "error='recovery'" is considered, as unquoted attribute value and
            // skipped during recovery and iteration finished
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);

            ////////////////////////////////////////////////////////////////////

            let mut iter = Attributes::new(r#"tag key= regular= 'attribute'"#, 3);
            //                                0        ^ = 9               ^ = 29

            // In that case "regular=" considered as unquoted value
            assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(9))));
            // In that case "'attribute'" considered as a key, because we do not check
            // validity of key names
            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(29))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);

            ////////////////////////////////////////////////////////////////////

            let mut iter = Attributes::new(r#"tag key= regular ='attribute'"#, 3);
            //                                0        ^ = 9               ^ = 29

            // In that case "regular" considered as unquoted value
            assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(9))));
            // In that case "='attribute'" considered as a key, because we do not check
            // validity of key names
            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(29))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);

            ////////////////////////////////////////////////////////////////////

            let mut iter = Attributes::new(r#"tag key= regular = 'attribute'"#, 3);
            //                                0        ^ = 9     ^ = 19     ^ = 30

            assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(9))));
            // In that case second "=" considered as a key, because we do not check
            // validity of key names
            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(19))));
            // In that case "'attribute'" considered as a key, because we do not check
            // validity of key names
            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(30))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }
    }

    /// Copy of single, but with additional spaces in markup
    mod sparsed {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Attribute have a value enclosed in single quotes
        #[test]
        fn single_quoted() {
            let mut iter = Attributes::new(r#"tag key = 'value' "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value enclosed in double quotes
        #[test]
        fn double_quoted() {
            let mut iter = Attributes::new(r#"tag key = "value" "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value, not enclosed in quotes
        #[test]
        fn unquoted() {
            let mut iter = Attributes::new(r#"tag key = value "#, 3);
            //                                0         ^ = 10

            assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(10))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Only attribute key is present
        #[test]
        fn key_only() {
            let mut iter = Attributes::new(r#"tag key "#, 3);
            //                                0       ^ = 8

            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(8))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key is started with an invalid symbol (a single quote in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_start_invalid() {
            let mut iter = Attributes::new(r#"tag 'key' = 'value' "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"'key'"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key contains an invalid symbol (an ampersand in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_contains_invalid() {
            let mut iter = Attributes::new(r#"tag key&jey = 'value' "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key&jey"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute value is missing after `=`
        #[test]
        fn missed_value() {
            let mut iter = Attributes::new(r#"tag key = "#, 3);
            //                                0         ^ = 10

            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedValue(10))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }
    }

    /// Checks that duplicated attributes correctly reported and recovering is
    /// possible after that
    mod duplicated {
        use super::*;

        mod with_check {
            use super::*;
            use pretty_assertions::assert_eq;

            /// Attribute have a value enclosed in single quotes
            #[test]
            fn single_quoted() {
                let mut iter = Attributes::new(r#"tag key='value' key='dup' another=''"#, 3);
                //                                0   ^ = 4       ^ = 16

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::Duplicated(16, 4))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value enclosed in double quotes
            #[test]
            fn double_quoted() {
                let mut iter = Attributes::new(r#"tag key='value' key="dup" another=''"#, 3);
                //                                0   ^ = 4       ^ = 16

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::Duplicated(16, 4))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value, not enclosed in quotes
            #[test]
            fn unquoted() {
                let mut iter = Attributes::new(r#"tag key='value' key=dup another=''"#, 3);
                //                                0   ^ = 4       ^ = 16

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::Duplicated(16, 4))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Only attribute key is present
            #[test]
            fn key_only() {
                let mut iter = Attributes::new(r#"tag key='value' key another=''"#, 3);
                //                                0                   ^ = 20

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(20))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }
        }

        /// Check for duplicated names is disabled
        mod without_check {
            use super::*;
            use pretty_assertions::assert_eq;

            /// Attribute have a value enclosed in single quotes
            #[test]
            fn single_quoted() {
                let mut iter = Attributes::new(r#"tag key='value' key='dup' another=''"#, 3);
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"dup"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value enclosed in double quotes
            #[test]
            fn double_quoted() {
                let mut iter = Attributes::new(r#"tag key='value' key="dup" another=''"#, 3);
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"dup"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value, not enclosed in quotes
            #[test]
            fn unquoted() {
                let mut iter = Attributes::new(r#"tag key='value' key=dup another=''"#, 3);
                //                                0                   ^ = 20
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::UnquotedValue(20))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Only attribute key is present
            #[test]
            fn key_only() {
                let mut iter = Attributes::new(r#"tag key='value' key another=''"#, 3);
                //                                0                   ^ = 20
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::ExpectedEq(20))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }
        }
    }

    #[test]
    fn mixed_quote() {
        let mut iter = Attributes::new(r#"tag a='a' b = "b" c='cc"cc' d="dd'dd""#, 3);

        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: QName(b"a"),
                value: Cow::Borrowed(b"a"),
            }))
        );
        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: QName(b"b"),
                value: Cow::Borrowed(b"b"),
            }))
        );
        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: QName(b"c"),
                value: Cow::Borrowed(br#"cc"cc"#),
            }))
        );
        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: QName(b"d"),
                value: Cow::Borrowed(b"dd'dd"),
            }))
        );
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }
}

/// Checks, how parsing of HTML-style attributes works. Each attribute can be
/// in three forms:
/// - XML-like: have a value, enclosed in single or double quotes
/// - have a value, do not enclosed in quotes
/// - without value, key only
#[cfg(test)]
mod html {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Checked attribute is the single attribute
    mod single {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Attribute have a value enclosed in single quotes
        #[test]
        fn single_quoted() {
            let mut iter = Attributes::html(r#"tag key='value'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value enclosed in double quotes
        #[test]
        fn double_quoted() {
            let mut iter = Attributes::html(r#"tag key="value""#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value, not enclosed in quotes
        #[test]
        fn unquoted() {
            let mut iter = Attributes::html(r#"tag key=value"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Only attribute key is present
        #[test]
        fn key_only() {
            let mut iter = Attributes::html(r#"tag key"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(&[]),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key is started with an invalid symbol (a single quote in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_start_invalid() {
            let mut iter = Attributes::html(r#"tag 'key'='value'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"'key'"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key contains an invalid symbol (an ampersand in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_contains_invalid() {
            let mut iter = Attributes::html(r#"tag key&jey='value'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key&jey"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute value is missing after `=`
        #[test]
        fn missed_value() {
            let mut iter = Attributes::html(r#"tag key="#, 3);
            //                                0       ^ = 8

            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedValue(8))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }
    }

    /// Checked attribute is the first attribute in the list of many attributes
    mod first {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Attribute have a value enclosed in single quotes
        #[test]
        fn single_quoted() {
            let mut iter = Attributes::html(r#"tag key='value' regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"regular"),
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value enclosed in double quotes
        #[test]
        fn double_quoted() {
            let mut iter = Attributes::html(r#"tag key="value" regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"regular"),
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value, not enclosed in quotes
        #[test]
        fn unquoted() {
            let mut iter = Attributes::html(r#"tag key=value regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"regular"),
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Only attribute key is present
        #[test]
        fn key_only() {
            let mut iter = Attributes::html(r#"tag key regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(&[]),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"regular"),
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key is started with an invalid symbol (a single quote in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_start_invalid() {
            let mut iter = Attributes::html(r#"tag 'key'='value' regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"'key'"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"regular"),
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key contains an invalid symbol (an ampersand in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_contains_invalid() {
            let mut iter = Attributes::html(r#"tag key&jey='value' regular='attribute'"#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key&jey"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"regular"),
                    value: Cow::Borrowed(b"attribute"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute value is missing after `=`
        #[test]
        fn missed_value() {
            let mut iter = Attributes::html(r#"tag key= regular='attribute'"#, 3);

            // Because we do not check validity of keys and values during parsing,
            // "regular='attribute'" is considered as unquoted attribute value
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"regular='attribute'"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);

            ////////////////////////////////////////////////////////////////////

            let mut iter = Attributes::html(r#"tag key= regular= 'attribute'"#, 3);

            // Because we do not check validity of keys and values during parsing,
            // "regular=" is considered as unquoted attribute value
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"regular="),
                }))
            );
            // Because we do not check validity of keys and values during parsing,
            // "'attribute'" is considered as key-only attribute
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"'attribute'"),
                    value: Cow::Borrowed(&[]),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);

            ////////////////////////////////////////////////////////////////////

            let mut iter = Attributes::html(r#"tag key= regular ='attribute'"#, 3);

            // Because we do not check validity of keys and values during parsing,
            // "regular" is considered as unquoted attribute value
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"regular"),
                }))
            );
            // Because we do not check validity of keys and values during parsing,
            // "='attribute'" is considered as key-only attribute
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"='attribute'"),
                    value: Cow::Borrowed(&[]),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);

            ////////////////////////////////////////////////////////////////////

            let mut iter = Attributes::html(r#"tag key= regular = 'attribute'"#, 3);
            //                                 0        ^ = 9     ^ = 19     ^ = 30

            // Because we do not check validity of keys and values during parsing,
            // "regular" is considered as unquoted attribute value
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"regular"),
                }))
            );
            // Because we do not check validity of keys and values during parsing,
            // "=" is considered as key-only attribute
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"="),
                    value: Cow::Borrowed(&[]),
                }))
            );
            // Because we do not check validity of keys and values during parsing,
            // "'attribute'" is considered as key-only attribute
            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"'attribute'"),
                    value: Cow::Borrowed(&[]),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }
    }

    /// Copy of single, but with additional spaces in markup
    mod sparsed {
        use super::*;
        use pretty_assertions::assert_eq;

        /// Attribute have a value enclosed in single quotes
        #[test]
        fn single_quoted() {
            let mut iter = Attributes::html(r#"tag key = 'value' "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value enclosed in double quotes
        #[test]
        fn double_quoted() {
            let mut iter = Attributes::html(r#"tag key = "value" "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute have a value, not enclosed in quotes
        #[test]
        fn unquoted() {
            let mut iter = Attributes::html(r#"tag key = value "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Only attribute key is present
        #[test]
        fn key_only() {
            let mut iter = Attributes::html(r#"tag key "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key"),
                    value: Cow::Borrowed(&[]),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key is started with an invalid symbol (a single quote in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_start_invalid() {
            let mut iter = Attributes::html(r#"tag 'key' = 'value' "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"'key'"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Key contains an invalid symbol (an ampersand in this test).
        /// Because we do not check validity of keys and values during parsing,
        /// that invalid attribute will be returned
        #[test]
        fn key_contains_invalid() {
            let mut iter = Attributes::html(r#"tag key&jey = 'value' "#, 3);

            assert_eq!(
                iter.next(),
                Some(Ok(Attribute {
                    key: QName(b"key&jey"),
                    value: Cow::Borrowed(b"value"),
                }))
            );
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }

        /// Attribute value is missing after `=`
        #[test]
        fn missed_value() {
            let mut iter = Attributes::html(r#"tag key = "#, 3);
            //                                 0         ^ = 10

            assert_eq!(iter.next(), Some(Err(AttrError::ExpectedValue(10))));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
        }
    }

    /// Checks that duplicated attributes correctly reported and recovering is
    /// possible after that
    mod duplicated {
        use super::*;

        mod with_check {
            use super::*;
            use pretty_assertions::assert_eq;

            /// Attribute have a value enclosed in single quotes
            #[test]
            fn single_quoted() {
                let mut iter = Attributes::html(r#"tag key='value' key='dup' another=''"#, 3);
                //                                 0   ^ = 4       ^ = 16

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::Duplicated(16, 4))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value enclosed in double quotes
            #[test]
            fn double_quoted() {
                let mut iter = Attributes::html(r#"tag key='value' key="dup" another=''"#, 3);
                //                                 0   ^ = 4       ^ = 16

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::Duplicated(16, 4))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value, not enclosed in quotes
            #[test]
            fn unquoted() {
                let mut iter = Attributes::html(r#"tag key='value' key=dup another=''"#, 3);
                //                                 0   ^ = 4       ^ = 16

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::Duplicated(16, 4))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Only attribute key is present
            #[test]
            fn key_only() {
                let mut iter = Attributes::html(r#"tag key='value' key another=''"#, 3);
                //                                 0   ^ = 4       ^ = 16

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(iter.next(), Some(Err(AttrError::Duplicated(16, 4))));
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }
        }

        /// Check for duplicated names is disabled
        mod without_check {
            use super::*;
            use pretty_assertions::assert_eq;

            /// Attribute have a value enclosed in single quotes
            #[test]
            fn single_quoted() {
                let mut iter = Attributes::html(r#"tag key='value' key='dup' another=''"#, 3);
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"dup"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value enclosed in double quotes
            #[test]
            fn double_quoted() {
                let mut iter = Attributes::html(r#"tag key='value' key="dup" another=''"#, 3);
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"dup"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Attribute have a value, not enclosed in quotes
            #[test]
            fn unquoted() {
                let mut iter = Attributes::html(r#"tag key='value' key=dup another=''"#, 3);
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"dup"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }

            /// Only attribute key is present
            #[test]
            fn key_only() {
                let mut iter = Attributes::html(r#"tag key='value' key another=''"#, 3);
                iter.with_checks(false);

                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(b"value"),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"key"),
                        value: Cow::Borrowed(&[]),
                    }))
                );
                assert_eq!(
                    iter.next(),
                    Some(Ok(Attribute {
                        key: QName(b"another"),
                        value: Cow::Borrowed(b""),
                    }))
                );
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
            }
        }
    }

    #[test]
    fn mixed_quote() {
        let mut iter = Attributes::html(r#"tag a='a' b = "b" c='cc"cc' d="dd'dd""#, 3);

        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: QName(b"a"),
                value: Cow::Borrowed(b"a"),
            }))
        );
        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: QName(b"b"),
                value: Cow::Borrowed(b"b"),
            }))
        );
        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: QName(b"c"),
                value: Cow::Borrowed(br#"cc"cc"#),
            }))
        );
        assert_eq!(
            iter.next(),
            Some(Ok(Attribute {
                key: QName(b"d"),
                value: Cow::Borrowed(b"dd'dd"),
            }))
        );
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }
}
