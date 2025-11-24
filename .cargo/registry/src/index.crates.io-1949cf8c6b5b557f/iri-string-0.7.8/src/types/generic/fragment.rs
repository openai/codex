//! Fragment string.

use crate::{
    spec::Spec,
    validate::{fragment, Error},
};

define_custom_string_slice! {
    /// A borrowed slice of an IRI fragment (i.e. after the first `#` character).
    ///
    /// This corresponds to [`ifragment` rule] in [RFC 3987] (and [`fragment` rule] in [RFC 3986]).
    /// The rule for `ifragment` is `*( ipchar / "/" / "?" )`.
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
    ///
    /// [RFC 3986]: https://tools.ietf.org/html/rfc3986
    /// [RFC 3987]: https://tools.ietf.org/html/rfc3987
    /// [`fragment` rule]: https://tools.ietf.org/html/rfc3986#section-3.5
    /// [`ifragment` rule]: https://tools.ietf.org/html/rfc3987#section-2.2
    struct RiFragmentStr {
        validator = fragment,
        expecting_msg = "IRI fragment string",
    }
}

#[cfg(feature = "alloc")]
define_custom_string_owned! {
    /// An owned string of an IRI fragment (i.e. after the first `#` character).
    ///
    /// This corresponds to [`ifragment` rule] in [RFC 3987] (and [`fragment` rule] in [RFC 3986]).
    /// The rule for `absolute-IRI` is `*( ipchar / "/" / "?" )`.
    ///
    /// For details, see the documentation for [`RiFragmentStr`].
    ///
    /// Enabled by `alloc` or `std` feature.
    ///
    /// [RFC 3986]: https://tools.ietf.org/html/rfc3986
    /// [RFC 3987]: https://tools.ietf.org/html/rfc3987
    /// [`fragment` rule]: https://tools.ietf.org/html/rfc3986#section-3.5
    /// [`ifragment` rule]: https://tools.ietf.org/html/rfc3987#section-2.2
    /// [`RiFragmentStr`]: struct.RiFragmentStr.html
    struct RiFragmentString {
        validator = fragment,
        slice = RiFragmentStr,
        expecting_msg = "IRI fragment string",
    }
}

impl<S: Spec> RiFragmentStr<S> {
    /// Creates a new `&RiFragmentStr` from the fragment part prefixed by `#`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::types::IriFragmentStr;
    /// assert!(IriFragmentStr::from_prefixed("#").is_ok());
    /// assert!(IriFragmentStr::from_prefixed("#foo").is_ok());
    /// assert!(IriFragmentStr::from_prefixed("#foo/bar").is_ok());
    /// assert!(IriFragmentStr::from_prefixed("#/foo/bar").is_ok());
    /// assert!(IriFragmentStr::from_prefixed("#//foo/bar").is_ok());
    /// assert!(IriFragmentStr::from_prefixed("#https://user:pass@example.com:8080").is_ok());
    /// assert!(IriFragmentStr::from_prefixed("#https://example.com/").is_ok());
    ///
    /// // `<` and `>` cannot directly appear in an IRI.
    /// assert!(IriFragmentStr::from_prefixed("#<not allowed>").is_err());
    /// // Broken percent encoding cannot appear in an IRI.
    /// assert!(IriFragmentStr::new("#%").is_err());
    /// assert!(IriFragmentStr::new("#%GG").is_err());
    /// // `#` prefix is expected.
    /// assert!(IriFragmentStr::from_prefixed("").is_err());
    /// assert!(IriFragmentStr::from_prefixed("foo").is_err());
    /// // Hash sign `#` cannot appear in an IRI fragment.
    /// assert!(IriFragmentStr::from_prefixed("##hash").is_err());
    /// ```
    pub fn from_prefixed(s: &str) -> Result<&Self, Error> {
        if !s.starts_with('#') {
            return Err(Error::new());
        }
        TryFrom::try_from(&s[1..])
    }
}
