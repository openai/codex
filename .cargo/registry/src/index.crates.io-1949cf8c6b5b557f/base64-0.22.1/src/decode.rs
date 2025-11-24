use crate::engine::{general_purpose::STANDARD, DecodeEstimate, Engine};
#[cfg(any(feature = "alloc", test))]
use alloc::vec::Vec;
use core::fmt;
#[cfg(any(feature = "std", test))]
use std::error;

/// Errors that can occur while decoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// An invalid byte was found in the input. The offset and offending byte are provided.
    ///
    /// Padding characters (`=`) interspersed in the encoded form are invalid, as they may only
    /// be present as the last 0-2 bytes of input.
    ///
    /// This error may also indicate that extraneous trailing input bytes are present, causing
    /// otherwise valid padding to no longer be the last bytes of input.
    InvalidByte(usize, u8),
    /// The length of the input, as measured in valid base64 symbols, is invalid.
    /// There must be 2-4 symbols in the last input quad.
    InvalidLength(usize),
    /// The last non-padding input symbol's encoded 6 bits have nonzero bits that will be discarded.
    /// This is indicative of corrupted or truncated Base64.
    /// Unlike [DecodeError::InvalidByte], which reports symbols that aren't in the alphabet,
    /// this error is for symbols that are in the alphabet but represent nonsensical encodings.
    InvalidLastSymbol(usize, u8),
    /// The nature of the padding was not as configured: absent or incorrect when it must be
    /// canonical, or present when it must be absent, etc.
    InvalidPadding,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::InvalidByte(index, byte) => {
                write!(f, "Invalid symbol {}, offset {}.", byte, index)
            }
            Self::InvalidLength(len) => write!(f, "Invalid input length: {}", len),
            Self::InvalidLastSymbol(index, byte) => {
                write!(f, "Invalid last symbol {}, offset {}.", byte, index)
            }
            Self::InvalidPadding => write!(f, "Invalid padding"),
        }
    }
}

#[cfg(any(feature = "std", test))]
impl error::Error for DecodeError {}

/// Errors that can occur while decoding into a slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeSliceError {
    /// A [DecodeError] occurred
    DecodeError(DecodeError),
    /// The provided slice is too small.
    OutputSliceTooSmall,
}

impl fmt::Display for DecodeSliceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DecodeError(e) => write!(f, "DecodeError: {}", e),
            Self::OutputSliceTooSmall => write!(f, "Output slice too small"),
        }
    }
}

#[cfg(any(feature = "std", test))]
impl error::Error for DecodeSliceError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            DecodeSliceError::DecodeError(e) => Some(e),
            DecodeSliceError::OutputSliceTooSmall => None,
        }
    }
}

impl From<DecodeError> for DecodeSliceError {
    fn from(e: DecodeError) -> Self {
        DecodeSliceError::DecodeError(e)
    }
}

/// Decode base64 using the [`STANDARD` engine](STANDARD).
///
/// See [Engine::decode].
#[deprecated(since = "0.21.0", note = "Use Engine::decode")]
#[cfg(any(feature = "alloc", test))]
pub fn decode<T: AsRef<[u8]>>(input: T) -> Result<Vec<u8>, DecodeError> {
    STANDARD.decode(input)
}

/// Decode from string reference as octets using the specified [Engine].
///
/// See [Engine::decode].
///Returns a `Result` containing a `Vec<u8>`.
#[deprecated(since = "0.21.0", note = "Use Engine::decode")]
#[cfg(any(feature = "alloc", test))]
pub fn decode_engine<E: Engine, T: AsRef<[u8]>>(
    input: T,
    engine: &E,
) -> Result<Vec<u8>, DecodeError> {
    engine.decode(input)
}

/// Decode from string reference as octets.
///
/// See [Engine::decode_vec].
#[cfg(any(feature = "alloc", test))]
#[deprecated(since = "0.21.0", note = "Use Engine::decode_vec")]
pub fn decode_engine_vec<E: Engine, T: AsRef<[u8]>>(
    input: T,
    buffer: &mut Vec<u8>,
    engine: &E,
) -> Result<(), DecodeError> {
    engine.decode_vec(input, buffer)
}

