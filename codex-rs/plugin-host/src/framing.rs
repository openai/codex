//! Shared wire-format configuration for both ends of the plugin pipes.

use tokio_util::codec::LengthDelimitedCodec;

pub const MAX_FRAME: usize = 256 * 1024;

pub fn codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .little_endian()
        .length_field_length(/*val*/ 4)
        .max_frame_length(MAX_FRAME)
        .new_codec()
}
