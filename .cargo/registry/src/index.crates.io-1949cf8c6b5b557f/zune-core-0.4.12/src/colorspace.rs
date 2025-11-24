/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Image Colorspace information and manipulation utilities.

/// All possible image colorspaces
/// Some of them aren't yet supported exist here.
#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ColorSpace {
    /// Red, Green , Blue
    RGB,
    /// Red, Green, Blue, Alpha
    RGBA,
    /// YUV colorspace
    YCbCr,
    /// Grayscale colorspace
    Luma,
    /// Grayscale with alpha colorspace
    LumaA,
    YCCK,
    /// Cyan , Magenta, Yellow, Black
    CMYK,
    /// Blue, Green, Red
    BGR,
    /// Blue, Green, Red, Alpha
    BGRA,
    /// The colorspace is unknown
    Unknown,
    /// Alpha Red Green Blue
    ARGB,
    /// Hue,Saturation,Lightness
    /// Conversion from RGB to HSL and back matches that of Python [colorsys](https://docs.python.org/3/library/colorsys.html) module
    /// Color type is expected to be in floating point
    HSL,
    /// Hue, Saturation,Value
    ///
    /// Conversion from RGB to HSV and back matches that of Python [colorsys](https://docs.python.org/3/library/colorsys.html) module
    /// Color type is expected to be in floating point
    HSV
}

impl ColorSpace {
    /// Number of color channels present for a certain colorspace
    ///
    /// E.g. RGB returns 3 since it contains R,G and B colors to make up a pixel
    pub const fn num_components(&self) -> usize {
        match self {
            Self::RGB | Self::YCbCr | Self::BGR | Self::HSV | Self::HSL => 3,
            Self::RGBA | Self::YCCK | Self::CMYK | Self::BGRA | Self::ARGB => 4,
            Self::Luma => 1,
            Self::LumaA => 2,
            Self::Unknown => 0
        }
    }

    pub const fn has_alpha(&self) -> bool {
        matches!(self, Self::RGBA | Self::LumaA | Self::BGRA | Self::ARGB)
    }

    pub const fn is_grayscale(&self) -> bool {
        matches!(self, Self::LumaA | Self::Luma)
    }

    /// Returns the position of the alpha pixel in a pixel
    ///
    ///
    /// That is for an array of color components say `[0,1,2,3]` if the image has an alpha channel
    /// and is in RGBA format, this will return `Some(3)`, indicating alpha is found in the third index
    /// but if the image is in `ARGB` format, it will return `Some(0)` indicating alpha is found in  
    /// index 0
    ///
    /// If an image doesn't have an alpha channel returns `None`
    ///
    pub const fn alpha_position(&self) -> Option<usize> {
        match self {
            ColorSpace::RGBA => Some(3),
            ColorSpace::LumaA => Some(1),
            ColorSpace::BGRA => Some(3),
            ColorSpace::ARGB => Some(0),
            _ => None
        }
    }
}

/// Encapsulates all colorspaces supported by
/// the library
pub static ALL_COLORSPACES: [ColorSpace; 12] = [
    ColorSpace::RGB,
    ColorSpace::RGBA,
    ColorSpace::LumaA,
    ColorSpace::Luma,
    ColorSpace::CMYK,
    ColorSpace::BGRA,
    ColorSpace::BGR,
    ColorSpace::YCCK,
    ColorSpace::YCbCr,
    ColorSpace::ARGB,
    ColorSpace::HSL,
    ColorSpace::HSV
];

/// Color characteristics
///
/// Gives more information about values in a certain
/// colorspace
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ColorCharacteristics {
    /// Normal default gamma setting
    /// The float contains gamma present
    ///
    /// The default gamma value is 2.2 but for
    /// decoders that allow specifying gamma values,e.g PNG,
    /// the gamma value becomes the specified value by the decoder
    sRGB,
    /// Linear transfer characteristics
    /// The image is in linear colorspace
    Linear
}
