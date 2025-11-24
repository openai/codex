//! Usual absolute IRI (fragment part being allowed).

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::string::String;

use crate::components::AuthorityComponents;
#[cfg(feature = "alloc")]
use crate::mask_password::password_range_to_hide;
use crate::mask_password::PasswordMasked;
use crate::normalize::{Error, NormalizationInput, Normalized, NormalizednessCheckMode};
use crate::parser::trusted as trusted_parser;
#[cfg(feature = "alloc")]
use crate::raw;
use crate::spec::Spec;
use crate::types::{RiAbsoluteStr, RiFragmentStr, RiQueryStr, RiReferenceStr};
#[cfg(feature = "alloc")]
use crate::types::{RiAbsoluteString, RiFragmentString, RiReferenceString};
use crate::validate::iri;

define_custom_string_slice! {
    /// A borrowed string of an absolute IRI possibly with fragment part.
    ///
    /// This corresponds to [`IRI` rule] in [RFC 3987] (and [`URI` rule] in [RFC 3986]).
    /// The rule for `IRI` is `scheme ":" ihier-part [ "?" iquery ] [ "#" ifragment ]`.
    /// In other words, this is [`RiAbsoluteStr`] with fragment part allowed.
    ///
    /// # Valid values
    ///
    /// This type can have an IRI (which is absolute, and may have fragment part).
    ///
    /// ```
    /// # use iri_string::types::IriStr;
    /// assert!(IriStr::new("https://user:pass@example.com:8080").is_ok());
    /// assert!(IriStr::new("https://example.com/").is_ok());
    /// assert!(IriStr::new("https://example.com/foo?bar=baz").is_ok());
    /// assert!(IriStr::new("https://example.com/foo?bar=baz#qux").is_ok());
    /// assert!(IriStr::new("foo:bar").is_ok());
    /// assert!(IriStr::new("foo:").is_ok());
    /// // `foo://.../` below are all allowed. See the crate documentation for detail.
    /// assert!(IriStr::new("foo:/").is_ok());
    /// assert!(IriStr::new("foo://").is_ok());
    /// assert!(IriStr::new("foo:///").is_ok());
    /// assert!(IriStr::new("foo:////").is_ok());
    /// assert!(IriStr::new("foo://///").is_ok());
    /// ```
    ///
    /// Relative IRI reference is not allowed.
    ///
    /// ```
    /// # use iri_string::types::IriStr;
    /// // This is relative path.
    /// assert!(IriStr::new("foo/bar").is_err());
    /// // `/foo/bar` is an absolute path, but it is authority-relative.
    /// assert!(IriStr::new("/foo/bar").is_err());
    /// // `//foo/bar` is termed "network-path reference",
    /// // or usually called "protocol-relative reference".
    /// assert!(IriStr::new("//foo/bar").is_err());
    /// // Same-document reference is relative.
    /// assert!(IriStr::new("#foo").is_err());
    /// // Empty string is not a valid absolute IRI.
    /// assert!(IriStr::new("").is_err());
    /// ```
    ///
    /// Some characters and sequences cannot used in an IRI.
    ///
    /// ```
    /// # use iri_string::types::IriStr;
    /// // `<` and `>` cannot directly appear in an IRI.
    /// assert!(IriStr::new("<not allowed>").is_err());
    /// // Broken percent encoding cannot appear in an IRI.
    /// assert!(IriStr::new("%").is_err());
    /// assert!(IriStr::new("%GG").is_err());
    /// ```
    ///
    /// [RFC 3986]: https://tools.ietf.org/html/rfc3986
    /// [RFC 3987]: https://tools.ietf.org/html/rfc3987
    /// [`IRI` rule]: https://tools.ietf.org/html/rfc3987#section-2.2
    /// [`URI` rule]: https://tools.ietf.org/html/rfc3986#section-3
    /// [`RiAbsoluteStr`]: struct.RiAbsoluteStr.html
    struct RiStr {
        validator = iri,
        expecting_msg = "IRI string",
    }
}

