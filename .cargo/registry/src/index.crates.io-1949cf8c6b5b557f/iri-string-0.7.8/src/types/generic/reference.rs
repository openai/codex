//! IRI reference.

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::string::String;

use crate::components::AuthorityComponents;
#[cfg(feature = "alloc")]
use crate::mask_password::password_range_to_hide;
use crate::mask_password::PasswordMasked;
use crate::normalize::Normalized;
use crate::parser::trusted as trusted_parser;
#[cfg(feature = "alloc")]
use crate::raw;
use crate::resolve::FixedBaseResolver;
use crate::spec::Spec;
use crate::types::{RiAbsoluteStr, RiFragmentStr, RiQueryStr, RiRelativeStr, RiStr};
#[cfg(feature = "alloc")]
use crate::types::{RiRelativeString, RiString};
#[cfg(feature = "alloc")]
use crate::validate::iri;
use crate::validate::iri_reference;

define_custom_string_slice! {
    /// A borrowed string of an absolute IRI possibly with fragment part.
    ///
    /// This corresponds to [`IRI-reference` rule] in [RFC 3987]
    /// (and [`URI-reference` rule] in [RFC 3986]).
    /// The rule for `IRI-reference` is `IRI / irelative-ref`.
    /// In other words, this is union of [`RiStr`] and [`RiRelativeStr`].
    ///
    /// # Valid values
    ///
    /// This type can have an IRI reference (which can be absolute or relative).
    ///
    /// ```
    /// # use iri_string::types::IriReferenceStr;
    /// assert!(IriReferenceStr::new("https://user:pass@example.com:8080").is_ok());
    /// assert!(IriReferenceStr::new("https://example.com/").is_ok());
    /// assert!(IriReferenceStr::new("https://example.com/foo?bar=baz").is_ok());
    /// assert!(IriReferenceStr::new("https://example.com/foo?bar=baz#qux").is_ok());
    /// assert!(IriReferenceStr::new("foo:bar").is_ok());
    /// assert!(IriReferenceStr::new("foo:").is_ok());
    /// // `foo://.../` below are all allowed. See the crate documentation for detail.
    /// assert!(IriReferenceStr::new("foo:/").is_ok());
    /// assert!(IriReferenceStr::new("foo://").is_ok());
    /// assert!(IriReferenceStr::new("foo:///").is_ok());
    /// assert!(IriReferenceStr::new("foo:////").is_ok());
    /// assert!(IriReferenceStr::new("foo://///").is_ok());
    /// assert!(IriReferenceStr::new("foo/bar").is_ok());
    /// assert!(IriReferenceStr::new("/foo/bar").is_ok());
    /// assert!(IriReferenceStr::new("//foo/bar").is_ok());
    /// assert!(IriReferenceStr::new("#foo").is_ok());
    /// ```
    ///
    /// Some characters and sequences cannot used in an IRI reference.
    ///
    /// ```
    /// # use iri_string::types::IriReferenceStr;
    /// // `<` and `>` cannot directly appear in an IRI reference.
    /// assert!(IriReferenceStr::new("<not allowed>").is_err());
    /// // Broken percent encoding cannot appear in an IRI reference.
    /// assert!(IriReferenceStr::new("%").is_err());
    /// assert!(IriReferenceStr::new("%GG").is_err());
    /// ```
    ///
    /// [RFC 3986]: https://tools.ietf.org/html/rfc3986
    /// [RFC 3987]: https://tools.ietf.org/html/rfc3987
    /// [`IRI-reference` rule]: https://tools.ietf.org/html/rfc3987#section-2.2
    /// [`URI-reference` rule]: https://tools.ietf.org/html/rfc3986#section-4.1
    /// [`RiRelativeStr`]: struct.RiRelativeStr.html
    /// [`RiStr`]: struct.RiStr.html
    struct RiReferenceStr {
        validator = iri_reference,
        expecting_msg = "IRI reference string",
    }
}

