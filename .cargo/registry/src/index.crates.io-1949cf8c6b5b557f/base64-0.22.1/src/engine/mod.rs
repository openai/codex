//! Provides the [Engine] abstraction and out of the box implementations.
#[cfg(any(feature = "alloc", test))]
use crate::chunked_encoder;
use crate::{
    encode::{encode_with_padding, EncodeSliceError},
    encoded_len, DecodeError, DecodeSliceError,
};
#[cfg(any(feature = "alloc", test))]
use alloc::vec::Vec;

#[cfg(any(feature = "alloc", test))]
use alloc::{string::String, vec};

pub mod general_purpose;

#[cfg(test)]
mod naive;

#[cfg(test)]
mod tests;

pub use general_purpose::{GeneralPurpose, GeneralPurposeConfig};

/// An `Engine` provides low-level encoding and decoding operations that all other higher-level parts of the API use. Users of the library will generally not need to implement this.
///
/// Different implementations offer different characteristics. The library currently ships with
/// [GeneralPurpose] that offers good speed and works on any CPU, with more choices
/// coming later, like a constant-time one when side channel resistance is called for, and vendor-specific vectorized ones for more speed.
///
/// See [general_purpose::STANDARD_NO_PAD] if you just want standard base64. Otherwise, when possible, it's
/// recommended to store the engine in a `const` so that references to it won't pose any lifetime
/// issues, and to avoid repeating the cost of engine setup.
///
/// Since almost nobody will need to implement `Engine`, docs for internal methods are hidden.
// When adding an implementation of Engine, include them in the engine test suite:
// - add an implementation of [engine::tests::EngineWrapper]
// - add the implementation to the `all_engines` macro
// All tests run on all engines listed in the macro.
pub trait Engine: Send + Sync {
    /// The config type used by this engine
    type Config: Config;
    /// The decode estimate used by this engine
    type DecodeEstimate: DecodeEstimate;

    /// This is not meant to be called directly; it is only for `Engine` implementors.
    /// See the other `encode*` functions on this trait.
    ///
    /// Encode the `input` bytes into the `output` buffer based on the mapping in `encode_table`.
    ///
    /// `output` will be long enough to hold the encoded data.
    ///
    /// Returns the number of bytes written.
    ///
    /// No padding should be written; that is handled separately.
    ///
    /// Must not write any bytes into the output slice other than the encoded data.
    #[doc(hidden)]
    fn internal_encode(&self, input: &[u8], output: &mut [u8]) -> usize;

    /// This is not meant to be called directly; it is only for `Engine` implementors.
    ///
    /// As an optimization to prevent the decoded length from being calculated twice, it is
    /// sometimes helpful to have a conservative estimate of the decoded size before doing the
    /// decoding, so this calculation is done separately and passed to [Engine::decode()] as needed.
    #[doc(hidden)]
    fn internal_decoded_len_estimate(&self, input_len: usize) -> Self::DecodeEstimate;

    /// This is not meant to be called directly; it is only for `Engine` implementors.
    /// See the other `decode*` functions on this trait.
    ///
    /// Decode `input` base64 bytes into the `output` buffer.
    ///
    /// `decode_estimate` is the result of [Engine::internal_decoded_len_estimate()], which is passed in to avoid
    /// calculating it again (expensive on short inputs).`
    ///
    /// Each complete 4-byte chunk of encoded data decodes to 3 bytes of decoded data, but this
    /// function must also handle the final possibly partial chunk.
    /// If the input length is not a multiple of 4, or uses padding bytes to reach a multiple of 4,
    /// the trailing 2 or 3 bytes must decode to 1 or 2 bytes, respectively, as per the
    /// [RFC](https://tools.ietf.org/html/rfc4648#section-3.5).
    ///
    /// Decoding must not write any bytes into the output slice other than the decoded data.
    ///
    /// Non-canonical trailing bits in the final tokens or non-canonical padding must be reported as
    /// errors unless the engine is configured otherwise.
    #[doc(hidden)]
    fn internal_decode(
        &self,
        input: &[u8],
        output: &mut [u8],
        decode_estimate: Self::DecodeEstimate,
    ) -> Result<DecodeMetadata, DecodeSliceError>;

    /// Returns the config for this engine.
    fn config(&self) -> &Self::Config;

