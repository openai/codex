//! Image Processing Functions
use std::cmp;

use crate::traits::{Lerp, Pixel, Primitive};
use crate::{GenericImage, GenericImageView, SubImage};

pub use self::sample::FilterType;

pub use self::sample::FilterType::{CatmullRom, Gaussian, Lanczos3, Nearest, Triangle};

/// Affine transformations
pub use self::affine::{
    flip_horizontal, flip_horizontal_in, flip_horizontal_in_place, flip_vertical, flip_vertical_in,
    flip_vertical_in_place, rotate180, rotate180_in, rotate180_in_place, rotate270, rotate270_in,
    rotate90, rotate90_in,
};

pub use self::sample::{
    blur, filter3x3, interpolate_bilinear, interpolate_nearest, resize, sample_bilinear,
    sample_nearest, thumbnail, unsharpen,
};

/// Color operations
pub use self::colorops::{
    brighten, contrast, dither, grayscale, grayscale_alpha, grayscale_with_type,
    grayscale_with_type_alpha, huerotate, index_colors, invert, BiLevel, ColorMap,
};

mod affine;
// Public only because of Rust bug:
// https://github.com/rust-lang/rust/issues/18241
pub mod colorops;
mod fast_blur;
mod filter_1d;
mod sample;

pub use fast_blur::fast_blur;
pub(crate) use sample::gaussian_blur_dyn_image;
pub use sample::{blur_advanced, GaussianBlurParameters};

