use rayon::iter::plumbing::*;
use rayon::iter::{IndexedParallelIterator, ParallelIterator};
use rayon::slice::{ChunksExact, ChunksExactMut, ParallelSlice, ParallelSliceMut};
use std::fmt;
use std::ops::{Deref, DerefMut};

use crate::traits::Pixel;
use crate::ImageBuffer;

/// Parallel iterator over pixel refs.
#[derive(Clone)]
pub struct PixelsPar<'a, P>
where
    P: Pixel + Sync + 'a,
    P::Subpixel: Sync + 'a,
{
    chunks: ChunksExact<'a, P::Subpixel>,
}

impl<'a, P> ParallelIterator for PixelsPar<'a, P>
where
    P: Pixel + Sync + 'a,
    P::Subpixel: Sync + 'a,
{
    type Item = &'a P;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.chunks
            .map(|v| <P as Pixel>::from_slice(v))
            .drive_unindexed(consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'a, P> IndexedParallelIterator for PixelsPar<'a, P>
where
    P: Pixel + Sync + 'a,
    P::Subpixel: Sync + 'a,
{
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        self.chunks
            .map(|v| <P as Pixel>::from_slice(v))
            .drive(consumer)
    }

    fn len(&self) -> usize {
        self.chunks.len()
    }

    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
        self.chunks
            .map(|v| <P as Pixel>::from_slice(v))
            .with_producer(callback)
    }
}

impl<P> fmt::Debug for PixelsPar<'_, P>
where
    P: Pixel + Sync,
    P::Subpixel: Sync + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PixelsPar")
            .field("chunks", &self.chunks)
            .finish()
    }
}

/// Parallel iterator over mutable pixel refs.
pub struct PixelsMutPar<'a, P>
where
    P: Pixel + Send + Sync + 'a,
    P::Subpixel: Send + Sync + 'a,
{
    chunks: ChunksExactMut<'a, P::Subpixel>,
}

impl<'a, P> ParallelIterator for PixelsMutPar<'a, P>
where
    P: Pixel + Send + Sync + 'a,
    P::Subpixel: Send + Sync + 'a,
{
    type Item = &'a mut P;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.chunks
            .map(|v| <P as Pixel>::from_slice_mut(v))
            .drive_unindexed(consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'a, P> IndexedParallelIterator for PixelsMutPar<'a, P>
where
    P: Pixel + Send + Sync + 'a,
    P::Subpixel: Send + Sync + 'a,
{
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        self.chunks
            .map(|v| <P as Pixel>::from_slice_mut(v))
            .drive(consumer)
    }

    fn len(&self) -> usize {
        self.chunks.len()
    }

    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
        self.chunks
            .map(|v| <P as Pixel>::from_slice_mut(v))
            .with_producer(callback)
    }
}

impl<P> fmt::Debug for PixelsMutPar<'_, P>
where
    P: Pixel + Send + Sync,
    P::Subpixel: Send + Sync + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PixelsMutPar")
            .field("chunks", &self.chunks)
            .finish()
    }
}

/// Parallel iterator over pixel refs and their coordinates.
#[derive(Clone)]
pub struct EnumeratePixelsPar<'a, P>
where
    P: Pixel + Sync + 'a,
    P::Subpixel: Sync + 'a,
{
    pixels: PixelsPar<'a, P>,
    width: u32,
}

impl<'a, P> ParallelIterator for EnumeratePixelsPar<'a, P>
where
    P: Pixel + Sync + 'a,
    P::Subpixel: Sync + 'a,
{
    type Item = (u32, u32, &'a P);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.pixels
            .enumerate()
            .map(|(i, p)| {
                (
                    (i % self.width as usize) as u32,
                    (i / self.width as usize) as u32,
                    p,
                )
            })
            .drive_unindexed(consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'a, P> IndexedParallelIterator for EnumeratePixelsPar<'a, P>
where
    P: Pixel + Sync + 'a,
    P::Subpixel: Sync + 'a,
{
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        self.pixels
            .enumerate()
            .map(|(i, p)| {
                (
                    (i % self.width as usize) as u32,
                    (i / self.width as usize) as u32,
                    p,
                )
            })
            .drive(consumer)
    }

    fn len(&self) -> usize {
        self.pixels.len()
    }

    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
        self.pixels
            .enumerate()
            .map(|(i, p)| {
                (
                    (i % self.width as usize) as u32,
                    (i / self.width as usize) as u32,
                    p,
                )
            })
            .with_producer(callback)
    }
}

impl<P> fmt::Debug for EnumeratePixelsPar<'_, P>
where
    P: Pixel + Sync,
    P::Subpixel: Sync + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EnumeratePixelsPar")
            .field("pixels", &self.pixels)
            .field("width", &self.width)
            .finish()
    }
}

