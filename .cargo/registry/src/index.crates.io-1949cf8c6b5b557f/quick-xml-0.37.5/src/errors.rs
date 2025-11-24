//! Error management module

use crate::encoding::{Decoder, EncodingError};
use crate::escape::EscapeError;
use crate::events::attributes::AttrError;
use crate::name::{NamespaceError, QName};
use std::fmt;
use std::io::Error as IoError;
use std::sync::Arc;

/// An error returned if parsed document does not correspond to the XML grammar,
/// for example, a tag opened by `<` not closed with `>`. This error does not
/// represent invalid XML constructs, for example, tags `<>` and `</>` a well-formed
/// from syntax point-of-view.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SyntaxError {
    /// The parser started to parse `<!`, but the input ended before it can recognize
    /// anything.
    InvalidBangMarkup,
    /// The parser started to parse processing instruction or XML declaration (`<?`),
    /// but the input ended before the `?>` sequence was found.
    UnclosedPIOrXmlDecl,
    /// The parser started to parse comment (`<!--`) content, but the input ended
    /// before the `-->` sequence was found.
    UnclosedComment,
    /// The parser started to parse DTD (`<!DOCTYPE`) content, but the input ended
    /// before the closing `>` character was found.
    UnclosedDoctype,
    /// The parser started to parse `<![CDATA[` content, but the input ended
    /// before the `]]>` sequence was found.
    UnclosedCData,
    /// The parser started to parse tag content, but the input ended
    /// before the closing `>` character was found.
    UnclosedTag,
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidBangMarkup => f.write_str("unknown or missed symbol in markup"),
            Self::UnclosedPIOrXmlDecl => {
                f.write_str("processing instruction or xml declaration not closed: `?>` not found before end of input")
            }
            Self::UnclosedComment => {
                f.write_str("comment not closed: `-->` not found before end of input")
            }
            Self::UnclosedDoctype => {
                f.write_str("DOCTYPE not closed: `>` not found before end of input")
            }
            Self::UnclosedCData => {
                f.write_str("CDATA not closed: `]]>` not found before end of input")
            }
            Self::UnclosedTag => f.write_str("tag not closed: `>` not found before end of input"),
        }
    }
}

impl std::error::Error for SyntaxError {}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// An error returned if parsed document is not [well-formed], for example,
/// an opened tag is not closed before end of input.
///
/// Those errors are not fatal: after encountering an error you can continue
/// parsing the document.
///
/// [well-formed]: https://www.w3.org/TR/xml11/#dt-wellformed
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IllFormedError {
    /// A `version` attribute was not found in an XML declaration or is not the
    /// first attribute.
    ///
    /// According to the [specification], the XML declaration (`<?xml ?>`) MUST contain
    /// a `version` attribute and it MUST be the first attribute. This error indicates,
    /// that the declaration does not contain attributes at all (if contains `None`)
    /// or either `version` attribute is not present or not the first attribute in
    /// the declaration. In the last case it contains the name of the found attribute.
    ///
    /// [specification]: https://www.w3.org/TR/xml11/#sec-prolog-dtd
    MissingDeclVersion(Option<String>),
    /// A document type definition (DTD) does not contain a name of a root element.
    ///
    /// According to the [specification], document type definition (`<!DOCTYPE foo>`)
    /// MUST contain a name which defines a document type (`foo`). If that name
    /// is missed, this error is returned.
    ///
    /// [specification]: https://www.w3.org/TR/xml11/#NT-doctypedecl
    MissingDoctypeName,
    /// The end tag was not found during reading of a sub-tree of elements due to
    /// encountering an EOF from the underlying reader. This error is returned from
    /// [`Reader::read_to_end`].
    ///
    /// [`Reader::read_to_end`]: crate::reader::Reader::read_to_end
    MissingEndTag(String),
    /// The specified end tag was encountered without corresponding open tag at the
    /// same level of hierarchy
    UnmatchedEndTag(String),
    /// The specified end tag does not match the start tag at that nesting level.
    MismatchedEndTag {
        /// Name of open tag, that is expected to be closed
        expected: String,
        /// Name of actually closed tag
        found: String,
    },
    /// A comment contains forbidden double-hyphen (`--`) sequence inside.
    ///
    /// According to the [specification], for compatibility, comments MUST NOT contain
    /// double-hyphen (`--`) sequence, in particular, they cannot end by `--->`.
    ///
    /// The quick-xml by default does not check that, because this restriction is
    /// mostly artificial, but you can enable it in the [configuration].
    ///
    /// [specification]: https://www.w3.org/TR/xml11/#sec-comments
    /// [configuration]: crate::reader::Config::check_comments
    DoubleHyphenInComment,
}

