//! Common types shared between the encoder and decoder
use crate::text_metadata::{ITXtChunk, TEXtChunk, ZTXtChunk};
#[allow(unused_imports)] // used by doc comments only
use crate::Filter;
use crate::{chunk, encoder};
use io::Write;
use std::{borrow::Cow, convert::TryFrom, fmt, io};

/// Describes how a pixel is encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ColorType {
    /// 1 grayscale sample.
    Grayscale = 0,
    /// 1 red sample, 1 green sample, 1 blue sample.
    Rgb = 2,
    /// 1 sample for the palette index.
    Indexed = 3,
    /// 1 grayscale sample, then 1 alpha sample.
    GrayscaleAlpha = 4,
    /// 1 red sample, 1 green sample, 1 blue sample, and finally, 1 alpha sample.
    Rgba = 6,
}

impl ColorType {
    /// Returns the number of samples used per pixel encoded in this way.
    pub fn samples(self) -> usize {
        self.samples_u8().into()
    }

    pub(crate) fn samples_u8(self) -> u8 {
        use self::ColorType::*;
        match self {
            Grayscale | Indexed => 1,
            Rgb => 3,
            GrayscaleAlpha => 2,
            Rgba => 4,
        }
    }

    /// u8 -> Self. Temporary solution until Rust provides a canonical one.
    pub fn from_u8(n: u8) -> Option<ColorType> {
        match n {
            0 => Some(ColorType::Grayscale),
            2 => Some(ColorType::Rgb),
            3 => Some(ColorType::Indexed),
            4 => Some(ColorType::GrayscaleAlpha),
            6 => Some(ColorType::Rgba),
            _ => None,
        }
    }

    pub(crate) fn checked_raw_row_length(self, depth: BitDepth, width: u32) -> Option<usize> {
        // No overflow can occur in 64 bits, we multiply 32-bit with 5 more bits.
        let bits = u64::from(width) * u64::from(self.samples_u8()) * u64::from(depth.into_u8());
        TryFrom::try_from(1 + (bits + 7) / 8).ok()
    }

    pub(crate) fn raw_row_length_from_width(self, depth: BitDepth, width: u32) -> usize {
        let samples = width as usize * self.samples();
        1 + match depth {
            BitDepth::Sixteen => samples * 2,
            BitDepth::Eight => samples,
            subbyte => {
                let samples_per_byte = 8 / subbyte as usize;
                let whole = samples / samples_per_byte;
                let fract = usize::from(samples % samples_per_byte > 0);
                whole + fract
            }
        }
    }

    pub(crate) fn is_combination_invalid(self, bit_depth: BitDepth) -> bool {
        // Section 11.2.2 of the PNG standard disallows several combinations
        // of bit depth and color type
        ((bit_depth == BitDepth::One || bit_depth == BitDepth::Two || bit_depth == BitDepth::Four)
            && (self == ColorType::Rgb
                || self == ColorType::GrayscaleAlpha
                || self == ColorType::Rgba))
            || (bit_depth == BitDepth::Sixteen && self == ColorType::Indexed)
    }

    pub(crate) fn bits_per_pixel(&self, bit_depth: BitDepth) -> usize {
        self.samples() * bit_depth as usize
    }

    pub(crate) fn bytes_per_pixel(&self, bit_depth: BitDepth) -> usize {
        // If adjusting this for expansion or other transformation passes, remember to keep the old
        // implementation for bpp_in_prediction, which is internal to the png specification.
        self.samples() * ((bit_depth as usize + 7) >> 3)
    }
}

/// Bit depth of the PNG file.
/// Specifies the number of bits per sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BitDepth {
    One = 1,
    Two = 2,
    Four = 4,
    Eight = 8,
    Sixteen = 16,
}

/// Internal count of bytes per pixel.
/// This is used for filtering which never uses sub-byte units. This essentially reduces the number
/// of possible byte chunk lengths to a very small set of values appropriate to be defined as an
/// enum.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub(crate) enum BytesPerPixel {
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Six = 6,
    Eight = 8,
}

impl BitDepth {
    /// u8 -> Self. Temporary solution until Rust provides a canonical one.
    pub fn from_u8(n: u8) -> Option<BitDepth> {
        match n {
            1 => Some(BitDepth::One),
            2 => Some(BitDepth::Two),
            4 => Some(BitDepth::Four),
            8 => Some(BitDepth::Eight),
            16 => Some(BitDepth::Sixteen),
            _ => None,
        }
    }

