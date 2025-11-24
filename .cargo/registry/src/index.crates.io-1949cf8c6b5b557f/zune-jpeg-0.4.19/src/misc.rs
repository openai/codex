/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//!Miscellaneous stuff
#![allow(dead_code)]

use alloc::format;
use core::cmp::max;
use core::fmt;

use zune_core::bytestream::{ZByteReader, ZReaderTrait};
use zune_core::colorspace::ColorSpace;
use zune_core::log::{trace, warn};

use crate::components::{ComponentID, SampleRatios};
use crate::errors::DecodeErrors;
use crate::huffman::HuffmanTable;
use crate::JpegDecoder;

/// Start of baseline DCT Huffman coding

pub const START_OF_FRAME_BASE: u16 = 0xffc0;

/// Start of another frame

pub const START_OF_FRAME_EXT_SEQ: u16 = 0xffc1;

/// Start of progressive DCT encoding

pub const START_OF_FRAME_PROG_DCT: u16 = 0xffc2;

/// Start of Lossless sequential Huffman coding

pub const START_OF_FRAME_LOS_SEQ: u16 = 0xffc3;

/// Start of extended sequential DCT arithmetic coding

pub const START_OF_FRAME_EXT_AR: u16 = 0xffc9;

/// Start of Progressive DCT arithmetic coding

pub const START_OF_FRAME_PROG_DCT_AR: u16 = 0xffca;

/// Start of Lossless sequential Arithmetic coding

pub const START_OF_FRAME_LOS_SEQ_AR: u16 = 0xffcb;

/// Undo run length encoding of coefficients by placing them in natural order
#[rustfmt::skip]
pub const UN_ZIGZAG: [usize; 64 + 16] = [
     0,  1,  8, 16,  9,  2,  3, 10,
    17, 24, 32, 25, 18, 11,  4,  5,
    12, 19, 26, 33, 40, 48, 41, 34,
    27, 20, 13,  6,  7, 14, 21, 28,
    35, 42, 49, 56, 57, 50, 43, 36,
    29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46,
    53, 60, 61, 54, 47, 55, 62, 63,
    // Prevent overflowing
    63, 63, 63, 63, 63, 63, 63, 63,
    63, 63, 63, 63, 63, 63, 63, 63
];

/// Align data to a 16 byte boundary
#[repr(align(16))]
#[derive(Clone)]

pub struct Aligned16<T: ?Sized>(pub T);

impl<T> Default for Aligned16<T>
where
    T: Default
{
    fn default() -> Self {
        Aligned16(T::default())
    }
}

/// Align data to a 32 byte boundary
#[repr(align(32))]
#[derive(Clone)]
pub struct Aligned32<T: ?Sized>(pub T);

impl<T> Default for Aligned32<T>
where
    T: Default
{
    fn default() -> Self {
        Aligned32(T::default())
    }
}

/// Markers that identify different Start of Image markers
/// They identify the type of encoding and whether the file use lossy(DCT) or
/// lossless compression and whether we use Huffman or arithmetic coding schemes
#[derive(Eq, PartialEq, Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub enum SOFMarkers {
    /// Baseline DCT markers
    BaselineDct,
    /// SOF_1 Extended sequential DCT,Huffman coding
    ExtendedSequentialHuffman,
    /// Progressive DCT, Huffman coding
    ProgressiveDctHuffman,
    /// Lossless (sequential), huffman coding,
    LosslessHuffman,
    /// Extended sequential DEC, arithmetic coding
    ExtendedSequentialDctArithmetic,
    /// Progressive DCT, arithmetic coding,
    ProgressiveDctArithmetic,
    /// Lossless ( sequential), arithmetic coding
    LosslessArithmetic
}

impl Default for SOFMarkers {
    fn default() -> Self {
        Self::BaselineDct
    }
}

impl SOFMarkers {
    /// Check if a certain marker is sequential DCT or not

    pub fn is_sequential_dct(self) -> bool {
        matches!(
            self,
            Self::BaselineDct
                | Self::ExtendedSequentialHuffman
                | Self::ExtendedSequentialDctArithmetic
        )
    }

