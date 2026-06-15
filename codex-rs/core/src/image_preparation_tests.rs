use std::io::Cursor;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
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
fn preparation_preserves_small_image_bytes_and_non_data_urls() {
    let (data_url, original_bytes) = png_data_url(/*width*/ 64, /*height*/ 32);
    let http_url = "https://example.com/image.png".to_string();
    let mut items = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputImage {
                image_url: data_url,
                detail: Some(ImageDetail::High),
            },
            ContentItem::InputImage {
                image_url: http_url.clone(),
                detail: Some(ImageDetail::Low),
            },
        ],
        phase: None,
        metadata: None,
    }];

    prepare_response_items(&mut items);

    let ResponseItem::Message { content, .. } = &items[0] else {
        panic!("expected message");
    };
    let [
        ContentItem::InputImage { image_url, .. },
        ContentItem::InputImage {
            image_url: preserved_http_url,
            detail: Some(ImageDetail::Low),
        },
    ] = content.as_slice()
    else {
        panic!("expected two images");
    };
    assert_eq!(decoded_image(image_url).0, original_bytes);
    assert_eq!(preserved_http_url, &http_url);
}

#[test]
fn detail_policies_apply_the_expected_budgets() {
    for (detail, input_dimensions, expected_dimensions) in [
        (Some(ImageDetail::High), (2048, 2048), (1600, 1600)),
        (Some(ImageDetail::Original), (6401, 100), (6000, 94)),
        (Some(ImageDetail::Original), (3201, 3201), (3200, 3200)),
        (Some(ImageDetail::Auto), (2048, 2048), (1600, 1600)),
        (None, (2048, 2048), (1600, 1600)),
    ] {
        let (image_url, _) = png_data_url(input_dimensions.0, input_dimensions.1);
        let mut items = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputImage { image_url, detail }],
            phase: None,
            metadata: None,
        }];

        prepare_response_items(&mut items);

        let ResponseItem::Message { content, .. } = &items[0] else {
            panic!("expected message");
        };
        let [ContentItem::InputImage { image_url, .. }] = content.as_slice() else {
            panic!("expected image");
        };
        assert_eq!(decoded_image(image_url).1.dimensions(), expected_dimensions);
    }
}

#[test]
fn preparation_replaces_only_failed_tool_images_and_preserves_metadata() {
    let (valid_image_url, _) = png_data_url(/*width*/ 64, /*height*/ 32);
    let expected_valid_image_url = valid_image_url.clone();
    let mut items = vec![ResponseItem::CustomToolCallOutput {
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
        metadata: None,
    }];

    prepare_response_items(&mut items);

    assert_eq!(
        items,
        vec![ResponseItem::CustomToolCallOutput {
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
            metadata: None,
        }]
    );
}

#[test]
fn preparation_errors_use_bounded_actionable_placeholders() {
    let cases = [
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
