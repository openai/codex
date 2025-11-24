//!  Decoding of DXT (S3TC) compression
//!
//!  DXT is an image format that supports lossy compression
//!
//!  # Related Links
//!  * <https://www.khronos.org/registry/OpenGL/extensions/EXT/EXT_texture_compression_s3tc.txt> - Description of the DXT compression OpenGL extensions.
//!
//!  Note: this module only implements bare DXT encoding/decoding, it does not parse formats that can contain DXT files like .dds

use std::io::{self, Read};

use crate::color::ColorType;
use crate::error::{ImageError, ImageResult, ParameterError, ParameterErrorKind};
use crate::io::ReadExt;
use crate::ImageDecoder;

/// What version of DXT compression are we using?
/// Note that DXT2 and DXT4 are left away as they're
/// just DXT3 and DXT5 with premultiplied alpha
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DxtVariant {
    /// The DXT1 format. 48 bytes of RGB data in a 4x4 pixel square is
    /// compressed into an 8 byte block of DXT1 data
    DXT1,
    /// The DXT3 format. 64 bytes of RGBA data in a 4x4 pixel square is
    /// compressed into a 16 byte block of DXT3 data
    DXT3,
    /// The DXT5 format. 64 bytes of RGBA data in a 4x4 pixel square is
    /// compressed into a 16 byte block of DXT5 data
    DXT5,
}

impl DxtVariant {
    /// Returns the amount of bytes of raw image data
    /// that is encoded in a single DXTn block
    fn decoded_bytes_per_block(self) -> usize {
        match self {
            DxtVariant::DXT1 => 48,
            DxtVariant::DXT3 | DxtVariant::DXT5 => 64,
        }
    }

    /// Returns the amount of bytes per block of encoded DXTn data
    fn encoded_bytes_per_block(self) -> usize {
        match self {
            DxtVariant::DXT1 => 8,
            DxtVariant::DXT3 | DxtVariant::DXT5 => 16,
        }
    }

    /// Returns the color type that is stored in this DXT variant
    pub(crate) fn color_type(self) -> ColorType {
        match self {
            DxtVariant::DXT1 => ColorType::Rgb8,
            DxtVariant::DXT3 | DxtVariant::DXT5 => ColorType::Rgba8,
        }
    }
}

/// DXT decoder
pub(crate) struct DxtDecoder<R: Read> {
    inner: R,
    width_blocks: u32,
    height_blocks: u32,
    variant: DxtVariant,
    row: u32,
}

impl<R: Read> DxtDecoder<R> {
    /// Create a new DXT decoder that decodes from the stream ```r```.
    /// As DXT is often stored as raw buffers with the width/height
    /// somewhere else the width and height of the image need
    /// to be passed in ```width``` and ```height```, as well as the
    /// DXT variant in ```variant```.
    /// width and height are required to be powers of 2 and at least 4.
    /// otherwise an error will be returned
    pub(crate) fn new(
        r: R,
        width: u32,
        height: u32,
        variant: DxtVariant,
    ) -> Result<DxtDecoder<R>, ImageError> {
        if width % 4 != 0 || height % 4 != 0 {
            // TODO: this is actually a bit of a weird case. We could return `DecodingError` but
            // it's not really the format that is wrong However, the encoder should surely return
            // `EncodingError` so it would be the logical choice for symmetry.
            return Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::DimensionMismatch,
            )));
        }
        let width_blocks = width / 4;
        let height_blocks = height / 4;
        Ok(DxtDecoder {
            inner: r,
            width_blocks,
            height_blocks,
            variant,
            row: 0,
        })
    }

    fn scanline_bytes(&self) -> u64 {
        self.variant.decoded_bytes_per_block() as u64 * u64::from(self.width_blocks)
    }

    fn read_scanline(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        assert_eq!(
            u64::try_from(buf.len()),
            Ok(
                #[allow(deprecated)]
                self.scanline_bytes()
            )
        );

        let len = self.variant.encoded_bytes_per_block() * self.width_blocks as usize;
        let mut src = Vec::new();
        self.inner.read_exact_vec(&mut src, len)?;

        match self.variant {
            DxtVariant::DXT1 => decode_dxt1_row(&src, buf),
            DxtVariant::DXT3 => decode_dxt3_row(&src, buf),
            DxtVariant::DXT5 => decode_dxt5_row(&src, buf),
        }
        self.row += 1;
        Ok(buf.len())
    }
}

