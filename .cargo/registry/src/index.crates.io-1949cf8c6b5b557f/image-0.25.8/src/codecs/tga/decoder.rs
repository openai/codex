use super::header::{Header, ImageType, ALPHA_BIT_MASK};
use crate::error::DecodingError;
use crate::io::ReadExt;
use crate::utils::vec_try_with_capacity;
use crate::{
    color::{ColorType, ExtendedColorType},
    error::{ImageError, ImageResult, UnsupportedError, UnsupportedErrorKind},
    ImageDecoder, ImageFormat,
};
use byteorder_lite::ReadBytesExt;
use std::io::{self, Read};

struct ColorMap {
    /// sizes in bytes
    start_offset: usize,
    entry_size: usize,
    bytes: Vec<u8>,
}

impl ColorMap {
    /// Get one entry from the color map
    pub(crate) fn get(&self, index: usize) -> Option<&[u8]> {
        // TODO: Should we actually be *subtracting* start_offset from the index here?
        let entry = self.start_offset + self.entry_size * index;
        self.bytes.get(entry..entry + self.entry_size)
    }
}

/// The representation of a TGA decoder
pub struct TgaDecoder<R> {
    r: R,

    width: usize,
    height: usize,

    // The number of bytes in the raw input data for each pixel. If a color map is used, this is the
    // number of bytes for each color map index.
    raw_bytes_per_pixel: usize,

    image_type: ImageType,
    color_type: ColorType,
    original_color_type: Option<ExtendedColorType>,

    header: Header,
    color_map: Option<ColorMap>,
}

