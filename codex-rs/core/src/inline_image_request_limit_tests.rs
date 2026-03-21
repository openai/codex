use super::*;
use codex_protocol::models::FunctionCallOutputPayload;
use pretty_assertions::assert_eq;

#[test]
fn total_inline_image_request_bytes_counts_across_messages_and_tool_outputs() {
    let first = "data:image/png;base64,AAA".to_string();
    let second = "data:image/jpeg;base64,BBBB".to_string();
    let third = "data:image/gif;base64,CCCCC".to_string();
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputImage {
                image_url: first.clone(),
            }],
            end_turn: None,
            phase: None,
        },
        ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputImage {
                    image_url: second.clone(),
                    detail: Some(ImageDetail::Original),
                },
            ]),
        },
        ResponseItem::CustomToolCallOutput {
            call_id: "call-2".to_string(),
            name: None,
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputImage {
                    image_url: third.clone(),
                    detail: None,
                },
            ]),
        },
    ];

    assert_eq!(
        total_inline_image_request_bytes(&items),
        first.len() + second.len() + third.len()
    );
}

#[test]
fn total_inline_image_request_bytes_ignores_remote_and_file_backed_images() {
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputImage {
                image_url: "https://example.com/image.png".to_string(),
            }],
            end_turn: None,
            phase: None,
        },
        ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputImage {
                    image_url: "file:///tmp/image.png".to_string(),
                    detail: None,
                },
            ]),
        },
    ];

    assert_eq!(total_inline_image_request_bytes(&items), 0);
}

#[test]
fn total_inline_image_request_bytes_uses_utf8_byte_length() {
    let image_url = "data:image/svg+xml,<svg>😀</svg>".to_string();
    let items = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputImage {
            image_url: image_url.clone(),
        }],
        end_turn: None,
        phase: None,
    }];

    assert_eq!(total_inline_image_request_bytes(&items), image_url.len());
}

#[test]
fn total_image_request_count_counts_all_request_images() {
    let items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputImage {
                    image_url: "file:///tmp/first.png".to_string(),
                },
                ContentItem::InputImage {
                    image_url: "https://example.com/second.png".to_string(),
                },
            ],
            end_turn: None,
            phase: None,
        },
        ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,THIRD".to_string(),
                    detail: None,
                },
            ]),
        },
    ];

    assert_eq!(total_image_request_count(&items), 3);
}

#[test]
fn inline_image_request_limit_error_applies_to_user_images() {
    let mut model_info = crate::models_manager::model_info::model_info_from_slug("test-model");
    model_info.inline_image_request_limit_image_count = Some(1);
    let items = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputImage {
                image_url: "https://example.com/one.png".to_string(),
            },
            ContentItem::InputImage {
                image_url: "https://example.com/two.png".to_string(),
            },
        ],
        end_turn: None,
        phase: None,
    }];

    assert_eq!(
        inline_image_request_limit_error(&items, &model_info)
            .expect("count overflow should produce an error")
            .to_string(),
        "This request contains 2 images, which exceeds the 1 image limit for a single Responses API request. Use fewer images, smaller images, lower detail, or JPEG compression and try again."
    );
}
