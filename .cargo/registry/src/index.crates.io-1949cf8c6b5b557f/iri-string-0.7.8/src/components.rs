//! Components of IRIs.

mod authority;

use core::num::NonZeroUsize;
use core::ops::{Range, RangeFrom, RangeTo};

use crate::parser::trusted as trusted_parser;
use crate::spec::Spec;
use crate::types::RiReferenceStr;

pub use self::authority::AuthorityComponents;

/// Positions to split an IRI into components.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Splitter {
    /// Scheme end.
    scheme_end: Option<NonZeroUsize>,
    /// Authority end.
    ///
    /// Note that absence of the authority and the empty authority is
    /// distinguished.
    authority_end: Option<NonZeroUsize>,
    /// Query start (after the leading `?`).
    query_start: Option<NonZeroUsize>,
    /// Fragment start (after the leading `#`).
    fragment_start: Option<NonZeroUsize>,
}

impl Splitter {
    /// Creates a new splitter.
    #[inline]
    #[must_use]
    pub(crate) fn new(
        scheme_end: Option<NonZeroUsize>,
        authority_end: Option<NonZeroUsize>,
        query_start: Option<NonZeroUsize>,
        fragment_start: Option<NonZeroUsize>,
    ) -> Self {
        Self {
            scheme_end,
            authority_end,
            query_start,
            fragment_start,
        }
    }

    /// Decomposes an IRI into five major components: scheme, authority, path, query, and fragment.
    #[must_use]
    fn split_into_major(
        self,
        s: &str,
    ) -> (Option<&str>, Option<&str>, &str, Option<&str>, Option<&str>) {
        let (scheme, next_of_scheme) = match self.scheme_end {
            // +1: ":".len()
            Some(end) => (Some(&s[..end.get()]), end.get() + 1),
            None => (None, 0),
        };
        let (authority, next_of_authority) = match self.authority_end {
            // +2: "//".len()
            Some(end) => (Some(&s[(next_of_scheme + 2)..end.get()]), end.get()),
            None => (None, next_of_scheme),
        };
        let (fragment, end_of_prev_of_fragment) = match self.fragment_start {
            // -1: "#".len()
            Some(start) => (Some(&s[start.get()..]), start.get() - 1),
            None => (None, s.len()),
        };
        let (query, end_of_path) = match self.query_start {
            Some(start) => (
                Some(&s[start.get()..end_of_prev_of_fragment]),
                // -1: "?".len()
                start.get() - 1,
            ),
            None => (None, end_of_prev_of_fragment),
        };
        let path = &s[next_of_authority..end_of_path];
        (scheme, authority, path, query, fragment)
    }

    /// Returns the range for the scheme part.
    #[inline]
    #[must_use]
    fn scheme_range(self) -> Option<RangeTo<usize>> {
        self.scheme_end.map(|end| ..end.get())
    }

