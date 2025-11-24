//! IRI-specific implementations.

#[cfg(feature = "alloc")]
use alloc::collections::TryReserveError;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::string::String;

#[cfg(feature = "alloc")]
use crate::convert::try_percent_encode_iri_inline;
use crate::convert::MappedToUri;
use crate::spec::IriSpec;
use crate::types::{
    RiAbsoluteStr, RiFragmentStr, RiQueryStr, RiReferenceStr, RiRelativeStr, RiStr,
};
#[cfg(feature = "alloc")]
use crate::types::{
    RiAbsoluteString, RiFragmentString, RiQueryString, RiReferenceString, RiRelativeString,
    RiString,
};
use crate::types::{
    UriAbsoluteStr, UriFragmentStr, UriQueryStr, UriReferenceStr, UriRelativeStr, UriStr,
};
#[cfg(feature = "alloc")]
use crate::types::{
    UriAbsoluteString, UriFragmentString, UriQueryString, UriReferenceString, UriRelativeString,
    UriString,
};

/// A type alias for [`RiAbsoluteStr`]`<`[`IriSpec`]`>`.
pub type IriAbsoluteStr = RiAbsoluteStr<IriSpec>;

/// A type alias for [`RiAbsoluteString`]`<`[`IriSpec`]`>`.
#[cfg(feature = "alloc")]
pub type IriAbsoluteString = RiAbsoluteString<IriSpec>;

/// A type alias for [`RiFragmentStr`]`<`[`IriSpec`]`>`.
pub type IriFragmentStr = RiFragmentStr<IriSpec>;

/// A type alias for [`RiFragmentString`]`<`[`IriSpec`]`>`.
#[cfg(feature = "alloc")]
pub type IriFragmentString = RiFragmentString<IriSpec>;

/// A type alias for [`RiStr`]`<`[`IriSpec`]`>`.
pub type IriStr = RiStr<IriSpec>;

/// A type alias for [`RiString`]`<`[`IriSpec`]`>`.
#[cfg(feature = "alloc")]
pub type IriString = RiString<IriSpec>;

/// A type alias for [`RiReferenceStr`]`<`[`IriSpec`]`>`.
pub type IriReferenceStr = RiReferenceStr<IriSpec>;

/// A type alias for [`RiReferenceString`]`<`[`IriSpec`]`>`.
#[cfg(feature = "alloc")]
pub type IriReferenceString = RiReferenceString<IriSpec>;

/// A type alias for [`RiRelativeStr`]`<`[`IriSpec`]`>`.
pub type IriRelativeStr = RiRelativeStr<IriSpec>;

/// A type alias for [`RiRelativeString`]`<`[`IriSpec`]`>`.
#[cfg(feature = "alloc")]
pub type IriRelativeString = RiRelativeString<IriSpec>;

/// A type alias for [`RiQueryStr`]`<`[`IriSpec`]`>`.
pub type IriQueryStr = RiQueryStr<IriSpec>;

/// A type alias for [`RiQueryString`]`<`[`IriSpec`]`>`.
#[cfg(feature = "alloc")]
pub type IriQueryString = RiQueryString<IriSpec>;

