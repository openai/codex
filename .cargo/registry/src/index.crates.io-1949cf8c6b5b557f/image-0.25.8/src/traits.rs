//! This module provides useful traits that were deprecated in rust

// Note copied from the stdlib under MIT license

use num_traits::{Bounded, Num, NumCast};
use std::ops::AddAssign;

use crate::color::{Luma, LumaA, Rgb, Rgba};
use crate::ExtendedColorType;

/// Types which are safe to treat as an immutable byte slice in a pixel layout
/// for image encoding.
pub trait EncodableLayout: seals::EncodableLayout {
    /// Get the bytes of this value.
    fn as_bytes(&self) -> &[u8];
}

impl EncodableLayout for [u8] {
    fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self)
    }
}

impl EncodableLayout for [u16] {
    fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self)
    }
}

impl EncodableLayout for [f32] {
    fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self)
    }
}

/// The type of each channel in a pixel. For example, this can be `u8`, `u16`, `f32`.
// TODO rename to `PixelComponent`? Split up into separate traits? Seal?
pub trait Primitive: Copy + NumCast + Num + PartialOrd<Self> + Clone + Bounded {
    /// The maximum value for this type of primitive within the context of color.
    /// For floats, the maximum is `1.0`, whereas the integer types inherit their usual maximum values.
    const DEFAULT_MAX_VALUE: Self;

    /// The minimum value for this type of primitive within the context of color.
    /// For floats, the minimum is `0.0`, whereas the integer types inherit their usual minimum values.
    const DEFAULT_MIN_VALUE: Self;
}

macro_rules! declare_primitive {
    ($base:ty: ($from:expr)..$to:expr) => {
        impl Primitive for $base {
            const DEFAULT_MAX_VALUE: Self = $to;
            const DEFAULT_MIN_VALUE: Self = $from;
        }
    };
}

declare_primitive!(usize: (0)..Self::MAX);
declare_primitive!(u8: (0)..Self::MAX);
declare_primitive!(u16: (0)..Self::MAX);
declare_primitive!(u32: (0)..Self::MAX);
declare_primitive!(u64: (0)..Self::MAX);

declare_primitive!(isize: (Self::MIN)..Self::MAX);
declare_primitive!(i8: (Self::MIN)..Self::MAX);
declare_primitive!(i16: (Self::MIN)..Self::MAX);
declare_primitive!(i32: (Self::MIN)..Self::MAX);
declare_primitive!(i64: (Self::MIN)..Self::MAX);
declare_primitive!(f32: (0.0)..1.0);
declare_primitive!(f64: (0.0)..1.0);

/// An `Enlargable::Larger` value should be enough to calculate
/// the sum (average) of a few hundred or thousand Enlargeable values.
pub trait Enlargeable: Sized + Bounded + NumCast {
    type Larger: Copy + NumCast + Num + PartialOrd<Self::Larger> + Clone + Bounded + AddAssign;

    fn clamp_from(n: Self::Larger) -> Self {
        if n > Self::max_value().to_larger() {
            Self::max_value()
        } else if n < Self::min_value().to_larger() {
            Self::min_value()
        } else {
            NumCast::from(n).unwrap()
        }
    }

    fn to_larger(self) -> Self::Larger {
        NumCast::from(self).unwrap()
    }
}

impl Enlargeable for u8 {
    type Larger = u32;
}
impl Enlargeable for u16 {
    type Larger = u32;
}
impl Enlargeable for u32 {
    type Larger = u64;
}
impl Enlargeable for u64 {
    type Larger = u128;
}
impl Enlargeable for usize {
    // Note: On 32-bit architectures, u64 should be enough here.
    type Larger = u128;
}
impl Enlargeable for i8 {
    type Larger = i32;
}
impl Enlargeable for i16 {
    type Larger = i32;
}
impl Enlargeable for i32 {
    type Larger = i64;
}
impl Enlargeable for i64 {
    type Larger = i128;
}
impl Enlargeable for isize {
    // Note: On 32-bit architectures, i64 should be enough here.
    type Larger = i128;
}
impl Enlargeable for f32 {
    type Larger = f64;
}
impl Enlargeable for f64 {
    type Larger = f64;
}

/// Linear interpolation without involving floating numbers.
pub trait Lerp: Bounded + NumCast {
    type Ratio: Primitive;

