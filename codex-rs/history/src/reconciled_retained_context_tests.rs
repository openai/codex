use super::*;
use crate::RetainedContextEvent;
use crate::RetainedInputSource;
use crate::VerifiedAnswer;
use crate::VerifiedQuestionAnswer;
use pretty_assertions::assert_eq;

fn instruction(text: &str) -> RetainedUserMessage {
    RetainedUserMessage {
        origin: crate::UserInputOrigin::User,
        turn_id: "turn-1".to_owned(),
        message_id: None,
        text: text.to_owned(),
        complete: false,
    }
}

#[test]
fn recovery_preserves_source_identity_acceptance_order_and_checkpoint_gaps() {
    let initial = instruction("Inspect the deployment.");
    let retained_excerpt = RetainedUserMessage {
        message_id: Some("retained-message".to_owned()),
        ..instruction("")
    };
    let original = RetainedUserMessage {
        text: "Deploy the reviewed change.".to_owned(),
        ..retained_excerpt.clone()
    };
    let mut retained = RetainedContext::default();
    retained.record_user_message(initial.clone(), RetainedInputSource::Local(Some(0)));
    retained.record_user_message(retained_excerpt, RetainedInputSource::Local(Some(1)));
    retained.record(&RetainedContextEvent::VerifiedAnswer {
        answer: VerifiedAnswer {
            turn_id: "answer-turn".to_owned(),
            call_id: "publish-question".to_owned(),
            questions: vec![VerifiedQuestionAnswer {
                question: "Publish?".to_owned(),
                answer: "Never publicly.".to_owned(),
            }],
        },
        acceptance_order: Some(3),
    });
    retained.mark_user_messages_incomplete();
    let checkpoint = retained.clone();

    let reconciled = ReconciledRetainedContext::new(
        Some(&retained),
        [
            // Existing identities match before requiring an order, including an omitted excerpt.
            (None, initial.clone()),
            (None, original.clone()),
            (Some(4), instruction("Do not deploy after all.")),
            // This steer arrived before the checkpoint-only answer but was recorded later.
            (Some(2), instruction("Make the deployment public.")),
            // A second identical message is a distinct source once the retained one matched.
            (Some(5), initial.clone()),
        ],
    );

    assert_eq!(
        (
            reconciled
                .ordered_entries()
                .map(|(order, entry)| match entry {
                    RetainedContextEntry::UserMessage(message)
                    | RetainedContextEntry::AssistantMessage(message) =>
                        (order, message.text.as_str()),
                    RetainedContextEntry::VerifiedAnswer(answer) => {
                        (order, answer.questions[0].answer.as_str())
                    }
                })
                .collect::<Vec<_>>(),
            reconciled.missing_user_messages,
        ),
        (
            vec![
                (RetainedContextOrder::Local(0), "Inspect the deployment."),
                (RetainedContextOrder::Local(1), ""),
                (
                    RetainedContextOrder::Local(2),
                    "Make the deployment public."
                ),
                (RetainedContextOrder::Local(3), "Never publicly."),
                (RetainedContextOrder::Local(4), "Do not deploy after all."),
                (RetainedContextOrder::Local(5), "Inspect the deployment."),
            ],
            true,
        ),
    );
    let legacy_instruction = instruction("Only publish to staging.");
    assert_eq!(
        reconciled
            .unmatched_user_messages(
                [
                    initial,
                    original,
                    instruction("Do not deploy after all."),
                    legacy_instruction.clone(),
                ]
                .into_iter(),
            )
            .collect::<Vec<_>>(),
        vec![legacy_instruction],
    );
    assert_eq!(retained, checkpoint);
}

