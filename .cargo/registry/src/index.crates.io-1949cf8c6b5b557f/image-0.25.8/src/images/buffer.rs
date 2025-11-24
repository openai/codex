//! Contains the generic `ImageBuffer` struct.
use num_traits::Zero;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Index, IndexMut, Range};
use std::path::Path;
use std::slice::{ChunksExact, ChunksExactMut};

use crate::color::{FromColor, FromPrimitive, Luma, LumaA, Rgb, Rgba};
use crate::error::{
    ImageResult, ParameterError, ParameterErrorKind, UnsupportedError, UnsupportedErrorKind,
};
use crate::flat::{FlatSamples, SampleLayout};
use crate::math::Rect;
use crate::metadata::cicp::{CicpApplicable, CicpPixelCast, CicpRgb, ColorComponentForCicp};
use crate::traits::{EncodableLayout, Pixel, PixelWithColorType};
use crate::utils::expand_packed;
use crate::{
    metadata::{Cicp, CicpColorPrimaries, CicpTransferCharacteristics, CicpTransform},
    save_buffer, save_buffer_with_format, write_buffer_with_format, ImageError,
};
use crate::{DynamicImage, GenericImage, GenericImageView, ImageEncoder, ImageFormat};

/// Iterate over pixel refs.
pub struct Pixels<'a, P: Pixel + 'a>
where
    P::Subpixel: 'a,
{
    chunks: ChunksExact<'a, P::Subpixel>,
}

impl<'a, P: Pixel + 'a> Iterator for Pixels<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = &'a P;

    #[inline(always)]
    fn next(&mut self) -> Option<&'a P> {
        self.chunks.next().map(|v| <P as Pixel>::from_slice(v))
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for Pixels<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.chunks.len()
    }
}

impl<'a, P: Pixel + 'a> DoubleEndedIterator for Pixels<'a, P>
where
    P::Subpixel: 'a,
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<&'a P> {
        self.chunks.next_back().map(|v| <P as Pixel>::from_slice(v))
    }
}

impl<P: Pixel> Clone for Pixels<'_, P> {
    fn clone(&self) -> Self {
        Pixels {
            chunks: self.chunks.clone(),
        }
    }
}

impl<P: Pixel> fmt::Debug for Pixels<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Pixels")
            .field("chunks", &self.chunks)
            .finish()
    }
}

/// Iterate over mutable pixel refs.
pub struct PixelsMut<'a, P: Pixel + 'a>
where
    P::Subpixel: 'a,
{
    chunks: ChunksExactMut<'a, P::Subpixel>,
}

impl<'a, P: Pixel + 'a> Iterator for PixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = &'a mut P;

    #[inline(always)]
    fn next(&mut self) -> Option<&'a mut P> {
        self.chunks.next().map(|v| <P as Pixel>::from_slice_mut(v))
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for PixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.chunks.len()
    }
}

impl<'a, P: Pixel + 'a> DoubleEndedIterator for PixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<&'a mut P> {
        self.chunks
            .next_back()
            .map(|v| <P as Pixel>::from_slice_mut(v))
    }
}

impl<P: Pixel> fmt::Debug for PixelsMut<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PixelsMut")
            .field("chunks", &self.chunks)
            .finish()
    }
}

/// Iterate over rows of an image
///
/// This iterator is created with [`ImageBuffer::rows`]. See its document for details.
///
/// [`ImageBuffer::rows`]: ../struct.ImageBuffer.html#method.rows
pub struct Rows<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    pixels: ChunksExact<'a, P::Subpixel>,
}

impl<'a, P: Pixel + 'a> Rows<'a, P> {
    /// Construct the iterator from image pixels. This is not public since it has a (hidden) panic
    /// condition. The `pixels` slice must be large enough so that all pixels are addressable.
    fn with_image(pixels: &'a [P::Subpixel], width: u32, height: u32) -> Self {
        let row_len = (width as usize) * usize::from(<P as Pixel>::CHANNEL_COUNT);
        if row_len == 0 {
            Rows {
                pixels: [].chunks_exact(1),
            }
        } else {
            let pixels = pixels
                .get(..row_len * height as usize)
                .expect("Pixel buffer has too few subpixels");
            // Rows are physically present. In particular, height is smaller than `usize::MAX` as
            // all subpixels can be indexed.
            Rows {
                pixels: pixels.chunks_exact(row_len),
            }
        }
    }
}

impl<'a, P: Pixel + 'a> Iterator for Rows<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = Pixels<'a, P>;

    #[inline(always)]
    fn next(&mut self) -> Option<Pixels<'a, P>> {
        let row = self.pixels.next()?;
        Some(Pixels {
            // Note: this is not reached when CHANNEL_COUNT is 0.
            chunks: row.chunks_exact(<P as Pixel>::CHANNEL_COUNT as usize),
        })
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for Rows<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.pixels.len()
    }
}

impl<'a, P: Pixel + 'a> DoubleEndedIterator for Rows<'a, P>
where
    P::Subpixel: 'a,
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<Pixels<'a, P>> {
        let row = self.pixels.next_back()?;
        Some(Pixels {
            // Note: this is not reached when CHANNEL_COUNT is 0.
            chunks: row.chunks_exact(<P as Pixel>::CHANNEL_COUNT as usize),
        })
    }
}

impl<P: Pixel> Clone for Rows<'_, P> {
    fn clone(&self) -> Self {
        Rows {
            pixels: self.pixels.clone(),
        }
    }
}

impl<P: Pixel> fmt::Debug for Rows<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Rows")
            .field("pixels", &self.pixels)
            .finish()
    }
}

/// Iterate over mutable rows of an image
///
/// This iterator is created with [`ImageBuffer::rows_mut`]. See its document for details.
///
/// [`ImageBuffer::rows_mut`]: ../struct.ImageBuffer.html#method.rows_mut
pub struct RowsMut<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    pixels: ChunksExactMut<'a, P::Subpixel>,
}

impl<'a, P: Pixel + 'a> RowsMut<'a, P> {
    /// Construct the iterator from image pixels. This is not public since it has a (hidden) panic
    /// condition. The `pixels` slice must be large enough so that all pixels are addressable.
    fn with_image(pixels: &'a mut [P::Subpixel], width: u32, height: u32) -> Self {
        let row_len = (width as usize) * usize::from(<P as Pixel>::CHANNEL_COUNT);
        if row_len == 0 {
            RowsMut {
                pixels: [].chunks_exact_mut(1),
            }
        } else {
            let pixels = pixels
                .get_mut(..row_len * height as usize)
                .expect("Pixel buffer has too few subpixels");
            // Rows are physically present. In particular, height is smaller than `usize::MAX` as
            // all subpixels can be indexed.
            RowsMut {
                pixels: pixels.chunks_exact_mut(row_len),
            }
        }
    }
}

impl<'a, P: Pixel + 'a> Iterator for RowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = PixelsMut<'a, P>;

    #[inline(always)]
    fn next(&mut self) -> Option<PixelsMut<'a, P>> {
        let row = self.pixels.next()?;
        Some(PixelsMut {
            // Note: this is not reached when CHANNEL_COUNT is 0.
            chunks: row.chunks_exact_mut(<P as Pixel>::CHANNEL_COUNT as usize),
        })
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for RowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.pixels.len()
    }
}

impl<'a, P: Pixel + 'a> DoubleEndedIterator for RowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<PixelsMut<'a, P>> {
        let row = self.pixels.next_back()?;
        Some(PixelsMut {
            // Note: this is not reached when CHANNEL_COUNT is 0.
            chunks: row.chunks_exact_mut(<P as Pixel>::CHANNEL_COUNT as usize),
        })
    }
}

impl<P: Pixel> fmt::Debug for RowsMut<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RowsMut")
            .field("pixels", &self.pixels)
            .finish()
    }
}

/// Enumerate the pixels of an image.
pub struct EnumeratePixels<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    pixels: Pixels<'a, P>,
    x: u32,
    y: u32,
    width: u32,
}

impl<'a, P: Pixel + 'a> Iterator for EnumeratePixels<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = (u32, u32, &'a P);

    #[inline(always)]
    fn next(&mut self) -> Option<(u32, u32, &'a P)> {
        if self.x >= self.width {
            self.x = 0;
            self.y += 1;
        }
        let (x, y) = (self.x, self.y);
        self.x += 1;
        self.pixels.next().map(|p| (x, y, p))
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for EnumeratePixels<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.pixels.len()
    }
}

