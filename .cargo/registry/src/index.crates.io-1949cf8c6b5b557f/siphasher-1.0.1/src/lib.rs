#![doc = include_str!("../README.md")]

#![cfg_attr(not(test), no_std)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::many_single_char_names)]

pub mod sip;
pub mod sip128;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests128;

#[cfg(any(feature = "serde", feature = "serde_std", feature = "serde_no_std"))]
pub mod reexports {
    pub use serde;
    #[cfg(feature = "serde_json")]
    pub use serde_json;
}

pub mod prelude {
    pub use core::hash::Hasher as _;

    pub use sip128::Hasher128 as _;

    pub use crate::{sip, sip128};
}
