//! Checks that profile retention cannot move the collected-transcript cursor backwards.

use super::*;
use crate::ContextProfile;
use crate::ConversationTranscriptEntryKind;
use pretty_assertions::assert_eq;

#[test]
fn delta_cursor_tracks_collected_entries_across_sliding_window_retention() {
    let entries = ["first", "second", "third", "fourth"]
        .into_iter()
        .map(|text| ConversationTranscriptEntry {
            kind: ConversationTranscriptEntryKind::Assistant,
            text: text.to_owned(),
            original_bytes: text.len(),
            retained_source: None,
        })
        .collect::<Vec<_>>();
    let mut profile = ContextProfile::asynchronous();
    profile.retention.max_recent_non_user_entries = 1;

    let (selection, cursor) =
        TranscriptMode::Full.select(&entries[..3], /*parent_history_version*/ 7);
    let TranscriptSelection::Full(initial) = selection else {
        panic!("first request must select a full transcript");
    };
    let initial = profile.render_transcript(initial, /*entry_number_offset*/ 0);
    assert_eq!(
        initial
            .items
            .into_iter()
            .map(|item| item.content)
            .collect::<Vec<_>>(),
        vec!["[3] assistant: third\n"],
    );

    let (selection, next_cursor) =
        TranscriptMode::Delta { cursor }.select(&entries, /*parent_history_version*/ 7);
    let TranscriptSelection::Delta {
        entries: delta,
        offset,
    } = selection
    else {
        panic!("an append must select only the new entry");
    };
    let delta = profile.render_transcript(delta, offset);
    assert_eq!(
        (
            delta
                .items
                .into_iter()
                .map(|item| item.content)
                .collect::<Vec<_>>(),
            next_cursor,
        ),
        (
            vec!["[4] assistant: fourth\n".to_owned()],
            TranscriptCursor {
                parent_history_version: 7,
                transcript_entry_count: 4,
            },
        ),
    );
}
