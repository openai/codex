mod interlace_info;
mod read_decoder;
pub(crate) mod stream;
pub(crate) mod transform;
mod unfiltering_buffer;
mod zlib;

use self::read_decoder::{ImageDataCompletionStatus, ReadDecoder};
use self::stream::{DecodeOptions, DecodingError, FormatErrorInner};
use self::transform::{create_transform_fn, TransformFn};
use self::unfiltering_buffer::UnfilteringBuffer;

use std::io::{BufRead, Seek};
use std::mem;

use crate::adam7::Adam7Info;
use crate::common::{
    BitDepth, BytesPerPixel, ColorType, Info, ParameterErrorKind, Transformations,
};
use crate::FrameControl;
pub use zlib::{UnfilterBuf, UnfilterRegion};

pub use interlace_info::InterlaceInfo;
use interlace_info::InterlaceInfoIter;

/*
pub enum InterlaceHandling {
    /// Outputs the raw rows
    RawRows,
    /// Fill missing the pixels from the existing ones
    Rectangle,
    /// Only fill the needed pixels
    Sparkle
}
*/

/// Output info.
///
/// This describes one particular frame of the image that was written into the output buffer.
#[derive(Debug, PartialEq, Eq)]
pub struct OutputInfo {
    /// The pixel width of this frame.
    pub width: u32,
    /// The pixel height of this frame.
    pub height: u32,
    /// The chosen output color type.
    pub color_type: ColorType,
    /// The chosen output bit depth.
    pub bit_depth: BitDepth,
    /// The byte count of each scan line in the image.
    pub line_size: usize,
}

impl OutputInfo {
    /// Returns the size needed to hold a decoded frame
    /// If the output buffer was larger then bytes after this count should be ignored. They may
    /// still have been changed.
    pub fn buffer_size(&self) -> usize {
        self.line_size * self.height as usize
    }
}

#[derive(Clone, Copy, Debug)]
/// Limits on the resources the `Decoder` is allowed too use
pub struct Limits {
    /// maximum number of bytes the decoder is allowed to allocate, default is 64Mib
    pub bytes: usize,
}

impl Limits {
    pub(crate) fn reserve_bytes(&mut self, bytes: usize) -> Result<(), DecodingError> {
        if self.bytes >= bytes {
            self.bytes -= bytes;
            Ok(())
        } else {
            Err(DecodingError::LimitsExceeded)
        }
    }
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            bytes: 1024 * 1024 * 64,
        }
    }
}

/// PNG Decoder
pub struct Decoder<R: BufRead + Seek> {
    read_decoder: ReadDecoder<R>,
    /// Output transformations
    transform: Transformations,
}

/// A row of data with interlace information attached.
#[derive(Clone, Copy, Debug)]
pub struct InterlacedRow<'data> {
    data: &'data [u8],
    interlace: InterlaceInfo,
}

impl<'data> InterlacedRow<'data> {
    pub fn data(&self) -> &'data [u8] {
        self.data
    }

    pub fn interlace(&self) -> &InterlaceInfo {
        &self.interlace
    }
}

/// A row of data without interlace information.
#[derive(Clone, Copy, Debug)]
pub struct Row<'data> {
    data: &'data [u8],
}

impl<'data> Row<'data> {
    pub fn data(&self) -> &'data [u8] {
        self.data
    }
}

impl<R: BufRead + Seek> Decoder<R> {
    /// Create a new decoder configuration with default limits.
    pub fn new(r: R) -> Decoder<R> {
        Decoder::new_with_limits(r, Limits::default())
    }

    /// Create a new decoder configuration with custom limits.
    pub fn new_with_limits(r: R, limits: Limits) -> Decoder<R> {
        let mut read_decoder = ReadDecoder::new(r);
        read_decoder.set_limits(limits);

        Decoder {
            read_decoder,
            transform: Transformations::IDENTITY,
        }
    }

    /// Create a new decoder configuration with custom [`DecodeOptions`].
    pub fn new_with_options(r: R, decode_options: DecodeOptions) -> Decoder<R> {
        let mut read_decoder = ReadDecoder::with_options(r, decode_options);
        read_decoder.set_limits(Limits::default());

        Decoder {
            read_decoder,
            transform: Transformations::IDENTITY,
        }
    }

