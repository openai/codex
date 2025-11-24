//! Parsers for path.

use crate::parser::char;
use crate::parser::str::{find_split2_hole, satisfy_chars_with_pct_encoded};
use crate::spec::Spec;
use crate::validate::Error;

/// Returns `Ok(_)` if the string matches `path-abempty` or `ipath-abempty`.
pub(super) fn validate_path_abempty<S: Spec>(i: &str) -> Result<(), Error> {
    if i.is_empty() {
        return Ok(());
    }
    let i = match i.strip_prefix('/') {
        Some(rest) => rest,
        None => return Err(Error::new()),
    };
    let is_valid = satisfy_chars_with_pct_encoded(
        i,
        char::is_ascii_pchar_slash,
        S::is_nonascii_char_unreserved,
    );
    if is_valid {
        Ok(())
    } else {
        Err(Error::new())
    }
}

/// Returns `Ok(_)` if the string matches `hier-part` or `ihier-part` modulo
/// `"//" authority path-abempty`.
pub(super) fn validate_path_absolute_authority_absent<S: Spec>(i: &str) -> Result<(), Error> {
    if i.is_empty() {
        // `path-empty`.
        return Ok(());
    }
    if i.starts_with("//") {
        unreachable!("this case should be handled by the caller");
    }
    let is_valid = satisfy_chars_with_pct_encoded(
        i,
        char::is_ascii_pchar_slash,
        S::is_nonascii_char_unreserved,
    );
    if is_valid {
        Ok(())
    } else {
        Err(Error::new())
    }
}

/// Returns `Ok(_)` if the string matches `relative-part` or `irelative-part` modulo
/// `"//" authority path-abempty`.
pub(super) fn validate_path_relative_authority_absent<S: Spec>(i: &str) -> Result<(), Error> {
    if i.starts_with("//") {
        unreachable!("this case should be handled by the caller");
    }
    let is_valid = match find_split2_hole(i, b'/', b':') {
        Some((_, b'/', _)) | None => satisfy_chars_with_pct_encoded(
            i,
            char::is_ascii_pchar_slash,
            S::is_nonascii_char_unreserved,
        ),
        Some((_, c, _)) => {
            debug_assert_eq!(c, b':');
            // `foo:bar`-style. This does not match `path-noscheme`.
            return Err(Error::new());
        }
    };
    if is_valid {
        Ok(())
    } else {
        Err(Error::new())
    }
}

/// Returns `Ok(_)` if the string matches `path`/`ipath` rules.
pub(crate) fn validate_path<S: Spec>(i: &str) -> Result<(), Error> {
    if i.starts_with("//") {
        return Err(Error::new());
    }
    let is_valid = satisfy_chars_with_pct_encoded(
        i,
        char::is_ascii_pchar_slash,
        S::is_nonascii_char_unreserved,
    );
    if is_valid {
        Ok(())
    } else {
        Err(Error::new())
    }
}
