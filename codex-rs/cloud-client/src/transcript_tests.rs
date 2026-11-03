//! Independent protobuf fixtures cover the public transcript contract.
use super::*;
use pretty_assertions::assert_eq;

fn message(tag: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    prost::encoding::encode_key(tag, prost::encoding::WireType::LengthDelimited, &mut out);
    prost::encoding::encode_varint(payload.len() as u64, &mut out);
    out.extend_from_slice(payload);
    out
}

fn notification(tag: u32, params: &[u8]) -> Vec<u8> {
    message(tag, &message(/*tag*/ 2, params))
}

const DELTA: &[u8] = b"\x0a\x01d\x12\x01i\x1a\x01t\x22\x01u";

#[tokio::test]
async fn attach_decodes_live_deltas_with_unknown_fields() {
    let fixture = fixture(response(
        [32, 37, 39]
            .into_iter()
            .map(|tag| {
                let mut payload = notification(tag, DELTA);
                payload.extend([0xa0, 0x06, 1]); // unknown field 100
                frame(&message(/*tag*/ 1, &payload))
            })
            .collect(),
        tonic::Status::ok(""),
    ))
    .await;
    let mut stream = fixture.client.attach("t").await.unwrap();
    for expected in [
        TranscriptUpdate::AgentMessageDelta as fn(ItemDelta) -> TranscriptUpdate,
        TranscriptUpdate::CommandOutputDelta,
        TranscriptUpdate::FileChangeOutputDelta,
    ] {
        let Event::Transcript(event) = stream.next().await.unwrap().unwrap() else {
            panic!("expected transcript event")
        };
        assert_eq!(format!("{event:?}"), "TranscriptEvent([redacted])");
        assert_eq!(event.thread_id, "t");
        assert_eq!(event.emitted_at_ms, None);
        assert_eq!(
            event.update,
            expected(ItemDelta {
                item_id: "i".into(),
                turn_id: "u".into(),
                delta: "d".into(),
            })
        );
    }
    assert!(stream.next().await.is_none());
}

#[test]
fn patches_preserve_order_kinds_and_move_paths() {
    let mut changes = Vec::new();
    for kind in [
        b"\x0a\x00".as_slice(),
        b"\x12\x00",
        b"\x1a\x03\x0a\x01m",
        b"\x22\x00",
    ] {
        changes.extend(message(
            /*tag*/ 1,
            &[
                b"\x0a\x01d".to_vec(),
                message(/*tag*/ 2, kind),
                b"\x1a\x01p".to_vec(),
            ]
            .concat(),
        ));
    }
    let params = [message(/*tag*/ 1, &changes), DELTA[3..].to_vec()].concat();
    let payload = notification(/*tag*/ 40, &params);
    let event = TranscriptEvent::decode("t", &payload).unwrap().unwrap();
    assert_eq!(
        event.update,
        TranscriptUpdate::FileChangePatchUpdated(PatchUpdate {
            item_id: "i".into(),
            turn_id: "u".into(),
            changes: [
                PatchKind::Add,
                PatchKind::Delete,
                PatchKind::Update {
                    move_path: Some("m".into())
                },
                PatchKind::Unknown,
            ]
            .into_iter()
            .map(|kind| FileUpdate {
                path: "p".into(),
                diff: "d".into(),
                kind,
            })
            .collect(),
        })
    );
}

#[test]
fn turn_events_preserve_status_failure_and_timing() {
    for (tag, status, expected) in [
        (23, 1, TurnStatus::Completed),
        (23, 2, TurnStatus::Interrupted),
        (21, 4, TurnStatus::InProgress),
        (23, 3, TurnStatus::Failed),
        (23, 99, TurnStatus::Unknown(99)),
    ] {
        let (timing, started_at, completed_at, duration_ms) = match expected {
            TurnStatus::InProgress => (b"\x38\x00".as_slice(), Some(0), None, None),
            TurnStatus::Completed => (
                b"\x08\x0d\x10\xf4\x03\x38\x0b".as_slice(),
                Some(11),
                Some(13),
                Some(500),
            ),
            _ => (&[][..], None, None, None),
        };
        let turn = [
            b"\x1a\x06\x0a\x01a\x1a\x01e\x22\x01u\x2a\x00\x40".to_vec(),
            vec![status],
            timing.to_vec(),
        ]
        .concat();
        let params = [b"\x0a\x01t".to_vec(), message(/*tag*/ 2, &turn)].concat();
        let event = TranscriptEvent::decode(
            "t",
            &message(tag, &[vec![8, 7], message(/*tag*/ 2, &params)].concat()),
        )
        .unwrap()
        .unwrap();
        let turn = TurnSummary {
            id: "u".into(),
            status: expected,
            started_at,
            completed_at,
            duration_ms,
            error: Some(TurnFailure {
                message: "e".into(),
                additional_details: Some("a".into()),
            }),
        };
        assert_eq!(event.emitted_at_ms, Some(7));
        assert_eq!(
            event.update,
            if tag == 21 {
                TranscriptUpdate::TurnStarted(turn)
            } else {
                TranscriptUpdate::TurnCompleted(turn)
            }
        );
    }
}

