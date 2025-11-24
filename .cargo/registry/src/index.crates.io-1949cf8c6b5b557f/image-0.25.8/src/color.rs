use std::ops::{Index, IndexMut};

use num_traits::{NumCast, ToPrimitive, Zero};

use crate::{
    error::TryFromExtendedColorError,
    traits::{Enlargeable, Pixel, Primitive},
};

/// An enumeration over supported color types and bit depths
#[derive(Copy, PartialEq, Eq, Debug, Clone, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ColorType {
    /// Pixel is 8-bit luminance
    L8,
    /// Pixel is 8-bit luminance with an alpha channel
    La8,
    /// Pixel contains 8-bit R, G and B channels
    Rgb8,
    /// Pixel is 8-bit RGB with an alpha channel
    Rgba8,

    /// Pixel is 16-bit luminance
    L16,
    /// Pixel is 16-bit luminance with an alpha channel
    La16,
    /// Pixel is 16-bit RGB
    Rgb16,
    /// Pixel is 16-bit RGBA
    Rgba16,

    /// Pixel is 32-bit float RGB
    Rgb32F,
    /// Pixel is 32-bit float RGBA
    Rgba32F,
}

impl ColorType {
    /// Returns the number of bytes contained in a pixel of `ColorType` ```c```
    #[must_use]
    pub fn bytes_per_pixel(self) -> u8 {
        match self {
            ColorType::L8 => 1,
            ColorType::L16 | ColorType::La8 => 2,
            ColorType::Rgb8 => 3,
            ColorType::Rgba8 | ColorType::La16 => 4,
            ColorType::Rgb16 => 6,
            ColorType::Rgba16 => 8,
            ColorType::Rgb32F => 3 * 4,
            ColorType::Rgba32F => 4 * 4,
        }
    }

    /// Returns if there is an alpha channel.
    #[must_use]
    pub fn has_alpha(self) -> bool {
        use ColorType::*;
        match self {
            L8 | L16 | Rgb8 | Rgb16 | Rgb32F => false,
            La8 | Rgba8 | La16 | Rgba16 | Rgba32F => true,
        }
    }

    /// Returns false if the color scheme is grayscale, true otherwise.
    #[must_use]
    pub fn has_color(self) -> bool {
        use ColorType::*;
        match self {
            L8 | L16 | La8 | La16 => false,
            Rgb8 | Rgb16 | Rgba8 | Rgba16 | Rgb32F | Rgba32F => true,
        }
    }

    /// Returns the number of bits contained in a pixel of `ColorType` ```c``` (which will always be
    /// a multiple of 8).
    #[must_use]
    pub fn bits_per_pixel(self) -> u16 {
        <u16 as From<u8>>::from(self.bytes_per_pixel()) * 8
    }

    /// Returns the number of color channels that make up this pixel
    #[must_use]
    pub fn channel_count(self) -> u8 {
        let e: ExtendedColorType = self.into();
        e.channel_count()
    }
}

/// An enumeration of color types encountered in image formats.
///
/// This is not exhaustive over all existing image formats but should be granular enough to allow
/// round tripping of decoding and encoding as much as possible. The variants will be extended as
/// necessary to enable this.
///
/// Another purpose is to advise users of a rough estimate of the accuracy and effort of the
/// decoding from and encoding to such an image format.
#[derive(Copy, PartialEq, Eq, Debug, Clone, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ExtendedColorType {
    /// Pixel is 8-bit alpha
    A8,
    /// Pixel is 1-bit luminance
    L1,
    /// Pixel is 1-bit luminance with an alpha channel
    La1,
    /// Pixel contains 1-bit R, G and B channels
    Rgb1,
    /// Pixel is 1-bit RGB with an alpha channel
    Rgba1,
    /// Pixel is 2-bit luminance
    L2,
    /// Pixel is 2-bit luminance with an alpha channel
    La2,
    /// Pixel contains 2-bit R, G and B channels
    Rgb2,
    /// Pixel is 2-bit RGB with an alpha channel
    Rgba2,
    /// Pixel is 4-bit luminance
    L4,
    /// Pixel is 4-bit luminance with an alpha channel
    La4,
    /// Pixel contains 4-bit R, G and B channels
    Rgb4,
    /// Pixel is 4-bit RGB with an alpha channel
    Rgba4,
    /// Pixel is 8-bit luminance
    L8,
    /// Pixel is 8-bit luminance with an alpha channel
    La8,
    /// Pixel contains 8-bit R, G and B channels
    Rgb8,
    /// Pixel is 8-bit RGB with an alpha channel
    Rgba8,
    /// Pixel is 16-bit luminance
    L16,
    /// Pixel is 16-bit luminance with an alpha channel
    La16,
    /// Pixel contains 16-bit R, G and B channels
    Rgb16,
    /// Pixel is 16-bit RGB with an alpha channel
    Rgba16,
    /// Pixel contains 8-bit B, G and R channels
    Bgr8,
    /// Pixel is 8-bit BGR with an alpha channel
    Bgra8,

    // TODO f16 types?
    /// Pixel is 32-bit float RGB
    Rgb32F,
    /// Pixel is 32-bit float RGBA
    Rgba32F,

    /// Pixel is 8-bit CMYK
    Cmyk8,

    /// Pixel is of unknown color type with the specified bits per pixel. This can apply to pixels
    /// which are associated with an external palette. In that case, the pixel value is an index
    /// into the palette.
    Unknown(u8),
}

