//! Functions for performing affine transformations.

use crate::error::{ImageError, ParameterError, ParameterErrorKind};
use crate::traits::Pixel;
use crate::{GenericImage, GenericImageView, ImageBuffer};

/// Rotate an image 90 degrees clockwise.
pub fn rotate90<I: GenericImageView>(
    image: &I,
) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
where
    I::Pixel: 'static,
{
    let (width, height) = image.dimensions();
    let mut out = image.buffer_with_dimensions(height, width);
    let _ = rotate90_in(image, &mut out);
    out
}

/// Rotate an image 180 degrees clockwise.
pub fn rotate180<I: GenericImageView>(
    image: &I,
) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
where
    I::Pixel: 'static,
{
    let (width, height) = image.dimensions();
    let mut out = image.buffer_with_dimensions(width, height);
    let _ = rotate180_in(image, &mut out);
    out
}

/// Rotate an image 270 degrees clockwise.
pub fn rotate270<I: GenericImageView>(
    image: &I,
) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
where
    I::Pixel: 'static,
{
    let (width, height) = image.dimensions();
    let mut out = image.buffer_with_dimensions(height, width);
    let _ = rotate270_in(image, &mut out);
    out
}

/// Rotate an image 90 degrees clockwise and put the result into the destination [`ImageBuffer`].
pub fn rotate90_in<I, Container>(
    image: &I,
    destination: &mut ImageBuffer<I::Pixel, Container>,
) -> crate::ImageResult<()>
where
    I: GenericImageView,
    I::Pixel: 'static,
    Container: std::ops::DerefMut<Target = [<I::Pixel as Pixel>::Subpixel]>,
{
    let ((w0, h0), (w1, h1)) = (image.dimensions(), destination.dimensions());
    if w0 != h1 || h0 != w1 {
        return Err(ImageError::Parameter(ParameterError::from_kind(
            ParameterErrorKind::DimensionMismatch,
        )));
    }

    for y in 0..h0 {
        for x in 0..w0 {
            let p = image.get_pixel(x, y);
            destination.put_pixel(h0 - y - 1, x, p);
        }
    }
    Ok(())
}

/// Rotate an image 180 degrees clockwise and put the result into the destination [`ImageBuffer`].
pub fn rotate180_in<I, Container>(
    image: &I,
    destination: &mut ImageBuffer<I::Pixel, Container>,
) -> crate::ImageResult<()>
where
    I: GenericImageView,
    I::Pixel: 'static,
    Container: std::ops::DerefMut<Target = [<I::Pixel as Pixel>::Subpixel]>,
{
    let ((w0, h0), (w1, h1)) = (image.dimensions(), destination.dimensions());
    if w0 != w1 || h0 != h1 {
        return Err(ImageError::Parameter(ParameterError::from_kind(
            ParameterErrorKind::DimensionMismatch,
        )));
    }

    for y in 0..h0 {
        for x in 0..w0 {
            let p = image.get_pixel(x, y);
            destination.put_pixel(w0 - x - 1, h0 - y - 1, p);
        }
    }
    Ok(())
}

/// Rotate an image 270 degrees clockwise and put the result into the destination [`ImageBuffer`].
pub fn rotate270_in<I, Container>(
    image: &I,
    destination: &mut ImageBuffer<I::Pixel, Container>,
) -> crate::ImageResult<()>
where
    I: GenericImageView,
    I::Pixel: 'static,
    Container: std::ops::DerefMut<Target = [<I::Pixel as Pixel>::Subpixel]>,
{
    let ((w0, h0), (w1, h1)) = (image.dimensions(), destination.dimensions());
    if w0 != h1 || h0 != w1 {
        return Err(ImageError::Parameter(ParameterError::from_kind(
            ParameterErrorKind::DimensionMismatch,
        )));
    }

    for y in 0..h0 {
        for x in 0..w0 {
            let p = image.get_pixel(x, y);
            destination.put_pixel(y, w0 - x - 1, p);
        }
    }
    Ok(())
}

/// Flip an image horizontally
pub fn flip_horizontal<I: GenericImageView>(
    image: &I,
) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
where
    I::Pixel: 'static,
{
    let mut out = image.buffer_like();
    let _ = flip_horizontal_in(image, &mut out);
    out
}

/// Flip an image vertically
pub fn flip_vertical<I: GenericImageView>(
    image: &I,
) -> ImageBuffer<I::Pixel, Vec<<I::Pixel as Pixel>::Subpixel>>
where
    I::Pixel: 'static,
{
    let mut out = image.buffer_like();
    let _ = flip_vertical_in(image, &mut out);
    out
}

/// Flip an image horizontally and put the result into the destination [`ImageBuffer`].
pub fn flip_horizontal_in<I, Container>(
    image: &I,
    destination: &mut ImageBuffer<I::Pixel, Container>,
) -> crate::ImageResult<()>
where
    I: GenericImageView,
    I::Pixel: 'static,
    Container: std::ops::DerefMut<Target = [<I::Pixel as Pixel>::Subpixel]>,
{
    let ((w0, h0), (w1, h1)) = (image.dimensions(), destination.dimensions());
    if w0 != w1 || h0 != h1 {
        return Err(ImageError::Parameter(ParameterError::from_kind(
            ParameterErrorKind::DimensionMismatch,
        )));
    }

    for y in 0..h0 {
        for x in 0..w0 {
            let p = image.get_pixel(x, y);
            destination.put_pixel(w0 - x - 1, y, p);
        }
    }
    Ok(())
}