#[tokio::test]
async fn attach_preserves_opaque_notifications_and_continues() {
    let foreign_turn = b"\x0a\x01x\x12\x05\x22\x01u\x40\x01";
    let opaque = [
        message(/*tag*/ 100, b"unknown notification"),
        notification(
            /*tag*/ 27,
            b"\x0a\x02\x4a\x00\x10\x03\x1a\x01t\x22\x01u",
        ), // Unsupported MCP item.
        notification(/*tag*/ 21, foreign_turn),
        notification(/*tag*/ 23, foreign_turn),
        notification(/*tag*/ 32, &DELTA[..9]), // missing turn ID
    ];
    let valid = notification(/*tag*/ 32, DELTA);
    let fixture = fixture(response(
        opaque
            .iter()
            .chain([&valid])
            .map(|payload| frame(&message(/*tag*/ 1, payload)))
            .collect(),
        tonic::Status::ok(""),
    ))
    .await;
    let mut stream = fixture.client.attach("t").await.unwrap();
    for payload in opaque {
        let Some(Ok(Event::Notification(bytes))) = stream.next().await else {
            panic!("expected opaque notification");
        };
        assert_eq!(bytes.as_ref(), payload.as_slice());
    }
    let Some(Ok(Event::Transcript(event))) = stream.next().await else {
        panic!("expected transcript after opaque notifications");
    };
    assert_eq!(event.thread_id, "t");
    assert!(stream.next().await.is_none());
}

#[test]
fn decoder_keeps_incomplete_events_opaque_and_rejects_foreign_deltas() {
    let mut wrong_owner = DELTA.to_vec();
    wrong_owner[8] = b'x';
    for payload in [
        notification(/*tag*/ 32, &DELTA[3..]),
        notification(/*tag*/ 40, &DELTA[3..]),
        notification(/*tag*/ 23, b"\x0a\x01t"),
    ] {
        assert!(TranscriptEvent::decode("t", &payload).unwrap().is_none());
    }
    assert!(TranscriptEvent::decode("t", &notification(/*tag*/ 32, &wrong_owner)).is_err());
    assert!(
        TranscriptEvent::decode("t", &message(/*tag*/ 100, b""))
            .unwrap()
            .is_none()
    );
}

#[test]
fn notification_selection_merges_only_repeated_variants() {
    let partial = notification(/*tag*/ 32, &DELTA[..6]);
    let tail = notification(/*tag*/ 32, &DELTA[6..]);
    assert!(
        TranscriptEvent::decode("t", &[partial.clone(), tail.clone()].concat())
            .unwrap()
            .is_some()
    );
    assert!(
        TranscriptEvent::decode("t", &[partial, message(/*tag*/ 85, b""), tail].concat())
            .unwrap()
            .is_none()
    );
    assert!(
        TranscriptEvent::decode(
            "t",
            &[notification(/*tag*/ 32, DELTA), message(/*tag*/ 89, b"")].concat()
        )
        .unwrap()
        .is_none()
    );

    let envelope = message(/*tag*/ 2, DELTA);
    let split_inside_params = [
        message(/*tag*/ 32, &envelope[..4]),
        message(/*tag*/ 32, &envelope[4..]),
    ]
    .concat();
    assert!(TranscriptEvent::decode("t", &split_inside_params).is_err());
}

#[test]
fn public_payload_debug_is_redacted() {
    for formatted in [
        format!(
            "{:?}",
            FileUpdate {
                path: "secret".into(),
                diff: "secret".into(),
                kind: PatchKind::Update {
                    move_path: Some("secret".into()),
                },
            }
        ),
        format!(
            "{:?}",
            TurnFailure {
                message: "secret".into(),
                additional_details: Some("secret".into()),
            }
        ),
    ] {
        assert!(formatted.ends_with("([redacted])"));
        assert!(!formatted.contains("secret"));
    }
}

#[test]
fn notification_selection_handles_many_fragments_and_obsolete_malformed_messages() {
    let mut payload = message(/*tag*/ 32, b"\xff");
    payload.extend(message(/*tag*/ 85, b""));
    for _ in 0..100_000 {
        payload.extend(message(/*tag*/ 32, b""));
    }
    payload.extend(notification(/*tag*/ 32, &DELTA[..6]));
    payload.extend(message(/*tag*/ 100, b"\xff"));
    payload.extend(notification(/*tag*/ 32, &DELTA[6..]));
    let event = TranscriptEvent::decode("t", &payload).unwrap().unwrap();
    assert_eq!(
        event.update,
        TranscriptUpdate::AgentMessageDelta(ItemDelta {
            item_id: "i".into(),
            turn_id: "u".into(),
            delta: "d".into(),
        })
    );

    payload.extend(message(/*tag*/ 89, b"\xff"));
    assert!(TranscriptEvent::decode("t", &payload).unwrap().is_none());
}

#[path = "item_tests.rs"]
mod items;

#[path = "history_tests.rs"]
mod history;

#[path = "turns_tests.rs"]
mod turns_tests;
