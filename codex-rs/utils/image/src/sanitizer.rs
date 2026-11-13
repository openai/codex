//! Decode supported prompt images and write fresh PNG or JPEG bytes without the source metadata.

use std::io::Cursor;
use std::path::Path;

use image::DynamicImage;
use image::ImageDecoder;
use image::ImageFormat;
use image::ImageReader;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::CompressionType;
use image::codecs::png::FilterType;
use image::codecs::png::PngEncoder;
use moxcms::ColorProfile;

use crate::EncodedImage;
use crate::ImageProcessingError;
use crate::MAX_SANITIZED_IMAGE_BYTES;
use crate::PromptImageMode;
use crate::resize_for_prompt;

// Original detail allows 10,000 patches of 32 × 32 pixels each.
const MAX_DECODED_PIXELS: u64 = 10_240_000;

pub(super) fn sanitize(
    path: &Path,
    bytes: &[u8],
    mode: PromptImageMode,
) -> Result<EncodedImage, ImageProcessingError> {
    if bytes.len() > MAX_SANITIZED_IMAGE_BYTES {
        return Err(ImageProcessingError::ImageTooLarge {
            representation: "input",
            size: bytes.len(),
            max: MAX_SANITIZED_IMAGE_BYTES,
        });
    }

    let decode_error = |source| ImageProcessingError::decode_error(path, source);
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| decode_error(error.into()))?;
    let (format, output_format) = match reader.format() {
        Some(format @ (ImageFormat::Png | ImageFormat::Gif)) => (format, ImageFormat::Png),
        Some(format @ (ImageFormat::Jpeg | ImageFormat::WebP)) => (format, ImageFormat::Jpeg),
        format => {
            return Err(ImageProcessingError::UnsupportedImageFormat {
                mime: format
                    .as_ref()
                    .map_or("unknown", ImageFormat::to_mime_type)
                    .to_string(),
            });
        }
    };

    let (mut img, orientation, icc_profile) = std::panic::catch_unwind(|| {
        let mut decoder = reader.into_decoder().map_err(decode_error)?;
        let (width, height) = decoder.dimensions();
        let pixels = u64::from(width) * u64::from(height);
        if pixels > MAX_DECODED_PIXELS {
            return Err(ImageProcessingError::ImageTooManyPixels {
                pixels,
                max: MAX_DECODED_PIXELS,
            });
        }
        let icc_profile = decoder.icc_profile().ok().flatten();
        // Read EXIF orientation only for JPEG.
        let orientation = if format == ImageFormat::Jpeg {
            decoder.orientation().ok()
        } else {
            None
        };
        let img = DynamicImage::from_decoder(decoder).map_err(decode_error)?;
        Ok((img, orientation, icc_profile))
    })
    .map_err(|_| {
        decode_error(image::ImageError::Decoding(
            image::error::DecodingError::new(
                format.into(),
                "image decoder rejected malformed data",
            ),
        ))
    })??;

    if let Some(orientation) = orientation {
        img.apply_orientation(orientation);
    }
    let (source_width, source_height) = (img.width(), img.height());
    if format == ImageFormat::WebP {
        // Drop WebP alpha before resizing because the JPEG output has no alpha channel.
        img = img.into_rgb8().into();
    }

    if let Some(resized) = resize_for_prompt(&img, mode) {
        img = resized;
    }

    if let Some(icc_profile) = icc_profile
        && let Some(new_img) = convert_color_profile(&img, &icc_profile)
    {
        img = new_img;
    }

    let mut output = Vec::new();
    let encoded = if output_format == ImageFormat::Png {
        img.write_with_encoder(PngEncoder::new_with_quality(
            &mut output,
            CompressionType::Fast,
            FilterType::Adaptive,
        ))
    } else {
        img.write_with_encoder(JpegEncoder::new_with_quality(
            &mut output,
            /*quality*/ 90,
        ))
    };
    encoded.map_err(|source| ImageProcessingError::Encode {
        format: output_format,
        source,
    })?;
    if output.len() > MAX_SANITIZED_IMAGE_BYTES {
        return Err(ImageProcessingError::ImageTooLarge {
            representation: "sanitized output",
            size: output.len(),
            max: MAX_SANITIZED_IMAGE_BYTES,
        });
    }

    Ok(EncodedImage {
        bytes: output.into(),
        mime: output_format.to_mime_type().to_string(),
        source_width,
        source_height,
        width: img.width(),
        height: img.height(),
    })
}

fn convert_color_profile(img: &DynamicImage, icc_profile: &[u8]) -> Option<DynamicImage> {
    // Convert color profiles only for 8-bit RGB and RGBA images.
    let layout = match img.color() {
        image::ColorType::Rgba8 => moxcms::Layout::Rgba,
        image::ColorType::Rgb8 => moxcms::Layout::Rgb,
        _ => return None,
    };

    let parsed_icc_profile = ColorProfile::new_from_slice(icc_profile).ok()?;
    let dest_profile = ColorProfile::new_srgb();

    let transform_opts = moxcms::TransformOptions {
        // Use floating point for more precise color conversion.
        prefer_fixed_point: false,
        ..Default::default()
    };

    let transform = parsed_icc_profile
        .create_transform_8bit(layout, &dest_profile, layout, transform_opts)
        .ok()?;

    let mut output = vec![0; img.as_bytes().len()];
    transform.transform(img.as_bytes(), &mut output).ok()?;
    let (width, height) = (img.width(), img.height());
    if layout == moxcms::Layout::Rgba {
        image::RgbaImage::from_raw(width, height, output).map(DynamicImage::ImageRgba8)
    } else {
        image::RgbImage::from_raw(width, height, output).map(DynamicImage::ImageRgb8)
    }
}
