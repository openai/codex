//! Filesystem path operations.

mod arg;
mod dec_int;

pub use arg::{option_into_with_c_str, Arg};
pub use dec_int::{DecInt, Integer};

pub(crate) const SMALL_PATH_BUFFER_SIZE: usize = 256;
