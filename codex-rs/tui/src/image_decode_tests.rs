#![cfg(test)]

use super::decode_oriented;
use image::DynamicImage;
use image::ImageEncoder;
use image::ImageFormat;
use image::ImageReader;
use image::Limits;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use pretty_assertions::assert_eq;
use std::io::Cursor;

fn write_fixture(mut encoder: impl ImageEncoder, orientation: Option<u8>) {
    if let Some(orientation) = orientation {
        // A little-endian EXIF directory with a single orientation entry.
        let mut exif = b"II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
        exif.extend_from_slice(&[orientation, 0, 0, 0, 0, 0, 0, 0]);
        encoder.set_exif_metadata(exif).unwrap();
    }
    encoder
        .write_image(
            &[0, 40, 80, 120, 160, 200],
            /*width*/ 3,
            /*height*/ 2,
            image::ExtendedColorType::L8,
        )
        .unwrap();
}

fn check_orientation(orientation: Option<u8>, display: fn(&DynamicImage) -> DynamicImage) {
    let mut png = Vec::new();
    write_fixture(PngEncoder::new(&mut png), orientation);
    let mut jpeg = Vec::new();
    write_fixture(JpegEncoder::new(&mut jpeg), orientation);

    for (data, format) in [(png, ImageFormat::Png), (jpeg, ImageFormat::Jpeg)] {
        let raw = image::load_from_memory_with_format(&data, format).unwrap();
        let actual = decode_oriented(
            ImageReader::with_format(Cursor::new(data), format),
            Limits::default(),
        )
        .unwrap();
        assert_eq!(
            actual.to_rgb8(),
            display(&raw).to_rgb8(),
            "{format:?} {orientation:?}"
        );
    }
}

#[test]
fn decodes_images_in_display_orientation() {
    check_orientation(/*orientation*/ None, DynamicImage::clone);
    check_orientation(Some(1), DynamicImage::clone);
    check_orientation(Some(2), DynamicImage::fliph);
    check_orientation(Some(3), DynamicImage::rotate180);
    check_orientation(Some(4), DynamicImage::flipv);
    check_orientation(Some(5), |image| image.rotate90().fliph());
    check_orientation(Some(6), DynamicImage::rotate90);
    check_orientation(Some(7), |image| image.rotate270().fliph());
    check_orientation(Some(8), DynamicImage::rotate270);
}

#[test]
fn rejects_images_larger_than_the_pixel_budget() {
    let mut jpeg = Vec::new();
    write_fixture(JpegEncoder::new(&mut jpeg), /*orientation*/ None);
    let reader = ImageReader::with_format(Cursor::new(jpeg), ImageFormat::Jpeg);
    let mut limits = Limits::default();
    limits.max_alloc = Some(5); // The fixture has six grayscale pixels.
    assert!(matches!(
        decode_oriented(reader, limits),
        Err(image::ImageError::Limits(_))
    ));
}