impl<P: Pixel> Clone for EnumeratePixels<'_, P> {
    fn clone(&self) -> Self {
        EnumeratePixels {
            pixels: self.pixels.clone(),
            ..*self
        }
    }
}

impl<P: Pixel> fmt::Debug for EnumeratePixels<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EnumeratePixels")
            .field("pixels", &self.pixels)
            .field("x", &self.x)
            .field("y", &self.y)
            .field("width", &self.width)
            .finish()
    }
}

/// Enumerate the rows of an image.
pub struct EnumerateRows<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    rows: Rows<'a, P>,
    y: u32,
    width: u32,
}

impl<'a, P: Pixel + 'a> Iterator for EnumerateRows<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = (u32, EnumeratePixels<'a, P>);

    #[inline(always)]
    fn next(&mut self) -> Option<(u32, EnumeratePixels<'a, P>)> {
        let y = self.y;
        self.y += 1;
        self.rows.next().map(|r| {
            (
                y,
                EnumeratePixels {
                    x: 0,
                    y,
                    width: self.width,
                    pixels: r,
                },
            )
        })
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for EnumerateRows<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.rows.len()
    }
}

impl<P: Pixel> Clone for EnumerateRows<'_, P> {
    fn clone(&self) -> Self {
        EnumerateRows {
            rows: self.rows.clone(),
            ..*self
        }
    }
}

impl<P: Pixel> fmt::Debug for EnumerateRows<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EnumerateRows")
            .field("rows", &self.rows)
            .field("y", &self.y)
            .field("width", &self.width)
            .finish()
    }
}

/// Enumerate the pixels of an image.
pub struct EnumeratePixelsMut<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    pixels: PixelsMut<'a, P>,
    x: u32,
    y: u32,
    width: u32,
}

impl<'a, P: Pixel + 'a> Iterator for EnumeratePixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = (u32, u32, &'a mut P);

    #[inline(always)]
    fn next(&mut self) -> Option<(u32, u32, &'a mut P)> {
        if self.x >= self.width {
            self.x = 0;
            self.y += 1;
        }
        let (x, y) = (self.x, self.y);
        self.x += 1;
        self.pixels.next().map(|p| (x, y, p))
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for EnumeratePixelsMut<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.pixels.len()
    }
}

impl<P: Pixel> fmt::Debug for EnumeratePixelsMut<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EnumeratePixelsMut")
            .field("pixels", &self.pixels)
            .field("x", &self.x)
            .field("y", &self.y)
            .field("width", &self.width)
            .finish()
    }
}

/// Enumerate the rows of an image.
pub struct EnumerateRowsMut<'a, P: Pixel + 'a>
where
    <P as Pixel>::Subpixel: 'a,
{
    rows: RowsMut<'a, P>,
    y: u32,
    width: u32,
}

impl<'a, P: Pixel + 'a> Iterator for EnumerateRowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    type Item = (u32, EnumeratePixelsMut<'a, P>);

    #[inline(always)]
    fn next(&mut self) -> Option<(u32, EnumeratePixelsMut<'a, P>)> {
        let y = self.y;
        self.y += 1;
        self.rows.next().map(|r| {
            (
                y,
                EnumeratePixelsMut {
                    x: 0,
                    y,
                    width: self.width,
                    pixels: r,
                },
            )
        })
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a, P: Pixel + 'a> ExactSizeIterator for EnumerateRowsMut<'a, P>
where
    P::Subpixel: 'a,
{
    fn len(&self) -> usize {
        self.rows.len()
    }
}

impl<P: Pixel> fmt::Debug for EnumerateRowsMut<'_, P>
where
    P::Subpixel: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EnumerateRowsMut")
            .field("rows", &self.rows)
            .field("y", &self.y)
            .field("width", &self.width)
            .finish()
    }
}

/// Generic image buffer
///
/// This is an image parameterised by its Pixel types, represented by a width and height and a
/// container of channel data. It provides direct access to its pixels and implements the
/// [`GenericImageView`] and [`GenericImage`] traits. In many ways, this is the standard buffer
/// implementing those traits. Using this concrete type instead of a generic type parameter has
/// been shown to improve performance.
///
/// The crate defines a few type aliases with regularly used pixel types for your convenience, such
/// as [`RgbImage`], [`GrayImage`] etc.
///
/// [`GenericImage`]: trait.GenericImage.html
/// [`GenericImageView`]: trait.GenericImageView.html
/// [`RgbImage`]: type.RgbImage.html
/// [`GrayImage`]: type.GrayImage.html
///
/// To convert between images of different Pixel types use [`DynamicImage`].
///
/// You can retrieve a complete description of the buffer's layout and contents through
/// [`as_flat_samples`] and [`as_flat_samples_mut`]. This can be handy to also use the contents in
/// a foreign language, map it as a GPU host buffer or other similar tasks.
///
/// [`DynamicImage`]: enum.DynamicImage.html
/// [`as_flat_samples`]: #method.as_flat_samples
/// [`as_flat_samples_mut`]: #method.as_flat_samples_mut
///
/// ## Examples
///
/// Create a simple canvas and paint a small cross.
///
/// ```
/// use image::{RgbImage, Rgb};
///
/// let mut img = RgbImage::new(32, 32);
///
/// for x in 15..=17 {
///     for y in 8..24 {
///         img.put_pixel(x, y, Rgb([255, 0, 0]));
///         img.put_pixel(y, x, Rgb([255, 0, 0]));
///     }
/// }
/// ```
///
/// Overlays an image on top of a larger background raster.
///
/// ```no_run
/// use image::{GenericImage, GenericImageView, ImageBuffer, open};
///
/// let on_top = open("path/to/some.png").unwrap().into_rgb8();
/// let mut img = ImageBuffer::from_fn(512, 512, |x, y| {
///     if (x + y) % 2 == 0 {
///         image::Rgb([0, 0, 0])
///     } else {
///         image::Rgb([255, 255, 255])
///     }
/// });
///
/// image::imageops::overlay(&mut img, &on_top, 128, 128);
/// ```
///
/// Convert an `RgbaImage` to a `GrayImage`.
///
/// ```no_run
/// use image::{open, DynamicImage};
///
/// let rgba = open("path/to/some.png").unwrap().into_rgba8();
/// let gray = DynamicImage::ImageRgba8(rgba).into_luma8();
/// ```
#[derive(Debug, Hash, PartialEq, Eq)]
pub struct ImageBuffer<P: Pixel, Container> {
    width: u32,
    height: u32,
    _phantom: PhantomData<P>,
    color: CicpRgb,
    data: Container,
}

