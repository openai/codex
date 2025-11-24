//! Image representations for ffi.
//!
//! # Usage
//!
//! Imagine you want to offer a very simple ffi interface: The caller provides an image buffer and
//! your program creates a thumbnail from it and dumps that image as `png`. This module is designed
//! to help you transition from raw memory data to Rust representation.
//!
//! ```no_run
//! use std::ptr;
//! use std::slice;
//! use image::Rgb;
//! use image::flat::{FlatSamples, SampleLayout};
//! use image::imageops::thumbnail;
//!
//! #[no_mangle]
//! pub extern "C" fn store_rgb8_compressed(
//!     data: *const u8, len: usize,
//!     layout: *const SampleLayout
//! )
//!     -> bool
//! {
//!     let samples = unsafe { slice::from_raw_parts(data, len) };
//!     let layout = unsafe { ptr::read(layout) };
//!
//!     let buffer = FlatSamples {
//!         samples,
//!         layout,
//!         color_hint: None,
//!     };
//!
//!     let view = match buffer.as_view::<Rgb<u8>>() {
//!         Err(_) => return false, // Invalid layout.
//!         Ok(view) => view,
//!     };
//!
//!     thumbnail(&view, 64, 64)
//!         .save("output.png")
//!         .map(|_| true)
//!         .unwrap_or_else(|_| false)
//! }
//! ```
//!
use std::marker::PhantomData;
use std::ops::{Deref, Index, IndexMut};
use std::{cmp, error, fmt};

use num_traits::Zero;

use crate::color::ColorType;
use crate::error::{
    DecodingError, ImageError, ImageFormatHint, ParameterError, ParameterErrorKind,
    UnsupportedError, UnsupportedErrorKind,
};
use crate::traits::Pixel;
use crate::{GenericImage, GenericImageView, ImageBuffer};

/// A flat buffer over a (multi channel) image.
///
/// In contrast to `ImageBuffer`, this representation of a sample collection is much more lenient
/// in the layout thereof. It also allows grouping by color planes instead of by pixel as long as
/// the strides of each extent are constant. This struct itself has no invariants on the strides
/// but not every possible configuration can be interpreted as a [`GenericImageView`] or
/// [`GenericImage`]. The methods [`as_view`] and [`as_view_mut`] construct the actual implementors
/// of these traits and perform necessary checks. To manually perform this and other layout checks
/// use [`is_normal`] or [`has_aliased_samples`].
///
/// Instances can be constructed not only by hand. The buffer instances returned by library
/// functions such as [`ImageBuffer::as_flat_samples`] guarantee that the conversion to a generic
/// image or generic view succeeds. A very different constructor is [`with_monocolor`]. It uses a
/// single pixel as the backing storage for an arbitrarily sized read-only raster by mapping each
/// pixel to the same samples by setting some strides to `0`.
///
/// [`GenericImage`]: ../trait.GenericImage.html
/// [`GenericImageView`]: ../trait.GenericImageView.html
/// [`ImageBuffer::as_flat_samples`]: ../struct.ImageBuffer.html#method.as_flat_samples
/// [`is_normal`]: #method.is_normal
/// [`has_aliased_samples`]: #method.has_aliased_samples
/// [`as_view`]: #method.as_view
/// [`as_view_mut`]: #method.as_view_mut
/// [`with_monocolor`]: #method.with_monocolor
#[derive(Clone, Debug)]
pub struct FlatSamples<Buffer> {
    /// Underlying linear container holding sample values.
    pub samples: Buffer,

    /// A `repr(C)` description of the layout of buffer samples.
    pub layout: SampleLayout,

    /// Supplementary color information.
    ///
    /// You may keep this as `None` in most cases. This is NOT checked in `View` or other
    /// converters. It is intended mainly as a way for types that convert to this buffer type to
    /// attach their otherwise static color information. A dynamic image representation could
    /// however use this to resolve representational ambiguities such as the order of RGB channels.
    pub color_hint: Option<ColorType>,
}

/// A ffi compatible description of a sample buffer.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SampleLayout {
    /// The number of channels in the color representation of the image.
    pub channels: u8,

    /// Add this to an index to get to the sample in the next channel.
    pub channel_stride: usize,

    /// The width of the represented image.
    pub width: u32,

    /// Add this to an index to get to the next sample in x-direction.
    pub width_stride: usize,

    /// The height of the represented image.
    pub height: u32,

    /// Add this to an index to get to the next sample in y-direction.
    pub height_stride: usize,
}

/// Helper struct for an unnamed (stride, length) pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Dim(usize, usize);

impl SampleLayout {
    /// Describe a row-major image packed in all directions.
    ///
    /// The resulting will surely be `NormalForm::RowMajorPacked`. It can therefore be converted to
    /// safely to an `ImageBuffer` with a large enough underlying buffer.
    ///
    /// ```
    /// # use image::flat::{NormalForm, SampleLayout};
    /// let layout = SampleLayout::row_major_packed(3, 640, 480);
    /// assert!(layout.is_normal(NormalForm::RowMajorPacked));
    /// ```
    ///
    /// # Panics
    ///
    /// On platforms where `usize` has the same size as `u32` this panics when the resulting stride
    /// in the `height` direction would be larger than `usize::MAX`. On other platforms
    /// where it can surely accommodate `u8::MAX * u32::MAX`, this can never happen.
    #[must_use]
    pub fn row_major_packed(channels: u8, width: u32, height: u32) -> Self {
        let height_stride = (channels as usize).checked_mul(width as usize).expect(
            "Row major packed image can not be described because it does not fit into memory",
        );
        SampleLayout {
            channels,
            channel_stride: 1,
            width,
            width_stride: channels as usize,
            height,
            height_stride,
        }
    }

    /// Describe a column-major image packed in all directions.
    ///
    /// The resulting will surely be `NormalForm::ColumnMajorPacked`. This is not particularly
    /// useful for conversion but can be used to describe such a buffer without pitfalls.
    ///
    /// ```
    /// # use image::flat::{NormalForm, SampleLayout};
    /// let layout = SampleLayout::column_major_packed(3, 640, 480);
    /// assert!(layout.is_normal(NormalForm::ColumnMajorPacked));
    /// ```
    ///
    /// # Panics
    ///
    /// On platforms where `usize` has the same size as `u32` this panics when the resulting stride
    /// in the `width` direction would be larger than `usize::MAX`. On other platforms
    /// where it can surely accommodate `u8::MAX * u32::MAX`, this can never happen.
    #[must_use]
    pub fn column_major_packed(channels: u8, width: u32, height: u32) -> Self {
        let width_stride = (channels as usize).checked_mul(height as usize).expect(
            "Column major packed image can not be described because it does not fit into memory",
        );
        SampleLayout {
            channels,
            channel_stride: 1,
            height,
            height_stride: channels as usize,
            width,
            width_stride,
        }
    }

    /// Get the strides for indexing matrix-like `[(c, w, h)]`.
    ///
    /// For a row-major layout with grouped samples, this tuple is strictly
    /// increasing.
    #[must_use]
    pub fn strides_cwh(&self) -> (usize, usize, usize) {
        (self.channel_stride, self.width_stride, self.height_stride)
    }

    /// Get the dimensions `(channels, width, height)`.
    ///
    /// The interface is optimized for use with `strides_cwh` instead. The channel extent will be
    /// before width and height.
    #[must_use]
    pub fn extents(&self) -> (usize, usize, usize) {
        (
            self.channels as usize,
            self.width as usize,
            self.height as usize,
        )
    }

