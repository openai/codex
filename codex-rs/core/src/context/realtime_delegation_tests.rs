use super::*;
use pretty_assertions::assert_eq;

#[test]
fn bounded_context_attributes_only_retained_events_in_source_order() {
    let middle = "<&é>".repeat(2048);
    let tail = "new context".repeat(600);
    let entries = [
        (Some("query"), RealtimeTranscriptRole::User, "do the thing"),
        (
            Some("dropped"),
            RealtimeTranscriptRole::Assistant,
            middle.as_str(),
        ),
        (
            Some("tail"),
            RealtimeTranscriptRole::Assistant,
            tail.as_str(),
        ),
    ];
    let (text, sources) = RealtimeDelegation::from_events(
        "do the thing",
        &entries,
        RealtimeDelegationSource::Handoff,
        /*preserve_transcript*/ false,
    )
    .unwrap();
    assert_eq!(sources, vec!["query", "tail"]);
    assert!(!text.contains("&lt;"));
    assert!(text.contains("<input>do the thing</input>"));
    assert!(text.len() < 9 * 1024);
}

#[test]
fn transcript_fallback_attributes_both_the_input_prefix_and_context_suffix() {
    let long = "x".repeat(9000);
    let entries = [
        (Some("first"), RealtimeTranscriptRole::User, "question"),
        (
            Some("middle"),
            RealtimeTranscriptRole::Assistant,
            long.as_str(),
        ),
        (Some("last"), RealtimeTranscriptRole::User, "followup"),
    ];
    let (text, sources) = RealtimeDelegation::from_events(
        "",
        &entries,
        RealtimeDelegationSource::Handoff,
        /*preserve_transcript*/ false,
    )
    .unwrap();
    assert_eq!(sources, vec!["first", "middle", "last"]);
    assert!(text.contains("<input>user: question"));
    assert!(text.contains("user: followup</transcript_delta>"));
}
