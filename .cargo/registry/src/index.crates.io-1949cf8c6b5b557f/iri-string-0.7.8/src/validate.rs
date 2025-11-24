//! Validators.

use core::fmt;

#[cfg(feature = "std")]
use std::error;

use crate::parser::validate as parser;
use crate::spec::Spec;

/// Resource identifier validation error.
// Note that this type should implement `Copy` trait.
// To return additional non-`Copy` data as an error, use wrapper type
// (as `std::string::FromUtf8Error` contains `std::str::Utf8Error`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(());

impl Error {
    /// Creates a new `Error`.
    ///
    /// For internal use.
    #[inline]
    #[must_use]
    pub(crate) fn new() -> Self {
        Error(())
    }
}

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Invalid IRI")
    }
}

#[cfg(feature = "std")]
impl error::Error for Error {}

/// Validates [IRI][uri].
///
/// This validator corresponds to [`RiStr`] and [`RiString`] types.
///
/// # Examples
///
/// This type can have an IRI (which is absolute, and may have fragment part).
///
/// ```
/// use iri_string::{spec::UriSpec, validate::iri};
/// assert!(iri::<UriSpec>("https://user:pass@example.com:8080").is_ok());
/// assert!(iri::<UriSpec>("https://example.com/").is_ok());
/// assert!(iri::<UriSpec>("https://example.com/foo?bar=baz").is_ok());
/// assert!(iri::<UriSpec>("https://example.com/foo?bar=baz#qux").is_ok());
/// assert!(iri::<UriSpec>("foo:bar").is_ok());
/// assert!(iri::<UriSpec>("foo:").is_ok());
/// // `foo://.../` below are all allowed. See the crate documentation for detail.
/// assert!(iri::<UriSpec>("foo:/").is_ok());
/// assert!(iri::<UriSpec>("foo://").is_ok());
/// assert!(iri::<UriSpec>("foo:///").is_ok());
/// assert!(iri::<UriSpec>("foo:////").is_ok());
/// assert!(iri::<UriSpec>("foo://///").is_ok());
/// ```
///
/// Relative IRI reference is not allowed.
///
/// ```
/// use iri_string::{spec::UriSpec, validate::iri};
/// // This is relative path.
/// assert!(iri::<UriSpec>("foo/bar").is_err());
/// // `/foo/bar` is an absolute path, but it is authority-relative.
/// assert!(iri::<UriSpec>("/foo/bar").is_err());
/// // `//foo/bar` is termed "network-path reference",
/// // or usually called "protocol-relative reference".
/// assert!(iri::<UriSpec>("//foo/bar").is_err());
/// // Same-document reference is relative.
/// assert!(iri::<UriSpec>("#foo").is_err());
/// // Empty string is not a valid absolute IRI.
/// assert!(iri::<UriSpec>("").is_err());
/// ```
///
/// Some characters and sequences cannot used in an IRI.
///
/// ```
/// use iri_string::{spec::UriSpec, validate::iri};
/// // `<` and `>` cannot directly appear in an IRI.
/// assert!(iri::<UriSpec>("<not allowed>").is_err());
/// // Broken percent encoding cannot appear in an IRI.
/// assert!(iri::<UriSpec>("%").is_err());
/// assert!(iri::<UriSpec>("%GG").is_err());
/// ```
///
/// [uri]: https://tools.ietf.org/html/rfc3986#section-3
/// [`RiStr`]: ../types/struct.RiStr.html
/// [`RiString`]: ../types/struct.RiString.html
pub fn iri<S: Spec>(s: &str) -> Result<(), Error> {
    parser::validate_uri::<S>(s)
}

