/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::too_many_arguments,
    clippy::doc_markdown
)]

//! Color space conversion routines
//!
//! This files exposes functions to convert one colorspace to another in a jpeg
//! image
//!
//! Currently supported conversions are
//!
//! - `YCbCr` to `RGB,RGBA,GRAYSCALE,RGBX`.
//!
//!
//! Hey there, if your reading this it means you probably need something, so let me help you.
//!
//! There are 3 supported cpu extensions here.
//! 1. Scalar
//! 2. SSE
//! 3. AVX
//!
//! There are two types of the color convert functions
//!
//! 1. Acts on 16 pixels.
//! 2. Acts on 8 pixels.
//!
//! The reason for this is because when implementing the AVX part it occurred to me that we can actually
//! do better and process 2 MCU's if we change IDCT return type to be `i16's`, since a lot of
//! CPU's these days support AVX extensions, it becomes nice if we optimize for that path ,
//! therefore AVX routines can process 16 pixels directly and SSE and Scalar just compensate.
//!
//! By compensating, I mean I wrote the 16 pixels version operating on the 8 pixel version twice.
//!
//! Therefore if your looking to optimize some routines, probably start there.

pub use scalar::ycbcr_to_grayscale;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(feature = "x86")]
pub use crate::color_convert::avx::{ycbcr_to_rgb_avx2, ycbcr_to_rgba_avx2};
use crate::decoder::ColorConvert16Ptr;

mod avx;
mod neon64;
mod scalar;

#[allow(unused_variables)]
pub fn choose_ycbcr_to_rgb_convert_func(
    type_need: ColorSpace, options: &DecoderOptions
) -> Option<ColorConvert16Ptr> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[cfg(feature = "x86")]
    {
        use zune_core::log::debug;
        if options.use_avx2() {
            debug!("Using AVX optimised color conversion functions");

            // I believe avx2 means sse4 is also available
            // match colorspace
            match type_need {
                ColorSpace::RGB => return Some(ycbcr_to_rgb_avx2),
                ColorSpace::RGBA => return Some(ycbcr_to_rgba_avx2),
                _ => () // fall through to scalar, which has more types
            };
        }
    }
    #[cfg(all(feature = "neon", target_arch = "aarch64"))]
    {
        if options.use_neon() {
            use crate::color_convert::neon64::{ycbcr_to_rgb_neon, ycbcr_to_rgba_neon};
            match type_need {
                ColorSpace::RGB => return Some(ycbcr_to_rgb_neon),
                ColorSpace::RGBA => return Some(ycbcr_to_rgba_neon),
                _ => () // fall through to scalar, which has more types
            };
        }
    }
    // when there is no x86 or we haven't returned by here, resort to scalar
    return match type_need {
        ColorSpace::RGB => Some(scalar::ycbcr_to_rgb_inner_16_scalar::<false>),
        ColorSpace::RGBA => Some(scalar::ycbcr_to_rgba_inner_16_scalar::<false>),
        ColorSpace::BGRA => Some(scalar::ycbcr_to_rgba_inner_16_scalar::<true>),
        ColorSpace::BGR => Some(scalar::ycbcr_to_rgb_inner_16_scalar::<true>),
        _ => None
    };
}