use crate::{
    engine::{general_purpose::INVALID_VALUE, DecodeEstimate, DecodeMetadata, DecodePaddingMode},
    DecodeError, DecodeSliceError, PAD_BYTE,
};

#[doc(hidden)]
pub struct GeneralPurposeEstimate {
    /// input len % 4
    rem: usize,
    conservative_decoded_len: usize,
}

impl GeneralPurposeEstimate {
    pub(crate) fn new(encoded_len: usize) -> Self {
        let rem = encoded_len % 4;
        Self {
            rem,
            conservative_decoded_len: (encoded_len / 4 + (rem > 0) as usize) * 3,
        }
    }
}

impl DecodeEstimate for GeneralPurposeEstimate {
    fn decoded_len_estimate(&self) -> usize {
        self.conservative_decoded_len
    }
}

/// Helper to avoid duplicating num_chunks calculation, which is costly on short inputs.
/// Returns the decode metadata, or an error.
// We're on the fragile edge of compiler heuristics here. If this is not inlined, slow. If this is
// inlined(always), a different slow. plain ol' inline makes the benchmarks happiest at the moment,
// but this is fragile and the best setting changes with only minor code modifications.
#[inline]
pub(crate) fn decode_helper(
    input: &[u8],
    estimate: GeneralPurposeEstimate,
    output: &mut [u8],
    decode_table: &[u8; 256],
    decode_allow_trailing_bits: bool,
    padding_mode: DecodePaddingMode,
) -> Result<DecodeMetadata, DecodeSliceError> {
    let input_complete_nonterminal_quads_len =
        complete_quads_len(input, estimate.rem, output.len(), decode_table)?;

    const UNROLLED_INPUT_CHUNK_SIZE: usize = 32;
    const UNROLLED_OUTPUT_CHUNK_SIZE: usize = UNROLLED_INPUT_CHUNK_SIZE / 4 * 3;

    let input_complete_quads_after_unrolled_chunks_len =
        input_complete_nonterminal_quads_len % UNROLLED_INPUT_CHUNK_SIZE;

    let input_unrolled_loop_len =
        input_complete_nonterminal_quads_len - input_complete_quads_after_unrolled_chunks_len;

    // chunks of 32 bytes
    for (chunk_index, chunk) in input[..input_unrolled_loop_len]
        .chunks_exact(UNROLLED_INPUT_CHUNK_SIZE)
        .enumerate()
    {
        let input_index = chunk_index * UNROLLED_INPUT_CHUNK_SIZE;
        let chunk_output = &mut output[chunk_index * UNROLLED_OUTPUT_CHUNK_SIZE
            ..(chunk_index + 1) * UNROLLED_OUTPUT_CHUNK_SIZE];

        decode_chunk_8(
            &chunk[0..8],
            input_index,
            decode_table,
            &mut chunk_output[0..6],
        )?;
        decode_chunk_8(
            &chunk[8..16],
            input_index + 8,
            decode_table,
            &mut chunk_output[6..12],
        )?;
        decode_chunk_8(
            &chunk[16..24],
            input_index + 16,
            decode_table,
            &mut chunk_output[12..18],
        )?;
        decode_chunk_8(
            &chunk[24..32],
            input_index + 24,
            decode_table,
            &mut chunk_output[18..24],
        )?;
    }

    // remaining quads, except for the last possibly partial one, as it may have padding
    let output_unrolled_loop_len = input_unrolled_loop_len / 4 * 3;
    let output_complete_quad_len = input_complete_nonterminal_quads_len / 4 * 3;
    {
        let output_after_unroll = &mut output[output_unrolled_loop_len..output_complete_quad_len];

        for (chunk_index, chunk) in input
            [input_unrolled_loop_len..input_complete_nonterminal_quads_len]
            .chunks_exact(4)
            .enumerate()
        {
            let chunk_output = &mut output_after_unroll[chunk_index * 3..chunk_index * 3 + 3];

            decode_chunk_4(
                chunk,
                input_unrolled_loop_len + chunk_index * 4,
                decode_table,
                chunk_output,
            )?;
        }
    }

    super::decode_suffix::decode_suffix(
        input,
        input_complete_nonterminal_quads_len,
        output,
        output_complete_quad_len,
        decode_table,
        decode_allow_trailing_bits,
        padding_mode,
    )
}