// generic implementation, shared along all image buffers
impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]>,
{
    /// Constructs a buffer from a generic container
    /// (for example a `Vec` or a slice)
    ///
    /// Returns `None` if the container is not big enough (including when the image dimensions
    /// necessitate an allocation of more bytes than supported by the container).
    pub fn from_raw(width: u32, height: u32, buf: Container) -> Option<ImageBuffer<P, Container>> {
        if Self::check_image_fits(width, height, buf.len()) {
            Some(ImageBuffer {
                data: buf,
                width,
                height,
                color: Cicp::SRGB.into_rgb(),
                _phantom: PhantomData,
            })
        } else {
            None
        }
    }

    /// Returns the underlying raw buffer
    pub fn into_raw(self) -> Container {
        self.data
    }

    /// Returns the underlying raw buffer
    pub fn as_raw(&self) -> &Container {
        &self.data
    }

    /// The width and height of this image.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The width of this image.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The height of this image.
    pub fn height(&self) -> u32 {
        self.height
    }

    // TODO: choose name under which to expose.
    pub(crate) fn inner_pixels(&self) -> &[P::Subpixel] {
        let len = Self::image_buffer_len(self.width, self.height).unwrap();
        &self.data[..len]
    }

    /// Returns an iterator over the pixels of this image.
    /// The iteration order is x = 0 to width then y = 0 to height
    pub fn pixels(&self) -> Pixels<'_, P> {
        Pixels {
            chunks: self
                .inner_pixels()
                .chunks_exact(<P as Pixel>::CHANNEL_COUNT as usize),
        }
    }

    /// Returns an iterator over the rows of this image.
    ///
    /// Only non-empty rows can be iterated in this manner. In particular the iterator will not
    /// yield any item when the width of the image is `0` or a pixel type without any channels is
    /// used. This ensures that its length can always be represented by `usize`.
    pub fn rows(&self) -> Rows<'_, P> {
        Rows::with_image(&self.data, self.width, self.height)
    }

    /// Enumerates over the pixels of the image.
    /// The iterator yields the coordinates of each pixel
    /// along with a reference to them.
    /// The iteration order is x = 0 to width then y = 0 to height
    /// Starting from the top left.
    pub fn enumerate_pixels(&self) -> EnumeratePixels<'_, P> {
        EnumeratePixels {
            pixels: self.pixels(),
            x: 0,
            y: 0,
            width: self.width,
        }
    }

    /// Enumerates over the rows of the image.
    /// The iterator yields the y-coordinate of each row
    /// along with a reference to them.
    pub fn enumerate_rows(&self) -> EnumerateRows<'_, P> {
        EnumerateRows {
            rows: self.rows(),
            y: 0,
            width: self.width,
        }
    }

    /// Gets a reference to the pixel at location `(x, y)`
    ///
    /// # Panics
    ///
    /// Panics if `(x, y)` is out of the bounds `(width, height)`.
    #[inline]
    #[track_caller]
    pub fn get_pixel(&self, x: u32, y: u32) -> &P {
        match self.pixel_indices(x, y) {
            None => panic!(
                "Image index {:?} out of bounds {:?}",
                (x, y),
                (self.width, self.height)
            ),
            Some(pixel_indices) => <P as Pixel>::from_slice(&self.data[pixel_indices]),
        }
    }

    /// Gets a reference to the pixel at location `(x, y)` or returns `None` if
    /// the index is out of the bounds `(width, height)`.
    pub fn get_pixel_checked(&self, x: u32, y: u32) -> Option<&P> {
        if x >= self.width {
            return None;
        }
        let num_channels = <P as Pixel>::CHANNEL_COUNT as usize;
        let i = (y as usize)
            .saturating_mul(self.width as usize)
            .saturating_add(x as usize)
            .saturating_mul(num_channels);

        self.data
            .get(i..i.checked_add(num_channels)?)
            .map(|pixel_indices| <P as Pixel>::from_slice(pixel_indices))
    }

    /// Test that the image fits inside the buffer.
    ///
    /// Verifies that the maximum image of pixels inside the bounds is smaller than the provided
    /// length. Note that as a corrolary we also have that the index calculation of pixels inside
    /// the bounds will not overflow.
    fn check_image_fits(width: u32, height: u32, len: usize) -> bool {
        let checked_len = Self::image_buffer_len(width, height);
        checked_len.is_some_and(|min_len| min_len <= len)
    }

    fn image_buffer_len(width: u32, height: u32) -> Option<usize> {
        Some(<P as Pixel>::CHANNEL_COUNT as usize)
            .and_then(|size| size.checked_mul(width as usize))
            .and_then(|size| size.checked_mul(height as usize))
    }

    #[inline(always)]
    fn pixel_indices(&self, x: u32, y: u32) -> Option<Range<usize>> {
        if x >= self.width || y >= self.height {
            return None;
        }

        Some(self.pixel_indices_unchecked(x, y))
    }

    #[inline(always)]
    fn pixel_indices_unchecked(&self, x: u32, y: u32) -> Range<usize> {
        let no_channels = <P as Pixel>::CHANNEL_COUNT as usize;
        // If in bounds, this can't overflow as we have tested that at construction!
        let min_index = (y as usize * self.width as usize + x as usize) * no_channels;
        min_index..min_index + no_channels
    }

    /// Get the format of the buffer when viewed as a matrix of samples.
    pub fn sample_layout(&self) -> SampleLayout {
        // None of these can overflow, as all our memory is addressable.
        SampleLayout::row_major_packed(<P as Pixel>::CHANNEL_COUNT, self.width, self.height)
    }

    /// Return the raw sample buffer with its stride an dimension information.
    ///
    /// The returned buffer is guaranteed to be well formed in all cases. It is laid out by
    /// colors, width then height, meaning `channel_stride <= width_stride <= height_stride`. All
    /// strides are in numbers of elements but those are mostly `u8` in which case the strides are
    /// also byte strides.
    pub fn into_flat_samples(self) -> FlatSamples<Container>
    where
        Container: AsRef<[P::Subpixel]>,
    {
        // None of these can overflow, as all our memory is addressable.
        let layout = self.sample_layout();
        FlatSamples {
            samples: self.data,
            layout,
            color_hint: None, // TODO: the pixel type might contain P::COLOR_TYPE if it satisfies PixelWithColorType
        }
    }

    /// Return a view on the raw sample buffer.
    ///
    /// See [`into_flat_samples`](#method.into_flat_samples) for more details.
    pub fn as_flat_samples(&self) -> FlatSamples<&[P::Subpixel]>
    where
        Container: AsRef<[P::Subpixel]>,
    {
        let layout = self.sample_layout();
        FlatSamples {
            samples: self.data.as_ref(),
            layout,
            color_hint: None, // TODO: the pixel type might contain P::COLOR_TYPE if it satisfies PixelWithColorType
        }
    }

    /// Return a mutable view on the raw sample buffer.
    ///
    /// See [`into_flat_samples`](#method.into_flat_samples) for more details.
    pub fn as_flat_samples_mut(&mut self) -> FlatSamples<&mut [P::Subpixel]>
    where
        Container: AsMut<[P::Subpixel]>,
    {
        let layout = self.sample_layout();
        FlatSamples {
            samples: self.data.as_mut(),
            layout,
            color_hint: None, // TODO: the pixel type might contain P::COLOR_TYPE if it satisfies PixelWithColorType
        }
    }
}

impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    // TODO: choose name under which to expose.
    pub(crate) fn inner_pixels_mut(&mut self) -> &mut [P::Subpixel] {
        let len = Self::image_buffer_len(self.width, self.height).unwrap();
        &mut self.data[..len]
    }

    /// Returns an iterator over the mutable pixels of this image.
    pub fn pixels_mut(&mut self) -> PixelsMut<'_, P> {
        PixelsMut {
            chunks: self
                .inner_pixels_mut()
                .chunks_exact_mut(<P as Pixel>::CHANNEL_COUNT as usize),
        }
    }

    /// Returns an iterator over the mutable rows of this image.
    ///
    /// Only non-empty rows can be iterated in this manner. In particular the iterator will not
    /// yield any item when the width of the image is `0` or a pixel type without any channels is
    /// used. This ensures that its length can always be represented by `usize`.
    pub fn rows_mut(&mut self) -> RowsMut<'_, P> {
        RowsMut::with_image(&mut self.data, self.width, self.height)
    }

    /// Enumerates over the pixels of the image.
    /// The iterator yields the coordinates of each pixel
    /// along with a mutable reference to them.
    pub fn enumerate_pixels_mut(&mut self) -> EnumeratePixelsMut<'_, P> {
        let width = self.width;
        EnumeratePixelsMut {
            pixels: self.pixels_mut(),
            x: 0,
            y: 0,
            width,
        }
    }

    /// Enumerates over the rows of the image.
    /// The iterator yields the y-coordinate of each row
    /// along with a mutable reference to them.
    pub fn enumerate_rows_mut(&mut self) -> EnumerateRowsMut<'_, P> {
        let width = self.width;
        EnumerateRowsMut {
            rows: self.rows_mut(),
            y: 0,
            width,
        }
    }

    /// Gets a reference to the mutable pixel at location `(x, y)`
    ///
    /// # Panics
    ///
    /// Panics if `(x, y)` is out of the bounds `(width, height)`.
    #[inline]
    #[track_caller]
    pub fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut P {
        match self.pixel_indices(x, y) {
            None => panic!(
                "Image index {:?} out of bounds {:?}",
                (x, y),
                (self.width, self.height)
            ),
            Some(pixel_indices) => <P as Pixel>::from_slice_mut(&mut self.data[pixel_indices]),
        }
    }

    /// Gets a reference to the mutable pixel at location `(x, y)` or returns
    /// `None` if the index is out of the bounds `(width, height)`.
    pub fn get_pixel_mut_checked(&mut self, x: u32, y: u32) -> Option<&mut P> {
        if x >= self.width {
            return None;
        }
        let num_channels = <P as Pixel>::CHANNEL_COUNT as usize;
        let i = (y as usize)
            .saturating_mul(self.width as usize)
            .saturating_add(x as usize)
            .saturating_mul(num_channels);

        self.data
            .get_mut(i..i.checked_add(num_channels)?)
            .map(|pixel_indices| <P as Pixel>::from_slice_mut(pixel_indices))
    }

    /// Puts a pixel at location `(x, y)`
    ///
    /// # Panics
    ///
    /// Panics if `(x, y)` is out of the bounds `(width, height)`.
    #[inline]
    #[track_caller]
    pub fn put_pixel(&mut self, x: u32, y: u32, pixel: P) {
        *self.get_pixel_mut(x, y) = pixel;
    }
}

