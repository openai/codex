//! Item fixtures cover lifecycle order, outcomes, presence, and opaque fallback.
use super::*;
use pretty_assertions::assert_eq;
use prost::Message;

const AGENT: &[u8] = b"\x1a\x06\x12\x01i\x32\x01d";

const COMMAND: &[u8] =
    b"\x0a\x00\x12\x01c\x1a\x07\x0a\x05\x22\x03\x0a\x01c\x22\x01w\x28\x00\x30\x00\x3a\x01i\x60\x03";

fn lifecycle(tag: u32, item: &[u8]) -> Vec<u8> {
    let (at_ms, emitted_at_ms) = if tag == 27 { (3, 7) } else { (5, 11) };
    let params = if tag == 27 {
        [
            message(/*tag*/ 1, item),
            vec![16, at_ms as u8],
            b"\x1a\x01t\x22\x01u".to_vec(),
        ]
        .concat()
    } else {
        [
            vec![8, at_ms as u8],
            message(/*tag*/ 2, item),
            b"\x1a\x01t\x22\x01u".to_vec(),
        ]
        .concat()
    };
    message(
        tag,
        &[vec![8, emitted_at_ms as u8], message(/*tag*/ 2, &params)].concat(),
    )
}

#[tokio::test]
async fn item_lifecycle_keeps_identity_and_timestamps() {
    let payloads = [
        lifecycle(/*tag*/ 27, AGENT),
        notification(/*tag*/ 32, DELTA),
        lifecycle(/*tag*/ 31, AGENT),
    ];
    let fixture = fixture(response(
        payloads
            .iter()
            .map(|payload| frame(&message(/*tag*/ 1, payload)))
            .collect(),
        tonic::Status::ok(""),
    ))
    .await;
    let mut stream = fixture.client.attach("t").await.unwrap();
    let item = |at_ms| ItemLifecycle {
        turn_id: "u".into(),
        at_ms,
        item: ItemSummary::AgentMessage {
            id: "i".into(),
            text: "d".into(),
            questions: None,
        },
    };
    let expected = [
        (Some(7), TranscriptUpdate::ItemStarted(item(3))),
        (
            None,
            TranscriptUpdate::AgentMessageDelta(ItemDelta {
                item_id: "i".into(),
                turn_id: "u".into(),
                delta: "d".into(),
            }),
        ),
        (Some(11), TranscriptUpdate::ItemCompleted(item(5))),
    ];
    for (emitted_at_ms, update) in expected {
        let Event::Transcript(event) = stream.next().await.unwrap().unwrap() else {
            panic!("expected transcript event")
        };
        assert_eq!(
            (&event.thread_id, event.emitted_at_ms, &event.update),
            (&"t".to_owned(), emitted_at_ms, &update)
        );
    }
    assert!(stream.next().await.is_none());
}

#[test]
fn user_message_preserves_client_id_and_text() {
    let user = b"\x0a\x23\x0a\x09client-id\x12\x0b\x0a\x09\x0a\x07\x0a\x05hello\x1a\x09user-item";
    assert_eq!(
        summary(user),
        Some(ItemSummary::UserMessage {
            id: "user-item".into(),
            client_id: Some("client-id".into()),
            text: vec!["hello".into()],
        })
    );
}

fn summary(bytes: &[u8]) -> Option<ItemSummary> {
    crate::items::summary(crate::item_wire::Item::decode(bytes).unwrap())
        .unwrap_or_else(|_| panic!("expected supported item fields"))
}

#[test]
fn command_and_patch_summaries_preserve_status_and_presence() {
    let command = CommandSummary {
        id: "i".into(),
        command: "c".into(),
        cwd: "w".into(),
        actions: vec![CommandAction::Unknown {
            command: "c".into(),
        }],
        source: CommandExecutionSource::Agent,
        aggregated_output: Some(String::new()),
        duration_ms: Some(0),
        exit_code: Some(0),
        status: ItemStatus::Failed,
    };
    assert_eq!(
        summary(&message(/*tag*/ 7, &[COMMAND, b"\x58\x02"].concat())),
        Some(ItemSummary::CommandExecution(CommandSummary {
            source: CommandExecutionSource::UserShell,
            ..command.clone()
        }))
    );
    let change = b"\x0a\x01d\x12\x02\x0a\x00\x1a\x01p";
    for (wire_status, status) in [
        (1, ItemStatus::InProgress),
        (2, ItemStatus::Completed),
        (3, ItemStatus::Failed),
        (4, ItemStatus::Declined),
        (99, ItemStatus::Unknown(99)),
    ] {
        assert_eq!(
            summary(&message(
                /*tag*/ 7,
                &[COMMAND, &[96, wire_status]].concat()
            )),
            Some(ItemSummary::CommandExecution(CommandSummary {
                status,
                ..command.clone()
            }))
        );
        let patch = [
            message(/*tag*/ 1, &message(/*tag*/ 1, change)),
            b"\x12\x01i".to_vec(),
            vec![24, wire_status],
        ]
        .concat();
        assert_eq!(
            summary(&message(/*tag*/ 8, &patch)),
            Some(ItemSummary::FileChange {
                id: "i".into(),
                changes: vec![FileUpdate {
                    path: "p".into(),
                    diff: "d".into(),
                    kind: PatchKind::Add
                }],
                status,
            })
        );
    }
    // Lifecycle completion must not rewrite a failed command's outcome.
    for tag in [27, 31] {
        let event = TranscriptEvent::decode("t", &lifecycle(tag, &message(/*tag*/ 7, COMMAND)))
            .unwrap()
            .unwrap();
        let (TranscriptUpdate::ItemStarted(item) | TranscriptUpdate::ItemCompleted(item)) =
            event.update
        else {
            panic!("expected item lifecycle");
        };
        assert_eq!(item.item, ItemSummary::CommandExecution(command.clone()));
    }
}