/// Decode the input into the provided output slice.
///
/// See [Engine::decode_slice].
#[deprecated(since = "0.21.0", note = "Use Engine::decode_slice")]
pub fn decode_engine_slice<E: Engine, T: AsRef<[u8]>>(
    input: T,
    output: &mut [u8],
    engine: &E,
) -> Result<usize, DecodeSliceError> {
    engine.decode_slice(input, output)
}

/// Returns a conservative estimate of the decoded size of `encoded_len` base64 symbols (rounded up
/// to the next group of 3 decoded bytes).
///
/// The resulting length will be a safe choice for the size of a decode buffer, but may have up to
/// 2 trailing bytes that won't end up being needed.
///
/// # Examples
///
/// ```
/// use base64::decoded_len_estimate;
///
/// assert_eq!(3, decoded_len_estimate(1));
/// assert_eq!(3, decoded_len_estimate(2));
/// assert_eq!(3, decoded_len_estimate(3));
/// assert_eq!(3, decoded_len_estimate(4));
/// // start of the next quad of encoded symbols
/// assert_eq!(6, decoded_len_estimate(5));
/// ```
pub fn decoded_len_estimate(encoded_len: usize) -> usize {
    STANDARD
        .internal_decoded_len_estimate(encoded_len)
        .decoded_len_estimate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        alphabet,
        engine::{general_purpose, Config, GeneralPurpose},
        tests::{assert_encode_sanity, random_engine},
    };
    use rand::{
        distributions::{Distribution, Uniform},
        Rng, SeedableRng,
    };

    #[test]
    fn decode_into_nonempty_vec_doesnt_clobber_existing_prefix() {
        let mut orig_data = Vec::new();
        let mut encoded_data = String::new();
        let mut decoded_with_prefix = Vec::new();
        let mut decoded_without_prefix = Vec::new();
        let mut prefix = Vec::new();

        let prefix_len_range = Uniform::new(0, 1000);
        let input_len_range = Uniform::new(0, 1000);

        let mut rng = rand::rngs::SmallRng::from_entropy();

        for _ in 0..10_000 {
            orig_data.clear();
            encoded_data.clear();
            decoded_with_prefix.clear();
            decoded_without_prefix.clear();
            prefix.clear();

            let input_len = input_len_range.sample(&mut rng);

            for _ in 0..input_len {
                orig_data.push(rng.gen());
            }

            let engine = random_engine(&mut rng);
            engine.encode_string(&orig_data, &mut encoded_data);
            assert_encode_sanity(&encoded_data, engine.config().encode_padding(), input_len);

            let prefix_len = prefix_len_range.sample(&mut rng);

            // fill the buf with a prefix
            for _ in 0..prefix_len {
                prefix.push(rng.gen());
            }

            decoded_with_prefix.resize(prefix_len, 0);
            decoded_with_prefix.copy_from_slice(&prefix);

            // decode into the non-empty buf
            engine
                .decode_vec(&encoded_data, &mut decoded_with_prefix)
                .unwrap();
            // also decode into the empty buf
            engine
                .decode_vec(&encoded_data, &mut decoded_without_prefix)
                .unwrap();

            assert_eq!(
                prefix_len + decoded_without_prefix.len(),
                decoded_with_prefix.len()
            );
            assert_eq!(orig_data, decoded_without_prefix);

            // append plain decode onto prefix
            prefix.append(&mut decoded_without_prefix);

            assert_eq!(prefix, decoded_with_prefix);
        }
    }

    #[test]
    fn decode_slice_doesnt_clobber_existing_prefix_or_suffix() {
        do_decode_slice_doesnt_clobber_existing_prefix_or_suffix(|e, input, output| {
            e.decode_slice(input, output).unwrap()
        })
    }

    #[test]
    fn decode_slice_unchecked_doesnt_clobber_existing_prefix_or_suffix() {
        do_decode_slice_doesnt_clobber_existing_prefix_or_suffix(|e, input, output| {
            e.decode_slice_unchecked(input, output).unwrap()
        })
    }

    #[test]
    fn decode_engine_estimation_works_for_various_lengths() {
        let engine = GeneralPurpose::new(&alphabet::STANDARD, general_purpose::NO_PAD);
        for num_prefix_quads in 0..100 {
            for suffix in &["AA", "AAA", "AAAA"] {
                let mut prefix = "AAAA".repeat(num_prefix_quads);
                prefix.push_str(suffix);
                // make sure no overflow (and thus a panic) occurs
                let res = engine.decode(prefix);
                assert!(res.is_ok());
            }
        }
    }

    #[test]
    fn decode_slice_output_length_errors() {
        for num_quads in 1..100 {
            let input = "AAAA".repeat(num_quads);
            let mut vec = vec![0; (num_quads - 1) * 3];
            assert_eq!(
                DecodeSliceError::OutputSliceTooSmall,
                STANDARD.decode_slice(&input, &mut vec).unwrap_err()
            );
            vec.push(0);
            assert_eq!(
                DecodeSliceError::OutputSliceTooSmall,
                STANDARD.decode_slice(&input, &mut vec).unwrap_err()
            );
            vec.push(0);
            assert_eq!(
                DecodeSliceError::OutputSliceTooSmall,
                STANDARD.decode_slice(&input, &mut vec).unwrap_err()
            );
            vec.push(0);
            // now it works
            assert_eq!(
                num_quads * 3,
                STANDARD.decode_slice(&input, &mut vec).unwrap()
            );
        }
    }

    fn do_decode_slice_doesnt_clobber_existing_prefix_or_suffix<
        F: Fn(&GeneralPurpose, &[u8], &mut [u8]) -> usize,
    >(
        call_decode: F,
    ) {
        let mut orig_data = Vec::new();
        let mut encoded_data = String::new();
        let mut decode_buf = Vec::new();
        let mut decode_buf_copy: Vec<u8> = Vec::new();

        let input_len_range = Uniform::new(0, 1000);

        let mut rng = rand::rngs::SmallRng::from_entropy();

        for _ in 0..10_000 {
            orig_data.clear();
            encoded_data.clear();
            decode_buf.clear();
            decode_buf_copy.clear();

            let input_len = input_len_range.sample(&mut rng);

            for _ in 0..input_len {
                orig_data.push(rng.gen());
            }

            let engine = random_engine(&mut rng);
            engine.encode_string(&orig_data, &mut encoded_data);
            assert_encode_sanity(&encoded_data, engine.config().encode_padding(), input_len);

            // fill the buffer with random garbage, long enough to have some room before and after
            for _ in 0..5000 {
                decode_buf.push(rng.gen());
            }

            // keep a copy for later comparison
            decode_buf_copy.extend(decode_buf.iter());

            let offset = 1000;

            // decode into the non-empty buf
            let decode_bytes_written =
                call_decode(&engine, encoded_data.as_bytes(), &mut decode_buf[offset..]);

            assert_eq!(orig_data.len(), decode_bytes_written);
            assert_eq!(
                orig_data,
                &decode_buf[offset..(offset + decode_bytes_written)]
            );
            assert_eq!(&decode_buf_copy[0..offset], &decode_buf[0..offset]);
            assert_eq!(
                &decode_buf_copy[offset + decode_bytes_written..],
                &decode_buf[offset + decode_bytes_written..]
            );
        }
    }
}