#[cfg(feature = "alloc")]
define_custom_string_owned! {
    /// An owned string of an absolute IRI possibly with fragment part.
    ///
    /// This corresponds to [`IRI-reference` rule] in [RFC 3987]
    /// (and [`URI-reference` rule] in [RFC 3986]).
    /// The rule for `IRI-reference` is `IRI / irelative-ref`.
    /// In other words, this is union of [`RiString`] and [`RiRelativeString`].
    ///
    /// For details, see the document for [`RiReferenceStr`].
    ///
    /// Enabled by `alloc` or `std` feature.
    ///
    /// [RFC 3986]: https://tools.ietf.org/html/rfc3986
    /// [RFC 3987]: https://tools.ietf.org/html/rfc3987
    /// [`IRI-reference` rule]: https://tools.ietf.org/html/rfc3987#section-2.2
    /// [`URI-reference` rule]: https://tools.ietf.org/html/rfc3986#section-4.1
    /// [`RiReferenceStr`]: struct.RiReferenceString.html
    /// [`RiRelativeString`]: struct.RiRelativeString.html
    /// [`RiString`]: struct.RiString.html
    struct RiReferenceString {
        validator = iri_reference,
        slice = RiReferenceStr,
        expecting_msg = "IRI reference string",
    }
}

impl<S: Spec> RiReferenceStr<S> {
    /// Returns the string as [`&RiStr`][`RiStr`], if it is valid as an IRI.
    ///
    /// If it is not an IRI, then [`&RiRelativeStr`][`RiRelativeStr`] is returned as `Err(_)`.
    ///
    /// [`RiRelativeStr`]: struct.RiRelativeStr.html
    /// [`RiStr`]: struct.RiStr.html
    pub fn to_iri(&self) -> Result<&RiStr<S>, &RiRelativeStr<S>> {
        // Check with `IRI` rule first, because the syntax rule for `IRI-reference` is
        // `IRI / irelative-ref`.
        //
        // > Some productions are ambiguous. The "first-match-wins" (a.k.a.
        // > "greedy") algorithm applies. For details, see [RFC3986].
        // >
        // > --- <https://tools.ietf.org/html/rfc3987#section-2.2>.

        <&RiStr<S>>::try_from(self.as_str()).map_err(|_| {
            // SAFETY: if an IRI reference is not an IRI, then it is a relative IRI.
            // See the RFC 3987 syntax rule `IRI-reference = IRI / irelative-ref`.
            unsafe { RiRelativeStr::new_maybe_unchecked(self.as_str()) }
        })
    }

    /// Returns the string as [`&RiRelativeStr`][`RiRelativeStr`], if it is valid as an IRI.
    ///
    /// If it is not an IRI, then [`&RiStr`][`RiStr`] is returned as `Err(_)`.
    ///
    /// [`RiRelativeStr`]: struct.RiRelativeStr.html
    /// [`RiStr`]: struct.RiStr.html
    pub fn to_relative_iri(&self) -> Result<&RiRelativeStr<S>, &RiStr<S>> {
        match self.to_iri() {
            Ok(iri) => Err(iri),
            Err(relative) => Ok(relative),
        }
    }

    /// Returns resolved IRI against the given base IRI.
    ///
    /// For IRI reference resolution output examples, see [RFC 3986 section 5.4].
    ///
    /// If you are going to resolve multiple references against the common base,
    /// consider using [`FixedBaseResolver`].
    ///
    /// # Strictness
    ///
    /// The IRI parsers provided by this crate is strict (e.g. `http:g` is
    /// always interpreted as a composition of the scheme `http` and the path
    /// `g`), so backward compatible parsing and resolution are not provided.
    /// About parser and resolver strictness, see [RFC 3986 section 5.4.2]:
    ///
    /// > Some parsers allow the scheme name to be present in a relative
    /// > reference if it is the same as the base URI scheme. This is considered
    /// > to be a loophole in prior specifications of partial URI
    /// > [RFC1630](https://tools.ietf.org/html/rfc1630). Its use should be
    /// > avoided but is allowed for backward compatibility.
    /// >
    /// > --- <https://tools.ietf.org/html/rfc3986#section-5.4.2>
    ///
    /// # Failures
    ///
    /// This method itself does not fail, but IRI resolution without WHATWG URL
    /// Standard serialization can fail in some minor cases.
    ///
    /// To see examples of such unresolvable IRIs, visit the documentation
    /// for [`normalize`][`crate::normalize`] module.
    ///
    /// [RFC 3986 section 5.4]: https://tools.ietf.org/html/rfc3986#section-5.4
    /// [RFC 3986 section 5.4.2]: https://tools.ietf.org/html/rfc3986#section-5.4.2
    pub fn resolve_against<'a>(&'a self, base: &'a RiAbsoluteStr<S>) -> Normalized<'a, RiStr<S>> {
        FixedBaseResolver::new(base).resolve(self.as_ref())
    }