impl fmt::Display for IllFormedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::MissingDeclVersion(None) => {
                f.write_str("an XML declaration does not contain `version` attribute")
            }
            Self::MissingDeclVersion(Some(attr)) => {
                write!(f, "an XML declaration must start with `version` attribute, but in starts with `{}`", attr)
            }
            Self::MissingDoctypeName => {
                f.write_str("`<!DOCTYPE>` declaration does not contain a name of a document type")
            }
            Self::MissingEndTag(tag) => write!(
                f,
                "start tag not closed: `</{}>` not found before end of input",
                tag,
            ),
            Self::UnmatchedEndTag(tag) => {
                write!(f, "close tag `</{}>` does not match any open tag", tag)
            }
            Self::MismatchedEndTag { expected, found } => write!(
                f,
                "expected `</{}>`, but `</{}>` was found",
                expected, found,
            ),
            Self::DoubleHyphenInComment => {
                f.write_str("forbidden string `--` was found in a comment")
            }
        }
    }
}

impl std::error::Error for IllFormedError {}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// The error type used by this crate.
#[derive(Clone, Debug)]
pub enum Error {
    /// XML document cannot be read from underlying source.
    ///
    /// Contains the reference-counted I/O error to make the error type `Clone`able.
    Io(Arc<IoError>),
    /// The document does not corresponds to the XML grammar.
    Syntax(SyntaxError),
    /// The document is not [well-formed](https://www.w3.org/TR/xml11/#dt-wellformed).
    IllFormed(IllFormedError),
    /// Attribute parsing error
    InvalidAttr(AttrError),
    /// Encoding error
    Encoding(EncodingError),
    /// Escape error
    Escape(EscapeError),
    /// Parsed XML has some namespace-related problems
    Namespace(NamespaceError),
}

impl Error {
    pub(crate) fn missed_end(name: QName, decoder: Decoder) -> Self {
        match decoder.decode(name.as_ref()) {
            Ok(name) => IllFormedError::MissingEndTag(name.into()).into(),
            Err(err) => err.into(),
        }
    }
}

impl From<IoError> for Error {
    /// Creates a new `Error::Io` from the given error
    #[inline]
    fn from(error: IoError) -> Error {
        Self::Io(Arc::new(error))
    }
}

impl From<SyntaxError> for Error {
    /// Creates a new `Error::Syntax` from the given error
    #[inline]
    fn from(error: SyntaxError) -> Self {
        Self::Syntax(error)
    }
}

impl From<IllFormedError> for Error {
    /// Creates a new `Error::IllFormed` from the given error
    #[inline]
    fn from(error: IllFormedError) -> Self {
        Self::IllFormed(error)
    }
}

impl From<EncodingError> for Error {
    /// Creates a new `Error::EncodingError` from the given error
    #[inline]
    fn from(error: EncodingError) -> Error {
        Self::Encoding(error)
    }
}

impl From<EscapeError> for Error {
    /// Creates a new `Error::EscapeError` from the given error
    #[inline]
    fn from(error: EscapeError) -> Error {
        Self::Escape(error)
    }
}

impl From<AttrError> for Error {
    #[inline]
    fn from(error: AttrError) -> Self {
        Self::InvalidAttr(error)
    }
}

impl From<NamespaceError> for Error {
    #[inline]
    fn from(error: NamespaceError) -> Self {
        Self::Namespace(error)
    }
}

/// A specialized `Result` type where the error is hard-wired to [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::Syntax(e) => write!(f, "syntax error: {}", e),
            Self::IllFormed(e) => write!(f, "ill-formed document: {}", e),
            Self::InvalidAttr(e) => write!(f, "error while parsing attribute: {}", e),
            Self::Encoding(e) => e.fmt(f),
            Self::Escape(e) => e.fmt(f),
            Self::Namespace(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Syntax(e) => Some(e),
            Self::IllFormed(e) => Some(e),
            Self::InvalidAttr(e) => Some(e),
            Self::Encoding(e) => Some(e),
            Self::Escape(e) => Some(e),
            Self::Namespace(e) => Some(e),
        }
    }
}

#[cfg(feature = "serialize")]
pub mod serialize {
    //! A module to handle serde (de)serialization errors

    use super::*;
    use crate::utils::write_byte_string;
    use std::borrow::Cow;
    #[cfg(feature = "overlapped-lists")]
    use std::num::NonZeroUsize;
    use std::str::Utf8Error;

