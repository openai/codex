//! # Overview
//!
//! This crate provides native rust implementations of image encoding and decoding as well as some
//! basic image manipulation functions. Additional documentation can currently also be found in the
//! [README.md file which is most easily viewed on
//! github](https://github.com/image-rs/image/blob/main/README.md).
//!
//! There are two core problems for which this library provides solutions: a unified interface for image
//! encodings and simple generic buffers for their content. It's possible to use either feature
//! without the other. The focus is on a small and stable set of common operations that can be
//! supplemented by other specialized crates. The library also prefers safe solutions with few
//! dependencies.
//!
//! # High level API
//!
//! Load images using [`ImageReader`](crate::ImageReader):
//!
//! ```rust,no_run
//! use std::io::Cursor;
//! use image::ImageReader;
//! # fn main() -> Result<(), image::ImageError> {
//! # let bytes = vec![0u8];
//!
//! let img = ImageReader::open("myimage.png")?.decode()?;
//! let img2 = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?.decode()?;
//! # Ok(())
//! # }
//! ```
//!
//! And save them using [`save`] or [`write_to`] methods:
//!
//! ```rust,no_run
//! # use std::io::{Write, Cursor};
//! # use image::{DynamicImage, ImageFormat};
//! # #[cfg(feature = "png")]
//! # fn main() -> Result<(), image::ImageError> {
//! # let img: DynamicImage = unimplemented!();
//! # let img2: DynamicImage = unimplemented!();
//! img.save("empty.jpg")?;
//!
//! let mut bytes: Vec<u8> = Vec::new();
//! img2.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)?;
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "png"))] fn main() {}
//! ```
//!
//! With default features, the crate includes support for [many common image formats](codecs/index.html#supported-formats).
//!
//! [`save`]: enum.DynamicImage.html#method.save
//! [`write_to`]: enum.DynamicImage.html#method.write_to
//! [`ImageReader`]: struct.Reader.html
//!
//! # Image buffers
//!
//! The two main types for storing images:
//! * [`ImageBuffer`] which holds statically typed image contents.
//! * [`DynamicImage`] which is an enum over the supported `ImageBuffer` formats
//!   and supports conversions between them.
//!
//! As well as a few more specialized options:
//! * [`GenericImage`] trait for a mutable image buffer.
//! * [`GenericImageView`] trait for read only references to a `GenericImage`.
//! * [`flat`] module containing types for interoperability with generic channel
//!   matrices and foreign interfaces.
//!
//! [`GenericImageView`]: trait.GenericImageView.html
//! [`GenericImage`]: trait.GenericImage.html
//! [`ImageBuffer`]: struct.ImageBuffer.html
//! [`DynamicImage`]: enum.DynamicImage.html
//! [`flat`]: flat/index.html
//!
//! # Low level encoding/decoding API
//!
//! Implementations of [`ImageEncoder`] provides low level control over encoding:
//! ```rust,no_run
//! # use std::io::Write;
//! # use image::DynamicImage;
//! # use image::ImageEncoder;
//! # #[cfg(feature = "jpeg")]
//! # fn main() -> Result<(), image::ImageError> {
//! # use image::codecs::jpeg::JpegEncoder;
//! # let img: DynamicImage = unimplemented!();
//! # let writer: Box<dyn Write> = unimplemented!();
//! let encoder = JpegEncoder::new_with_quality(&mut writer, 95);
//! img.write_with_encoder(encoder)?;
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "jpeg"))] fn main() {}
//! ```
//! While [`ImageDecoder`] and [`ImageDecoderRect`] give access to more advanced decoding options:
//!
//! ```rust,no_run
//! # use std::io::{BufReader, Cursor};
//! # use image::DynamicImage;
//! # use image::ImageDecoder;
//! # #[cfg(feature = "png")]
//! # fn main() -> Result<(), image::ImageError> {
//! # use image::codecs::png::PngDecoder;
//! # let img: DynamicImage = unimplemented!();
//! # let reader: BufReader<Cursor<&[u8]>> = unimplemented!();
//! let decoder = PngDecoder::new(&mut reader)?;
//! let icc = decoder.icc_profile();
//! let img = DynamicImage::from_decoder(decoder)?;
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "png"))] fn main() {}
//! ```
//!
//! [`DynamicImage::from_decoder`]: enum.DynamicImage.html#method.from_decoder
//! [`ImageDecoderRect`]: trait.ImageDecoderRect.html
//! [`ImageDecoder`]: trait.ImageDecoder.html
//! [`ImageEncoder`]: trait.ImageEncoder.html
#![warn(missing_docs)]
#![warn(unused_qualifications)]
#![deny(unreachable_pub)]
#![deny(deprecated)]
#![deny(missing_copy_implementations)]
#![cfg_attr(all(test, feature = "benchmarks"), feature(test))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
// We've temporarily disabled PCX support for 0.25.5 release
// by removing the corresponding feature.
// We want to ship bug fixes without committing to PCX support.
//
// Cargo shows warnings about code depending on a nonexistent feature
// even to people using the crate as a dependency,
// so we have to suppress those warnings.
#![allow(unexpected_cfgs)]

#[cfg(all(test, feature = "benchmarks"))]
extern crate test;

#[cfg(test)]
#[macro_use]
extern crate quickcheck;

pub use crate::color::{ColorType, ExtendedColorType};

pub use crate::color::{Luma, LumaA, Rgb, Rgba};

pub use crate::error::{ImageError, ImageResult};

pub use crate::images::generic_image::{GenericImage, GenericImageView, Pixels};

pub use crate::images::sub_image::SubImage;