impl<P: Pixel, Container> ImageBuffer<P, Container> {
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
    ///
    /// The default color space is [`Cicp::SRGB`].
    pub fn set_rgb_primaries(&mut self, color: CicpColorPrimaries) {
        self.color.primaries = color;
    }

    /// Define the transfer function for the image.
    ///
    /// The color data is unchanged. Reinterprets all (non-alpha) components in the image,
    /// potentially changing the apparent shade of pixels. Individual components are always
    /// interpreted as encoded numbers. To denote numbers in a linear RGB space, use
    /// [`CicpTransferCharacteristics::Linear`].
    ///
    /// The default color space is [`Cicp::SRGB`].
    pub fn set_transfer_function(&mut self, tf: CicpTransferCharacteristics) {
        self.color.transfer = tf;
    }

    /// Get the Cicp encoding of this buffer's color data.
    pub fn color_space(&self) -> Cicp {
        self.color.into()
    }

    /// Set primaries and transfer characteristics from a Cicp color space.
    ///
    /// Returns an error if `cicp` uses features that are not support with an RGB color space, e.g.
    /// a matrix or narrow range (studio encoding) channels.
    pub fn set_color_space(&mut self, cicp: Cicp) -> ImageResult<()> {
        self.color = cicp.try_into_rgb()?;
        Ok(())
    }

    pub(crate) fn set_rgb_color_space(&mut self, color: CicpRgb) {
        self.color = color;
    }
}

impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    [P::Subpixel]: EncodableLayout,
    Container: Deref<Target = [P::Subpixel]>,
{
    /// Saves the buffer to a file at the path specified.
    ///
    /// The image format is derived from the file extension.
    pub fn save<Q>(&self, path: Q) -> ImageResult<()>
    where
        Q: AsRef<Path>,
        P: PixelWithColorType,
    {
        save_buffer(
            path,
            self.inner_pixels().as_bytes(),
            self.width(),
            self.height(),
            <P as PixelWithColorType>::COLOR_TYPE,
        )
    }
}

impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    [P::Subpixel]: EncodableLayout,
    Container: Deref<Target = [P::Subpixel]>,
{
    /// Saves the buffer to a file at the specified path in
    /// the specified format.
    ///
    /// See [`save_buffer_with_format`](fn.save_buffer_with_format.html) for
    /// supported types.
    pub fn save_with_format<Q>(&self, path: Q, format: ImageFormat) -> ImageResult<()>
    where
        Q: AsRef<Path>,
        P: PixelWithColorType,
    {
        // This is valid as the subpixel is u8.
        save_buffer_with_format(
            path,
            self.inner_pixels().as_bytes(),
            self.width(),
            self.height(),
            <P as PixelWithColorType>::COLOR_TYPE,
            format,
        )
    }
}

impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    [P::Subpixel]: EncodableLayout,
    Container: Deref<Target = [P::Subpixel]>,
{
    /// Writes the buffer to a writer in the specified format.
    ///
    /// Assumes the writer is buffered. In most cases, you should wrap your writer in a `BufWriter`
    /// for best performance.
    pub fn write_to<W>(&self, writer: &mut W, format: ImageFormat) -> ImageResult<()>
    where
        W: std::io::Write + std::io::Seek,
        P: PixelWithColorType,
    {
        // This is valid as the subpixel is u8.
        write_buffer_with_format(
            writer,
            self.inner_pixels().as_bytes(),
            self.width(),
            self.height(),
            <P as PixelWithColorType>::COLOR_TYPE,
            format,
        )
    }
}

impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel,
    [P::Subpixel]: EncodableLayout,
    Container: Deref<Target = [P::Subpixel]>,
{
    /// Writes the buffer with the given encoder.
    pub fn write_with_encoder<E>(&self, encoder: E) -> ImageResult<()>
    where
        E: ImageEncoder,
        P: PixelWithColorType,
    {
        // This is valid as the subpixel is u8.
        encoder.write_image(
            self.inner_pixels().as_bytes(),
            self.width(),
            self.height(),
            <P as PixelWithColorType>::COLOR_TYPE,
        )
    }
}

impl<P, Container> Default for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Default,
{
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            _phantom: PhantomData,
            color: Cicp::SRGB_LINEAR.into_rgb(),
            data: Default::default(),
        }
    }
}

impl<P, Container> Deref for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]>,
{
    type Target = [P::Subpixel];

    fn deref(&self) -> &<Self as Deref>::Target {
        &self.data
    }
}

impl<P, Container> DerefMut for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    fn deref_mut(&mut self) -> &mut <Self as Deref>::Target {
        &mut self.data
    }
}

impl<P, Container> Index<(u32, u32)> for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]>,
{
    type Output = P;

    fn index(&self, (x, y): (u32, u32)) -> &P {
        self.get_pixel(x, y)
    }
}

impl<P, Container> IndexMut<(u32, u32)> for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    fn index_mut(&mut self, (x, y): (u32, u32)) -> &mut P {
        self.get_pixel_mut(x, y)
    }
}

impl<P, Container> Clone for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + Clone,
{
    fn clone(&self) -> ImageBuffer<P, Container> {
        ImageBuffer {
            data: self.data.clone(),
            width: self.width,
            height: self.height,
            color: self.color,
            _phantom: PhantomData,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.data.clone_from(&source.data);
        self.width = source.width;
        self.height = source.height;
        self.color = source.color;
    }
}

impl<P, Container> GenericImageView for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + Deref,
{
    type Pixel = P;

    fn dimensions(&self) -> (u32, u32) {
        self.dimensions()
    }

    fn get_pixel(&self, x: u32, y: u32) -> P {
        *self.get_pixel(x, y)
    }

    /// Returns the pixel located at (x, y), ignoring bounds checking.
    #[inline(always)]
    unsafe fn unsafe_get_pixel(&self, x: u32, y: u32) -> P {
        let indices = self.pixel_indices_unchecked(x, y);
        *<P as Pixel>::from_slice(self.data.get_unchecked(indices))
    }

    fn buffer_with_dimensions(&self, width: u32, height: u32) -> ImageBuffer<P, Vec<P::Subpixel>> {
        let mut buffer = ImageBuffer::new(width, height);
        buffer.copy_color_space_from(self);
        buffer
    }
}

impl<P, Container> GenericImage for ImageBuffer<P, Container>
where
    P: Pixel,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut P {
        self.get_pixel_mut(x, y)
    }

    fn put_pixel(&mut self, x: u32, y: u32, pixel: P) {
        *self.get_pixel_mut(x, y) = pixel;
    }

    /// Puts a pixel at location (x, y), ignoring bounds checking.
    #[inline(always)]
    unsafe fn unsafe_put_pixel(&mut self, x: u32, y: u32, pixel: P) {
        let indices = self.pixel_indices_unchecked(x, y);
        let p = <P as Pixel>::from_slice_mut(self.data.get_unchecked_mut(indices));
        *p = pixel;
    }

    /// Put a pixel at location (x, y), taking into account alpha channels
    ///
    /// DEPRECATED: This method will be removed. Blend the pixel directly instead.
    fn blend_pixel(&mut self, x: u32, y: u32, p: P) {
        self.get_pixel_mut(x, y).blend(&p);
    }

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

        if sy < dy {
            for y in (0..height).rev() {
                let sy = sy + y;
                let dy = dy + y;
                let Range { start, .. } = self.pixel_indices_unchecked(sx, sy);
                let Range { end, .. } = self.pixel_indices_unchecked(sx + width - 1, sy);
                let dst = self.pixel_indices_unchecked(dx, dy).start;
                self.data.copy_within(start..end, dst);
            }
        } else {
            for y in 0..height {
                let sy = sy + y;
                let dy = dy + y;
                let Range { start, .. } = self.pixel_indices_unchecked(sx, sy);
                let Range { end, .. } = self.pixel_indices_unchecked(sx + width - 1, sy);
                let dst = self.pixel_indices_unchecked(dx, dy).start;
                self.data.copy_within(start..end, dst);
            }
        }
        true
    }
}

