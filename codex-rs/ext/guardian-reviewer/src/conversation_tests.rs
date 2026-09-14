//! Checks that forks use committed history and progress while the next review runs.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn fork_keeps_committed_history_and_cursor_while_next_review_is_uncommitted() {
    let mut state = ConversationState::default();
    let first = TranscriptCursor {
        parent_history_version: 1,
        transcript_entry_count: 2,
    };
    state.complete_review(first);
    state.commit_snapshot(vec!["first review"]);
    let second = TranscriptCursor {
        transcript_entry_count: 4,
        ..first
    };
    state.complete_review(second);

    let (fork, history) = ConversationState::fork(state.snapshot().unwrap().clone());
    assert_eq!(
        (history, fork.cursor(), fork.completed_review_count()),
        (vec!["first review"], Some(first), 1),
    );

    state.commit_snapshot(vec!["first review", "second review"]);
    let (fork, history) = ConversationState::fork(state.snapshot().unwrap().clone());
    assert_eq!(
        (history, fork.cursor(), fork.completed_review_count()),
        (vec!["first review", "second review"], Some(second), 2),
    );
}
