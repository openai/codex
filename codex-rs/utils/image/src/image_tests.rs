use std::io::Cursor;

use super::*;
use image::GenericImageView;
use image::ImageBuffer;
use image::ImageDecoder;
use image::Rgba;
use image::metadata::Orientation;

const TEST_RGB_ICC_PROFILE: &[u8] = b"0123456789abcdefRGB ";
const TEST_CMYK_ICC_PROFILE: &[u8] = b"0123456789abcdefCMYK";
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
    icc_profile: &[u8],
) -> Vec<u8> {
    let mut encoded = Vec::new();
    match format {
        ImageFormat::Png => {
            let mut encoder = PngEncoder::new(&mut encoded);
            encoder
                .set_icc_profile(icc_profile.to_vec())
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
                .set_icc_profile(icc_profile.to_vec())
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
                .set_icc_profile(icc_profile.to_vec())
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
        assert_eq!(*encoded.bytes, original_bytes);
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
async fn resizing_preserves_supported_metadata() {
    for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
        let image = ImageBuffer::from_pixel(2050, 2, Rgba([200u8, 10, 10, 255]));
        let original_bytes = image_bytes_with_metadata(&image, format, TEST_RGB_ICC_PROFILE);

        let processed = load_for_prompt_bytes(
            Path::new("in-memory-image"),
            original_bytes,
            PromptImageMode::ResizeToFit,
        )
        .expect("process image");

        assert_eq!((processed.width, processed.height), (2048, 2));

        let mut decoder = ImageReader::with_format(Cursor::new(processed.bytes.as_slice()), format)
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
                (2048, 2),
                Orientation::Rotate90,
                Some(TEST_RGB_ICC_PROFILE.to_vec()),
                Some(ROTATE_90_EXIF.to_vec()),
            )
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn resizing_drops_non_rgb_icc_profile() {
    let image = ImageBuffer::from_pixel(2050, 2, Rgba([200u8, 10, 10, 255]));
    let original_bytes =
        image_bytes_with_metadata(&image, ImageFormat::Jpeg, TEST_CMYK_ICC_PROFILE);

    let processed = load_for_prompt_bytes(
        Path::new("in-memory-image"),
        original_bytes,
        PromptImageMode::ResizeToFit,
    )
    .expect("process image");

    let mut decoder =
        ImageReader::with_format(Cursor::new(processed.bytes.as_slice()), ImageFormat::Jpeg)
            .into_decoder()
            .expect("create decoder");
    assert_eq!(
        (
            decoder.icc_profile().expect("read ICC profile"),
            decoder.exif_metadata().expect("read EXIF metadata"),
        ),
        (None, Some(ROTATE_90_EXIF.to_vec()))
    );
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
    assert_eq!(*processed.bytes, original_bytes);
}

#[tokio::test(flavor = "current_thread")]
async fn data_url_processing_preserves_supported_source_bytes() {
    let image = ImageBuffer::from_pixel(64, 32, Rgba([10u8, 20, 30, 255]));
    let original_bytes = image_bytes(&image, ImageFormat::Png);
    let image_url = data_url_from_bytes("image/png", &original_bytes)
        .replacen("data:", "DATA:", 1)
        .replacen(";base64,", ";BASE64,", 1);

    let processed = load_data_url_for_prompt_uncached(
        &image_url,
        PromptImageMode::ResizeToFit,
        PromptImageDecoder::Legacy,
    )
    .expect("process data URL image");

    assert_eq!(processed.width, 64);
    assert_eq!(processed.height, 32);
    assert_eq!(processed.mime, "image/png");
    assert_eq!(*processed.bytes, original_bytes);
}

#[tokio::test(flavor = "multi_thread")]
async fn data_url_processing_converts_gif_to_png() {
    let image = ImageBuffer::from_pixel(64, 32, Rgba([10u8, 20, 30, 255]));
    let gif_bytes = image_bytes(&image, ImageFormat::Gif);
    let image_url = data_url_from_bytes("image/gif", &gif_bytes);

    let processed = load_data_url_for_prompt(
        &image_url,
        PromptImageMode::ResizeToFit,
        PromptImageDecoder::Legacy,
    )
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
            load_data_url_for_prompt(
                image_url,
                PromptImageMode::ResizeToFit,
                PromptImageDecoder::Legacy,
            ),
            Err(ImageProcessingError::InvalidDataUrl { .. })
        ));
    }
}