/// Implements the conversion from an IRI into a URI.
macro_rules! impl_conversion_between_uri {
    (
        $ty_owned_iri:ident,
        $ty_owned_uri:ident,
        $ty_borrowed_iri:ident,
        $ty_borrowed_uri:ident,
        $example_iri:expr,
        $example_uri:expr
    ) => {
        /// Conversion from an IRI into a URI.
        impl $ty_borrowed_iri {
            /// Percent-encodes the IRI into a valid URI that identifies the equivalent resource.
            ///
            /// If you need more precise control over memory allocation and buffer
            /// handling, use [`MappedToUri`][`crate::convert::MappedToUri`] type.
            ///
            /// # Examples
            ///
            /// ```
            /// # use iri_string::validate::Error;
            /// # #[cfg(feature = "alloc")] {
            #[doc = concat!("use iri_string::format::ToDedicatedString;")]
            #[doc = concat!("use iri_string::types::{", stringify!($ty_borrowed_iri), ", ", stringify!($ty_owned_uri), "};")]
            ///
            #[doc = concat!("let iri = ", stringify!($ty_borrowed_iri), "::new(", stringify!($example_iri), ")?;")]
            /// // Type annotation here is not necessary.
            #[doc = concat!("let uri: ", stringify!($ty_owned_uri), " = iri.encode_to_uri().to_dedicated_string();")]
            #[doc = concat!("assert_eq!(uri, ", stringify!($example_uri), ");")]
            /// # }
            /// # Ok::<_, Error>(())
            /// ```
            #[inline]
            #[must_use]
            pub fn encode_to_uri(&self) -> MappedToUri<'_, Self> {
                MappedToUri::from(self)
            }

            /// Converts an IRI into a URI without modification, if possible.
            ///
            /// This is semantically equivalent to
            #[doc = concat!("`", stringify!($ty_borrowed_uri), "::new(self.as_str()).ok()`.")]
            ///
            /// # Examples
            ///
            /// ```
            /// # use iri_string::validate::Error;
            #[doc = concat!("use iri_string::types::{", stringify!($ty_borrowed_iri), ", ", stringify!($ty_borrowed_uri), "};")]
            ///
            #[doc = concat!("let ascii_iri = ", stringify!($ty_borrowed_iri), "::new(", stringify!($example_uri), ")?;")]
            /// assert_eq!(
            ///     ascii_iri.as_uri().map(AsRef::as_ref),
            #[doc = concat!("    Some(", stringify!($example_uri), ")")]
            /// );
            ///
            #[doc = concat!("let nonascii_iri = ", stringify!($ty_borrowed_iri), "::new(", stringify!($example_iri), ")?;")]
            /// assert_eq!(nonascii_iri.as_uri(), None);
            /// # Ok::<_, Error>(())
            /// ```
            #[must_use]
            pub fn as_uri(&self) -> Option<&$ty_borrowed_uri> {
                if !self.as_str().is_ascii() {
                    return None;
                }
                debug_assert!(
                    <$ty_borrowed_uri>::new(self.as_str()).is_ok(),
                    "[consistency] the ASCII-only IRI must also be a valid URI"
                );
                // SAFETY: An ASCII-only IRI is a URI.
                // URI (by `UriSpec`) is a subset of IRI (by `IriSpec`),
                // and the difference is that URIs can only have ASCII characters.
                let uri = unsafe { <$ty_borrowed_uri>::new_maybe_unchecked(self.as_str()) };
                Some(uri)
            }
        }

        /// Conversion from an IRI into a URI.
        #[cfg(feature = "alloc")]
        impl $ty_owned_iri {
            /// Percent-encodes the IRI into a valid URI that identifies the equivalent resource.
            ///
            /// After the encode, the IRI is also a valid URI.
            ///
            /// If you want a new URI string rather than modifying the IRI
            /// string, or if you need more precise control over memory
            /// allocation and buffer handling, use
            #[doc = concat!("[`encode_to_uri`][`", stringify!($ty_borrowed_iri), "::encode_to_uri`]")]
            /// method.
            ///
            /// # Panics
            ///
            /// Panics if the memory allocation failed.
            ///
            /// # Examples
            ///
            /// ```
            /// # use iri_string::validate::Error;
            /// #[cfg(feature = "alloc")] {
            #[doc = concat!("use iri_string::types::", stringify!($ty_owned_iri), ";")]
            ///
            #[doc = concat!("let mut iri = ", stringify!($ty_owned_iri), "::try_from(", stringify!($example_iri), ")?;")]
            /// iri.encode_to_uri_inline();
            #[doc = concat!("assert_eq!(iri, ", stringify!($example_uri), ");")]
            /// # }
            /// # Ok::<_, Error>(())
            /// ```
            #[inline]
            pub fn encode_to_uri_inline(&mut self) {
                self.try_encode_to_uri_inline()
                    .expect("failed to allocate memory");
            }

            /// Percent-encodes the IRI into a valid URI that identifies the equivalent resource.
            ///
            /// After the encode, the IRI is also a valid URI.
            ///
            /// If you want a new URI string rather than modifying the IRI
            /// string, or if you need more precise control over memory
            /// allocation and buffer handling, use
            #[doc = concat!("[`encode_to_uri`][`", stringify!($ty_borrowed_iri), "::encode_to_uri`]")]
            /// method.
            ///
            // TODO: This seems true as of this writing, but is this guaranteed? See
            // <https://users.rust-lang.org/t/does-try-reserve-guarantees-that-the-content-is-preserved-on-allocation-failure/77446>.
            // /// If the memory allocation failed, the content is preserved without modification.
            // ///
            /// # Examples
            ///
            /// ```
            /// # use iri_string::validate::Error;
            /// #[cfg(feature = "alloc")] {
            #[doc = concat!("use iri_string::types::", stringify!($ty_owned_iri), ";")]
            ///
            #[doc = concat!("let mut iri = ", stringify!($ty_owned_iri), "::try_from(", stringify!($example_iri), ")?;")]
            /// iri.try_encode_to_uri_inline()
            ///     .expect("failed to allocate memory");
            #[doc = concat!("assert_eq!(iri, ", stringify!($example_uri), ");")]
            /// # }
            /// # Ok::<_, Error>(())
            /// ```
            #[inline]
            pub fn try_encode_to_uri_inline(&mut self) -> Result<(), TryReserveError> {
                // SAFETY: IRI is valid after it is encoded to URI (by percent encoding).
                unsafe {
                    let buf = self.as_inner_mut();
                    try_percent_encode_iri_inline(buf)?;
                }
                debug_assert!(
                    <$ty_borrowed_iri>::new(self.as_str()).is_ok(),
                    "[consistency] the content must be valid at any time"
                );
                Ok(())
            }

            /// Percent-encodes the IRI into a valid URI that identifies the equivalent resource.
            ///
            /// If you want a new URI string rather than modifying the IRI
            /// string, or if you need more precise control over memory
            /// allocation and buffer handling, use
            #[doc = concat!("[`encode_to_uri`][`", stringify!($ty_borrowed_iri), "::encode_to_uri`]")]
            /// method.
            ///
            /// # Examples
            ///
            /// ```
            /// # use iri_string::validate::Error;
            /// #[cfg(feature = "alloc")] {
            #[doc = concat!("use iri_string::types::{", stringify!($ty_owned_iri), ", ", stringify!($ty_owned_uri), "};")]
            ///
            #[doc = concat!("let iri = ", stringify!($ty_owned_iri), "::try_from(", stringify!($example_iri), ")?;")]
            /// // Type annotation here is not necessary.
            #[doc = concat!("let uri: ", stringify!($ty_owned_uri), " = iri.encode_into_uri();")]
            #[doc = concat!("assert_eq!(uri, ", stringify!($example_uri), ");")]
            /// # }
            /// # Ok::<_, Error>(())
            /// ```
            #[inline]
            #[must_use]
            pub fn encode_into_uri(self) -> $ty_owned_uri {
                self.try_encode_into_uri()
                    .expect("failed to allocate memory")
            }

            /// Percent-encodes the IRI into a valid URI that identifies the equivalent resource.
            ///
            /// If you want a new URI string rather than modifying the IRI
            /// string, or if you need more precise control over memory
            /// allocation and buffer handling, use
            #[doc = concat!("[`encode_to_uri`][`", stringify!($ty_borrowed_iri), "::encode_to_uri`]")]
            /// method.
            ///
            // TODO: This seems true as of this writing, but is this guaranteed? See
            // <https://users.rust-lang.org/t/does-try-reserve-guarantees-that-the-content-is-preserved-on-allocation-failure/77446>.
            // /// If the memory allocation failed, the content is preserved without modification.
            // ///
            /// # Examples
            ///
            /// ```
            /// # use iri_string::validate::Error;
            /// #[cfg(feature = "alloc")] {
            #[doc = concat!("use iri_string::types::{", stringify!($ty_owned_iri), ", ", stringify!($ty_owned_uri), "};")]
            ///
            #[doc = concat!("let iri = ", stringify!($ty_owned_iri), "::try_from(", stringify!($example_iri), ")?;")]
            /// // Type annotation here is not necessary.
            #[doc = concat!("let uri: ", stringify!($ty_owned_uri), " = iri.try_encode_into_uri()")]
            ///     .expect("failed to allocate memory");
            #[doc = concat!("assert_eq!(uri, ", stringify!($example_uri), ");")]
            /// # }
            /// # Ok::<_, Error>(())
            /// ```
            pub fn try_encode_into_uri(mut self) -> Result<$ty_owned_uri, TryReserveError> {
                self.try_encode_to_uri_inline()?;
                let s: String = self.into();
                debug_assert!(
                    <$ty_borrowed_uri>::new(s.as_str()).is_ok(),
                    "[consistency] the encoded IRI must also be a valid URI"
                );
                // SAFETY: An ASCII-only IRI is a URI.
                // URI (by `UriSpec`) is a subset of IRI (by `IriSpec`),
                // and the difference is that URIs can only have ASCII characters.
                let uri = unsafe { <$ty_owned_uri>::new_maybe_unchecked(s) };
                Ok(uri)
            }

            /// Converts an IRI into a URI without modification, if possible.
            ///
            /// # Examples
            ///
            /// ```
            /// # use iri_string::validate::Error;
            #[doc = concat!("use iri_string::types::{", stringify!($ty_owned_iri), ", ", stringify!($ty_owned_uri), "};")]
            ///
            #[doc = concat!("let ascii_iri = ", stringify!($ty_owned_iri), "::try_from(", stringify!($example_uri), ")?;")]
            /// assert_eq!(
            ///     ascii_iri.try_into_uri().map(|uri| uri.to_string()),
            #[doc = concat!("    Ok(", stringify!($example_uri), ".to_string())")]
            /// );
            ///
            #[doc = concat!("let nonascii_iri = ", stringify!($ty_owned_iri), "::try_from(", stringify!($example_iri), ")?;")]
            /// assert_eq!(
            ///     nonascii_iri.try_into_uri().map_err(|iri| iri.to_string()),
            #[doc = concat!("    Err(", stringify!($example_iri), ".to_string())")]
            /// );
            /// # Ok::<_, Error>(())
            /// ```
            pub fn try_into_uri(self) -> Result<$ty_owned_uri, $ty_owned_iri> {
                if !self.as_str().is_ascii() {
                    return Err(self);
                }
                let s: String = self.into();
                debug_assert!(
                    <$ty_borrowed_uri>::new(s.as_str()).is_ok(),
                    "[consistency] the ASCII-only IRI must also be a valid URI"
                );
                // SAFETY: An ASCII-only IRI is a URI.
                // URI (by `UriSpec`) is a subset of IRI (by `IriSpec`),
                // and the difference is that URIs can only have ASCII characters.
                let uri = unsafe { <$ty_owned_uri>::new_maybe_unchecked(s) };
                Ok(uri)
            }
        }
    };
}

