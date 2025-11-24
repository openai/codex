use std::fs::File;
use std::io::{self, BufWriter, Seek, Write};
use std::path::Path;

use crate::color::{self, FromColor, IntoColor};
use crate::error::{ImageError, ImageResult, ParameterError, ParameterErrorKind};
use crate::flat::FlatSamples;
use crate::imageops::{gaussian_blur_dyn_image, GaussianBlurParameters};
use crate::images::buffer::{
    ConvertBuffer, Gray16Image, GrayAlpha16Image, GrayAlphaImage, GrayImage, ImageBuffer,
    Rgb16Image, Rgb32FImage, RgbImage, Rgba16Image, Rgba32FImage, RgbaImage,
};
use crate::io::encoder::ImageEncoderBoxed;
use crate::io::free_functions::{self, encoder_for_format};
use crate::math::resize_dimensions;
use crate::metadata::Orientation;
use crate::traits::Pixel;
use crate::{
    imageops,
    metadata::{Cicp, CicpColorPrimaries, CicpTransferCharacteristics},
    ConvertColorOptions, ExtendedColorType, GenericImage, GenericImageView, ImageDecoder,
    ImageEncoder, ImageFormat, ImageReader, Luma, LumaA,
};

/// A Dynamic Image
///
/// This represents a _matrix_ of _pixels_ which are _convertible_ from and to an _RGBA_
/// representation. More variants that adhere to these principles may get added in the future, in
/// particular to cover other combinations typically used.
///
/// # Usage
///
/// This type can act as a converter between specific `ImageBuffer` instances.
///
/// ```
/// use image::{DynamicImage, GrayImage, RgbImage};
///
/// let rgb: RgbImage = RgbImage::new(10, 10);
/// let luma: GrayImage = DynamicImage::ImageRgb8(rgb).into_luma8();
/// ```
///
/// # Design
///
/// There is no goal to provide an all-encompassing type with all possible memory layouts. This
/// would hardly be feasible as a simple enum, due to the sheer number of combinations of channel
/// kinds, channel order, and bit depth. Rather, this type provides an opinionated selection with
/// normalized channel order which can store common pixel values without loss.
///
/// # Color space
///
/// Each image has an associated color space in the form of [CICP] data ([ITU Rec H.273]). Not all
/// color spaces are supported in the sense that you can compute in them ([Context][w3c-png]).
/// Conversion into different pixels types ([`ColorType`][`crate::ColorType`]) _generally_ take the
/// color space into account, with the exception of [`DynamicImage::to`] due to historical design
/// baggage.
///
/// The imageops functions operate in _encoded_ space, directly on the channel values, and do _not_
/// linearize colors internally as you might be used to from GPU shader programming. Their return
/// values however copy the color space annotation of the source.
///
/// The IO functions do _not yet_ write ICC or CICP indications into the result formats. We're
/// aware of this problem, it is tracked in [#2493] and [#1460].
///
/// [CICP]: https://www.w3.org/TR/png-3/#cICP-chunk
/// [w3c-png]: https://github.com/w3c/png/issues/312
/// [ITU Rec H.273]: https://www.itu.int/rec/T-REC-H.273-202407-I/en
/// [#2493]: https://github.com/image-rs/image/issues/2493
/// [#1460]: https://github.com/image-rs/image/issues/1460
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum DynamicImage {
    /// Each pixel in this image is 8-bit Luma
    ImageLuma8(GrayImage),

    /// Each pixel in this image is 8-bit Luma with alpha
    ImageLumaA8(GrayAlphaImage),

    /// Each pixel in this image is 8-bit Rgb
    ImageRgb8(RgbImage),

    /// Each pixel in this image is 8-bit Rgb with alpha
    ImageRgba8(RgbaImage),

    /// Each pixel in this image is 16-bit Luma
    ImageLuma16(Gray16Image),

    /// Each pixel in this image is 16-bit Luma with alpha
    ImageLumaA16(GrayAlpha16Image),

    /// Each pixel in this image is 16-bit Rgb
    ImageRgb16(Rgb16Image),

    /// Each pixel in this image is 16-bit Rgb with alpha
    ImageRgba16(Rgba16Image),

    /// Each pixel in this image is 32-bit float Rgb
    ImageRgb32F(Rgb32FImage),

    /// Each pixel in this image is 32-bit float Rgb with alpha
    ImageRgba32F(Rgba32FImage),
}

macro_rules! dynamic_map(
        ($dynimage: expr, $image: pat => $action: expr) => ({
            use DynamicImage::*;
            match $dynimage {
                ImageLuma8($image) => ImageLuma8($action),
                ImageLumaA8($image) => ImageLumaA8($action),
                ImageRgb8($image) => ImageRgb8($action),
                ImageRgba8($image) => ImageRgba8($action),
                ImageLuma16($image) => ImageLuma16($action),
                ImageLumaA16($image) => ImageLumaA16($action),
                ImageRgb16($image) => ImageRgb16($action),
                ImageRgba16($image) => ImageRgba16($action),
                ImageRgb32F($image) => ImageRgb32F($action),
                ImageRgba32F($image) => ImageRgba32F($action),
            }
        });

        ($dynimage: expr, $image:pat_param, $action: expr) => (
            match $dynimage {
                DynamicImage::ImageLuma8($image) => $action,
                DynamicImage::ImageLumaA8($image) => $action,
                DynamicImage::ImageRgb8($image) => $action,
                DynamicImage::ImageRgba8($image) => $action,
                DynamicImage::ImageLuma16($image) => $action,
                DynamicImage::ImageLumaA16($image) => $action,
                DynamicImage::ImageRgb16($image) => $action,
                DynamicImage::ImageRgba16($image) => $action,
                DynamicImage::ImageRgb32F($image) => $action,
                DynamicImage::ImageRgba32F($image) => $action,
            }
        );
);

impl Clone for DynamicImage {
    fn clone(&self) -> Self {
        dynamic_map!(*self, ref p, DynamicImage::from(p.clone()))
    }

    fn clone_from(&mut self, source: &Self) {
        match (self, source) {
            (Self::ImageLuma8(p1), Self::ImageLuma8(p2)) => p1.clone_from(p2),
            (Self::ImageLumaA8(p1), Self::ImageLumaA8(p2)) => p1.clone_from(p2),
            (Self::ImageRgb8(p1), Self::ImageRgb8(p2)) => p1.clone_from(p2),
            (Self::ImageRgba8(p1), Self::ImageRgba8(p2)) => p1.clone_from(p2),
            (Self::ImageLuma16(p1), Self::ImageLuma16(p2)) => p1.clone_from(p2),
            (Self::ImageLumaA16(p1), Self::ImageLumaA16(p2)) => p1.clone_from(p2),
            (Self::ImageRgb16(p1), Self::ImageRgb16(p2)) => p1.clone_from(p2),
            (Self::ImageRgba16(p1), Self::ImageRgba16(p2)) => p1.clone_from(p2),
            (Self::ImageRgb32F(p1), Self::ImageRgb32F(p2)) => p1.clone_from(p2),
            (Self::ImageRgba32F(p1), Self::ImageRgba32F(p2)) => p1.clone_from(p2),
            (this, source) => *this = source.clone(),
        }
    }
}

impl DynamicImage {
    /// Creates a dynamic image backed by a buffer depending on
    /// the color type given.
    ///
    /// The color space is initially set to [`sRGB`][`Cicp::SRGB`].
    #[must_use]
    pub fn new(w: u32, h: u32, color: color::ColorType) -> DynamicImage {
        use color::ColorType::*;
        match color {
            L8 => Self::new_luma8(w, h),
            La8 => Self::new_luma_a8(w, h),
            Rgb8 => Self::new_rgb8(w, h),
            Rgba8 => Self::new_rgba8(w, h),
            L16 => Self::new_luma16(w, h),
            La16 => Self::new_luma_a16(w, h),
            Rgb16 => Self::new_rgb16(w, h),
            Rgba16 => Self::new_rgba16(w, h),
            Rgb32F => Self::new_rgb32f(w, h),
            Rgba32F => Self::new_rgba32f(w, h),
        }
    }

