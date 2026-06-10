use std::fs::OpenOptions;
use std::io;
use std::io::Read;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::LazyLock;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_utils_cache::BlockingLruCache;
use codex_utils_cache::sha1_digest;
use image::ColorType;
use image::DynamicImage;
use image::GenericImageView;
use image::ImageEncoder;
use image::ImageFormat;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
/// Maximum width or height used when resizing images before uploading.
pub const MAX_DIMENSION: u32 = 2048;
/// Maximum compressed file size accepted for an image added to a prompt.
///
/// 50 MiB accommodates large source images while bounding the allocation made before decoding.
pub const MAX_PROMPT_IMAGE_FILE_BYTES: u64 = 50 * 1024 * 1024;

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
    ResizeToFit,
    Original,
}

fn validate_prompt_image_file_size(file_size: u64) -> io::Result<()> {
    if file_size > MAX_PROMPT_IMAGE_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("image exceeds the {MAX_PROMPT_IMAGE_FILE_BYTES}-byte limit"),
        ));
    }
    Ok(())
}

/// Reads a regular image file into memory, up to the prompt image size limit.
///
/// On Unix, the file is opened in nonblocking mode so a path resolving to a FIFO or other special
/// file cannot block before its type is checked. Metadata and bytes are read from the same handle.
pub fn read_prompt_image_file(path: &Path) -> io::Result<Vec<u8>> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NONBLOCK);

    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local image path is not a regular file",
        ));
    }
    validate_prompt_image_file_size(metadata.len())?;

    let capacity = usize::try_from(metadata.len()).unwrap_or_default();
    let mut file_bytes = Vec::with_capacity(capacity);
    file.take(MAX_PROMPT_IMAGE_FILE_BYTES + 1)
        .read_to_end(&mut file_bytes)?;
    let file_size = u64::try_from(file_bytes.len()).unwrap_or(u64::MAX);
    validate_prompt_image_file_size(file_size)?;
    Ok(file_bytes)
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
    let file_size = u64::try_from(file_bytes.len()).unwrap_or(u64::MAX);
    validate_prompt_image_file_size(file_size).map_err(|source| ImageProcessingError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let path_buf = path.to_path_buf();

    let key = ImageCacheKey {
        digest: sha1_digest(&file_bytes),
        mode,
    };

    IMAGE_CACHE.get_or_try_insert_with(key, move || {
        let format = match image::guess_format(&file_bytes) {
            Ok(ImageFormat::Png) => Some(ImageFormat::Png),
            Ok(ImageFormat::Jpeg) => Some(ImageFormat::Jpeg),
            Ok(ImageFormat::Gif) => Some(ImageFormat::Gif),
            Ok(ImageFormat::WebP) => Some(ImageFormat::WebP),
            _ => None,
        };

        let dynamic = image::load_from_memory(&file_bytes)
            .map_err(|source| ImageProcessingError::decode_error(&path_buf, source))?;

        let (width, height) = dynamic.dimensions();

        let encoded = if mode == PromptImageMode::Original
            || (width <= MAX_DIMENSION && height <= MAX_DIMENSION)
        {
            if let Some(format) = format.filter(|format| can_preserve_source_bytes(*format)) {
                let mime = format_to_mime(format);
                EncodedImage {
                    bytes: file_bytes,
                    mime,
                    width,
                    height,
                }
            } else {
                let (bytes, output_format) = encode_image(&dynamic, ImageFormat::Png)?;
                let mime = format_to_mime(output_format);
                EncodedImage {
                    bytes,
                    mime,
                    width,
                    height,
                }
            }
        } else {
            let resized = dynamic.resize(MAX_DIMENSION, MAX_DIMENSION, FilterType::Triangle);
            let target_format = format
                .filter(|format| can_preserve_source_bytes(*format))
                .unwrap_or(ImageFormat::Png);
            let (bytes, output_format) = encode_image(&resized, target_format)?;
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
) -> Result<(Vec<u8>, ImageFormat), ImageProcessingError> {
    let target_format = match preferred_format {
        ImageFormat::Jpeg => ImageFormat::Jpeg,
        ImageFormat::WebP => ImageFormat::WebP,
        _ => ImageFormat::Png,
    };

    let mut buffer = Vec::new();

    match target_format {
        ImageFormat::Png => {
            let rgba = image.to_rgba8();
            let encoder = PngEncoder::new(&mut buffer);
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
            encoder
                .encode_image(image)
                .map_err(|source| ImageProcessingError::Encode {
                    format: target_format,
                    source,
                })?;
        }
        ImageFormat::WebP => {
            let rgba = image.to_rgba8();
            let encoder = WebPEncoder::new_lossless(&mut buffer);
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

fn format_to_mime(format: ImageFormat) -> String {
    match format {
        ImageFormat::Jpeg => "image/jpeg".to_string(),
        ImageFormat::Gif => "image/gif".to_string(),
        ImageFormat::WebP => "image/webp".to_string(),
        _ => "image/png".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    #[cfg(unix)]
    use std::ffi::CString;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;
    #[cfg(unix)]
    use std::sync::mpsc;
    #[cfg(unix)]
    use std::time::Duration;

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

    #[test]
    fn prompt_image_file_size_limit_is_inclusive() {
        assert!(validate_prompt_image_file_size(MAX_PROMPT_IMAGE_FILE_BYTES).is_ok());

        let error = validate_prompt_image_file_size(MAX_PROMPT_IMAGE_FILE_BYTES + 1)
            .expect_err("reject oversized image");
        assert_eq!(
            (error.kind(), error.to_string()),
            (
                io::ErrorKind::InvalidData,
                format!("image exceeds the {MAX_PROMPT_IMAGE_FILE_BYTES}-byte limit"),
            )
        );
    }

    #[cfg(unix)]
    #[test]
    fn prompt_image_file_read_rejects_fifo_without_blocking() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let fifo_path = dir.path().join("image.png");
        let c_path = CString::new(fifo_path.as_os_str().as_bytes()).expect("path without nul");
        // SAFETY: `c_path` is NUL-terminated and remains valid for the duration of the call.
        let result = unsafe {
            libc::mkfifo(c_path.as_ptr(), /*mode*/ 0o600)
        };
        assert_eq!(result, 0);

        let (sender, receiver) = mpsc::channel();
        let read_thread = std::thread::spawn(move || {
            sender
                .send(read_prompt_image_file(&fifo_path))
                .expect("send read result");
        });
        let read_result = receiver
            .recv_timeout(Duration::from_secs(/*secs*/ 5))
            .expect("FIFO read should return without blocking");
        read_thread.join().expect("join read thread");

        let error = read_result.expect_err("reject FIFO");
        assert_eq!(
            (error.kind(), error.to_string()),
            (
                io::ErrorKind::InvalidInput,
                "local image path is not a regular file".to_string(),
            )
        );
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

            let loaded = image::load_from_memory(&processed.bytes)
                .expect("read resized bytes back into image");
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
    async fn fails_cleanly_for_invalid_images() {
        let err = load_for_prompt_bytes(
            Path::new("in-memory-image"),
            b"not an image".to_vec(),
            PromptImageMode::ResizeToFit,
        )
        .expect_err("invalid image should fail");
        assert!(matches!(
            err,
            ImageProcessingError::Decode { .. }
                | ImageProcessingError::UnsupportedImageFormat { .. }
        ));
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
}