    fn lerp(a: Self, b: Self, ratio: Self::Ratio) -> Self {
        let a = <Self::Ratio as NumCast>::from(a).unwrap();
        let b = <Self::Ratio as NumCast>::from(b).unwrap();

        let res = a + (b - a) * ratio;

        if res > NumCast::from(Self::max_value()).unwrap() {
            Self::max_value()
        } else if res < NumCast::from(0).unwrap() {
            NumCast::from(0).unwrap()
        } else {
            NumCast::from(res).unwrap()
        }
    }
}

impl Lerp for u8 {
    type Ratio = f32;
}

impl Lerp for u16 {
    type Ratio = f32;
}

impl Lerp for u32 {
    type Ratio = f64;
}

impl Lerp for f32 {
    type Ratio = f32;

    fn lerp(a: Self, b: Self, ratio: Self::Ratio) -> Self {
        a + (b - a) * ratio
    }
}

/// The pixel with an associated `ColorType`.
/// Not all possible pixels represent one of the predefined `ColorType`s.
pub trait PixelWithColorType:
    Pixel + private::SealedPixelWithColorType<TransformableSubpixel = <Self as Pixel>::Subpixel>
{
    /// This pixel has the format of one of the predefined `ColorType`s,
    /// such as `Rgb8`, `La16` or `Rgba32F`.
    /// This is needed for automatically detecting
    /// a color format when saving an image as a file.
    const COLOR_TYPE: ExtendedColorType;
}

impl PixelWithColorType for Rgb<u8> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::Rgb8;
}
impl PixelWithColorType for Rgb<u16> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::Rgb16;
}
impl PixelWithColorType for Rgb<f32> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::Rgb32F;
}

impl PixelWithColorType for Rgba<u8> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::Rgba8;
}
impl PixelWithColorType for Rgba<u16> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::Rgba16;
}
impl PixelWithColorType for Rgba<f32> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::Rgba32F;
}

impl PixelWithColorType for Luma<u8> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::L8;
}
impl PixelWithColorType for Luma<u16> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::L16;
}
impl PixelWithColorType for LumaA<u8> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::La8;
}
impl PixelWithColorType for LumaA<u16> {
    const COLOR_TYPE: ExtendedColorType = ExtendedColorType::La16;
}

/// Prevents down-stream users from implementing the `Primitive` trait
pub(crate) mod private {
    use crate::color::*;
    use crate::metadata::cicp::{self, CicpApplicable};

    #[derive(Clone, Copy, Debug)]
    pub enum LayoutWithColor {
        Rgb,
        Rgba,
        Luma,
        LumaAlpha,
    }

    impl From<ColorType> for LayoutWithColor {
        fn from(color: ColorType) -> LayoutWithColor {
            match color {
                ColorType::L8 | ColorType::L16 => LayoutWithColor::Luma,
                ColorType::La8 | ColorType::La16 => LayoutWithColor::LumaAlpha,
                ColorType::Rgb8 | ColorType::Rgb16 | ColorType::Rgb32F => LayoutWithColor::Rgb,
                ColorType::Rgba8 | ColorType::Rgba16 | ColorType::Rgba32F => LayoutWithColor::Rgba,
            }
        }
    }

    impl LayoutWithColor {
        pub(crate) fn channels(self) -> usize {
            match self {
                Self::Rgb => 3,
                Self::Rgba => 4,
                Self::Luma => 1,
                Self::LumaAlpha => 2,
            }
        }
    }

    #[derive(Clone, Copy)]
    pub struct PrivateToken;

    pub trait SealedPixelWithColorType {
        #[expect(private_bounds)] // This is a sealed trait.
        type TransformableSubpixel: HelpDispatchTransform;
        fn layout(_: PrivateToken) -> LayoutWithColor;
    }

