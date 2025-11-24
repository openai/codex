//!  Decoding of DDS images
//!
//!  DDS (DirectDraw Surface) is a container format for storing DXT (S3TC) compressed images.
//!
//!  # Related Links
//!  * <https://docs.microsoft.com/en-us/windows/win32/direct3ddds/dx-graphics-dds-pguide> - Description of the DDS format.

use std::io::Read;
use std::{error, fmt};

use byteorder_lite::{LittleEndian, ReadBytesExt};

#[allow(deprecated)]
use crate::codecs::dxt::{DxtDecoder, DxtVariant};
use crate::color::ColorType;
use crate::error::{
    DecodingError, ImageError, ImageFormatHint, ImageResult, UnsupportedError, UnsupportedErrorKind,
};
use crate::{ImageDecoder, ImageFormat};

/// Errors that can occur during decoding and parsing a DDS image
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[allow(clippy::enum_variant_names)]
enum DecoderError {
    /// Wrong DDS channel width
    PixelFormatSizeInvalid(u32),
    /// Wrong DDS header size
    HeaderSizeInvalid(u32),
    /// Wrong DDS header flags
    HeaderFlagsInvalid(u32),

    /// Invalid DXGI format in DX10 header
    DxgiFormatInvalid(u32),
    /// Invalid resource dimension
    ResourceDimensionInvalid(u32),
    /// Invalid flags in DX10 header
    Dx10FlagsInvalid(u32),
    /// Invalid array size in DX10 header
    Dx10ArraySizeInvalid(u32),

    /// DDS "DDS " signature invalid or missing
    DdsSignatureInvalid,
}

impl fmt::Display for DecoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecoderError::PixelFormatSizeInvalid(s) => {
                f.write_fmt(format_args!("Invalid DDS PixelFormat size: {s}"))
            }
            DecoderError::HeaderSizeInvalid(s) => {
                f.write_fmt(format_args!("Invalid DDS header size: {s}"))
            }
            DecoderError::HeaderFlagsInvalid(fs) => {
                f.write_fmt(format_args!("Invalid DDS header flags: {fs:#010X}"))
            }
            DecoderError::DxgiFormatInvalid(df) => {
                f.write_fmt(format_args!("Invalid DDS DXGI format: {df}"))
            }
            DecoderError::ResourceDimensionInvalid(d) => {
                f.write_fmt(format_args!("Invalid DDS resource dimension: {d}"))
            }
            DecoderError::Dx10FlagsInvalid(fs) => {
                f.write_fmt(format_args!("Invalid DDS DX10 header flags: {fs:#010X}"))
            }
            DecoderError::Dx10ArraySizeInvalid(s) => {
                f.write_fmt(format_args!("Invalid DDS DX10 array size: {s}"))
            }
            DecoderError::DdsSignatureInvalid => f.write_str("DDS signature not found"),
        }
    }
}

impl From<DecoderError> for ImageError {
    fn from(e: DecoderError) -> ImageError {
        ImageError::Decoding(DecodingError::new(ImageFormat::Dds.into(), e))
    }
}

impl error::Error for DecoderError {}

/// Header used by DDS image files
#[derive(Debug)]
struct Header {
    _flags: u32,
    height: u32,
    width: u32,
    _pitch_or_linear_size: u32,
    _depth: u32,
    _mipmap_count: u32,
    pixel_format: PixelFormat,
    _caps: u32,
    _caps2: u32,
}

/// Extended DX10 header used by some DDS image files
#[derive(Debug)]
struct DX10Header {
    dxgi_format: u32,
    resource_dimension: u32,
    misc_flag: u32,
    array_size: u32,
    misc_flags_2: u32,
}

/// DDS pixel format
#[derive(Debug)]
struct PixelFormat {
    flags: u32,
    fourcc: [u8; 4],
    _rgb_bit_count: u32,
    _r_bit_mask: u32,
    _g_bit_mask: u32,
    _b_bit_mask: u32,
    _a_bit_mask: u32,
}

impl PixelFormat {
    fn from_reader(r: &mut dyn Read) -> ImageResult<Self> {
        let size = r.read_u32::<LittleEndian>()?;
        if size != 32 {
            return Err(DecoderError::PixelFormatSizeInvalid(size).into());
        }

        Ok(Self {
            flags: r.read_u32::<LittleEndian>()?,
            fourcc: {
                let mut v = [0; 4];
                r.read_exact(&mut v)?;
                v
            },
            _rgb_bit_count: r.read_u32::<LittleEndian>()?,
            _r_bit_mask: r.read_u32::<LittleEndian>()?,
            _g_bit_mask: r.read_u32::<LittleEndian>()?,
            _b_bit_mask: r.read_u32::<LittleEndian>()?,
            _a_bit_mask: r.read_u32::<LittleEndian>()?,
        })
    }
}