    /// Encode arbitrary octets as base64 using the provided `Engine`.
    /// Returns a `String`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use base64::{Engine as _, engine::{self, general_purpose}, alphabet};
    ///
    /// let b64 = general_purpose::STANDARD.encode(b"hello world~");
    /// println!("{}", b64);
    ///
    /// const CUSTOM_ENGINE: engine::GeneralPurpose =
    ///     engine::GeneralPurpose::new(&alphabet::URL_SAFE, general_purpose::NO_PAD);
    ///
    /// let b64_url = CUSTOM_ENGINE.encode(b"hello internet~");
    /// ```
    #[cfg(any(feature = "alloc", test))]
    #[inline]
    fn encode<T: AsRef<[u8]>>(&self, input: T) -> String {
        fn inner<E>(engine: &E, input_bytes: &[u8]) -> String
        where
            E: Engine + ?Sized,
        {
            let encoded_size = encoded_len(input_bytes.len(), engine.config().encode_padding())
                .expect("integer overflow when calculating buffer size");

            let mut buf = vec![0; encoded_size];

            encode_with_padding(input_bytes, &mut buf[..], engine, encoded_size);

            String::from_utf8(buf).expect("Invalid UTF8")
        }

        inner(self, input.as_ref())
    }

    /// Encode arbitrary octets as base64 into a supplied `String`.
    /// Writes into the supplied `String`, which may allocate if its internal buffer isn't big enough.
    ///
    /// # Example
    ///
    /// ```rust
    /// use base64::{Engine as _, engine::{self, general_purpose}, alphabet};
    /// const CUSTOM_ENGINE: engine::GeneralPurpose =
    ///     engine::GeneralPurpose::new(&alphabet::URL_SAFE, general_purpose::NO_PAD);
    ///
    /// fn main() {
    ///     let mut buf = String::new();
    ///     general_purpose::STANDARD.encode_string(b"hello world~", &mut buf);
    ///     println!("{}", buf);
    ///
    ///     buf.clear();
    ///     CUSTOM_ENGINE.encode_string(b"hello internet~", &mut buf);
    ///     println!("{}", buf);
    /// }
    /// ```
    #[cfg(any(feature = "alloc", test))]
    #[inline]
    fn encode_string<T: AsRef<[u8]>>(&self, input: T, output_buf: &mut String) {
        fn inner<E>(engine: &E, input_bytes: &[u8], output_buf: &mut String)
        where
            E: Engine + ?Sized,
        {
            let mut sink = chunked_encoder::StringSink::new(output_buf);

            chunked_encoder::ChunkedEncoder::new(engine)
                .encode(input_bytes, &mut sink)
                .expect("Writing to a String shouldn't fail");
        }

        inner(self, input.as_ref(), output_buf)
    }

    /// Encode arbitrary octets as base64 into a supplied slice.
    /// Writes into the supplied output buffer.
    ///
    /// This is useful if you wish to avoid allocation entirely (e.g. encoding into a stack-resident
    /// or statically-allocated buffer).
    ///
    /// # Example
    ///
    #[cfg_attr(feature = "alloc", doc = "```")]
    #[cfg_attr(not(feature = "alloc"), doc = "```ignore")]
    /// use base64::{Engine as _, engine::general_purpose};
    /// let s = b"hello internet!";
    /// let mut buf = Vec::new();
    /// // make sure we'll have a slice big enough for base64 + padding
    /// buf.resize(s.len() * 4 / 3 + 4, 0);
    ///
    /// let bytes_written = general_purpose::STANDARD.encode_slice(s, &mut buf).unwrap();
    ///
    /// // shorten our vec down to just what was written
    /// buf.truncate(bytes_written);
    ///
    /// assert_eq!(s, general_purpose::STANDARD.decode(&buf).unwrap().as_slice());
    /// ```
    #[inline]
    fn encode_slice<T: AsRef<[u8]>>(
        &self,
        input: T,
        output_buf: &mut [u8],
    ) -> Result<usize, EncodeSliceError> {
        fn inner<E>(
            engine: &E,
            input_bytes: &[u8],
            output_buf: &mut [u8],
        ) -> Result<usize, EncodeSliceError>
        where
            E: Engine + ?Sized,
        {
            let encoded_size = encoded_len(input_bytes.len(), engine.config().encode_padding())
                .expect("usize overflow when calculating buffer size");

            if output_buf.len() < encoded_size {
                return Err(EncodeSliceError::OutputSliceTooSmall);
            }

            let b64_output = &mut output_buf[0..encoded_size];

            encode_with_padding(input_bytes, b64_output, engine, encoded_size);

            Ok(encoded_size)
        }

        inner(self, input.as_ref(), output_buf)
    }

