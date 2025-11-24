//! A set of test utilities.
//!
//! There is some overlap between this module and `src/encoder.rs` module, but:
//!
//! * This module (unlike `src/encoder.rs`) performs no validation of the data being written - this
//!   allows building testcases that use arbitrary, potentially invalid PNGs as input.
//! * This module can be reused from `benches/decoder.rs` (a separate crate).

use byteorder::WriteBytesExt;
use std::io::Write;

/// Generates a store-only, non-compressed image:
///
/// * `00` compression mode (i.e.`BTYPE` = `00` = no compression) is used
/// * No filter is applied to the image rows
///
/// Currently the image always has the following properties:
///
/// * Single `IDAT` chunk
/// * Zlib chunks of maximum possible size
/// * 8-bit RGBA
///
/// These images are somewhat artificial, but may be useful for benchmarking performance of parts
/// outside of `fdeflate` crate and/or the `unfilter` function (e.g. these images were originally
/// used to evaluate changes to minimize copying of image pixels between various buffers - see
/// [this
/// discussion](https://github.com/image-rs/image-png/discussions/416#discussioncomment-7436871)
/// for more details).
pub fn write_noncompressed_png(w: &mut impl Write, size: u32, idat_bytes: usize) {
    write_png_sig(w);
    write_rgba8_ihdr_with_width(w, size);
    write_rgba8_idats(w, size, idat_bytes);
    write_iend(w);
}

/// Writes PNG signature.
/// See http://www.libpng.org/pub/png/spec/1.2/PNG-Structure.html#PNG-file-signature
pub fn write_png_sig(w: &mut impl Write) {
    const SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    w.write_all(&SIG).unwrap();
}

/// Writes an arbitrary PNG chunk.
pub fn write_chunk(w: &mut impl Write, chunk_type: &[u8], data: &[u8]) {
    assert_eq!(chunk_type.len(), 4);
    let crc = {
        let input = chunk_type
            .iter()
            .copied()
            .chain(data.iter().copied())
            .collect::<Vec<_>>();
        crc32fast::hash(input.as_slice())
    };
    w.write_u32::<byteorder::BigEndian>(data.len() as u32)
        .unwrap();
    w.write_all(chunk_type).unwrap();
    w.write_all(data).unwrap();
    w.write_u32::<byteorder::BigEndian>(crc).unwrap();
}

/// Writes an IHDR chunk that indicates a non-interlaced RGBA8 that uses the same height and
/// `width`.  See http://www.libpng.org/pub/png/spec/1.2/PNG-Chunks.html#C.IHDR
pub fn write_rgba8_ihdr_with_width(w: &mut impl Write, width: u32) {
    let mut data = Vec::new();
    data.write_u32::<byteorder::BigEndian>(width).unwrap();
    data.write_u32::<byteorder::BigEndian>(width).unwrap(); // height
    data.write_u8(8).unwrap(); // bit depth = always 8-bits per channel
    data.write_u8(6).unwrap(); // color type = color + alpha
    data.write_u8(0).unwrap(); // compression method (0 is the only allowed value)
    data.write_u8(0).unwrap(); // filter method (0 is the only allowed value)
    data.write_u8(0).unwrap(); // interlace method = no interlacing
    write_chunk(w, b"IHDR", &data);
}

/// Generates RGBA8 `width` x `height` image and wraps it in a store-only zlib container.
pub fn generate_rgba8_with_width_and_height(width: u32, height: u32) -> Vec<u8> {
    // Generate arbitrary test pixels.
    let image_pixels = {
        let mut row = Vec::new();
        row.write_u8(0).unwrap(); // filter = no filter

        let row_pixels = (0..width).flat_map(|i| {
            let color: u8 = (i * 255 / width) as u8;
            let alpha: u8 = 0xff;
            [color, 255 - color, color / 2, alpha]
        });
        row.extend(row_pixels);

        std::iter::repeat(row)
            .take(height as usize)
            .flatten()
            .collect::<Vec<_>>()
    };

    let mut zlib_data = Vec::new();
    let mut store_only_compressor =
        fdeflate::StoredOnlyCompressor::new(std::io::Cursor::new(&mut zlib_data)).unwrap();
    store_only_compressor.write_data(&image_pixels).unwrap();
    store_only_compressor.finish().unwrap();

    zlib_data
}

/// Writes an IDAT chunk.
pub fn write_rgba8_idats(w: &mut impl Write, size: u32, idat_bytes: usize) {
    let data = generate_rgba8_with_width_and_height(size, size);

    for chunk in data.chunks(idat_bytes) {
        write_chunk(w, b"IDAT", chunk);
    }
}

/// Writes an IEND chunk.
/// See http://www.libpng.org/pub/png/spec/1.2/PNG-Chunks.html#C.IEND
pub fn write_iend(w: &mut impl Write) {
    write_chunk(w, b"IEND", &[]);
}