/// Flip an image vertically and put the result into the destination [`ImageBuffer`].
pub fn flip_vertical_in<I, Container>(
    image: &I,
    destination: &mut ImageBuffer<I::Pixel, Container>,
) -> crate::ImageResult<()>
where
    I: GenericImageView,
    I::Pixel: 'static,
    Container: std::ops::DerefMut<Target = [<I::Pixel as Pixel>::Subpixel]>,
{
    let ((w0, h0), (w1, h1)) = (image.dimensions(), destination.dimensions());
    if w0 != w1 || h0 != h1 {
        return Err(ImageError::Parameter(ParameterError::from_kind(
            ParameterErrorKind::DimensionMismatch,
        )));
    }

    for y in 0..h0 {
        for x in 0..w0 {
            let p = image.get_pixel(x, y);
            destination.put_pixel(x, h0 - 1 - y, p);
        }
    }
    Ok(())
}

/// Rotate an image 180 degrees clockwise in place.
pub fn rotate180_in_place<I: GenericImage>(image: &mut I) {
    let (width, height) = image.dimensions();

    for y in 0..height / 2 {
        for x in 0..width {
            let p = image.get_pixel(x, y);

            let x2 = width - x - 1;
            let y2 = height - y - 1;

            let p2 = image.get_pixel(x2, y2);
            image.put_pixel(x, y, p2);
            image.put_pixel(x2, y2, p);
        }
    }

    if height % 2 != 0 {
        let middle = height / 2;

        for x in 0..width / 2 {
            let p = image.get_pixel(x, middle);
            let x2 = width - x - 1;

            let p2 = image.get_pixel(x2, middle);
            image.put_pixel(x, middle, p2);
            image.put_pixel(x2, middle, p);
        }
    }
}

/// Flip an image horizontally in place.
pub fn flip_horizontal_in_place<I: GenericImage>(image: &mut I) {
    let (width, height) = image.dimensions();

    for y in 0..height {
        for x in 0..width / 2 {
            let x2 = width - x - 1;
            let p2 = image.get_pixel(x2, y);
            let p = image.get_pixel(x, y);
            image.put_pixel(x2, y, p);
            image.put_pixel(x, y, p2);
        }
    }
}

/// Flip an image vertically in place.
pub fn flip_vertical_in_place<I: GenericImage>(image: &mut I) {
    let (width, height) = image.dimensions();

    for y in 0..height / 2 {
        for x in 0..width {
            let y2 = height - y - 1;
            let p2 = image.get_pixel(x, y2);
            let p = image.get_pixel(x, y);
            image.put_pixel(x, y2, p);
            image.put_pixel(x, y, p2);
        }
    }
}

#[cfg(test)]
mod test {
    use super::{
        flip_horizontal, flip_horizontal_in_place, flip_vertical, flip_vertical_in_place,
        rotate180, rotate180_in_place, rotate270, rotate90,
    };

    use crate::traits::Pixel;
    use crate::{GenericImage, GrayImage, ImageBuffer};

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
    fn test_rotate90() {
        let image: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![0u8, 1u8, 2u8, 10u8, 11u8, 12u8]).unwrap();

        let expected: GrayImage =
            ImageBuffer::from_raw(2, 3, vec![10u8, 0u8, 11u8, 1u8, 12u8, 2u8]).unwrap();

        assert_pixels_eq!(&rotate90(&image), &expected);
    }

    #[test]
    fn test_rotate180() {
        let image: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![0u8, 1u8, 2u8, 10u8, 11u8, 12u8]).unwrap();

        let expected: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![12u8, 11u8, 10u8, 2u8, 1u8, 0u8]).unwrap();

        assert_pixels_eq!(&rotate180(&image), &expected);
    }

    #[test]
    fn test_rotate270() {
        let image: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![0u8, 1u8, 2u8, 10u8, 11u8, 12u8]).unwrap();

        let expected: GrayImage =
            ImageBuffer::from_raw(2, 3, vec![2u8, 12u8, 1u8, 11u8, 0u8, 10u8]).unwrap();

        assert_pixels_eq!(&rotate270(&image), &expected);
    }

    #[test]
    fn test_rotate180_in_place() {
        let mut image: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![0u8, 1u8, 2u8, 10u8, 11u8, 12u8]).unwrap();

        let expected: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![12u8, 11u8, 10u8, 2u8, 1u8, 0u8]).unwrap();

        rotate180_in_place(&mut image);

        assert_pixels_eq!(&image, &expected);
    }

    #[test]
    fn test_flip_horizontal() {
        let image: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![0u8, 1u8, 2u8, 10u8, 11u8, 12u8]).unwrap();

        let expected: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![2u8, 1u8, 0u8, 12u8, 11u8, 10u8]).unwrap();

        assert_pixels_eq!(&flip_horizontal(&image), &expected);
    }

    #[test]
    fn test_flip_vertical() {
        let image: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![0u8, 1u8, 2u8, 10u8, 11u8, 12u8]).unwrap();

        let expected: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![10u8, 11u8, 12u8, 0u8, 1u8, 2u8]).unwrap();

        assert_pixels_eq!(&flip_vertical(&image), &expected);
    }

    #[test]
    fn test_flip_horizontal_in_place() {
        let mut image: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![0u8, 1u8, 2u8, 10u8, 11u8, 12u8]).unwrap();

        let expected: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![2u8, 1u8, 0u8, 12u8, 11u8, 10u8]).unwrap();

        flip_horizontal_in_place(&mut image);

        assert_pixels_eq!(&image, &expected);
    }

    #[test]
    fn test_flip_vertical_in_place() {
        let mut image: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![0u8, 1u8, 2u8, 10u8, 11u8, 12u8]).unwrap();

        let expected: GrayImage =
            ImageBuffer::from_raw(3, 2, vec![10u8, 11u8, 12u8, 0u8, 1u8, 2u8]).unwrap();

        flip_vertical_in_place(&mut image);

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
