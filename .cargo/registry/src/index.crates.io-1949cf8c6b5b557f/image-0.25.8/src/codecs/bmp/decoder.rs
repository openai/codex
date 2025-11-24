use crate::utils::vec_try_with_capacity;
use std::cmp::{self, Ordering};
use std::io::{self, BufRead, Seek, SeekFrom};
use std::iter::{repeat, Rev};
use std::slice::ChunksExactMut;
use std::{error, fmt};

use byteorder_lite::{LittleEndian, ReadBytesExt};

use crate::color::ColorType;
use crate::error::{
    DecodingError, ImageError, ImageResult, UnsupportedError, UnsupportedErrorKind,
};
use crate::io::free_functions::load_rect;
use crate::io::ReadExt;
use crate::{ImageDecoder, ImageDecoderRect, ImageFormat};

const BITMAPCOREHEADER_SIZE: u32 = 12;
const BITMAPINFOHEADER_SIZE: u32 = 40;
const BITMAPV2HEADER_SIZE: u32 = 52;
const BITMAPV3HEADER_SIZE: u32 = 56;
const BITMAPV4HEADER_SIZE: u32 = 108;
const BITMAPV5HEADER_SIZE: u32 = 124;

static LOOKUP_TABLE_3_BIT_TO_8_BIT: [u8; 8] = [0, 36, 73, 109, 146, 182, 219, 255];
static LOOKUP_TABLE_4_BIT_TO_8_BIT: [u8; 16] = [
    0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255,
];
static LOOKUP_TABLE_5_BIT_TO_8_BIT: [u8; 32] = [
    0, 8, 16, 25, 33, 41, 49, 58, 66, 74, 82, 90, 99, 107, 115, 123, 132, 140, 148, 156, 165, 173,
    181, 189, 197, 206, 214, 222, 230, 239, 247, 255,
];
static LOOKUP_TABLE_6_BIT_TO_8_BIT: [u8; 64] = [
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 45, 49, 53, 57, 61, 65, 69, 73, 77, 81, 85, 89, 93,
    97, 101, 105, 109, 113, 117, 121, 125, 130, 134, 138, 142, 146, 150, 154, 158, 162, 166, 170,
    174, 178, 182, 186, 190, 194, 198, 202, 206, 210, 215, 219, 223, 227, 231, 235, 239, 243, 247,
    251, 255,
];

static R5_G5_B5_COLOR_MASK: Bitfields = Bitfields {
    r: Bitfield { len: 5, shift: 10 },
    g: Bitfield { len: 5, shift: 5 },
    b: Bitfield { len: 5, shift: 0 },
    a: Bitfield { len: 0, shift: 0 },
};
const R8_G8_B8_COLOR_MASK: Bitfields = Bitfields {
    r: Bitfield { len: 8, shift: 24 },
    g: Bitfield { len: 8, shift: 16 },
    b: Bitfield { len: 8, shift: 8 },
    a: Bitfield { len: 0, shift: 0 },
};
const R8_G8_B8_A8_COLOR_MASK: Bitfields = Bitfields {
    r: Bitfield { len: 8, shift: 16 },
    g: Bitfield { len: 8, shift: 8 },
    b: Bitfield { len: 8, shift: 0 },
    a: Bitfield { len: 8, shift: 24 },
};

const RLE_ESCAPE: u8 = 0;
const RLE_ESCAPE_EOL: u8 = 0;
const RLE_ESCAPE_EOF: u8 = 1;
const RLE_ESCAPE_DELTA: u8 = 2;

/// The maximum width/height the decoder will process.
const MAX_WIDTH_HEIGHT: i32 = 0xFFFF;

#[derive(PartialEq, Copy, Clone)]
enum ImageType {
    Palette,
    RGB16,
    RGB24,
    RGB32,
    RGBA32,
    RLE8,
    RLE4,
    Bitfields16,
    Bitfields32,
}

#[derive(PartialEq)]
enum BMPHeaderType {
    Core,
    Info,
    V2,
    V3,
    V4,
    V5,
}

#[derive(PartialEq)]
enum FormatFullBytes {
    RGB24,
    RGB32,
    RGBA32,
    Format888,
}

enum Chunker<'a> {
    FromTop(ChunksExactMut<'a, u8>),
    FromBottom(Rev<ChunksExactMut<'a, u8>>),
}

pub(crate) struct RowIterator<'a> {
    chunks: Chunker<'a>,
}

impl<'a> Iterator for RowIterator<'a> {
    type Item = &'a mut [u8];

    #[inline(always)]
    fn next(&mut self) -> Option<&'a mut [u8]> {
        match self.chunks {
            Chunker::FromTop(ref mut chunks) => chunks.next(),
            Chunker::FromBottom(ref mut chunks) => chunks.next(),
        }
    }
}

/// All errors that can occur when attempting to parse a BMP
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum DecoderError {
    // Failed to decompress RLE data.
    CorruptRleData,

    /// The bitfield mask interleaves set and unset bits
    BitfieldMaskNonContiguous,
    /// Bitfield mask invalid (e.g. too long for specified type)
    BitfieldMaskInvalid,
    /// Bitfield (of the specified width – 16- or 32-bit) mask not present
    BitfieldMaskMissing(u32),
    /// Bitfield (of the specified width – 16- or 32-bit) masks not present
    BitfieldMasksMissing(u32),

    /// BMP's "BM" signature wrong or missing
    BmpSignatureInvalid,
    /// More than the exactly one allowed plane specified by the format
    MoreThanOnePlane,
    /// Invalid amount of bits per channel for the specified image type
    InvalidChannelWidth(ChannelWidthError, u16),

    /// The width is negative
    NegativeWidth(i32),
    /// One of the dimensions is larger than a soft limit
    ImageTooLarge(i32, i32),
    /// The height is `i32::min_value()`
    ///
    /// General negative heights specify top-down DIBs
    InvalidHeight,

    /// Specified image type is invalid for top-down BMPs (i.e. is compressed)
    ImageTypeInvalidForTopDown(u32),
    /// Image type not currently recognized by the decoder
    ImageTypeUnknown(u32),

    /// Bitmap header smaller than the core header
    HeaderTooSmall(u32),

    /// The palette is bigger than allowed by the bit count of the BMP
    PaletteSizeExceeded {
        colors_used: u32,
        bit_count: u16,
    },
}

