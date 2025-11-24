use byteorder_lite::{LittleEndian, WriteBytesExt};
use std::borrow::Cow;
use std::io::{self, Write};

use crate::codecs::png::PngEncoder;
use crate::error::{ImageError, ImageResult, ParameterError, ParameterErrorKind};
use crate::{ExtendedColorType, ImageEncoder};

// Enum value indicating an ICO image (as opposed to a CUR image):
const ICO_IMAGE_TYPE: u16 = 1;
// The length of an ICO file ICONDIR structure, in bytes:
const ICO_ICONDIR_SIZE: u32 = 6;
// The length of an ICO file DIRENTRY structure, in bytes:
const ICO_DIRENTRY_SIZE: u32 = 16;

/// ICO encoder
pub struct IcoEncoder<W: Write> {
    w: W,
}

/// An ICO image entry
pub struct IcoFrame<'a> {
    // Pre-encoded PNG or BMP
    encoded_image: Cow<'a, [u8]>,
    // Stored as `0 => 256, n => n`
    width: u8,
    // Stored as `0 => 256, n => n`
    height: u8,
    color_type: ExtendedColorType,
}

impl<'a> IcoFrame<'a> {
    /// Construct a new `IcoFrame` using a pre-encoded PNG or BMP
    ///
    /// The `width` and `height` must be between 1 and 256 (inclusive).
    pub fn with_encoded(
        encoded_image: impl Into<Cow<'a, [u8]>>,
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<Self> {
        let encoded_image = encoded_image.into();

        if !(1..=256).contains(&width) {
            return Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::Generic(format!(
                    "the image width must be `1..=256`, instead width {width} was provided",
                )),
            )));
        }

        if !(1..=256).contains(&height) {
            return Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::Generic(format!(
                    "the image height must be `1..=256`, instead height {height} was provided",
                )),
            )));
        }

        Ok(Self {
            encoded_image,
            width: width as u8,
            height: height as u8,
            color_type,
        })
    }

    /// Construct a new `IcoFrame` by encoding `buf` as a PNG
    ///
    /// The `width` and `height` must be between 1 and 256 (inclusive)
    pub fn as_png(
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<Self> {
        let mut image_data: Vec<u8> = Vec::new();
        PngEncoder::new(&mut image_data).write_image(buf, width, height, color_type)?;

        let frame = Self::with_encoded(image_data, width, height, color_type)?;
        Ok(frame)
    }
}

impl<W: Write> IcoEncoder<W> {
    /// Create a new encoder that writes its output to ```w```.
    pub fn new(w: W) -> IcoEncoder<W> {
        IcoEncoder { w }
    }

    /// Takes some [`IcoFrame`]s and encodes them into an ICO.
    ///
    /// `images` is a list of images, usually ordered by dimension, which
    /// must be between 1 and 65535 (inclusive) in length.
    pub fn encode_images(mut self, images: &[IcoFrame<'_>]) -> ImageResult<()> {
        if !(1..=usize::from(u16::MAX)).contains(&images.len()) {
            return Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::Generic(format!(
                    "the number of images must be `1..=u16::MAX`, instead {} images were provided",
                    images.len(),
                )),
            )));
        }
        let num_images = images.len() as u16;

        let mut offset = ICO_ICONDIR_SIZE + (ICO_DIRENTRY_SIZE * (images.len() as u32));
        write_icondir(&mut self.w, num_images)?;
        for image in images {
            write_direntry(
                &mut self.w,
                image.width,
                image.height,
                image.color_type,
                offset,
                image.encoded_image.len() as u32,
            )?;

            offset += image.encoded_image.len() as u32;
        }
        for image in images {
            self.w.write_all(&image.encoded_image)?;
        }
        Ok(())
    }
}

impl<W: Write> ImageEncoder for IcoEncoder<W> {
    /// Write an ICO image with the specified width, height, and color type.
    ///
    /// For color types with 16-bit per channel or larger, the contents of `buf` should be in
    /// native endian.
    ///
    /// WARNING: In image 0.23.14 and earlier this method erroneously expected buf to be in big endian.
    #[track_caller]
    fn write_image(
        self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        let expected_buffer_len = color_type.buffer_size(width, height);
        assert_eq!(
            expected_buffer_len,
            buf.len() as u64,
            "Invalid buffer length: expected {expected_buffer_len} got {} for {width}x{height} image",
            buf.len(),
        );

        let image = IcoFrame::as_png(buf, width, height, color_type)?;
        self.encode_images(&[image])
    }
}

fn write_icondir<W: Write>(w: &mut W, num_images: u16) -> io::Result<()> {
    // Reserved field (must be zero):
    w.write_u16::<LittleEndian>(0)?;
    // Image type (ICO or CUR):
    w.write_u16::<LittleEndian>(ICO_IMAGE_TYPE)?;
    // Number of images in the file:
    w.write_u16::<LittleEndian>(num_images)?;
    Ok(())
}

fn write_direntry<W: Write>(
    w: &mut W,
    width: u8,
    height: u8,
    color: ExtendedColorType,
    data_start: u32,
    data_size: u32,
) -> io::Result<()> {
    // Image dimensions:
    w.write_u8(width)?;
    w.write_u8(height)?;
    // Number of colors in palette (or zero for no palette):
    w.write_u8(0)?;
    // Reserved field (must be zero):
    w.write_u8(0)?;
    // Color planes:
    w.write_u16::<LittleEndian>(0)?;
    // Bits per pixel:
    w.write_u16::<LittleEndian>(color.bits_per_pixel())?;
    // Image data size, in bytes:
    w.write_u32::<LittleEndian>(data_size)?;
    // Image data offset, in bytes:
    w.write_u32::<LittleEndian>(data_start)?;
    Ok(())
}