// concrete implementation for `Vec`-backed buffers
// TODO: I think that rustc does not "see" this impl any more: the impl with
// Container meets the same requirements. At least, I got compile errors that
// there is no such function as `into_vec`, whereas `into_raw` did work, and
// `into_vec` is redundant anyway, because `into_raw` will give you the vector,
// and it is more generic.
impl<P: Pixel> ImageBuffer<P, Vec<P::Subpixel>> {
    /// Creates a new image buffer based on a `Vec<P::Subpixel>`.
    ///
    /// all the pixels of this image have a value of zero, regardless of the data type or number of channels.
    ///
    /// The color space is initially set to [`sRGB`][`Cicp::SRGB`].
    ///
    /// # Panics
    ///
    /// Panics when the resulting image is larger than the maximum size of a vector.
    #[must_use]
    pub fn new(width: u32, height: u32) -> ImageBuffer<P, Vec<P::Subpixel>> {
        let size = Self::image_buffer_len(width, height)
            .expect("Buffer length in `ImageBuffer::new` overflows usize");
        ImageBuffer {
            data: vec![Zero::zero(); size],
            width,
            height,
            color: Cicp::SRGB.into_rgb(),
            _phantom: PhantomData,
        }
    }

    /// Constructs a new `ImageBuffer` by copying a pixel
    ///
    /// # Panics
    ///
    /// Panics when the resulting image is larger than the maximum size of a vector.
    pub fn from_pixel(width: u32, height: u32, pixel: P) -> ImageBuffer<P, Vec<P::Subpixel>> {
        let mut buf = ImageBuffer::new(width, height);
        for p in buf.pixels_mut() {
            *p = pixel;
        }
        buf
    }

    /// Constructs a new `ImageBuffer` by repeated application of the supplied function.
    ///
    /// The arguments to the function are the pixel's x and y coordinates.
    ///
    /// # Panics
    ///
    /// Panics when the resulting image is larger than the maximum size of a vector.
    pub fn from_fn<F>(width: u32, height: u32, mut f: F) -> ImageBuffer<P, Vec<P::Subpixel>>
    where
        F: FnMut(u32, u32) -> P,
    {
        let mut buf = ImageBuffer::new(width, height);
        for (x, y, p) in buf.enumerate_pixels_mut() {
            *p = f(x, y);
        }
        buf
    }

    /// Creates an image buffer out of an existing buffer.
    /// Returns None if the buffer is not big enough.
    #[must_use]
    pub fn from_vec(
        width: u32,
        height: u32,
        buf: Vec<P::Subpixel>,
    ) -> Option<ImageBuffer<P, Vec<P::Subpixel>>> {
        ImageBuffer::from_raw(width, height, buf)
    }

    /// Consumes the image buffer and returns the underlying data
    /// as an owned buffer
    #[must_use]
    pub fn into_vec(self) -> Vec<P::Subpixel> {
        self.into_raw()
    }

    /// Transfer the meta data, not the pixel values.
    ///
    /// This will reinterpret all the pixels.
    ///
    /// We may want to export this but under what name?
    pub(crate) fn copy_color_space_from<O: Pixel, C>(&mut self, other: &ImageBuffer<O, C>) {
        self.color = other.color;
    }
}

/// Provides color conversions for whole image buffers.
pub trait ConvertBuffer<T> {
    /// Converts `self` to a buffer of type T
    ///
    /// A generic implementation is provided to convert any image buffer to a image buffer
    /// based on a `Vec<T>`.
    fn convert(&self) -> T;
}

// concrete implementation Luma -> Rgba
impl GrayImage {
    /// Expands a color palette by re-using the existing buffer.
    /// Assumes 8 bit per pixel. Uses an optionally transparent index to
    /// adjust it's alpha value accordingly.
    #[must_use]
    pub fn expand_palette(
        self,
        palette: &[(u8, u8, u8)],
        transparent_idx: Option<u8>,
    ) -> RgbaImage {
        let (width, height) = self.dimensions();
        let mut data = self.into_raw();
        let entries = data.len();
        data.resize(entries.checked_mul(4).unwrap(), 0);
        let mut buffer = ImageBuffer::from_vec(width, height, data).unwrap();
        expand_packed(&mut buffer, 4, 8, |idx, pixel| {
            let (r, g, b) = palette[idx as usize];
            let a = if let Some(t_idx) = transparent_idx {
                if t_idx == idx {
                    0
                } else {
                    255
                }
            } else {
                255
            };
            pixel[0] = r;
            pixel[1] = g;
            pixel[2] = b;
            pixel[3] = a;
        });
        buffer
    }
}

/// This copies the color space information but is somewhat wrong, in numeric terms this conversion
/// fails to actually convert rgb/luma with consistent treatment. But this trait impl is too
/// generic to handle it correctly (missing any CICP related parameter for the coefficients) so the
/// best effort here is to copy the metadata and have slighly incorrect color. May you've only been
/// adding an alpha channel or converting sample types, which is fine.
///
/// It will very likely be deprecated in a future release.
impl<Container, FromType: Pixel, ToType: Pixel>
    ConvertBuffer<ImageBuffer<ToType, Vec<ToType::Subpixel>>> for ImageBuffer<FromType, Container>
where
    Container: Deref<Target = [FromType::Subpixel]>,
    ToType: FromColor<FromType>,
{
    /// # Examples
    /// Convert RGB image to gray image.
    /// ```no_run
    /// use image::buffer::ConvertBuffer;
    /// use image::GrayImage;
    ///
    /// let image_path = "examples/fractal.png";
    /// let image = image::open(&image_path)
    ///     .expect("Open file failed")
    ///     .to_rgba8();
    ///
    /// let gray_image: GrayImage = image.convert();
    /// ```
    fn convert(&self) -> ImageBuffer<ToType, Vec<ToType::Subpixel>> {
        let mut buffer: ImageBuffer<ToType, Vec<ToType::Subpixel>> =
            ImageBuffer::new(self.width, self.height);
        buffer.copy_color_space_from(self);
        for (to, from) in buffer.pixels_mut().zip(self.pixels()) {
            to.from_color(from);
        }
        buffer
    }
}

/// Inputs to [`ImageBuffer::copy_from_color_space`].
#[non_exhaustive]
#[derive(Default)]
pub struct ConvertColorOptions {
    /// A pre-calculated transform. This is only used when the actual colors of the input and
    /// output image match the color spaces with which the was constructed.
    ///
    /// FIXME: Clarify that the transform is cheap to clone, i.e. internally an Arc of precomputed
    /// tables and not expensive despite having `Clone`.
    pub(crate) transform: Option<CicpTransform>,
    /// Make sure we can later add options that are bound to the thread. That does not mean that
    /// all attributes will be bound to the thread, only that we can add `!Sync` options later. You
    /// should be constructing the options at the call site with each attribute being cheap to move
    /// into here.
    pub(crate) _auto_traits: PhantomData<std::rc::Rc<()>>,
}

impl ConvertColorOptions {
    pub(crate) fn as_transform(
        &mut self,
        from_color: Cicp,
        into_color: Cicp,
    ) -> Result<&CicpTransform, ImageError> {
        if let Some(tr) = &self.transform {
            tr.check_applicable(from_color, into_color)?;
        }

        if self.transform.is_none() {
            self.transform = CicpTransform::new(from_color, into_color);
        }

        self.transform.as_ref().ok_or_else(|| {
            ImageError::Unsupported(UnsupportedError::from_format_and_kind(
                crate::error::ImageFormatHint::Unknown,
                // One of them is responsible.
                UnsupportedErrorKind::ColorspaceCicp(if from_color.qualify_stability() {
                    into_color
                } else {
                    from_color
                }),
            ))
        })
    }

    pub(crate) fn as_transform_fn<FromType, IntoType>(
        &mut self,
        from_color: Cicp,
        into_color: Cicp,
    ) -> Result<&'_ CicpApplicable<'_, FromType::Subpixel>, ImageError>
    where
        FromType: PixelWithColorType,
        IntoType: PixelWithColorType,
    {
        Ok(self
            .as_transform(from_color, into_color)?
            .supported_transform_fn::<FromType, IntoType>())
    }
}

