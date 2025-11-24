//! Decoding and Encoding of PNG Images
//!
//! PNG (Portable Network Graphics) is an image format that supports lossless compression.
//!
//! # Related Links
//! * <http://www.w3.org/TR/PNG/> - The PNG Specification

use std::borrow::Cow;
use std::io::{BufRead, Seek, Write};

use png::{BlendOp, DisposeOp};

use crate::animation::{Delay, Frame, Frames, Ratio};
use crate::color::{Blend, ColorType, ExtendedColorType};
use crate::error::{
    DecodingError, ImageError, ImageResult, LimitError, LimitErrorKind, ParameterError,
    ParameterErrorKind, UnsupportedError, UnsupportedErrorKind,
};
use crate::utils::vec_try_with_capacity;
use crate::{
    AnimationDecoder, DynamicImage, GenericImage, GenericImageView, ImageBuffer, ImageDecoder,
    ImageEncoder, ImageFormat, Limits, Luma, LumaA, Rgb, Rgba, RgbaImage,
};

// http://www.w3.org/TR/PNG-Structure.html
// The first eight bytes of a PNG file always contain the following (decimal) values:
pub(crate) const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

/// PNG decoder
pub struct PngDecoder<R: BufRead + Seek> {
    color_type: ColorType,
    reader: png::Reader<R>,
    limits: Limits,
}

impl<R: BufRead + Seek> PngDecoder<R> {
    /// Creates a new decoder that decodes from the stream ```r```
    pub fn new(r: R) -> ImageResult<PngDecoder<R>> {
        Self::with_limits(r, Limits::no_limits())
    }

    /// Creates a new decoder that decodes from the stream ```r``` with the given limits.
    pub fn with_limits(r: R, limits: Limits) -> ImageResult<PngDecoder<R>> {
        limits.check_support(&crate::LimitSupport::default())?;

        let max_bytes = usize::try_from(limits.max_alloc.unwrap_or(u64::MAX)).unwrap_or(usize::MAX);
        let mut decoder = png::Decoder::new_with_limits(r, png::Limits { bytes: max_bytes });

        let info = decoder.read_header_info().map_err(ImageError::from_png)?;
        limits.check_dimensions(info.width, info.height)?;

        // By default the PNG decoder will scale 16 bpc to 8 bpc, so custom
        // transformations must be set. EXPAND preserves the default behavior
        // expanding bpc < 8 to 8 bpc.
        decoder.set_transformations(png::Transformations::EXPAND);
        let reader = decoder.read_info().map_err(ImageError::from_png)?;
        let (color_type, bits) = reader.output_color_type();
        let color_type = match (color_type, bits) {
            (png::ColorType::Grayscale, png::BitDepth::Eight) => ColorType::L8,
            (png::ColorType::Grayscale, png::BitDepth::Sixteen) => ColorType::L16,
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight) => ColorType::La8,
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Sixteen) => ColorType::La16,
            (png::ColorType::Rgb, png::BitDepth::Eight) => ColorType::Rgb8,
            (png::ColorType::Rgb, png::BitDepth::Sixteen) => ColorType::Rgb16,
            (png::ColorType::Rgba, png::BitDepth::Eight) => ColorType::Rgba8,
            (png::ColorType::Rgba, png::BitDepth::Sixteen) => ColorType::Rgba16,

            (png::ColorType::Grayscale, png::BitDepth::One) => {
                return Err(unsupported_color(ExtendedColorType::L1))
            }
            (png::ColorType::GrayscaleAlpha, png::BitDepth::One) => {
                return Err(unsupported_color(ExtendedColorType::La1))
            }
            (png::ColorType::Rgb, png::BitDepth::One) => {
                return Err(unsupported_color(ExtendedColorType::Rgb1))
            }
            (png::ColorType::Rgba, png::BitDepth::One) => {
                return Err(unsupported_color(ExtendedColorType::Rgba1))
            }

