//! Decoding and Encoding of TIFF Images
//!
//! TIFF (Tagged Image File Format) is a versatile image format that supports
//! lossless and lossy compression.
//!
//! # Related Links
//! * <http://partners.adobe.com/public/developer/tiff/index.html> - The TIFF specification
use std::io::{self, BufRead, Cursor, Read, Seek, Write};
use std::marker::PhantomData;
use std::mem;

use tiff::decoder::{Decoder, DecodingResult};
use tiff::tags::Tag;

use crate::color::{ColorType, ExtendedColorType};
use crate::error::{
    DecodingError, EncodingError, ImageError, ImageResult, LimitError, LimitErrorKind,
    ParameterError, ParameterErrorKind, UnsupportedError, UnsupportedErrorKind,
};
use crate::metadata::Orientation;
use crate::{utils, ImageDecoder, ImageEncoder, ImageFormat};

/// Decoder for TIFF images.
pub struct TiffDecoder<R>
where
    R: BufRead + Seek,
{
    dimensions: (u32, u32),
    color_type: ColorType,
    original_color_type: ExtendedColorType,

    // We only use an Option here so we can call with_limits on the decoder without moving.
    inner: Option<Decoder<R>>,
}

impl<R> TiffDecoder<R>
where
    R: BufRead + Seek,
{
    /// Create a new `TiffDecoder`.
    pub fn new(r: R) -> Result<TiffDecoder<R>, ImageError> {
        let mut inner = Decoder::new(r).map_err(ImageError::from_tiff_decode)?;

        let dimensions = inner.dimensions().map_err(ImageError::from_tiff_decode)?;
        let tiff_color_type = inner.colortype().map_err(ImageError::from_tiff_decode)?;

        match inner.find_tag_unsigned_vec::<u16>(Tag::SampleFormat) {
            Ok(Some(sample_formats)) => {
                for format in sample_formats {
                    check_sample_format(format, tiff_color_type)?;
                }
            }
            Ok(None) => { /* assume UInt format */ }
            Err(other) => return Err(ImageError::from_tiff_decode(other)),
        }

        let planar_config = inner
            .find_tag(Tag::PlanarConfiguration)
            .map(|res| res.and_then(|r| r.into_u16().ok()).unwrap_or_default())
            .unwrap_or_default();

        // Decode not supported for non Chunky Planar Configuration
        if planar_config > 1 {
            Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Tiff.into(),
                    UnsupportedErrorKind::GenericFeature(String::from("PlanarConfiguration = 2")),
                ),
            ))?;
        }

        let color_type = match tiff_color_type {
            tiff::ColorType::Gray(1) => ColorType::L8,
            tiff::ColorType::Gray(8) => ColorType::L8,
            tiff::ColorType::Gray(16) => ColorType::L16,
            tiff::ColorType::GrayA(8) => ColorType::La8,
            tiff::ColorType::GrayA(16) => ColorType::La16,
            tiff::ColorType::RGB(8) => ColorType::Rgb8,
            tiff::ColorType::RGB(16) => ColorType::Rgb16,
            tiff::ColorType::RGBA(8) => ColorType::Rgba8,
            tiff::ColorType::RGBA(16) => ColorType::Rgba16,
            tiff::ColorType::CMYK(8) => ColorType::Rgb8,
            tiff::ColorType::RGB(32) => ColorType::Rgb32F,
            tiff::ColorType::RGBA(32) => ColorType::Rgba32F,

            tiff::ColorType::Palette(n) | tiff::ColorType::Gray(n) => {
                return Err(err_unknown_color_type(n))
            }
            tiff::ColorType::GrayA(n) => return Err(err_unknown_color_type(n.saturating_mul(2))),
            tiff::ColorType::RGB(n) => return Err(err_unknown_color_type(n.saturating_mul(3))),
            tiff::ColorType::YCbCr(n) => return Err(err_unknown_color_type(n.saturating_mul(3))),
            tiff::ColorType::RGBA(n) | tiff::ColorType::CMYK(n) => {
                return Err(err_unknown_color_type(n.saturating_mul(4)))
            }
            tiff::ColorType::Multiband {
                bit_depth,
                num_samples,
            } => {
                return Err(err_unknown_color_type(
                    bit_depth.saturating_mul(num_samples.min(255) as u8),
                ))
            }
            _ => return Err(err_unknown_color_type(0)),
        };

        let original_color_type = match tiff_color_type {
            tiff::ColorType::Gray(1) => ExtendedColorType::L1,
            tiff::ColorType::CMYK(8) => ExtendedColorType::Cmyk8,
            _ => color_type.into(),
        };

        Ok(TiffDecoder {
            dimensions,
            color_type,
            original_color_type,
            inner: Some(inner),
        })
    }

    // The buffer can be larger for CMYK than the RGB output
    fn total_bytes_buffer(&self) -> u64 {
        let dimensions = self.dimensions();
        let total_pixels = u64::from(dimensions.0) * u64::from(dimensions.1);
        let bytes_per_pixel = if self.original_color_type == ExtendedColorType::Cmyk8 {
            16
        } else {
            u64::from(self.color_type().bytes_per_pixel())
        };
        total_pixels.saturating_mul(bytes_per_pixel)
    }
}