    pub(crate) fn into_u8(self) -> u8 {
        self as u8
    }
}

/// Pixel dimensions information
#[derive(Clone, Copy, Debug)]
pub struct PixelDimensions {
    /// Pixels per unit, X axis
    pub xppu: u32,
    /// Pixels per unit, Y axis
    pub yppu: u32,
    /// Either *Meter* or *Unspecified*
    pub unit: Unit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
/// Physical unit of the pixel dimensions
pub enum Unit {
    Unspecified = 0,
    Meter = 1,
}

impl Unit {
    /// u8 -> Self. Temporary solution until Rust provides a canonical one.
    pub fn from_u8(n: u8) -> Option<Unit> {
        match n {
            0 => Some(Unit::Unspecified),
            1 => Some(Unit::Meter),
            _ => None,
        }
    }
}

/// How to reset buffer of an animated png (APNG) at the end of a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DisposeOp {
    /// Leave the buffer unchanged.
    None = 0,
    /// Clear buffer with the background color.
    Background = 1,
    /// Reset the buffer to the state before the current frame.
    Previous = 2,
}

impl DisposeOp {
    /// u8 -> Self. Using enum_primitive or transmute is probably the right thing but this will do for now.
    pub fn from_u8(n: u8) -> Option<DisposeOp> {
        match n {
            0 => Some(DisposeOp::None),
            1 => Some(DisposeOp::Background),
            2 => Some(DisposeOp::Previous),
            _ => None,
        }
    }
}

impl fmt::Display for DisposeOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match *self {
            DisposeOp::None => "DISPOSE_OP_NONE",
            DisposeOp::Background => "DISPOSE_OP_BACKGROUND",
            DisposeOp::Previous => "DISPOSE_OP_PREVIOUS",
        };
        write!(f, "{}", name)
    }
}

/// How pixels are written into the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BlendOp {
    /// Pixels overwrite the value at their position.
    Source = 0,
    /// The new pixels are blended into the current state based on alpha.
    Over = 1,
}

impl BlendOp {
    /// u8 -> Self. Using enum_primitive or transmute is probably the right thing but this will do for now.
    pub fn from_u8(n: u8) -> Option<BlendOp> {
        match n {
            0 => Some(BlendOp::Source),
            1 => Some(BlendOp::Over),
            _ => None,
        }
    }
}

impl fmt::Display for BlendOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match *self {
            BlendOp::Source => "BLEND_OP_SOURCE",
            BlendOp::Over => "BLEND_OP_OVER",
        };
        write!(f, "{}", name)
    }
}

/// Frame control information
#[derive(Clone, Copy, Debug)]
pub struct FrameControl {
    /// Sequence number of the animation chunk, starting from 0
    pub sequence_number: u32,
    /// Width of the following frame
    pub width: u32,
    /// Height of the following frame
    pub height: u32,
    /// X position at which to render the following frame
    pub x_offset: u32,
    /// Y position at which to render the following frame
    pub y_offset: u32,
    /// Frame delay fraction numerator
    pub delay_num: u16,
    /// Frame delay fraction denominator
    pub delay_den: u16,
    /// Type of frame area disposal to be done after rendering this frame
    pub dispose_op: DisposeOp,
    /// Type of frame area rendering for this frame
    pub blend_op: BlendOp,
}

impl Default for FrameControl {
    fn default() -> FrameControl {
        FrameControl {
            sequence_number: 0,
            width: 0,
            height: 0,
            x_offset: 0,
            y_offset: 0,
            delay_num: 1,
            delay_den: 30,
            dispose_op: DisposeOp::None,
            blend_op: BlendOp::Source,
        }
    }
}

impl FrameControl {
    pub fn set_seq_num(&mut self, s: u32) {
        self.sequence_number = s;
    }

    pub fn inc_seq_num(&mut self, i: u32) {
        self.sequence_number += i;
    }

    pub fn encode<W: Write>(self, w: &mut W) -> encoder::Result<()> {
        let mut data = [0u8; 26];
        data[..4].copy_from_slice(&self.sequence_number.to_be_bytes());
        data[4..8].copy_from_slice(&self.width.to_be_bytes());
        data[8..12].copy_from_slice(&self.height.to_be_bytes());
        data[12..16].copy_from_slice(&self.x_offset.to_be_bytes());
        data[16..20].copy_from_slice(&self.y_offset.to_be_bytes());
        data[20..22].copy_from_slice(&self.delay_num.to_be_bytes());
        data[22..24].copy_from_slice(&self.delay_den.to_be_bytes());
        data[24] = self.dispose_op as u8;
        data[25] = self.blend_op as u8;

        encoder::write_chunk(w, chunk::fcTL, &data)
    }
}

