use crate::error::{ImageError, ImageResult, ParameterError, ParameterErrorKind};
use crate::math::Rect;
use crate::traits::Pixel;
use crate::{ImageBuffer, SubImage};

/// Trait to inspect an image.
///
/// ```
/// use image::{GenericImageView, Rgb, RgbImage};
///
/// let buffer = RgbImage::new(10, 10);
/// let image: &dyn GenericImageView<Pixel = Rgb<u8>> = &buffer;
/// ```
pub trait GenericImageView {
    /// The type of pixel.
    type Pixel: Pixel;

    /// The width and height of this image.
    fn dimensions(&self) -> (u32, u32);

    /// The width of this image.
    fn width(&self) -> u32 {
        let (w, _) = self.dimensions();
        w
    }

    /// The height of this image.
    fn height(&self) -> u32 {
        let (_, h) = self.dimensions();
        h
    }

    /// Returns true if this x, y coordinate is contained inside the image.
    fn in_bounds(&self, x: u32, y: u32) -> bool {
        let (width, height) = self.dimensions();
        x < width && y < height
    }

    /// Returns the pixel located at (x, y). Indexed from top left.
    ///
    /// # Panics
    ///
    /// Panics if `(x, y)` is out of bounds.
    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel;

    /// Returns the pixel located at (x, y). Indexed from top left.
    ///
    /// This function can be implemented in a way that ignores bounds checking.
    /// # Safety
    ///
    /// The coordinates must be [`in_bounds`] of the image.
    ///
    /// [`in_bounds`]: #method.in_bounds
    unsafe fn unsafe_get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        self.get_pixel(x, y)
    }

    /// Returns an Iterator over the pixels of this image.
    /// The iterator yields the coordinates of each pixel
    /// along with their value
    fn pixels(&self) -> Pixels<'_, Self>
    where
        Self: Sized,
    {
        let (width, height) = self.dimensions();

        Pixels {
            image: self,
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    /// Returns a subimage that is an immutable view into this image.
    /// You can use [`GenericImage::sub_image`] if you need a mutable view instead.
    /// The coordinates set the position of the top left corner of the view.
    ///
    ///  # Panics
    ///
    /// Panics if the dimensions provided fall out of bounds.
    fn view(&self, x: u32, y: u32, width: u32, height: u32) -> SubImage<&Self>
    where
        Self: Sized,
    {
        assert!(u64::from(x) + u64::from(width) <= u64::from(self.width()));
        assert!(u64::from(y) + u64::from(height) <= u64::from(self.height()));
        SubImage::new(self, x, y, width, height)
    }

    /// Returns a subimage that is an immutable view into this image so long as
    /// the provided coordinates and dimensions are within the bounds of this Image.
    fn try_view(
        &self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<SubImage<&Self>, ImageError>
    where
        Self: Sized,
    {
        if u64::from(x) + u64::from(width) > u64::from(self.width())
            || u64::from(y) + u64::from(height) > u64::from(self.height())
        {
            Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::DimensionMismatch,
            )))
        } else {
            Ok(SubImage::new(self, x, y, width, height))
        }
    }

    /// Create an empty [`ImageBuffer`] with the same pixel type as this image.
    ///
    /// This should ensure metadata such as the color space are transferred without copying any of
    /// the pixel data. The idea is to prepare a buffer ready to be filled with a filtered or
    /// portion of the channel data from the current image without performing the work of copying
    /// the data into that buffer twice.
    ///
    /// The default implementation defers to [`GenericImageView::buffer_like`].
    fn buffer_like(&self) -> ImageBuffer<Self::Pixel, Vec<<Self::Pixel as Pixel>::Subpixel>> {
        let (w, h) = self.dimensions();
        self.buffer_with_dimensions(w, h)
    }

    /// Create an empty [`ImageBuffer`] with different dimensions.
    ///
    /// See [`GenericImageView::buffer_like`].
    ///
    /// Uses for this are for instances preparing a buffer for only a portion of the image, or
    /// extracting the metadata to prepare a buffer of a different pixel type.
    fn buffer_with_dimensions(
        &self,
        width: u32,
        height: u32,
    ) -> ImageBuffer<Self::Pixel, Vec<<Self::Pixel as Pixel>::Subpixel>> {
        ImageBuffer::new(width, height)
    }
}

