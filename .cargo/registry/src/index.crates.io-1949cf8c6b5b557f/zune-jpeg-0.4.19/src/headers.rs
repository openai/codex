/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Decode Decoder markers/segments
//!
//! This file deals with decoding header information in a jpeg file
//!
use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;

use zune_core::bytestream::ZReaderTrait;
use zune_core::colorspace::ColorSpace;
use zune_core::log::{debug, error, trace, warn};

use crate::components::Components;
use crate::decoder::{ICCChunk, JpegDecoder, MAX_COMPONENTS};
use crate::errors::DecodeErrors;
use crate::huffman::HuffmanTable;
use crate::misc::{SOFMarkers, UN_ZIGZAG};

///**B.2.4.2 Huffman table-specification syntax**
#[allow(clippy::similar_names, clippy::cast_sign_loss)]
pub(crate) fn parse_huffman<T: ZReaderTrait>(
    decoder: &mut JpegDecoder<T>
) -> Result<(), DecodeErrors>
where
{
    // Read the length of the Huffman table
    let mut dht_length = i32::from(decoder.stream.get_u16_be_err()?.checked_sub(2).ok_or(
        DecodeErrors::FormatStatic("Invalid Huffman length in image")
    )?);

    while dht_length > 16 {
        // HT information
        let ht_info = decoder.stream.get_u8_err()?;
        // third bit indicates whether the huffman encoding is DC or AC type
        let dc_or_ac = (ht_info >> 4) & 0xF;
        // Indicate the position of this table, should be less than 4;
        let index = (ht_info & 0xF) as usize;
        // read the number of symbols
        let mut num_symbols: [u8; 17] = [0; 17];

        if index >= MAX_COMPONENTS {
            return Err(DecodeErrors::HuffmanDecode(format!(
                "Invalid DHT index {index}, expected between 0 and 3"
            )));
        }

        if dc_or_ac > 1 {
            return Err(DecodeErrors::HuffmanDecode(format!(
                "Invalid DHT position {dc_or_ac}, should be 0 or 1"
            )));
        }

        decoder
            .stream
            .read_exact(&mut num_symbols[1..17])
            .map_err(|_| DecodeErrors::ExhaustedData)?;

        dht_length -= 1 + 16;

        let symbols_sum: i32 = num_symbols.iter().map(|f| i32::from(*f)).sum();

        // The sum of the number of symbols cannot be greater than 256;
        if symbols_sum > 256 {
            return Err(DecodeErrors::FormatStatic(
                "Encountered Huffman table with excessive length in DHT"
            ));
        }
        if symbols_sum > dht_length {
            return Err(DecodeErrors::HuffmanDecode(format!(
                "Excessive Huffman table of length {symbols_sum} found when header length is {dht_length}"
            )));
        }
        dht_length -= symbols_sum;
        // A table containing symbols in increasing code length
        let mut symbols = [0; 256];

        decoder
            .stream
            .read_exact(&mut symbols[0..(symbols_sum as usize)])
            .map_err(|x| {
                DecodeErrors::Format(format!("Could not read symbols into the buffer\n{x}"))
            })?;
        // store
        match dc_or_ac {
            0 => {
                decoder.dc_huffman_tables[index] = Some(HuffmanTable::new(
                    &num_symbols,
                    symbols,
                    true,
                    decoder.is_progressive
                )?);
            }
            _ => {
                decoder.ac_huffman_tables[index] = Some(HuffmanTable::new(
                    &num_symbols,
                    symbols,
                    false,
                    decoder.is_progressive
                )?);
            }
        }
    }

    if dht_length > 0 {
        return Err(DecodeErrors::FormatStatic("Bogus Huffman table definition"));
    }

    Ok(())
}