/// Animation control information
#[derive(Clone, Copy, Debug)]
pub struct AnimationControl {
    /// Number of frames
    pub num_frames: u32,
    /// Number of times to loop this APNG.  0 indicates infinite looping.
    pub num_plays: u32,
}

impl AnimationControl {
    pub fn encode<W: Write>(self, w: &mut W) -> encoder::Result<()> {
        let mut data = [0; 8];
        data[..4].copy_from_slice(&self.num_frames.to_be_bytes());
        data[4..].copy_from_slice(&self.num_plays.to_be_bytes());
        encoder::write_chunk(w, chunk::acTL, &data)
    }
}

/// The type and strength of applied compression.
///
/// This is a simple, high-level interface that will automatically choose
/// the appropriate DEFLATE compression mode and PNG filter.
///
/// If you need more control over the encoding parameters,
/// you can set the [DeflateCompression] and [Filter] manually.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum Compression {
    /// No compression whatsoever. Fastest, but results in large files.
    NoCompression,
    /// Extremely fast but light compression.
    ///
    /// Note: When used in streaming mode, this compression level can actually result in files
    /// *larger* than would be produced by `NoCompression` on incompressible data because
    /// it doesn't do any buffering of the output stream to detect whether the data is being compressed or not.
    Fastest,
    /// Extremely fast compression with a decent compression ratio.
    ///
    /// Significantly outperforms libpng and other popular encoders by using a [specialized DEFLATE
    /// implementation tuned for PNG](https://crates.io/crates/fdeflate), while still providing
    /// better compression ratio than the fastest modes of other encoders.
    ///
    /// Like `Compression::Fast` this can currently produce files larger than `NoCompression` in
    /// streaming mode when given incompressible data. This may change in the future.
    Fast,
    /// Balances encoding speed and compression ratio
    Balanced,
    /// Spend much more time to produce a slightly smaller file than with `Balanced`.
    High,
}

impl Default for Compression {
    fn default() -> Self {
        Self::Balanced
    }
}

/// Advanced compression settings with more customization options than [Compression].
///
/// Note that this setting only affects DEFLATE compression.
/// Another setting that influences the compression ratio and lets you choose
/// between encoding speed and compression ratio is the [Filter].
///
/// ### Stability guarantees
///
/// The implementation details of DEFLATE compression may evolve over time,
/// even without a semver-breaking change to the version of `png` crate.
///
/// If a certain compression setting is superseded by other options,
/// it may be marked deprecated and remapped to a different option.
/// You will see a deprecation notice when compiling code relying on such options.
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub enum DeflateCompression {
    /// Do not compress the data at all.
    ///
    /// Useful for incompressible images, or when speed is paramount and you don't care about size
    /// at all.
    ///
    /// This mode also disables filters, forcing [Filter::NoFilter].
    NoCompression,

    /// Excellent for creating lightly compressed PNG images very quickly.
    ///
    /// Uses the [fdeflate](https://crates.io/crates/fdeflate) crate under the hood to achieve
    /// speeds far exceeding what libpng is capable of while still providing a decent compression
    /// ratio.
    ///
    /// Note: When used in streaming mode, this compression level can actually result in files
    /// *larger* than would be produced by `NoCompression` because it doesn't do any buffering of
    /// the output stream to detect whether the data is being compressed or not.
    FdeflateUltraFast,

    /// Compression level between 1 and 9, where higher values mean better compression at the cost of
    /// speed.
    ///
    /// This is currently implemented via [flate2](https://crates.io/crates/flate2) crate
    /// by passing through the [compression level](flate2::Compression::new).
    ///
    /// The implementation details and the exact meaning of each level may change in the future,
    /// including in semver-compatible releases.
    Level(u8),
    // Other variants can be added in the future
}

impl Default for DeflateCompression {
    fn default() -> Self {
        Self::from_simple(Compression::Balanced)
    }
}

