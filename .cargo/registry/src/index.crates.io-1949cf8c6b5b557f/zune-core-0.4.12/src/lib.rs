/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Core routines shared by all libraries
//!
//! This crate provides a set of core routines shared
//! by the decoders and encoders under `zune` umbrella
//!
//! It currently contains
//!
//! - A bytestream reader and writer with endian aware reads and writes
//! - Colorspace and bit depth information shared by images
//! - Image decoder and encoder options
//! - A simple enum type to hold image decoding results.
//!
//! This library is `#[no_std]` with `alloc` feature needed for defining `Vec`
//! which we need for storing decoded  bytes.
//!
//!
//! # Features
//!  - `no_std`: Enables `#[no_std]` compilation support.
//!
//!  - `serde`: Enables serializing of some of the data structures
//!     present in the crate
//!
#![cfg_attr(not(feature = "std"), no_std)]
#![macro_use]
extern crate alloc;

#[cfg(not(feature = "log"))]
pub mod log;

#[cfg(feature = "log")]
pub use log;

pub mod bit_depth;
pub mod bytestream;
pub mod colorspace;
pub mod options;
pub mod result;
mod serde;
