//! Desktop-facing live notifications and reconciled voice handoff context.

use super::*;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case(
    "A bunch ", "of stuff", "A bunch of stuff, including coding.",
    "A bunch of stuff, including coding."; "final expands partial"
)]
#[test_case(
    "Okay.", "I will check", "Okay.", "Okay.I will check";
    "delayed final preserves newer speech"
)]
#[tokio::test]
async fn websocket_v3_reconciles_handoff_transcripts_without_changing_live_notifications(
    first_delta: &str,
    second_delta: &str,
    assistant_final: &str,
    assistant_transcript: &str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let mut harness = RealtimeE2eHarness::new(
        RealtimeTestVersion::V1,
        main_loop_responses(vec![create_final_assistant_message_sse_response("Done.")?]),
        realtime_sideband(vec![open_realtime_sideband_connection(vec![vec![
            session_started("interleaved"),
            json!({"type": "input_transcript.added", "item": {"text": "What can you do?"}}),
            json!({"type": "output_transcript.added", "item": {"text": first_delta}}),
            json!({"type": "output_transcript.added", "item": {"text": second_delta}}),
            json!({"type": "turn.done", "turn": {"role": "user", "transcript": "What can you do?"}}),
            json!({"type": "turn.done", "turn": {"role": "assistant", "transcript": assistant_final}}),
            json!({
                "type": "delegation.created",
                "item": {
                    "id": "handoff",
                    "type": "delegation",
                    "target": "client",
                    "content": [{"type": "input_text", "text": "What can you do?"}]
                }
            }),
        ]])]),
    ).await?;
    harness
        .start_frameless_bidi_realtime(
            /*codex_response_handoff_mode*/ None,
            /*codex_response_handoff_channel_prefixes*/ None, /*initial_items*/ None,
        )
        .await?;

    for (role, delta) in [
        ("user", "What can you do?"),
        ("assistant", first_delta),
        ("assistant", second_delta),
    ] {
        assert_eq!(
            harness
                .read_notification::<ThreadRealtimeTranscriptDeltaNotification>(
                    "thread/realtime/transcript/delta"
                )
                .await?,
            ThreadRealtimeTranscriptDeltaNotification {
                thread_id: harness.thread_id.clone(),
                role: role.into(),
                delta: delta.into()
            },
        );
    }
    for (role, text) in [("user", "What can you do?"), ("assistant", assistant_final)] {
        assert_eq!(
            harness
                .read_notification::<ThreadRealtimeTranscriptDoneNotification>(
                    "thread/realtime/transcript/done"
                )
                .await?,
            ThreadRealtimeTranscriptDoneNotification {
                thread_id: harness.thread_id.clone(),
                role: role.into(),
                text: text.into()
            },
        );
    }
    assert_eq!(
        harness
            .read_notification::<ThreadRealtimeItemAddedNotification>("thread/realtime/itemAdded")
            .await?,
        ThreadRealtimeItemAddedNotification {
            thread_id: harness.thread_id.clone(),
            item: json!({
                "type": "handoff_request",
                "handoff_id": "handoff",
                "item_id": "handoff",
                "input_transcript": "What can you do?",
                "active_transcript": [
                    {"role": "user", "text": "What can you do?"},
                    {"role": "assistant", "text": assistant_transcript},
                ],
            }),
        },
    );
    harness
        .read_notification::<TurnCompletedNotification>("turn/completed")
        .await?;
    let requests = harness.main_loop_responses_requests().await?;
    assert_eq!(requests.len(), 1);
    assert!(response_request_contains_text(
        &requests[0],
        &format!(
            "<realtime_delegation>\n  <input>What can you do?</input>\n  <transcript_delta>user: What can you do?\nassistant: {assistant_transcript}</transcript_delta>\n</realtime_delegation>"
        ),
    ));
    harness.shutdown().await;
    Ok(())
}