impl<C, SelfPixel: Pixel> ImageBuffer<SelfPixel, C>
where
    SelfPixel: PixelWithColorType,
    C: Deref<Target = [SelfPixel::Subpixel]> + DerefMut,
{
    /// Convert the color data to another pixel type, the color space.
    ///
    /// This method is supposed to be called by exposed monomorphized methods, not directly by
    /// users. In particular it serves to implement `DynamicImage`'s casts that go beyond those
    /// offered by `PixelWithColorType` and include, e.g., `LumaAlpha<f32>`.
    ///
    /// Before exposing this method, decide if we want a design like [`DynamicImage::to`] (many
    /// trait parameters) with color space aware `FromColor` or if we want a design that takes a
    /// `ColorType` parameter / `PixelWithColorType`. The latter is not quite as flexible but
    /// allows much greater internal changes that do not tie in with the _external_ stable API.
    pub(crate) fn cast_in_color_space<IntoPixel>(
        &self,
    ) -> ImageBuffer<IntoPixel, Vec<IntoPixel::Subpixel>>
    where
        SelfPixel: Pixel,
        IntoPixel: Pixel,
        IntoPixel: CicpPixelCast<SelfPixel>,
        SelfPixel::Subpixel: ColorComponentForCicp,
        IntoPixel::Subpixel: ColorComponentForCicp + FromPrimitive<SelfPixel::Subpixel>,
    {
        let vec = self
            .color
            .cast_pixels::<SelfPixel, IntoPixel>(self.inner_pixels(), &|| [0.2126, 0.7152, 0.0722]);
        let mut buffer = ImageBuffer::from_vec(self.width, self.height, vec)
            .expect("cast_pixels returned the right number of pixels");
        buffer.copy_color_space_from(self);
        buffer
    }

    /// Copy pixel data from one buffer to another, calculating equivalent color representations
    /// for the target's color space.
    ///
    /// Returns `Ok` if:
    /// - Both images to have the same dimensions, otherwise returns a [`ImageError::Parameter`].
    /// - The primaries and transfer functions of both image's color spaces must be supported,
    ///   otherwise returns a [`ImageError::Unsupported`].
    /// - The pixel's channel layout must be supported for conversion, otherwise returns a
    ///   [`ImageError::Unsupported`]. You can rely on RGB and RGBA always being supported. If a
    ///   layout is supported for one color space it is supported for all of them.
    ///
    /// To copy color data of arbitrary channel layouts use `DynamicImage` with the overhead of
    /// having data converted into and from RGB representation.
    pub fn copy_from_color_space<FromType, D>(
        &mut self,
        from: &ImageBuffer<FromType, D>,
        mut options: ConvertColorOptions,
    ) -> ImageResult<()>
    where
        FromType: Pixel<Subpixel = SelfPixel::Subpixel> + PixelWithColorType,
        D: Deref<Target = [SelfPixel::Subpixel]>,
    {
        if self.dimensions() != from.dimensions() {
            return Err(ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::DimensionMismatch,
            )));
        }

        let transform = options
            .as_transform_fn::<FromType, SelfPixel>(from.color_space(), self.color_space())?;

        let from = from.inner_pixels();
        let into = self.inner_pixels_mut();

        debug_assert_eq!(
            from.len() / usize::from(FromType::CHANNEL_COUNT),
            into.len() / usize::from(SelfPixel::CHANNEL_COUNT),
            "Diverging pixel count despite same size",
        );

        transform(from, into);

        Ok(())
    }

    /// Convert this buffer into a newly allocated buffer, changing the color representation.
    ///
    /// This will avoid an allocation if the target layout or the color conversion is not supported
    /// (yet).
    ///
    /// See [`ImageBuffer::copy_from_color_space`] if you intend to assign to an existing buffer,
    /// swapping the argument with `self`.
    pub fn to_color_space<IntoType>(
        &self,
        color: Cicp,
        mut options: ConvertColorOptions,
    ) -> Result<ImageBuffer<IntoType, Vec<SelfPixel::Subpixel>>, ImageError>
    where
        IntoType: Pixel<Subpixel = SelfPixel::Subpixel> + PixelWithColorType,
    {
        let transform =
            options.as_transform_fn::<SelfPixel, IntoType>(self.color_space(), color)?;

        let (width, height) = self.dimensions();
        let mut target = ImageBuffer::new(width, height);

        let from = self.inner_pixels();
        let into = target.inner_pixels_mut();

        transform(from, into);

        Ok(target)
    }

    /// Apply a color space to an image, transforming the pixel representation.
    pub fn apply_color_space(
        &mut self,
        color: Cicp,
        mut options: ConvertColorOptions,
    ) -> ImageResult<()> {
        if self.color_space() == color {
            return Ok(());
        }

        let transform =
            options.as_transform_fn::<SelfPixel, SelfPixel>(self.color_space(), color)?;

        let mut scratch = [<SelfPixel::Subpixel as crate::Primitive>::DEFAULT_MIN_VALUE; 1200];
        let chunk_len = scratch.len() / usize::from(<SelfPixel as Pixel>::CHANNEL_COUNT)
            * usize::from(<SelfPixel as Pixel>::CHANNEL_COUNT);

        for chunk in self.data.chunks_mut(chunk_len) {
            let scratch = &mut scratch[..chunk.len()];
            scratch.copy_from_slice(chunk);
            transform(scratch, chunk);
        }

        self.color = color.into_rgb();

        Ok(())
    }
}

/// Sendable Rgb image buffer
pub type RgbImage = ImageBuffer<Rgb<u8>, Vec<u8>>;
/// Sendable Rgb + alpha channel image buffer
pub type RgbaImage = ImageBuffer<Rgba<u8>, Vec<u8>>;
/// Sendable grayscale image buffer
pub type GrayImage = ImageBuffer<Luma<u8>, Vec<u8>>;
/// Sendable grayscale + alpha channel image buffer
pub type GrayAlphaImage = ImageBuffer<LumaA<u8>, Vec<u8>>;
/// Sendable 16-bit Rgb image buffer
pub(crate) type Rgb16Image = ImageBuffer<Rgb<u16>, Vec<u16>>;
/// Sendable 16-bit Rgb + alpha channel image buffer
pub(crate) type Rgba16Image = ImageBuffer<Rgba<u16>, Vec<u16>>;
/// Sendable 16-bit grayscale image buffer
pub(crate) type Gray16Image = ImageBuffer<Luma<u16>, Vec<u16>>;
/// Sendable 16-bit grayscale + alpha channel image buffer
pub(crate) type GrayAlpha16Image = ImageBuffer<LumaA<u16>, Vec<u16>>;

/// An image buffer for 32-bit float RGB pixels,
/// where the backing container is a flattened vector of floats.
pub type Rgb32FImage = ImageBuffer<Rgb<f32>, Vec<f32>>;

/// An image buffer for 32-bit float RGBA pixels,
/// where the backing container is a flattened vector of floats.
pub type Rgba32FImage = ImageBuffer<Rgba<f32>, Vec<f32>>;

impl From<DynamicImage> for RgbImage {
    fn from(value: DynamicImage) -> Self {
        value.into_rgb8()
    }
}

impl From<DynamicImage> for RgbaImage {
    fn from(value: DynamicImage) -> Self {
        value.into_rgba8()
    }
}

impl From<DynamicImage> for GrayImage {
    fn from(value: DynamicImage) -> Self {
        value.into_luma8()
    }
}

impl From<DynamicImage> for GrayAlphaImage {
    fn from(value: DynamicImage) -> Self {
        value.into_luma_alpha8()
    }
}

impl From<DynamicImage> for Rgb16Image {
    fn from(value: DynamicImage) -> Self {
        value.into_rgb16()
    }
}

impl From<DynamicImage> for Rgba16Image {
    fn from(value: DynamicImage) -> Self {
        value.into_rgba16()
    }
}

impl From<DynamicImage> for Gray16Image {
    fn from(value: DynamicImage) -> Self {
        value.into_luma16()
    }
}

impl From<DynamicImage> for GrayAlpha16Image {
    fn from(value: DynamicImage) -> Self {
        value.into_luma_alpha16()
    }
}

