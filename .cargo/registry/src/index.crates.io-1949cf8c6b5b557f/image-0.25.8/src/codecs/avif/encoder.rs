//! Encoding of AVIF images.
///
/// The [AVIF] specification defines an image derivative of the AV1 bitstream, an open video codec.
///
/// [AVIF]: https://aomediacodec.github.io/av1-avif/
use std::borrow::Cow;
use std::cmp::min;
use std::io::Write;
use std::mem::size_of;

use crate::buffer::ConvertBuffer;
use crate::color::{FromColor, Luma, LumaA, Rgb, Rgba};
use crate::error::{
    EncodingError, ParameterError, ParameterErrorKind, UnsupportedError, UnsupportedErrorKind,
};
use crate::{ExtendedColorType, ImageBuffer, ImageEncoder, ImageFormat, Pixel};
use crate::{ImageError, ImageResult};

use bytemuck::{try_cast_slice, try_cast_slice_mut, Pod, PodCastError};
use num_traits::Zero;
use ravif::{BitDepth, Encoder, Img, RGB8, RGBA8};
use rgb::AsPixels;

/// AVIF Encoder.
///
/// Writes one image into the chosen output.
pub struct AvifEncoder<W> {
    inner: W,
    encoder: Encoder,
}

/// An enumeration over supported AVIF color spaces
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ColorSpace {
    /// sRGB colorspace
    Srgb,
    /// BT.709 colorspace
    Bt709,
}

impl ColorSpace {
    fn to_ravif(self) -> ravif::ColorModel {
        match self {
            Self::Srgb => ravif::ColorModel::RGB,
            Self::Bt709 => ravif::ColorModel::YCbCr,
        }
    }
}