/// Returns the length of complete quads, except for the last one, even if it is complete.
///
/// Returns an error if the output len is not big enough for decoding those complete quads, or if
/// the input % 4 == 1, and that last byte is an invalid value other than a pad byte.
///
/// - `input` is the base64 input
/// - `input_len_rem` is input len % 4
/// - `output_len` is the length of the output slice
pub(crate) fn complete_quads_len(
    input: &[u8],
    input_len_rem: usize,
    output_len: usize,
    decode_table: &[u8; 256],
) -> Result<usize, DecodeSliceError> {
    debug_assert!(input.len() % 4 == input_len_rem);

    // detect a trailing invalid byte, like a newline, as a user convenience
    if input_len_rem == 1 {
        let last_byte = input[input.len() - 1];
        // exclude pad bytes; might be part of padding that extends from earlier in the input
        if last_byte != PAD_BYTE && decode_table[usize::from(last_byte)] == INVALID_VALUE {
            return Err(DecodeError::InvalidByte(input.len() - 1, last_byte).into());
        }
    };

    // skip last quad, even if it's complete, as it may have padding
    let input_complete_nonterminal_quads_len = input
        .len()
        .saturating_sub(input_len_rem)
        // if rem was 0, subtract 4 to avoid padding
        .saturating_sub((input_len_rem == 0) as usize * 4);
    debug_assert!(
        input.is_empty() || (1..=4).contains(&(input.len() - input_complete_nonterminal_quads_len))
    );

    // check that everything except the last quad handled by decode_suffix will fit
    if output_len < input_complete_nonterminal_quads_len / 4 * 3 {
        return Err(DecodeSliceError::OutputSliceTooSmall);
    };
    Ok(input_complete_nonterminal_quads_len)
}

/// Decode 8 bytes of input into 6 bytes of output.
///
/// `input` is the 8 bytes to decode.
/// `index_at_start_of_input` is the offset in the overall input (used for reporting errors
/// accurately)
/// `decode_table` is the lookup table for the particular base64 alphabet.
/// `output` will have its first 6 bytes overwritten
// yes, really inline (worth 30-50% speedup)
#[inline(always)]
fn decode_chunk_8(
    input: &[u8],
    index_at_start_of_input: usize,
    decode_table: &[u8; 256],
    output: &mut [u8],
) -> Result<(), DecodeError> {
    let morsel = decode_table[usize::from(input[0])];
    if morsel == INVALID_VALUE {
        return Err(DecodeError::InvalidByte(index_at_start_of_input, input[0]));
    }
    let mut accum = u64::from(morsel) << 58;

    let morsel = decode_table[usize::from(input[1])];
    if morsel == INVALID_VALUE {
        return Err(DecodeError::InvalidByte(
            index_at_start_of_input + 1,
            input[1],
        ));
    }
    accum |= u64::from(morsel) << 52;

    let morsel = decode_table[usize::from(input[2])];
    if morsel == INVALID_VALUE {
        return Err(DecodeError::InvalidByte(
            index_at_start_of_input + 2,
            input[2],
        ));
    }
    accum |= u64::from(morsel) << 46;

    let morsel = decode_table[usize::from(input[3])];
    if morsel == INVALID_VALUE {
        return Err(DecodeError::InvalidByte(
            index_at_start_of_input + 3,
            input[3],
        ));
    }
    accum |= u64::from(morsel) << 40;

    let morsel = decode_table[usize::from(input[4])];
    if morsel == INVALID_VALUE {
        return Err(DecodeError::InvalidByte(
            index_at_start_of_input + 4,
            input[4],
        ));
    }
    accum |= u64::from(morsel) << 34;

    let morsel = decode_table[usize::from(input[5])];
    if morsel == INVALID_VALUE {
        return Err(DecodeError::InvalidByte(
            index_at_start_of_input + 5,
            input[5],
        ));
    }
    accum |= u64::from(morsel) << 28;

    let morsel = decode_table[usize::from(input[6])];
    if morsel == INVALID_VALUE {
        return Err(DecodeError::InvalidByte(
            index_at_start_of_input + 6,
            input[6],
        ));
    }
    accum |= u64::from(morsel) << 22;

    let morsel = decode_table[usize::from(input[7])];
    if morsel == INVALID_VALUE {
        return Err(DecodeError::InvalidByte(
            index_at_start_of_input + 7,
            input[7],
        ));
    }
    accum |= u64::from(morsel) << 16;

    output[..6].copy_from_slice(&accum.to_be_bytes()[..6]);

    Ok(())
}

