use std::io::Cursor;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::InternalChatMessageMetadataPassthrough;
use codex_utils_image::data_url_from_bytes;
use image::DynamicImage;
use image::GenericImageView;
use image::ImageBuffer;
use image::ImageFormat;
use image::Rgba;
use pretty_assertions::assert_eq;

use super::*;

fn png_data_url(width: u32, height: u32) -> (String, Vec<u8>) {
    let image = ImageBuffer::from_pixel(width, height, Rgba([10u8, 20, 30, 255]));
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut encoded, ImageFormat::Png)
        .expect("encode PNG");
    let bytes = encoded.into_inner();
    (data_url_from_bytes("image/png", &bytes), bytes)
}

fn decoded_image(image_url: &str) -> (Vec<u8>, DynamicImage) {
    let (_, payload) = image_url.split_once(',').expect("data URL payload");
    let bytes = BASE64_STANDARD.decode(payload).expect("decode image URL");
    let image = image::load_from_memory(&bytes).expect("decode processed image");
    (bytes, image)
}

#[test]
fn preparation_preserves_small_image_bytes_and_replaces_remote_urls() {
    let (data_url, original_bytes) = png_data_url(/*width*/ 64, /*height*/ 32);
    let mut items = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputImage {
                image_url: data_url,
                detail: Some(ImageDetail::High),
            },
            ContentItem::InputImage {
                image_url: "https://example.com/image.png".to_string(),
                detail: Some(ImageDetail::Low),
            },
        ],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }];
    items.push(ResponseItem::Message {
        id: None,
        role: "developer".to_string(),
        content: vec![ContentItem::InputImage {
            image_url: "https://example.com/developer-image.png".to_string(),
            detail: Some(ImageDetail::High),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    });

    prepare_response_items(
        &mut items,
        ImagePreparationMode::DetailBased,
        ImageResizeNoticeMode::Disabled,
    );

    let ResponseItem::Message { content, .. } = &items[0] else {
        panic!("expected message");
    };
    let [
        ContentItem::InputImage { image_url, .. },
        ContentItem::InputText { text },
    ] = content.as_slice()
    else {
        panic!("expected two images");
    };
    assert_eq!(decoded_image(image_url).0, original_bytes);
    assert_eq!(text, REMOTE_IMAGE_URL_PLACEHOLDER);
    assert_eq!(
        &items[1],
        &ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: REMOTE_IMAGE_URL_PLACEHOLDER.to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: Some(
                InternalChatMessageMetadataPassthrough {
                    content_item_kinds: Some(vec![ContentItemKind(
                        "images.preparation_error".to_string()
                    )]),
                    ..Default::default()
                }
            ),
        }
    );
}

#[test]
fn detail_policies_apply_the_expected_budgets() {
    for (detail, effective_detail, input_dimensions, expected_dimensions) in [
        (
            Some(ImageDetail::High),
            ImageDetailSetting::High,
            (2048, 2048),
            (1600, 1600),
        ),
        (
            Some(ImageDetail::Original),
            ImageDetailSetting::Original,
            (6401, 100),
            (6000, 94),
        ),
        (
            Some(ImageDetail::Original),
            ImageDetailSetting::Original,
            (3201, 3201),
            (3200, 3200),
        ),
        (
            Some(ImageDetail::Auto),
            ImageDetailSetting::High,
            (2048, 2048),
            (1600, 1600),
        ),
        (None, ImageDetailSetting::High, (2048, 2048), (1600, 1600)),
    ] {
        let (image_url, _) = png_data_url(input_dimensions.0, input_dimensions.1);
        let mut items = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputImage { image_url, detail }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }];

        let metadata = prepare_response_items(
            &mut items,
            ImagePreparationMode::DetailBased,
            ImageResizeNoticeMode::Disabled,
        );

        let ResponseItem::Message { content, .. } = &items[0] else {
            panic!("expected message");
        };
        let [ContentItem::InputImage { image_url, .. }] = content.as_slice() else {
            panic!("expected image");
        };
        assert_eq!(decoded_image(image_url).1.dimensions(), expected_dimensions);
        assert_eq!(
            metadata,
            vec![ImagePreparationMetadata {
                message_role: Some("user".to_string()),
                item_id: None,
                effective_detail,
                source_width: input_dimensions.0,
                source_height: input_dimensions.1,
                prepared_width: expected_dimensions.0,
                prepared_height: expected_dimensions.1,
            }]
        );
    }
}

