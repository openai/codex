//! Rendering surfaces supported by streaming controllers.
//!
//! Inline terminals can only append stable rows, so their controllers retain
//! the existing queue and table-holdback behavior. Retained viewports own the
//! rendered transcript and can replace a live entry in place, so their entire
//! completed source remains mutable until finalization.

/// Determines whether a stream emits immutable rows or exposes one mutable render.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StreamSurface {
    /// Emit stable rendered rows into terminal scrollback.
    Inline,
    /// Keep the full completed source mutable in an application-owned viewport.
    Retained,
}

impl StreamSurface {
    pub(super) fn is_retained(self) -> bool {
        self == Self::Retained
    }
}