impl DeflateCompression {
    pub(crate) fn from_simple(value: Compression) -> Self {
        match value {
            Compression::NoCompression => Self::NoCompression,
            Compression::Fastest => Self::FdeflateUltraFast,
            Compression::Fast => Self::FdeflateUltraFast,
            Compression::Balanced => Self::Level(flate2::Compression::default().level() as u8),
            Compression::High => Self::Level(flate2::Compression::best().level() as u8),
        }
    }
}

/// An unsigned integer scaled version of a floating point value,
/// equivalent to an integer quotient with fixed denominator (100_000)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScaledFloat(u32);

impl ScaledFloat {
    const SCALING: f32 = 100_000.0;

    /// Gets whether the value is within the clamped range of this type.
    pub fn in_range(value: f32) -> bool {
        value >= 0.0 && (value * Self::SCALING).floor() <= u32::MAX as f32
    }

    /// Gets whether the value can be exactly converted in round-trip.
    #[allow(clippy::float_cmp)] // Stupid tool, the exact float compare is _the entire point_.
    pub fn exact(value: f32) -> bool {
        let there = Self::forward(value);
        let back = Self::reverse(there);
        value == back
    }

    fn forward(value: f32) -> u32 {
        (value.max(0.0) * Self::SCALING).floor() as u32
    }

    fn reverse(encoded: u32) -> f32 {
        encoded as f32 / Self::SCALING
    }

    /// Slightly inaccurate scaling and quantization.
    /// Clamps the value into the representable range if it is negative or too large.
    pub fn new(value: f32) -> Self {
        Self(Self::forward(value))
    }

    /// Fully accurate construction from a value scaled as per specification.
    pub fn from_scaled(val: u32) -> Self {
        Self(val)
    }

    /// Get the accurate encoded value.
    pub fn into_scaled(self) -> u32 {
        self.0
    }

    /// Get the unscaled value as a floating point.
    pub fn into_value(self) -> f32 {
        Self::reverse(self.0)
    }

    pub(crate) fn encode_gama<W: Write>(self, w: &mut W) -> encoder::Result<()> {
        encoder::write_chunk(w, chunk::gAMA, &self.into_scaled().to_be_bytes())
    }
}

/// Chromaticities of the color space primaries
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceChromaticities {
    pub white: (ScaledFloat, ScaledFloat),
    pub red: (ScaledFloat, ScaledFloat),
    pub green: (ScaledFloat, ScaledFloat),
    pub blue: (ScaledFloat, ScaledFloat),
}

impl SourceChromaticities {
    pub fn new(white: (f32, f32), red: (f32, f32), green: (f32, f32), blue: (f32, f32)) -> Self {
        SourceChromaticities {
            white: (ScaledFloat::new(white.0), ScaledFloat::new(white.1)),
            red: (ScaledFloat::new(red.0), ScaledFloat::new(red.1)),
            green: (ScaledFloat::new(green.0), ScaledFloat::new(green.1)),
            blue: (ScaledFloat::new(blue.0), ScaledFloat::new(blue.1)),
        }
    }

    #[rustfmt::skip]
    pub fn to_be_bytes(self) -> [u8; 32] {
        let white_x = self.white.0.into_scaled().to_be_bytes();
        let white_y = self.white.1.into_scaled().to_be_bytes();
        let red_x   = self.red.0.into_scaled().to_be_bytes();
        let red_y   = self.red.1.into_scaled().to_be_bytes();
        let green_x = self.green.0.into_scaled().to_be_bytes();
        let green_y = self.green.1.into_scaled().to_be_bytes();
        let blue_x  = self.blue.0.into_scaled().to_be_bytes();
        let blue_y  = self.blue.1.into_scaled().to_be_bytes();
        [
            white_x[0], white_x[1], white_x[2], white_x[3],
            white_y[0], white_y[1], white_y[2], white_y[3],
            red_x[0],   red_x[1],   red_x[2],   red_x[3],
            red_y[0],   red_y[1],   red_y[2],   red_y[3],
            green_x[0], green_x[1], green_x[2], green_x[3],
            green_y[0], green_y[1], green_y[2], green_y[3],
            blue_x[0],  blue_x[1],  blue_x[2],  blue_x[3],
            blue_y[0],  blue_y[1],  blue_y[2],  blue_y[3],
        ]
    }

    pub fn encode<W: Write>(self, w: &mut W) -> encoder::Result<()> {
        encoder::write_chunk(w, chunk::cHRM, &self.to_be_bytes())
    }
}