impl fmt::Display for DecoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecoderError::CorruptRleData => f.write_str("Corrupt RLE data"),
            DecoderError::BitfieldMaskNonContiguous => f.write_str("Non-contiguous bitfield mask"),
            DecoderError::BitfieldMaskInvalid => f.write_str("Invalid bitfield mask"),
            DecoderError::BitfieldMaskMissing(bb) => {
                f.write_fmt(format_args!("Missing {bb}-bit bitfield mask"))
            }
            DecoderError::BitfieldMasksMissing(bb) => {
                f.write_fmt(format_args!("Missing {bb}-bit bitfield masks"))
            }
            DecoderError::BmpSignatureInvalid => f.write_str("BMP signature not found"),
            DecoderError::MoreThanOnePlane => f.write_str("More than one plane"),
            DecoderError::InvalidChannelWidth(tp, n) => {
                f.write_fmt(format_args!("Invalid channel bit count for {tp}: {n}"))
            }
            DecoderError::NegativeWidth(w) => f.write_fmt(format_args!("Negative width ({w})")),
            DecoderError::ImageTooLarge(w, h) => f.write_fmt(format_args!(
                "Image too large (one of ({w}, {h}) > soft limit of {MAX_WIDTH_HEIGHT})"
            )),
            DecoderError::InvalidHeight => f.write_str("Invalid height"),
            DecoderError::ImageTypeInvalidForTopDown(tp) => f.write_fmt(format_args!(
                "Invalid image type {tp} for top-down image."
            )),
            DecoderError::ImageTypeUnknown(tp) => {
                f.write_fmt(format_args!("Unknown image compression type {tp}"))
            }
            DecoderError::HeaderTooSmall(s) => {
                f.write_fmt(format_args!("Bitmap header too small ({s} bytes)"))
            }
            DecoderError::PaletteSizeExceeded {
                colors_used,
                bit_count,
            } => f.write_fmt(format_args!(
                "Palette size {colors_used} exceeds maximum size for BMP with bit count of {bit_count}"
            )),
        }
    }
}

impl From<DecoderError> for ImageError {
    fn from(e: DecoderError) -> ImageError {
        ImageError::Decoding(DecodingError::new(ImageFormat::Bmp.into(), e))
    }
}

impl error::Error for DecoderError {}

/// Distinct image types whose saved channel width can be invalid
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum ChannelWidthError {
    /// RGB
    Rgb,
    /// 8-bit run length encoding
    Rle8,
    /// 4-bit run length encoding
    Rle4,
    /// Bitfields (16- or 32-bit)
    Bitfields,
}

impl fmt::Display for ChannelWidthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ChannelWidthError::Rgb => "RGB",
            ChannelWidthError::Rle8 => "RLE8",
            ChannelWidthError::Rle4 => "RLE4",
            ChannelWidthError::Bitfields => "bitfields",
        })
    }
}

/// Convenience function to check if the combination of width, length and number of
/// channels would result in a buffer that would overflow.
fn check_for_overflow(width: i32, length: i32, channels: usize) -> ImageResult<()> {
    num_bytes(width, length, channels)
        .map(|_| ())
        .ok_or_else(|| {
            ImageError::Unsupported(UnsupportedError::from_format_and_kind(
                ImageFormat::Bmp.into(),
                UnsupportedErrorKind::GenericFeature(format!(
                    "Image dimensions ({width}x{length} w/{channels} channels) are too large"
                )),
            ))
        })
}

/// Calculate how many many bytes a buffer holding a decoded image with these properties would
/// require. Returns `None` if the buffer size would overflow or if one of the sizes are negative.
fn num_bytes(width: i32, length: i32, channels: usize) -> Option<usize> {
    if width <= 0 || length <= 0 {
        None
    } else {
        match channels.checked_mul(width as usize) {
            Some(n) => n.checked_mul(length as usize),
            None => None,
        }
    }
}

/// Call the provided function on each row of the provided buffer, returning Err if the provided
/// function returns an error, extends the buffer if it's not large enough.
fn with_rows<F>(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    channels: usize,
    top_down: bool,
    mut func: F,
) -> io::Result<()>
where
    F: FnMut(&mut [u8]) -> io::Result<()>,
{
    // An overflow should already have been checked for when this is called,
    // though we check anyhow, as it somehow seems to increase performance slightly.
    let row_width = channels.checked_mul(width as usize).unwrap();
    let full_image_size = row_width.checked_mul(height as usize).unwrap();
    assert_eq!(buffer.len(), full_image_size);

    if !top_down {
        for row in buffer.chunks_mut(row_width).rev() {
            func(row)?;
        }
    } else {
        for row in buffer.chunks_mut(row_width) {
            func(row)?;
        }
    }
    Ok(())
}

fn set_8bit_pixel_run<'a, T: Iterator<Item = &'a u8>>(
    pixel_iter: &mut ChunksExactMut<u8>,
    palette: &[[u8; 3]],
    indices: T,
    n_pixels: usize,
) -> bool {
    for idx in indices.take(n_pixels) {
        if let Some(pixel) = pixel_iter.next() {
            let rgb = palette[*idx as usize];
            pixel[0] = rgb[0];
            pixel[1] = rgb[1];
            pixel[2] = rgb[2];
        } else {
            return false;
        }
    }
    true
}

fn set_4bit_pixel_run<'a, T: Iterator<Item = &'a u8>>(
    pixel_iter: &mut ChunksExactMut<u8>,
    palette: &[[u8; 3]],
    indices: T,
    mut n_pixels: usize,
) -> bool {
    for idx in indices {
        macro_rules! set_pixel {
            ($i:expr) => {
                if n_pixels == 0 {
                    break;
                }
                if let Some(pixel) = pixel_iter.next() {
                    let rgb = palette[$i as usize];
                    pixel[0] = rgb[0];
                    pixel[1] = rgb[1];
                    pixel[2] = rgb[2];
                } else {
                    return false;
                }
                n_pixels -= 1;
            };
        }
        set_pixel!(idx >> 4);
        set_pixel!(idx & 0xf);
    }
    true
}

