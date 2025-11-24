/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Global Decoder options
#![allow(clippy::zero_prefixed_literal)]

use crate::bit_depth::ByteEndian;
use crate::colorspace::ColorSpace;

fn decoder_strict_mode() -> DecoderFlags {
    DecoderFlags {
        inflate_confirm_adler:        true,
        png_confirm_crc:              true,
        jpg_error_on_non_conformance: true,

        zune_use_unsafe:           true,
        zune_use_neon:             true,
        zune_use_avx:              true,
        zune_use_avx2:             true,
        zune_use_sse2:             true,
        zune_use_sse3:             true,
        zune_use_sse41:            true,
        png_add_alpha_channel:     false,
        png_strip_16_bit_to_8_bit: false,
        png_decode_animated:       true,
        jxl_decode_animated:       true
    }
}

/// Fast decoder options
///
/// Enables all intrinsics + unsafe routines
///
/// Disables png adler and crc checking.
fn fast_options() -> DecoderFlags {
    DecoderFlags {
        inflate_confirm_adler:        false,
        png_confirm_crc:              false,
        jpg_error_on_non_conformance: false,

        zune_use_unsafe: true,
        zune_use_neon:   true,
        zune_use_avx:    true,
        zune_use_avx2:   true,
        zune_use_sse2:   true,
        zune_use_sse3:   true,
        zune_use_sse41:  true,

        png_add_alpha_channel:     false,
        png_strip_16_bit_to_8_bit: false,
        png_decode_animated:       true,
        jxl_decode_animated:       true
    }
}

/// Command line options error resilient and fast
///
/// Features
/// - Ignore CRC and Adler in png
/// - Do not error out on non-conformance in jpg
/// - Use unsafe paths
fn cmd_options() -> DecoderFlags {
    DecoderFlags {
        inflate_confirm_adler:        false,
        png_confirm_crc:              false,
        jpg_error_on_non_conformance: false,

        zune_use_unsafe: true,
        zune_use_neon:   true,
        zune_use_avx:    true,
        zune_use_avx2:   true,
        zune_use_sse2:   true,
        zune_use_sse3:   true,
        zune_use_sse41:  true,

        png_add_alpha_channel:     false,
        png_strip_16_bit_to_8_bit: false,

        png_decode_animated: true,
        jxl_decode_animated: true
    }
}

/// Decoder options that are flags
///
/// NOTE: When you extend this, add true or false to
/// all options above that return a `DecoderFlag`
#[derive(Copy, Debug, Clone, Default)]
pub struct DecoderFlags {
    /// Whether the decoder should confirm and report adler mismatch
    inflate_confirm_adler:        bool,
    /// Whether the PNG decoder should confirm crc
    png_confirm_crc:              bool,
    /// Whether the png decoder should error out on image non-conformance
    jpg_error_on_non_conformance: bool,
    /// Whether the decoder should use unsafe  platform specific intrinsics
    ///
    /// This will also shut down platform specific intrinsics `(ZUNE_USE_{EXT})` value
    zune_use_unsafe:              bool,
    /// Whether we should use SSE2.
    ///
    /// This should be enabled for all x64 platforms but can be turned off if
    /// `ZUNE_USE_UNSAFE` is false
    zune_use_sse2:                bool,
    /// Whether we should use SSE3 instructions where possible.
    zune_use_sse3:                bool,
    /// Whether we should use sse4.1 instructions where possible.
    zune_use_sse41:               bool,
    /// Whether we should use avx instructions where possible.
    zune_use_avx:                 bool,
    /// Whether we should use avx2 instructions where possible.
    zune_use_avx2:                bool,
    /// Whether the png decoder should add alpha channel where possible.
    png_add_alpha_channel:        bool,
    /// Whether we should use neon instructions where possible.
    zune_use_neon:                bool,
    /// Whether the png decoder should strip 16 bit to 8 bit
    png_strip_16_bit_to_8_bit:    bool,
    /// Decode all frames for an animated images
    png_decode_animated:          bool,
    jxl_decode_animated:          bool
}

