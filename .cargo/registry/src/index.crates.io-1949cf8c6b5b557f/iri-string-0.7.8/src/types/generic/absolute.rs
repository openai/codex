//! Absolute IRI (without fragment part).

use crate::components::AuthorityComponents;
#[cfg(feature = "alloc")]
use crate::mask_password::password_range_to_hide;
use crate::mask_password::PasswordMasked;
use crate::normalize::{Error, NormalizationInput, Normalized, NormalizednessCheckMode};
use crate::parser::trusted as trusted_parser;
use crate::spec::Spec;
use crate::types::{RiQueryStr, RiReferenceStr, RiStr};
#[cfg(feature = "alloc")]
use crate::types::{RiReferenceString, RiString};
use crate::validate::absolute_iri;

define_custom_string_slice! {
    /// A borrowed slice of an absolute IRI without fragment part.
    ///
    /// This corresponds to [`absolute-IRI` rule] in [RFC 3987]
    /// (and [`absolute-URI` rule] in [RFC 3986]).
    /// In other words, this is [`RiStr`] without fragment part.
    ///
    /// If you want to accept fragment part, use [`RiStr`].
    ///
    /// # Valid values
    ///
    /// This type can have an absolute IRI without fragment part.
    ///
    /// ```
    /// # use iri_string::types::IriAbsoluteStr;
    /// assert!(IriAbsoluteStr::new("https://example.com/foo?bar=baz").is_ok());
    /// assert!(IriAbsoluteStr::new("foo:bar").is_ok());
    /// // Scheme `foo` and empty path.
    /// assert!(IriAbsoluteStr::new("foo:").is_ok());
    /// // `foo://.../` below are all allowed. See the crate documentation for detail.
    /// assert!(IriAbsoluteStr::new("foo:/").is_ok());
    /// assert!(IriAbsoluteStr::new("foo://").is_ok());
    /// assert!(IriAbsoluteStr::new("foo:///").is_ok());
    /// assert!(IriAbsoluteStr::new("foo:////").is_ok());
    /// assert!(IriAbsoluteStr::new("foo://///").is_ok());
    ///
    /// ```
    ///
    /// Relative IRI is not allowed.
    ///
    /// ```
    /// # use iri_string::types::IriAbsoluteStr;
    /// // This is relative path.
    /// assert!(IriAbsoluteStr::new("foo/bar").is_err());
    /// // `/foo/bar` is an absolute path, but it is authority-relative.
    /// assert!(IriAbsoluteStr::new("/foo/bar").is_err());
    /// // `//foo/bar` is termed "network-path reference",
    /// // or usually called "protocol-relative reference".
    /// assert!(IriAbsoluteStr::new("//foo/bar").is_err());
    /// // Empty string is not a valid absolute IRI.
    /// assert!(IriAbsoluteStr::new("").is_err());
    /// ```
    ///
    /// Fragment part (such as trailing `#foo`) is not allowed.
    ///
    /// ```
    /// # use iri_string::types::IriAbsoluteStr;
    /// // Fragment part is not allowed.
    /// assert!(IriAbsoluteStr::new("https://example.com/foo?bar=baz#qux").is_err());
    /// ```
    ///
    /// Some characters and sequences cannot used in an absolute IRI.
    ///
    /// ```
    /// # use iri_string::types::IriAbsoluteStr;
    /// // `<` and `>` cannot directly appear in an absolute IRI.
    /// assert!(IriAbsoluteStr::new("<not allowed>").is_err());
    /// // Broken percent encoding cannot appear in an absolute IRI.
    /// assert!(IriAbsoluteStr::new("%").is_err());
    /// assert!(IriAbsoluteStr::new("%GG").is_err());
    /// ```
    ///
    /// [RFC 3986]: https://tools.ietf.org/html/rfc3986
    /// [RFC 3987]: https://tools.ietf.org/html/rfc3987
    /// [`absolute-IRI` rule]: https://tools.ietf.org/html/rfc3987#section-2.2
    /// [`absolute-URI` rule]: https://tools.ietf.org/html/rfc3986#section-4.3
    /// [`RiStr`]: struct.RiStr.html
    struct RiAbsoluteStr {
        validator = absolute_iri,
        expecting_msg = "Absolute IRI string",
    }
}

