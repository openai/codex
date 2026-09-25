use super::*;
use crate::CodexHarnessMetadata;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;

fn publish_answer() -> RetainedContextEvent {
    RetainedContextEvent::VerifiedAnswer {
        answer: VerifiedAnswer {
            turn_id: "turn-1".to_owned(),
            call_id: "ask-1".to_owned(),
            questions: vec![VerifiedQuestionAnswer {
                question: "Publish?".to_owned(),
                answer: "Yes, but never publicly.".to_owned(),
            }],
        },
        acceptance_order: None,
    }
}

#[test]
fn retained_evidence_preserves_order_through_recovery_checkpoint_and_rollback() {
    let mut context = RetainedContext::default();
    let first = publish_answer();
    assert!(context.record(&first));
    let before_restriction = context.clone();
    context.record_user_message(
        RetainedUserMessage {
            origin: crate::UserInputOrigin::User,
            turn_id: "revocation-turn".to_owned(),
            message_id: Some("revocation".to_owned()),
            text: String::new(),
            complete: false,
        },
        RetainedInputSource::Local(None),
    );
    let mut expected = context.clone();
    expected.user_messages[0].value.text = "Do not publish after all.".to_owned();
    context.recover_user_message_excerpts(|id| {
        assert_eq!(id, "revocation");
        Some("Do not publish after all.".to_owned())
    });
    assert_eq!(context, expected);
    context.recover_user_message_excerpts(|_| panic!("existing text must not be replaced"));
    let snapshot = context.clone();
    assert!(!context.record(&first));
    assert_eq!(context, snapshot);
    let checkpoint =
        serde_json::from_str(&serde_json::to_string(&context).expect("retained answer fixture"))
            .expect("retained answer fixture");
    let mut restored = RetainedContext::default();
    restored.restore(Some(&checkpoint), &[]);
    assert_eq!(restored, snapshot);
    assert_eq!(
        restored
            .ordered_entries()
            .map(|(_, entry)| match entry {
                RetainedContextEntry::VerifiedAnswer(answer) => answer.questions[0].answer.as_str(),
                RetainedContextEntry::UserMessage(message)
                | RetainedContextEntry::AssistantMessage(message) => message.text.as_str(),
            })
            .collect::<Vec<_>>(),
        vec!["Yes, but never publicly.", "Do not publish after all."]
    );
    restored.rollback(
        &["revocation-turn"],
        Some("revocation"),
        RetainedInputSource::Local(None),
    );
    assert_eq!(
        restored,
        RetainedContext {
            next_order: snapshot.next_order,
            ..before_restriction
        }
    );
}