enum RgbColor<'buf> {
    Rgb8(Img<&'buf [RGB8]>),
    Rgba8(Img<&'buf [RGBA8]>),
}

impl<W: Write> AvifEncoder<W> {
    /// Create a new encoder that writes its output to `w`.
    pub fn new(w: W) -> Self {
        AvifEncoder::new_with_speed_quality(w, 4, 80) // `cavif` uses these defaults
    }

    /// Create a new encoder with a specified speed and quality that writes its output to `w`.
    /// `speed` accepts a value in the range 1-10, where 1 is the slowest and 10 is the fastest.
    /// Slower speeds generally yield better compression results.
    /// `quality` accepts a value in the range 1-100, where 1 is the worst and 100 is the best.
    pub fn new_with_speed_quality(w: W, speed: u8, quality: u8) -> Self {
        // Clamp quality and speed to range
        let quality = min(quality, 100);
        let speed = min(speed, 10);

        let encoder = Encoder::new()
            .with_quality(f32::from(quality))
            .with_alpha_quality(f32::from(quality))
            .with_speed(speed)
            .with_bit_depth(BitDepth::Eight);

        AvifEncoder { inner: w, encoder }
    }

    /// Encode with the specified `color_space`.
    pub fn with_colorspace(mut self, color_space: ColorSpace) -> Self {
        self.encoder = self
            .encoder
            .with_internal_color_model(color_space.to_ravif());
        self
    }

    /// Configures `rayon` thread pool size.
    /// The default `None` is to use all threads in the default `rayon` thread pool.
    pub fn with_num_threads(mut self, num_threads: Option<usize>) -> Self {
        self.encoder = self.encoder.with_num_threads(num_threads);
        self
    }
}

impl<W: Write> ImageEncoder for AvifEncoder<W> {
    /// Encode image data with the indicated color type.
    ///
    /// The encoder currently requires all data to be RGBA8, it will be converted internally if
    /// necessary. When data is suitably aligned, i.e. u16 channels to two bytes, then the
    /// conversion may be more efficient.
    #[track_caller]
    fn write_image(
        mut self,
        data: &[u8],
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<()> {
        let expected_buffer_len = color.buffer_size(width, height);
        assert_eq!(
            expected_buffer_len,
            data.len() as u64,
            "Invalid buffer length: expected {expected_buffer_len} got {} for {width}x{height} image",
            data.len(),
        );

        self.set_color(color);
        // `ravif` needs strongly typed data so let's convert. We can either use a temporarily
        // owned version in our own buffer or zero-copy if possible by using the input buffer.
        // This requires going through `rgb`.
        let mut fallback = vec![]; // This vector is used if we need to do a color conversion.
        let result = match Self::encode_as_img(&mut fallback, data, width, height, color)? {
            RgbColor::Rgb8(buffer) => self.encoder.encode_rgb(buffer),
            RgbColor::Rgba8(buffer) => self.encoder.encode_rgba(buffer),
        };
        let data = result.map_err(|err| {
            ImageError::Encoding(EncodingError::new(ImageFormat::Avif.into(), err))
        })?;
        self.inner.write_all(&data.avif_file)?;
        Ok(())
    }
}

impl<W: Write> AvifEncoder<W> {
    // Does not currently do anything. Mirrors behaviour of old config function.
    fn set_color(&mut self, _color: ExtendedColorType) {
        // self.config.color_space = ColorSpace::RGB;
    }

    fn encode_as_img<'buf>(
        fallback: &'buf mut Vec<u8>,
        data: &'buf [u8],
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<RgbColor<'buf>> {
        // Error wrapping utility for color dependent buffer dimensions.
        fn try_from_raw<P: Pixel + 'static>(
            data: &[P::Subpixel],
            width: u32,
            height: u32,
        ) -> ImageResult<ImageBuffer<P, &[P::Subpixel]>> {
            ImageBuffer::from_raw(width, height, data).ok_or_else(|| {
                ImageError::Parameter(ParameterError::from_kind(
                    ParameterErrorKind::DimensionMismatch,
                ))
            })
        }

        // Convert to target color type using few buffer allocations.
        fn convert_into<'buf, P>(
            buf: &'buf mut Vec<u8>,
            image: ImageBuffer<P, &[P::Subpixel]>,
        ) -> Img<&'buf [RGBA8]>
        where
            P: Pixel + 'static,
            Rgba<u8>: FromColor<P>,
        {
            let (width, height) = image.dimensions();
            // TODO: conversion re-using the target buffer?
            let image: ImageBuffer<Rgba<u8>, _> = image.convert();
            *buf = image.into_raw();
            Img::new(buf.as_pixels(), width as usize, height as usize)
        }

        // Cast the input slice using few buffer allocations if possible.
        // In particular try not to allocate if the caller did the infallible reverse.
        fn cast_buffer<Channel>(buf: &[u8]) -> ImageResult<Cow<'_, [Channel]>>
        where
            Channel: Pod + Zero,
        {
            match try_cast_slice(buf) {
                Ok(slice) => Ok(Cow::Borrowed(slice)),
                Err(PodCastError::OutputSliceWouldHaveSlop) => Err(ImageError::Parameter(
                    ParameterError::from_kind(ParameterErrorKind::DimensionMismatch),
                )),
                Err(PodCastError::TargetAlignmentGreaterAndInputNotAligned) => {
                    // Sad, but let's allocate.
                    // bytemuck checks alignment _before_ slop but size mismatch before this..
                    if buf.len() % size_of::<Channel>() != 0 {
                        Err(ImageError::Parameter(ParameterError::from_kind(
                            ParameterErrorKind::DimensionMismatch,
                        )))
                    } else {
                        let len = buf.len() / size_of::<Channel>();
                        let mut data = vec![Channel::zero(); len];
                        let view = try_cast_slice_mut::<_, u8>(data.as_mut_slice()).unwrap();
                        view.copy_from_slice(buf);
                        Ok(Cow::Owned(data))
                    }
                }
                Err(err) => {
                    // Are you trying to encode a ZST??
                    Err(ImageError::Parameter(ParameterError::from_kind(
                        ParameterErrorKind::Generic(format!("{err:?}")),
                    )))
                }
            }
        }

        match color {
            ExtendedColorType::Rgb8 => {
                // ravif doesn't do any checks but has some asserts, so we do the checks.
                let img = try_from_raw::<Rgb<u8>>(data, width, height)?;
                // Now, internally ravif uses u32 but it takes usize. We could do some checked
                // conversion but instead we use that a non-empty image must be addressable.
                if img.pixels().len() == 0 {
                    return Err(ImageError::Parameter(ParameterError::from_kind(
                        ParameterErrorKind::DimensionMismatch,
                    )));
                }

                Ok(RgbColor::Rgb8(Img::new(
                    AsPixels::as_pixels(data),
                    width as usize,
                    height as usize,
                )))
            }
            ExtendedColorType::Rgba8 => {
                // ravif doesn't do any checks but has some asserts, so we do the checks.
                let img = try_from_raw::<Rgba<u8>>(data, width, height)?;
                // Now, internally ravif uses u32 but it takes usize. We could do some checked
                // conversion but instead we use that a non-empty image must be addressable.
                if img.pixels().len() == 0 {
                    return Err(ImageError::Parameter(ParameterError::from_kind(
                        ParameterErrorKind::DimensionMismatch,
                    )));
                }

                Ok(RgbColor::Rgba8(Img::new(
                    AsPixels::as_pixels(data),
                    width as usize,
                    height as usize,
                )))
            }
            // we need a separate buffer..
            ExtendedColorType::L8 => {
                let image = try_from_raw::<Luma<u8>>(data, width, height)?;
                Ok(RgbColor::Rgba8(convert_into(fallback, image)))
            }
            ExtendedColorType::La8 => {
                let image = try_from_raw::<LumaA<u8>>(data, width, height)?;
                Ok(RgbColor::Rgba8(convert_into(fallback, image)))
            }
            // we need to really convert data..
            ExtendedColorType::L16 => {
                let buffer = cast_buffer(data)?;
                let image = try_from_raw::<Luma<u16>>(&buffer, width, height)?;
                Ok(RgbColor::Rgba8(convert_into(fallback, image)))
            }
            ExtendedColorType::La16 => {
                let buffer = cast_buffer(data)?;
                let image = try_from_raw::<LumaA<u16>>(&buffer, width, height)?;
                Ok(RgbColor::Rgba8(convert_into(fallback, image)))
            }
            ExtendedColorType::Rgb16 => {
                let buffer = cast_buffer(data)?;
                let image = try_from_raw::<Rgb<u16>>(&buffer, width, height)?;
                Ok(RgbColor::Rgba8(convert_into(fallback, image)))
            }
            ExtendedColorType::Rgba16 => {
                let buffer = cast_buffer(data)?;
                let image = try_from_raw::<Rgba<u16>>(&buffer, width, height)?;
                Ok(RgbColor::Rgba8(convert_into(fallback, image)))
            }
            // for cases we do not support at all?
            _ => Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Avif.into(),
                    UnsupportedErrorKind::Color(color),
                ),
            )),
        }
    }
}