#[test]
fn recovery_marks_missing_and_conflicting_orders_incomplete() {
    let mut retained = RetainedContext::default();
    retained.record_user_message(
        instruction("Inspect the deployment."),
        RetainedInputSource::Local(Some(1)),
    );
    assert!(!retained.has_missing_user_messages());
    let checkpoint = retained.clone();

    for invalid_order in [None, Some(1), Some(2)] {
        let reconciled = ReconciledRetainedContext::new(
            Some(&retained),
            [
                (Some(2), instruction("Do not deploy.")),
                (invalid_order, instruction("Invalid-order instruction.")),
            ],
        );
        assert_eq!(
            (
                reconciled
                    .ordered_entries()
                    .map(|(order, entry)| match entry {
                        RetainedContextEntry::UserMessage(message)
                        | RetainedContextEntry::AssistantMessage(message) =>
                            (order, message.text.as_str()),
                        RetainedContextEntry::VerifiedAnswer(_) => panic!("unexpected answer"),
                    })
                    .collect::<Vec<_>>(),
                reconciled.missing_user_messages,
            ),
            (
                vec![
                    (RetainedContextOrder::Local(1), "Inspect the deployment."),
                    (RetainedContextOrder::Local(2), "Do not deploy.")
                ],
                true,
            ),
            "invalid order: {invalid_order:?}",
        );
    }
    assert_eq!(retained, checkpoint);
}

#[test]
fn heartbeat_versions_survive_retention_restore_and_reconciliation() {
    let messages = (0..35).map(|index| {
        let (instructions, origin) = match index {
            1 => ("Create a worktree.", crate::UserInputOrigin::User),
            31 => ("Stop monitoring.", crate::UserInputOrigin::Heartbeat),
            // Identical text from a real user remains a new instruction.
            33 => ("Monitor only.", crate::UserInputOrigin::User),
            _ => ("Monitor only.", crate::UserInputOrigin::Heartbeat),
        };
        RetainedUserMessage {
            turn_id: format!("turn-{index}"),
            message_id: Some(format!("message-{index}")),
            text: format!("<heartbeat>\n  <automation_id>monitor</automation_id>\n  <current_time_iso>2026-09-23T00:{index:02}:00Z</current_time_iso>\n  <instructions>\n{instructions}\n  </instructions>\n</heartbeat>\n"),
            complete: true,
            origin,
        }
    }).collect::<Vec<_>>();
    let mut retained = RetainedContext::default();
    for (index, message) in messages.iter().enumerate() {
        if index == 30 {
            let checkpoint =
                serde_json::from_str(&serde_json::to_string(&retained).unwrap()).unwrap();
            retained = RetainedContext::default();
            retained.restore(Some(&checkpoint), &[]);
        }
        retained.record_user_message(
            message.clone(),
            RetainedInputSource::Local(Some(index as u64)),
        );
    }
    let expected = [0, 1, 31, 32, 33]
        .map(|index| {
            (
                RetainedContextOrder::Local(index as u64),
                messages[index].clone(),
            )
        })
        .to_vec();
    let user_message = |(order, entry): (_, RetainedContextEntry<'_>)| {
        let RetainedContextEntry::UserMessage(message) = entry else {
            panic!("unexpected answer")
        };
        (order, message.clone())
    };
    // Check storage before reconciliation can recover or coalesce anything.
    assert_eq!(
        retained
            .ordered_entries()
            .map(user_message)
            .collect::<Vec<_>>(),
        expected
    );
    assert!(retained.user_messages_complete());
    for missing in [false, true] {
        if missing {
            retained.mark_user_messages_incomplete();
        }
        let checkpoint = retained.clone();
        let reconciled = ReconciledRetainedContext::new(
            Some(&retained),
            // Reverse delivery order to exercise sorting across recovered and retained versions.
            messages.iter().enumerate().rev().map(|(index, message)| {
                (
                    Some(index as u64),
                    RetainedUserMessage {
                        complete: false,
                        ..message.clone()
                    },
                )
            }),
        );
        assert_eq!(
            (
                reconciled
                    .ordered_entries()
                    .map(user_message)
                    .collect::<Vec<_>>(),
                reconciled.latest_user_turn_id.as_deref(),
                reconciled.missing_user_messages,
            ),
            (expected.clone(), Some("turn-34"), missing),
        );
        assert_eq!(retained, checkpoint);
    }
}
