/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//!Routines for progressive decoding
/*
This file is needlessly complicated,

It is that way to ensure we don't burn memory anyhow

Memory is a scarce resource in some environments, I would like this to be viable
in such environments

Half of the complexity comes from the jpeg spec, because progressive decoding,
is one hell of a ride.

*/
use alloc::string::ToString;
use alloc::vec::Vec;
use alloc::{format, vec};
use core::cmp::min;
use zune_core::bytestream::{ZByteReader, ZReaderTrait};
use zune_core::colorspace::ColorSpace;
use zune_core::log::{debug, error, warn};

use crate::bitstream::BitStream;
use crate::components::{ComponentID, SampleRatios};
use crate::decoder::{JpegDecoder, MAX_COMPONENTS};
use crate::errors::DecodeErrors;
use crate::headers::{parse_sos};
use crate::marker::Marker;
use crate::mcu::DCT_BLOCK;
use crate::misc::{calculate_padded_width, setup_component_params};

impl<T: ZReaderTrait> JpegDecoder<T> {
    /// Decode a progressive image
    ///
    /// This routine decodes a progressive image, stopping if it finds any error.
    #[allow(
        clippy::needless_range_loop,
        clippy::cast_sign_loss,
        clippy::redundant_else,
        clippy::too_many_lines
    )]
    #[inline(never)]
    pub(crate) fn decode_mcu_ycbcr_progressive(
        &mut self, pixels: &mut [u8],
    ) -> Result<(), DecodeErrors> {
        setup_component_params(self)?;

        let mut mcu_height;

        // memory location for decoded pixels for components
        let mut block: [Vec<i16>; MAX_COMPONENTS] = [vec![], vec![], vec![], vec![]];
        let mut mcu_width;

        let mut seen_scans = 1;

        if self.input_colorspace == ColorSpace::Luma && self.is_interleaved {
            warn!("Grayscale image with down-sampled component, resetting component details");
            self.reset_params();
        }

        if self.is_interleaved {
            // this helps us catch component errors.
            self.set_upsampling()?;
        }
        if self.is_interleaved {
            mcu_width = self.mcu_x;
            mcu_height = self.mcu_y;
        } else {
            mcu_width = (self.info.width as usize + 7) / 8;
            mcu_height = (self.info.height as usize + 7) / 8;
        }
        if self.is_interleaved
            && self.input_colorspace.num_components() > 1
            && self.options.jpeg_get_out_colorspace().num_components() == 1
            && (self.sub_sample_ratio == SampleRatios::V
                || self.sub_sample_ratio == SampleRatios::HV)
        {
            // For a specific set of images, e.g interleaved,
            // when converting from YcbCr to grayscale, we need to
            // take into account mcu height since the MCU decoding needs to take
            // it into account for padding purposes and the post processor
            // parses two rows per mcu width.
            //
            // set coeff to be 2 to ensure that we increment two rows
            // for every mcu processed also
            mcu_height *= self.v_max;
            mcu_height /= self.h_max;
            self.coeff = 2;
        }

        mcu_width *= 64;


        for i in 0..self.input_colorspace.num_components() {
            let comp = &self.components[i];
            let len = mcu_width * comp.vertical_sample * comp.horizontal_sample * mcu_height;

            block[i] = vec![0; len];
        }

        let mut stream = BitStream::new_progressive(
            self.succ_high,
            self.succ_low,
            self.spec_start,
            self.spec_end
        );

        // there are multiple scans in the stream, this should resolve the first scan
        self.parse_entropy_coded_data(&mut stream, &mut block)?;

        // extract marker
        let mut marker = stream
            .marker
            .take()
            .ok_or(DecodeErrors::FormatStatic("Marker missing where expected"))?;

        // if marker is EOI, we are done, otherwise continue scanning.
        //
        // In case we have a premature image, we print a warning or return
        // an error, depending on the strictness of the decoder, so there
        // is that logic to handle too
        'eoi: while marker != Marker::EOI {
            match marker {
                Marker::SOS => {
                    parse_sos(self)?;

                    stream.update_progressive_params(
                        self.succ_high,
                        self.succ_low,
                        self.spec_start,
                        self.spec_end
                    );
                    // after every SOS, marker, parse data for that scan.
                    self.parse_entropy_coded_data(&mut stream, &mut block)?;
                    // extract marker, might either indicate end of image or we continue
                    // scanning(hence the continue statement to determine).
                    match get_marker(&mut self.stream, &mut stream) {
                        Ok(marker_n) => {
                            marker = marker_n;
                            seen_scans += 1;
                            if seen_scans > self.options.jpeg_get_max_scans() {
                                return Err(DecodeErrors::Format(format!(
                                    "Too many scans, exceeded limit of {}",
                                    self.options.jpeg_get_max_scans()
                                )));
                            }

                            stream.reset();
                            continue 'eoi;
                        }
                        Err(msg) => {
                            if self.options.get_strict_mode() {
                                return Err(msg);
                            }
                            error!("{:?}", msg);
                            break 'eoi;
                        }
                    }
                }
                Marker::RST(_n) => {
                    self.handle_rst(&mut stream)?;
                }
                _ => {
                    self.parse_marker_inner(marker)?;
                }
            }

            match get_marker(&mut self.stream, &mut stream) {
                Ok(marker_n) => {
                    marker = marker_n;
                }
                Err(e) => {
                    if self.options.get_strict_mode() {
                        return Err(e);
                    }
                    error!("{}", e);
                }
            }
        }

        self.finish_progressive_decoding(&block, mcu_width, pixels)
    }

    /// Reset progressive parameters
    fn reset_prog_params(&mut self, stream: &mut BitStream) {
        stream.reset();
        self.components.iter_mut().for_each(|x| x.dc_pred = 0);

        // Also reset JPEG restart intervals
        self.todo = if self.restart_interval != 0 { self.restart_interval } else { usize::MAX };
    }

    #[allow(clippy::too_many_lines, clippy::cast_sign_loss)]
    fn parse_entropy_coded_data(
        &mut self, stream: &mut BitStream, buffer: &mut [Vec<i16>; MAX_COMPONENTS]
    ) -> Result<(), DecodeErrors> {
        self.reset_prog_params(stream);

        if usize::from(self.num_scans) > self.input_colorspace.num_components() {
            return Err(DecodeErrors::Format(format!(
                "Number of scans {} cannot be greater than number of components, {}",
                self.num_scans,
                self.input_colorspace.num_components()
            )));
        }
        if self.num_scans == 1 {
            // Safety checks
            if self.spec_end != 0 && self.spec_start == 0 {
                return Err(DecodeErrors::FormatStatic(
                    "Can't merge DC and AC corrupt jpeg"
                ));
            }
            // non interleaved data, process one block at a time in trivial scanline order

            let k = self.z_order[0];

            if k >= self.components.len() {
                return Err(DecodeErrors::Format(format!(
                    "Cannot find component {k}, corrupt image"
                )));
            }

            let (mcu_width, mcu_height);

            if self.components[k].component_id == ComponentID::Y
                && (self.components[k].vertical_sample != 1
                    || self.components[k].horizontal_sample != 1)
                || !self.is_interleaved
            {
                // For Y channel  or non interleaved scans ,
                // mcu's is the image dimensions divided by 8
                mcu_width = ((self.info.width + 7) / 8) as usize;
                mcu_height = ((self.info.height + 7) / 8) as usize;
            } else {
                // For other channels, in an interleaved mcu, number of MCU's
                // are determined by some weird maths done in headers.rs->parse_sos()
                mcu_width = self.mcu_x;
                mcu_height = self.mcu_y;
            }

            for i in 0..mcu_height {
                for j in 0..mcu_width {
                    if self.spec_start != 0 && self.succ_high == 0 && stream.eob_run > 0 {
                        // handle EOB runs here.
                        stream.eob_run -= 1;
                    } else {
                        let start = 64 * (j + i * (self.components[k].width_stride / 8));

                        let data: &mut [i16; 64] = buffer
                            .get_mut(k)
                            .unwrap()
                            .get_mut(start..start + 64)
                            .unwrap()
                            .try_into()
                            .unwrap();

                        if self.spec_start == 0 {
                            let pos = self.components[k].dc_huff_table & (MAX_COMPONENTS - 1);
                            let dc_table = self
                                .dc_huffman_tables
                                .get(pos)
                                .ok_or(DecodeErrors::FormatStatic(
                                    "No huffman table for DC component"
                                ))?
                                .as_ref()
                                .ok_or(DecodeErrors::FormatStatic(
                                    "Huffman table at index  {} not initialized"
                                ))?;

                            let dc_pred = &mut self.components[k].dc_pred;

                            if self.succ_high == 0 {
                                // first scan for this mcu
                                stream.decode_prog_dc_first(
                                    &mut self.stream,
                                    dc_table,
                                    &mut data[0],
                                    dc_pred
                                )?;
                            } else {
                                // refining scans for this MCU
                                stream.decode_prog_dc_refine(&mut self.stream, &mut data[0])?;
                            }
                        } else {
                            let pos = self.components[k].ac_huff_table;
                            let ac_table = self
                                .ac_huffman_tables
                                .get(pos)
                                .ok_or_else(|| {
                                    DecodeErrors::Format(format!(
                                        "No huffman table for component:{pos}"
                                    ))
                                })?
                                .as_ref()
                                .ok_or_else(|| {
                                    DecodeErrors::Format(format!(
                                        "Huffman table at index  {pos} not initialized"
                                    ))
                                })?;

                            if self.succ_high == 0 {
                                debug_assert!(stream.eob_run == 0, "EOB run is not zero");

                                stream.decode_mcu_ac_first(&mut self.stream, ac_table, data)?;
                            } else {
                                // refinement scan
                                stream.decode_mcu_ac_refine(&mut self.stream, ac_table, data)?;
                            }
                        }
                    }

                    // + EOB and investigate effect.
                    self.todo -= 1;

                    self.handle_rst_main(stream)?;
                }
            }
        } else {
            if self.spec_end != 0 {
                return Err(DecodeErrors::HuffmanDecode(
                    "Can't merge dc and AC corrupt jpeg".to_string()
                ));
            }
            // process scan n elements in order

            // Do the error checking with allocs here.
            // Make the one in the inner loop free of allocations.
            for k in 0..self.num_scans {
                let n = self.z_order[k as usize];

                if n >= self.components.len() {
                    return Err(DecodeErrors::Format(format!(
                        "Cannot find component {n}, corrupt image"
                    )));
                }

                let component = &mut self.components[n];
                let _ = self
                    .dc_huffman_tables
                    .get(component.dc_huff_table)
                    .ok_or_else(|| {
                        DecodeErrors::Format(format!(
                            "No huffman table for component:{}",
                            component.dc_huff_table
                        ))
                    })?
                    .as_ref()
                    .ok_or_else(|| {
                        DecodeErrors::Format(format!(
                            "Huffman table at index  {} not initialized",
                            component.dc_huff_table
                        ))
                    })?;
            }
            // Interleaved scan

            // Components shall not be interleaved in progressive mode, except for
            // the DC coefficients in the first scan for each component of a progressive frame.
            for i in 0..self.mcu_y {
                for j in 0..self.mcu_x {
                    // process scan n elements in order
                    for k in 0..self.num_scans {
                        let n = self.z_order[k as usize];
                        let component = &mut self.components[n];
                        let huff_table = self
                            .dc_huffman_tables
                            .get(component.dc_huff_table)
                            .ok_or(DecodeErrors::FormatStatic("No huffman table for component"))?
                            .as_ref()
                            .ok_or(DecodeErrors::FormatStatic(
                                "Huffman table at index not initialized"
                            ))?;

                        for v_samp in 0..component.vertical_sample {
                            for h_samp in 0..component.horizontal_sample {
                                let x2 = j * component.horizontal_sample + h_samp;
                                let y2 = i * component.vertical_sample + v_samp;
                                let position = 64 * (x2 + y2 * component.width_stride / 8);
                                let buf_n = &mut buffer[n];

                                let Some(data) = &mut buf_n.get_mut(position) else {
                                    // TODO: (CAE), this is another weird sub-sampling bug, so on fix
                                    // remove this
                                    return Err(DecodeErrors::FormatStatic("Invalid image"));
                                };

                                if self.succ_high == 0 {
                                    stream.decode_prog_dc_first(
                                        &mut self.stream,
                                        huff_table,
                                        data,
                                        &mut component.dc_pred
                                    )?;
                                } else {
                                    stream.decode_prog_dc_refine(&mut self.stream, data)?;
                                }
                            }
                        }
                    }
                    // We want wrapping subtraction here because it means
                    // we get a higher number in the case this underflows
                    self.todo -= 1;
                    // after every scan that's a mcu, count down restart markers.
                    self.handle_rst_main(stream)?;
                }
            }
        }
        return Ok(());
    }

    pub(crate) fn handle_rst_main(&mut self, stream: &mut BitStream) -> Result<(), DecodeErrors> {
        if self.todo == 0 {
            stream.refill(&mut self.stream)?;
        }

        if self.todo == 0
            && self.restart_interval != 0
            && stream.marker.is_none()
            && !stream.seen_eoi
        {
            // if no marker and we are to reset RST, look for the marker, this matches
            // libjpeg-turbo behaviour and allows us to decode images in
            // https://github.com/etemesi254/zune-image/issues/261
            let _start = self.stream.get_position();
            // skip bytes until we find marker
            let marker = get_marker(&mut self.stream, stream)?;
            let _end = self.stream.get_position();
            stream.marker = Some(marker);
            // NB some warnings may be false positives.
            warn!(
                "{} Extraneous bytes before marker {:?}",
                _end - _start,
                marker
            );
        }
        if self.todo == 0 {
            self.handle_rst(stream)?
        }
        Ok(())
    }
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::needless_range_loop, clippy::cast_sign_loss)]
    fn finish_progressive_decoding(
        &mut self, block: &[Vec<i16>; MAX_COMPONENTS], _mcu_width: usize, pixels: &mut [u8]
    ) -> Result<(), DecodeErrors> {
        // This function is complicated because we need to replicate
        // the function in mcu.rs
        //
        // The advantage is that we do very little allocation and very lot
        // channel reusing.
        // The trick is to notice that we repeat the same procedure per MCU
        // width.
        //
        // So we can set it up that we only allocate temporary storage large enough
        // to store a single mcu width, then reuse it per invocation.
        //
        // This is advantageous to us.
        //
        // Remember we need to have the whole MCU buffer so we store 3 unprocessed
        // channels in memory, and then we allocate the whole output buffer in memory, both of
        // which are huge.
        //
        //

        let mcu_height = if self.is_interleaved {
            self.mcu_y
        } else {
            // For non-interleaved images( (1*1) subsampling)
            // number of MCU's are the widths (+7 to account for paddings) divided by 8.
            ((self.info.height + 7) / 8) as usize
        };

        // Size of our output image(width*height)
        let is_hv = usize::from(self.is_interleaved);
        let upsampler_scratch_size = is_hv * self.components[0].width_stride;
        let width = usize::from(self.info.width);
        let padded_width = calculate_padded_width(width, self.sub_sample_ratio);

        //let mut pixels = vec![0; capacity * out_colorspace_components];
        let mut upsampler_scratch_space = vec![0; upsampler_scratch_size];
        let mut tmp = [0_i32; DCT_BLOCK];

        for (pos, comp) in self.components.iter_mut().enumerate() {
            // Allocate only needed components.
            //
            // For special colorspaces i.e YCCK and CMYK, just allocate all of the needed
            // components.
            if min(
                self.options.jpeg_get_out_colorspace().num_components() - 1,
                pos,
            ) == pos
                || self.input_colorspace == ColorSpace::YCCK
                || self.input_colorspace == ColorSpace::CMYK
            {
                // allocate enough space to hold a whole MCU width
                // this means we should take into account sampling ratios
                // `*8` is because each MCU spans 8 widths.
                let len = comp.width_stride * comp.vertical_sample * 8;

                comp.needed = true;
                comp.raw_coeff = vec![0; len];
            } else {
                comp.needed = false;
            }
        }

        let mut pixels_written = 0;

        // dequantize, idct and color convert.
        for i in 0..mcu_height {
            'component: for (position, component) in &mut self.components.iter_mut().enumerate() {
                if !component.needed {
                    continue 'component;
                }
                let qt_table = &component.quantization_table;

                // step is the number of pixels this iteration wil be handling
                // Given by the number of mcu's height and the length of the component block
                // Since the component block contains the whole channel as raw pixels
                // we this evenly divides the pixels into MCU blocks
                //
                // For interleaved images, this gives us the exact pixels comprising a whole MCU
                // block
                let step = block[position].len() / mcu_height;
                // where we will be reading our pixels from.
                let start = i * step;

                let slice = &block[position][start..start + step];

                let temp_channel = &mut component.raw_coeff;

                // The next logical step is to iterate width wise.
                // To figure out how many pixels we iterate by we use effective pixels
                // Given to us by component.x
                // iterate per effective pixels.
                let mcu_x = component.width_stride / 8;

                // iterate per every vertical sample.
                for k in 0..component.vertical_sample {
                    for j in 0..mcu_x {
                        // after writing a single stride, we need to skip 8 rows.
                        // This does the row calculation
                        let width_stride = k * 8 * component.width_stride;
                        let start = j * 64 + width_stride;

                        // See https://github.com/etemesi254/zune-image/issues/262 sample 3.
                        let Some(qt_slice) = slice.get(start..start + 64) else {
                            return Err(DecodeErrors::FormatStatic("Invalid slice , would panic, invalid image"))
                        };
                        // dequantize
                        for ((x, out), qt_val) in qt_slice
                            .iter()
                            .zip(tmp.iter_mut())
                            .zip(qt_table.iter())
                        {
                            *out = i32::from(*x) * qt_val;
                        }
                        // determine where to write.
                        let sl = &mut temp_channel[component.idct_pos..];

                        component.idct_pos += 8;
                        // tmp now contains a dequantized block so idct it
                        (self.idct_func)(&mut tmp, sl, component.width_stride);
                    }
                    // after every write of 8, skip 7 since idct write stride wise 8 times.
                    //
                    // Remember each MCU is 8x8 block, so each idct will write 8 strides into
                    // sl
                    //
                    // and component.idct_pos is one stride long
                    component.idct_pos += 7 * component.width_stride;
                }
                component.idct_pos = 0;
            }

            // process that width up until it's impossible
            self.post_process(
                pixels,
                i,
                mcu_height,
                width,
                padded_width,
                &mut pixels_written,
                &mut upsampler_scratch_space
            )?;
        }

        debug!("Finished decoding image");

        return Ok(());
    }
    pub(crate) fn reset_params(&mut self) {
        /*
        Apparently, grayscale images which can be down sampled exists, which is weird in the sense
        that it has one component Y, which is not usually down sampled.

        This means some calculations will be wrong, so for that we explicitly reset params
        for such occurrences, warn and reset the image info to appear as if it were
        a non-sampled image to ensure decoding works
        */
        self.h_max = 1;
        self.options = self.options.jpeg_set_out_colorspace(ColorSpace::Luma);
        self.v_max = 1;
        self.sub_sample_ratio = SampleRatios::None;
        self.is_interleaved = false;
        self.components[0].vertical_sample = 1;
        self.components[0].width_stride = (((self.info.width as usize) + 7) / 8) * 8;
        self.components[0].horizontal_sample = 1;
    }
}

///Get a marker from the bit-stream.
///
/// This reads until it gets a marker or end of file is encountered
pub fn get_marker<T>(
    reader: &mut ZByteReader<T>, stream: &mut BitStream
) -> Result<Marker, DecodeErrors>
where
    T: ZReaderTrait
{
    if let Some(marker) = stream.marker {
        stream.marker = None;
        return Ok(marker);
    }

    // read until we get a marker

    while !reader.eof() {
        let marker = reader.get_u8_err()?;

        if marker == 255 {
            let mut r = reader.get_u8_err()?;
            // 0xFF 0XFF(some images may be like that)
            while r == 0xFF {
                r = reader.get_u8_err()?;
            }

            if r != 0 {
                return Marker::from_u8(r)
                    .ok_or_else(|| DecodeErrors::Format(format!("Unknown marker 0xFF{r:X}")));
            }
        }
    }
    return Err(DecodeErrors::ExhaustedData);
}