#[rustfmt::skip]
fn set_2bit_pixel_run<'a, T: Iterator<Item = &'a u8>>(
    pixel_iter: &mut ChunksExactMut<u8>,
    palette: &[[u8; 3]],
    indices: T,
    mut n_pixels: usize,
) -> bool {
    for idx in indices {
        macro_rules! set_pixel {
            ($i:expr) => {
                if n_pixels == 0 {
                    break;
                }
                if let Some(pixel) = pixel_iter.next() {
                    let rgb = palette[$i as usize];
                    pixel[0] = rgb[0];
                    pixel[1] = rgb[1];
                    pixel[2] = rgb[2];
                } else {
                    return false;
                }
                n_pixels -= 1;
            };
        }
        set_pixel!((idx >> 6) & 0x3u8);
        set_pixel!((idx >> 4) & 0x3u8);
        set_pixel!((idx >> 2) & 0x3u8);
        set_pixel!( idx       & 0x3u8);
    }
    true
}

fn set_1bit_pixel_run<'a, T: Iterator<Item = &'a u8>>(
    pixel_iter: &mut ChunksExactMut<u8>,
    palette: &[[u8; 3]],
    indices: T,
) {
    for idx in indices {
        let mut bit = 0x80;
        loop {
            if let Some(pixel) = pixel_iter.next() {
                let rgb = palette[usize::from((idx & bit) != 0)];
                pixel[0] = rgb[0];
                pixel[1] = rgb[1];
                pixel[2] = rgb[2];
            } else {
                return;
            }

            bit >>= 1;
            if bit == 0 {
                break;
            }
        }
    }
}

#[derive(PartialEq, Eq)]
struct Bitfield {
    shift: u32,
    len: u32,
}

impl Bitfield {
    fn from_mask(mask: u32, max_len: u32) -> ImageResult<Bitfield> {
        if mask == 0 {
            return Ok(Bitfield { shift: 0, len: 0 });
        }
        let mut shift = mask.trailing_zeros();
        let mut len = (!(mask >> shift)).trailing_zeros();
        if len != mask.count_ones() {
            return Err(DecoderError::BitfieldMaskNonContiguous.into());
        }
        if len + shift > max_len {
            return Err(DecoderError::BitfieldMaskInvalid.into());
        }
        if len > 8 {
            shift += len - 8;
            len = 8;
        }
        Ok(Bitfield { shift, len })
    }

    fn read(&self, data: u32) -> u8 {
        let data = data >> self.shift;
        match self.len {
            1 => ((data & 0b1) * 0xff) as u8,
            2 => ((data & 0b11) * 0x55) as u8,
            3 => LOOKUP_TABLE_3_BIT_TO_8_BIT[(data & 0b00_0111) as usize],
            4 => LOOKUP_TABLE_4_BIT_TO_8_BIT[(data & 0b00_1111) as usize],
            5 => LOOKUP_TABLE_5_BIT_TO_8_BIT[(data & 0b01_1111) as usize],
            6 => LOOKUP_TABLE_6_BIT_TO_8_BIT[(data & 0b11_1111) as usize],
            7 => (((data & 0x7f) << 1) | ((data & 0x7f) >> 6)) as u8,
            8 => (data & 0xff) as u8,
            _ => panic!(),
        }
    }
}

#[derive(PartialEq, Eq)]
struct Bitfields {
    r: Bitfield,
    g: Bitfield,
    b: Bitfield,
    a: Bitfield,
}

impl Bitfields {
    fn from_mask(
        r_mask: u32,
        g_mask: u32,
        b_mask: u32,
        a_mask: u32,
        max_len: u32,
    ) -> ImageResult<Bitfields> {
        let bitfields = Bitfields {
            r: Bitfield::from_mask(r_mask, max_len)?,
            g: Bitfield::from_mask(g_mask, max_len)?,
            b: Bitfield::from_mask(b_mask, max_len)?,
            a: Bitfield::from_mask(a_mask, max_len)?,
        };
        if bitfields.r.len == 0 || bitfields.g.len == 0 || bitfields.b.len == 0 {
            return Err(DecoderError::BitfieldMaskMissing(max_len).into());
        }
        Ok(bitfields)
    }
}

/// A bmp decoder
pub struct BmpDecoder<R> {
    reader: R,

    bmp_header_type: BMPHeaderType,
    indexed_color: bool,

    width: i32,
    height: i32,
    data_offset: u64,
    top_down: bool,
    no_file_header: bool,
    add_alpha_channel: bool,
    has_loaded_metadata: bool,
    image_type: ImageType,

    bit_count: u16,
    colors_used: u32,
    palette: Option<Vec<[u8; 3]>>,
    bitfields: Option<Bitfields>,
}

enum RLEInsn {
    EndOfFile,
    EndOfRow,
    Delta(u8, u8),
    Absolute(u8, Vec<u8>),
    PixelRun(u8, u8),
}

impl<R: BufRead + Seek> BmpDecoder<R> {
    fn new_decoder(reader: R) -> BmpDecoder<R> {
        BmpDecoder {
            reader,

            bmp_header_type: BMPHeaderType::Info,
            indexed_color: false,

            width: 0,
            height: 0,
            data_offset: 0,
            top_down: false,
            no_file_header: false,
            add_alpha_channel: false,
            has_loaded_metadata: false,
            image_type: ImageType::Palette,

            bit_count: 0,
            colors_used: 0,
            palette: None,
            bitfields: None,
        }
    }

    /// Create a new decoder that decodes from the stream ```r```
    pub fn new(reader: R) -> ImageResult<BmpDecoder<R>> {
        let mut decoder = Self::new_decoder(reader);
        decoder.read_metadata()?;
        Ok(decoder)
    }

    /// Create a new decoder that decodes from the stream ```r``` without first
    /// reading a BITMAPFILEHEADER. This is useful for decoding the `CF_DIB` format
    /// directly from the Windows clipboard.
    pub fn new_without_file_header(reader: R) -> ImageResult<BmpDecoder<R>> {
        let mut decoder = Self::new_decoder(reader);
        decoder.no_file_header = true;
        decoder.read_metadata()?;
        Ok(decoder)
    }

    #[cfg(feature = "ico")]
    pub(crate) fn new_with_ico_format(reader: R) -> ImageResult<BmpDecoder<R>> {
        let mut decoder = Self::new_decoder(reader);
        decoder.read_metadata_in_ico_format()?;
        Ok(decoder)
    }