// Note that, due to the way that DXT compression works, a scanline is considered to consist out of
// 4 lines of pixels.
impl<R: Read> ImageDecoder for DxtDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        (self.width_blocks * 4, self.height_blocks * 4)
    }

    fn color_type(&self) -> ColorType {
        self.variant.color_type()
    }

    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));

        #[allow(deprecated)]
        for chunk in buf.chunks_mut(self.scanline_bytes().max(1) as usize) {
            self.read_scanline(chunk)?;
        }
        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

/**
 * Actual encoding/decoding logic below.
 */
type Rgb = [u8; 3];

/// decodes a 5-bit R, 6-bit G, 5-bit B 16-bit packed color value into 8-bit RGB
/// mapping is done so min/max range values are preserved. So for 5-bit
/// values 0x00 -> 0x00 and 0x1F -> 0xFF
fn enc565_decode(value: u16) -> Rgb {
    let red = (value >> 11) & 0x1F;
    let green = (value >> 5) & 0x3F;
    let blue = (value) & 0x1F;
    [
        (red * 0xFF / 0x1F) as u8,
        (green * 0xFF / 0x3F) as u8,
        (blue * 0xFF / 0x1F) as u8,
    ]
}

/*
 * Functions for decoding DXT compression
 */

/// Constructs the DXT5 alpha lookup table from the two alpha entries
/// if alpha0 > alpha1, constructs a table of [a0, a1, 6 linearly interpolated values from a0 to a1]
/// if alpha0 <= alpha1, constructs a table of [a0, a1, 4 linearly interpolated values from a0 to a1, 0, 0xFF]
fn alpha_table_dxt5(alpha0: u8, alpha1: u8) -> [u8; 8] {
    let mut table = [alpha0, alpha1, 0, 0, 0, 0, 0, 0xFF];
    if alpha0 > alpha1 {
        for i in 2..8u16 {
            table[i as usize] =
                (((8 - i) * u16::from(alpha0) + (i - 1) * u16::from(alpha1)) / 7) as u8;
        }
    } else {
        for i in 2..6u16 {
            table[i as usize] =
                (((6 - i) * u16::from(alpha0) + (i - 1) * u16::from(alpha1)) / 5) as u8;
        }
    }
    table
}

/// decodes an 8-byte dxt color block into the RGB channels of a 16xRGB or 16xRGBA block.
/// source should have a length of 8, dest a length of 48 (RGB) or 64 (RGBA)
fn decode_dxt_colors(source: &[u8], dest: &mut [u8], is_dxt1: bool) {
    // sanity checks, also enable the compiler to elide all following bound checks
    assert!(source.len() == 8 && (dest.len() == 48 || dest.len() == 64));
    // calculate pitch to store RGB values in dest (3 for RGB, 4 for RGBA)
    let pitch = dest.len() / 16;

    // extract color data
    let color0 = u16::from(source[0]) | (u16::from(source[1]) << 8);
    let color1 = u16::from(source[2]) | (u16::from(source[3]) << 8);
    let color_table = u32::from(source[4])
        | (u32::from(source[5]) << 8)
        | (u32::from(source[6]) << 16)
        | (u32::from(source[7]) << 24);
    // let color_table = source[4..8].iter().rev().fold(0, |t, &b| (t << 8) | b as u32);

    // decode the colors to rgb format
    let mut colors = [[0; 3]; 4];
    colors[0] = enc565_decode(color0);
    colors[1] = enc565_decode(color1);

    // determine color interpolation method
    if color0 > color1 || !is_dxt1 {
        // linearly interpolate the other two color table entries
        for i in 0..3 {
            colors[2][i] = ((u16::from(colors[0][i]) * 2 + u16::from(colors[1][i]) + 1) / 3) as u8;
            colors[3][i] = ((u16::from(colors[0][i]) + u16::from(colors[1][i]) * 2 + 1) / 3) as u8;
        }
    } else {
        // linearly interpolate one other entry, keep the other at 0
        for i in 0..3 {
            colors[2][i] = (u16::from(colors[0][i]) + u16::from(colors[1][i])).div_ceil(2) as u8;
        }
    }

    // serialize the result. Every color is determined by looking up
    // two bits in color_table which identify which color to actually pick from the 4 possible colors
    for i in 0..16 {
        dest[i * pitch..i * pitch + 3]
            .copy_from_slice(&colors[(color_table >> (i * 2)) as usize & 3]);
    }
}

