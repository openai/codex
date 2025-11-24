use crate::error::{UnsupportedError, UnsupportedErrorKind};
use crate::{ExtendedColorType, ImageError, ImageFormat, ImageResult};
use byteorder_lite::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

pub(crate) const ALPHA_BIT_MASK: u8 = 0b1111;
pub(crate) const SCREEN_ORIGIN_BIT_MASK: u8 = 0b10_0000;

pub(crate) enum ImageType {
    NoImageData = 0,
    /// Uncompressed images.
    RawColorMap = 1,
    RawTrueColor = 2,
    RawGrayScale = 3,
    /// Run length encoded images.
    RunColorMap = 9,
    RunTrueColor = 10,
    RunGrayScale = 11,
    Unknown,
}

impl ImageType {
    /// Create a new image type from a u8.
    pub(crate) fn new(img_type: u8) -> ImageType {
        match img_type {
            0 => ImageType::NoImageData,

            1 => ImageType::RawColorMap,
            2 => ImageType::RawTrueColor,
            3 => ImageType::RawGrayScale,

            9 => ImageType::RunColorMap,
            10 => ImageType::RunTrueColor,
            11 => ImageType::RunGrayScale,

            _ => ImageType::Unknown,
        }
    }

    /// Check if the image format uses colors as opposed to gray scale.
    pub(crate) fn is_color(&self) -> bool {
        matches! { *self,
            ImageType::RawColorMap
            | ImageType::RawTrueColor
            | ImageType::RunTrueColor
            | ImageType::RunColorMap
        }
    }

    /// Does the image use a color map.
    pub(crate) fn is_color_mapped(&self) -> bool {
        matches!(*self, Self::RawColorMap | Self::RunColorMap)
    }

    /// Is the image run length encoded.
    pub(crate) fn is_encoded(&self) -> bool {
        matches!(
            *self,
            Self::RunColorMap | Self::RunTrueColor | Self::RunGrayScale
        )
    }
}

/// Header used by TGA image files.
#[derive(Debug, Default)]
pub(crate) struct Header {
    pub(crate) id_length: u8,      // length of ID string
    pub(crate) map_type: u8,       // color map type
    pub(crate) image_type: u8,     // image type code
    pub(crate) map_origin: u16,    // starting index of map
    pub(crate) map_length: u16,    // length of map
    pub(crate) map_entry_size: u8, // size of map entries in bits
    pub(crate) x_origin: u16,      // x-origin of image
    pub(crate) y_origin: u16,      // y-origin of image
    pub(crate) image_width: u16,   // width of image
    pub(crate) image_height: u16,  // height of image
    pub(crate) pixel_depth: u8,    // bits per pixel
    pub(crate) image_desc: u8,     // image descriptor
}

impl Header {
    /// Load the header with values from pixel information.
    pub(crate) fn from_pixel_info(
        color_type: ExtendedColorType,
        width: u16,
        height: u16,
        use_rle: bool,
    ) -> ImageResult<Self> {
        let mut header = Self::default();

        if width > 0 && height > 0 {
            let (num_alpha_bits, other_channel_bits, image_type) = match (color_type, use_rle) {
                (ExtendedColorType::Rgba8, true) => (8, 24, ImageType::RunTrueColor),
                (ExtendedColorType::Rgb8, true) => (0, 24, ImageType::RunTrueColor),
                (ExtendedColorType::La8, true) => (8, 8, ImageType::RunGrayScale),
                (ExtendedColorType::L8, true) => (0, 8, ImageType::RunGrayScale),
                (ExtendedColorType::Rgba8, false) => (8, 24, ImageType::RawTrueColor),
                (ExtendedColorType::Rgb8, false) => (0, 24, ImageType::RawTrueColor),
                (ExtendedColorType::La8, false) => (8, 8, ImageType::RawGrayScale),
                (ExtendedColorType::L8, false) => (0, 8, ImageType::RawGrayScale),
                _ => {
                    return Err(ImageError::Unsupported(
                        UnsupportedError::from_format_and_kind(
                            ImageFormat::Tga.into(),
                            UnsupportedErrorKind::Color(color_type),
                        ),
                    ))
                }
            };

            header.image_type = image_type as u8;
            header.image_width = width;
            header.image_height = height;
            header.pixel_depth = num_alpha_bits + other_channel_bits;
            header.image_desc = num_alpha_bits & ALPHA_BIT_MASK;
            header.image_desc |= SCREEN_ORIGIN_BIT_MASK; // Upper left origin.
        }

        Ok(header)
    }

    /// Load the header with values from the reader.
    pub(crate) fn from_reader(r: &mut dyn Read) -> ImageResult<Self> {
        Ok(Self {
            id_length: r.read_u8()?,
            map_type: r.read_u8()?,
            image_type: r.read_u8()?,
            map_origin: r.read_u16::<LittleEndian>()?,
            map_length: r.read_u16::<LittleEndian>()?,
            map_entry_size: r.read_u8()?,
            x_origin: r.read_u16::<LittleEndian>()?,
            y_origin: r.read_u16::<LittleEndian>()?,
            image_width: r.read_u16::<LittleEndian>()?,
            image_height: r.read_u16::<LittleEndian>()?,
            pixel_depth: r.read_u8()?,
            image_desc: r.read_u8()?,
        })
    }

    /// Write out the header values.
    pub(crate) fn write_to(&self, w: &mut dyn Write) -> ImageResult<()> {
        w.write_u8(self.id_length)?;
        w.write_u8(self.map_type)?;
        w.write_u8(self.image_type)?;
        w.write_u16::<LittleEndian>(self.map_origin)?;
        w.write_u16::<LittleEndian>(self.map_length)?;
        w.write_u8(self.map_entry_size)?;
        w.write_u16::<LittleEndian>(self.x_origin)?;
        w.write_u16::<LittleEndian>(self.y_origin)?;
        w.write_u16::<LittleEndian>(self.image_width)?;
        w.write_u16::<LittleEndian>(self.image_height)?;
        w.write_u8(self.pixel_depth)?;
        w.write_u8(self.image_desc)?;
        Ok(())
    }
}