impl ExtendedColorType {
    /// Get the number of channels for colors of this type.
    ///
    /// Note that the `Unknown` variant returns a value of `1` since pixels can only be treated as
    /// an opaque datum by the library.
    #[must_use]
    pub fn channel_count(self) -> u8 {
        match self {
            ExtendedColorType::A8
            | ExtendedColorType::L1
            | ExtendedColorType::L2
            | ExtendedColorType::L4
            | ExtendedColorType::L8
            | ExtendedColorType::L16
            | ExtendedColorType::Unknown(_) => 1,
            ExtendedColorType::La1
            | ExtendedColorType::La2
            | ExtendedColorType::La4
            | ExtendedColorType::La8
            | ExtendedColorType::La16 => 2,
            ExtendedColorType::Rgb1
            | ExtendedColorType::Rgb2
            | ExtendedColorType::Rgb4
            | ExtendedColorType::Rgb8
            | ExtendedColorType::Rgb16
            | ExtendedColorType::Rgb32F
            | ExtendedColorType::Bgr8 => 3,
            ExtendedColorType::Rgba1
            | ExtendedColorType::Rgba2
            | ExtendedColorType::Rgba4
            | ExtendedColorType::Rgba8
            | ExtendedColorType::Rgba16
            | ExtendedColorType::Rgba32F
            | ExtendedColorType::Bgra8
            | ExtendedColorType::Cmyk8 => 4,
        }
    }

    /// Returns the number of bits per pixel for this color type.
    #[must_use]
    pub fn bits_per_pixel(&self) -> u16 {
        match *self {
            ExtendedColorType::A8 => 8,
            ExtendedColorType::L1 => 1,
            ExtendedColorType::La1 => 2,
            ExtendedColorType::Rgb1 => 3,
            ExtendedColorType::Rgba1 => 4,
            ExtendedColorType::L2 => 2,
            ExtendedColorType::La2 => 4,
            ExtendedColorType::Rgb2 => 6,
            ExtendedColorType::Rgba2 => 8,
            ExtendedColorType::L4 => 4,
            ExtendedColorType::La4 => 8,
            ExtendedColorType::Rgb4 => 12,
            ExtendedColorType::Rgba4 => 16,
            ExtendedColorType::L8 => 8,
            ExtendedColorType::La8 => 16,
            ExtendedColorType::Rgb8 => 24,
            ExtendedColorType::Rgba8 => 32,
            ExtendedColorType::L16 => 16,
            ExtendedColorType::La16 => 32,
            ExtendedColorType::Rgb16 => 48,
            ExtendedColorType::Rgba16 => 64,
            ExtendedColorType::Rgb32F => 96,
            ExtendedColorType::Rgba32F => 128,
            ExtendedColorType::Bgr8 => 24,
            ExtendedColorType::Bgra8 => 32,
            ExtendedColorType::Cmyk8 => 32,
            ExtendedColorType::Unknown(bpp) => bpp as u16,
        }
    }

    /// Returns the ColorType that is equivalent to this ExtendedColorType.
    ///
    /// # Example
    ///
    /// ```
    /// use image::{ColorType, ExtendedColorType};
    ///
    /// assert_eq!(Some(ColorType::L8), ExtendedColorType::L8.color_type());
    /// assert_eq!(None, ExtendedColorType::L1.color_type());
    /// ```
    ///
    /// The method is equivalent to converting via the `TryFrom`/`TryInto` traits except for the
    /// error path. Choose the more ergonomic option in your usage.
    ///
    /// ```
    /// use image::{ColorType, ExtendedColorType, ImageError};
    ///
    /// fn handle_errors() -> Result<(), ImageError> {
    ///     let color: ColorType = ExtendedColorType::L8.try_into()?;
    ///     assert_eq!(color, ColorType::L8);
    ///     # Ok(())
    /// }
    /// ```
    pub fn color_type(&self) -> Option<ColorType> {
        match *self {
            ExtendedColorType::L8 => Some(ColorType::L8),
            ExtendedColorType::La8 => Some(ColorType::La8),
            ExtendedColorType::Rgb8 => Some(ColorType::Rgb8),
            ExtendedColorType::Rgba8 => Some(ColorType::Rgba8),
            ExtendedColorType::L16 => Some(ColorType::L16),
            ExtendedColorType::La16 => Some(ColorType::La16),
            ExtendedColorType::Rgb16 => Some(ColorType::Rgb16),
            ExtendedColorType::Rgba16 => Some(ColorType::Rgba16),
            ExtendedColorType::Rgb32F => Some(ColorType::Rgb32F),
            ExtendedColorType::Rgba32F => Some(ColorType::Rgba32F),
            _ => None,
        }
    }