///**B.2.4.1 Quantization table-specification syntax**
#[allow(clippy::cast_possible_truncation, clippy::needless_range_loop)]
pub(crate) fn parse_dqt<T: ZReaderTrait>(img: &mut JpegDecoder<T>) -> Result<(), DecodeErrors> {
    // read length
    let mut qt_length =
        img.stream
            .get_u16_be_err()?
            .checked_sub(2)
            .ok_or(DecodeErrors::FormatStatic(
                "Invalid DQT length. Length should be greater than 2"
            ))?;
    // A single DQT header may have multiple QT's
    while qt_length > 0 {
        let qt_info = img.stream.get_u8_err()?;
        // 0 = 8 bit otherwise 16 bit dqt
        let precision = (qt_info >> 4) as usize;
        // last 4 bits give us position
        let table_position = (qt_info & 0x0f) as usize;
        let precision_value = 64 * (precision + 1);

        if (precision_value + 1) as u16 > qt_length {
            return Err(DecodeErrors::DqtError(format!("Invalid QT table bytes left :{}. Too small to construct a valid qt table which should be {} long", qt_length, precision_value + 1)));
        }

        let dct_table = match precision {
            0 => {
                let mut qt_values = [0; 64];

                img.stream.read_exact(&mut qt_values).map_err(|x| {
                    DecodeErrors::Format(format!("Could not read symbols into the buffer\n{x}"))
                })?;
                qt_length -= (precision_value as u16) + 1 /*QT BIT*/;
                // carry out un zig-zag here
                un_zig_zag(&qt_values)
            }
            1 => {
                // 16 bit quantization tables
                let mut qt_values = [0_u16; 64];

                for i in 0..64 {
                    qt_values[i] = img.stream.get_u16_be_err()?;
                }
                qt_length -= (precision_value as u16) + 1;

                un_zig_zag(&qt_values)
            }
            _ => {
                return Err(DecodeErrors::DqtError(format!(
                    "Expected QT precision value of either 0 or 1, found {precision:?}"
                )));
            }
        };

        if table_position >= MAX_COMPONENTS {
            return Err(DecodeErrors::DqtError(format!(
                "Too large table position for QT :{table_position}, expected between 0 and 3"
            )));
        }

        img.qt_tables[table_position] = Some(dct_table);
    }

    return Ok(());
}

/// Section:`B.2.2 Frame header syntax`

pub(crate) fn parse_start_of_frame<T: ZReaderTrait>(
    sof: SOFMarkers, img: &mut JpegDecoder<T>
) -> Result<(), DecodeErrors> {
    if img.seen_sof {
        return Err(DecodeErrors::SofError(
            "Two Start of Frame Markers".to_string()
        ));
    }
    // Get length of the frame header
    let length = img.stream.get_u16_be_err()?;
    // usually 8, but can be 12 and 16, we currently support only 8
    // so sorry about that 12 bit images
    let dt_precision = img.stream.get_u8_err()?;

    if dt_precision != 8 {
        return Err(DecodeErrors::SofError(format!(
            "The library can only parse 8-bit images, the image has {dt_precision} bits of precision"
        )));
    }

    img.info.set_density(dt_precision);

    // read  and set the image height.
    let img_height = img.stream.get_u16_be_err()?;
    img.info.set_height(img_height);

    // read and set the image width
    let img_width = img.stream.get_u16_be_err()?;
    img.info.set_width(img_width);

    trace!("Image width  :{}", img_width);
    trace!("Image height :{}", img_height);

    if usize::from(img_width) > img.options.get_max_width() {
        return Err(DecodeErrors::Format(format!("Image width {} greater than width limit {}. If use `set_limits` if you want to support huge images", img_width, img.options.get_max_width())));
    }

    if usize::from(img_height) > img.options.get_max_height() {
        return Err(DecodeErrors::Format(format!("Image height {} greater than height limit {}. If use `set_limits` if you want to support huge images", img_height, img.options.get_max_height())));
    }

    // Check image width or height is zero
    if img_width == 0 || img_height == 0 {
        return Err(DecodeErrors::ZeroError);
    }

    // Number of components for the image.
    let num_components = img.stream.get_u8_err()?;

    if num_components == 0 {
        return Err(DecodeErrors::SofError(
            "Number of components cannot be zero.".to_string()
        ));
    }

    let expected = 8 + 3 * u16::from(num_components);
    // length should be equal to num components
    if length != expected {
        return Err(DecodeErrors::SofError(format!(
            "Length of start of frame differs from expected {expected},value is {length}"
        )));
    }

    trace!("Image components : {}", num_components);

    if num_components == 1 {
        // SOF sets the number of image components
        // and that to us translates to setting input and output
        // colorspaces to zero
        img.input_colorspace = ColorSpace::Luma;
        img.options = img.options.jpeg_set_out_colorspace(ColorSpace::Luma);
        debug!("Overriding default colorspace set to Luma");
    }
    if num_components == 4 && img.input_colorspace == ColorSpace::YCbCr {
        trace!("Input image has 4 components, defaulting to CMYK colorspace");
        // https://entropymine.wordpress.com/2018/10/22/how-is-a-jpeg-images-color-type-determined/
        img.input_colorspace = ColorSpace::CMYK;
    }

    // set number of components
    img.info.components = num_components;

    let mut components = Vec::with_capacity(num_components as usize);
    let mut temp = [0; 3];

    for pos in 0..num_components {
        // read 3 bytes for each component
        img.stream
            .read_exact(&mut temp)
            .map_err(|x| DecodeErrors::Format(format!("Could not read component data\n{x}")))?;
        // create a component.
        let component = Components::from(temp, pos)?;

        components.push(component);
    }
    img.seen_sof = true;

    img.info.set_sof_marker(sof);

    img.components = components;

    Ok(())
}