            (png::ColorType::Grayscale, png::BitDepth::Two) => {
                return Err(unsupported_color(ExtendedColorType::L2))
            }
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Two) => {
                return Err(unsupported_color(ExtendedColorType::La2))
            }
            (png::ColorType::Rgb, png::BitDepth::Two) => {
                return Err(unsupported_color(ExtendedColorType::Rgb2))
            }
            (png::ColorType::Rgba, png::BitDepth::Two) => {
                return Err(unsupported_color(ExtendedColorType::Rgba2))
            }

            (png::ColorType::Grayscale, png::BitDepth::Four) => {
                return Err(unsupported_color(ExtendedColorType::L4))
            }
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Four) => {
                return Err(unsupported_color(ExtendedColorType::La4))
            }
            (png::ColorType::Rgb, png::BitDepth::Four) => {
                return Err(unsupported_color(ExtendedColorType::Rgb4))
            }
            (png::ColorType::Rgba, png::BitDepth::Four) => {
                return Err(unsupported_color(ExtendedColorType::Rgba4))
            }

            (png::ColorType::Indexed, bits) => {
                return Err(unsupported_color(ExtendedColorType::Unknown(bits as u8)))
            }
        };

        Ok(PngDecoder {
            color_type,
            reader,
            limits,
        })
    }

    /// Returns the gamma value of the image or None if no gamma value is indicated.
    ///
    /// If an sRGB chunk is present this method returns a gamma value of 0.45455 and ignores the
    /// value in the gAMA chunk. This is the recommended behavior according to the PNG standard:
    ///
    /// > When the sRGB chunk is present, [...] decoders that recognize the sRGB chunk but are not
    /// > capable of colour management are recommended to ignore the gAMA and cHRM chunks, and use
    /// > the values given above as if they had appeared in gAMA and cHRM chunks.
    pub fn gamma_value(&self) -> ImageResult<Option<f64>> {
        Ok(self
            .reader
            .info()
            .source_gamma
            .map(|x| f64::from(x.into_scaled()) / 100_000.0))
    }

    /// Turn this into an iterator over the animation frames.
    ///
    /// Reading the complete animation requires more memory than reading the data from the IDAT
    /// frame–multiple frame buffers need to be reserved at the same time. We further do not
    /// support compositing 16-bit colors. In any case this would be lossy as the interface of
    /// animation decoders does not support 16-bit colors.
    ///
    /// If something is not supported or a limit is violated then the decoding step that requires
    /// them will fail and an error will be returned instead of the frame. No further frames will
    /// be returned.
    pub fn apng(self) -> ImageResult<ApngDecoder<R>> {
        Ok(ApngDecoder::new(self))
    }

    /// Returns if the image contains an animation.
    ///
    /// Note that the file itself decides if the default image is considered to be part of the
    /// animation. When it is not the common interpretation is to use it as a thumbnail.
    ///
    /// If a non-animated image is converted into an `ApngDecoder` then its iterator is empty.
    pub fn is_apng(&self) -> ImageResult<bool> {
        Ok(self.reader.info().animation_control.is_some())
    }
}

fn unsupported_color(ect: ExtendedColorType) -> ImageError {
    ImageError::Unsupported(UnsupportedError::from_format_and_kind(
        ImageFormat::Png.into(),
        UnsupportedErrorKind::Color(ect),
    ))
}

