//! Functions for altering and converting the color of pixelbufs

use num_traits::NumCast;

use crate::color::{FromColor, IntoColor, Luma, LumaA};
use crate::metadata::{CicpColorPrimaries, CicpTransferCharacteristics};
use crate::traits::{Pixel, Primitive};
use crate::utils::clamp;
use crate::{GenericImage, GenericImageView, ImageBuffer};

type Subpixel<I> = <<I as GenericImageView>::Pixel as Pixel>::Subpixel;

/// Convert the supplied image to grayscale. Alpha channel is discarded.
pub fn grayscale<I: GenericImageView>(
    image: &I,
) -> ImageBuffer<Luma<Subpixel<I>>, Vec<Subpixel<I>>> {
    grayscale_with_type(image)
}

/// Convert the supplied image to grayscale. Alpha channel is preserved.
pub fn grayscale_alpha<I: GenericImageView>(
    image: &I,
) -> ImageBuffer<LumaA<Subpixel<I>>, Vec<Subpixel<I>>> {
    grayscale_with_type_alpha(image)
}

/// Convert the supplied image to a grayscale image with the specified pixel type. Alpha channel is discarded.
pub fn grayscale_with_type<NewPixel, I: GenericImageView>(
    image: &I,
) -> ImageBuffer<NewPixel, Vec<NewPixel::Subpixel>>
where
    NewPixel: Pixel + FromColor<Luma<Subpixel<I>>>,
{
    let (width, height) = image.dimensions();
    let mut out = ImageBuffer::new(width, height);
    out.copy_color_space_from(&image.buffer_with_dimensions(0, 0));

    for (x, y, pixel) in image.pixels() {
        let grayscale = pixel.to_luma();
        let new_pixel = grayscale.into_color(); // no-op for luma->luma

        out.put_pixel(x, y, new_pixel);
    }

    out
}

/// Convert the supplied image to a grayscale image with the specified pixel type. Alpha channel is preserved.
pub fn grayscale_with_type_alpha<NewPixel, I: GenericImageView>(
    image: &I,
) -> ImageBuffer<NewPixel, Vec<NewPixel::Subpixel>>
where
    NewPixel: Pixel + FromColor<LumaA<Subpixel<I>>>,
{
    let (width, height) = image.dimensions();
    let mut out = ImageBuffer::new(width, height);
    out.copy_color_space_from(&image.buffer_with_dimensions(0, 0));

    for (x, y, pixel) in image.pixels() {
        let grayscale = pixel.to_luma_alpha();
        let new_pixel = grayscale.into_color(); // no-op for luma->luma

        out.put_pixel(x, y, new_pixel);
    }

    out
}

/// Invert each pixel within the supplied image.
/// This function operates in place.
pub fn invert<I: GenericImage>(image: &mut I) {
    // TODO find a way to use pixels?
    let (width, height) = image.dimensions();

    for y in 0..height {
        for x in 0..width {
            let mut p = image.get_pixel(x, y);
            p.invert();

            image.put_pixel(x, y, p);
        }
    }
}

/// Adjust the contrast of the supplied image.
/// ```contrast``` is the amount to adjust the contrast by.
/// Negative values decrease the contrast and positive values increase the contrast.
///
/// *[See also `contrast_in_place`.][contrast_in_place]*
pub fn contrast<I, P, S>(image: &I, contrast: f32) -> ImageBuffer<P, Vec<S>>
where
    I: GenericImageView<Pixel = P>,
    P: Pixel<Subpixel = S> + 'static,
    S: Primitive + 'static,
{
    let mut out = image.buffer_like();

    let max = S::DEFAULT_MAX_VALUE;
    let max: f32 = NumCast::from(max).unwrap();

    let percent = ((100.0 + contrast) / 100.0).powi(2);

    for (x, y, pixel) in image.pixels() {
        let f = pixel.map(|b| {
            let c: f32 = NumCast::from(b).unwrap();

            let d = ((c / max - 0.5) * percent + 0.5) * max;
            let e = clamp(d, 0.0, max);

            NumCast::from(e).unwrap()
        });
        out.put_pixel(x, y, f);
    }

    out
}