pub use crate::images::buffer::{
    ConvertColorOptions,
    GrayAlphaImage,
    GrayImage,
    // Image types
    ImageBuffer,
    Rgb32FImage,
    RgbImage,
    Rgba32FImage,
    RgbaImage,
};

pub use crate::flat::FlatSamples;

// Traits
pub use crate::traits::{EncodableLayout, Pixel, PixelWithColorType, Primitive};

// Opening and loading images
pub use crate::images::dynimage::{
    image_dimensions, load_from_memory, load_from_memory_with_format, open,
    write_buffer_with_format,
};
pub use crate::io::free_functions::{guess_format, load, save_buffer, save_buffer_with_format};

pub use crate::io::{
    decoder::{AnimationDecoder, ImageDecoder, ImageDecoderRect},
    encoder::ImageEncoder,
    format::ImageFormat,
    image_reader_type::ImageReader,
    limits::{LimitSupport, Limits},
};

pub use crate::images::dynimage::DynamicImage;

pub use crate::animation::{Delay, Frame, Frames};

// More detailed error type
pub mod error;

/// Iterators and other auxiliary structure for the `ImageBuffer` type.
pub mod buffer {
    // Only those not exported at the top-level
    pub use crate::images::buffer::{
        ConvertBuffer, EnumeratePixels, EnumeratePixelsMut, EnumerateRows, EnumerateRowsMut,
        Pixels, PixelsMut, Rows, RowsMut,
    };

    #[cfg(feature = "rayon")]
    pub use crate::images::buffer_par::*;
}

// Math utils
pub mod math;

// Image processing functions
pub mod imageops;

// Buffer representations for ffi.
pub use crate::images::flat;

/// Encoding and decoding for various image file formats.
///
/// # Supported formats
///
/// | Feature | Format   | Notes
/// | ------- | -------- | -----
/// | `avif`  | AVIF     | Decoding requires the `avif-native` feature, uses the libdav1d C library.
/// | `bmp`   | BMP      |
/// | `dds`   | DDS      | Only decoding is supported.
/// | `exr`   | OpenEXR  |
/// | `ff`    | Farbfeld |
/// | `gif`   | GIF      |
/// | `hdr`   | HDR      |
/// | `ico`   | ICO      |
/// | `jpeg`  | JPEG     |
/// | `png`   | PNG      |
/// | `pnm`   | PNM      |
/// | `qoi`   | QOI      |
/// | `tga`   | TGA      |
/// | `tiff`  | TIFF     |
/// | `webp`  | WebP     | Only lossless encoding is currently supported.
///
/// ## A note on format specific features
///
/// One of the main goals of `image` is stability, in runtime but also for programmers. This
/// ensures that performance as well as safety fixes reach a majority of its user base with little
/// effort. Re-exporting all details of its dependencies would run counter to this goal as it
/// linked _all_ major version bumps between them and `image`. As such, we are wary of exposing too
/// many details, or configuration options, that are not shared between different image formats.
///
/// Nevertheless, the advantage of precise control is hard to ignore. We will thus consider
/// _wrappers_, not direct re-exports, in either of the following cases:
///
/// 1. A standard specifies that configuration _x_ is required for decoders/encoders and there
///    exists an essentially canonical way to control it.
/// 2. At least two different implementations agree on some (sub-)set of features in practice.
/// 3. A technical argument including measurements of the performance, space benefits, or otherwise
///    objectively quantified benefits can be made, and the added interface is unlikely to require
///    breaking changes.
///
/// Features that fulfill two or more criteria are preferred.
///
/// Re-exports of dependencies that reach version `1` will be discussed when it happens.
pub mod codecs {
    #[cfg(any(feature = "avif", feature = "avif-native"))]
    pub mod avif;
    #[cfg(feature = "bmp")]
    pub mod bmp;
    #[cfg(feature = "dds")]
    pub mod dds;
    #[cfg(feature = "ff")]
    pub mod farbfeld;
    #[cfg(feature = "gif")]
    pub mod gif;
    #[cfg(feature = "hdr")]
    pub mod hdr;
    #[cfg(feature = "ico")]
    pub mod ico;
    #[cfg(feature = "jpeg")]
    pub mod jpeg;
    #[cfg(feature = "exr")]
    pub mod openexr;
    #[cfg(feature = "pcx")]
    pub mod pcx;
    #[cfg(feature = "png")]
    pub mod png;
    #[cfg(feature = "pnm")]
    pub mod pnm;
    #[cfg(feature = "qoi")]
    pub mod qoi;
    #[cfg(feature = "tga")]
    pub mod tga;
    #[cfg(feature = "tiff")]
    pub mod tiff;
    #[cfg(feature = "webp")]
    pub mod webp;

    #[cfg(feature = "dds")]
    mod dxt;
}

mod animation;
mod color;
pub mod hooks;
mod images;
/// Deprecated io module the original io module has been renamed to `image_reader`.
/// This is going to be internal.
pub mod io;
pub mod metadata;
//TODO delete this module after a few releases
mod traits;
mod utils;

// Can't use the macro-call itself within the `doc` attribute. So force it to eval it as part of
// the macro invocation.
//
// The inspiration for the macro and implementation is from
// <https://github.com/GuillaumeGomez/doc-comment>
//
// MIT License
//
// Copyright (c) 2018 Guillaume Gomez
macro_rules! insert_as_doc {
    { $content:expr } => {
        #[allow(unused_doc_comments)]
        #[doc = $content] extern "Rust" { }
    }
}

// Provides the README.md as doc, to ensure the example works!
insert_as_doc!(include_str!("../README.md"));
