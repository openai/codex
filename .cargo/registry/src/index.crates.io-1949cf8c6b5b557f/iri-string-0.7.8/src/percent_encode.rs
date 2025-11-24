//! Percent encoding.

use core::fmt::{self, Write as _};
use core::marker::PhantomData;

use crate::parser::char;
use crate::spec::{IriSpec, Spec, UriSpec};

/// A proxy to percent-encode a string as a part of URI.
pub type PercentEncodedForUri<T> = PercentEncoded<T, UriSpec>;

/// A proxy to percent-encode a string as a part of IRI.
pub type PercentEncodedForIri<T> = PercentEncoded<T, IriSpec>;

/// Context for percent encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
enum Context {
    /// Encode the string as a reg-name (usually called as "hostname").
    RegName,
    /// Encode the string as a user name or a password (inside the `userinfo` component).
    UserOrPassword,
    /// Encode the string as a path segment.
    ///
    /// A slash (`/`) will be encoded to `%2F`.
    PathSegment,
    /// Encode the string as path segments joined with `/`.
    ///
    /// A slash (`/`) will be used as is.
    Path,
    /// Encode the string as a query string (without the `?` prefix).
    Query,
    /// Encode the string as a fragment string (without the `#` prefix).
    Fragment,
    /// Encode all characters except for `unreserved` characters.
    Unreserve,
    /// Encode characters only if they cannot appear anywhere in an IRI reference.
    ///
    /// `%` character will be always encoded.
    Character,
}

/// A proxy to percent-encode a string.
///
/// Type aliases [`PercentEncodedForIri`] and [`PercentEncodedForUri`] are provided.
/// You can use them to make the expression simpler, for example write
/// `PercentEncodedForUri::from_path(foo)` instead of
/// `PercentEncoded::<_, UriSpec>::from_path(foo)`.
#[derive(Debug, Clone, Copy)]
pub struct PercentEncoded<T, S> {
    /// Source string context.
    context: Context,
    /// Raw string before being encoded.
    raw: T,
    /// Spec.
    _spec: PhantomData<fn() -> S>,
}

impl<T: fmt::Display, S: Spec> PercentEncoded<T, S> {
    /// Creates an encoded string from a raw reg-name (i.e. hostname or domain).
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::percent_encode::PercentEncoded;
    /// use iri_string::spec::UriSpec;
    ///
    /// let raw = "alpha.\u{03B1}.example.com";
    /// let encoded = "alpha.%CE%B1.example.com";
    /// assert_eq!(
    ///     PercentEncoded::<_, UriSpec>::from_reg_name(raw).to_string(),
    ///     encoded
    /// );
    /// # }
    /// ```
    pub fn from_reg_name(raw: T) -> Self {
        Self {
            context: Context::RegName,
            raw,
            _spec: PhantomData,
        }
    }

    /// Creates an encoded string from a raw user name (inside `userinfo` component).
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::percent_encode::PercentEncoded;
    /// use iri_string::spec::UriSpec;
    ///
    /// let raw = "user:\u{03B1}";
    /// // The first `:` will be interpreted as a delimiter, so colons will be escaped.
    /// let encoded = "user%3A%CE%B1";
    /// assert_eq!(
    ///     PercentEncoded::<_, UriSpec>::from_user(raw).to_string(),
    ///     encoded
    /// );
    /// # }
    /// ```
    pub fn from_user(raw: T) -> Self {
        Self {
            context: Context::UserOrPassword,
            raw,
            _spec: PhantomData,
        }
    }

    /// Creates an encoded string from a raw user name (inside `userinfo` component).
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::percent_encode::PercentEncoded;
    /// use iri_string::spec::UriSpec;
    ///
    /// let raw = "password:\u{03B1}";
    /// // The first `:` will be interpreted as a delimiter, and the colon
    /// // inside the password will be the first one if the user name is empty,
    /// // so colons will be escaped.
    /// let encoded = "password%3A%CE%B1";
    /// assert_eq!(
    ///     PercentEncoded::<_, UriSpec>::from_password(raw).to_string(),
    ///     encoded
    /// );
    /// # }
    /// ```
    pub fn from_password(raw: T) -> Self {
        Self {
            context: Context::UserOrPassword,
            raw,
            _spec: PhantomData,
        }
    }