#[cfg(feature = "alloc")]
define_custom_string_owned! {
    /// An owned string of an absolute IRI without fragment part.
    ///
    /// This corresponds to [`absolute-IRI` rule] in [RFC 3987]
    /// (and [`absolute-URI` rule] in [RFC 3986]).
    /// The rule for `absolute-IRI` is `scheme ":" ihier-part [ "?" iquery ]`.
    /// In other words, this is [`RiString`] without fragment part.
    ///
    /// If you want to accept fragment part, use [`RiString`].
    ///
    /// For details, see the document for [`RiAbsoluteStr`].
    ///
    /// Enabled by `alloc` or `std` feature.
    ///
    /// [RFC 3986]: https://tools.ietf.org/html/rfc3986
    /// [RFC 3987]: https://tools.ietf.org/html/rfc3987
    /// [`absolute-IRI` rule]: https://tools.ietf.org/html/rfc3987#section-2.2
    /// [`absolute-URI` rule]: https://tools.ietf.org/html/rfc3986#section-4.3
    /// [`RiAbsoluteStr`]: struct.RiAbsoluteStr.html
    /// [`RiString`]: struct.RiString.html
    struct RiAbsoluteString {
        validator = absolute_iri,
        slice = RiAbsoluteStr,
        expecting_msg = "Absolute IRI string",
    }
}

impl<S: Spec> RiAbsoluteStr<S> {
    /// Returns Ok`(())` if the IRI is normalizable by the RFC 3986 algorithm.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("HTTP://example.COM/foo/%2e/bar/..")?;
    /// assert!(iri.ensure_rfc3986_normalizable().is_ok());
    ///
    /// let iri2 = IriAbsoluteStr::new("scheme:/..//bar")?;
    /// // The normalization result would be `scheme://bar` according to RFC
    /// // 3986, but it is unintended and should be treated as a failure.
    /// // This crate automatically handles this case so that `.normalize()` won't fail.
    /// assert!(!iri.ensure_rfc3986_normalizable().is_err());
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    pub fn ensure_rfc3986_normalizable(&self) -> Result<(), Error> {
        NormalizationInput::from(self).ensure_rfc3986_normalizable()
    }