    /// Returns the proxy to the IRI with password masking feature.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("http://user:password@example.com/path?query")?;
    /// let masked = iri.mask_password();
    /// assert_eq!(masked.to_dedicated_string(), "http://user:@example.com/path?query");
    ///
    /// assert_eq!(
    ///     masked.replace_password("${password}").to_string(),
    ///     "http://user:${password}@example.com/path?query"
    /// );
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn mask_password(&self) -> PasswordMasked<'_, Self> {
        PasswordMasked::new(self)
    }
}

/// Components getters.
impl<S: Spec> RiReferenceStr<S> {
    /// Returns the scheme.
    ///
    /// The following colon is truncated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("http://example.com/pathpath?queryquery#fragfrag")?;
    /// assert_eq!(iri.scheme_str(), Some("http"));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("foo/bar:baz")?;
    /// assert_eq!(iri.scheme_str(), None);
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn scheme_str(&self) -> Option<&str> {
        trusted_parser::extract_scheme(self.as_str())
    }

    /// Returns the authority.
    ///
    /// The leading `//` is truncated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("http://example.com/pathpath?queryquery#fragfrag")?;
    /// assert_eq!(iri.authority_str(), Some("example.com"));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("urn:uuid:10db315b-fcd1-4428-aca8-15babc9a2da2")?;
    /// assert_eq!(iri.authority_str(), None);
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("foo/bar:baz")?;
    /// assert_eq!(iri.authority_str(), None);
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn authority_str(&self) -> Option<&str> {
        trusted_parser::extract_authority(self.as_str())
    }

    /// Returns the path.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("http://example.com/pathpath?queryquery#fragfrag")?;
    /// assert_eq!(iri.path_str(), "/pathpath");
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("urn:uuid:10db315b-fcd1-4428-aca8-15babc9a2da2")?;
    /// assert_eq!(iri.path_str(), "uuid:10db315b-fcd1-4428-aca8-15babc9a2da2");
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("foo/bar:baz")?;
    /// assert_eq!(iri.path_str(), "foo/bar:baz");
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn path_str(&self) -> &str {
        trusted_parser::extract_path(self.as_str())
    }

    /// Returns the query.
    ///
    /// The leading question mark (`?`) is truncated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::{IriQueryStr, IriReferenceStr};
    ///
    /// let iri = IriReferenceStr::new("http://example.com/pathpath?queryquery#fragfrag")?;
    /// let query = IriQueryStr::new("queryquery")?;
    /// assert_eq!(iri.query(), Some(query));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("urn:uuid:10db315b-fcd1-4428-aca8-15babc9a2da2")?;
    /// assert_eq!(iri.query(), None);
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::{IriQueryStr, IriReferenceStr};
    ///
    /// let iri = IriReferenceStr::new("foo/bar:baz?")?;
    /// let query = IriQueryStr::new("")?;
    /// assert_eq!(iri.query(), Some(query));
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn query(&self) -> Option<&RiQueryStr<S>> {
        trusted_parser::extract_query(self.as_str()).map(|query| {
            // SAFETY: `extract_query` returns the query part of an IRI, and the
            // returned string should have only valid characters since is the
            // substring of the source IRI.
            unsafe { RiQueryStr::new_maybe_unchecked(query) }
        })
    }

    /// Returns the query as a raw string slice.
    ///
    /// The leading question mark (`?`) is truncated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("http://example.com/pathpath?queryquery#fragfrag")?;
    /// assert_eq!(iri.query_str(), Some("queryquery"));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("urn:uuid:10db315b-fcd1-4428-aca8-15babc9a2da2")?;
    /// assert_eq!(iri.query_str(), None);
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("foo/bar:baz?")?;
    /// assert_eq!(iri.query_str(), Some(""));
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn query_str(&self) -> Option<&str> {
        trusted_parser::extract_query(self.as_str())
    }

