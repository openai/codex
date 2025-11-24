//! Normalization.
//!
//! # IRI normalization (and resolution) can fail
//!
//! Though this is not explicitly stated in RFC 3986, IRI normalization can fail.
//! For example, `foo:.///bar`, `foo:./..//bar`, and `foo:/..//bar` are all
//! normalized to `foo://bar` as a string. However, IRI without authority (note
//! that this is different from "with empty authority") cannot have a path
//! starting with `//`, since it is ambiguous and can be interpreted as an IRI
//! with authority. So, `foo://bar` is decomposed as scheme `foo`, authority
//! `bar`, and empty path. The expected result is the combination of scheme
//! `foo`, no authority, and path `//bar` (though this is not possible to
//! serialize), so the algorithm fails as it cannot return the intended result.
//!
//! IRI resolution can also fail since it (conditionally) invokes normalization
//! during the resolution process. For example, resolving a reference `.///bar`
//! or `/..//bar` against the base `foo:` fail.
//!
//! Thus, IRI resolution can fail for some abnormal cases.
//!
//! Note that this kind of failure can happen only when the base IRI has no
//! authority and empty path. This would be rare in the wild, since many people
//! would use an IRI with authority part, such as `http://`.
//!
//! If you are handling `scheme://`-style URIs and IRIs, don't worry about the
//! failure. Currently no cases are known to fail when at least one of the base
//! IRI or the relative IRI contains authorities.
//!
//! To know what will happen on resolution failure, see the module documentation
//! for [`resolve`][`crate::resolve`].
//!
//! ## Examples
//!
//! ### Normalization failure
//!
//! ```
//! # #[cfg(feature = "alloc")] {
//! use iri_string::normalize::Error;
//! use iri_string::types::{IriAbsoluteStr, IriReferenceStr};
//!
//! let base = IriAbsoluteStr::new("foo:.///bar")?;
//! assert!(
//!     base.normalize().ensure_rfc3986_normalizable().is_err(),
//!     "this normalization should fails without WAHTWG URL Standard serialization"
//! );
//! # }
//! # Ok::<_, iri_string::validate::Error>(())
//! ```
//!
//! ### Resolution failure
//!
//! ```
//! # #[cfg(feature = "alloc")] {
//! use iri_string::types::{IriAbsoluteStr, IriReferenceStr};
//!
//! let base = IriAbsoluteStr::new("scheme:")?;
//! {
//!     let reference = IriReferenceStr::new(".///bar")?;
//!     let result = reference.resolve_against(base)
//!         .ensure_rfc3986_normalizable();
//!     assert!(result.is_err());
//! }
//!
//! {
//!     let reference2 = IriReferenceStr::new("/..//bar")?;
//!     // Resulting string will be `scheme://bar`, but `bar` should be a path
//!     // segment, not a host. So, the semantically correct target IRI cannot
//!     // be represented.
//!     let result2 = reference2.resolve_against(base)
//!         .ensure_rfc3986_normalizable();
//!     assert!(result2.is_err());
//! }
//! # }
//! # Ok::<_, iri_string::validate::Error>(())
//! ```

mod error;
mod path;
mod pct_case;

use core::fmt::{self, Display as _, Write as _};
use core::marker::PhantomData;

#[cfg(feature = "alloc")]
use alloc::collections::TryReserveError;

use crate::components::{RiReferenceComponents, Splitter};
#[cfg(feature = "alloc")]
use crate::format::{ToDedicatedString, ToStringFallible};
use crate::parser::str::rfind_split_hole;
use crate::parser::trusted::is_ascii_only_host;
use crate::spec::Spec;
use crate::types::{RiAbsoluteStr, RiReferenceStr, RiStr};
#[cfg(feature = "alloc")]
use crate::types::{RiAbsoluteString, RiString};

pub use self::error::Error;
pub(crate) use self::path::{Path, PathCharacteristic, PathToNormalize};
pub(crate) use self::pct_case::{
    is_pct_case_normalized, NormalizedAsciiOnlyHost, PctCaseNormalized,
};

/// Normalization algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NormalizationMode {
    /// No normalization.
    None,
    /// Default normalization mode.
    ///
    /// Applies RFC 3986 normalization whenever possible. When not possible,
    /// applies serialization algorithm defined in WHATWG URL standard.
    Default,
    /// WHATWG-like normalization mode.
    ///
    /// Preserves relative path as is (modulo case/pct normalization) when the
    /// authority component is absent.
    PreserveAuthoritylessRelativePath,
}

