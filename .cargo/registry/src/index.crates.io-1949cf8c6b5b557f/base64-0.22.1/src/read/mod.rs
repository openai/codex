//! Implementations of `io::Read` to transparently decode base64.
mod decoder;
pub use self::decoder::DecoderReader;

#[cfg(test)]
mod decoder_tests;