/// Decoder options
///
/// Not all options are respected by decoders all decoders
#[derive(Debug, Copy, Clone)]
pub struct DecoderOptions {
    /// Maximum width for which decoders will
    /// not try to decode images larger than
    /// the specified width.
    ///
    /// - Default value: 16384
    /// - Respected by: `all decoders`
    max_width:      usize,
    /// Maximum height for which decoders will not
    /// try to decode images larger than the
    /// specified height
    ///
    /// - Default value: 16384
    /// - Respected by: `all decoders`
    max_height:     usize,
    /// Output colorspace
    ///
    /// The jpeg decoder allows conversion to a separate colorspace
    /// than the input.
    ///
    /// I.e you can convert a RGB jpeg image to grayscale without
    /// first decoding it to RGB to get
    ///
    /// - Default value: `ColorSpace::RGB`
    /// - Respected by: `jpeg`
    out_colorspace: ColorSpace,

    /// Maximum number of scans allowed
    /// for progressive jpeg images
    ///
    /// Progressive jpegs have scans
    ///
    /// - Default value:100
    /// - Respected by: `jpeg`
    max_scans:     usize,
    /// Maximum size for deflate.
    /// Respected by all decoders that use inflate/deflate
    deflate_limit: usize,
    /// Boolean flags that influence decoding
    flags:         DecoderFlags,
    /// The byte endian of the returned bytes will be stored in
    /// in case a single pixel spans more than a byte
    endianness:    ByteEndian
}

/// Initializers
impl DecoderOptions {
    /// Create the decoder with options  setting most configurable
    /// options to be their safe counterparts
    ///
    /// This is the same as `default` option as default initializes
    /// options to the  safe variant.
    ///
    /// Note, decoders running on this will be slower as it disables
    /// platform specific intrinsics
    pub fn new_safe() -> DecoderOptions {
        DecoderOptions::default()
    }

    /// Create the decoder with options setting the configurable options
    /// to the fast  counterparts
    ///
    /// This enables platform specific code paths and enable use of unsafe
    pub fn new_fast() -> DecoderOptions {
        let flag = fast_options();
        DecoderOptions::default().set_decoder_flags(flag)
    }

    /// Create the decoder options with the following characteristics
    ///
    /// - Use unsafe paths.
    /// - Ignore error checksuming, e.g in png we do not confirm adler and crc in this mode
    /// - Enable fast intrinsics paths
    pub fn new_cmd() -> DecoderOptions {
        let flag = cmd_options();
        DecoderOptions::default().set_decoder_flags(flag)
    }
}

/// Global options respected by all decoders
impl DecoderOptions {
    /// Get maximum width configured for which the decoder
    /// should not try to decode images greater than this width
    pub const fn get_max_width(&self) -> usize {
        self.max_width
    }

    /// Get maximum height configured for which the decoder should
    /// not try to decode images greater than this height
    pub const fn get_max_height(&self) -> usize {
        self.max_height
    }

    /// Return true whether the decoder should be in strict mode
    /// And reject most errors
    pub fn get_strict_mode(&self) -> bool {
        self.flags.jpg_error_on_non_conformance
            | self.flags.png_confirm_crc
            | self.flags.inflate_confirm_adler
    }
    /// Return true if the decoder should use unsafe
    /// routines where possible
    pub const fn get_use_unsafe(&self) -> bool {
        self.flags.zune_use_unsafe
    }

    /// Set maximum width for which the decoder should not try
    /// decoding images greater than that width
    ///
    /// # Arguments
    ///
    /// * `width`:  The maximum width allowed
    ///
    /// returns: DecoderOptions
    pub fn set_max_width(mut self, width: usize) -> Self {
        self.max_width = width;
        self
    }

