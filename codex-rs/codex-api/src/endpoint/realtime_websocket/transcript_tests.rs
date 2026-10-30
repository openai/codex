//! V3 transcript reconciliation across interleaved speakers, reconnects, and turn boundaries.

use super::*;
use codex_protocol::protocol::RealtimeTranscriptDone;
use pretty_assertions::assert_eq;

fn events(transcript_state: RealtimeTranscriptState) -> RealtimeWebsocketEvents {
    let (_, rx_message) = async_channel::unbounded();
    RealtimeWebsocketEvents {
        rx_message,
        pending_events: Arc::new(Mutex::new(VecDeque::new())),
        transcript_state,
        event_parser: RealtimeEventParser::FramelessBidi,
        is_closed: Arc::new(AtomicBool::new(false)),
    }
}

fn entry(role: &str, text: &str) -> RealtimeTranscriptEntry {
    RealtimeTranscriptEntry {
        role: role.to_string(),
        text: text.to_string(),
    }
}

async fn observe(events: &RealtimeWebsocketEvents, mut event: RealtimeEvent) {
    let original = event.clone();
    events.update_active_transcript(&mut event).await;
    assert_eq!(
        event, original,
        "live transcript events must pass through unchanged"
    );
}

#[tokio::test]
async fn interleaved_transcripts_reconcile_across_reconnect_in_either_completion_order() {
    for assistant_finishes_first in [false, true] {
        let state = RealtimeTranscriptState::default();
        let first = events(state.clone());
        for event in [
            RealtimeEvent::InputTranscriptDelta(RealtimeTranscriptDelta {
                delta: "What can ".into(),
            }),
            RealtimeEvent::OutputTranscriptDelta(RealtimeTranscriptDelta {
                delta: "A bunch ".into(),
            }),
            RealtimeEvent::InputTranscriptDelta(RealtimeTranscriptDelta {
                delta: "you do?".into(),
            }),
            RealtimeEvent::OutputTranscriptDelta(RealtimeTranscriptDelta {
                delta: "of stuff".into(),
            }),
        ] {
            observe(&first, event).await;
        }
        let reconnected = events(state);
        let user = RealtimeEvent::InputTranscriptDone(RealtimeTranscriptDone {
            text: "What can you do?".into(),
        });
        let assistant = RealtimeEvent::OutputTranscriptDone(RealtimeTranscriptDone {
            text: "A bunch of stuff, including coding.".into(),
        });
        let finals = if assistant_finishes_first {
            [assistant, user]
        } else {
            [user, assistant]
        };
        for event in finals {
            observe(&reconnected, event).await;
        }
        assert_eq!(
            reconnected.take_transcript_tail().await,
            vec![
                entry("user", "What can you do?"),
                entry("assistant", "A bunch of stuff, including coding."),
            ]
        );
        assert_eq!(reconnected.take_transcript_tail().await, vec![]);
    }
}

#[tokio::test]
async fn completed_utterances_keep_repeated_speech_and_final_only_turns_distinct() {
    let events = events(RealtimeTranscriptState::default());
    for role in ["user", "assistant"] {
        for with_delta in [true, true, false] {
            if with_delta {
                let delta = RealtimeTranscriptDelta {
                    delta: "Again".into(),
                };
                observe(
                    &events,
                    if role == "user" {
                        RealtimeEvent::InputTranscriptDelta(delta)
                    } else {
                        RealtimeEvent::OutputTranscriptDelta(delta)
                    },
                )
                .await;
            }
            let done = RealtimeTranscriptDone {
                text: "Again.".into(),
            };
            observe(
                &events,
                if role == "user" {
                    RealtimeEvent::InputTranscriptDone(done)
                } else {
                    RealtimeEvent::OutputTranscriptDone(done)
                },
            )
            .await;
            // An empty delta must not reopen the utterance we just completed.
            let empty = RealtimeTranscriptDelta {
                delta: String::new(),
            };
            observe(
                &events,
                if role == "user" {
                    RealtimeEvent::InputTranscriptDelta(empty)
                } else {
                    RealtimeEvent::OutputTranscriptDelta(empty)
                },
            )
            .await;
        }
        assert_eq!(
            events.take_transcript_tail().await,
            vec![entry(role, "Again."); 3]
        );
    }
}