    /// Returns the number of bytes required to hold a width x height image of this color type.
    pub(crate) fn buffer_size(self, width: u32, height: u32) -> u64 {
        let bpp = self.bits_per_pixel() as u64;
        let row_pitch = (width as u64 * bpp).div_ceil(8);
        row_pitch.saturating_mul(height as u64)
    }
}

impl From<ColorType> for ExtendedColorType {
    fn from(c: ColorType) -> Self {
        match c {
            ColorType::L8 => ExtendedColorType::L8,
            ColorType::La8 => ExtendedColorType::La8,
            ColorType::Rgb8 => ExtendedColorType::Rgb8,
            ColorType::Rgba8 => ExtendedColorType::Rgba8,
            ColorType::L16 => ExtendedColorType::L16,
            ColorType::La16 => ExtendedColorType::La16,
            ColorType::Rgb16 => ExtendedColorType::Rgb16,
            ColorType::Rgba16 => ExtendedColorType::Rgba16,
            ColorType::Rgb32F => ExtendedColorType::Rgb32F,
            ColorType::Rgba32F => ExtendedColorType::Rgba32F,
        }
    }
}

impl TryFrom<ExtendedColorType> for ColorType {
    type Error = TryFromExtendedColorError;

    fn try_from(value: ExtendedColorType) -> Result<ColorType, Self::Error> {
        value
            .color_type()
            .ok_or(TryFromExtendedColorError { was: value })
    }
}

