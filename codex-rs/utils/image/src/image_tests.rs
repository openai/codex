use std::io::Cursor;

use super::*;
use image::GenericImageView;
use image::ImageBuffer;
use image::ImageDecoder;
use image::Rgba;
use image::metadata::Orientation;

const TEST_ICC_PROFILE: &[u8] = b"codex test icc profile";
const ROTATE_90_EXIF: &[u8] = &[
    0x49, 0x49, 0x2a, 0x00, 0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x12, 0x01, 0x03, 0x00, 0x01, 0x00,
    0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

fn image_bytes(image: &ImageBuffer<Rgba<u8>, Vec<u8>>, format: ImageFormat) -> Vec<u8> {
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut encoded, format)
        .expect("encode image to bytes");
    encoded.into_inner()
}

fn image_bytes_with_metadata(
    image: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    format: ImageFormat,
) -> Vec<u8> {
    let mut encoded = Vec::new();
    match format {
        ImageFormat::Png => {
            let mut encoder = PngEncoder::new(&mut encoded);
            encoder
                .set_icc_profile(TEST_ICC_PROFILE.to_vec())
                .expect("set PNG ICC profile");
            encoder
                .set_exif_metadata(ROTATE_90_EXIF.to_vec())
                .expect("set PNG EXIF metadata");
            encoder
                .write_image(
                    image.as_raw(),
                    image.width(),
                    image.height(),
                    ColorType::Rgba8.into(),
                )
                .expect("encode PNG with metadata");
        }
        ImageFormat::Jpeg => {
            let mut encoder = JpegEncoder::new_with_quality(&mut encoded, 90);
            encoder
                .set_icc_profile(TEST_ICC_PROFILE.to_vec())
                .expect("set JPEG ICC profile");
            encoder
                .set_exif_metadata(ROTATE_90_EXIF.to_vec())
                .expect("set JPEG EXIF metadata");
            encoder
                .encode_image(&DynamicImage::ImageRgba8(image.clone()))
                .expect("encode JPEG with metadata");
        }
        ImageFormat::WebP => {
            let mut encoder = WebPEncoder::new_lossless(&mut encoded);
            encoder
                .set_icc_profile(TEST_ICC_PROFILE.to_vec())
                .expect("set WebP ICC profile");
            encoder
                .set_exif_metadata(ROTATE_90_EXIF.to_vec())
                .expect("set WebP EXIF metadata");
            encoder
                .write_image(
                    image.as_raw(),
                    image.width(),
                    image.height(),
                    ColorType::Rgba8.into(),
                )
                .expect("encode WebP with metadata");
        }
        _ => panic!("unsupported test format"),
    }
    encoded
}

