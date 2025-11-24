#![doc(html_root_url = "https://docs.rs/prost/0.13.5")]
#![cfg_attr(not(feature = "std"), no_std)]
#![doc = include_str!("../README.md")]

// Re-export the alloc crate for use within derived code.
#[doc(hidden)]
pub extern crate alloc;

// Re-export the bytes crate for use within derived code.
pub use bytes;

mod error;
mod message;
mod name;
mod types;

#[doc(hidden)]
pub mod encoding;

pub use crate::encoding::length_delimiter::{
    decode_length_delimiter, encode_length_delimiter, length_delimiter_len,
};
pub use crate::error::{DecodeError, EncodeError, UnknownEnumValue};
pub use crate::message::Message;
pub use crate::name::Name;

// See `encoding::DecodeContext` for more info.
// 100 is the default recursion limit in the C++ implementation.
#[cfg(not(feature = "no-recursion-limit"))]
const RECURSION_LIMIT: u32 = 100;

// Re-export #[derive(Message, Enumeration, Oneof)].
// Based on serde's equivalent re-export [1], but enabled by default.
//
// [1]: https://github.com/serde-rs/serde/blob/v1.0.89/serde/src/lib.rs#L245-L256
#[cfg(feature = "derive")]
#[allow(unused_imports)]
#[macro_use]
extern crate prost_derive;
#[cfg(feature = "derive")]
#[doc(hidden)]
pub use prost_derive::*;