macro_rules! define_colors {
    {$(
        $(#[$doc:meta])*
        pub struct $ident:ident<T: $($bound:ident)*>([T; $channels:expr, $alphas:expr])
            = $interpretation:literal;
    )*} => {

$( // START Structure definitions

$(#[$doc])*
#[derive(PartialEq, Eq, Clone, Debug, Copy, Hash)]
#[repr(transparent)]
#[allow(missing_docs)]
pub struct $ident<T> (pub [T; $channels]);

impl<T: $($bound+)*> Pixel for $ident<T> {
    type Subpixel = T;

    const CHANNEL_COUNT: u8 = $channels;

    #[inline(always)]
    fn channels(&self) -> &[T] {
        &self.0
    }

    #[inline(always)]
    fn channels_mut(&mut self) -> &mut [T] {
        &mut self.0
    }

    const COLOR_MODEL: &'static str = $interpretation;

    const HAS_ALPHA: bool = $alphas > 0;

    fn channels4(&self) -> (T, T, T, T) {
        const CHANNELS: usize = $channels;
        let mut channels = [T::DEFAULT_MAX_VALUE; 4];
        channels[0..CHANNELS].copy_from_slice(&self.0);
        (channels[0], channels[1], channels[2], channels[3])
    }

    fn from_channels(a: T, b: T, c: T, d: T,) -> $ident<T> {
        const CHANNELS: usize = $channels;
        *<$ident<T> as Pixel>::from_slice(&[a, b, c, d][..CHANNELS])
    }

    fn from_slice(slice: &[T]) -> &$ident<T> {
        assert_eq!(slice.len(), $channels);
        unsafe { &*(slice.as_ptr() as *const $ident<T>) }
    }
    fn from_slice_mut(slice: &mut [T]) -> &mut $ident<T> {
        assert_eq!(slice.len(), $channels);
        unsafe { &mut *(slice.as_mut_ptr() as *mut $ident<T>) }
    }

    fn to_rgb(&self) -> Rgb<T> {
        let mut pix = Rgb([Zero::zero(), Zero::zero(), Zero::zero()]);
        pix.from_color(self);
        pix
    }

    fn to_rgba(&self) -> Rgba<T> {
        let mut pix = Rgba([Zero::zero(), Zero::zero(), Zero::zero(), Zero::zero()]);
        pix.from_color(self);
        pix
    }

    fn to_luma(&self) -> Luma<T> {
        let mut pix = Luma([Zero::zero()]);
        pix.from_color(self);
        pix
    }

    fn to_luma_alpha(&self) -> LumaA<T> {
        let mut pix = LumaA([Zero::zero(), Zero::zero()]);
        pix.from_color(self);
        pix
    }

    fn map<F>(& self, f: F) -> $ident<T> where F: FnMut(T) -> T {
        let mut this = (*self).clone();
        this.apply(f);
        this
    }

    fn apply<F>(&mut self, mut f: F) where F: FnMut(T) -> T {
        for v in &mut self.0 {
            *v = f(*v)
        }
    }

    fn map_with_alpha<F, G>(&self, f: F, g: G) -> $ident<T> where F: FnMut(T) -> T, G: FnMut(T) -> T {
        let mut this = (*self).clone();
        this.apply_with_alpha(f, g);
        this
    }

    fn apply_with_alpha<F, G>(&mut self, mut f: F, mut g: G) where F: FnMut(T) -> T, G: FnMut(T) -> T {
        const ALPHA: usize = $channels - $alphas;
        for v in self.0[..ALPHA].iter_mut() {
            *v = f(*v)
        }
        // The branch of this match is `const`. This way ensures that no subexpression fails the
        // `const_err` lint (the expression `self.0[ALPHA]` would).
        if let Some(v) = self.0.get_mut(ALPHA) {
            *v = g(*v)
        }
    }

    fn map2<F>(&self, other: &Self, f: F) -> $ident<T> where F: FnMut(T, T) -> T {
        let mut this = (*self).clone();
        this.apply2(other, f);
        this
    }

    fn apply2<F>(&mut self, other: &$ident<T>, mut f: F) where F: FnMut(T, T) -> T {
        for (a, &b) in self.0.iter_mut().zip(other.0.iter()) {
            *a = f(*a, b)
        }
    }

    fn invert(&mut self) {
        Invert::invert(self)
    }

    fn blend(&mut self, other: &$ident<T>) {
        Blend::blend(self, other)
    }
}

impl<T> Index<usize> for $ident<T> {
    type Output = T;
    #[inline(always)]
    fn index(&self, _index: usize) -> &T {
        &self.0[_index]
    }
}

impl<T> IndexMut<usize> for $ident<T> {
    #[inline(always)]
    fn index_mut(&mut self, _index: usize) -> &mut T {
        &mut self.0[_index]
    }
}

impl<T> From<[T; $channels]> for $ident<T> {
    fn from(c: [T; $channels]) -> Self {
        Self(c)
    }
}

)* // END Structure definitions

    }
}

define_colors! {
    /// RGB colors.
    ///
    /// For the purpose of color conversion, as well as blending, the implementation of `Pixel`
    /// assumes an `sRGB` color space of its data.
    pub struct Rgb<T: Primitive Enlargeable>([T; 3, 0]) = "RGB";
    /// Grayscale colors.
    pub struct Luma<T: Primitive>([T; 1, 0]) = "Y";
    /// RGB colors + alpha channel
    pub struct Rgba<T: Primitive Enlargeable>([T; 4, 1]) = "RGBA";
    /// Grayscale colors + alpha channel
    pub struct LumaA<T: Primitive>([T; 2, 1]) = "YA";
}

/// Convert from one pixel component type to another. For example, convert from `u8` to `f32` pixel values.
pub trait FromPrimitive<Component> {
    /// Converts from any pixel component type to this type.
    fn from_primitive(component: Component) -> Self;
}

impl<T: Primitive> FromPrimitive<T> for T {
    fn from_primitive(sample: T) -> Self {
        sample
    }
}

// from f32:
// Note that in to-integer-conversion we are performing rounding but NumCast::from is implemented
// as truncate towards zero. We emulate rounding by adding a bias.

// All other special values are clamped inbetween 0.0 and 1.0 (infinities and subnormals)
// NaN however always maps to NaN therefore we have to force it towards some value.
// 1.0 (white) was picked as firefox and chrome choose to map NaN to that.
#[inline]
fn normalize_float(float: f32, max: f32) -> f32 {
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    let clamped = if !(float < 1.0) { 1.0 } else { float.max(0.0) };
    (clamped * max).round()
}

impl FromPrimitive<f32> for u8 {
    fn from_primitive(float: f32) -> Self {
        NumCast::from(normalize_float(float, u8::MAX as f32)).unwrap()
    }
}

impl FromPrimitive<f32> for u16 {
    fn from_primitive(float: f32) -> Self {
        NumCast::from(normalize_float(float, u16::MAX as f32)).unwrap()
    }
}

// from u16:

impl FromPrimitive<u16> for u8 {
    fn from_primitive(c16: u16) -> Self {
        fn from(c: impl Into<u32>) -> u32 {
            c.into()
        }
        // The input c is the numerator of `c / u16::MAX`.
        // Derive numerator of `num / u8::MAX`, with rounding.
        //
        // This method is based on the inverse (see FromPrimitive<u8> for u16) and was tested
        // exhaustively in Python. It's the same as the reference function:
        //  round(c * (2**8 - 1) / (2**16 - 1))
        NumCast::from((from(c16) + 128) / 257).unwrap()
    }
}

impl FromPrimitive<u16> for f32 {
    fn from_primitive(int: u16) -> Self {
        (int as f32 / u16::MAX as f32).clamp(0.0, 1.0)
    }
}

// from u8:

impl FromPrimitive<u8> for f32 {
    fn from_primitive(int: u8) -> Self {
        (int as f32 / u8::MAX as f32).clamp(0.0, 1.0)
    }
}

impl FromPrimitive<u8> for u16 {
    fn from_primitive(c8: u8) -> Self {
        let x = c8.to_u64().unwrap();
        NumCast::from((x << 8) | x).unwrap()
    }
}

/// Provides color conversions for the different pixel types.
pub trait FromColor<Other> {
    /// Changes `self` to represent `Other` in the color space of `Self`
    #[allow(clippy::wrong_self_convention)]
    fn from_color(&mut self, _: &Other);
}

/// Copy-based conversions to target pixel types using `FromColor`.
// FIXME: this trait should be removed and replaced with real color space models
// rather than assuming sRGB.
pub(crate) trait IntoColor<Other> {
    /// Constructs a pixel of the target type and converts this pixel into it.
    #[allow(clippy::wrong_self_convention)]
    fn into_color(&self) -> Other;
}

impl<O, S> IntoColor<O> for S
where
    O: Pixel + FromColor<S>,
{
    #[allow(clippy::wrong_self_convention)]
    fn into_color(&self) -> O {
        // Note we cannot use Pixel::CHANNELS_COUNT here to directly construct
        // the pixel due to a current bug/limitation of consts.
        #[allow(deprecated)]
        let mut pix = O::from_channels(Zero::zero(), Zero::zero(), Zero::zero(), Zero::zero());
        pix.from_color(self);
        pix
    }
}

/// Coefficients to transform from sRGB to a CIE Y (luminance) value.
const SRGB_LUMA: [u32; 3] = [2126, 7152, 722];
const SRGB_LUMA_DIV: u32 = 10000;

#[inline]
fn rgb_to_luma<T: Primitive + Enlargeable>(rgb: &[T]) -> T {
    let l = <T::Larger as NumCast>::from(SRGB_LUMA[0]).unwrap() * rgb[0].to_larger()
        + <T::Larger as NumCast>::from(SRGB_LUMA[1]).unwrap() * rgb[1].to_larger()
        + <T::Larger as NumCast>::from(SRGB_LUMA[2]).unwrap() * rgb[2].to_larger();
    T::clamp_from(l / <T::Larger as NumCast>::from(SRGB_LUMA_DIV).unwrap())
}

// `FromColor` for Luma
impl<S: Primitive, T: Primitive> FromColor<Luma<S>> for Luma<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Luma<S>) {
        let own = self.channels_mut();
        let other = other.channels();
        own[0] = T::from_primitive(other[0]);
    }
}