/// Validates [IRI reference][uri-reference].
///
/// This validator corresponds to [`RiReferenceStr`] and [`RiReferenceString`] types.
///
/// # Examples
///
/// This type can have an IRI reference (which can be absolute or relative).
///
/// ```
/// use iri_string::{spec::UriSpec, validate::iri_reference};
/// assert!(iri_reference::<UriSpec>("https://user:pass@example.com:8080").is_ok());
/// assert!(iri_reference::<UriSpec>("https://example.com/").is_ok());
/// assert!(iri_reference::<UriSpec>("https://example.com/foo?bar=baz").is_ok());
/// assert!(iri_reference::<UriSpec>("https://example.com/foo?bar=baz#qux").is_ok());
/// assert!(iri_reference::<UriSpec>("foo:bar").is_ok());
/// assert!(iri_reference::<UriSpec>("foo:").is_ok());
/// // `foo://.../` below are all allowed. See the crate documentation for detail.
/// assert!(iri_reference::<UriSpec>("foo:/").is_ok());
/// assert!(iri_reference::<UriSpec>("foo://").is_ok());
/// assert!(iri_reference::<UriSpec>("foo:///").is_ok());
/// assert!(iri_reference::<UriSpec>("foo:////").is_ok());
/// assert!(iri_reference::<UriSpec>("foo://///").is_ok());
/// assert!(iri_reference::<UriSpec>("foo/bar").is_ok());
/// assert!(iri_reference::<UriSpec>("/foo/bar").is_ok());
/// assert!(iri_reference::<UriSpec>("//foo/bar").is_ok());
/// assert!(iri_reference::<UriSpec>("#foo").is_ok());
/// ```
///
/// Some characters and sequences cannot used in an IRI reference.
///
/// ```
/// use iri_string::{spec::UriSpec, validate::iri_reference};
/// // `<` and `>` cannot directly appear in an IRI reference.
/// assert!(iri_reference::<UriSpec>("<not allowed>").is_err());
/// // Broken percent encoding cannot appear in an IRI reference.
/// assert!(iri_reference::<UriSpec>("%").is_err());
/// assert!(iri_reference::<UriSpec>("%GG").is_err());
/// ```
///
/// [uri-reference]: https://tools.ietf.org/html/rfc3986#section-4.1
/// [`RiReferenceStr`]: ../types/struct.RiReferenceStr.html
/// [`RiReferenceString`]: ../types/struct.RiReferenceString.html
pub fn iri_reference<S: Spec>(s: &str) -> Result<(), Error> {
    parser::validate_uri_reference::<S>(s)
}

/// Validates [absolute IRI][absolute-uri].
///
/// This validator corresponds to [`RiAbsoluteStr`] and [`RiAbsoluteString`] types.
///
/// # Examples
///
/// This type can have an absolute IRI without fragment part.
///
/// ```
/// use iri_string::{spec::UriSpec, validate::absolute_iri};
/// assert!(absolute_iri::<UriSpec>("https://example.com/foo?bar=baz").is_ok());
/// assert!(absolute_iri::<UriSpec>("foo:bar").is_ok());
/// // Scheme `foo` and empty path.
/// assert!(absolute_iri::<UriSpec>("foo:").is_ok());
/// // `foo://.../` below are all allowed. See the crate documentation for detail.
/// assert!(absolute_iri::<UriSpec>("foo:/").is_ok());
/// assert!(absolute_iri::<UriSpec>("foo://").is_ok());
/// assert!(absolute_iri::<UriSpec>("foo:///").is_ok());
/// assert!(absolute_iri::<UriSpec>("foo:////").is_ok());
/// assert!(absolute_iri::<UriSpec>("foo://///").is_ok());
///
/// ```
///
/// Relative IRI is not allowed.
///
/// ```
/// use iri_string::{spec::UriSpec, validate::absolute_iri};
/// // This is relative path.
/// assert!(absolute_iri::<UriSpec>("foo/bar").is_err());
/// // `/foo/bar` is an absolute path, but it is authority-relative.
/// assert!(absolute_iri::<UriSpec>("/foo/bar").is_err());
/// // `//foo/bar` is termed "network-path reference",
/// // or usually called "protocol-relative reference".
/// assert!(absolute_iri::<UriSpec>("//foo/bar").is_err());
/// // Empty string is not a valid absolute IRI.
/// assert!(absolute_iri::<UriSpec>("").is_err());
/// ```
///
/// Fragment part (such as trailing `#foo`) is not allowed.
///
/// ```
/// use iri_string::{spec::UriSpec, validate::absolute_iri};
/// // Fragment part is not allowed.
/// assert!(absolute_iri::<UriSpec>("https://example.com/foo?bar=baz#qux").is_err());
/// ```
///
/// Some characters and sequences cannot used in an absolute IRI.
///
/// ```
/// use iri_string::{spec::UriSpec, validate::absolute_iri};
/// // `<` and `>` cannot directly appear in an absolute IRI.
/// assert!(absolute_iri::<UriSpec>("<not allowed>").is_err());
/// // Broken percent encoding cannot appear in an absolute IRI.
/// assert!(absolute_iri::<UriSpec>("%").is_err());
/// assert!(absolute_iri::<UriSpec>("%GG").is_err());
/// ```
///
/// [absolute-uri]: https://tools.ietf.org/html/rfc3986#section-4.3
/// [`RiAbsoluteStr`]: ../types/struct.RiAbsoluteStr.html
/// [`RiAbsoluteString`]: ../types/struct.RiAbsoluteString.html
pub fn absolute_iri<S: Spec>(s: &str) -> Result<(), Error> {
    parser::validate_absolute_uri::<S>(s)
}