    /// Tuple of bounds in the order of coordinate inputs.
    ///
    /// This function should be used whenever working with image coordinates opposed to buffer
    /// coordinates. The only difference compared to `extents` is the output type.
    #[must_use]
    pub fn bounds(&self) -> (u8, u32, u32) {
        (self.channels, self.width, self.height)
    }

    /// Get the minimum length of a buffer such that all in-bounds samples have valid indices.
    ///
    /// This method will allow zero strides, allowing compact representations of monochrome images.
    /// To check that no aliasing occurs, try `check_alias_invariants`. For compact images (no
    /// aliasing and no unindexed samples) this is `width*height*channels`. But for both of the
    /// other cases, the reasoning is slightly more involved.
    ///
    /// # Explanation
    ///
    /// Note that there is a difference between `min_length` and the index of the sample
    /// 'one-past-the-end'. This is due to strides that may be larger than the dimension below.
    ///
    /// ## Example with holes
    ///
    /// Let's look at an example of a grayscale image with
    /// * `width_stride = 1`
    /// * `width = 2`
    /// * `height_stride = 3`
    /// * `height = 2`
    ///
    /// ```text
    /// | x x   | x x m | $
    ///  min_length m ^
    ///                   ^ one-past-the-end $
    /// ```
    ///
    /// The difference is also extreme for empty images with large strides. The one-past-the-end
    /// sample index is still as large as the largest of these strides while `min_length = 0`.
    ///
    /// ## Example with aliasing
    ///
    /// The concept gets even more important when you allow samples to alias each other. Here we
    /// have the buffer of a small grayscale image where this is the case, this time we will first
    /// show the buffer and then the individual rows below.
    ///
    /// * `width_stride = 1`
    /// * `width = 3`
    /// * `height_stride = 2`
    /// * `height = 2`
    ///
    /// ```text
    ///  1 2 3 4 5 m
    /// |1 2 3| row one
    ///     |3 4 5| row two
    ///            ^ m min_length
    ///          ^ ??? one-past-the-end
    /// ```
    ///
    /// This time 'one-past-the-end' is not even simply the largest stride times the extent of its
    /// dimension. That still points inside the image because `height*height_stride = 4` but also
    /// `index_of(1, 2) = 4`.
    #[must_use]
    pub fn min_length(&self) -> Option<usize> {
        if self.width == 0 || self.height == 0 || self.channels == 0 {
            return Some(0);
        }

        self.index(self.channels - 1, self.width - 1, self.height - 1)
            .and_then(|idx| idx.checked_add(1))
    }

    /// Check if a buffer of length `len` is large enough.
    #[must_use]
    pub fn fits(&self, len: usize) -> bool {
        self.min_length().is_some_and(|min| len >= min)
    }

    /// The extents of this array, in order of increasing strides.
    fn increasing_stride_dims(&self) -> [Dim; 3] {
        // Order extents by strides, then check that each is less equal than the next stride.
        let mut grouped: [Dim; 3] = [
            Dim(self.channel_stride, self.channels as usize),
            Dim(self.width_stride, self.width as usize),
            Dim(self.height_stride, self.height as usize),
        ];

        grouped.sort();

        let (min_dim, mid_dim, max_dim) = (grouped[0], grouped[1], grouped[2]);
        assert!(min_dim.stride() <= mid_dim.stride() && mid_dim.stride() <= max_dim.stride());

        grouped
    }

    /// If there are any samples aliasing each other.
    ///
    /// If this is not the case, it would always be safe to allow mutable access to two different
    /// samples at the same time. Otherwise, this operation would need additional checks. When one
    /// dimension overflows `usize` with its stride we also consider this aliasing.
    #[must_use]
    pub fn has_aliased_samples(&self) -> bool {
        let grouped = self.increasing_stride_dims();
        let (min_dim, mid_dim, max_dim) = (grouped[0], grouped[1], grouped[2]);

        let min_size = match min_dim.checked_len() {
            None => return true,
            Some(size) => size,
        };

        let mid_size = match mid_dim.checked_len() {
            None => return true,
            Some(size) => size,
        };

        if max_dim.checked_len().is_none() {
            return true;
        }

        // Each higher dimension must walk over all of one lower dimension.
        min_size > mid_dim.stride() || mid_size > max_dim.stride()
    }

    /// Check if a buffer fulfills the requirements of a normal form.
    ///
    /// Certain conversions have preconditions on the structure of the sample buffer that are not
    /// captured (by design) by the type system. These are then checked before the conversion. Such
    /// checks can all be done in constant time and will not inspect the buffer content. You can
    /// perform these checks yourself when the conversion is not required at this moment but maybe
    /// still performed later.
    #[must_use]
    pub fn is_normal(&self, form: NormalForm) -> bool {
        if self.has_aliased_samples() {
            return false;
        }

        if form >= NormalForm::PixelPacked && self.channel_stride != 1 {
            return false;
        }

        if form >= NormalForm::ImagePacked {
            // has aliased already checked for overflows.
            let grouped = self.increasing_stride_dims();
            let (min_dim, mid_dim, max_dim) = (grouped[0], grouped[1], grouped[2]);

            if 1 != min_dim.stride() {
                return false;
            }

            if min_dim.len() != mid_dim.stride() {
                return false;
            }

            if mid_dim.len() != max_dim.stride() {
                return false;
            }
        }

        if form >= NormalForm::RowMajorPacked {
            if self.width_stride != self.channels as usize {
                return false;
            }

            if self.width as usize * self.width_stride != self.height_stride {
                return false;
            }
        }

        if form >= NormalForm::ColumnMajorPacked {
            if self.height_stride != self.channels as usize {
                return false;
            }

            if self.height as usize * self.height_stride != self.width_stride {
                return false;
            }
        }

        true
    }

    /// Check that the pixel and the channel index are in bounds.
    ///
    /// An in-bound coordinate does not yet guarantee that the corresponding calculation of a
    /// buffer index does not overflow. However, if such a buffer large enough to hold all samples
    /// actually exists in memory, this property of course follows.
    #[must_use]
    pub fn in_bounds(&self, channel: u8, x: u32, y: u32) -> bool {
        channel < self.channels && x < self.width && y < self.height
    }

    /// Resolve the index of a particular sample.
    ///
    /// `None` if the index is outside the bounds or does not fit into a `usize`.
    #[must_use]
    pub fn index(&self, channel: u8, x: u32, y: u32) -> Option<usize> {
        if !self.in_bounds(channel, x, y) {
            return None;
        }

        self.index_ignoring_bounds(channel as usize, x as usize, y as usize)
    }

    /// Get the theoretical position of sample (channel, x, y).
    ///
    /// The 'check' is for overflow during index calculation, not that it is contained in the
    /// image. Two samples may return the same index, even when one of them is out of bounds. This
    /// happens when all strides are `0`, i.e. the image is an arbitrarily large monochrome image.
    #[must_use]
    pub fn index_ignoring_bounds(&self, channel: usize, x: usize, y: usize) -> Option<usize> {
        let idx_c = channel.checked_mul(self.channel_stride);
        let idx_x = x.checked_mul(self.width_stride);
        let idx_y = y.checked_mul(self.height_stride);

        let (Some(idx_c), Some(idx_x), Some(idx_y)) = (idx_c, idx_x, idx_y) else {
            return None;
        };

        Some(0usize)
            .and_then(|b| b.checked_add(idx_c))
            .and_then(|b| b.checked_add(idx_x))
            .and_then(|b| b.checked_add(idx_y))
    }

