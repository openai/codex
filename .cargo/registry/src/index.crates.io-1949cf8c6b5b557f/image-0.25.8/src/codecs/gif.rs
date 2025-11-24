//!  Decoding of GIF Images
//!
//!  GIF (Graphics Interchange Format) is an image format that supports lossless compression.
//!
//!  # Related Links
//!  * <http://www.w3.org/Graphics/GIF/spec-gif89a.txt> - The GIF Specification
//!
//! # Examples
//! ```rust,no_run
//! use image::codecs::gif::{GifDecoder, GifEncoder};
//! use image::{ImageDecoder, AnimationDecoder};
//! use std::fs::File;
//! use std::io::BufReader;
//! # fn main() -> std::io::Result<()> {
//! // Decode a gif into frames
//! let file_in = BufReader::new(File::open("foo.gif")?);
//! let mut decoder = GifDecoder::new(file_in).unwrap();
//! let frames = decoder.into_frames();
//! let frames = frames.collect_frames().expect("error decoding gif");
//!
//! // Encode frames into a gif and save to a file
//! let mut file_out = File::open("out.gif")?;
//! let mut encoder = GifEncoder::new(file_out);
//! encoder.encode_frames(frames.into_iter());
//! # Ok(())
//! # }
//! ```
#![allow(clippy::while_let_loop)]

use std::io::{self, BufRead, Cursor, Read, Seek, Write};
use std::marker::PhantomData;
use std::mem;

use gif::ColorOutput;
use gif::{DisposalMethod, Frame};

use crate::animation::{self, Ratio};
use crate::color::{ColorType, Rgba};
use crate::error::{
    DecodingError, EncodingError, ImageError, ImageResult, LimitError, LimitErrorKind,
    ParameterError, ParameterErrorKind, UnsupportedError, UnsupportedErrorKind,
};
use crate::traits::Pixel;
use crate::{
    AnimationDecoder, ExtendedColorType, ImageBuffer, ImageDecoder, ImageEncoder, ImageFormat,
    Limits,
};

/// GIF decoder
pub struct GifDecoder<R: Read> {
    reader: gif::Decoder<R>,
    limits: Limits,
}

impl<R: Read> GifDecoder<R> {
    /// Creates a new decoder that decodes the input steam `r`
    pub fn new(r: R) -> ImageResult<GifDecoder<R>> {
        let mut decoder = gif::DecodeOptions::new();
        decoder.set_color_output(ColorOutput::RGBA);

        Ok(GifDecoder {
            reader: decoder.read_info(r).map_err(ImageError::from_decoding)?,
            limits: Limits::no_limits(),
        })
    }
}

/// Wrapper struct around a `Cursor<Vec<u8>>`
#[allow(dead_code)]
#[deprecated]
pub struct GifReader<R>(Cursor<Vec<u8>>, PhantomData<R>);
#[allow(deprecated)]
impl<R> Read for GifReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        if self.0.position() == 0 && buf.is_empty() {
            mem::swap(buf, self.0.get_mut());
            Ok(buf.len())
        } else {
            self.0.read_to_end(buf)
        }
    }
}

impl<R: BufRead + Seek> ImageDecoder for GifDecoder<R> {
    fn dimensions(&self) -> (u32, u32) {
        (
            u32::from(self.reader.width()),
            u32::from(self.reader.height()),
        )
    }

    fn color_type(&self) -> ColorType {
        ColorType::Rgba8
    }

    fn set_limits(&mut self, limits: Limits) -> ImageResult<()> {
        limits.check_support(&crate::LimitSupport::default())?;

        let (width, height) = self.dimensions();
        limits.check_dimensions(width, height)?;

        self.limits = limits;

        Ok(())
    }

    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        assert_eq!(u64::try_from(buf.len()), Ok(self.total_bytes()));

        let frame = match self
            .reader
            .next_frame_info()
            .map_err(ImageError::from_decoding)?
        {
            Some(frame) => FrameInfo::new_from_frame(frame),
            None => {
                return Err(ImageError::Parameter(ParameterError::from_kind(
                    ParameterErrorKind::NoMoreData,
                )))
            }
        };

        let (width, height) = self.dimensions();

