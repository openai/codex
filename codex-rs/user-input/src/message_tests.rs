#![cfg(test)]

use crate::LocalImagePreparation;
use crate::ResolvedUserInput;
use anyhow::Result;
use codex_protocol::models::AudioReference;
use codex_protocol::models::ContentItem;
use codex_protocol::models::DEFAULT_IMAGE_DETAIL;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::audio_close_tag_text;
use codex_protocol::models::image_close_tag_text;
use codex_protocol::models::local_audio_open_tag_text_with_path;
use codex_protocol::models::local_image_open_tag_text_with_path;
use codex_protocol::user_input::UserInput;
use codex_utils_image::data_url_from_bytes;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::tempdir;

// A tiny valid PNG (1x1) so image conversion tests don't depend on cross-crate
// file paths, which break under Bazel sandboxing.
const TINY_PNG_BYTES: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2, 0, 0, 5, 0, 1,
    122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn user_message(items: Vec<UserInput>) -> ResponseInputItem {
    let items: Vec<_> = items
        .into_iter()
        .map(ResolvedUserInput::from_client)
        .collect();
    ResolvedUserInput::into_message(&items, LocalImagePreparation::Process).0
}

#[test]
fn serializes_image_user_input_without_tags() -> Result<()> {
    let image_url = "data:image/png;base64,abc".to_string();

    let item = user_message(vec![UserInput::Image {
        image: ImageReference::Inline {
            image_url: image_url.clone(),
        },
        detail: None,
    }]);

    match item {
        ResponseInputItem::Message { content, .. } => {
            let expected = vec![ContentItem::InputImage {
                image: ImageReference::Inline { image_url },
                detail: Some(DEFAULT_IMAGE_DETAIL),
            }];
            assert_eq!(content, expected);
        }
        other => panic!("expected message response but got {other:?}"),
    }

    Ok(())
}

/// A file-backed user image must reach the Responses API without being resolved by Core.
#[test]
fn file_image_user_input_serializes_file_id() -> Result<()> {
    let file_id = "file_123".to_string();

    let item = user_message(vec![UserInput::Image {
        image: ImageReference::File {
            file_id: file_id.clone(),
        },
        detail: None,
    }]);

    let expected = ResponseInputItem::Message {
        role: "user".to_string(),
        content: vec![ContentItem::InputImage {
            image: ImageReference::File { file_id },
            detail: Some(DEFAULT_IMAGE_DETAIL),
        }],
        phase: None,
    };
    assert_eq!(item, expected);
    assert_eq!(
        serde_json::to_value(item)?,
        serde_json::json!({
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_image",
                "file_id": "file_123",
                "detail": "high"
            }]
        })
    );

    Ok(())
}

#[test]
fn serializes_audio_user_input_without_tags() -> Result<()> {
    for (audio, wire) in [
        (
            AudioReference::Inline {
                audio_url: "data:audio/wav;base64,abc".into(),
            },
            serde_json::json!({"type": "input_audio", "audio_url": "data:audio/wav;base64,abc"}),
        ),
        (
            AudioReference::File {
                file_id: "file_audio".into(),
            },
            serde_json::json!({"type": "input_audio", "file_id": "file_audio"}),
        ),
    ] {
        let input = UserInput::Audio {
            audio: audio.clone(),
        };
        assert_eq!(
            serde_json::from_value::<UserInput>(serde_json::to_value(&input)?)?,
            input
        );
        let item = user_message(vec![input]);
        assert_eq!(
            item,
            ResponseInputItem::Message {
                role: "user".into(),
                content: vec![ContentItem::InputAudio { audio }],
                phase: None,
            }
        );
        assert_eq!(
            serde_json::to_value(&item)?,
            serde_json::json!({
                "type": "message", "role": "user", "content": [wire],
            })
        );
        assert_eq!(
            serde_json::from_value::<ResponseInputItem>(serde_json::to_value(&item)?)?,
            item
        );
    }
    Ok(())
}

#[test]
fn serializes_local_audio_user_input_with_label_and_data_url() -> Result<()> {
    let temp_dir = tempdir()?;
    for (extension, mime) in [
        ("wav", "audio/wav"),
        ("mp3", "audio/mpeg"),
        ("m4a", "audio/mp4"),
        ("webm", "audio/webm"),
        ("ogg", "audio/ogg"),
    ] {
        let audio_path = temp_dir.path().join(format!("sample.{extension}"));
        std::fs::write(&audio_path, b"audio")?;

        let item = user_message(vec![UserInput::LocalAudio {
            path: audio_path.clone(),
        }]);

        assert_eq!(
            item,
            ResponseInputItem::Message {
                role: "user".to_string(),
                content: vec![
                    ContentItem::InputText {
                        text: local_audio_open_tag_text_with_path(
                            /*label_number*/ 1,
                            &audio_path,
                        ),
                    },
                    ContentItem::InputAudio {
                        audio: AudioReference::Inline {
                            audio_url: format!("data:{mime};base64,YXVkaW8=")
                        },
                    },
                    ContentItem::InputText {
                        text: audio_close_tag_text(),
                    },
                ],
                phase: None,
            }
        );
    }

    Ok(())
}