impl Header {
    fn from_reader(r: &mut dyn Read) -> ImageResult<Self> {
        let size = r.read_u32::<LittleEndian>()?;
        if size != 124 {
            return Err(DecoderError::HeaderSizeInvalid(size).into());
        }

        const REQUIRED_FLAGS: u32 = 0x1 | 0x2 | 0x4 | 0x1000;
        const VALID_FLAGS: u32 = 0x1 | 0x2 | 0x4 | 0x8 | 0x1000 | 0x20000 | 0x80000 | 0x0080_0000;
        let flags = r.read_u32::<LittleEndian>()?;
        if flags & (REQUIRED_FLAGS | !VALID_FLAGS) != REQUIRED_FLAGS {
            return Err(DecoderError::HeaderFlagsInvalid(flags).into());
        }

        let height = r.read_u32::<LittleEndian>()?;
        let width = r.read_u32::<LittleEndian>()?;
        let pitch_or_linear_size = r.read_u32::<LittleEndian>()?;
        let depth = r.read_u32::<LittleEndian>()?;
        let mipmap_count = r.read_u32::<LittleEndian>()?;
        // Skip `dwReserved1`
        {
            let mut skipped = [0; 4 * 11];
            r.read_exact(&mut skipped)?;
        }
        let pixel_format = PixelFormat::from_reader(r)?;
        let caps = r.read_u32::<LittleEndian>()?;
        let caps2 = r.read_u32::<LittleEndian>()?;
        // Skip `dwCaps3`, `dwCaps4`, `dwReserved2` (unused)
        {
            let mut skipped = [0; 4 + 4 + 4];
            r.read_exact(&mut skipped)?;
        }

        Ok(Self {
            _flags: flags,
            height,
            width,
            _pitch_or_linear_size: pitch_or_linear_size,
            _depth: depth,
            _mipmap_count: mipmap_count,
            pixel_format,
            _caps: caps,
            _caps2: caps2,
        })
    }
}

impl DX10Header {
    fn from_reader(r: &mut dyn Read) -> ImageResult<Self> {
        let dxgi_format = r.read_u32::<LittleEndian>()?;
        let resource_dimension = r.read_u32::<LittleEndian>()?;
        let misc_flag = r.read_u32::<LittleEndian>()?;
        let array_size = r.read_u32::<LittleEndian>()?;
        let misc_flags_2 = r.read_u32::<LittleEndian>()?;

        let dx10_header = Self {
            dxgi_format,
            resource_dimension,
            misc_flag,
            array_size,
            misc_flags_2,
        };
        dx10_header.validate()?;

        Ok(dx10_header)
    }

    fn validate(&self) -> Result<(), ImageError> {
        // Note: see https://docs.microsoft.com/en-us/windows/win32/direct3ddds/dds-header-dxt10 for info on valid values
        if self.dxgi_format > 132 {
            // Invalid format
            return Err(DecoderError::DxgiFormatInvalid(self.dxgi_format).into());
        }

        if self.resource_dimension < 2 || self.resource_dimension > 4 {
            // Invalid dimension
            // Only 1D (2), 2D (3) and 3D (4) resource dimensions are allowed
            return Err(DecoderError::ResourceDimensionInvalid(self.resource_dimension).into());
        }

        if self.misc_flag != 0x0 && self.misc_flag != 0x4 {
            // Invalid flag
            // Only no (0x0) and DDS_RESOURCE_MISC_TEXTURECUBE (0x4) flags are allowed
            return Err(DecoderError::Dx10FlagsInvalid(self.misc_flag).into());
        }

        if self.resource_dimension == 4 && self.array_size != 1 {
            // Invalid array size
            // 3D textures (resource dimension == 4) must have an array size of 1
            return Err(DecoderError::Dx10ArraySizeInvalid(self.array_size).into());
        }

        if self.misc_flags_2 > 0x4 {
            // Invalid alpha flags
            return Err(DecoderError::Dx10FlagsInvalid(self.misc_flags_2).into());
        }

        Ok(())
    }
}

/// The representation of a DDS decoder
pub struct DdsDecoder<R: Read> {
    #[allow(deprecated)]
    inner: DxtDecoder<R>,
}