    /// Decode the input into a new `Vec`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use base64::{Engine as _, alphabet, engine::{self, general_purpose}};
    ///
    /// let bytes = general_purpose::STANDARD
    ///     .decode("aGVsbG8gd29ybGR+Cg==").unwrap();
    /// println!("{:?}", bytes);
    ///
    /// // custom engine setup
    /// let bytes_url = engine::GeneralPurpose::new(
    ///              &alphabet::URL_SAFE,
    ///              general_purpose::NO_PAD)
    ///     .decode("aGVsbG8gaW50ZXJuZXR-Cg").unwrap();
    /// println!("{:?}", bytes_url);
    /// ```
    #[cfg(any(feature = "alloc", test))]
    #[inline]
    fn decode<T: AsRef<[u8]>>(&self, input: T) -> Result<Vec<u8>, DecodeError> {
        fn inner<E>(engine: &E, input_bytes: &[u8]) -> Result<Vec<u8>, DecodeError>
        where
            E: Engine + ?Sized,
        {
            let estimate = engine.internal_decoded_len_estimate(input_bytes.len());
            let mut buffer = vec![0; estimate.decoded_len_estimate()];

            let bytes_written = engine
                .internal_decode(input_bytes, &mut buffer, estimate)
                .map_err(|e| match e {
                    DecodeSliceError::DecodeError(e) => e,
                    DecodeSliceError::OutputSliceTooSmall => {
                        unreachable!("Vec is sized conservatively")
                    }
                })?
                .decoded_len;

            buffer.truncate(bytes_written);

            Ok(buffer)
        }

        inner(self, input.as_ref())
    }

    /// Decode the `input` into the supplied `buffer`.
    ///
    /// Writes into the supplied `Vec`, which may allocate if its internal buffer isn't big enough.
    /// Returns a `Result` containing an empty tuple, aka `()`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use base64::{Engine as _, alphabet, engine::{self, general_purpose}};
    /// const CUSTOM_ENGINE: engine::GeneralPurpose =
    ///     engine::GeneralPurpose::new(&alphabet::URL_SAFE, general_purpose::PAD);
    ///
    /// fn main() {
    ///     use base64::Engine;
    ///     let mut buffer = Vec::<u8>::new();
    ///     // with the default engine
    ///     general_purpose::STANDARD
    ///         .decode_vec("aGVsbG8gd29ybGR+Cg==", &mut buffer,).unwrap();
    ///     println!("{:?}", buffer);
    ///
    ///     buffer.clear();
    ///
    ///     // with a custom engine
    ///     CUSTOM_ENGINE.decode_vec(
    ///         "aGVsbG8gaW50ZXJuZXR-Cg==",
    ///         &mut buffer,
    ///     ).unwrap();
    ///     println!("{:?}", buffer);
    /// }
    /// ```
    #[cfg(any(feature = "alloc", test))]
    #[inline]
    fn decode_vec<T: AsRef<[u8]>>(
        &self,
        input: T,
        buffer: &mut Vec<u8>,
    ) -> Result<(), DecodeError> {
        fn inner<E>(engine: &E, input_bytes: &[u8], buffer: &mut Vec<u8>) -> Result<(), DecodeError>
        where
            E: Engine + ?Sized,
        {
            let starting_output_len = buffer.len();
            let estimate = engine.internal_decoded_len_estimate(input_bytes.len());

            let total_len_estimate = estimate
                .decoded_len_estimate()
                .checked_add(starting_output_len)
                .expect("Overflow when calculating output buffer length");

            buffer.resize(total_len_estimate, 0);

            let buffer_slice = &mut buffer.as_mut_slice()[starting_output_len..];

            let bytes_written = engine
                .internal_decode(input_bytes, buffer_slice, estimate)
                .map_err(|e| match e {
                    DecodeSliceError::DecodeError(e) => e,
                    DecodeSliceError::OutputSliceTooSmall => {
                        unreachable!("Vec is sized conservatively")
                    }
                })?
                .decoded_len;

            buffer.truncate(starting_output_len + bytes_written);

            Ok(())
        }

        inner(self, input.as_ref(), buffer)
    }

