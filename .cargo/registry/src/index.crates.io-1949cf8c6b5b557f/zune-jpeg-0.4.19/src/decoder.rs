/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Main image logic.
#![allow(clippy::doc_markdown)]

use alloc::string::ToString;
use alloc::vec::Vec;
use alloc::{format, vec};

use zune_core::bytestream::{ZByteReader, ZReaderTrait};
use zune_core::colorspace::ColorSpace;
use zune_core::log::{error, trace, warn};
use zune_core::options::DecoderOptions;

use crate::color_convert::choose_ycbcr_to_rgb_convert_func;
use crate::components::{Components, SampleRatios};
use crate::errors::{DecodeErrors, UnsupportedSchemes};
use crate::headers::{
    parse_app1, parse_app14, parse_app2, parse_dqt, parse_huffman, parse_sos, parse_start_of_frame
};
use crate::huffman::HuffmanTable;
use crate::idct::choose_idct_func;
use crate::marker::Marker;
use crate::misc::SOFMarkers;
use crate::upsampler::{
    choose_horizontal_samp_function, choose_hv_samp_function, choose_v_samp_function,
    generic_sampler, upsample_no_op
};

/// Maximum components
pub(crate) const MAX_COMPONENTS: usize = 4;

/// Maximum image dimensions supported.
pub(crate) const MAX_DIMENSIONS: usize = 1 << 27;

/// Color conversion function that can convert YCbCr colorspace to RGB(A/X) for
/// 16 values
///
/// The following are guarantees to the following functions
///
/// 1. The `&[i16]` slices passed contain 16 items
///
/// 2. The slices passed are in the following order
///     `y,cb,cr`
///
/// 3. `&mut [u8]` is zero initialized
///
/// 4. `&mut usize` points to the position in the array where new values should
/// be used
///
/// The pointer should
/// 1. Carry out color conversion
/// 2. Update `&mut usize` with the new position

pub type ColorConvert16Ptr = fn(&[i16; 16], &[i16; 16], &[i16; 16], &mut [u8], &mut usize);

/// IDCT  function prototype
///
/// This encapsulates a dequantize and IDCT function which will carry out the
/// following functions
///
/// Multiply each 64 element block of `&mut [i16]` with `&Aligned32<[i32;64]>`
/// Carry out IDCT (type 3 dct) on ach block of 64 i16's
pub type IDCTPtr = fn(&mut [i32; 64], &mut [i16], usize);

/// An encapsulation of an ICC chunk
pub(crate) struct ICCChunk {
    pub(crate) seq_no:      u8,
    pub(crate) num_markers: u8,
    pub(crate) data:        Vec<u8>
}

/// A JPEG Decoder Instance.
#[allow(clippy::upper_case_acronyms, clippy::struct_excessive_bools)]
pub struct JpegDecoder<T: ZReaderTrait> {
    /// Struct to hold image information from SOI
    pub(crate) info:              ImageInfo,
    ///  Quantization tables, will be set to none and the tables will
    /// be moved to `components` field
    pub(crate) qt_tables:         [Option<[i32; 64]>; MAX_COMPONENTS],
    /// DC Huffman Tables with a maximum of 4 tables for each  component
    pub(crate) dc_huffman_tables: [Option<HuffmanTable>; MAX_COMPONENTS],
    /// AC Huffman Tables with a maximum of 4 tables for each component
    pub(crate) ac_huffman_tables: [Option<HuffmanTable>; MAX_COMPONENTS],
    /// Image components, holds information like DC prediction and quantization
    /// tables of a component
    pub(crate) components:        Vec<Components>,
    /// maximum horizontal component of all channels in the image
    pub(crate) h_max:             usize,
    // maximum vertical component of all channels in the image
    pub(crate) v_max:             usize,
    /// mcu's  width (interleaved scans)
    pub(crate) mcu_width:         usize,
    /// MCU height(interleaved scans
    pub(crate) mcu_height:        usize,
    /// Number of MCU's in the x plane
    pub(crate) mcu_x:             usize,
    /// Number of MCU's in the y plane
    pub(crate) mcu_y:             usize,
    /// Is the image interleaved?
    pub(crate) is_interleaved:    bool,
    pub(crate) sub_sample_ratio:  SampleRatios,
    /// Image input colorspace, should be YCbCr for a sane image, might be
    /// grayscale too
    pub(crate) input_colorspace:  ColorSpace,
    // Progressive image details
    /// Is the image progressive?
    pub(crate) is_progressive:    bool,