/// Validates [relative reference][relative-ref].
///
/// This validator corresponds to [`RiRelativeStr`] and [`RiRelativeString`] types.
///
/// # Valid values
///
/// This type can have a relative IRI reference.
///
/// ```
/// use iri_string::{spec::UriSpec, validate::relative_ref};
/// assert!(relative_ref::<UriSpec>("foo").is_ok());
/// assert!(relative_ref::<UriSpec>("foo/bar").is_ok());
/// assert!(relative_ref::<UriSpec>("/foo").is_ok());
/// assert!(relative_ref::<UriSpec>("//foo/bar").is_ok());
/// assert!(relative_ref::<UriSpec>("?foo").is_ok());
/// assert!(relative_ref::<UriSpec>("#foo").is_ok());
/// assert!(relative_ref::<UriSpec>("foo/bar?baz#qux").is_ok());
/// // The first path component can have colon if the path is absolute.
/// assert!(relative_ref::<UriSpec>("/foo:bar/").is_ok());
/// // Second or following path components can have colon.
/// assert!(relative_ref::<UriSpec>("foo/bar://baz/").is_ok());
/// assert!(relative_ref::<UriSpec>("./foo://bar").is_ok());
/// ```
///
/// Absolute form of a reference is not allowed.
///
/// ```
/// use iri_string::{spec::UriSpec, validate::relative_ref};
/// assert!(relative_ref::<UriSpec>("https://example.com/").is_err());
/// // The first path component cannot have colon, if the path is not absolute.
/// assert!(relative_ref::<UriSpec>("foo:bar").is_err());
/// assert!(relative_ref::<UriSpec>("foo:").is_err());
/// assert!(relative_ref::<UriSpec>("foo:/").is_err());
/// assert!(relative_ref::<UriSpec>("foo://").is_err());
/// assert!(relative_ref::<UriSpec>("foo:///").is_err());
/// assert!(relative_ref::<UriSpec>("foo:////").is_err());
/// assert!(relative_ref::<UriSpec>("foo://///").is_err());
/// ```
///
/// Some characters and sequences cannot used in an IRI reference.
///
/// ```
/// use iri_string::{spec::UriSpec, validate::relative_ref};
/// // `<` and `>` cannot directly appear in a relative IRI reference.
/// assert!(relative_ref::<UriSpec>("<not allowed>").is_err());
/// // Broken percent encoding cannot appear in a relative IRI reference.
/// assert!(relative_ref::<UriSpec>("%").is_err());
/// assert!(relative_ref::<UriSpec>("%GG").is_err());
/// ```
///
/// [relative-ref]: https://tools.ietf.org/html/rfc3986#section-4.2
/// [`RiRelativeStr`]: ../types/struct.RiRelativeStr.html
/// [`RiRelativeString`]: ../types/struct.RiRelativeString.html
pub fn relative_ref<S: Spec>(s: &str) -> Result<(), Error> {
    parser::validate_relative_ref::<S>(s)
}

/// Validates [IRI path][path].
///
/// [path]: https://tools.ietf.org/html/rfc3986#section-3.3
pub fn path<S: Spec>(s: &str) -> Result<(), Error> {
    parser::validate_path::<S>(s)
}

