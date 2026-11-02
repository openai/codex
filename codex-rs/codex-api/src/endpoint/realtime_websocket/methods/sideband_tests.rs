#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio::time::timeout;

fn connection() -> (
    RealtimeWebsocketConnection,
    mpsc::Sender<String>,
    mpsc::Receiver<String>,
) {
    let (events, incoming) = mpsc::channel(1);
    let (outgoing, commands) = mpsc::channel(1);
    (
        RealtimeWebsocketConnection::from_sideband(
            RealtimeSidebandTransport { incoming, outgoing },
            RealtimeEventParser::FramelessBidi,
        ),
        events,
        commands,
    )
}

/// A blocked command receiver must not prevent receiving a realtime event on the other half.
#[tokio::test]
async fn stalled_output_does_not_block_input() {
    let (connection, events, commands) = connection();
    let writer = connection.writer();
    writer.send_payload("first".into()).await.unwrap();
    let blocked = tokio::spawn(async move { writer.send_payload("second".into()).await });
    events
        .send(r#"{"type":"session.started","session":{"id":"call"}}"#.into())
        .await
        .unwrap();
    assert!(
        timeout(Duration::from_secs(1), connection.next_event())
            .await
            .unwrap()
            .unwrap()
            .is_some()
    );
    assert!(!blocked.is_finished());
    drop(commands);
    assert!(
        timeout(Duration::from_secs(1), blocked)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert!(
        timeout(Duration::from_secs(1), connection.next_event())
            .await
            .unwrap()
            .is_err()
    );
}

/// Ending the input stream closes the output too and never attempts reconnection.
#[tokio::test]
async fn input_half_close_ends_attachment() {
    let (connection, events, mut commands) = connection();
    drop(events);
    assert!(
        timeout(Duration::from_secs(1), connection.next_event())
            .await
            .unwrap()
            .is_err()
    );
    assert_eq!(
        timeout(Duration::from_secs(1), commands.recv())
            .await
            .unwrap(),
        None
    );
    assert!(
        connection
            .writer()
            .send_payload("late".into())
            .await
            .is_err()
    );
}

/// A caller dropping its response half releases an idle transport without waiting for another command.
#[tokio::test]
async fn response_half_close_ends_idle_attachment() {
    let (_connection, events, commands) = connection();
    drop(commands);
    timeout(Duration::from_secs(1), events.closed())
        .await
        .unwrap();
}

/// Oversized payloads fail the transport before entering the parser or caller's outgoing queue.
#[tokio::test]
async fn oversized_payloads_close_both_directions() {
    let (input_connection, events, mut commands) = connection();
    events
        .send("x".repeat(MAX_MESSAGE_BYTES + 1))
        .await
        .unwrap();
    assert!(
        timeout(Duration::from_secs(1), input_connection.next_event())
            .await
            .unwrap()
            .is_err()
    );
    assert_eq!(
        timeout(Duration::from_secs(1), commands.recv())
            .await
            .unwrap(),
        None
    );
    let (output_connection, _events, mut commands) = connection();
    assert!(
        output_connection
            .writer()
            .send_payload("x".repeat(MAX_MESSAGE_BYTES + 1))
            .await
            .is_err()
    );
    assert_eq!(
        timeout(Duration::from_secs(1), commands.recv())
            .await
            .unwrap(),
        None
    );
}

/// A final transcript accepted before peer hangup must reach both the parser and transcript tail.
#[tokio::test(flavor = "current_thread")]
async fn queued_transcript_survives_peer_hangup() {
    for _ in 0..64 {
        let (connection, events, commands) = connection();
        events
            .try_send(r#"{"type":"input_transcript.added","item":{"text":"last words"}}"#.into())
            .unwrap();
        drop(events);
        drop(commands);
        assert_eq!(
            timeout(Duration::from_secs(1), connection.next_event())
                .await
                .unwrap()
                .unwrap(),
            Some(
                codex_protocol::protocol::RealtimeEvent::InputTranscriptDelta(
                    codex_protocol::protocol::RealtimeTranscriptDelta {
                        delta: "last words".into()
                    }
                )
            )
        );
        assert_eq!(
            connection.events().take_transcript_tail().await,
            vec![codex_protocol::protocol::RealtimeTranscriptEntry {
                role: "user".into(),
                text: "last words".into()
            }]
        );
    }
}

/// Explicit shutdown preserves a frame blocked on parser capacity and does not wait for an unsent permit.
#[tokio::test]
async fn input_stop_drains_backpressure_without_waiting_for_reserved_permit() {
    let (events, incoming) = mpsc::channel(64);
    let (outgoing, _commands) = mpsc::channel(1);
    let connection = RealtimeWebsocketConnection::from_sideband(
        RealtimeSidebandTransport { incoming, outgoing },
        RealtimeEventParser::FramelessBidi,
    );
    let _permit = events.reserve().await.unwrap();
    for _ in 0..63 {
        events
            .try_send(r#"{"type":"input_transcript.added","item":{"text":"tail"}}"#.into())
            .unwrap();
    }
    timeout(Duration::from_secs(1), async {
        while events.capacity() < 33 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    connection.writer().close_sideband_input();
    for _ in 0..63 {
        assert_eq!(
            timeout(Duration::from_secs(1), connection.next_event())
                .await
                .unwrap()
                .unwrap(),
            Some(
                codex_protocol::protocol::RealtimeEvent::InputTranscriptDelta(
                    codex_protocol::protocol::RealtimeTranscriptDelta {
                        delta: "tail".into()
                    }
                )
            )
        );
    }
    assert!(
        timeout(Duration::from_secs(1), connection.next_event())
            .await
            .unwrap()
            .is_err()
    );
    assert!(events.is_closed());
}