    /// Start of spectral scan
    pub(crate) spec_start:       u8,
    /// End of spectral scan
    pub(crate) spec_end:         u8,
    /// Successive approximation bit position high
    pub(crate) succ_high:        u8,
    /// Successive approximation bit position low
    pub(crate) succ_low:         u8,
    /// Number of components.
    pub(crate) num_scans:        u8,
    // Function pointers, for pointy stuff.
    /// Dequantize and idct function
    // This is determined at runtime which function to run, statically it's
    // initialized to a platform independent one and during initialization
    // of this struct, we check if we can switch to a faster one which
    // depend on certain CPU extensions.
    pub(crate) idct_func: IDCTPtr,
    // Color convert function which acts on 16 YCbCr values
    pub(crate) color_convert_16: ColorConvert16Ptr,
    pub(crate) z_order:          [usize; MAX_COMPONENTS],
    /// restart markers
    pub(crate) restart_interval: usize,
    pub(crate) todo:             usize,
    // decoder options
    pub(crate) options:          DecoderOptions,
    // byte-stream
    pub(crate) stream:           ZByteReader<T>,
    // Indicate whether headers have been decoded
    pub(crate) headers_decoded:  bool,
    pub(crate) seen_sof:         bool,
    // exif data, lifted from app2
    pub(crate) exif_data:        Option<Vec<u8>>,

    pub(crate) icc_data: Vec<ICCChunk>,
    pub(crate) is_mjpeg: bool,
    pub(crate) coeff:    usize // Solves some weird bug :)
}