/// Parse a start of scan data
pub(crate) fn parse_sos<T: ZReaderTrait>(image: &mut JpegDecoder<T>) -> Result<(), DecodeErrors> {
    // Scan header length
    let ls = image.stream.get_u16_be_err()?;
    // Number of image components in scan
    let ns = image.stream.get_u8_err()?;

    let mut seen = [-1; { MAX_COMPONENTS + 1 }];

    image.num_scans = ns;

    if ls != 6 + 2 * u16::from(ns) {
        return Err(DecodeErrors::SosError(format!(
            "Bad SOS length {ls},corrupt jpeg"
        )));
    }

    // Check number of components.
    if !(1..5).contains(&ns) {
        return Err(DecodeErrors::SosError(format!(
            "Number of components in start of scan should be less than 3 but more than 0. Found {ns}"
        )));
    }

    if image.info.components == 0 {
        return Err(DecodeErrors::FormatStatic(
            "Error decoding SOF Marker, Number of components cannot be zero."
        ));
    }

    // consume spec parameters
    for i in 0..ns {
        // CS_i parameter, I don't need it so I might as well delete it
        let id = image.stream.get_u8_err()?;

        if seen.contains(&i32::from(id)) {
            return Err(DecodeErrors::SofError(format!(
                "Duplicate ID {id} seen twice in the same component"
            )));
        }

        seen[usize::from(i)] = i32::from(id);
        // DC and AC huffman table position
        // top 4 bits contain dc huffman destination table
        // lower four bits contain ac huffman destination table
        let y = image.stream.get_u8_err()?;

        let mut j = 0;

        while j < image.info.components {
            if image.components[j as usize].id == id {
                break;
            }

            j += 1;
        }

        if j == image.info.components {
            return Err(DecodeErrors::SofError(format!(
                "Invalid component id {}, expected a value between 0 and {}",
                id,
                image.components.len()
            )));
        }

        image.components[usize::from(j)].dc_huff_table = usize::from((y >> 4) & 0xF);
        image.components[usize::from(j)].ac_huff_table = usize::from(y & 0xF);
        image.z_order[i as usize] = j as usize;
    }

    // Collect the component spec parameters
    // This is only needed for progressive images but I'll read
    // them in order to ensure they are correct according to the spec

    // Extract progressive information

    // https://www.w3.org/Graphics/JPEG/itu-t81.pdf
    // Page 42

    // Start of spectral / predictor selection. (between 0 and 63)
    image.spec_start = image.stream.get_u8_err()?;
    // End of spectral selection
    image.spec_end = image.stream.get_u8_err()?;

    let bit_approx = image.stream.get_u8_err()?;
    // successive approximation bit position high
    image.succ_high = bit_approx >> 4;

    if image.spec_end > 63 {
        return Err(DecodeErrors::SosError(format!(
            "Invalid Se parameter {}, range should be 0-63",
            image.spec_end
        )));
    }
    if image.spec_start > 63 {
        return Err(DecodeErrors::SosError(format!(
            "Invalid Ss parameter {}, range should be 0-63",
            image.spec_start
        )));
    }
    if image.succ_high > 13 {
        return Err(DecodeErrors::SosError(format!(
            "Invalid Ah parameter {}, range should be 0-13",
            image.succ_low
        )));
    }
    // successive approximation bit position low
    image.succ_low = bit_approx & 0xF;

    if image.succ_low > 13 {
        return Err(DecodeErrors::SosError(format!(
            "Invalid Al parameter {}, range should be 0-13",
            image.succ_low
        )));
    }

    trace!(
        "Ss={}, Se={} Ah={} Al={}",
        image.spec_start,
        image.spec_end,
        image.succ_high,
        image.succ_low
    );

    Ok(())
}

