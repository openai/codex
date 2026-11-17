//! Resolves one client input at a time. A local attachment carries its captured contents
//! or its read error. Its path is retained for display, labels, and diagnostics.

use crate::local_media::audio_mime_for_path;
use crate::local_media::read_local_audio_url;
use crate::local_media::read_local_image_bytes;
use codex_protocol::models::AudioReference;
use codex_protocol::models::ContentItem;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::audio_close_tag_text;
use codex_protocol::models::local_audio_open_tag_text_with_path;
use codex_protocol::models::local_image_content_items;
use codex_protocol::models::local_image_content_items_with_label_number;
use codex_protocol::user_input::TextElement;
use codex_protocol::user_input::UserInput;
use codex_utils_image::data_url_from_bytes;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalImagePreparation {
    Process,
    Defer,
}

/// One input admitted to the harness. Clones share captured attachment payloads.
#[derive(Clone, PartialEq, derive_more::Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResolvedUserInput {
    Text {
        #[debug("{:?} <{} bytes>", &text[..text.floor_char_boundary(/*index*/ 512)], text.len())]
        text: String,
        #[serde(default)]
        text_elements: Vec<TextElement>,
    },
    Image {
        #[debug(skip)]
        #[serde(flatten)]
        image: Arc<ImageReference>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
    },
    LocalImage {
        path: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<ImageDetail>,
        #[debug(skip)]
        bytes: Result<Arc<[u8]>, String>,
    },
    Audio {
        #[debug(skip)]
        #[serde(flatten)]
        audio: Arc<AudioReference>,
    },
    LocalAudio {
        path: PathBuf,
        #[debug(skip)]
        audio_url: Result<Arc<str>, String>,
    },
    Skill {
        name: String,
        path: PathBuf,
    },
    Mention {
        name: String,
        path: String,
    },
}

impl ResolvedUserInput {
    /// Reads a client-selected attachment on this host. Executor-owned paths must be resolved there.
    pub fn from_client(input: UserInput) -> Self {
        match input {
            UserInput::Text {
                text,
                text_elements,
            } => Self::Text {
                text,
                text_elements,
            },
            UserInput::Image { image, detail } => Self::Image {
                image: Arc::new(image),
                detail,
            },
            UserInput::LocalImage { path, detail } => {
                let bytes = read_local_image_bytes(&path).map(Arc::from).map_err(|err| {
                    format!(
                        "Codex could not read the local image at `{}`: {err}",
                        path.display()
                    )
                });
                Self::LocalImage {
                    path,
                    detail,
                    bytes,
                }
            }
            UserInput::Audio { audio } => Self::Audio {
                audio: Arc::new(audio),
            },
            UserInput::LocalAudio { path } => {
                let audio_url = read_local_audio_url(&path).map(Arc::from).map_err(|err| {
                    let name = path.display();
                    if audio_mime_for_path(&path).is_none() {
                        format!("Codex cannot attach audio at `{name}`: unsupported audio format; use wav, mp3, m4a, webm, or ogg.")
                    } else {
                        format!("Codex could not read the local audio at `{name}`: {err}")
                    }
                });
                Self::LocalAudio { path, audio_url }
            }
            UserInput::Skill { name, path } => Self::Skill { name, path },
            UserInput::Mention { name, path } => Self::Mention { name, path },
            _ => Self::text("Codex cannot prepare this input type.".into()),
        }
    }

    pub fn text(text: String) -> Self {
        Self::Text {
            text,
            text_elements: Vec::new(),
        }
    }