#[allow(deprecated)]
#[cfg(test)]
mod coverage_gaming {
    use super::*;
    use std::error::Error;

    #[test]
    fn decode_error() {
        let _ = format!("{:?}", DecodeError::InvalidPadding.clone());
        let _ = format!(
            "{} {} {} {}",
            DecodeError::InvalidByte(0, 0),
            DecodeError::InvalidLength(0),
            DecodeError::InvalidLastSymbol(0, 0),
            DecodeError::InvalidPadding,
        );
    }

    #[test]
    fn decode_slice_error() {
        let _ = format!("{:?}", DecodeSliceError::OutputSliceTooSmall.clone());
        let _ = format!(
            "{} {}",
            DecodeSliceError::OutputSliceTooSmall,
            DecodeSliceError::DecodeError(DecodeError::InvalidPadding)
        );
        let _ = DecodeSliceError::OutputSliceTooSmall.source();
        let _ = DecodeSliceError::DecodeError(DecodeError::InvalidPadding).source();
    }

    #[test]
    fn deprecated_fns() {
        let _ = decode("");
        let _ = decode_engine("", &crate::prelude::BASE64_STANDARD);
        let _ = decode_engine_vec("", &mut Vec::new(), &crate::prelude::BASE64_STANDARD);
        let _ = decode_engine_slice("", &mut [], &crate::prelude::BASE64_STANDARD);
    }

    #[test]
    fn decoded_len_est() {
        assert_eq!(3, decoded_len_estimate(4));
    }
}
