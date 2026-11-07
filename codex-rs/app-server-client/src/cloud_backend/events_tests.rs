#![cfg(test)]
//! Covers the user receipt and final assistant message presented by Cloud.
use super::*;
use pretty_assertions::assert_eq;

#[test]
fn image_receipt_preserves_submission_identity_and_attachment() {
    let label = "[Image #1]";
    let actual = item_summary(cloud::ItemSummary::UserMessage {
        id: "item-1".into(),
        client_id: Some("submission-1".into()),
        content: vec![
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
        ],
    });
    assert_eq!(
        actual,
        Some(protocol::ThreadItem::untimed(
            protocol::ThreadItemPayload::UserMessage {
                id: "item-1".into(),
                client_id: Some("submission-1".into()),
                content: vec![
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
            },
        ))
    );
}

#[test]
fn completed_reply_preserves_the_full_text_questions_and_timestamp() -> Result<()> {
    let item = cloud::ItemSummary::AgentMessage {
        id: "reply".into(),
        text: "Choose one".into(),
        questions: Some(vec![cloud::AgentQuestion {
            title: "Which?".into(),
            options: Some(vec!["One".into(), "Two".into()]),
        }]),
    };
    let update = cloud::TranscriptUpdate::ItemCompleted(cloud::ItemLifecycle {
        turn_id: "turn".into(),
        at_ms: 45,
        item,
    });
    let Some(protocol::ServerNotification::ItemCompleted(actual)) =
        transcript("thread".into(), update)?
    else {
        panic!("expected a completed message");
    };
    assert_eq!(
        actual,
        protocol::ItemCompletedNotification::new(
            "thread".into(),
            "turn".into(),
            protocol::ThreadItemPayload::AgentMessage {
                moderation: None,
                id: "reply".into(),
                text: "Choose one".into(),
                phase: None,
                memory_citation: None,
                delivery: None,
                questions: Some(vec![protocol::AsyncUserInputQuestion {
                    title: "Which?".into(),
                    options: Some(vec!["One".into(), "Two".into()]),
                }]),
            },
            /*started_at_ms*/ None,
            /*completed_at_ms*/ 45
        )
    );
    Ok(())
}
