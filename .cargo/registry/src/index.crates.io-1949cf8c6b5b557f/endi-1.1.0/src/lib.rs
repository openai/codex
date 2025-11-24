#![allow(clippy::unusual_byte_groupings)]
#![deny(rust_2018_idioms)]
#![doc = include_str!("../README.md")]
#![doc(test(attr(warn(unused), deny(warnings))))]

mod endian;
pub use endian::{Endian, BE, LE, NATIVE_ENDIAN, NETWORK_ENDIAN};

#[cfg(feature = "std")]
mod io;
#[cfg(feature = "std")]
pub use io::{ReadBytes, WriteBytes};