/// Parse Adobe App14 segment
pub(crate) fn parse_app14<T: ZReaderTrait>(
    decoder: &mut JpegDecoder<T>
) -> Result<(), DecodeErrors> {
    // skip length
    let mut length = usize::from(decoder.stream.get_u16_be());

    if length < 2 || !decoder.stream.has(length - 2) {
        return Err(DecodeErrors::ExhaustedData);
    }
    if length < 14 {
        return Err(DecodeErrors::FormatStatic(
            "Too short of a length for App14 segment"
        ));
    }
    if decoder.stream.peek_at(0, 5) == Ok(b"Adobe") {
        // move stream 6 bytes to remove adobe id
        decoder.stream.skip(6);
        // skip version, flags0 and flags1
        decoder.stream.skip(5);
        // get color transform
        let transform = decoder.stream.get_u8();
        // https://exiftool.org/TagNames/JPEG.html#Adobe
        match transform {
            0 => decoder.input_colorspace = ColorSpace::CMYK,
            1 => decoder.input_colorspace = ColorSpace::YCbCr,
            2 => decoder.input_colorspace = ColorSpace::YCCK,
            _ => {
                return Err(DecodeErrors::Format(format!(
                    "Unknown Adobe colorspace {transform}"
                )))
            }
        }
        // length   = 2
        // adobe id = 6
        // version =  5
        // transform = 1
        length = length.saturating_sub(14);
    } else if decoder.options.get_strict_mode() {
        return Err(DecodeErrors::FormatStatic("Corrupt Adobe App14 segment"));
    } else {
        length = length.saturating_sub(2);
        error!("Not a valid Adobe APP14 Segment");
    }
    // skip any proceeding lengths.
    // we do not need them
    decoder.stream.skip(length);

    Ok(())
}

/// Parse the APP1 segment
///
/// This contains the exif tag
pub(crate) fn parse_app1<T: ZReaderTrait>(
    decoder: &mut JpegDecoder<T>
) -> Result<(), DecodeErrors> {
    // contains exif data
    let mut length = usize::from(decoder.stream.get_u16_be());

    if length < 2 || !decoder.stream.has(length - 2) {
        return Err(DecodeErrors::ExhaustedData);
    }
    // length bytes
    length -= 2;

    if length > 6 && decoder.stream.peek_at(0, 6).unwrap() == b"Exif\x00\x00" {
        trace!("Exif segment present");
        // skip bytes we read above
        decoder.stream.skip(6);
        length -= 6;

        let exif_bytes = decoder.stream.peek_at(0, length).unwrap().to_vec();

        decoder.exif_data = Some(exif_bytes);
    } else {
        warn!("Wrongly formatted exif tag");
    }

    decoder.stream.skip(length);
    Ok(())
}

pub(crate) fn parse_app2<T: ZReaderTrait>(
    decoder: &mut JpegDecoder<T>
) -> Result<(), DecodeErrors> {
    let mut length = usize::from(decoder.stream.get_u16_be());

    if length < 2 || !decoder.stream.has(length - 2) {
        return Err(DecodeErrors::ExhaustedData);
    }
    // length bytes
    length -= 2;

    if length > 14 && decoder.stream.peek_at(0, 12).unwrap() == *b"ICC_PROFILE\0" {
        trace!("ICC Profile present");
        // skip 12 bytes which indicate ICC profile
        length -= 12;
        decoder.stream.skip(12);
        let seq_no = decoder.stream.get_u8();
        let num_markers = decoder.stream.get_u8();
        // deduct the two bytes we read above
        length -= 2;

        let data = decoder.stream.peek_at(0, length).unwrap().to_vec();

        let icc_chunk = ICCChunk {
            seq_no,
            num_markers,
            data
        };
        decoder.icc_data.push(icc_chunk);
    }

    decoder.stream.skip(length);

    Ok(())
}

/// Small utility function to print Un-zig-zagged quantization tables

fn un_zig_zag<T>(a: &[T]) -> [i32; 64]
where
    T: Default + Copy,
    i32: core::convert::From<T>
{
    let mut output = [i32::default(); 64];

    for i in 0..64 {
        output[UN_ZIGZAG[i]] = i32::from(a[i]);
    }

    output
}