    /// Get an index provided it is inbouds.
    ///
    /// Assumes that the image is backed by some sufficiently large buffer. Then computation can
    /// not overflow as we could represent the maximum coordinate. Since overflow is defined either
    /// way, this method can not be unsafe.
    ///
    /// Behavior is *unspecified* if the index is out of bounds or this sample layout would require
    /// a buffer larger than `isize::MAX` bytes.
    #[must_use]
    pub fn in_bounds_index(&self, c: u8, x: u32, y: u32) -> usize {
        let (c_stride, x_stride, y_stride) = self.strides_cwh();
        (y as usize * y_stride) + (x as usize * x_stride) + (c as usize * c_stride)
    }

    /// Shrink the image to the minimum of current and given extents.
    ///
    /// This does not modify the strides, so that the resulting sample buffer may have holes
    /// created by the shrinking operation. Shrinking could also lead to an non-aliasing image when
    /// samples had aliased each other before.
    pub fn shrink_to(&mut self, channels: u8, width: u32, height: u32) {
        self.channels = self.channels.min(channels);
        self.width = self.width.min(width);
        self.height = self.height.min(height);
    }
}

impl Dim {
    fn stride(self) -> usize {
        self.0
    }

    /// Length of this dimension in memory.
    fn checked_len(self) -> Option<usize> {
        self.0.checked_mul(self.1)
    }

    fn len(self) -> usize {
        self.0 * self.1
    }
}

impl<Buffer> FlatSamples<Buffer> {
    /// Get the strides for indexing matrix-like `[(c, w, h)]`.
    ///
    /// For a row-major layout with grouped samples, this tuple is strictly
    /// increasing.
    pub fn strides_cwh(&self) -> (usize, usize, usize) {
        self.layout.strides_cwh()
    }

    /// Get the dimensions `(channels, width, height)`.
    ///
    /// The interface is optimized for use with `strides_cwh` instead. The channel extent will be
    /// before width and height.
    pub fn extents(&self) -> (usize, usize, usize) {
        self.layout.extents()
    }

    /// Tuple of bounds in the order of coordinate inputs.
    ///
    /// This function should be used whenever working with image coordinates opposed to buffer
    /// coordinates. The only difference compared to `extents` is the output type.
    pub fn bounds(&self) -> (u8, u32, u32) {
        self.layout.bounds()
    }

    /// Get a reference based version.
    pub fn as_ref<T>(&self) -> FlatSamples<&[T]>
    where
        Buffer: AsRef<[T]>,
    {
        FlatSamples {
            samples: self.samples.as_ref(),
            layout: self.layout,
            color_hint: self.color_hint,
        }
    }

    /// Get a mutable reference based version.
    pub fn as_mut<T>(&mut self) -> FlatSamples<&mut [T]>
    where
        Buffer: AsMut<[T]>,
    {
        FlatSamples {
            samples: self.samples.as_mut(),
            layout: self.layout,
            color_hint: self.color_hint,
        }
    }

    /// Copy the data into an owned vector.
    pub fn to_vec<T>(&self) -> FlatSamples<Vec<T>>
    where
        T: Clone,
        Buffer: AsRef<[T]>,
    {
        FlatSamples {
            samples: self.samples.as_ref().to_vec(),
            layout: self.layout,
            color_hint: self.color_hint,
        }
    }

    /// Get a reference to a single sample.
    ///
    /// This more restrictive than the method based on `std::ops::Index` but guarantees to properly
    /// check all bounds and not panic as long as `Buffer::as_ref` does not do so.
    ///
    /// ```
    /// # use image::{RgbImage};
    /// let flat = RgbImage::new(480, 640).into_flat_samples();
    ///
    /// // Get the blue channel at (10, 10).
    /// assert!(flat.get_sample(1, 10, 10).is_some());
    ///
    /// // There is no alpha channel.
    /// assert!(flat.get_sample(3, 10, 10).is_none());
    /// ```
    ///
    /// For cases where a special buffer does not provide `AsRef<[T]>`, consider encapsulating
    /// bounds checks with `min_length` in a type similar to `View`. Then you may use
    /// `in_bounds_index` as a small speedup over the index calculation of this method which relies
    /// on `index_ignoring_bounds` since it can not have a-priori knowledge that the sample
    /// coordinate is in fact backed by any memory buffer.
    pub fn get_sample<T>(&self, channel: u8, x: u32, y: u32) -> Option<&T>
    where
        Buffer: AsRef<[T]>,
    {
        self.index(channel, x, y)
            .and_then(|idx| self.samples.as_ref().get(idx))
    }

    /// Get a mutable reference to a single sample.
    ///
    /// This more restrictive than the method based on `std::ops::IndexMut` but guarantees to
    /// properly check all bounds and not panic as long as `Buffer::as_ref` does not do so.
    /// Contrary to conversion to `ViewMut`, this does not require that samples are packed since it
    /// does not need to convert samples to a color representation.
    ///
    /// **WARNING**: Note that of course samples may alias, so that the mutable reference returned
    /// here can in fact modify more than the coordinate in the argument.
    ///
    /// ```
    /// # use image::{RgbImage};
    /// let mut flat = RgbImage::new(480, 640).into_flat_samples();
    ///
    /// // Assign some new color to the blue channel at (10, 10).
    /// *flat.get_mut_sample(1, 10, 10).unwrap() = 255;
    ///
    /// // There is no alpha channel.
    /// assert!(flat.get_mut_sample(3, 10, 10).is_none());
    /// ```
    ///
    /// For cases where a special buffer does not provide `AsRef<[T]>`, consider encapsulating
    /// bounds checks with `min_length` in a type similar to `View`. Then you may use
    /// `in_bounds_index` as a small speedup over the index calculation of this method which relies
    /// on `index_ignoring_bounds` since it can not have a-priori knowledge that the sample
    /// coordinate is in fact backed by any memory buffer.
    pub fn get_mut_sample<T>(&mut self, channel: u8, x: u32, y: u32) -> Option<&mut T>
    where
        Buffer: AsMut<[T]>,
    {
        match self.index(channel, x, y) {
            None => None,
            Some(idx) => self.samples.as_mut().get_mut(idx),
        }
    }

    /// View this buffer as an image over some type of pixel.
    ///
    /// This first ensures that all in-bounds coordinates refer to valid indices in the sample
    /// buffer. It also checks that the specified pixel format expects the same number of channels
    /// that are present in this buffer. Neither are larger nor a smaller number will be accepted.
    /// There is no automatic conversion.
    pub fn as_view<P>(&self) -> Result<View<&[P::Subpixel], P>, Error>
    where
        P: Pixel,
        Buffer: AsRef<[P::Subpixel]>,
    {
        if self.layout.channels != P::CHANNEL_COUNT {
            return Err(Error::ChannelCountMismatch(
                self.layout.channels,
                P::CHANNEL_COUNT,
            ));
        }

        let as_ref = self.samples.as_ref();
        if !self.layout.fits(as_ref.len()) {
            return Err(Error::TooLarge);
        }

        Ok(View {
            inner: FlatSamples {
                samples: as_ref,
                layout: self.layout,
                color_hint: self.color_hint,
            },
            phantom: PhantomData,
        })
    }

