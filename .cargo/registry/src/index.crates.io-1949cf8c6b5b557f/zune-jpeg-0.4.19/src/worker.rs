/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use alloc::format;
use core::convert::TryInto;

use zune_core::colorspace::ColorSpace;

use crate::color_convert::ycbcr_to_grayscale;
use crate::components::{Components, SampleRatios};
use crate::decoder::{ColorConvert16Ptr, MAX_COMPONENTS};
use crate::errors::DecodeErrors;

/// fast 0..255 * 0..255 => 0..255 rounded multiplication
///
/// Borrowed from stb
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
#[inline]
fn blinn_8x8(in_val: u8, y: u8) -> u8 {
    let t = i32::from(in_val) * i32::from(y) + 128;
    return ((t + (t >> 8)) >> 8) as u8;
}

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub(crate) fn color_convert(
    unprocessed: &[&[i16]; MAX_COMPONENTS], color_convert_16: ColorConvert16Ptr,
    input_colorspace: ColorSpace, output_colorspace: ColorSpace, output: &mut [u8], width: usize,
    padded_width: usize
) -> Result<(), DecodeErrors> // so many parameters..
{
    // maximum sampling factors are in Y-channel, no need to pass them.

    if input_colorspace.num_components() == 3 && input_colorspace == output_colorspace {
        // sort things like RGB to RGB conversion
        copy_removing_padding(unprocessed, width, padded_width, output);
        return Ok(());
    }
    if input_colorspace.num_components() == 4 && input_colorspace == output_colorspace {
        copy_removing_padding_4x(unprocessed, width, padded_width, output);
        return Ok(());
    }
    // color convert
    match (input_colorspace, output_colorspace) {
        (ColorSpace::YCbCr | ColorSpace::Luma, ColorSpace::Luma) => {
            ycbcr_to_grayscale(unprocessed[0], width, padded_width, output);
        }
        (
            ColorSpace::YCbCr,
            ColorSpace::RGB | ColorSpace::RGBA | ColorSpace::BGR | ColorSpace::BGRA
        ) => {
            color_convert_ycbcr(
                unprocessed,
                width,
                padded_width,
                output_colorspace,
                color_convert_16,
                output
            );
        }
        (ColorSpace::YCCK, ColorSpace::RGB) => {
            color_convert_ycck_to_rgb::<3>(
                unprocessed,
                width,
                padded_width,
                output_colorspace,
                color_convert_16,
                output
            );
        }

        (ColorSpace::YCCK, ColorSpace::RGBA) => {
            color_convert_ycck_to_rgb::<4>(
                unprocessed,
                width,
                padded_width,
                output_colorspace,
                color_convert_16,
                output
            );
        }
        (ColorSpace::CMYK, ColorSpace::RGB) => {
            color_convert_cymk_to_rgb::<3>(unprocessed, width, padded_width, output);
        }
        (ColorSpace::CMYK, ColorSpace::RGBA) => {
            color_convert_cymk_to_rgb::<4>(unprocessed, width, padded_width, output);
        }
        // For the other components we do nothing(currently)
        _ => {
            let msg = format!(
                "Unimplemented colorspace mapping from {input_colorspace:?} to {output_colorspace:?}");

            return Err(DecodeErrors::Format(msg));
        }
    }
    Ok(())
}