    /// Limit resource usage.
    ///
    /// Note that your allocations, e.g. when reading into a pre-allocated buffer, are __NOT__
    /// considered part of the limits. Nevertheless, required intermediate buffers such as for
    /// singular lines is checked against the limit.
    ///
    /// Note that this is a best-effort basis.
    ///
    /// ```
    /// use std::fs::File;
    /// use std::io::BufReader;
    /// use png::{Decoder, Limits};
    /// // This image is 32×32, 1bit per pixel. The reader buffers one row which requires 4 bytes.
    /// let mut limits = Limits::default();
    /// limits.bytes = 3;
    /// let mut decoder = Decoder::new_with_limits(BufReader::new(File::open("tests/pngsuite/basi0g01.png").unwrap()), limits);
    /// assert!(decoder.read_info().is_err());
    ///
    /// // This image is 32x32 pixels, so the decoder will allocate less than 10Kib
    /// let mut limits = Limits::default();
    /// limits.bytes = 10*1024;
    /// let mut decoder = Decoder::new_with_limits(BufReader::new(File::open("tests/pngsuite/basi0g01.png").unwrap()), limits);
    /// assert!(decoder.read_info().is_ok());
    /// ```
    pub fn set_limits(&mut self, limits: Limits) {
        self.read_decoder.set_limits(limits);
    }

    /// Read the PNG header and return the information contained within.
    ///
    /// Most image metadata will not be read until `read_info` is called, so those fields will be
    /// None or empty.
    pub fn read_header_info(&mut self) -> Result<&Info<'static>, DecodingError> {
        self.read_decoder.read_header_info()
    }

    /// Reads all meta data until the first IDAT chunk
    pub fn read_info(mut self) -> Result<Reader<R>, DecodingError> {
        let info = self.read_header_info()?;
        let unfiltering_buffer = UnfilteringBuffer::new(info);

        let mut reader = Reader {
            decoder: self.read_decoder,
            bpp: BytesPerPixel::One,
            subframe: SubframeInfo::not_yet_init(),
            remaining_frames: 0, // Temporary value - fixed below after reading `acTL` and `fcTL`.
            unfiltering_buffer,
            transform: self.transform,
            transform_fn: None,
            scratch_buffer: Vec::new(),
            finished: false,
        };

        // Check if the decoding buffer of a single raw line has a valid size.
        //
        // FIXME: this check and the next can be delayed until processing image data. This would
        // allow usage where only the metadata is processes, or where the image is processed
        // line-by-line even on targets that can not fit the whole image into their address space.
        // We should strive for a balance between implementation complexity (still ensure that the
        // no-overflow preconditions are met for internal calculation) and use possibilities.
        if reader.info().checked_raw_row_length().is_none() {
            return Err(DecodingError::LimitsExceeded);
        }

        // Check if the output buffer has a valid size.
        //
        // FIXME: see above and
        // <https://github.com/image-rs/image-png/pull/608#issuecomment-3003576956>
        if reader.output_buffer_size().is_none() {
            return Err(DecodingError::LimitsExceeded);
        }

        reader.read_until_image_data()?;

        reader.remaining_frames = match reader.info().animation_control.as_ref() {
            None => 1, // No `acTL` => only expecting `IDAT` frame.
            Some(animation) => {
                let mut num_frames = animation.num_frames as usize;
                if reader.info().frame_control.is_none() {
                    // No `fcTL` before `IDAT` => `IDAT` is not part of the animation, but
                    // represents an *extra*, default frame for non-APNG-aware decoders.
                    num_frames += 1;
                }
                num_frames
            }
        };
        Ok(reader)
    }

    /// Set the allowed and performed transformations.
    ///
    /// A transformation is a pre-processing on the raw image data modifying content or encoding.
    /// Many options have an impact on memory or CPU usage during decoding.
    pub fn set_transformations(&mut self, transform: Transformations) {
        self.transform = transform;
    }

    /// Set the decoder to ignore all text chunks while parsing.
    ///
    /// eg.
    /// ```
    /// use std::fs::File;
    /// use std::io::BufReader;
    /// use png::Decoder;
    /// let mut decoder = Decoder::new(BufReader::new(File::open("tests/pngsuite/basi0g01.png").unwrap()));
    /// decoder.set_ignore_text_chunk(true);
    /// assert!(decoder.read_info().is_ok());
    /// ```
    pub fn set_ignore_text_chunk(&mut self, ignore_text_chunk: bool) {
        self.read_decoder.set_ignore_text_chunk(ignore_text_chunk);
    }

    /// Set the decoder to ignore iccp chunks while parsing.
    ///
    /// eg.
    /// ```
    /// use std::fs::File;
    /// use std::io::BufReader;
    /// use png::Decoder;
    /// let mut decoder = Decoder::new(BufReader::new(File::open("tests/iccp/broken_iccp.png").unwrap()));
    /// decoder.set_ignore_iccp_chunk(true);
    /// assert!(decoder.read_info().is_ok());
    /// ```
    pub fn set_ignore_iccp_chunk(&mut self, ignore_iccp_chunk: bool) {
        self.read_decoder.set_ignore_iccp_chunk(ignore_iccp_chunk);
    }

    /// Set the decoder to ignore and not verify the Adler-32 checksum
    /// and CRC code.
    pub fn ignore_checksums(&mut self, ignore_checksums: bool) {
        self.read_decoder.ignore_checksums(ignore_checksums);
    }
}