#[cfg(feature = "alloc")]
define_custom_string_owned! {
    /// An owned string of an absolute IRI possibly with fragment part.
    ///
    /// This corresponds to [`IRI` rule] in [RFC 3987] (and [`URI` rule] in [RFC 3986]).
    /// The rule for `IRI` is `scheme ":" ihier-part [ "?" iquery ] [ "#" ifragment ]`.
    /// In other words, this is [`RiAbsoluteString`] with fragment part allowed.
    ///
    /// For details, see the document for [`RiStr`].
    ///
    /// Enabled by `alloc` or `std` feature.
    ///
    /// [RFC 3986]: https://tools.ietf.org/html/rfc3986
    /// [RFC 3987]: https://tools.ietf.org/html/rfc3987
    /// [`IRI` rule]: https://tools.ietf.org/html/rfc3987#section-2.2
    /// [`URI` rule]: https://tools.ietf.org/html/rfc3986#section-3
    /// [`RiAbsoluteString`]: struct.RiAbsoluteString.html
    struct RiString {
        validator = iri,
        slice = RiStr,
        expecting_msg = "IRI string",
    }
}

impl<S: Spec> RiStr<S> {
    /// Splits the IRI into an absolute IRI part and a fragment part.
    ///
    /// A leading `#` character is truncated if the fragment part exists.
    ///
    /// # Examples
    ///
    /// If the IRI has a fragment part, `Some(_)` is returned.
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::{IriFragmentStr, IriStr}, validate::Error};
    /// let iri = IriStr::new("foo://bar/baz?qux=quux#corge")?;
    /// let (absolute, fragment) = iri.to_absolute_and_fragment();
    /// let fragment_expected = IriFragmentStr::new("corge")?;
    /// assert_eq!(absolute, "foo://bar/baz?qux=quux");
    /// assert_eq!(fragment, Some(fragment_expected));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// When the fragment part exists but is empty string, `Some(_)` is returned.
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::{IriFragmentStr, IriStr}, validate::Error};
    /// let iri = IriStr::new("foo://bar/baz?qux=quux#")?;
    /// let (absolute, fragment) = iri.to_absolute_and_fragment();
    /// let fragment_expected = IriFragmentStr::new("")?;
    /// assert_eq!(absolute, "foo://bar/baz?qux=quux");
    /// assert_eq!(fragment, Some(fragment_expected));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// If the IRI has no fragment, `None` is returned.
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriStr, validate::Error};
    /// let iri = IriStr::new("foo://bar/baz?qux=quux")?;
    /// let (absolute, fragment) = iri.to_absolute_and_fragment();
    /// assert_eq!(absolute, "foo://bar/baz?qux=quux");
    /// assert_eq!(fragment, None);
    /// # Ok::<_, Error>(())
    /// ```
    #[must_use]
    pub fn to_absolute_and_fragment(&self) -> (&RiAbsoluteStr<S>, Option<&RiFragmentStr<S>>) {
        let (prefix, fragment) = trusted_parser::split_fragment(self.as_str());
        // SAFETY: an IRI without fragment part is also an absolute IRI.
        let prefix = unsafe { RiAbsoluteStr::new_maybe_unchecked(prefix) };
        let fragment = fragment.map(|fragment| {
            // SAFETY: `trusted_parser::split_fragment()` must return a valid fragment component.
            unsafe { RiFragmentStr::new_maybe_unchecked(fragment) }
        });

        (prefix, fragment)
    }

    /// Strips the fragment part if exists, and returns [`&RiAbsoluteStr`][`RiAbsoluteStr`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriStr, validate::Error};
    /// let iri = IriStr::new("foo://bar/baz?qux=quux#corge")?;
    /// assert_eq!(iri.to_absolute(), "foo://bar/baz?qux=quux");
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriStr, validate::Error};
    /// let iri = IriStr::new("foo://bar/baz?qux=quux")?;
    /// assert_eq!(iri.to_absolute(), "foo://bar/baz?qux=quux");
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// [`RiAbsoluteStr`]: struct.RiAbsoluteStr.html
    #[must_use]
    pub fn to_absolute(&self) -> &RiAbsoluteStr<S> {
        let prefix_len = trusted_parser::split_fragment(self.as_str()).0.len();
        // SAFETY: IRI without the fragment part (including a leading `#` character)
        // is also an absolute IRI.
        unsafe { RiAbsoluteStr::new_maybe_unchecked(&self.as_str()[..prefix_len]) }
    }

    /// Returns Ok`(())` if the IRI is normalizable by the RFC 3986 algorithm.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("HTTP://example.COM/foo/%2e/bar/..")?;
    /// assert!(iri.ensure_rfc3986_normalizable().is_ok());
    ///
    /// let iri2 = IriStr::new("scheme:/..//bar")?;
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
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("HTTP://example.COM/foo/./bar/%2e%2e/../baz?query#fragment")?;
    /// assert!(!iri.is_normalized());
    ///
    /// let normalized = iri.normalize().to_dedicated_string();
    /// assert_eq!(normalized, "http://example.com/baz?query#fragment");
    /// assert!(normalized.is_normalized());
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("scheme:/.///foo")?;
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
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("scheme:relative/..//not-a-host")?;
    /// // Default normalization algorithm assumes the path part to be NOT opaque.
    /// assert!(!iri.is_normalized());
    ///
    /// let normalized = iri.normalize().to_dedicated_string();
    /// assert_eq!(normalized, "scheme:/.//not-a-host");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[must_use]
    #[inline]
    pub fn is_normalized(&self) -> bool {
        trusted_parser::is_normalized::<S>(self.as_str(), NormalizednessCheckMode::Default)
    }

    /// Returns `true` if the IRI is already normalized in the sense of RFC 3986.
    ///
    /// This returns the same result as
    /// `self.ensure_rfc3986_normalizable() && (self.normalize().to_string() == self)`,
    /// but does this more efficiently without heap allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("HTTP://example.COM/foo/./bar/%2e%2e/../baz?query#fragment")?;
    /// assert!(!iri.is_normalized_rfc3986());
    ///
    /// let normalized = iri.normalize().to_dedicated_string();
    /// assert_eq!(normalized, "http://example.com/baz?query#fragment");
    /// assert!(normalized.is_normalized_rfc3986());
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("scheme:/.///foo")?;
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
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("scheme:relative/..//not-a-host")?;
    /// // RFC 3986 normalization algorithm assumes the path part to be NOT opaque.
    /// assert!(!iri.is_normalized_rfc3986());
    ///
    /// let normalized = iri.normalize().to_dedicated_string();
    /// assert_eq!(normalized, "scheme:/.//not-a-host");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    #[must_use]
    #[inline]
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
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("HTTP://example.COM/foo/./bar/%2e%2e/../baz?query#fragment")?;
    /// assert!(!iri.is_normalized_but_authorityless_relative_path_preserved());
    ///
    /// let normalized = iri
    ///     .normalize_but_preserve_authorityless_relative_path()
    ///     .to_dedicated_string();
    /// assert_eq!(normalized, "http://example.com/baz?query#fragment");
    /// assert!(normalized.is_normalized());
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("scheme:/.///foo")?;
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
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("scheme:relative/..//not-a-host")?;
    /// // Relative path is treated as opaque since the autority component is absent.
    /// assert!(iri.is_normalized_but_authorityless_relative_path_preserved());
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// [`normalize_but_preserve_authorityless_relative_path`]:
    ///     `Self::normalize_but_preserve_authorityless_relative_path`
    #[must_use]
    #[inline]
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
    /// If you worry about this, test by [`RiStr::ensure_rfc3986_normalizable`]
    /// method or [`Normalized::ensure_rfc3986_normalizable`] before using the
    /// result string.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("HTTP://example.COM/foo/./bar/%2e%2e/../baz?query#fragment")?;
    ///
    /// let normalized = iri.normalize().to_dedicated_string();
    /// assert_eq!(normalized, "http://example.com/baz?query#fragment");
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
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("HTTP://example.COM/foo/./bar/%2e%2e/../baz?query#fragment")?;
    ///
    /// let normalized = iri
    ///     .normalize_but_preserve_authorityless_relative_path()
    ///     .to_dedicated_string();
    /// assert_eq!(normalized, "http://example.com/baz?query#fragment");
    /// # }
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::format::ToDedicatedString;
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("scheme:relative/../f%6f%6f")?;
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
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("http://user:password@example.com/path?query")?;
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
impl<S: Spec> RiStr<S> {
    /// Returns the scheme.
    ///
    /// The following colon is truncated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("http://example.com/pathpath?queryquery#fragfrag")?;
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
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("http://example.com/pathpath?queryquery#fragfrag")?;
    /// assert_eq!(iri.authority_str(), Some("example.com"));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("urn:uuid:10db315b-fcd1-4428-aca8-15babc9a2da2")?;
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
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("http://example.com/pathpath?queryquery#fragfrag")?;
    /// assert_eq!(iri.path_str(), "/pathpath");
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("urn:uuid:10db315b-fcd1-4428-aca8-15babc9a2da2")?;
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
    /// use iri_string::types::{IriQueryStr, IriStr};
    ///
    /// let iri = IriStr::new("http://example.com/pathpath?queryquery#fragfrag")?;
    /// let query = IriQueryStr::new("queryquery")?;
    /// assert_eq!(iri.query(), Some(query));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("urn:uuid:10db315b-fcd1-4428-aca8-15babc9a2da2")?;
    /// assert_eq!(iri.query(), None);
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn query(&self) -> Option<&RiQueryStr<S>> {
        AsRef::<RiReferenceStr<S>>::as_ref(self).query()
    }