    /// Creates an encoded string from a raw path segment.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::percent_encode::PercentEncoded;
    /// use iri_string::spec::UriSpec;
    ///
    /// let raw = "alpha/\u{03B1}?#";
    /// // Note that `/` is encoded to `%2F`.
    /// let encoded = "alpha%2F%CE%B1%3F%23";
    /// assert_eq!(
    ///     PercentEncoded::<_, UriSpec>::from_path_segment(raw).to_string(),
    ///     encoded
    /// );
    /// # }
    /// ```
    pub fn from_path_segment(raw: T) -> Self {
        Self {
            context: Context::PathSegment,
            raw,
            _spec: PhantomData,
        }
    }

    /// Creates an encoded string from a raw path.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::percent_encode::PercentEncoded;
    /// use iri_string::spec::UriSpec;
    ///
    /// let raw = "alpha/\u{03B1}?#";
    /// // Note that `/` is NOT percent encoded.
    /// let encoded = "alpha/%CE%B1%3F%23";
    /// assert_eq!(
    ///     PercentEncoded::<_, UriSpec>::from_path(raw).to_string(),
    ///     encoded
    /// );
    /// # }
    /// ```
    pub fn from_path(raw: T) -> Self {
        Self {
            context: Context::Path,
            raw,
            _spec: PhantomData,
        }
    }

    /// Creates an encoded string from a raw query.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::percent_encode::PercentEncoded;
    /// use iri_string::spec::UriSpec;
    ///
    /// let raw = "alpha/\u{03B1}?#";
    /// let encoded = "alpha/%CE%B1?%23";
    /// assert_eq!(
    ///     PercentEncoded::<_, UriSpec>::from_query(raw).to_string(),
    ///     encoded
    /// );
    /// # }
    /// ```
    pub fn from_query(raw: T) -> Self {
        Self {
            context: Context::Query,
            raw,
            _spec: PhantomData,
        }
    }

    /// Creates an encoded string from a raw fragment.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::percent_encode::PercentEncoded;
    /// use iri_string::spec::UriSpec;
    ///
    /// let raw = "alpha/\u{03B1}?#";
    /// let encoded = "alpha/%CE%B1?%23";
    /// assert_eq!(
    ///     PercentEncoded::<_, UriSpec>::from_fragment(raw).to_string(),
    ///     encoded
    /// );
    /// # }
    /// ```
    pub fn from_fragment(raw: T) -> Self {
        Self {
            context: Context::Fragment,
            raw,
            _spec: PhantomData,
        }
    }

    /// Creates a string consists of only `unreserved` string and percent-encoded triplets.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::percent_encode::PercentEncoded;
    /// use iri_string::spec::UriSpec;
    ///
    /// let unreserved = "%a0-._~\u{03B1}";
    /// let unreserved_encoded = "%25a0-._~%CE%B1";
    /// assert_eq!(
    ///     PercentEncoded::<_, UriSpec>::unreserve(unreserved).to_string(),
    ///     unreserved_encoded
    /// );
    ///
    /// let reserved = ":/?#[]@ !$&'()*+,;=";
    /// let reserved_encoded =
    ///     "%3A%2F%3F%23%5B%5D%40%20%21%24%26%27%28%29%2A%2B%2C%3B%3D";
    /// assert_eq!(
    ///     PercentEncoded::<_, UriSpec>::unreserve(reserved).to_string(),
    ///     reserved_encoded
    /// );
    /// # }
    /// ```
    #[inline]
    #[must_use]
    pub fn unreserve(raw: T) -> Self {
        Self {
            context: Context::Unreserve,
            raw,
            _spec: PhantomData,
        }
    }