/// Adjust the contrast of the supplied image in place.
/// ```contrast``` is the amount to adjust the contrast by.
/// Negative values decrease the contrast and positive values increase the contrast.
///
/// *[See also `contrast`.][contrast]*
pub fn contrast_in_place<I>(image: &mut I, contrast: f32)
where
    I: GenericImage,
{
    let (width, height) = image.dimensions();

    let max = <I::Pixel as Pixel>::Subpixel::DEFAULT_MAX_VALUE;
    let max: f32 = NumCast::from(max).unwrap();

    let percent = ((100.0 + contrast) / 100.0).powi(2);

    // TODO find a way to use pixels?
    for y in 0..height {
        for x in 0..width {
            let f = image.get_pixel(x, y).map(|b| {
                let c: f32 = NumCast::from(b).unwrap();

                let d = ((c / max - 0.5) * percent + 0.5) * max;
                let e = clamp(d, 0.0, max);

                NumCast::from(e).unwrap()
            });

            image.put_pixel(x, y, f);
        }
    }
}

/// Brighten the supplied image.
/// ```value``` is the amount to brighten each pixel by.
/// Negative values decrease the brightness and positive values increase it.
///
/// *[See also `brighten_in_place`.][brighten_in_place]*
pub fn brighten<I, P, S>(image: &I, value: i32) -> ImageBuffer<P, Vec<S>>
where
    I: GenericImageView<Pixel = P>,
    P: Pixel<Subpixel = S> + 'static,
    S: Primitive + 'static,
{
    let mut out = image.buffer_like();

    let max = S::DEFAULT_MAX_VALUE;
    let max: i32 = NumCast::from(max).unwrap();

    for (x, y, pixel) in image.pixels() {
        let e = pixel.map_with_alpha(
            |b| {
                let c: i32 = NumCast::from(b).unwrap();
                let d = clamp(c + value, 0, max);

                NumCast::from(d).unwrap()
            },
            |alpha| alpha,
        );
        out.put_pixel(x, y, e);
    }

    out
}

/// Brighten the supplied image in place.
/// ```value``` is the amount to brighten each pixel by.
/// Negative values decrease the brightness and positive values increase it.
///
/// *[See also `brighten`.][brighten]*
pub fn brighten_in_place<I>(image: &mut I, value: i32)
where
    I: GenericImage,
{
    let (width, height) = image.dimensions();

    let max = <I::Pixel as Pixel>::Subpixel::DEFAULT_MAX_VALUE;
    let max: i32 = NumCast::from(max).unwrap(); // TODO what does this do for f32? clamp at 1??

    // TODO find a way to use pixels?
    for y in 0..height {
        for x in 0..width {
            let e = image.get_pixel(x, y).map_with_alpha(
                |b| {
                    let c: i32 = NumCast::from(b).unwrap();
                    let d = clamp(c + value, 0, max);

                    NumCast::from(d).unwrap()
                },
                |alpha| alpha,
            );

            image.put_pixel(x, y, e);
        }
    }
}

/// Hue rotate the supplied image.
/// `value` is the degrees to rotate each pixel by.
/// 0 and 360 do nothing, the rest rotates by the given degree value.
/// just like the css webkit filter hue-rotate(180)
///
/// *[See also `huerotate_in_place`.][huerotate_in_place]*
pub fn huerotate<I, P, S>(image: &I, value: i32) -> ImageBuffer<P, Vec<S>>
where
    I: GenericImageView<Pixel = P>,
    P: Pixel<Subpixel = S> + 'static,
    S: Primitive + 'static,
{
    let mut out = image.buffer_like();

    let angle: f64 = NumCast::from(value).unwrap();

    let cosv = angle.to_radians().cos();
    let sinv = angle.to_radians().sin();
    let matrix: [f64; 9] = [
        // Reds
        0.213 + cosv * 0.787 - sinv * 0.213,
        0.715 - cosv * 0.715 - sinv * 0.715,
        0.072 - cosv * 0.072 + sinv * 0.928,
        // Greens
        0.213 - cosv * 0.213 + sinv * 0.143,
        0.715 + cosv * 0.285 + sinv * 0.140,
        0.072 - cosv * 0.072 - sinv * 0.283,
        // Blues
        0.213 - cosv * 0.213 - sinv * 0.787,
        0.715 - cosv * 0.715 + sinv * 0.715,
        0.072 + cosv * 0.928 + sinv * 0.072,
    ];
    for (x, y, pixel) in out.enumerate_pixels_mut() {
        let p = image.get_pixel(x, y);

        #[allow(deprecated)]
        let (k1, k2, k3, k4) = p.channels4();
        let vec: (f64, f64, f64, f64) = (
            NumCast::from(k1).unwrap(),
            NumCast::from(k2).unwrap(),
            NumCast::from(k3).unwrap(),
            NumCast::from(k4).unwrap(),
        );

        let r = vec.0;
        let g = vec.1;
        let b = vec.2;

        let new_r = matrix[0] * r + matrix[1] * g + matrix[2] * b;
        let new_g = matrix[3] * r + matrix[4] * g + matrix[5] * b;
        let new_b = matrix[6] * r + matrix[7] * g + matrix[8] * b;
        let max = 255f64;

        #[allow(deprecated)]
        let outpixel = Pixel::from_channels(
            NumCast::from(clamp(new_r, 0.0, max)).unwrap(),
            NumCast::from(clamp(new_g, 0.0, max)).unwrap(),
            NumCast::from(clamp(new_b, 0.0, max)).unwrap(),
            NumCast::from(clamp(vec.3, 0.0, max)).unwrap(),
        );
        *pixel = outpixel;
    }
    out
}

