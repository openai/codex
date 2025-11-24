//! Generic resource identifier types.
//!
//! ```text
//! IRI           = scheme ":" ihier-part [ "?" iquery ] [ "#" ifragment ]
//! IRI-reference = IRI / irelative-ref
//! absolute-IRI  = scheme ":" ihier-part [ "?" iquery ]
//! irelative-ref = irelative-part [ "?" iquery ] [ "#" ifragment ]
//!     (`irelative-part` is roughly same as `ihier-part`.)
//! ```
//!
//! Hierarchy:
//!
//! ```text
//! RiReferenceStr
//! |-- RiStr
//! |   `-- RiAbsoluteStr
//! `-- RiRelativeStr
//! ```
//!
//! Therefore, the conversions below are safe and cheap:
//!
//! * `RiStr -> RiReferenceStr`
//! * `RiAbsoluteStr -> RiStr`
//! * `RiAbsoluteStr -> RiReferenceStr`
//! * `RiRelativeStr -> RiReferenceStr`
//!
//! For safely convertible types (consider `FooStr -> BarStr` is safe), traits
//! below are implemented:
//!
//! * `AsRef<BarStr> for FooStr`
//! * `AsRef<BarStr> for FooString`
//! * `From<FooString> for BarString`
//! * `PartialEq<FooStr> for BarStr` and lots of impls like that
//!     + `PartialEq` and `ParitalOrd`.
//!     + Slice, owned, `Cow`, reference, etc...

pub use self::{
    absolute::RiAbsoluteStr, fragment::RiFragmentStr, normal::RiStr, query::RiQueryStr,
    reference::RiReferenceStr, relative::RiRelativeStr,
};
#[cfg(feature = "alloc")]
pub use self::{
    absolute::RiAbsoluteString, error::CreationError, fragment::RiFragmentString, normal::RiString,
    query::RiQueryString, reference::RiReferenceString, relative::RiRelativeString,
};

#[macro_use]
mod macros;

mod absolute;
#[cfg(feature = "alloc")]
mod error;
mod fragment;
mod normal;
mod query;
mod reference;
mod relative;
