//! Selects full or incremental review evidence before profile retention.
//! Selection only proposes the next cursor; callers commit it with reviewed history.
//! Hosts must invalidate cursors when collection settings or history offsets change.

use crate::ConversationTranscriptEntry;

/// End of the collected transcript in one host-owned review-history generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranscriptCursor {
    pub parent_history_version: u64,
    pub transcript_entry_count: usize,
}

/// Whether to start a transcript or continue from previously reviewed evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptMode {
    Full,
    Delta { cursor: TranscriptCursor },
}

/// Selected evidence and its original numbering, before rendering or admission.
pub enum TranscriptSelection<'a> {
    Full(&'a [ConversationTranscriptEntry]),
    Delta {
        entries: &'a [ConversationTranscriptEntry],
        offset: usize,
    },
}

impl TranscriptMode {
    /// Falls back to full evidence if the saved cursor no longer addresses this history.
    /// The returned cursor counts collected entries, including any later omitted by a profile.
    pub fn select(
        self,
        entries: &[ConversationTranscriptEntry],
        parent_history_version: u64,
    ) -> (TranscriptSelection<'_>, TranscriptCursor) {
        let next_cursor = TranscriptCursor {
            parent_history_version,
            transcript_entry_count: entries.len(),
        };
        let selection = match self {
            Self::Delta { cursor }
                if cursor.parent_history_version == parent_history_version
                    && cursor.transcript_entry_count <= entries.len() =>
            {
                TranscriptSelection::Delta {
                    entries: &entries[cursor.transcript_entry_count..],
                    offset: cursor.transcript_entry_count,
                }
            }
            Self::Full | Self::Delta { .. } => TranscriptSelection::Full(entries),
        };
        (selection, next_cursor)
    }
}

#[cfg(test)]
#[path = "cursor_tests.rs"]
mod tests;
