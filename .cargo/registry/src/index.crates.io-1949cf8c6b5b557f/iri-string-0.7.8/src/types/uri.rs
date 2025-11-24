//! URI-specific implementations.

use crate::spec::UriSpec;
use crate::types::{
    IriAbsoluteStr, IriFragmentStr, IriQueryStr, IriReferenceStr, IriRelativeStr, IriStr,
    RiAbsoluteStr, RiFragmentStr, RiQueryStr, RiReferenceStr, RiRelativeStr, RiStr,
};
#[cfg(feature = "alloc")]
use crate::types::{
    IriAbsoluteString, IriFragmentString, IriQueryString, IriReferenceString, IriRelativeString,
    IriString, RiAbsoluteString, RiFragmentString, RiQueryString, RiReferenceString,
    RiRelativeString, RiString,
};

/// A type alias for [`RiAbsoluteStr`]`<`[`UriSpec`]`>`.
pub type UriAbsoluteStr = RiAbsoluteStr<UriSpec>;

/// A type alias for [`RiAbsoluteString`]`<`[`UriSpec`]`>`.
#[cfg(feature = "alloc")]
pub type UriAbsoluteString = RiAbsoluteString<UriSpec>;

/// A type alias for [`RiFragmentStr`]`<`[`UriSpec`]`>`.
pub type UriFragmentStr = RiFragmentStr<UriSpec>;

/// A type alias for [`RiFragmentString`]`<`[`UriSpec`]`>`.
#[cfg(feature = "alloc")]
pub type UriFragmentString = RiFragmentString<UriSpec>;

/// A type alias for [`RiStr`]`<`[`UriSpec`]`>`.
pub type UriStr = RiStr<UriSpec>;

/// A type alias for [`RiString`]`<`[`UriSpec`]`>`.
#[cfg(feature = "alloc")]
pub type UriString = RiString<UriSpec>;

/// A type alias for [`RiReferenceStr`]`<`[`UriSpec`]`>`.
pub type UriReferenceStr = RiReferenceStr<UriSpec>;

/// A type alias for [`RiReferenceString`]`<`[`UriSpec`]`>`.
#[cfg(feature = "alloc")]
pub type UriReferenceString = RiReferenceString<UriSpec>;

/// A type alias for [`RiRelativeStr`]`<`[`UriSpec`]`>`.
pub type UriRelativeStr = RiRelativeStr<UriSpec>;

/// A type alias for [`RiRelativeString`]`<`[`UriSpec`]`>`.
#[cfg(feature = "alloc")]
pub type UriRelativeString = RiRelativeString<UriSpec>;

/// A type alias for [`RiQueryStr`]`<`[`UriSpec`]`>`.
pub type UriQueryStr = RiQueryStr<UriSpec>;

/// A type alias for [`RiQueryString`]`<`[`UriSpec`]`>`.
#[cfg(feature = "alloc")]
pub type UriQueryString = RiQueryString<UriSpec>;

/// Implements the trivial conversions between a URI and an IRI.
macro_rules! impl_conversions_between_iri {
    (
        $borrowed_uri:ident,
        $owned_uri:ident,
        $borrowed_iri:ident,
        $owned_iri:ident,
    ) => {
        impl AsRef<$borrowed_iri> for $borrowed_uri {
            fn as_ref(&self) -> &$borrowed_iri {
                // SAFETY: A valid URI is also a valid IRI.
                unsafe { <$borrowed_iri>::new_maybe_unchecked(self.as_str()) }
            }
        }

        #[cfg(feature = "alloc")]
        impl From<$owned_uri> for $owned_iri {
            #[inline]
            fn from(uri: $owned_uri) -> Self {
                // SAFETY: A valid URI is also a valid IRI.
                unsafe { Self::new_maybe_unchecked(uri.into()) }
            }
        }

        #[cfg(feature = "alloc")]
        impl AsRef<$borrowed_iri> for $owned_uri {
            fn as_ref(&self) -> &$borrowed_iri {
                AsRef::<$borrowed_uri>::as_ref(self).as_ref()
            }
        }
    };
}

impl_conversions_between_iri!(
    UriAbsoluteStr,
    UriAbsoluteString,
    IriAbsoluteStr,
    IriAbsoluteString,
);
impl_conversions_between_iri!(
    UriReferenceStr,
    UriReferenceString,
    IriReferenceStr,
    IriReferenceString,
);
impl_conversions_between_iri!(
    UriRelativeStr,
    UriRelativeString,
    IriRelativeStr,
    IriRelativeString,
);
impl_conversions_between_iri!(UriStr, UriString, IriStr, IriString,);
impl_conversions_between_iri!(UriQueryStr, UriQueryString, IriQueryStr, IriQueryString,);
impl_conversions_between_iri!(
    UriFragmentStr,
    UriFragmentString,
    IriFragmentStr,
    IriFragmentString,
);