    /// Returns the fragment part if exists.
    ///
    /// A leading `#` character is truncated if the fragment part exists.
    ///
    /// # Examples
    ///
    /// If the IRI has a fragment part, `Some(_)` is returned.
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::{IriFragmentStr, IriReferenceStr}, validate::Error};
    /// let iri = IriReferenceStr::new("foo://bar/baz?qux=quux#corge")?;
    /// let fragment = IriFragmentStr::new("corge")?;
    /// assert_eq!(iri.fragment(), Some(fragment));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::{IriFragmentStr, IriReferenceStr}, validate::Error};
    /// let iri = IriReferenceStr::new("#foo")?;
    /// let fragment = IriFragmentStr::new("foo")?;
    /// assert_eq!(iri.fragment(), Some(fragment));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// When the fragment part exists but is empty string, `Some(_)` is returned.
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::{IriFragmentStr, IriReferenceStr}, validate::Error};
    /// let iri = IriReferenceStr::new("foo://bar/baz?qux=quux#")?;
    /// let fragment = IriFragmentStr::new("")?;
    /// assert_eq!(iri.fragment(), Some(fragment));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::{IriFragmentStr, IriReferenceStr}, validate::Error};
    /// let iri = IriReferenceStr::new("#")?;
    /// let fragment = IriFragmentStr::new("")?;
    /// assert_eq!(iri.fragment(), Some(fragment));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// If the IRI has no fragment, `None` is returned.
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriReferenceStr, validate::Error};
    /// let iri = IriReferenceStr::new("foo://bar/baz?qux=quux")?;
    /// assert_eq!(iri.fragment(), None);
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriReferenceStr, validate::Error};
    /// let iri = IriReferenceStr::new("")?;
    /// assert_eq!(iri.fragment(), None);
    /// # Ok::<_, Error>(())
    /// ```
    #[must_use]
    pub fn fragment(&self) -> Option<&RiFragmentStr<S>> {
        trusted_parser::extract_fragment(self.as_str()).map(|fragment| {
            // SAFETY: `extract_fragment` returns the fragment part of an IRI,
            // and the returned string should have only valid characters since
            // is the substring of the source IRI.
            unsafe { RiFragmentStr::new_maybe_unchecked(fragment) }
        })
    }

    /// Returns the fragment part as a raw string slice if exists.
    ///
    /// A leading `#` character is truncated if the fragment part exists.
    ///
    /// # Examples
    ///
    /// If the IRI has a fragment part, `Some(_)` is returned.
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriReferenceStr, validate::Error};
    /// let iri = IriReferenceStr::new("foo://bar/baz?qux=quux#corge")?;
    /// assert_eq!(iri.fragment_str(), Some("corge"));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriReferenceStr, validate::Error};
    /// let iri = IriReferenceStr::new("#foo")?;
    /// assert_eq!(iri.fragment_str(), Some("foo"));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// When the fragment part exists but is empty string, `Some(_)` is returned.
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriReferenceStr, validate::Error};
    /// let iri = IriReferenceStr::new("foo://bar/baz?qux=quux#")?;
    /// assert_eq!(iri.fragment_str(), Some(""));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriReferenceStr, validate::Error};
    /// let iri = IriReferenceStr::new("#")?;
    /// assert_eq!(iri.fragment_str(), Some(""));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// If the IRI has no fragment, `None` is returned.
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriReferenceStr, validate::Error};
    /// let iri = IriReferenceStr::new("foo://bar/baz?qux=quux")?;
    /// assert_eq!(iri.fragment(), None);
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriReferenceStr, validate::Error};
    /// let iri = IriReferenceStr::new("")?;
    /// assert_eq!(iri.fragment(), None);
    /// # Ok::<_, Error>(())
    /// ```
    #[must_use]
    pub fn fragment_str(&self) -> Option<&str> {
        trusted_parser::extract_fragment(self.as_str())
    }

    /// Returns the authority components.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("http://user:pass@example.com:8080/pathpath?queryquery")?;
    /// let authority = iri.authority_components()
    ///     .expect("authority is available");
    /// assert_eq!(authority.userinfo(), Some("user:pass"));
    /// assert_eq!(authority.host(), "example.com");
    /// assert_eq!(authority.port(), Some("8080"));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriReferenceStr;
    ///
    /// let iri = IriReferenceStr::new("foo//bar:baz")?;
    /// assert_eq!(iri.authority_str(), None);
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn authority_components(&self) -> Option<AuthorityComponents<'_>> {
        AuthorityComponents::from_iri(self)
    }
}

