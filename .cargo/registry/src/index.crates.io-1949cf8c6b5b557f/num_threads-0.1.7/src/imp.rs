//! Fallback if no OS matches.

use std::num::NonZeroUsize;

pub(crate) fn num_threads() -> Option<NonZeroUsize> {
    None
}