    /// Returns the query in a raw string slice.
    ///
    /// The leading question mark (`?`) is truncated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("http://example.com/pathpath?queryquery#fragfrag")?;
    /// assert_eq!(iri.query_str(), Some("queryquery"));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("urn:uuid:10db315b-fcd1-4428-aca8-15babc9a2da2")?;
    /// assert_eq!(iri.query_str(), None);
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
    /// ```
    /// # use iri_string::{spec::IriSpec, types::{IriFragmentStr, IriStr}, validate::Error};
    /// let iri = IriStr::new("foo://bar/baz?qux=quux#corge")?;
    /// let fragment = IriFragmentStr::new("corge")?;
    /// assert_eq!(iri.fragment(), Some(fragment));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::{IriFragmentStr, IriStr}, validate::Error};
    /// let iri = IriStr::new("foo://bar/baz?qux=quux#")?;
    /// let fragment = IriFragmentStr::new("")?;
    /// assert_eq!(iri.fragment(), Some(fragment));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriStr, validate::Error};
    /// let iri = IriStr::new("foo://bar/baz?qux=quux")?;
    /// assert_eq!(iri.fragment(), None);
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn fragment(&self) -> Option<&RiFragmentStr<S>> {
        AsRef::<RiReferenceStr<S>>::as_ref(self).fragment()
    }

