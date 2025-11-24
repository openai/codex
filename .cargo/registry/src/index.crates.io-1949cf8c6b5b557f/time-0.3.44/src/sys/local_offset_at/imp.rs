//! A fallback for any OS not covered.

use crate::{OffsetDateTime, UtcOffset};

#[expect(clippy::missing_docs_in_private_items)]
#[inline]
pub(super) fn local_offset_at(_datetime: OffsetDateTime) -> Option<UtcOffset> {
    None
}