impl NormalizationMode {
    /// Returns true if case normalization and percent-encoding normalization should be applied.
    ///
    /// Note that even when this option is `true`, plain US-ASCII characters
    /// won't be automatically lowered. Users should apply case normalization
    /// for US-ASCII only `host` component by themselves.
    #[inline]
    #[must_use]
    fn case_pct_normalization(self) -> bool {
        match self {
            Self::None => false,
            Self::Default | Self::PreserveAuthoritylessRelativePath => true,
        }
    }
}

/// Normalizedness check algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NormalizednessCheckMode {
    /// Default algorithm (corresponding to [`NormalizationMode::Default`]).
    Default,
    /// Strict RFC 3986 normalization.
    Rfc3986,
    /// WHATWG-like normalization algorithm (corresponding to
    /// [`NormalizationMode::PreserveAuthoritylessRelativePath`]).
    PreserveAuthoritylessRelativePath,
}

/// Normalization operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NormalizationOp {
    /// Normalization mode.
    pub(crate) mode: NormalizationMode,
}

/// Spec-agnostic IRI normalization/resolution input.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NormalizationInput<'a> {
    /// Target scheme.
    scheme: &'a str,
    /// Target authority.
    authority: Option<&'a str>,
    /// Target path without dot-removal.
    path: Path<'a>,
    /// Target query.
    query: Option<&'a str>,
    /// Target fragment.
    fragment: Option<&'a str>,
    /// Normalization type.
    op: NormalizationOp,
}

impl<'a> NormalizationInput<'a> {
    /// Creates a `NormalizedInput` from IRIs to resolve.
    #[inline]
    #[must_use]
    pub(crate) fn with_resolution_params<S: Spec>(
        base_components: &RiReferenceComponents<'a, S>,
        reference: &'a RiReferenceStr<S>,
    ) -> Self {
        let r = RiReferenceComponents::from(reference);

        Self::create_normalization_input(
            r.iri.as_str(),
            &r.splitter,
            base_components.iri.as_str(),
            &base_components.splitter,
        )
    }

    /// Creates a `NormalizationInput` from components to resolve an IRI.
    #[must_use]
    fn create_normalization_input(
        r_iri: &'a str,
        r: &Splitter,
        b_iri: &'a str,
        b: &Splitter,
    ) -> Self {
        /// The toplevel component the reference has.
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        enum RefToplevel {
            /// Scheme.
            Scheme,
            /// Authority.
            Authority,
            /// Path.
            Path,
            /// Query.
            Query,
            /// Reference is empty or has only fragment.
            None,
        }

        impl RefToplevel {
            /// Choose a component from either of the reference or the base,
            /// based on the toplevel component of the reference.
            #[inline]
            #[must_use]
            fn choose_then<T, F, G>(self, component: RefToplevel, reference: F, base: G) -> T
            where
                F: FnOnce() -> T,
                G: FnOnce() -> T,
            {
                if self <= component {
                    reference()
                } else {
                    base()
                }
            }
        }

        let ref_toplevel = if r.has_scheme() {
            RefToplevel::Scheme
        } else if r.has_authority() {
            RefToplevel::Authority
        } else if !r.is_path_empty(r_iri.len()) {
            RefToplevel::Path
        } else if r.has_query() {
            RefToplevel::Query
        } else {
            RefToplevel::None
        };

        let path = match ref_toplevel {
            RefToplevel::Scheme | RefToplevel::Authority => {
                Path::NeedsProcessing(PathToNormalize::from_single_path(r.path_str(r_iri)))
            }
            RefToplevel::Path => {
                let r_path = r.path_str(r_iri);
                if r_path.starts_with('/') {
                    Path::NeedsProcessing(PathToNormalize::from_single_path(r_path))
                } else {
                    // About this branch, see
                    // <https://datatracker.ietf.org/doc/html/rfc3986#section-5.2.3>.
                    //
                    // > o  If the base URI has a defined authority component and an empty
                    // >    path, then return a string consisting of "/" concatenated with the
                    // >    reference's path; otherwise,
                    let b_path = b.path_str(b_iri);
                    let b_path = if b.has_authority() && b_path.is_empty() {
                        "/"
                    } else {
                        b_path
                    };
                    Path::NeedsProcessing(PathToNormalize::from_paths_to_be_resolved(
                        b_path, r_path,
                    ))
                }
            }
            RefToplevel::Query | RefToplevel::None => Path::Done(b.path_str(b_iri)),
        };

        Self {
            scheme: r.scheme_str(r_iri).unwrap_or_else(|| {
                b.scheme_str(b_iri)
                    .expect("[validity] non-relative IRI must have a scheme")
            }),
            authority: ref_toplevel.choose_then(
                RefToplevel::Authority,
                || r.authority_str(r_iri),
                || b.authority_str(b_iri),
            ),
            path,
            query: ref_toplevel.choose_then(
                RefToplevel::Query,
                || r.query_str(r_iri),
                || b.query_str(b_iri),
            ),
            fragment: r.fragment_str(r_iri),
            op: NormalizationOp {
                mode: NormalizationMode::None,
            },
        }
    }
}