impl<R: BufRead + Seek> ImageDecoder for PngDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        self.reader.info().size()
    }

    fn color_type(&self) -> ColorType {
        self.color_type
    }

    fn icc_profile(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(self.reader.info().icc_profile.as_ref().map(|x| x.to_vec()))
    }

    fn exif_metadata(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(self
            .reader
            .info()
            .exif_metadata
            .as_ref()
            .map(|x| x.to_vec()))
    }

    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        use byteorder_lite::{BigEndian, ByteOrder, NativeEndian};

        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));
        self.reader.next_frame(buf).map_err(ImageError::from_png)?;
        // PNG images are big endian. For 16 bit per channel and larger types,
        // the buffer may need to be reordered to native endianness per the
        // contract of `read_image`.
        // TODO: assumes equal channel bit depth.
        let bpc = self.color_type().bytes_per_pixel() / self.color_type().channel_count();

        match bpc {
            1 => (), // No reodering necessary for u8
            2 => buf.chunks_exact_mut(2).for_each(|c| {
                let v = BigEndian::read_u16(c);
                NativeEndian::write_u16(c, v);
            }),
            _ => unreachable!(),
        }
        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }

    fn set_limits(&mut self, limits: Limits) -> ImageResult<()> {
        limits.check_support(&crate::LimitSupport::default())?;
        let info = self.reader.info();
        limits.check_dimensions(info.width, info.height)?;
        self.limits = limits;
        // TODO: add `png::Reader::change_limits()` and call it here
        // to also constrain the internal buffer allocations in the PNG crate
        Ok(())
    }
}

/// An [`AnimationDecoder`] adapter of [`PngDecoder`].
///
/// See [`PngDecoder::apng`] for more information.
///
/// [`AnimationDecoder`]: ../trait.AnimationDecoder.html
/// [`PngDecoder`]: struct.PngDecoder.html
/// [`PngDecoder::apng`]: struct.PngDecoder.html#method.apng
pub struct ApngDecoder<R: BufRead + Seek> {
    inner: PngDecoder<R>,
    /// The current output buffer.
    current: Option<RgbaImage>,
    /// The previous output buffer, used for dispose op previous.
    previous: Option<RgbaImage>,
    /// The dispose op of the current frame.
    dispose: DisposeOp,

    /// The region to dispose of the previous frame.
    dispose_region: Option<(u32, u32, u32, u32)>,
    /// The number of image still expected to be able to load.
    remaining: u32,
    /// The next (first) image is the thumbnail.
    has_thumbnail: bool,
}

impl<R: BufRead + Seek> ApngDecoder<R> {
    fn new(inner: PngDecoder<R>) -> Self {
        let info = inner.reader.info();
        let remaining = match info.animation_control() {
            // The expected number of fcTL in the remaining image.
            Some(actl) => actl.num_frames,
            None => 0,
        };
        // If the IDAT has no fcTL then it is not part of the animation counted by
        // num_frames. All following fdAT chunks must be preceded by an fcTL
        let has_thumbnail = info.frame_control.is_none();
        ApngDecoder {
            inner,
            current: None,
            previous: None,
            dispose: DisposeOp::Background,
            dispose_region: None,
            remaining,
            has_thumbnail,
        }
    }

    // TODO: thumbnail(&mut self) -> Option<impl ImageDecoder<'_>>

    /// Decode one subframe and overlay it on the canvas.
    fn mix_next_frame(&mut self) -> Result<Option<&RgbaImage>, ImageError> {
        // The iterator always produces RGBA8 images
        const COLOR_TYPE: ColorType = ColorType::Rgba8;

        // Allocate the buffers, honoring the memory limits
        let (width, height) = self.inner.dimensions();
        {
            let limits = &mut self.inner.limits;
            if self.previous.is_none() {
                limits.reserve_buffer(width, height, COLOR_TYPE)?;
                self.previous = Some(RgbaImage::new(width, height));
            }

            if self.current.is_none() {
                limits.reserve_buffer(width, height, COLOR_TYPE)?;
                self.current = Some(RgbaImage::new(width, height));
            }
        }

        // Remove this image from remaining.
        self.remaining = match self.remaining.checked_sub(1) {
            None => return Ok(None),
            Some(next) => next,
        };

        // Shorten ourselves to 0 in case of error.
        let remaining = self.remaining;
        self.remaining = 0;

        // Skip the thumbnail that is not part of the animation.
        if self.has_thumbnail {
            // Clone the limits so that our one-off allocation that's destroyed after this scope doesn't persist
            let mut limits = self.inner.limits.clone();

            let buffer_size = self.inner.reader.output_buffer_size().ok_or_else(|| {
                ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory))
            })?;

