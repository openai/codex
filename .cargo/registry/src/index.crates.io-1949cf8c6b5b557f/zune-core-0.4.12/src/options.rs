//! Decoder and Encoder Options
//!
//! This module exposes a struct for which all implemented
//! decoders get shared options for decoding
//!
//! All supported options are put into one _Options to allow for global configurations
//! options e.g the same  `DecoderOption` can be reused for all other decoders
//!
pub use decoder::DecoderOptions;
pub use encoder::EncoderOptions;

mod decoder;
mod encoder;
