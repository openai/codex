use super::MAX_TRANSCRIPT_IMAGE_BYTES;
use super::TranscriptImageInput;
use super::TranscriptImages;
use crate::CollectedContext;
use crate::ContextBudgeting;
use crate::ContextPresentation;
use crate::ContextProfile;
use crate::ContextSection;
use crate::ConversationTranscriptEntry;
use crate::ConversationTranscriptEntryKind;
use crate::HistoryTruncation;
use crate::PlannedAction;
use crate::PlannedActionKind;
use crate::PreparedTranscript;
use crate::RequestBudget;
use crate::TranscriptContent;
use crate::composition::user_message;
use crate::prepare_context;
use crate::prepare_sync_images;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;
use std::borrow::Cow;
use std::future::Future;
use std::pin::pin;
use std::task::Context;
use std::task::Waker;

fn image(url: &str) -> ContentItem {
    ContentItem::InputImage {
        image: ImageReference::Inline {
            image_url: url.into(),
        },
        detail: Some(ImageDetail::High),
    }
}

#[test]
fn image_selection_preserves_source_policy_order_and_both_limits() {
    let history = [
        ResponseItem::Message {
            status: None,
            encrypted_content: None,
            id: None,
            role: "user".into(),
            content: ["first", "second", "third", "fourth"].map(image).to_vec(),
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("screenshot".into()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputImage {
                    image: ImageReference::Inline {
                        image_url: "tool".into(),
                    },
                    detail: Some(ImageDetail::High),
                },
            ]),
            internal_chat_message_metadata_passthrough: None,
        },
    ];
    let repl = [image("repl")];
    let input = TranscriptImageInput {
        enabled: true,
        include_tool_outputs: true,
        node_repl_images: &repl,
    };
    assert_eq!(
        TranscriptImages::collect(&history, input),
        TranscriptImages {
            images: ["third", "fourth", "tool", "repl"].map(image).to_vec(),
            omitted_bytes: "firstsecond".len(),
        }
    );
    assert_eq!(
        TranscriptImages::collect(
            &history,
            TranscriptImageInput {
                include_tool_outputs: false,
                ..input
            }
        ),
        TranscriptImages {
            images: ["first", "second", "third", "fourth"].map(image).to_vec(),
            omitted_bytes: 0,
        }
    );
    assert_eq!(
        TranscriptImages::collect(
            &history,
            TranscriptImageInput {
                enabled: false,
                ..input
            }
        ),
        TranscriptImages::default()
    );

    let oversized = "x".repeat(MAX_TRANSCRIPT_IMAGE_BYTES + 1);
    let fits_alone = "y".repeat(MAX_TRANSCRIPT_IMAGE_BYTES);
    let repl = [image(&oversized), image(&fits_alone), image("recent")];
    assert_eq!(
        TranscriptImages::collect(
            &[],
            TranscriptImageInput {
                node_repl_images: &repl,
                ..input
            }
        ),
        TranscriptImages {
            images: vec![image("recent")],
            omitted_bytes: oversized.len() + fits_alone.len(),
        }
    );
}

