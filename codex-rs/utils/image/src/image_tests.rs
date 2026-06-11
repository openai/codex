use std::io::Cursor;

use super::*;
use image::GenericImageView;
use image::ImageBuffer;
use image::Rgba;

fn image_bytes(image: &ImageBuffer<Rgba<u8>, Vec<u8>>, format: ImageFormat) -> Vec<u8> {
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut encoded, format)
        .expect("encode image to bytes");
    encoded.into_inner()
}

#[tokio::test(flavor = "multi_thread")]
async fn returns_original_image_when_within_bounds() {
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
        let image = ImageBuffer::from_pixel(4096, 2048, Rgba([200u8, 10, 10, 255]));
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
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn downscales_tall_image_to_fit_square_bounds() {
    let image = ImageBuffer::from_pixel(1024, 4096, Rgba([200u8, 10, 10, 255]));
    let original_bytes = image_bytes(&image, ImageFormat::Png);

    let processed = load_for_prompt_bytes(
        Path::new("in-memory-image"),
        original_bytes,
        PromptImageMode::ResizeToFit,
    )
    .expect("process image");

    assert_eq!(processed.width, 512);
    assert_eq!(processed.height, MAX_DIMENSION);
    assert_eq!(processed.mime, "image/png");
}

#[tokio::test(flavor = "multi_thread")]
async fn preserves_large_image_in_original_mode() {
    let image = ImageBuffer::from_pixel(4096, 2048, Rgba([180u8, 30, 30, 255]));
    let original_bytes = image_bytes(&image, ImageFormat::Png);

    let processed = load_for_prompt_bytes(
        Path::new("in-memory-image"),
        original_bytes.clone(),
        PromptImageMode::Original,
    )
    .expect("process image");

    assert_eq!(processed.width, 4096);
    assert_eq!(processed.height, 2048);
    assert_eq!(processed.mime, "image/png");
    assert_eq!(processed.bytes, original_bytes);
}

#[tokio::test(flavor = "multi_thread")]
async fn data_url_processing_preserves_supported_source_bytes() {
    let image = ImageBuffer::from_pixel(64, 32, Rgba([10u8, 20, 30, 255]));
    let original_bytes = image_bytes(&image, ImageFormat::Png);
    let encoded = BASE64_STANDARD.encode(&original_bytes);
    let image_url = format!("data:image/png;base64,{encoded}")
        .replacen("data:", "DATA:", 1)
        .replacen(";base64,", ";BASE64,", 1);

    let processed = load_data_url_for_prompt(&image_url, PromptImageMode::ResizeToFit)
        .expect("process data URL image");

    assert_eq!(processed.width, 64);
    assert_eq!(processed.height, 32);
    assert_eq!(processed.mime, "image/png");
    assert_eq!(processed.bytes, original_bytes);
}

#[tokio::test(flavor = "multi_thread")]
async fn data_url_processing_converts_gif_to_png() {
    let image = ImageBuffer::from_pixel(64, 32, Rgba([10u8, 20, 30, 255]));
    let gif_bytes = image_bytes(&image, ImageFormat::Gif);
    let encoded = BASE64_STANDARD.encode(&gif_bytes);
    let image_url = format!("data:image/gif;base64,{encoded}");

    let processed = load_data_url_for_prompt(&image_url, PromptImageMode::ResizeToFit)
        .expect("process GIF data URL");

    assert_eq!(processed.mime, "image/png");
    assert_eq!(
        image::guess_format(&processed.bytes).expect("detect processed format"),
        ImageFormat::Png
    );
}

#[test]
fn data_url_processing_rejects_malformed_input() {
    for image_url in [
        "image/png;base64,AAAA",
        "data:image/png;base64",
        "data:image/png,AAAA",
        "data:image/png;base64,not base64",
    ] {
        assert!(matches!(
            load_data_url_for_prompt(image_url, PromptImageMode::ResizeToFit),
            Err(ImageProcessingError::InvalidDataUrl { .. })
        ));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn resize_with_limits_respects_dimension_and_patch_budgets() {
    let image = ImageBuffer::from_pixel(2048, 2048, Rgba([200u8, 10, 10, 255]));
    let original_bytes = image_bytes(&image, ImageFormat::Png);
    let limits = PromptImageResizeLimits {
        max_dimension: 2048,
        max_patches: 2_500,
    };

    let processed = load_for_prompt_bytes(
        Path::new("in-memory-image"),
        original_bytes,
        PromptImageMode::ResizeWithLimits(limits),
    )
    .expect("process image with explicit limits");

    assert_eq!((processed.width, processed.height), (1600, 1600));
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

#[tokio::test(flavor = "multi_thread")]
async fn reprocesses_updated_file_contents() {
    IMAGE_CACHE.clear();

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