/// Parallel iterator over mutable pixel refs and their coordinates.
pub struct EnumeratePixelsMutPar<'a, P>
where
    P: Pixel + Send + Sync + 'a,
    P::Subpixel: Send + Sync + 'a,
{
    pixels: PixelsMutPar<'a, P>,
    width: u32,
}

impl<'a, P> ParallelIterator for EnumeratePixelsMutPar<'a, P>
where
    P: Pixel + Send + Sync + 'a,
    P::Subpixel: Send + Sync + 'a,
{
    type Item = (u32, u32, &'a mut P);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.pixels
            .enumerate()
            .map(|(i, p)| {
                (
                    (i % self.width as usize) as u32,
                    (i / self.width as usize) as u32,
                    p,
                )
            })
            .drive_unindexed(consumer)
    }

    fn opt_len(&self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'a, P> IndexedParallelIterator for EnumeratePixelsMutPar<'a, P>
where
    P: Pixel + Send + Sync + 'a,
    P::Subpixel: Send + Sync + 'a,
{
    fn drive<C: Consumer<Self::Item>>(self, consumer: C) -> C::Result {
        self.pixels
            .enumerate()
            .map(|(i, p)| {
                (
                    (i % self.width as usize) as u32,
                    (i / self.width as usize) as u32,
                    p,
                )
            })
            .drive(consumer)
    }

    fn len(&self) -> usize {
        self.pixels.len()
    }

    fn with_producer<CB: ProducerCallback<Self::Item>>(self, callback: CB) -> CB::Output {
        self.pixels
            .enumerate()
            .map(|(i, p)| {
                (
                    (i % self.width as usize) as u32,
                    (i / self.width as usize) as u32,
                    p,
                )
            })
            .with_producer(callback)
    }
}

impl<P> fmt::Debug for EnumeratePixelsMutPar<'_, P>
where
    P: Pixel + Send + Sync,
    P::Subpixel: Send + Sync + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("EnumeratePixelsMutPar")
            .field("pixels", &self.pixels)
            .field("width", &self.width)
            .finish()
    }
}

impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel + Sync,
    P::Subpixel: Sync,
    Container: Deref<Target = [P::Subpixel]>,
{
    /// Returns a parallel iterator over the pixels of this image, usable with `rayon`.
    /// See [`pixels`] for more information.
    ///
    /// [`pixels`]: #method.pixels
    pub fn par_pixels(&self) -> PixelsPar<'_, P> {
        PixelsPar {
            chunks: self
                .inner_pixels()
                .par_chunks_exact(<P as Pixel>::CHANNEL_COUNT as usize),
        }
    }

    /// Returns a parallel iterator over the pixels of this image and their coordinates, usable with `rayon`.
    /// See [`enumerate_pixels`] for more information.
    ///
    /// [`enumerate_pixels`]: #method.enumerate_pixels
    pub fn par_enumerate_pixels(&self) -> EnumeratePixelsPar<'_, P> {
        EnumeratePixelsPar {
            pixels: self.par_pixels(),
            width: self.width(),
        }
    }
}