#[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
enum TgaOrientation {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

impl TgaOrientation {
    fn from_image_desc_byte(value: u8) -> Self {
        // Set bits 4 and 5 indicates direction, if bit 4 is set then pixel order right -> left,
        // when bit 5 is set it indicates rows top -> bottom direction.
        // Sources:
        // https://en.wikipedia.org/wiki/Truevision_TGA ; Image specification (field 5)
        if value & (1u8 << 4) == 0 {
            // Left -> Right
            if value & (1u8 << 5) == 0 {
                TgaOrientation::BottomLeft
            } else {
                TgaOrientation::TopLeft
            }
        } else {
            // Right -> Left
            if value & (1u8 << 5) == 0 {
                TgaOrientation::BottomRight
            } else {
                TgaOrientation::TopRight
            }
        }
    }
}

impl<R: Read> TgaDecoder<R> {
    /// Create a new decoder that decodes from the stream `r`
    pub fn new(mut r: R) -> ImageResult<TgaDecoder<R>> {
        // Read header
        let header = Header::from_reader(&mut r)?;
        let image_type = ImageType::new(header.image_type);
        let width = header.image_width as usize;
        let height = header.image_height as usize;
        let raw_bytes_per_pixel = (header.pixel_depth as usize).div_ceil(8);
        let num_alpha_bits = header.image_desc & ALPHA_BIT_MASK;

        // Validate header
        if ![8, 16, 24, 32].contains(&header.pixel_depth) || ![0, 8].contains(&num_alpha_bits) {
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Tga.into(),
                    UnsupportedErrorKind::Color(ExtendedColorType::Unknown(header.pixel_depth)),
                ),
            ));
        }
        if image_type.is_color_mapped() {
            if header.map_type != 1 {
                return Err(ImageError::Decoding(DecodingError::new(
                    ImageFormat::Tga.into(),
                    "Color map type must be 1 for color mapped images",
                )));
            } else if ![8, 16].contains(&header.pixel_depth) {
                return Err(ImageError::Decoding(DecodingError::new(
                    ImageFormat::Tga.into(),
                    "Color map must use 1 or 2 byte indexes",
                )));
            } else if header.pixel_depth > header.map_entry_size {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Tga.into(),
                        UnsupportedErrorKind::GenericFeature(
                            "Indices larger than pixel values".into(),
                        ),
                    ),
                ));
            }
        }

        // TODO: validate the rest of the fields in the header.

        // Read image ID (and ignore it)
        let mut tmp = [0u8; 256];
        r.read_exact(&mut tmp[0..header.id_length as usize])?;

        // Read color map
        let mut color_map = None;
        if header.map_type == 1 {
            let entry_size = (header.map_entry_size as usize).div_ceil(8);
            if ![2, 3, 4].contains(&entry_size) {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Tga.into(),
                        UnsupportedErrorKind::GenericFeature(
                            "Unsupported color map entry size".into(),
                        ),
                    ),
                ));
            }

            let mut bytes = Vec::new();
            r.read_exact_vec(&mut bytes, entry_size * header.map_length as usize)?;

            // Color maps are technically allowed in non-color-mapped images, so check that we
            // actually need the color map before storing it.
            if image_type.is_color_mapped() {
                color_map = Some(ColorMap {
                    entry_size,
                    start_offset: header.map_origin as usize,
                    bytes,
                });
            }
        }

        // Compute output pixel depth
        let total_pixel_bits = if header.map_type == 1 {
            header.map_entry_size
        } else {
            header.pixel_depth
        };
        let num_other_bits = total_pixel_bits
            .checked_sub(num_alpha_bits)
            .ok_or_else(|| {
                ImageError::Decoding(DecodingError::new(
                    ImageFormat::Tga.into(),
                    "More alpha bits than pixel bits",
                ))
            })?;

        // Determine color type
        let color_type;
        let mut original_color_type = None;
        match (num_alpha_bits, num_other_bits, image_type.is_color()) {
            // really, the encoding is BGR and BGRA, this is fixed up with
            // `TgaDecoder::reverse_encoding`.
            (0, 32, true) => color_type = ColorType::Rgba8,
            (8, 24, true) => color_type = ColorType::Rgba8,
            (0, 24, true) => color_type = ColorType::Rgb8,
            (8, 8, false) => color_type = ColorType::La8,
            (0, 8, false) => color_type = ColorType::L8,
            (8, 0, false) => {
                // alpha-only image is treated as L8
                color_type = ColorType::L8;
                original_color_type = Some(ExtendedColorType::A8);
            }
            _ => {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Tga.into(),
                        UnsupportedErrorKind::Color(ExtendedColorType::Unknown(header.pixel_depth)),
                    ),
                ))
            }
        }

        Ok(TgaDecoder {
            r,

            width,
            height,
            raw_bytes_per_pixel,

            image_type,
            color_type,
            original_color_type,

            header,
            color_map,
        })
    }

    /// Reads a run length encoded data for given number of bytes
    fn read_encoded_data(&mut self, buf: &mut [u8]) -> io::Result<()> {
        assert!(self.raw_bytes_per_pixel <= 4);
        let mut repeat_buf = [0; 4];
        let repeat_buf = &mut repeat_buf[..self.raw_bytes_per_pixel];

        let mut index = 0;
        while index < buf.len() {
            let run_packet = self.r.read_u8()?;
            // If the highest bit in `run_packet` is set, then we repeat pixels
            //
            // Note: the TGA format adds 1 to both counts because having a count
            // of 0 would be pointless.
            if (run_packet & 0x80) != 0 {
                // high bit set, so we will repeat the data
                let repeat_count = ((run_packet & !0x80) + 1) as usize;
                self.r.read_exact(repeat_buf)?;

                for chunk in buf[index..]
                    .chunks_exact_mut(self.raw_bytes_per_pixel)
                    .take(repeat_count)
                {
                    chunk.copy_from_slice(repeat_buf);
                }
                index += repeat_count * self.raw_bytes_per_pixel;
            } else {
                // not set, so `run_packet+1` is the number of non-encoded pixels
                let num_raw_bytes =
                    ((run_packet + 1) as usize * self.raw_bytes_per_pixel).min(buf.len() - index);

                self.r.read_exact(&mut buf[index..][..num_raw_bytes])?;
                index += num_raw_bytes;
            }
        }

        Ok(())
    }

    /// Expands indices into its mapped color
    fn expand_color_map(
        &self,
        input: &[u8],
        output: &mut [u8],
        color_map: &ColorMap,
    ) -> ImageResult<()> {
        if self.raw_bytes_per_pixel == 1 {
            for (&index, chunk) in input
                .iter()
                .zip(output.chunks_exact_mut(color_map.entry_size))
            {
                if let Some(color) = color_map.get(index as usize) {
                    chunk.copy_from_slice(color);
                } else {
                    return Err(ImageError::Decoding(DecodingError::new(
                        ImageFormat::Tga.into(),
                        "Invalid color map index",
                    )));
                }
            }
        } else if self.raw_bytes_per_pixel == 2 {
            for (index, chunk) in input
                .chunks_exact(2)
                .zip(output.chunks_exact_mut(color_map.entry_size))
            {
                let index = u16::from_le_bytes(index.try_into().unwrap());
                if let Some(color) = color_map.get(index as usize) {
                    chunk.copy_from_slice(color);
                } else {
                    return Err(ImageError::Decoding(DecodingError::new(
                        ImageFormat::Tga.into(),
                        "Invalid color map index",
                    )));
                }
            }
        } else {
            unreachable!("Supported bytes_per_pixel values are checked in TgaDecoder::new");
        }

        Ok(())
    }

    /// Reverse from BGR encoding to RGB encoding
    ///
    /// TGA files are stored in the BGRA encoding. This function swaps
    /// the blue and red bytes in the `pixels` array.
    fn reverse_encoding_in_output(&mut self, pixels: &mut [u8]) {
        // We only need to reverse the encoding of color images
        match self.color_type {
            ColorType::Rgb8 | ColorType::Rgba8 => {
                for chunk in pixels.chunks_mut(self.color_type.bytes_per_pixel().into()) {
                    chunk.swap(0, 2);
                }
            }
            _ => {}
        }
    }

    /// Change image orientation depending on the flags set
    fn fixup_orientation(&mut self, pixels: &mut [u8]) {
        let orientation = TgaOrientation::from_image_desc_byte(self.header.image_desc);

        // Flip image if bottom->top direction
        if (orientation == TgaOrientation::BottomLeft || orientation == TgaOrientation::BottomRight)
            && self.height > 1
        {
            let row_stride = self.width * self.raw_bytes_per_pixel;

            let (left_part, right_part) = pixels.split_at_mut(self.height / 2 * row_stride);

            for (src, dst) in left_part
                .chunks_exact_mut(row_stride)
                .zip(right_part.chunks_exact_mut(row_stride).rev())
            {
                for (src, dst) in src.iter_mut().zip(dst.iter_mut()) {
                    std::mem::swap(src, dst);
                }
            }
        }

        // Flop image if right->left direction
        if (orientation == TgaOrientation::BottomRight || orientation == TgaOrientation::TopRight)
            && self.width > 1
        {
            for row in pixels.chunks_exact_mut(self.width * self.raw_bytes_per_pixel) {
                let (left_part, right_part) =
                    row.split_at_mut(self.width / 2 * self.raw_bytes_per_pixel);
                for (src, dst) in left_part
                    .chunks_exact_mut(self.raw_bytes_per_pixel)
                    .zip(right_part.chunks_exact_mut(self.raw_bytes_per_pixel).rev())
                {
                    for (src, dst) in src.iter_mut().zip(dst.iter_mut()) {
                        std::mem::swap(dst, src);
                    }
                }
            }
        }
    }
}

