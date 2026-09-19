//! Effective transcript ownership for this running TUI, independent of backend feature updates.
//!
//! Terminal restrictions take precedence over the configured ownership preference. The launch-time
//! mode survives session changes so native scrollback never receives retained-only updates.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TranscriptMode {
    Owned,
    Terminal,
}

impl TranscriptMode {
    pub(crate) fn resolve(owned_enabled: bool, alternate_screen_enabled: bool) -> Self {
        if owned_enabled && alternate_screen_enabled {
            Self::Owned
        } else {
            Self::Terminal
        }
    }

    pub(crate) fn is_owned(self) -> bool {
        self == Self::Owned
    }
}
