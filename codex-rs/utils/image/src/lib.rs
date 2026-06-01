use std::io::Cursor;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::LazyLock;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_utils_cache::BlockingLruCache;
use codex_utils_cache::sha1_digest;
use image::ColorType;
use image::DynamicImage;
use image::GenericImageView;
use image::ImageDecoder;
use image::ImageEncoder;
use image::ImageFormat;
use image::ImageReader;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
const DATA_URL_PREFIX: &str = "data:";
pub const PROMPT_IMAGE_PATCH_SIZE: u32 = 32;
pub const HIGH_DETAIL_MAX_DIMENSION: u32 = 2048;
pub const HIGH_DETAIL_MAX_PATCHES: usize = 2_500;
pub const ORIGINAL_DETAIL_MAX_DIMENSION: u32 = 6000;
pub const ORIGINAL_DETAIL_MAX_PATCHES: usize = 10_000;
/// Maximum width or height used when resizing high-detail images before uploading.
pub const MAX_DIMENSION: u32 = HIGH_DETAIL_MAX_DIMENSION;
/// Maximum accepted byte length for prompt image input representations.
///
/// This is a high sanity guard against pathological inputs, not a protocol
/// requirement or target upload size.
pub const MAX_PROMPT_IMAGE_INPUT_BYTES: usize = 1024 * 1024 * 1024;

pub mod error;

pub use crate::error::ImageProcessingError;

#[derive(Debug, Clone)]
pub struct EncodedImage {
    pub bytes: Vec<u8>,
    pub mime: String,
    pub width: u32,
    pub height: u32,
}

