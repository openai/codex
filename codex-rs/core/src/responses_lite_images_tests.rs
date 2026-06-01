use super::*;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use pretty_assertions::assert_eq;

const TINY_PNG_BYTES: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2, 0, 0, 5, 0, 1,
    122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn tiny_png_data_url() -> String {
    format!(
        "data:image/png;base64,{}",
        BASE64_STANDARD.encode(TINY_PNG_BYTES)
    )
}

fn assert_message_image_is_replaced(image_url: String, detail: Option<ImageDetail>) {
    let mut items = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputImage { image_url, detail }],
        phase: None,
    }];

    prepare_response_items_for_responses_lite(&mut items);

    assert_eq!(
        items,
        vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: IMAGE_PROCESSING_ERROR_PLACEHOLDER.to_string(),
            }],
            phase: None,
        }]
    );
}

#[test]
fn responses_lite_preparation_strips_detail_from_message_images() {
    let mut items = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputImage {
            image_url: tiny_png_data_url(),
            detail: Some(ImageDetail::High),
        }],
        phase: None,
    }];

    prepare_response_items_for_responses_lite(&mut items);

    let ResponseItem::Message { content, .. } = &items[0] else {
        panic!("expected message item");
    };
    let [ContentItem::InputImage { image_url, detail }] = content.as_slice() else {
        panic!("expected one input image");
    };
    assert!(image_url.starts_with("data:image/png;base64,"));
    assert_eq!(*detail, None);
}

#[test]
fn responses_lite_detail_modes_match_responses_defaults() {
    assert_eq!(
        prompt_image_mode_for_responses_lite_detail(/*detail*/ None)
            .expect("missing detail should default to original"),
        PromptImageMode::ResponsesLiteOriginal
    );
    assert_eq!(
        prompt_image_mode_for_responses_lite_detail(Some(ImageDetail::Auto))
            .expect("auto detail should use original"),
        PromptImageMode::ResponsesLiteOriginal
    );
    assert_eq!(
        prompt_image_mode_for_responses_lite_detail(Some(ImageDetail::High))
            .expect("high detail should use high"),
        PromptImageMode::ResizeToFit
    );
    assert_eq!(
        prompt_image_mode_for_responses_lite_detail(Some(ImageDetail::Original))
            .expect("original detail should use original"),
        PromptImageMode::ResponsesLiteOriginal
    );
}

#[test]
fn responses_lite_preparation_replaces_only_failed_tool_images() {
    let valid_image_url = tiny_png_data_url();
    let mut items = vec![ResponseItem::FunctionCallOutput {
        call_id: "call-1".to_string(),
        output: FunctionCallOutputPayload {
            body: codex_protocol::models::FunctionCallOutputBody::ContentItems(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "before".to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "https://example.com/image.png".to_string(),
                    detail: Some(ImageDetail::Original),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: valid_image_url.clone(),
                    detail: Some(ImageDetail::High),
                },
            ]),
            success: Some(true),
        },
    }];

    prepare_response_items_for_responses_lite(&mut items);

    assert_eq!(
        items,
        vec![ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload {
                body: codex_protocol::models::FunctionCallOutputBody::ContentItems(vec![
                    FunctionCallOutputContentItem::InputText {
                        text: "before".to_string(),
                    },
                    FunctionCallOutputContentItem::InputText {
                        text: IMAGE_PROCESSING_ERROR_PLACEHOLDER.to_string(),
                    },
                    FunctionCallOutputContentItem::InputImage {
                        image_url: valid_image_url,
                        detail: None,
                    },
                ]),
                success: Some(true),
            },
        }]
    );
}

#[test]
fn responses_lite_preparation_replaces_low_detail_images() {
    assert_message_image_is_replaced(tiny_png_data_url(), Some(ImageDetail::Low));
}

#[test]
fn responses_lite_preparation_replaces_invalid_data_urls() {
    for image_url in [
        "data:image/png;base64,%%%".to_string(),
        format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD.encode("not an image")
        ),
    ] {
        assert_message_image_is_replaced(image_url, Some(ImageDetail::High));
    }
}
