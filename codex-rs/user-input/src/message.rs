//! Converts user inputs to model content and records image positions for UI events.

use crate::local_media::audio_mime_for_path;
use codex_protocol::models::AudioReference;
use codex_protocol::models::ContentItem;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::audio_close_tag_text;
use codex_protocol::models::local_audio_open_tag_text_with_path;
use codex_protocol::models::local_image_content_items;
use codex_protocol::models::local_image_content_items_with_label_number;
use codex_protocol::user_input::UserInput;
use codex_utils_image::data_url_from_bytes;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalMediaKind {
    Audio,
    Image,
}

impl LocalMediaKind {
    fn name(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Image => "image",
        }
    }
}

fn local_media_error_placeholder(
    path: &std::path::Path,
    error: impl std::fmt::Display,
    media_kind: LocalMediaKind,
) -> ContentItem {
    let media_name = media_kind.name();
    let path = path.display();
    ContentItem::InputText {
        text: format!("Codex could not read the local {media_name} at `{path}`: {error}"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalImagePreparation {
    Process,
    Defer,
}

fn unsupported_audio_error_placeholder(path: &std::path::Path) -> ContentItem {
    ContentItem::InputText {
        text: format!(
            "Codex cannot attach audio at `{}`: unsupported audio format; use wav, mp3, m4a, webm, or ogg.",
            path.display()
        ),
    }
}

fn local_audio_content_items(
    path: &std::path::Path,
    file_bytes: &[u8],
    label_number: usize,
) -> Vec<ContentItem> {
    let Some(mime) = audio_mime_for_path(path) else {
        return vec![unsupported_audio_error_placeholder(path)];
    };

    vec![
        ContentItem::InputText {
            text: local_audio_open_tag_text_with_path(label_number, path),
        },
        ContentItem::InputAudio {
            audio: AudioReference::Inline {
                audio_url: data_url_from_bytes(mime, file_bytes),
            },
        },
        ContentItem::InputText {
            text: audio_close_tag_text(),
        },
    ]
}

pub fn from_user_input(
    items: Vec<UserInput>,
    local_image_preparation: LocalImagePreparation,
    user_image_content_indices: &mut HashMap<usize, usize>,
) -> ResponseInputItem {
    user_image_content_indices.clear();
    let mut image_index = 0;
    let mut audio_index = 0;
    let mut content = Vec::new();
    for (input_index, input) in items.into_iter().enumerate() {
        let generated = match input {
            UserInput::Text { text, .. } => vec![ContentItem::InputText { text }],
            UserInput::Image { image, detail, .. } => {
                image_index += 1;
                let detail = detail.unwrap_or(DEFAULT_IMAGE_DETAIL);
                vec![ContentItem::InputImage {
                    image,
                    detail: Some(detail),
                }]
            }
            UserInput::LocalImage { path, detail, .. } => {
                image_index += 1;
                let detail = detail.unwrap_or(DEFAULT_IMAGE_DETAIL);
                match std::fs::read(&path) {
                    Ok(file_bytes) => match local_image_preparation {
                        LocalImagePreparation::Process => {
                            local_image_content_items_with_label_number(
                                &path,
                                file_bytes,
                                Some(image_index),
                                detail,
                            )
                        }
                        LocalImagePreparation::Defer => local_image_content_items(
                            &path,
                            data_url_from_bytes("application/octet-stream", &file_bytes),
                            Some(image_index),
                            detail,
                        ),
                    },
                    Err(err) => vec![local_media_error_placeholder(
                        &path,
                        err,
                        LocalMediaKind::Image,
                    )],
                }
            }
            UserInput::Audio { audio } => {
                audio_index += 1;
                vec![ContentItem::InputAudio { audio }]
            }
            UserInput::LocalAudio { path } => {
                audio_index += 1;
                match std::fs::read(&path) {
                    Ok(file_bytes) => local_audio_content_items(&path, &file_bytes, audio_index),
                    Err(err) => vec![local_media_error_placeholder(
                        &path,
                        err,
                        LocalMediaKind::Audio,
                    )],
                }
            }
            _ => Vec::new(), // Skills and mentions are injected later in core.
        };
        // Local images add framing text, while mentions and skills add no content.
        // Capture the image's actual slot before preparation replaces its reference.
        for (offset, item) in generated.iter().enumerate() {
            if matches!(item, ContentItem::InputImage { .. }) {
                user_image_content_indices.insert(input_index, content.len() + offset);
            }
        }
        content.extend(generated);
    }
    ResponseInputItem::Message {
        role: "user".to_string(),
        content,
        phase: None,
    }
}