/// Copy a block to output removing padding bytes from input
/// if necessary
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
fn copy_removing_padding(
    mcu_block: &[&[i16]; MAX_COMPONENTS], width: usize, padded_width: usize, output: &mut [u8]
) {
    for (((pix_w, c_w), m_w), y_w) in output
        .chunks_exact_mut(width * 3)
        .zip(mcu_block[0].chunks_exact(padded_width))
        .zip(mcu_block[1].chunks_exact(padded_width))
        .zip(mcu_block[2].chunks_exact(padded_width))
    {
        for (((pix, c), y), m) in pix_w.chunks_exact_mut(3).zip(c_w).zip(m_w).zip(y_w) {
            pix[0] = *c as u8;
            pix[1] = *y as u8;
            pix[2] = *m as u8;
        }
    }
}
fn copy_removing_padding_4x(
    mcu_block: &[&[i16]; MAX_COMPONENTS], width: usize, padded_width: usize, output: &mut [u8]
) {
    for ((((pix_w, c_w), m_w), y_w), k_w) in output
        .chunks_exact_mut(width * 4)
        .zip(mcu_block[0].chunks_exact(padded_width))
        .zip(mcu_block[1].chunks_exact(padded_width))
        .zip(mcu_block[2].chunks_exact(padded_width))
        .zip(mcu_block[3].chunks_exact(padded_width))
    {
        for ((((pix, c), y), m), k) in pix_w
            .chunks_exact_mut(4)
            .zip(c_w)
            .zip(m_w)
            .zip(y_w)
            .zip(k_w)
        {
            pix[0] = *c as u8;
            pix[1] = *y as u8;
            pix[2] = *m as u8;
            pix[3] = *k as u8;
        }
    }
}
/// Convert YCCK image to rgb
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn color_convert_ycck_to_rgb<const NUM_COMPONENTS: usize>(
    mcu_block: &[&[i16]; MAX_COMPONENTS], width: usize, padded_width: usize,
    output_colorspace: ColorSpace, color_convert_16: ColorConvert16Ptr, output: &mut [u8]
) {
    color_convert_ycbcr(
        mcu_block,
        width,
        padded_width,
        output_colorspace,
        color_convert_16,
        output
    );
    for (pix_w, m_w) in output
        .chunks_exact_mut(width * 3)
        .zip(mcu_block[3].chunks_exact(padded_width))
    {
        for (pix, m) in pix_w.chunks_exact_mut(NUM_COMPONENTS).zip(m_w) {
            let m = (*m) as u8;
            pix[0] = blinn_8x8(255 - pix[0], m);
            pix[1] = blinn_8x8(255 - pix[1], m);
            pix[2] = blinn_8x8(255 - pix[2], m);
        }
    }
}

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
fn color_convert_cymk_to_rgb<const NUM_COMPONENTS: usize>(
    mcu_block: &[&[i16]; MAX_COMPONENTS], width: usize, padded_width: usize, output: &mut [u8]
) {
    for ((((pix_w, c_w), m_w), y_w), k_w) in output
        .chunks_exact_mut(width * NUM_COMPONENTS)
        .zip(mcu_block[0].chunks_exact(padded_width))
        .zip(mcu_block[1].chunks_exact(padded_width))
        .zip(mcu_block[2].chunks_exact(padded_width))
        .zip(mcu_block[3].chunks_exact(padded_width))
    {
        for ((((pix, c), m), y), k) in pix_w
            .chunks_exact_mut(3)
            .zip(c_w)
            .zip(m_w)
            .zip(y_w)
            .zip(k_w)
        {
            let c = *c as u8;
            let m = *m as u8;
            let y = *y as u8;
            let k = *k as u8;

            pix[0] = blinn_8x8(c, k);
            pix[1] = blinn_8x8(m, k);
            pix[2] = blinn_8x8(y, k);
        }
    }
}