    impl SealedPixelWithColorType for Rgb<u8> {
        type TransformableSubpixel = u8;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Rgb
        }
    }

    impl SealedPixelWithColorType for Rgb<u16> {
        type TransformableSubpixel = u16;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Rgb
        }
    }

    impl SealedPixelWithColorType for Rgb<f32> {
        type TransformableSubpixel = f32;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Rgb
        }
    }

    impl SealedPixelWithColorType for Rgba<u8> {
        type TransformableSubpixel = u8;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Rgba
        }
    }

    impl SealedPixelWithColorType for Rgba<u16> {
        type TransformableSubpixel = u16;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Rgba
        }
    }

    impl SealedPixelWithColorType for Rgba<f32> {
        type TransformableSubpixel = f32;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Rgba
        }
    }

    impl SealedPixelWithColorType for Luma<u8> {
        type TransformableSubpixel = u8;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Luma
        }
    }

    impl SealedPixelWithColorType for LumaA<u8> {
        type TransformableSubpixel = u8;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::LumaAlpha
        }
    }

    impl SealedPixelWithColorType for Luma<u16> {
        type TransformableSubpixel = u16;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Luma
        }
    }

    impl SealedPixelWithColorType for Luma<f32> {
        type TransformableSubpixel = f32;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::Luma
        }
    }

    impl SealedPixelWithColorType for LumaA<u16> {
        type TransformableSubpixel = u16;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::LumaAlpha
        }
    }

    impl SealedPixelWithColorType for LumaA<f32> {
        type TransformableSubpixel = f32;
        fn layout(_: PrivateToken) -> LayoutWithColor {
            LayoutWithColor::LumaAlpha
        }
    }

    // Consider a situation in a function bounded `Self: Pixel + PixelWithColorType`. Then, if we
    // tried this directly:
    //
    // <
    //   <Self as SealedPixelWithColorType>::TransformableSubpixel as HelpDispatchTransform
    // >::transform_on::<Self>(tr, LayoutWithColor::Rgb);
    //
    // the type checker is mightily confused. I think what's going on is as follows: It find the
    // fact that `Self::Subpixel` is used for `TransformableSubpixel` from the bound on
    // `PixelWithColorType`, but then there is no existing bound on `Subpixel` that would guarantee
    // that `HelpDispatchTransform` is fulfilled. That would only be available by substituting
    // _back_ so that the bound on `TransformableSubpixel` gets applied to the `Subpixel` generic,
    // too. But now there are no variables here, so unification of bounds takes place we never
    // never get to see the bound (until next gen, I guess?). It finally find that there is still
    // an unfulfilled bound and complains.
    //
    // Hence we must avoid mentioning the `Pixel` and `PixelWithColorType` bound so that _only_ the
    // `TransformableSubpixel` is available. Then all substitutions work forwards, and since we
    // return a `TransformableSubpixel` we get the function back without new variables to solve
    // for, and that can then be unified just fine. This extra function essentially introduces that
    // missing unknown which can unify the available impl set. Yay.
    pub(crate) fn dispatch_transform_from_sealed<P: SealedPixelWithColorType>(
        transform: &cicp::CicpTransform,
        into: LayoutWithColor,
    ) -> &'_ CicpApplicable<'_, P::TransformableSubpixel> {
        <P::TransformableSubpixel as HelpDispatchTransform>::transform_on::<P>(transform, into)
    }

    pub(crate) fn double_dispatch_transform_from_sealed<
        P: SealedPixelWithColorType,
        Into: SealedPixelWithColorType,
    >(
        transform: &cicp::CicpTransform,
    ) -> &'_ CicpApplicable<'_, P::TransformableSubpixel> {
        dispatch_transform_from_sealed::<P>(transform, Into::layout(PrivateToken))
    }

    pub(crate) trait HelpDispatchTransform: Sized + 'static {
        fn transform_on<O: SealedPixelWithColorType<TransformableSubpixel = Self>>(
            transform: &cicp::CicpTransform,
            into: LayoutWithColor,
        ) -> &'_ (dyn Fn(&[Self], &mut [Self]) + Send + Sync);
    }

    impl HelpDispatchTransform for u8 {
        fn transform_on<O: SealedPixelWithColorType<TransformableSubpixel = Self>>(
            transform: &cicp::CicpTransform,
            into: LayoutWithColor,
        ) -> &'_ (dyn Fn(&[Self], &mut [Self]) + Send + Sync) {
            &**transform.select_transform_u8::<O>(into)
        }
    }

    impl HelpDispatchTransform for u16 {
        fn transform_on<O: SealedPixelWithColorType<TransformableSubpixel = Self>>(
            transform: &cicp::CicpTransform,
            into: LayoutWithColor,
        ) -> &'_ (dyn Fn(&[Self], &mut [Self]) + Send + Sync) {
            &**transform.select_transform_u16::<O>(into)
        }
    }

    impl HelpDispatchTransform for f32 {
        fn transform_on<O: SealedPixelWithColorType<TransformableSubpixel = Self>>(
            transform: &cicp::CicpTransform,
            into: LayoutWithColor,
        ) -> &'_ (dyn Fn(&[Self], &mut [Self]) + Send + Sync) {
            &**transform.select_transform_f32::<O>(into)
        }
    }
}

/// A generalized pixel.
///
/// A pixel object is usually not used standalone but as a view into an image buffer.
pub trait Pixel: Copy + Clone {
    /// The scalar type that is used to store each channel in this pixel.
    type Subpixel: Primitive;

    /// The number of channels of this pixel type.
    const CHANNEL_COUNT: u8;

    /// Returns the components as a slice.
    fn channels(&self) -> &[Self::Subpixel];