/// Validates [IRI query][query].
///
/// This validator corresponds to [`RiQueryStr`] and [`RiQueryString`] types.
///
/// Note that the first `?` character in an IRI is not a part of a query.
/// For example, `https://example.com/?foo#bar` has a query `foo`, **not** `?foo`.
///
/// # Examples
///
/// This type can have an IRI query.
/// Note that the IRI `foo://bar/baz?qux#quux` has the query `qux`, **not** `?qux`.
///
/// ```
/// use iri_string::{spec::UriSpec, validate::query};
/// assert!(query::<UriSpec>("").is_ok());
/// assert!(query::<UriSpec>("foo").is_ok());
/// assert!(query::<UriSpec>("foo/bar").is_ok());
/// assert!(query::<UriSpec>("/foo/bar").is_ok());
/// assert!(query::<UriSpec>("//foo/bar").is_ok());
/// assert!(query::<UriSpec>("https://user:pass@example.com:8080").is_ok());
/// assert!(query::<UriSpec>("https://example.com/").is_ok());
/// // Question sign `?` can appear in an IRI query.
/// assert!(query::<UriSpec>("query?again").is_ok());
/// ```
///
/// Some characters and sequences cannot used in a query.
///
/// ```
/// use iri_string::{spec::UriSpec, validate::query};
/// // `<` and `>` cannot directly appear in an IRI reference.
/// assert!(query::<UriSpec>("<not allowed>").is_err());
/// // Broken percent encoding cannot appear in an IRI reference.
/// assert!(query::<UriSpec>("%").is_err());
/// assert!(query::<UriSpec>("%GG").is_err());
/// // Hash sign `#` cannot appear in an IRI query.
/// assert!(query::<UriSpec>("#hash").is_err());
/// ```
///
/// [query]: https://tools.ietf.org/html/rfc3986#section-3.4
/// [`RiQueryStr`]: ../types/struct.RiQueryStr.html
/// [`RiQueryString`]: ../types/struct.RiQueryString.html
pub fn query<S: Spec>(s: &str) -> Result<(), Error> {
    parser::validate_query::<S>(s)
}

/// Validates [IRI fragment][fragment].
///
/// This validator corresponds to [`RiFragmentStr`] and [`RiFragmentString`] types.
///
/// Note that the first `#` character in an IRI is not a part of a fragment.
/// For example, `https://example.com/#foo` has a fragment `foo`, **not** `#foo`.
///
/// # Examples
///
/// This type can have an IRI fragment.
/// Note that the IRI `foo://bar/baz#qux` has the fragment `qux`, **not** `#qux`.
///
/// ```
/// use iri_string::{spec::UriSpec, validate::fragment};
/// assert!(fragment::<UriSpec>("").is_ok());
/// assert!(fragment::<UriSpec>("foo").is_ok());
/// assert!(fragment::<UriSpec>("foo/bar").is_ok());
/// assert!(fragment::<UriSpec>("/foo/bar").is_ok());
/// assert!(fragment::<UriSpec>("//foo/bar").is_ok());
/// assert!(fragment::<UriSpec>("https://user:pass@example.com:8080").is_ok());
/// assert!(fragment::<UriSpec>("https://example.com/").is_ok());
/// ```
///
/// Some characters and sequences cannot used in a fragment.
///
/// ```
/// use iri_string::{spec::UriSpec, validate::fragment};
/// // `<` and `>` cannot directly appear in an IRI reference.
/// assert!(fragment::<UriSpec>("<not allowed>").is_err());
/// // Broken percent encoding cannot appear in an IRI reference.
/// assert!(fragment::<UriSpec>("%").is_err());
/// assert!(fragment::<UriSpec>("%GG").is_err());
/// // Hash sign `#` cannot appear in an IRI fragment.
/// assert!(fragment::<UriSpec>("#hash").is_err());
/// ```
///
/// [fragment]: https://tools.ietf.org/html/rfc3986#section-3.5
/// [`RiFragmentStr`]: ../types/struct.RiFragmentStr.html
/// [`RiFragmentString`]: ../types/struct.RiFragmentString.html
pub fn fragment<S: Spec>(s: &str) -> Result<(), Error> {
    parser::validate_fragment::<S>(s)
}
