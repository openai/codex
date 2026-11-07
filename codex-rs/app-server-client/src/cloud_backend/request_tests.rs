#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn ordinary_multi_backend_resume_queries_cloud_interactive_threads_on_every_page() {
    // The mixed picker has no shared cwd/provider filter. It sends conventional
    // interactive source names, even when the connection belongs to Cloud.
    for (cursor, state_db_only) in [(None, true), (Some("page-2"), false)] {
        let params = serde_json::from_value(json!({
            "currentAccountOnly": true,
            "cursor": cursor,
            "limit": 25,
            "sortKey": "updated_at",
            "modelProviders": [],
            "sourceKinds": ["cli", "vscode"],
            "archived": false,
            "useStateDbOnly": state_db_only,
        }))
        .unwrap();
        let query = thread_list_query(params).unwrap();
        assert_eq!(query.cursor.as_deref(), cursor);
        assert_eq!(query.limit, Some(25));
        assert!(matches!(query.sort_key, cloud::ThreadSortKey::UpdatedAt));
        assert!(matches!(
            query.sort_direction,
            cloud::ThreadSortDirection::Desc
        ));
        assert!(query.originators.is_empty());
    }
}

#[test]
fn unsupported_filters_do_not_return_unrelated_cloud_rows() {
    for filter in [
        json!({"archived": true}),
        json!({"sourceKinds": ["exec"]}),
        json!({"attachedToThreadId": "other"}),
        json!({"modelProviders": ["other"]}),
    ] {
        let params = serde_json::from_value(filter).unwrap();
        assert!(thread_list_query(params).is_err());
    }
}

fn message() -> protocol::TurnStartParams {
    let text = |text: &str| protocol::UserInput::Text {
        text: text.to_owned(),
        text_elements: Vec::new(),
    };
    protocol::TurnStartParams {
        thread_id: "thread-a".into(),
        client_user_message_id: Some("submission-a".into()),
        turn_trigger: Some("safety_recourse_follow_up".into()),
        input: vec![text("one"), text("two")],
        ..Default::default()
    }
}

#[test]
fn user_input_uses_stable_id_and_the_existing_cloud_configuration() {
    let request = cloud_message(message()).unwrap();
    assert_eq!(request.thread_id, "thread-a");
    assert_eq!(request.client_user_message_id, "submission-a");
    assert_eq!(
        request.input,
        vec![cloud::UserMessageInput::Text {
            text: "one\ntwo".into(),
            text_elements: Vec::new(),
        }]
    );
    assert_eq!(
        request.turn_trigger.as_deref(),
        Some("safety_recourse_follow_up")
    );
    assert!(matches!(request.mode, cloud::SubmissionMode::StartIfIdle));
    assert!(!request.shared_access);

    for tier in ["default", "priority"] {
        assert!(
            cloud_message(protocol::TurnStartParams {
                service_tier_for_turn: Some(tier.into()),
                ..message()
            })
            .is_err()
        );
    }
    assert!(
        cloud_message(protocol::TurnStartParams {
            client_user_message_id: None,
            ..message()
        })
        .is_err()
    );
}

#[test]
fn image_submission_keeps_attachment_markers_and_input_order() {
    let label = "[Image #1]";
    let request = cloud_message(protocol::TurnStartParams {
        input: vec![
            protocol::UserInput::Image {
                image: protocol::ImageReference::File {
                    file_id: "file-synthetic".into(),
                },
                detail: Some(ImageDetail::High),
            },
            protocol::UserInput::Text {
                text: format!("{label} describe"),
                text_elements: vec![protocol::TextElement::new(
                    protocol::ByteRange {
                        start: 0,
                        end: label.len(),
                    },
                    Some(label.into()),
                )],
            },
        ],
        ..message()
    })
    .unwrap();

    assert_eq!(
        request.input,
        vec![
            cloud::UserMessageInput::Image {
                image: cloud::ImageReference::FileId("file-synthetic".into()),
                detail: Some(cloud::ImageDetail::High),
            },
            cloud::UserMessageInput::Text {
                text: format!("{label} describe"),
                text_elements: vec![cloud::UserMessageTextElement {
                    start: 0,
                    end: label.len(),
                    placeholder: Some(label.into()),
                }],
            },
        ]
    );
}

#[test]
fn duplicate_and_queued_admissions_never_invent_a_running_turn() {
    for outcome in [
        cloud::AdmissionOutcome::Duplicate { turn_id: None },
        cloud::AdmissionOutcome::Duplicate {
            turn_id: Some("completed-or-running".into()),
        },
        cloud::AdmissionOutcome::Queued {
            submission_id: "queued".into(),
        },
    ] {
        assert!(
            admitted_turn(outcome)
                .unwrap_err()
                .to_string()
                .contains("already accepted")
        );
    }
}

#[test]
fn admission_receipts_preserve_server_turns_and_duplicate_identity() -> Result<()> {
    let turn = |id: &str, status| cloud::TurnSummary {
        id: id.into(),
        status,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    };
    for (outcome, expected) in [
        (
            cloud::AdmissionOutcome::Started(turn("u", cloud::TurnStatus::InProgress)),
            ("started", "u", protocol::TurnStatus::InProgress),
        ),
        (
            cloud::AdmissionOutcome::Steered(turn("v", cloud::TurnStatus::Completed)),
            ("steered", "v", protocol::TurnStatus::Completed),
        ),
    ] {
        let (kind, turn) = match admitted_message(outcome)? {
            CloudUserMessageReceipt::Started(turn) => ("started", turn),
            CloudUserMessageReceipt::Steered(turn) => ("steered", turn),
            CloudUserMessageReceipt::Duplicate { .. } => panic!("expected a turn"),
        };
        assert_eq!((kind, turn.id.as_str(), turn.status), expected);
    }
    for turn_id in [None, Some("original".into())] {
        let CloudUserMessageReceipt::Duplicate { turn_id: actual } =
            admitted_message(cloud::AdmissionOutcome::Duplicate {
                turn_id: turn_id.clone(),
            })?
        else {
            panic!("a duplicate must not become a running turn");
        };
        assert_eq!(actual, turn_id);
    }
    Ok(())
}