    /// If true, the palette in BMP does not apply to the image even if it is found.
    /// In other words, the output image is the indexed color.
    pub fn set_indexed_color(&mut self, indexed_color: bool) {
        self.indexed_color = indexed_color;
    }

    #[cfg(feature = "ico")]
    pub(crate) fn reader(&mut self) -> &mut R {
        &mut self.reader
    }

    fn read_file_header(&mut self) -> ImageResult<()> {
        if self.no_file_header {
            return Ok(());
        }
        let mut signature = [0; 2];
        self.reader.read_exact(&mut signature)?;

        if signature != b"BM"[..] {
            return Err(DecoderError::BmpSignatureInvalid.into());
        }

        // The next 8 bytes represent file size, followed the 4 reserved bytes
        // We're not interesting these values
        self.reader.read_u32::<LittleEndian>()?;
        self.reader.read_u32::<LittleEndian>()?;

        self.data_offset = u64::from(self.reader.read_u32::<LittleEndian>()?);

        Ok(())
    }

    /// Read BITMAPCOREHEADER <https://msdn.microsoft.com/en-us/library/vs/alm/dd183372(v=vs.85).aspx>
    ///
    /// returns Err if any of the values are invalid.
    fn read_bitmap_core_header(&mut self) -> ImageResult<()> {
        // As height/width values in BMP files with core headers are only 16 bits long,
        // they won't be larger than `MAX_WIDTH_HEIGHT`.
        self.width = i32::from(self.reader.read_u16::<LittleEndian>()?);
        self.height = i32::from(self.reader.read_u16::<LittleEndian>()?);

        check_for_overflow(self.width, self.height, self.num_channels())?;

        // Number of planes (format specifies that this should be 1).
        if self.reader.read_u16::<LittleEndian>()? != 1 {
            return Err(DecoderError::MoreThanOnePlane.into());
        }

        self.bit_count = self.reader.read_u16::<LittleEndian>()?;
        self.image_type = match self.bit_count {
            1 | 4 | 8 => ImageType::Palette,
            24 => ImageType::RGB24,
            _ => {
                return Err(DecoderError::InvalidChannelWidth(
                    ChannelWidthError::Rgb,
                    self.bit_count,
                )
                .into())
            }
        };

        Ok(())
    }