impl<S: Primitive, T: Primitive> FromColor<LumaA<S>> for Luma<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &LumaA<S>) {
        self.channels_mut()[0] = T::from_primitive(other.channels()[0]);
    }
}

impl<S: Primitive + Enlargeable, T: Primitive> FromColor<Rgb<S>> for Luma<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgb<S>) {
        let gray = self.channels_mut();
        let rgb = other.channels();
        gray[0] = T::from_primitive(rgb_to_luma(rgb));
    }
}

impl<S: Primitive + Enlargeable, T: Primitive> FromColor<Rgba<S>> for Luma<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgba<S>) {
        let gray = self.channels_mut();
        let rgb = other.channels();
        let l = rgb_to_luma(rgb);
        gray[0] = T::from_primitive(l);
    }
}

// `FromColor` for LumaA

impl<S: Primitive, T: Primitive> FromColor<LumaA<S>> for LumaA<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &LumaA<S>) {
        let own = self.channels_mut();
        let other = other.channels();
        own[0] = T::from_primitive(other[0]);
        own[1] = T::from_primitive(other[1]);
    }
}

impl<S: Primitive + Enlargeable, T: Primitive> FromColor<Rgb<S>> for LumaA<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgb<S>) {
        let gray_a = self.channels_mut();
        let rgb = other.channels();
        gray_a[0] = T::from_primitive(rgb_to_luma(rgb));
        gray_a[1] = T::DEFAULT_MAX_VALUE;
    }
}

impl<S: Primitive + Enlargeable, T: Primitive> FromColor<Rgba<S>> for LumaA<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgba<S>) {
        let gray_a = self.channels_mut();
        let rgba = other.channels();
        gray_a[0] = T::from_primitive(rgb_to_luma(rgba));
        gray_a[1] = T::from_primitive(rgba[3]);
    }
}

impl<S: Primitive, T: Primitive> FromColor<Luma<S>> for LumaA<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Luma<S>) {
        let gray_a = self.channels_mut();
        gray_a[0] = T::from_primitive(other.channels()[0]);
        gray_a[1] = T::DEFAULT_MAX_VALUE;
    }
}