            limits.reserve_usize(buffer_size)?;
            let mut buffer = vec![0; buffer_size];
            // TODO: add `png::Reader::change_limits()` and call it here
            // to also constrain the internal buffer allocations in the PNG crate
            self.inner
                .reader
                .next_frame(&mut buffer)
                .map_err(ImageError::from_png)?;
            self.has_thumbnail = false;
        }

        self.animatable_color_type()?;

        // We've initialized them earlier in this function
        let previous = self.previous.as_mut().unwrap();
        let current = self.current.as_mut().unwrap();

        // Dispose of the previous frame.

        match self.dispose {
            DisposeOp::None => {
                previous.clone_from(current);
            }
            DisposeOp::Background => {
                previous.clone_from(current);
                if let Some((px, py, width, height)) = self.dispose_region {
                    let mut region_current = current.sub_image(px, py, width, height);

                    // FIXME: This is a workaround for the fact that `pixels_mut` is not implemented
                    let pixels: Vec<_> = region_current.pixels().collect();

                    for (x, y, _) in &pixels {
                        region_current.put_pixel(*x, *y, Rgba::from([0, 0, 0, 0]));
                    }
                } else {
                    // The first frame is always a background frame.
                    current.pixels_mut().for_each(|pixel| {
                        *pixel = Rgba::from([0, 0, 0, 0]);
                    });
                }
            }
            DisposeOp::Previous => {
                let (px, py, width, height) = self
                    .dispose_region
                    .expect("The first frame must not set dispose=Previous");
                let region_previous = previous.sub_image(px, py, width, height);
                current
                    .copy_from(&region_previous.to_image(), px, py)
                    .unwrap();
            }
        }

        // The allocations from now on are not going to persist,
        // and will be destroyed at the end of the scope.
        // Clone the limits so that any changes to them die with the allocations.
        let mut limits = self.inner.limits.clone();

        // Read next frame data.
        let raw_frame_size = self.inner.reader.output_buffer_size().ok_or_else(|| {
            ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory))
        })?;

        limits.reserve_usize(raw_frame_size)?;
        let mut buffer = vec![0; raw_frame_size];
        // TODO: add `png::Reader::change_limits()` and call it here
        // to also constrain the internal buffer allocations in the PNG crate
        self.inner
            .reader
            .next_frame(&mut buffer)
            .map_err(ImageError::from_png)?;
        let info = self.inner.reader.info();

        // Find out how to interpret the decoded frame.
        let (width, height, px, py, blend);
        match info.frame_control() {
            None => {
                width = info.width;
                height = info.height;
                px = 0;
                py = 0;
                blend = BlendOp::Source;
            }
            Some(fc) => {
                width = fc.width;
                height = fc.height;
                px = fc.x_offset;
                py = fc.y_offset;
                blend = fc.blend_op;
                self.dispose = fc.dispose_op;
            }
        }

        self.dispose_region = Some((px, py, width, height));

        // Turn the data into an rgba image proper.
        limits.reserve_buffer(width, height, COLOR_TYPE)?;
        let source = match self.inner.color_type {
            ColorType::L8 => {
                let image = ImageBuffer::<Luma<_>, _>::from_raw(width, height, buffer).unwrap();
                DynamicImage::ImageLuma8(image).into_rgba8()
            }
            ColorType::La8 => {
                let image = ImageBuffer::<LumaA<_>, _>::from_raw(width, height, buffer).unwrap();
                DynamicImage::ImageLumaA8(image).into_rgba8()
            }
            ColorType::Rgb8 => {
                let image = ImageBuffer::<Rgb<_>, _>::from_raw(width, height, buffer).unwrap();
                DynamicImage::ImageRgb8(image).into_rgba8()
            }
            ColorType::Rgba8 => ImageBuffer::<Rgba<_>, _>::from_raw(width, height, buffer).unwrap(),
            ColorType::L16 | ColorType::Rgb16 | ColorType::La16 | ColorType::Rgba16 => {
                // TODO: to enable remove restriction in `animatable_color_type` method.
                unreachable!("16-bit apng not yet support")
            }
            _ => unreachable!("Invalid png color"),
        };
        // We've converted the raw frame to RGBA8 and disposed of the original allocation
        limits.free_usize(raw_frame_size);

        match blend {
            BlendOp::Source => {
                current
                    .copy_from(&source, px, py)
                    .expect("Invalid png image not detected in png");
            }
            BlendOp::Over => {
                // TODO: investigate speed, speed-ups, and bounds-checks.
                for (x, y, p) in source.enumerate_pixels() {
                    current.get_pixel_mut(x + px, y + py).blend(p);
                }
            }
        }

        // Ok, we can proceed with actually remaining images.
        self.remaining = remaining;
        // Return composited output buffer.

        Ok(Some(self.current.as_ref().unwrap()))
    }

    fn animatable_color_type(&self) -> Result<(), ImageError> {
        match self.inner.color_type {
            ColorType::L8 | ColorType::Rgb8 | ColorType::La8 | ColorType::Rgba8 => Ok(()),
            // TODO: do not handle multi-byte colors. Remember to implement it in `mix_next_frame`.
            ColorType::L16 | ColorType::Rgb16 | ColorType::La16 | ColorType::Rgba16 => {
                Err(unsupported_color(self.inner.color_type.into()))
            }
            _ => unreachable!("{:?} not a valid png color", self.inner.color_type),
        }
    }
}