/// Like [decode_chunk_8] but for 4 bytes of input and 3 bytes of output.
#[inline(always)]
fn decode_chunk_4(
    input: &[u8],
    index_at_start_of_input: usize,
    decode_table: &[u8; 256],
    output: &mut [u8],
) -> Result<(), DecodeError> {
    let morsel = decode_table[usize::from(input[0])];
    if morsel == INVALID_VALUE {
        return Err(DecodeError::InvalidByte(index_at_start_of_input, input[0]));
    }
    let mut accum = u32::from(morsel) << 26;

    let morsel = decode_table[usize::from(input[1])];
    if morsel == INVALID_VALUE {
        return Err(DecodeError::InvalidByte(
            index_at_start_of_input + 1,
            input[1],
        ));
    }
    accum |= u32::from(morsel) << 20;

    let morsel = decode_table[usize::from(input[2])];
    if morsel == INVALID_VALUE {
        return Err(DecodeError::InvalidByte(
            index_at_start_of_input + 2,
            input[2],
        ));
    }
    accum |= u32::from(morsel) << 14;

    let morsel = decode_table[usize::from(input[3])];
    if morsel == INVALID_VALUE {
        return Err(DecodeError::InvalidByte(
            index_at_start_of_input + 3,
            input[3],
        ));
    }
    accum |= u32::from(morsel) << 8;

    output[..3].copy_from_slice(&accum.to_be_bytes()[..3]);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::engine::general_purpose::STANDARD;

    #[test]
    fn decode_chunk_8_writes_only_6_bytes() {
        let input = b"Zm9vYmFy"; // "foobar"
        let mut output = [0_u8, 1, 2, 3, 4, 5, 6, 7];

        decode_chunk_8(&input[..], 0, &STANDARD.decode_table, &mut output).unwrap();
        assert_eq!(&vec![b'f', b'o', b'o', b'b', b'a', b'r', 6, 7], &output);
    }

    #[test]
    fn decode_chunk_4_writes_only_3_bytes() {
        let input = b"Zm9v"; // "foobar"
        let mut output = [0_u8, 1, 2, 3];

        decode_chunk_4(&input[..], 0, &STANDARD.decode_table, &mut output).unwrap();
        assert_eq!(&vec![b'f', b'o', b'o', 3], &output);
    }

    #[test]
    fn estimate_short_lengths() {
        for (range, decoded_len_estimate) in [
            (0..=0, 0),
            (1..=4, 3),
            (5..=8, 6),
            (9..=12, 9),
            (13..=16, 12),
            (17..=20, 15),
        ] {
            for encoded_len in range {
                let estimate = GeneralPurposeEstimate::new(encoded_len);
                assert_eq!(decoded_len_estimate, estimate.decoded_len_estimate());
            }
        }
    }

    #[test]
    fn estimate_via_u128_inflation() {
        // cover both ends of usize
        (0..1000)
            .chain(usize::MAX - 1000..=usize::MAX)
            .for_each(|encoded_len| {
                // inflate to 128 bit type to be able to safely use the easy formulas
                let len_128 = encoded_len as u128;

                let estimate = GeneralPurposeEstimate::new(encoded_len);
                assert_eq!(
                    (len_128 + 3) / 4 * 3,
                    estimate.conservative_decoded_len as u128
                );
            })
    }
}