    /// Percent-encodes characters only if they cannot appear anywhere in an IRI reference.
    ///
    /// `%` character will be always encoded. In other words, this conversion
    /// is not aware of percent-encoded triplets.
    ///
    /// Note that this encoding process does not guarantee that the resulting
    /// string is a valid IRI reference.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::percent_encode::PercentEncoded;
    /// use iri_string::spec::UriSpec;
    ///
    /// let unreserved = "%a0-._~\u{03B1}";
    /// let unreserved_encoded = "%25a0-._~%CE%B1";
    /// assert_eq!(
    ///     PercentEncoded::<_, UriSpec>::characters(unreserved).to_string(),
    ///     unreserved_encoded
    /// );
    ///
    /// let reserved = ":/?#[]@ !$&'()*+,;=";
    /// // Note that `%20` cannot appear directly in an IRI reference.
    /// let expected = ":/?#[]@%20!$&'()*+,;=";
    /// assert_eq!(
    ///     PercentEncoded::<_, UriSpec>::characters(reserved).to_string(),
    ///     expected
    /// );
    /// # }
    /// ```
    #[inline]
    #[must_use]
    pub fn characters(raw: T) -> Self {
        Self {
            context: Context::Character,
            raw,
            _spec: PhantomData,
        }
    }
}

impl<T: fmt::Display, S: Spec> fmt::Display for PercentEncoded<T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        /// Filter that encodes a character before written if necessary.
        struct Filter<'a, 'b, S> {
            /// Encoding context.
            context: Context,
            /// Writer.
            writer: &'a mut fmt::Formatter<'b>,
            /// Spec.
            _spec: PhantomData<fn() -> S>,
        }
        impl<S: Spec> fmt::Write for Filter<'_, '_, S> {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                s.chars().try_for_each(|c| self.write_char(c))
            }
            fn write_char(&mut self, c: char) -> fmt::Result {
                let is_valid_char = match (self.context, c.is_ascii()) {
                    (Context::RegName, true) => char::is_ascii_regname(c as u8),
                    (Context::RegName, false) => char::is_nonascii_regname::<S>(c),
                    (Context::UserOrPassword, true) => {
                        c != ':' && char::is_ascii_userinfo_ipvfutureaddr(c as u8)
                    }
                    (Context::UserOrPassword, false) => char::is_nonascii_userinfo::<S>(c),
                    (Context::PathSegment, true) => char::is_ascii_pchar(c as u8),
                    (Context::PathSegment, false) => S::is_nonascii_char_unreserved(c),
                    (Context::Path, true) => c == '/' || char::is_ascii_pchar(c as u8),
                    (Context::Path, false) => S::is_nonascii_char_unreserved(c),
                    (Context::Query, true) => c == '/' || char::is_ascii_frag_query(c as u8),
                    (Context::Query, false) => char::is_nonascii_query::<S>(c),
                    (Context::Fragment, true) => c == '/' || char::is_ascii_frag_query(c as u8),
                    (Context::Fragment, false) => char::is_nonascii_fragment::<S>(c),
                    (Context::Unreserve, true) => char::is_ascii_unreserved(c as u8),
                    (Context::Unreserve, false) => S::is_nonascii_char_unreserved(c),
                    (Context::Character, true) => char::is_ascii_unreserved_or_reserved(c as u8),
                    (Context::Character, false) => {
                        S::is_nonascii_char_unreserved(c) || S::is_nonascii_char_private(c)
                    }
                };
                if is_valid_char {
                    self.writer.write_char(c)
                } else {
                    write_pct_encoded_char(&mut self.writer, c)
                }
            }
        }
        let mut filter = Filter {
            context: self.context,
            writer: f,
            _spec: PhantomData::<fn() -> S>,
        };
        write!(filter, "{}", self.raw)
    }
}

/// Percent-encodes the given character and writes it.
#[inline]
fn write_pct_encoded_char<W: fmt::Write>(writer: &mut W, c: char) -> fmt::Result {
    let mut buf = [0_u8; 4];
    let buf = c.encode_utf8(&mut buf);
    buf.bytes().try_for_each(|b| write!(writer, "%{:02X}", b))
}