/// Return a mutable view into an image
/// The coordinates set the position of the top left corner of the crop.
pub fn crop<I: GenericImageView>(
    image: &mut I,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> SubImage<&mut I> {
    let (x, y, width, height) = crop_dimms(image, x, y, width, height);
    SubImage::new(image, x, y, width, height)
}

/// Return an immutable view into an image
/// The coordinates set the position of the top left corner of the crop.
pub fn crop_imm<I: GenericImageView>(
    image: &I,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> SubImage<&I> {
    let (x, y, width, height) = crop_dimms(image, x, y, width, height);
    SubImage::new(image, x, y, width, height)
}

fn crop_dimms<I: GenericImageView>(
    image: &I,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> (u32, u32, u32, u32) {
    let (iwidth, iheight) = image.dimensions();

    let x = cmp::min(x, iwidth);
    let y = cmp::min(y, iheight);

    let height = cmp::min(height, iheight - y);
    let width = cmp::min(width, iwidth - x);

    (x, y, width, height)
}

/// Calculate the region that can be copied from top to bottom.
///
/// Given image size of bottom and top image, and a point at which we want to place the top image
/// onto the bottom image, how large can we be? Have to wary of the following issues:
/// * Top might be larger than bottom
/// * Overflows in the computation
/// * Coordinates could be completely out of bounds
///
/// The main idea is to make use of inequalities provided by the nature of `saturating_add` and
/// `saturating_sub`. These intrinsically validate that all resulting coordinates will be in bounds
/// for both images.
///
/// We want that all these coordinate accesses are safe:
/// 1. `bottom.get_pixel(x + [0..x_range), y + [0..y_range))`
/// 2. `top.get_pixel([0..x_range), [0..y_range))`
///
/// Proof that the function provides the necessary bounds for width. Note that all unaugmented math
/// operations are to be read in standard arithmetic, not integer arithmetic. Since no direct
/// integer arithmetic occurs in the implementation, this is unambiguous.
///
/// ```text
/// Three short notes/lemmata:
/// - Iff `(a - b) <= 0` then `a.saturating_sub(b) = 0`
/// - Iff `(a - b) >= 0` then `a.saturating_sub(b) = a - b`
/// - If  `a <= c` then `a.saturating_sub(b) <= c.saturating_sub(b)`
///
/// 1.1 We show that if `bottom_width <= x`, then `x_range = 0` therefore `x + [0..x_range)` is empty.
///
/// x_range
///  = (top_width.saturating_add(x).min(bottom_width)).saturating_sub(x)
/// <= bottom_width.saturating_sub(x)
///
/// bottom_width <= x
/// <==> bottom_width - x <= 0
/// <==> bottom_width.saturating_sub(x) = 0
///  ==> x_range <= 0
///  ==> x_range  = 0
///
/// 1.2 If `x < bottom_width` then `x + x_range < bottom_width`
///
/// x + x_range
/// <= x + bottom_width.saturating_sub(x)
///  = x + (bottom_width - x)
///  = bottom_width
///
/// 2. We show that `x_range <= top_width`
///
/// x_range
///  = (top_width.saturating_add(x).min(bottom_width)).saturating_sub(x)
/// <= top_width.saturating_add(x).saturating_sub(x)
/// <= (top_wdith + x).saturating_sub(x)
///  = top_width (due to `top_width >= 0` and `x >= 0`)
/// ```
///
/// Proof is the same for height.
#[must_use]
pub fn overlay_bounds(
    (bottom_width, bottom_height): (u32, u32),
    (top_width, top_height): (u32, u32),
    x: u32,
    y: u32,
) -> (u32, u32) {
    let x_range = top_width
        .saturating_add(x) // Calculate max coordinate
        .min(bottom_width) // Restrict to lower width
        .saturating_sub(x); // Determinate length from start `x`
    let y_range = top_height
        .saturating_add(y)
        .min(bottom_height)
        .saturating_sub(y);
    (x_range, y_range)
}

/// Calculate the region that can be copied from top to bottom.
///
/// Given image size of bottom and top image, and a point at which we want to place the top image
/// onto the bottom image, how large can we be? Have to wary of the following issues:
/// * Top might be larger than bottom
/// * Overflows in the computation
/// * Coordinates could be completely out of bounds
///
/// The returned value is of the form:
///
/// `(origin_bottom_x, origin_bottom_y, origin_top_x, origin_top_y, x_range, y_range)`
///
/// The main idea is to do computations on i64's and then clamp to image dimensions.
/// In particular, we want to ensure that all these coordinate accesses are safe:
/// 1. `bottom.get_pixel(origin_bottom_x + [0..x_range), origin_bottom_y + [0..y_range))`
/// 2. `top.get_pixel(origin_top_y + [0..x_range), origin_top_y + [0..y_range))`
fn overlay_bounds_ext(
    (bottom_width, bottom_height): (u32, u32),
    (top_width, top_height): (u32, u32),
    x: i64,
    y: i64,
) -> (u32, u32, u32, u32, u32, u32) {
    // Return a predictable value if the two images don't overlap at all.
    if x > i64::from(bottom_width)
        || y > i64::from(bottom_height)
        || x.saturating_add(i64::from(top_width)) <= 0
        || y.saturating_add(i64::from(top_height)) <= 0
    {
        return (0, 0, 0, 0, 0, 0);
    }

    // Find the maximum x and y coordinates in terms of the bottom image.
    let max_x = x.saturating_add(i64::from(top_width));
    let max_y = y.saturating_add(i64::from(top_height));

    // Clip the origin and maximum coordinates to the bounds of the bottom image.
    // Casting to a u32 is safe because both 0 and `bottom_{width,height}` fit
    // into 32-bits.
    let max_inbounds_x = max_x.clamp(0, i64::from(bottom_width)) as u32;
    let max_inbounds_y = max_y.clamp(0, i64::from(bottom_height)) as u32;
    let origin_bottom_x = x.clamp(0, i64::from(bottom_width)) as u32;
    let origin_bottom_y = y.clamp(0, i64::from(bottom_height)) as u32;

    // The range is the difference between the maximum inbounds coordinates and
    // the clipped origin. Unchecked subtraction is safe here because both are
    // always positive and `max_inbounds_{x,y}` >= `origin_{x,y}` due to
    // `top_{width,height}` being >= 0.
    let x_range = max_inbounds_x - origin_bottom_x;
    let y_range = max_inbounds_y - origin_bottom_y;

    // If x (or y) is negative, then the origin of the top image is shifted by -x (or -y).
    let origin_top_x = x.saturating_mul(-1).clamp(0, i64::from(top_width)) as u32;
    let origin_top_y = y.saturating_mul(-1).clamp(0, i64::from(top_height)) as u32;

    (
        origin_bottom_x,
        origin_bottom_y,
        origin_top_x,
        origin_top_y,
        x_range,
        y_range,
    )
}

/// Overlay an image at a given coordinate (x, y)
pub fn overlay<I, J>(bottom: &mut I, top: &J, x: i64, y: i64)
where
    I: GenericImage,
    J: GenericImageView<Pixel = I::Pixel>,
{
    let bottom_dims = bottom.dimensions();
    let top_dims = top.dimensions();

    // Crop our top image if we're going out of bounds
    let (origin_bottom_x, origin_bottom_y, origin_top_x, origin_top_y, range_width, range_height) =
        overlay_bounds_ext(bottom_dims, top_dims, x, y);

    for y in 0..range_height {
        for x in 0..range_width {
            let p = top.get_pixel(origin_top_x + x, origin_top_y + y);
            let mut bottom_pixel = bottom.get_pixel(origin_bottom_x + x, origin_bottom_y + y);
            bottom_pixel.blend(&p);

            bottom.put_pixel(origin_bottom_x + x, origin_bottom_y + y, bottom_pixel);
        }
    }
}

/// Tile an image by repeating it multiple times
///
/// # Examples
/// ```no_run
/// use image::RgbaImage;
///
/// let mut img = RgbaImage::new(1920, 1080);
/// let tile = image::open("tile.png").unwrap();
///
/// image::imageops::tile(&mut img, &tile);
/// img.save("tiled_wallpaper.png").unwrap();
/// ```
pub fn tile<I, J>(bottom: &mut I, top: &J)
where
    I: GenericImage,
    J: GenericImageView<Pixel = I::Pixel>,
{
    for x in (0..bottom.width()).step_by(top.width() as usize) {
        for y in (0..bottom.height()).step_by(top.height() as usize) {
            overlay(bottom, top, i64::from(x), i64::from(y));
        }
    }
}

/// Fill the image with a linear vertical gradient
///
/// This function assumes a linear color space.
///
/// # Examples
/// ```no_run
/// use image::{Rgba, RgbaImage, Pixel};
///
/// let mut img = RgbaImage::new(100, 100);
/// let start = Rgba::from_slice(&[0, 128, 0, 0]);
/// let end = Rgba::from_slice(&[255, 255, 255, 255]);
///
/// image::imageops::vertical_gradient(&mut img, start, end);
/// img.save("vertical_gradient.png").unwrap();
pub fn vertical_gradient<S, P, I>(img: &mut I, start: &P, stop: &P)
where
    I: GenericImage<Pixel = P>,
    P: Pixel<Subpixel = S> + 'static,
    S: Primitive + Lerp + 'static,
{
    for y in 0..img.height() {
        let pixel = start.map2(stop, |a, b| {
            let y = <S::Ratio as num_traits::NumCast>::from(y).unwrap();
            let height = <S::Ratio as num_traits::NumCast>::from(img.height() - 1).unwrap();
            S::lerp(a, b, y / height)
        });

        for x in 0..img.width() {
            img.put_pixel(x, y, pixel);
        }
    }
}

/// Fill the image with a linear horizontal gradient
///
/// This function assumes a linear color space.
///
/// # Examples
/// ```no_run
/// use image::{Rgba, RgbaImage, Pixel};
///
/// let mut img = RgbaImage::new(100, 100);
/// let start = Rgba::from_slice(&[0, 128, 0, 0]);
/// let end = Rgba::from_slice(&[255, 255, 255, 255]);
///
/// image::imageops::horizontal_gradient(&mut img, start, end);
/// img.save("horizontal_gradient.png").unwrap();
pub fn horizontal_gradient<S, P, I>(img: &mut I, start: &P, stop: &P)
where
    I: GenericImage<Pixel = P>,
    P: Pixel<Subpixel = S> + 'static,
    S: Primitive + Lerp + 'static,
{
    for x in 0..img.width() {
        let pixel = start.map2(stop, |a, b| {
            let x = <S::Ratio as num_traits::NumCast>::from(x).unwrap();
            let width = <S::Ratio as num_traits::NumCast>::from(img.width() - 1).unwrap();
            S::lerp(a, b, x / width)
        });

        for y in 0..img.height() {
            img.put_pixel(x, y, pixel);
        }
    }
}

/// Replace the contents of an image at a given coordinate (x, y)
pub fn replace<I, J>(bottom: &mut I, top: &J, x: i64, y: i64)
where
    I: GenericImage,
    J: GenericImageView<Pixel = I::Pixel>,
{
    let bottom_dims = bottom.dimensions();
    let top_dims = top.dimensions();

    // Crop our top image if we're going out of bounds
    let (origin_bottom_x, origin_bottom_y, origin_top_x, origin_top_y, range_width, range_height) =
        overlay_bounds_ext(bottom_dims, top_dims, x, y);

    for y in 0..range_height {
        for x in 0..range_width {
            let p = top.get_pixel(origin_top_x + x, origin_top_y + y);
            bottom.put_pixel(origin_bottom_x + x, origin_bottom_y + y, p);
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::color::Rgb;
    use crate::GrayAlphaImage;
    use crate::GrayImage;
    use crate::ImageBuffer;
    use crate::RgbImage;
    use crate::RgbaImage;

    #[test]
    fn test_overlay_bounds_ext() {
        assert_eq!(
            overlay_bounds_ext((10, 10), (10, 10), 0, 0),
            (0, 0, 0, 0, 10, 10)
        );
        assert_eq!(
            overlay_bounds_ext((10, 10), (10, 10), 1, 0),
            (1, 0, 0, 0, 9, 10)
        );
        assert_eq!(
            overlay_bounds_ext((10, 10), (10, 10), 0, 11),
            (0, 0, 0, 0, 0, 0)
        );
        assert_eq!(
            overlay_bounds_ext((10, 10), (10, 10), -1, 0),
            (0, 0, 1, 0, 9, 10)
        );
        assert_eq!(
            overlay_bounds_ext((10, 10), (10, 10), -10, 0),
            (0, 0, 0, 0, 0, 0)
        );
        assert_eq!(
            overlay_bounds_ext((10, 10), (10, 10), 1i64 << 50, 0),
            (0, 0, 0, 0, 0, 0)
        );
        assert_eq!(
            overlay_bounds_ext((10, 10), (10, 10), -(1i64 << 50), 0),
            (0, 0, 0, 0, 0, 0)
        );
        assert_eq!(
            overlay_bounds_ext((10, 10), (u32::MAX, 10), 10 - i64::from(u32::MAX), 0),
            (0, 0, u32::MAX - 10, 0, 10, 10)
        );
    }

    #[test]
    /// Test that images written into other images works
    fn test_image_in_image() {
        let mut target = ImageBuffer::new(32, 32);
        let source = ImageBuffer::from_pixel(16, 16, Rgb([255u8, 0, 0]));
        overlay(&mut target, &source, 0, 0);
        assert!(*target.get_pixel(0, 0) == Rgb([255u8, 0, 0]));
        assert!(*target.get_pixel(15, 0) == Rgb([255u8, 0, 0]));
        assert!(*target.get_pixel(16, 0) == Rgb([0u8, 0, 0]));
        assert!(*target.get_pixel(0, 15) == Rgb([255u8, 0, 0]));
        assert!(*target.get_pixel(0, 16) == Rgb([0u8, 0, 0]));
    }

    #[test]
    /// Test that images written outside of a frame doesn't blow up
    fn test_image_in_image_outside_of_bounds() {
        let mut target = ImageBuffer::new(32, 32);
        let source = ImageBuffer::from_pixel(32, 32, Rgb([255u8, 0, 0]));
        overlay(&mut target, &source, 1, 1);
        assert!(*target.get_pixel(0, 0) == Rgb([0, 0, 0]));
        assert!(*target.get_pixel(1, 1) == Rgb([255u8, 0, 0]));
        assert!(*target.get_pixel(31, 31) == Rgb([255u8, 0, 0]));
    }

    #[test]
    /// Test that images written to coordinates out of the frame doesn't blow up
    /// (issue came up in #848)
    fn test_image_outside_image_no_wrap_around() {
        let mut target = ImageBuffer::new(32, 32);
        let source = ImageBuffer::from_pixel(32, 32, Rgb([255u8, 0, 0]));
        overlay(&mut target, &source, 33, 33);
        assert!(*target.get_pixel(0, 0) == Rgb([0, 0, 0]));
        assert!(*target.get_pixel(1, 1) == Rgb([0, 0, 0]));
        assert!(*target.get_pixel(31, 31) == Rgb([0, 0, 0]));
    }

    #[test]
    /// Test that images written to coordinates with overflow works
    fn test_image_coordinate_overflow() {
        let mut target = ImageBuffer::new(16, 16);
        let source = ImageBuffer::from_pixel(32, 32, Rgb([255u8, 0, 0]));
        // Overflows to 'sane' coordinates but top is larger than bot.
        overlay(
            &mut target,
            &source,
            i64::from(u32::MAX - 31),
            i64::from(u32::MAX - 31),
        );
        assert!(*target.get_pixel(0, 0) == Rgb([0, 0, 0]));
        assert!(*target.get_pixel(1, 1) == Rgb([0, 0, 0]));
        assert!(*target.get_pixel(15, 15) == Rgb([0, 0, 0]));
    }

    use super::{horizontal_gradient, vertical_gradient};

    #[test]
    /// Test that horizontal gradients are correctly generated
    fn test_image_horizontal_gradient_limits() {
        let mut img = ImageBuffer::new(100, 1);

        let start = Rgb([0u8, 128, 0]);
        let end = Rgb([255u8, 255, 255]);

        horizontal_gradient(&mut img, &start, &end);

        assert_eq!(img.get_pixel(0, 0), &start);
        assert_eq!(img.get_pixel(img.width() - 1, 0), &end);
    }

    #[test]
    /// Test that vertical gradients are correctly generated
    fn test_image_vertical_gradient_limits() {
        let mut img = ImageBuffer::new(1, 100);

        let start = Rgb([0u8, 128, 0]);
        let end = Rgb([255u8, 255, 255]);

        vertical_gradient(&mut img, &start, &end);

        assert_eq!(img.get_pixel(0, 0), &start);
        assert_eq!(img.get_pixel(0, img.height() - 1), &end);
    }

    #[test]
    /// Test blur doesn't panic when passed 0.0
    fn test_blur_zero() {
        let image = RgbaImage::new(50, 50);
        let _ = blur(&image, 0.);
    }

    #[test]
    /// Test fast blur doesn't panic when passed 0.0
    fn test_fast_blur_zero() {
        let image = RgbaImage::new(50, 50);
        let _ = fast_blur(&image, 0.0);
    }

    #[test]
    /// Test fast blur doesn't panic when passed negative numbers
    fn test_fast_blur_negative() {
        let image = RgbaImage::new(50, 50);
        let _ = fast_blur(&image, -1.0);
    }

    #[test]
    /// Test fast blur doesn't panic when sigma produces boxes larger than the image
    fn test_fast_large_sigma() {
        let image = RgbaImage::new(1, 1);
        let _ = fast_blur(&image, 50.0);
    }

    #[test]
    /// Test blur doesn't panic when passed an empty image (any direction)
    fn test_fast_blur_empty() {
        let image = RgbaImage::new(0, 0);
        let _ = fast_blur(&image, 1.0);
        let image = RgbaImage::new(20, 0);
        let _ = fast_blur(&image, 1.0);
        let image = RgbaImage::new(0, 20);
        let _ = fast_blur(&image, 1.0);
    }

    #[test]
    /// Test fast blur works with 3 channels
    fn test_fast_blur_3_channels() {
        let image = RgbImage::new(50, 50);
        let _ = fast_blur(&image, 1.0);
    }

    #[test]
    /// Test fast blur works with 2 channels
    fn test_fast_blur_2_channels() {
        let image = GrayAlphaImage::new(50, 50);
        let _ = fast_blur(&image, 1.0);
    }

    #[test]
    /// Test fast blur works with 1 channel
    fn test_fast_blur_1_channels() {
        let image = GrayImage::new(50, 50);
        let _ = fast_blur(&image, 1.0);
    }

    #[test]
    #[cfg(feature = "tiff")]
    fn fast_blur_approximates_gaussian_blur_well() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/images/tiff/testsuite/rgb-3c-16b.tiff"
        );
        let image = crate::open(path).unwrap();
        let image_blurred_gauss = image
            .blur_advanced(GaussianBlurParameters::new_from_sigma(50.0))
            .to_rgb8();
        let image_blurred_gauss_samples = image_blurred_gauss.as_flat_samples();
        let image_blurred_gauss_bytes = image_blurred_gauss_samples.as_slice();
        let image_blurred_fast = image.fast_blur(50.0).to_rgb8();
        let image_blurred_fast_samples = image_blurred_fast.as_flat_samples();
        let image_blurred_fast_bytes = image_blurred_fast_samples.as_slice();

        let error = image_blurred_gauss_bytes
            .iter()
            .zip(image_blurred_fast_bytes.iter())
            .map(|(a, b)| (f32::from(*a) - f32::from(*b)) / f32::from(*a))
            .sum::<f32>()
            / (image_blurred_gauss_bytes.len() as f32);
        assert!(error < 0.05);
    }
}