/// The rendering intent for an sRGB image.
///
/// Presence of this data also indicates that the image conforms to the sRGB color space.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SrgbRenderingIntent {
    /// For images preferring good adaptation to the output device gamut at the expense of colorimetric accuracy, such as photographs.
    Perceptual = 0,
    /// For images requiring colour appearance matching (relative to the output device white point), such as logos.
    RelativeColorimetric = 1,
    /// For images preferring preservation of saturation at the expense of hue and lightness, such as charts and graphs.
    Saturation = 2,
    /// For images requiring preservation of absolute colorimetry, such as previews of images destined for a different output device (proofs).
    AbsoluteColorimetric = 3,
}

impl SrgbRenderingIntent {
    pub(crate) fn into_raw(self) -> u8 {
        self as u8
    }

    pub(crate) fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(SrgbRenderingIntent::Perceptual),
            1 => Some(SrgbRenderingIntent::RelativeColorimetric),
            2 => Some(SrgbRenderingIntent::Saturation),
            3 => Some(SrgbRenderingIntent::AbsoluteColorimetric),
            _ => None,
        }
    }

    pub fn encode<W: Write>(self, w: &mut W) -> encoder::Result<()> {
        encoder::write_chunk(w, chunk::sRGB, &[self.into_raw()])
    }
}

/// Coding-independent code points (cICP) specify the color space (primaries),
/// transfer function, matrix coefficients and scaling factor of the image using
/// the code points specified in [ITU-T-H.273](https://www.itu.int/rec/T-REC-H.273).
///
/// See https://www.w3.org/TR/png-3/#cICP-chunk for more details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodingIndependentCodePoints {
    /// Id number of the color primaries defined in
    /// [ITU-T-H.273](https://www.itu.int/rec/T-REC-H.273) in "Table 2 -
    /// Interpretation of colour primaries (ColourPrimaries) value".
    pub color_primaries: u8,

    /// Id number of the transfer characteristics defined in
    /// [ITU-T-H.273](https://www.itu.int/rec/T-REC-H.273) in "Table 3 -
    /// Interpretation of transfer characteristics (TransferCharacteristics)
    /// value".
    pub transfer_function: u8,

    /// Id number of the matrix coefficients defined in
    /// [ITU-T-H.273](https://www.itu.int/rec/T-REC-H.273) in "Table 4 -
    /// Interpretation of matrix coefficients (MatrixCoefficients) value".
    ///
    /// This field is included to faithfully replicate the base
    /// [ITU-T-H.273](https://www.itu.int/rec/T-REC-H.273) specification, but matrix coefficients
    /// will always be set to 0, because RGB is currently the only supported color mode in PNG.
    pub matrix_coefficients: u8,

    /// Whether the image is
    /// [a full range image](https://www.w3.org/TR/png-3/#dfn-full-range-image)
    /// or
    /// [a narrow range image](https://www.w3.org/TR/png-3/#dfn-narrow-range-image).
    ///
    /// This field is included to faithfully replicate the base
    /// [ITU-T-H.273](https://www.itu.int/rec/T-REC-H.273) specification, but it has limited
    /// practical application to PNG images, because narrow-range images are [quite
    /// rare](https://github.com/w3c/png/issues/312#issuecomment-2327349614) in practice.
    pub is_video_full_range_image: bool,
}

/// Mastering Display Color Volume (mDCV) used at the point of content creation,
/// as specified in [SMPTE-ST-2086](https://ieeexplore.ieee.org/stamp/stamp.jsp?arnumber=8353899).
///
/// See https://www.w3.org/TR/png-3/#mDCV-chunk for more details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MasteringDisplayColorVolume {
    /// Mastering display chromaticities.
    pub chromaticities: SourceChromaticities,

    /// Mastering display maximum luminance.
    ///
    /// The value is expressed in units of 0.0001 cd/m^2 - for example if this field
    /// is set to `10000000` then it indicates 1000 cd/m^2.
    pub max_luminance: u32,

    /// Mastering display minimum luminance.
    ///
    /// The value is expressed in units of 0.0001 cd/m^2 - for example if this field
    /// is set to `10000000` then it indicates 1000 cd/m^2.
    pub min_luminance: u32,
}