#[test]
fn retained_families_enforce_storage_limits_without_changing_snapshots() {
    let mut context = RetainedContext::default();
    let first = publish_answer();
    context.record(&first);
    let snapshot = context.clone();

    for index in 2..=10 {
        context.record(&RetainedContextEvent::VerifiedAnswer {
            answer: VerifiedAnswer {
                turn_id: "turn-2".to_owned(),
                call_id: format!("ask-{index}"),
                questions: vec![VerifiedQuestionAnswer {
                    question: "Continue?".to_owned(),
                    answer: "Yes".to_owned(),
                }],
            },
            acceptance_order: None,
        });
    }
    assert!(!context.verified_answers_complete());
    assert_eq!(context.verified_answers().count(), MAX_FAMILY_RECORDS);
    context.rollback(
        &["turn-2"],
        /*first_removed_message_id*/ None,
        RetainedInputSource::Local(None),
    );
    assert_eq!(context.verified_answers().count(), 0);
    assert!(!context.verified_answers_complete());

    let mut restored = snapshot.clone();
    let mut oversized = first.clone();
    let RetainedContextEvent::VerifiedAnswer { answer, .. } = &mut oversized;
    answer.questions[0].answer = "a".repeat(MAX_RECORD_BYTES);
    restored.record(&oversized);
    assert!(!restored.verified_answers_complete());
    assert!(
        restored
            .verified_answers()
            .next()
            .expect("retained answer fixture")
            .questions
            .is_empty()
    );
    assert_eq!(
        snapshot
            .verified_answers()
            .next()
            .expect("retained answer fixture")
            .questions[0]
            .answer,
        "Yes, but never publicly."
    );
    for index in 0..=MAX_FAMILY_RECORDS {
        restored.record_user_message(
            RetainedUserMessage {
                origin: crate::UserInputOrigin::User,
                turn_id: "later-turn".to_owned(),
                message_id: Some(format!("message-{index}")),
                text: "Keep the repository private.".to_owned(),
                complete: true,
            },
            RetainedInputSource::Local(None),
        );
    }
    assert!(!restored.user_messages_complete());
    assert_eq!(
        restored
            .ordered_entries()
            .filter(|(_, entry)| matches!(entry, RetainedContextEntry::UserMessage(_)))
            .count(),
        MAX_FAMILY_RECORDS
    );
    let recent = restored.clone();
    restored.record_user_message(
        RetainedUserMessage {
            origin: crate::UserInputOrigin::User,
            turn_id: "earlier-turn".to_owned(),
            message_id: Some("delayed-message".to_owned()),
            text: "An older queued instruction.".to_owned(),
            complete: true,
        },
        RetainedInputSource::Local(Some(0)),
    );
    assert_eq!(
        restored, recent,
        "delayed old input must not evict newer evidence"
    );
    restored.record_user_message(
        RetainedUserMessage {
            origin: crate::UserInputOrigin::User,
            turn_id: "oversized-turn".to_owned(),
            message_id: Some("oversized-message".to_owned()),
            text: "restriction ".repeat(MAX_RECORD_BYTES),
            complete: true,
        },
        RetainedInputSource::Local(None),
    );
    let Some((_, RetainedContextEntry::UserMessage(message))) =
        restored.ordered_entries().next_back()
    else {
        panic!("latest user evidence");
    };
    assert_eq!((&message.text, message.complete), (&String::new(), false));
    let restrictions = restored.user_messages.clone();
    for index in 0..=MAX_FAMILY_RECORDS {
        let order = restored.reserve_order();
        restored.record_assistant_message(
            RetainedUserMessage {
                message_id: Some(format!("assistant-{index}")),
                text: "Run smoke tests?".to_owned(),
                complete: true,
                ..restrictions.back().unwrap().value.clone()
            },
            RetainedInputSource::Local(Some(order)),
        );
    }
    assert_eq!(restored.user_messages, restrictions);
    assert_eq!(restored.assistant_messages.len(), MAX_FAMILY_RECORDS);
    assert!(restored.has_omitted_assistant_messages());
}

#[test]
fn recovered_excerpts_obey_record_and_family_limits() {
    let mut context = RetainedContext::default();
    for index in 0..MAX_FAMILY_RECORDS {
        context.record_user_message(
            RetainedUserMessage {
                origin: crate::UserInputOrigin::User,
                turn_id: "turn-1".to_owned(),
                message_id: Some(format!("message-{index}")),
                text: String::new(),
                complete: false,
            },
            RetainedInputSource::Local(None),
        );
    }
    let unchanged = context.clone();
    context.recover_user_message_excerpts(|_| Some("x".repeat(MAX_RECORD_BYTES)));
    assert_eq!(context, unchanged);
    context.recover_user_message_excerpts(|_| Some("x".repeat(MAX_RECORD_BYTES - 1_024)));
    assert!(context.user_messages_incomplete);
    assert!(context.user_messages.len() < MAX_FAMILY_RECORDS);
    assert!(serde_json::to_vec(&context.user_messages).unwrap().len() <= MAX_FAMILY_BYTES);
}

