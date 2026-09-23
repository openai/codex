use super::*;
use crate::RetainedContextEvent;
use crate::RetainedInputSource;
use crate::VerifiedAnswer;
use crate::VerifiedQuestionAnswer;
use pretty_assertions::assert_eq;

fn instruction(text: &str) -> RetainedUserMessage {
    RetainedUserMessage {
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
            (None, original),
            (Some(4), instruction("Do not deploy after all.")),
            // This steer arrived before the checkpoint-only answer but was recorded later.
            (Some(2), instruction("Make the deployment public.")),
            // A second identical message is a distinct source once the retained one matched.
            (Some(5), initial),
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