    /// Set maximum height for which the decoder should not try
    /// decoding images greater than that height
    /// # Arguments
    ///
    /// * `height`: The maximum height allowed
    ///
    /// returns: DecoderOptions
    ///
    pub fn set_max_height(mut self, height: usize) -> Self {
        self.max_height = height;
        self
    }

    /// Whether the routines can use unsafe platform specific
    /// intrinsics when necessary
    ///
    /// Platform intrinsics are implemented for operations which
    /// the compiler can't auto-vectorize, or we can do a marginably
    /// better job at it
    ///
    /// All decoders with unsafe routines respect it.
    ///
    /// Treat this with caution, disabling it will cause slowdowns but
    /// it's provided for mainly for debugging use.
    ///
    /// - Respected by: `png` and `jpeg`(decoders with unsafe routines)
    pub fn set_use_unsafe(mut self, yes: bool) -> Self {
        // first clear the flag
        self.flags.zune_use_unsafe = yes;
        self
    }

    fn set_decoder_flags(mut self, flags: DecoderFlags) -> Self {
        self.flags = flags;
        self
    }
    /// Set whether the decoder should be in standards conforming/
    /// strict mode
    ///
    /// This reduces the error tolerance level for the decoders and invalid
    /// samples will be rejected by the decoder
    ///
    /// # Arguments
    ///
    /// * `yes`:
    ///
    /// returns: DecoderOptions
    ///
    pub fn set_strict_mode(mut self, yes: bool) -> Self {
        self.flags.jpg_error_on_non_conformance = yes;
        self.flags.png_confirm_crc = yes;
        self.flags.inflate_confirm_adler = yes;
        self
    }

    /// Set the byte endian for which raw samples will be stored in
    /// in case a single pixel sample spans more than a byte.
    ///
    /// The default is usually native endian hence big endian values
    /// will be converted to little endian on little endian systems,
    ///
    /// and little endian values will be converted to big endian on big endian systems
    ///
    /// # Arguments
    ///
    /// * `endian`: The endianness to which to set the bytes to
    ///
    /// returns: DecoderOptions
    pub fn set_byte_endian(mut self, endian: ByteEndian) -> Self {
        self.endianness = endian;
        self
    }

    /// Get the byte endian for which samples that span more than one byte will
    /// be treated
    pub const fn get_byte_endian(&self) -> ByteEndian {
        self.endianness
    }
}

/// PNG specific options
impl DecoderOptions {
    /// Whether the inflate decoder should confirm
    /// adler checksums
    pub const fn inflate_get_confirm_adler(&self) -> bool {
        self.flags.inflate_confirm_adler
    }
    /// Set whether the inflate decoder should confirm
    /// adler checksums
    pub fn inflate_set_confirm_adler(mut self, yes: bool) -> Self {
        self.flags.inflate_confirm_adler = yes;
        self
    }
    /// Get default inflate limit for which the decoder
    /// will not try to decompress further
    pub const fn inflate_get_limit(&self) -> usize {
        self.deflate_limit
    }
    /// Set the default inflate limit for which decompressors
    /// relying on inflate won't surpass this limit
    #[must_use]
    pub fn inflate_set_limit(mut self, limit: usize) -> Self {
        self.deflate_limit = limit;
        self
    }
    /// Whether the inflate decoder should confirm
    /// crc 32 checksums
    pub const fn png_get_confirm_crc(&self) -> bool {
        self.flags.png_confirm_crc
    }
    /// Set whether the png decoder should confirm
    /// CRC 32 checksums
    #[must_use]
    pub fn png_set_confirm_crc(mut self, yes: bool) -> Self {
        self.flags.png_confirm_crc = yes;
        self
    }
    /// Set whether the png decoder should add an alpha channel to
    /// images where possible.
    ///
    /// For Luma images, it converts it to Luma+Alpha
    ///
    /// For RGB images it converts it to RGB+Alpha
    pub fn png_set_add_alpha_channel(mut self, yes: bool) -> Self {
        self.flags.png_add_alpha_channel = yes;
        self
    }
    /// Return true whether the png decoder should add an alpha
    /// channel to images where possible
    pub const fn png_get_add_alpha_channel(&self) -> bool {
        self.flags.png_add_alpha_channel
    }