    /// Returns the fragment part as a raw string slice if exists.
    ///
    /// A leading `#` character is truncated if the fragment part exists.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriStr, validate::Error};
    /// let iri = IriStr::new("foo://bar/baz?qux=quux#corge")?;
    /// assert_eq!(iri.fragment_str(), Some("corge"));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriStr, validate::Error};
    /// let iri = IriStr::new("foo://bar/baz?qux=quux#")?;
    /// assert_eq!(iri.fragment_str(), Some(""));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriStr, validate::Error};
    /// let iri = IriStr::new("foo://bar/baz?qux=quux")?;
    /// assert_eq!(iri.fragment_str(), None);
    /// # Ok::<_, Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn fragment_str(&self) -> Option<&str> {
        AsRef::<RiReferenceStr<S>>::as_ref(self).fragment_str()
    }

    /// Returns the authority components.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("http://user:pass@example.com:8080/pathpath?queryquery")?;
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
    /// use iri_string::types::IriStr;
    ///
    /// let iri = IriStr::new("urn:uuid:10db315b-fcd1-4428-aca8-15babc9a2da2")?;
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
impl<S: Spec> RiString<S> {
    /// Splits the IRI into an absolute IRI part and a fragment part.
    ///
    /// A leading `#` character is truncated if the fragment part exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::convert::TryFrom;
    /// # use iri_string::{spec::IriSpec, types::{IriFragmentString, IriString}, validate::Error};
    /// let iri = "foo://bar/baz?qux=quux#corge".parse::<IriString>()?;
    /// let (absolute, fragment) = iri.into_absolute_and_fragment();
    /// let fragment_expected = IriFragmentString::try_from("corge".to_owned())
    ///     .map_err(|e| e.validation_error())?;
    /// assert_eq!(absolute, "foo://bar/baz?qux=quux");
    /// assert_eq!(fragment, Some(fragment_expected));
    /// # Ok::<_, Error>(())
    ///
    /// ```
    ///
    /// ```
    /// use std::convert::TryFrom;
    /// # use iri_string::{spec::IriSpec, types::{IriFragmentString, IriString}, validate::Error};
    /// let iri = "foo://bar/baz?qux=quux#".parse::<IriString>()?;
    /// let (absolute, fragment) = iri.into_absolute_and_fragment();
    /// let fragment_expected = IriFragmentString::try_from("".to_owned())
    ///     .map_err(|e| e.validation_error())?;
    /// assert_eq!(absolute, "foo://bar/baz?qux=quux");
    /// assert_eq!(fragment, Some(fragment_expected));
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// use std::convert::TryFrom;
    /// # use iri_string::{spec::IriSpec, types::IriString, validate::Error};
    /// let iri = "foo://bar/baz?qux=quux".parse::<IriString>()?;
    /// let (absolute, fragment) = iri.into_absolute_and_fragment();
    /// assert_eq!(absolute, "foo://bar/baz?qux=quux");
    /// assert_eq!(fragment, None);
    /// # Ok::<_, Error>(())
    /// ```
    #[must_use]
    pub fn into_absolute_and_fragment(self) -> (RiAbsoluteString<S>, Option<RiFragmentString<S>>) {
        let (prefix, fragment) = raw::split_fragment_owned(self.into());
        // SAFETY: an IRI without fragment part is also an absolute IRI.
        let prefix = unsafe { RiAbsoluteString::new_maybe_unchecked(prefix) };
        let fragment = fragment.map(|fragment| {
            // SAFETY: the string returned by `raw::split_fragment_owned()` must
            // be the fragment part, and must also be a substring of the source IRI.
            unsafe { RiFragmentString::new_maybe_unchecked(fragment) }
        });

        (prefix, fragment)
    }