    /// Returns `true` if the IRI is already normalized.
    ///
    /// This returns the same result as `self.normalize().to_string() == self`,
    /// but does this more efficiently without heap allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("HTTP://example.COM/foo/./bar/%2e%2e/../baz?query")?;
    /// assert!(!iri.is_normalized());
    ///
    /// let normalized = iri.normalize().to_dedicated_string();
    /// assert_eq!(normalized, "http://example.com/baz?query");
    /// assert!(normalized.is_normalized());
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("scheme:/.///foo")?;
    /// // Already normalized.
    /// assert!(iri.is_normalized());
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("scheme:relative/..//not-a-host")?;
    /// // Default normalization algorithm assumes the path part to be NOT opaque.
    /// assert!(!iri.is_normalized());
    ///
    /// let normalized = iri.normalize().to_dedicated_string();
    /// assert_eq!(normalized, "scheme:/.//not-a-host");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn is_normalized(&self) -> bool {
        trusted_parser::is_normalized::<S>(self.as_str(), NormalizednessCheckMode::Default)
    }

    /// Returns `true` if the IRI is already normalized.
    ///
    /// This returns the same result as
    /// `self.ensure_rfc3986_normalizable() && (self.normalize().to_string() == self)`,
    /// does this more efficiently without heap allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("HTTP://example.COM/foo/./bar/%2e%2e/../baz?query")?;
    /// assert!(!iri.is_normalized_rfc3986());
    ///
    /// let normalized = iri.normalize().to_dedicated_string();
    /// assert_eq!(normalized, "http://example.com/baz?query");
    /// assert!(normalized.is_normalized_rfc3986());
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("scheme:/.///foo")?;
    /// // Not normalized in the sense of RFC 3986.
    /// assert!(!iri.is_normalized_rfc3986());
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("scheme:relative/..//not-a-host")?;
    /// // RFC 3986 normalization algorithm assumes the path part to be NOT opaque.
    /// assert!(!iri.is_normalized_rfc3986());
    ///
    /// let normalized = iri.normalize().to_dedicated_string();
    /// assert_eq!(normalized, "scheme:/.//not-a-host");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn is_normalized_rfc3986(&self) -> bool {
        trusted_parser::is_normalized::<S>(self.as_str(), NormalizednessCheckMode::Rfc3986)
    }

    /// Returns `true` if the IRI is already normalized in the sense of
    /// [`normalize_but_preserve_authorityless_relative_path`] method.
    ///
    /// This returns the same result as
    /// `self.normalize_but_preserve_authorityless_relative_path().to_string() == self`,
    /// but does this more efficiently without heap allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("HTTP://example.COM/foo/./bar/%2e%2e/../baz?query")?;
    /// assert!(!iri.is_normalized_but_authorityless_relative_path_preserved());
    ///
    /// let normalized = iri
    ///     .normalize_but_preserve_authorityless_relative_path()
    ///     .to_dedicated_string();
    /// assert_eq!(normalized, "http://example.com/baz?query");
    /// assert!(normalized.is_normalized());
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("scheme:/.///foo")?;
    /// // Already normalized in the sense of
    /// // `normalize_but_opaque_authorityless_relative_path()` method.
    /// assert!(iri.is_normalized_but_authorityless_relative_path_preserved());
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("scheme:relative/..//not-a-host")?;
    /// // Relative path is treated as opaque since the autority component is absent.
    /// assert!(iri.is_normalized_but_authorityless_relative_path_preserved());
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// [`normalize_but_preserve_authorityless_relative_path`]:
    ///     `Self::normalize_but_preserve_authorityless_relative_path`
    #[inline]
    #[must_use]
    pub fn is_normalized_but_authorityless_relative_path_preserved(&self) -> bool {
        trusted_parser::is_normalized::<S>(
            self.as_str(),
            NormalizednessCheckMode::PreserveAuthoritylessRelativePath,
        )
    }

    /// Returns the normalized IRI.
    ///
    /// # Notes
    ///
    /// For some abnormal IRIs, the normalization can produce semantically
    /// incorrect string that looks syntactically valid. To avoid security
    /// issues by this trap, the normalization algorithm by this crate
    /// automatically applies the workaround.
    ///
    /// If you worry about this, test by
    /// [`RiAbsoluteStr::ensure_rfc3986_normalizable`] method or
    /// [`Normalized::ensure_rfc3986_normalizable`] before using the result
    /// string.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("HTTP://example.COM/foo/./bar/%2e%2e/../baz?query")?;
    ///
    /// let normalized = iri.normalize().to_dedicated_string();
    /// assert_eq!(normalized, "http://example.com/baz?query");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn normalize(&self) -> Normalized<'_, Self> {
        Normalized::from_input(NormalizationInput::from(self)).and_normalize()
    }

    /// Returns the normalized IRI, but preserving dot segments in relative path
    /// if the authority component is absent.
    ///
    /// This normalization would be similar to that of [WHATWG URL Standard]
    /// while this implementation is not guaranteed to stricly follow the spec.
    ///
    /// Note that this normalization algorithm is not compatible with RFC 3986
    /// algorithm for some inputs.
    ///
    /// Note that case normalization and percent-encoding normalization will
    /// still be applied to any path.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("HTTP://example.COM/foo/./bar/%2e%2e/../baz?query")?;
    ///
    /// let normalized = iri
    ///     .normalize_but_preserve_authorityless_relative_path()
    ///     .to_dedicated_string();
    /// assert_eq!(normalized, "http://example.com/baz?query");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("scheme:relative/../f%6f%6f")?;
    ///
    /// let normalized = iri
    ///     .normalize_but_preserve_authorityless_relative_path()
    ///     .to_dedicated_string();
    /// assert_eq!(normalized, "scheme:relative/../foo");
    /// // `.normalize()` would normalize this to `scheme:/foo`.
    /// # assert_eq!(iri.normalize().to_dedicated_string(), "scheme:/foo");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// [WHATWG URL Standard]: https://url.spec.whatwg.org/
    #[inline]
    #[must_use]
    pub fn normalize_but_preserve_authorityless_relative_path(&self) -> Normalized<'_, Self> {
        Normalized::from_input(NormalizationInput::from(self))
            .and_normalize_but_preserve_authorityless_relative_path()
    }

    /// Returns the proxy to the IRI with password masking feature.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("http://user:password@example.com/path?query")?;
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
impl<S: Spec> RiAbsoluteStr<S> {
    /// Returns the scheme.
    ///
    /// The following colon is truncated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("http://example.com/pathpath?queryquery")?;
    /// assert_eq!(iri.scheme_str(), "http");
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn scheme_str(&self) -> &str {
        trusted_parser::extract_scheme_absolute(self.as_str())
    }

    /// Returns the authority.
    ///
    /// The leading `//` is truncated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("http://example.com/pathpath?queryquery")?;
    /// assert_eq!(iri.authority_str(), Some("example.com"));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("urn:uuid:10db315b-fcd1-4428-aca8-15babc9a2da2")?;
    /// assert_eq!(iri.authority_str(), None);
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn authority_str(&self) -> Option<&str> {
        trusted_parser::extract_authority_absolute(self.as_str())
    }

    /// Returns the path.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("http://example.com/pathpath?queryquery")?;
    /// assert_eq!(iri.path_str(), "/pathpath");
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("urn:uuid:10db315b-fcd1-4428-aca8-15babc9a2da2")?;
    /// assert_eq!(iri.path_str(), "uuid:10db315b-fcd1-4428-aca8-15babc9a2da2");
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn path_str(&self) -> &str {
        trusted_parser::extract_path_absolute(self.as_str())
    }