    /// Creates a dynamic image backed by a buffer of gray pixels.
    #[must_use]
    pub fn new_luma8(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageLuma8(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of gray
    /// pixels with transparency.
    #[must_use]
    pub fn new_luma_a8(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageLumaA8(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of RGB pixels.
    #[must_use]
    pub fn new_rgb8(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of RGBA pixels.
    #[must_use]
    pub fn new_rgba8(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of gray pixels.
    #[must_use]
    pub fn new_luma16(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageLuma16(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of gray
    /// pixels with transparency.
    #[must_use]
    pub fn new_luma_a16(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageLumaA16(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of RGB pixels.
    #[must_use]
    pub fn new_rgb16(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb16(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of RGBA pixels.
    #[must_use]
    pub fn new_rgba16(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba16(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of RGB pixels.
    #[must_use]
    pub fn new_rgb32f(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb32F(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of RGBA pixels.
    #[must_use]
    pub fn new_rgba32f(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba32F(ImageBuffer::new(w, h))
    }

    /// Decodes an encoded image into a dynamic image.
    pub fn from_decoder(decoder: impl ImageDecoder) -> ImageResult<Self> {
        decoder_to_image(decoder)
    }

    /// Encodes a dynamic image into a buffer.
    ///
    /// **WARNING**: Conversion between RGB and Luma is not aware of the color space and always
    /// uses sRGB coefficients to determine a non-constant luminance from an RGB color (and
    /// conversely).
    ///
    /// This unfortunately owes to the public bounds of `T` which does not allow for passing a
    /// color space as a parameter. This function will likely be deprecated and replaced.
    #[inline]
    #[must_use]
    pub fn to<
        T: Pixel
            + FromColor<color::Rgb<u8>>
            + FromColor<color::Rgb<f32>>
            + FromColor<color::Rgba<u8>>
            + FromColor<color::Rgba<u16>>
            + FromColor<color::Rgba<f32>>
            + FromColor<color::Rgb<u16>>
            + FromColor<Luma<u8>>
            + FromColor<Luma<u16>>
            + FromColor<LumaA<u16>>
            + FromColor<LumaA<u8>>,
    >(
        &self,
    ) -> ImageBuffer<T, Vec<T::Subpixel>> {
        dynamic_map!(*self, ref p, p.convert())
    }

    /// Returns a copy of this image as an RGB image.
    #[must_use]
    pub fn to_rgb8(&self) -> RgbImage {
        match self {
            DynamicImage::ImageRgb8(x) => x.clone(),
            x => dynamic_map!(x, ref p, p.cast_in_color_space()),
        }
    }

    /// Returns a copy of this image as an RGB image.
    #[must_use]
    pub fn to_rgb16(&self) -> Rgb16Image {
        match self {
            DynamicImage::ImageRgb16(x) => x.clone(),
            x => dynamic_map!(x, ref p, p.cast_in_color_space()),
        }
    }

    /// Returns a copy of this image as an RGB image.
    #[must_use]
    pub fn to_rgb32f(&self) -> Rgb32FImage {
        match self {
            DynamicImage::ImageRgb32F(x) => x.clone(),
            x => dynamic_map!(x, ref p, p.cast_in_color_space()),
        }
    }

    /// Returns a copy of this image as an RGBA image.
    #[must_use]
    pub fn to_rgba8(&self) -> RgbaImage {
        match self {
            DynamicImage::ImageRgba8(x) => x.clone(),
            x => dynamic_map!(x, ref p, p.cast_in_color_space()),
        }
    }

    /// Returns a copy of this image as an RGBA image.
    #[must_use]
    pub fn to_rgba16(&self) -> Rgba16Image {
        match self {
            DynamicImage::ImageRgba16(x) => x.clone(),
            x => dynamic_map!(x, ref p, p.cast_in_color_space()),
        }
    }

    /// Returns a copy of this image as an RGBA image.
    #[must_use]
    pub fn to_rgba32f(&self) -> Rgba32FImage {
        match self {
            DynamicImage::ImageRgba32F(x) => x.clone(),
            x => dynamic_map!(x, ref p, p.cast_in_color_space()),
        }
    }

    /// Returns a copy of this image as a Luma image.
    #[must_use]
    pub fn to_luma8(&self) -> GrayImage {
        match self {
            DynamicImage::ImageLuma8(x) => x.clone(),
            x => dynamic_map!(x, ref p, p.cast_in_color_space()),
        }
    }

    /// Returns a copy of this image as a Luma image.
    #[must_use]
    pub fn to_luma16(&self) -> Gray16Image {
        match self {
            DynamicImage::ImageLuma16(x) => x.clone(),
            x => dynamic_map!(x, ref p, p.cast_in_color_space()),
        }
    }

    /// Returns a copy of this image as a Luma image.
    #[must_use]
    pub fn to_luma32f(&self) -> ImageBuffer<Luma<f32>, Vec<f32>> {
        dynamic_map!(self, ref p, p.cast_in_color_space())
    }

    /// Returns a copy of this image as a `LumaA` image.
    #[must_use]
    pub fn to_luma_alpha8(&self) -> GrayAlphaImage {
        match self {
            DynamicImage::ImageLumaA8(x) => x.clone(),
            x => dynamic_map!(x, ref p, p.cast_in_color_space()),
        }
    }

    /// Returns a copy of this image as a `LumaA` image.
    #[must_use]
    pub fn to_luma_alpha16(&self) -> GrayAlpha16Image {
        match self {
            DynamicImage::ImageLumaA16(x) => x.clone(),
            x => dynamic_map!(x, ref p, p.cast_in_color_space()),
        }
    }

    /// Returns a copy of this image as a `LumaA` image.
    #[must_use]
    pub fn to_luma_alpha32f(&self) -> ImageBuffer<LumaA<f32>, Vec<f32>> {
        dynamic_map!(self, ref p, p.cast_in_color_space())
    }

    /// Consume the image and returns a RGB image.
    ///
    /// If the image was already the correct format, it is returned as is.
    /// Otherwise, a copy is created.
    #[must_use]
    pub fn into_rgb8(self) -> RgbImage {
        match self {
            DynamicImage::ImageRgb8(x) => x,
            x => x.to_rgb8(),
        }
    }

    /// Consume the image and returns a RGB image.
    ///
    /// If the image was already the correct format, it is returned as is.
    /// Otherwise, a copy is created.
    #[must_use]
    pub fn into_rgb16(self) -> Rgb16Image {
        match self {
            DynamicImage::ImageRgb16(x) => x,
            x => x.to_rgb16(),
        }
    }

    /// Consume the image and returns a RGB image.
    ///
    /// If the image was already the correct format, it is returned as is.
    /// Otherwise, a copy is created.
    #[must_use]
    pub fn into_rgb32f(self) -> Rgb32FImage {
        match self {
            DynamicImage::ImageRgb32F(x) => x,
            x => x.to_rgb32f(),
        }
    }

    /// Consume the image and returns a RGBA image.
    ///
    /// If the image was already the correct format, it is returned as is.
    /// Otherwise, a copy is created.
    #[must_use]
    pub fn into_rgba8(self) -> RgbaImage {
        match self {
            DynamicImage::ImageRgba8(x) => x,
            x => x.to_rgba8(),
        }
    }

    /// Consume the image and returns a RGBA image.
    ///
    /// If the image was already the correct format, it is returned as is.
    /// Otherwise, a copy is created.
    #[must_use]
    pub fn into_rgba16(self) -> Rgba16Image {
        match self {
            DynamicImage::ImageRgba16(x) => x,
            x => x.to_rgba16(),
        }
    }

    /// Consume the image and returns a RGBA image.
    ///
    /// If the image was already the correct format, it is returned as is.
    /// Otherwise, a copy is created.
    #[must_use]
    pub fn into_rgba32f(self) -> Rgba32FImage {
        match self {
            DynamicImage::ImageRgba32F(x) => x,
            x => x.to_rgba32f(),
        }
    }

    /// Consume the image and returns a Luma image.
    ///
    /// If the image was already the correct format, it is returned as is.
    /// Otherwise, a copy is created.
    #[must_use]
    pub fn into_luma8(self) -> GrayImage {
        match self {
            DynamicImage::ImageLuma8(x) => x,
            x => x.to_luma8(),
        }
    }

    /// Consume the image and returns a Luma image.
    ///
    /// If the image was already the correct format, it is returned as is.
    /// Otherwise, a copy is created.
    #[must_use]
    pub fn into_luma16(self) -> Gray16Image {
        match self {
            DynamicImage::ImageLuma16(x) => x,
            x => x.to_luma16(),
        }
    }

    /// Consume the image and returns a `LumaA` image.
    ///
    /// If the image was already the correct format, it is returned as is.
    /// Otherwise, a copy is created.
    #[must_use]
    pub fn into_luma_alpha8(self) -> GrayAlphaImage {
        match self {
            DynamicImage::ImageLumaA8(x) => x,
            x => x.to_luma_alpha8(),
        }
    }

    /// Consume the image and returns a `LumaA` image.
    ///
    /// If the image was already the correct format, it is returned as is.
    /// Otherwise, a copy is created.
    #[must_use]
    pub fn into_luma_alpha16(self) -> GrayAlpha16Image {
        match self {
            DynamicImage::ImageLumaA16(x) => x,
            x => x.to_luma_alpha16(),
        }
    }

    /// Return a cut-out of this image delimited by the bounding rectangle.
    ///
    /// Note: this method does *not* modify the object,
    /// and its signature will be replaced with `crop_imm()`'s in the 0.24 release
    #[must_use]
    pub fn crop(&mut self, x: u32, y: u32, width: u32, height: u32) -> DynamicImage {
        dynamic_map!(*self, ref mut p => imageops::crop(p, x, y, width, height).to_image())
    }

    /// Return a cut-out of this image delimited by the bounding rectangle.
    #[must_use]
    pub fn crop_imm(&self, x: u32, y: u32, width: u32, height: u32) -> DynamicImage {
        dynamic_map!(*self, ref p => imageops::crop_imm(p, x, y, width, height).to_image())
    }

    /// Return a reference to an 8bit RGB image
    #[must_use]
    pub fn as_rgb8(&self) -> Option<&RgbImage> {
        match *self {
            DynamicImage::ImageRgb8(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to an 8bit RGB image
    pub fn as_mut_rgb8(&mut self) -> Option<&mut RgbImage> {
        match *self {
            DynamicImage::ImageRgb8(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to an 8bit RGBA image
    #[must_use]
    pub fn as_rgba8(&self) -> Option<&RgbaImage> {
        match *self {
            DynamicImage::ImageRgba8(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to an 8bit RGBA image
    pub fn as_mut_rgba8(&mut self) -> Option<&mut RgbaImage> {
        match *self {
            DynamicImage::ImageRgba8(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to an 8bit Grayscale image
    #[must_use]
    pub fn as_luma8(&self) -> Option<&GrayImage> {
        match *self {
            DynamicImage::ImageLuma8(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to an 8bit Grayscale image
    pub fn as_mut_luma8(&mut self) -> Option<&mut GrayImage> {
        match *self {
            DynamicImage::ImageLuma8(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to an 8bit Grayscale image with an alpha channel
    #[must_use]
    pub fn as_luma_alpha8(&self) -> Option<&GrayAlphaImage> {
        match *self {
            DynamicImage::ImageLumaA8(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to an 8bit Grayscale image with an alpha channel
    pub fn as_mut_luma_alpha8(&mut self) -> Option<&mut GrayAlphaImage> {
        match *self {
            DynamicImage::ImageLumaA8(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to an 16bit RGB image
    #[must_use]
    pub fn as_rgb16(&self) -> Option<&Rgb16Image> {
        match *self {
            DynamicImage::ImageRgb16(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to an 16bit RGB image
    pub fn as_mut_rgb16(&mut self) -> Option<&mut Rgb16Image> {
        match *self {
            DynamicImage::ImageRgb16(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to an 16bit RGBA image
    #[must_use]
    pub fn as_rgba16(&self) -> Option<&Rgba16Image> {
        match *self {
            DynamicImage::ImageRgba16(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to an 16bit RGBA image
    pub fn as_mut_rgba16(&mut self) -> Option<&mut Rgba16Image> {
        match *self {
            DynamicImage::ImageRgba16(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to an 32bit RGB image
    #[must_use]
    pub fn as_rgb32f(&self) -> Option<&Rgb32FImage> {
        match *self {
            DynamicImage::ImageRgb32F(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to an 32bit RGB image
    pub fn as_mut_rgb32f(&mut self) -> Option<&mut Rgb32FImage> {
        match *self {
            DynamicImage::ImageRgb32F(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to an 32bit RGBA image
    #[must_use]
    pub fn as_rgba32f(&self) -> Option<&Rgba32FImage> {
        match *self {
            DynamicImage::ImageRgba32F(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to an 32bit RGBA image
    pub fn as_mut_rgba32f(&mut self) -> Option<&mut Rgba32FImage> {
        match *self {
            DynamicImage::ImageRgba32F(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to an 16bit Grayscale image
    #[must_use]
    pub fn as_luma16(&self) -> Option<&Gray16Image> {
        match *self {
            DynamicImage::ImageLuma16(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to an 16bit Grayscale image
    pub fn as_mut_luma16(&mut self) -> Option<&mut Gray16Image> {
        match *self {
            DynamicImage::ImageLuma16(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to an 16bit Grayscale image with an alpha channel
    #[must_use]
    pub fn as_luma_alpha16(&self) -> Option<&GrayAlpha16Image> {
        match *self {
            DynamicImage::ImageLumaA16(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to an 16bit Grayscale image with an alpha channel
    pub fn as_mut_luma_alpha16(&mut self) -> Option<&mut GrayAlpha16Image> {
        match *self {
            DynamicImage::ImageLumaA16(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a view on the raw sample buffer for 8 bit per channel images.
    #[must_use]
    pub fn as_flat_samples_u8(&self) -> Option<FlatSamples<&[u8]>> {
        match *self {
            DynamicImage::ImageLuma8(ref p) => Some(p.as_flat_samples()),
            DynamicImage::ImageLumaA8(ref p) => Some(p.as_flat_samples()),
            DynamicImage::ImageRgb8(ref p) => Some(p.as_flat_samples()),
            DynamicImage::ImageRgba8(ref p) => Some(p.as_flat_samples()),
            _ => None,
        }
    }

    /// Return a view on the raw sample buffer for 16 bit per channel images.
    #[must_use]
    pub fn as_flat_samples_u16(&self) -> Option<FlatSamples<&[u16]>> {
        match *self {
            DynamicImage::ImageLuma16(ref p) => Some(p.as_flat_samples()),
            DynamicImage::ImageLumaA16(ref p) => Some(p.as_flat_samples()),
            DynamicImage::ImageRgb16(ref p) => Some(p.as_flat_samples()),
            DynamicImage::ImageRgba16(ref p) => Some(p.as_flat_samples()),
            _ => None,
        }
    }

    /// Return a view on the raw sample buffer for 32bit per channel images.
    #[must_use]
    pub fn as_flat_samples_f32(&self) -> Option<FlatSamples<&[f32]>> {
        match *self {
            DynamicImage::ImageRgb32F(ref p) => Some(p.as_flat_samples()),
            DynamicImage::ImageRgba32F(ref p) => Some(p.as_flat_samples()),
            _ => None,
        }
    }

    /// Return this image's pixels as a native endian byte slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        // we can do this because every variant contains an `ImageBuffer<_, Vec<_>>`
        dynamic_map!(
            *self,
            ref image_buffer,
            bytemuck::cast_slice(image_buffer.as_raw())
        )
    }

    /// Return this image's pixels as a byte vector. If the `ImageBuffer`
    /// container is `Vec<u8>`, this operation is free. Otherwise, a copy
    /// is returned.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        // we can do this because every variant contains an `ImageBuffer<_, Vec<_>>`
        dynamic_map!(self, image_buffer, {
            match bytemuck::allocation::try_cast_vec(image_buffer.into_raw()) {
                Ok(vec) => vec,
                Err((_, vec)) => {
                    // Fallback: vector requires an exact alignment and size match
                    // Reuse of the allocation as done in the Ok branch only works if the
                    // underlying container is exactly Vec<u8> (or compatible but that's the only
                    // alternative at the time of writing).
                    // In all other cases we must allocate a new vector with the 'same' contents.
                    bytemuck::cast_slice(&vec).to_owned()
                }
            }
        })
    }

    /// Return this image's color type.
    #[must_use]
    pub fn color(&self) -> color::ColorType {
        match *self {
            DynamicImage::ImageLuma8(_) => color::ColorType::L8,
            DynamicImage::ImageLumaA8(_) => color::ColorType::La8,
            DynamicImage::ImageRgb8(_) => color::ColorType::Rgb8,
            DynamicImage::ImageRgba8(_) => color::ColorType::Rgba8,
            DynamicImage::ImageLuma16(_) => color::ColorType::L16,
            DynamicImage::ImageLumaA16(_) => color::ColorType::La16,
            DynamicImage::ImageRgb16(_) => color::ColorType::Rgb16,
            DynamicImage::ImageRgba16(_) => color::ColorType::Rgba16,
            DynamicImage::ImageRgb32F(_) => color::ColorType::Rgb32F,
            DynamicImage::ImageRgba32F(_) => color::ColorType::Rgba32F,
        }
    }

    /// Returns the width of the underlying image
    #[must_use]
    pub fn width(&self) -> u32 {
        dynamic_map!(*self, ref p, { p.width() })
    }

    /// Returns the height of the underlying image
    #[must_use]
    pub fn height(&self) -> u32 {
        dynamic_map!(*self, ref p, { p.height() })
    }

    /// Define the color space for the image.
    ///
    /// The color data is unchanged. Reinterprets the existing red, blue, green channels as points
    /// in the new set of primary colors, changing the apparent shade of pixels.
    ///
    /// Note that the primaries also define a reference whitepoint When this buffer contains Luma
    /// data, the luminance channel is interpreted as the `Y` channel of a related `YCbCr` color
    /// space as if by a non-constant chromaticity derived matrix. That is, coefficients are *not*
    /// applied in the linear RGB space but use encoded channel values. (In a color space with the
    /// linear transfer function there is no difference).
    pub fn set_rgb_primaries(&mut self, color: CicpColorPrimaries) {
        dynamic_map!(self, ref mut p, p.set_rgb_primaries(color));
    }

    /// Define the transfer function for the image.
    ///
    /// The color data is unchanged. Reinterprets all (non-alpha) components in the image,
    /// potentially changing the apparent shade of pixels. Individual components are always
    /// interpreted as encoded numbers. To denote numbers in a linear RGB space, use
    /// [`CicpTransferCharacteristics::Linear`].
    pub fn set_transfer_function(&mut self, tf: CicpTransferCharacteristics) {
        dynamic_map!(self, ref mut p, p.set_transfer_function(tf));
    }

    /// Get the Cicp encoding of this buffer's color data.
    pub fn color_space(&self) -> Cicp {
        dynamic_map!(self, ref p, p.color_space())
    }

    /// Set primaries and transfer characteristics from a Cicp color space.
    ///
    /// Returns an error if `cicp` uses features that are not support with an RGB color space, e.g.
    /// a matrix or narrow range (studio encoding) channels.
    pub fn set_color_space(&mut self, cicp: Cicp) -> ImageResult<()> {
        dynamic_map!(self, ref mut p, p.set_color_space(cicp))
    }

    /// Whether the image contains an alpha channel
    ///
    /// This is a convenience wrapper around `self.color().has_alpha()`.
    /// For inspecting other properties of the color type you should call
    /// [DynamicImage::color] and use the methods on the returned [ColorType](color::ColorType).
    ///
    /// This only checks that the image's pixel type can express transparency,
    /// not whether the image actually has any transparent areas.
    #[must_use]
    pub fn has_alpha(&self) -> bool {
        self.color().has_alpha()
    }

    /// Return a grayscale version of this image.
    /// Returns `Luma` images in most cases. However, for `f32` images,
    /// this will return a grayscale `Rgb/Rgba` image instead.
    #[must_use]
    pub fn grayscale(&self) -> DynamicImage {
        match *self {
            DynamicImage::ImageLuma8(ref p) => DynamicImage::ImageLuma8(p.clone()),
            DynamicImage::ImageLumaA8(ref p) => {
                DynamicImage::ImageLumaA8(imageops::grayscale_alpha(p))
            }
            DynamicImage::ImageRgb8(ref p) => DynamicImage::ImageLuma8(imageops::grayscale(p)),
            DynamicImage::ImageRgba8(ref p) => {
                DynamicImage::ImageLumaA8(imageops::grayscale_alpha(p))
            }
            DynamicImage::ImageLuma16(ref p) => DynamicImage::ImageLuma16(p.clone()),
            DynamicImage::ImageLumaA16(ref p) => {
                DynamicImage::ImageLumaA16(imageops::grayscale_alpha(p))
            }
            DynamicImage::ImageRgb16(ref p) => DynamicImage::ImageLuma16(imageops::grayscale(p)),
            DynamicImage::ImageRgba16(ref p) => {
                DynamicImage::ImageLumaA16(imageops::grayscale_alpha(p))
            }
            DynamicImage::ImageRgb32F(ref p) => {
                DynamicImage::ImageRgb32F(imageops::grayscale_with_type(p))
            }
            DynamicImage::ImageRgba32F(ref p) => {
                DynamicImage::ImageRgba32F(imageops::grayscale_with_type_alpha(p))
            }
        }
    }

    /// Invert the colors of this image.
    /// This method operates inplace.
    ///
    /// This method operates on pixel channel values directly without taking into account color
    /// space data.
    pub fn invert(&mut self) {
        dynamic_map!(*self, ref mut p, imageops::invert(p));
    }

    /// Resize this image using the specified filter algorithm.
    /// Returns a new image. The image's aspect ratio is preserved.
    /// The image is scaled to the maximum possible size that fits
    /// within the bounds specified by `nwidth` and `nheight`.
    ///
    /// This method operates on pixel channel values directly without taking into account color
    /// space data.
    #[must_use]
    pub fn resize(&self, nwidth: u32, nheight: u32, filter: imageops::FilterType) -> DynamicImage {
        if (nwidth, nheight) == self.dimensions() {
            return self.clone();
        }
        let (width2, height2) =
            resize_dimensions(self.width(), self.height(), nwidth, nheight, false);

        self.resize_exact(width2, height2, filter)
    }

    /// Resize this image using the specified filter algorithm.
    /// Returns a new image. Does not preserve aspect ratio.
    /// `nwidth` and `nheight` are the new image's dimensions
    ///
    /// This method operates on pixel channel values directly without taking into account color
    /// space data.
    #[must_use]
    pub fn resize_exact(
        &self,
        nwidth: u32,
        nheight: u32,
        filter: imageops::FilterType,
    ) -> DynamicImage {
        dynamic_map!(*self, ref p => imageops::resize(p, nwidth, nheight, filter))
    }

    /// Scale this image down to fit within a specific size.
    /// Returns a new image. The image's aspect ratio is preserved.
    /// The image is scaled to the maximum possible size that fits
    /// within the bounds specified by `nwidth` and `nheight`.
    ///
    /// This method uses a fast integer algorithm where each source
    /// pixel contributes to exactly one target pixel.
    /// May give aliasing artifacts if new size is close to old size.
    ///
    /// This method operates on pixel channel values directly without taking into account color
    /// space data.
    #[must_use]
    pub fn thumbnail(&self, nwidth: u32, nheight: u32) -> DynamicImage {
        let (width2, height2) =
            resize_dimensions(self.width(), self.height(), nwidth, nheight, false);
        self.thumbnail_exact(width2, height2)
    }

    /// Scale this image down to a specific size.
    /// Returns a new image. Does not preserve aspect ratio.
    /// `nwidth` and `nheight` are the new image's dimensions.
    /// This method uses a fast integer algorithm where each source
    /// pixel contributes to exactly one target pixel.
    /// May give aliasing artifacts if new size is close to old size.
    ///
    /// This method operates on pixel channel values directly without taking into account color
    /// space data.
    #[must_use]
    pub fn thumbnail_exact(&self, nwidth: u32, nheight: u32) -> DynamicImage {
        dynamic_map!(*self, ref p => imageops::thumbnail(p, nwidth, nheight))
    }

    /// Resize this image using the specified filter algorithm.
    /// Returns a new image. The image's aspect ratio is preserved.
    /// The image is scaled to the maximum possible size that fits
    /// within the larger (relative to aspect ratio) of the bounds
    /// specified by `nwidth` and `nheight`, then cropped to
    /// fit within the other bound.
    ///
    /// This method operates on pixel channel values directly without taking into account color
    /// space data.
    #[must_use]
    pub fn resize_to_fill(
        &self,
        nwidth: u32,
        nheight: u32,
        filter: imageops::FilterType,
    ) -> DynamicImage {
        let (width2, height2) =
            resize_dimensions(self.width(), self.height(), nwidth, nheight, true);

        let mut intermediate = self.resize_exact(width2, height2, filter);
        let (iwidth, iheight) = intermediate.dimensions();
        let ratio = u64::from(iwidth) * u64::from(nheight);
        let nratio = u64::from(nwidth) * u64::from(iheight);

        if nratio > ratio {
            intermediate.crop(0, (iheight - nheight) / 2, nwidth, nheight)
        } else {
            intermediate.crop((iwidth - nwidth) / 2, 0, nwidth, nheight)
        }
    }

    /// Performs a Gaussian blur on this image.
    ///
    /// # Arguments
    ///
    /// * `sigma` - gaussian bell flattening level.
    ///
    /// Use [DynamicImage::fast_blur()] for a faster but less
    /// accurate version.
    ///
    /// This method assumes alpha pre-multiplication for images that contain non-constant alpha.
    /// This method typically assumes that the input is scene-linear light.
    /// If it is not, color distortion may occur.
    ///
    /// This method operates on pixel channel values directly without taking into account color
    /// space data.
    #[must_use]
    pub fn blur(&self, sigma: f32) -> DynamicImage {
        gaussian_blur_dyn_image(
            self,
            GaussianBlurParameters::new_from_sigma(if sigma == 0.0 { 0.8 } else { sigma }),
        )
    }

    /// Performs a Gaussian blur on this image.
    ///
    /// # Arguments
    ///
    /// * `parameters` - see [GaussianBlurParameters] for more info
    ///
    /// This method assumes alpha pre-multiplication for images that contain non-constant alpha.
    /// This method typically assumes that the input is scene-linear light.
    /// If it is not, color distortion may occur.
    ///
    /// This method operates on pixel channel values directly without taking into account color
    /// space data.
    #[must_use]
    pub fn blur_advanced(&self, parameters: GaussianBlurParameters) -> DynamicImage {
        gaussian_blur_dyn_image(self, parameters)
    }

    /// Performs a fast blur on this image.
    ///
    /// # Arguments
    ///
    /// * `sigma` - value controls image flattening level.
    ///
    /// This method typically assumes that the input is scene-linear light.
    /// If it is not, color distortion may occur.
    ///
    /// This method operates on pixel channel values directly without taking into account color
    /// space data.
    #[must_use]
    pub fn fast_blur(&self, sigma: f32) -> DynamicImage {
        dynamic_map!(*self, ref p => imageops::fast_blur(p, sigma))
    }

    /// Performs an unsharpen mask on this image.
    ///
    /// # Arguments
    ///
    /// * `sigma` - value controls image flattening level.
    /// * `threshold` - is a control of how much to sharpen.
    ///
    /// This method typically assumes that the input is scene-linear light. If it is not, color
    /// distortion may occur. It operates on pixel channel values directly without taking into
    /// account color space data.
    ///
    /// See [Digital unsharp masking](https://en.wikipedia.org/wiki/Unsharp_masking#Digital_unsharp_masking)
    /// for more information
    #[must_use]
    pub fn unsharpen(&self, sigma: f32, threshold: i32) -> DynamicImage {
        dynamic_map!(*self, ref p => imageops::unsharpen(p, sigma, threshold))
    }

    /// Filters this image with the specified 3x3 kernel.
    ///
    /// # Arguments
    ///
    /// * `kernel` - slice contains filter. Only slice len is 9 length is accepted.
    ///
    /// This method typically assumes that the input is scene-linear light. It operates on pixel
    /// channel values directly without taking into account color space data. If it is not, color
    /// distortion may occur.
    #[must_use]
    pub fn filter3x3(&self, kernel: &[f32]) -> DynamicImage {
        assert_eq!(9, kernel.len(), "filter must be 3 x 3");

        dynamic_map!(*self, ref p => imageops::filter3x3(p, kernel))
    }

    /// Adjust the contrast of this image.
    /// `contrast` is the amount to adjust the contrast by.
    /// Negative values decrease the contrast and positive values increase the contrast.
    ///
    /// This method operates on pixel channel values directly without taking into account color
    /// space data.
    #[must_use]
    pub fn adjust_contrast(&self, c: f32) -> DynamicImage {
        dynamic_map!(*self, ref p => imageops::contrast(p, c))
    }

    /// Brighten the pixels of this image.
    /// `value` is the amount to brighten each pixel by.
    /// Negative values decrease the brightness and positive values increase it.
    ///
    /// This method operates on pixel channel values directly without taking into account color
    /// space data.
    #[must_use]
    pub fn brighten(&self, value: i32) -> DynamicImage {
        dynamic_map!(*self, ref p => imageops::brighten(p, value))
    }

    /// Hue rotate the supplied image.
    /// `value` is the degrees to rotate each pixel by.
    /// 0 and 360 do nothing, the rest rotates by the given degree value.
    /// just like the css webkit filter hue-rotate(180)
    ///
    /// This method operates on pixel channel values directly without taking into account color
    /// space data. The HSV color space is dependent on the current color space primaries.
    #[must_use]
    pub fn huerotate(&self, value: i32) -> DynamicImage {
        dynamic_map!(*self, ref p => imageops::huerotate(p, value))
    }

    /// Flip this image vertically
    ///
    /// Use [`apply_orientation`](Self::apply_orientation) if you want to flip the image in-place instead.
    #[must_use]
    pub fn flipv(&self) -> DynamicImage {
        dynamic_map!(*self, ref p => imageops::flip_vertical(p))
    }

    /// Flip this image vertically in place
    fn flipv_in_place(&mut self) {
        dynamic_map!(*self, ref mut p, imageops::flip_vertical_in_place(p))
    }

    /// Flip this image horizontally
    ///
    /// Use [`apply_orientation`](Self::apply_orientation) if you want to flip the image in-place.
    #[must_use]
    pub fn fliph(&self) -> DynamicImage {
        dynamic_map!(*self, ref p => imageops::flip_horizontal(p))
    }

    /// Flip this image horizontally in place
    fn fliph_in_place(&mut self) {
        dynamic_map!(*self, ref mut p, imageops::flip_horizontal_in_place(p))
    }

    /// Rotate this image 90 degrees clockwise.
    #[must_use]
    pub fn rotate90(&self) -> DynamicImage {
        dynamic_map!(*self, ref p => imageops::rotate90(p))
    }

    /// Rotate this image 180 degrees.
    ///
    /// Use [`apply_orientation`](Self::apply_orientation) if you want to rotate the image in-place.
    #[must_use]
    pub fn rotate180(&self) -> DynamicImage {
        dynamic_map!(*self, ref p => imageops::rotate180(p))
    }

    /// Rotate this image 180 degrees in place.
    fn rotate180_in_place(&mut self) {
        dynamic_map!(*self, ref mut p, imageops::rotate180_in_place(p))
    }

    /// Rotate this image 270 degrees clockwise.
    #[must_use]
    pub fn rotate270(&self) -> DynamicImage {
        dynamic_map!(*self, ref p => imageops::rotate270(p))
    }

    /// Rotates and/or flips the image as indicated by [Orientation].
    ///
    /// This can be used to apply Exif orientation to an image,
    /// e.g. to correctly display a photo taken by a smartphone camera:
    ///
    /// ```
    /// # fn only_check_if_this_compiles() -> Result<(), Box<dyn std::error::Error>> {
    /// use image::{DynamicImage, ImageReader, ImageDecoder};
    ///
    /// let mut decoder = ImageReader::open("file.jpg")?.into_decoder()?;
    /// let orientation = decoder.orientation()?;
    /// let mut image = DynamicImage::from_decoder(decoder)?;
    /// image.apply_orientation(orientation);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Note that for some orientations cannot be efficiently applied in-place.
    /// In that case this function will make a copy of the image internally.
    ///
    /// If this matters to you, please see the documentation on the variants of [Orientation]
    /// to learn which orientations can and cannot be applied without copying.
    pub fn apply_orientation(&mut self, orientation: Orientation) {
        let image = self;
        match orientation {
            Orientation::NoTransforms => (),
            Orientation::Rotate90 => *image = image.rotate90(),
            Orientation::Rotate180 => image.rotate180_in_place(),
            Orientation::Rotate270 => *image = image.rotate270(),
            Orientation::FlipHorizontal => image.fliph_in_place(),
            Orientation::FlipVertical => image.flipv_in_place(),
            Orientation::Rotate90FlipH => {
                let mut new_image = image.rotate90();
                new_image.fliph_in_place();
                *image = new_image;
            }
            Orientation::Rotate270FlipH => {
                let mut new_image = image.rotate270();
                new_image.fliph_in_place();
                *image = new_image;
            }
        }
    }

    /// Copy pixel data from one buffer to another.
    ///
    /// On success, this dynamic image contains color data equivalent to the sources color data.
    /// Neither the color space nor the sample type of `self` is changed, the data representation
    /// is transformed and copied into the current buffer.
    ///
    /// Returns `Ok` if:
    /// - Both images to have the same dimensions, otherwise returns a [`ImageError::Parameter`].
    /// - The primaries and transfer functions of both image's color spaces must be supported,
    ///   otherwise returns a [`ImageError::Unsupported`].
    ///
    /// See also [`Self::apply_color_space`] and [`Self::convert_color_space`] to modify an image
    /// directly.
    ///
    /// ## Accuracy
    ///
    /// All color values are subject to change to their _intended_ values. Please do not rely on
    /// them further than your own colorimetric understanding shows them correct. For instance,
    /// conversion of RGB to their corresponding Luma values needs to be modified in future
    /// versions of this library. Expect colors to be too bright or too dark until further notice.
    pub fn copy_from_color_space(
        &mut self,
        other: &DynamicImage,
        mut options: ConvertColorOptions,
    ) -> ImageResult<()> {
        // Try to no-op this transformation, we may be lucky..
        if self.color_space() == other.color_space() {
            // Nothing to transform, just rescale samples and type cast.
            dynamic_map!(
                self,
                ref mut p,
                *p = dynamic_map!(other, ref o, o.cast_in_color_space())
            );

            return Ok(());
        }

        // Do a transformation from existing buffer to existing buffer, only for color types that
        // are currently supported. Other color types must use the fallback below. If we expand the
        // range of supported color types we must consider how to write this more neatly.
        match (&mut *self, other) {
            // u8 sample types
            (DynamicImage::ImageRgb8(img), DynamicImage::ImageRgb8(other)) => {
                return img.copy_from_color_space(other, options);
            }
            (DynamicImage::ImageRgb8(img), DynamicImage::ImageRgba8(other)) => {
                return img.copy_from_color_space(other, options);
            }
            (DynamicImage::ImageRgba8(img), DynamicImage::ImageRgb8(other)) => {
                return img.copy_from_color_space(other, options);
            }
            (DynamicImage::ImageRgba8(img), DynamicImage::ImageRgba8(other)) => {
                return img.copy_from_color_space(other, options);
            }
            // u16 sample types
            (DynamicImage::ImageRgb16(img), DynamicImage::ImageRgb16(other)) => {
                return img.copy_from_color_space(other, options);
            }
            (DynamicImage::ImageRgb16(img), DynamicImage::ImageRgba16(other)) => {
                return img.copy_from_color_space(other, options);
            }
            (DynamicImage::ImageRgba16(img), DynamicImage::ImageRgb16(other)) => {
                return img.copy_from_color_space(other, options);
            }
            (DynamicImage::ImageRgba16(img), DynamicImage::ImageRgba16(other)) => {
                return img.copy_from_color_space(other, options);
            }
            // 32F sample types.
            (DynamicImage::ImageRgb32F(img), DynamicImage::ImageRgb32F(other)) => {
                return img.copy_from_color_space(other, options);
            }
            (DynamicImage::ImageRgb32F(img), DynamicImage::ImageRgba32F(other)) => {
                return img.copy_from_color_space(other, options);
            }
            (DynamicImage::ImageRgba32F(img), DynamicImage::ImageRgb32F(other)) => {
                return img.copy_from_color_space(other, options);
            }
            (DynamicImage::ImageRgba32F(img), DynamicImage::ImageRgba32F(other)) => {
                return img.copy_from_color_space(other, options);
            }
            _ => {}
        };

        // If we reach here we have a mismatch of sample types. Our conversion supports only input
        // and output of the same sample type. As a simplification we will do the conversion only
        // in `f32` samples. Thus we first convert the source to `f32` samples taking care it does
        // not involve (lossy) color conversion (into with luma -> rgb for instance). Note: we do
        // not have Luma<f32> as a type.
        let cicp = options.as_transform(other.color_space(), self.color_space())?;
        cicp.transform_dynamic(self, other);

        Ok(())
    }

    /// Change the color space, modifying pixel values to refer to the same colors.
    ///
    /// On success, this dynamic image contains color data equivalent to its previous color data.
    /// The sample type of `self` is not changed, the data representation is transformed within the
    /// current buffer.
    ///
    /// Returns `Ok` if:
    /// - The primaries and transfer functions of both image's color spaces must be supported,
    ///   otherwise returns a [`ImageError::Unsupported`].
    /// - The target `Cicp` must have full range and an `Identity` matrix. (This library's
    ///   [`Luma`][`crate::Luma`] refers implicity to a chromaticity derived non-constant luminance
    ///   color).
    ///
    /// See also [`Self::copy_from_color_space`].
    pub fn apply_color_space(
        &mut self,
        cicp: Cicp,
        options: ConvertColorOptions,
    ) -> ImageResult<()> {
        // If the color space is already set, we can just return.
        if self.color_space() == cicp {
            return Ok(());
        }

        // We could conceivably do this in-place faster but to handle the Luma conversion as we
        // want this requires the full machinery as `CicpTransform::transform_dynamic` which is
        // quite the replication. Let's just see if it is fast enough. Feel free to PR something if
        // it is easy enough to review.
        let mut target = self.clone();
        target.set_color_space(cicp)?;
        target.copy_from_color_space(self, options)?;

        *self = target;
        Ok(())
    }

    /// Change the color space and pixel type of this image.
    ///
    /// On success, this dynamic image contains color data equivalent to its previous color data
    /// with another type of pixels.
    ///
    /// Returns `Ok` if:
    /// - The primaries and transfer functions of both image's color spaces must be supported,
    ///   otherwise returns a [`ImageError::Unsupported`].
    /// - The target `Cicp` must have full range and an `Identity` matrix. (This library's
    ///   [`Luma`][`crate::Luma`] refers implicity to a chromaticity derived non-constant luminance
    ///   color).
    ///
    /// See also [`Self::copy_from_color_space`].
    pub fn convert_color_space(
        &mut self,
        cicp: Cicp,
        options: ConvertColorOptions,
        color: color::ColorType,
    ) -> ImageResult<()> {
        if self.color() == color {
            return self.apply_color_space(cicp, options);
        }

        // Forward compatibility: make sure we do not drop any details here.
        let rgb = cicp.try_into_rgb()?;
        let mut target = DynamicImage::new(self.width(), self.height(), color);
        dynamic_map!(target, ref mut p, p.set_rgb_color_space(rgb));
        target.copy_from_color_space(self, options)?;

        *self = target;
        Ok(())
    }

    fn write_with_encoder_impl<'a>(
        &self,
        encoder: Box<dyn ImageEncoderBoxed + 'a>,
    ) -> ImageResult<()> {
        let converted = encoder.make_compatible_img(crate::io::encoder::MethodSealedToImage, self);
        let img = converted.as_ref().unwrap_or(self);

        encoder.write_image(
            img.as_bytes(),
            img.width(),
            img.height(),
            img.color().into(),
        )
    }

    /// Encode this image and write it to `w`.
    ///
    /// Assumes the writer is buffered. In most cases, you should wrap your writer in a `BufWriter`
    /// for best performance.
    ///
    /// ## Color Conversion
    ///
    /// Unlike other encoding methods in this crate, methods on `DynamicImage` try to automatically
    /// convert the image to some color type supported by the encoder. This may result in a loss of
    /// precision or the removal of the alpha channel.
    pub fn write_to<W: Write + Seek>(&self, mut w: W, format: ImageFormat) -> ImageResult<()> {
        let encoder = encoder_for_format(format, &mut w)?;
        self.write_with_encoder_impl(encoder)
    }

    /// Encode this image with the provided encoder.
    ///
    /// ## Color Conversion
    ///
    /// Unlike other encoding methods in this crate, methods on `DynamicImage` try to automatically
    /// convert the image to some color type supported by the encoder. This may result in a loss of
    /// precision or the removal of the alpha channel.
    pub fn write_with_encoder(&self, encoder: impl ImageEncoder) -> ImageResult<()> {
        self.write_with_encoder_impl(Box::new(encoder))
    }

    /// Saves the buffer to a file with the format derived from the file extension.
    ///
    /// ## Color Conversion
    ///
    /// Unlike other encoding methods in this crate, methods on `DynamicImage` try to automatically
    /// convert the image to some color type supported by the encoder. This may result in a loss of
    /// precision or the removal of the alpha channel.
    pub fn save<Q>(&self, path: Q) -> ImageResult<()>
    where
        Q: AsRef<Path>,
    {
        let format = ImageFormat::from_path(path.as_ref())?;
        self.save_with_format(path, format)
    }

    /// Saves the buffer to a file with the specified format.
    ///
    /// ## Color Conversion
    ///
    /// Unlike other encoding methods in this crate, methods on `DynamicImage` try to automatically
    /// convert the image to some color type supported by the encoder. This may result in a loss of
    /// precision or the removal of the alpha channel.
    pub fn save_with_format<Q>(&self, path: Q, format: ImageFormat) -> ImageResult<()>
    where
        Q: AsRef<Path>,
    {
        let file = &mut BufWriter::new(File::create(path)?);
        let encoder = encoder_for_format(format, file)?;
        self.write_with_encoder_impl(encoder)
    }
}

impl From<GrayImage> for DynamicImage {
    fn from(image: GrayImage) -> Self {
        DynamicImage::ImageLuma8(image)
    }
}

impl From<GrayAlphaImage> for DynamicImage {
    fn from(image: GrayAlphaImage) -> Self {
        DynamicImage::ImageLumaA8(image)
    }
}

impl From<RgbImage> for DynamicImage {
    fn from(image: RgbImage) -> Self {
        DynamicImage::ImageRgb8(image)
    }
}

impl From<RgbaImage> for DynamicImage {
    fn from(image: RgbaImage) -> Self {
        DynamicImage::ImageRgba8(image)
    }
}

impl From<Gray16Image> for DynamicImage {
    fn from(image: Gray16Image) -> Self {
        DynamicImage::ImageLuma16(image)
    }
}

impl From<GrayAlpha16Image> for DynamicImage {
    fn from(image: GrayAlpha16Image) -> Self {
        DynamicImage::ImageLumaA16(image)
    }
}

impl From<Rgb16Image> for DynamicImage {
    fn from(image: Rgb16Image) -> Self {
        DynamicImage::ImageRgb16(image)
    }
}

impl From<Rgba16Image> for DynamicImage {
    fn from(image: Rgba16Image) -> Self {
        DynamicImage::ImageRgba16(image)
    }
}

impl From<Rgb32FImage> for DynamicImage {
    fn from(image: Rgb32FImage) -> Self {
        DynamicImage::ImageRgb32F(image)
    }
}

impl From<Rgba32FImage> for DynamicImage {
    fn from(image: Rgba32FImage) -> Self {
        DynamicImage::ImageRgba32F(image)
    }
}

impl From<ImageBuffer<Luma<f32>, Vec<f32>>> for DynamicImage {
    fn from(image: ImageBuffer<Luma<f32>, Vec<f32>>) -> Self {
        DynamicImage::ImageRgb32F(image.convert())
    }
}

impl From<ImageBuffer<LumaA<f32>, Vec<f32>>> for DynamicImage {
    fn from(image: ImageBuffer<LumaA<f32>, Vec<f32>>) -> Self {
        DynamicImage::ImageRgba32F(image.convert())
    }
}

#[allow(deprecated)]
impl GenericImageView for DynamicImage {
    type Pixel = color::Rgba<u8>; // TODO use f32 as default for best precision and unbounded color?

    fn dimensions(&self) -> (u32, u32) {
        dynamic_map!(*self, ref p, p.dimensions())
    }

    fn get_pixel(&self, x: u32, y: u32) -> color::Rgba<u8> {
        dynamic_map!(*self, ref p, p.get_pixel(x, y).to_rgba().into_color())
    }
}

#[allow(deprecated)]
impl GenericImage for DynamicImage {
    fn put_pixel(&mut self, x: u32, y: u32, pixel: color::Rgba<u8>) {
        match *self {
            DynamicImage::ImageLuma8(ref mut p) => p.put_pixel(x, y, pixel.to_luma()),
            DynamicImage::ImageLumaA8(ref mut p) => p.put_pixel(x, y, pixel.to_luma_alpha()),
            DynamicImage::ImageRgb8(ref mut p) => p.put_pixel(x, y, pixel.to_rgb()),
            DynamicImage::ImageRgba8(ref mut p) => p.put_pixel(x, y, pixel),
            DynamicImage::ImageLuma16(ref mut p) => p.put_pixel(x, y, pixel.to_luma().into_color()),
            DynamicImage::ImageLumaA16(ref mut p) => {
                p.put_pixel(x, y, pixel.to_luma_alpha().into_color());
            }
            DynamicImage::ImageRgb16(ref mut p) => p.put_pixel(x, y, pixel.to_rgb().into_color()),
            DynamicImage::ImageRgba16(ref mut p) => p.put_pixel(x, y, pixel.into_color()),
            DynamicImage::ImageRgb32F(ref mut p) => p.put_pixel(x, y, pixel.to_rgb().into_color()),
            DynamicImage::ImageRgba32F(ref mut p) => p.put_pixel(x, y, pixel.into_color()),
        }
    }

    fn blend_pixel(&mut self, x: u32, y: u32, pixel: color::Rgba<u8>) {
        match *self {
            DynamicImage::ImageLuma8(ref mut p) => p.blend_pixel(x, y, pixel.to_luma()),
            DynamicImage::ImageLumaA8(ref mut p) => p.blend_pixel(x, y, pixel.to_luma_alpha()),
            DynamicImage::ImageRgb8(ref mut p) => p.blend_pixel(x, y, pixel.to_rgb()),
            DynamicImage::ImageRgba8(ref mut p) => p.blend_pixel(x, y, pixel),
            DynamicImage::ImageLuma16(ref mut p) => {
                p.blend_pixel(x, y, pixel.to_luma().into_color());
            }
            DynamicImage::ImageLumaA16(ref mut p) => {
                p.blend_pixel(x, y, pixel.to_luma_alpha().into_color());
            }
            DynamicImage::ImageRgb16(ref mut p) => p.blend_pixel(x, y, pixel.to_rgb().into_color()),
            DynamicImage::ImageRgba16(ref mut p) => p.blend_pixel(x, y, pixel.into_color()),
            DynamicImage::ImageRgb32F(ref mut p) => {
                p.blend_pixel(x, y, pixel.to_rgb().into_color());
            }
            DynamicImage::ImageRgba32F(ref mut p) => p.blend_pixel(x, y, pixel.into_color()),
        }
    }

    /// Do not use is function: It is unimplemented!
    fn get_pixel_mut(&mut self, _: u32, _: u32) -> &mut color::Rgba<u8> {
        unimplemented!()
    }
}

impl Default for DynamicImage {
    fn default() -> Self {
        Self::ImageRgba8(Default::default())
    }
}

/// Decodes an image and stores it into a dynamic image
fn decoder_to_image<I: ImageDecoder>(decoder: I) -> ImageResult<DynamicImage> {
    let (w, h) = decoder.dimensions();
    let color_type = decoder.color_type();

    let mut image = match color_type {
        color::ColorType::Rgb8 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgb8)
        }

        color::ColorType::Rgba8 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgba8)
        }

        color::ColorType::L8 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageLuma8)
        }

        color::ColorType::La8 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageLumaA8)
        }

        color::ColorType::Rgb16 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgb16)
        }

        color::ColorType::Rgba16 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgba16)
        }

        color::ColorType::Rgb32F => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgb32F)
        }

        color::ColorType::Rgba32F => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgba32F)
        }

        color::ColorType::L16 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageLuma16)
        }

        color::ColorType::La16 => {
            let buf = free_functions::decoder_to_vec(decoder)?;
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageLumaA16)
        }
    }
    .ok_or_else(|| {
        ImageError::Parameter(ParameterError::from_kind(
            ParameterErrorKind::DimensionMismatch,
        ))
    })?;

    // Presume SRGB for now. This is the one we convert into in some decoders and the one that is
    // most widely used in the wild. FIXME: add an API to decoder to indicate the color space as a
    // CICP directly or through interpreting the ICC information.
    image.set_rgb_primaries(Cicp::SRGB.primaries);
    image.set_transfer_function(Cicp::SRGB.transfer);

    Ok(image)
}

/// Open the image located at the path specified.
/// The image's format is determined from the path's file extension.
///
/// Try [`ImageReader`] for more advanced uses, including guessing the format based on the file's
/// content before its path.
pub fn open<P>(path: P) -> ImageResult<DynamicImage>
where
    P: AsRef<Path>,
{
    ImageReader::open(path)?.decode()
}

/// Read a tuple containing the (width, height) of the image located at the specified path.
/// This is faster than fully loading the image and then getting its dimensions.
///
/// Try [`ImageReader`] for more advanced uses, including guessing the format based on the file's
/// content before its path or manually supplying the format.
pub fn image_dimensions<P>(path: P) -> ImageResult<(u32, u32)>
where
    P: AsRef<Path>,
{
    ImageReader::open(path)?.into_dimensions()
}

/// Writes the supplied buffer to a writer in the specified format.
///
/// The buffer is assumed to have the correct format according to the specified color type. This
/// will lead to corrupted writers if the buffer contains malformed data.
///
/// Assumes the writer is buffered. In most cases, you should wrap your writer in a `BufWriter` for
/// best performance.
pub fn write_buffer_with_format<W: Write + Seek>(
    buffered_writer: &mut W,
    buf: &[u8],
    width: u32,
    height: u32,
    color: impl Into<ExtendedColorType>,
    format: ImageFormat,
) -> ImageResult<()> {
    let encoder = encoder_for_format(format, buffered_writer)?;
    encoder.write_image(buf, width, height, color.into())
}

/// Create a new image from a byte slice
///
/// Makes an educated guess about the image format.
/// TGA is not supported by this function.
///
/// Try [`ImageReader`] for more advanced uses.
pub fn load_from_memory(buffer: &[u8]) -> ImageResult<DynamicImage> {
    let format = free_functions::guess_format(buffer)?;
    load_from_memory_with_format(buffer, format)
}

/// Create a new image from a byte slice
///
/// This is just a simple wrapper that constructs an `std::io::Cursor` around the buffer and then
/// calls `load` with that reader.
///
/// Try [`ImageReader`] for more advanced uses.
///
/// [`load`]: fn.load.html
#[inline(always)]
pub fn load_from_memory_with_format(buf: &[u8], format: ImageFormat) -> ImageResult<DynamicImage> {
    // Note: this function (and `load_from_memory`) where supposed to be generic over `AsRef<[u8]>`
    // so that we do not monomorphize copies of all our decoders unless some downsteam crate
    // actually calls one of these functions. See https://github.com/image-rs/image/pull/2470.
    //
    // However the type inference break of this is apparently quite large in the ecosystem so for
    // now they are unfortunately not. See https://github.com/image-rs/image/issues/2585.
    let b = io::Cursor::new(buf);
    free_functions::load(b, format)
}

#[cfg(test)]
mod bench {
    #[bench]
    #[cfg(feature = "benchmarks")]
    fn bench_conversion(b: &mut test::Bencher) {
        let a = super::DynamicImage::ImageRgb8(crate::ImageBuffer::new(1000, 1000));
        b.iter(|| a.to_luma8());
        b.bytes = 1000 * 1000 * 3;
    }
}

#[cfg(test)]
mod test {
    use crate::metadata::{CicpColorPrimaries, CicpTransform};
    use crate::ConvertColorOptions;
    use crate::{color::ColorType, images::dynimage::Gray16Image};
    use crate::{metadata::Cicp, ImageBuffer, Luma, Rgb, Rgba};

    #[test]
    fn test_empty_file() {
        assert!(super::load_from_memory(b"").is_err());
    }

    #[cfg(feature = "jpeg")]
    #[test]
    fn image_dimensions() {
        let im_path = "./tests/images/jpg/progressive/cat.jpg";
        let dims = super::image_dimensions(im_path).unwrap();
        assert_eq!(dims, (320, 240));
    }

    #[cfg(feature = "png")]
    #[test]
    fn open_16bpc_png() {
        let im_path = "./tests/images/png/16bpc/basn6a16.png";
        let image = super::open(im_path).unwrap();
        assert_eq!(image.color(), ColorType::Rgba16);
    }

    fn test_grayscale(mut img: super::DynamicImage, alpha_discarded: bool) {
        use crate::{GenericImage as _, GenericImageView as _};
        img.put_pixel(0, 0, Rgba([255, 0, 0, 100]));
        let expected_alpha = if alpha_discarded { 255 } else { 100 };
        assert_eq!(
            img.grayscale().get_pixel(0, 0),
            Rgba([54, 54, 54, expected_alpha])
        );
    }

    fn test_grayscale_alpha_discarded(img: super::DynamicImage) {
        test_grayscale(img, true);
    }

    fn test_grayscale_alpha_preserved(img: super::DynamicImage) {
        test_grayscale(img, false);
    }

    #[test]
    fn test_grayscale_luma8() {
        test_grayscale_alpha_discarded(super::DynamicImage::new_luma8(1, 1));
        test_grayscale_alpha_discarded(super::DynamicImage::new(1, 1, ColorType::L8));
    }

    #[test]
    fn test_grayscale_luma_a8() {
        test_grayscale_alpha_preserved(super::DynamicImage::new_luma_a8(1, 1));
        test_grayscale_alpha_preserved(super::DynamicImage::new(1, 1, ColorType::La8));
    }

    #[test]
    fn test_grayscale_rgb8() {
        test_grayscale_alpha_discarded(super::DynamicImage::new_rgb8(1, 1));
        test_grayscale_alpha_discarded(super::DynamicImage::new(1, 1, ColorType::Rgb8));
    }

    #[test]
    fn test_grayscale_rgba8() {
        test_grayscale_alpha_preserved(super::DynamicImage::new_rgba8(1, 1));
        test_grayscale_alpha_preserved(super::DynamicImage::new(1, 1, ColorType::Rgba8));
    }

    #[test]
    fn test_grayscale_luma16() {
        test_grayscale_alpha_discarded(super::DynamicImage::new_luma16(1, 1));
        test_grayscale_alpha_discarded(super::DynamicImage::new(1, 1, ColorType::L16));
    }

    #[test]
    fn test_grayscale_luma_a16() {
        test_grayscale_alpha_preserved(super::DynamicImage::new_luma_a16(1, 1));
        test_grayscale_alpha_preserved(super::DynamicImage::new(1, 1, ColorType::La16));
    }

    #[test]
    fn test_grayscale_rgb16() {
        test_grayscale_alpha_discarded(super::DynamicImage::new_rgb16(1, 1));
        test_grayscale_alpha_discarded(super::DynamicImage::new(1, 1, ColorType::Rgb16));
    }

    #[test]
    fn test_grayscale_rgba16() {
        test_grayscale_alpha_preserved(super::DynamicImage::new_rgba16(1, 1));
        test_grayscale_alpha_preserved(super::DynamicImage::new(1, 1, ColorType::Rgba16));
    }

    #[test]
    fn test_grayscale_rgb32f() {
        test_grayscale_alpha_discarded(super::DynamicImage::new_rgb32f(1, 1));
        test_grayscale_alpha_discarded(super::DynamicImage::new(1, 1, ColorType::Rgb32F));
    }

    #[test]
    fn test_grayscale_rgba32f() {
        test_grayscale_alpha_preserved(super::DynamicImage::new_rgba32f(1, 1));
        test_grayscale_alpha_preserved(super::DynamicImage::new(1, 1, ColorType::Rgba32F));
    }

    #[test]
    fn test_dynamic_image_default_implementation() {
        // Test that structs wrapping a DynamicImage are able to auto-derive the Default trait
        // ensures that DynamicImage implements Default (if it didn't, this would cause a compile error).
        #[derive(Default)]
        #[allow(dead_code)]
        struct Foo {
            _image: super::DynamicImage,
        }
    }

    #[test]
    fn test_to_vecu8() {
        let _ = super::DynamicImage::new_luma8(1, 1).into_bytes();
        let _ = super::DynamicImage::new_luma16(1, 1).into_bytes();
    }

    #[test]
    fn issue_1705_can_turn_16bit_image_into_bytes() {
        let pixels = vec![65535u16; 64 * 64];
        let img = ImageBuffer::from_vec(64, 64, pixels).unwrap();

        let img = super::DynamicImage::ImageLuma16(img);
        assert!(img.as_luma16().is_some());

        let bytes: Vec<u8> = img.into_bytes();
        assert_eq!(bytes, vec![0xFF; 64 * 64 * 2]);
    }

    #[test]
    fn test_convert_to() {
        use crate::Luma;
        let image_luma8 = super::DynamicImage::new_luma8(1, 1);
        let image_luma16 = super::DynamicImage::new_luma16(1, 1);
        assert_eq!(image_luma8.to_luma16(), image_luma16.to_luma16());

        // test conversion using typed result
        let conv: Gray16Image = image_luma8.to();
        assert_eq!(image_luma8.to_luma16(), conv);

        // test conversion using turbofish syntax
        let converted = image_luma8.to::<Luma<u16>>();
        assert_eq!(image_luma8.to_luma16(), converted);
    }

    #[test]
    fn color_conversion_srgb_p3() {
        let mut source = super::DynamicImage::ImageRgb8({
            ImageBuffer::from_fn(128, 128, |_, _| Rgb([255, 0, 0]))
        });

        let mut target = super::DynamicImage::ImageRgba8({
            ImageBuffer::from_fn(128, 128, |_, _| Rgba(Default::default()))
        });

        source.set_rgb_primaries(Cicp::SRGB.primaries);
        source.set_transfer_function(Cicp::SRGB.transfer);
        target.set_rgb_primaries(Cicp::DISPLAY_P3.primaries);
        target.set_transfer_function(Cicp::DISPLAY_P3.transfer);

        let result = target.copy_from_color_space(&source, Default::default());

        assert!(result.is_ok(), "{result:?}");
        let target = target.as_rgba8().expect("Sample type unchanged");
        assert_eq!(target[(0, 0)], Rgba([234u8, 51, 35, 255]));
    }

    #[test]
    fn color_conversion_preserves_sample() {
        let mut source = super::DynamicImage::ImageRgb16({
            ImageBuffer::from_fn(128, 128, |_, _| Rgb([u16::MAX, 0, 0]))
        });

        let mut target = super::DynamicImage::ImageRgba8({
            ImageBuffer::from_fn(128, 128, |_, _| Rgba(Default::default()))
        });

        source.set_rgb_primaries(Cicp::SRGB.primaries);
        source.set_transfer_function(Cicp::SRGB.transfer);
        target.set_rgb_primaries(Cicp::DISPLAY_P3.primaries);
        target.set_transfer_function(Cicp::DISPLAY_P3.transfer);

        let result = target.copy_from_color_space(&source, Default::default());

        assert!(result.is_ok(), "{result:?}");
        let target = target.as_rgba8().expect("Sample type unchanged");
        assert_eq!(target[(0, 0)], Rgba([234u8, 51, 35, 255]));
    }

    #[test]
    fn color_conversion_preserves_sample_in_fastpath() {
        let source = super::DynamicImage::ImageRgb16({
            ImageBuffer::from_fn(128, 128, |_, _| Rgb([u16::MAX, 0, 0]))
        });

        let mut target = super::DynamicImage::ImageRgba8({
            ImageBuffer::from_fn(128, 128, |_, _| Rgba(Default::default()))
        });

        // No color space change takes place, but still sample should be converted.
        let result = target.copy_from_color_space(&source, Default::default());

        assert!(result.is_ok(), "{result:?}");
        let target = target.as_rgba8().expect("Sample type unchanged");
        assert_eq!(target[(0, 0)], Rgba([255u8, 0, 0, 255]));
    }

    #[test]
    fn color_conversion_rgb_to_luma() {
        let source = super::DynamicImage::ImageRgb16({
            ImageBuffer::from_fn(128, 128, |_, _| Rgb([0, u16::MAX, 0]))
        });

        let mut target = super::DynamicImage::ImageLuma8({
            ImageBuffer::from_fn(128, 128, |_, _| Luma(Default::default()))
        });

        // No color space change takes place, but still sample should be converted.
        let result = target.copy_from_color_space(&source, Default::default());

        assert!(result.is_ok(), "{result:?}");
        // FIXME: but the result value is .. not ideal.
        target.as_luma8().expect("Sample type unchanged");
    }

    #[test]
    fn copy_color_space_coverage() {
        const TYPES: [ColorType; 10] = [
            ColorType::L8,
            ColorType::La8,
            ColorType::Rgb8,
            ColorType::Rgba8,
            ColorType::L16,
            ColorType::La16,
            ColorType::Rgb16,
            ColorType::Rgba16,
            ColorType::Rgb32F,
            ColorType::Rgba32F,
        ];

        let transform =
            CicpTransform::new(Cicp::SRGB, Cicp::DISPLAY_P3).expect("Failed to create transform");

        for from in TYPES {
            for to in TYPES {
                let mut source = super::DynamicImage::new(16, 16, from);
                let mut target = super::DynamicImage::new(16, 16, to);

                source.set_rgb_primaries(Cicp::SRGB.primaries);
                source.set_transfer_function(Cicp::SRGB.transfer);

                target.set_rgb_primaries(Cicp::DISPLAY_P3.primaries);
                target.set_transfer_function(Cicp::DISPLAY_P3.transfer);

                target
                    .copy_from_color_space(
                        &source,
                        ConvertColorOptions {
                            transform: Some(transform.clone()),
                            ..Default::default()
                        },
                    )
                    .expect("Failed to convert color space");
            }
        }
    }

    #[test]
    fn apply_color_space() {
        let mut buffer = super::DynamicImage::ImageRgb8({
            ImageBuffer::from_fn(128, 128, |_, _| Rgb([u8::MAX, 0, 0]))
        });

        buffer.set_rgb_primaries(Cicp::SRGB.primaries);
        buffer.set_transfer_function(Cicp::SRGB.transfer);

        buffer
            .apply_color_space(Cicp::DISPLAY_P3, Default::default())
            .unwrap();

        let target = buffer.as_rgb8().expect("Sample type unchanged");
        assert_eq!(target[(0, 0)], Rgb([234u8, 51, 35]));
    }

    #[test]
    fn apply_color_space_coverage() {
        const TYPES: [ColorType; 10] = [
            ColorType::L8,
            ColorType::La8,
            ColorType::Rgb8,
            ColorType::Rgba8,
            ColorType::L16,
            ColorType::La16,
            ColorType::Rgb16,
            ColorType::Rgba16,
            ColorType::Rgb32F,
            ColorType::Rgba32F,
        ];

        let transform =
            CicpTransform::new(Cicp::SRGB, Cicp::DISPLAY_P3).expect("Failed to create transform");

        for buffer in TYPES {
            let mut buffer = super::DynamicImage::new(16, 16, buffer);

            buffer.set_rgb_primaries(Cicp::SRGB.primaries);
            buffer.set_transfer_function(Cicp::SRGB.transfer);

            buffer
                .apply_color_space(
                    Cicp::DISPLAY_P3,
                    ConvertColorOptions {
                        transform: Some(transform.clone()),
                        ..Default::default()
                    },
                )
                .expect("Failed to convert color space");
        }
    }

    #[test]
    fn convert_color_space() {
        let mut buffer = super::DynamicImage::ImageRgb16({
            ImageBuffer::from_fn(128, 128, |_, _| Rgb([u16::MAX, 0, 0]))
        });

        buffer.set_rgb_primaries(Cicp::SRGB.primaries);
        buffer.set_transfer_function(Cicp::SRGB.transfer);

        buffer
            .convert_color_space(Cicp::DISPLAY_P3, Default::default(), ColorType::Rgb8)
            .unwrap();

        let target = buffer.as_rgb8().expect("Sample type now rgb8");
        assert_eq!(target[(0, 0)], Rgb([234u8, 51, 35]));
    }

    #[test]
    fn into_luma_is_color_space_aware() {
        let mut buffer = super::DynamicImage::ImageRgb16({
            ImageBuffer::from_fn(128, 128, |_, _| Rgb([u16::MAX, 0, 0]))
        });

        buffer.set_rgb_primaries(Cicp::DISPLAY_P3.primaries);
        buffer.set_transfer_function(Cicp::DISPLAY_P3.transfer);

        let luma8 = buffer.clone().into_luma8();
        assert_eq!(luma8[(0, 0)], Luma([58u8]));
        assert_eq!(luma8.color_space(), Cicp::DISPLAY_P3);

        buffer.set_rgb_primaries(Cicp::SRGB.primaries);

        let luma8 = buffer.clone().into_luma8();
        assert_eq!(luma8[(0, 0)], Luma([54u8]));
        assert_ne!(luma8.color_space(), Cicp::DISPLAY_P3);
    }

    #[test]
    fn from_luma_is_color_space_aware() {
        let mut buffer = super::DynamicImage::ImageLuma16({
            ImageBuffer::from_fn(128, 128, |_, _| Luma([u16::MAX]))
        });

        buffer.set_rgb_primaries(Cicp::DISPLAY_P3.primaries);
        buffer.set_transfer_function(Cicp::DISPLAY_P3.transfer);

        let rgb8 = buffer.clone().into_rgb8();
        assert_eq!(rgb8[(0, 0)], Rgb([u8::MAX; 3]));
        assert_eq!(rgb8.color_space(), Cicp::DISPLAY_P3);

        buffer.set_rgb_primaries(Cicp::SRGB.primaries);

        let rgb8 = buffer.clone().into_rgb8();
        assert_eq!(rgb8[(0, 0)], Rgb([u8::MAX; 3]));
        assert_ne!(rgb8.color_space(), Cicp::DISPLAY_P3);
    }

    #[test]
    fn from_luma_for_all_chromaticities() {
        const CHROMA: &[CicpColorPrimaries] = &[
            (CicpColorPrimaries::SRgb),
            (CicpColorPrimaries::RgbM),
            (CicpColorPrimaries::RgbB),
            (CicpColorPrimaries::Bt601),
            (CicpColorPrimaries::Rgb240m),
            (CicpColorPrimaries::GenericFilm),
            (CicpColorPrimaries::Rgb2020),
            // Note: here red=X and blue=Z and both are free of luminance
            (CicpColorPrimaries::Xyz),
            (CicpColorPrimaries::SmpteRp431),
            (CicpColorPrimaries::SmpteRp432),
            (CicpColorPrimaries::Industry22),
            // Falls back to sRGB
            (CicpColorPrimaries::Unspecified),
        ];

        let mut buffer = super::DynamicImage::ImageLuma16({
            ImageBuffer::from_fn(128, 128, |_, _| Luma([u16::MAX]))
        });

        for &chroma in CHROMA {
            buffer.set_rgb_primaries(chroma);
            let rgb = buffer.to_rgb8();
            assert_eq!(
                rgb[(0, 0)],
                Rgb([u8::MAX; 3]),
                "Failed for chroma: {chroma:?}"
            );
        }
    }

    #[test]
    fn from_rgb_for_all_chromaticities() {
        // The colors following the coefficients must result in a luma that is the square-root
        // length of the coefficient vector, which is unique enough for a test.
        const CHROMA: &[(CicpColorPrimaries, [u8; 3], u8)] = &[
            (CicpColorPrimaries::SRgb, [54, 182, 18], 143),
            (CicpColorPrimaries::RgbM, [76, 150, 29], 114),
            (CicpColorPrimaries::RgbB, [57, 180, 18], 141),
            (CicpColorPrimaries::Bt601, [54, 179, 22], 139),
            (CicpColorPrimaries::Rgb240m, [54, 179, 22], 139),
            (CicpColorPrimaries::GenericFilm, [65, 173, 17], 135),
            (CicpColorPrimaries::Rgb2020, [67, 173, 15], 136),
            // Note: here red=X and blue=Z and both are free of luminance
            (CicpColorPrimaries::Xyz, [0, 255, 0], 255),
            (CicpColorPrimaries::SmpteRp431, [53, 184, 18], 145),
            (CicpColorPrimaries::SmpteRp432, [58, 176, 20], 137),
            (CicpColorPrimaries::Industry22, [59, 171, 24], 131),
            // Falls back to sRGB
            (CicpColorPrimaries::Unspecified, [54, 182, 18], 143),
        ];

        let mut buffer = super::DynamicImage::ImageRgb8({
            ImageBuffer::from_fn(128, 128, |_, _| Rgb([u8::MAX; 3]))
        });

        for &(chroma, rgb, luma) in CHROMA {
            buffer.set_rgb_primaries(chroma);

            for px in buffer.as_mut_rgb8().unwrap().pixels_mut() {
                px.0 = rgb;
            }

            let buf = buffer.to_luma8();
            assert_eq!(buf[(0, 0)], Luma([luma]), "Failed for chroma: {chroma:?}");
        }
    }

    #[test]
    fn convert_color_space_coverage() {
        const TYPES: [ColorType; 10] = [
            ColorType::L8,
            ColorType::La8,
            ColorType::Rgb8,
            ColorType::Rgba8,
            ColorType::L16,
            ColorType::La16,
            ColorType::Rgb16,
            ColorType::Rgba16,
            ColorType::Rgb32F,
            ColorType::Rgba32F,
        ];

        let transform =
            CicpTransform::new(Cicp::SRGB, Cicp::DISPLAY_P3).expect("Failed to create transform");

        for from in TYPES {
            for to in TYPES {
                let mut buffer = super::DynamicImage::new(16, 16, from);

                buffer.set_rgb_primaries(Cicp::SRGB.primaries);
                buffer.set_transfer_function(Cicp::SRGB.transfer);

                let options = ConvertColorOptions {
                    transform: Some(transform.clone()),
                    ..Default::default()
                };

                buffer
                    .convert_color_space(Cicp::DISPLAY_P3, options, to)
                    .expect("Failed to convert color space");
            }
        }
    }

    /// Check that operations that are not cicp-aware behave as such. We introduce new methods (not
    /// based directly on the public imageops interface) at a later point.
    #[cfg(feature = "png")]
    #[test]
    fn color_space_independent_imageops() {
        let im_path = "./tests/images/png/16bpc/basn6a16.png";

        let mut image = super::open(im_path).unwrap();
        let mut clone = image.clone();

        image.set_color_space(Cicp::SRGB).unwrap();
        clone.set_color_space(Cicp::DISPLAY_P3).unwrap();

        const IMAGEOPS: &[&dyn Fn(&super::DynamicImage) -> super::DynamicImage] = &[
            &|img| img.resize(32, 32, crate::imageops::FilterType::Lanczos3),
            &|img| img.resize_exact(32, 32, crate::imageops::FilterType::Lanczos3),
            &|img| img.thumbnail(8, 8),
            &|img| img.thumbnail_exact(8, 8),
            &|img| img.resize_to_fill(32, 32, crate::imageops::FilterType::Lanczos3),
            &|img| img.blur(1.0),
            &|img| {
                img.blur_advanced(
                    crate::imageops::GaussianBlurParameters::new_anisotropic_kernel_size(1.0, 2.0),
                )
            },
            &|img| img.fast_blur(1.0),
            &|img| img.unsharpen(1.0, 3),
            &|img| img.filter3x3(&[0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0]),
            &|img| img.adjust_contrast(0.5),
            &|img| img.brighten(10),
            &|img| img.huerotate(180),
        ];

        for (idx, &op) in IMAGEOPS.iter().enumerate() {
            let result_a = op(&image);
            let result_b = op(&clone);
            assert_eq!(result_a.color_space(), image.color_space(), "{idx}");
            assert_eq!(result_b.color_space(), clone.color_space(), "{idx}");

            assert_ne!(result_a, result_b, "{idx}");
            assert_eq!(result_a.as_bytes(), result_b.as_bytes(), "{idx}");
        }
    }
}