fn check_sample_format(sample_format: u16, color_type: tiff::ColorType) -> Result<(), ImageError> {
    use tiff::{tags::SampleFormat, ColorType};
    let num_bits = match color_type {
        ColorType::CMYK(k) => k,
        ColorType::Gray(k) => k,
        ColorType::RGB(k) => k,
        ColorType::RGBA(k) => k,
        ColorType::GrayA(k) => k,
        ColorType::Palette(k) | ColorType::YCbCr(k) => {
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Tiff.into(),
                    UnsupportedErrorKind::GenericFeature(format!(
                        "Unhandled TIFF color type {color_type:?} for {k} bits",
                    )),
                ),
            ))
        }
        _ => {
            return Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Tiff.into(),
                    UnsupportedErrorKind::GenericFeature(format!(
                        "Unhandled TIFF color type {color_type:?}",
                    )),
                ),
            ))
        }
    };

    match SampleFormat::from_u16(sample_format) {
        Some(SampleFormat::Uint) if num_bits <= 16 => Ok(()),
        Some(SampleFormat::IEEEFP) if num_bits == 32 => Ok(()),
        _ => Err(ImageError::Unsupported(
            UnsupportedError::from_format_and_kind(
                ImageFormat::Tiff.into(),
                UnsupportedErrorKind::GenericFeature(format!(
                    "Unhandled TIFF sample format {sample_format:?} for {num_bits} bits",
                )),
            ),
        )),
    }
}

fn err_unknown_color_type(value: u8) -> ImageError {
    ImageError::Unsupported(UnsupportedError::from_format_and_kind(
        ImageFormat::Tiff.into(),
        UnsupportedErrorKind::Color(ExtendedColorType::Unknown(value)),
    ))
}

impl ImageError {
    fn from_tiff_decode(err: tiff::TiffError) -> ImageError {
        match err {
            tiff::TiffError::IoError(err) => ImageError::IoError(err),
            err @ (tiff::TiffError::FormatError(_)
            | tiff::TiffError::IntSizeError
            | tiff::TiffError::UsageError(_)) => {
                ImageError::Decoding(DecodingError::new(ImageFormat::Tiff.into(), err))
            }
            tiff::TiffError::UnsupportedError(desc) => {
                ImageError::Unsupported(UnsupportedError::from_format_and_kind(
                    ImageFormat::Tiff.into(),
                    UnsupportedErrorKind::GenericFeature(desc.to_string()),
                ))
            }
            tiff::TiffError::LimitsExceeded => {
                ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory))
            }
        }
    }

    fn from_tiff_encode(err: tiff::TiffError) -> ImageError {
        match err {
            tiff::TiffError::IoError(err) => ImageError::IoError(err),
            err @ (tiff::TiffError::FormatError(_)
            | tiff::TiffError::IntSizeError
            | tiff::TiffError::UsageError(_)) => {
                ImageError::Encoding(EncodingError::new(ImageFormat::Tiff.into(), err))
            }
            tiff::TiffError::UnsupportedError(desc) => {
                ImageError::Unsupported(UnsupportedError::from_format_and_kind(
                    ImageFormat::Tiff.into(),
                    UnsupportedErrorKind::GenericFeature(desc.to_string()),
                ))
            }
            tiff::TiffError::LimitsExceeded => {
                ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory))
            }
        }
    }
}

/// Wrapper struct around a `Cursor<Vec<u8>>`
#[allow(dead_code)]
#[deprecated]
pub struct TiffReader<R>(Cursor<Vec<u8>>, PhantomData<R>);
#[allow(deprecated)]
impl<R> Read for TiffReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        if self.0.position() == 0 && buf.is_empty() {
            mem::swap(buf, self.0.get_mut());
            Ok(buf.len())
        } else {
            self.0.read_to_end(buf)
        }
    }
}