    /// Decode the input into the provided output slice.
    ///
    /// Returns the number of bytes written to the slice, or an error if `output` is smaller than
    /// the estimated decoded length.
    ///
    /// This will not write any bytes past exactly what is decoded (no stray garbage bytes at the end).
    ///
    /// See [crate::decoded_len_estimate] for calculating buffer sizes.
    ///
    /// See [Engine::decode_slice_unchecked] for a version that panics instead of returning an error
    /// if the output buffer is too small.
    #[inline]
    fn decode_slice<T: AsRef<[u8]>>(
        &self,
        input: T,
        output: &mut [u8],
    ) -> Result<usize, DecodeSliceError> {
        fn inner<E>(
            engine: &E,
            input_bytes: &[u8],
            output: &mut [u8],
        ) -> Result<usize, DecodeSliceError>
        where
            E: Engine + ?Sized,
        {
            engine
                .internal_decode(
                    input_bytes,
                    output,
                    engine.internal_decoded_len_estimate(input_bytes.len()),
                )
                .map(|dm| dm.decoded_len)
        }

        inner(self, input.as_ref(), output)
    }

    /// Decode the input into the provided output slice.
    ///
    /// Returns the number of bytes written to the slice.
    ///
    /// This will not write any bytes past exactly what is decoded (no stray garbage bytes at the end).
    ///
    /// See [crate::decoded_len_estimate] for calculating buffer sizes.
    ///
    /// See [Engine::decode_slice] for a version that returns an error instead of panicking if the output
    /// buffer is too small.
    ///
    /// # Panics
    ///
    /// Panics if the provided output buffer is too small for the decoded data.
    #[inline]
    fn decode_slice_unchecked<T: AsRef<[u8]>>(
        &self,
        input: T,
        output: &mut [u8],
    ) -> Result<usize, DecodeError> {
        fn inner<E>(engine: &E, input_bytes: &[u8], output: &mut [u8]) -> Result<usize, DecodeError>
        where
            E: Engine + ?Sized,
        {
            engine
                .internal_decode(
                    input_bytes,
                    output,
                    engine.internal_decoded_len_estimate(input_bytes.len()),
                )
                .map(|dm| dm.decoded_len)
                .map_err(|e| match e {
                    DecodeSliceError::DecodeError(e) => e,
                    DecodeSliceError::OutputSliceTooSmall => {
                        panic!("Output slice is too small")
                    }
                })
        }

        inner(self, input.as_ref(), output)
    }
}

/// The minimal level of configuration that engines must support.
pub trait Config {
    /// Returns `true` if padding should be added after the encoded output.
    ///
    /// Padding is added outside the engine's encode() since the engine may be used
    /// to encode only a chunk of the overall output, so it can't always know when
    /// the output is "done" and would therefore need padding (if configured).
    // It could be provided as a separate parameter when encoding, but that feels like
    // leaking an implementation detail to the user, and it's hopefully more convenient
    // to have to only pass one thing (the engine) to any part of the API.
    fn encode_padding(&self) -> bool;
}

/// The decode estimate used by an engine implementation. Users do not need to interact with this;
/// it is only for engine implementors.
///
/// Implementors may store relevant data here when constructing this to avoid having to calculate
/// them again during actual decoding.
pub trait DecodeEstimate {
    /// Returns a conservative (err on the side of too big) estimate of the decoded length to use
    /// for pre-allocating buffers, etc.
    ///
    /// The estimate must be no larger than the next largest complete triple of decoded bytes.
    /// That is, the final quad of tokens to decode may be assumed to be complete with no padding.
    fn decoded_len_estimate(&self) -> usize;
}

/// Controls how pad bytes are handled when decoding.
///
/// Each [Engine] must support at least the behavior indicated by
/// [DecodePaddingMode::RequireCanonical], and may support other modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodePaddingMode {
    /// Canonical padding is allowed, but any fewer padding bytes than that is also allowed.
    Indifferent,
    /// Padding must be canonical (0, 1, or 2 `=` as needed to produce a 4 byte suffix).
    RequireCanonical,
    /// Padding must be absent -- for when you want predictable padding, without any wasted bytes.
    RequireNone,
}

/// Metadata about the result of a decode operation
#[derive(PartialEq, Eq, Debug)]
pub struct DecodeMetadata {
    /// Number of decoded bytes output
    pub(crate) decoded_len: usize,
    /// Offset of the first padding byte in the input, if any
    pub(crate) padding_offset: Option<usize>,
}

impl DecodeMetadata {
    pub(crate) fn new(decoded_bytes: usize, padding_index: Option<usize>) -> Self {
        Self {
            decoded_len: decoded_bytes,
            padding_offset: padding_index,
        }
    }
}