    /// Read BITMAPINFOHEADER <https://msdn.microsoft.com/en-us/library/vs/alm/dd183376(v=vs.85).aspx>
    /// or BITMAPV{2|3|4|5}HEADER.
    ///
    /// returns Err if any of the values are invalid.
    fn read_bitmap_info_header(&mut self) -> ImageResult<()> {
        self.width = self.reader.read_i32::<LittleEndian>()?;
        self.height = self.reader.read_i32::<LittleEndian>()?;

        // Width can not be negative
        if self.width < 0 {
            return Err(DecoderError::NegativeWidth(self.width).into());
        } else if self.width > MAX_WIDTH_HEIGHT || self.height > MAX_WIDTH_HEIGHT {
            // Limit very large image sizes to avoid OOM issues. Images with these sizes are
            // unlikely to be valid anyhow.
            return Err(DecoderError::ImageTooLarge(self.width, self.height).into());
        }

        if self.height == i32::MIN {
            return Err(DecoderError::InvalidHeight.into());
        }

        // A negative height indicates a top-down DIB.
        if self.height < 0 {
            self.height *= -1;
            self.top_down = true;
        }

        check_for_overflow(self.width, self.height, self.num_channels())?;

        // Number of planes (format specifies that this should be 1).
        if self.reader.read_u16::<LittleEndian>()? != 1 {
            return Err(DecoderError::MoreThanOnePlane.into());
        }

        self.bit_count = self.reader.read_u16::<LittleEndian>()?;
        let image_type_u32 = self.reader.read_u32::<LittleEndian>()?;

        // Top-down dibs can not be compressed.
        if self.top_down && image_type_u32 != 0 && image_type_u32 != 3 {
            return Err(DecoderError::ImageTypeInvalidForTopDown(image_type_u32).into());
        }
        self.image_type = match image_type_u32 {
            0 => match self.bit_count {
                1 | 2 | 4 | 8 => ImageType::Palette,
                16 => ImageType::RGB16,
                24 => ImageType::RGB24,
                32 if self.add_alpha_channel => ImageType::RGBA32,
                32 => ImageType::RGB32,
                _ => {
                    return Err(DecoderError::InvalidChannelWidth(
                        ChannelWidthError::Rgb,
                        self.bit_count,
                    )
                    .into())
                }
            },
            1 => match self.bit_count {
                8 => ImageType::RLE8,
                _ => {
                    return Err(DecoderError::InvalidChannelWidth(
                        ChannelWidthError::Rle8,
                        self.bit_count,
                    )
                    .into())
                }
            },
            2 => match self.bit_count {
                4 => ImageType::RLE4,
                _ => {
                    return Err(DecoderError::InvalidChannelWidth(
                        ChannelWidthError::Rle4,
                        self.bit_count,
                    )
                    .into())
                }
            },
            3 => match self.bit_count {
                16 => ImageType::Bitfields16,
                32 => ImageType::Bitfields32,
                _ => {
                    return Err(DecoderError::InvalidChannelWidth(
                        ChannelWidthError::Bitfields,
                        self.bit_count,
                    )
                    .into())
                }
            },
            4 => {
                // JPEG compression is not implemented yet.
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Bmp.into(),
                        UnsupportedErrorKind::GenericFeature("JPEG compression".to_owned()),
                    ),
                ));
            }
            5 => {
                // PNG compression is not implemented yet.
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Bmp.into(),
                        UnsupportedErrorKind::GenericFeature("PNG compression".to_owned()),
                    ),
                ));
            }
            11..=13 => {
                // CMYK types are not implemented yet.
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Bmp.into(),
                        UnsupportedErrorKind::GenericFeature("CMYK format".to_owned()),
                    ),
                ));
            }
            _ => {
                // Unknown compression type.
                return Err(DecoderError::ImageTypeUnknown(image_type_u32).into());
            }
        };

        // The next 12 bytes represent data array size in bytes,
        // followed the horizontal and vertical printing resolutions
        // We will calculate the pixel array size using width & height of image
        // We're not interesting the horz or vert printing resolutions
        self.reader.read_u32::<LittleEndian>()?;
        self.reader.read_u32::<LittleEndian>()?;
        self.reader.read_u32::<LittleEndian>()?;

        self.colors_used = self.reader.read_u32::<LittleEndian>()?;

        // The next 4 bytes represent number of "important" colors
        // We're not interested in this value, so we'll skip it
        self.reader.read_u32::<LittleEndian>()?;

        Ok(())
    }

    fn read_bitmasks(&mut self) -> ImageResult<()> {
        let r_mask = self.reader.read_u32::<LittleEndian>()?;
        let g_mask = self.reader.read_u32::<LittleEndian>()?;
        let b_mask = self.reader.read_u32::<LittleEndian>()?;

        let a_mask = match self.bmp_header_type {
            BMPHeaderType::V3 | BMPHeaderType::V4 | BMPHeaderType::V5 => {
                self.reader.read_u32::<LittleEndian>()?
            }
            _ => 0,
        };

        self.bitfields = match self.image_type {
            ImageType::Bitfields16 => {
                Some(Bitfields::from_mask(r_mask, g_mask, b_mask, a_mask, 16)?)
            }
            ImageType::Bitfields32 => {
                Some(Bitfields::from_mask(r_mask, g_mask, b_mask, a_mask, 32)?)
            }
            _ => None,
        };

        if self.bitfields.is_some() && a_mask != 0 {
            self.add_alpha_channel = true;
        }

        Ok(())
    }

    fn read_metadata(&mut self) -> ImageResult<()> {
        if !self.has_loaded_metadata {
            self.read_file_header()?;
            let bmp_header_offset = self.reader.stream_position()?;
            let bmp_header_size = self.reader.read_u32::<LittleEndian>()?;
            let bmp_header_end = bmp_header_offset + u64::from(bmp_header_size);

            self.bmp_header_type = match bmp_header_size {
                BITMAPCOREHEADER_SIZE => BMPHeaderType::Core,
                BITMAPINFOHEADER_SIZE => BMPHeaderType::Info,
                BITMAPV2HEADER_SIZE => BMPHeaderType::V2,
                BITMAPV3HEADER_SIZE => BMPHeaderType::V3,
                BITMAPV4HEADER_SIZE => BMPHeaderType::V4,
                BITMAPV5HEADER_SIZE => BMPHeaderType::V5,
                _ if bmp_header_size < BITMAPCOREHEADER_SIZE => {
                    // Size of any valid header types won't be smaller than core header type.
                    return Err(DecoderError::HeaderTooSmall(bmp_header_size).into());
                }
                _ => {
                    return Err(ImageError::Unsupported(
                        UnsupportedError::from_format_and_kind(
                            ImageFormat::Bmp.into(),
                            UnsupportedErrorKind::GenericFeature(format!(
                                "Unknown bitmap header type (size={bmp_header_size})"
                            )),
                        ),
                    ))
                }
            };

            match self.bmp_header_type {
                BMPHeaderType::Core => {
                    self.read_bitmap_core_header()?;
                }
                BMPHeaderType::Info
                | BMPHeaderType::V2
                | BMPHeaderType::V3
                | BMPHeaderType::V4
                | BMPHeaderType::V5 => {
                    self.read_bitmap_info_header()?;
                }
            }

            let mut bitmask_bytes_offset = 0;
            if self.image_type == ImageType::Bitfields16
                || self.image_type == ImageType::Bitfields32
            {
                self.read_bitmasks()?;

                // Per https://learn.microsoft.com/en-us/windows/win32/gdi/bitmap-header-types, bitmaps
                // using the `BITMAPINFOHEADER`, `BITMAPV4HEADER`, or `BITMAPV5HEADER` structures with
                // an image type of `BI_BITFIELD` contain RGB bitfield masks immediately after the header.
                //
                // `read_bitmasks` correctly reads these from earlier in the header itself but we must
                // ensure the reader starts on the image data itself, not these extra mask bytes.
                if matches!(
                    self.bmp_header_type,
                    BMPHeaderType::Info | BMPHeaderType::V4 | BMPHeaderType::V5
                ) {
                    // This is `size_of::<u32>() * 3` (a red, green, and blue mask), but with less noise.
                    bitmask_bytes_offset = 12;
                }
            };

            self.reader
                .seek(SeekFrom::Start(bmp_header_end + bitmask_bytes_offset))?;

            match self.image_type {
                ImageType::Palette | ImageType::RLE4 | ImageType::RLE8 => self.read_palette()?,
                _ => {}
            }

            if self.no_file_header {
                // Use the offset of the end of metadata instead of reading a BMP file header.
                self.data_offset = self.reader.stream_position()?;
            }

            self.has_loaded_metadata = true;
        }
        Ok(())
    }

    #[cfg(feature = "ico")]
    #[doc(hidden)]
    pub fn read_metadata_in_ico_format(&mut self) -> ImageResult<()> {
        self.no_file_header = true;
        self.add_alpha_channel = true;
        self.read_metadata()?;

        // The height field in an ICO file is doubled to account for the AND mask
        // (whether or not an AND mask is actually present).
        self.height /= 2;
        Ok(())
    }

    fn get_palette_size(&mut self) -> ImageResult<usize> {
        match self.colors_used {
            0 => Ok(1 << self.bit_count),
            _ => {
                if self.colors_used > 1 << self.bit_count {
                    return Err(DecoderError::PaletteSizeExceeded {
                        colors_used: self.colors_used,
                        bit_count: self.bit_count,
                    }
                    .into());
                }
                Ok(self.colors_used as usize)
            }
        }
    }

    fn bytes_per_color(&self) -> usize {
        match self.bmp_header_type {
            BMPHeaderType::Core => 3,
            _ => 4,
        }
    }

    fn read_palette(&mut self) -> ImageResult<()> {
        const MAX_PALETTE_SIZE: usize = 256; // Palette indices are u8.

        let bytes_per_color = self.bytes_per_color();
        let palette_size = self.get_palette_size()?;
        let max_length = MAX_PALETTE_SIZE * bytes_per_color;

        let length = palette_size * bytes_per_color;
        let mut buf = vec_try_with_capacity(max_length)?;

        // Resize and read the palette entries to the buffer.
        // We limit the buffer to at most 256 colours to avoid any oom issues as
        // 8-bit images can't reference more than 256 indexes anyhow.
        buf.resize(cmp::min(length, max_length), 0);
        self.reader.by_ref().read_exact(&mut buf)?;

        // Allocate 256 entries even if palette_size is smaller, to prevent corrupt files from
        // causing an out-of-bounds array access.
        match length.cmp(&max_length) {
            Ordering::Greater => {
                self.reader
                    .seek(SeekFrom::Current((length - max_length) as i64))?;
            }
            Ordering::Less => buf.resize(max_length, 0),
            Ordering::Equal => (),
        }

        let p: Vec<[u8; 3]> = (0..MAX_PALETTE_SIZE)
            .map(|i| {
                let b = buf[bytes_per_color * i];
                let g = buf[bytes_per_color * i + 1];
                let r = buf[bytes_per_color * i + 2];
                [r, g, b]
            })
            .collect();

        self.palette = Some(p);

        Ok(())
    }

    /// Get the palette that is embedded in the BMP image, if any.
    pub fn get_palette(&self) -> Option<&[[u8; 3]]> {
        self.palette.as_ref().map(|vec| &vec[..])
    }

    fn num_channels(&self) -> usize {
        if self.indexed_color {
            1
        } else if self.add_alpha_channel {
            4
        } else {
            3
        }
    }

    fn rows<'a>(&self, pixel_data: &'a mut [u8]) -> RowIterator<'a> {
        let stride = self.width as usize * self.num_channels();
        if self.top_down {
            RowIterator {
                chunks: Chunker::FromTop(pixel_data.chunks_exact_mut(stride)),
            }
        } else {
            RowIterator {
                chunks: Chunker::FromBottom(pixel_data.chunks_exact_mut(stride).rev()),
            }
        }
    }

    fn read_palettized_pixel_data(&mut self, buf: &mut [u8]) -> ImageResult<()> {
        let num_channels = self.num_channels();
        let row_byte_length = ((i32::from(self.bit_count) * self.width + 31) / 32 * 4) as usize;
        let mut indices = vec![0; row_byte_length];
        let palette = self.palette.as_ref().unwrap();
        let bit_count = self.bit_count;
        let reader = &mut self.reader;
        let width = self.width as usize;
        let skip_palette = self.indexed_color;

        reader.seek(SeekFrom::Start(self.data_offset))?;

        if num_channels == 4 {
            buf.chunks_exact_mut(4).for_each(|c| c[3] = 0xFF);
        }

        with_rows(
            buf,
            self.width,
            self.height,
            num_channels,
            self.top_down,
            |row| {
                reader.read_exact(&mut indices)?;
                if skip_palette {
                    row.clone_from_slice(&indices[0..width]);
                } else {
                    let mut pixel_iter = row.chunks_exact_mut(num_channels);
                    match bit_count {
                        1 => {
                            set_1bit_pixel_run(&mut pixel_iter, palette, indices.iter());
                        }
                        2 => {
                            set_2bit_pixel_run(&mut pixel_iter, palette, indices.iter(), width);
                        }
                        4 => {
                            set_4bit_pixel_run(&mut pixel_iter, palette, indices.iter(), width);
                        }
                        8 => {
                            set_8bit_pixel_run(&mut pixel_iter, palette, indices.iter(), width);
                        }
                        _ => panic!(),
                    }
                }
                Ok(())
            },
        )?;

        Ok(())
    }

    fn read_16_bit_pixel_data(
        &mut self,
        buf: &mut [u8],
        bitfields: Option<&Bitfields>,
    ) -> ImageResult<()> {
        let num_channels = self.num_channels();
        let row_padding_len = self.width as usize % 2 * 2;
        let row_padding = &mut [0; 2][..row_padding_len];
        let bitfields = match bitfields {
            Some(b) => b,
            None => self.bitfields.as_ref().unwrap(),
        };
        let reader = &mut self.reader;

        reader.seek(SeekFrom::Start(self.data_offset))?;

        with_rows(
            buf,
            self.width,
            self.height,
            num_channels,
            self.top_down,
            |row| {
                for pixel in row.chunks_mut(num_channels) {
                    let data = u32::from(reader.read_u16::<LittleEndian>()?);

                    pixel[0] = bitfields.r.read(data);
                    pixel[1] = bitfields.g.read(data);
                    pixel[2] = bitfields.b.read(data);
                    if num_channels == 4 {
                        if bitfields.a.len != 0 {
                            pixel[3] = bitfields.a.read(data);
                        } else {
                            pixel[3] = 0xFF;
                        }
                    }
                }
                reader.read_exact(row_padding)
            },
        )?;

        Ok(())
    }

    /// Read image data from a reader in 32-bit formats that use bitfields.
    fn read_32_bit_pixel_data(&mut self, buf: &mut [u8]) -> ImageResult<()> {
        let num_channels = self.num_channels();

        let bitfields = self.bitfields.as_ref().unwrap();

        let reader = &mut self.reader;
        reader.seek(SeekFrom::Start(self.data_offset))?;

        with_rows(
            buf,
            self.width,
            self.height,
            num_channels,
            self.top_down,
            |row| {
                for pixel in row.chunks_mut(num_channels) {
                    let data = reader.read_u32::<LittleEndian>()?;

                    pixel[0] = bitfields.r.read(data);
                    pixel[1] = bitfields.g.read(data);
                    pixel[2] = bitfields.b.read(data);
                    if num_channels == 4 {
                        if bitfields.a.len != 0 {
                            pixel[3] = bitfields.a.read(data);
                        } else {
                            pixel[3] = 0xff;
                        }
                    }
                }
                Ok(())
            },
        )?;

        Ok(())
    }

    /// Read image data from a reader where the colours are stored as 8-bit values (24 or 32-bit).
    fn read_full_byte_pixel_data(
        &mut self,
        buf: &mut [u8],
        format: &FormatFullBytes,
    ) -> ImageResult<()> {
        let num_channels = self.num_channels();
        let row_padding_len = match *format {
            FormatFullBytes::RGB24 => (4 - (self.width as usize * 3) % 4) % 4,
            _ => 0,
        };
        let row_padding = &mut [0; 4][..row_padding_len];

        self.reader.seek(SeekFrom::Start(self.data_offset))?;

        let reader = &mut self.reader;

        with_rows(
            buf,
            self.width,
            self.height,
            num_channels,
            self.top_down,
            |row| {
                for pixel in row.chunks_mut(num_channels) {
                    if *format == FormatFullBytes::Format888 {
                        reader.read_u8()?;
                    }

                    // Read the colour values (b, g, r).
                    // Reading 3 bytes and reversing them is significantly faster than reading one
                    // at a time.
                    reader.read_exact(&mut pixel[0..3])?;
                    pixel[0..3].reverse();

                    if *format == FormatFullBytes::RGB32 {
                        reader.read_u8()?;
                    }

                    // Read the alpha channel if present
                    if *format == FormatFullBytes::RGBA32 {
                        reader.read_exact(&mut pixel[3..4])?;
                    } else if num_channels == 4 {
                        pixel[3] = 0xFF;
                    }
                }
                reader.read_exact(row_padding)
            },
        )?;

        Ok(())
    }

    fn read_rle_data(&mut self, buf: &mut [u8], image_type: ImageType) -> ImageResult<()> {
        // Seek to the start of the actual image data.
        self.reader.seek(SeekFrom::Start(self.data_offset))?;

        let num_channels = self.num_channels();
        let p = self.palette.as_ref().unwrap();

        // Handling deltas in the RLE scheme means that we need to manually
        // iterate through rows and pixels.  Even if we didn't have to handle
        // deltas, we have to ensure that a single runlength doesn't straddle
        // two rows.
        let mut row_iter = self.rows(buf);

        while let Some(row) = row_iter.next() {
            let mut pixel_iter = row.chunks_exact_mut(num_channels);

            let mut x = 0;
            loop {
                let instruction = {
                    let control_byte = self.reader.read_u8()?;
                    match control_byte {
                        RLE_ESCAPE => {
                            let op = self.reader.read_u8()?;

                            match op {
                                RLE_ESCAPE_EOL => RLEInsn::EndOfRow,
                                RLE_ESCAPE_EOF => RLEInsn::EndOfFile,
                                RLE_ESCAPE_DELTA => {
                                    let xdelta = self.reader.read_u8()?;
                                    let ydelta = self.reader.read_u8()?;
                                    RLEInsn::Delta(xdelta, ydelta)
                                }
                                _ => {
                                    let mut length = op as usize;
                                    if self.image_type == ImageType::RLE4 {
                                        length = length.div_ceil(2);
                                    }
                                    length += length & 1;
                                    let mut buffer = Vec::new();
                                    self.reader.read_exact_vec(&mut buffer, length)?;
                                    RLEInsn::Absolute(op, buffer)
                                }
                            }
                        }
                        _ => {
                            let palette_index = self.reader.read_u8()?;
                            RLEInsn::PixelRun(control_byte, palette_index)
                        }
                    }
                };

                match instruction {
                    RLEInsn::EndOfFile => {
                        pixel_iter.for_each(|p| p.fill(0));
                        row_iter.for_each(|r| r.fill(0));
                        return Ok(());
                    }
                    RLEInsn::EndOfRow => {
                        pixel_iter.for_each(|p| p.fill(0));
                        break;
                    }
                    RLEInsn::Delta(x_delta, y_delta) => {
                        // The msdn site on bitmap compression doesn't specify
                        // what happens to the values skipped when encountering
                        // a delta code, however IE and the windows image
                        // preview seems to replace them with black pixels,
                        // so we stick to that.

                        if y_delta > 0 {
                            // Zero out the remainder of the current row.
                            pixel_iter.for_each(|p| p.fill(0));

                            // If any full rows are skipped, zero them out.
                            for _ in 1..y_delta {
                                let row = row_iter.next().ok_or(DecoderError::CorruptRleData)?;
                                row.fill(0);
                            }

                            // Set the pixel iterator to the start of the next row.
                            pixel_iter = row_iter
                                .next()
                                .ok_or(DecoderError::CorruptRleData)?
                                .chunks_exact_mut(num_channels);

                            // Zero out the pixels up to the current point in the row.
                            for _ in 0..x {
                                pixel_iter
                                    .next()
                                    .ok_or(DecoderError::CorruptRleData)?
                                    .fill(0);
                            }
                        }

                        for _ in 0..x_delta {
                            let pixel = pixel_iter.next().ok_or(DecoderError::CorruptRleData)?;
                            pixel.fill(0);
                        }
                        x += x_delta as usize;
                    }
                    RLEInsn::Absolute(length, indices) => {
                        // Absolute mode cannot span rows, so if we run
                        // out of pixels to process, we should stop
                        // processing the image.
                        match image_type {
                            ImageType::RLE8 => {
                                if !set_8bit_pixel_run(
                                    &mut pixel_iter,
                                    p,
                                    indices.iter(),
                                    length as usize,
                                ) {
                                    return Err(DecoderError::CorruptRleData.into());
                                }
                            }
                            ImageType::RLE4 => {
                                if !set_4bit_pixel_run(
                                    &mut pixel_iter,
                                    p,
                                    indices.iter(),
                                    length as usize,
                                ) {
                                    return Err(DecoderError::CorruptRleData.into());
                                }
                            }
                            _ => unreachable!(),
                        }
                        x += length as usize;
                    }
                    RLEInsn::PixelRun(n_pixels, palette_index) => {
                        match image_type {
                            ImageType::RLE8 => {
                                // A pixel run isn't allowed to span rows.
                                // imagemagick produces invalid images where n_pixels exceeds row length,
                                // so we clamp n_pixels to the row length to display them properly:
                                // https://github.com/image-rs/image/issues/2321
                                //
                                // This is like set_8bit_pixel_run() but doesn't fail when `n_pixels` is too large
                                let repeat_pixel: [u8; 3] = p[palette_index as usize];
                                (&mut pixel_iter).take(n_pixels as usize).for_each(|p| {
                                    p[2] = repeat_pixel[2];
                                    p[1] = repeat_pixel[1];
                                    p[0] = repeat_pixel[0];
                                });
                            }
                            ImageType::RLE4 => {
                                if !set_4bit_pixel_run(
                                    &mut pixel_iter,
                                    p,
                                    repeat(&palette_index),
                                    n_pixels as usize,
                                ) {
                                    return Err(DecoderError::CorruptRleData.into());
                                }
                            }
                            _ => unreachable!(),
                        }
                        x += n_pixels as usize;
                    }
                }
            }
        }

        Ok(())
    }

    /// Read the actual data of the image. This function is deliberately not public because it
    /// cannot be called multiple times without seeking back the underlying reader in between.
    pub(crate) fn read_image_data(&mut self, buf: &mut [u8]) -> ImageResult<()> {
        match self.image_type {
            ImageType::Palette => self.read_palettized_pixel_data(buf),
            ImageType::RGB16 => self.read_16_bit_pixel_data(buf, Some(&R5_G5_B5_COLOR_MASK)),
            ImageType::RGB24 => self.read_full_byte_pixel_data(buf, &FormatFullBytes::RGB24),
            ImageType::RGB32 => self.read_full_byte_pixel_data(buf, &FormatFullBytes::RGB32),
            ImageType::RGBA32 => self.read_full_byte_pixel_data(buf, &FormatFullBytes::RGBA32),
            ImageType::RLE8 => self.read_rle_data(buf, ImageType::RLE8),
            ImageType::RLE4 => self.read_rle_data(buf, ImageType::RLE4),
            ImageType::Bitfields16 => match self.bitfields {
                Some(_) => self.read_16_bit_pixel_data(buf, None),
                None => Err(DecoderError::BitfieldMasksMissing(16).into()),
            },
            ImageType::Bitfields32 => match self.bitfields {
                Some(R8_G8_B8_COLOR_MASK) => {
                    self.read_full_byte_pixel_data(buf, &FormatFullBytes::Format888)
                }
                Some(R8_G8_B8_A8_COLOR_MASK) => {
                    self.read_full_byte_pixel_data(buf, &FormatFullBytes::RGBA32)
                }
                Some(_) => self.read_32_bit_pixel_data(buf),
                None => Err(DecoderError::BitfieldMasksMissing(32).into()),
            },
        }
    }
}