impl From<DynamicImage> for Rgba32FImage {
    fn from(value: DynamicImage) -> Self {
        value.into_rgba32f()
    }
}

#[cfg(test)]
mod test {
    use super::{GrayImage, ImageBuffer, RgbImage};
    use crate::math::Rect;
    use crate::metadata::Cicp;
    use crate::metadata::CicpTransform;
    use crate::GenericImage as _;
    use crate::ImageFormat;
    use crate::{Luma, LumaA, Pixel, Rgb, Rgba};
    use num_traits::Zero;

    #[test]
    /// Tests if image buffers from slices work
    fn slice_buffer() {
        let data = [0; 9];
        let buf: ImageBuffer<Luma<u8>, _> = ImageBuffer::from_raw(3, 3, &data[..]).unwrap();
        assert_eq!(&*buf, &data[..]);
    }

    macro_rules! new_buffer_zero_test {
        ($test_name:ident, $pxt:ty) => {
            #[test]
            fn $test_name() {
                let buffer = ImageBuffer::<$pxt, Vec<<$pxt as Pixel>::Subpixel>>::new(2, 2);
                assert!(buffer
                    .iter()
                    .all(|p| *p == <$pxt as Pixel>::Subpixel::zero()));
            }
        };
    }

    new_buffer_zero_test!(luma_u8_zero_test, Luma<u8>);
    new_buffer_zero_test!(luma_u16_zero_test, Luma<u16>);
    new_buffer_zero_test!(luma_f32_zero_test, Luma<f32>);
    new_buffer_zero_test!(luma_a_u8_zero_test, LumaA<u8>);
    new_buffer_zero_test!(luma_a_u16_zero_test, LumaA<u16>);
    new_buffer_zero_test!(luma_a_f32_zero_test, LumaA<f32>);
    new_buffer_zero_test!(rgb_u8_zero_test, Rgb<u8>);
    new_buffer_zero_test!(rgb_u16_zero_test, Rgb<u16>);
    new_buffer_zero_test!(rgb_f32_zero_test, Rgb<f32>);
    new_buffer_zero_test!(rgb_a_u8_zero_test, Rgba<u8>);
    new_buffer_zero_test!(rgb_a_u16_zero_test, Rgba<u16>);
    new_buffer_zero_test!(rgb_a_f32_zero_test, Rgba<f32>);

    #[test]
    fn get_pixel() {
        let mut a: RgbImage = ImageBuffer::new(10, 10);
        {
            let b = a.get_mut(3 * 10).unwrap();
            *b = 255;
        }
        assert_eq!(a.get_pixel(0, 1)[0], 255);
    }

    #[test]
    fn get_pixel_checked() {
        let mut a: RgbImage = ImageBuffer::new(10, 10);
        a.get_pixel_mut_checked(0, 1).unwrap()[0] = 255;

        assert_eq!(a.get_pixel_checked(0, 1), Some(&Rgb([255, 0, 0])));
        assert_eq!(a.get_pixel_checked(0, 1).unwrap(), a.get_pixel(0, 1));
        assert_eq!(a.get_pixel_checked(10, 0), None);
        assert_eq!(a.get_pixel_checked(0, 10), None);
        assert_eq!(a.get_pixel_mut_checked(10, 0), None);
        assert_eq!(a.get_pixel_mut_checked(0, 10), None);

        // From image/issues/1672
        const WHITE: Rgb<u8> = Rgb([255_u8, 255, 255]);
        let mut a = RgbImage::new(2, 1);
        a.put_pixel(1, 0, WHITE);

        assert_eq!(a.get_pixel_checked(1, 0), Some(&WHITE));
        assert_eq!(a.get_pixel_checked(1, 0).unwrap(), a.get_pixel(1, 0));
    }

    #[test]
    fn mut_iter() {
        let mut a: RgbImage = ImageBuffer::new(10, 10);
        {
            let val = a.pixels_mut().next().unwrap();
            *val = Rgb([42, 0, 0]);
        }
        assert_eq!(a.data[0], 42);
    }

    #[test]
    fn zero_width_zero_height() {
        let mut image = RgbImage::new(0, 0);

        assert_eq!(image.rows_mut().count(), 0);
        assert_eq!(image.pixels_mut().count(), 0);
        assert_eq!(image.rows().count(), 0);
        assert_eq!(image.pixels().count(), 0);
    }

    #[test]
    fn zero_width_nonzero_height() {
        let mut image = RgbImage::new(0, 2);

        assert_eq!(image.rows_mut().count(), 0);
        assert_eq!(image.pixels_mut().count(), 0);
        assert_eq!(image.rows().count(), 0);
        assert_eq!(image.pixels().count(), 0);
    }

    #[test]
    fn nonzero_width_zero_height() {
        let mut image = RgbImage::new(2, 0);

        assert_eq!(image.rows_mut().count(), 0);
        assert_eq!(image.pixels_mut().count(), 0);
        assert_eq!(image.rows().count(), 0);
        assert_eq!(image.pixels().count(), 0);
    }

    #[test]
    fn pixels_on_large_buffer() {
        let mut image = RgbImage::from_raw(1, 1, vec![0; 6]).unwrap();

        assert_eq!(image.pixels().count(), 1);
        assert_eq!(image.enumerate_pixels().count(), 1);
        assert_eq!(image.pixels_mut().count(), 1);
        assert_eq!(image.enumerate_pixels_mut().count(), 1);

        assert_eq!(image.rows().count(), 1);
        assert_eq!(image.rows_mut().count(), 1);
    }

    #[test]
    fn default() {
        let image = ImageBuffer::<Rgb<u8>, Vec<u8>>::default();
        assert_eq!(image.dimensions(), (0, 0));
    }

    #[test]
    #[rustfmt::skip]
    fn test_image_buffer_copy_within_oob() {
        let mut image: GrayImage = ImageBuffer::from_raw(4, 4, vec![0u8; 16]).unwrap();
        assert!(!image.copy_within(Rect { x: 0, y: 0, width: 5, height: 4 }, 0, 0));
        assert!(!image.copy_within(Rect { x: 0, y: 0, width: 4, height: 5 }, 0, 0));
        assert!(!image.copy_within(Rect { x: 1, y: 0, width: 4, height: 4 }, 0, 0));
        assert!(!image.copy_within(Rect { x: 0, y: 0, width: 4, height: 4 }, 1, 0));
        assert!(!image.copy_within(Rect { x: 0, y: 1, width: 4, height: 4 }, 0, 0));
        assert!(!image.copy_within(Rect { x: 0, y: 0, width: 4, height: 4 }, 0, 1));
        assert!(!image.copy_within(Rect { x: 1, y: 1, width: 4, height: 4 }, 0, 0));
    }