// `FromColor` for RGBA

impl<S: Primitive, T: Primitive> FromColor<Rgba<S>> for Rgba<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgba<S>) {
        let own = &mut self.0;
        let other = &other.0;
        own[0] = T::from_primitive(other[0]);
        own[1] = T::from_primitive(other[1]);
        own[2] = T::from_primitive(other[2]);
        own[3] = T::from_primitive(other[3]);
    }
}

impl<S: Primitive, T: Primitive> FromColor<Rgb<S>> for Rgba<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgb<S>) {
        let rgba = &mut self.0;
        let rgb = &other.0;
        rgba[0] = T::from_primitive(rgb[0]);
        rgba[1] = T::from_primitive(rgb[1]);
        rgba[2] = T::from_primitive(rgb[2]);
        rgba[3] = T::DEFAULT_MAX_VALUE;
    }
}

impl<S: Primitive, T: Primitive> FromColor<LumaA<S>> for Rgba<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, gray: &LumaA<S>) {
        let rgba = &mut self.0;
        let gray = &gray.0;
        rgba[0] = T::from_primitive(gray[0]);
        rgba[1] = T::from_primitive(gray[0]);
        rgba[2] = T::from_primitive(gray[0]);
        rgba[3] = T::from_primitive(gray[1]);
    }
}

impl<S: Primitive, T: Primitive> FromColor<Luma<S>> for Rgba<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, gray: &Luma<S>) {
        let rgba = &mut self.0;
        let gray = gray.0[0];
        rgba[0] = T::from_primitive(gray);
        rgba[1] = T::from_primitive(gray);
        rgba[2] = T::from_primitive(gray);
        rgba[3] = T::DEFAULT_MAX_VALUE;
    }
}

// `FromColor` for RGB

impl<S: Primitive, T: Primitive> FromColor<Rgb<S>> for Rgb<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgb<S>) {
        let own = &mut self.0;
        let other = &other.0;
        own[0] = T::from_primitive(other[0]);
        own[1] = T::from_primitive(other[1]);
        own[2] = T::from_primitive(other[2]);
    }
}

impl<S: Primitive, T: Primitive> FromColor<Rgba<S>> for Rgb<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Rgba<S>) {
        let rgb = &mut self.0;
        let rgba = &other.0;
        rgb[0] = T::from_primitive(rgba[0]);
        rgb[1] = T::from_primitive(rgba[1]);
        rgb[2] = T::from_primitive(rgba[2]);
    }
}

impl<S: Primitive, T: Primitive> FromColor<LumaA<S>> for Rgb<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &LumaA<S>) {
        let rgb = &mut self.0;
        let gray = other.0[0];
        rgb[0] = T::from_primitive(gray);
        rgb[1] = T::from_primitive(gray);
        rgb[2] = T::from_primitive(gray);
    }
}

impl<S: Primitive, T: Primitive> FromColor<Luma<S>> for Rgb<T>
where
    T: FromPrimitive<S>,
{
    fn from_color(&mut self, other: &Luma<S>) {
        let rgb = &mut self.0;
        let gray = other.0[0];
        rgb[0] = T::from_primitive(gray);
        rgb[1] = T::from_primitive(gray);
        rgb[2] = T::from_primitive(gray);
    }
}

/// Blends a color inter another one
pub(crate) trait Blend {
    /// Blends a color in-place.
    fn blend(&mut self, other: &Self);
}

impl<T: Primitive> Blend for LumaA<T> {
    fn blend(&mut self, other: &LumaA<T>) {
        let max_t = T::DEFAULT_MAX_VALUE;
        let max_t = max_t.to_f32().unwrap();
        let (bg_luma, bg_a) = (self.0[0], self.0[1]);
        let (fg_luma, fg_a) = (other.0[0], other.0[1]);

        let (bg_luma, bg_a) = (
            bg_luma.to_f32().unwrap() / max_t,
            bg_a.to_f32().unwrap() / max_t,
        );
        let (fg_luma, fg_a) = (
            fg_luma.to_f32().unwrap() / max_t,
            fg_a.to_f32().unwrap() / max_t,
        );

        let alpha_final = bg_a + fg_a - bg_a * fg_a;
        if alpha_final == 0.0 {
            return;
        };
        let bg_luma_a = bg_luma * bg_a;
        let fg_luma_a = fg_luma * fg_a;

        let out_luma_a = fg_luma_a + bg_luma_a * (1.0 - fg_a);
        let out_luma = out_luma_a / alpha_final;

        *self = LumaA([
            NumCast::from(max_t * out_luma).unwrap(),
            NumCast::from(max_t * alpha_final).unwrap(),
        ]);
    }
}