impl<T> JpegDecoder<T>
where
    T: ZReaderTrait
{
    #[allow(clippy::redundant_field_names)]
    fn default(options: DecoderOptions, buffer: T) -> Self {
        let color_convert = choose_ycbcr_to_rgb_convert_func(ColorSpace::RGB, &options).unwrap();
        JpegDecoder {
            info:              ImageInfo::default(),
            qt_tables:         [None, None, None, None],
            dc_huffman_tables: [None, None, None, None],
            ac_huffman_tables: [None, None, None, None],
            components:        vec![],
            // Interleaved information
            h_max:             1,
            v_max:             1,
            mcu_height:        0,
            mcu_width:         0,
            mcu_x:             0,
            mcu_y:             0,
            is_interleaved:    false,
            sub_sample_ratio:  SampleRatios::None,
            is_progressive:    false,
            spec_start:        0,
            spec_end:          0,
            succ_high:         0,
            succ_low:          0,
            num_scans:         0,
            idct_func:         choose_idct_func(&options),
            color_convert_16:  color_convert,
            input_colorspace:  ColorSpace::YCbCr,
            z_order:           [0; MAX_COMPONENTS],
            restart_interval:  0,
            todo:              0x7fff_ffff,
            options:           options,
            stream:            ZByteReader::new(buffer),
            headers_decoded:   false,
            seen_sof:          false,
            exif_data:         None,
            icc_data:          vec![],
            is_mjpeg:          false,
            coeff:             1
        }
    }
    /// Decode a buffer already in memory
    ///
    /// The buffer should be a valid jpeg file, perhaps created by the command
    /// `std:::fs::read()` or a JPEG file downloaded from the internet.
    ///
    /// # Errors
    /// See DecodeErrors for an explanation
    pub fn decode(&mut self) -> Result<Vec<u8>, DecodeErrors> {
        self.decode_headers()?;
        let size = self.output_buffer_size().unwrap();
        let mut out = vec![0; size];
        self.decode_into(&mut out)?;
        Ok(out)
    }

    /// Create a new Decoder instance
    ///
    /// # Arguments
    ///  - `stream`: The raw bytes of a jpeg file.
    #[must_use]
    #[allow(clippy::new_without_default)]
    pub fn new(stream: T) -> JpegDecoder<T> {
        JpegDecoder::default(DecoderOptions::default(), stream)
    }

    /// Returns the image information
    ///
    /// This **must** be called after a subsequent call to [`decode`] or [`decode_headers`]
    /// it will return `None`
    ///
    /// # Returns
    /// - `Some(info)`: Image information,width, height, number of components
    /// - None: Indicates image headers haven't been decoded
    ///
    /// [`decode`]: JpegDecoder::decode
    /// [`decode_headers`]: JpegDecoder::decode_headers
    #[must_use]
    pub fn info(&self) -> Option<ImageInfo> {
        // we check for fails to that call by comparing what we have to the default, if
        // it's default we assume that the caller failed to uphold the
        // guarantees. We can be sure that an image cannot be the default since
        // its a hard panic in-case width or height are set to zero.
        if !self.headers_decoded {
            return None;
        }

        return Some(self.info.clone());
    }

    /// Return the number of bytes required to hold a decoded image frame
    /// decoded using the given input transformations
    ///
    /// # Returns
    ///  - `Some(usize)`: Minimum size for a buffer needed to decode the image
    ///  - `None`: Indicates the image was not decoded, or image dimensions would overflow a usize
    ///
    #[must_use]
    pub fn output_buffer_size(&self) -> Option<usize> {
        return if self.headers_decoded {
            Some(
                usize::from(self.width())
                    .checked_mul(usize::from(self.height()))?
                    .checked_mul(self.options.jpeg_get_out_colorspace().num_components())?
            )
        } else {
            None
        };
    }

    /// Get a mutable reference to the decoder options
    /// for the decoder instance
    ///
    /// This can be used to modify options before actual decoding
    /// but after initial creation
    ///
    /// # Example
    /// ```no_run
    /// use zune_jpeg::JpegDecoder;
    ///
    /// let mut decoder = JpegDecoder::new(&[]);
    /// // get current options
    /// let mut options = decoder.get_options();
    /// // modify it
    ///  let new_options = options.set_max_width(10);
    /// // set it back
    /// decoder.set_options(new_options);
    ///
    /// ```
    #[must_use]
    pub const fn get_options(&self) -> &DecoderOptions {
        &self.options
    }
    /// Return the input colorspace of the image
    ///
    /// This indicates the colorspace that is present in
    /// the image, but this may be different to the colorspace that
    /// the output will be transformed to
    ///
    /// # Returns
    /// -`Some(Colorspace)`: Input colorspace
    /// - None : Indicates the headers weren't decoded
    #[must_use]
    pub fn get_input_colorspace(&self) -> Option<ColorSpace> {
        return if self.headers_decoded { Some(self.input_colorspace) } else { None };
    }
    /// Set decoder options
    ///
    /// This can be used to set new options even after initialization
    /// but before decoding.
    ///
    /// This does not bear any significance after decoding an image
    ///
    /// # Arguments
    /// - `options`: New decoder options
    ///
    /// # Example
    /// Set maximum jpeg progressive passes to be 4
    ///
    /// ```no_run
    /// use zune_jpeg::JpegDecoder;
    /// let mut decoder =JpegDecoder::new(&[]);
    /// // this works also because DecoderOptions implements `Copy`
    /// let options = decoder.get_options().jpeg_set_max_scans(4);
    /// // set the new options
    /// decoder.set_options(options);
    /// // now decode
    /// decoder.decode().unwrap();
    /// ```
    pub fn set_options(&mut self, options: DecoderOptions) {
        self.options = options;
    }
    /// Decode Decoder headers
    ///
    /// This routine takes care of parsing supported headers from a Decoder
    /// image
    ///
    /// # Supported Headers
    ///  - APP(0)
    ///  - SOF(O)
    ///  - DQT -> Quantization tables
    ///  - DHT -> Huffman tables
    ///  - SOS -> Start of Scan
    /// # Unsupported Headers
    ///  - SOF(n) -> Decoder images which are not baseline/progressive
    ///  - DAC -> Images using Arithmetic tables
    ///  - JPG(n)
    fn decode_headers_internal(&mut self) -> Result<(), DecodeErrors> {
        if self.headers_decoded {
            trace!("Headers decoded!");
            return Ok(());
        }
        // match output colorspace here
        // we know this will only be called once per image
        // so makes sense
        // We only care for ycbcr to rgb/rgba here
        // in case one is using another colorspace.
        // May god help you
        let out_colorspace = self.options.jpeg_get_out_colorspace();

        if matches!(
            out_colorspace,
            ColorSpace::BGR | ColorSpace::BGRA | ColorSpace::RGB | ColorSpace::RGBA
        ) {
            self.color_convert_16 = choose_ycbcr_to_rgb_convert_func(
                self.options.jpeg_get_out_colorspace(),
                &self.options
            )
            .unwrap();
        }
        // First two bytes should be jpeg soi marker
        let magic_bytes = self.stream.get_u16_be_err()?;

        let mut last_byte = 0;
        let mut bytes_before_marker = 0;

        if magic_bytes != 0xffd8 {
            return Err(DecodeErrors::IllegalMagicBytes(magic_bytes));
        }

        loop {
            // read a byte
            let mut m = self.stream.get_u8_err()?;

            // AND OF COURSE some images will have fill bytes in their marker
            // bitstreams because why not.
            //
            // I am disappointed as a man.
            if (m == 0xFF || m == 0) && last_byte == 0xFF {
                // This handles the edge case where
                // images have markers with fill bytes(0xFF)
                // or byte stuffing (0)
                // I.e 0xFF 0xFF 0xDA
                // and
                // 0xFF 0 0xDA
                // It should ignore those fill bytes and take 0xDA
                // I don't know why such images exist
                // but they do.
                // so this is for you (with love)
                while m == 0xFF || m == 0x0 {
                    last_byte = m;
                    m = self.stream.get_u8_err()?;
                }
            }
            // Last byte should be 0xFF to confirm existence of a marker since markers look
            // like OxFF(some marker data)
            if last_byte == 0xFF {
                let marker = Marker::from_u8(m);
                if let Some(n) = marker {
                    if bytes_before_marker > 3 {
                        if self.options.get_strict_mode()
                        /*No reason to use this*/
                        {
                            return Err(DecodeErrors::FormatStatic(
                                "[strict-mode]: Extra bytes between headers"
                            ));
                        }

                        error!(
                            "Extra bytes {} before marker 0xFF{:X}",
                            bytes_before_marker - 3,
                            m
                        );
                    }

                    bytes_before_marker = 0;

                    self.parse_marker_inner(n)?;

                    if n == Marker::SOS {
                        self.headers_decoded = true;
                        trace!("Input colorspace {:?}", self.input_colorspace);
                        return Ok(());
                    }
                } else {
                    bytes_before_marker = 0;

                    warn!("Marker 0xFF{:X} not known", m);

                    let length = self.stream.get_u16_be_err()?;

                    if length < 2 {
                        return Err(DecodeErrors::Format(format!(
                            "Found a marker with invalid length : {length}"
                        )));
                    }

                    warn!("Skipping {} bytes", length - 2);
                    self.stream.skip((length - 2) as usize);
                }
            }
            last_byte = m;
            bytes_before_marker += 1;
        }
    }
    #[allow(clippy::too_many_lines)]
    pub(crate) fn parse_marker_inner(&mut self, m: Marker) -> Result<(), DecodeErrors> {
        match m {
            Marker::SOF(0..=2) => {
                let marker = {
                    // choose marker
                    if m == Marker::SOF(0) || m == Marker::SOF(1) {
                        SOFMarkers::BaselineDct
                    } else {
                        self.is_progressive = true;
                        SOFMarkers::ProgressiveDctHuffman
                    }
                };

                trace!("Image encoding scheme =`{:?}`", marker);
                // get components
                parse_start_of_frame(marker, self)?;
            }
            // Start of Frame Segments not supported
            Marker::SOF(v) => {
                let feature = UnsupportedSchemes::from_int(v);

                if let Some(feature) = feature {
                    return Err(DecodeErrors::Unsupported(feature));
                }

                return Err(DecodeErrors::Format("Unsupported image format".to_string()));
            }
            //APP(0) segment
            Marker::APP(0) => {
                let mut length = self.stream.get_u16_be_err()?;

                if length < 2 {
                    return Err(DecodeErrors::Format(format!(
                        "Found a marker with invalid length:{length}\n"
                    )));
                }
                // skip for now
                if length > 5 && self.stream.has(5) {
                    let mut buffer = [0u8; 5];
                    self.stream.read_exact(&mut buffer).unwrap();
                    if &buffer == b"AVI1\0" {
                        self.is_mjpeg = true;
                    }
                    length -= 5;
                }
                self.stream.skip(length.saturating_sub(2) as usize);

                //parse_app(buf, m, &mut self.info)?;
            }
            Marker::APP(1) => {
                parse_app1(self)?;
            }

            Marker::APP(2) => {
                parse_app2(self)?;
            }
            // Quantization tables
            Marker::DQT => {
                parse_dqt(self)?;
            }
            // Huffman tables
            Marker::DHT => {
                parse_huffman(self)?;
            }
            // Start of Scan Data
            Marker::SOS => {
                parse_sos(self)?;

                // break after reading the start of scan.
                // what follows is the image data
                return Ok(());
            }
            Marker::EOI => return Err(DecodeErrors::FormatStatic("Premature End of image")),

            Marker::DAC | Marker::DNL => {
                return Err(DecodeErrors::Format(format!(
                    "Parsing of the following header `{m:?}` is not supported,\
                                cannot continue"
                )));
            }
            Marker::DRI => {
                trace!("DRI marker present");

                if self.stream.get_u16_be_err()? != 4 {
                    return Err(DecodeErrors::Format(
                        "Bad DRI length, Corrupt JPEG".to_string()
                    ));
                }

                self.restart_interval = usize::from(self.stream.get_u16_be_err()?);
                self.todo = self.restart_interval;
            }
            Marker::APP(14) => {
                parse_app14(self)?;
            }
            _ => {
                warn!(
                    "Capabilities for processing marker \"{:?}\" not implemented",
                    m
                );

                let length = self.stream.get_u16_be_err()?;

                if length < 2 {
                    return Err(DecodeErrors::Format(format!(
                        "Found a marker with invalid length:{length}\n"
                    )));
                }
                warn!("Skipping {} bytes", length - 2);
                self.stream.skip((length - 2) as usize);
            }
        }
        Ok(())
    }
    /// Get the embedded ICC profile if it exists
    /// and is correct
    ///
    /// One needs not to decode the whole image to extract this,
    /// calling [`decode_headers`] for an image with an ICC profile
    /// allows you to decode this
    ///
    /// # Returns
    /// - `Some(Vec<u8>)`: The raw ICC profile of the image
    /// - `None`: May indicate an error  in the ICC profile , non-existence of
    /// an ICC profile, or that the headers weren't decoded.
    ///
    /// [`decode_headers`]:Self::decode_headers
    #[must_use]
    pub fn icc_profile(&self) -> Option<Vec<u8>> {
        let mut marker_present: [Option<&ICCChunk>; 256] = [None; 256];

        if !self.headers_decoded {
            return None;
        }
        let num_markers = self.icc_data.len();

        if num_markers == 0 || num_markers >= 255 {
            return None;
        }
        // check validity
        for chunk in &self.icc_data {
            if usize::from(chunk.num_markers) != num_markers {
                // all the lengths must match
                return None;
            }
            if chunk.seq_no == 0 {
                warn!("Zero sequence number in ICC, corrupt ICC chunk");
                return None;
            }
            if marker_present[usize::from(chunk.seq_no)].is_some() {
                // duplicate seq_no
                warn!("Duplicate sequence number in ICC, corrupt chunk");
                return None;
            }

            marker_present[usize::from(chunk.seq_no)] = Some(chunk);
        }
        let mut data = Vec::with_capacity(1000);
        // assemble the data now
        for chunk in marker_present.get(1..=num_markers).unwrap() {
            if let Some(ch) = chunk {
                data.extend_from_slice(&ch.data);
            } else {
                warn!("Missing icc sequence number, corrupt ICC chunk ");
                return None;
            }
        }

        Some(data)
    }
    /// Return the exif data for the file
    ///
    /// This returns the raw exif data starting at the
    /// TIFF header
    ///
    /// # Returns
    /// -`Some(data)`: The raw exif data, if present in the image
    /// - None: May indicate the following
    ///
    ///    1. The image doesn't have exif data
    ///    2. The image headers haven't been decoded
    #[must_use]
    pub fn exif(&self) -> Option<&Vec<u8>> {
        return self.exif_data.as_ref();
    }
    /// Get the output colorspace the image pixels will be decoded into
    ///
    ///
    /// # Note.
    /// This field can only be regarded after decoding headers,
    /// as markers such as Adobe APP14 may dictate different colorspaces
    /// than requested.
    ///
    /// Calling `decode_headers` is sufficient to know what colorspace the
    /// output is, if this is called after `decode` it indicates the colorspace
    /// the output is currently in
    ///
    /// Additionally not all input->output colorspace mappings are supported
    /// but all input colorspaces can map to RGB colorspace, so that's a safe bet
    /// if one is handling image formats
    ///
    ///# Returns
    /// - `Some(Colorspace)`: If headers have been decoded, the colorspace the
    ///output array will be in
    ///- `None
    #[must_use]
    pub fn get_output_colorspace(&self) -> Option<ColorSpace> {
        return if self.headers_decoded {
            Some(self.options.jpeg_get_out_colorspace())
        } else {
            None
        };
    }

    /// Decode into a pre-allocated buffer
    ///
    /// It is an error if the buffer size is smaller than
    /// [`output_buffer_size()`](Self::output_buffer_size)
    ///
    /// If the buffer is bigger than expected, we ignore the end padding bytes
    ///
    /// # Example
    ///
    /// - Read  headers and then alloc a buffer big enough to hold the image
    ///
    /// ```no_run
    /// use zune_jpeg::JpegDecoder;
    /// let mut decoder = JpegDecoder::new(&[]);
    /// // before we get output, we must decode the headers to get width
    /// // height, and input colorspace
    /// decoder.decode_headers().unwrap();
    ///
    /// let mut out = vec![0;decoder.output_buffer_size().unwrap()];
    /// // write into out
    /// decoder.decode_into(&mut out).unwrap();
    /// ```
    ///
    ///
    pub fn decode_into(&mut self, out: &mut [u8]) -> Result<(), DecodeErrors> {
        self.decode_headers_internal()?;

        let expected_size = self.output_buffer_size().unwrap();

        if out.len() < expected_size {
            // too small of a size
            return Err(DecodeErrors::TooSmallOutput(expected_size, out.len()));
        }

        // ensure we don't touch anyone else's scratch space
        let out_len = core::cmp::min(out.len(), expected_size);
        let out = &mut out[0..out_len];

        if self.is_progressive {
            self.decode_mcu_ycbcr_progressive(out)
        } else {
            self.decode_mcu_ycbcr_baseline(out)
        }
    }

    /// Read only headers from a jpeg image buffer
    ///
    /// This allows you to extract important information like
    /// image width and height without decoding the full image
    ///
    /// # Examples
    /// ```no_run
    /// use zune_jpeg::{JpegDecoder};
    ///
    /// let img_data = std::fs::read("a_valid.jpeg").unwrap();
    /// let mut decoder = JpegDecoder::new(&img_data);
    /// decoder.decode_headers().unwrap();
    ///
    /// println!("Total decoder dimensions are : {:?} pixels",decoder.dimensions());
    /// println!("Number of components in the image are {}", decoder.info().unwrap().components);
    /// ```
    /// # Errors
    /// See DecodeErrors enum for list of possible errors during decoding
    pub fn decode_headers(&mut self) -> Result<(), DecodeErrors> {
        self.decode_headers_internal()?;
        Ok(())
    }
    /// Create a new decoder with the specified options to be used for decoding
    /// an image
    ///
    /// # Arguments
    /// - `buf`: The input buffer from where we will pull in compressed jpeg bytes from
    /// - `options`: Options specific to this decoder instance
    #[must_use]
    pub fn new_with_options(buf: T, options: DecoderOptions) -> JpegDecoder<T> {
        JpegDecoder::default(options, buf)
    }

    /// Set up-sampling routines in case an image is down sampled
    pub(crate) fn set_upsampling(&mut self) -> Result<(), DecodeErrors> {
        // no sampling, return early
        // check if horizontal max ==1
        if self.h_max == self.v_max && self.h_max == 1 {
            return Ok(());
        }
        match (self.h_max, self.v_max) {
            (1, 1) => {
                self.sub_sample_ratio = SampleRatios::None;
            }
            (1, 2) => {
                self.sub_sample_ratio = SampleRatios::V;
            }
            (2, 1) => {
                self.sub_sample_ratio = SampleRatios::H;
            }
            (2, 2) => {
                self.sub_sample_ratio = SampleRatios::HV;
            }
            (hs, vs) => {
                self.sub_sample_ratio = SampleRatios::Generic(hs, vs)
                // return Err(DecodeErrors::Format(format!(
                //     "Unknown down-sampling method ({hs},{vs}), cannot continue")
                // ))
            }
        }

        for comp in self.components.iter_mut() {
            let hs = self.h_max / comp.horizontal_sample;
            let vs = self.v_max / comp.vertical_sample;

            let samp_factor = match (hs, vs) {
                (1, 1) => {
                    comp.sample_ratio = SampleRatios::None;
                    upsample_no_op
                }
                (2, 1) => {
                    comp.sample_ratio = SampleRatios::H;
                    choose_horizontal_samp_function(self.options.get_use_unsafe())
                }
                (1, 2) => {
                    comp.sample_ratio = SampleRatios::V;
                    choose_v_samp_function(self.options.get_use_unsafe())
                }
                (2, 2) => {
                    comp.sample_ratio = SampleRatios::HV;
                    choose_hv_samp_function(self.options.get_use_unsafe())
                }
                (hs, vs) => {

                    comp.sample_ratio = SampleRatios::Generic(hs, vs);
                    generic_sampler()
                }
            };
            comp.setup_upsample_scanline();
            comp.up_sampler = samp_factor;
        }

        return Ok(());
    }
    #[must_use]
    /// Get the width of the image as a u16
    ///
    /// The width lies between 1 and 65535
    pub(crate) fn width(&self) -> u16 {
        self.info.width
    }

    /// Get the height of the image as a u16
    ///
    /// The height lies between 1 and 65535
    #[must_use]
    pub(crate) fn height(&self) -> u16 {
        self.info.height
    }

    /// Get image dimensions as a tuple of width and height
    /// or `None` if the image hasn't been decoded.
    ///
    /// # Returns
    /// - `Some(width,height)`: Image dimensions
    /// -  None : The image headers haven't been decoded
    #[must_use]
    pub const fn dimensions(&self) -> Option<(usize, usize)> {
        return if self.headers_decoded {
            Some((self.info.width as usize, self.info.height as usize))
        } else {
            None
        };
    }
}