    /// View this buffer but keep mutability at a sample level.
    ///
    /// This is similar to `as_view` but subtly different from `as_view_mut`. The resulting type
    /// can be used as a `GenericImage` with the same prior invariants needed as for `as_view`.
    /// It can not be used as a mutable `GenericImage` but does not need channels to be packed in
    /// their pixel representation.
    ///
    /// This first ensures that all in-bounds coordinates refer to valid indices in the sample
    /// buffer. It also checks that the specified pixel format expects the same number of channels
    /// that are present in this buffer. Neither are larger nor a smaller number will be accepted.
    /// There is no automatic conversion.
    ///
    /// **WARNING**: Note that of course samples may alias, so that the mutable reference returned
    /// for one sample can in fact modify other samples as well. Sometimes exactly this is
    /// intended.
    pub fn as_view_with_mut_samples<P>(&mut self) -> Result<View<&mut [P::Subpixel], P>, Error>
    where
        P: Pixel,
        Buffer: AsMut<[P::Subpixel]>,
    {
        if self.layout.channels != P::CHANNEL_COUNT {
            return Err(Error::ChannelCountMismatch(
                self.layout.channels,
                P::CHANNEL_COUNT,
            ));
        }

        let as_mut = self.samples.as_mut();
        if !self.layout.fits(as_mut.len()) {
            return Err(Error::TooLarge);
        }

        Ok(View {
            inner: FlatSamples {
                samples: as_mut,
                layout: self.layout,
                color_hint: self.color_hint,
            },
            phantom: PhantomData,
        })
    }

    /// Interpret this buffer as a mutable image.
    ///
    /// To succeed, the pixels in this buffer may not alias each other and the samples of each
    /// pixel must be packed (i.e. `channel_stride` is `1`). The number of channels must be
    /// consistent with the channel count expected by the pixel format.
    ///
    /// This is similar to an `ImageBuffer` except it is a temporary view that is not normalized as
    /// strongly. To get an owning version, consider copying the data into an `ImageBuffer`. This
    /// provides many more operations, is possibly faster (if not you may want to open an issue) is
    /// generally polished. You can also try to convert this buffer inline, see
    /// `ImageBuffer::from_raw`.
    pub fn as_view_mut<P>(&mut self) -> Result<ViewMut<&mut [P::Subpixel], P>, Error>
    where
        P: Pixel,
        Buffer: AsMut<[P::Subpixel]>,
    {
        if !self.layout.is_normal(NormalForm::PixelPacked) {
            return Err(Error::NormalFormRequired(NormalForm::PixelPacked));
        }

        if self.layout.channels != P::CHANNEL_COUNT {
            return Err(Error::ChannelCountMismatch(
                self.layout.channels,
                P::CHANNEL_COUNT,
            ));
        }

        let as_mut = self.samples.as_mut();
        if !self.layout.fits(as_mut.len()) {
            return Err(Error::TooLarge);
        }

        Ok(ViewMut {
            inner: FlatSamples {
                samples: as_mut,
                layout: self.layout,
                color_hint: self.color_hint,
            },
            phantom: PhantomData,
        })
    }

    /// View the samples as a slice.
    ///
    /// The slice is not limited to the region of the image and not all sample indices are valid
    /// indices into this buffer. See `image_mut_slice` as an alternative.
    pub fn as_slice<T>(&self) -> &[T]
    where
        Buffer: AsRef<[T]>,
    {
        self.samples.as_ref()
    }

    /// View the samples as a slice.
    ///
    /// The slice is not limited to the region of the image and not all sample indices are valid
    /// indices into this buffer. See `image_mut_slice` as an alternative.
    pub fn as_mut_slice<T>(&mut self) -> &mut [T]
    where
        Buffer: AsMut<[T]>,
    {
        self.samples.as_mut()
    }

    /// Return the portion of the buffer that holds sample values.
    ///
    /// This may fail when the coordinates in this image are either out-of-bounds of the underlying
    /// buffer or can not be represented. Note that the slice may have holes that do not correspond
    /// to any sample in the image represented by it.
    pub fn image_slice<T>(&self) -> Option<&[T]>
    where
        Buffer: AsRef<[T]>,
    {
        let min_length = self.min_length()?;

        let slice = self.samples.as_ref();
        if slice.len() < min_length {
            return None;
        }

        Some(&slice[..min_length])
    }

    /// Mutable portion of the buffer that holds sample values.
    pub fn image_mut_slice<T>(&mut self) -> Option<&mut [T]>
    where
        Buffer: AsMut<[T]>,
    {
        let min_length = self.min_length()?;

        let slice = self.samples.as_mut();
        if slice.len() < min_length {
            return None;
        }

        Some(&mut slice[..min_length])
    }

    /// Move the data into an image buffer.
    ///
    /// This does **not** convert the sample layout. The buffer needs to be in packed row-major form
    /// before calling this function. In case of an error, returns the buffer again so that it does
    /// not release any allocation.
    pub fn try_into_buffer<P>(self) -> Result<ImageBuffer<P, Buffer>, (Error, Self)>
    where
        P: Pixel + 'static,
        P::Subpixel: 'static,
        Buffer: Deref<Target = [P::Subpixel]>,
    {
        if !self.is_normal(NormalForm::RowMajorPacked) {
            return Err((Error::NormalFormRequired(NormalForm::RowMajorPacked), self));
        }

        if self.layout.channels != P::CHANNEL_COUNT {
            return Err((
                Error::ChannelCountMismatch(self.layout.channels, P::CHANNEL_COUNT),
                self,
            ));
        }

        if !self.fits(self.samples.deref().len()) {
            return Err((Error::TooLarge, self));
        }

        Ok(
            ImageBuffer::from_raw(self.layout.width, self.layout.height, self.samples)
                .unwrap_or_else(|| {
                    panic!("Preconditions should have been ensured before conversion")
                }),
        )
    }

    /// Get the minimum length of a buffer such that all in-bounds samples have valid indices.
    ///
    /// This method will allow zero strides, allowing compact representations of monochrome images.
    /// To check that no aliasing occurs, try `check_alias_invariants`. For compact images (no
    /// aliasing and no unindexed samples) this is `width*height*channels`. But for both of the
    /// other cases, the reasoning is slightly more involved.
    ///
    /// # Explanation
    ///
    /// Note that there is a difference between `min_length` and the index of the sample
    /// 'one-past-the-end'. This is due to strides that may be larger than the dimension below.
    ///
    /// ## Example with holes
    ///
    /// Let's look at an example of a grayscale image with
    /// * `width_stride = 1`
    /// * `width = 2`
    /// * `height_stride = 3`
    /// * `height = 2`
    ///
    /// ```text
    /// | x x   | x x m | $
    ///  min_length m ^
    ///                   ^ one-past-the-end $
    /// ```
    ///
    /// The difference is also extreme for empty images with large strides. The one-past-the-end
    /// sample index is still as large as the largest of these strides while `min_length = 0`.
    ///
    /// ## Example with aliasing
    ///
    /// The concept gets even more important when you allow samples to alias each other. Here we
    /// have the buffer of a small grayscale image where this is the case, this time we will first
    /// show the buffer and then the individual rows below.
    ///
    /// * `width_stride = 1`
    /// * `width = 3`
    /// * `height_stride = 2`
    /// * `height = 2`
    ///
    /// ```text
    ///  1 2 3 4 5 m
    /// |1 2 3| row one
    ///     |3 4 5| row two
    ///            ^ m min_length
    ///          ^ ??? one-past-the-end
    /// ```
    ///
    /// This time 'one-past-the-end' is not even simply the largest stride times the extent of its
    /// dimension. That still points inside the image because `height*height_stride = 4` but also
    /// `index_of(1, 2) = 4`.
    pub fn min_length(&self) -> Option<usize> {
        self.layout.min_length()
    }

