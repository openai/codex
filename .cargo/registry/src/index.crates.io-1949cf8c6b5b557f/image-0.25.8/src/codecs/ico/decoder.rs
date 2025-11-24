use byteorder_lite::{LittleEndian, ReadBytesExt};
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::{error, fmt};

use crate::color::ColorType;
use crate::error::{
    DecodingError, ImageError, ImageResult, UnsupportedError, UnsupportedErrorKind,
};
use crate::{ImageDecoder, ImageFormat};

use self::InnerDecoder::*;
use crate::codecs::bmp::BmpDecoder;
use crate::codecs::png::{PngDecoder, PNG_SIGNATURE};

/// Errors that can occur during decoding and parsing an ICO image or one of its enclosed images.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum DecoderError {
    /// The ICO directory is empty
    NoEntries,
    /// The number of color planes (0 or 1), or the horizontal coordinate of the hotspot for CUR files too big.
    IcoEntryTooManyPlanesOrHotspot,
    /// The bit depth (may be 0 meaning unspecified), or the vertical coordinate of the hotspot for CUR files too big.
    IcoEntryTooManyBitsPerPixelOrHotspot,

    /// The entry is in PNG format and specified a length that is shorter than PNG header.
    PngShorterThanHeader,
    /// The enclosed PNG is not in RGBA, which is invalid: <https://blogs.msdn.microsoft.com/oldnewthing/20101022-00/?p=12473>/.
    PngNotRgba,

    /// The entry is in BMP format and specified a data size that is not correct for the image and optional mask data.
    InvalidDataSize,

    /// The dimensions specified by the entry does not match the dimensions in the header of the enclosed image.
    ImageEntryDimensionMismatch {
        /// The mismatched subimage's type
        format: IcoEntryImageFormat,
        /// The dimensions specified by the entry
        entry: (u16, u16),
        /// The dimensions of the image itself
        image: (u32, u32),
    },
}

impl fmt::Display for DecoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecoderError::NoEntries => f.write_str("ICO directory contains no image"),
            DecoderError::IcoEntryTooManyPlanesOrHotspot => {
                f.write_str("ICO image entry has too many color planes or too large hotspot value")
            }
            DecoderError::IcoEntryTooManyBitsPerPixelOrHotspot => f.write_str(
                "ICO image entry has too many bits per pixel or too large hotspot value",
            ),
            DecoderError::PngShorterThanHeader => {
                f.write_str("Entry specified a length that is shorter than PNG header!")
            }
            DecoderError::PngNotRgba => f.write_str("The PNG is not in RGBA format!"),
            DecoderError::InvalidDataSize => {
                f.write_str("ICO image data size did not match expected size")
            }
            DecoderError::ImageEntryDimensionMismatch {
                format,
                entry,
                image,
            } => f.write_fmt(format_args!(
                "Entry{entry:?} and {format}{image:?} dimensions do not match!"
            )),
        }
    }
}

impl From<DecoderError> for ImageError {
    fn from(e: DecoderError) -> ImageError {
        ImageError::Decoding(DecodingError::new(ImageFormat::Ico.into(), e))
    }
}

impl error::Error for DecoderError {}

/// The image formats an ICO may contain
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum IcoEntryImageFormat {
    /// PNG in ARGB
    Png,
    /// BMP with optional alpha mask
    Bmp,
}

impl fmt::Display for IcoEntryImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            IcoEntryImageFormat::Png => "PNG",
            IcoEntryImageFormat::Bmp => "BMP",
        })
    }
}

impl From<IcoEntryImageFormat> for ImageFormat {
    fn from(val: IcoEntryImageFormat) -> Self {
        match val {
            IcoEntryImageFormat::Png => ImageFormat::Png,
            IcoEntryImageFormat::Bmp => ImageFormat::Bmp,
        }
    }
}

/// An ico decoder
pub struct IcoDecoder<R: BufRead + Seek> {
    selected_entry: DirEntry,
    inner_decoder: InnerDecoder<R>,
}

enum InnerDecoder<R: BufRead + Seek> {
    Bmp(BmpDecoder<R>),
    Png(Box<PngDecoder<R>>),
}

#[derive(Clone, Copy, Default)]
struct DirEntry {
    width: u8,
    height: u8,
    // We ignore some header fields as they will be replicated in the PNG, BMP and they are not
    // necessary for determining the best_entry.
    #[allow(unused)]
    color_count: u8,
    // Wikipedia has this to say:
    // Although Microsoft's technical documentation states that this value must be zero, the icon
    // encoder built into .NET (System.Drawing.Icon.Save) sets this value to 255. It appears that
    // the operating system ignores this value altogether.
    #[allow(unused)]
    reserved: u8,