/// Content light level information of HDR content.
///
/// See https://www.w3.org/TR/png-3/#cLLI-chunk for more details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContentLightLevelInfo {
    /// Maximum Content Light Level indicates the maximum light level of any
    /// single pixel (in cd/m^2, also known as nits) of the entire playback
    /// sequence.
    ///
    /// The value is expressed in units of 0.0001 cd/m^2 - for example if this field
    /// is set to `10000000` then it indicates 1000 cd/m^2.
    ///
    /// A value of zero means that the value is unknown or not currently calculable.
    pub max_content_light_level: u32,

    /// Maximum Frame Average Light Level indicates the maximum value of the
    /// frame average light level (in cd/m^2, also known as nits) of the entire
    /// playback sequence. It is calculated by first averaging the decoded
    /// luminance values of all the pixels in each frame, and then using the
    /// value for the frame with the highest value.
    ///
    /// The value is expressed in units of 0.0001 cd/m^2 - for example if this field
    /// is set to `10000000` then it indicates 1000 cd/m^2.
    ///
    /// A value of zero means that the value is unknown or not currently calculable.
    pub max_frame_average_light_level: u32,
}

/// PNG info struct
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Info<'a> {
    pub width: u32,
    pub height: u32,
    pub bit_depth: BitDepth,
    /// How colors are stored in the image.
    pub color_type: ColorType,
    pub interlaced: bool,
    /// The image's `sBIT` chunk, if present; contains significant bits of the sample.
    pub sbit: Option<Cow<'a, [u8]>>,
    /// The image's `tRNS` chunk, if present; contains the alpha channel of the image's palette, 1 byte per entry.
    pub trns: Option<Cow<'a, [u8]>>,
    pub pixel_dims: Option<PixelDimensions>,
    /// The image's `PLTE` chunk, if present; contains the RGB channels (in that order) of the image's palettes, 3 bytes per entry (1 per channel).
    pub palette: Option<Cow<'a, [u8]>>,
    /// The contents of the image's gAMA chunk, if present.
    /// Prefer `source_gamma` to also get the derived replacement gamma from sRGB chunks.
    pub gama_chunk: Option<ScaledFloat>,
    /// The contents of the image's `cHRM` chunk, if present.
    /// Prefer `source_chromaticities` to also get the derived replacements from sRGB chunks.
    pub chrm_chunk: Option<SourceChromaticities>,
    /// The contents of the image's `bKGD` chunk, if present.
    pub bkgd: Option<Cow<'a, [u8]>>,

    pub frame_control: Option<FrameControl>,
    pub animation_control: Option<AnimationControl>,
    /// Gamma of the source system.
    /// Set by both `gAMA` as well as to a replacement by `sRGB` chunk.
    pub source_gamma: Option<ScaledFloat>,
    /// Chromaticities of the source system.
    /// Set by both `cHRM` as well as to a replacement by `sRGB` chunk.
    pub source_chromaticities: Option<SourceChromaticities>,
    /// The rendering intent of an SRGB image.
    ///
    /// Presence of this value also indicates that the image conforms to the SRGB color space.
    pub srgb: Option<SrgbRenderingIntent>,
    /// The ICC profile for the image.
    pub icc_profile: Option<Cow<'a, [u8]>>,
    /// The coding-independent code points for video signal type identification of the image.
    pub coding_independent_code_points: Option<CodingIndependentCodePoints>,
    /// The mastering display color volume for the image.
    pub mastering_display_color_volume: Option<MasteringDisplayColorVolume>,
    /// The content light information for the image.
    pub content_light_level: Option<ContentLightLevelInfo>,
    /// The EXIF metadata for the image.
    pub exif_metadata: Option<Cow<'a, [u8]>>,
    /// tEXt field
    pub uncompressed_latin1_text: Vec<TEXtChunk>,
    /// zTXt field
    pub compressed_latin1_text: Vec<ZTXtChunk>,
    /// iTXt field
    pub utf8_text: Vec<ITXtChunk>,
}

impl Default for Info<'_> {
    fn default() -> Info<'static> {
        Info {
            width: 0,
            height: 0,
            bit_depth: BitDepth::Eight,
            color_type: ColorType::Grayscale,
            interlaced: false,
            palette: None,
            sbit: None,
            trns: None,
            gama_chunk: None,
            chrm_chunk: None,
            bkgd: None,
            pixel_dims: None,
            frame_control: None,
            animation_control: None,
            source_gamma: None,
            source_chromaticities: None,
            srgb: None,
            icc_profile: None,
            coding_independent_code_points: None,
            mastering_display_color_volume: None,
            content_light_level: None,
            exif_metadata: None,
            uncompressed_latin1_text: Vec::new(),
            compressed_latin1_text: Vec::new(),
            utf8_text: Vec::new(),
        }
    }
}