    /// Check if a marker is a Lossles type or not

    pub fn is_lossless(self) -> bool {
        matches!(self, Self::LosslessHuffman | Self::LosslessArithmetic)
    }

    /// Check whether a marker is a progressive marker or not

    pub fn is_progressive(self) -> bool {
        matches!(
            self,
            Self::ProgressiveDctHuffman | Self::ProgressiveDctArithmetic
        )
    }

    /// Create a marker from an integer

    pub fn from_int(int: u16) -> Option<SOFMarkers> {
        match int {
            START_OF_FRAME_BASE => Some(Self::BaselineDct),
            START_OF_FRAME_PROG_DCT => Some(Self::ProgressiveDctHuffman),
            START_OF_FRAME_PROG_DCT_AR => Some(Self::ProgressiveDctArithmetic),
            START_OF_FRAME_LOS_SEQ => Some(Self::LosslessHuffman),
            START_OF_FRAME_LOS_SEQ_AR => Some(Self::LosslessArithmetic),
            START_OF_FRAME_EXT_SEQ => Some(Self::ExtendedSequentialHuffman),
            START_OF_FRAME_EXT_AR => Some(Self::ExtendedSequentialDctArithmetic),
            _ => None
        }
    }
}

impl fmt::Debug for SOFMarkers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            Self::BaselineDct => write!(f, "Baseline DCT"),
            Self::ExtendedSequentialHuffman => {
                write!(f, "Extended sequential DCT, Huffman Coding")
            }
            Self::ProgressiveDctHuffman => write!(f, "Progressive DCT,Huffman Encoding"),
            Self::LosslessHuffman => write!(f, "Lossless (sequential) Huffman encoding"),
            Self::ExtendedSequentialDctArithmetic => {
                write!(f, "Extended sequential DCT, arithmetic coding")
            }
            Self::ProgressiveDctArithmetic => write!(f, "Progressive DCT, arithmetic coding"),
            Self::LosslessArithmetic => write!(f, "Lossless (sequential) arithmetic coding")
        }
    }
}

/// Read `buf.len()*2` data from the underlying `u8` buffer and convert it into
/// u16, and store it into `buf`
///
/// # Arguments
/// - reader: A mutable reference to the underlying reader.
/// - buf: A mutable reference to a slice containing u16's
#[inline]
pub fn read_u16_into<T>(reader: &mut ZByteReader<T>, buf: &mut [u16]) -> Result<(), DecodeErrors>
where
    T: ZReaderTrait
{
    for i in buf {
        *i = reader.get_u16_be_err()?;
    }

    Ok(())
}

