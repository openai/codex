use codex_protocol::models::AudioReference;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseInputItem;
use pretty_assertions::assert_eq;

use super::response_input_to_code_mode_result;

#[test]
fn code_mode_text_preserves_message_content() {
    let response = ResponseInputItem::Message {
        role: "user".to_string(),
        content: vec![
            ContentItem::InputText {
                text: "Input text".to_string(),
            },
            ContentItem::OutputText {
                text: "Output text".to_string(),
                annotations: Some(vec![serde_json::json!({"type": "citation"})]),
                logprobs: Some(vec![serde_json::json!({"token": "Output"})]),
            },
            ContentItem::InputImage {
                image: ImageReference::File {
                    file_id: "file_123".to_string(),
                },
                detail: None,
            },
            ContentItem::InputImage {
                image: ImageReference::Inline {
                    image_url: "data:image/png;base64,abc".to_string(),
                },
                detail: None,
            },
            ContentItem::InputAudio {
                audio: AudioReference::Inline {
                    audio_url: "data:audio/wav;base64,def".to_string(),
                },
            },
            ContentItem::InputAudio {
                audio: AudioReference::File {
                    file_id: "file_audio".to_string(),
                },
            },
        ],
        phase: None,
    };

    assert_eq!(
        response_input_to_code_mode_result(response),
        serde_json::json!(
            "Input text\nOutput text\nfile_123\ndata:image/png;base64,abc\ndata:audio/wav;base64,def\nfile_audio"
        )
    );
}