#[test]
fn reasoning_preserves_summary_and_content() {
    let reasoning = [
        message(/*tag*/ 1, &message(/*tag*/ 1, b"detail")),
        b"\x12\x01i".to_vec(),
        message(/*tag*/ 3, &message(/*tag*/ 1, b"summary")),
    ]
    .concat();
    assert_eq!(
        summary(&message(/*tag*/ 6, &reasoning)),
        Some(ItemSummary::Reasoning {
            id: "i".into(),
            summary: vec!["summary".into()],
            content: vec!["detail".into()],
        })
    );
}

#[test]
fn agent_messages_keep_questions_with_empty_text() {
    let choices = message(/*tag*/ 1, b"Continue");
    let questions = [
        message(/*tag*/ 1, &message(/*tag*/ 2, b"Freeform")),
        message(/*tag*/ 1, b"\x0a\x00\x12\x05Empty"),
        message(
            /*tag*/ 1,
            &[
                message(/*tag*/ 1, &choices),
                message(/*tag*/ 2, b"Proceed?"),
            ]
            .concat(),
        ),
    ]
    .concat();
    let expected = vec![
        AgentQuestion {
            title: "Freeform".into(),
            options: None,
        },
        AgentQuestion {
            title: "Empty".into(),
            options: Some(vec![]),
        },
        AgentQuestion {
            title: "Proceed?".into(),
            options: Some(vec!["Continue".into()]),
        },
    ];
    for (questions, expected) in [(questions.as_slice(), expected), (&[], vec![])] {
        let item = message(
            /*tag*/ 3,
            &[b"\x12\x01i\x32\x00".to_vec(), message(/*tag*/ 5, questions)].concat(),
        );
        assert_eq!(
            summary(&item),
            Some(ItemSummary::AgentMessage {
                id: "i".into(),
                text: String::new(),
                questions: Some(expected),
            })
        );
    }
}

#[test]
fn unsupported_and_incomplete_items_stay_opaque() {
    let absent_time = [message(/*tag*/ 1, AGENT), b"\x1a\x01t\x22\x01u".to_vec()].concat();
    // A later audio variant replaces text in the same native UserInput oneof.
    let text_then_audio = b"\x0a\x03\x0a\x01x\x22\x03\x0a\x01u";
    let user_content = message(/*tag*/ 2, &message(/*tag*/ 1, text_then_audio));
    let opaque = [
        message(
            /*tag*/ 1,
            &[user_content, message(/*tag*/ 3, b"i")].concat(),
        ),
        message(/*tag*/ 7, &[COMMAND, b"\x58\x63"].concat()), // Future command source (99).
        message(/*tag*/ 9, b""),
        message(/*tag*/ 20, b""),
        [AGENT.to_vec(), message(/*tag*/ 19, b"")].concat(),
        message(/*tag*/ 3, b"\x12\x01i"),
        message(/*tag*/ 7, b"\x12\x01c\x22\x01w\x3a\x01i"),
        message(/*tag*/ 8, b"\x12\x01i\x18\x01"),
    ];
    for item in opaque {
        assert!(
            TranscriptEvent::decode("t", &lifecycle(/*tag*/ 27, &item))
                .unwrap()
                .is_none()
        );
    }
    assert!(
        TranscriptEvent::decode("t", &notification(/*tag*/ 27, &absent_time))
            .unwrap()
            .is_none()
    );
}

#[test]
fn item_decoder_rejects_malformed_items_and_foreign_owners() {
    assert!(TranscriptEvent::decode("t", &lifecycle(/*tag*/ 27, &[0xff])).is_err());
    assert!(TranscriptEvent::decode("other", &lifecycle(/*tag*/ 31, AGENT)).is_err());
}

#[test]
fn item_summaries_redact_sensitive_fields() {
    let item = ItemSummary::AgentMessage {
        questions: None,
        id: "secret".into(),
        text: "secret".into(),
    };
    let text = format!("{item:?}");
    assert_eq!(text, "ItemSummary([redacted])");
    assert!(!text.contains("secret"));
    let action = CommandAction::Unknown {
        command: "secret".into(),
    };
    assert_eq!(format!("{action:?}"), "CommandAction([redacted])");
}
