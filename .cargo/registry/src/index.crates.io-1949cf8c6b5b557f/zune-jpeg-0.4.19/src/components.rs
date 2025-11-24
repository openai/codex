/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! This module exports a single struct to store information about
//! JPEG image components
//!
//! The data is extracted from a SOF header.

use alloc::vec::Vec;
use alloc::{format, vec};

use zune_core::log::trace;

use crate::decoder::MAX_COMPONENTS;
use crate::errors::DecodeErrors;
use crate::upsampler::upsample_no_op;

/// Represents an up-sampler function, this function will be called to upsample
/// a down-sampled image

pub type UpSampler = fn(
    input: &[i16],
    in_near: &[i16],
    in_far: &[i16],
    scratch_space: &mut [i16],
    output: &mut [i16]
);

/// Component Data from start of frame
#[derive(Clone)]
pub(crate) struct Components {
    /// The type of component that has the metadata below, can be Y,Cb or Cr
    pub component_id: ComponentID,
    /// Sub-sampling ratio of this component in the x-plane
    pub vertical_sample: usize,
    /// Sub-sampling ratio of this component in the y-plane
    pub horizontal_sample: usize,
    /// DC huffman table position
    pub dc_huff_table: usize,
    /// AC huffman table position for this element.
    pub ac_huff_table: usize,
    /// Quantization table number
    pub quantization_table_number: u8,
    /// Specifies quantization table to use with this component
    pub quantization_table: [i32; 64],
    /// dc prediction for the component
    pub dc_pred: i32,
    /// An up-sampling function, can be basic or SSE, depending
    /// on the platform
    pub up_sampler: UpSampler,
    /// How pixels do we need to go to get to the next line?
    pub width_stride: usize,
    /// Component ID for progressive
    pub id: u8,
    /// Whether we need to decode this image component.
    pub needed: bool,
    /// Upsample scanline
    pub raw_coeff: Vec<i16>,
    /// Upsample destination, stores a scanline worth of sub sampled data
    pub upsample_dest: Vec<i16>,
    /// previous row, used to handle MCU boundaries
    pub row_up: Vec<i16>,
    /// current row, used to handle MCU boundaries again
    pub row: Vec<i16>,
    pub first_row_upsample_dest: Vec<i16>,
    pub idct_pos: usize,
    pub x: usize,
    pub w2: usize,
    pub y: usize,
    pub sample_ratio: SampleRatios,
    // a very annoying bug
    pub fix_an_annoying_bug: usize
}

impl Components {
    /// Create a new instance from three bytes from the start of frame
    #[inline]
    pub fn from(a: [u8; 3], pos: u8) -> Result<Components, DecodeErrors> {
        // it's a unique identifier.
        // doesn't have to be ascending
        // see tests/inputs/huge_sof_number
        //
        // For such cases, use the position of the component
        // to determine width

        let id = match pos {
            0 => ComponentID::Y,
            1 => ComponentID::Cb,
            2 => ComponentID::Cr,
            3 => ComponentID::Q,
            _ => {
                return Err(DecodeErrors::Format(format!(
                    "Unknown component id found,{pos}, expected value between 1 and 4"
                )))
            }
        };

        let horizontal_sample = (a[1] >> 4) as usize;
        let vertical_sample = (a[1] & 0x0f) as usize;
        let quantization_table_number = a[2];
        // confirm quantization number is between 0 and MAX_COMPONENTS
        if usize::from(quantization_table_number) >= MAX_COMPONENTS {
            return Err(DecodeErrors::Format(format!(
                "Too large quantization number :{quantization_table_number}, expected value between 0 and {MAX_COMPONENTS}"
            )));
        }
        // check that upsampling ratios are powers of two
        // if these fail, it's probably a corrupt image.
        if !horizontal_sample.is_power_of_two() {
            return Err(DecodeErrors::Format(format!(
                "Horizontal sample is not a power of two({horizontal_sample}) cannot decode"
            )));
        }

        if !vertical_sample.is_power_of_two() {
            return Err(DecodeErrors::Format(format!(
                "Vertical sub-sample is not power of two({vertical_sample}) cannot decode"
            )));
        }

        trace!(
            "Component ID:{:?} \tHS:{} VS:{} QT:{}",
            id,
            horizontal_sample,
            vertical_sample,
            quantization_table_number
        );

        Ok(Components {
            component_id: id,
            vertical_sample,
            horizontal_sample,
            quantization_table_number,
            first_row_upsample_dest: vec![],
            // These two will be set with sof marker
            dc_huff_table: 0,
            ac_huff_table: 0,
            quantization_table: [0; 64],
            dc_pred: 0,
            up_sampler: upsample_no_op,
            // set later
            width_stride: horizontal_sample,
            id: a[0],
            needed: true,
            raw_coeff: vec![],
            upsample_dest: vec![],
            row_up: vec![],
            row: vec![],
            idct_pos: 0,
            x: 0,
            y: 0,
            w2: 0,
            sample_ratio: SampleRatios::None,
            fix_an_annoying_bug: 1
        })
    }
    /// Setup space for upsampling
    ///
    /// During upsample, we need a reference of the last row so that upsampling can
    /// proceed correctly,
    /// so we store the last line of every scanline and use it for the next upsampling procedure
    /// to store this, but since we don't need it for 1v1 upsampling,
    /// we only call this for routines that need upsampling
    ///
    /// # Requirements
    ///  - width stride of this element is set for the component.
    pub fn setup_upsample_scanline(&mut self) {
        self.row = vec![0; self.width_stride * self.vertical_sample];
        self.row_up = vec![0; self.width_stride * self.vertical_sample];
        self.first_row_upsample_dest =
            vec![128; self.vertical_sample * self.width_stride * self.sample_ratio.sample()];
        self.upsample_dest =
            vec![0; self.width_stride * self.sample_ratio.sample() * self.fix_an_annoying_bug * 8];
    }
}

/// Component ID's
#[derive(Copy, Debug, Clone, PartialEq, Eq)]
pub enum ComponentID {
    /// Luminance channel
    Y,
    /// Blue chrominance
    Cb,
    /// Red chrominance
    Cr,
    /// Q or fourth component
    Q
}

#[derive(Copy, Debug, Clone, PartialEq, Eq)]
pub enum SampleRatios {
    HV,
    V,
    H,
    Generic(usize, usize),
    None
}

impl SampleRatios {
    pub fn sample(self) -> usize {
        match self {
            SampleRatios::HV => 4,
            SampleRatios::V | SampleRatios::H => 2,
            SampleRatios::Generic(a, b) => a * b,
            SampleRatios::None => 1
        }
    }
}