/// PNG reader (mostly high-level interface)
///
/// Provides a high level that iterates over lines or whole images.
pub struct Reader<R: BufRead + Seek> {
    decoder: ReadDecoder<R>,
    bpp: BytesPerPixel,
    subframe: SubframeInfo,
    /// How many frames remain to be decoded.  Decremented after each `IDAT` or `fdAT` sequence.
    remaining_frames: usize,
    /// Buffer with not-yet-`unfilter`-ed image rows
    unfiltering_buffer: UnfilteringBuffer,
    /// Output transformations
    transform: Transformations,
    /// Function that can transform decompressed, unfiltered rows into final output.
    /// See the `transform.rs` module for more details.
    transform_fn: Option<TransformFn>,
    /// This buffer is only used so that `next_row` and `next_interlaced_row` can return reference
    /// to a byte slice. In a future version of this library, this buffer will be removed and
    /// `next_row` and `next_interlaced_row` will write directly into a user provided output buffer.
    scratch_buffer: Vec<u8>,
    /// Whether `ImageEnd` was already reached by `fn finish`.
    finished: bool,
}

/// The subframe specific information.
///
/// In APNG the frames are constructed by combining previous frame and a new subframe (through a
/// combination of `dispose_op` and `overlay_op`). These sub frames specify individual dimension
/// information and reuse the global interlace options. This struct encapsulates the state of where
/// in a particular IDAT-frame or subframe we are.
struct SubframeInfo {
    width: u32,
    height: u32,
    rowlen: usize,
    current_interlace_info: Option<InterlaceInfo>,
    interlace_info_iter: InterlaceInfoIter,
    consumed_and_flushed: bool,
}

impl<R: BufRead + Seek> Reader<R> {
    /// Advances to the start of the next animation frame and
    /// returns a reference to the `FrameControl` info that describes it.
    /// Skips and discards the image data of the previous frame if necessary.
    ///
    /// Returns a [`ParameterError`] when there are no more animation frames.
    /// To avoid this the caller can check if [`Info::animation_control`] exists
    /// and consult [`AnimationControl::num_frames`].
    pub fn next_frame_info(&mut self) -> Result<&FrameControl, DecodingError> {
        let remaining_frames = if self.subframe.consumed_and_flushed {
            self.remaining_frames
        } else {
            // One remaining frame will be consumed by the `finish_decoding` call below.
            self.remaining_frames - 1
        };
        if remaining_frames == 0 {
            return Err(DecodingError::Parameter(
                ParameterErrorKind::PolledAfterEndOfImage.into(),
            ));
        }

        if !self.subframe.consumed_and_flushed {
            self.subframe.current_interlace_info = None;
            self.finish_decoding()?;
        }
        self.read_until_image_data()?;

        // The PNG standard (and `StreamingDecoder `) guarantes that there is an `fcTL` chunk
        // before the start of image data in a sequence of `fdAT` chunks.  Therefore `unwrap`
        // below is guaranteed to not panic.
        Ok(self.info().frame_control.as_ref().unwrap())
    }

    /// Reads all meta data until the next frame data starts.
    /// Requires IHDR before the IDAT and fcTL before fdAT.
    fn read_until_image_data(&mut self) -> Result<(), DecodingError> {
        self.decoder.read_until_image_data()?;

        self.subframe = SubframeInfo::new(self.info());
        self.bpp = self.info().bpp_in_prediction();
        self.unfiltering_buffer.reset_all();

        // Allocate output buffer.
        let buflen = self.unguarded_output_line_size(self.subframe.width);
        self.decoder.reserve_bytes(buflen)?;

        Ok(())
    }