    /// Returns the components as a mutable slice
    fn channels_mut(&mut self) -> &mut [Self::Subpixel];

    /// A string that can help to interpret the meaning each channel
    /// See [gimp babl](http://gegl.org/babl/).
    const COLOR_MODEL: &'static str;

    /// Returns true if the alpha channel is contained.
    const HAS_ALPHA: bool = false;

    /// Returns the channels of this pixel as a 4 tuple. If the pixel
    /// has less than 4 channels the remainder is filled with the maximum value
    #[deprecated(since = "0.24.0", note = "Use `channels()` or `channels_mut()`")]
    fn channels4(
        &self,
    ) -> (
        Self::Subpixel,
        Self::Subpixel,
        Self::Subpixel,
        Self::Subpixel,
    );

    /// Construct a pixel from the 4 channels a, b, c and d.
    /// If the pixel does not contain 4 channels the extra are ignored.
    #[deprecated(
        since = "0.24.0",
        note = "Use the constructor of the pixel, for example `Rgba([r,g,b,a])` or `Pixel::from_slice`"
    )]
    fn from_channels(
        a: Self::Subpixel,
        b: Self::Subpixel,
        c: Self::Subpixel,
        d: Self::Subpixel,
    ) -> Self;

    /// Returns a view into a slice.
    ///
    /// Note: The slice length is not checked on creation. Thus the caller has to ensure
    /// that the slice is long enough to prevent panics if the pixel is used later on.
    fn from_slice(slice: &[Self::Subpixel]) -> &Self;

    /// Returns mutable view into a mutable slice.
    ///
    /// Note: The slice length is not checked on creation. Thus the caller has to ensure
    /// that the slice is long enough to prevent panics if the pixel is used later on.
    fn from_slice_mut(slice: &mut [Self::Subpixel]) -> &mut Self;

    /// Convert this pixel to RGB
    fn to_rgb(&self) -> Rgb<Self::Subpixel>;

    /// Convert this pixel to RGB with an alpha channel
    fn to_rgba(&self) -> Rgba<Self::Subpixel>;

    /// Convert this pixel to luma
    fn to_luma(&self) -> Luma<Self::Subpixel>;

    /// Convert this pixel to luma with an alpha channel
    fn to_luma_alpha(&self) -> LumaA<Self::Subpixel>;

    /// Apply the function ```f``` to each channel of this pixel.
    fn map<F>(&self, f: F) -> Self
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel;

    /// Apply the function ```f``` to each channel of this pixel.
    fn apply<F>(&mut self, f: F)
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel;

    /// Apply the function ```f``` to each channel except the alpha channel.
    /// Apply the function ```g``` to the alpha channel.
    fn map_with_alpha<F, G>(&self, f: F, g: G) -> Self
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
        G: FnMut(Self::Subpixel) -> Self::Subpixel;

    /// Apply the function ```f``` to each channel except the alpha channel.
    /// Apply the function ```g``` to the alpha channel. Works in-place.
    fn apply_with_alpha<F, G>(&mut self, f: F, g: G)
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
        G: FnMut(Self::Subpixel) -> Self::Subpixel;

    /// Apply the function ```f``` to each channel except the alpha channel.
    fn map_without_alpha<F>(&self, f: F) -> Self
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
    {
        let mut this = *self;
        this.apply_with_alpha(f, |x| x);
        this
    }

    /// Apply the function ```f``` to each channel except the alpha channel.
    /// Works in place.
    fn apply_without_alpha<F>(&mut self, f: F)
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
    {
        self.apply_with_alpha(f, |x| x);
    }

    /// Apply the function ```f``` to each channel of this pixel and
    /// ```other``` pairwise.
    fn map2<F>(&self, other: &Self, f: F) -> Self
    where
        F: FnMut(Self::Subpixel, Self::Subpixel) -> Self::Subpixel;

    /// Apply the function ```f``` to each channel of this pixel and
    /// ```other``` pairwise. Works in-place.
    fn apply2<F>(&mut self, other: &Self, f: F)
    where
        F: FnMut(Self::Subpixel, Self::Subpixel) -> Self::Subpixel;

    /// Invert this pixel
    fn invert(&mut self);

    /// Blend the color of a given pixel into ourself, taking into account alpha channels
    fn blend(&mut self, other: &Self);
}

/// Private module for supertraits of sealed traits.
mod seals {
    pub trait EncodableLayout {}

    impl EncodableLayout for [u8] {}
    impl EncodableLayout for [u16] {}
    impl EncodableLayout for [f32] {}
}