/// Hue rotate the supplied image in place.
///
/// `value` is the degrees to rotate each pixel by.
/// 0 and 360 do nothing, the rest rotates by the given degree value.
/// just like the css webkit filter hue-rotate(180)
///
/// *[See also `huerotate`.][huerotate]*
pub fn huerotate_in_place<I>(image: &mut I, value: i32)
where
    I: GenericImage,
{
    let (width, height) = image.dimensions();

    let angle: f64 = NumCast::from(value).unwrap();

    let cosv = angle.to_radians().cos();
    let sinv = angle.to_radians().sin();
    let matrix: [f64; 9] = [
        // Reds
        0.213 + cosv * 0.787 - sinv * 0.213,
        0.715 - cosv * 0.715 - sinv * 0.715,
        0.072 - cosv * 0.072 + sinv * 0.928,
        // Greens
        0.213 - cosv * 0.213 + sinv * 0.143,
        0.715 + cosv * 0.285 + sinv * 0.140,
        0.072 - cosv * 0.072 - sinv * 0.283,
        // Blues
        0.213 - cosv * 0.213 - sinv * 0.787,
        0.715 - cosv * 0.715 + sinv * 0.715,
        0.072 + cosv * 0.928 + sinv * 0.072,
    ];

    // TODO find a way to use pixels?
    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y);

            #[allow(deprecated)]
            let (k1, k2, k3, k4) = pixel.channels4();

            let vec: (f64, f64, f64, f64) = (
                NumCast::from(k1).unwrap(),
                NumCast::from(k2).unwrap(),
                NumCast::from(k3).unwrap(),
                NumCast::from(k4).unwrap(),
            );

            let r = vec.0;
            let g = vec.1;
            let b = vec.2;

            let new_r = matrix[0] * r + matrix[1] * g + matrix[2] * b;
            let new_g = matrix[3] * r + matrix[4] * g + matrix[5] * b;
            let new_b = matrix[6] * r + matrix[7] * g + matrix[8] * b;
            let max = 255f64;

            #[allow(deprecated)]
            let outpixel = Pixel::from_channels(
                NumCast::from(clamp(new_r, 0.0, max)).unwrap(),
                NumCast::from(clamp(new_g, 0.0, max)).unwrap(),
                NumCast::from(clamp(new_b, 0.0, max)).unwrap(),
                NumCast::from(clamp(vec.3, 0.0, max)).unwrap(),
            );

            image.put_pixel(x, y, outpixel);
        }
    }
}

/// A color map
pub trait ColorMap {
    /// The color type on which the map operates on
    type Color;
    /// Returns the index of the closest match of `color`
    /// in the color map.
    fn index_of(&self, color: &Self::Color) -> usize;
    /// Looks up color by index in the color map.  If `idx` is out of range for the color map, or
    /// `ColorMap` doesn't implement `lookup` `None` is returned.
    fn lookup(&self, index: usize) -> Option<Self::Color> {
        let _ = index;
        None
    }
    /// Determine if this implementation of `ColorMap` overrides the default `lookup`.
    fn has_lookup(&self) -> bool {
        false
    }
    /// Maps `color` to the closest color in the color map.
    fn map_color(&self, color: &mut Self::Color);
}

/// A bi-level color map
///
/// # Examples
/// ```
/// use image::imageops::colorops::{index_colors, BiLevel, ColorMap};
/// use image::{ImageBuffer, Luma};
///
/// let (w, h) = (16, 16);
/// // Create an image with a smooth horizontal gradient from black (0) to white (255).
/// let gray = ImageBuffer::from_fn(w, h, |x, y| -> Luma<u8> { [(255 * x / w) as u8].into() });
/// // Mapping the gray image through the `BiLevel` filter should map gray pixels less than half
/// // intensity (127) to black (0), and anything greater to white (255).
/// let cmap = BiLevel;
/// let palletized = index_colors(&gray, &cmap);
/// let mapped = ImageBuffer::from_fn(w, h, |x, y| {
///     let p = palletized.get_pixel(x, y);
///     cmap.lookup(p.0[0] as usize)
///         .expect("indexed color out-of-range")
/// });
/// // Create an black and white image of expected output.
/// let bw = ImageBuffer::from_fn(w, h, |x, y| -> Luma<u8> {
///     if x <= (w / 2) {
///         [0].into()
///     } else {
///         [255].into()
///     }
/// });
/// assert_eq!(mapped, bw);
/// ```
#[derive(Clone, Copy)]
pub struct BiLevel;