#[test]
fn replaces_unsupported_local_audio_format_with_placeholder() -> Result<()> {
    let temp_dir = tempdir()?;
    let audio_path = temp_dir.path().join("sample.flac");
    std::fs::write(&audio_path, b"audio")?;

    let item = user_message(vec![UserInput::LocalAudio {
        path: audio_path.clone(),
    }]);

    assert_eq!(
        item,
        ResponseInputItem::Message {
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!(
                    "Codex cannot attach audio at `{}`: unsupported audio format; use wav, mp3, m4a, webm, or ogg.",
                    audio_path.display()
                ),
            }],
            phase: None,
        }
    );

    Ok(())
}

#[test]
fn replaces_unreadable_local_audio_with_placeholder() {
    let audio_path = PathBuf::from("missing.wav");

    let item = user_message(vec![UserInput::LocalAudio { path: audio_path }]);

    let ResponseInputItem::Message { content, .. } = item else {
        panic!("expected message response");
    };
    let [ContentItem::InputText { text }] = content.as_slice() else {
        panic!("expected local audio error placeholder");
    };
    assert!(
        text.starts_with("Codex could not read the local audio at `missing.wav`: "),
        "unexpected placeholder: {text}"
    );
}

#[test]
fn image_user_input_preserves_requested_detail() -> Result<()> {
    let image_url = "data:image/png;base64,abc".to_string();

    let item = user_message(vec![UserInput::Image {
        image: ImageReference::Inline {
            image_url: image_url.clone(),
        },
        detail: Some(ImageDetail::Original),
    }]);

    match item {
        ResponseInputItem::Message { content, .. } => {
            assert_eq!(
                content.first(),
                Some(&ContentItem::InputImage {
                    image: ImageReference::Inline { image_url },
                    detail: Some(ImageDetail::Original),
                })
            );
        }
        other => panic!("expected message response but got {other:?}"),
    }

    Ok(())
}

/// Image associations use original input and expanded content positions, so omitted inputs,
/// local-image framing, failed reads, and duplicate URLs cannot shift a later substitution.
#[test]
fn user_input_image_indices_survive_content_expansion() -> Result<()> {
    let dir = tempdir()?;
    let local_path = dir.path().join("local.png");
    std::fs::write(&local_path, TINY_PNG_BYTES)?;
    let inline_image = UserInput::Image {
        image: ImageReference::Inline {
            image_url: data_url_from_bytes("image/png", TINY_PNG_BYTES),
        },
        detail: None,
    };
    let inputs = vec![
        UserInput::Text {
            text: "compare images".to_string(),
            text_elements: Vec::new(),
        },
        UserInput::Mention {
            name: "app".to_string(),
            path: "app://example".to_string(),
        },
        UserInput::Skill {
            name: "skill".to_string(),
            path: dir.path().join("SKILL.md"),
        },
        UserInput::LocalImage {
            path: dir.path().join("missing.png"),
            detail: None,
        },
        UserInput::LocalImage {
            path: local_path,
            detail: None,
        },
        inline_image.clone(),
        inline_image,
    ];
    let inputs: Vec<_> = inputs
        .into_iter()
        .map(ResolvedUserInput::from_client)
        .collect();
    for preparation in [LocalImagePreparation::Defer, LocalImagePreparation::Process] {
        let (item, indices) = ResolvedUserInput::into_message(&inputs, preparation);
        assert_eq!(indices, HashMap::from([(4, 3), (5, 5), (6, 6)]));
        let ResponseInputItem::Message { content, .. } = item else {
            panic!("expected a user message");
        };
        for content_index in indices.into_values() {
            assert!(matches!(
                content[content_index],
                ContentItem::InputImage { .. }
            ));
        }
    }
    Ok(())
}

#[test]
fn mixed_remote_and_local_images_share_label_sequence() -> Result<()> {
    let image_url = "data:image/png;base64,abc".to_string();
    let dir = tempdir()?;
    let local_path = dir.path().join("local.png");
    std::fs::write(&local_path, TINY_PNG_BYTES)?;

    let item = user_message(vec![
        UserInput::Image {
            image: ImageReference::Inline {
                image_url: image_url.clone(),
            },
            detail: None,
        },
        UserInput::LocalImage {
            path: local_path.clone(),
            detail: None,
        },
    ]);

    match item {
        ResponseInputItem::Message { content, .. } => {
            assert_eq!(
                content.first(),
                Some(&ContentItem::InputImage {
                    image: ImageReference::Inline { image_url },
                    detail: Some(DEFAULT_IMAGE_DETAIL),
                })
            );
            assert_eq!(
                content.get(1),
                Some(&ContentItem::InputText {
                    text: local_image_open_tag_text_with_path(/*label_number*/ 2, &local_path),
                })
            );
            assert!(matches!(
                content.get(2),
                Some(ContentItem::InputImage { .. })
            ));
            assert_eq!(
                content.get(3),
                Some(&ContentItem::InputText {
                    text: image_close_tag_text(),
                })
            );
        }
        other => panic!("expected message response but got {other:?}"),
    }

    Ok(())
}

