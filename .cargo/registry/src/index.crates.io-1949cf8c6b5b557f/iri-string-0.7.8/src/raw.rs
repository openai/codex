//! Raw IRI strings manipulation.
//!
//! Note that functions in this module may operates on raw `&str` types.
//! It is caller's responsilibility to guarantee that the given string satisfies the precondition.

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::string::String;

#[cfg(feature = "alloc")]
use crate::parser::trusted as trusted_parser;

/// Sets the fragment part to the given string.
///
/// Removes fragment part (and following `#` character) if `None` is given.
#[cfg(feature = "alloc")]
pub(crate) fn set_fragment(s: &mut String, fragment: Option<&str>) {
    remove_fragment(s);
    if let Some(fragment) = fragment {
        s.reserve(fragment.len() + 1);
        s.push('#');
        s.push_str(fragment);
    }
}

/// Removes the fragment part from the string.
#[cfg(feature = "alloc")]
#[inline]
pub(crate) fn remove_fragment(s: &mut String) {
    if let Some(colon_pos) = s.find('#') {
        s.truncate(colon_pos);
    }
}

/// Splits the string into the prefix and the fragment part.
///
/// A leading `#` character is truncated if the fragment part exists.
#[cfg(feature = "alloc")]
pub(crate) fn split_fragment_owned(mut s: String) -> (String, Option<String>) {
    let prefix_len = match trusted_parser::split_fragment(&s) {
        (_, None) => return (s, None),
        (prefix, Some(_fragment)) => prefix.len(),
    };

    // `+ 1` is for leading `#` character.
    let fragment = s.split_off(prefix_len + 1);
    // Current `s` contains a trailing `#` character, which should be removed.
    {
        // Remove a trailing `#`.
        let hash = s.pop();
        assert_eq!(hash, Some('#'));
    }
    assert_eq!(s.len(), prefix_len);

    (s, Some(fragment))
}