#[test]
fn preparation_reports_tool_output_item_id() {
    let call_id = "call-image";
    let (image_url, _) = png_data_url(/*width*/ 64, /*height*/ 32);
    let mut items = vec![ResponseItem::FunctionCallOutput {
        id: None,
        call_id: Some(call_id.to_string()),
        name: None,
        namespace: None,
        output: FunctionCallOutputPayload::from_content_items(vec![
            FunctionCallOutputContentItem::InputImage {
                image_url,
                detail: Some(ImageDetail::High),
            },
        ]),
        internal_chat_message_metadata_passthrough: None,
    }];
    let metadata = prepare_response_items(
        &mut items,
        ImagePreparationMode::DetailBased,
        ImageResizeNoticeMode::Disabled,
    );

    assert_eq!(
        metadata,
        vec![ImagePreparationMetadata {
            message_role: None,
            item_id: Some(call_id.to_string()),
            effective_detail: ImageDetailSetting::High,
            source_width: 64,
            source_height: 32,
            prepared_width: 64,
            prepared_height: 32,
        }]
    );
}

#[test]
fn resize_notices_preserve_original_image_positions_and_skip_failed_images() {
    let (large_image_url, _) = png_data_url(/*width*/ 2048, /*height*/ 2048);
    let (small_image_url, _) = png_data_url(/*width*/ 64, /*height*/ 32);
    let mut items = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputImage {
                    image_url: small_image_url,
                    detail: Some(ImageDetail::High),
                },
                ContentItem::InputImage {
                    image_url: "data:image/png;base64,%%%".to_string(),
                    detail: Some(ImageDetail::High),
                },
                ContentItem::InputImage {
                    image_url: large_image_url.clone(),
                    detail: Some(ImageDetail::High),
                },
            ],
            phase: None,
            internal_chat_message_metadata_passthrough: Some(
                InternalChatMessageMetadataPassthrough {
                    content_item_kinds: Some(vec![
                        ContentItemKind("user.image".to_string()),
                        ContentItemKind("user.image".to_string()),
                        ContentItemKind("user.image".to_string()),
                    ]),
                    ..Default::default()
                },
            ),
        },
        ResponseItem::FunctionCallOutput {
            id: None,
            call_id: Some("call-image".to_string()),
            name: None,
            namespace: None,
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,%%%".to_string(),
                    detail: Some(ImageDetail::High),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: large_image_url,
                    detail: Some(ImageDetail::High),
                },
            ]),
            internal_chat_message_metadata_passthrough: None,
        },
    ];

    prepare_response_items(
        &mut items,
        ImagePreparationMode::DetailBased,
        ImageResizeNoticeMode::Enabled,
    );
    let expected_user_notice = concat!(
        "<image_resize_notice>\n",
        "Image 3 of 3 in the preceding user message was resized from 2048x2048 to 1600x1600 pixels.\n",
        "</image_resize_notice>"
    );

    let ResponseItem::Message {
        content,
        internal_chat_message_metadata_passthrough,
        ..
    } = &items[0]
    else {
        panic!("expected message");
    };
    assert_eq!(
        internal_chat_message_metadata_passthrough,
        &Some(InternalChatMessageMetadataPassthrough {
            content_item_kinds: Some(vec![
                ContentItemKind("user.image".to_string()),
                ContentItemKind("images.preparation_error".to_string()),
                ContentItemKind("user.image".to_string()),
            ]),
            ..Default::default()
        }),
    );
    let [
        ContentItem::InputImage {
            image_url: small_message_image_url,
            ..
        },
        ContentItem::InputText {
            text: failed_message_image,
        },
        ContentItem::InputImage {
            image_url: resized_message_image_url,
            ..
        },
    ] = content.as_slice()
    else {
        panic!("expected unchanged image, failed image placeholder, and resized image");
    };
    assert_eq!(
        decoded_image(small_message_image_url).1.dimensions(),
        (64, 32)
    );
    assert_eq!(failed_message_image, IMAGE_PROCESSING_ERROR_PLACEHOLDER);
    assert_eq!(
        decoded_image(resized_message_image_url).1.dimensions(),
        (1600, 1600)
    );

    assert_eq!(
        &items[1],
        &ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: expected_user_notice.to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: Some(
                InternalChatMessageMetadataPassthrough {
                    content_item_kinds: Some(vec![ContentItemKind(
                        "images.resize_notice".to_string()
                    )]),
                    ..Default::default()
                },
            ),
        }
    );

    let ResponseItem::FunctionCallOutput { output, .. } = &items[2] else {
        panic!("expected function call output");
    };
    let [
        FunctionCallOutputContentItem::InputText {
            text: failed_tool_image,
        },
        FunctionCallOutputContentItem::InputImage {
            image_url: resized_tool_image_url,
            ..
        },
    ] = output.content_items().expect("tool output content items")
    else {
        panic!("expected failed image placeholder and resized image in the tool output");
    };
    assert_eq!(failed_tool_image, IMAGE_PROCESSING_ERROR_PLACEHOLDER);
    assert_eq!(
        decoded_image(resized_tool_image_url).1.dimensions(),
        (1600, 1600)
    );
    assert_eq!(
        &items[3],
        &ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: concat!(
                    "<image_resize_notice>\n",
                    "Image 2 of 2 in the preceding tool output was resized from 2048x2048 to 1600x1600 pixels.\n",
                    "</image_resize_notice>"
                )
                .to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: Some(
                InternalChatMessageMetadataPassthrough {
                    content_item_kinds: Some(vec![ContentItemKind(
                        "images.resize_notice".to_string()
                    )]),
                    ..Default::default()
                },
            ),
        }
    );
}

