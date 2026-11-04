#![cfg(test)]

use super::*;
use codex_protocol::models::AudioReference;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::ImageReference;
use codex_protocol::models::InternalChatMessageMetadataPassthrough;
use codex_protocol::models::audio_close_tag_text;
use codex_protocol::models::audio_open_tag_text;
use codex_protocol::models::image_close_tag_text;
use codex_protocol::models::local_image_open_tag_text_with_path;
use pretty_assertions::assert_eq;

fn message(content: Vec<ContentItem>) -> ResponseItem {
    ResponseItem::Message {
        status: None,
        encrypted_content: None,
        id: None,
        role: "user".to_string(),
        content,
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

fn image() -> ContentItem {
    ContentItem::InputImage {
        image: ImageReference::Inline {
            image_url: "data:image/png;base64,abc".to_string(),
        },
        detail: None,
    }
}

fn file_image() -> ContentItem {
    ContentItem::InputImage {
        image: ImageReference::File {
            file_id: "file_123".to_string(),
        },
        detail: Some(codex_protocol::models::ImageDetail::Original),
    }
}

fn file_audio() -> ContentItem {
    ContentItem::InputAudio {
        audio: AudioReference::File {
            file_id: "file_audio".to_string(),
        },
    }
}

fn text(value: &str) -> ContentItem {
    ContentItem::InputText {
        text: value.to_string(),
    }
}

fn trim(items: Vec<ResponseItem>, max_tokens: usize) -> Vec<ResponseItem> {
    truncate_retained_messages(
        items.into_iter().map(ResponseItemEnvelope::new).collect(),
        max_tokens,
        RetainedImageBudget::Enabled,
        &FileAudioDurations::default(),
    )
    .into_iter()
    .map(ResponseItemEnvelope::into_item)
    .collect()
}

#[test]
fn image_only_boundary_is_atomic_and_does_not_backfill_older_messages() {
    let newest = message(vec![text("new")]);
    for image in [image(), file_image()] {
        let items = vec![
            message(vec![text("old")]),
            message(vec![image.clone()]),
            newest.clone(),
        ];
        let image_tokens = media::content_item_token_count(
            &image,
            RetainedImageBudget::Enabled,
            &FileAudioDurations::default(),
        );
        for (max_tokens, expected) in [
            (
                image_tokens + 1,
                vec![message(vec![image.clone()]), newest.clone()],
            ),
            (image_tokens, vec![newest.clone()]),
            (1, vec![newest.clone()]),
        ] {
            assert_eq!(trim(items.clone(), max_tokens), expected);
        }
    }
}

#[test]
fn file_audio_is_charged_and_atomic_in_both_image_budget_modes() {
    let audio = file_audio();
    let audio_tokens = media::content_item_token_count(
        &audio,
        RetainedImageBudget::Enabled,
        &FileAudioDurations::default(),
    );
    let newest = message(vec![text("new")]);
    let parts = [text("before"), audio, text("after")];
    let kinds = ["before", "audio", "after"];
    let annotated_boundary = |start: usize| {
        let mut item = message(parts[start..].to_vec());
        let ResponseItem::Message {
            internal_chat_message_metadata_passthrough,
            ..
        } = &mut item
        else {
            unreachable!()
        };
        *internal_chat_message_metadata_passthrough =
            Some(InternalChatMessageMetadataPassthrough {
                content_item_kinds: Some(
                    kinds[start..]
                        .iter()
                        .map(|kind| ContentItemKind((*kind).to_string()))
                        .collect(),
                ),
                ..Default::default()
            });
        item
    };
    let boundary = annotated_boundary(0);
    let newest_tokens = retained_message_token_count(
        &newest,
        RetainedImageBudget::Disabled,
        &FileAudioDurations::default(),
    )
    .max(1);
    let after_tokens = media::content_item_token_count(
        &text("after"),
        RetainedImageBudget::Enabled,
        &FileAudioDurations::default(),
    );
    for image_budget in [RetainedImageBudget::Disabled, RetainedImageBudget::Enabled] {
        for (budget, expected) in [
            (
                retained_message_token_count(
                    &boundary,
                    image_budget,
                    &FileAudioDurations::default(),
                ) + newest_tokens,
                vec![boundary.clone(), newest.clone()],
            ),
            (
                audio_tokens + after_tokens + newest_tokens,
                vec![annotated_boundary(1), newest.clone()],
            ),
            (
                audio_tokens + after_tokens + newest_tokens - 1,
                vec![annotated_boundary(2), newest.clone()],
            ),
        ] {
            let actual = truncate_retained_messages(
                vec![
                    message(vec![text("older")]),
                    boundary.clone(),
                    newest.clone(),
                ]
                .into_iter()
                .map(ResponseItemEnvelope::new)
                .collect(),
                budget,
                image_budget,
                &FileAudioDurations::default(),
            )
            .into_iter()
            .map(ResponseItemEnvelope::into_item)
            .collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn file_audio_cutoff_does_not_backfill_older_uncharged_media() {
    let inline_audio = ContentItem::InputAudio {
        audio: AudioReference::Inline {
            audio_url: "data:audio/wav;base64,older".to_string(),
        },
    };
    let trailing_text = text("after");
    let boundary = message(vec![
        text(&local_image_open_tag_text_with_path(
            /*label_number*/ 7,
            std::path::Path::new("older.png"),
        )),
        image(),
        text(&image_close_tag_text()),
        text("older recording"),
        inline_audio,
        text("newer recording"),
        file_audio(),
        trailing_text.clone(),
    ]);
    let max_tokens = media::content_item_token_count(
        &file_audio(),
        RetainedImageBudget::Enabled,
        &FileAudioDurations::default(),
    ) + media::content_item_token_count(
        &trailing_text,
        RetainedImageBudget::Enabled,
        &FileAudioDurations::default(),
    ) - 1;

    for image_budget in [RetainedImageBudget::Disabled, RetainedImageBudget::Enabled] {
        let actual = truncate_retained_messages(
            vec![ResponseItemEnvelope::new(boundary.clone())],
            max_tokens,
            image_budget,
            &FileAudioDurations::default(),
        )
        .into_iter()
        .map(ResponseItemEnvelope::into_item)
        .collect::<Vec<_>>();
        let [ResponseItem::Message { content, .. }] = actual.as_slice() else {
            panic!("expected one truncated user message");
        };
        assert_eq!(content, std::slice::from_ref(&trailing_text));
    }
}

#[test]
fn older_media_keeps_both_labels_or_is_dropped_after_file_audio_fits() {
    let file = file_audio();
    let file_tokens = media::content_item_token_count(
        &file,
        RetainedImageBudget::Enabled,
        &FileAudioDurations::default(),
    );
    for (open, media, close) in [
        (
            local_image_open_tag_text_with_path(
                /*label_number*/ 7,
                std::path::Path::new("older.png"),
            ),
            image(),
            image_close_tag_text(),
        ),
        (
            audio_open_tag_text(),
            ContentItem::InputAudio {
                audio: AudioReference::Inline {
                    audio_url: "data:audio/wav;base64,older".to_string(),
                },
            },
            audio_close_tag_text(),
        ),
    ] {
        let parts = vec![text(&open), media, text(&close), file.clone()];
        let whole = message(parts.clone());
        let source = message([vec![text("older text")], parts.clone()].concat());
        for image_budget in [RetainedImageBudget::Disabled, RetainedImageBudget::Enabled] {
            let whole_tokens =
                retained_message_token_count(&whole, image_budget, &FileAudioDurations::default());
            for (budget, expected) in [
                (file_tokens, vec![file.clone()]),
                (whole_tokens - 1, vec![file.clone()]),
                (whole_tokens, parts.clone()),
            ] {
                let actual = truncate_retained_messages(
                    vec![ResponseItemEnvelope::new(source.clone())],
                    budget,
                    image_budget,
                    &FileAudioDurations::default(),
                );
                let [
                    ResponseItemEnvelope {
                        item: ResponseItem::Message { content, .. },
                        ..
                    },
                ] = actual.as_slice()
                else {
                    panic!("expected one retained user message");
                };
                assert_eq!(content, &expected);
            }
        }
    }
}

#[test]
fn later_image_parts_preserve_labels_audio_and_annotations() {
    let parts = vec![
        text("earlier text"),
        text(&local_image_open_tag_text_with_path(
            /*label_number*/ 7,
            std::path::Path::new("image.png"),
        )),
        image(),
        text(&image_close_tag_text()),
        text("keep"),
        ContentItem::InputAudio {
            audio: AudioReference::Inline {
                audio_url: "data:audio/wav;base64,abc".to_string(),
            },
        },
    ];
    let kinds = (0..parts.len())
        .map(|i| ContentItemKind(format!("part.{i}")))
        .collect::<Vec<_>>();
    let mut source = message(parts.clone());
    let ResponseItem::Message {
        internal_chat_message_metadata_passthrough,
        ..
    } = &mut source
    else {
        unreachable!()
    };
    *internal_chat_message_metadata_passthrough = Some(InternalChatMessageMetadataPassthrough {
        turn_id: Some("turn-1".to_string()),
        content_item_kinds: Some(kinds.clone()),
        ..Default::default()
    });
    let image_tokens = parts[1..4]
        .iter()
        .map(|item| {
            media::content_item_token_count(
                item,
                RetainedImageBudget::Enabled,
                &FileAudioDurations::default(),
            )
        })
        .sum::<usize>();
    for (max_tokens, start) in [(image_tokens, 4), (image_tokens + 1, 1)] {
        let mut expected = source.clone();
        let ResponseItem::Message {
            content,
            internal_chat_message_metadata_passthrough,
            ..
        } = &mut expected
        else {
            unreachable!()
        };
        *content = parts[start..].to_vec();
        internal_chat_message_metadata_passthrough
            .as_mut()
            .unwrap()
            .content_item_kinds = Some(kinds[start..].to_vec());
        assert_eq!(trim(vec![source.clone()], max_tokens), vec![expected]);
    }
}

#[test]
fn image_treatment_preserves_text_only_boundary_behavior() {
    let source = ResponseItemEnvelope::new(message(vec![
        text("earlier text"),
        text(&"middle".repeat(20)),
        text("later text"),
        ContentItem::InputAudio {
            audio: AudioReference::Inline {
                audio_url: "data:audio/wav;base64,abc".to_string(),
            },
        },
    ]));
    for max_tokens in [0, 1, 4, 16, 100] {
        assert_eq!(
            truncate_retained_messages(
                vec![source.clone()],
                max_tokens,
                RetainedImageBudget::Enabled,
                &FileAudioDurations::default()
            ),
            truncate_retained_messages_for_remote_compaction(
                vec![source.clone()],
                max_tokens,
                &FileAudioDurations::default()
            ),
        );
    }
}

#[test]
fn image_treatment_preserves_client_developer_boundary_behavior() {
    let mut item = message(vec![image(), text(&"a".repeat(1000))]);
    let ResponseItem::Message { role, .. } = &mut item else {
        unreachable!()
    };
    *role = "developer".to_string();
    let source = ResponseItemEnvelope {
        item,
        metadata: Some(CodexHarnessMetadata {
            client_authored: true,
            ..Default::default()
        }),
    };
    for max_tokens in [1871, 1879, 1880, 1950, 2200] {
        assert_eq!(
            truncate_retained_messages(
                vec![source.clone()],
                max_tokens,
                RetainedImageBudget::Enabled,
                &FileAudioDurations::default()
            ),
            truncate_retained_messages_for_remote_compaction(
                vec![source.clone()],
                max_tokens,
                &FileAudioDurations::default()
            ),
        );
    }
}