impl<'a, R: BufRead + Seek + 'a> AnimationDecoder<'a> for ApngDecoder<R> {
    fn into_frames(self) -> Frames<'a> {
        struct FrameIterator<R: BufRead + Seek>(ApngDecoder<R>);

        impl<R: BufRead + Seek> Iterator for FrameIterator<R> {
            type Item = ImageResult<Frame>;

            fn next(&mut self) -> Option<Self::Item> {
                let image = match self.0.mix_next_frame() {
                    Ok(Some(image)) => image.clone(),
                    Ok(None) => return None,
                    Err(err) => return Some(Err(err)),
                };

                let info = self.0.inner.reader.info();
                let fc = info.frame_control().unwrap();
                // PNG delays are rations in seconds.
                let num = u32::from(fc.delay_num) * 1_000u32;
                let denom = match fc.delay_den {
                    // The standard dictates to replace by 100 when the denominator is 0.
                    0 => 100,
                    d => u32::from(d),
                };
                let delay = Delay::from_ratio(Ratio::new(num, denom));
                Some(Ok(Frame::from_parts(image, 0, 0, delay)))
            }
        }

        Frames::new(Box::new(FrameIterator(self)))
    }
}

/// PNG encoder
pub struct PngEncoder<W: Write> {
    w: W,
    compression: CompressionType,
    filter: FilterType,
    icc_profile: Vec<u8>,
    exif_metadata: Vec<u8>,
}

/// Compression level of a PNG encoder. The default setting is `Fast`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
#[derive(Default)]
pub enum CompressionType {
    /// Default compression level
    Default,
    /// Fast, minimal compression
    #[default]
    Fast,
    /// High compression level
    Best,
}

/// Filter algorithms used to process image data to improve compression.
///
/// The default filter is `Adaptive`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
#[derive(Default)]
pub enum FilterType {
    /// No processing done, best used for low bit depth grayscale or data with a
    /// low color count
    NoFilter,
    /// Filters based on previous pixel in the same scanline
    Sub,
    /// Filters based on the scanline above
    Up,
    /// Filters based on the average of left and right neighbor pixels
    Avg,
    /// Algorithm that takes into account the left, upper left, and above pixels
    Paeth,
    /// Uses a heuristic to select one of the preceding filters for each
    /// scanline rather than one filter for the entire image
    #[default]
    Adaptive,
}

