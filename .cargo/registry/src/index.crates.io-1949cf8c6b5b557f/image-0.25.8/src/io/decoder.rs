use crate::animation::Frames;
use crate::color::{ColorType, ExtendedColorType};
use crate::error::ImageResult;
use crate::metadata::Orientation;

/// The trait that all decoders implement
pub trait ImageDecoder {
    /// Returns a tuple containing the width and height of the image
    fn dimensions(&self) -> (u32, u32);

    /// Returns the color type of the image data produced by this decoder
    fn color_type(&self) -> ColorType;

    /// Returns the color type of the image file before decoding
    fn original_color_type(&self) -> ExtendedColorType {
        self.color_type().into()
    }

    /// Returns the ICC color profile embedded in the image, or `Ok(None)` if the image does not have one.
    ///
    /// For formats that don't support embedded profiles this function should always return `Ok(None)`.
    fn icc_profile(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(None)
    }

    /// Returns the raw [Exif](https://en.wikipedia.org/wiki/Exif) chunk, if it is present.
    /// A third-party crate such as [`kamadak-exif`](https://docs.rs/kamadak-exif/) is required to actually parse it.
    ///
    /// For formats that don't support embedded profiles this function should always return `Ok(None)`.
    fn exif_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(None)
    }

    /// Returns the orientation of the image.
    ///
    /// This is usually obtained from the Exif metadata, if present. Formats that don't support
    /// indicating orientation in their image metadata will return `Ok(Orientation::NoTransforms)`.
    fn orientation(&mut self) -> ImageResult<Orientation> {
        Ok(self
            .exif_metadata()?
            .and_then(|chunk| Orientation::from_exif_chunk(&chunk))
            .unwrap_or(Orientation::NoTransforms))
    }

    /// Returns the total number of bytes in the decoded image.
    ///
    /// This is the size of the buffer that must be passed to `read_image` or
    /// `read_image_with_progress`. The returned value may exceed `usize::MAX`, in
    /// which case it isn't actually possible to construct a buffer to decode all the image data
    /// into. If, however, the size does not fit in a u64 then `u64::MAX` is returned.
    fn total_bytes(&self) -> u64 {
        let dimensions = self.dimensions();
        let total_pixels = u64::from(dimensions.0) * u64::from(dimensions.1);
        let bytes_per_pixel = u64::from(self.color_type().bytes_per_pixel());
        total_pixels.saturating_mul(bytes_per_pixel)
    }

    /// Returns all the bytes in the image.
    ///
    /// This function takes a slice of bytes and writes the pixel data of the image into it.
    /// `buf` does not need to be aligned to any byte boundaries. However,
    /// alignment to 2 or 4 byte boundaries may result in small performance
    /// improvements for certain decoder implementations.
    ///
    /// The returned pixel data will always be in native endian. This allows
    /// `[u16]` and `[f32]` slices to be cast to `[u8]` and used for this method.
    ///
    /// # Panics
    ///
    /// This function panics if `buf.len() != self.total_bytes()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use image::ImageDecoder;
    /// fn read_16bit_image(decoder: impl ImageDecoder) -> Vec<u16> {
    ///     let mut buf: Vec<u16> = vec![0; (decoder.total_bytes() / 2) as usize];
    ///     decoder.read_image(bytemuck::cast_slice_mut(&mut buf));
    ///     buf
    /// }
    /// ```
    fn read_image(self, buf: &mut [u8]) -> ImageResult<()>
    where
        Self: Sized;

    /// Set the decoder to have the specified limits. See [`Limits`] for the different kinds of
    /// limits that is possible to set.
    ///
    /// Note to implementors: make sure you call [`Limits::check_support`] so that
    /// decoding fails if any unsupported strict limits are set. Also make sure
    /// you call [`Limits::check_dimensions`] to check the `max_image_width` and
    /// `max_image_height` limits.
    ///
    /// **Note**: By default, _no_ limits are defined. This may be changed in future major version
    /// increases.
    ///
    /// [`Limits`]: ./io/struct.Limits.html
    /// [`Limits::check_support`]: ./io/struct.Limits.html#method.check_support
    /// [`Limits::check_dimensions`]: ./io/struct.Limits.html#method.check_dimensions
    fn set_limits(&mut self, limits: crate::Limits) -> ImageResult<()> {
        limits.check_support(&crate::LimitSupport::default())?;
        let (width, height) = self.dimensions();
        limits.check_dimensions(width, height)?;
        Ok(())
    }

    /// Use `read_image` instead; this method is an implementation detail needed so the trait can
    /// be object safe.
    ///
    /// Note to implementors: This method should be implemented by calling `read_image` on
    /// the boxed decoder...
    /// ```ignore
    /// fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
    ///     (*self).read_image(buf)
    /// }
    /// ```
    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()>;
}

#[deny(clippy::missing_trait_methods)]
impl<T: ?Sized + ImageDecoder> ImageDecoder for Box<T> {
    fn dimensions(&self) -> (u32, u32) {
        (**self).dimensions()
    }
    fn color_type(&self) -> ColorType {
        (**self).color_type()
    }
    fn original_color_type(&self) -> ExtendedColorType {
        (**self).original_color_type()
    }
    fn icc_profile(&mut self) -> ImageResult<Option<Vec<u8>>> {
        (**self).icc_profile()
    }
    fn exif_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        (**self).exif_metadata()
    }
    fn orientation(&mut self) -> ImageResult<Orientation> {
        (**self).orientation()
    }
    fn total_bytes(&self) -> u64 {
        (**self).total_bytes()
    }
    fn read_image(self, buf: &mut [u8]) -> ImageResult<()>
    where
        Self: Sized,
    {
        T::read_image_boxed(self, buf)
    }
    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        T::read_image_boxed(*self, buf)
    }
    fn set_limits(&mut self, limits: crate::Limits) -> ImageResult<()> {
        (**self).set_limits(limits)
    }
}

/// Specialized image decoding not be supported by all formats
pub trait ImageDecoderRect: ImageDecoder {
    /// Decode a rectangular section of the image.
    ///
    /// This function takes a slice of bytes and writes the pixel data of the image into it.
    /// The rectangle is specified by the x and y coordinates of the top left corner, the width
    /// and height of the rectangle, and the row pitch of the buffer. The row pitch is the number
    /// of bytes between the start of one row and the start of the next row. The row pitch must be
    /// at least as large as the width of the rectangle in bytes.
    fn read_rect(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        buf: &mut [u8],
        row_pitch: usize,
    ) -> ImageResult<()>;
}

/// `AnimationDecoder` trait
pub trait AnimationDecoder<'a> {
    /// Consume the decoder producing a series of frames.
    fn into_frames(self) -> Frames<'a>;
}

#[cfg(test)]
mod tests {
    use super::{ColorType, ImageDecoder, ImageResult};

    #[test]
    fn total_bytes_overflow() {
        struct D;
        impl ImageDecoder for D {
            fn color_type(&self) -> ColorType {
                ColorType::Rgb8
            }
            fn dimensions(&self) -> (u32, u32) {
                (0xffff_ffff, 0xffff_ffff)
            }
            fn read_image(self, _buf: &mut [u8]) -> ImageResult<()> {
                unimplemented!()
            }
            fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
                (*self).read_image(buf)
            }
        }
        assert_eq!(D.total_bytes(), u64::MAX);

        let v: ImageResult<Vec<u8>> = crate::io::free_functions::decoder_to_vec(D);
        assert!(v.is_err());
    }
}