impl Info<'_> {
    /// A utility constructor for a default info with width and height.
    pub fn with_size(width: u32, height: u32) -> Self {
        Info {
            width,
            height,
            ..Default::default()
        }
    }

    /// Size of the image, width then height.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Returns true if the image is an APNG image.
    pub fn is_animated(&self) -> bool {
        self.frame_control.is_some() && self.animation_control.is_some()
    }

    /// Returns the frame control information of the image.
    pub fn animation_control(&self) -> Option<&AnimationControl> {
        self.animation_control.as_ref()
    }

    /// Returns the frame control information of the current frame
    pub fn frame_control(&self) -> Option<&FrameControl> {
        self.frame_control.as_ref()
    }

    /// Returns the number of bits per pixel.
    pub fn bits_per_pixel(&self) -> usize {
        self.color_type.bits_per_pixel(self.bit_depth)
    }

    /// Returns the number of bytes per pixel.
    pub fn bytes_per_pixel(&self) -> usize {
        // If adjusting this for expansion or other transformation passes, remember to keep the old
        // implementation for bpp_in_prediction, which is internal to the png specification.
        self.color_type.bytes_per_pixel(self.bit_depth)
    }

    /// Return the number of bytes for this pixel used in prediction.
    ///
    /// Some filters use prediction, over the raw bytes of a scanline. Where a previous pixel is
    /// require for such forms the specification instead references previous bytes. That is, for
    /// a gray pixel of bit depth 2, the pixel used in prediction is actually 4 pixels prior. This
    /// has the consequence that the number of possible values is rather small. To make this fact
    /// more obvious in the type system and the optimizer we use an explicit enum here.
    pub(crate) fn bpp_in_prediction(&self) -> BytesPerPixel {
        BytesPerPixel::from_usize(self.bytes_per_pixel())
    }

    /// Returns the number of bytes needed for one deinterlaced image.
    pub fn raw_bytes(&self) -> usize {
        self.height as usize * self.raw_row_length()
    }

    /// Returns the number of bytes needed for one deinterlaced row.
    pub fn raw_row_length(&self) -> usize {
        self.raw_row_length_from_width(self.width)
    }

    pub(crate) fn checked_raw_row_length(&self) -> Option<usize> {
        self.color_type
            .checked_raw_row_length(self.bit_depth, self.width)
    }

    /// Returns the number of bytes needed for one deinterlaced row of width `width`.
    pub fn raw_row_length_from_width(&self, width: u32) -> usize {
        self.color_type
            .raw_row_length_from_width(self.bit_depth, width)
    }

    /// Gamma dependent on sRGB chunk
    pub fn gamma(&self) -> Option<ScaledFloat> {
        if self.srgb.is_some() {
            Some(crate::srgb::substitute_gamma())
        } else {
            self.gama_chunk
        }
    }

    /// Chromaticities dependent on sRGB chunk
    pub fn chromaticities(&self) -> Option<SourceChromaticities> {
        if self.srgb.is_some() {
            Some(crate::srgb::substitute_chromaticities())
        } else {
            self.chrm_chunk
        }
    }

    /// Mark the image data as conforming to the SRGB color space with the specified rendering intent.
    ///
    /// Any ICC profiles will be ignored.
    ///
    /// Source gamma and chromaticities will be written only if they're set to fallback
    /// values specified in [11.3.2.5](https://www.w3.org/TR/png-3/#sRGB-gAMA-cHRM).
    pub(crate) fn set_source_srgb(&mut self, rendering_intent: SrgbRenderingIntent) {
        self.srgb = Some(rendering_intent);
        self.icc_profile = None;
    }
}

impl BytesPerPixel {
    pub(crate) fn from_usize(bpp: usize) -> Self {
        match bpp {
            1 => BytesPerPixel::One,
            2 => BytesPerPixel::Two,
            3 => BytesPerPixel::Three,
            4 => BytesPerPixel::Four,
            6 => BytesPerPixel::Six,   // Only rgb×16bit
            8 => BytesPerPixel::Eight, // Only rgba×16bit
            _ => unreachable!("Not a possible byte rounded pixel width"),
        }
    }

    pub(crate) fn into_usize(self) -> usize {
        self as usize
    }
}