    /// Whether the png decoder should reduce 16 bit images to 8 bit
    /// images implicitly.
    ///
    /// Equivalent to [png::Transformations::STRIP_16](https://docs.rs/png/latest/png/struct.Transformations.html#associatedconstant.STRIP_16)
    pub fn png_set_strip_to_8bit(mut self, yes: bool) -> Self {
        self.flags.png_strip_16_bit_to_8_bit = yes;
        self
    }

    /// Return a boolean indicating whether the png decoder should reduce
    /// 16 bit images to 8 bit images implicitly
    pub const fn png_get_strip_to_8bit(&self) -> bool {
        self.flags.png_strip_16_bit_to_8_bit
    }

    /// Return whether `zune-image` should decode animated images or
    /// whether we should just decode the first frame only
    pub const fn png_decode_animated(&self) -> bool {
        self.flags.png_decode_animated
    }
    /// Set  whether `zune-image` should decode animated images or
    /// whether we should just decode the first frame only
    pub const fn png_set_decode_animated(mut self, yes: bool) -> Self {
        self.flags.png_decode_animated = yes;
        self
    }
}

/// JPEG specific options
impl DecoderOptions {
    /// Get maximum scans for which the jpeg decoder
    /// should not go above for progressive images
    pub const fn jpeg_get_max_scans(&self) -> usize {
        self.max_scans
    }

    /// Set maximum scans for which the jpeg decoder should
    /// not exceed when reconstructing images.
    pub fn jpeg_set_max_scans(mut self, max_scans: usize) -> Self {
        self.max_scans = max_scans;
        self
    }
    /// Get expected output colorspace set by the user for which the image
    /// is expected to be reconstructed into.
    ///
    /// This may be different from the
    pub const fn jpeg_get_out_colorspace(&self) -> ColorSpace {
        self.out_colorspace
    }
    /// Set expected colorspace for which the jpeg output is expected to be in
    ///
    /// This is mainly provided as is, we do not guarantee the decoder can convert to all colorspaces
    /// and the decoder can change it internally when it sees fit.
    #[must_use]
    pub fn jpeg_set_out_colorspace(mut self, colorspace: ColorSpace) -> Self {
        self.out_colorspace = colorspace;
        self
    }
}

/// Intrinsics support
///
/// These routines are compiled depending
/// on the platform they are used, if compiled for a platform
/// it doesn't support,(e.g avx2 on Arm), it will always return `false`
impl DecoderOptions {
    /// Use SSE 2 code paths where possible
    ///
    /// This checks for existence of SSE2 first and returns
    /// false if it's not present
    #[allow(unreachable_code)]
    pub fn use_sse2(&self) -> bool {
        let opt = self.flags.zune_use_sse2 | self.flags.zune_use_unsafe;
        // options says no
        if !opt {
            return false;
        }

        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        {
            // where we can do runtime check if feature is present
            #[cfg(feature = "std")]
            {
                if is_x86_feature_detected!("sse2") {
                    return true;
                }
            }
            // where we can't do runtime check if feature is present
            // check if the compile feature had it enabled
            #[cfg(all(not(feature = "std"), target_feature = "sse2"))]
            {
                return true;
            }
        }
        // everything failed return false
        false
    }

    /// Use SSE 3 paths where possible
    ///
    ///
    /// This also checks for SSE3 support and returns false if
    /// it's not present
    #[allow(unreachable_code)]
    pub fn use_sse3(&self) -> bool {
        let opt = self.flags.zune_use_sse3 | self.flags.zune_use_unsafe;
        // options says no
        if !opt {
            return false;
        }

        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        {
            // where we can do runtime check if feature is present
            #[cfg(feature = "std")]
            {
                if is_x86_feature_detected!("sse3") {
                    return true;
                }
            }
            // where we can't do runtime check if feature is present
            // check if the compile feature had it enabled
            #[cfg(all(not(feature = "std"), target_feature = "sse3"))]
            {
                return true;
            }
        }
        // everything failed return false
        false
    }

