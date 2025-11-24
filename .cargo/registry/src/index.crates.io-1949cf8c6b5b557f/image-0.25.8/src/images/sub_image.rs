use crate::{GenericImage, GenericImageView, ImageBuffer, Pixel};
use std::ops::{Deref, DerefMut};

/// A View into another image
///
/// Instances of this struct can be created using:
///   - [`GenericImage::sub_image`] to create a mutable view,
///   - [`GenericImageView::view`] to create an immutable view,
///   - [`SubImage::new`] to instantiate the struct directly.
///
/// Note that this does _not_ implement `GenericImage`, but it dereferences to one which allows you
/// to use it as if it did. See [Design Considerations](#Design-Considerations) below for details.
///
/// # Design Considerations
///
/// For reasons relating to coherence, this is not itself a `GenericImage` or a `GenericImageView`.
/// In short, we want to reserve the ability of adding traits implemented for _all_ generic images
/// but in a different manner for `SubImage`. This may be required to ensure that stacking
/// sub-images comes at no double indirect cost.
///
/// If, ultimately, this is not needed then a directly implementation of `GenericImage` can and
/// will get added. This inconvenience may alternatively get resolved if Rust allows some forms of
/// specialization, which might make this trick unnecessary and thus also allows for a direct
/// implementation.
#[derive(Copy, Clone)]
pub struct SubImage<I> {
    inner: SubImageInner<I>,
}

/// The inner type of `SubImage` that implements `GenericImage{,View}`.
///
/// This type is _nominally_ `pub` but it is not exported from the crate. It should be regarded as
/// an existential type in any case.
#[derive(Copy, Clone)]
pub struct SubImageInner<I> {
    image: I,
    xoffset: u32,
    yoffset: u32,
    xstride: u32,
    ystride: u32,
}

/// Alias to access Pixel behind a reference
type DerefPixel<I> = <<I as Deref>::Target as GenericImageView>::Pixel;

/// Alias to access Subpixel behind a reference
type DerefSubpixel<I> = <DerefPixel<I> as Pixel>::Subpixel;

impl<I> SubImage<I> {
    /// Construct a new subimage
    /// The coordinates set the position of the top left corner of the `SubImage`.
    pub fn new(image: I, x: u32, y: u32, width: u32, height: u32) -> SubImage<I> {
        SubImage {
            inner: SubImageInner {
                image,
                xoffset: x,
                yoffset: y,
                xstride: width,
                ystride: height,
            },
        }
    }

    /// Change the coordinates of this subimage.
    pub fn change_bounds(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.inner.xoffset = x;
        self.inner.yoffset = y;
        self.inner.xstride = width;
        self.inner.ystride = height;
    }

    /// The offsets of this subimage relative to the underlying image.
    pub fn offsets(&self) -> (u32, u32) {
        (self.inner.xoffset, self.inner.yoffset)
    }

    /// Convert this subimage to an `ImageBuffer`
    pub fn to_image(&self) -> ImageBuffer<DerefPixel<I>, Vec<DerefSubpixel<I>>>
    where
        I: Deref,
        I::Target: GenericImageView + 'static,
    {
        let borrowed = &*self.inner.image;
        let mut out = borrowed.buffer_with_dimensions(self.inner.xstride, self.inner.ystride);

        for y in 0..self.inner.ystride {
            for x in 0..self.inner.xstride {
                let p = borrowed.get_pixel(x + self.inner.xoffset, y + self.inner.yoffset);
                out.put_pixel(x, y, p);
            }
        }

        out
    }
}