bitflags::bitflags! {
    /// Output transformations
    ///
    /// Many flags from libpng are not yet supported. A PR discussing/adding them would be nice.
    ///
    #[doc = "
    ```c
    /// Discard the alpha channel
    const STRIP_ALPHA         = 0x0002; // read only
    /// Expand 1; 2 and 4-bit samples to bytes
    const PACKING             = 0x0004; // read and write
    /// Change order of packed pixels to LSB first
    const PACKSWAP            = 0x0008; // read and write
    /// Invert monochrome images
    const INVERT_MONO         = 0x0020; // read and write
    /// Normalize pixels to the sBIT depth
    const SHIFT               = 0x0040; // read and write
    /// Flip RGB to BGR; RGBA to BGRA
    const BGR                 = 0x0080; // read and write
    /// Flip RGBA to ARGB or GA to AG
    const SWAP_ALPHA          = 0x0100; // read and write
    /// Byte-swap 16-bit samples
    const SWAP_ENDIAN         = 0x0200; // read and write
    /// Change alpha from opacity to transparency
    const INVERT_ALPHA        = 0x0400; // read and write
    const STRIP_FILLER        = 0x0800; // write only
    const STRIP_FILLER_BEFORE = 0x0800; // write only
    const STRIP_FILLER_AFTER  = 0x1000; // write only
    const GRAY_TO_RGB         = 0x2000; // read only
    const EXPAND_16           = 0x4000; // read only
    /// Similar to STRIP_16 but in libpng considering gamma?
    /// Not entirely sure the documentation says it is more
    /// accurate but doesn't say precisely how.
    const SCALE_16            = 0x8000; // read only
    ```
    "]
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub struct Transformations: u32 {
        /// No transformation
        const IDENTITY            = 0x00000; // read and write */
        /// Strip 16-bit samples to 8 bits
        const STRIP_16            = 0x00001; // read only */
        /// Expand paletted images to RGB; expand grayscale images of
        /// less than 8-bit depth to 8-bit depth; and expand tRNS chunks
        /// to alpha channels.
        const EXPAND              = 0x00010; // read only */
        /// Expand paletted images to include an alpha channel. Implies `EXPAND`.
        const ALPHA               = 0x10000; // read only */
    }
}

impl Transformations {
    /// Transform every input to 8bit grayscale or color.
    ///
    /// This sets `EXPAND` and `STRIP_16` which is similar to the default transformation used by
    /// this library prior to `0.17`.
    pub fn normalize_to_color8() -> Transformations {
        Transformations::EXPAND | Transformations::STRIP_16
    }
}

/// Instantiate the default transformations, the identity transform.
impl Default for Transformations {
    fn default() -> Transformations {
        Transformations::IDENTITY
    }
}

#[derive(Debug)]
pub struct ParameterError {
    inner: ParameterErrorKind,
}

#[derive(Debug)]
pub(crate) enum ParameterErrorKind {
    /// A provided buffer must be have the exact size to hold the image data. Where the buffer can
    /// be allocated by the caller, they must ensure that it has a minimum size as hinted previously.
    /// Even though the size is calculated from image data, this does counts as a parameter error
    /// because they must react to a value produced by this library, which can have been subjected
    /// to limits.
    ImageBufferSize { expected: usize, actual: usize },
    /// A bit like return `None` from an iterator.
    /// We use it to differentiate between failing to seek to the next image in a sequence and the
    /// absence of a next image. This is an error of the caller because they should have checked
    /// the number of images by inspecting the header data returned when opening the image. This
    /// library will perform the checks necessary to ensure that data was accurate or error with a
    /// format error otherwise.
    PolledAfterEndOfImage,
    /// Attempt to continue decoding after a fatal, non-resumable error was reported (e.g. after
    /// [`DecodingError::Format`]).  The only case when it is possible to resume after an error
    /// is an `UnexpectedEof` scenario - see [`DecodingError::IoError`].
    PolledAfterFatalError,
}

impl From<ParameterErrorKind> for ParameterError {
    fn from(inner: ParameterErrorKind) -> Self {
        ParameterError { inner }
    }
}

impl fmt::Display for ParameterError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use ParameterErrorKind::*;
        match self.inner {
            ImageBufferSize { expected, actual } => {
                write!(fmt, "wrong data size, expected {} got {}", expected, actual)
            }
            PolledAfterEndOfImage => write!(fmt, "End of image has been reached"),
            PolledAfterFatalError => {
                write!(fmt, "A fatal decoding error has been encounted earlier")
            }
        }
    }
}