impl<R: BufRead + Seek> ImageDecoder for BmpDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        (self.width as u32, self.height as u32)
    }

    fn color_type(&self) -> ColorType {
        if self.indexed_color {
            ColorType::L8
        } else if self.add_alpha_channel {
            ColorType::Rgba8
        } else {
            ColorType::Rgb8
        }
    }

    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));
        self.read_image_data(buf)
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

impl<R: BufRead + Seek> ImageDecoderRect for BmpDecoder<R> {
    fn read_rect(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        buf: &mut [u8],
        row_pitch: usize,
    ) -> ImageResult<()> {
        let start = self.reader.stream_position()?;
        load_rect(
            x,
            y,
            width,
            height,
            buf,
            row_pitch,
            self,
            self.total_bytes() as usize,
            |_, _| Ok(()),
            |s, buf| s.read_image_data(buf),
        )?;
        self.reader.seek(SeekFrom::Start(start))?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::{BufReader, Cursor};

    use super::*;

    #[test]
    fn test_bitfield_len() {
        for len in 1..9 {
            let bitfield = Bitfield { shift: 0, len };
            for i in 0..(1 << len) {
                let read = bitfield.read(i);
                let calc = (f64::from(i) / f64::from((1 << len) - 1) * 255f64).round() as u8;
                if read != calc {
                    println!("len:{len} i:{i} read:{read} calc:{calc}");
                }
                assert_eq!(read, calc);
            }
        }
    }

    #[test]
    fn read_rect() {
        let f =
            BufReader::new(std::fs::File::open("tests/images/bmp/images/Core_8_Bit.bmp").unwrap());
        let mut decoder = BmpDecoder::new(f).unwrap();

        let mut buf: Vec<u8> = vec![0; 8 * 8 * 3];
        decoder.read_rect(0, 0, 8, 8, &mut buf, 8 * 3).unwrap();
    }

    #[test]
    fn read_rle_too_short() {
        let data = vec![
            0x42, 0x4d, 0x04, 0xee, 0xfe, 0xff, 0xff, 0x10, 0xff, 0x00, 0x04, 0x00, 0x00, 0x00,
            0x7c, 0x00, 0x00, 0x00, 0x0c, 0x41, 0x00, 0x00, 0x07, 0x10, 0x00, 0x00, 0x01, 0x00,
            0x04, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0d, 0x00, 0x00, 0x00,
            0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xfe, 0x21,
            0xff, 0x00, 0x66, 0x61, 0x72, 0x62, 0x66, 0x65, 0x6c, 0x64, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0xff, 0xd8, 0xff, 0x00, 0x00, 0x19, 0x51, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0xfa, 0xff, 0x00, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0f, 0x00,
            0x00, 0x00, 0x00, 0x2d, 0x31, 0x31, 0x35, 0x36, 0x00, 0xff, 0x00, 0x00, 0x52, 0x3a,
            0x37, 0x30, 0x7e, 0x71, 0x63, 0x91, 0x5a, 0x04, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2d, 0x35, 0x37, 0x00, 0xff, 0x00, 0x00, 0x52,
            0x3a, 0x37, 0x30, 0x7e, 0x71, 0x63, 0x91, 0x5a, 0x04, 0x05, 0x3c, 0x00, 0x00, 0x11,
            0x00, 0x5d, 0x7a, 0x82, 0xb7, 0xca, 0x2d, 0x31, 0xff, 0xff, 0xc7, 0x95, 0x33, 0x2e,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7c, 0x00,
            0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x66, 0x00, 0x4d,
            0x4d, 0x00, 0x2a, 0x00,
        ];

        let decoder = BmpDecoder::new(Cursor::new(&data)).unwrap();
        let mut buf = vec![0; usize::try_from(decoder.total_bytes()).unwrap()];
        assert!(decoder.read_image(&mut buf).is_ok());
    }

    #[test]
    fn test_no_header() {
        let tests = [
            "Info_R8_G8_B8.bmp",
            "Info_A8_R8_G8_B8.bmp",
            "Info_8_Bit.bmp",
            "Info_4_Bit.bmp",
            "Info_1_Bit.bmp",
        ];

        for name in &tests {
            let path = format!("tests/images/bmp/images/{name}");
            let ref_img = crate::open(&path).unwrap();
            let mut data = std::fs::read(&path).unwrap();
            // skip the BITMAPFILEHEADER
            let slice = &mut data[14..];
            let decoder = BmpDecoder::new_without_file_header(Cursor::new(slice)).unwrap();
            let no_hdr_img = crate::DynamicImage::from_decoder(decoder).unwrap();
            assert_eq!(ref_img, no_hdr_img);
        }
    }
}