    /// Returns the scheme as a string.
    #[inline]
    #[must_use]
    pub(crate) fn scheme_str<'a>(&self, s: &'a str) -> Option<&'a str> {
        self.scheme_range().map(|range| &s[range])
    }

    /// Returns true if the IRI has a scheme part, false otherwise.
    #[inline]
    #[must_use]
    pub(crate) fn has_scheme(&self) -> bool {
        self.scheme_end.is_some()
    }

    /// Returns the range for the authority part.
    #[inline]
    #[must_use]
    fn authority_range(self) -> Option<Range<usize>> {
        let end = self.authority_end?.get();
        // 2: "//".len()
        // +3: "://".len()
        let start = self.scheme_end.map_or(2, |v| v.get() + 3);
        Some(start..end)
    }

    /// Returns the authority as a string.
    #[inline]
    #[must_use]
    pub(crate) fn authority_str<'a>(&self, s: &'a str) -> Option<&'a str> {
        self.authority_range().map(|range| &s[range])
    }

    /// Returns true if the IRI has an authority part, false otherwise.
    #[inline]
    #[must_use]
    pub(crate) fn has_authority(&self) -> bool {
        self.authority_end.is_some()
    }

    /// Returns the range for the path part.
    #[inline]
    #[must_use]
    fn path_range(self, full_len: usize) -> Range<usize> {
        // -1: "?".len() and "#".len()
        let end = self
            .query_start
            .or(self.fragment_start)
            .map_or(full_len, |v| v.get() - 1);
        let start = self.authority_end.map_or_else(
            // +1: ":".len()
            || self.scheme_end.map_or(0, |v| v.get() + 1),
            NonZeroUsize::get,
        );

        start..end
    }

    /// Returns the path as a string.
    #[inline]
    #[must_use]
    pub(crate) fn path_str<'a>(&self, s: &'a str) -> &'a str {
        &s[self.path_range(s.len())]
    }

    /// Returns true if the path part of the IRI is empty.
    #[inline]
    #[must_use]
    pub(crate) fn is_path_empty(&self, full_len: usize) -> bool {
        self.path_range(full_len).is_empty()
    }

    /// Returns the range for the query part excluding a prefix `?`.
    #[inline]
    #[must_use]
    fn query_range(self, full_len: usize) -> Option<Range<usize>> {
        let start = self.query_start?.get();
        // -1: "#".len()
        let end = self.fragment_start.map_or(full_len, |v| v.get() - 1);

        Some(start..end)
    }

    /// Returns the query as a string.
    #[inline]
    #[must_use]
    pub(crate) fn query_str<'a>(&self, s: &'a str) -> Option<&'a str> {
        self.query_range(s.len()).map(|range| &s[range])
    }

    /// Returns true if the IRI has a query part, false otherwise.
    #[inline]
    #[must_use]
    pub(crate) fn has_query(&self) -> bool {
        self.query_start.is_some()
    }

    /// Returns the range for the fragment part excluding a prefix `#`.
    #[inline]
    #[must_use]
    pub(crate) fn fragment_range(self) -> Option<RangeFrom<usize>> {
        self.fragment_start.map(|v| v.get()..)
    }

    /// Returns the fragment as a string.
    #[inline]
    #[must_use]
    pub(crate) fn fragment_str<'a>(&self, s: &'a str) -> Option<&'a str> {
        self.fragment_range().map(|range| &s[range])
    }
}

/// Components of an IRI reference.
///
/// See <https://tools.ietf.org/html/rfc3986#section-5.2.2>.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RiReferenceComponents<'a, S: Spec> {
    /// Original complete string.
    pub(crate) iri: &'a RiReferenceStr<S>,
    /// Positions to split the IRI into components.
    pub(crate) splitter: Splitter,
}

impl<'a, S: Spec> RiReferenceComponents<'a, S> {
    /// Returns five major components: scheme, authority, path, query, and fragment.
    #[inline]
    #[must_use]
    pub(crate) fn to_major(
        self,
    ) -> (
        Option<&'a str>,
        Option<&'a str>,
        &'a str,
        Option<&'a str>,
        Option<&'a str>,
    ) {
        self.splitter.split_into_major(self.iri.as_str())
    }

    /// Returns the IRI reference.
    #[inline]
    #[must_use]
    pub(crate) fn iri(&self) -> &'a RiReferenceStr<S> {
        self.iri
    }

    /// Returns the scheme as a string.
    #[inline]
    #[must_use]
    pub(crate) fn scheme_str(&self) -> Option<&str> {
        self.splitter.scheme_str(self.iri.as_str())
    }

    /// Returns the authority as a string.
    #[inline]
    #[must_use]
    pub(crate) fn authority_str(&self) -> Option<&str> {
        self.splitter.authority_str(self.iri.as_str())
    }

    /// Returns the path as a string.
    #[inline]
    #[must_use]
    pub(crate) fn path_str(&self) -> &str {
        self.splitter.path_str(self.iri.as_str())
    }

    /// Returns the query as a string.
    #[inline]
    #[must_use]
    pub(crate) fn query_str(&self) -> Option<&str> {
        self.splitter.query_str(self.iri.as_str())
    }
}

impl<'a, S: Spec> From<&'a RiReferenceStr<S>> for RiReferenceComponents<'a, S> {
    #[inline]
    fn from(s: &'a RiReferenceStr<S>) -> Self {
        trusted_parser::decompose_iri_reference(s)
    }
}
