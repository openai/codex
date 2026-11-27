#![cfg(test)]
//! Pixel-level checks for fitting previews into terminal cells.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn padding_keeps_fitted_pixels_at_their_original_scale() {
    let pixel = |x, y| image::Rgba([x as u8, y as u8, 42, 255]);
    let toolbar = RgbaImage::from_fn(/*width*/ 299, /*height*/ 37, pixel);
    let expected = RgbaImage::from_fn(/*width*/ 300, /*height*/ 52, |x, y| {
        *toolbar
            .get_pixel_checked(x, y)
            .unwrap_or(&image::Rgba([0; 4]))
    });
    assert_eq!(
        pad_to_cells(toolbar, /*cell_width*/ 12, /*cell_height*/ 26),
        expected
    );
}

#[test]
fn sixel_output_fits_wide_images_within_the_original_encoding_budget() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("image.png");
    let load = |columns| {
        TranscriptImage::load_for_cells(
            &path,
            columns,
            ImageProtocol::Sixel,
            PreviewKind::Generated,
            /*cell_size*/ Some((10, 20)),
        )
        .unwrap()
    };
    let source = RgbaImage::from_fn(
        /*width*/ 300,
        /*height*/ 37,
        |x, y| image::Rgba([x as u8, y as u8, 42, 255]),
    );
    source.save(&path).unwrap();
    let image = load(30);
    assert_eq!((image.columns, image.rows), (30, 2));
    assert_eq!(image.pixels, source);
    assert_eq!(load(20).pixels.dimensions(), (200, 25));

    for (width, height) in [(600, 74), (900, 42), (720, 320)] {
        RgbaImage::new(width, height).save(&path).unwrap();
        let (w, h) = load(48).pixels.dimensions();
        assert!(w * h <= 160 * 80);
        assert!(w * h.div_ceil(6) <= 160 * 14);
    }
}