    /// Check if a buffer of length `len` is large enough.
    pub fn fits(&self, len: usize) -> bool {
        self.layout.fits(len)
    }

    /// If there are any samples aliasing each other.
    ///
    /// If this is not the case, it would always be safe to allow mutable access to two different
    /// samples at the same time. Otherwise, this operation would need additional checks. When one
    /// dimension overflows `usize` with its stride we also consider this aliasing.
    pub fn has_aliased_samples(&self) -> bool {
        self.layout.has_aliased_samples()
    }

    /// Check if a buffer fulfills the requirements of a normal form.
    ///
    /// Certain conversions have preconditions on the structure of the sample buffer that are not
    /// captured (by design) by the type system. These are then checked before the conversion. Such
    /// checks can all be done in constant time and will not inspect the buffer content. You can
    /// perform these checks yourself when the conversion is not required at this moment but maybe
    /// still performed later.
    pub fn is_normal(&self, form: NormalForm) -> bool {
        self.layout.is_normal(form)
    }

    /// Check that the pixel and the channel index are in bounds.
    ///
    /// An in-bound coordinate does not yet guarantee that the corresponding calculation of a
    /// buffer index does not overflow. However, if such a buffer large enough to hold all samples
    /// actually exists in memory, this property of course follows.
    pub fn in_bounds(&self, channel: u8, x: u32, y: u32) -> bool {
        self.layout.in_bounds(channel, x, y)
    }

    /// Resolve the index of a particular sample.
    ///
    /// `None` if the index is outside the bounds or does not fit into a `usize`.
    pub fn index(&self, channel: u8, x: u32, y: u32) -> Option<usize> {
        self.layout.index(channel, x, y)
    }

    /// Get the theoretical position of sample (x, y, channel).
    ///
    /// The 'check' is for overflow during index calculation, not that it is contained in the
    /// image. Two samples may return the same index, even when one of them is out of bounds. This
    /// happens when all strides are `0`, i.e. the image is an arbitrarily large monochrome image.
    pub fn index_ignoring_bounds(&self, channel: usize, x: usize, y: usize) -> Option<usize> {
        self.layout.index_ignoring_bounds(channel, x, y)
    }

    /// Get an index provided it is inbouds.
    ///
    /// Assumes that the image is backed by some sufficiently large buffer. Then computation can
    /// not overflow as we could represent the maximum coordinate. Since overflow is defined either
    /// way, this method can not be unsafe.
    pub fn in_bounds_index(&self, channel: u8, x: u32, y: u32) -> usize {
        self.layout.in_bounds_index(channel, x, y)
    }

    /// Shrink the image to the minimum of current and given extents.
    ///
    /// This does not modify the strides, so that the resulting sample buffer may have holes
    /// created by the shrinking operation. Shrinking could also lead to an non-aliasing image when
    /// samples had aliased each other before.
    pub fn shrink_to(&mut self, channels: u8, width: u32, height: u32) {
        self.layout.shrink_to(channels, width, height);
    }
}

impl<'buf, Subpixel> FlatSamples<&'buf [Subpixel]> {
    /// Create a monocolor image from a single pixel.
    ///
    /// This can be used as a very cheap source of a `GenericImageView` with an arbitrary number of
    /// pixels of a single color, without any dynamic allocation.
    ///
    /// ## Examples
    ///
    /// ```
    /// # fn paint_something<T>(_: T) {}
    /// use image::{flat::FlatSamples, GenericImage, RgbImage, Rgb};
    ///
    /// let background = Rgb([20, 20, 20]);
    /// let bg = FlatSamples::with_monocolor(&background, 200, 200);
    ///
    /// let mut image = RgbImage::new(200, 200);
    /// paint_something(&mut image);
    ///
    /// // Reset the canvas
    /// image.copy_from(&bg.as_view().unwrap(), 0, 0);
    /// ```
    pub fn with_monocolor<P>(pixel: &'buf P, width: u32, height: u32) -> Self
    where
        P: Pixel<Subpixel = Subpixel>,
        Subpixel: crate::Primitive,
    {
        FlatSamples {
            samples: pixel.channels(),
            layout: SampleLayout {
                channels: P::CHANNEL_COUNT,
                channel_stride: 1,
                width,
                width_stride: 0,
                height,
                height_stride: 0,
            },

            // TODO this value is never set. It should be set in all places where the Pixel type implements PixelWithColorType
            color_hint: None,
        }
    }
}

/// A flat buffer that can be used as an image view.
///
/// This is a nearly trivial wrapper around a buffer but at least sanitizes by checking the buffer
/// length first and constraining the pixel type.
///
/// Note that this does not eliminate panics as the `AsRef<[T]>` implementation of `Buffer` may be
/// unreliable, i.e. return different buffers at different times. This of course is a non-issue for
/// all common collections where the bounds check once must be enough.
///
/// # Inner invariants
///
/// * For all indices inside bounds, the corresponding index is valid in the buffer
/// * `P::channel_count()` agrees with `self.inner.layout.channels`
#[derive(Clone, Debug)]
pub struct View<Buffer, P: Pixel>
where
    Buffer: AsRef<[P::Subpixel]>,
{
    inner: FlatSamples<Buffer>,
    phantom: PhantomData<P>,
}

/// A mutable owning version of a flat buffer.
///
/// While this wraps a buffer similar to `ImageBuffer`, this is mostly intended as a utility. The
/// library endorsed normalized representation is still `ImageBuffer`. Also, the implementation of
/// `AsMut<[P::Subpixel]>` must always yield the same buffer. Therefore there is no public way to
/// construct this with an owning buffer.
///
/// # Inner invariants
///
/// * For all indices inside bounds, the corresponding index is valid in the buffer
/// * There is no aliasing of samples
/// * The samples are packed, i.e. `self.inner.layout.sample_stride == 1`
/// * `P::channel_count()` agrees with `self.inner.layout.channels`
#[derive(Clone, Debug)]
pub struct ViewMut<Buffer, P: Pixel>
where
    Buffer: AsMut<[P::Subpixel]>,
{
    inner: FlatSamples<Buffer>,
    phantom: PhantomData<P>,
}

/// Denotes invalid flat sample buffers when trying to convert to stricter types.
///
/// The biggest use case being `ImageBuffer` which expects closely packed
/// samples in a row major matrix representation. But this error type may be
/// reused for other import functions. A more versatile user may also try to
/// correct the underlying representation depending on the error variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Error {
    /// The represented image was too large.
    ///
    /// The optional value denotes a possibly accepted maximal bound.
    TooLarge,

    /// The represented image can not use this representation.
    ///
    /// Has an additional value of the normalized form that would be accepted.
    NormalFormRequired(NormalForm),

    /// The color format did not match the channel count.
    ///
    /// In some cases you might be able to fix this by lowering the reported pixel count of the
    /// buffer without touching the strides.
    ///
    /// In very special circumstances you *may* do the opposite. This is **VERY** dangerous but not
    /// directly memory unsafe although that will likely alias pixels. One scenario is when you
    /// want to construct an `Rgba` image but have only 3 bytes per pixel and for some reason don't
    /// care about the value of the alpha channel even though you need `Rgba`.
    ChannelCountMismatch(u8, u8),

    /// Deprecated - `ChannelCountMismatch` is used instead
    WrongColor(ColorType),
}

