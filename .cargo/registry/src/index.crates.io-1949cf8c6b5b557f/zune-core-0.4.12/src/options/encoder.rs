/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use crate::bit_depth::BitDepth;
use crate::colorspace::ColorSpace;

/// Encoder options that are flags
#[derive(Copy, Debug, Clone, Default)]
struct EncoderFlags {
    /// Whether JPEG images should be encoded as progressive images
    jpeg_encode_progressive: bool,
    /// Whether JPEG images should use optimized huffman tables
    jpeg_optimize_huffman:   bool,
    /// Whether to not preserve metadata across image transformations
    image_strip_metadata:    bool
}

/// Options shared by some of the encoders in
/// the `zune-` family of image crates
#[derive(Debug, Copy, Clone)]
pub struct EncoderOptions {
    width:       usize,
    height:      usize,
    colorspace:  ColorSpace,
    quality:     u8,
    depth:       BitDepth,
    num_threads: u8,
    effort:      u8,
    flags:       EncoderFlags
}

impl Default for EncoderOptions {
    fn default() -> Self {
        Self {
            width:       0,
            height:      0,
            colorspace:  ColorSpace::RGB,
            quality:     80,
            depth:       BitDepth::Eight,
            num_threads: 4,
            effort:      4,
            flags:       EncoderFlags::default()
        }
    }
}

impl EncoderOptions {
    ///  Create  new encode options
    ///
    /// # Arguments
    ///  
    /// * `width`: Image width
    /// * `height`: Image height
    /// * `colorspace`:  Image colorspaces
    /// * `depth`: Image depth
    ///
    /// returns: EncoderOptions
    ///
    pub fn new(
        width: usize, height: usize, colorspace: ColorSpace, depth: BitDepth
    ) -> EncoderOptions {
        EncoderOptions {
            width,
            height,
            colorspace,
            depth,
            ..Default::default()
        }
    }
    /// Get the width for which the image will be encoded in
    pub const fn get_width(&self) -> usize {
        self.width
    }

    /// Get height for which the image will be encoded in
    ///
    /// returns: usize
    ///
    /// # Panics
    /// If height is zero
    pub fn get_height(&self) -> usize {
        assert_ne!(self.height, 0);
        self.height
    }
    /// Get the depth for which the image will be encoded in
    pub const fn get_depth(&self) -> BitDepth {
        self.depth
    }
    /// Get the quality for which the image will be encoded with
    ///
    ///  # Lossy
    /// - Higher quality means some images take longer to write and
    /// are big but they look good
    ///
    /// - Lower quality means small images and low quality.
    ///
    /// # Lossless
    /// - High quality indicates more time is spent in making the file
    /// smaller
    ///
    /// - Low quality indicates less time is spent in making the file bigger
    pub const fn get_quality(&self) -> u8 {
        self.quality
    }
    /// Get the colorspace for which the image will be encoded in
    pub const fn get_colorspace(&self) -> ColorSpace {
        self.colorspace
    }
    pub const fn get_effort(&self) -> u8 {
        self.effort
    }

    /// Set width for the image to be encoded
    pub fn set_width(mut self, width: usize) -> Self {
        self.width = width;
        self
    }

    /// Set height for the image to be encoded
    pub fn set_height(mut self, height: usize) -> Self {
        self.height = height;
        self
    }
    /// Set depth for the image to be encoded
    pub fn set_depth(mut self, depth: BitDepth) -> Self {
        self.depth = depth;
        self
    }
    /// Set quality of the image to be encoded
    ///
    /// Quality is clamped from 0..100
    ///
    /// Quality means different options depending on the encoder, see
    /// [get_quality](Self::get_quality)
    pub fn set_quality(mut self, quality: u8) -> Self {
        self.quality = quality.clamp(0, 100);
        self
    }
    /// Set colorspace for the image to be encoded
    pub fn set_colorspace(mut self, colorspace: ColorSpace) -> Self {
        self.colorspace = colorspace;
        self
    }
    /// Set the number of threads allowed for multithreaded encoding
    /// where supported
    ///
    /// Zero means use a single thread
    pub fn set_num_threads(mut self, threads: u8) -> Self {
        self.num_threads = threads;

        self
    }
    pub fn set_effort(mut self, effort: u8) -> Self {
        self.effort = effort;
        self
    }

    /// Return number of threads configured for multithreading
    /// where possible
    ///
    /// This is used for multi-threaded encoders,
    /// currently only jpeg-xl
    pub const fn get_num_threads(&self) -> u8 {
        self.num_threads
    }

    /// Set whether the encoder should remove metadata from the image
    ///
    /// When set to `true`, supported encoders will strip away metadata
    /// from the resulting image. If set to false, where supported, encoders
    /// will not remove metadata from images
    pub fn set_strip_metadata(mut self, yes: bool) -> Self {
        self.flags.image_strip_metadata = yes;
        self
    }
    /// Whether or not the encoder should remove metadata from the image
    ///
    /// The default value is false, and encoders that respect this try to preserve as much
    /// data as possible from one image to another
    pub const fn strip_metadata(&self) -> bool {
        !self.flags.image_strip_metadata
    }
}

/// JPEG options
impl EncoderOptions {
    /// Whether the jpeg encoder should encode the image in progressive mode
    ///
    /// Default is `false`.
    ///
    /// This may be used to create slightly smaller images at the cost of more processing
    /// time
    pub const fn jpeg_encode_progressive(&self) -> bool {
        self.flags.jpeg_encode_progressive
    }

    /// Whether the jpeg encoder should optimize huffman tables to create smaller files
    /// at the cost of processing time
    ///
    /// Default is `false`.
    pub const fn jpeg_optimized_huffman_tables(&self) -> bool {
        self.flags.jpeg_optimize_huffman
    }

    /// Set whether the jpeg encoder should encode the imagei in progressive mode
    ///
    /// Default is `false`
    pub fn set_jpeg_encode_progressive(mut self, yes: bool) -> Self {
        self.flags.jpeg_optimize_huffman = yes;
        self
    }
}
