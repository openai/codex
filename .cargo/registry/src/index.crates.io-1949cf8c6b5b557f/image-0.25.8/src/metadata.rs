//! Types describing image metadata
pub(crate) mod cicp;

use std::io::{Cursor, Read};

use byteorder_lite::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};

pub use self::cicp::{
    Cicp, CicpColorPrimaries, CicpMatrixCoefficients, CicpTransferCharacteristics, CicpTransform,
    CicpVideoFullRangeFlag,
};

/// Describes the transformations to be applied to the image.
/// Compatible with [Exif orientation](https://web.archive.org/web/20200412005226/https://www.impulseadventure.com/photo/exif-orientation.html).
///
/// Orientation is specified in the file's metadata, and is often written by cameras.
///
/// You can apply it to an image via [`DynamicImage::apply_orientation`](crate::DynamicImage::apply_orientation).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Orientation {
    /// Do not perform any transformations.
    NoTransforms,
    /// Rotate by 90 degrees clockwise.
    Rotate90,
    /// Rotate by 180 degrees. Can be performed in-place.
    Rotate180,
    /// Rotate by 270 degrees clockwise. Equivalent to rotating by 90 degrees counter-clockwise.
    Rotate270,
    /// Flip horizontally. Can be performed in-place.
    FlipHorizontal,
    /// Flip vertically. Can be performed in-place.
    FlipVertical,
    /// Rotate by 90 degrees clockwise and flip horizontally.
    Rotate90FlipH,
    /// Rotate by 270 degrees clockwise and flip horizontally.
    Rotate270FlipH,
}

impl Orientation {
    /// Converts from [Exif orientation](https://web.archive.org/web/20200412005226/https://www.impulseadventure.com/photo/exif-orientation.html)
    #[must_use]
    pub fn from_exif(exif_orientation: u8) -> Option<Self> {
        match exif_orientation {
            1 => Some(Self::NoTransforms),
            2 => Some(Self::FlipHorizontal),
            3 => Some(Self::Rotate180),
            4 => Some(Self::FlipVertical),
            5 => Some(Self::Rotate90FlipH),
            6 => Some(Self::Rotate90),
            7 => Some(Self::Rotate270FlipH),
            8 => Some(Self::Rotate270),
            0 | 9.. => None,
        }
    }

    /// Converts into [Exif orientation](https://web.archive.org/web/20200412005226/https://www.impulseadventure.com/photo/exif-orientation.html)
    #[must_use]
    pub fn to_exif(self) -> u8 {
        match self {
            Self::NoTransforms => 1,
            Self::FlipHorizontal => 2,
            Self::Rotate180 => 3,
            Self::FlipVertical => 4,
            Self::Rotate90FlipH => 5,
            Self::Rotate90 => 6,
            Self::Rotate270FlipH => 7,
            Self::Rotate270 => 8,
        }
    }

    /// Extracts the image orientation from a raw Exif chunk.
    ///
    /// You can obtain the Exif chunk using
    /// [ImageDecoder::exif_metadata](crate::ImageDecoder::exif_metadata).
    ///
    /// It is more convenient to use [ImageDecoder::orientation](crate::ImageDecoder::orientation)
    /// than to invoke this function.
    /// Only use this function if you extract and process the Exif chunk separately.
    #[must_use]
    pub fn from_exif_chunk(chunk: &[u8]) -> Option<Self> {
        Self::from_exif_chunk_inner(chunk).map(|res| res.0)
    }

    /// Extracts the image orientation from a raw Exif chunk and sets the orientation in the Exif chunk to `Orientation::NoTransforms`.
    /// This is useful if you want to apply the orientation yourself, and then encode the image with the rest of the Exif chunk intact.
    ///
    /// If the orientation data is not cleared from the Exif chunk after you apply the orientation data yourself,
    /// the image will end up being rotated once again by any software that correctly handles Exif, leading to an incorrect result.
    ///
    /// If the Exif value is present but invalid, `None` is returned and the Exif chunk is not modified.
    #[must_use]
    pub fn remove_from_exif_chunk(chunk: &mut [u8]) -> Option<Self> {
        if let Some((orientation, offset, endian)) = Self::from_exif_chunk_inner(chunk) {
            let mut writer = Cursor::new(chunk);
            writer.set_position(offset);
            let no_orientation: u16 = Self::NoTransforms.to_exif().into();
            match endian {
                ExifEndian::Big => writer.write_u16::<BigEndian>(no_orientation).unwrap(),
                ExifEndian::Little => writer.write_u16::<LittleEndian>(no_orientation).unwrap(),
            }
            Some(orientation)
        } else {
            None
        }
    }