/// Immutable pixel iterator
#[derive(Debug)]
pub struct Pixels<'a, I: ?Sized + 'a> {
    image: &'a I,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl<I: GenericImageView> Iterator for Pixels<'_, I> {
    type Item = (u32, u32, I::Pixel);

    fn next(&mut self) -> Option<(u32, u32, I::Pixel)> {
        if self.x >= self.width {
            self.x = 0;
            self.y += 1;
        }

        if self.y >= self.height {
            None
        } else {
            let pixel = self.image.get_pixel(self.x, self.y);
            let p = (self.x, self.y, pixel);

            self.x += 1;

            Some(p)
        }
    }
}

impl<I: ?Sized> Clone for Pixels<'_, I> {
    fn clone(&self) -> Self {
        Pixels { ..*self }
    }
}

/// A trait for manipulating images.
pub trait GenericImage: GenericImageView {
    /// Gets a reference to the mutable pixel at location `(x, y)`. Indexed from top left.
    ///
    /// # Panics
    ///
    /// Panics if `(x, y)` is out of bounds.
    ///
    /// Panics for dynamic images (this method is deprecated and will be removed).
    ///
    /// ## Known issues
    ///
    /// This requires the buffer to contain a unique set of continuous channels in the exact order
    /// and byte representation that the pixel type requires. This is somewhat restrictive.
    ///
    /// TODO: Maybe use some kind of entry API? this would allow pixel type conversion on the fly
    /// while still doing only one array lookup:
    ///
    /// ```ignore
    /// let px = image.pixel_entry_at(x,y);
    /// px.set_from_rgba(rgba)
    /// ```
    #[deprecated(since = "0.24.0", note = "Use `get_pixel` and `put_pixel` instead.")]
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel;

