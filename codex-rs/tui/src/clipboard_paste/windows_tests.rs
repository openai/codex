#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn dibv5_preserves_pixels_and_respects_allocation_limits() {
    let mut header = [0; BMP_HEADER + V5_HEADER];
    let dib = &mut header[BMP_HEADER..];
    for (offset, value) in [
        (0, 124u32),
        (4, 1),               // width
        (8, 2),               // bottom-up rows
        (12, 1 | (32 << 16)), // planes and bit depth
        (16, 3),              // BI_BITFIELDS
        (20, 8),              // image size
        (40, 0x00ff0000),
        (44, 0x0000ff00),
        (48, 0x000000ff),
        (52, 0xff000000),
    ] {
        dib[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    let masks = dib[40..52].to_vec();
    assert!(matches!(
        decode_bitmap(header.to_vec(), image::Limits::default()),
        Some(Err(PasteImageError::ImageReadFailed(_)))
    ));
    let pixels = [3, 2, 1, 255, 6, 5, 4, 128];
    for data in [
        pixels.to_vec(),
        [&pixels[..], &[0; 12]].concat(),
        [&masks[..], &pixels].concat(),
    ] {
        let bmp = [&header[..], &data].concat();
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(7);
        assert!(matches!(
            decode_bitmap(bmp.clone(), limits),
            Some(Err(PasteImageError::ImageReadFailed(_)))
        ));
        let image = decode_bitmap(bmp, image::Limits::default())
            .unwrap()
            .unwrap();
        assert_eq!(image.into_rgba8().into_raw(), [4, 5, 6, 128, 1, 2, 3, 255]);
    }
}