/// Do color-conversion for interleaved MCU
#[allow(
    clippy::similar_names,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    clippy::unwrap_used
)]
fn color_convert_ycbcr(
    mcu_block: &[&[i16]; MAX_COMPONENTS], width: usize, padded_width: usize,
    output_colorspace: ColorSpace, color_convert_16: ColorConvert16Ptr, output: &mut [u8]
) {
    let num_components = output_colorspace.num_components();

    let stride = width * num_components;
    // Allocate temporary buffer for small widths less than  16.
    let mut temp = [0; 64];
    // We need to chunk per width to ensure we can discard extra values at the end of the width.
    // Since the encoder may pad bits to ensure the width is a multiple of 8.
    for (((y_width, cb_width), cr_width), out) in mcu_block[0]
        .chunks_exact(padded_width)
        .zip(mcu_block[1].chunks_exact(padded_width))
        .zip(mcu_block[2].chunks_exact(padded_width))
        .zip(output.chunks_exact_mut(stride))
    {
        if width < 16 {
            // allocate temporary buffers for the values received from idct
            let mut y_out = [0; 16];
            let mut cb_out = [0; 16];
            let mut cr_out = [0; 16];
            // copy those small widths to that buffer
            y_out[0..y_width.len()].copy_from_slice(y_width);
            cb_out[0..cb_width.len()].copy_from_slice(cb_width);
            cr_out[0..cr_width.len()].copy_from_slice(cr_width);
            // we handle widths less than 16 a bit differently, allocating a temporary
            // buffer and writing to that and then flushing to the out buffer
            // because of the optimizations applied below,
            (color_convert_16)(&y_out, &cb_out, &cr_out, &mut temp, &mut 0);
            // copy to stride
            out[0..width * num_components].copy_from_slice(&temp[0..width * num_components]);
            // next
            continue;
        }

        // Chunk in outputs of 16 to pass to color_convert as an array of 16 i16's.
        for (((y, cb), cr), out_c) in y_width
            .chunks_exact(16)
            .zip(cb_width.chunks_exact(16))
            .zip(cr_width.chunks_exact(16))
            .zip(out.chunks_exact_mut(16 * num_components))
        {
            (color_convert_16)(
                y.try_into().unwrap(),
                cb.try_into().unwrap(),
                cr.try_into().unwrap(),
                out_c,
                &mut 0
            );
        }
        //we have more pixels in the end that can't be handled by the main loop.
        //move pointer back a little bit to get last 16 bytes,
        //color convert, and overwrite
        //This means some values will be color converted twice.
        for ((y, cb), cr) in y_width[width - 16..]
            .chunks_exact(16)
            .zip(cb_width[width - 16..].chunks_exact(16))
            .zip(cr_width[width - 16..].chunks_exact(16))
            .take(1)
        {
            (color_convert_16)(
                y.try_into().unwrap(),
                cb.try_into().unwrap(),
                cr.try_into().unwrap(),
                &mut temp,
                &mut 0
            );
        }

        let rem = out[(width - 16) * num_components..]
            .chunks_exact_mut(16 * num_components)
            .next()
            .unwrap();

        rem.copy_from_slice(&temp[0..rem.len()]);
    }
}
pub(crate) fn upsample(
    component: &mut Components, mcu_height: usize, i: usize, upsampler_scratch_space: &mut [i16],
    has_vertical_sample: bool
) {
    match component.sample_ratio {
        SampleRatios::V | SampleRatios::HV => {
            /*
            When upsampling vertically sampled images, we have a certain problem
            which is that we do not have all MCU's decoded, this usually sucks at boundaries
            e.g we can't upsample the last mcu row, since the row_down currently doesn't exist

            To solve this we need to do two things

            1. Carry over coefficients when we lack enough data to upsample
            2. Upsample when we have enough data

            To achieve (1), we store a previous row, and the current row in components themselves
            which will later be used to make (2)

            To achieve (2), we take the stored previous row(second last MCU row),
            current row(last mcu row) and row down(first row of newly decoded MCU)

            and upsample that and store it in first_row_upsample_dest, this contains
            up-sampled coefficients for the last for the previous decoded mcu row.

            The caller is then expected to process first_row_upsample_dest before processing data
            in component.upsample_dest which stores the up-sampled components excluding the last row
            */

            let mut dest_start = 0;
            let stride_bytes_written = component.width_stride * component.sample_ratio.sample();

            if i > 0 {
                // Handle the last MCU of the previous row
                // This wasn't up-sampled as we didn't have the row_down
                // so we do it now

                let stride = component.width_stride;

                let dest = &mut component.first_row_upsample_dest[0..stride_bytes_written];

                // get current row
                let row = &component.row[..];
                let row_up = &component.row_up[..];
                let row_down = &component.raw_coeff[0..stride];
                (component.up_sampler)(row, row_up, row_down, upsampler_scratch_space, dest);
            }

            // we have the Y component width stride.
            // this may be higher than the actual width,(2x because vertical sampling)
            //
            // This will not upsample the last row

            // if false, do not upsample.
            // set to false on the last row of an mcu
            let mut upsample = true;

            let stride = component.width_stride * component.vertical_sample;
            let stop_offset = component.raw_coeff.len() / component.width_stride;
            for (pos, curr_row) in component
                .raw_coeff
                .chunks_exact(component.width_stride)
                .enumerate()
            {
                let mut dest: &mut [i16] = &mut [];
                let mut row_up: &[i16] = &[];
                // row below current sample
                let mut row_down: &[i16] = &[];

                // Order of ifs matters

                if i == 0 && pos == 0 {
                    // first IMAGE row, row_up is the same as current row
                    // row_down is the row below.
                    row_up = &component.raw_coeff[pos * stride..(pos + 1) * stride];
                    row_down = &component.raw_coeff[(pos + 1) * stride..(pos + 2) * stride];
                } else if i > 0 && pos == 0 {
                    // first row of a new mcu, previous row was copied so use that
                    row_up = &component.row[..];
                    row_down = &component.raw_coeff[(pos + 1) * stride..(pos + 2) * stride];
                } else if i == mcu_height.saturating_sub(1) && pos == stop_offset - 1 {
                    // last IMAGE row, adjust pointer to use previous row and current row
                    row_up = &component.raw_coeff[(pos - 1) * stride..pos * stride];
                    row_down = &component.raw_coeff[pos * stride..(pos + 1) * stride];
                } else if pos > 0 && pos < stop_offset - 1 {
                    // other rows, get row up and row down relative to our current row
                    // ignore last row of each mcu
                    row_up = &component.raw_coeff[(pos - 1) * stride..pos * stride];
                    row_down = &component.raw_coeff[(pos + 1) * stride..(pos + 2) * stride];
                } else if pos == stop_offset - 1 {
                    // last MCU in a row
                    //
                    // we need a row at the next MCU but we haven't decoded that MCU yet
                    // so we should save this and when we have the next MCU,
                    // do the upsampling

                    // store the current row and previous row in a buffer
                    let prev_row = &component.raw_coeff[(pos - 1) * stride..pos * stride];

                    component.row_up.copy_from_slice(prev_row);
                    component.row.copy_from_slice(curr_row);
                    upsample = false;
                } else {
                    unreachable!("Uh oh!");
                }
                if upsample {
                    dest =
                        &mut component.upsample_dest[dest_start..dest_start + stride_bytes_written];
                    dest_start += stride_bytes_written;
                }

                if upsample {
                    // upsample
                    (component.up_sampler)(
                        curr_row,
                        row_up,
                        row_down,
                        upsampler_scratch_space,
                        dest
                    );
                }
            }
        }
        SampleRatios::H => {
            assert_eq!(component.raw_coeff.len() * 2, component.upsample_dest.len());

            let raw_coeff = &component.raw_coeff;
            let dest_coeff = &mut component.upsample_dest;

            if has_vertical_sample {
                /*
                There have been images that have the following configurations.

                Component ID:Y    HS:2 VS:2 QT:0
                Component ID:Cb   HS:1 VS:1 QT:1
                Component ID:Cr   HS:1 VS:2 QT:1

                This brings out a nasty case of misaligned sampling factors. Cr will need to save a row because
                of the way we process boundaries but Cb won't since Cr is horizontally sampled while Cb is
                HV sampled with respect to the image sampling factors.

                So during decoding of one MCU, we could only do 7 and not 8 rows, but the SampleRatio::H never had to
                save a single line, since it doesn't suffer from boundary issues.

                Now this takes care of that, saving the last MCU row in case it will be needed.
                We save the previous row before up-sampling this row because the boundary issue is in
                the last MCU row of the previous MCU.

                PS(cae): I can't add the image to the repo as it is nsfw, but can send if required
                */
                let length = component.first_row_upsample_dest.len();
                component
                    .first_row_upsample_dest
                    .copy_from_slice(&dest_coeff.rchunks_exact(length).next().unwrap());
            }
            // up-sample each row
            for (single_row, output_stride) in raw_coeff
                .chunks_exact(component.width_stride)
                .zip(dest_coeff.chunks_exact_mut(component.width_stride * 2))
            {
                // upsample using the fn pointer, should only be H, so no need for
                // row up and row down
                (component.up_sampler)(single_row, &[], &[], &mut [], output_stride);
            }
        }
        SampleRatios::Generic(h, v) => {
            let raw_coeff = &component.raw_coeff;
            let dest_coeff = &mut component.upsample_dest;


            for (single_row, output_stride) in raw_coeff
                .chunks_exact(component.width_stride)
                .zip(dest_coeff.chunks_exact_mut(component.width_stride * h*v))
            {
                // upsample using the fn pointer, should only be H, so no need for
                // row up and row down
                (component.up_sampler)(single_row, &[], &[], &mut [], output_stride);
            }
        }
        SampleRatios::None => {}
    };
}
