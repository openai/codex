/*!
This module defines a few quickcheck properties for substring search.

It also provides a forward and reverse macro for conveniently defining
quickcheck tests that run these properties over any substring search
implementation.
*/

use crate::tests::substring::naive;

/// $fwd is a `impl FnMut(haystack, needle) -> Option<Option<usize>>`. When the
/// routine returns `None`, then it's skipped, which is useful for substring
/// implementations that don't work for all inputs.
#[macro_export]
macro_rules! define_substring_forward_quickcheck {
    ($fwd:expr) => {
        #[cfg(not(miri))]
        quickcheck::quickcheck! {
            fn qc_fwd_prefix_is_substring(bs: alloc::vec::Vec<u8>) -> bool {
                crate::tests::substring::prop::prefix_is_substring(&bs, $fwd)
            }

            fn qc_fwd_suffix_is_substring(bs: alloc::vec::Vec<u8>) -> bool {
                crate::tests::substring::prop::suffix_is_substring(&bs, $fwd)
            }

            fn qc_fwd_matches_naive(
                haystack: alloc::vec::Vec<u8>,
                needle: alloc::vec::Vec<u8>
            ) -> bool {
                crate::tests::substring::prop::same_as_naive(
                    false,
                    &haystack,
                    &needle,
                    $fwd,
                )
            }
        }
    };
}

/// $rev is a `impl FnMut(haystack, needle) -> Option<Option<usize>>`. When the
/// routine returns `None`, then it's skipped, which is useful for substring
/// implementations that don't work for all inputs.
#[macro_export]
macro_rules! define_substring_reverse_quickcheck {
    ($rev:expr) => {
        #[cfg(not(miri))]
        quickcheck::quickcheck! {
            fn qc_rev_prefix_is_substring(bs: alloc::vec::Vec<u8>) -> bool {
                crate::tests::substring::prop::prefix_is_substring(&bs, $rev)
            }

            fn qc_rev_suffix_is_substring(bs: alloc::vec::Vec<u8>) -> bool {
                crate::tests::substring::prop::suffix_is_substring(&bs, $rev)
            }

            fn qc_rev_matches_naive(
                haystack: alloc::vec::Vec<u8>,
                needle: alloc::vec::Vec<u8>
            ) -> bool {
                crate::tests::substring::prop::same_as_naive(
                    true,
                    &haystack,
                    &needle,
                    $rev,
                )
            }
        }
    };
}

/// Check that every prefix of the given byte string is a substring.
pub(crate) fn prefix_is_substring(
    bs: &[u8],
    mut search: impl FnMut(&[u8], &[u8]) -> Option<Option<usize>>,
) -> bool {
    for i in 0..bs.len().saturating_sub(1) {
        let prefix = &bs[..i];
        let result = match search(bs, prefix) {
            None => continue,
            Some(result) => result,
        };
        if !result.is_some() {
            return false;
        }
    }
    true
}

/// Check that every suffix of the given byte string is a substring.
pub(crate) fn suffix_is_substring(
    bs: &[u8],
    mut search: impl FnMut(&[u8], &[u8]) -> Option<Option<usize>>,
) -> bool {
    for i in 0..bs.len().saturating_sub(1) {
        let suffix = &bs[i..];
        let result = match search(bs, suffix) {
            None => continue,
            Some(result) => result,
        };
        if !result.is_some() {
            return false;
        }
    }
    true
}

/// Check that naive substring search matches the result of the given search
/// algorithm.
pub(crate) fn same_as_naive(
    reverse: bool,
    haystack: &[u8],
    needle: &[u8],
    mut search: impl FnMut(&[u8], &[u8]) -> Option<Option<usize>>,
) -> bool {
    let result = match search(haystack, needle) {
        None => return true,
        Some(result) => result,
    };
    if reverse {
        result == naive::rfind(haystack, needle)
    } else {
        result == naive::find(haystack, needle)
    }
}