impl ColorMap for BiLevel {
    type Color = Luma<u8>;

    #[inline(always)]
    fn index_of(&self, color: &Luma<u8>) -> usize {
        let luma = color.0;
        if luma[0] > 127 {
            1
        } else {
            0
        }
    }

    #[inline(always)]
    fn lookup(&self, idx: usize) -> Option<Self::Color> {
        match idx {
            0 => Some([0].into()),
            1 => Some([255].into()),
            _ => None,
        }
    }

    /// Indicate `NeuQuant` implements `lookup`.
    fn has_lookup(&self) -> bool {
        true
    }

    #[inline(always)]
    fn map_color(&self, color: &mut Luma<u8>) {
        let new_color = 0xFF * self.index_of(color) as u8;
        let luma = &mut color.0;
        luma[0] = new_color;
    }
}

#[cfg(feature = "color_quant")]
impl ColorMap for color_quant::NeuQuant {
    type Color = crate::color::Rgba<u8>;

    #[inline(always)]
    fn index_of(&self, color: &Self::Color) -> usize {
        self.index_of(color.channels())
    }

    #[inline(always)]
    fn lookup(&self, idx: usize) -> Option<Self::Color> {
        self.lookup(idx).map(|p| p.into())
    }

    /// Indicate NeuQuant implements `lookup`.
    fn has_lookup(&self) -> bool {
        true
    }

    #[inline(always)]
    fn map_color(&self, color: &mut Self::Color) {
        self.map_pixel(color.channels_mut());
    }
}

/// Floyd-Steinberg error diffusion
fn diffuse_err<P: Pixel<Subpixel = u8>>(pixel: &mut P, error: [i16; 3], factor: i16) {
    for (e, c) in error.iter().zip(pixel.channels_mut().iter_mut()) {
        *c = match <i16 as From<_>>::from(*c) + e * factor / 16 {
            val if val < 0 => 0,
            val if val > 0xFF => 0xFF,
            val => val as u8,
        }
    }
}

macro_rules! do_dithering(
    ($map:expr, $image:expr, $err:expr, $x:expr, $y:expr) => (
        {
            let old_pixel = $image[($x, $y)];
            let new_pixel = $image.get_pixel_mut($x, $y);
            $map.map_color(new_pixel);
            for ((e, &old), &new) in $err.iter_mut()
                                        .zip(old_pixel.channels().iter())
                                        .zip(new_pixel.channels().iter())
            {
                *e = <i16 as From<_>>::from(old) - <i16 as From<_>>::from(new)
            }
        }
    )
);

/// Reduces the colors of the image using the supplied `color_map` while applying
/// Floyd-Steinberg dithering to improve the visual conception
pub fn dither<Pix, Map>(image: &mut ImageBuffer<Pix, Vec<u8>>, color_map: &Map)
where
    Map: ColorMap<Color = Pix> + ?Sized,
    Pix: Pixel<Subpixel = u8> + 'static,
{
    let (width, height) = image.dimensions();
    let mut err: [i16; 3] = [0; 3];
    for y in 0..height - 1 {
        let x = 0;
        do_dithering!(color_map, image, err, x, y);
        diffuse_err(image.get_pixel_mut(x + 1, y), err, 7);
        diffuse_err(image.get_pixel_mut(x, y + 1), err, 5);
        diffuse_err(image.get_pixel_mut(x + 1, y + 1), err, 1);
        for x in 1..width - 1 {
            do_dithering!(color_map, image, err, x, y);
            diffuse_err(image.get_pixel_mut(x + 1, y), err, 7);
            diffuse_err(image.get_pixel_mut(x - 1, y + 1), err, 3);
            diffuse_err(image.get_pixel_mut(x, y + 1), err, 5);
            diffuse_err(image.get_pixel_mut(x + 1, y + 1), err, 1);
        }
        let x = width - 1;
        do_dithering!(color_map, image, err, x, y);
        diffuse_err(image.get_pixel_mut(x - 1, y + 1), err, 3);
        diffuse_err(image.get_pixel_mut(x, y + 1), err, 5);
    }
    let y = height - 1;
    let x = 0;
    do_dithering!(color_map, image, err, x, y);
    diffuse_err(image.get_pixel_mut(x + 1, y), err, 7);
    for x in 1..width - 1 {
        do_dithering!(color_map, image, err, x, y);
        diffuse_err(image.get_pixel_mut(x + 1, y), err, 7);
    }
    let x = width - 1;
    do_dithering!(color_map, image, err, x, y);
}

