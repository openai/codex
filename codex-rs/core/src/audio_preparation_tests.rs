use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use pretty_assertions::assert_eq;

use super::*;

#[test]
fn preparation_canonicalizes_data_urls_and_rejects_remote_urls() {
    let mut items = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputAudio {
                audio_url: "data:audio/x-wav;base64,YXVkaW8=".to_string(),
            },
            ContentItem::InputAudio {
                audio_url: "data:audio/ogg;base64,YXVkaW8=".to_string(),
            },
            ContentItem::InputAudio {
                audio_url: "https://example.com/audio.mp3".to_string(),
            },
        ],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }];

    prepare_response_items(&mut items);

    assert_eq!(
        items,
        vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputAudio {
                    audio_url: "data:audio/wav;base64,YXVkaW8=".to_string(),
                },
                ContentItem::InputAudio {
                    audio_url: "data:audio/ogg;base64,YXVkaW8=".to_string(),
                },
                ContentItem::InputText {
                    text: "audio content omitted because it could not be processed".to_string(),
                },
            ],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }]
    );
}

#[test]
fn preparation_replaces_invalid_message_audio_with_placeholders() {
    let mut items = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputAudio {
                audio_url: "data:audio/wav;base64,%%%".to_string(),
            },
            ContentItem::InputAudio {
                audio_url: "data:audio/flac;base64,YXVkaW8=".to_string(),
            },
        ],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }];

    prepare_response_items(&mut items);

    assert_eq!(
        items,
        vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText {
                    text: "audio content omitted because it could not be processed".to_string(),
                },
                ContentItem::InputText {
                    text: "audio content omitted because its format is not supported; use wav, mp3, m4a, webm, or ogg".to_string(),
                },
            ],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }]
    );
}

#[test]
fn preparation_replaces_only_failed_tool_audio_and_preserves_metadata() {
    let mut items = vec![ResponseItem::FunctionCallOutput {
        id: None,
        call_id: "call-1".to_string(),
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::ContentItems(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "before".to_string(),
                },
                FunctionCallOutputContentItem::InputAudio {
                    audio_url: "data:audio/wav;base64,YXVkaW8=".to_string(),
                },
                FunctionCallOutputContentItem::InputAudio {
                    audio_url: "data:audio/wav,not-base64".to_string(),
                },
            ]),
            success: Some(true),
        },
        internal_chat_message_metadata_passthrough: None,
    }];

    prepare_response_items(&mut items);

    assert_eq!(
        items,
        vec![ResponseItem::FunctionCallOutput {
            id: None,
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::ContentItems(vec![
                    FunctionCallOutputContentItem::InputText {
                        text: "before".to_string(),
                    },
                    FunctionCallOutputContentItem::InputAudio {
                        audio_url: "data:audio/wav;base64,YXVkaW8=".to_string(),
                    },
                    FunctionCallOutputContentItem::InputText {
                        text: "audio content omitted because it could not be processed".to_string(),
                    },
                ]),
                success: Some(true),
            },
            internal_chat_message_metadata_passthrough: None,
        }]
    );
}

#[test]
fn preparation_errors_map_to_expected_placeholders() {
    let cases = [
        (
            AudioPreparationError::InvalidDataUrl {
                reason: "details remain in logs",
            },
            "audio content omitted because it could not be processed",
        ),
        (
            AudioPreparationError::UnsupportedFormat,
            "audio content omitted because its format is not supported; use wav, mp3, m4a, webm, or ogg",
        ),
        (
            AudioPreparationError::AudioTooLarge { size: usize::MAX },
            "audio content omitted because it exceeded the supported size limit; use a smaller audio file",
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(error.placeholder(), expected);
    }
}