impl<R: Read> ImageDecoder for TgaDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        (self.width as u32, self.height as u32)
    }

    fn color_type(&self) -> ColorType {
        self.color_type
    }

    fn original_color_type(&self) -> ExtendedColorType {
        self.original_color_type
            .unwrap_or_else(|| self.color_type().into())
    }

    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));

        // Decode the raw data
        //
        // We have already checked in `TgaDecoder::new` that the indices take less space than the
        // pixels they encode, so it is safe to read the raw data into `buf`.
        let num_raw_bytes = self.width * self.height * self.raw_bytes_per_pixel;
        if self.image_type.is_encoded() {
            self.read_encoded_data(&mut buf[..num_raw_bytes])?;
        } else {
            self.r.read_exact(&mut buf[..num_raw_bytes])?;
        }

        self.fixup_orientation(buf);

        // Expand the indices using the color map if necessary
        if let Some(ref color_map) = self.color_map {
            // This allocation could be avoided by expanding each row (or block of pixels) as it is
            // read, or by doing the color map expansion in-place. But those may be more effort than
            // it is worth.
            let mut rawbuf = vec_try_with_capacity(num_raw_bytes)?;
            rawbuf.extend_from_slice(&buf[..num_raw_bytes]);

            self.expand_color_map(&rawbuf, buf, color_map)?;
        }

        self.reverse_encoding_in_output(buf);

        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}