#[test]
fn legacy_checkpoints_mark_user_messages_incomplete() {
    let RetainedContextEvent::VerifiedAnswer { answer, .. } = publish_answer();
    let mut wire = serde_json::json!({
        "verified_answers": [answer], "incomplete": false
    });
    let legacy: RetainedContext =
        serde_json::from_value(wire.clone()).expect("legacy retained-answer checkpoint");
    assert!(legacy.verified_answers_complete());
    assert!(!legacy.user_messages_complete());

    // Retain the original wire key even though the internal field names its family.
    wire["verified_answers"][0]["order"] = serde_json::json!(0);
    wire["user_messages"] = serde_json::json!([]);
    wire["user_messages_incomplete"] = serde_json::json!(true);
    wire["next_order"] = serde_json::json!(0);
    wire["assistant_messages"] = serde_json::json!([]);
    wire["assistant_messages_incomplete"] = serde_json::json!(false);
    assert_eq!(serde_json::to_value(&legacy).unwrap(), wire);

    let mut restored = RetainedContext::default();
    restored.restore(/*checkpoint*/ None, &[]);
    assert!(!restored.user_messages_complete());
}

#[test]
fn restored_input_order_accounts_for_surviving_local_sources() {
    let surviving_items = [(Some(7), false), (Some(100), true), (None, false)].map(
        |(user_input_order, inherited_user_message)| ResponseItemEnvelope {
            item: ResponseItem::Message {
                id: None,
                role: "user".to_owned(),
                content: vec![ContentItem::InputText {
                    text: "Inspect the deployment.".to_owned(),
                }],
                phase: None,
                internal_chat_message_metadata_passthrough: None,
            },
            metadata: Some(CodexHarnessMetadata {
                user_input_order,
                inherited_user_message,
                ..Default::default()
            }),
        },
    );
    let checkpoint = RetainedContext {
        next_order: 20,
        ..Default::default()
    };
    for (checkpoint, expected_order) in [(None, 8), (Some(&checkpoint), 20)] {
        let mut restored = RetainedContext::default();
        restored.restore(checkpoint, &surviving_items);
        assert_eq!(
            (
                restored.reserve_order(),
                restored.ordered_entries().count(),
                restored.user_messages_complete(),
            ),
            (expected_order, 0, checkpoint.is_some()),
        );
    }
}

#[test]
fn accepted_order_survives_delayed_recording_and_checkpoint_replay() {
    let mut context = RetainedContext::default();
    // Rejected/canceled input consumes a sequence but never becomes evidence.
    context.reserve_order();
    let steer_order = context.reserve_order();
    let answer_order = context.reserve_order();
    let RetainedContextEvent::VerifiedAnswer { answer, .. } = publish_answer();
    let event = RetainedContextEvent::VerifiedAnswer {
        answer,
        acceptance_order: Some(answer_order),
    };
    context.record(&event);
    let instruction = RetainedUserMessage {
        origin: crate::UserInputOrigin::User,
        turn_id: "turn-1".to_owned(),
        message_id: Some("steer".to_owned()),
        text: "Keep the repository private.".to_owned(),
        complete: true,
    };
    // Assistant delivery after accepted steering must not move the steering past it.
    let assistant_order = context.reserve_order();
    context.record_assistant_message(
        RetainedUserMessage {
            message_id: Some("assistant".to_owned()),
            text: "Publish publicly?".to_owned(),
            ..instruction.clone()
        },
        RetainedInputSource::Local(Some(assistant_order)),
    );
    let checkpoint = context.clone();
    context.record_user_message(
        instruction.clone(),
        RetainedInputSource::Local(Some(steer_order)),
    );
    let mut resumed = RetainedContext::default();
    resumed.restore(Some(&checkpoint), &[]);
    resumed.record_user_message(instruction, RetainedInputSource::Local(Some(steer_order)));
    // This suffix has no captured version metadata, so replay creates a fresh revision.
    // Its original contents and acceptance order still reconstruct identically.
    let mut expected = context.clone();
    assert_ne!(
        resumed.user_messages[0].revision,
        expected.user_messages[0].revision
    );
    expected.user_messages[0].revision = resumed.user_messages[0].revision.clone();
    assert_eq!(resumed, expected);
    let source = context
        .source(RetainedContextEntry::UserMessage(
            &context.user_messages[0].value,
        ))
        .unwrap();
    assert!(resumed.restore_source_revision(&source));
    assert_eq!(resumed, context);
    assert_eq!(
        resumed
            .ordered_entries()
            .map(|(order, entry)| match entry {
                RetainedContextEntry::UserMessage(message)
                | RetainedContextEntry::AssistantMessage(message) => (order, message.text.as_str()),
                RetainedContextEntry::VerifiedAnswer(answer) => {
                    (order, answer.questions[0].answer.as_str())
                }
            })
            .collect::<Vec<_>>(),
        vec![
            (
                RetainedContextOrder::Local(steer_order),
                "Keep the repository private."
            ),
            (
                RetainedContextOrder::Local(answer_order),
                "Yes, but never publicly."
            ),
            (
                RetainedContextOrder::Local(assistant_order),
                "Publish publicly?"
            ),
        ],
    );
    // This checkpoint predates model delivery of the queued instruction. Rollback
    // still removes its later-accepted answer using the persisted boundary order.
    let mut before_delivery = checkpoint;
    before_delivery.rollback(
        &["turn-1"],
        Some("steer"),
        RetainedInputSource::Local(Some(steer_order)),
    );
    resumed.rollback(
        &["turn-1"],
        Some("steer"),
        RetainedInputSource::Local(Some(steer_order)),
    );
    assert_eq!(before_delivery, resumed);
    assert_eq!(
        resumed,
        RetainedContext {
            next_order: 4,
            ..Default::default()
        }
    );
}