impl<'a, S: Spec> From<&'a RiStr<S>> for NormalizationInput<'a> {
    fn from(iri: &'a RiStr<S>) -> Self {
        let components = RiReferenceComponents::<S>::from(iri.as_ref());
        let (scheme, authority, path, query, fragment) = components.to_major();
        let scheme = scheme.expect("[validity] `absolute IRI must have `scheme`");
        let path = Path::NeedsProcessing(PathToNormalize::from_single_path(path));

        NormalizationInput {
            scheme,
            authority,
            path,
            query,
            fragment,
            op: NormalizationOp {
                mode: NormalizationMode::None,
            },
        }
    }
}

#[cfg(feature = "alloc")]
impl<'a, S: Spec> From<&'a RiString<S>> for NormalizationInput<'a> {
    #[inline]
    fn from(iri: &'a RiString<S>) -> Self {
        Self::from(iri.as_slice())
    }
}

impl<'a, S: Spec> From<&'a RiAbsoluteStr<S>> for NormalizationInput<'a> {
    fn from(iri: &'a RiAbsoluteStr<S>) -> Self {
        let components = RiReferenceComponents::<S>::from(iri.as_ref());
        let (scheme, authority, path, query, fragment) = components.to_major();
        let scheme = scheme.expect("[validity] `absolute IRI must have `scheme`");
        let path = Path::NeedsProcessing(PathToNormalize::from_single_path(path));

        NormalizationInput {
            scheme,
            authority,
            path,
            query,
            fragment,
            op: NormalizationOp {
                mode: NormalizationMode::None,
            },
        }
    }
}

#[cfg(feature = "alloc")]
impl<'a, S: Spec> From<&'a RiAbsoluteString<S>> for NormalizationInput<'a> {
    #[inline]
    fn from(iri: &'a RiAbsoluteString<S>) -> Self {
        Self::from(iri.as_slice())
    }
}

impl NormalizationInput<'_> {
    /// Checks if the path is normalizable by RFC 3986 algorithm.
    ///
    /// Returns `Ok(())` when normalizable, returns `Err(_)` if not.
    pub(crate) fn ensure_rfc3986_normalizable(&self) -> Result<(), Error> {
        if self.authority.is_some() {
            return Ok(());
        }
        match self.path {
            Path::Done(_) => Ok(()),
            Path::NeedsProcessing(path) => path.ensure_rfc3986_normalizable_with_authority_absent(),
        }
    }
}

/// Writable as a normalized IRI.
///
/// Note that this implicitly apply serialization rule defined by WHATWG URL
/// Standard (to handle normalization impossible by RFC 3986) because `Display`
/// should not fail by reasons other than backend I/O failure. If you make the
/// normalization fail in such cases, check if the path starts with `/./`.
/// When the normalization succeeds by RFC 3986 algorithm, the path never starts
/// with `/./`.
struct NormalizedInner<'a, S> {
    /// Spec-agnostic normalization input.
    input: NormalizationInput<'a>,
    /// Spec.
    _spec: PhantomData<fn() -> S>,
}

impl<S: Spec> fmt::Debug for NormalizedInner<'_, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Normalized")
            .field("input", &self.input)
            .finish()
    }
}

impl<'a, S: Spec> NormalizedInner<'a, S> {
    /// Creates a new `Normalized` object from the given input.
    #[inline]
    #[must_use]
    fn from_input(input: NormalizationInput<'a>) -> Self {
        Self {
            input,
            _spec: PhantomData,
        }
    }
}