/// Set up component parameters.
///
/// This modifies the components in place setting up details needed by other
/// parts fo the decoder.
pub(crate) fn setup_component_params<T: ZReaderTrait>(
    img: &mut JpegDecoder<T>
) -> Result<(), DecodeErrors> {
    let img_width = img.width();
    let img_height = img.height();

    // in case of adobe app14 being present, zero may indicate
    // either CMYK if components are 4 or RGB if components are 3,
    // see https://docs.oracle.com/javase/6/docs/api/javax/imageio/metadata/doc-files/jpeg_metadata.html
    // so since we may not know how many number of components
    // we have when decoding app14, we have to defer that check
    // until now.
    //
    // We know adobe app14 was present since it's the only one that can modify
    // input colorspace to be CMYK
    if img.components.len() == 3 && img.input_colorspace == ColorSpace::CMYK {
        img.input_colorspace = ColorSpace::RGB;
    }

    for component in &mut img.components {
        // compute interleaved image info
        // h_max contains the maximum horizontal component
        img.h_max = max(img.h_max, component.horizontal_sample);
        // v_max contains the maximum vertical component
        img.v_max = max(img.v_max, component.vertical_sample);
        img.mcu_width = img.h_max * 8;
        img.mcu_height = img.v_max * 8;
        // Number of MCU's per width
        img.mcu_x = (usize::from(img.info.width) + img.mcu_width - 1) / img.mcu_width;
        // Number of MCU's per height
        img.mcu_y = (usize::from(img.info.height) + img.mcu_height - 1) / img.mcu_height;

        if img.h_max != 1 || img.v_max != 1 {
            // interleaved images have horizontal and vertical sampling factors
            // not equal to 1.
            img.is_interleaved = true;
        }
        // Extract quantization tables from the arrays into components
        let qt_table = *img.qt_tables[component.quantization_table_number as usize]
            .as_ref()
            .ok_or_else(|| {
                DecodeErrors::DqtError(format!(
                    "No quantization table for component {:?}",
                    component.component_id
                ))
            })?;

        let x = (usize::from(img_width) * component.horizontal_sample + img.h_max - 1) / img.h_max;
        let y = (usize::from(img_height) * component.horizontal_sample + img.h_max - 1) / img.v_max;
        component.x = x;
        component.w2 = img.mcu_x * component.horizontal_sample * 8;
        // probably not needed. :)
        component.y = y;
        component.quantization_table = qt_table;
        // initially stride contains its horizontal sub-sampling
        component.width_stride *= img.mcu_x * 8;
    }
    {
        // Sampling factors are one thing that suck
        // this fixes a specific problem with images like
        //
        // (2 2) None
        // (2 1) H
        // (2 1) H
        //
        // The images exist in the wild, the images are not meant to exist
        // but they do, it's just an annoying horizontal sub-sampling that
        // I don't know why it exists.
        // But it does
        // So we try to cope with that.
        // I am not sure of how to explain how to fix it, but it involved a debugger
        // and to much coke(the legal one)
        //
        // If this wasn't present, self.upsample_dest would have the wrong length
        let mut handle_that_annoying_bug = false;

        if let Some(y_component) = img
            .components
            .iter()
            .find(|c| c.component_id == ComponentID::Y)
        {
            if y_component.horizontal_sample == 2 || y_component.vertical_sample == 2 {
                handle_that_annoying_bug = true;
            }
        }
        if handle_that_annoying_bug {
            for comp in &mut img.components {
                if (comp.component_id != ComponentID::Y)
                    && (comp.horizontal_sample != 1 || comp.vertical_sample != 1)
                {
                    comp.fix_an_annoying_bug = 2;
                }
            }
        }
    }

    if img.is_mjpeg {
        fill_default_mjpeg_tables(
            img.is_progressive,
            &mut img.dc_huffman_tables,
            &mut img.ac_huffman_tables
        );
    }

    // check colorspace matches
    if img.input_colorspace.num_components() > img.components.len() {
        if img.input_colorspace == ColorSpace::YCCK {
            // Some images may have YCCK format (from adobe app14 segment) which is supposed to be 4 components
            // but only 3 components, see issue https://github.com/etemesi254/zune-image/issues/275
            // So this is the behaviour of other decoders
            // - stb_image: Treats it as YCbCr image
            // - libjpeg_turbo: Does not know how to parse YCCK images (transform 2 app14) so treats
            // it as YCbCr
            // So I will match that to match existing ones
            warn!("Treating YCCK colorspace as YCbCr as component length does not match");
            img.input_colorspace = ColorSpace::YCbCr
        } else {
            let msg = format!(
                " Expected {} number of components but found {}",
                img.input_colorspace.num_components(),
                img.components.len()
            );

            return Err(DecodeErrors::Format(msg));
        }
    }
    Ok(())
}

///Calculate number of fill bytes added to the end of a JPEG image
/// to fill the image
///
/// JPEG usually inserts padding bytes if the image width cannot be evenly divided into
/// 8 , 16 or 32 chunks depending on the sub sampling ratio. So given a sub-sampling ratio,
/// and the actual width, this calculates the padded bytes that were added to the image
///
///  # Params
/// -actual_width: Actual width of the image
/// -sub_sample: Sub sampling factor of the image
///
/// # Returns
/// The padded width, this is how long the width is for a particular image
pub fn calculate_padded_width(actual_width: usize, sub_sample: SampleRatios) -> usize {
    match sub_sample {
        SampleRatios::None | SampleRatios::V => {
            // None+V sends one MCU row, so that's a simple calculation
            ((actual_width + 7) / 8) * 8
        }
        SampleRatios::H | SampleRatios::HV => {
            // sends two rows, width can be expanded by up to 15 more bytes
            ((actual_width + 15) / 16) * 16
        }
        SampleRatios::Generic(h, _) => {
            ((actual_width + ((h * 8).saturating_sub(1))) / (h * 8)) * (h * 8)
        }
    }
}

