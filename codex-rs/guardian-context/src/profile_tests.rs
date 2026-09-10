//! Profile retention keeps the existing sync and async evidence priorities.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn profiles_preserve_distinct_retention_and_original_numbering() {
    let entries = [
        (ConversationTranscriptEntryKind::User, "inspect only"),
        (
            ConversationTranscriptEntryKind::ProtectedAssistant,
            "proposed action",
        ),
        (ConversationTranscriptEntryKind::Assistant, "working"),
    ]
    .into_iter()
    .map(|(kind, text)| ConversationTranscriptEntry {
        kind,
        text: text.to_owned(),
        original_bytes: text.len(),
    })
    .collect::<Vec<_>>();
    let mut sync = ContextProfile::synchronous();
    sync.retention.max_recent_non_user_entries = 1;
    let mut asynchronous = ContextProfile::asynchronous();
    asynchronous.retention.max_recent_non_user_entries = 1;
    let sync = sync.render_transcript(&entries, /*entry_number_offset*/ 7);
    let asynchronous = asynchronous.render_transcript(&entries, /*entry_number_offset*/ 0);
    assert_eq!(
        (sync.items, sync.omission_note),
        (
            vec![
                Budgeted::historical("[8] user: inspect only".to_owned()),
                Budgeted::optional(
                    "[10] assistant: working".to_owned(),
                    BudgetPriority::Commentary
                )
            ],
            Some("Some conversation entries were omitted.".to_owned()),
        ),
    );
    assert_eq!(
        (asynchronous.items, asynchronous.omission_note),
        (
            vec![
                Budgeted::historical("[1] user: inspect only\n".to_owned()),
                Budgeted::required("[2] assistant: proposed action\n".to_owned())
            ],
            None,
        ),
    );
    assert_eq!(
        asynchronous
            .truncations
            .into_iter()
            .map(|observation| (
                observation.component,
                observation.original_bytes,
                observation.retained_bytes,
            ))
            .collect::<Vec<_>>(),
        vec![("transcript_message", "working".len(), 0)],
    );
}

#[test]
fn profiles_reserve_the_newest_five_tool_entries_for_aggregate_enforcement() {
    let entries = (0..8)
        .map(|index| ConversationTranscriptEntry {
            kind: ConversationTranscriptEntryKind::ToolOutput("tool result".to_owned()),
            text: format!("result {index}"),
            original_bytes: 8,
        })
        .collect::<Vec<_>>();
    for profile in [
        ContextProfile::synchronous(),
        ContextProfile::asynchronous(),
    ] {
        let transcript = profile.render_transcript(&entries, /*entry_number_offset*/ 0);
        assert_eq!(
            transcript
                .items
                .iter()
                .map(|item| item.retention)
                .collect::<Vec<_>>(),
            vec![
                Retention::Optional(BudgetPriority::Tool),
                Retention::Optional(BudgetPriority::Tool),
                Retention::Optional(BudgetPriority::Tool),
                Retention::Required,
                Retention::Required,
                Retention::Required,
                Retention::Required,
                Retention::Required
            ]
        );
    }
}