        if frame.left == 0
            && frame.width == width
            && (u64::from(frame.top) + u64::from(frame.height) <= u64::from(height))
        {
            // If the frame matches the logical screen, or, as a more general case,
            // fits into it and touches its left and right borders, then
            // we can directly write it into the buffer without causing line wraparound.
            let line_length = usize::try_from(width)
                .unwrap()
                .checked_mul(self.color_type().bytes_per_pixel() as usize)
                .unwrap();

            // isolate the portion of the buffer to read the frame data into.
            // the chunks above and below it are going to be zeroed.
            let (blank_top, rest) =
                buf.split_at_mut(line_length.checked_mul(frame.top as usize).unwrap());
            let (buf, blank_bottom) =
                rest.split_at_mut(line_length.checked_mul(frame.height as usize).unwrap());

            debug_assert_eq!(buf.len(), self.reader.buffer_size());

            // this is only necessary in case the buffer is not zeroed
            for b in blank_top {
                *b = 0;
            }
            // fill the middle section with the frame data
            self.reader
                .read_into_buffer(buf)
                .map_err(ImageError::from_decoding)?;
            // this is only necessary in case the buffer is not zeroed
            for b in blank_bottom {
                *b = 0;
            }
        } else {
            // If the frame does not match the logical screen, read into an extra buffer
            // and 'insert' the frame from left/top to logical screen width/height.
            let buffer_size = (frame.width as usize)
                .checked_mul(frame.height as usize)
                .and_then(|s| s.checked_mul(4))
                .ok_or(ImageError::Limits(LimitError::from_kind(
                    LimitErrorKind::InsufficientMemory,
                )))?;

            self.limits.reserve_usize(buffer_size)?;
            let mut frame_buffer = vec![0; buffer_size];
            self.limits.free_usize(buffer_size);

            self.reader
                .read_into_buffer(&mut frame_buffer[..])
                .map_err(ImageError::from_decoding)?;

            let frame_buffer = ImageBuffer::from_raw(frame.width, frame.height, frame_buffer);
            let image_buffer = ImageBuffer::from_raw(width, height, buf);

            // `buffer_size` uses wrapping arithmetic, thus might not report the
            // correct storage requirement if the result does not fit in `usize`.
            // `ImageBuffer::from_raw` detects overflow and reports by returning `None`.
            if frame_buffer.is_none() || image_buffer.is_none() {
                return Err(ImageError::Unsupported(
                    UnsupportedError::from_format_and_kind(
                        ImageFormat::Gif.into(),
                        UnsupportedErrorKind::GenericFeature(format!(
                            "Image dimensions ({}, {}) are too large",
                            frame.width, frame.height
                        )),
                    ),
                ));
            }

            let frame_buffer = frame_buffer.unwrap();
            let mut image_buffer = image_buffer.unwrap();

            for (x, y, pixel) in image_buffer.enumerate_pixels_mut() {
                let frame_x = x.wrapping_sub(frame.left);
                let frame_y = y.wrapping_sub(frame.top);

                if frame_x < frame.width && frame_y < frame.height {
                    *pixel = *frame_buffer.get_pixel(frame_x, frame_y);
                } else {
                    // this is only necessary in case the buffer is not zeroed
                    *pixel = Rgba([0, 0, 0, 0]);
                }
            }
        }

        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }
}

struct GifFrameIterator<R: Read> {
    reader: gif::Decoder<R>,

    width: u32,
    height: u32,

    non_disposed_frame: Option<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    limits: Limits,
    // `is_end` is used to indicate whether the iterator has reached the end of the frames.
    // Or encounter any un-recoverable error.
    is_end: bool,
}

impl<R: BufRead + Seek> GifFrameIterator<R> {
    fn new(decoder: GifDecoder<R>) -> GifFrameIterator<R> {
        let (width, height) = decoder.dimensions();
        let limits = decoder.limits.clone();

        // intentionally ignore the background color for web compatibility

        GifFrameIterator {
            reader: decoder.reader,
            width,
            height,
            non_disposed_frame: None,
            limits,
            is_end: false,
        }
    }
}

impl<R: Read> Iterator for GifFrameIterator<R> {
    type Item = ImageResult<animation::Frame>;

    fn next(&mut self) -> Option<ImageResult<animation::Frame>> {
        if self.is_end {
            return None;
        }

        // The iterator always produces RGBA8 images
        const COLOR_TYPE: ColorType = ColorType::Rgba8;

        // Allocate the buffer for the previous frame.
        // This is done here and not in the constructor because
        // the constructor cannot return an error when the allocation limit is exceeded.
        if self.non_disposed_frame.is_none() {
            if let Err(e) = self
                .limits
                .reserve_buffer(self.width, self.height, COLOR_TYPE)
            {
                return Some(Err(e));
            }
            self.non_disposed_frame = Some(ImageBuffer::from_pixel(
                self.width,
                self.height,
                Rgba([0, 0, 0, 0]),
            ));
        }
        // Bind to a variable to avoid repeated `.unwrap()` calls
        let non_disposed_frame = self.non_disposed_frame.as_mut().unwrap();

        // begin looping over each frame

        let frame = match self.reader.next_frame_info() {
            Ok(frame_info) => {
                if let Some(frame) = frame_info {
                    FrameInfo::new_from_frame(frame)
                } else {
                    // no more frames
                    return None;
                }
            }
            Err(err) => match err {
                gif::DecodingError::Io(ref e) => {
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        // end of file reached, no more frames
                        self.is_end = true;
                    }
                    return Some(Err(ImageError::from_decoding(err)));
                }
                _ => {
                    return Some(Err(ImageError::from_decoding(err)));
                }
            },
        };