/// Different normal forms of buffers.
///
/// A normal form is an unaliased buffer with some additional constraints.  The `ÌmageBuffer` uses
/// row major form with packed samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NormalForm {
    /// No pixel aliases another.
    ///
    /// Unaliased also guarantees that all index calculations in the image bounds using
    /// `dim_index*dim_stride` (such as `x*width_stride + y*height_stride`) do not overflow.
    Unaliased,

    /// At least pixels are packed.
    ///
    /// Images of these types can wrap `[T]`-slices into the standard color types. This is a
    /// precondition for `GenericImage` which requires by-reference access to pixels.
    PixelPacked,

    /// All samples are packed.
    ///
    /// This is orthogonal to `PixelPacked`. It requires that there are no holes in the image but
    /// it is not necessary that the pixel samples themselves are adjacent. An example of this
    /// behaviour is a planar image layout.
    ImagePacked,

    /// The samples are in row-major form and all samples are packed.
    ///
    /// In addition to `PixelPacked` and `ImagePacked` this also asserts that the pixel matrix is
    /// in row-major form.
    RowMajorPacked,

    /// The samples are in column-major form and all samples are packed.
    ///
    /// In addition to `PixelPacked` and `ImagePacked` this also asserts that the pixel matrix is
    /// in column-major form.
    ColumnMajorPacked,
}

impl<Buffer, P: Pixel> View<Buffer, P>
where
    Buffer: AsRef<[P::Subpixel]>,
{
    /// Take out the sample buffer.
    ///
    /// Gives up the normalization invariants on the buffer format.
    pub fn into_inner(self) -> FlatSamples<Buffer> {
        self.inner
    }

    /// Get a reference on the inner sample descriptor.
    ///
    /// There is no mutable counterpart as modifying the buffer format, including strides and
    /// lengths, could invalidate the accessibility invariants of the `View`. It is not specified
    /// if the inner buffer is the same as the buffer of the image from which this view was
    /// created. It might have been truncated as an optimization.
    pub fn flat(&self) -> &FlatSamples<Buffer> {
        &self.inner
    }

    /// Get a reference on the inner buffer.
    ///
    /// There is no mutable counter part since it is not intended to allow you to reassign the
    /// buffer or otherwise change its size or properties.
    pub fn samples(&self) -> &Buffer {
        &self.inner.samples
    }

    /// Get a reference to a selected subpixel if it is in-bounds.
    ///
    /// This method will return `None` when the sample is out-of-bounds. All errors that could
    /// occur due to overflow have been eliminated while construction the `View`.
    pub fn get_sample(&self, channel: u8, x: u32, y: u32) -> Option<&P::Subpixel> {
        if !self.inner.in_bounds(channel, x, y) {
            return None;
        }

        let index = self.inner.in_bounds_index(channel, x, y);
        // Should always be `Some(_)` but checking is more costly.
        self.samples().as_ref().get(index)
    }

    /// Get a mutable reference to a selected subpixel if it is in-bounds.
    ///
    /// This is relevant only when constructed with `FlatSamples::as_view_with_mut_samples`.  This
    /// method will return `None` when the sample is out-of-bounds. All errors that could occur due
    /// to overflow have been eliminated while construction the `View`.
    ///
    /// **WARNING**: Note that of course samples may alias, so that the mutable reference returned
    /// here can in fact modify more than the coordinate in the argument.
    pub fn get_mut_sample(&mut self, channel: u8, x: u32, y: u32) -> Option<&mut P::Subpixel>
    where
        Buffer: AsMut<[P::Subpixel]>,
    {
        if !self.inner.in_bounds(channel, x, y) {
            return None;
        }

        let index = self.inner.in_bounds_index(channel, x, y);
        // Should always be `Some(_)` but checking is more costly.
        self.inner.samples.as_mut().get_mut(index)
    }

    /// Get the minimum length of a buffer such that all in-bounds samples have valid indices.
    ///
    /// See `FlatSamples::min_length`. This method will always succeed.
    pub fn min_length(&self) -> usize {
        self.inner.min_length().unwrap()
    }

    /// Return the portion of the buffer that holds sample values.
    ///
    /// While this can not fail–the validity of all coordinates has been validated during the
    /// conversion from `FlatSamples`–the resulting slice may still contain holes.
    pub fn image_slice(&self) -> &[P::Subpixel] {
        &self.samples().as_ref()[..self.min_length()]
    }

    /// Return the mutable portion of the buffer that holds sample values.
    ///
    /// This is relevant only when constructed with `FlatSamples::as_view_with_mut_samples`. While
    /// this can not fail–the validity of all coordinates has been validated during the conversion
    /// from `FlatSamples`–the resulting slice may still contain holes.
    pub fn image_mut_slice(&mut self) -> &mut [P::Subpixel]
    where
        Buffer: AsMut<[P::Subpixel]>,
    {
        let min_length = self.min_length();
        &mut self.inner.samples.as_mut()[..min_length]
    }

    /// Shrink the inner image.
    ///
    /// The new dimensions will be the minimum of the previous dimensions. Since the set of
    /// in-bounds pixels afterwards is a subset of the current ones, this is allowed on a `View`.
    /// Note that you can not change the number of channels as an intrinsic property of `P`.
    pub fn shrink_to(&mut self, width: u32, height: u32) {
        let channels = self.inner.layout.channels;
        self.inner.shrink_to(channels, width, height);
    }

    /// Try to convert this into an image with mutable pixels.
    ///
    /// The resulting image implements `GenericImage` in addition to `GenericImageView`. While this
    /// has mutable samples, it does not enforce that pixel can not alias and that samples are
    /// packed enough for a mutable pixel reference. This is slightly cheaper than the chain
    /// `self.into_inner().as_view_mut()` and keeps the `View` alive on failure.
    ///
    /// ```
    /// # use image::RgbImage;
    /// # use image::Rgb;
    /// let mut buffer = RgbImage::new(480, 640).into_flat_samples();
    /// let view = buffer.as_view_with_mut_samples::<Rgb<u8>>().unwrap();
    ///
    /// // Inspect some pixels, …
    ///
    /// // Doesn't fail because it was originally an `RgbImage`.
    /// let view_mut = view.try_upgrade().unwrap();
    /// ```
    pub fn try_upgrade(self) -> Result<ViewMut<Buffer, P>, (Error, Self)>
    where
        Buffer: AsMut<[P::Subpixel]>,
    {
        if !self.inner.is_normal(NormalForm::PixelPacked) {
            return Err((Error::NormalFormRequired(NormalForm::PixelPacked), self));
        }

        // No length check or channel count check required, all the same.
        Ok(ViewMut {
            inner: self.inner,
            phantom: PhantomData,
        })
    }
}