    /// Put a pixel at location (x, y). Indexed from top left.
    ///
    /// # Panics
    ///
    /// Panics if `(x, y)` is out of bounds.
    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel);

    /// Puts a pixel at location (x, y). Indexed from top left.
    ///
    /// This function can be implemented in a way that ignores bounds checking.
    /// # Safety
    ///
    /// The coordinates must be [`in_bounds`] of the image.
    ///
    /// [`in_bounds`]: traits.GenericImageView.html#method.in_bounds
    unsafe fn unsafe_put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        self.put_pixel(x, y, pixel);
    }

    /// Put a pixel at location (x, y), taking into account alpha channels
    #[deprecated(
        since = "0.24.0",
        note = "Use iterator `pixels_mut` to blend the pixels directly"
    )]
    fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel);

    /// Copies all of the pixels from another image into this image.
    ///
    /// The other image is copied with the top-left corner of the
    /// other image placed at (x, y).
    ///
    /// In order to copy only a piece of the other image, use [`GenericImageView::view`].
    ///
    /// You can use [`FlatSamples`] to source pixels from an arbitrary regular raster of channel
    /// values, for example from a foreign interface or a fixed image.
    ///
    /// # Returns
    /// Returns an error if the image is too large to be copied at the given position
    ///
    /// [`GenericImageView::view`]: trait.GenericImageView.html#method.view
    /// [`FlatSamples`]: flat/struct.FlatSamples.html
    fn copy_from<O>(&mut self, other: &O, x: u32, y: u32) -> ImageResult<()>
    where
        O: GenericImageView<Pixel = Self::Pixel>,
    {
        // Do bounds checking here so we can use the non-bounds-checking
        // functions to copy pixels.
        if self.width() < other.width() + x || self.height() < other.height() + y {
            return Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::DimensionMismatch,
            )));
        }

        for k in 0..other.height() {
            for i in 0..other.width() {
                let p = other.get_pixel(i, k);
                self.put_pixel(i + x, k + y, p);
            }
        }
        Ok(())
    }

    /// Copies all of the pixels from one part of this image to another part of this image.
    ///
    /// The destination rectangle of the copy is specified with the top-left corner placed at (x, y).
    ///
    /// # Returns
    /// `true` if the copy was successful, `false` if the image could not
    /// be copied due to size constraints.
    fn copy_within(&mut self, source: Rect, x: u32, y: u32) -> bool {
        let Rect {
            x: sx,
            y: sy,
            width,
            height,
        } = source;
        let dx = x;
        let dy = y;
        assert!(sx < self.width() && dx < self.width());
        assert!(sy < self.height() && dy < self.height());
        if self.width() - dx.max(sx) < width || self.height() - dy.max(sy) < height {
            return false;
        }
        // since `.rev()` creates a new dype we would either have to go with dynamic dispatch for the ranges
        // or have quite a lot of code bloat. A macro gives us static dispatch with less visible bloat.
        macro_rules! copy_within_impl_ {
            ($xiter:expr, $yiter:expr) => {
                for y in $yiter {
                    let sy = sy + y;
                    let dy = dy + y;
                    for x in $xiter {
                        let sx = sx + x;
                        let dx = dx + x;
                        let pixel = self.get_pixel(sx, sy);
                        self.put_pixel(dx, dy, pixel);
                    }
                }
            };
        }
        // check how target and source rectangles relate to each other so we dont overwrite data before we copied it.
        match (sx < dx, sy < dy) {
            (true, true) => copy_within_impl_!((0..width).rev(), (0..height).rev()),
            (true, false) => copy_within_impl_!((0..width).rev(), 0..height),
            (false, true) => copy_within_impl_!(0..width, (0..height).rev()),
            (false, false) => copy_within_impl_!(0..width, 0..height),
        }
        true
    }

    /// Returns a mutable subimage that is a view into this image.
    /// If you want an immutable subimage instead, use [`GenericImageView::view`]
    /// The coordinates set the position of the top left corner of the `SubImage`.
    fn sub_image(&mut self, x: u32, y: u32, width: u32, height: u32) -> SubImage<&mut Self>
    where
        Self: Sized,
    {
        assert!(u64::from(x) + u64::from(width) <= u64::from(self.width()));
        assert!(u64::from(y) + u64::from(height) <= u64::from(self.height()));
        SubImage::new(self, x, y, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::{GenericImage, GenericImageView};

    use crate::color::Rgba;
    use crate::math::Rect;
    use crate::{GrayImage, ImageBuffer};

    #[test]
    #[allow(deprecated)]
    /// Test that alpha blending works as expected
    fn test_image_alpha_blending() {
        let mut target = ImageBuffer::new(1, 1);
        target.put_pixel(0, 0, Rgba([255u8, 0, 0, 255]));
        assert!(*target.get_pixel(0, 0) == Rgba([255, 0, 0, 255]));
        target.blend_pixel(0, 0, Rgba([0, 255, 0, 255]));
        assert!(*target.get_pixel(0, 0) == Rgba([0, 255, 0, 255]));

        // Blending an alpha channel onto a solid background
        target.blend_pixel(0, 0, Rgba([255, 0, 0, 127]));
        assert!(*target.get_pixel(0, 0) == Rgba([127, 127, 0, 255]));

        // Blending two alpha channels
        target.put_pixel(0, 0, Rgba([0, 255, 0, 127]));
        target.blend_pixel(0, 0, Rgba([255, 0, 0, 127]));
        assert!(*target.get_pixel(0, 0) == Rgba([169, 85, 0, 190]));
    }

    #[test]
    fn test_in_bounds() {
        let mut target = ImageBuffer::new(2, 2);
        target.put_pixel(0, 0, Rgba([255u8, 0, 0, 255]));

        assert!(target.in_bounds(0, 0));
        assert!(target.in_bounds(1, 0));
        assert!(target.in_bounds(0, 1));
        assert!(target.in_bounds(1, 1));

        assert!(!target.in_bounds(2, 0));
        assert!(!target.in_bounds(0, 2));
        assert!(!target.in_bounds(2, 2));
    }

    #[test]
    fn test_can_subimage_clone_nonmut() {
        let mut source = ImageBuffer::new(3, 3);
        source.put_pixel(1, 1, Rgba([255u8, 0, 0, 255]));

        // A non-mutable copy of the source image
        let source = source.clone();

        // Clone a view into non-mutable to a separate buffer
        let cloned = source.view(1, 1, 1, 1).to_image();

        assert!(cloned.get_pixel(0, 0) == source.get_pixel(1, 1));
    }

    #[test]
    fn test_can_nest_views() {
        let mut source = ImageBuffer::from_pixel(3, 3, Rgba([255u8, 0, 0, 255]));

        {
            let mut sub1 = source.sub_image(0, 0, 2, 2);
            let mut sub2 = sub1.sub_image(1, 1, 1, 1);
            sub2.put_pixel(0, 0, Rgba([0, 0, 0, 0]));
        }

        assert_eq!(*source.get_pixel(1, 1), Rgba([0, 0, 0, 0]));

        let view1 = source.view(0, 0, 2, 2);
        assert_eq!(*source.get_pixel(1, 1), view1.get_pixel(1, 1));

        let view2 = view1.view(1, 1, 1, 1);
        assert_eq!(*source.get_pixel(1, 1), view2.get_pixel(0, 0));
    }

    #[test]
    #[should_panic]
    fn test_view_out_of_bounds() {
        let source = ImageBuffer::from_pixel(3, 3, Rgba([255u8, 0, 0, 255]));
        source.view(1, 1, 3, 3);
    }

    #[test]
    #[should_panic]
    fn test_view_coordinates_out_of_bounds() {
        let source = ImageBuffer::from_pixel(3, 3, Rgba([255u8, 0, 0, 255]));
        source.view(3, 3, 3, 3);
    }

    #[test]
    #[should_panic]
    fn test_view_width_out_of_bounds() {
        let source = ImageBuffer::from_pixel(3, 3, Rgba([255u8, 0, 0, 255]));
        source.view(1, 1, 3, 2);
    }

    #[test]
    #[should_panic]
    fn test_view_height_out_of_bounds() {
        let source = ImageBuffer::from_pixel(3, 3, Rgba([255u8, 0, 0, 255]));
        source.view(1, 1, 2, 3);
    }

    #[test]
    #[should_panic]
    fn test_view_x_out_of_bounds() {
        let source = ImageBuffer::from_pixel(3, 3, Rgba([255u8, 0, 0, 255]));
        source.view(3, 1, 3, 3);
    }

    #[test]
    #[should_panic]
    fn test_view_y_out_of_bounds() {
        let source = ImageBuffer::from_pixel(3, 3, Rgba([255u8, 0, 0, 255]));
        source.view(1, 3, 3, 3);
    }

    #[test]
    fn test_view_in_bounds() {
        let source = ImageBuffer::from_pixel(3, 3, Rgba([255u8, 0, 0, 255]));
        source.view(0, 0, 3, 3);
        source.view(1, 1, 2, 2);
        source.view(2, 2, 0, 0);
    }

    #[test]
    fn test_copy_sub_image() {
        let source = ImageBuffer::from_pixel(3, 3, Rgba([255u8, 0, 0, 255]));
        let view = source.view(0, 0, 3, 3);
        let _view2 = view;
        view.to_image();
    }

    #[test]
    fn test_generic_image_copy_within_oob() {
        let mut image: GrayImage = ImageBuffer::from_raw(4, 4, vec![0u8; 16]).unwrap();
        assert!(!image.sub_image(0, 0, 4, 4).copy_within(
            Rect {
                x: 0,
                y: 0,
                width: 5,
                height: 4
            },
            0,
            0
        ));
        assert!(!image.sub_image(0, 0, 4, 4).copy_within(
            Rect {
                x: 0,
                y: 0,
                width: 4,
                height: 5
            },
            0,
            0
        ));
        assert!(!image.sub_image(0, 0, 4, 4).copy_within(
            Rect {
                x: 1,
                y: 0,
                width: 4,
                height: 4
            },
            0,
            0
        ));
        assert!(!image.sub_image(0, 0, 4, 4).copy_within(
            Rect {
                x: 0,
                y: 0,
                width: 4,
                height: 4
            },
            1,
            0
        ));
        assert!(!image.sub_image(0, 0, 4, 4).copy_within(
            Rect {
                x: 0,
                y: 1,
                width: 4,
                height: 4
            },
            0,
            0
        ));
        assert!(!image.sub_image(0, 0, 4, 4).copy_within(
            Rect {
                x: 0,
                y: 0,
                width: 4,
                height: 4
            },
            0,
            1
        ));
        assert!(!image.sub_image(0, 0, 4, 4).copy_within(
            Rect {
                x: 1,
                y: 1,
                width: 4,
                height: 4
            },
            0,
            0
        ));
    }

    #[test]
    fn test_generic_image_copy_within_tl() {
        let data = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let expected = [0, 1, 2, 3, 4, 0, 1, 2, 8, 4, 5, 6, 12, 8, 9, 10];
        let mut image: GrayImage = ImageBuffer::from_raw(4, 4, Vec::from(&data[..])).unwrap();
        assert!(image.sub_image(0, 0, 4, 4).copy_within(
            Rect {
                x: 0,
                y: 0,
                width: 3,
                height: 3
            },
            1,
            1
        ));
        assert_eq!(&image.into_raw(), &expected);
    }

    #[test]
    fn test_generic_image_copy_within_tr() {
        let data = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let expected = [0, 1, 2, 3, 1, 2, 3, 7, 5, 6, 7, 11, 9, 10, 11, 15];
        let mut image: GrayImage = ImageBuffer::from_raw(4, 4, Vec::from(&data[..])).unwrap();
        assert!(image.sub_image(0, 0, 4, 4).copy_within(
            Rect {
                x: 1,
                y: 0,
                width: 3,
                height: 3
            },
            0,
            1
        ));
        assert_eq!(&image.into_raw(), &expected);
    }

    #[test]
    fn test_generic_image_copy_within_bl() {
        let data = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let expected = [0, 4, 5, 6, 4, 8, 9, 10, 8, 12, 13, 14, 12, 13, 14, 15];
        let mut image: GrayImage = ImageBuffer::from_raw(4, 4, Vec::from(&data[..])).unwrap();
        assert!(image.sub_image(0, 0, 4, 4).copy_within(
            Rect {
                x: 0,
                y: 1,
                width: 3,
                height: 3
            },
            1,
            0
        ));
        assert_eq!(&image.into_raw(), &expected);
    }

    #[test]
    fn test_generic_image_copy_within_br() {
        let data = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let expected = [5, 6, 7, 3, 9, 10, 11, 7, 13, 14, 15, 11, 12, 13, 14, 15];
        let mut image: GrayImage = ImageBuffer::from_raw(4, 4, Vec::from(&data[..])).unwrap();
        assert!(image.sub_image(0, 0, 4, 4).copy_within(
            Rect {
                x: 1,
                y: 1,
                width: 3,
                height: 3
            },
            0,
            0
        ));
        assert_eq!(&image.into_raw(), &expected);
    }
}
