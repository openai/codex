//! Filesystem path operations.

mod arg;
#[cfg(feature = "itoa")]
mod dec_int;

pub use arg::{option_into_with_c_str, Arg};
#[cfg(feature = "itoa")]
#[cfg_attr(docsrs, doc(cfg(feature = "itoa")))]
pub use dec_int::DecInt;

pub(crate) const SMALL_PATH_BUFFER_SIZE: usize = 256;
