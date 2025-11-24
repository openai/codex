/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

pub fn upsample_horizontal(
    input: &[i16], _ref: &[i16], _in_near: &[i16], _scratch: &mut [i16], output: &mut [i16]
) {
    assert_eq!(
        input.len() * 2,
        output.len(),
        "Input length is not half the size of the output length"
    );
    assert!(
        output.len() > 4 && input.len() > 2,
        "Too Short of a vector, cannot upsample"
    );

    output[0] = input[0];
    output[1] = (input[0] * 3 + input[1] + 2) >> 2;

    // This code is written for speed and not readability
    //
    // The readable code is
    //
    //      for i in 1..input.len() - 1{
    //         let sample = 3 * input[i] + 2;
    //         out[i * 2] = (sample + input[i - 1]) >> 2;
    //         out[i * 2 + 1] = (sample + input[i + 1]) >> 2;
    //     }
    //
    // The output of a pixel is determined by it's surrounding neighbours but we attach more weight to it's nearest
    // neighbour (input[i]) than to the next nearest neighbour.

    for (output_window, input_window) in output[2..].chunks_exact_mut(2).zip(input.windows(3)) {
        let sample = 3 * input_window[1] + 2;

        output_window[0] = (sample + input_window[0]) >> 2;
        output_window[1] = (sample + input_window[2]) >> 2;
    }
    // Get lengths
    let out_len = output.len() - 2;
    let input_len = input.len() - 2;

    // slice the output vector
    let f_out = &mut output[out_len..];
    let i_last = &input[input_len..];

    // write out manually..
    f_out[0] = (3 * i_last[0] + i_last[1] + 2) >> 2;
    f_out[1] = i_last[1];
}
pub fn upsample_vertical(
    input: &[i16], in_near: &[i16], in_far: &[i16], _scratch_space: &mut [i16], output: &mut [i16]
) {
    assert_eq!(input.len() * 2, output.len());
    assert_eq!(in_near.len(), input.len());
    assert_eq!(in_far.len(), input.len());

    let middle = output.len() / 2;

    let (out_top, out_bottom) = output.split_at_mut(middle);

    // for the first row, closest row is in_near
    for ((near, far), x) in input.iter().zip(in_near.iter()).zip(out_top) {
        *x = (((3 * near) + 2) + far) >> 2;
    }
    // for the second row, the closest row to input is in_far
    for ((near, far), x) in input.iter().zip(in_far.iter()).zip(out_bottom) {
        *x = (((3 * near) + 2) + far) >> 2;
    }
}

pub fn upsample_hv(
    input: &[i16], in_near: &[i16], in_far: &[i16], scratch_space: &mut [i16], output: &mut [i16]
) {
    assert_eq!(input.len() * 4, output.len());

    let mut t = [0];
    upsample_vertical(input, in_near, in_far, &mut t, scratch_space);
    // horizontal upsampling must be done separate for every line
    // Otherwise it introduces artifacts that may cause the edge colors
    // to appear on the other line.

    // Since this is called for two scanlines/widths currently
    // splitting the inputs and outputs into half ensures we only handle
    // one scanline per iteration
    let scratch_half = scratch_space.len() / 2;

    let output_half = output.len() / 2;

    upsample_horizontal(
        &scratch_space[..scratch_half],
        &[],
        &[],
        &mut t,
        &mut output[..output_half]
    );

    upsample_horizontal(
        &scratch_space[scratch_half..],
        &[],
        &[],
        &mut t,
        &mut output[output_half..]
    );
}

pub fn upsample_generic(
    input: &[i16], _in_near: &[i16], _in_far: &[i16], _scratch_space: &mut [i16],
    output: &mut [i16]
) {
    // use nearest sample
    let difference = output.len() / input.len();
    if difference > 0 {
        // nearest neighbour
        for (input, chunk_output) in input.iter().zip(output.chunks_exact_mut(difference)) {
            chunk_output.iter_mut().for_each(|x| *x = *input);
        }
    }
}