    /// Projects the original client-facing input, including UI text spans and local display paths.
    pub fn to_user_input(&self) -> UserInput {
        match self {
            Self::Text {
                text,
                text_elements,
            } => UserInput::Text {
                text: text.clone(),
                text_elements: text_elements.clone(),
            },
            Self::Image { image, detail } => UserInput::Image {
                image: image.as_ref().clone(),
                detail: *detail,
            },
            Self::LocalImage { path, detail, .. } => UserInput::LocalImage {
                path: path.clone(),
                detail: *detail,
            },
            Self::Audio { audio } => UserInput::Audio {
                audio: audio.as_ref().clone(),
            },
            Self::LocalAudio { path, .. } => UserInput::LocalAudio { path: path.clone() },
            Self::Skill { name, path } => UserInput::Skill {
                name: name.clone(),
                path: path.clone(),
            },
            Self::Mention { name, path } => UserInput::Mention {
                name: name.clone(),
                path: path.clone(),
            },
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text, .. } => Some(text),
            _ => None,
        }
    }

    pub fn is_uploaded_audio_file(&self) -> bool {
        matches!(self, Self::Audio { audio } if matches!(audio.as_ref(), AudioReference::File { .. }))
    }

    pub fn is_image(&self) -> bool {
        matches!(self, Self::Image { .. } | Self::LocalImage { .. })
    }

    /// Renders the captured inputs and maps the supplied slice's image indices to message slots.
    pub fn into_message(
        items: &[Self],
        images: LocalImagePreparation,
    ) -> (ResponseInputItem, HashMap<usize, usize>) {
        let mut image_positions = HashMap::new();
        let mut content = Vec::new();
        let mut image_number = 0;
        let mut audio_number = 0;
        for (index, item) in items.iter().enumerate() {
            let start = content.len();
            match item {
                Self::Text { text, .. } => {
                    content.push(ContentItem::InputText { text: text.clone() })
                }
                Self::Image { image, detail } => {
                    image_number += 1;
                    content.push(ContentItem::InputImage {
                        image: image.as_ref().clone(),
                        detail: Some(detail.unwrap_or(DEFAULT_IMAGE_DETAIL)),
                    });
                }
                Self::LocalImage {
                    path,
                    bytes,
                    detail,
                } => {
                    image_number += 1;
                    let detail = detail.unwrap_or(DEFAULT_IMAGE_DETAIL);
                    match bytes {
                        Ok(bytes) => content.extend(match images {
                            LocalImagePreparation::Process => {
                                local_image_content_items_with_label_number(
                                    path,
                                    bytes.to_vec(),
                                    Some(image_number),
                                    detail,
                                )
                            }
                            LocalImagePreparation::Defer => local_image_content_items(
                                path,
                                data_url_from_bytes("application/octet-stream", bytes),
                                Some(image_number),
                                detail,
                            ),
                        }),
                        Err(message) => content.push(ContentItem::InputText {
                            text: message.clone(),
                        }),
                    }
                }
                Self::Audio { audio } => {
                    audio_number += 1;
                    content.push(ContentItem::InputAudio {
                        audio: audio.as_ref().clone(),
                    });
                }
                Self::LocalAudio { path, audio_url } => {
                    audio_number += 1;
                    match audio_url {
                        Ok(audio_url) => content.extend([
                            ContentItem::InputText {
                                text: local_audio_open_tag_text_with_path(audio_number, path),
                            },
                            ContentItem::InputAudio {
                                audio: AudioReference::Inline {
                                    audio_url: audio_url.to_string(),
                                },
                            },
                            ContentItem::InputText {
                                text: audio_close_tag_text(),
                            },
                        ]),
                        Err(message) => content.push(ContentItem::InputText {
                            text: message.clone(),
                        }),
                    }
                }
                Self::Skill { .. } | Self::Mention { .. } => continue,
            }
            if let Some(offset) = content[start..]
                .iter()
                .position(|item| matches!(item, ContentItem::InputImage { .. }))
            {
                image_positions.insert(index, start + offset);
            }
        }
        (
            ResponseInputItem::Message {
                role: "user".into(),
                content,
                phase: None,
            },
            image_positions,
        )
    }
}

#[cfg(test)]
#[path = "resolved_user_input_tests.rs"]
mod tests;