impl<T: Primitive> Blend for Luma<T> {
    fn blend(&mut self, other: &Luma<T>) {
        *self = *other;
    }
}

impl<T: Primitive> Blend for Rgba<T> {
    fn blend(&mut self, other: &Rgba<T>) {
        // http://stackoverflow.com/questions/7438263/alpha-compositing-algorithm-blend-modes#answer-11163848

        if other.0[3].is_zero() {
            return;
        }
        if other.0[3] == T::DEFAULT_MAX_VALUE {
            *self = *other;
            return;
        }

        // First, as we don't know what type our pixel is, we have to convert to floats between 0.0 and 1.0
        let max_t = T::DEFAULT_MAX_VALUE;
        let max_t = max_t.to_f32().unwrap();
        let (bg_r, bg_g, bg_b, bg_a) = (self.0[0], self.0[1], self.0[2], self.0[3]);
        let (fg_r, fg_g, fg_b, fg_a) = (other.0[0], other.0[1], other.0[2], other.0[3]);
        let (bg_r, bg_g, bg_b, bg_a) = (
            bg_r.to_f32().unwrap() / max_t,
            bg_g.to_f32().unwrap() / max_t,
            bg_b.to_f32().unwrap() / max_t,
            bg_a.to_f32().unwrap() / max_t,
        );
        let (fg_r, fg_g, fg_b, fg_a) = (
            fg_r.to_f32().unwrap() / max_t,
            fg_g.to_f32().unwrap() / max_t,
            fg_b.to_f32().unwrap() / max_t,
            fg_a.to_f32().unwrap() / max_t,
        );

        // Work out what the final alpha level will be
        let alpha_final = bg_a + fg_a - bg_a * fg_a;
        if alpha_final == 0.0 {
            return;
        };

        // We premultiply our channels by their alpha, as this makes it easier to calculate
        let (bg_r_a, bg_g_a, bg_b_a) = (bg_r * bg_a, bg_g * bg_a, bg_b * bg_a);
        let (fg_r_a, fg_g_a, fg_b_a) = (fg_r * fg_a, fg_g * fg_a, fg_b * fg_a);

        // Standard formula for src-over alpha compositing
        let (out_r_a, out_g_a, out_b_a) = (
            fg_r_a + bg_r_a * (1.0 - fg_a),
            fg_g_a + bg_g_a * (1.0 - fg_a),
            fg_b_a + bg_b_a * (1.0 - fg_a),
        );

        // Unmultiply the channels by our resultant alpha channel
        let (out_r, out_g, out_b) = (
            out_r_a / alpha_final,
            out_g_a / alpha_final,
            out_b_a / alpha_final,
        );

        // Cast back to our initial type on return
        *self = Rgba([
            NumCast::from(max_t * out_r).unwrap(),
            NumCast::from(max_t * out_g).unwrap(),
            NumCast::from(max_t * out_b).unwrap(),
            NumCast::from(max_t * alpha_final).unwrap(),
        ]);
    }
}

impl<T: Primitive> Blend for Rgb<T> {
    fn blend(&mut self, other: &Rgb<T>) {
        *self = *other;
    }
}

/// Invert a color
pub(crate) trait Invert {
    /// Inverts a color in-place.
    fn invert(&mut self);
}

impl<T: Primitive> Invert for LumaA<T> {
    fn invert(&mut self) {
        let l = self.0;
        let max = T::DEFAULT_MAX_VALUE;

        *self = LumaA([max - l[0], l[1]]);
    }
}

impl<T: Primitive> Invert for Luma<T> {
    fn invert(&mut self) {
        let l = self.0;

        let max = T::DEFAULT_MAX_VALUE;
        let l1 = max - l[0];

        *self = Luma([l1]);
    }
}

impl<T: Primitive> Invert for Rgba<T> {
    fn invert(&mut self) {
        let rgba = self.0;

        let max = T::DEFAULT_MAX_VALUE;

        *self = Rgba([max - rgba[0], max - rgba[1], max - rgba[2], rgba[3]]);
    }
}

impl<T: Primitive> Invert for Rgb<T> {
    fn invert(&mut self) {
        let rgb = self.0;

        let max = T::DEFAULT_MAX_VALUE;

        let r1 = max - rgb[0];
        let g1 = max - rgb[1];
        let b1 = max - rgb[2];

        *self = Rgb([r1, g1, b1]);
    }
}

#[cfg(test)]
mod tests {
    use super::{Luma, LumaA, Pixel, Rgb, Rgba};

    #[test]
    fn test_apply_with_alpha_rgba() {
        let mut rgba = Rgba([0, 0, 0, 0]);
        rgba.apply_with_alpha(|s| s, |_| 0xFF);
        assert_eq!(rgba, Rgba([0, 0, 0, 0xFF]));
    }