/// A struct representing Image Information
#[derive(Default, Clone, Eq, PartialEq)]
#[allow(clippy::module_name_repetitions)]
pub struct ImageInfo {
    /// Width of the image
    pub width:         u16,
    /// Height of image
    pub height:        u16,
    /// PixelDensity
    pub pixel_density: u8,
    /// Start of frame markers
    pub sof:           SOFMarkers,
    /// Horizontal sample
    pub x_density:     u16,
    /// Vertical sample
    pub y_density:     u16,
    /// Number of components
    pub components:    u8
}

impl ImageInfo {
    /// Set width of the image
    ///
    /// Found in the start of frame

    pub(crate) fn set_width(&mut self, width: u16) {
        self.width = width;
    }

    /// Set height of the image
    ///
    /// Found in the start of frame

    pub(crate) fn set_height(&mut self, height: u16) {
        self.height = height;
    }

    /// Set the image density
    ///
    /// Found in the start of frame

    pub(crate) fn set_density(&mut self, density: u8) {
        self.pixel_density = density;
    }

    /// Set image Start of frame marker
    ///
    /// found in the Start of frame header

    pub(crate) fn set_sof_marker(&mut self, marker: SOFMarkers) {
        self.sof = marker;
    }

    /// Set image x-density(dots per pixel)
    ///
    /// Found in the APP(0) marker
    #[allow(dead_code)]
    pub(crate) fn set_x(&mut self, sample: u16) {
        self.x_density = sample;
    }

    /// Set image y-density
    ///
    /// Found in the APP(0) marker
    #[allow(dead_code)]
    pub(crate) fn set_y(&mut self, sample: u16) {
        self.y_density = sample;
    }
}
