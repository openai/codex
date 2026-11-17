#![cfg(test)]

use super::*;
use anyhow::Result;
use codex_protocol::models::image_close_tag_text;
use codex_protocol::models::local_image_open_tag_text_with_path;
use pretty_assertions::assert_eq;

// A tiny valid PNG (1x1) so image conversion tests don't depend on cross-crate
// file paths, which break under Bazel sandboxing.
const TINY_PNG_BYTES: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2, 0, 0, 5, 0, 1,
    122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[test]
fn resolved_media_renders_after_source_files_are_removed() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let missing = dir.path().join("missing.png");
    let path = dir.path().join("image.png");
    let audio_path = dir.path().join("clip.wav");
    std::fs::write(&path, TINY_PNG_BYTES)?;
    std::fs::write(&audio_path, b"audio")?;

    let image = ImageReference::File {
        file_id: "file_image".into(),
    };
    let audio = AudioReference::File {
        file_id: "file_audio".into(),
    };
    let inline = ImageReference::Inline {
        image_url: data_url_from_bytes("image/png", TINY_PNG_BYTES),
    };
    let inputs = vec![
        UserInput::Text {
            text: "compare".into(),
            text_elements: vec![TextElement::new((0..7).into(), Some("selection".into()))],
        },
        UserInput::Mention {
            name: "app".into(),
            path: "app://example".into(),
        },
        UserInput::Skill {
            name: "skill".into(),
            path: dir.path().join("SKILL.md"),
        },
        UserInput::LocalImage {
            path: missing.clone(),
            detail: None,
        },
        UserInput::Image {
            image: image.clone(),
            detail: None,
        },
        UserInput::Audio {
            audio: audio.clone(),
        },
        UserInput::LocalAudio {
            path: audio_path.clone(),
        },
        UserInput::LocalImage {
            path: path.clone(),
            detail: Some(ImageDetail::Original),
        },
        UserInput::Image {
            image: inline.clone(),
            detail: Some(ImageDetail::Original),
        },
    ];
    let resolved: Vec<_> = inputs
        .iter()
        .cloned()
        .map(ResolvedUserInput::from_client)
        .collect();
    for (original, item) in inputs.iter().zip(&resolved) {
        let wire = serde_json::to_value(original)?;
        if matches!(
            original,
            UserInput::LocalImage { .. } | UserInput::LocalAudio { .. }
        ) {
            // A path alone cannot rehydrate an already-captured attachment.
            assert!(serde_json::from_value::<ResolvedUserInput>(wire).is_err());
        } else {
            assert_eq!(serde_json::to_value(item)?, wire);
        }
    }
    let serialized = serde_json::to_vec(&resolved)?;
    dir.close()?;
    let restored: Vec<ResolvedUserInput> = serde_json::from_slice(&serialized)?;
    assert_eq!(restored, resolved);
    assert_eq!(
        restored
            .iter()
            .map(ResolvedUserInput::to_user_input)
            .collect::<Vec<_>>(),
        inputs
    );

    // Filtering a turn changes both label numbers and which UI positions own images.
    let (subset, positions) =
        ResolvedUserInput::into_message(&restored[6..8], LocalImagePreparation::Defer);
    let ResponseInputItem::Message { content, .. } = subset else {
        panic!("expected a user message");
    };
    assert_eq!(positions, HashMap::from([(1, 4)]));
    assert_eq!(
        content[0],
        ContentItem::InputText {
            text: local_audio_open_tag_text_with_path(/*label_number*/ 1, &audio_path)
        }
    );
    assert_eq!(
        content[3],
        ContentItem::InputText {
            text: local_image_open_tag_text_with_path(/*label_number*/ 1, &path)
        }
    );

    for mode in [LocalImagePreparation::Defer, LocalImagePreparation::Process] {
        let (message, positions) = ResolvedUserInput::into_message(&restored, mode);
        let ResponseInputItem::Message { content, .. } = message else {
            panic!("expected a user message");
        };
        assert_eq!(positions, HashMap::from([(4, 2), (7, 8), (8, 10)]));
        assert_eq!(
            content[0],
            ContentItem::InputText {
                text: "compare".into()
            }
        );
        let ContentItem::InputText { text } = &content[1] else {
            panic!("expected a read error");
        };
        assert!(text.starts_with(&format!(
            "Codex could not read the local image at `{}`:",
            missing.display()
        )));
        let image_url = match mode {
            LocalImagePreparation::Defer => {
                data_url_from_bytes("application/octet-stream", TINY_PNG_BYTES)
            }
            LocalImagePreparation::Process => codex_utils_image::load_for_prompt_bytes(
                &path,
                TINY_PNG_BYTES.to_vec(),
                codex_utils_image::PromptImageMode::Original,
            )?
            .into_data_url(),
        };
        assert_eq!(
            &content[2..],
            &[
                ContentItem::InputImage {
                    image: image.clone(),
                    detail: Some(DEFAULT_IMAGE_DETAIL)
                },
                ContentItem::InputAudio {
                    audio: audio.clone()
                },
                ContentItem::InputText {
                    text: local_audio_open_tag_text_with_path(/*label_number*/ 2, &audio_path)
                },
                ContentItem::InputAudio {
                    audio: AudioReference::Inline {
                        audio_url: "data:audio/wav;base64,YXVkaW8=".into()
                    }
                },
                ContentItem::InputText {
                    text: audio_close_tag_text()
                },
                ContentItem::InputText {
                    text: local_image_open_tag_text_with_path(/*label_number*/ 3, &path)
                },
                ContentItem::InputImage {
                    image: ImageReference::Inline { image_url },
                    detail: Some(ImageDetail::Original)
                },
                ContentItem::InputText {
                    text: image_close_tag_text()
                },
                ContentItem::InputImage {
                    image: inline.clone(),
                    detail: Some(ImageDetail::Original)
                },
            ]
        );
    }
    Ok(())
}