impl EncodedImage {
    pub fn into_data_url(self) -> String {
        let encoded = BASE64_STANDARD.encode(&self.bytes);
        format!("data:{};base64,{encoded}", self.mime)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptImageMode {
    /// Resize using the high-detail local upload budget.
    ResizeToFit,
    /// Preserve legacy original-detail behavior: validate the image, but do not resize locally.
    Original,
    /// Resize using Responses Lite's original-detail budget.
    ResponsesLiteOriginal,
}

impl PromptImageMode {
    fn resize_limits(self) -> Option<PromptImageResizeLimits> {
        match self {
            PromptImageMode::ResizeToFit => Some(PromptImageResizeLimits {
                max_dimension: HIGH_DETAIL_MAX_DIMENSION,
                max_patches: HIGH_DETAIL_MAX_PATCHES,
            }),
            PromptImageMode::Original => None,
            PromptImageMode::ResponsesLiteOriginal => Some(PromptImageResizeLimits {
                max_dimension: ORIGINAL_DETAIL_MAX_DIMENSION,
                max_patches: ORIGINAL_DETAIL_MAX_PATCHES,
            }),
        }
    }
}

#[derive(Clone, Copy)]
struct PromptImageResizeLimits {
    max_dimension: u32,
    max_patches: usize,
}

#[derive(Default)]
struct ImageMetadata {
    icc_profile: Option<Vec<u8>>,
    exif: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ImageCacheKey {
    digest: [u8; 20],
    mode: PromptImageMode,
}

static IMAGE_CACHE: LazyLock<BlockingLruCache<ImageCacheKey, EncodedImage>> =
    LazyLock::new(|| BlockingLruCache::new(NonZeroUsize::new(32).unwrap_or(NonZeroUsize::MIN)));

pub fn load_for_prompt_bytes(
    path: &Path,
    file_bytes: Vec<u8>,
    mode: PromptImageMode,
) -> Result<EncodedImage, ImageProcessingError> {
    ensure_prompt_image_input_size("decoded input", file_bytes.len())?;

    let path_buf = path.to_path_buf();

    let key = ImageCacheKey {
        digest: sha1_digest(&file_bytes),
        mode,
    };

    IMAGE_CACHE.get_or_try_insert_with(key, move || {
        let guessed_format = image::guess_format(&file_bytes)
            .map_err(|source| ImageProcessingError::decode_error(&path_buf, source))?;
        let format = match guessed_format {
            ImageFormat::Png => Some(ImageFormat::Png),
            ImageFormat::Jpeg => Some(ImageFormat::Jpeg),
            ImageFormat::Gif => Some(ImageFormat::Gif),
            ImageFormat::WebP => Some(ImageFormat::WebP),
            _ => None,
        };

        let mut decoder = ImageReader::with_format(Cursor::new(&file_bytes), guessed_format)
            .into_decoder()
            .map_err(|source| ImageProcessingError::decode_error(&path_buf, source))?;
        let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
        let metadata = ImageMetadata {
            icc_profile: decoder.icc_profile().ok().flatten(),
            exif: decoder.exif_metadata().ok().flatten().map(|mut exif| {
                if orientation != Orientation::NoTransforms {
                    let _ = Orientation::remove_from_exif_chunk(&mut exif);
                }
                exif
            }),
        };
        let mut dynamic = DynamicImage::from_decoder(decoder)
            .map_err(|source| ImageProcessingError::decode_error(&path_buf, source))?;
        dynamic.apply_orientation(orientation);

        let (width, height) = dynamic.dimensions();

        let (target_width, target_height) = match mode.resize_limits() {
            Some(limits) => prompt_image_output_dimensions_for_limits(width, height, limits),
            None => (width, height),
        };
        let encoded = if (target_width, target_height) == (width, height) {
            if let Some(format) = format.filter(|format| can_preserve_source_bytes(*format)) {
                let mime = format_to_mime(format);
                EncodedImage {
                    bytes: file_bytes,
                    mime,
                    width,
                    height,
                }
            } else {
                let (bytes, output_format) = encode_image(&dynamic, ImageFormat::Png, metadata)?;
                let mime = format_to_mime(output_format);
                EncodedImage {
                    bytes,
                    mime,
                    width,
                    height,
                }
            }
        } else {
            let resized = dynamic.resize_exact(target_width, target_height, FilterType::Triangle);
            let target_format = format
                .filter(|format| can_preserve_source_bytes(*format))
                .unwrap_or(ImageFormat::Png);
            let (bytes, output_format) = encode_image(&resized, target_format, metadata)?;
            let mime = format_to_mime(output_format);
            EncodedImage {
                bytes,
                mime,
                width: resized.width(),
                height: resized.height(),
            }
        };

        Ok(encoded)
    })
}

pub fn load_data_url_for_prompt(
    image_url: &str,
    mode: PromptImageMode,
) -> Result<EncodedImage, ImageProcessingError> {
    let rest =
        strip_data_url_prefix(image_url).ok_or_else(|| ImageProcessingError::InvalidDataUrl {
            reason: "missing data: prefix".to_string(),
        })?;
    let (metadata, encoded) =
        rest.split_once(',')
            .ok_or_else(|| ImageProcessingError::InvalidDataUrl {
                reason: "missing comma separator".to_string(),
            })?;
    let is_base64 = metadata
        .split(';')
        .any(|part| part.eq_ignore_ascii_case("base64"));
    if !is_base64 {
        return Err(ImageProcessingError::InvalidDataUrl {
            reason: "only base64 data URLs are supported".to_string(),
        });
    }

    ensure_prompt_image_input_size("base64 payload", encoded.len())?;

    let file_bytes =
        BASE64_STANDARD
            .decode(encoded)
            .map_err(|source| ImageProcessingError::InvalidDataUrl {
                reason: format!("invalid base64 payload: {source}"),
            })?;
    load_for_prompt_bytes(Path::new("<data-url-image>"), file_bytes, mode)
}

fn strip_data_url_prefix(image_url: &str) -> Option<&str> {
    image_url
        .get(..DATA_URL_PREFIX.len())
        .filter(|prefix| prefix.eq_ignore_ascii_case(DATA_URL_PREFIX))?;
    image_url.get(DATA_URL_PREFIX.len()..)
}

pub fn image_dimensions_from_base64_payload(
    payload: &str,
) -> Result<(u32, u32), ImageProcessingError> {
    ensure_prompt_image_input_size("base64 payload", payload.len())?;

    let file_bytes =
        BASE64_STANDARD
            .decode(payload)
            .map_err(|source| ImageProcessingError::InvalidDataUrl {
                reason: format!("invalid base64 payload: {source}"),
            })?;
    image_dimensions_for_prompt_bytes(Path::new("<data-url-image>"), &file_bytes)
}

pub fn image_dimensions_for_prompt_bytes(
    path: &Path,
    file_bytes: &[u8],
) -> Result<(u32, u32), ImageProcessingError> {
    ensure_prompt_image_input_size("decoded input", file_bytes.len())?;

    let format = image::guess_format(file_bytes)
        .map_err(|source| ImageProcessingError::decode_error(path, source))?;
    ImageReader::with_format(Cursor::new(file_bytes), format)
        .into_dimensions()
        .map_err(|source| ImageProcessingError::decode_error(path, source))
}

fn ensure_prompt_image_input_size(
    representation: &'static str,
    size: usize,
) -> Result<(), ImageProcessingError> {
    if size > MAX_PROMPT_IMAGE_INPUT_BYTES {
        return Err(ImageProcessingError::ImageTooLarge {
            representation,
            size,
            max: MAX_PROMPT_IMAGE_INPUT_BYTES,
        });
    }
    Ok(())
}

pub fn prompt_image_output_dimensions(
    width: u32,
    height: u32,
    mode: PromptImageMode,
) -> (u32, u32) {
    let Some(limits) = mode.resize_limits() else {
        return (width.max(1), height.max(1));
    };
    prompt_image_output_dimensions_for_limits(width, height, limits)
}

fn prompt_image_output_dimensions_for_limits(
    width: u32,
    height: u32,
    limits: PromptImageResizeLimits,
) -> (u32, u32) {
    let width = width.max(1);
    let height = height.max(1);
    if prompt_image_dimensions_fit(width, height, limits) {
        return (width, height);
    }
    prompt_image_resize_dimensions(width, height, limits)
}

fn prompt_image_dimensions_fit(width: u32, height: u32, limits: PromptImageResizeLimits) -> bool {
    width <= limits.max_dimension
        && height <= limits.max_dimension
        && prompt_image_patch_count(width, height) <= limits.max_patches
}

fn prompt_image_resize_dimensions(
    width: u32,
    height: u32,
    limits: PromptImageResizeLimits,
) -> (u32, u32) {
    let max_dimension_scale = f64::from(limits.max_dimension) / f64::from(width.max(height));
    let max_dimension_scale = max_dimension_scale.min(1.0);
    let width = ((f64::from(width) * max_dimension_scale).round() as u32).max(1);
    let height = ((f64::from(height) * max_dimension_scale).round() as u32).max(1);
    if prompt_image_dimensions_fit(width, height, limits) {
        return (width, height);
    }

    let patch_budget_scale = prompt_image_patch_budget_scale(width, height, limits.max_patches);
    prompt_image_dimensions_at_scale(width, height, patch_budget_scale)
}

fn prompt_image_patch_budget_scale(width: u32, height: u32, max_patches: usize) -> f64 {
    let width = f64::from(width);
    let height = f64::from(height);
    let patch_size = f64::from(PROMPT_IMAGE_PATCH_SIZE);
    let mut scale = (patch_size * patch_size * max_patches as f64 / width / height).sqrt();
    // Match the Responses/LPE patch-budget math: shrink by area, then round the
    // scaled patch grid down so ceil(width / patch_size) * ceil(height / patch_size)
    // stays within the budget after integer dimensions are chosen.
    let scaled_patches_wide = width * scale / patch_size;
    let scaled_patches_high = height * scale / patch_size;
    scale *= (scaled_patches_wide.floor() / scaled_patches_wide)
        .min(scaled_patches_high.floor() / scaled_patches_high);
    scale
}

fn prompt_image_dimensions_at_scale(width: u32, height: u32, scale: f64) -> (u32, u32) {
    let scaled_width = (f64::from(width) * scale).floor() as u32;
    let scaled_height = (f64::from(height) * scale).floor() as u32;
    (scaled_width.max(1), scaled_height.max(1))
}

pub fn prompt_image_patch_count(width: u32, height: u32) -> usize {
    let patches_wide = width.div_ceil(PROMPT_IMAGE_PATCH_SIZE);
    let patches_high = height.div_ceil(PROMPT_IMAGE_PATCH_SIZE);
    usize::try_from(u64::from(patches_wide) * u64::from(patches_high)).unwrap_or(usize::MAX)
}

fn can_preserve_source_bytes(format: ImageFormat) -> bool {
    // Public API docs explicitly call out non-animated GIF support only.
    // Preserve byte-for-byte only for formats we can safely pass through.
    matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    )
}

fn encode_image(
    image: &DynamicImage,
    preferred_format: ImageFormat,
    metadata: ImageMetadata,
) -> Result<(Vec<u8>, ImageFormat), ImageProcessingError> {
    let target_format = match preferred_format {
        ImageFormat::Jpeg => ImageFormat::Jpeg,
        ImageFormat::WebP => ImageFormat::WebP,
        _ => ImageFormat::Png,
    };

    let mut buffer = Vec::new();
    let ImageMetadata { icc_profile, exif } = metadata;

    match target_format {
        ImageFormat::Png => {
            let rgba = image.to_rgba8();
            let mut encoder = PngEncoder::new(&mut buffer);
            apply_image_metadata(&mut encoder, icc_profile, exif, target_format)?;
            encoder
                .write_image(
                    rgba.as_raw(),
                    image.width(),
                    image.height(),
                    ColorType::Rgba8.into(),
                )
                .map_err(|source| ImageProcessingError::Encode {
                    format: target_format,
                    source,
                })?;
        }
        ImageFormat::Jpeg => {
            let mut encoder = JpegEncoder::new_with_quality(&mut buffer, 85);
            apply_image_metadata(&mut encoder, icc_profile, exif, target_format)?;
            encoder
                .encode_image(image)
                .map_err(|source| ImageProcessingError::Encode {
                    format: target_format,
                    source,
                })?;
        }
        ImageFormat::WebP => {
            let rgba = image.to_rgba8();
            let mut encoder = WebPEncoder::new_lossless(&mut buffer);
            apply_image_metadata(&mut encoder, icc_profile, exif, target_format)?;
            encoder
                .write_image(
                    rgba.as_raw(),
                    image.width(),
                    image.height(),
                    ColorType::Rgba8.into(),
                )
                .map_err(|source| ImageProcessingError::Encode {
                    format: target_format,
                    source,
                })?;
        }
        _ => unreachable!("unsupported target_format should have been handled earlier"),
    }

    Ok((buffer, target_format))
}

fn apply_image_metadata(
    encoder: &mut impl ImageEncoder,
    icc_profile: Option<Vec<u8>>,
    exif: Option<Vec<u8>>,
    format: ImageFormat,
) -> Result<(), ImageProcessingError> {
    if let Some(icc_profile) = icc_profile {
        encoder
            .set_icc_profile(icc_profile)
            .map_err(|source| ImageProcessingError::Encode {
                format,
                source: image::ImageError::Unsupported(source),
            })?;
    }
    if let Some(exif) = exif {
        encoder
            .set_exif_metadata(exif)
            .map_err(|source| ImageProcessingError::Encode {
                format,
                source: image::ImageError::Unsupported(source),
            })?;
    }
    Ok(())
}

fn format_to_mime(format: ImageFormat) -> String {
    match format {
        ImageFormat::Jpeg => "image/jpeg".to_string(),
        ImageFormat::Gif => "image/gif".to_string(),
        ImageFormat::WebP => "image/webp".to_string(),
        _ => "image/png".to_string(),
    }
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;