    // We ignore some header fields as they will be replicated in the PNG, BMP and they are not
    // necessary for determining the best_entry.
    #[allow(unused)]
    num_color_planes: u16,
    bits_per_pixel: u16,

    image_length: u32,
    image_offset: u32,
}

impl<R: BufRead + Seek> IcoDecoder<R> {
    /// Create a new decoder that decodes from the stream ```r```
    pub fn new(mut r: R) -> ImageResult<IcoDecoder<R>> {
        let entries = read_entries(&mut r)?;
        let entry = best_entry(entries)?;
        let decoder = entry.decoder(r)?;

        Ok(IcoDecoder {
            selected_entry: entry,
            inner_decoder: decoder,
        })
    }
}

fn read_entries<R: Read>(r: &mut R) -> ImageResult<Vec<DirEntry>> {
    let _reserved = r.read_u16::<LittleEndian>()?;
    let _type = r.read_u16::<LittleEndian>()?;
    let count = r.read_u16::<LittleEndian>()?;
    (0..count).map(|_| read_entry(r)).collect()
}

fn read_entry<R: Read>(r: &mut R) -> ImageResult<DirEntry> {
    Ok(DirEntry {
        width: r.read_u8()?,
        height: r.read_u8()?,
        color_count: r.read_u8()?,
        reserved: r.read_u8()?,
        num_color_planes: {
            // This may be either the number of color planes (0 or 1), or the horizontal coordinate
            // of the hotspot for CUR files.
            let num = r.read_u16::<LittleEndian>()?;
            if num > 256 {
                return Err(DecoderError::IcoEntryTooManyPlanesOrHotspot.into());
            }
            num
        },
        bits_per_pixel: {
            // This may be either the bit depth (may be 0 meaning unspecified),
            // or the vertical coordinate of the hotspot for CUR files.
            let num = r.read_u16::<LittleEndian>()?;
            if num > 256 {
                return Err(DecoderError::IcoEntryTooManyBitsPerPixelOrHotspot.into());
            }
            num
        },
        image_length: r.read_u32::<LittleEndian>()?,
        image_offset: r.read_u32::<LittleEndian>()?,
    })
}

/// Find the entry with the highest (color depth, size).
fn best_entry(mut entries: Vec<DirEntry>) -> ImageResult<DirEntry> {
    let mut best = entries.pop().ok_or(DecoderError::NoEntries)?;

    let mut best_score = (
        best.bits_per_pixel,
        u32::from(best.real_width()) * u32::from(best.real_height()),
    );

    for entry in entries {
        let score = (
            entry.bits_per_pixel,
            u32::from(entry.real_width()) * u32::from(entry.real_height()),
        );
        if score > best_score {
            best = entry;
            best_score = score;
        }
    }
    Ok(best)
}

impl DirEntry {
    fn real_width(&self) -> u16 {
        match self.width {
            0 => 256,
            w => u16::from(w),
        }
    }

    fn real_height(&self) -> u16 {
        match self.height {
            0 => 256,
            h => u16::from(h),
        }
    }

    fn matches_dimensions(&self, width: u32, height: u32) -> bool {
        u32::from(self.real_width()) == width.min(256)
            && u32::from(self.real_height()) == height.min(256)
    }

    fn seek_to_start<R: Read + Seek>(&self, r: &mut R) -> ImageResult<()> {
        r.seek(SeekFrom::Start(u64::from(self.image_offset)))?;
        Ok(())
    }

    fn is_png<R: Read + Seek>(&self, r: &mut R) -> ImageResult<bool> {
        self.seek_to_start(r)?;

        // Read the first 8 bytes to sniff the image.
        let mut signature = [0u8; 8];
        r.read_exact(&mut signature)?;

        Ok(signature == PNG_SIGNATURE)
    }

    fn decoder<R: BufRead + Seek>(&self, mut r: R) -> ImageResult<InnerDecoder<R>> {
        let is_png = self.is_png(&mut r)?;
        self.seek_to_start(&mut r)?;

        if is_png {
            Ok(Png(Box::new(PngDecoder::new(r)?)))
        } else {
            Ok(Bmp(BmpDecoder::new_with_ico_format(r)?))
        }
    }
}