#[test]
fn adopted_instructions_preserve_local_order_and_rollback_scope() {
    let mut context = RetainedContext::default();
    context.record(&publish_answer());
    for index in 0..2 {
        let message = RetainedUserMessage {
            origin: crate::UserInputOrigin::User,
            turn_id: "parent-turn".to_owned(),
            message_id: Some(format!("parent-{index}")),
            text: format!("Parent instruction {index}"),
            complete: true,
        };
        context.record_user_message(message.clone(), RetainedInputSource::Inherited);
        context.record_user_message(message.clone(), RetainedInputSource::Inherited);
        context.record_assistant_message(message, RetainedInputSource::Inherited);
    }
    assert_eq!(context.reserve_order(), 1);
    assert!(!context.verified_answers_complete());
    assert_eq!(
        context
            .ordered_entries()
            .map(|(order, _)| order)
            .collect::<Vec<_>>(),
        vec![
            RetainedContextOrder::Inherited(0),
            RetainedContextOrder::Inherited(1),
            RetainedContextOrder::Inherited(2),
            RetainedContextOrder::Inherited(3),
            RetainedContextOrder::Local(0)
        ],
    );
    let checkpoint = serde_json::from_slice(&serde_json::to_vec(&context).unwrap()).unwrap();
    let mut restored = RetainedContext::default();
    restored.restore(Some(&checkpoint), &[]);
    assert_eq!(restored, context);
    // Older roots adopted complete instructions without recording the missing parent answers.
    let mut legacy_checkpoint = serde_json::to_value(&checkpoint).unwrap();
    legacy_checkpoint["incomplete"] = serde_json::json!(false);
    let legacy_checkpoint = serde_json::from_value(legacy_checkpoint).unwrap();
    restored.restore(Some(&legacy_checkpoint), &[]);
    assert_eq!(restored, context);
    restored.rollback(
        &["turn-1"],
        /*first_removed_message_id*/ None,
        RetainedInputSource::Local(Some(0)),
    );
    let mut expected = context;
    expected.verified_answers.clear();
    assert_eq!(restored, expected);
    restored.rollback(
        &["parent-turn"],
        Some("parent-1"),
        RetainedInputSource::Inherited,
    );
    expected.user_messages.pop_back();
    expected.assistant_messages.pop_back();
    assert_eq!(restored, expected);
}
