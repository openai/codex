//! Parsers for trusted `authority` string.

use crate::components::AuthorityComponents;
use crate::parser::str::{find_split_hole, rfind_split2};

/// Decomposes the authority into `(userinfo, host, port)`.
///
/// The leading `:` is truncated.
///
/// # Precondition
///
/// The given string must be a valid IRI reference.
#[inline]
#[must_use]
pub(crate) fn decompose_authority(authority: &str) -> AuthorityComponents<'_> {
    let i = authority;
    let (i, host_start) = match find_split_hole(i, b'@') {
        Some((userinfo, rest)) => (rest, userinfo.len() + 1),
        None => (authority, 0),
    };
    let colon_port_len = match rfind_split2(i, b':', b']') {
        Some((_, suffix)) if suffix.starts_with(':') => suffix.len(),
        _ => 0,
    };
    let host_end = authority.len() - colon_port_len;

    AuthorityComponents {
        authority,
        host_start,
        host_end,
    }
}