impl<W: Write> PngEncoder<W> {
    /// Create a new encoder that writes its output to ```w```
    pub fn new(w: W) -> PngEncoder<W> {
        PngEncoder {
            w,
            compression: CompressionType::default(),
            filter: FilterType::default(),
            icc_profile: Vec::new(),
            exif_metadata: Vec::new(),
        }
    }

    /// Create a new encoder that writes its output to `w` with `CompressionType` `compression` and
    /// `FilterType` `filter`.
    ///
    /// It is best to view the options as a _hint_ to the implementation on the smallest or fastest
    /// option for encoding a particular image. That is, using options that map directly to a PNG
    /// image parameter will use this parameter where possible. But variants that have no direct
    /// mapping may be interpreted differently in minor versions. The exact output is expressly
    /// __not__ part of the SemVer stability guarantee.
    ///
    /// Note that it is not optimal to use a single filter type, so an adaptive
    /// filter type is selected as the default. The filter which best minimizes
    /// file size may change with the type of compression used.
    pub fn new_with_quality(
        w: W,
        compression: CompressionType,
        filter: FilterType,
    ) -> PngEncoder<W> {
        PngEncoder {
            w,
            compression,
            filter,
            icc_profile: Vec::new(),
            exif_metadata: Vec::new(),
        }
    }

    fn encode_inner(
        self,
        data: &[u8],
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<()> {
        let (ct, bits) = match color {
            ExtendedColorType::L8 => (png::ColorType::Grayscale, png::BitDepth::Eight),
            ExtendedColorType::L16 => (png::ColorType::Grayscale, png::BitDepth::Sixteen),
            ExtendedColorType::La8 => (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight),
            ExtendedColorType::La16 => (png::ColorType::GrayscaleAlpha, png::BitDepth::Sixteen),
            ExtendedColorType::Rgb8 => (png::ColorType::Rgb, png::BitDepth::Eight),
            ExtendedColorType::Rgb16 => (png::ColorType::Rgb, png::BitDepth::Sixteen),
            ExtendedColorType::Rgba8 => (png::ColorType::Rgba, png::BitDepth::Eight),
            ExtendedColorType::Rgba16 => (png::ColorType::Rgba, png::BitDepth::Sixteen),
            _ => {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Png.into(),
                        UnsupportedErrorKind::Color(color),
                    ),
                ))
            }
        };

        let comp = match self.compression {
            CompressionType::Default => png::Compression::Balanced,
            CompressionType::Best => png::Compression::High,
            _ => png::Compression::Fast,
        };

        let filter = match self.filter {
            FilterType::NoFilter => png::Filter::NoFilter,
            FilterType::Sub => png::Filter::Sub,
            FilterType::Up => png::Filter::Up,
            FilterType::Avg => png::Filter::Avg,
            FilterType::Paeth => png::Filter::Paeth,
            FilterType::Adaptive => png::Filter::Adaptive,
        };

        let mut info = png::Info::with_size(width, height);

        if !self.icc_profile.is_empty() {
            info.icc_profile = Some(Cow::Borrowed(&self.icc_profile));
        }
        if !self.exif_metadata.is_empty() {
            info.exif_metadata = Some(Cow::Borrowed(&self.exif_metadata));
        }

        let mut encoder =
            png::Encoder::with_info(self.w, info).map_err(|e| ImageError::IoError(e.into()))?;

        encoder.set_color(ct);
        encoder.set_depth(bits);
        encoder.set_compression(comp);
        encoder.set_filter(filter);
        let mut writer = encoder
            .write_header()
            .map_err(|e| ImageError::IoError(e.into()))?;
        writer
            .write_image_data(data)
            .map_err(|e| ImageError::IoError(e.into()))
    }
}