    /// Returns the query.
    ///
    /// The leading question mark (`?`) is truncated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::{IriAbsoluteStr, IriQueryStr};
    ///
    /// let iri = IriAbsoluteStr::new("http://example.com/pathpath?queryquery")?;
    /// let query = IriQueryStr::new("queryquery")?;
    /// assert_eq!(iri.query(), Some(query));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("urn:uuid:10db315b-fcd1-4428-aca8-15babc9a2da2")?;
    /// assert_eq!(iri.query(), None);
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn query(&self) -> Option<&RiQueryStr<S>> {
        trusted_parser::extract_query_absolute_iri(self.as_str()).map(|query| {
            // SAFETY: `trusted_parser::extract_query_absolute_iri()` must return
            // the query part of an IRI (including the leading `?` character),
            // and the returned string consists of allowed characters since it
            // is a substring of the source IRI.
            unsafe { RiQueryStr::new_maybe_unchecked(query) }
        })
    }

    /// Returns the query in a raw string slice.
    ///
    /// The leading question mark (`?`) is truncated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("http://example.com/pathpath?queryquery")?;
    /// assert_eq!(iri.query_str(), Some("queryquery"));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("urn:uuid:10db315b-fcd1-4428-aca8-15babc9a2da2")?;
    /// assert_eq!(iri.query_str(), None);
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn query_str(&self) -> Option<&str> {
        trusted_parser::extract_query_absolute_iri(self.as_str())
    }

    /// Returns the authority components.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("http://user:pass@example.com:8080/pathpath?queryquery")?;
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
    /// use iri_string::types::IriAbsoluteStr;
    ///
    /// let iri = IriAbsoluteStr::new("urn:uuid:10db315b-fcd1-4428-aca8-15babc9a2da2")?;
    /// assert_eq!(iri.authority_str(), None);
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn authority_components(&self) -> Option<AuthorityComponents<'_>> {
        AuthorityComponents::from_iri(self.as_ref())
    }
}

#[cfg(feature = "alloc")]
impl<S: Spec> RiAbsoluteString<S> {
    /// Removes the password completely (including separator colon) from `self` even if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::types::IriAbsoluteString;
    ///
    /// let mut iri = IriAbsoluteString::try_from("http://user:password@example.com/path?query")?;
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
    /// use iri_string::types::IriAbsoluteString;
    ///
    /// let mut iri = IriAbsoluteString::try_from("http://user:@example.com/path?query")?;
    /// iri.remove_password_inline();
    /// assert_eq!(iri, "http://user@example.com/path?query");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    pub fn remove_password_inline(&mut self) {
        let pw_range = match password_range_to_hide(self.as_slice().as_ref()) {
            Some(v) => v,
            None => return,
        };
        let separator_colon = pw_range.start - 1;
        // SAFETY: the IRI must still be valid after the password component and
        // the leading separator colon is removed.
        unsafe {
            let buf = self.as_inner_mut();
            buf.drain(separator_colon..pw_range.end);
            debug_assert!(
                RiAbsoluteStr::<S>::new(buf).is_ok(),
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
    /// use iri_string::types::IriAbsoluteString;
    ///
    /// let mut iri = IriAbsoluteString::try_from("http://user:password@example.com/path?query")?;
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
    /// use iri_string::types::IriAbsoluteString;
    ///
    /// let mut iri = IriAbsoluteString::try_from("http://user:@example.com/path?query")?;
    /// iri.remove_nonempty_password_inline();
    /// assert_eq!(iri, "http://user:@example.com/path?query");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    pub fn remove_nonempty_password_inline(&mut self) {
        let pw_range = match password_range_to_hide(self.as_slice().as_ref()) {
            Some(v) if !v.is_empty() => v,
            _ => return,
        };
        debug_assert_eq!(
            self.as_str().as_bytes().get(pw_range.start - 1).copied(),
            Some(b':'),
            "[validity] the password component must be prefixed with a separator colon"
        );
        // SAFETY: the IRI must be valid after the password is replaced with empty string.
        unsafe {
            let buf = self.as_inner_mut();
            buf.drain(pw_range);
            debug_assert!(
                RiAbsoluteStr::<S>::new(buf).is_ok(),
                "[validity] the IRI must be valid after the password component is removed"
            );
        }
    }
}

impl_trivial_conv_between_iri! {
    from_slice: RiAbsoluteStr,
    from_owned: RiAbsoluteString,
    to_slice: RiStr,
    to_owned: RiString,
}

impl_trivial_conv_between_iri! {
    from_slice: RiAbsoluteStr,
    from_owned: RiAbsoluteString,
    to_slice: RiReferenceStr,
    to_owned: RiReferenceString,
}