impl<Buffer, P: Pixel> ViewMut<Buffer, P>
where
    Buffer: AsMut<[P::Subpixel]>,
{
    /// Take out the sample buffer.
    ///
    /// Gives up the normalization invariants on the buffer format.
    pub fn into_inner(self) -> FlatSamples<Buffer> {
        self.inner
    }

    /// Get a reference on the sample buffer descriptor.
    ///
    /// There is no mutable counterpart as modifying the buffer format, including strides and
    /// lengths, could invalidate the accessibility invariants of the `View`. It is not specified
    /// if the inner buffer is the same as the buffer of the image from which this view was
    /// created. It might have been truncated as an optimization.
    pub fn flat(&self) -> &FlatSamples<Buffer> {
        &self.inner
    }

    /// Get a reference on the inner buffer.
    ///
    /// There is no mutable counter part since it is not intended to allow you to reassign the
    /// buffer or otherwise change its size or properties. However, its contents can be accessed
    /// mutable through a slice with `image_mut_slice`.
    pub fn samples(&self) -> &Buffer {
        &self.inner.samples
    }

    /// Get the minimum length of a buffer such that all in-bounds samples have valid indices.
    ///
    /// See `FlatSamples::min_length`. This method will always succeed.
    pub fn min_length(&self) -> usize {
        self.inner.min_length().unwrap()
    }

    /// Get a reference to a selected subpixel.
    ///
    /// This method will return `None` when the sample is out-of-bounds. All errors that could
    /// occur due to overflow have been eliminated while construction the `View`.
    pub fn get_sample(&self, channel: u8, x: u32, y: u32) -> Option<&P::Subpixel>
    where
        Buffer: AsRef<[P::Subpixel]>,
    {
        if !self.inner.in_bounds(channel, x, y) {
            return None;
        }

        let index = self.inner.in_bounds_index(channel, x, y);
        // Should always be `Some(_)` but checking is more costly.
        self.samples().as_ref().get(index)
    }

    /// Get a mutable reference to a selected sample.
    ///
    /// This method will return `None` when the sample is out-of-bounds. All errors that could
    /// occur due to overflow have been eliminated while construction the `View`.
    pub fn get_mut_sample(&mut self, channel: u8, x: u32, y: u32) -> Option<&mut P::Subpixel> {
        if !self.inner.in_bounds(channel, x, y) {
            return None;
        }

        let index = self.inner.in_bounds_index(channel, x, y);
        // Should always be `Some(_)` but checking is more costly.
        self.inner.samples.as_mut().get_mut(index)
    }

    /// Return the portion of the buffer that holds sample values.
    ///
    /// While this can not fail–the validity of all coordinates has been validated during the
    /// conversion from `FlatSamples`–the resulting slice may still contain holes.
    pub fn image_slice(&self) -> &[P::Subpixel]
    where
        Buffer: AsRef<[P::Subpixel]>,
    {
        &self.inner.samples.as_ref()[..self.min_length()]
    }

    /// Return the mutable buffer that holds sample values.
    pub fn image_mut_slice(&mut self) -> &mut [P::Subpixel] {
        let length = self.min_length();
        &mut self.inner.samples.as_mut()[..length]
    }

    /// Shrink the inner image.
    ///
    /// The new dimensions will be the minimum of the previous dimensions. Since the set of
    /// in-bounds pixels afterwards is a subset of the current ones, this is allowed on a `View`.
    /// Note that you can not change the number of channels as an intrinsic property of `P`.
    pub fn shrink_to(&mut self, width: u32, height: u32) {
        let channels = self.inner.layout.channels;
        self.inner.shrink_to(channels, width, height);
    }
}

// The out-of-bounds panic for single sample access similar to `slice::index`.
#[inline(never)]
#[cold]
fn panic_cwh_out_of_bounds(
    (c, x, y): (u8, u32, u32),
    bounds: (u8, u32, u32),
    strides: (usize, usize, usize),
) -> ! {
    panic!(
        "Sample coordinates {:?} out of sample matrix bounds {:?} with strides {:?}",
        (c, x, y),
        bounds,
        strides
    )
}

// The out-of-bounds panic for pixel access similar to `slice::index`.
#[inline(never)]
#[cold]
fn panic_pixel_out_of_bounds((x, y): (u32, u32), bounds: (u32, u32)) -> ! {
    panic!("Image index {:?} out of bounds {:?}", (x, y), bounds)
}

impl<Buffer> Index<(u8, u32, u32)> for FlatSamples<Buffer>
where
    Buffer: Index<usize>,
{
    type Output = Buffer::Output;

    /// Return a reference to a single sample at specified coordinates.
    ///
    /// # Panics
    ///
    /// When the coordinates are out of bounds or the index calculation fails.
    fn index(&self, (c, x, y): (u8, u32, u32)) -> &Self::Output {
        let bounds = self.bounds();
        let strides = self.strides_cwh();
        let index = self
            .index(c, x, y)
            .unwrap_or_else(|| panic_cwh_out_of_bounds((c, x, y), bounds, strides));
        &self.samples[index]
    }
}

impl<Buffer> IndexMut<(u8, u32, u32)> for FlatSamples<Buffer>
where
    Buffer: IndexMut<usize>,
{
    /// Return a mutable reference to a single sample at specified coordinates.
    ///
    /// # Panics
    ///
    /// When the coordinates are out of bounds or the index calculation fails.
    fn index_mut(&mut self, (c, x, y): (u8, u32, u32)) -> &mut Self::Output {
        let bounds = self.bounds();
        let strides = self.strides_cwh();
        let index = self
            .index(c, x, y)
            .unwrap_or_else(|| panic_cwh_out_of_bounds((c, x, y), bounds, strides));
        &mut self.samples[index]
    }
}

impl<Buffer, P: Pixel> GenericImageView for View<Buffer, P>
where
    Buffer: AsRef<[P::Subpixel]>,
{
    type Pixel = P;

    fn dimensions(&self) -> (u32, u32) {
        (self.inner.layout.width, self.inner.layout.height)
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        if !self.inner.in_bounds(0, x, y) {
            panic_pixel_out_of_bounds((x, y), self.dimensions())
        }

        let image = self.inner.samples.as_ref();
        let base_index = self.inner.in_bounds_index(0, x, y);
        let channels = P::CHANNEL_COUNT as usize;

        let mut buffer = [Zero::zero(); 256];
        buffer
            .iter_mut()
            .enumerate()
            .take(channels)
            .for_each(|(c, to)| {
                let index = base_index + c * self.inner.layout.channel_stride;
                *to = image[index];
            });

        *P::from_slice(&buffer[..channels])
    }
}

impl<Buffer, P: Pixel> GenericImageView for ViewMut<Buffer, P>
where
    Buffer: AsMut<[P::Subpixel]> + AsRef<[P::Subpixel]>,
{
    type Pixel = P;

    fn dimensions(&self) -> (u32, u32) {
        (self.inner.layout.width, self.inner.layout.height)
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        if !self.inner.in_bounds(0, x, y) {
            panic_pixel_out_of_bounds((x, y), self.dimensions())
        }

        let image = self.inner.samples.as_ref();
        let base_index = self.inner.in_bounds_index(0, x, y);
        let channels = P::CHANNEL_COUNT as usize;

        let mut buffer = [Zero::zero(); 256];
        buffer
            .iter_mut()
            .enumerate()
            .take(channels)
            .for_each(|(c, to)| {
                let index = base_index + c * self.inner.layout.channel_stride;
                *to = image[index];
            });

        *P::from_slice(&buffer[..channels])
    }
}

impl<Buffer, P: Pixel> GenericImage for ViewMut<Buffer, P>
where
    Buffer: AsMut<[P::Subpixel]> + AsRef<[P::Subpixel]>,
{
    fn get_pixel_mut(&mut self, x: u32, y: u32) -> &mut Self::Pixel {
        if !self.inner.in_bounds(0, x, y) {
            panic_pixel_out_of_bounds((x, y), self.dimensions())
        }

        let base_index = self.inner.in_bounds_index(0, x, y);
        let channel_count = <P as Pixel>::CHANNEL_COUNT as usize;
        let pixel_range = base_index..base_index + channel_count;
        P::from_slice_mut(&mut self.inner.samples.as_mut()[pixel_range])
    }

    #[allow(deprecated)]
    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        *self.get_pixel_mut(x, y) = pixel;
    }

    #[allow(deprecated)]
    fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        self.get_pixel_mut(x, y).blend(&pixel);
    }
}