    /// Use SSE4 paths where possible
    ///
    /// This also checks for sse 4.1 support and returns false if it
    /// is not present
    #[allow(unreachable_code)]
    pub fn use_sse41(&self) -> bool {
        let opt = self.flags.zune_use_sse41 | self.flags.zune_use_unsafe;
        // options says no
        if !opt {
            return false;
        }

        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        {
            // where we can do runtime check if feature is present
            #[cfg(feature = "std")]
            {
                if is_x86_feature_detected!("sse4.1") {
                    return true;
                }
            }
            // where we can't do runtime check if feature is present
            // check if the compile feature had it enabled
            #[cfg(all(not(feature = "std"), target_feature = "sse4.1"))]
            {
                return true;
            }
        }
        // everything failed return false
        false
    }

    /// Use AVX paths where possible
    ///
    /// This also checks for AVX support and returns false if it's
    /// not present
    #[allow(unreachable_code)]
    pub fn use_avx(&self) -> bool {
        let opt = self.flags.zune_use_avx | self.flags.zune_use_unsafe;
        // options says no
        if !opt {
            return false;
        }

        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        {
            // where we can do runtime check if feature is present
            #[cfg(feature = "std")]
            {
                if is_x86_feature_detected!("avx") {
                    return true;
                }
            }
            // where we can't do runitme check if feature is present
            // check if the compile feature had it enabled
            #[cfg(all(not(feature = "std"), target_feature = "avx"))]
            {
                return true;
            }
        }
        // everything failed return false
        false
    }

    /// Use avx2 paths where possible
    ///
    /// This also checks for AVX2 support and returns false if it's not
    /// present
    #[allow(unreachable_code)]
    pub fn use_avx2(&self) -> bool {
        let opt = self.flags.zune_use_avx2 | self.flags.zune_use_unsafe;
        // options says no
        if !opt {
            return false;
        }

        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        {
            // where we can do runtime check if feature is present
            #[cfg(feature = "std")]
            {
                if is_x86_feature_detected!("avx2") {
                    return true;
                }
            }
            // where we can't do runitme check if feature is present
            // check if the compile feature had it enabled
            #[cfg(all(not(feature = "std"), target_feature = "avx2"))]
            {
                return true;
            }
        }
        // everything failed return false
        false
    }

    #[allow(unreachable_code)]
    pub fn use_neon(&self) -> bool {
        let opt = self.flags.zune_use_neon | self.flags.zune_use_unsafe;
        // options says no
        if !opt {
            return false;
        }

        #[cfg(target_arch = "aarch64")]
        {
            // aarch64 implies neon on a compliant cpu
            // but for real prod should do something better here
            return true;
        }
        // everything failed return false
        false
    }
}

/// JPEG_XL specific options
impl DecoderOptions {
    /// Return whether `zune-image` should decode animated images or
    /// whether we should just decode the first frame only
    pub const fn jxl_decode_animated(&self) -> bool {
        self.flags.jxl_decode_animated
    }
    /// Set  whether `zune-image` should decode animated images or
    /// whether we should just decode the first frame only
    pub const fn jxl_set_decode_animated(mut self, yes: bool) -> Self {
        self.flags.jxl_decode_animated = yes;
        self
    }
}
impl Default for DecoderOptions {
    fn default() -> Self {
        Self {
            out_colorspace: ColorSpace::RGB,
            max_width:      1 << 14,
            max_height:     1 << 14,
            max_scans:      100,
            deflate_limit:  1 << 30,
            flags:          decoder_strict_mode(),
            endianness:     ByteEndian::BE
        }
    }
}