    /// Get information on the image.
    ///
    /// The structure will change as new frames of an animated image are decoded.
    pub fn info(&self) -> &Info<'static> {
        self.decoder.info().unwrap()
    }

    /// Decodes the next frame into `buf`.
    ///
    /// Note that this decodes raw subframes that need to be mixed according to blend-op and
    /// dispose-op by the caller.
    ///
    /// The caller must always provide a buffer large enough to hold a complete frame (the APNG
    /// specification restricts subframes to the dimensions given in the image header). The region
    /// that has been written be checked afterwards by calling `info` after a successful call and
    /// inspecting the `frame_control` data. This requirement may be lifted in a later version of
    /// `png`.
    ///
    /// Output lines will be written in row-major, packed matrix with width and height of the read
    /// frame (or subframe), all samples are in big endian byte order where this matters.
    pub fn next_frame(&mut self, buf: &mut [u8]) -> Result<OutputInfo, DecodingError> {
        if self.remaining_frames == 0 {
            return Err(DecodingError::Parameter(
                ParameterErrorKind::PolledAfterEndOfImage.into(),
            ));
        } else if self.subframe.consumed_and_flushed {
            // Advance until the next `fdAT`
            // (along the way we should encounter the fcTL for this frame).
            self.read_until_image_data()?;
        }

        // Note that we only check if the buffer size calculation holds in a call to decoding the
        // frame. Consequently, we can represent the `Info` and frameless decoding even when the
        // target architecture's address space is too small for a frame. However reading the actual
        let required_len = self
            .output_buffer_size()
            .ok_or(DecodingError::LimitsExceeded)?;

        if buf.len() < required_len {
            return Err(DecodingError::Parameter(
                ParameterErrorKind::ImageBufferSize {
                    expected: required_len,
                    actual: buf.len(),
                }
                .into(),
            ));
        }

        let (color_type, bit_depth) = self.output_color_type();
        let output_info = OutputInfo {
            width: self.subframe.width,
            height: self.subframe.height,
            color_type,
            bit_depth,
            line_size: self.unguarded_output_line_size(self.subframe.width),
        };

        if self.info().interlaced {
            let stride = self.unguarded_output_line_size(self.info().width);
            let samples = color_type.samples() as u8;
            let bits_pp = samples * (bit_depth as u8);
            let expand = crate::adam7::expand_pass;

            while let Some(InterlacedRow {
                data: row,
                interlace,
                ..
            }) = self.next_interlaced_row()?
            {
                // `unwrap` won't panic, because we checked `self.info().interlaced` above.
                let adam7info = interlace.get_adam7_info().unwrap();
                expand(buf, stride, row, adam7info, bits_pp);
            }
        } else {
            let current_interlace_info = self.subframe.current_interlace_info.as_ref();
            let already_done_rows = current_interlace_info
                .map(|info| info.line_number())
                .unwrap_or(self.subframe.height);

            for row in buf
                .chunks_exact_mut(output_info.line_size)
                .take(self.subframe.height as usize)
                .skip(already_done_rows as usize)
            {
                self.next_interlaced_row_impl(self.subframe.rowlen, row)?;
            }
        }

        // Advance over the rest of data for this (sub-)frame.
        self.finish_decoding()?;

        Ok(output_info)
    }

    fn mark_subframe_as_consumed_and_flushed(&mut self) {
        assert!(self.remaining_frames > 0);
        self.remaining_frames -= 1;

        self.subframe.consumed_and_flushed = true;
    }

    /// Advance over the rest of data for this (sub-)frame.
    /// Called after decoding the last row of a frame.
    fn finish_decoding(&mut self) -> Result<(), DecodingError> {
        // Double-check that all rows of this frame have been decoded (i.e. that the potential
        // `finish_decoding` call below won't be discarding any data).
        assert!(self.subframe.current_interlace_info.is_none());

        // Discard the remaining data in the current sequence of `IDAT` or `fdAT` chunks.
        if !self.subframe.consumed_and_flushed {
            self.decoder.finish_decoding_image_data()?;
            self.mark_subframe_as_consumed_and_flushed();
        }

        Ok(())
    }

    /// Returns the next processed row of the image (discarding `InterlaceInfo`).
    ///
    /// See also [`Reader.read_row`], which reads into a caller-provided buffer.
    pub fn next_row(&mut self) -> Result<Option<Row<'_>>, DecodingError> {
        self.next_interlaced_row()
            .map(|v| v.map(|v| Row { data: v.data }))
    }

    /// Returns the next processed row of the image.
    ///
    /// See also [`Reader.read_row`], which reads into a caller-provided buffer.
    pub fn next_interlaced_row(&mut self) -> Result<Option<InterlacedRow<'_>>, DecodingError> {
        let mut output_buffer = mem::take(&mut self.scratch_buffer);
        let max_line_size = self
            .output_line_size(self.info().width)
            .ok_or(DecodingError::LimitsExceeded)?;
        output_buffer.resize(max_line_size, 0u8);
        let result = self.read_row(&mut output_buffer);
        self.scratch_buffer = output_buffer;
        result.map(move |option| {
            option.map(move |interlace| {
                let output_line_size = self.output_line_size_for_interlace_info(&interlace);
                InterlacedRow {
                    data: &self.scratch_buffer[..output_line_size],
                    interlace,
                }
            })
        })
    }

    /// Reads the next row of the image into the provided `output_buffer`.
    /// `Ok(None)` will be returned if the current image frame has no more rows.
    ///
    /// `output_buffer` needs to be long enough to accommodate [`Reader.output_line_size`] for
    /// [`Info.width`] (initial interlaced rows may need less than that).
    ///
    /// See also [`Reader.next_row`] and [`Reader.next_interlaced_row`], which read into a
    /// `Reader`-owned buffer.
    pub fn read_row(
        &mut self,
        output_buffer: &mut [u8],
    ) -> Result<Option<InterlaceInfo>, DecodingError> {
        let interlace = match self.subframe.current_interlace_info.as_ref() {
            None => {
                self.finish_decoding()?;
                return Ok(None);
            }
            Some(interlace) => *interlace,
        };
        if interlace.line_number() == 0 {
            self.unfiltering_buffer.reset_prev_row();
        }
        let rowlen = match interlace {
            InterlaceInfo::Null(_) => self.subframe.rowlen,
            InterlaceInfo::Adam7(Adam7Info { samples: width, .. }) => {
                self.info().raw_row_length_from_width(width)
            }
        };

        let output_line_size = self.output_line_size_for_interlace_info(&interlace);
        let output_buffer = &mut output_buffer[..output_line_size];

        self.next_interlaced_row_impl(rowlen, output_buffer)?;

        Ok(Some(interlace))
    }

    fn output_line_size_for_interlace_info(&self, interlace: &InterlaceInfo) -> usize {
        let width = match interlace {
            InterlaceInfo::Adam7(Adam7Info { samples: width, .. }) => *width,
            InterlaceInfo::Null(_) => self.subframe.width,
        };
        self.unguarded_output_line_size(width)
    }

    /// Read the rest of the image and chunks and finish up, including text chunks or others
    /// This will discard the rest of the image if the image is not read already with [`Reader::next_frame`], [`Reader::next_row`] or [`Reader::next_interlaced_row`]
    pub fn finish(&mut self) -> Result<(), DecodingError> {
        if self.finished {
            return Err(DecodingError::Parameter(
                ParameterErrorKind::PolledAfterEndOfImage.into(),
            ));
        }

        self.remaining_frames = 0;
        self.unfiltering_buffer.reset_all();
        self.decoder.read_until_end_of_input()?;

        self.finished = true;
        Ok(())
    }

    /// Fetch the next interlaced row and filter it according to our own transformations.
    fn next_interlaced_row_impl(
        &mut self,
        rowlen: usize,
        output_buffer: &mut [u8],
    ) -> Result<(), DecodingError> {
        self.next_raw_interlaced_row(rowlen)?;
        let row = self.unfiltering_buffer.prev_row();
        assert_eq!(row.len(), rowlen - 1);

        // Apply transformations and write resulting data to buffer.
        let transform_fn = {
            if self.transform_fn.is_none() {
                self.transform_fn = Some(create_transform_fn(self.info(), self.transform)?);
            }
            self.transform_fn.as_deref().unwrap()
        };
        transform_fn(row, output_buffer, self.info());

        self.subframe.current_interlace_info = self.subframe.interlace_info_iter.next();
        Ok(())
    }

    /// Returns the color type and the number of bits per sample
    /// of the data returned by `Reader::next_row` and Reader::frames`.
    pub fn output_color_type(&self) -> (ColorType, BitDepth) {
        use crate::common::ColorType::*;
        let t = self.transform;
        let info = self.info();
        if t == Transformations::IDENTITY {
            (info.color_type, info.bit_depth)
        } else {
            let bits = match info.bit_depth as u8 {
                16 if t.intersects(Transformations::STRIP_16) => 8,
                n if n < 8
                    && (t.contains(Transformations::EXPAND)
                        || t.contains(Transformations::ALPHA)) =>
                {
                    8
                }
                n => n,
            };
            let color_type =
                if t.contains(Transformations::EXPAND) || t.contains(Transformations::ALPHA) {
                    let has_trns = info.trns.is_some() || t.contains(Transformations::ALPHA);
                    match info.color_type {
                        Grayscale if has_trns => GrayscaleAlpha,
                        Rgb if has_trns => Rgba,
                        Indexed if has_trns => Rgba,
                        Indexed => Rgb,
                        ct => ct,
                    }
                } else {
                    info.color_type
                };
            (color_type, BitDepth::from_u8(bits).unwrap())
        }
    }

    /// Return the number of bytes required to hold a deinterlaced image frame that is decoded
    /// using the given input transformations.
    ///
    /// Returns `None` if the output buffer does not fit into the memory space of the machine,
    /// otherwise returns the byte length in `Some`. The length is smaller than [`isize::MAX`].
    pub fn output_buffer_size(&self) -> Option<usize> {
        let (width, height) = self.info().size();
        let (color, depth) = self.output_color_type();
        // The subtraction should always work, but we do this for consistency. Also note that by
        // calling `checked_raw_row_length` the row buffer is guaranteed to work whereas if we
        // ran other function that didn't include the filter byte that could later fail on an image
        // that is `1xN`...
        let linelen = color.checked_raw_row_length(depth, width)?.checked_sub(1)?;
        let height = usize::try_from(height).ok()?;
        let imglen = linelen.checked_mul(height)?;
        // Ensure that it fits into address space not only `usize` to allocate.
        (imglen <= isize::MAX as usize).then_some(imglen)
    }

    /// Returns the number of bytes required to hold a deinterlaced row.
    pub(crate) fn unguarded_output_line_size(&self, width: u32) -> usize {
        let (color, depth) = self.output_color_type();
        color.raw_row_length_from_width(depth, width) - 1
    }

    /// Returns the number of bytes required to hold a deinterlaced row.
    ///
    /// Returns `None` if the output buffer does not fit into the memory space of the machine,
    /// otherwise returns the byte length in `Some`. The length is smaller than [`isize::MAX`].
    pub fn output_line_size(&self, width: u32) -> Option<usize> {
        let (color, depth) = self.output_color_type();
        let length = color.checked_raw_row_length(depth, width)?.checked_sub(1)?;
        // Ensure that it fits into address space not only `usize` to allocate.
        (length <= isize::MAX as usize).then_some(length)
    }

    /// Unfilter the next raw interlaced row into `self.unfiltering_buffer`.
    fn next_raw_interlaced_row(&mut self, rowlen: usize) -> Result<(), DecodingError> {
        // Read image data until we have at least one full row (but possibly more than one).
        while self.unfiltering_buffer.curr_row_len() < rowlen {
            if self.subframe.consumed_and_flushed {
                return Err(DecodingError::Format(
                    FormatErrorInner::NoMoreImageData.into(),
                ));
            }

            let mut buffer = self.unfiltering_buffer.as_unfilled_buffer();
            match self.decoder.decode_image_data(Some(&mut buffer))? {
                ImageDataCompletionStatus::ExpectingMoreData => (),
                ImageDataCompletionStatus::Done => self.mark_subframe_as_consumed_and_flushed(),
            }
        }

        self.unfiltering_buffer.unfilter_curr_row(rowlen, self.bpp)
    }
}

impl SubframeInfo {
    fn not_yet_init() -> Self {
        SubframeInfo {
            width: 0,
            height: 0,
            rowlen: 0,
            current_interlace_info: None,
            interlace_info_iter: InterlaceInfoIter::empty(),
            consumed_and_flushed: false,
        }
    }

    fn new(info: &Info) -> Self {
        // The apng fctnl overrides width and height.
        // All other data is set by the main info struct.
        let (width, height) = if let Some(fc) = info.frame_control {
            (fc.width, fc.height)
        } else {
            (info.width, info.height)
        };

        let mut interlace_info_iter = InterlaceInfoIter::new(width, height, info.interlaced);
        let current_interlace_info = interlace_info_iter.next();
        SubframeInfo {
            width,
            height,
            rowlen: info.raw_row_length_from_width(width),
            current_interlace_info,
            interlace_info_iter,
            consumed_and_flushed: false,
        }
    }
}