impl<S: Spec> fmt::Display for NormalizedInner<'_, S> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Write the scheme.
        if self.input.op.mode.case_pct_normalization() {
            normalize_scheme(f, self.input.scheme)?;
        } else {
            f.write_str(self.input.scheme)?;
        }
        f.write_str(":")?;

        // Write the authority if available.
        if let Some(authority) = self.input.authority {
            f.write_str("//")?;
            if self.input.op.mode.case_pct_normalization() {
                normalize_authority::<S>(f, authority)?;
            } else {
                // No case/pct normalization.
                f.write_str(authority)?;
            }
        }

        // Process and write the path.
        match self.input.path {
            Path::Done(s) => {
                if self.input.op.mode.case_pct_normalization() {
                    // Normalize the path.
                    PathToNormalize::from_single_path(s).fmt_write_normalize::<S, _>(
                        f,
                        self.input.op,
                        self.input.authority.is_some(),
                    )?
                } else {
                    // No normalization.
                    f.write_str(s)?
                }
            }
            Path::NeedsProcessing(path) => {
                path.fmt_write_normalize::<S, _>(f, self.input.op, self.input.authority.is_some())?
            }
        }

        // Write the query if available.
        if let Some(query) = self.input.query {
            f.write_char('?')?;
            if self.input.op.mode.case_pct_normalization() {
                normalize_query::<S>(f, query)?;
            } else {
                f.write_str(query)?;
            }
        }

        // Write the fragment if available.
        if let Some(fragment) = self.input.fragment {
            f.write_char('#')?;
            if self.input.op.mode.case_pct_normalization() {
                normalize_fragment::<S>(f, fragment)?;
            } else {
                f.write_str(fragment)?;
            }
        }

        Ok(())
    }
}

/// Writes the normalized scheme.
pub(crate) fn normalize_scheme(f: &mut fmt::Formatter<'_>, scheme: &str) -> fmt::Result {
    // Apply case normalization.
    //
    // > namely, that the scheme and US-ASCII only host are case
    // > insensitive and therefore should be normalized to lowercase.
    // >
    // > --- <https://datatracker.ietf.org/doc/html/rfc3987#section-5.3.2.1>.
    //
    // Note that `scheme` consists of only ASCII characters and contains
    // no percent-encoded characters.
    scheme
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .try_for_each(|c| f.write_char(c))
}

/// Writes the normalized authority.
fn normalize_authority<S: Spec>(f: &mut fmt::Formatter<'_>, authority: &str) -> fmt::Result {
    let host_port = match rfind_split_hole(authority, b'@') {
        Some((userinfo, host_port)) => {
            // Don't lowercase `userinfo` even if it is ASCII only. `userinfo`
            // is not a part of `host`.
            PctCaseNormalized::<S>::new(userinfo).fmt(f)?;
            f.write_char('@')?;
            host_port
        }
        None => authority,
    };
    normalize_host_port::<S>(f, host_port)
}

/// Writes the normalized host and port.
pub(crate) fn normalize_host_port<S: Spec>(
    f: &mut fmt::Formatter<'_>,
    host_port: &str,
) -> fmt::Result {
    // If the suffix is a colon, it is a delimiter between the host and empty
    // port. An empty port should be removed during normalization (see RFC 3986
    // section 3.2.3), so strip it.
    //
    // > URI producers and normalizers should omit the port component and its
    // > ":" delimiter if port is empty or if its value would be the same as
    // > that of the scheme's default.
    // >
    // > --- [RFC 3986 section 3.2.3. Port](https://www.rfc-editor.org/rfc/rfc3986.html#section-3.2.3)
    let host_port = host_port.strip_suffix(':').unwrap_or(host_port);

    // Apply case normalization and percent-encoding normalization to `host`.
    // Optional `":" port` part only consists of an ASCII colon and ASCII
    // digits, so this won't affect to the test result.
    if is_ascii_only_host(host_port) {
        // If the host is ASCII characters only, make plain alphabets lower case.
        NormalizedAsciiOnlyHost::new(host_port).fmt(f)
    } else {
        PctCaseNormalized::<S>::new(host_port).fmt(f)
    }
}

/// Writes the normalized query without the '?' prefix.
pub(crate) fn normalize_query<S: Spec>(f: &mut fmt::Formatter<'_>, query: &str) -> fmt::Result {
    // Apply percent-encoding normalization.
    PctCaseNormalized::<S>::new(query).fmt(f)
}

/// Writes the normalized query without the '#' prefix.
pub(crate) fn normalize_fragment<S: Spec>(
    f: &mut fmt::Formatter<'_>,
    fragment: &str,
) -> fmt::Result {
    // Apply percent-encoding normalization.
    PctCaseNormalized::<S>::new(fragment).fmt(f)
}

/// Normalized OR resolved IRI.
///
/// Resolved IRI can be represented by this type. In that case, the result might
/// not be normalized. If you want the IRI resolution result to be normalized,
/// use [`enable_normalization`][`Self::enable_normalization`] method.
///
/// [`Display`]: `core::fmt::Display`
pub struct Normalized<'a, T: ?Sized> {
    /// Spec-agnostic normalization input.
    input: NormalizationInput<'a>,
    /// Expected result type.
    _ty_str: PhantomData<fn() -> T>,
}

