//!  Decoding of Radiance HDR Images
//!
//!  A decoder for Radiance HDR images
//!
//!  # Related Links
//!
//!  * <http://radsite.lbl.gov/radiance/refer/filefmts.pdf>
//!  * <http://www.graphics.cornell.edu/~bjw/rgbe/rgbe.c>

mod decoder;
mod encoder;

pub use self::decoder::*;
pub use self::encoder::*;
