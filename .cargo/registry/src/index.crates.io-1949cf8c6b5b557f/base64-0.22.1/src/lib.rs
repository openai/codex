//! Correct, fast, and configurable [base64][] decoding and encoding. Base64
//! transports binary data efficiently in contexts where only plain text is
//! allowed.
//!
//! [base64]: https://developer.mozilla.org/en-US/docs/Glossary/Base64
//!
//! # Usage
//!
//! Use an [`Engine`] to decode or encode base64, configured with the base64
//! alphabet and padding behavior best suited to your application.
//!
//! ## Engine setup
//!
//! There is more than one way to encode a stream of bytes as “base64”.
//! Different applications use different encoding
//! [alphabets][alphabet::Alphabet] and
//! [padding behaviors][engine::general_purpose::GeneralPurposeConfig].
//!
//! ### Encoding alphabet
//!
//! Almost all base64 [alphabets][alphabet::Alphabet] use `A-Z`, `a-z`, and
//! `0-9`, which gives nearly 64 characters (26 + 26 + 10 = 62), but they differ
//! in their choice of their final 2.
//!
//! Most applications use the [standard][alphabet::STANDARD] alphabet specified
//! in [RFC 4648][rfc-alphabet].  If that’s all you need, you can get started
//! quickly by using the pre-configured
//! [`STANDARD`][engine::general_purpose::STANDARD] engine, which is also available
//! in the [`prelude`] module as shown here, if you prefer a minimal `use`
//! footprint.
//!
#![cfg_attr(feature = "alloc", doc = "```")]
#![cfg_attr(not(feature = "alloc"), doc = "```ignore")]
//! use base64::prelude::*;
//!
//! # fn main() -> Result<(), base64::DecodeError> {
//! assert_eq!(BASE64_STANDARD.decode(b"+uwgVQA=")?, b"\xFA\xEC\x20\x55\0");
//! assert_eq!(BASE64_STANDARD.encode(b"\xFF\xEC\x20\x55\0"), "/+wgVQA=");
//! # Ok(())
//! # }
//! ```
//!
//! [rfc-alphabet]: https://datatracker.ietf.org/doc/html/rfc4648#section-4
//!
//! Other common alphabets are available in the [`alphabet`] module.
//!
//! #### URL-safe alphabet
//!
//! The standard alphabet uses `+` and `/` as its two non-alphanumeric tokens,
//! which cannot be safely used in URL’s without encoding them as `%2B` and
//! `%2F`.
//!
//! To avoid that, some applications use a [“URL-safe” alphabet][alphabet::URL_SAFE],
//! which uses `-` and `_` instead. To use that alternative alphabet, use the
//! [`URL_SAFE`][engine::general_purpose::URL_SAFE] engine. This example doesn't
//! use [`prelude`] to show what a more explicit `use` would look like.
//!
#![cfg_attr(feature = "alloc", doc = "```")]
#![cfg_attr(not(feature = "alloc"), doc = "```ignore")]
//! use base64::{engine::general_purpose::URL_SAFE, Engine as _};
//!
//! # fn main() -> Result<(), base64::DecodeError> {
//! assert_eq!(URL_SAFE.decode(b"-uwgVQA=")?, b"\xFA\xEC\x20\x55\0");
//! assert_eq!(URL_SAFE.encode(b"\xFF\xEC\x20\x55\0"), "_-wgVQA=");
//! # Ok(())
//! # }
//! ```
//!
//! ### Padding characters
//!
//! Each base64 character represents 6 bits (2⁶ = 64) of the original binary
//! data, and every 3 bytes of input binary data will encode to 4 base64
//! characters (8 bits × 3 = 6 bits × 4 = 24 bits).
//!
//! When the input is not an even multiple of 3 bytes in length, [canonical][]
//! base64 encoders insert padding characters at the end, so that the output
//! length is always a multiple of 4:
//!
//! [canonical]: https://datatracker.ietf.org/doc/html/rfc4648#section-3.5
//!
#![cfg_attr(feature = "alloc", doc = "```")]
#![cfg_attr(not(feature = "alloc"), doc = "```ignore")]
//! use base64::{engine::general_purpose::STANDARD, Engine as _};
//!
//! assert_eq!(STANDARD.encode(b""),    "");
//! assert_eq!(STANDARD.encode(b"f"),   "Zg==");
//! assert_eq!(STANDARD.encode(b"fo"),  "Zm8=");
//! assert_eq!(STANDARD.encode(b"foo"), "Zm9v");
//! ```
//!
//! Canonical encoding ensures that base64 encodings will be exactly the same,
//! byte-for-byte, regardless of input length. But the `=` padding characters
//! aren’t necessary for decoding, and they may be omitted by using a
//! [`NO_PAD`][engine::general_purpose::NO_PAD] configuration:
//!
#![cfg_attr(feature = "alloc", doc = "```")]
#![cfg_attr(not(feature = "alloc"), doc = "```ignore")]
//! use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
//!
//! assert_eq!(STANDARD_NO_PAD.encode(b""),    "");
//! assert_eq!(STANDARD_NO_PAD.encode(b"f"),   "Zg");
//! assert_eq!(STANDARD_NO_PAD.encode(b"fo"),  "Zm8");
//! assert_eq!(STANDARD_NO_PAD.encode(b"foo"), "Zm9v");
//! ```
//!
//! The pre-configured `NO_PAD` engines will reject inputs containing padding
//! `=` characters. To encode without padding and still accept padding while
//! decoding, create an [engine][engine::general_purpose::GeneralPurpose] with
//! that [padding mode][engine::DecodePaddingMode].
//!
#![cfg_attr(feature = "alloc", doc = "```")]
#![cfg_attr(not(feature = "alloc"), doc = "```ignore")]
//! # use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
//! assert_eq!(STANDARD_NO_PAD.decode(b"Zm8="), Err(base64::DecodeError::InvalidPadding));
//! ```
//!
//! ### Further customization
//!
//! Decoding and encoding behavior can be customized by creating an
//! [engine][engine::GeneralPurpose] with an [alphabet][alphabet::Alphabet] and
//! [padding configuration][engine::GeneralPurposeConfig]:
//!
#![cfg_attr(feature = "alloc", doc = "```")]
#![cfg_attr(not(feature = "alloc"), doc = "```ignore")]
//! use base64::{engine, alphabet, Engine as _};
//!
//! // bizarro-world base64: +/ as the first symbols instead of the last
//! let alphabet =
//!     alphabet::Alphabet::new("+/ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789")
//!     .unwrap();
//!
//! // a very weird config that encodes with padding but requires no padding when decoding...?
//! let crazy_config = engine::GeneralPurposeConfig::new()
//!     .with_decode_allow_trailing_bits(true)
//!     .with_encode_padding(true)
//!     .with_decode_padding_mode(engine::DecodePaddingMode::RequireNone);
//!
//! let crazy_engine = engine::GeneralPurpose::new(&alphabet, crazy_config);
//!
//! let encoded = crazy_engine.encode(b"abc 123");
//!
//! ```
//!
//! ## Memory allocation
//!
//! The [decode][Engine::decode()] and [encode][Engine::encode()] engine methods
//! allocate memory for their results – `decode` returns a `Vec<u8>` and
//! `encode` returns a `String`. To instead decode or encode into a buffer that
//! you allocated, use one of the alternative methods:
//!
//! #### Decoding
//!
//! | Method                     | Output                        | Allocates memory              |
//! | -------------------------- | ----------------------------- | ----------------------------- |
//! | [`Engine::decode`]         | returns a new `Vec<u8>`       | always                        |
//! | [`Engine::decode_vec`]     | appends to provided `Vec<u8>` | if `Vec` lacks capacity       |
//! | [`Engine::decode_slice`]   | writes to provided `&[u8]`    | never
//!
//! #### Encoding
//!
//! | Method                     | Output                       | Allocates memory               |
//! | -------------------------- | ---------------------------- | ------------------------------ |
//! | [`Engine::encode`]         | returns a new `String`       | always                         |
//! | [`Engine::encode_string`]  | appends to provided `String` | if `String` lacks capacity     |
//! | [`Engine::encode_slice`]   | writes to provided `&[u8]`   | never                          |
//!
//! ## Input and output
//!
//! The `base64` crate can [decode][Engine::decode()] and
//! [encode][Engine::encode()] values in memory, or
//! [`DecoderReader`][read::DecoderReader] and
//! [`EncoderWriter`][write::EncoderWriter] provide streaming decoding and
//! encoding for any [readable][std::io::Read] or [writable][std::io::Write]
//! byte stream.
//!
//! #### Decoding
//!
#![cfg_attr(feature = "std", doc = "```")]
#![cfg_attr(not(feature = "std"), doc = "```ignore")]
//! # use std::io;
//! use base64::{engine::general_purpose::STANDARD, read::DecoderReader};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut input = io::stdin();
//! let mut decoder = DecoderReader::new(&mut input, &STANDARD);
//! io::copy(&mut decoder, &mut io::stdout())?;
//! # Ok(())
//! # }
//! ```
//!
//! #### Encoding
//!
#![cfg_attr(feature = "std", doc = "```")]
#![cfg_attr(not(feature = "std"), doc = "```ignore")]
//! # use std::io;
//! use base64::{engine::general_purpose::STANDARD, write::EncoderWriter};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut output = io::stdout();
//! let mut encoder = EncoderWriter::new(&mut output, &STANDARD);
//! io::copy(&mut io::stdin(), &mut encoder)?;
//! # Ok(())
//! # }
//! ```
//!
//! #### Display
//!
//! If you only need a base64 representation for implementing the
//! [`Display`][std::fmt::Display] trait, use
//! [`Base64Display`][display::Base64Display]:
//!
//! ```
//! use base64::{display::Base64Display, engine::general_purpose::STANDARD};
//!
//! let value = Base64Display::new(b"\0\x01\x02\x03", &STANDARD);
//! assert_eq!("base64: AAECAw==", format!("base64: {}", value));
//! ```
//!
//! # Panics
//!
//! If length calculations result in overflowing `usize`, a panic will result.