#[cfg(feature = "alloc")]
impl<S: Spec> RiReferenceString<S> {
    /// Returns the string as [`RiString`], if it is valid as an IRI.
    ///
    /// If it is not an IRI, then [`RiRelativeString`] is returned as `Err(_)`.
    ///
    /// [`RiRelativeString`]: struct.RiRelativeString.html
    /// [`RiString`]: struct.RiString.html
    pub fn into_iri(self) -> Result<RiString<S>, RiRelativeString<S>> {
        let s: String = self.into();
        // Check with `IRI` rule first, because of the syntax.
        //
        // > Some productions are ambiguous. The "first-match-wins" (a.k.a.
        // > "greedy") algorithm applies. For details, see [RFC3986].
        // >
        // > --- <https://tools.ietf.org/html/rfc3987#section-2.2>.
        if iri::<S>(&s).is_ok() {
            // SAFETY: just checked `s` is valid as an IRI.
            Ok(unsafe { RiString::new_always_unchecked(s) })
        } else {
            // SAFETY: if an IRI reference is not an IRI, then it is a relative IRI.
            // See the RFC 3987 syntax rule `IRI-reference = IRI / irelative-ref`.
            Err(unsafe { RiRelativeString::new_maybe_unchecked(s) })
        }
    }

    /// Returns the string as [`RiRelativeString`], if it is valid as an IRI.
    ///
    /// If it is not an IRI, then [`RiString`] is returned as `Err(_)`.
    ///
    /// [`RiRelativeString`]: struct.RiRelativeString.html
    /// [`RiString`]: struct.RiString.html
    pub fn into_relative_iri(self) -> Result<RiRelativeString<S>, RiString<S>> {
        match self.into_iri() {
            Ok(iri) => Err(iri),
            Err(relative) => Ok(relative),
        }
    }

    /// Sets the fragment part to the given string.
    ///
    /// Removes fragment part (and following `#` character) if `None` is given.
    pub fn set_fragment(&mut self, fragment: Option<&RiFragmentStr<S>>) {
        raw::set_fragment(&mut self.inner, fragment.map(AsRef::as_ref));
        debug_assert!(iri_reference::<S>(&self.inner).is_ok());
    }

    /// Removes the password completely (including separator colon) from `self` even if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::types::IriReferenceString;
    ///
    /// let mut iri = IriReferenceString::try_from("http://user:password@example.com/path?query")?;
    /// iri.remove_password_inline();
    /// assert_eq!(iri, "http://user@example.com/path?query");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// Even if the password is empty, the password and separator will be removed.
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::types::IriReferenceString;
    ///
    /// let mut iri = IriReferenceString::try_from("http://user:@example.com/path?query")?;
    /// iri.remove_password_inline();
    /// assert_eq!(iri, "http://user@example.com/path?query");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    pub fn remove_password_inline(&mut self) {
        let pw_range = match password_range_to_hide(self.as_slice()) {
            Some(v) => v,
            None => return,
        };
        let separator_colon = pw_range.start - 1;
        // SAFETY: the IRI must be valid after the password component and
        // the leading separator colon is removed.
        unsafe {
            let buf = self.as_inner_mut();
            buf.drain(separator_colon..pw_range.end);
            debug_assert!(
                RiReferenceStr::<S>::new(buf).is_ok(),
                "[validity] the IRI must be valid after the password component is removed"
            );
        }
    }

    /// Replaces the non-empty password in `self` to the empty password.
    ///
    /// This leaves the separator colon if the password part was available.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::types::IriReferenceString;
    ///
    /// let mut iri = IriReferenceString::try_from("http://user:password@example.com/path?query")?;
    /// iri.remove_nonempty_password_inline();
    /// assert_eq!(iri, "http://user:@example.com/path?query");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// If the password is empty, it is left as is.
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::types::IriReferenceString;
    ///
    /// let mut iri = IriReferenceString::try_from("http://user:@example.com/path?query")?;
    /// iri.remove_nonempty_password_inline();
    /// assert_eq!(iri, "http://user:@example.com/path?query");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    pub fn remove_nonempty_password_inline(&mut self) {
        let pw_range = match password_range_to_hide(self.as_slice()) {
            Some(v) if !v.is_empty() => v,
            _ => return,
        };
        debug_assert_eq!(
            self.as_str().as_bytes().get(pw_range.start - 1).copied(),
            Some(b':'),
            "[validity] the password component must be prefixed with a separator colon"
        );
        // SAFETY: the IRI must be valid after the password component is
        // replaced with the empty password.
        unsafe {
            let buf = self.as_inner_mut();
            buf.drain(pw_range);
            debug_assert!(
                RiReferenceStr::<S>::new(buf).is_ok(),
                "[validity] the IRI must be valid after the password component \
                 is replaced with the empty password"
            );
        }
    }
}