        // All allocations we do from now on will be freed at the end of this function.
        // Therefore, do not count them towards the persistent limits.
        // Instead, create a local instance of `Limits` for this function alone
        // which will be dropped along with all the buffers when they go out of scope.
        let mut local_limits = self.limits.clone();

        // Check the allocation we're about to perform against the limits
        if let Err(e) = local_limits.reserve_buffer(frame.width, frame.height, COLOR_TYPE) {
            return Some(Err(e));
        }
        // Allocate the buffer now that the limits allowed it
        let mut vec = vec![0; self.reader.buffer_size()];
        if let Err(err) = self.reader.read_into_buffer(&mut vec) {
            return Some(Err(ImageError::from_decoding(err)));
        }

        // create the image buffer from the raw frame.
        // `buffer_size` uses wrapping arithmetic, thus might not report the
        // correct storage requirement if the result does not fit in `usize`.
        // on the other hand, `ImageBuffer::from_raw` detects overflow and
        // reports by returning `None`.
        let Some(mut frame_buffer) = ImageBuffer::from_raw(frame.width, frame.height, vec) else {
            return Some(Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Gif.into(),
                    UnsupportedErrorKind::GenericFeature(format!(
                        "Image dimensions ({}, {}) are too large",
                        frame.width, frame.height
                    )),
                ),
            )));
        };

        // blend the current frame with the non-disposed frame, then update
        // the non-disposed frame according to the disposal method.
        fn blend_and_dispose_pixel(
            dispose: DisposalMethod,
            previous: &mut Rgba<u8>,
            current: &mut Rgba<u8>,
        ) {
            let pixel_alpha = current.channels()[3];
            if pixel_alpha == 0 {
                *current = *previous;
            }

            match dispose {
                DisposalMethod::Any | DisposalMethod::Keep => {
                    // do not dispose
                    // (keep pixels from this frame)
                    // note: the `Any` disposal method is underspecified in the GIF
                    // spec, but most viewers treat it identically to `Keep`
                    *previous = *current;
                }
                DisposalMethod::Background => {
                    // restore to background color
                    // (background shows through transparent pixels in the next frame)
                    *previous = Rgba([0, 0, 0, 0]);
                }
                DisposalMethod::Previous => {
                    // restore to previous
                    // (dispose frames leaving the last none disposal frame)
                }
            }
        }

        // if `frame_buffer`'s frame exactly matches the entire image, then
        // use it directly, else create a new buffer to hold the composited
        // image.
        let image_buffer = if (frame.left, frame.top) == (0, 0)
            && (self.width, self.height) == frame_buffer.dimensions()
        {
            for (x, y, pixel) in frame_buffer.enumerate_pixels_mut() {
                let previous_pixel = non_disposed_frame.get_pixel_mut(x, y);
                blend_and_dispose_pixel(frame.disposal_method, previous_pixel, pixel);
            }
            frame_buffer
        } else {
            // Check limits before allocating the buffer
            if let Err(e) = local_limits.reserve_buffer(self.width, self.height, COLOR_TYPE) {
                return Some(Err(e));
            }
            ImageBuffer::from_fn(self.width, self.height, |x, y| {
                let frame_x = x.wrapping_sub(frame.left);
                let frame_y = y.wrapping_sub(frame.top);
                let previous_pixel = non_disposed_frame.get_pixel_mut(x, y);

                if frame_x < frame_buffer.width() && frame_y < frame_buffer.height() {
                    let mut pixel = *frame_buffer.get_pixel(frame_x, frame_y);
                    blend_and_dispose_pixel(frame.disposal_method, previous_pixel, &mut pixel);
                    pixel
                } else {
                    // out of bounds, return pixel from previous frame
                    *previous_pixel
                }
            })
        };

        Some(Ok(animation::Frame::from_parts(
            image_buffer,
            0,
            0,
            frame.delay,
        )))
    }
}