impl From<Error> for ImageError {
    fn from(error: Error) -> ImageError {
        #[derive(Debug)]
        struct NormalFormRequiredError(NormalForm);
        impl fmt::Display for NormalFormRequiredError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "Required sample buffer in normal form {:?}", self.0)
            }
        }
        impl error::Error for NormalFormRequiredError {}

        match error {
            Error::TooLarge => ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::DimensionMismatch,
            )),
            Error::NormalFormRequired(form) => ImageError::Decoding(DecodingError::new(
                ImageFormatHint::Unknown,
                NormalFormRequiredError(form),
            )),
            Error::ChannelCountMismatch(_lc, _pc) => ImageError::Parameter(
                ParameterError::from_kind(ParameterErrorKind::DimensionMismatch),
            ),
            Error::WrongColor(color) => {
                ImageError::Unsupported(UnsupportedError::from_format_and_kind(
                    ImageFormatHint::Unknown,
                    UnsupportedErrorKind::Color(color.into()),
                ))
            }
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::TooLarge => write!(f, "The layout is too large"),
            Error::NormalFormRequired(form) => write!(
                f,
                "The layout needs to {}",
                match form {
                    NormalForm::ColumnMajorPacked => "be packed and in column major form",
                    NormalForm::ImagePacked => "be fully packed",
                    NormalForm::PixelPacked => "have packed pixels",
                    NormalForm::RowMajorPacked => "be packed and in row major form",
                    NormalForm::Unaliased => "not have any aliasing channels",
                }
            ),
            Error::ChannelCountMismatch(layout_channels, pixel_channels) => {
                write!(f, "The channel count of the chosen pixel (={pixel_channels}) does agree with the layout (={layout_channels})")
            }
            Error::WrongColor(color) => {
                write!(f, "The chosen color type does not match the hint {color:?}")
            }
        }
    }
}

impl error::Error for Error {}

impl PartialOrd for NormalForm {
    /// Compares the logical preconditions.
    ///
    /// `a < b` if the normal form `a` has less preconditions than `b`.
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        match (*self, *other) {
            (NormalForm::Unaliased, NormalForm::Unaliased) => Some(cmp::Ordering::Equal),
            (NormalForm::PixelPacked, NormalForm::PixelPacked) => Some(cmp::Ordering::Equal),
            (NormalForm::ImagePacked, NormalForm::ImagePacked) => Some(cmp::Ordering::Equal),
            (NormalForm::RowMajorPacked, NormalForm::RowMajorPacked) => Some(cmp::Ordering::Equal),
            (NormalForm::ColumnMajorPacked, NormalForm::ColumnMajorPacked) => {
                Some(cmp::Ordering::Equal)
            }

            (NormalForm::Unaliased, _) => Some(cmp::Ordering::Less),
            (_, NormalForm::Unaliased) => Some(cmp::Ordering::Greater),

            (NormalForm::PixelPacked, NormalForm::ColumnMajorPacked) => Some(cmp::Ordering::Less),
            (NormalForm::PixelPacked, NormalForm::RowMajorPacked) => Some(cmp::Ordering::Less),
            (NormalForm::RowMajorPacked, NormalForm::PixelPacked) => Some(cmp::Ordering::Greater),
            (NormalForm::ColumnMajorPacked, NormalForm::PixelPacked) => {
                Some(cmp::Ordering::Greater)
            }

            (NormalForm::ImagePacked, NormalForm::ColumnMajorPacked) => Some(cmp::Ordering::Less),
            (NormalForm::ImagePacked, NormalForm::RowMajorPacked) => Some(cmp::Ordering::Less),
            (NormalForm::RowMajorPacked, NormalForm::ImagePacked) => Some(cmp::Ordering::Greater),
            (NormalForm::ColumnMajorPacked, NormalForm::ImagePacked) => {
                Some(cmp::Ordering::Greater)
            }

            (NormalForm::ImagePacked, NormalForm::PixelPacked) => None,
            (NormalForm::PixelPacked, NormalForm::ImagePacked) => None,
            (NormalForm::RowMajorPacked, NormalForm::ColumnMajorPacked) => None,
            (NormalForm::ColumnMajorPacked, NormalForm::RowMajorPacked) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{LumaA, Rgb};
    use crate::images::buffer::GrayAlphaImage;

    #[test]
    fn aliasing_view() {
        let buffer = FlatSamples {
            samples: &[42],
            layout: SampleLayout {
                channels: 3,
                channel_stride: 0,
                width: 100,
                width_stride: 0,
                height: 100,
                height_stride: 0,
            },
            color_hint: None,
        };

        let view = buffer.as_view::<Rgb<u8>>().expect("This is a valid view");
        let pixel_count = view
            .pixels()
            .inspect(|pixel| assert!(pixel.2 == Rgb([42, 42, 42])))
            .count();
        assert_eq!(pixel_count, 100 * 100);
    }

    #[test]
    fn mutable_view() {
        let mut buffer = FlatSamples {
            samples: [0; 18],
            layout: SampleLayout {
                channels: 2,
                channel_stride: 1,
                width: 3,
                width_stride: 2,
                height: 3,
                height_stride: 6,
            },
            color_hint: None,
        };

        {
            let mut view = buffer
                .as_view_mut::<LumaA<u16>>()
                .expect("This should be a valid mutable buffer");
            assert_eq!(view.dimensions(), (3, 3));
            #[allow(deprecated)]
            for i in 0..9 {
                *view.get_pixel_mut(i % 3, i / 3) = LumaA([2 * i as u16, 2 * i as u16 + 1]);
            }
        }

        buffer
            .samples
            .iter()
            .enumerate()
            .for_each(|(idx, sample)| assert_eq!(idx, *sample as usize));
    }

    #[test]
    fn normal_forms() {
        assert!(FlatSamples {
            samples: [0u8; 0],
            layout: SampleLayout {
                channels: 2,
                channel_stride: 1,
                width: 3,
                width_stride: 9,
                height: 3,
                height_stride: 28,
            },
            color_hint: None,
        }
        .is_normal(NormalForm::PixelPacked));

        assert!(FlatSamples {
            samples: [0u8; 0],
            layout: SampleLayout {
                channels: 2,
                channel_stride: 8,
                width: 4,
                width_stride: 1,
                height: 2,
                height_stride: 4,
            },
            color_hint: None,
        }
        .is_normal(NormalForm::ImagePacked));

        assert!(FlatSamples {
            samples: [0u8; 0],
            layout: SampleLayout {
                channels: 2,
                channel_stride: 1,
                width: 4,
                width_stride: 2,
                height: 2,
                height_stride: 8,
            },
            color_hint: None,
        }
        .is_normal(NormalForm::RowMajorPacked));

        assert!(FlatSamples {
            samples: [0u8; 0],
            layout: SampleLayout {
                channels: 2,
                channel_stride: 1,
                width: 4,
                width_stride: 4,
                height: 2,
                height_stride: 2,
            },
            color_hint: None,
        }
        .is_normal(NormalForm::ColumnMajorPacked));
    }

    #[test]
    fn image_buffer_conversion() {
        let expected_layout = SampleLayout {
            channels: 2,
            channel_stride: 1,
            width: 4,
            width_stride: 2,
            height: 2,
            height_stride: 8,
        };

        let initial = GrayAlphaImage::new(expected_layout.width, expected_layout.height);
        let buffer = initial.into_flat_samples();

        assert_eq!(buffer.layout, expected_layout);

        let _: GrayAlphaImage = buffer
            .try_into_buffer()
            .unwrap_or_else(|(error, _)| panic!("Expected buffer to be convertible but {error:?}"));
    }
}