#[test]
fn preparation_replaces_only_failed_tool_images_and_preserves_metadata() {
    let (valid_image_url, _) = png_data_url(/*width*/ 64, /*height*/ 32);
    let expected_valid_image_url = valid_image_url.clone();
    let mut items = vec![ResponseItem::CustomToolCallOutput {
        id: None,
        call_id: "call-1".to_string(),
        name: None,
        output: FunctionCallOutputPayload {
            body: FunctionCallOutputBody::ContentItems(vec![
                FunctionCallOutputContentItem::InputText {
                    text: "before".to_string(),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,%%%".to_string(),
                    detail: Some(ImageDetail::High),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: data_url_from_bytes("image/png", b"not an image"),
                    detail: Some(ImageDetail::High),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: valid_image_url.clone(),
                    detail: Some(ImageDetail::Low),
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url: valid_image_url,
                    detail: Some(ImageDetail::High),
                },
            ]),
            success: Some(true),
        },
        internal_chat_message_metadata_passthrough: None,
    }];

    prepare_response_items(
        &mut items,
        ImagePreparationMode::DetailBased,
        ImageResizeNoticeMode::Disabled,
    );

    assert_eq!(
        items,
        vec![ResponseItem::CustomToolCallOutput {
            id: None,
            call_id: "call-1".to_string(),
            name: None,
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::ContentItems(vec![
                    FunctionCallOutputContentItem::InputText {
                        text: "before".to_string(),
                    },
                    FunctionCallOutputContentItem::InputText {
                        text: IMAGE_PROCESSING_ERROR_PLACEHOLDER.to_string(),
                    },
                    FunctionCallOutputContentItem::InputText {
                        text: IMAGE_PROCESSING_ERROR_PLACEHOLDER.to_string(),
                    },
                    FunctionCallOutputContentItem::InputText {
                        text: UNSUPPORTED_LOW_DETAIL_PLACEHOLDER.to_string(),
                    },
                    FunctionCallOutputContentItem::InputImage {
                        image_url: expected_valid_image_url,
                        detail: Some(ImageDetail::High),
                    },
                ]),
                success: Some(true),
            },
            internal_chat_message_metadata_passthrough: None,
        }]
    );
}

#[test]
fn preparation_errors_use_bounded_actionable_placeholders() {
    let cases = [
        (
            ImagePreparationError::RemoteUrlUnsupported,
            REMOTE_IMAGE_URL_PLACEHOLDER,
        ),
        (
            ImagePreparationError::UnsupportedLowDetail,
            UNSUPPORTED_LOW_DETAIL_PLACEHOLDER,
        ),
        (
            ImagePreparationError::Processing(ImageProcessingError::ImageTooLarge {
                representation: "decoded input",
                size: 2,
                max: 1,
            }),
            IMAGE_TOO_LARGE_PLACEHOLDER,
        ),
        (
            ImagePreparationError::Processing(ImageProcessingError::InvalidDataUrl {
                reason: "details remain in logs".to_string(),
            }),
            IMAGE_PROCESSING_ERROR_PLACEHOLDER,
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(error.placeholder(), expected);
    }
}
