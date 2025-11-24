//! Validating parsers for non-trusted (possibly invalid) input.

mod authority;
mod path;

use crate::parser::char;
use crate::parser::str::{
    find_split, find_split2_hole, find_split_hole, satisfy_chars_with_pct_encoded,
};
use crate::spec::Spec;
use crate::validate::Error;

use self::authority::validate_authority;
pub(crate) use self::authority::{validate_host, validate_userinfo};
pub(crate) use self::path::validate_path;
use self::path::{
    validate_path_abempty, validate_path_absolute_authority_absent,
    validate_path_relative_authority_absent,
};

/// Returns `Ok(_)` if the string matches `scheme`.
pub(crate) fn validate_scheme(i: &str) -> Result<(), Error> {
    debug_assert!(!i.is_empty());
    let bytes = i.as_bytes();
    if bytes[0].is_ascii_alphabetic()
        && bytes[1..]
            .iter()
            .all(|&b| b.is_ascii() && char::is_ascii_scheme_continue(b))
    {
        Ok(())
    } else {
        Err(Error::new())
    }
}

/// Returns `Ok(_)` if the string matches `query` or `iquery`.
pub(crate) fn validate_query<S: Spec>(i: &str) -> Result<(), Error> {
    let is_valid =
        satisfy_chars_with_pct_encoded(i, char::is_ascii_frag_query, char::is_nonascii_query::<S>);
    if is_valid {
        Ok(())
    } else {
        Err(Error::new())
    }
}

/// Returns `Ok(_)` if the string matches `authority path-abempty` rule sequence.
fn validate_authority_path_abempty<S: Spec>(i: &str) -> Result<(), Error> {
    let (maybe_authority, maybe_path) = match find_split(i, b'/') {
        Some(v) => v,
        None => (i, ""),
    };
    validate_authority::<S>(maybe_authority)?;
    validate_path_abempty::<S>(maybe_path)
}

/// Returns `Ok(_)` if the string matches `URI`/`IRI` rules.
#[inline]
pub(crate) fn validate_uri<S: Spec>(i: &str) -> Result<(), Error> {
    validate_uri_reference_common::<S>(i, UriReferenceRule::Absolute)
}

/// Returns `Ok(_)` if the string matches `URI-reference`/`IRI-reference` rules.
#[inline]
pub(crate) fn validate_uri_reference<S: Spec>(i: &str) -> Result<(), Error> {
    validate_uri_reference_common::<S>(i, UriReferenceRule::Any)
}

/// Returns `Ok(_)` if the string matches `absolute-URI`/`absolute-IRI` rules.
#[inline]
pub(crate) fn validate_absolute_uri<S: Spec>(i: &str) -> Result<(), Error> {
    validate_uri_reference_common::<S>(i, UriReferenceRule::AbsoluteWithoutFragment)
}

/// Syntax rule for URI/IRI references.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum UriReferenceRule {
    /// `URI` and `IRI`.
    ///
    /// This can have a fragment.
    Absolute,
    /// `absolute-URI` and `absolute-IRI`.
    ///
    /// This cannot have a fragment.
    AbsoluteWithoutFragment,
    /// `URI-reference` and `IRI-reference`.
    ///
    /// This can be relative.
    Any,
}

impl UriReferenceRule {
    /// Returns `true` is the relative reference is allowed.
    #[inline]
    #[must_use]
    fn is_relative_allowed(self) -> bool {
        self == Self::Any
    }

    /// Returns `true` is the fragment part is allowed.
    #[inline]
    #[must_use]
    fn is_fragment_allowed(self) -> bool {
        matches!(self, Self::Absolute | Self::Any)
    }
}