// https://www.loc.gov/preservation/digital/formats/fdd/fdd000063.shtml
// "Avery Lee, writing in the rec.video.desktop newsgroup in 2001, commented that "MJPEG, or at
//  least the MJPEG in AVIs having the MJPG fourcc, is restricted JPEG with a fixed -- and
//  *omitted* -- Huffman table. The JPEG must be YCbCr colorspace, it must be 4:2:2, and it must
//  use basic Huffman encoding, not arithmetic or progressive.... You can indeed extract the
//  MJPEG frames and decode them with a regular JPEG decoder, but you have to prepend the DHT
//  segment to them, or else the decoder won't have any idea how to decompress the data.
//  The exact table necessary is given in the OpenDML spec.""
pub fn fill_default_mjpeg_tables(
    is_progressive: bool, dc_huffman_tables: &mut [Option<HuffmanTable>],
    ac_huffman_tables: &mut [Option<HuffmanTable>]
) {
    // Section K.3.3
    trace!("Filling with default mjpeg tables");

    if dc_huffman_tables[0].is_none() {
        // Table K.3
        dc_huffman_tables[0] = Some(
            HuffmanTable::new_unfilled(
                &[
                    0x00, 0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00
                ],
                &[
                    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B
                ],
                true,
                is_progressive
            )
            .unwrap()
        );
    }
    if dc_huffman_tables[1].is_none() {
        // Table K.4
        dc_huffman_tables[1] = Some(
            HuffmanTable::new_unfilled(
                &[
                    0x00, 0x00, 0x03, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00,
                    0x00, 0x00, 0x00, 0x00
                ],
                &[
                    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B
                ],
                true,
                is_progressive
            )
            .unwrap()
        );
    }
    if ac_huffman_tables[0].is_none() {
        // Table K.5
        ac_huffman_tables[0] = Some(
            HuffmanTable::new_unfilled(
                &[
                    0x00, 0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04, 0x03, 0x05, 0x05, 0x04, 0x04,
                    0x00, 0x00, 0x01, 0x7D
                ],
                &[
                    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13,
                    0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08, 0x23, 0x42,
                    0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A,
                    0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x34, 0x35,
                    0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A,
                    0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67,
                    0x68, 0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84,
                    0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98,
                    0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3,
                    0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7,
                    0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1,
                    0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4,
                    0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA
                ],
                false,
                is_progressive
            )
            .unwrap()
        );
    }
    if ac_huffman_tables[1].is_none() {
        // Table K.6
        ac_huffman_tables[1] = Some(
            HuffmanTable::new_unfilled(
                &[
                    0x00, 0x00, 0x02, 0x01, 0x02, 0x04, 0x04, 0x03, 0x04, 0x07, 0x05, 0x04, 0x04,
                    0x00, 0x01, 0x02, 0x77
                ],
                &[
                    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51,
                    0x07, 0x61, 0x71, 0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xA1, 0xB1,
                    0xC1, 0x09, 0x23, 0x33, 0x52, 0xF0, 0x15, 0x62, 0x72, 0xD1, 0x0A, 0x16, 0x24,
                    0x34, 0xE1, 0x25, 0xF1, 0x17, 0x18, 0x19, 0x1A, 0x26, 0x27, 0x28, 0x29, 0x2A,
                    0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
                    0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66,
                    0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x82,
                    0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96,
                    0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA,
                    0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5,
                    0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9,
                    0xDA, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF2, 0xF3, 0xF4,
                    0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA
                ],
                false,
                is_progressive
            )
            .unwrap()
        );
    }
}
