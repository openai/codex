//! Encoding of AVIF images.
///
/// The [AVIF] specification defines an image derivative of the AV1 bitstream, an open video codec.
///
/// [AVIF]: https://aomediacodec.github.io/av1-avif/
#[cfg(feature = "avif-native")]
pub use self::decoder::AvifDecoder;
#[cfg(feature = "avif")]
pub use self::encoder::{AvifEncoder, ColorSpace};

#[cfg(feature = "avif-native")]
mod decoder;
#[cfg(feature = "avif")]
mod encoder;
#[cfg(feature = "avif-native")]
mod ycgco;
#[cfg(feature = "avif-native")]
mod yuv;