#![cfg_attr(feature = "cargo-clippy", allow(clippy::cast_lossless))]
#![deny(
    missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    unused_results,
    variant_size_differences
)]
#![forbid(unsafe_code)]
// Allow globally until https://github.com/rust-lang/rust-clippy/issues/8768 is resolved.
// The desired state is to allow it only for the rstest_reuse import.
#![allow(clippy::single_component_path_imports)]
#![cfg_attr(not(any(feature = "std", test)), no_std)]

#[cfg(any(feature = "alloc", test))]
extern crate alloc;

// has to be included at top level because of the way rstest_reuse defines its macros
#[cfg(test)]
use rstest_reuse;

mod chunked_encoder;
pub mod display;
#[cfg(any(feature = "std", test))]
pub mod read;
#[cfg(any(feature = "std", test))]
pub mod write;

pub mod engine;
pub use engine::Engine;

pub mod alphabet;

mod encode;
#[allow(deprecated)]
#[cfg(any(feature = "alloc", test))]
pub use crate::encode::{encode, encode_engine, encode_engine_string};
#[allow(deprecated)]
pub use crate::encode::{encode_engine_slice, encoded_len, EncodeSliceError};

mod decode;
#[allow(deprecated)]
#[cfg(any(feature = "alloc", test))]
pub use crate::decode::{decode, decode_engine, decode_engine_vec};
#[allow(deprecated)]
pub use crate::decode::{decode_engine_slice, decoded_len_estimate, DecodeError, DecodeSliceError};

pub mod prelude;

#[cfg(test)]
mod tests;

const PAD_BYTE: u8 = b'=';