#[tokio::test(flavor = "multi_thread")]
async fn preserves_supported_source_bytes_when_within_bounds() {
    for (format, mime) in [
        (ImageFormat::Png, "image/png"),
        (ImageFormat::WebP, "image/webp"),
    ] {
        let image = ImageBuffer::from_pixel(64, 32, Rgba([10u8, 20, 30, 255]));
        let original_bytes = image_bytes(&image, format);

        let encoded = load_for_prompt_bytes(
            Path::new("in-memory-image"),
            original_bytes.clone(),
            PromptImageMode::ResizeToFit,
        )
        .expect("process image");

        assert_eq!(encoded.width, 64);
        assert_eq!(encoded.height, 32);
        assert_eq!(encoded.mime, mime);
        assert_eq!(encoded.bytes, original_bytes);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn downscales_large_image() {
    for (format, mime) in [
        (ImageFormat::Png, "image/png"),
        (ImageFormat::WebP, "image/webp"),
    ] {
        let image = ImageBuffer::from_pixel(2050, 1025, Rgba([200u8, 10, 10, 255]));
        let original_bytes = image_bytes(&image, format);

        let processed = load_for_prompt_bytes(
            Path::new("in-memory-image"),
            original_bytes,
            PromptImageMode::ResizeToFit,
        )
        .expect("process image");

        assert!(processed.width <= MAX_DIMENSION);
        assert!(processed.height <= MAX_DIMENSION);
        assert_eq!(processed.mime, mime);

        let detected_format =
            image::guess_format(&processed.bytes).expect("detect resized output format");
        assert_eq!(detected_format, format);

        let loaded =
            image::load_from_memory(&processed.bytes).expect("read resized bytes back into image");
        assert_eq!(loaded.dimensions(), (processed.width, processed.height));
        assert!(
            prompt_image_patch_count(processed.width, processed.height) <= HIGH_DETAIL_MAX_PATCHES
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn applies_calculated_dimensions_exactly() {
    let image = ImageBuffer::from_pixel(2050, 2, Rgba([200u8, 10, 10, 255]));
    let original_bytes = image_bytes(&image, ImageFormat::Png);

    let processed = load_for_prompt_bytes(
        Path::new("in-memory-image"),
        original_bytes,
        PromptImageMode::ResizeToFit,
    )
    .expect("process image");

    assert_eq!((processed.width, processed.height), (2048, 2));
}

#[tokio::test(flavor = "multi_thread")]
async fn resizing_applies_orientation_and_preserves_supported_metadata() {
    for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
        let image = ImageBuffer::from_pixel(2050, 2, Rgba([200u8, 10, 10, 255]));
        let original_bytes = image_bytes_with_metadata(&image, format);

        let processed = load_for_prompt_bytes(
            Path::new("in-memory-image"),
            original_bytes,
            PromptImageMode::ResizeToFit,
        )
        .expect("process image");

        assert_eq!((processed.width, processed.height), (2, 2048));

        let mut decoder = ImageReader::with_format(Cursor::new(&processed.bytes), format)
            .into_decoder()
            .expect("create decoder");
        assert_eq!(
            (
                decoder.dimensions(),
                decoder.orientation().expect("read orientation"),
                decoder.icc_profile().expect("read ICC profile"),
                decoder.exif_metadata().expect("read EXIF metadata"),
            ),
            (
                (2, 2048),
                Orientation::NoTransforms,
                Some(TEST_ICC_PROFILE.to_vec()),
                Some({
                    let mut exif = ROTATE_90_EXIF.to_vec();
                    let _ = Orientation::remove_from_exif_chunk(&mut exif);
                    exif
                }),
            )
        );
    }
}

#[test]
fn prompt_image_output_dimensions_respect_high_detail_limits() {
    assert_eq!(
        prompt_image_output_dimensions(
            /*width*/ 2048,
            /*height*/ 1024,
            PromptImageMode::ResizeToFit,
        ),
        (2048, 1024)
    );
    assert_eq!(
        prompt_image_output_dimensions(
            /*width*/ 4096,
            /*height*/ 2048,
            PromptImageMode::ResizeToFit,
        ),
        (2048, 1024)
    );
    assert_eq!(
        prompt_image_output_dimensions(
            /*width*/ 2048,
            /*height*/ 2048,
            PromptImageMode::ResizeToFit,
        ),
        (1600, 1600)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn load_data_url_for_prompt_accepts_case_insensitive_markers() {
    let image = ImageBuffer::from_pixel(64, 32, Rgba([10u8, 20, 30, 255]));
    let original_bytes = image_bytes(&image, ImageFormat::Png);
    let image_url = EncodedImage {
        bytes: original_bytes.clone(),
        mime: "image/png".to_string(),
        width: 64,
        height: 32,
    }
    .into_data_url()
    .replacen("data:", "DATA:", 1)
    .replacen(";base64,", ";BASE64,", 1);

    let processed = load_data_url_for_prompt(&image_url, PromptImageMode::Original)
        .expect("process data URL image");

    assert_eq!(processed.width, 64);
    assert_eq!(processed.height, 32);
    assert_eq!(processed.bytes, original_bytes);
}

#[tokio::test(flavor = "multi_thread")]
async fn second_pass_preserves_prepared_jpeg_bytes_when_within_bounds() {
    let image = ImageBuffer::from_pixel(1601, 1601, Rgba([200u8, 10, 10, 255]));
    let original_bytes = image_bytes(&image, ImageFormat::Jpeg);

    let first = load_for_prompt_bytes(
        Path::new("in-memory-image"),
        original_bytes,
        PromptImageMode::ResizeToFit,
    )
    .expect("process image");

    assert_eq!((first.width, first.height), (1600, 1600));
    assert_eq!(first.mime, "image/jpeg");

    let prepared_image_url = first.clone().into_data_url();
    let second = load_data_url_for_prompt(&prepared_image_url, PromptImageMode::ResizeToFit)
        .expect("process prepared data URL image");

    assert_eq!(second.width, first.width);
    assert_eq!(second.height, first.height);
    assert_eq!(second.mime, first.mime);
    assert_eq!(second.bytes, first.bytes);
}

#[test]
fn image_dimensions_from_base64_payload_reads_image_header() {
    let image = ImageBuffer::from_pixel(320, 240, Rgba([10u8, 20, 30, 255]));
    let original_bytes = image_bytes(&image, ImageFormat::Png);
    let payload = BASE64_STANDARD.encode(original_bytes);

    let dimensions = image_dimensions_from_base64_payload(&payload)
        .expect("read dimensions from base64 image payload");

    assert_eq!(dimensions, (320, 240));
}

#[tokio::test(flavor = "multi_thread")]
async fn downscales_tall_image_to_fit_square_bounds() {
    let image = ImageBuffer::from_pixel(512, 4096, Rgba([200u8, 10, 10, 255]));
    let original_bytes = image_bytes(&image, ImageFormat::Png);

    let processed = load_for_prompt_bytes(
        Path::new("in-memory-image"),
        original_bytes,
        PromptImageMode::ResizeToFit,
    )
    .expect("process image");

    assert_eq!(processed.width, 256);
    assert_eq!(processed.height, MAX_DIMENSION);
    assert_eq!(processed.mime, "image/png");
}

#[tokio::test(flavor = "multi_thread")]
async fn preserves_large_image_in_original_mode() {
    let image = ImageBuffer::from_pixel(6401, 100, Rgba([180u8, 30, 30, 255]));
    let original_bytes = image_bytes(&image, ImageFormat::Png);

    let processed = load_for_prompt_bytes(
        Path::new("in-memory-image"),
        original_bytes.clone(),
        PromptImageMode::Original,
    )
    .expect("process image");

    assert_eq!(processed.width, 6401);
    assert_eq!(processed.height, 100);
    assert_eq!(processed.mime, "image/png");
    assert_eq!(processed.bytes, original_bytes);
}

#[tokio::test(flavor = "multi_thread")]
async fn responses_lite_original_downscales_to_dimension_budget() {
    let image = ImageBuffer::from_pixel(6401, 100, Rgba([180u8, 30, 30, 255]));
    let original_bytes = image_bytes(&image, ImageFormat::Png);

    let processed = load_for_prompt_bytes(
        Path::new("in-memory-image"),
        original_bytes,
        PromptImageMode::ResponsesLiteOriginal,
    )
    .expect("process image");

    assert!(processed.width < 6401);
    assert!(processed.width <= ORIGINAL_DETAIL_MAX_DIMENSION);
    assert!(processed.height <= ORIGINAL_DETAIL_MAX_DIMENSION);
    assert!(
        prompt_image_patch_count(processed.width, processed.height) <= ORIGINAL_DETAIL_MAX_PATCHES
    );
}

#[test]
fn prompt_image_output_dimensions_respect_responses_lite_original_limits() {
    assert_eq!(
        prompt_image_output_dimensions(
            /*width*/ 6401,
            /*height*/ 100,
            PromptImageMode::ResponsesLiteOriginal,
        ),
        (6000, 94)
    );
    assert_eq!(
        prompt_image_output_dimensions(
            /*width*/ 3201,
            /*height*/ 3201,
            PromptImageMode::ResponsesLiteOriginal,
        ),
        (3200, 3200)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fails_cleanly_for_invalid_images() {
    let err = load_for_prompt_bytes(
        Path::new("in-memory-image"),
        b"not an image".to_vec(),
        PromptImageMode::ResizeToFit,
    )
    .expect_err("invalid image should fail");
    assert!(matches!(
        err,
        ImageProcessingError::Decode { .. } | ImageProcessingError::UnsupportedImageFormat { .. }
    ));
}

#[test]
fn prompt_image_input_size_limit_is_inclusive_for_each_representation() {
    let size = MAX_PROMPT_IMAGE_INPUT_BYTES + 1;
    for representation in ["base64 payload", "decoded input"] {
        ensure_prompt_image_input_size(representation, MAX_PROMPT_IMAGE_INPUT_BYTES)
            .expect("input at the limit should be accepted");
        let err = ensure_prompt_image_input_size(representation, size)
            .expect_err("input over the limit should fail");

        assert!(matches!(
            err,
            ImageProcessingError::ImageTooLarge {
                representation: got_representation,
                size: got_size,
                max: MAX_PROMPT_IMAGE_INPUT_BYTES,
            } if got_representation == representation && got_size == size
        ));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn reprocesses_updated_file_contents() {
    {
        IMAGE_CACHE.clear();
    }

    let first_image = ImageBuffer::from_pixel(32, 16, Rgba([20u8, 120, 220, 255]));
    let first_bytes = image_bytes(&first_image, ImageFormat::Png);

    let first = load_for_prompt_bytes(
        Path::new("in-memory-image"),
        first_bytes,
        PromptImageMode::ResizeToFit,
    )
    .expect("process first image");

    let second_image = ImageBuffer::from_pixel(96, 48, Rgba([50u8, 60, 70, 255]));
    let second_bytes = image_bytes(&second_image, ImageFormat::Png);

    let second = load_for_prompt_bytes(
        Path::new("in-memory-image"),
        second_bytes,
        PromptImageMode::ResizeToFit,
    )
    .expect("process updated image");

    assert_eq!(first.width, 32);
    assert_eq!(first.height, 16);
    assert_eq!(second.width, 96);
    assert_eq!(second.height, 48);
    assert_ne!(second.bytes, first.bytes);
}