impl<'a, R: BufRead + Seek + 'a> AnimationDecoder<'a> for GifDecoder<R> {
    fn into_frames(self) -> animation::Frames<'a> {
        animation::Frames::new(Box::new(GifFrameIterator::new(self)))
    }
}

struct FrameInfo {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    disposal_method: DisposalMethod,
    delay: animation::Delay,
}

impl FrameInfo {
    fn new_from_frame(frame: &Frame) -> FrameInfo {
        FrameInfo {
            left: u32::from(frame.left),
            top: u32::from(frame.top),
            width: u32::from(frame.width),
            height: u32::from(frame.height),
            disposal_method: frame.dispose,
            // frame.delay is in units of 10ms so frame.delay*10 is in ms
            delay: animation::Delay::from_ratio(Ratio::new(u32::from(frame.delay) * 10, 1)),
        }
    }
}

/// Number of repetitions for a GIF animation
#[derive(Clone, Copy, Debug)]
pub enum Repeat {
    /// Finite number of repetitions
    Finite(u16),
    /// Looping GIF
    Infinite,
}

impl Repeat {
    pub(crate) fn to_gif_enum(self) -> gif::Repeat {
        match self {
            Repeat::Finite(n) => gif::Repeat::Finite(n),
            Repeat::Infinite => gif::Repeat::Infinite,
        }
    }
}

/// GIF encoder.
pub struct GifEncoder<W: Write> {
    w: Option<W>,
    gif_encoder: Option<gif::Encoder<W>>,
    speed: i32,
    repeat: Option<Repeat>,
}

impl<W: Write> GifEncoder<W> {
    /// Creates a new GIF encoder with a speed of 1. This prioritizes quality over performance at any cost.
    pub fn new(w: W) -> GifEncoder<W> {
        Self::new_with_speed(w, 1)
    }

    /// Create a new GIF encoder, and has the speed parameter `speed`. See
    /// [`Frame::from_rgba_speed`](https://docs.rs/gif/latest/gif/struct.Frame.html#method.from_rgba_speed)
    /// for more information.
    pub fn new_with_speed(w: W, speed: i32) -> GifEncoder<W> {
        assert!(
            (1..=30).contains(&speed),
            "speed needs to be in the range [1, 30]"
        );
        GifEncoder {
            w: Some(w),
            gif_encoder: None,
            speed,
            repeat: None,
        }
    }

    /// Set the repeat behaviour of the encoded GIF
    pub fn set_repeat(&mut self, repeat: Repeat) -> ImageResult<()> {
        if let Some(ref mut encoder) = self.gif_encoder {
            encoder
                .set_repeat(repeat.to_gif_enum())
                .map_err(ImageError::from_encoding)?;
        }
        self.repeat = Some(repeat);
        Ok(())
    }