impl_conversion_between_uri!(
    IriAbsoluteString,
    UriAbsoluteString,
    IriAbsoluteStr,
    UriAbsoluteStr,
    "http://example.com/?alpha=\u{03B1}",
    "http://example.com/?alpha=%CE%B1"
);
impl_conversion_between_uri!(
    IriReferenceString,
    UriReferenceString,
    IriReferenceStr,
    UriReferenceStr,
    "http://example.com/?alpha=\u{03B1}",
    "http://example.com/?alpha=%CE%B1"
);
impl_conversion_between_uri!(
    IriRelativeString,
    UriRelativeString,
    IriRelativeStr,
    UriRelativeStr,
    "../?alpha=\u{03B1}",
    "../?alpha=%CE%B1"
);
impl_conversion_between_uri!(
    IriString,
    UriString,
    IriStr,
    UriStr,
    "http://example.com/?alpha=\u{03B1}",
    "http://example.com/?alpha=%CE%B1"
);
impl_conversion_between_uri!(
    IriQueryString,
    UriQueryString,
    IriQueryStr,
    UriQueryStr,
    "alpha-is-\u{03B1}",
    "alpha-is-%CE%B1"
);
impl_conversion_between_uri!(
    IriFragmentString,
    UriFragmentString,
    IriFragmentStr,
    UriFragmentStr,
    "alpha-is-\u{03B1}",
    "alpha-is-%CE%B1"
);