    #[test]
    fn test_image_buffer_copy_within_tl() {
        let data = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let expected = [0, 1, 2, 3, 4, 0, 1, 2, 8, 4, 5, 6, 12, 8, 9, 10];
        let mut image: GrayImage = ImageBuffer::from_raw(4, 4, Vec::from(&data[..])).unwrap();
        assert!(image.copy_within(
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
    fn test_image_buffer_copy_within_tr() {
        let data = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let expected = [0, 1, 2, 3, 1, 2, 3, 7, 5, 6, 7, 11, 9, 10, 11, 15];
        let mut image: GrayImage = ImageBuffer::from_raw(4, 4, Vec::from(&data[..])).unwrap();
        assert!(image.copy_within(
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
    fn test_image_buffer_copy_within_bl() {
        let data = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let expected = [0, 4, 5, 6, 4, 8, 9, 10, 8, 12, 13, 14, 12, 13, 14, 15];
        let mut image: GrayImage = ImageBuffer::from_raw(4, 4, Vec::from(&data[..])).unwrap();
        assert!(image.copy_within(
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
    fn test_image_buffer_copy_within_br() {
        let data = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let expected = [5, 6, 7, 3, 9, 10, 11, 7, 13, 14, 15, 11, 12, 13, 14, 15];
        let mut image: GrayImage = ImageBuffer::from_raw(4, 4, Vec::from(&data[..])).unwrap();
        assert!(image.copy_within(
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

    #[test]
    #[cfg(feature = "png")]
    fn write_to_with_large_buffer() {
        // A buffer of 1 pixel, padded to 4 bytes as would be common in, e.g. BMP.

        let img: GrayImage = ImageBuffer::from_raw(1, 1, vec![0u8; 4]).unwrap();
        let mut buffer = std::io::Cursor::new(vec![]);
        assert!(img.write_to(&mut buffer, ImageFormat::Png).is_ok());
    }

    #[test]
    fn exact_size_iter_size_hint() {
        // The docs for `std::iter::ExactSizeIterator` requires that the implementation of
        // `size_hint` on the iterator returns the same value as the `len` implementation.

        // This test should work for any size image.
        const N: u32 = 10;

        let mut image = RgbImage::from_raw(N, N, vec![0; (N * N * 3) as usize]).unwrap();

        let iter = image.pixels();
        let exact_len = ExactSizeIterator::len(&iter);
        assert_eq!(iter.size_hint(), (exact_len, Some(exact_len)));

        let iter = image.pixels_mut();
        let exact_len = ExactSizeIterator::len(&iter);
        assert_eq!(iter.size_hint(), (exact_len, Some(exact_len)));

        let iter = image.rows();
        let exact_len = ExactSizeIterator::len(&iter);
        assert_eq!(iter.size_hint(), (exact_len, Some(exact_len)));

        let iter = image.rows_mut();
        let exact_len = ExactSizeIterator::len(&iter);
        assert_eq!(iter.size_hint(), (exact_len, Some(exact_len)));

        let iter = image.enumerate_pixels();
        let exact_len = ExactSizeIterator::len(&iter);
        assert_eq!(iter.size_hint(), (exact_len, Some(exact_len)));

        let iter = image.enumerate_rows();
        let exact_len = ExactSizeIterator::len(&iter);
        assert_eq!(iter.size_hint(), (exact_len, Some(exact_len)));

        let iter = image.enumerate_pixels_mut();
        let exact_len = ExactSizeIterator::len(&iter);
        assert_eq!(iter.size_hint(), (exact_len, Some(exact_len)));

        let iter = image.enumerate_rows_mut();
        let exact_len = ExactSizeIterator::len(&iter);
        assert_eq!(iter.size_hint(), (exact_len, Some(exact_len)));
    }

    #[test]
    fn color_conversion() {
        let mut source = ImageBuffer::from_fn(128, 128, |_, _| Rgb([255, 0, 0]));
        let mut target = ImageBuffer::from_fn(128, 128, |_, _| Rgba(Default::default()));

        source.set_rgb_primaries(Cicp::SRGB.primaries);
        source.set_transfer_function(Cicp::SRGB.transfer);

        target.set_rgb_primaries(Cicp::DISPLAY_P3.primaries);
        target.set_transfer_function(Cicp::DISPLAY_P3.transfer);

        let result = target.copy_from_color_space(&source, Default::default());

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(target[(0, 0)], Rgba([234u8, 51, 35, 255]));
    }

    #[test]
    fn gray_conversions() {
        let mut source = ImageBuffer::from_fn(128, 128, |_, _| Luma([255u8]));
        let mut target = ImageBuffer::from_fn(128, 128, |_, _| Rgba(Default::default()));

        source.set_rgb_primaries(Cicp::SRGB.primaries);
        source.set_transfer_function(Cicp::SRGB.transfer);

        target.set_rgb_primaries(Cicp::SRGB.primaries);
        target.set_transfer_function(Cicp::SRGB.transfer);

        let result = target.copy_from_color_space(&source, Default::default());

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(target[(0, 0)], Rgba([u8::MAX; 4]));
    }

    #[test]
    fn rgb_to_gray_conversion() {
        let mut source = ImageBuffer::from_fn(128, 128, |_, _| Rgb([128u8; 3]));
        let mut target = ImageBuffer::from_fn(128, 128, |_, _| Luma(Default::default()));

        source.set_rgb_primaries(Cicp::SRGB.primaries);
        source.set_transfer_function(Cicp::SRGB.transfer);

        target.set_rgb_primaries(Cicp::SRGB.primaries);
        target.set_transfer_function(Cicp::SRGB.transfer);

        let result = target.copy_from_color_space(&source, Default::default());

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(target[(0, 0)], Luma([128u8]));
    }

    #[test]
    fn apply_color() {
        let mut buffer = ImageBuffer::from_fn(128, 128, |_, _| Rgb([255u8, 0, 0]));

        buffer.set_rgb_primaries(Cicp::SRGB.primaries);
        buffer.set_transfer_function(Cicp::SRGB.transfer);

        buffer
            .apply_color_space(Cicp::DISPLAY_P3, Default::default())
            .expect("supported transform");

        buffer.pixels().for_each(|&p| {
            assert_eq!(p, Rgb([234u8, 51, 35]));
        });
    }

    #[test]
    fn to_color() {
        let mut source = ImageBuffer::from_fn(128, 128, |_, _| Rgba([255u8, 0, 0, 255]));
        source.set_rgb_primaries(Cicp::SRGB.primaries);
        source.set_transfer_function(Cicp::SRGB.transfer);

        let target = source
            .to_color_space::<Rgb<u8>>(Cicp::DISPLAY_P3, Default::default())
            .expect("supported transform");

        assert_eq!(target[(0, 0)], Rgb([234u8, 51, 35]));
    }

    #[test]
    fn transformation_mismatch() {
        let mut source = ImageBuffer::from_fn(128, 128, |_, _| Luma([255u8]));
        let mut target = ImageBuffer::from_fn(128, 128, |_, _| Rgba(Default::default()));

        source.set_color_space(Cicp::SRGB).unwrap();
        target.set_color_space(Cicp::DISPLAY_P3).unwrap();

        let options = super::ConvertColorOptions {
            transform: CicpTransform::new(Cicp::SRGB, Cicp::SRGB),
            ..super::ConvertColorOptions::default()
        };

        let result = target.copy_from_color_space(&source, options);
        assert!(matches!(result, Err(crate::ImageError::Parameter(_))));
    }
}

#[cfg(test)]
#[cfg(feature = "benchmarks")]
mod benchmarks {
    use super::{ConvertBuffer, GrayImage, ImageBuffer, Pixel, RgbImage};

    #[bench]
    fn conversion(b: &mut test::Bencher) {
        let mut a: RgbImage = ImageBuffer::new(1000, 1000);
        for p in a.pixels_mut() {
            let rgb = p.channels_mut();
            rgb[0] = 255;
            rgb[1] = 23;
            rgb[2] = 42;
        }

        assert!(a.data[0] != 0);
        b.iter(|| {
            let b: GrayImage = a.convert();
            assert!(0 != b.data[0]);
            assert!(a.data[0] != b.data[0]);
            test::black_box(b);
        });
        b.bytes = 1000 * 1000 * 3;
    }

    #[bench]
    fn image_access_row_by_row(b: &mut test::Bencher) {
        let mut a: RgbImage = ImageBuffer::new(1000, 1000);
        for p in a.pixels_mut() {
            let rgb = p.channels_mut();
            rgb[0] = 255;
            rgb[1] = 23;
            rgb[2] = 42;
        }

        b.iter(move || {
            let image: &RgbImage = test::black_box(&a);
            let mut sum: usize = 0;
            for y in 0..1000 {
                for x in 0..1000 {
                    let pixel = image.get_pixel(x, y);
                    sum = sum.wrapping_add(pixel[0] as usize);
                    sum = sum.wrapping_add(pixel[1] as usize);
                    sum = sum.wrapping_add(pixel[2] as usize);
                }
            }
            test::black_box(sum)
        });

        b.bytes = 1000 * 1000 * 3;
    }

    #[bench]
    fn image_access_col_by_col(b: &mut test::Bencher) {
        let mut a: RgbImage = ImageBuffer::new(1000, 1000);
        for p in a.pixels_mut() {
            let rgb = p.channels_mut();
            rgb[0] = 255;
            rgb[1] = 23;
            rgb[2] = 42;
        }

        b.iter(move || {
            let image: &RgbImage = test::black_box(&a);
            let mut sum: usize = 0;
            for x in 0..1000 {
                for y in 0..1000 {
                    let pixel = image.get_pixel(x, y);
                    sum = sum.wrapping_add(pixel[0] as usize);
                    sum = sum.wrapping_add(pixel[1] as usize);
                    sum = sum.wrapping_add(pixel[2] as usize);
                }
            }
            test::black_box(sum)
        });

        b.bytes = 1000 * 1000 * 3;
    }
}