impl<W: Write> ImageEncoder for PngEncoder<W> {
    /// Write a PNG image with the specified width, height, and color type.
    ///
    /// For color types with 16-bit per channel or larger, the contents of `buf` should be in
    /// native endian. `PngEncoder` will automatically convert to big endian as required by the
    /// underlying PNG format.
    #[track_caller]
    fn write_image(
        self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        use ExtendedColorType::*;

        let expected_buffer_len = color_type.buffer_size(width, height);
        assert_eq!(
            expected_buffer_len,
            buf.len() as u64,
            "Invalid buffer length: expected {expected_buffer_len} got {} for {width}x{height} image",
            buf.len(),
        );

        // PNG images are big endian. For 16 bit per channel and larger types,
        // the buffer may need to be reordered to big endian per the
        // contract of `write_image`.
        // TODO: assumes equal channel bit depth.
        match color_type {
            L8 | La8 | Rgb8 | Rgba8 => {
                // No reodering necessary for u8
                self.encode_inner(buf, width, height, color_type)
            }
            L16 | La16 | Rgb16 | Rgba16 => {
                // Because the buffer is immutable and the PNG encoder does not
                // yet take Write/Read traits, create a temporary buffer for
                // big endian reordering.
                let mut reordered;
                let buf = if cfg!(target_endian = "little") {
                    reordered = vec_try_with_capacity(buf.len())?;
                    reordered.extend(buf.chunks_exact(2).flat_map(|le| [le[1], le[0]]));
                    &reordered
                } else {
                    buf
                };
                self.encode_inner(buf, width, height, color_type)
            }
            _ => Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Hdr.into(),
                    UnsupportedErrorKind::Color(color_type),
                ),
            )),
        }
    }

    fn set_icc_profile(&mut self, icc_profile: Vec<u8>) -> Result<(), UnsupportedError> {
        self.icc_profile = icc_profile;
        Ok(())
    }

    fn set_exif_metadata(&mut self, exif: Vec<u8>) -> Result<(), UnsupportedError> {
        self.exif_metadata = exif;
        Ok(())
    }
}

impl ImageError {
    fn from_png(err: png::DecodingError) -> ImageError {
        use png::DecodingError::*;
        match err {
            IoError(err) => ImageError::IoError(err),
            // The input image was not a valid PNG.
            err @ Format(_) => {
                ImageError::Decoding(DecodingError::new(ImageFormat::Png.into(), err))
            }
            // Other is used when:
            // - The decoder is polled for more animation frames despite being done (or not being animated
            //   in the first place).
            // - The output buffer does not have the required size.
            err @ Parameter(_) => ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::Generic(err.to_string()),
            )),
            LimitsExceeded => {
                ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::free_functions::decoder_to_vec;
    use std::io::{BufReader, Cursor, Read};

    #[test]
    fn ensure_no_decoder_off_by_one() {
        let dec = PngDecoder::new(BufReader::new(
            std::fs::File::open("tests/images/png/bugfixes/debug_triangle_corners_widescreen.png")
                .unwrap(),
        ))
        .expect("Unable to read PNG file (does it exist?)");

        assert_eq![(2000, 1000), dec.dimensions()];

        assert_eq![
            ColorType::Rgb8,
            dec.color_type(),
            "Image MUST have the Rgb8 format"
        ];

        let correct_bytes = decoder_to_vec(dec)
            .expect("Unable to read file")
            .bytes()
            .map(|x| x.expect("Unable to read byte"))
            .collect::<Vec<u8>>();

        assert_eq![6_000_000, correct_bytes.len()];
    }

    #[test]
    fn underlying_error() {
        use std::error::Error;

        let mut not_png =
            std::fs::read("tests/images/png/bugfixes/debug_triangle_corners_widescreen.png")
                .unwrap();
        not_png[0] = 0;

        let error = PngDecoder::new(Cursor::new(&not_png)).err().unwrap();
        let _ = error
            .source()
            .unwrap()
            .downcast_ref::<png::DecodingError>()
            .expect("Caused by a png error");
    }

    #[test]
    fn encode_bad_color_type() {
        // regression test for issue #1663
        let image = DynamicImage::new_rgb32f(1, 1);
        let mut target = Cursor::new(vec![]);
        let _ = image.write_to(&mut target, ImageFormat::Png);
    }
}