    /// Encode a single image.
    pub fn encode(
        &mut self,
        data: &[u8],
        width: u32,
        height: u32,
        color: ExtendedColorType,
    ) -> ImageResult<()> {
        let (width, height) = self.gif_dimensions(width, height)?;
        match color {
            ExtendedColorType::Rgb8 => {
                self.encode_gif(Frame::from_rgb_speed(width, height, data, self.speed))
            }
            ExtendedColorType::Rgba8 => self.encode_gif(Frame::from_rgba_speed(
                width,
                height,
                &mut data.to_owned(),
                self.speed,
            )),
            _ => Err(ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormat::Gif.into(),
                    UnsupportedErrorKind::Color(color),
                ),
            )),
        }
    }

    /// Encode one frame of animation.
    pub fn encode_frame(&mut self, img_frame: animation::Frame) -> ImageResult<()> {
        let frame = self.convert_frame(img_frame)?;
        self.encode_gif(frame)
    }

    /// Encodes Frames.
    /// Consider using `try_encode_frames` instead to encode an `animation::Frames` like iterator.
    pub fn encode_frames<F>(&mut self, frames: F) -> ImageResult<()>
    where
        F: IntoIterator<Item = animation::Frame>,
    {
        for img_frame in frames {
            self.encode_frame(img_frame)?;
        }
        Ok(())
    }

    /// Try to encode a collection of `ImageResult<animation::Frame>` objects.
    /// Use this function to encode an `animation::Frames` like iterator.
    /// Whenever an `Err` item is encountered, that value is returned without further actions.
    pub fn try_encode_frames<F>(&mut self, frames: F) -> ImageResult<()>
    where
        F: IntoIterator<Item = ImageResult<animation::Frame>>,
    {
        for img_frame in frames {
            self.encode_frame(img_frame?)?;
        }
        Ok(())
    }

    pub(crate) fn convert_frame(
        &mut self,
        img_frame: animation::Frame,
    ) -> ImageResult<Frame<'static>> {
        // get the delay before converting img_frame
        let frame_delay = img_frame.delay().into_ratio().to_integer();
        // convert img_frame into RgbaImage
        let mut rbga_frame = img_frame.into_buffer();
        let (width, height) = self.gif_dimensions(rbga_frame.width(), rbga_frame.height())?;

        // Create the gif::Frame from the animation::Frame
        let mut frame = Frame::from_rgba_speed(width, height, &mut rbga_frame, self.speed);
        // Saturate the conversion to u16::MAX instead of returning an error as that
        // would require a new special cased variant in ParameterErrorKind which most
        // likely couldn't be reused for other cases. This isn't a bad trade-off given
        // that the current algorithm is already lossy.
        frame.delay = (frame_delay / 10).try_into().unwrap_or(u16::MAX);

        Ok(frame)
    }

    fn gif_dimensions(&self, width: u32, height: u32) -> ImageResult<(u16, u16)> {
        fn inner_dimensions(width: u32, height: u32) -> Option<(u16, u16)> {
            let width = u16::try_from(width).ok()?;
            let height = u16::try_from(height).ok()?;
            Some((width, height))
        }

        // TODO: this is not very idiomatic yet. Should return an EncodingError.
        inner_dimensions(width, height).ok_or_else(|| {
            ImageError::Parameter(ParameterError::from_kind(
                ParameterErrorKind::DimensionMismatch,
            ))
        })
    }

    pub(crate) fn encode_gif(&mut self, mut frame: Frame) -> ImageResult<()> {
        let gif_encoder;
        if let Some(ref mut encoder) = self.gif_encoder {
            gif_encoder = encoder;
        } else {
            let writer = self.w.take().unwrap();
            let mut encoder = gif::Encoder::new(writer, frame.width, frame.height, &[])
                .map_err(ImageError::from_encoding)?;
            if let Some(ref repeat) = self.repeat {
                encoder
                    .set_repeat(repeat.to_gif_enum())
                    .map_err(ImageError::from_encoding)?;
            }
            self.gif_encoder = Some(encoder);
            gif_encoder = self.gif_encoder.as_mut().unwrap();
        }

        frame.dispose = DisposalMethod::Background;

        gif_encoder
            .write_frame(&frame)
            .map_err(ImageError::from_encoding)
    }
}
impl<W: Write> ImageEncoder for GifEncoder<W> {
    fn write_image(
        mut self,
        buf: &[u8],
        width: u32,
        height: u32,
        color_type: ExtendedColorType,
    ) -> ImageResult<()> {
        self.encode(buf, width, height, color_type)
    }
}

impl ImageError {
    fn from_decoding(err: gif::DecodingError) -> ImageError {
        use gif::DecodingError::*;
        match err {
            err @ Format(_) => {
                ImageError::Decoding(DecodingError::new(ImageFormat::Gif.into(), err))
            }
            Io(io_err) => ImageError::IoError(io_err),
        }
    }

    fn from_encoding(err: gif::EncodingError) -> ImageError {
        use gif::EncodingError::*;
        match err {
            err @ Format(_) => {
                ImageError::Encoding(EncodingError::new(ImageFormat::Gif.into(), err))
            }
            Io(io_err) => ImageError::IoError(io_err),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn frames_exceeding_logical_screen_size() {
        // This is a gif with 10x10 logical screen, but a 16x16 frame + 6px offset inside.
        let data = vec![
            0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x0A, 0x00, 0x0A, 0x00, 0xF0, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x0E, 0xFF, 0x1F, 0x21, 0xF9, 0x04, 0x09, 0x64, 0x00, 0x00, 0x00, 0x2C,
            0x06, 0x00, 0x06, 0x00, 0x10, 0x00, 0x10, 0x00, 0x00, 0x02, 0x23, 0x84, 0x8F, 0xA9,
            0xBB, 0xE1, 0xE8, 0x42, 0x8A, 0x0F, 0x50, 0x79, 0xAE, 0xD1, 0xF9, 0x7A, 0xE8, 0x71,
            0x5B, 0x48, 0x81, 0x64, 0xD5, 0x91, 0xCA, 0x89, 0x4D, 0x21, 0x63, 0x89, 0x4C, 0x09,
            0x77, 0xF5, 0x6D, 0x14, 0x00, 0x3B,
        ];

        let decoder = GifDecoder::new(Cursor::new(data)).unwrap();
        let mut buf = vec![0u8; decoder.total_bytes() as usize];

        assert!(decoder.read_image(&mut buf).is_ok());
    }
}
