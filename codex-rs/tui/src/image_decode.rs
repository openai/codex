//! Decode image pixels in their display orientation within the caller's allocation limits.

use image::DynamicImage;
use image::ImageDecoder;
use image::ImageReader;
use image::ImageResult;
use image::Limits;
use std::io::BufRead;
use std::io::Seek;

pub(crate) fn decode_oriented(
    mut reader: ImageReader<impl BufRead + Seek>,
    mut limits: Limits,
) -> ImageResult<DynamicImage> {
    reader.limits(limits.clone());
    let mut decoder = reader.into_decoder()?;
    // Match ImageReader::decode's allocation check before allocating the pixel buffer.
    limits.reserve(decoder.total_bytes())?;
    decoder.set_limits(limits)?;

    let orientation = decoder.orientation().ok();
    let mut image = DynamicImage::from_decoder(decoder)?;
    if let Some(orientation) = orientation {
        image.apply_orientation(orientation);
    }
    Ok(image)
}

#[cfg(test)]
#[path = "image_decode_tests.rs"]
mod tests;