    /// Strips the fragment part if exists, and returns an [`RiAbsoluteString`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriString, validate::Error};
    /// let iri = "foo://bar/baz?qux=quux#corge".parse::<IriString>()?;
    /// assert_eq!(iri.into_absolute(), "foo://bar/baz?qux=quux");
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// ```
    /// # use iri_string::{spec::IriSpec, types::IriString, validate::Error};
    /// let iri = "foo://bar/baz?qux=quux".parse::<IriString>()?;
    /// assert_eq!(iri.into_absolute(), "foo://bar/baz?qux=quux");
    /// # Ok::<_, Error>(())
    /// ```
    ///
    /// [`RiAbsoluteString`]: struct.RiAbsoluteString.html
    #[must_use]
    pub fn into_absolute(self) -> RiAbsoluteString<S> {
        let mut s: String = self.into();
        raw::remove_fragment(&mut s);
        // SAFETY: an IRI without fragment part is also an absolute IRI.
        unsafe { RiAbsoluteString::new_maybe_unchecked(s) }
    }

    /// Sets the fragment part to the given string.
    ///
    /// Removes fragment part (and following `#` character) if `None` is given.
    pub fn set_fragment(&mut self, fragment: Option<&RiFragmentStr<S>>) {
        raw::set_fragment(&mut self.inner, fragment.map(AsRef::as_ref));
        debug_assert!(iri::<S>(&self.inner).is_ok());
    }

    /// Removes the password completely (including separator colon) from `self` even if it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use iri_string::validate::Error;
    /// # #[cfg(feature = "alloc")] {
    /// use iri_string::types::IriString;
    ///
    /// let mut iri = IriString::try_from("http://user:password@example.com/path?query")?;
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
    /// use iri_string::types::IriString;
    ///
    /// let mut iri = IriString::try_from("http://user:@example.com/path?query")?;
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
                RiStr::<S>::new(buf).is_ok(),
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
    /// use iri_string::types::IriString;
    ///
    /// let mut iri = IriString::try_from("http://user:password@example.com/path?query")?;
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
    /// use iri_string::types::IriString;
    ///
    /// let mut iri = IriString::try_from("http://user:@example.com/path?query")?;
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
        // SAFETY: the IRI must still be valid if the password is replaced with
        // empty string.
        unsafe {
            let buf = self.as_inner_mut();
            buf.drain(pw_range);
            debug_assert!(
                RiStr::<S>::new(buf).is_ok(),
                "[validity] the IRI must be valid after the password component is removed"
            );
        }
    }
}

impl_trivial_conv_between_iri! {
    from_slice: RiStr,
    from_owned: RiString,
    to_slice: RiReferenceStr,
    to_owned: RiReferenceString,
}