impl<R: BufRead + Seek> ImageDecoder for TiffDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }

    fn color_type(&self) -> ColorType {
        self.color_type
    }

    fn original_color_type(&self) -> ExtendedColorType {
        self.original_color_type
    }

    fn icc_profile(&mut self) -> ImageResult<Option<Vec<u8>>> {
        if let Some(decoder) = &mut self.inner {
            Ok(decoder.get_tag_u8_vec(Tag::Unknown(34675)).ok())
        } else {
            Ok(None)
        }
    }

    fn orientation(&mut self) -> ImageResult<Orientation> {
        if let Some(decoder) = &mut self.inner {
            Ok(decoder
                .find_tag(Tag::Orientation)
                .map_err(ImageError::from_tiff_decode)?
                .and_then(|v| Orientation::from_exif(v.into_u16().ok()?.min(255) as u8))
                .unwrap_or(Orientation::NoTransforms))
        } else {
            Ok(Orientation::NoTransforms)
        }
    }

    fn set_limits(&mut self, limits: crate::Limits) -> ImageResult<()> {
        limits.check_support(&crate::LimitSupport::default())?;

        let (width, height) = self.dimensions();
        limits.check_dimensions(width, height)?;

        let max_alloc = limits.max_alloc.unwrap_or(u64::MAX);
        let max_intermediate_alloc = max_alloc.saturating_sub(self.total_bytes_buffer());

        let mut tiff_limits: tiff::decoder::Limits = Default::default();
        tiff_limits.decoding_buffer_size =
            usize::try_from(max_alloc - max_intermediate_alloc).unwrap_or(usize::MAX);
        tiff_limits.intermediate_buffer_size =
            usize::try_from(max_intermediate_alloc).unwrap_or(usize::MAX);
        tiff_limits.ifd_value_size = tiff_limits.intermediate_buffer_size;
        self.inner = Some(self.inner.take().unwrap().with_limits(tiff_limits));

        Ok(())
    }

    fn read_image(self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));

        match self
            .inner
            .unwrap()
            .read_image()
            .map_err(ImageError::from_tiff_decode)?
        {
            DecodingResult::U8(v) if self.original_color_type == ExtendedColorType::Cmyk8 => {
                let mut out_cur = Cursor::new(buf);
                for cmyk in v.chunks_exact(4) {
                    out_cur.write_all(&cmyk_to_rgb(cmyk))?;
                }
            }
            DecodingResult::U8(v) if self.original_color_type == ExtendedColorType::L1 => {
                let width = self.dimensions.0;
                let row_bytes = width.div_ceil(8);

                for (in_row, out_row) in v
                    .chunks_exact(row_bytes as usize)
                    .zip(buf.chunks_exact_mut(width as usize))
                {
                    out_row.copy_from_slice(&utils::expand_bits(1, width, in_row));
                }
            }
            DecodingResult::U8(v) => {
                buf.copy_from_slice(&v);
            }
            DecodingResult::U16(v) => {
                buf.copy_from_slice(bytemuck::cast_slice(&v));
            }
            DecodingResult::U32(v) => {
                buf.copy_from_slice(bytemuck::cast_slice(&v));
            }
            DecodingResult::U64(v) => {
                buf.copy_from_slice(bytemuck::cast_slice(&v));
            }
            DecodingResult::I8(v) => {
                buf.copy_from_slice(bytemuck::cast_slice(&v));
            }
            DecodingResult::I16(v) => {
                buf.copy_from_slice(bytemuck::cast_slice(&v));
            }
            DecodingResult::I32(v) => {
                buf.copy_from_slice(bytemuck::cast_slice(&v));
            }
            DecodingResult::I64(v) => {
                buf.copy_from_slice(bytemuck::cast_slice(&v));
            }
            DecodingResult::F32(v) => {
                buf.copy_from_slice(bytemuck::cast_slice(&v));
            }
            DecodingResult::F64(v) => {
                buf.copy_from_slice(bytemuck::cast_slice(&v));
            }
            DecodingResult::F16(_) => unreachable!(),
        }
        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

/// Encoder for tiff images
pub struct TiffEncoder<W> {
    w: W,
}

