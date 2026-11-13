#![cfg(test)]

use super::*;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::InternalChatMessageMetadataPassthrough;
use pretty_assertions::assert_eq;

fn file_audio(file_id: &str) -> ContentItem {
    ContentItem::InputAudio {
        audio: AudioReference::File {
            file_id: file_id.to_string(),
        },
    }
}

fn inline_audio() -> ContentItem {
    ContentItem::InputAudio {
        audio: AudioReference::Inline {
            audio_url: "data:audio/wav;base64,AA==".to_string(),
        },
    }
}

fn message(content: Vec<ContentItem>) -> ResponseItem {
    ResponseItem::Message {
        encrypted_content: None,
        status: None,
        id: None,
        role: "user".to_string(),
        content,
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

fn audio_error() -> CodexErrorDetails {
    CodexErrorDetails::InvalidAudioInput {
        per_recording: true,
        message: "invalid recording".to_string(),
    }
}

fn rejected_file_audio_id(error: &CodexErrorDetails, items: &[ResponseItem]) -> Option<String> {
    SubmittedFileAudio::from_request(items, /*accepted*/ None, std::iter::empty())?
        .rejected_file_id(error)
}

#[test]
fn previously_accepted_file_only_disambiguates_clip_specific_errors() {
    let mut context = codex_history::RetainedContext::default();
    let good = message(vec![file_audio("file_a")]);
    let evidence = pending_audio_confirmation(
        /*previous*/ None,
        "turn",
        "model",
        "provider",
        std::slice::from_ref(&good),
    )
    .expect("new audio");
    context.record(
        &codex_history::RetainedContextEvent::AcceptedFileAudio {
            evidence: evidence.into(),
        },
        &codex_protocol_test_support::uuid_v7_item_id_generator_for_test(),
    );
    assert!(
        context
            .accepted_file_audio("another model", "provider")
            .is_none()
    );
    assert!(
        context
            .accepted_file_audio("model", "another provider")
            .is_none()
    );
    let accepted = context.accepted_file_audio("model", "provider");
    let items = [good, message(vec![file_audio("file_b")])];
    let candidate = SubmittedFileAudio::from_request(&items, accepted, std::iter::empty())
        .expect("one new recording");
    assert!(candidate.matches_rejection(&audio_error()));
    assert!(
        !candidate.matches_rejection(&CodexErrorDetails::InvalidAudioInput {
            message: "The combined audio input is too large.".into(),
            per_recording: false,
        })
    );
    assert_eq!(
        candidate.rejected_file_id(&audio_error()).as_deref(),
        Some("file_b")
    );
}

#[test]
fn tool_output_audio_makes_an_invalid_audio_error_ambiguous() {
    for kind in ["function_call_output", "custom_tool_call_output"] {
        let output = serde_json::from_value(serde_json::json!({
            "type": kind,
            "call_id": "call",
            "output": [{"type": "input_audio", "audio_url": "data:audio/wav;base64,AA=="}],
        }))
        .expect("valid tool output");
        let items = [message(vec![file_audio("valid_upload")]), output];
        assert_eq!(
            rejected_file_audio_id(&audio_error(), &items),
            None,
            "{kind}"
        );
    }
}

#[test]
fn only_file_backed_tool_audio_competes_with_a_user_upload_for_an_unknown_file_error() {
    use serde_json::json;
    for kind in ["function_call_output", "custom_tool_call_output"] {
        for (part, expected) in [
            (
                json!({"type":"input_audio","file_id":"expired_tool_clip"}),
                None,
            ),
            (
                json!({"type":"input_audio","audio_url":"data:audio/wav;base64,AA=="}),
                Some("valid_upload"),
            ),
        ] {
            let output =
                serde_json::from_value(json!({"type":kind, "call_id":"call", "output":[part]}))
                    .unwrap();
            let items = [message(vec![file_audio("valid_upload")]), output];
            assert_eq!(
                rejected_file_audio_id(&unknown_file_error(), &items).as_deref(),
                expected,
                "{kind}"
            );
            assert_eq!(
                rejected_file_audio_id(&audio_error(), &items),
                None,
                "{kind}"
            );
        }
    }
}

#[test]
fn attributes_audio_error_only_to_one_visible_file_audio() {
    let error = audio_error();
    assert_eq!(
        rejected_file_audio_id(&error, &[message(vec![file_audio("file_a")])]),
        Some("file_a".to_string())
    );
    assert_eq!(
        rejected_file_audio_id(
            &error,
            &[message(vec![file_audio("file_a"), file_audio("file_b")])]
        ),
        None
    );
    assert_eq!(
        rejected_file_audio_id(
            &error,
            &[message(vec![file_audio("file_a"), inline_audio()])]
        ),
        None
    );
    assert_eq!(
        rejected_file_audio_id(
            &error,
            &[
                message(vec![file_audio("file_a")]),
                ResponseItem::ContextCompaction {
                    id: None,
                    encrypted_content: None,
                    internal_chat_message_metadata_passthrough: None,
                },
            ]
        ),
        None
    );
}

fn unknown_file_error() -> CodexErrorDetails {
    CodexErrorDetails::UnexpectedStatus(codex_protocol::error::UnexpectedResponseError {
        status: http::StatusCode::NOT_FOUND,
        body: serde_json::json!({"error": {"message": "Invalid input: Unknown `file_id` in input content."}}).to_string(),
        url: None,
        user_message: None,
        cf_ray: None,
        request_id: None,
        identity_authorization_error: None,
        identity_error_code: None,
    })
}

#[test]
fn unknown_file_error_counts_the_submitted_projection_not_raw_history() {
    let error = unknown_file_error();
    let unavailable_image_projection = message(vec![
        ContentItem::InputText {
            text: "[Image unavailable]".to_string(),
        },
        file_audio("file_audio"),
    ]);
    assert_eq!(
        rejected_file_audio_id(&error, &[unavailable_image_projection]),
        Some("file_audio".to_string())
    );
    let unresolved_image = message(vec![
        ContentItem::InputImage {
            image: ImageReference::File {
                file_id: "file_image".to_string(),
            },
            detail: None,
        },
        file_audio("file_audio"),
    ]);
    assert_eq!(rejected_file_audio_id(&error, &[unresolved_image]), None);
}

#[test]
fn projection_removes_only_rejected_audio_and_keeps_metadata_aligned() {
    let mut first = message(vec![
        ContentItem::InputText {
            text: "first".to_string(),
        },
        file_audio("file_a"),
    ]);
    let ResponseItem::Message {
        internal_chat_message_metadata_passthrough,
        ..
    } = &mut first
    else {
        unreachable!()
    };
    *internal_chat_message_metadata_passthrough = Some(InternalChatMessageMetadataPassthrough {
        content_item_kinds: Some(vec![
            ContentItemKind("user.text".to_string()),
            ContentItemKind("user.audio".to_string()),
        ]),
        ..Default::default()
    });
    let second = message(vec![file_audio("file_b")]);
    let rejected_only = message(vec![file_audio("file_a")]);
    let raw = vec![first, second.clone(), rejected_only];
    let mut projected = raw
        .clone()
        .into_iter()
        .map(ResponseItemEnvelope::new)
        .collect();
    project_file_audio(&mut projected, &BTreeSet::from(["file_a".to_string()]));
    let projected = projected
        .into_iter()
        .map(ResponseItemEnvelope::into_item)
        .collect::<Vec<_>>();

    let ResponseItem::Message {
        content,
        internal_chat_message_metadata_passthrough: Some(metadata),
        ..
    } = &projected[0]
    else {
        panic!("expected retained first message")
    };
    assert_eq!(
        content,
        &[ContentItem::InputText {
            text: "first".to_string()
        }]
    );
    assert_eq!(
        metadata.content_item_kinds,
        Some(vec![ContentItemKind("user.text".to_string())])
    );
    assert_eq!(projected[1], second);
    assert_eq!(projected.len(), 2);
    assert_eq!(raw.len(), 3, "the retained history was not rewritten");
}