/// Reduces the colors using the supplied `color_map` and returns an image of the indices
pub fn index_colors<Pix, Map>(
    image: &ImageBuffer<Pix, Vec<u8>>,
    color_map: &Map,
) -> ImageBuffer<Luma<u8>, Vec<u8>>
where
    Map: ColorMap<Color = Pix> + ?Sized,
    Pix: Pixel<Subpixel = u8> + 'static,
{
    // Special case, we do *not* want to copy the color space here.
    let mut indices = ImageBuffer::new(image.width(), image.height());
    indices.set_rgb_primaries(CicpColorPrimaries::Unspecified);
    indices.set_transfer_function(CicpTransferCharacteristics::Unspecified);
    for (pixel, idx) in image.pixels().zip(indices.pixels_mut()) {
        *idx = Luma([color_map.index_of(pixel) as u8]);
    }
    indices
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::GrayImage;

    macro_rules! assert_pixels_eq {
        ($actual:expr, $expected:expr) => {{
            let actual_dim = $actual.dimensions();
            let expected_dim = $expected.dimensions();

            if actual_dim != expected_dim {
                panic!(
                    "dimensions do not match. \
                     actual: {:?}, expected: {:?}",
                    actual_dim, expected_dim
                )
            }

            let diffs = pixel_diffs($actual, $expected);

            if !diffs.is_empty() {
                let mut err = "".to_string();

                let diff_messages = diffs
                    .iter()
                    .take(5)
                    .map(|d| format!("\nactual: {:?}, expected {:?} ", d.0, d.1))
                    .collect::<Vec<_>>()
                    .join("");

                err.push_str(&diff_messages);
                panic!("pixels do not match. {:?}", err)
            }
        }};
    }

    #[test]
    fn test_dither() {
        let mut image = ImageBuffer::from_raw(2, 2, vec![127, 127, 127, 127]).unwrap();
        let cmap = BiLevel;
        dither(&mut image, &cmap);
        assert_eq!(&*image, &[0, 0xFF, 0xFF, 0]);
        assert_eq!(index_colors(&image, &cmap).into_raw(), vec![0, 1, 1, 0]);
    }

    #[test]
    fn test_grayscale() {
        let image: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![0u8, 1u8, 2u8, 10u8, 11u8, 12u8]).unwrap();

        let expected: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![0u8, 1u8, 2u8, 10u8, 11u8, 12u8]).unwrap();

        assert_pixels_eq!(&grayscale(&image), &expected);
    }

    #[test]
    fn test_invert() {
        let mut image: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![0u8, 1u8, 2u8, 10u8, 11u8, 12u8]).unwrap();

        let expected: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![255u8, 254u8, 253u8, 245u8, 244u8, 243u8]).unwrap();

        invert(&mut image);
        assert_pixels_eq!(&image, &expected);
    }
    #[test]
    fn test_brighten() {
        let image: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![0u8, 1u8, 2u8, 10u8, 11u8, 12u8]).unwrap();

        let expected: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![10u8, 11u8, 12u8, 20u8, 21u8, 22u8]).unwrap();

        assert_pixels_eq!(&brighten(&image, 10), &expected);
    }

    #[test]
    fn test_brighten_place() {
        let mut image: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![0u8, 1u8, 2u8, 10u8, 11u8, 12u8]).unwrap();

        let expected: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![10u8, 11u8, 12u8, 20u8, 21u8, 22u8]).unwrap();

        brighten_in_place(&mut image, 10);
        assert_pixels_eq!(&image, &expected);
    }

    #[allow(clippy::type_complexity)]
    fn pixel_diffs<I, J, P>(left: &I, right: &J) -> Vec<((u32, u32, P), (u32, u32, P))>
    where
        I: GenericImage<Pixel = P>,
        J: GenericImage<Pixel = P>,
        P: Pixel + Eq,
    {
        left.pixels()
            .zip(right.pixels())
            .filter(|&(p, q)| p != q)
            .collect::<Vec<_>>()
    }
}