/// Returns `Ok(_)` if the string matches `URI-reference`/`IRI-reference` rules.
fn validate_uri_reference_common<S: Spec>(
    i: &str,
    ref_rule: UriReferenceRule,
) -> Result<(), Error> {
    // Validate `scheme ":"`.
    let (i, _scheme) = match find_split_hole(i, b':') {
        None => {
            if ref_rule.is_relative_allowed() {
                return validate_relative_ref::<S>(i);
            } else {
                return Err(Error::new());
            }
        }
        Some(("", _)) => return Err(Error::new()),
        Some((maybe_scheme, rest)) => {
            if validate_scheme(maybe_scheme).is_err() {
                // The string before the first colon is not a scheme.
                // Falling back to `relative-ref` parsing.
                if ref_rule.is_relative_allowed() {
                    return validate_relative_ref::<S>(i);
                } else {
                    return Err(Error::new());
                }
            }
            (rest, maybe_scheme)
        }
    };

    // Validate `hier-part`.
    let after_path = match i.strip_prefix("//") {
        Some(i) => {
            let (maybe_authority_path, after_path) = match find_split2_hole(i, b'?', b'#') {
                Some((maybe_authority_path, c, rest)) => (maybe_authority_path, Some((c, rest))),
                None => (i, None),
            };
            validate_authority_path_abempty::<S>(maybe_authority_path)?;
            after_path
        }
        None => {
            let (maybe_path, after_path) = match find_split2_hole(i, b'?', b'#') {
                Some((maybe_path, c, rest)) => (maybe_path, Some((c, rest))),
                None => (i, None),
            };
            // Authority is absent.
            validate_path_absolute_authority_absent::<S>(maybe_path)?;
            after_path
        }
    };

    // Validate `[ "?" query ] [ "#" fragment ]`.
    if let Some((first, rest)) = after_path {
        validate_after_path::<S>(first, rest, ref_rule.is_fragment_allowed())?;
    }
    Ok(())
}

/// Returns `Ok(_)` if the string matches `relative-ref`/`irelative-ref` rules.
pub(crate) fn validate_relative_ref<S: Spec>(i: &str) -> Result<(), Error> {
    // Validate `relative-part`.
    let after_path = match i.strip_prefix("//") {
        Some(i) => {
            let (maybe_authority_path, after_path) = match find_split2_hole(i, b'?', b'#') {
                Some((maybe_authority_path, c, rest)) => (maybe_authority_path, Some((c, rest))),
                None => (i, None),
            };
            validate_authority_path_abempty::<S>(maybe_authority_path)?;
            after_path
        }
        None => {
            let (maybe_path, after_path) = match find_split2_hole(i, b'?', b'#') {
                Some((maybe_path, c, rest)) => (maybe_path, Some((c, rest))),
                None => (i, None),
            };
            // Authority is absent.
            validate_path_relative_authority_absent::<S>(maybe_path)?;
            after_path
        }
    };

    // Validate `[ "?" query ] [ "#" fragment ]`.
    if let Some((first, rest)) = after_path {
        validate_after_path::<S>(first, rest, true)?;
    }
    Ok(())
}

/// Returns `Ok(_)` if the string matches `[ "?" query ] [ "#" fragment ]` (or IRI version).
fn validate_after_path<S: Spec>(first: u8, rest: &str, accept_fragment: bool) -> Result<(), Error> {
    let (maybe_query, maybe_fragment) = if first == b'?' {
        match find_split_hole(rest, b'#') {
            Some(v) => v,
            None => (rest, ""),
        }
    } else {
        debug_assert_eq!(first, b'#');
        ("", rest)
    };
    validate_query::<S>(maybe_query)?;
    if !accept_fragment && !maybe_fragment.is_empty() {
        return Err(Error::new());
    }
    validate_fragment::<S>(maybe_fragment)
}

/// Returns `Ok(_)` if the string matches `fragment`/`ifragment` rules.
pub(crate) fn validate_fragment<S: Spec>(i: &str) -> Result<(), Error> {
    let is_valid = satisfy_chars_with_pct_encoded(
        i,
        char::is_ascii_frag_query,
        char::is_nonascii_fragment::<S>,
    );
    if is_valid {
        Ok(())
    } else {
        Err(Error::new())
    }
}
