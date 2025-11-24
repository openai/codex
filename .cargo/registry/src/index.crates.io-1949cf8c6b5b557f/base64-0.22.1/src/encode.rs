#[cfg(any(feature = "alloc", test))]
use alloc::string::String;
use core::fmt;
#[cfg(any(feature = "std", test))]
use std::error;

#[cfg(any(feature = "alloc", test))]
use crate::engine::general_purpose::STANDARD;
use crate::engine::{Config, Engine};
use crate::PAD_BYTE;

/// Encode arbitrary octets as base64 using the [`STANDARD` engine](STANDARD).
///
/// See [Engine::encode].
#[allow(unused)]
#[deprecated(since = "0.21.0", note = "Use Engine::encode")]
#[cfg(any(feature = "alloc", test))]
pub fn encode<T: AsRef<[u8]>>(input: T) -> String {
    STANDARD.encode(input)
}

///Encode arbitrary octets as base64 using the provided `Engine` into a new `String`.
///
/// See [Engine::encode].
#[allow(unused)]
#[deprecated(since = "0.21.0", note = "Use Engine::encode")]
#[cfg(any(feature = "alloc", test))]
pub fn encode_engine<E: Engine, T: AsRef<[u8]>>(input: T, engine: &E) -> String {
    engine.encode(input)
}

///Encode arbitrary octets as base64 into a supplied `String`.
///
/// See [Engine::encode_string].
#[allow(unused)]
#[deprecated(since = "0.21.0", note = "Use Engine::encode_string")]
#[cfg(any(feature = "alloc", test))]
pub fn encode_engine_string<E: Engine, T: AsRef<[u8]>>(
    input: T,
    output_buf: &mut String,
    engine: &E,
) {
    engine.encode_string(input, output_buf)
}

/// Encode arbitrary octets as base64 into a supplied slice.
///
/// See [Engine::encode_slice].
#[allow(unused)]
#[deprecated(since = "0.21.0", note = "Use Engine::encode_slice")]
pub fn encode_engine_slice<E: Engine, T: AsRef<[u8]>>(
    input: T,
    output_buf: &mut [u8],
    engine: &E,
) -> Result<usize, EncodeSliceError> {
    engine.encode_slice(input, output_buf)
}

/// B64-encode and pad (if configured).
///
/// This helper exists to avoid recalculating encoded_size, which is relatively expensive on short
/// inputs.
///
/// `encoded_size` is the encoded size calculated for `input`.
///
/// `output` must be of size `encoded_size`.
///
/// All bytes in `output` will be written to since it is exactly the size of the output.
pub(crate) fn encode_with_padding<E: Engine + ?Sized>(
    input: &[u8],
    output: &mut [u8],
    engine: &E,
    expected_encoded_size: usize,
) {
    debug_assert_eq!(expected_encoded_size, output.len());

    let b64_bytes_written = engine.internal_encode(input, output);

    let padding_bytes = if engine.config().encode_padding() {
        add_padding(b64_bytes_written, &mut output[b64_bytes_written..])
    } else {
        0
    };

    let encoded_bytes = b64_bytes_written
        .checked_add(padding_bytes)
        .expect("usize overflow when calculating b64 length");

    debug_assert_eq!(expected_encoded_size, encoded_bytes);
}

/// Calculate the base64 encoded length for a given input length, optionally including any
/// appropriate padding bytes.
///
/// Returns `None` if the encoded length can't be represented in `usize`. This will happen for
/// input lengths in approximately the top quarter of the range of `usize`.
pub const fn encoded_len(bytes_len: usize, padding: bool) -> Option<usize> {
    let rem = bytes_len % 3;

    let complete_input_chunks = bytes_len / 3;
    // `?` is disallowed in const, and `let Some(_) = _ else` requires 1.65.0, whereas this
    // messier syntax works on 1.48
    let complete_chunk_output =
        if let Some(complete_chunk_output) = complete_input_chunks.checked_mul(4) {
            complete_chunk_output
        } else {
            return None;
        };

    if rem > 0 {
        if padding {
            complete_chunk_output.checked_add(4)
        } else {
            let encoded_rem = match rem {
                1 => 2,
                // only other possible remainder is 2
                // can't use a separate _ => unreachable!() in const fns in ancient rust versions
                _ => 3,
            };
            complete_chunk_output.checked_add(encoded_rem)
        }
    } else {
        Some(complete_chunk_output)
    }
}