fn cmyk_to_rgb(cmyk: &[u8]) -> [u8; 3] {
    let c = f32::from(cmyk[0]);
    let m = f32::from(cmyk[1]);
    let y = f32::from(cmyk[2]);
    let kf = 1. - f32::from(cmyk[3]) / 255.;
    [
        ((255. - c) * kf) as u8,
        ((255. - m) * kf) as u8,
        ((255. - y) * kf) as u8,
    ]
}

/// Convert a slice of sample bytes to its semantic type, being a `Pod`.
fn u8_slice_as_pod<P: bytemuck::Pod>(buf: &[u8]) -> ImageResult<std::borrow::Cow<'_, [P]>> {
    bytemuck::try_cast_slice(buf)
        .map(std::borrow::Cow::Borrowed)
        .or_else(|err| {
            match err {
                bytemuck::PodCastError::TargetAlignmentGreaterAndInputNotAligned => {
                    // If the buffer is not aligned for a native slice, copy the buffer into a Vec,
                    // aligning it in the process. This is only done if the element count can be
                    // represented exactly.
                    let vec = bytemuck::allocation::pod_collect_to_vec(buf);
                    Ok(std::borrow::Cow::Owned(vec))
                }
                /* only expecting: bytemuck::PodCastError::OutputSliceWouldHaveSlop */
                _ => {
                    // `bytemuck::PodCastError` of bytemuck-1.2.0 does not implement `Error` and
                    // `Display` trait.
                    // See <https://github.com/Lokathor/bytemuck/issues/22>.
                    Err(ImageError::Parameter(ParameterError::from_kind(
                        ParameterErrorKind::Generic(format!(
                            "Casting samples to their representation failed: {err:?}",
                        )),
                    )))
                }
            }
        })
}

impl<W: Write + Seek> TiffEncoder<W> {
    /// Create a new encoder that writes its output to `w`
    pub fn new(w: W) -> TiffEncoder<W> {
        TiffEncoder { w }
    }

    /// Encodes the image `image` that has dimensions `width` and `height` and `ColorType` `c`.
    ///
    /// 16-bit types assume the buffer is native endian.
    ///
    /// # Panics
    ///
    /// Panics if `width * height * color_type.bytes_per_pixel() != data.len()`.
    #[track_caller]
    pub fn encode(
        self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        use tiff::encoder::colortype::{
            Gray16, Gray8, RGB32Float, RGBA32Float, RGB16, RGB8, RGBA16, RGBA8,
        };
        let expected_buffer_len = color_type.buffer_size(width, height);
        assert_eq!(
            expected_buffer_len,
            buf.len() as u64,
            "Invalid buffer length: expected {expected_buffer_len} got {} for {width}x{height} image",
            buf.len(),
        );
        let mut encoder =
            tiff::encoder::TiffEncoder::new(self.w).map_err(ImageError::from_tiff_encode)?;
        match color_type {
            ExtendedColorType::L8 => encoder.write_image::<Gray8>(width, height, buf),
            ExtendedColorType::Rgb8 => encoder.write_image::<RGB8>(width, height, buf),
            ExtendedColorType::Rgba8 => encoder.write_image::<RGBA8>(width, height, buf),
            ExtendedColorType::L16 => {
                encoder.write_image::<Gray16>(width, height, u8_slice_as_pod::<u16>(buf)?.as_ref())
            }
            ExtendedColorType::Rgb16 => {
                encoder.write_image::<RGB16>(width, height, u8_slice_as_pod::<u16>(buf)?.as_ref())
            }
            ExtendedColorType::Rgba16 => {
                encoder.write_image::<RGBA16>(width, height, u8_slice_as_pod::<u16>(buf)?.as_ref())
            }
            ExtendedColorType::Rgb32F => encoder.write_image::<RGB32Float>(
                width,
                height,
                u8_slice_as_pod::<f32>(buf)?.as_ref(),
            ),
            ExtendedColorType::Rgba32F => encoder.write_image::<RGBA32Float>(
                width,
                height,
                u8_slice_as_pod::<f32>(buf)?.as_ref(),
            ),
            _ => {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Tiff.into(),
                        UnsupportedErrorKind::Color(color_type),
                    ),
                ))
            }
        }
        .map_err(ImageError::from_tiff_encode)?;

        Ok(())
    }
}

impl<W: Write + Seek> ImageEncoder for TiffEncoder<W> {
    #[track_caller]
    fn write_image(
        self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        self.encode(buf, width, height, color_type)
    }
}