/// File-backed history images share count and reference-byte limits with inline images.
#[test]
fn file_images_are_selected_from_history_sources() {
    let file_image = ContentItem::InputImage {
        image: ImageReference::File {
            file_id: "file_123".to_string(),
        },
        detail: Some(ImageDetail::High),
    };
    let tool_file_image = ContentItem::InputImage {
        image: ImageReference::File {
            file_id: "file_tool".to_string(),
        },
        detail: Some(ImageDetail::High),
    };
    let history = [
        ResponseItem::Message {
            status: None,
            encrypted_content: None,
            id: None,
            role: "user".into(),
            content: vec![image("first"), file_image.clone()],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("screenshot".into()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputImage {
                    image: ImageReference::File {
                        file_id: "file_tool".to_string(),
                    },
                    detail: Some(ImageDetail::High),
                },
                FunctionCallOutputContentItem::InputImage {
                    image: ImageReference::Inline {
                        image_url: "third".into(),
                    },
                    detail: Some(ImageDetail::High),
                },
            ]),
            internal_chat_message_metadata_passthrough: None,
        },
    ];
    let repl = [image("fourth")];

    assert_eq!(
        TranscriptImages::collect(
            &history,
            TranscriptImageInput {
                enabled: true,
                include_tool_outputs: true,
                node_repl_images: &repl,
            }
        ),
        TranscriptImages {
            images: vec![file_image, tool_file_image, image("third"), image("fourth")],
            omitted_bytes: "first".len(),
        }
    );

    // Count-only eviction must remain observable even when no inline bytes were omitted.
    let file_images =
        ["file_1", "file_2", "file_3", "file_4", "file_5"].map(|file_id| ContentItem::InputImage {
            image: ImageReference::File {
                file_id: file_id.to_owned(),
            },
            detail: Some(ImageDetail::High),
        });
    let history = [ResponseItem::Message {
        status: None,
        encrypted_content: None,
        id: None,
        role: "user".into(),
        content: file_images.to_vec(),
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }];
    let selected = TranscriptImages::collect(
        &history,
        TranscriptImageInput {
            enabled: true,
            include_tool_outputs: true,
            node_repl_images: &[],
        },
    );
    assert_eq!(
        selected,
        TranscriptImages {
            images: file_images[1..].to_vec(),
            omitted_bytes: "file_1".len(),
        }
    );
    let context = CollectedContext {
        sections: vec![ContextSection::TranscriptImages(selected)],
    }
    .compose_prepared(
        ContextPresentation::Async,
        PreparedTranscript {
            items: Vec::new(),
            omission_note: None,
            truncations: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(
        context
            .truncations
            .iter()
            .map(|observation| (
                observation.component,
                observation.original_bytes,
                observation.retained_bytes,
            ))
            .collect::<Vec<_>>(),
        vec![("transcript_image", "file_1".len(), 0)]
    );

    // Newer evidence evicts the full-budget ID; the oversized ID must not displace it.
    let content = [
        ImageReference::File {
            file_id: "x".repeat(MAX_TRANSCRIPT_IMAGE_BYTES),
        },
        ImageReference::Inline {
            image_url: "recent".into(),
        },
        ImageReference::File {
            file_id: "y".repeat(MAX_TRANSCRIPT_IMAGE_BYTES + 1),
        },
    ]
    .map(|image| ContentItem::InputImage {
        image,
        detail: Some(ImageDetail::High),
    });
    assert_eq!(
        TranscriptImages::collect(
            &[user_message(content.to_vec())],
            TranscriptImageInput {
                enabled: true,
                include_tool_outputs: true,
                node_repl_images: &[],
            },
        ),
        TranscriptImages {
            images: vec![image("recent")],
            omitted_bytes: 2 * MAX_TRANSCRIPT_IMAGE_BYTES + 1,
        }
    );
}

#[test]
fn fixed_screenshot_admission_precedes_optional_enclosing_call() {
    let profile = ContextProfile::synchronous();
    let estimate =
        |item: &ResponseItem| i64::try_from(serde_json::to_vec(item).unwrap().len()).unwrap();
    let prepare = |kind, context_window, screenshot| {
        let arguments = r#"{"code":"await checkout()"}"#;
        let collected = CollectedContext {
            sections: vec![
                ContextSection::ConversationTranscript {
                    items: vec![ConversationTranscriptEntry {
                        kind,
                        content: TranscriptContent::Text(arguments.to_owned()),
                        original_bytes: arguments.len(),
                        retained_source: None,
                    }],
                },
                ContextSection::TranscriptImages(TranscriptImages {
                    images: vec![image(screenshot)],
                    omitted_bytes: 0,
                }),
                ContextSection::PlannedAction(PlannedAction {
                    kind: PlannedActionKind::Nested {
                        json: r#"{"tool_name":"submit_order","arguments":{}}"#.into(),
                        enclosing_invocation_id: "active-wrapper".into(),
                        enclosing_call: None,
                    },
                    reason: None,
                    tool_descriptions: None,
                }),
            ],
        };
        let mut context = prepare_context(
            Cow::Owned(collected),
            &profile,
            /*entry_number_offset*/ 1,
            ContextPresentation::SyncDelta {
                session_id: "review",
            },
            ContextBudgeting::Fixed,
            &[],
        )
        .unwrap();
        {
            let future = pin!(prepare_sync_images(
                &mut context,
                /*history*/ &[],
                Some(context_window),
                |image, _| Some(Cow::Borrowed(image)),
                estimate,
                || std::future::ready(0),
                ContextBudgeting::Fixed,
            ));
            assert!(
                future
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_ready()
            );
        }
        context
    };

    let ordinary = || ConversationTranscriptEntryKind::ToolCall("tool js call".into());
    let enclosing = || ConversationTranscriptEntryKind::EnclosingToolCall("tool js call".into());
    let baseline = prepare(ordinary(), /*context_window*/ 10_000, "screenshot");
    assert_eq!(
        baseline
            .section_costs()
            .map(|(_, cost)| cost.image_count)
            .sum::<usize>(),
        1,
    );
    let screenshot_window = baseline.clone().into_messages().iter().map(estimate).sum();
    let max_input_tokens = baseline.estimated_tokens();
    let context = prepare(enclosing(), /*context_window*/ 10_000, "screenshot");
    let finalize = |context: crate::ComposedContext, max_input_tokens| {
        context
            .enforce_budget(
                RequestBudget {
                    max_input_tokens,
                    existing_context_tokens: 0,
                },
                "Evidence omitted.".into(),
                HistoryTruncation::Preserve,
            )
            .unwrap()
            .into_messages()
    };
    assert_eq!(
        finalize(context.clone(), max_input_tokens),
        baseline.into_messages(),
    );
    assert!(
        serde_json::to_string(&finalize(context, max_input_tokens + 1_000))
            .unwrap()
            .contains("Enclosing call for this review:")
    );

    // The image prompt can be full even when the aggregate budget has room.
    let screenshot =
        "x".repeat(10_000 - usize::try_from(screenshot_window).unwrap() + "screenshot".len());
    for (window, cap, screenshot) in [
        (20_000, 10_000, screenshot.as_str()),
        (screenshot_window, screenshot_window, "screenshot"),
    ] {
        let baseline = prepare(ordinary(), window, screenshot).into_messages();
        assert_eq!(baseline.iter().map(estimate).sum::<i64>(), cap);
        let context = prepare(enclosing(), window, screenshot);
        let roomy_budget = context.estimated_tokens() + 1_000;
        assert_eq!(finalize(context, roomy_budget), baseline);
    }
}
