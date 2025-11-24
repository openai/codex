/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//!This crate provides a library for decoding valid
//! ITU-T Rec. T.851 (09/2005) ITU-T T.81 (JPEG-1) or JPEG images.
//!
//!
//!
//! # Features
//!  - SSE and AVX accelerated functions to speed up certain decoding operations
//!  - FAST and accurate 32 bit IDCT algorithm
//!  - Fast color convert functions
//!  - RGBA and RGBX (4-Channel) color conversion functions
//!  - YCbCr to Luma(Grayscale) conversion.
//!
//! # Usage
//! Add zune-jpeg to the dependencies in the project Cargo.toml
//!
//! ```toml
//! [dependencies]
//! zune_jpeg = "0.3"
//! ```
//! # Examples
//!
//! ## Decode a JPEG file with default arguments.
//!```no_run
//! use std::fs::read;
//! use zune_jpeg::JpegDecoder;
//! let file_contents = read("a_jpeg.file").unwrap();
//! let mut decoder = JpegDecoder::new(&file_contents);
//! let mut pixels = decoder.decode().unwrap();
//! ```
//!
//! ## Decode a JPEG file to RGBA format
//!
//! - Other (limited) supported formats are and  BGR, BGRA
//!
//!```no_run
//! use zune_core::colorspace::ColorSpace;
//! use zune_core::options::DecoderOptions;
//! use zune_jpeg::JpegDecoder;
//!
//! let mut options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
//!
//! let mut decoder = JpegDecoder::new_with_options(&[],options);
//! let pixels = decoder.decode().unwrap();
//! ```
//!
//! ## Decode an image and get it's width and height.
//!```no_run
//! use zune_jpeg::JpegDecoder;
//!
//! let mut decoder = JpegDecoder::new(&[]);
//! decoder.decode_headers().unwrap();
//! let image_info = decoder.info().unwrap();
//! println!("{},{}",image_info.width,image_info.height)
//! ```
//! # Crate features.
//! This crate tries to be as minimal as possible while being extensible
//! enough to handle the complexities arising from parsing different types
//! of jpeg images.
//!
//! Safety is a top concern that is why we provide both static ways to disable unsafe code,
//! disabling x86 feature, and dynamic ,by using [`DecoderOptions::set_use_unsafe(false)`],
//! both of these disable platform specific optimizations, which reduce the speed of decompression.
//!
//! Please do note that careful consideration has been taken to ensure that the unsafe paths
//! are only unsafe because they depend on platform specific intrinsics, hence no need to disable them
//!
//! The crate tries to decode as many images as possible, as a best effort, even those violating the standard
//! , this means a lot of images may  get silent warnings and wrong output, but if you are sure you will be handling
//! images that follow the spec, set `ZuneJpegOptions::set_strict` to true.
//!
//![`DecoderOptions::set_use_unsafe(false)`]:  https://docs.rs/zune-core/0.2.1/zune_core/options/struct.DecoderOptions.html#method.set_use_unsafe

#![warn(
    clippy::correctness,
    clippy::perf,
    clippy::pedantic,
    clippy::inline_always,
    clippy::missing_errors_doc,
    clippy::panic
)]
#![allow(
    clippy::needless_return,
    clippy::similar_names,
    clippy::inline_always,
    clippy::similar_names,
    clippy::doc_markdown,
    clippy::module_name_repetitions,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc
)]
// no_std compatibility
#![deny(clippy::std_instead_of_alloc, clippy::alloc_instead_of_core)]
#![cfg_attr(not(any(feature = "x86", feature = "neon")), forbid(unsafe_code))]
#![cfg_attr(not(feature = "std"), no_std)]
#![macro_use]
extern crate alloc;
extern crate core;

pub use zune_core;

pub use crate::decoder::{ImageInfo, JpegDecoder};

mod bitstream;
mod color_convert;
mod components;
mod decoder;
pub mod errors;
mod headers;
mod huffman;
#[cfg(not(fuzzing))]
mod idct;
#[cfg(fuzzing)]
pub mod idct;
mod marker;
mod mcu;
mod mcu_prog;
mod misc;
mod unsafe_utils;
mod unsafe_utils_avx2;
mod unsafe_utils_neon;
mod upsampler;
mod worker;