impl<T: ?Sized> fmt::Debug for Normalized<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Normalized")
            .field("input", &self.input)
            .finish()
    }
}

impl<'a, T: ?Sized> Normalized<'a, T> {
    /// Creates a new `Normalized` object from the given input.
    #[inline]
    #[must_use]
    pub(crate) fn from_input(input: NormalizationInput<'a>) -> Self {
        Self {
            input,
            _ty_str: PhantomData,
        }
    }

    /// Enables the normalization.
    ///
    /// This lets the normalizer apply the case normalization, percent-encoding
    /// normalization, and dot segments removal.
    #[inline]
    pub fn enable_normalization(&mut self) {
        self.input.op.mode = NormalizationMode::Default;
    }

    /// Enables the normalization that preserve relative path under some condition.
    ///
    /// Note that this normalization algorithm is not compatible with RFC 3986
    /// algorithm for some inputs.
    ///
    /// See [`RiStr::normalize_but_preserve_authorityless_relative_path()`]
    /// for detail.
    #[inline]
    pub fn enable_normalization_preserving_authorityless_relative_path(&mut self) {
        self.input.op.mode = NormalizationMode::PreserveAuthoritylessRelativePath;
    }

    /// Returns `Self` with normalization enabled.
    #[inline]
    #[must_use]
    pub fn and_normalize(mut self) -> Self {
        self.enable_normalization();
        self
    }

    /// Returns `Self` with special normalization enabled.
    ///
    /// Note that this normalization algorithm is not compatible with RFC 3986
    /// algorithm for some inputs.
    ///
    /// See [`RiStr::normalize_but_preserve_authorityless_relative_path()`]
    /// for detail.
    #[inline]
    #[must_use]
    pub fn and_normalize_but_preserve_authorityless_relative_path(mut self) -> Self {
        self.enable_normalization_preserving_authorityless_relative_path();
        self
    }

    /// Checks if the path is normalizable by RFC 3986 algorithm.
    ///
    /// Returns `Ok(())` when normalizable, returns `Err(_)` if not.
    #[inline]
    pub fn ensure_rfc3986_normalizable(&self) -> Result<(), Error> {
        self.input.ensure_rfc3986_normalizable()
    }
}

impl<S: Spec> fmt::Display for Normalized<'_, RiStr<S>> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        NormalizedInner::<S>::from_input(self.input).fmt(f)
    }
}

impl<S: Spec> fmt::Display for Normalized<'_, RiAbsoluteStr<S>> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        NormalizedInner::<S>::from_input(self.input).fmt(f)
    }
}

#[cfg(feature = "alloc")]
impl<S: Spec> ToDedicatedString for Normalized<'_, RiStr<S>> {
    type Target = RiString<S>;

    fn try_to_dedicated_string(&self) -> Result<Self::Target, TryReserveError> {
        let s = self.try_to_string()?;
        Ok(TryFrom::try_from(s).expect("[validity] the normalization result must be a valid IRI"))
    }
}

#[cfg(feature = "alloc")]
impl<S: Spec> From<Normalized<'_, RiStr<S>>> for RiString<S> {
    #[inline]
    fn from(v: Normalized<'_, RiStr<S>>) -> Self {
        v.to_dedicated_string()
    }
}

#[cfg(feature = "alloc")]
impl<S: Spec> From<&Normalized<'_, RiStr<S>>> for RiString<S> {
    #[inline]
    fn from(v: &Normalized<'_, RiStr<S>>) -> Self {
        v.to_dedicated_string()
    }
}

#[cfg(feature = "alloc")]
impl<S: Spec> ToDedicatedString for Normalized<'_, RiAbsoluteStr<S>> {
    type Target = RiAbsoluteString<S>;

    fn try_to_dedicated_string(&self) -> Result<Self::Target, TryReserveError> {
        let s = self.try_to_string()?;
        Ok(TryFrom::try_from(s).expect("[validity] the normalization result must be a valid IRI"))
    }
}

#[cfg(feature = "alloc")]
impl<S: Spec> From<Normalized<'_, RiAbsoluteStr<S>>> for RiAbsoluteString<S> {
    #[inline]
    fn from(v: Normalized<'_, RiAbsoluteStr<S>>) -> Self {
        v.to_dedicated_string()
    }
}

#[cfg(feature = "alloc")]
impl<S: Spec> From<&Normalized<'_, RiAbsoluteStr<S>>> for RiAbsoluteString<S> {
    #[inline]
    fn from(v: &Normalized<'_, RiAbsoluteStr<S>>) -> Self {
        v.to_dedicated_string()
    }
}