/// Decodes a 16-byte bock of dxt5 data to a 16xRGBA block
fn decode_dxt5_block(source: &[u8], dest: &mut [u8]) {
    assert!(source.len() == 16 && dest.len() == 64);

    // extract alpha index table (stored as little endian 64-bit value)
    let alpha_table = source[2..8]
        .iter()
        .rev()
        .fold(0, |t, &b| (t << 8) | u64::from(b));

    // alpha level decode
    let alphas = alpha_table_dxt5(source[0], source[1]);

    // serialize alpha
    for i in 0..16 {
        dest[i * 4 + 3] = alphas[(alpha_table >> (i * 3)) as usize & 7];
    }

    // handle colors
    decode_dxt_colors(&source[8..16], dest, false);
}

/// Decodes a 16-byte bock of dxt3 data to a 16xRGBA block
fn decode_dxt3_block(source: &[u8], dest: &mut [u8]) {
    assert!(source.len() == 16 && dest.len() == 64);

    // extract alpha index table (stored as little endian 64-bit value)
    let alpha_table = source[0..8]
        .iter()
        .rev()
        .fold(0, |t, &b| (t << 8) | u64::from(b));

    // serialize alpha (stored as 4-bit values)
    for i in 0..16 {
        dest[i * 4 + 3] = ((alpha_table >> (i * 4)) as u8 & 0xF) * 0x11;
    }

    // handle colors
    decode_dxt_colors(&source[8..16], dest, false);
}

/// Decodes a 8-byte bock of dxt5 data to a 16xRGB block
fn decode_dxt1_block(source: &[u8], dest: &mut [u8]) {
    assert!(source.len() == 8 && dest.len() == 48);
    decode_dxt_colors(source, dest, true);
}

/// Decode a row of DXT1 data to four rows of RGB data.
/// `source.len()` should be a multiple of 8, otherwise this panics.
fn decode_dxt1_row(source: &[u8], dest: &mut [u8]) {
    assert!(source.len() % 8 == 0);
    let block_count = source.len() / 8;
    assert!(dest.len() >= block_count * 48);

    // contains the 16 decoded pixels per block
    let mut decoded_block = [0u8; 48];

    for (x, encoded_block) in source.chunks(8).enumerate() {
        decode_dxt1_block(encoded_block, &mut decoded_block);

        // copy the values from the decoded block to linewise RGB layout
        for line in 0..4 {
            let offset = (block_count * line + x) * 12;
            dest[offset..offset + 12].copy_from_slice(&decoded_block[line * 12..(line + 1) * 12]);
        }
    }
}

/// Decode a row of DXT3 data to four rows of RGBA data.
/// `source.len()` should be a multiple of 16, otherwise this panics.
fn decode_dxt3_row(source: &[u8], dest: &mut [u8]) {
    assert!(source.len() % 16 == 0);
    let block_count = source.len() / 16;
    assert!(dest.len() >= block_count * 64);

    // contains the 16 decoded pixels per block
    let mut decoded_block = [0u8; 64];

    for (x, encoded_block) in source.chunks(16).enumerate() {
        decode_dxt3_block(encoded_block, &mut decoded_block);

        // copy the values from the decoded block to linewise RGB layout
        for line in 0..4 {
            let offset = (block_count * line + x) * 16;
            dest[offset..offset + 16].copy_from_slice(&decoded_block[line * 16..(line + 1) * 16]);
        }
    }
}

/// Decode a row of DXT5 data to four rows of RGBA data.
/// `source.len()` should be a multiple of 16, otherwise this panics.
fn decode_dxt5_row(source: &[u8], dest: &mut [u8]) {
    assert!(source.len() % 16 == 0);
    let block_count = source.len() / 16;
    assert!(dest.len() >= block_count * 64);

    // contains the 16 decoded pixels per block
    let mut decoded_block = [0u8; 64];

    for (x, encoded_block) in source.chunks(16).enumerate() {
        decode_dxt5_block(encoded_block, &mut decoded_block);

        // copy the values from the decoded block to linewise RGB layout
        for line in 0..4 {
            let offset = (block_count * line + x) * 16;
            dest[offset..offset + 16].copy_from_slice(&decoded_block[line * 16..(line + 1) * 16]);
        }
    }
}