    #[test]
    fn test_apply_with_alpha_rgb() {
        let mut rgb = Rgb([0, 0, 0]);
        rgb.apply_with_alpha(|s| s, |_| panic!("bug"));
        assert_eq!(rgb, Rgb([0, 0, 0]));
    }

    #[test]
    fn test_map_with_alpha_rgba() {
        let rgba = Rgba([0, 0, 0, 0]).map_with_alpha(|s| s, |_| 0xFF);
        assert_eq!(rgba, Rgba([0, 0, 0, 0xFF]));
    }

    #[test]
    fn test_map_with_alpha_rgb() {
        let rgb = Rgb([0, 0, 0]).map_with_alpha(|s| s, |_| panic!("bug"));
        assert_eq!(rgb, Rgb([0, 0, 0]));
    }

    #[test]
    fn test_blend_luma_alpha() {
        let a = &mut LumaA([255_u8, 255]);
        let b = LumaA([255_u8, 255]);
        a.blend(&b);
        assert_eq!(a.0[0], 255);
        assert_eq!(a.0[1], 255);

        let a = &mut LumaA([255_u8, 0]);
        let b = LumaA([255_u8, 255]);
        a.blend(&b);
        assert_eq!(a.0[0], 255);
        assert_eq!(a.0[1], 255);

        let a = &mut LumaA([255_u8, 255]);
        let b = LumaA([255_u8, 0]);
        a.blend(&b);
        assert_eq!(a.0[0], 255);
        assert_eq!(a.0[1], 255);

        let a = &mut LumaA([255_u8, 0]);
        let b = LumaA([255_u8, 0]);
        a.blend(&b);
        assert_eq!(a.0[0], 255);
        assert_eq!(a.0[1], 0);
    }

    #[test]
    fn test_blend_rgba() {
        let a = &mut Rgba([255_u8, 255, 255, 255]);
        let b = Rgba([255_u8, 255, 255, 255]);
        a.blend(&b);
        assert_eq!(a.0, [255, 255, 255, 255]);

        let a = &mut Rgba([255_u8, 255, 255, 0]);
        let b = Rgba([255_u8, 255, 255, 255]);
        a.blend(&b);
        assert_eq!(a.0, [255, 255, 255, 255]);

        let a = &mut Rgba([255_u8, 255, 255, 255]);
        let b = Rgba([255_u8, 255, 255, 0]);
        a.blend(&b);
        assert_eq!(a.0, [255, 255, 255, 255]);

        let a = &mut Rgba([255_u8, 255, 255, 0]);
        let b = Rgba([255_u8, 255, 255, 0]);
        a.blend(&b);
        assert_eq!(a.0, [255, 255, 255, 0]);
    }

    #[test]
    fn test_apply_without_alpha_rgba() {
        let mut rgba = Rgba([0, 0, 0, 0]);
        rgba.apply_without_alpha(|s| s + 1);
        assert_eq!(rgba, Rgba([1, 1, 1, 0]));
    }

    #[test]
    fn test_apply_without_alpha_rgb() {
        let mut rgb = Rgb([0, 0, 0]);
        rgb.apply_without_alpha(|s| s + 1);
        assert_eq!(rgb, Rgb([1, 1, 1]));
    }

    #[test]
    fn test_map_without_alpha_rgba() {
        let rgba = Rgba([0, 0, 0, 0]).map_without_alpha(|s| s + 1);
        assert_eq!(rgba, Rgba([1, 1, 1, 0]));
    }

    #[test]
    fn test_map_without_alpha_rgb() {
        let rgb = Rgb([0, 0, 0]).map_without_alpha(|s| s + 1);
        assert_eq!(rgb, Rgb([1, 1, 1]));
    }

    macro_rules! test_lossless_conversion {
        ($a:ty, $b:ty, $c:ty) => {
            let a: $a = [<$a as Pixel>::Subpixel::DEFAULT_MAX_VALUE >> 2;
                <$a as Pixel>::CHANNEL_COUNT as usize]
                .into();
            let b: $b = a.into_color();
            let c: $c = b.into_color();
            assert_eq!(a.channels(), c.channels());
        };
    }

    #[test]
    fn test_lossless_conversions() {
        use super::IntoColor;
        use crate::traits::Primitive;

        test_lossless_conversion!(Luma<u8>, Luma<u16>, Luma<u8>);
        test_lossless_conversion!(LumaA<u8>, LumaA<u16>, LumaA<u8>);
        test_lossless_conversion!(Rgb<u8>, Rgb<u16>, Rgb<u8>);
        test_lossless_conversion!(Rgba<u8>, Rgba<u16>, Rgba<u8>);
    }

    #[test]
    fn accuracy_conversion() {
        use super::{Luma, Pixel, Rgb};
        let pixel = Rgb::from([13, 13, 13]);
        let Luma([luma]) = pixel.to_luma();
        assert_eq!(luma, 13);
    }
}