/// Write padding characters.
/// `unpadded_output_len` is the size of the unpadded but base64 encoded data.
/// `output` is the slice where padding should be written, of length at least 2.
///
/// Returns the number of padding bytes written.
pub(crate) fn add_padding(unpadded_output_len: usize, output: &mut [u8]) -> usize {
    let pad_bytes = (4 - (unpadded_output_len % 4)) % 4;
    // for just a couple bytes, this has better performance than using
    // .fill(), or iterating over mutable refs, which call memset()
    #[allow(clippy::needless_range_loop)]
    for i in 0..pad_bytes {
        output[i] = PAD_BYTE;
    }

    pad_bytes
}

/// Errors that can occur while encoding into a slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncodeSliceError {
    /// The provided slice is too small.
    OutputSliceTooSmall,
}

impl fmt::Display for EncodeSliceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutputSliceTooSmall => write!(f, "Output slice too small"),
        }
    }
}

#[cfg(any(feature = "std", test))]
impl error::Error for EncodeSliceError {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        alphabet,
        engine::general_purpose::{GeneralPurpose, NO_PAD, STANDARD},
        tests::{assert_encode_sanity, random_config, random_engine},
    };
    use rand::{
        distributions::{Distribution, Uniform},
        Rng, SeedableRng,
    };
    use std::str;

    const URL_SAFE_NO_PAD_ENGINE: GeneralPurpose = GeneralPurpose::new(&alphabet::URL_SAFE, NO_PAD);

    #[test]
    fn encoded_size_correct_standard() {
        assert_encoded_length(0, 0, &STANDARD, true);

        assert_encoded_length(1, 4, &STANDARD, true);
        assert_encoded_length(2, 4, &STANDARD, true);
        assert_encoded_length(3, 4, &STANDARD, true);

        assert_encoded_length(4, 8, &STANDARD, true);
        assert_encoded_length(5, 8, &STANDARD, true);
        assert_encoded_length(6, 8, &STANDARD, true);

        assert_encoded_length(7, 12, &STANDARD, true);
        assert_encoded_length(8, 12, &STANDARD, true);
        assert_encoded_length(9, 12, &STANDARD, true);

        assert_encoded_length(54, 72, &STANDARD, true);

        assert_encoded_length(55, 76, &STANDARD, true);
        assert_encoded_length(56, 76, &STANDARD, true);
        assert_encoded_length(57, 76, &STANDARD, true);

        assert_encoded_length(58, 80, &STANDARD, true);
    }

    #[test]
    fn encoded_size_correct_no_pad() {
        assert_encoded_length(0, 0, &URL_SAFE_NO_PAD_ENGINE, false);

        assert_encoded_length(1, 2, &URL_SAFE_NO_PAD_ENGINE, false);
        assert_encoded_length(2, 3, &URL_SAFE_NO_PAD_ENGINE, false);
        assert_encoded_length(3, 4, &URL_SAFE_NO_PAD_ENGINE, false);

        assert_encoded_length(4, 6, &URL_SAFE_NO_PAD_ENGINE, false);
        assert_encoded_length(5, 7, &URL_SAFE_NO_PAD_ENGINE, false);
        assert_encoded_length(6, 8, &URL_SAFE_NO_PAD_ENGINE, false);

        assert_encoded_length(7, 10, &URL_SAFE_NO_PAD_ENGINE, false);
        assert_encoded_length(8, 11, &URL_SAFE_NO_PAD_ENGINE, false);
        assert_encoded_length(9, 12, &URL_SAFE_NO_PAD_ENGINE, false);

        assert_encoded_length(54, 72, &URL_SAFE_NO_PAD_ENGINE, false);

        assert_encoded_length(55, 74, &URL_SAFE_NO_PAD_ENGINE, false);
        assert_encoded_length(56, 75, &URL_SAFE_NO_PAD_ENGINE, false);
        assert_encoded_length(57, 76, &URL_SAFE_NO_PAD_ENGINE, false);

        assert_encoded_length(58, 78, &URL_SAFE_NO_PAD_ENGINE, false);
    }

    #[test]
    fn encoded_size_overflow() {
        assert_eq!(None, encoded_len(usize::MAX, true));
    }

    #[test]
    fn encode_engine_string_into_nonempty_buffer_doesnt_clobber_prefix() {
        let mut orig_data = Vec::new();
        let mut prefix = String::new();
        let mut encoded_data_no_prefix = String::new();
        let mut encoded_data_with_prefix = String::new();
        let mut decoded = Vec::new();

        let prefix_len_range = Uniform::new(0, 1000);
        let input_len_range = Uniform::new(0, 1000);

        let mut rng = rand::rngs::SmallRng::from_entropy();

        for _ in 0..10_000 {
            orig_data.clear();
            prefix.clear();
            encoded_data_no_prefix.clear();
            encoded_data_with_prefix.clear();
            decoded.clear();

            let input_len = input_len_range.sample(&mut rng);

            for _ in 0..input_len {
                orig_data.push(rng.gen());
            }

            let prefix_len = prefix_len_range.sample(&mut rng);
            for _ in 0..prefix_len {
                // getting convenient random single-byte printable chars that aren't base64 is
                // annoying
                prefix.push('#');
            }
            encoded_data_with_prefix.push_str(&prefix);

            let engine = random_engine(&mut rng);
            engine.encode_string(&orig_data, &mut encoded_data_no_prefix);
            engine.encode_string(&orig_data, &mut encoded_data_with_prefix);

            assert_eq!(
                encoded_data_no_prefix.len() + prefix_len,
                encoded_data_with_prefix.len()
            );
            assert_encode_sanity(
                &encoded_data_no_prefix,
                engine.config().encode_padding(),
                input_len,
            );
            assert_encode_sanity(
                &encoded_data_with_prefix[prefix_len..],
                engine.config().encode_padding(),
                input_len,
            );

            // append plain encode onto prefix
            prefix.push_str(&encoded_data_no_prefix);

            assert_eq!(prefix, encoded_data_with_prefix);

            engine
                .decode_vec(&encoded_data_no_prefix, &mut decoded)
                .unwrap();
            assert_eq!(orig_data, decoded);
        }
    }

    #[test]
    fn encode_engine_slice_into_nonempty_buffer_doesnt_clobber_suffix() {
        let mut orig_data = Vec::new();
        let mut encoded_data = Vec::new();
        let mut encoded_data_original_state = Vec::new();
        let mut decoded = Vec::new();

        let input_len_range = Uniform::new(0, 1000);

        let mut rng = rand::rngs::SmallRng::from_entropy();

        for _ in 0..10_000 {
            orig_data.clear();
            encoded_data.clear();
            encoded_data_original_state.clear();
            decoded.clear();

            let input_len = input_len_range.sample(&mut rng);

            for _ in 0..input_len {
                orig_data.push(rng.gen());
            }

            // plenty of existing garbage in the encoded buffer
            for _ in 0..10 * input_len {
                encoded_data.push(rng.gen());
            }

            encoded_data_original_state.extend_from_slice(&encoded_data);

            let engine = random_engine(&mut rng);

            let encoded_size = encoded_len(input_len, engine.config().encode_padding()).unwrap();

            assert_eq!(
                encoded_size,
                engine.encode_slice(&orig_data, &mut encoded_data).unwrap()
            );

            assert_encode_sanity(
                str::from_utf8(&encoded_data[0..encoded_size]).unwrap(),
                engine.config().encode_padding(),
                input_len,
            );

            assert_eq!(
                &encoded_data[encoded_size..],
                &encoded_data_original_state[encoded_size..]
            );

            engine
                .decode_vec(&encoded_data[0..encoded_size], &mut decoded)
                .unwrap();
            assert_eq!(orig_data, decoded);
        }
    }

    #[test]
    fn encode_to_slice_random_valid_utf8() {
        let mut input = Vec::new();
        let mut output = Vec::new();

        let input_len_range = Uniform::new(0, 1000);

        let mut rng = rand::rngs::SmallRng::from_entropy();

        for _ in 0..10_000 {
            input.clear();
            output.clear();

            let input_len = input_len_range.sample(&mut rng);

            for _ in 0..input_len {
                input.push(rng.gen());
            }

            let config = random_config(&mut rng);
            let engine = random_engine(&mut rng);

            // fill up the output buffer with garbage
            let encoded_size = encoded_len(input_len, config.encode_padding()).unwrap();
            for _ in 0..encoded_size {
                output.push(rng.gen());
            }

            let orig_output_buf = output.clone();

            let bytes_written = engine.internal_encode(&input, &mut output);

            // make sure the part beyond bytes_written is the same garbage it was before
            assert_eq!(orig_output_buf[bytes_written..], output[bytes_written..]);

            // make sure the encoded bytes are UTF-8
            let _ = str::from_utf8(&output[0..bytes_written]).unwrap();
        }
    }

    #[test]
    fn encode_with_padding_random_valid_utf8() {
        let mut input = Vec::new();
        let mut output = Vec::new();

        let input_len_range = Uniform::new(0, 1000);

        let mut rng = rand::rngs::SmallRng::from_entropy();

        for _ in 0..10_000 {
            input.clear();
            output.clear();

            let input_len = input_len_range.sample(&mut rng);

            for _ in 0..input_len {
                input.push(rng.gen());
            }

            let engine = random_engine(&mut rng);

            // fill up the output buffer with garbage
            let encoded_size = encoded_len(input_len, engine.config().encode_padding()).unwrap();
            for _ in 0..encoded_size + 1000 {
                output.push(rng.gen());
            }

            let orig_output_buf = output.clone();

            encode_with_padding(&input, &mut output[0..encoded_size], &engine, encoded_size);

            // make sure the part beyond b64 is the same garbage it was before
            assert_eq!(orig_output_buf[encoded_size..], output[encoded_size..]);

            // make sure the encoded bytes are UTF-8
            let _ = str::from_utf8(&output[0..encoded_size]).unwrap();
        }
    }

    #[test]
    fn add_padding_random_valid_utf8() {
        let mut output = Vec::new();

        let mut rng = rand::rngs::SmallRng::from_entropy();

        // cover our bases for length % 4
        for unpadded_output_len in 0..20 {
            output.clear();

            // fill output with random
            for _ in 0..100 {
                output.push(rng.gen());
            }

            let orig_output_buf = output.clone();

            let bytes_written = add_padding(unpadded_output_len, &mut output);

            // make sure the part beyond bytes_written is the same garbage it was before
            assert_eq!(orig_output_buf[bytes_written..], output[bytes_written..]);

            // make sure the encoded bytes are UTF-8
            let _ = str::from_utf8(&output[0..bytes_written]).unwrap();
        }
    }

    fn assert_encoded_length<E: Engine>(
        input_len: usize,
        enc_len: usize,
        engine: &E,
        padded: bool,
    ) {
        assert_eq!(enc_len, encoded_len(input_len, padded).unwrap());

        let mut bytes: Vec<u8> = Vec::new();
        let mut rng = rand::rngs::SmallRng::from_entropy();

        for _ in 0..input_len {
            bytes.push(rng.gen());
        }

        let encoded = engine.encode(&bytes);
        assert_encode_sanity(&encoded, padded, input_len);

        assert_eq!(enc_len, encoded.len());
    }

    #[test]
    fn encode_imap() {
        assert_eq!(
            &GeneralPurpose::new(&alphabet::IMAP_MUTF7, NO_PAD).encode(b"\xFB\xFF"),
            &GeneralPurpose::new(&alphabet::STANDARD, NO_PAD)
                .encode(b"\xFB\xFF")
                .replace('/', ",")
        );
    }
}
