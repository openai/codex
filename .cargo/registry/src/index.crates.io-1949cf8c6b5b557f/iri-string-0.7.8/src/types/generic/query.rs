//! Query string.

use crate::{
    spec::Spec,
    validate::{query, Error},
};

define_custom_string_slice! {
    /// A borrowed slice of an IRI query (i.e. after the first `?` and before the first `#`).
    ///
    /// This corresponds to [`iquery` rule] in [RFC 3987] (and [`query` rule] in [RFC 3986]).
    /// The rule for `ifragment` is `*( ipchar / iprivate / "/" / "?" )`.
    ///
    /// # Valid values
    ///
    /// This type can have an IRI fragment.
    /// Note that the IRI `foo://bar/baz#qux` has the fragment `qux`, **not** `#qux`.
    ///
    /// ```
    /// # use iri_string::types::IriFragmentStr;
    /// assert!(IriFragmentStr::new("").is_ok());
    /// assert!(IriFragmentStr::new("foo").is_ok());
    /// assert!(IriFragmentStr::new("foo/bar").is_ok());
    /// assert!(IriFragmentStr::new("/foo/bar").is_ok());
    /// assert!(IriFragmentStr::new("//foo/bar").is_ok());
    /// assert!(IriFragmentStr::new("https://user:pass@example.com:8080").is_ok());
    /// assert!(IriFragmentStr::new("https://example.com/").is_ok());
    /// ```
    ///
    /// Some characters and sequences cannot used in a fragment.
    ///
    /// ```
    /// # use iri_string::types::IriFragmentStr;
    /// // `<` and `>` cannot directly appear in an IRI reference.
    /// assert!(IriFragmentStr::new("<not allowed>").is_err());
    /// // Broken percent encoding cannot appear in an IRI reference.
    /// assert!(IriFragmentStr::new("%").is_err());
    /// assert!(IriFragmentStr::new("%GG").is_err());
    /// // Hash sign `#` cannot appear in an IRI fragment.
    /// assert!(IriFragmentStr::new("#hash").is_err());
    /// ```
    /// ```
    /// use iri_string::types::IriQueryStr;
    /// assert!(IriQueryStr::new("").is_ok());
    /// assert!(IriQueryStr::new("foo").is_ok());
    /// assert!(IriQueryStr::new("foo/bar").is_ok());
    /// assert!(IriQueryStr::new("/foo/bar").is_ok());
    /// assert!(IriQueryStr::new("//foo/bar").is_ok());
    /// assert!(IriQueryStr::new("https://user:pass@example.com:8080").is_ok());
    /// assert!(IriQueryStr::new("https://example.com/").is_ok());
    /// // Question sign `?` can appear in an IRI query.
    /// assert!(IriQueryStr::new("query?again").is_ok());
    /// ```
    ///
    /// Some characters and sequences cannot used in a query.
    ///
    /// ```
    /// use iri_string::types::IriQueryStr;
    /// // `<` and `>` cannot directly appear in an IRI reference.
    /// assert!(IriQueryStr::new("<not allowed>").is_err());
    /// // Broken percent encoding cannot appear in an IRI reference.
    /// assert!(IriQueryStr::new("%").is_err());
    /// assert!(IriQueryStr::new("%GG").is_err());
    /// // Hash sign `#` cannot appear in an IRI query.
    /// assert!(IriQueryStr::new("#hash").is_err());
    /// ```
    ///
    /// [RFC 3986]: https://tools.ietf.org/html/rfc3986
    /// [RFC 3987]: https://tools.ietf.org/html/rfc3987
    /// [`query` rule]: https://tools.ietf.org/html/rfc3986#section-3.4
    /// [`iquery` rule]: https://tools.ietf.org/html/rfc3987#section-2.2
    struct RiQueryStr {
        validator = query,
        expecting_msg = "IRI query string",
    }
}

#[cfg(feature = "alloc")]
define_custom_string_owned! {
    /// An owned string of an IRI fragment (i.e. after the first `#` character).
    ///
    /// This corresponds to [`iquery` rule] in [RFC 3987] (and [`query` rule] in [RFC 3986]).
    /// The rule for `absolute-IRI` is `*( ipchar / iprivate / "/" / "?" )`.
    ///
    /// For details, see the documentation for [`RiQueryStr`].
    ///
    /// Enabled by `alloc` or `std` feature.
    ///
    /// [RFC 3986]: https://tools.ietf.org/html/rfc3986
    /// [RFC 3987]: https://tools.ietf.org/html/rfc3987
    /// [`query` rule]: https://tools.ietf.org/html/rfc3986#section-3.4
    /// [`iquery` rule]: https://tools.ietf.org/html/rfc3987#section-2.2
    /// [`RiQueryStr`]: struct.RiQueryStr.html
    struct RiQueryString {
        validator = query,
        slice = RiQueryStr,
        expecting_msg = "IRI query string",
    }
}

impl<S: Spec> RiQueryStr<S> {
    /// Creates a new `&RiQueryStr` from the query part prefixed by `?`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::types::IriQueryStr;
    /// assert!(IriQueryStr::from_prefixed("?").is_ok());
    /// assert!(IriQueryStr::from_prefixed("?foo").is_ok());
    /// assert!(IriQueryStr::from_prefixed("?foo/bar").is_ok());
    /// assert!(IriQueryStr::from_prefixed("?/foo/bar").is_ok());
    /// assert!(IriQueryStr::from_prefixed("?//foo/bar").is_ok());
    /// assert!(IriQueryStr::from_prefixed("?https://user:pass@example.com:8080").is_ok());
    /// assert!(IriQueryStr::from_prefixed("?https://example.com/").is_ok());
    /// // Question sign `?` can appear in an IRI query.
    /// assert!(IriQueryStr::from_prefixed("?query?again").is_ok());
    ///
    /// // `<` and `>` cannot directly appear in an IRI.
    /// assert!(IriQueryStr::from_prefixed("?<not allowed>").is_err());
    /// // Broken percent encoding cannot appear in an IRI.
    /// assert!(IriQueryStr::new("?%").is_err());
    /// assert!(IriQueryStr::new("?%GG").is_err());
    /// // `?` prefix is expected.
    /// assert!(IriQueryStr::from_prefixed("").is_err());
    /// assert!(IriQueryStr::from_prefixed("foo").is_err());
    /// // Hash sign `#` cannot appear in an IRI query.
    /// assert!(IriQueryStr::from_prefixed("?#hash").is_err());
    /// ```
    pub fn from_prefixed(s: &str) -> Result<&Self, Error> {
        if !s.starts_with('?') {
            return Err(Error::new());
        }
        TryFrom::try_from(&s[1..])
    }
}