impl<P, Container> ImageBuffer<P, Container>
where
    P: Pixel + Send + Sync,
    P::Subpixel: Send + Sync,
    Container: Deref<Target = [P::Subpixel]> + DerefMut,
{
    /// Returns a parallel iterator over the mutable pixels of this image, usable with `rayon`.
    /// See [`pixels_mut`] for more information.
    ///
    /// [`pixels_mut`]: #method.pixels_mut
    pub fn par_pixels_mut(&mut self) -> PixelsMutPar<'_, P> {
        PixelsMutPar {
            chunks: self
                .inner_pixels_mut()
                .par_chunks_exact_mut(<P as Pixel>::CHANNEL_COUNT as usize),
        }
    }

    /// Returns a parallel iterator over the mutable pixels of this image and their coordinates, usable with `rayon`.
    /// See [`enumerate_pixels_mut`] for more information.
    ///
    /// [`enumerate_pixels_mut`]: #method.enumerate_pixels_mut
    pub fn par_enumerate_pixels_mut(&mut self) -> EnumeratePixelsMutPar<'_, P> {
        let width = self.width();
        EnumeratePixelsMutPar {
            pixels: self.par_pixels_mut(),
            width,
        }
    }
}

impl<P> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel + Send + Sync,
    P::Subpixel: Send + Sync,
{
    /// Constructs a new `ImageBuffer` by repeated application of the supplied function,
    /// utilizing multi-threading via `rayon`.
    ///
    /// The arguments to the function are the pixel's x and y coordinates.
    ///
    /// # Panics
    ///
    /// Panics when the resulting image is larger than the maximum size of a vector.
    pub fn from_par_fn<F>(width: u32, height: u32, f: F) -> ImageBuffer<P, Vec<P::Subpixel>>
    where
        F: Fn(u32, u32) -> P + Send + Sync,
    {
        let mut buf = ImageBuffer::new(width, height);
        buf.par_enumerate_pixels_mut().for_each(|(x, y, p)| {
            *p = f(x, y);
        });

        buf
    }
}

#[cfg(test)]
mod test {
    use crate::{Rgb, RgbImage};
    use rayon::iter::{IndexedParallelIterator, ParallelIterator};

    fn test_width_height(width: u32, height: u32, len: usize) {
        let mut image = RgbImage::new(width, height);

        assert_eq!(image.par_enumerate_pixels_mut().len(), len);
        assert_eq!(image.par_enumerate_pixels().len(), len);
        assert_eq!(image.par_pixels_mut().len(), len);
        assert_eq!(image.par_pixels().len(), len);
    }

    #[test]
    fn zero_width_zero_height() {
        test_width_height(0, 0, 0);
    }

    #[test]
    fn zero_width_nonzero_height() {
        test_width_height(0, 2, 0);
    }

    #[test]
    fn nonzero_width_zero_height() {
        test_width_height(2, 0, 0);
    }

    #[test]
    fn iter_parity() {
        let mut image1 = RgbImage::from_fn(17, 29, |x, y| {
            Rgb(std::array::from_fn(|i| {
                ((x + y * 98 + i as u32 * 27) % 255) as u8
            }))
        });
        let mut image2 = image1.clone();

        assert_eq!(
            image1.enumerate_pixels_mut().collect::<Vec<_>>(),
            image2.par_enumerate_pixels_mut().collect::<Vec<_>>()
        );
        assert_eq!(
            image1.enumerate_pixels().collect::<Vec<_>>(),
            image2.par_enumerate_pixels().collect::<Vec<_>>()
        );
        assert_eq!(
            image1.pixels_mut().collect::<Vec<_>>(),
            image2.par_pixels_mut().collect::<Vec<_>>()
        );
        assert_eq!(
            image1.pixels().collect::<Vec<_>>(),
            image2.par_pixels().collect::<Vec<_>>()
        );
    }
}

#[cfg(test)]
#[cfg(feature = "benchmarks")]
mod benchmarks {
    use crate::{Rgb, RgbImage};

    const S: u32 = 1024;

    #[bench]
    fn creation(b: &mut test::Bencher) {
        let mut bytes = 0;
        b.iter(|| {
            let img = RgbImage::from_fn(S, S, |_, _| test::black_box(pixel_func()));

            bytes += img.as_raw().len() as u64;
        });

        b.bytes = bytes;
    }

    #[bench]
    fn creation_par(b: &mut test::Bencher) {
        let mut bytes = 0;
        b.iter(|| {
            let img = RgbImage::from_par_fn(S, S, |_, _| test::black_box(pixel_func()));

            bytes += img.as_raw().len() as u64;
        });

        b.bytes = bytes;
    }

    fn pixel_func() -> Rgb<u8> {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};
        Rgb(std::array::from_fn(|_| {
            RandomState::new().build_hasher().finish() as u8
        }))
    }
}