#[test]
fn mixed_remote_and_local_audio_share_label_sequence() -> Result<()> {
    let audio_url = "data:audio/wav;base64,abc".to_string();
    let dir = tempdir()?;
    let local_path = dir.path().join("local.mp3");
    std::fs::write(&local_path, b"audio")?;

    let item = user_message(vec![
        UserInput::Audio {
            audio: AudioReference::Inline {
                audio_url: audio_url.clone(),
            },
        },
        UserInput::LocalAudio {
            path: local_path.clone(),
        },
    ]);

    assert_eq!(
        item,
        ResponseInputItem::Message {
            role: "user".to_string(),
            content: vec![
                ContentItem::InputAudio {
                    audio: AudioReference::Inline { audio_url }
                },
                ContentItem::InputText {
                    text: local_audio_open_tag_text_with_path(/*label_number*/ 2, &local_path,),
                },
                ContentItem::InputAudio {
                    audio: AudioReference::Inline {
                        audio_url: "data:audio/mpeg;base64,YXVkaW8=".to_string()
                    }
                },
                ContentItem::InputText {
                    text: audio_close_tag_text(),
                },
            ],
            phase: None,
        }
    );

    Ok(())
}

#[test]
fn local_image_user_input_preserves_requested_detail() -> Result<()> {
    let dir = tempdir()?;
    let local_path = dir.path().join("local.png");
    std::fs::write(&local_path, TINY_PNG_BYTES)?;

    let item = user_message(vec![UserInput::LocalImage {
        path: local_path,
        detail: Some(ImageDetail::Original),
    }]);

    match item {
        ResponseInputItem::Message { content, .. } => {
            assert!(matches!(
                content.get(1),
                Some(ContentItem::InputImage {
                    detail: Some(ImageDetail::Original),
                    ..
                })
            ));
        }
        other => panic!("expected message response but got {other:?}"),
    }

    Ok(())
}

#[test]
fn local_image_read_error_adds_placeholder() -> Result<()> {
    let dir = tempdir()?;
    let missing_path = dir.path().join("missing-image.png");

    let item = user_message(vec![UserInput::LocalImage {
        path: missing_path.clone(),
        detail: None,
    }]);

    match item {
        ResponseInputItem::Message { content, .. } => {
            assert_eq!(content.len(), 1);
            match &content[0] {
                ContentItem::InputText { text } => {
                    let display_path = missing_path.display().to_string();
                    assert!(
                        text.contains(&display_path),
                        "placeholder should mention missing path: {text}"
                    );
                    assert!(
                        text.contains("could not read"),
                        "placeholder should mention read issue: {text}"
                    );
                }
                other => panic!("expected placeholder text but found {other:?}"),
            }
        }
        other => panic!("expected message response but got {other:?}"),
    }

    Ok(())
}

#[test]
fn local_image_non_image_adds_placeholder() -> Result<()> {
    let dir = tempdir()?;
    let json_path = dir.path().join("example.json");
    std::fs::write(&json_path, br#"{"hello":"world"}"#)?;

    let item = user_message(vec![UserInput::LocalImage {
        path: json_path.clone(),
        detail: None,
    }]);

    match item {
        ResponseInputItem::Message { content, .. } => {
            assert_eq!(content.len(), 1);
            match &content[0] {
                ContentItem::InputText { text } => {
                    assert!(
                        text.contains("unsupported image `application/json`"),
                        "placeholder should mention unsupported image MIME: {text}"
                    );
                    assert!(
                        text.contains(&json_path.display().to_string()),
                        "placeholder should mention path: {text}"
                    );
                }
                other => panic!("expected placeholder text but found {other:?}"),
            }
        }
        other => panic!("expected message response but got {other:?}"),
    }

    Ok(())
}

#[test]
fn local_image_unsupported_image_format_adds_placeholder() -> Result<()> {
    let dir = tempdir()?;
    let svg_path = dir.path().join("example.svg");
    std::fs::write(
        &svg_path,
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>"#,
    )?;

    let item = user_message(vec![UserInput::LocalImage {
        path: svg_path.clone(),
        detail: None,
    }]);

    match item {
        ResponseInputItem::Message { content, .. } => {
            assert_eq!(content.len(), 1);
            let expected = format!(
                "Codex cannot attach image at `{}`: unsupported image `image/svg+xml`.",
                svg_path.display()
            );
            match &content[0] {
                ContentItem::InputText { text } => assert_eq!(text, &expected),
                other => panic!("expected placeholder text but found {other:?}"),
            }
        }
        other => panic!("expected message response but got {other:?}"),
    }

    Ok(())
}