impl<R: Read> DdsDecoder<R> {
    /// Create a new decoder that decodes from the stream `r`
    pub fn new(mut r: R) -> ImageResult<Self> {
        let mut magic = [0; 4];
        r.read_exact(&mut magic)?;
        if magic != b"DDS "[..] {
            return Err(DecoderError::DdsSignatureInvalid.into());
        }

        let header = Header::from_reader(&mut r)?;

        if header.pixel_format.flags & 0x4 != 0 {
            #[allow(deprecated)]
            let variant = match &header.pixel_format.fourcc {
                b"DXT1" => DxtVariant::DXT1,
                b"DXT3" => DxtVariant::DXT3,
                b"DXT5" => DxtVariant::DXT5,
                b"DX10" => {
                    let dx10_header = DX10Header::from_reader(&mut r)?;
                    // Format equivalents were taken from https://docs.microsoft.com/en-us/windows/win32/direct3d11/texture-block-compression-in-direct3d-11
                    // The enum integer values were taken from https://docs.microsoft.com/en-us/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format
                    // DXT1 represents the different BC1 variants, DTX3 represents the different BC2 variants and DTX5 represents the different BC3 variants
                    match dx10_header.dxgi_format {
                        70..=72 => DxtVariant::DXT1, // DXGI_FORMAT_BC1_TYPELESS, DXGI_FORMAT_BC1_UNORM or DXGI_FORMAT_BC1_UNORM_SRGB
                        73..=75 => DxtVariant::DXT3, // DXGI_FORMAT_BC2_TYPELESS, DXGI_FORMAT_BC2_UNORM or DXGI_FORMAT_BC2_UNORM_SRGB
                        76..=78 => DxtVariant::DXT5, // DXGI_FORMAT_BC3_TYPELESS, DXGI_FORMAT_BC3_UNORM or DXGI_FORMAT_BC3_UNORM_SRGB
                        _ => {
                            return Err(ImageError::Unsupported(
                                UnsupportedError::from_format_and_kind(
                                    ImageFormat::Dds.into(),
                                    UnsupportedErrorKind::GenericFeature(format!(
                                        "DDS DXGI Format {}",
                                        dx10_header.dxgi_format
                                    )),
                                ),
                            ))
                        }
                    }
                }
                fourcc => {
                    return Err(ImageError::Unsupported(
                        UnsupportedError::from_format_and_kind(
                            ImageFormat::Dds.into(),
                            UnsupportedErrorKind::GenericFeature(format!("DDS FourCC {fourcc:?}")),
                        ),
                    ))
                }
            };

            #[allow(deprecated)]
            let bytes_per_pixel = variant.color_type().bytes_per_pixel();

            if crate::utils::check_dimension_overflow(header.width, header.height, bytes_per_pixel)
            {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Dds.into(),
                        UnsupportedErrorKind::GenericFeature(format!(
                            "Image dimensions ({}x{}) are too large",
                            header.width, header.height
                        )),
                    ),
                ));
            }

            #[allow(deprecated)]
            let inner = DxtDecoder::new(r, header.width, header.height, variant)?;
            Ok(Self { inner })
        } else {
            // For now, supports only DXT variants
            Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Dds.into(),
                    UnsupportedErrorKind::Format(ImageFormatHint::Name("DDS".to_string())),
                ),
            ))
        }
    }
}

impl<R: Read> ImageDecoder for DdsDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        self.inner.dimensions()
    }

    fn color_type(&self) -> ColorType {
        self.inner.color_type()
    }

    fn read_image(self, buf: &mut [u8]) -> ImageResult<()> {
        self.inner.read_image(buf)
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn dimension_overflow() {
        // A DXT1 header set to 0xFFFF_FFFC width and height (the highest u32%4 == 0)
        let header = [
            0x44, 0x44, 0x53, 0x20, 0x7C, 0x0, 0x0, 0x0, 0x7, 0x10, 0x8, 0x0, 0xFC, 0xFF, 0xFF,
            0xFF, 0xFC, 0xFF, 0xFF, 0xFF, 0x0, 0xC0, 0x12, 0x0, 0x0, 0x0, 0x0, 0x0, 0x1, 0x0, 0x0,
            0x0, 0x49, 0x4D, 0x41, 0x47, 0x45, 0x4D, 0x41, 0x47, 0x49, 0x43, 0x4B, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x20, 0x0, 0x0, 0x0,
            0x4, 0x0, 0x0, 0x0, 0x44, 0x58, 0x54, 0x31, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x10, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
        ];

        assert!(DdsDecoder::new(&header[..]).is_err());
    }
}