impl<R: BufRead + Seek> ImageDecoder for IcoDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        match self.inner_decoder {
            Bmp(ref decoder) => decoder.dimensions(),
            Png(ref decoder) => decoder.dimensions(),
        }
    }

    fn color_type(&self) -> ColorType {
        match self.inner_decoder {
            Bmp(ref decoder) => decoder.color_type(),
            Png(ref decoder) => decoder.color_type(),
        }
    }

    fn read_image(self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));
        match self.inner_decoder {
            Png(decoder) => {
                if self.selected_entry.image_length < PNG_SIGNATURE.len() as u32 {
                    return Err(DecoderError::PngShorterThanHeader.into());
                }

                // Check if the image dimensions match the ones in the image data.
                let (width, height) = decoder.dimensions();
                if !self.selected_entry.matches_dimensions(width, height) {
                    return Err(DecoderError::ImageEntryDimensionMismatch {
                        format: IcoEntryImageFormat::Png,
                        entry: (
                            self.selected_entry.real_width(),
                            self.selected_entry.real_height(),
                        ),
                        image: (width, height),
                    }
                    .into());
                }

                // Embedded PNG images can only be of the 32BPP RGBA format.
                // https://blogs.msdn.microsoft.com/oldnewthing/20101022-00/?p=12473/
                if decoder.color_type() != ColorType::Rgba8 {
                    return Err(DecoderError::PngNotRgba.into());
                }

                decoder.read_image(buf)
            }
            Bmp(mut decoder) => {
                let (width, height) = decoder.dimensions();
                if !self.selected_entry.matches_dimensions(width, height) {
                    return Err(DecoderError::ImageEntryDimensionMismatch {
                        format: IcoEntryImageFormat::Bmp,
                        entry: (
                            self.selected_entry.real_width(),
                            self.selected_entry.real_height(),
                        ),
                        image: (width, height),
                    }
                    .into());
                }

                // The ICO decoder needs an alpha channel to apply the AND mask.
                if decoder.color_type() != ColorType::Rgba8 {
                    return Err(ImageError::Unsupported(
                        UnsupportedError::from_format_and_kind(
                            ImageFormat::Bmp.into(),
                            UnsupportedErrorKind::Color(decoder.color_type().into()),
                        ),
                    ));
                }

                decoder.read_image_data(buf)?;

                let r = decoder.reader();
                let image_end = r.stream_position()?;
                let data_end = u64::from(self.selected_entry.image_offset)
                    + u64::from(self.selected_entry.image_length);

                let mask_row_bytes = width.div_ceil(32) * 4;
                let mask_length = u64::from(mask_row_bytes) * u64::from(height);

                // data_end should be image_end + the mask length (mask_row_bytes * height).
                // According to
                // https://devblogs.microsoft.com/oldnewthing/20101021-00/?p=12483
                // the mask is required, but according to Wikipedia
                // https://en.wikipedia.org/wiki/ICO_(file_format)
                // the mask is not required. Unfortunately, Wikipedia does not have a citation
                // for that claim, so we can't be sure which is correct.
                if data_end >= image_end + mask_length {
                    // If there's an AND mask following the image, read and apply it.
                    for y in 0..height {
                        let mut x = 0;
                        for _ in 0..mask_row_bytes {
                            // Apply the bits of each byte until we reach the end of the row.
                            let mask_byte = r.read_u8()?;
                            for bit in (0..8).rev() {
                                if x >= width {
                                    break;
                                }
                                if mask_byte & (1 << bit) != 0 {
                                    // Set alpha channel to transparent.
                                    buf[((height - y - 1) * width + x) as usize * 4 + 3] = 0;
                                }
                                x += 1;
                            }
                        }
                    }

                    Ok(())
                } else if data_end == image_end {
                    // accept images with no mask data
                    Ok(())
                } else {
                    Err(DecoderError::InvalidDataSize.into())
                }
            }
        }
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // Test if BMP images without alpha channel inside ICOs don't panic.
    // Because the test data is invalid decoding should produce an error.
    #[test]
    fn bmp_16_with_missing_alpha_channel() {
        let data = vec![
            0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x0e, 0x04, 0xc3, 0x7e, 0x00, 0x00, 0x00, 0x00,
            0x7c, 0x00, 0x00, 0x00, 0x0e, 0x00, 0x00, 0x00, 0xf8, 0xff, 0xff, 0xff, 0x01, 0x00,
            0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x8f, 0xf6, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x20, 0x66, 0x74, 0x83, 0x70, 0x61, 0x76, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
            0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xeb, 0x00, 0x9b, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4e, 0x47, 0x0d,
            0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x62, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x0c,
            0x00, 0x00, 0x00, 0xc3, 0x3f, 0x94, 0x61, 0xaa, 0x17, 0x4d, 0x8d, 0x79, 0x1d, 0x8b,
            0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x14, 0x2e, 0x28, 0x40, 0xe5, 0x9f,
            0x4b, 0x4d, 0xe9, 0x87, 0xd3, 0xda, 0xd6, 0x89, 0x81, 0xc5, 0xa4, 0xa1, 0x60, 0x98,
            0x31, 0xc7, 0x1d, 0xb6, 0x8f, 0x20, 0xc8, 0x3e, 0xee, 0xd8, 0xe4, 0x8f, 0xee, 0x7b,
            0x48, 0x9b, 0x88, 0x25, 0x13, 0xda, 0xa4, 0x13, 0xa4, 0x00, 0x00, 0x00, 0x00, 0x40,
            0x16, 0x01, 0xff, 0xff, 0xff, 0xff, 0xe9, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0xa3, 0x66, 0x64, 0x41, 0x54, 0xa3, 0xa3, 0x00, 0x00, 0x00, 0xb8, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xa3, 0x66, 0x64, 0x41, 0x54, 0xa3, 0xa3,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x8f, 0xf6, 0xff, 0xff,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x66, 0x74, 0x83, 0x70, 0x61, 0x76,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff,
            0xeb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x62, 0x49,
            0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00,
            0x00, 0x00, 0x00, 0xff, 0xff, 0x94, 0xc8, 0x00, 0x02, 0x0c, 0x00, 0xff, 0xff, 0xc6,
            0x84, 0x00, 0x2a, 0x75, 0x03, 0xa3, 0x05, 0xfb, 0xe1, 0x6e, 0xe8, 0x27, 0xd6, 0xd3,
            0x96, 0xc1, 0xe4, 0x30, 0x0c, 0x05, 0xb9, 0xa3, 0x8b, 0x29, 0xda, 0xa4, 0xf1, 0x4d,
            0xf3, 0xb2, 0x98, 0x2b, 0xe6, 0x93, 0x07, 0xf9, 0xca, 0x2b, 0xc2, 0x39, 0x20, 0xba,
            0x7c, 0xa0, 0xb1, 0x43, 0xe6, 0xf9, 0xdc, 0xd1, 0xc2, 0x52, 0xdc, 0x41, 0xc1, 0x2f,
            0x29, 0xf7, 0x46, 0x32, 0xda, 0x1b, 0x72, 0x8c, 0xe6, 0x2b, 0x01, 0xe5, 0x49, 0x21,
            0x89, 0x89, 0xe4, 0x3d, 0xa1, 0xdb, 0x3b, 0x4a, 0x0b, 0x52, 0x86, 0x52, 0x33, 0x9d,
            0xb2, 0xcf, 0x4a, 0x86, 0x53, 0xd7, 0xa9, 0x4b, 0xaf, 0x62, 0x06, 0x49, 0x53, 0x00,
            0xc3, 0x3f, 0x94, 0x61, 0xaa, 0x17, 0x4d, 0x8d, 0x79, 0x1d, 0x8b, 0x10, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x14, 0x2e, 0x28, 0x40, 0xe5, 0x9f, 0x4b, 0x4d, 0xe9,
            0x87, 0xd3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe7, 0xc5, 0x00,
            0x02, 0x00, 0x00, 0x00, 0x06, 0x00, 0x0b, 0x00, 0x50, 0x31, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x06, 0x76, 0x76, 0x01, 0x00, 0x00, 0x00, 0x76, 0x00,
            0x00, 0x23, 0x3f, 0x52, 0x41, 0x44, 0x49, 0x41, 0x4e, 0x43, 0x45, 0x61, 0x50, 0x35,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x4d, 0x47, 0x49, 0x46, 0x38, 0x37, 0x61, 0x05,
            0x50, 0x37, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc7, 0x37, 0x61,
        ];

        let decoder = IcoDecoder::new(std::io::Cursor::new(&data)).unwrap();
        let mut buf = vec![0; usize::try_from(decoder.total_bytes()).unwrap()];
        assert!(decoder.read_image(&mut buf).is_err());
    }
}