    /// Returns the orientation, the offset in the Exif chunk where it was found, and Exif chunk endianness
    #[must_use]
    fn from_exif_chunk_inner(chunk: &[u8]) -> Option<(Self, u64, ExifEndian)> {
        let mut reader = Cursor::new(chunk);

        let mut magic = [0; 4];
        reader.read_exact(&mut magic).ok()?;

        match magic {
            [0x49, 0x49, 42, 0] => {
                return Self::locate_orientation_entry::<LittleEndian>(&mut reader)
                    .map(|(orient, offset)| (orient, offset, ExifEndian::Little));
            }
            [0x4d, 0x4d, 0, 42] => {
                return Self::locate_orientation_entry::<BigEndian>(&mut reader)
                    .map(|(orient, offset)| (orient, offset, ExifEndian::Big));
            }
            _ => {}
        }
        None
    }

    /// Extracted into a helper function to be generic over endianness
    fn locate_orientation_entry<B>(reader: &mut Cursor<&[u8]>) -> Option<(Self, u64)>
    where
        B: byteorder_lite::ByteOrder,
    {
        let ifd_offset = reader.read_u32::<B>().ok()?;
        reader.set_position(u64::from(ifd_offset));
        let entries = reader.read_u16::<B>().ok()?;
        for _ in 0..entries {
            let tag = reader.read_u16::<B>().ok()?;
            let format = reader.read_u16::<B>().ok()?;
            let count = reader.read_u32::<B>().ok()?;
            let value = reader.read_u16::<B>().ok()?;
            let _padding = reader.read_u16::<B>().ok()?;
            if tag == 0x112 && format == 3 && count == 1 {
                let offset = reader.position() - 4; // we've read 4 bytes (2 * u16) past the start of the value
                let orientation = Self::from_exif(value.min(255) as u8);
                return orientation.map(|orient| (orient, offset));
            }
        }
        // If we reached this point without returning early, there was no orientation
        None
    }
}

#[derive(Debug, Copy, Clone)]
enum ExifEndian {
    Big,
    Little,
}

#[cfg(all(test, feature = "jpeg"))]
mod tests {
    use crate::{codecs::jpeg::JpegDecoder, ImageDecoder as _};

    // This brings all the items from the parent module into scope,
    // so you can directly use `add` instead of `super::add`.
    use super::*;

    const TEST_IMAGE: &[u8] = include_bytes!("../tests/images/jpg/portrait_2.jpg");

    #[test] // This attribute marks the function as a test function.
    fn test_extraction_and_clearing() {
        let reader = Cursor::new(TEST_IMAGE);
        let mut decoder = JpegDecoder::new(reader).expect("Failed to decode test image");
        let mut exif_chunk = decoder
            .exif_metadata()
            .expect("Failed to extract Exif chunk")
            .expect("No Exif chunk found in test image");

        let orientation = Orientation::from_exif_chunk(&exif_chunk)
            .expect("Failed to extract orientation from Exif chunk");
        assert_eq!(orientation, Orientation::FlipHorizontal);

        let orientation = Orientation::remove_from_exif_chunk(&mut exif_chunk)
            .expect("Failed to remove orientation from Exif chunk");
        assert_eq!(orientation, Orientation::FlipHorizontal);
        // Now that the orientation has been cleared, any subsequent extractions should return NoTransforms
        let orientation = Orientation::from_exif_chunk(&exif_chunk)
            .expect("Failed to extract orientation from Exif chunk after clearing it");
        assert_eq!(orientation, Orientation::NoTransforms);
    }
}