/// The shared detail modes apply the policies used by external byte-oriented callers.
#[tokio::test(flavor = "multi_thread")]
async fn detail_modes_apply_expected_budgets() {
    for (mode, input_dimensions, expected_dimensions) in [
        (PromptImageMode::HIGH_DETAIL, (2048, 2048), (1600, 1600)),
        (PromptImageMode::ORIGINAL_DETAIL, (6401, 100), (6000, 94)),
    ] {
        let image = ImageBuffer::from_pixel(
            input_dimensions.0,
            input_dimensions.1,
            Rgba([200u8, 10, 10, 255]),
        );
        let original_bytes = image_bytes(&image, ImageFormat::Png);
        let processed = load_for_prompt_bytes(Path::new("in-memory-image"), original_bytes, mode)
            .expect("process image with detail mode");

        assert_eq!(
            (
                processed.source_width,
                processed.source_height,
                processed.width,
                processed.height,
            ),
            (
                input_dimensions.0,
                input_dimensions.1,
                expected_dimensions.0,
                expected_dimensions.1,
            )
        );
    }
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
async fn rejects_png_with_readable_dimensions_and_corrupt_pixels() {
    let image = ImageBuffer::from_pixel(100, 100, Rgba([10u8, 20, 30, 255]));
    let mut bytes = image_bytes(&image, ImageFormat::Png);
    load_for_prompt_bytes(
        Path::new("corrupt.png"),
        bytes.clone(),
        PromptImageMode::ORIGINAL_DETAIL,
    )
    .expect("warm the cache with valid pixels");
    let idat = bytes
        .windows(4)
        .position(|chunk_type| chunk_type == b"IDAT")
        .expect("PNG has an IDAT chunk");
    // Damage the compressed pixel data while keeping the PNG header intact.
    bytes[idat + 4] ^= 0xff;

    let dimensions = ImageReader::with_format(Cursor::new(&bytes), ImageFormat::Png)
        .into_dimensions()
        .expect("dimensions should still be readable");
    assert_eq!(dimensions, (100, 100));

    let error = load_for_prompt_bytes(
        Path::new("corrupt.png"),
        bytes,
        PromptImageMode::ORIGINAL_DETAIL,
    )
    .expect_err("even a PNG small enough to pass through must have decodable pixels");
    assert!(
        matches!(error, ImageProcessingError::Decode { .. }),
        "{error}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn reprocesses_updated_file_contents() {
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

#[tokio::test(flavor = "multi_thread")]
async fn legacy_cache_entry_cannot_bypass_sanitization() {
    let image = ImageBuffer::from_pixel(7, 11, Rgba([42, 64, 128, 255]));
    let bytes = image_bytes_with_metadata(&image, ImageFormat::Jpeg, TEST_RGB_ICC_PROFILE);
    let path = Path::new("oriented.jpg");
    let mode = PromptImageMode::Original;

    let legacy = load_for_prompt_bytes(path, bytes.clone(), mode).expect("load legacy image");
    let sanitized =
        load_for_prompt_bytes_with_decoder(path, bytes, mode, PromptImageDecoder::Sanitizer)
            .expect("sanitize image after warming the legacy cache");

    assert_eq!((legacy.width, legacy.height), (7, 11));
    assert_eq!((sanitized.width, sanitized.height), (11, 7));
}

#[tokio::test(flavor = "multi_thread")]
async fn bounds_cache_by_source_length_and_output_capacity() {
    let cache = ImageCache::new(NonZeroUsize::new(4).expect("non-zero cache capacity"));
    let key = |digest_byte| ImageCacheKey {
        digest: blake3::hash(&[digest_byte]),
        mode: PromptImageMode::Original,
        decoder: PromptImageDecoder::Legacy,
    };
    let image = |capacity| {
        let mut bytes = Vec::with_capacity(capacity);
        bytes.push(0);
        PreparedImage::Encoded(EncodedImage {
            bytes: bytes.into(),
            mime: "image/png".to_string(),
            source_width: 1,
            source_height: 1,
            width: 1,
            height: 1,
        })
    };

    let source = PreparedImage::Source {
        bytes: Weak::new(),
        format: ImageFormat::Png,
        width: 1,
        height: 1,
        encoded_len: 3,
    };
    cache_image(&cache, key(1), source, vec![0; 3], /*byte_capacity*/ 5);
    cache_image(&cache, key(2), image(3), vec![], /*byte_capacity*/ 5);
    cache_image(&cache, key(3), image(6), vec![], /*byte_capacity*/ 5);

    assert!(cache.get(&key(1)).is_none());
    assert!(cache.get(&key(2)).is_some());
    assert!(cache.get(&key(3)).is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_cache_misses_share_the_first_live_buffer() {
    let image = ImageBuffer::from_pixel(13, 9, Rgba([14, 50, 213, 255]));
    let source = image_bytes(&image, ImageFormat::Png);
    let key = ImageCacheKey {
        digest: blake3::hash(&source),
        mode: PromptImageMode::Original,
        decoder: PromptImageDecoder::Legacy,
    };
    let cache = ImageCache::new(NonZeroUsize::MIN);
    let entry = prepare_for_prompt(Path::new("source.png"), &source, key.mode, key.decoder)
        .expect("prepare PNG");

    // Both callers finished decoding before either inserted its result.
    let first = cache_image(
        &cache,
        key,
        entry.clone(),
        source.clone(),
        MAX_IMAGE_CACHE_BYTES,
    );
    let second = cache_image(&cache, key, entry, source, MAX_IMAGE_CACHE_BYTES);
    assert!(Arc::ptr_eq(&first.bytes, &second.bytes));
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_shares_live_source_bytes_but_only_retains_transformed_output() {
    let image = ImageBuffer::from_pixel(13, 9, Rgba([14, 50, 213, 255]));
    let source = image_bytes(&image, ImageFormat::Png);
    let path = Path::new("source.png");
    let load = |mode| load_for_prompt_bytes(path, source.clone(), mode).expect("prepare PNG");

    let original = load(PromptImageMode::Original);
    let weak = Arc::downgrade(&original.bytes);
    let overlap = load(PromptImageMode::Original);
    assert!(Arc::ptr_eq(&original.bytes, &overlap.bytes));
    drop((original, overlap));
    assert!(weak.upgrade().is_none());

    let mut input = source.clone();
    input.reserve(1);
    let input_ptr = input.as_ptr();
    let hit = load_for_prompt_bytes(path, input, PromptImageMode::Original).expect("reuse PNG");
    assert_eq!(hit.bytes.as_ptr(), input_ptr);
    assert_eq!(*hit.bytes, source);
    assert!(Arc::ptr_eq(
        &hit.bytes,
        &load(PromptImageMode::Original).bytes
    ));
    assert_eq!(hit.mime, "image/png");
    assert_eq!(
        (hit.source_width, hit.source_height, hit.width, hit.height),
        (13, 9, 13, 9)
    );

    let mode = PromptImageMode::ResizeWithLimits(PromptImageResizeLimits {
        max_dimension: 8,
        max_patches: 1,
    });
    let resized = load(mode);
    assert_ne!(*resized.bytes, source);
    let transformed = Arc::downgrade(&resized.bytes);
    drop(resized);
    assert!(Arc::ptr_eq(
        &transformed.upgrade().expect("cached output"),
        &load(mode).bytes
    ));
}