/// Methods for readable images.
impl<I> SubImage<I>
where
    I: Deref,
    I::Target: GenericImageView,
{
    /// Create a sub-view of the image.
    ///
    /// The coordinates given are relative to the current view on the underlying image.
    ///
    /// Note that this method is preferred to the one from `GenericImageView`. This is accessible
    /// with the explicit method call syntax but it should rarely be needed due to causing an
    /// extra level of indirection.
    ///
    /// ```
    /// use image::{GenericImageView, RgbImage, SubImage};
    /// let buffer = RgbImage::new(10, 10);
    ///
    /// let subimage: SubImage<&RgbImage> = buffer.view(0, 0, 10, 10);
    /// let subview: SubImage<&RgbImage> = subimage.view(0, 0, 10, 10);
    ///
    /// // Less efficient and NOT &RgbImage
    /// let _: SubImage<&_> = GenericImageView::view(&*subimage, 0, 0, 10, 10);
    /// ```
    pub fn view(&self, x: u32, y: u32, width: u32, height: u32) -> SubImage<&I::Target> {
        use crate::GenericImageView as _;
        assert!(u64::from(x) + u64::from(width) <= u64::from(self.inner.width()));
        assert!(u64::from(y) + u64::from(height) <= u64::from(self.inner.height()));
        let x = self.inner.xoffset.saturating_add(x);
        let y = self.inner.yoffset.saturating_add(y);
        SubImage::new(&*self.inner.image, x, y, width, height)
    }

    /// Get a reference to the underlying image.
    pub fn inner(&self) -> &I::Target {
        &self.inner.image
    }
}

impl<I> SubImage<I>
where
    I: DerefMut,
    I::Target: GenericImage,
{
    /// Create a mutable sub-view of the image.
    ///
    /// The coordinates given are relative to the current view on the underlying image.
    pub fn sub_image(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> SubImage<&mut I::Target> {
        assert!(u64::from(x) + u64::from(width) <= u64::from(self.inner.width()));
        assert!(u64::from(y) + u64::from(height) <= u64::from(self.inner.height()));
        let x = self.inner.xoffset.saturating_add(x);
        let y = self.inner.yoffset.saturating_add(y);
        SubImage::new(&mut *self.inner.image, x, y, width, height)
    }

    /// Get a mutable reference to the underlying image.
    pub fn inner_mut(&mut self) -> &mut I::Target {
        &mut self.inner.image
    }
}

impl<I> Deref for SubImage<I>
where
    I: Deref,
{
    type Target = SubImageInner<I>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<I> DerefMut for SubImage<I>
where
    I: DerefMut,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[allow(deprecated)]
impl<I> GenericImageView for SubImageInner<I>
where
    I: Deref,
    I::Target: GenericImageView,
{
    type Pixel = DerefPixel<I>;

    fn dimensions(&self) -> (u32, u32) {
        (self.xstride, self.ystride)
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        self.image.get_pixel(x + self.xoffset, y + self.yoffset)
    }

    /// Create a buffer with the (color) metadata of the underlying image.
    fn buffer_with_dimensions(
        &self,
        width: u32,
        height: u32,
    ) -> ImageBuffer<
        <I::Target as GenericImageView>::Pixel,
        Vec<<<I::Target as GenericImageView>::Pixel as Pixel>::Subpixel>,
    > {
        self.image.buffer_with_dimensions(width, height)
    }
}

#[allow(deprecated)]
impl<I> GenericImage for SubImageInner<I>
where
    I: DerefMut,
    I::Target: GenericImage + Sized,
{
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel {
        self.image.get_pixel_mut(x + self.xoffset, y + self.yoffset)
    }

    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        self.image
            .put_pixel(x + self.xoffset, y + self.yoffset, pixel);
    }

    /// DEPRECATED: This method will be removed. Blend the pixel directly instead.
    fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        self.image
            .blend_pixel(x + self.xoffset, y + self.yoffset, pixel);
    }
}

#[cfg(test)]
mod tests {
    use crate::{metadata::Cicp, GenericImageView, RgbaImage};

    #[test]
    fn preserves_color_space() {
        let mut buffer = RgbaImage::new(16, 16);
        buffer[(0, 0)] = crate::Rgba([0xff, 0, 0, 255]);
        buffer.set_rgb_primaries(Cicp::DISPLAY_P3.primaries);

        let view = buffer.view(0, 0, 16, 16);
        let result = view.buffer_like();

        assert_eq!(buffer.color_space(), result.color_space());
    }

    #[test]
    fn deep_preserves_color_space() {
        let mut buffer = RgbaImage::new(16, 16);
        buffer[(0, 0)] = crate::Rgba([0xff, 0, 0, 255]);
        buffer.set_rgb_primaries(Cicp::DISPLAY_P3.primaries);

        let view = buffer.view(0, 0, 16, 16);
        let view = view.view(0, 0, 16, 16);
        let result = view.buffer_like();

        assert_eq!(buffer.color_space(), result.color_space());
    }
}
