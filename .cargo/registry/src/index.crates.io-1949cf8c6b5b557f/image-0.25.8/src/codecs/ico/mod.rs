//!  Decoding and Encoding of ICO files
//!
//!  A decoder and encoder for ICO (Windows Icon) image container files.
//!
//!  # Related Links
//!  * <https://msdn.microsoft.com/en-us/library/ms997538.aspx>
//!  * <https://en.wikipedia.org/wiki/ICO_%28file_format%29>

pub use self::decoder::IcoDecoder;
#[allow(deprecated)]
pub use self::encoder::{IcoEncoder, IcoFrame};

mod decoder;
mod encoder;