    /// (De)serialization error
    #[derive(Clone, Debug)]
    pub enum DeError {
        /// Serde custom error
        Custom(String),
        /// Xml parsing error
        InvalidXml(Error),
        /// This error indicates an error in the [`Deserialize`](serde::Deserialize)
        /// implementation when read a map or a struct: `MapAccess::next_value[_seed]`
        /// was called before `MapAccess::next_key[_seed]`.
        ///
        /// You should check your types, that implements corresponding trait.
        KeyNotRead,
        /// Deserializer encounter a start tag with a specified name when it is
        /// not expecting. This happens when you try to deserialize a primitive
        /// value (numbers, strings, booleans) from an XML element.
        UnexpectedStart(Vec<u8>),
        /// The [`Reader`] produced [`Event::Eof`] when it is not expecting,
        /// for example, after producing [`Event::Start`] but before corresponding
        /// [`Event::End`].
        ///
        /// [`Reader`]: crate::reader::Reader
        /// [`Event::Eof`]: crate::events::Event::Eof
        /// [`Event::Start`]: crate::events::Event::Start
        /// [`Event::End`]: crate::events::Event::End
        UnexpectedEof,
        /// Too many events were skipped while deserializing a sequence, event limit
        /// exceeded. The limit was provided as an argument
        #[cfg(feature = "overlapped-lists")]
        TooManyEvents(NonZeroUsize),
    }

    impl fmt::Display for DeError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                Self::Custom(s) => f.write_str(s),
                Self::InvalidXml(e) => e.fmt(f),
                Self::KeyNotRead => f.write_str("invalid `Deserialize` implementation: `MapAccess::next_value[_seed]` was called before `MapAccess::next_key[_seed]`"),
                Self::UnexpectedStart(e) => {
                    f.write_str("unexpected `Event::Start(")?;
                    write_byte_string(f, e)?;
                    f.write_str(")`")
                }
                Self::UnexpectedEof => f.write_str("unexpected `Event::Eof`"),
                #[cfg(feature = "overlapped-lists")]
                Self::TooManyEvents(s) => write!(f, "deserializer buffered {} events, limit exceeded", s),
            }
        }
    }

    impl std::error::Error for DeError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                Self::InvalidXml(e) => Some(e),
                _ => None,
            }
        }
    }

    impl serde::de::Error for DeError {
        fn custom<T: fmt::Display>(msg: T) -> Self {
            Self::Custom(msg.to_string())
        }
    }

    impl From<Error> for DeError {
        #[inline]
        fn from(e: Error) -> Self {
            Self::InvalidXml(e)
        }
    }

    impl From<EscapeError> for DeError {
        #[inline]
        fn from(e: EscapeError) -> Self {
            Self::InvalidXml(e.into())
        }
    }

    impl From<EncodingError> for DeError {
        #[inline]
        fn from(e: EncodingError) -> Self {
            Self::InvalidXml(e.into())
        }
    }

    impl From<AttrError> for DeError {
        #[inline]
        fn from(e: AttrError) -> Self {
            Self::InvalidXml(e.into())
        }
    }

    /// Serialization error
    #[derive(Clone, Debug)]
    pub enum SeError {
        /// Serde custom error
        Custom(String),
        /// XML document cannot be written to underlying source.
        ///
        /// Contains the reference-counted I/O error to make the error type `Clone`able.
        Io(Arc<IoError>),
        /// Some value could not be formatted
        Fmt(std::fmt::Error),
        /// Serialized type cannot be represented in an XML due to violation of the
        /// XML rules in the final XML document. For example, attempt to serialize
        /// a `HashMap<{integer}, ...>` would cause this error because [XML name]
        /// cannot start from a digit or a hyphen (minus sign). The same result
        /// would occur if map key is a complex type that cannot be serialized as
        /// a primitive type (i.e. string, char, bool, unit struct or unit variant).
        ///
        /// [XML name]: https://www.w3.org/TR/xml11/#sec-common-syn
        Unsupported(Cow<'static, str>),
        /// Some value could not be turned to UTF-8
        NonEncodable(Utf8Error),
    }

    impl fmt::Display for SeError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                Self::Custom(s) => f.write_str(s),
                Self::Io(e) => write!(f, "I/O error: {}", e),
                Self::Fmt(e) => write!(f, "formatting error: {}", e),
                Self::Unsupported(s) => write!(f, "unsupported value: {}", s),
                Self::NonEncodable(e) => write!(f, "malformed UTF-8: {}", e),
            }
        }
    }

    impl ::std::error::Error for SeError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                Self::Io(e) => Some(e),
                _ => None,
            }
        }
    }

    impl serde::ser::Error for SeError {
        fn custom<T: fmt::Display>(msg: T) -> Self {
            Self::Custom(msg.to_string())
        }
    }

    impl From<IoError> for SeError {
        #[inline]
        fn from(e: IoError) -> Self {
            Self::Io(Arc::new(e))
        }
    }

    impl From<Utf8Error> for SeError {
        #[inline]
        fn from(e: Utf8Error) -> Self {
            Self::NonEncodable(e)
        }
    }

    impl From<fmt::Error> for SeError {
        #[inline]
        fn from(e: fmt::Error) -> Self {
            Self::Fmt(e)
        }
    }
}
