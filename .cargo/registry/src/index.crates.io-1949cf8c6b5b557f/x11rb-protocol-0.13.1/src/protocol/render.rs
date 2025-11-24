// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Render` X11 extension.

#![allow(clippy::too_many_arguments)]
// The code generator is simpler if it can always use conversions
#![allow(clippy::useless_conversion)]

#[allow(unused_imports)]
use alloc::borrow::Cow;
#[allow(unused_imports)]
use core::convert::TryInto;
use alloc::vec;
use alloc::vec::Vec;
use core::convert::TryFrom;
use crate::errors::ParseError;
#[allow(unused_imports)]
use crate::x11_utils::TryIntoUSize;
use crate::BufWithFds;
#[allow(unused_imports)]
use crate::utils::{RawFdContainer, pretty_print_bitmask, pretty_print_enum};
#[allow(unused_imports)]
use crate::x11_utils::{Request, RequestHeader, Serialize, TryParse, TryParseFd};
#[allow(unused_imports)]
use super::xproto;

/// The X11 name of the extension for QueryExtension
pub const X11_EXTENSION_NAME: &str = "RENDER";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (0, 11);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PictType(u8);
impl PictType {
    pub const INDEXED: Self = Self(0);
    pub const DIRECT: Self = Self(1);
}
impl From<PictType> for u8 {
    #[inline]
    fn from(input: PictType) -> Self {
        input.0
    }
}
impl From<PictType> for Option<u8> {
    #[inline]
    fn from(input: PictType) -> Self {
        Some(input.0)
    }
}
impl From<PictType> for u16 {
    #[inline]
    fn from(input: PictType) -> Self {
        u16::from(input.0)
    }
}
impl From<PictType> for Option<u16> {
    #[inline]
    fn from(input: PictType) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<PictType> for u32 {
    #[inline]
    fn from(input: PictType) -> Self {
        u32::from(input.0)
    }
}
impl From<PictType> for Option<u32> {
    #[inline]
    fn from(input: PictType) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for PictType {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for PictType  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::INDEXED.0.into(), "INDEXED", "Indexed"),
            (Self::DIRECT.0.into(), "DIRECT", "Direct"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PictureEnum(u8);
impl PictureEnum {
    pub const NONE: Self = Self(0);
}
impl From<PictureEnum> for u8 {
    #[inline]
    fn from(input: PictureEnum) -> Self {
        input.0
    }
}
impl From<PictureEnum> for Option<u8> {
    #[inline]
    fn from(input: PictureEnum) -> Self {
        Some(input.0)
    }
}
impl From<PictureEnum> for u16 {
    #[inline]
    fn from(input: PictureEnum) -> Self {
        u16::from(input.0)
    }
}
impl From<PictureEnum> for Option<u16> {
    #[inline]
    fn from(input: PictureEnum) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<PictureEnum> for u32 {
    #[inline]
    fn from(input: PictureEnum) -> Self {
        u32::from(input.0)
    }
}
impl From<PictureEnum> for Option<u32> {
    #[inline]
    fn from(input: PictureEnum) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for PictureEnum {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for PictureEnum  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NONE.0.into(), "NONE", "None"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PictOp(u8);
impl PictOp {
    pub const CLEAR: Self = Self(0);
    pub const SRC: Self = Self(1);
    pub const DST: Self = Self(2);
    pub const OVER: Self = Self(3);
    pub const OVER_REVERSE: Self = Self(4);
    pub const IN: Self = Self(5);
    pub const IN_REVERSE: Self = Self(6);
    pub const OUT: Self = Self(7);
    pub const OUT_REVERSE: Self = Self(8);
    pub const ATOP: Self = Self(9);
    pub const ATOP_REVERSE: Self = Self(10);
    pub const XOR: Self = Self(11);
    pub const ADD: Self = Self(12);
    pub const SATURATE: Self = Self(13);
    pub const DISJOINT_CLEAR: Self = Self(16);
    pub const DISJOINT_SRC: Self = Self(17);
    pub const DISJOINT_DST: Self = Self(18);
    pub const DISJOINT_OVER: Self = Self(19);
    pub const DISJOINT_OVER_REVERSE: Self = Self(20);
    pub const DISJOINT_IN: Self = Self(21);
    pub const DISJOINT_IN_REVERSE: Self = Self(22);
    pub const DISJOINT_OUT: Self = Self(23);
    pub const DISJOINT_OUT_REVERSE: Self = Self(24);
    pub const DISJOINT_ATOP: Self = Self(25);
    pub const DISJOINT_ATOP_REVERSE: Self = Self(26);
    pub const DISJOINT_XOR: Self = Self(27);
    pub const CONJOINT_CLEAR: Self = Self(32);
    pub const CONJOINT_SRC: Self = Self(33);
    pub const CONJOINT_DST: Self = Self(34);
    pub const CONJOINT_OVER: Self = Self(35);
    pub const CONJOINT_OVER_REVERSE: Self = Self(36);
    pub const CONJOINT_IN: Self = Self(37);
    pub const CONJOINT_IN_REVERSE: Self = Self(38);
    pub const CONJOINT_OUT: Self = Self(39);
    pub const CONJOINT_OUT_REVERSE: Self = Self(40);
    pub const CONJOINT_ATOP: Self = Self(41);
    pub const CONJOINT_ATOP_REVERSE: Self = Self(42);
    pub const CONJOINT_XOR: Self = Self(43);
    pub const MULTIPLY: Self = Self(48);
    pub const SCREEN: Self = Self(49);
    pub const OVERLAY: Self = Self(50);
    pub const DARKEN: Self = Self(51);
    pub const LIGHTEN: Self = Self(52);
    pub const COLOR_DODGE: Self = Self(53);
    pub const COLOR_BURN: Self = Self(54);
    pub const HARD_LIGHT: Self = Self(55);
    pub const SOFT_LIGHT: Self = Self(56);
    pub const DIFFERENCE: Self = Self(57);
    pub const EXCLUSION: Self = Self(58);
    pub const HSL_HUE: Self = Self(59);
    pub const HSL_SATURATION: Self = Self(60);
    pub const HSL_COLOR: Self = Self(61);
    pub const HSL_LUMINOSITY: Self = Self(62);
}
impl From<PictOp> for u8 {
    #[inline]
    fn from(input: PictOp) -> Self {
        input.0
    }
}
impl From<PictOp> for Option<u8> {
    #[inline]
    fn from(input: PictOp) -> Self {
        Some(input.0)
    }
}
impl From<PictOp> for u16 {
    #[inline]
    fn from(input: PictOp) -> Self {
        u16::from(input.0)
    }
}
impl From<PictOp> for Option<u16> {
    #[inline]
    fn from(input: PictOp) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<PictOp> for u32 {
    #[inline]
    fn from(input: PictOp) -> Self {
        u32::from(input.0)
    }
}
impl From<PictOp> for Option<u32> {
    #[inline]
    fn from(input: PictOp) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for PictOp {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for PictOp  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::CLEAR.0.into(), "CLEAR", "Clear"),
            (Self::SRC.0.into(), "SRC", "Src"),
            (Self::DST.0.into(), "DST", "Dst"),
            (Self::OVER.0.into(), "OVER", "Over"),
            (Self::OVER_REVERSE.0.into(), "OVER_REVERSE", "OverReverse"),
            (Self::IN.0.into(), "IN", "In"),
            (Self::IN_REVERSE.0.into(), "IN_REVERSE", "InReverse"),
            (Self::OUT.0.into(), "OUT", "Out"),
            (Self::OUT_REVERSE.0.into(), "OUT_REVERSE", "OutReverse"),
            (Self::ATOP.0.into(), "ATOP", "Atop"),
            (Self::ATOP_REVERSE.0.into(), "ATOP_REVERSE", "AtopReverse"),
            (Self::XOR.0.into(), "XOR", "Xor"),
            (Self::ADD.0.into(), "ADD", "Add"),
            (Self::SATURATE.0.into(), "SATURATE", "Saturate"),
            (Self::DISJOINT_CLEAR.0.into(), "DISJOINT_CLEAR", "DisjointClear"),
            (Self::DISJOINT_SRC.0.into(), "DISJOINT_SRC", "DisjointSrc"),
            (Self::DISJOINT_DST.0.into(), "DISJOINT_DST", "DisjointDst"),
            (Self::DISJOINT_OVER.0.into(), "DISJOINT_OVER", "DisjointOver"),
            (Self::DISJOINT_OVER_REVERSE.0.into(), "DISJOINT_OVER_REVERSE", "DisjointOverReverse"),
            (Self::DISJOINT_IN.0.into(), "DISJOINT_IN", "DisjointIn"),
            (Self::DISJOINT_IN_REVERSE.0.into(), "DISJOINT_IN_REVERSE", "DisjointInReverse"),
            (Self::DISJOINT_OUT.0.into(), "DISJOINT_OUT", "DisjointOut"),
            (Self::DISJOINT_OUT_REVERSE.0.into(), "DISJOINT_OUT_REVERSE", "DisjointOutReverse"),
            (Self::DISJOINT_ATOP.0.into(), "DISJOINT_ATOP", "DisjointAtop"),
            (Self::DISJOINT_ATOP_REVERSE.0.into(), "DISJOINT_ATOP_REVERSE", "DisjointAtopReverse"),
            (Self::DISJOINT_XOR.0.into(), "DISJOINT_XOR", "DisjointXor"),
            (Self::CONJOINT_CLEAR.0.into(), "CONJOINT_CLEAR", "ConjointClear"),
            (Self::CONJOINT_SRC.0.into(), "CONJOINT_SRC", "ConjointSrc"),
            (Self::CONJOINT_DST.0.into(), "CONJOINT_DST", "ConjointDst"),
            (Self::CONJOINT_OVER.0.into(), "CONJOINT_OVER", "ConjointOver"),
            (Self::CONJOINT_OVER_REVERSE.0.into(), "CONJOINT_OVER_REVERSE", "ConjointOverReverse"),
            (Self::CONJOINT_IN.0.into(), "CONJOINT_IN", "ConjointIn"),
            (Self::CONJOINT_IN_REVERSE.0.into(), "CONJOINT_IN_REVERSE", "ConjointInReverse"),
            (Self::CONJOINT_OUT.0.into(), "CONJOINT_OUT", "ConjointOut"),
            (Self::CONJOINT_OUT_REVERSE.0.into(), "CONJOINT_OUT_REVERSE", "ConjointOutReverse"),
            (Self::CONJOINT_ATOP.0.into(), "CONJOINT_ATOP", "ConjointAtop"),
            (Self::CONJOINT_ATOP_REVERSE.0.into(), "CONJOINT_ATOP_REVERSE", "ConjointAtopReverse"),
            (Self::CONJOINT_XOR.0.into(), "CONJOINT_XOR", "ConjointXor"),
            (Self::MULTIPLY.0.into(), "MULTIPLY", "Multiply"),
            (Self::SCREEN.0.into(), "SCREEN", "Screen"),
            (Self::OVERLAY.0.into(), "OVERLAY", "Overlay"),
            (Self::DARKEN.0.into(), "DARKEN", "Darken"),
            (Self::LIGHTEN.0.into(), "LIGHTEN", "Lighten"),
            (Self::COLOR_DODGE.0.into(), "COLOR_DODGE", "ColorDodge"),
            (Self::COLOR_BURN.0.into(), "COLOR_BURN", "ColorBurn"),
            (Self::HARD_LIGHT.0.into(), "HARD_LIGHT", "HardLight"),
            (Self::SOFT_LIGHT.0.into(), "SOFT_LIGHT", "SoftLight"),
            (Self::DIFFERENCE.0.into(), "DIFFERENCE", "Difference"),
            (Self::EXCLUSION.0.into(), "EXCLUSION", "Exclusion"),
            (Self::HSL_HUE.0.into(), "HSL_HUE", "HSLHue"),
            (Self::HSL_SATURATION.0.into(), "HSL_SATURATION", "HSLSaturation"),
            (Self::HSL_COLOR.0.into(), "HSL_COLOR", "HSLColor"),
            (Self::HSL_LUMINOSITY.0.into(), "HSL_LUMINOSITY", "HSLLuminosity"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolyEdge(u32);
impl PolyEdge {
    pub const SHARP: Self = Self(0);
    pub const SMOOTH: Self = Self(1);
}
impl From<PolyEdge> for u32 {
    #[inline]
    fn from(input: PolyEdge) -> Self {
        input.0
    }
}
impl From<PolyEdge> for Option<u32> {
    #[inline]
    fn from(input: PolyEdge) -> Self {
        Some(input.0)
    }
}
impl From<u8> for PolyEdge {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for PolyEdge {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for PolyEdge {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for PolyEdge  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SHARP.0, "SHARP", "Sharp"),
            (Self::SMOOTH.0, "SMOOTH", "Smooth"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolyMode(u32);
impl PolyMode {
    pub const PRECISE: Self = Self(0);
    pub const IMPRECISE: Self = Self(1);
}
impl From<PolyMode> for u32 {
    #[inline]
    fn from(input: PolyMode) -> Self {
        input.0
    }
}
impl From<PolyMode> for Option<u32> {
    #[inline]
    fn from(input: PolyMode) -> Self {
        Some(input.0)
    }
}
impl From<u8> for PolyMode {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for PolyMode {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for PolyMode {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for PolyMode  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::PRECISE.0, "PRECISE", "Precise"),
            (Self::IMPRECISE.0, "IMPRECISE", "Imprecise"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CP(u32);
impl CP {
    pub const REPEAT: Self = Self(1 << 0);
    pub const ALPHA_MAP: Self = Self(1 << 1);
    pub const ALPHA_X_ORIGIN: Self = Self(1 << 2);
    pub const ALPHA_Y_ORIGIN: Self = Self(1 << 3);
    pub const CLIP_X_ORIGIN: Self = Self(1 << 4);
    pub const CLIP_Y_ORIGIN: Self = Self(1 << 5);
    pub const CLIP_MASK: Self = Self(1 << 6);
    pub const GRAPHICS_EXPOSURE: Self = Self(1 << 7);
    pub const SUBWINDOW_MODE: Self = Self(1 << 8);
    pub const POLY_EDGE: Self = Self(1 << 9);
    pub const POLY_MODE: Self = Self(1 << 10);
    pub const DITHER: Self = Self(1 << 11);
    pub const COMPONENT_ALPHA: Self = Self(1 << 12);
}
impl From<CP> for u32 {
    #[inline]
    fn from(input: CP) -> Self {
        input.0
    }
}
impl From<CP> for Option<u32> {
    #[inline]
    fn from(input: CP) -> Self {
        Some(input.0)
    }
}
impl From<u8> for CP {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for CP {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for CP {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for CP  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::REPEAT.0, "REPEAT", "Repeat"),
            (Self::ALPHA_MAP.0, "ALPHA_MAP", "AlphaMap"),
            (Self::ALPHA_X_ORIGIN.0, "ALPHA_X_ORIGIN", "AlphaXOrigin"),
            (Self::ALPHA_Y_ORIGIN.0, "ALPHA_Y_ORIGIN", "AlphaYOrigin"),
            (Self::CLIP_X_ORIGIN.0, "CLIP_X_ORIGIN", "ClipXOrigin"),
            (Self::CLIP_Y_ORIGIN.0, "CLIP_Y_ORIGIN", "ClipYOrigin"),
            (Self::CLIP_MASK.0, "CLIP_MASK", "ClipMask"),
            (Self::GRAPHICS_EXPOSURE.0, "GRAPHICS_EXPOSURE", "GraphicsExposure"),
            (Self::SUBWINDOW_MODE.0, "SUBWINDOW_MODE", "SubwindowMode"),
            (Self::POLY_EDGE.0, "POLY_EDGE", "PolyEdge"),
            (Self::POLY_MODE.0, "POLY_MODE", "PolyMode"),
            (Self::DITHER.0, "DITHER", "Dither"),
            (Self::COMPONENT_ALPHA.0, "COMPONENT_ALPHA", "ComponentAlpha"),
        ];
        pretty_print_bitmask(fmt, self.0, &variants)
    }
}
bitmask_binop!(CP, u32);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SubPixel(u32);
impl SubPixel {
    pub const UNKNOWN: Self = Self(0);
    pub const HORIZONTAL_RGB: Self = Self(1);
    pub const HORIZONTAL_BGR: Self = Self(2);
    pub const VERTICAL_RGB: Self = Self(3);
    pub const VERTICAL_BGR: Self = Self(4);
    pub const NONE: Self = Self(5);
}
impl From<SubPixel> for u32 {
    #[inline]
    fn from(input: SubPixel) -> Self {
        input.0
    }
}
impl From<SubPixel> for Option<u32> {
    #[inline]
    fn from(input: SubPixel) -> Self {
        Some(input.0)
    }
}
impl From<u8> for SubPixel {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for SubPixel {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for SubPixel {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SubPixel  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::UNKNOWN.0, "UNKNOWN", "Unknown"),
            (Self::HORIZONTAL_RGB.0, "HORIZONTAL_RGB", "HorizontalRGB"),
            (Self::HORIZONTAL_BGR.0, "HORIZONTAL_BGR", "HorizontalBGR"),
            (Self::VERTICAL_RGB.0, "VERTICAL_RGB", "VerticalRGB"),
            (Self::VERTICAL_BGR.0, "VERTICAL_BGR", "VerticalBGR"),
            (Self::NONE.0, "NONE", "None"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Repeat(u32);
impl Repeat {
    pub const NONE: Self = Self(0);
    pub const NORMAL: Self = Self(1);
    pub const PAD: Self = Self(2);
    pub const REFLECT: Self = Self(3);
}
impl From<Repeat> for u32 {
    #[inline]
    fn from(input: Repeat) -> Self {
        input.0
    }
}
impl From<Repeat> for Option<u32> {
    #[inline]
    fn from(input: Repeat) -> Self {
        Some(input.0)
    }
}
impl From<u8> for Repeat {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for Repeat {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for Repeat {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Repeat  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NONE.0, "NONE", "None"),
            (Self::NORMAL.0, "NORMAL", "Normal"),
            (Self::PAD.0, "PAD", "Pad"),
            (Self::REFLECT.0, "REFLECT", "Reflect"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

pub type Glyph = u32;

pub type Glyphset = u32;

pub type Picture = u32;

pub type Pictformat = u32;

pub type Fixed = i32;

/// Opcode for the PictFormat error
pub const PICT_FORMAT_ERROR: u8 = 0;

/// Opcode for the Picture error
pub const PICTURE_ERROR: u8 = 1;

/// Opcode for the PictOp error
pub const PICT_OP_ERROR: u8 = 2;

/// Opcode for the GlyphSet error
pub const GLYPH_SET_ERROR: u8 = 3;

/// Opcode for the Glyph error
pub const GLYPH_ERROR: u8 = 4;

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Directformat {
    pub red_shift: u16,
    pub red_mask: u16,
    pub green_shift: u16,
    pub green_mask: u16,
    pub blue_shift: u16,
    pub blue_mask: u16,
    pub alpha_shift: u16,
    pub alpha_mask: u16,
}
impl_debug_if_no_extra_traits!(Directformat, "Directformat");
impl TryParse for Directformat {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (red_shift, remaining) = u16::try_parse(remaining)?;
        let (red_mask, remaining) = u16::try_parse(remaining)?;
        let (green_shift, remaining) = u16::try_parse(remaining)?;
        let (green_mask, remaining) = u16::try_parse(remaining)?;
        let (blue_shift, remaining) = u16::try_parse(remaining)?;
        let (blue_mask, remaining) = u16::try_parse(remaining)?;
        let (alpha_shift, remaining) = u16::try_parse(remaining)?;
        let (alpha_mask, remaining) = u16::try_parse(remaining)?;
        let result = Directformat { red_shift, red_mask, green_shift, green_mask, blue_shift, blue_mask, alpha_shift, alpha_mask };
        Ok((result, remaining))
    }
}
impl Serialize for Directformat {
    type Bytes = [u8; 16];
    fn serialize(&self) -> [u8; 16] {
        let red_shift_bytes = self.red_shift.serialize();
        let red_mask_bytes = self.red_mask.serialize();
        let green_shift_bytes = self.green_shift.serialize();
        let green_mask_bytes = self.green_mask.serialize();
        let blue_shift_bytes = self.blue_shift.serialize();
        let blue_mask_bytes = self.blue_mask.serialize();
        let alpha_shift_bytes = self.alpha_shift.serialize();
        let alpha_mask_bytes = self.alpha_mask.serialize();
        [
            red_shift_bytes[0],
            red_shift_bytes[1],
            red_mask_bytes[0],
            red_mask_bytes[1],
            green_shift_bytes[0],
            green_shift_bytes[1],
            green_mask_bytes[0],
            green_mask_bytes[1],
            blue_shift_bytes[0],
            blue_shift_bytes[1],
            blue_mask_bytes[0],
            blue_mask_bytes[1],
            alpha_shift_bytes[0],
            alpha_shift_bytes[1],
            alpha_mask_bytes[0],
            alpha_mask_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        self.red_shift.serialize_into(bytes);
        self.red_mask.serialize_into(bytes);
        self.green_shift.serialize_into(bytes);
        self.green_mask.serialize_into(bytes);
        self.blue_shift.serialize_into(bytes);
        self.blue_mask.serialize_into(bytes);
        self.alpha_shift.serialize_into(bytes);
        self.alpha_mask.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pictforminfo {
    pub id: Pictformat,
    pub type_: PictType,
    pub depth: u8,
    pub direct: Directformat,
    pub colormap: xproto::Colormap,
}
impl_debug_if_no_extra_traits!(Pictforminfo, "Pictforminfo");
impl TryParse for Pictforminfo {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (id, remaining) = Pictformat::try_parse(remaining)?;
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (depth, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (direct, remaining) = Directformat::try_parse(remaining)?;
        let (colormap, remaining) = xproto::Colormap::try_parse(remaining)?;
        let type_ = type_.into();
        let result = Pictforminfo { id, type_, depth, direct, colormap };
        Ok((result, remaining))
    }
}
impl Serialize for Pictforminfo {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        let id_bytes = self.id.serialize();
        let type_bytes = u8::from(self.type_).serialize();
        let depth_bytes = self.depth.serialize();
        let direct_bytes = self.direct.serialize();
        let colormap_bytes = self.colormap.serialize();
        [
            id_bytes[0],
            id_bytes[1],
            id_bytes[2],
            id_bytes[3],
            type_bytes[0],
            depth_bytes[0],
            0,
            0,
            direct_bytes[0],
            direct_bytes[1],
            direct_bytes[2],
            direct_bytes[3],
            direct_bytes[4],
            direct_bytes[5],
            direct_bytes[6],
            direct_bytes[7],
            direct_bytes[8],
            direct_bytes[9],
            direct_bytes[10],
            direct_bytes[11],
            direct_bytes[12],
            direct_bytes[13],
            direct_bytes[14],
            direct_bytes[15],
            colormap_bytes[0],
            colormap_bytes[1],
            colormap_bytes[2],
            colormap_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(28);
        self.id.serialize_into(bytes);
        u8::from(self.type_).serialize_into(bytes);
        self.depth.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.direct.serialize_into(bytes);
        self.colormap.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pictvisual {
    pub visual: xproto::Visualid,
    pub format: Pictformat,
}
impl_debug_if_no_extra_traits!(Pictvisual, "Pictvisual");
impl TryParse for Pictvisual {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (visual, remaining) = xproto::Visualid::try_parse(remaining)?;
        let (format, remaining) = Pictformat::try_parse(remaining)?;
        let result = Pictvisual { visual, format };
        Ok((result, remaining))
    }
}
impl Serialize for Pictvisual {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let visual_bytes = self.visual.serialize();
        let format_bytes = self.format.serialize();
        [
            visual_bytes[0],
            visual_bytes[1],
            visual_bytes[2],
            visual_bytes[3],
            format_bytes[0],
            format_bytes[1],
            format_bytes[2],
            format_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.visual.serialize_into(bytes);
        self.format.serialize_into(bytes);
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pictdepth {
    pub depth: u8,
    pub visuals: Vec<Pictvisual>,
}
impl_debug_if_no_extra_traits!(Pictdepth, "Pictdepth");
impl TryParse for Pictdepth {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (depth, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (num_visuals, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let (visuals, remaining) = crate::x11_utils::parse_list::<Pictvisual>(remaining, num_visuals.try_to_usize()?)?;
        let result = Pictdepth { depth, visuals };
        Ok((result, remaining))
    }
}
impl Serialize for Pictdepth {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.depth.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        let num_visuals = u16::try_from(self.visuals.len()).expect("`visuals` has too many elements");
        num_visuals.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
        self.visuals.serialize_into(bytes);
    }
}
impl Pictdepth {
    /// Get the value of the `num_visuals` field.
    ///
    /// The `num_visuals` field is used as the length field of the `visuals` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_visuals(&self) -> u16 {
        self.visuals.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pictscreen {
    pub fallback: Pictformat,
    pub depths: Vec<Pictdepth>,
}
impl_debug_if_no_extra_traits!(Pictscreen, "Pictscreen");
impl TryParse for Pictscreen {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (num_depths, remaining) = u32::try_parse(remaining)?;
        let (fallback, remaining) = Pictformat::try_parse(remaining)?;
        let (depths, remaining) = crate::x11_utils::parse_list::<Pictdepth>(remaining, num_depths.try_to_usize()?)?;
        let result = Pictscreen { fallback, depths };
        Ok((result, remaining))
    }
}
impl Serialize for Pictscreen {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        let num_depths = u32::try_from(self.depths.len()).expect("`depths` has too many elements");
        num_depths.serialize_into(bytes);
        self.fallback.serialize_into(bytes);
        self.depths.serialize_into(bytes);
    }
}
impl Pictscreen {
    /// Get the value of the `num_depths` field.
    ///
    /// The `num_depths` field is used as the length field of the `depths` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_depths(&self) -> u32 {
        self.depths.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Indexvalue {
    pub pixel: u32,
    pub red: u16,
    pub green: u16,
    pub blue: u16,
    pub alpha: u16,
}
impl_debug_if_no_extra_traits!(Indexvalue, "Indexvalue");
impl TryParse for Indexvalue {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (pixel, remaining) = u32::try_parse(remaining)?;
        let (red, remaining) = u16::try_parse(remaining)?;
        let (green, remaining) = u16::try_parse(remaining)?;
        let (blue, remaining) = u16::try_parse(remaining)?;
        let (alpha, remaining) = u16::try_parse(remaining)?;
        let result = Indexvalue { pixel, red, green, blue, alpha };
        Ok((result, remaining))
    }
}
impl Serialize for Indexvalue {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let pixel_bytes = self.pixel.serialize();
        let red_bytes = self.red.serialize();
        let green_bytes = self.green.serialize();
        let blue_bytes = self.blue.serialize();
        let alpha_bytes = self.alpha.serialize();
        [
            pixel_bytes[0],
            pixel_bytes[1],
            pixel_bytes[2],
            pixel_bytes[3],
            red_bytes[0],
            red_bytes[1],
            green_bytes[0],
            green_bytes[1],
            blue_bytes[0],
            blue_bytes[1],
            alpha_bytes[0],
            alpha_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.pixel.serialize_into(bytes);
        self.red.serialize_into(bytes);
        self.green.serialize_into(bytes);
        self.blue.serialize_into(bytes);
        self.alpha.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Color {
    pub red: u16,
    pub green: u16,
    pub blue: u16,
    pub alpha: u16,
}
impl_debug_if_no_extra_traits!(Color, "Color");
impl TryParse for Color {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (red, remaining) = u16::try_parse(remaining)?;
        let (green, remaining) = u16::try_parse(remaining)?;
        let (blue, remaining) = u16::try_parse(remaining)?;
        let (alpha, remaining) = u16::try_parse(remaining)?;
        let result = Color { red, green, blue, alpha };
        Ok((result, remaining))
    }
}
impl Serialize for Color {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let red_bytes = self.red.serialize();
        let green_bytes = self.green.serialize();
        let blue_bytes = self.blue.serialize();
        let alpha_bytes = self.alpha.serialize();
        [
            red_bytes[0],
            red_bytes[1],
            green_bytes[0],
            green_bytes[1],
            blue_bytes[0],
            blue_bytes[1],
            alpha_bytes[0],
            alpha_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.red.serialize_into(bytes);
        self.green.serialize_into(bytes);
        self.blue.serialize_into(bytes);
        self.alpha.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pointfix {
    pub x: Fixed,
    pub y: Fixed,
}
impl_debug_if_no_extra_traits!(Pointfix, "Pointfix");
impl TryParse for Pointfix {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (x, remaining) = Fixed::try_parse(remaining)?;
        let (y, remaining) = Fixed::try_parse(remaining)?;
        let result = Pointfix { x, y };
        Ok((result, remaining))
    }
}
impl Serialize for Pointfix {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        [
            x_bytes[0],
            x_bytes[1],
            x_bytes[2],
            x_bytes[3],
            y_bytes[0],
            y_bytes[1],
            y_bytes[2],
            y_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Linefix {
    pub p1: Pointfix,
    pub p2: Pointfix,
}
impl_debug_if_no_extra_traits!(Linefix, "Linefix");
impl TryParse for Linefix {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (p1, remaining) = Pointfix::try_parse(remaining)?;
        let (p2, remaining) = Pointfix::try_parse(remaining)?;
        let result = Linefix { p1, p2 };
        Ok((result, remaining))
    }
}
impl Serialize for Linefix {
    type Bytes = [u8; 16];
    fn serialize(&self) -> [u8; 16] {
        let p1_bytes = self.p1.serialize();
        let p2_bytes = self.p2.serialize();
        [
            p1_bytes[0],
            p1_bytes[1],
            p1_bytes[2],
            p1_bytes[3],
            p1_bytes[4],
            p1_bytes[5],
            p1_bytes[6],
            p1_bytes[7],
            p2_bytes[0],
            p2_bytes[1],
            p2_bytes[2],
            p2_bytes[3],
            p2_bytes[4],
            p2_bytes[5],
            p2_bytes[6],
            p2_bytes[7],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        self.p1.serialize_into(bytes);
        self.p2.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Triangle {
    pub p1: Pointfix,
    pub p2: Pointfix,
    pub p3: Pointfix,
}
impl_debug_if_no_extra_traits!(Triangle, "Triangle");
impl TryParse for Triangle {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (p1, remaining) = Pointfix::try_parse(remaining)?;
        let (p2, remaining) = Pointfix::try_parse(remaining)?;
        let (p3, remaining) = Pointfix::try_parse(remaining)?;
        let result = Triangle { p1, p2, p3 };
        Ok((result, remaining))
    }
}
impl Serialize for Triangle {
    type Bytes = [u8; 24];
    fn serialize(&self) -> [u8; 24] {
        let p1_bytes = self.p1.serialize();
        let p2_bytes = self.p2.serialize();
        let p3_bytes = self.p3.serialize();
        [
            p1_bytes[0],
            p1_bytes[1],
            p1_bytes[2],
            p1_bytes[3],
            p1_bytes[4],
            p1_bytes[5],
            p1_bytes[6],
            p1_bytes[7],
            p2_bytes[0],
            p2_bytes[1],
            p2_bytes[2],
            p2_bytes[3],
            p2_bytes[4],
            p2_bytes[5],
            p2_bytes[6],
            p2_bytes[7],
            p3_bytes[0],
            p3_bytes[1],
            p3_bytes[2],
            p3_bytes[3],
            p3_bytes[4],
            p3_bytes[5],
            p3_bytes[6],
            p3_bytes[7],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(24);
        self.p1.serialize_into(bytes);
        self.p2.serialize_into(bytes);
        self.p3.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Trapezoid {
    pub top: Fixed,
    pub bottom: Fixed,
    pub left: Linefix,
    pub right: Linefix,
}
impl_debug_if_no_extra_traits!(Trapezoid, "Trapezoid");
impl TryParse for Trapezoid {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (top, remaining) = Fixed::try_parse(remaining)?;
        let (bottom, remaining) = Fixed::try_parse(remaining)?;
        let (left, remaining) = Linefix::try_parse(remaining)?;
        let (right, remaining) = Linefix::try_parse(remaining)?;
        let result = Trapezoid { top, bottom, left, right };
        Ok((result, remaining))
    }
}
impl Serialize for Trapezoid {
    type Bytes = [u8; 40];
    fn serialize(&self) -> [u8; 40] {
        let top_bytes = self.top.serialize();
        let bottom_bytes = self.bottom.serialize();
        let left_bytes = self.left.serialize();
        let right_bytes = self.right.serialize();
        [
            top_bytes[0],
            top_bytes[1],
            top_bytes[2],
            top_bytes[3],
            bottom_bytes[0],
            bottom_bytes[1],
            bottom_bytes[2],
            bottom_bytes[3],
            left_bytes[0],
            left_bytes[1],
            left_bytes[2],
            left_bytes[3],
            left_bytes[4],
            left_bytes[5],
            left_bytes[6],
            left_bytes[7],
            left_bytes[8],
            left_bytes[9],
            left_bytes[10],
            left_bytes[11],
            left_bytes[12],
            left_bytes[13],
            left_bytes[14],
            left_bytes[15],
            right_bytes[0],
            right_bytes[1],
            right_bytes[2],
            right_bytes[3],
            right_bytes[4],
            right_bytes[5],
            right_bytes[6],
            right_bytes[7],
            right_bytes[8],
            right_bytes[9],
            right_bytes[10],
            right_bytes[11],
            right_bytes[12],
            right_bytes[13],
            right_bytes[14],
            right_bytes[15],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(40);
        self.top.serialize_into(bytes);
        self.bottom.serialize_into(bytes);
        self.left.serialize_into(bytes);
        self.right.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Glyphinfo {
    pub width: u16,
    pub height: u16,
    pub x: i16,
    pub y: i16,
    pub x_off: i16,
    pub y_off: i16,
}
impl_debug_if_no_extra_traits!(Glyphinfo, "Glyphinfo");
impl TryParse for Glyphinfo {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (x_off, remaining) = i16::try_parse(remaining)?;
        let (y_off, remaining) = i16::try_parse(remaining)?;
        let result = Glyphinfo { width, height, x, y, x_off, y_off };
        Ok((result, remaining))
    }
}
impl Serialize for Glyphinfo {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let x_off_bytes = self.x_off.serialize();
        let y_off_bytes = self.y_off.serialize();
        [
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            x_off_bytes[0],
            x_off_bytes[1],
            y_off_bytes[0],
            y_off_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
        self.x_off.serialize_into(bytes);
        self.y_off.serialize_into(bytes);
    }
}

/// Opcode for the QueryVersion request
pub const QUERY_VERSION_REQUEST: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryVersionRequest {
    pub client_major_version: u32,
    pub client_minor_version: u32,
}
impl_debug_if_no_extra_traits!(QueryVersionRequest, "QueryVersionRequest");
impl QueryVersionRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let client_major_version_bytes = self.client_major_version.serialize();
        let client_minor_version_bytes = self.client_minor_version.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_VERSION_REQUEST,
            0,
            0,
            client_major_version_bytes[0],
            client_major_version_bytes[1],
            client_major_version_bytes[2],
            client_major_version_bytes[3],
            client_minor_version_bytes[0],
            client_minor_version_bytes[1],
            client_minor_version_bytes[2],
            client_minor_version_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != QUERY_VERSION_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (client_major_version, remaining) = u32::try_parse(value)?;
        let (client_minor_version, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(QueryVersionRequest {
            client_major_version,
            client_minor_version,
        })
    }
}
impl Request for QueryVersionRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryVersionRequest {
    type Reply = QueryVersionReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryVersionReply {
    pub sequence: u16,
    pub length: u32,
    pub major_version: u32,
    pub minor_version: u32,
}
impl_debug_if_no_extra_traits!(QueryVersionReply, "QueryVersionReply");
impl TryParse for QueryVersionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (major_version, remaining) = u32::try_parse(remaining)?;
        let (minor_version, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(16..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryVersionReply { sequence, length, major_version, minor_version };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryVersionReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let major_version_bytes = self.major_version.serialize();
        let minor_version_bytes = self.minor_version.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            major_version_bytes[0],
            major_version_bytes[1],
            major_version_bytes[2],
            major_version_bytes[3],
            minor_version_bytes[0],
            minor_version_bytes[1],
            minor_version_bytes[2],
            minor_version_bytes[3],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.major_version.serialize_into(bytes);
        self.minor_version.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 16]);
    }
}

/// Opcode for the QueryPictFormats request
pub const QUERY_PICT_FORMATS_REQUEST: u8 = 1;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryPictFormatsRequest;
impl_debug_if_no_extra_traits!(QueryPictFormatsRequest, "QueryPictFormatsRequest");
impl QueryPictFormatsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            QUERY_PICT_FORMATS_REQUEST,
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != QUERY_PICT_FORMATS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(QueryPictFormatsRequest
        )
    }
}
impl Request for QueryPictFormatsRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryPictFormatsRequest {
    type Reply = QueryPictFormatsReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryPictFormatsReply {
    pub sequence: u16,
    pub length: u32,
    pub num_depths: u32,
    pub num_visuals: u32,
    pub formats: Vec<Pictforminfo>,
    pub screens: Vec<Pictscreen>,
    pub subpixels: Vec<SubPixel>,
}
impl_debug_if_no_extra_traits!(QueryPictFormatsReply, "QueryPictFormatsReply");
impl TryParse for QueryPictFormatsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (num_formats, remaining) = u32::try_parse(remaining)?;
        let (num_screens, remaining) = u32::try_parse(remaining)?;
        let (num_depths, remaining) = u32::try_parse(remaining)?;
        let (num_visuals, remaining) = u32::try_parse(remaining)?;
        let (num_subpixel, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let (formats, remaining) = crate::x11_utils::parse_list::<Pictforminfo>(remaining, num_formats.try_to_usize()?)?;
        let (screens, remaining) = crate::x11_utils::parse_list::<Pictscreen>(remaining, num_screens.try_to_usize()?)?;
        let mut remaining = remaining;
        let list_length = num_subpixel.try_to_usize()?;
        let mut subpixels = Vec::with_capacity(list_length);
        for _ in 0..list_length {
            let (v, new_remaining) = u32::try_parse(remaining)?;
            let v = v.into();
            remaining = new_remaining;
            subpixels.push(v);
        }
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryPictFormatsReply { sequence, length, num_depths, num_visuals, formats, screens, subpixels };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryPictFormatsReply {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        let num_formats = u32::try_from(self.formats.len()).expect("`formats` has too many elements");
        num_formats.serialize_into(bytes);
        let num_screens = u32::try_from(self.screens.len()).expect("`screens` has too many elements");
        num_screens.serialize_into(bytes);
        self.num_depths.serialize_into(bytes);
        self.num_visuals.serialize_into(bytes);
        let num_subpixel = u32::try_from(self.subpixels.len()).expect("`subpixels` has too many elements");
        num_subpixel.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
        self.formats.serialize_into(bytes);
        self.screens.serialize_into(bytes);
        for element in self.subpixels.iter().copied() {
            u32::from(element).serialize_into(bytes);
        }
    }
}
impl QueryPictFormatsReply {
    /// Get the value of the `num_formats` field.
    ///
    /// The `num_formats` field is used as the length field of the `formats` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_formats(&self) -> u32 {
        self.formats.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_screens` field.
    ///
    /// The `num_screens` field is used as the length field of the `screens` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_screens(&self) -> u32 {
        self.screens.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_subpixel` field.
    ///
    /// The `num_subpixel` field is used as the length field of the `subpixels` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_subpixel(&self) -> u32 {
        self.subpixels.len()
            .try_into().unwrap()
    }
}

/// Opcode for the QueryPictIndexValues request
pub const QUERY_PICT_INDEX_VALUES_REQUEST: u8 = 2;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryPictIndexValuesRequest {
    pub format: Pictformat,
}
impl_debug_if_no_extra_traits!(QueryPictIndexValuesRequest, "QueryPictIndexValuesRequest");
impl QueryPictIndexValuesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let format_bytes = self.format.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_PICT_INDEX_VALUES_REQUEST,
            0,
            0,
            format_bytes[0],
            format_bytes[1],
            format_bytes[2],
            format_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != QUERY_PICT_INDEX_VALUES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (format, remaining) = Pictformat::try_parse(value)?;
        let _ = remaining;
        Ok(QueryPictIndexValuesRequest {
            format,
        })
    }
}
impl Request for QueryPictIndexValuesRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryPictIndexValuesRequest {
    type Reply = QueryPictIndexValuesReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryPictIndexValuesReply {
    pub sequence: u16,
    pub length: u32,
    pub values: Vec<Indexvalue>,
}
impl_debug_if_no_extra_traits!(QueryPictIndexValuesReply, "QueryPictIndexValuesReply");
impl TryParse for QueryPictIndexValuesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (num_values, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (values, remaining) = crate::x11_utils::parse_list::<Indexvalue>(remaining, num_values.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryPictIndexValuesReply { sequence, length, values };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryPictIndexValuesReply {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        let num_values = u32::try_from(self.values.len()).expect("`values` has too many elements");
        num_values.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        self.values.serialize_into(bytes);
    }
}
impl QueryPictIndexValuesReply {
    /// Get the value of the `num_values` field.
    ///
    /// The `num_values` field is used as the length field of the `values` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_values(&self) -> u32 {
        self.values.len()
            .try_into().unwrap()
    }
}

/// Auxiliary and optional information for the `create_picture` function
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreatePictureAux {
    pub repeat: Option<Repeat>,
    pub alphamap: Option<Picture>,
    pub alphaxorigin: Option<i32>,
    pub alphayorigin: Option<i32>,
    pub clipxorigin: Option<i32>,
    pub clipyorigin: Option<i32>,
    pub clipmask: Option<xproto::Pixmap>,
    pub graphicsexposure: Option<u32>,
    pub subwindowmode: Option<xproto::SubwindowMode>,
    pub polyedge: Option<PolyEdge>,
    pub polymode: Option<PolyMode>,
    pub dither: Option<xproto::Atom>,
    pub componentalpha: Option<u32>,
}
impl_debug_if_no_extra_traits!(CreatePictureAux, "CreatePictureAux");
impl CreatePictureAux {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], value_mask: u32) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u32::from(value_mask);
        let mut outer_remaining = value;
        let repeat = if switch_expr & u32::from(CP::REPEAT) != 0 {
            let remaining = outer_remaining;
            let (repeat, remaining) = u32::try_parse(remaining)?;
            let repeat = repeat.into();
            outer_remaining = remaining;
            Some(repeat)
        } else {
            None
        };
        let alphamap = if switch_expr & u32::from(CP::ALPHA_MAP) != 0 {
            let remaining = outer_remaining;
            let (alphamap, remaining) = Picture::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(alphamap)
        } else {
            None
        };
        let alphaxorigin = if switch_expr & u32::from(CP::ALPHA_X_ORIGIN) != 0 {
            let remaining = outer_remaining;
            let (alphaxorigin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(alphaxorigin)
        } else {
            None
        };
        let alphayorigin = if switch_expr & u32::from(CP::ALPHA_Y_ORIGIN) != 0 {
            let remaining = outer_remaining;
            let (alphayorigin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(alphayorigin)
        } else {
            None
        };
        let clipxorigin = if switch_expr & u32::from(CP::CLIP_X_ORIGIN) != 0 {
            let remaining = outer_remaining;
            let (clipxorigin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(clipxorigin)
        } else {
            None
        };
        let clipyorigin = if switch_expr & u32::from(CP::CLIP_Y_ORIGIN) != 0 {
            let remaining = outer_remaining;
            let (clipyorigin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(clipyorigin)
        } else {
            None
        };
        let clipmask = if switch_expr & u32::from(CP::CLIP_MASK) != 0 {
            let remaining = outer_remaining;
            let (clipmask, remaining) = xproto::Pixmap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(clipmask)
        } else {
            None
        };
        let graphicsexposure = if switch_expr & u32::from(CP::GRAPHICS_EXPOSURE) != 0 {
            let remaining = outer_remaining;
            let (graphicsexposure, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(graphicsexposure)
        } else {
            None
        };
        let subwindowmode = if switch_expr & u32::from(CP::SUBWINDOW_MODE) != 0 {
            let remaining = outer_remaining;
            let (subwindowmode, remaining) = u32::try_parse(remaining)?;
            let subwindowmode = subwindowmode.into();
            outer_remaining = remaining;
            Some(subwindowmode)
        } else {
            None
        };
        let polyedge = if switch_expr & u32::from(CP::POLY_EDGE) != 0 {
            let remaining = outer_remaining;
            let (polyedge, remaining) = u32::try_parse(remaining)?;
            let polyedge = polyedge.into();
            outer_remaining = remaining;
            Some(polyedge)
        } else {
            None
        };
        let polymode = if switch_expr & u32::from(CP::POLY_MODE) != 0 {
            let remaining = outer_remaining;
            let (polymode, remaining) = u32::try_parse(remaining)?;
            let polymode = polymode.into();
            outer_remaining = remaining;
            Some(polymode)
        } else {
            None
        };
        let dither = if switch_expr & u32::from(CP::DITHER) != 0 {
            let remaining = outer_remaining;
            let (dither, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(dither)
        } else {
            None
        };
        let componentalpha = if switch_expr & u32::from(CP::COMPONENT_ALPHA) != 0 {
            let remaining = outer_remaining;
            let (componentalpha, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(componentalpha)
        } else {
            None
        };
        let result = CreatePictureAux { repeat, alphamap, alphaxorigin, alphayorigin, clipxorigin, clipyorigin, clipmask, graphicsexposure, subwindowmode, polyedge, polymode, dither, componentalpha };
        Ok((result, outer_remaining))
    }
}
impl CreatePictureAux {
    #[allow(dead_code)]
    fn serialize(&self, value_mask: u32) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u32::from(value_mask));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, value_mask: u32) {
        assert_eq!(self.switch_expr(), u32::from(value_mask), "switch `value_list` has an inconsistent discriminant");
        if let Some(repeat) = self.repeat {
            u32::from(repeat).serialize_into(bytes);
        }
        if let Some(alphamap) = self.alphamap {
            alphamap.serialize_into(bytes);
        }
        if let Some(alphaxorigin) = self.alphaxorigin {
            alphaxorigin.serialize_into(bytes);
        }
        if let Some(alphayorigin) = self.alphayorigin {
            alphayorigin.serialize_into(bytes);
        }
        if let Some(clipxorigin) = self.clipxorigin {
            clipxorigin.serialize_into(bytes);
        }
        if let Some(clipyorigin) = self.clipyorigin {
            clipyorigin.serialize_into(bytes);
        }
        if let Some(clipmask) = self.clipmask {
            clipmask.serialize_into(bytes);
        }
        if let Some(graphicsexposure) = self.graphicsexposure {
            graphicsexposure.serialize_into(bytes);
        }
        if let Some(subwindowmode) = self.subwindowmode {
            u32::from(subwindowmode).serialize_into(bytes);
        }
        if let Some(polyedge) = self.polyedge {
            u32::from(polyedge).serialize_into(bytes);
        }
        if let Some(polymode) = self.polymode {
            u32::from(polymode).serialize_into(bytes);
        }
        if let Some(dither) = self.dither {
            dither.serialize_into(bytes);
        }
        if let Some(componentalpha) = self.componentalpha {
            componentalpha.serialize_into(bytes);
        }
    }
}
impl CreatePictureAux {
    fn switch_expr(&self) -> u32 {
        let mut expr_value = 0;
        if self.repeat.is_some() {
            expr_value |= u32::from(CP::REPEAT);
        }
        if self.alphamap.is_some() {
            expr_value |= u32::from(CP::ALPHA_MAP);
        }
        if self.alphaxorigin.is_some() {
            expr_value |= u32::from(CP::ALPHA_X_ORIGIN);
        }
        if self.alphayorigin.is_some() {
            expr_value |= u32::from(CP::ALPHA_Y_ORIGIN);
        }
        if self.clipxorigin.is_some() {
            expr_value |= u32::from(CP::CLIP_X_ORIGIN);
        }
        if self.clipyorigin.is_some() {
            expr_value |= u32::from(CP::CLIP_Y_ORIGIN);
        }
        if self.clipmask.is_some() {
            expr_value |= u32::from(CP::CLIP_MASK);
        }
        if self.graphicsexposure.is_some() {
            expr_value |= u32::from(CP::GRAPHICS_EXPOSURE);
        }
        if self.subwindowmode.is_some() {
            expr_value |= u32::from(CP::SUBWINDOW_MODE);
        }
        if self.polyedge.is_some() {
            expr_value |= u32::from(CP::POLY_EDGE);
        }
        if self.polymode.is_some() {
            expr_value |= u32::from(CP::POLY_MODE);
        }
        if self.dither.is_some() {
            expr_value |= u32::from(CP::DITHER);
        }
        if self.componentalpha.is_some() {
            expr_value |= u32::from(CP::COMPONENT_ALPHA);
        }
        expr_value
    }
}
impl CreatePictureAux {
    /// Create a new instance with all fields unset / not present.
    pub fn new() -> Self {
        Default::default()
    }
    /// Set the `repeat` field of this structure.
    #[must_use]
    pub fn repeat<I>(mut self, value: I) -> Self where I: Into<Option<Repeat>> {
        self.repeat = value.into();
        self
    }
    /// Set the `alphamap` field of this structure.
    #[must_use]
    pub fn alphamap<I>(mut self, value: I) -> Self where I: Into<Option<Picture>> {
        self.alphamap = value.into();
        self
    }
    /// Set the `alphaxorigin` field of this structure.
    #[must_use]
    pub fn alphaxorigin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.alphaxorigin = value.into();
        self
    }
    /// Set the `alphayorigin` field of this structure.
    #[must_use]
    pub fn alphayorigin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.alphayorigin = value.into();
        self
    }
    /// Set the `clipxorigin` field of this structure.
    #[must_use]
    pub fn clipxorigin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.clipxorigin = value.into();
        self
    }
    /// Set the `clipyorigin` field of this structure.
    #[must_use]
    pub fn clipyorigin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.clipyorigin = value.into();
        self
    }
    /// Set the `clipmask` field of this structure.
    #[must_use]
    pub fn clipmask<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Pixmap>> {
        self.clipmask = value.into();
        self
    }
    /// Set the `graphicsexposure` field of this structure.
    #[must_use]
    pub fn graphicsexposure<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.graphicsexposure = value.into();
        self
    }
    /// Set the `subwindowmode` field of this structure.
    #[must_use]
    pub fn subwindowmode<I>(mut self, value: I) -> Self where I: Into<Option<xproto::SubwindowMode>> {
        self.subwindowmode = value.into();
        self
    }
    /// Set the `polyedge` field of this structure.
    #[must_use]
    pub fn polyedge<I>(mut self, value: I) -> Self where I: Into<Option<PolyEdge>> {
        self.polyedge = value.into();
        self
    }
    /// Set the `polymode` field of this structure.
    #[must_use]
    pub fn polymode<I>(mut self, value: I) -> Self where I: Into<Option<PolyMode>> {
        self.polymode = value.into();
        self
    }
    /// Set the `dither` field of this structure.
    #[must_use]
    pub fn dither<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Atom>> {
        self.dither = value.into();
        self
    }
    /// Set the `componentalpha` field of this structure.
    #[must_use]
    pub fn componentalpha<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.componentalpha = value.into();
        self
    }
}

/// Opcode for the CreatePicture request
pub const CREATE_PICTURE_REQUEST: u8 = 4;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreatePictureRequest<'input> {
    pub pid: Picture,
    pub drawable: xproto::Drawable,
    pub format: Pictformat,
    pub value_list: Cow<'input, CreatePictureAux>,
}
impl_debug_if_no_extra_traits!(CreatePictureRequest<'_>, "CreatePictureRequest");
impl<'input> CreatePictureRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let pid_bytes = self.pid.serialize();
        let drawable_bytes = self.drawable.serialize();
        let format_bytes = self.format.serialize();
        let value_mask: u32 = self.value_list.switch_expr();
        let value_mask_bytes = value_mask.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_PICTURE_REQUEST,
            0,
            0,
            pid_bytes[0],
            pid_bytes[1],
            pid_bytes[2],
            pid_bytes[3],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            format_bytes[0],
            format_bytes[1],
            format_bytes[2],
            format_bytes[3],
            value_mask_bytes[0],
            value_mask_bytes[1],
            value_mask_bytes[2],
            value_mask_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let value_list_bytes = self.value_list.serialize(u32::from(value_mask));
        let length_so_far = length_so_far + value_list_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), value_list_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != CREATE_PICTURE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (pid, remaining) = Picture::try_parse(value)?;
        let (drawable, remaining) = xproto::Drawable::try_parse(remaining)?;
        let (format, remaining) = Pictformat::try_parse(remaining)?;
        let (value_mask, remaining) = u32::try_parse(remaining)?;
        let (value_list, remaining) = CreatePictureAux::try_parse(remaining, u32::from(value_mask))?;
        let _ = remaining;
        Ok(CreatePictureRequest {
            pid,
            drawable,
            format,
            value_list: Cow::Owned(value_list),
        })
    }
    /// Clone all borrowed data in this CreatePictureRequest.
    pub fn into_owned(self) -> CreatePictureRequest<'static> {
        CreatePictureRequest {
            pid: self.pid,
            drawable: self.drawable,
            format: self.format,
            value_list: Cow::Owned(self.value_list.into_owned()),
        }
    }
}
impl<'input> Request for CreatePictureRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for CreatePictureRequest<'input> {
}

/// Auxiliary and optional information for the `change_picture` function
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangePictureAux {
    pub repeat: Option<Repeat>,
    pub alphamap: Option<Picture>,
    pub alphaxorigin: Option<i32>,
    pub alphayorigin: Option<i32>,
    pub clipxorigin: Option<i32>,
    pub clipyorigin: Option<i32>,
    pub clipmask: Option<xproto::Pixmap>,
    pub graphicsexposure: Option<u32>,
    pub subwindowmode: Option<xproto::SubwindowMode>,
    pub polyedge: Option<PolyEdge>,
    pub polymode: Option<PolyMode>,
    pub dither: Option<xproto::Atom>,
    pub componentalpha: Option<u32>,
}
impl_debug_if_no_extra_traits!(ChangePictureAux, "ChangePictureAux");
impl ChangePictureAux {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], value_mask: u32) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u32::from(value_mask);
        let mut outer_remaining = value;
        let repeat = if switch_expr & u32::from(CP::REPEAT) != 0 {
            let remaining = outer_remaining;
            let (repeat, remaining) = u32::try_parse(remaining)?;
            let repeat = repeat.into();
            outer_remaining = remaining;
            Some(repeat)
        } else {
            None
        };
        let alphamap = if switch_expr & u32::from(CP::ALPHA_MAP) != 0 {
            let remaining = outer_remaining;
            let (alphamap, remaining) = Picture::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(alphamap)
        } else {
            None
        };
        let alphaxorigin = if switch_expr & u32::from(CP::ALPHA_X_ORIGIN) != 0 {
            let remaining = outer_remaining;
            let (alphaxorigin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(alphaxorigin)
        } else {
            None
        };
        let alphayorigin = if switch_expr & u32::from(CP::ALPHA_Y_ORIGIN) != 0 {
            let remaining = outer_remaining;
            let (alphayorigin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(alphayorigin)
        } else {
            None
        };
        let clipxorigin = if switch_expr & u32::from(CP::CLIP_X_ORIGIN) != 0 {
            let remaining = outer_remaining;
            let (clipxorigin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(clipxorigin)
        } else {
            None
        };
        let clipyorigin = if switch_expr & u32::from(CP::CLIP_Y_ORIGIN) != 0 {
            let remaining = outer_remaining;
            let (clipyorigin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(clipyorigin)
        } else {
            None
        };
        let clipmask = if switch_expr & u32::from(CP::CLIP_MASK) != 0 {
            let remaining = outer_remaining;
            let (clipmask, remaining) = xproto::Pixmap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(clipmask)
        } else {
            None
        };
        let graphicsexposure = if switch_expr & u32::from(CP::GRAPHICS_EXPOSURE) != 0 {
            let remaining = outer_remaining;
            let (graphicsexposure, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(graphicsexposure)
        } else {
            None
        };
        let subwindowmode = if switch_expr & u32::from(CP::SUBWINDOW_MODE) != 0 {
            let remaining = outer_remaining;
            let (subwindowmode, remaining) = u32::try_parse(remaining)?;
            let subwindowmode = subwindowmode.into();
            outer_remaining = remaining;
            Some(subwindowmode)
        } else {
            None
        };
        let polyedge = if switch_expr & u32::from(CP::POLY_EDGE) != 0 {
            let remaining = outer_remaining;
            let (polyedge, remaining) = u32::try_parse(remaining)?;
            let polyedge = polyedge.into();
            outer_remaining = remaining;
            Some(polyedge)
        } else {
            None
        };
        let polymode = if switch_expr & u32::from(CP::POLY_MODE) != 0 {
            let remaining = outer_remaining;
            let (polymode, remaining) = u32::try_parse(remaining)?;
            let polymode = polymode.into();
            outer_remaining = remaining;
            Some(polymode)
        } else {
            None
        };
        let dither = if switch_expr & u32::from(CP::DITHER) != 0 {
            let remaining = outer_remaining;
            let (dither, remaining) = xproto::Atom::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(dither)
        } else {
            None
        };
        let componentalpha = if switch_expr & u32::from(CP::COMPONENT_ALPHA) != 0 {
            let remaining = outer_remaining;
            let (componentalpha, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(componentalpha)
        } else {
            None
        };
        let result = ChangePictureAux { repeat, alphamap, alphaxorigin, alphayorigin, clipxorigin, clipyorigin, clipmask, graphicsexposure, subwindowmode, polyedge, polymode, dither, componentalpha };
        Ok((result, outer_remaining))
    }
}
impl ChangePictureAux {
    #[allow(dead_code)]
    fn serialize(&self, value_mask: u32) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u32::from(value_mask));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, value_mask: u32) {
        assert_eq!(self.switch_expr(), u32::from(value_mask), "switch `value_list` has an inconsistent discriminant");
        if let Some(repeat) = self.repeat {
            u32::from(repeat).serialize_into(bytes);
        }
        if let Some(alphamap) = self.alphamap {
            alphamap.serialize_into(bytes);
        }
        if let Some(alphaxorigin) = self.alphaxorigin {
            alphaxorigin.serialize_into(bytes);
        }
        if let Some(alphayorigin) = self.alphayorigin {
            alphayorigin.serialize_into(bytes);
        }
        if let Some(clipxorigin) = self.clipxorigin {
            clipxorigin.serialize_into(bytes);
        }
        if let Some(clipyorigin) = self.clipyorigin {
            clipyorigin.serialize_into(bytes);
        }
        if let Some(clipmask) = self.clipmask {
            clipmask.serialize_into(bytes);
        }
        if let Some(graphicsexposure) = self.graphicsexposure {
            graphicsexposure.serialize_into(bytes);
        }
        if let Some(subwindowmode) = self.subwindowmode {
            u32::from(subwindowmode).serialize_into(bytes);
        }
        if let Some(polyedge) = self.polyedge {
            u32::from(polyedge).serialize_into(bytes);
        }
        if let Some(polymode) = self.polymode {
            u32::from(polymode).serialize_into(bytes);
        }
        if let Some(dither) = self.dither {
            dither.serialize_into(bytes);
        }
        if let Some(componentalpha) = self.componentalpha {
            componentalpha.serialize_into(bytes);
        }
    }
}
impl ChangePictureAux {
    fn switch_expr(&self) -> u32 {
        let mut expr_value = 0;
        if self.repeat.is_some() {
            expr_value |= u32::from(CP::REPEAT);
        }
        if self.alphamap.is_some() {
            expr_value |= u32::from(CP::ALPHA_MAP);
        }
        if self.alphaxorigin.is_some() {
            expr_value |= u32::from(CP::ALPHA_X_ORIGIN);
        }
        if self.alphayorigin.is_some() {
            expr_value |= u32::from(CP::ALPHA_Y_ORIGIN);
        }
        if self.clipxorigin.is_some() {
            expr_value |= u32::from(CP::CLIP_X_ORIGIN);
        }
        if self.clipyorigin.is_some() {
            expr_value |= u32::from(CP::CLIP_Y_ORIGIN);
        }
        if self.clipmask.is_some() {
            expr_value |= u32::from(CP::CLIP_MASK);
        }
        if self.graphicsexposure.is_some() {
            expr_value |= u32::from(CP::GRAPHICS_EXPOSURE);
        }
        if self.subwindowmode.is_some() {
            expr_value |= u32::from(CP::SUBWINDOW_MODE);
        }
        if self.polyedge.is_some() {
            expr_value |= u32::from(CP::POLY_EDGE);
        }
        if self.polymode.is_some() {
            expr_value |= u32::from(CP::POLY_MODE);
        }
        if self.dither.is_some() {
            expr_value |= u32::from(CP::DITHER);
        }
        if self.componentalpha.is_some() {
            expr_value |= u32::from(CP::COMPONENT_ALPHA);
        }
        expr_value
    }
}
impl ChangePictureAux {
    /// Create a new instance with all fields unset / not present.
    pub fn new() -> Self {
        Default::default()
    }
    /// Set the `repeat` field of this structure.
    #[must_use]
    pub fn repeat<I>(mut self, value: I) -> Self where I: Into<Option<Repeat>> {
        self.repeat = value.into();
        self
    }
    /// Set the `alphamap` field of this structure.
    #[must_use]
    pub fn alphamap<I>(mut self, value: I) -> Self where I: Into<Option<Picture>> {
        self.alphamap = value.into();
        self
    }
    /// Set the `alphaxorigin` field of this structure.
    #[must_use]
    pub fn alphaxorigin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.alphaxorigin = value.into();
        self
    }
    /// Set the `alphayorigin` field of this structure.
    #[must_use]
    pub fn alphayorigin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.alphayorigin = value.into();
        self
    }
    /// Set the `clipxorigin` field of this structure.
    #[must_use]
    pub fn clipxorigin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.clipxorigin = value.into();
        self
    }
    /// Set the `clipyorigin` field of this structure.
    #[must_use]
    pub fn clipyorigin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.clipyorigin = value.into();
        self
    }
    /// Set the `clipmask` field of this structure.
    #[must_use]
    pub fn clipmask<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Pixmap>> {
        self.clipmask = value.into();
        self
    }
    /// Set the `graphicsexposure` field of this structure.
    #[must_use]
    pub fn graphicsexposure<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.graphicsexposure = value.into();
        self
    }
    /// Set the `subwindowmode` field of this structure.
    #[must_use]
    pub fn subwindowmode<I>(mut self, value: I) -> Self where I: Into<Option<xproto::SubwindowMode>> {
        self.subwindowmode = value.into();
        self
    }
    /// Set the `polyedge` field of this structure.
    #[must_use]
    pub fn polyedge<I>(mut self, value: I) -> Self where I: Into<Option<PolyEdge>> {
        self.polyedge = value.into();
        self
    }
    /// Set the `polymode` field of this structure.
    #[must_use]
    pub fn polymode<I>(mut self, value: I) -> Self where I: Into<Option<PolyMode>> {
        self.polymode = value.into();
        self
    }
    /// Set the `dither` field of this structure.
    #[must_use]
    pub fn dither<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Atom>> {
        self.dither = value.into();
        self
    }
    /// Set the `componentalpha` field of this structure.
    #[must_use]
    pub fn componentalpha<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.componentalpha = value.into();
        self
    }
}

/// Opcode for the ChangePicture request
pub const CHANGE_PICTURE_REQUEST: u8 = 5;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangePictureRequest<'input> {
    pub picture: Picture,
    pub value_list: Cow<'input, ChangePictureAux>,
}
impl_debug_if_no_extra_traits!(ChangePictureRequest<'_>, "ChangePictureRequest");
impl<'input> ChangePictureRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let picture_bytes = self.picture.serialize();
        let value_mask: u32 = self.value_list.switch_expr();
        let value_mask_bytes = value_mask.serialize();
        let mut request0 = vec![
            major_opcode,
            CHANGE_PICTURE_REQUEST,
            0,
            0,
            picture_bytes[0],
            picture_bytes[1],
            picture_bytes[2],
            picture_bytes[3],
            value_mask_bytes[0],
            value_mask_bytes[1],
            value_mask_bytes[2],
            value_mask_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let value_list_bytes = self.value_list.serialize(u32::from(value_mask));
        let length_so_far = length_so_far + value_list_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), value_list_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != CHANGE_PICTURE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (picture, remaining) = Picture::try_parse(value)?;
        let (value_mask, remaining) = u32::try_parse(remaining)?;
        let (value_list, remaining) = ChangePictureAux::try_parse(remaining, u32::from(value_mask))?;
        let _ = remaining;
        Ok(ChangePictureRequest {
            picture,
            value_list: Cow::Owned(value_list),
        })
    }
    /// Clone all borrowed data in this ChangePictureRequest.
    pub fn into_owned(self) -> ChangePictureRequest<'static> {
        ChangePictureRequest {
            picture: self.picture,
            value_list: Cow::Owned(self.value_list.into_owned()),
        }
    }
}
impl<'input> Request for ChangePictureRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for ChangePictureRequest<'input> {
}

/// Opcode for the SetPictureClipRectangles request
pub const SET_PICTURE_CLIP_RECTANGLES_REQUEST: u8 = 6;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetPictureClipRectanglesRequest<'input> {
    pub picture: Picture,
    pub clip_x_origin: i16,
    pub clip_y_origin: i16,
    pub rectangles: Cow<'input, [xproto::Rectangle]>,
}
impl_debug_if_no_extra_traits!(SetPictureClipRectanglesRequest<'_>, "SetPictureClipRectanglesRequest");
impl<'input> SetPictureClipRectanglesRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let picture_bytes = self.picture.serialize();
        let clip_x_origin_bytes = self.clip_x_origin.serialize();
        let clip_y_origin_bytes = self.clip_y_origin.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_PICTURE_CLIP_RECTANGLES_REQUEST,
            0,
            0,
            picture_bytes[0],
            picture_bytes[1],
            picture_bytes[2],
            picture_bytes[3],
            clip_x_origin_bytes[0],
            clip_x_origin_bytes[1],
            clip_y_origin_bytes[0],
            clip_y_origin_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let rectangles_bytes = self.rectangles.serialize();
        let length_so_far = length_so_far + rectangles_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), rectangles_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_PICTURE_CLIP_RECTANGLES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (picture, remaining) = Picture::try_parse(value)?;
        let (clip_x_origin, remaining) = i16::try_parse(remaining)?;
        let (clip_y_origin, remaining) = i16::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut rectangles = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = xproto::Rectangle::try_parse(remaining)?;
            remaining = new_remaining;
            rectangles.push(v);
        }
        let _ = remaining;
        Ok(SetPictureClipRectanglesRequest {
            picture,
            clip_x_origin,
            clip_y_origin,
            rectangles: Cow::Owned(rectangles),
        })
    }
    /// Clone all borrowed data in this SetPictureClipRectanglesRequest.
    pub fn into_owned(self) -> SetPictureClipRectanglesRequest<'static> {
        SetPictureClipRectanglesRequest {
            picture: self.picture,
            clip_x_origin: self.clip_x_origin,
            clip_y_origin: self.clip_y_origin,
            rectangles: Cow::Owned(self.rectangles.into_owned()),
        }
    }
}
impl<'input> Request for SetPictureClipRectanglesRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetPictureClipRectanglesRequest<'input> {
}

/// Opcode for the FreePicture request
pub const FREE_PICTURE_REQUEST: u8 = 7;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FreePictureRequest {
    pub picture: Picture,
}
impl_debug_if_no_extra_traits!(FreePictureRequest, "FreePictureRequest");
impl FreePictureRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let picture_bytes = self.picture.serialize();
        let mut request0 = vec![
            major_opcode,
            FREE_PICTURE_REQUEST,
            0,
            0,
            picture_bytes[0],
            picture_bytes[1],
            picture_bytes[2],
            picture_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != FREE_PICTURE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (picture, remaining) = Picture::try_parse(value)?;
        let _ = remaining;
        Ok(FreePictureRequest {
            picture,
        })
    }
}
impl Request for FreePictureRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for FreePictureRequest {
}

/// Opcode for the Composite request
pub const COMPOSITE_REQUEST: u8 = 8;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CompositeRequest {
    pub op: PictOp,
    pub src: Picture,
    pub mask: Picture,
    pub dst: Picture,
    pub src_x: i16,
    pub src_y: i16,
    pub mask_x: i16,
    pub mask_y: i16,
    pub dst_x: i16,
    pub dst_y: i16,
    pub width: u16,
    pub height: u16,
}
impl_debug_if_no_extra_traits!(CompositeRequest, "CompositeRequest");
impl CompositeRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let op_bytes = u8::from(self.op).serialize();
        let src_bytes = self.src.serialize();
        let mask_bytes = self.mask.serialize();
        let dst_bytes = self.dst.serialize();
        let src_x_bytes = self.src_x.serialize();
        let src_y_bytes = self.src_y.serialize();
        let mask_x_bytes = self.mask_x.serialize();
        let mask_y_bytes = self.mask_y.serialize();
        let dst_x_bytes = self.dst_x.serialize();
        let dst_y_bytes = self.dst_y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let mut request0 = vec![
            major_opcode,
            COMPOSITE_REQUEST,
            0,
            0,
            op_bytes[0],
            0,
            0,
            0,
            src_bytes[0],
            src_bytes[1],
            src_bytes[2],
            src_bytes[3],
            mask_bytes[0],
            mask_bytes[1],
            mask_bytes[2],
            mask_bytes[3],
            dst_bytes[0],
            dst_bytes[1],
            dst_bytes[2],
            dst_bytes[3],
            src_x_bytes[0],
            src_x_bytes[1],
            src_y_bytes[0],
            src_y_bytes[1],
            mask_x_bytes[0],
            mask_x_bytes[1],
            mask_y_bytes[0],
            mask_y_bytes[1],
            dst_x_bytes[0],
            dst_x_bytes[1],
            dst_y_bytes[0],
            dst_y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != COMPOSITE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (op, remaining) = u8::try_parse(value)?;
        let op = op.into();
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (src, remaining) = Picture::try_parse(remaining)?;
        let (mask, remaining) = Picture::try_parse(remaining)?;
        let (dst, remaining) = Picture::try_parse(remaining)?;
        let (src_x, remaining) = i16::try_parse(remaining)?;
        let (src_y, remaining) = i16::try_parse(remaining)?;
        let (mask_x, remaining) = i16::try_parse(remaining)?;
        let (mask_y, remaining) = i16::try_parse(remaining)?;
        let (dst_x, remaining) = i16::try_parse(remaining)?;
        let (dst_y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(CompositeRequest {
            op,
            src,
            mask,
            dst,
            src_x,
            src_y,
            mask_x,
            mask_y,
            dst_x,
            dst_y,
            width,
            height,
        })
    }
}
impl Request for CompositeRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CompositeRequest {
}

/// Opcode for the Trapezoids request
pub const TRAPEZOIDS_REQUEST: u8 = 10;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrapezoidsRequest<'input> {
    pub op: PictOp,
    pub src: Picture,
    pub dst: Picture,
    pub mask_format: Pictformat,
    pub src_x: i16,
    pub src_y: i16,
    pub traps: Cow<'input, [Trapezoid]>,
}
impl_debug_if_no_extra_traits!(TrapezoidsRequest<'_>, "TrapezoidsRequest");
impl<'input> TrapezoidsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let op_bytes = u8::from(self.op).serialize();
        let src_bytes = self.src.serialize();
        let dst_bytes = self.dst.serialize();
        let mask_format_bytes = self.mask_format.serialize();
        let src_x_bytes = self.src_x.serialize();
        let src_y_bytes = self.src_y.serialize();
        let mut request0 = vec![
            major_opcode,
            TRAPEZOIDS_REQUEST,
            0,
            0,
            op_bytes[0],
            0,
            0,
            0,
            src_bytes[0],
            src_bytes[1],
            src_bytes[2],
            src_bytes[3],
            dst_bytes[0],
            dst_bytes[1],
            dst_bytes[2],
            dst_bytes[3],
            mask_format_bytes[0],
            mask_format_bytes[1],
            mask_format_bytes[2],
            mask_format_bytes[3],
            src_x_bytes[0],
            src_x_bytes[1],
            src_y_bytes[0],
            src_y_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let traps_bytes = self.traps.serialize();
        let length_so_far = length_so_far + traps_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), traps_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != TRAPEZOIDS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (op, remaining) = u8::try_parse(value)?;
        let op = op.into();
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (src, remaining) = Picture::try_parse(remaining)?;
        let (dst, remaining) = Picture::try_parse(remaining)?;
        let (mask_format, remaining) = Pictformat::try_parse(remaining)?;
        let (src_x, remaining) = i16::try_parse(remaining)?;
        let (src_y, remaining) = i16::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut traps = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Trapezoid::try_parse(remaining)?;
            remaining = new_remaining;
            traps.push(v);
        }
        let _ = remaining;
        Ok(TrapezoidsRequest {
            op,
            src,
            dst,
            mask_format,
            src_x,
            src_y,
            traps: Cow::Owned(traps),
        })
    }
    /// Clone all borrowed data in this TrapezoidsRequest.
    pub fn into_owned(self) -> TrapezoidsRequest<'static> {
        TrapezoidsRequest {
            op: self.op,
            src: self.src,
            dst: self.dst,
            mask_format: self.mask_format,
            src_x: self.src_x,
            src_y: self.src_y,
            traps: Cow::Owned(self.traps.into_owned()),
        }
    }
}
impl<'input> Request for TrapezoidsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for TrapezoidsRequest<'input> {
}

/// Opcode for the Triangles request
pub const TRIANGLES_REQUEST: u8 = 11;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TrianglesRequest<'input> {
    pub op: PictOp,
    pub src: Picture,
    pub dst: Picture,
    pub mask_format: Pictformat,
    pub src_x: i16,
    pub src_y: i16,
    pub triangles: Cow<'input, [Triangle]>,
}
impl_debug_if_no_extra_traits!(TrianglesRequest<'_>, "TrianglesRequest");
impl<'input> TrianglesRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let op_bytes = u8::from(self.op).serialize();
        let src_bytes = self.src.serialize();
        let dst_bytes = self.dst.serialize();
        let mask_format_bytes = self.mask_format.serialize();
        let src_x_bytes = self.src_x.serialize();
        let src_y_bytes = self.src_y.serialize();
        let mut request0 = vec![
            major_opcode,
            TRIANGLES_REQUEST,
            0,
            0,
            op_bytes[0],
            0,
            0,
            0,
            src_bytes[0],
            src_bytes[1],
            src_bytes[2],
            src_bytes[3],
            dst_bytes[0],
            dst_bytes[1],
            dst_bytes[2],
            dst_bytes[3],
            mask_format_bytes[0],
            mask_format_bytes[1],
            mask_format_bytes[2],
            mask_format_bytes[3],
            src_x_bytes[0],
            src_x_bytes[1],
            src_y_bytes[0],
            src_y_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let triangles_bytes = self.triangles.serialize();
        let length_so_far = length_so_far + triangles_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), triangles_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != TRIANGLES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (op, remaining) = u8::try_parse(value)?;
        let op = op.into();
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (src, remaining) = Picture::try_parse(remaining)?;
        let (dst, remaining) = Picture::try_parse(remaining)?;
        let (mask_format, remaining) = Pictformat::try_parse(remaining)?;
        let (src_x, remaining) = i16::try_parse(remaining)?;
        let (src_y, remaining) = i16::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut triangles = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Triangle::try_parse(remaining)?;
            remaining = new_remaining;
            triangles.push(v);
        }
        let _ = remaining;
        Ok(TrianglesRequest {
            op,
            src,
            dst,
            mask_format,
            src_x,
            src_y,
            triangles: Cow::Owned(triangles),
        })
    }
    /// Clone all borrowed data in this TrianglesRequest.
    pub fn into_owned(self) -> TrianglesRequest<'static> {
        TrianglesRequest {
            op: self.op,
            src: self.src,
            dst: self.dst,
            mask_format: self.mask_format,
            src_x: self.src_x,
            src_y: self.src_y,
            triangles: Cow::Owned(self.triangles.into_owned()),
        }
    }
}
impl<'input> Request for TrianglesRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for TrianglesRequest<'input> {
}

/// Opcode for the TriStrip request
pub const TRI_STRIP_REQUEST: u8 = 12;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TriStripRequest<'input> {
    pub op: PictOp,
    pub src: Picture,
    pub dst: Picture,
    pub mask_format: Pictformat,
    pub src_x: i16,
    pub src_y: i16,
    pub points: Cow<'input, [Pointfix]>,
}
impl_debug_if_no_extra_traits!(TriStripRequest<'_>, "TriStripRequest");
impl<'input> TriStripRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let op_bytes = u8::from(self.op).serialize();
        let src_bytes = self.src.serialize();
        let dst_bytes = self.dst.serialize();
        let mask_format_bytes = self.mask_format.serialize();
        let src_x_bytes = self.src_x.serialize();
        let src_y_bytes = self.src_y.serialize();
        let mut request0 = vec![
            major_opcode,
            TRI_STRIP_REQUEST,
            0,
            0,
            op_bytes[0],
            0,
            0,
            0,
            src_bytes[0],
            src_bytes[1],
            src_bytes[2],
            src_bytes[3],
            dst_bytes[0],
            dst_bytes[1],
            dst_bytes[2],
            dst_bytes[3],
            mask_format_bytes[0],
            mask_format_bytes[1],
            mask_format_bytes[2],
            mask_format_bytes[3],
            src_x_bytes[0],
            src_x_bytes[1],
            src_y_bytes[0],
            src_y_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let points_bytes = self.points.serialize();
        let length_so_far = length_so_far + points_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), points_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != TRI_STRIP_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (op, remaining) = u8::try_parse(value)?;
        let op = op.into();
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (src, remaining) = Picture::try_parse(remaining)?;
        let (dst, remaining) = Picture::try_parse(remaining)?;
        let (mask_format, remaining) = Pictformat::try_parse(remaining)?;
        let (src_x, remaining) = i16::try_parse(remaining)?;
        let (src_y, remaining) = i16::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut points = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Pointfix::try_parse(remaining)?;
            remaining = new_remaining;
            points.push(v);
        }
        let _ = remaining;
        Ok(TriStripRequest {
            op,
            src,
            dst,
            mask_format,
            src_x,
            src_y,
            points: Cow::Owned(points),
        })
    }
    /// Clone all borrowed data in this TriStripRequest.
    pub fn into_owned(self) -> TriStripRequest<'static> {
        TriStripRequest {
            op: self.op,
            src: self.src,
            dst: self.dst,
            mask_format: self.mask_format,
            src_x: self.src_x,
            src_y: self.src_y,
            points: Cow::Owned(self.points.into_owned()),
        }
    }
}
impl<'input> Request for TriStripRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for TriStripRequest<'input> {
}

/// Opcode for the TriFan request
pub const TRI_FAN_REQUEST: u8 = 13;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TriFanRequest<'input> {
    pub op: PictOp,
    pub src: Picture,
    pub dst: Picture,
    pub mask_format: Pictformat,
    pub src_x: i16,
    pub src_y: i16,
    pub points: Cow<'input, [Pointfix]>,
}
impl_debug_if_no_extra_traits!(TriFanRequest<'_>, "TriFanRequest");
impl<'input> TriFanRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let op_bytes = u8::from(self.op).serialize();
        let src_bytes = self.src.serialize();
        let dst_bytes = self.dst.serialize();
        let mask_format_bytes = self.mask_format.serialize();
        let src_x_bytes = self.src_x.serialize();
        let src_y_bytes = self.src_y.serialize();
        let mut request0 = vec![
            major_opcode,
            TRI_FAN_REQUEST,
            0,
            0,
            op_bytes[0],
            0,
            0,
            0,
            src_bytes[0],
            src_bytes[1],
            src_bytes[2],
            src_bytes[3],
            dst_bytes[0],
            dst_bytes[1],
            dst_bytes[2],
            dst_bytes[3],
            mask_format_bytes[0],
            mask_format_bytes[1],
            mask_format_bytes[2],
            mask_format_bytes[3],
            src_x_bytes[0],
            src_x_bytes[1],
            src_y_bytes[0],
            src_y_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let points_bytes = self.points.serialize();
        let length_so_far = length_so_far + points_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), points_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != TRI_FAN_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (op, remaining) = u8::try_parse(value)?;
        let op = op.into();
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (src, remaining) = Picture::try_parse(remaining)?;
        let (dst, remaining) = Picture::try_parse(remaining)?;
        let (mask_format, remaining) = Pictformat::try_parse(remaining)?;
        let (src_x, remaining) = i16::try_parse(remaining)?;
        let (src_y, remaining) = i16::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut points = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Pointfix::try_parse(remaining)?;
            remaining = new_remaining;
            points.push(v);
        }
        let _ = remaining;
        Ok(TriFanRequest {
            op,
            src,
            dst,
            mask_format,
            src_x,
            src_y,
            points: Cow::Owned(points),
        })
    }
    /// Clone all borrowed data in this TriFanRequest.
    pub fn into_owned(self) -> TriFanRequest<'static> {
        TriFanRequest {
            op: self.op,
            src: self.src,
            dst: self.dst,
            mask_format: self.mask_format,
            src_x: self.src_x,
            src_y: self.src_y,
            points: Cow::Owned(self.points.into_owned()),
        }
    }
}
impl<'input> Request for TriFanRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for TriFanRequest<'input> {
}

/// Opcode for the CreateGlyphSet request
pub const CREATE_GLYPH_SET_REQUEST: u8 = 17;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateGlyphSetRequest {
    pub gsid: Glyphset,
    pub format: Pictformat,
}
impl_debug_if_no_extra_traits!(CreateGlyphSetRequest, "CreateGlyphSetRequest");
impl CreateGlyphSetRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let gsid_bytes = self.gsid.serialize();
        let format_bytes = self.format.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_GLYPH_SET_REQUEST,
            0,
            0,
            gsid_bytes[0],
            gsid_bytes[1],
            gsid_bytes[2],
            gsid_bytes[3],
            format_bytes[0],
            format_bytes[1],
            format_bytes[2],
            format_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != CREATE_GLYPH_SET_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (gsid, remaining) = Glyphset::try_parse(value)?;
        let (format, remaining) = Pictformat::try_parse(remaining)?;
        let _ = remaining;
        Ok(CreateGlyphSetRequest {
            gsid,
            format,
        })
    }
}
impl Request for CreateGlyphSetRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CreateGlyphSetRequest {
}

/// Opcode for the ReferenceGlyphSet request
pub const REFERENCE_GLYPH_SET_REQUEST: u8 = 18;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReferenceGlyphSetRequest {
    pub gsid: Glyphset,
    pub existing: Glyphset,
}
impl_debug_if_no_extra_traits!(ReferenceGlyphSetRequest, "ReferenceGlyphSetRequest");
impl ReferenceGlyphSetRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let gsid_bytes = self.gsid.serialize();
        let existing_bytes = self.existing.serialize();
        let mut request0 = vec![
            major_opcode,
            REFERENCE_GLYPH_SET_REQUEST,
            0,
            0,
            gsid_bytes[0],
            gsid_bytes[1],
            gsid_bytes[2],
            gsid_bytes[3],
            existing_bytes[0],
            existing_bytes[1],
            existing_bytes[2],
            existing_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != REFERENCE_GLYPH_SET_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (gsid, remaining) = Glyphset::try_parse(value)?;
        let (existing, remaining) = Glyphset::try_parse(remaining)?;
        let _ = remaining;
        Ok(ReferenceGlyphSetRequest {
            gsid,
            existing,
        })
    }
}
impl Request for ReferenceGlyphSetRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for ReferenceGlyphSetRequest {
}

/// Opcode for the FreeGlyphSet request
pub const FREE_GLYPH_SET_REQUEST: u8 = 19;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FreeGlyphSetRequest {
    pub glyphset: Glyphset,
}
impl_debug_if_no_extra_traits!(FreeGlyphSetRequest, "FreeGlyphSetRequest");
impl FreeGlyphSetRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let glyphset_bytes = self.glyphset.serialize();
        let mut request0 = vec![
            major_opcode,
            FREE_GLYPH_SET_REQUEST,
            0,
            0,
            glyphset_bytes[0],
            glyphset_bytes[1],
            glyphset_bytes[2],
            glyphset_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != FREE_GLYPH_SET_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (glyphset, remaining) = Glyphset::try_parse(value)?;
        let _ = remaining;
        Ok(FreeGlyphSetRequest {
            glyphset,
        })
    }
}
impl Request for FreeGlyphSetRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for FreeGlyphSetRequest {
}

/// Opcode for the AddGlyphs request
pub const ADD_GLYPHS_REQUEST: u8 = 20;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AddGlyphsRequest<'input> {
    pub glyphset: Glyphset,
    pub glyphids: Cow<'input, [u32]>,
    pub glyphs: Cow<'input, [Glyphinfo]>,
    pub data: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(AddGlyphsRequest<'_>, "AddGlyphsRequest");
impl<'input> AddGlyphsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 5]> {
        let length_so_far = 0;
        let glyphset_bytes = self.glyphset.serialize();
        let glyphs_len = u32::try_from(self.glyphids.len()).expect("`glyphids` has too many elements");
        let glyphs_len_bytes = glyphs_len.serialize();
        let mut request0 = vec![
            major_opcode,
            ADD_GLYPHS_REQUEST,
            0,
            0,
            glyphset_bytes[0],
            glyphset_bytes[1],
            glyphset_bytes[2],
            glyphset_bytes[3],
            glyphs_len_bytes[0],
            glyphs_len_bytes[1],
            glyphs_len_bytes[2],
            glyphs_len_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let glyphids_bytes = self.glyphids.serialize();
        let length_so_far = length_so_far + glyphids_bytes.len();
        assert_eq!(self.glyphs.len(), usize::try_from(glyphs_len).unwrap(), "`glyphs` has an incorrect length");
        let glyphs_bytes = self.glyphs.serialize();
        let length_so_far = length_so_far + glyphs_bytes.len();
        let length_so_far = length_so_far + self.data.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), glyphids_bytes.into(), glyphs_bytes.into(), self.data, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != ADD_GLYPHS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (glyphset, remaining) = Glyphset::try_parse(value)?;
        let (glyphs_len, remaining) = u32::try_parse(remaining)?;
        let (glyphids, remaining) = crate::x11_utils::parse_list::<u32>(remaining, glyphs_len.try_to_usize()?)?;
        let (glyphs, remaining) = crate::x11_utils::parse_list::<Glyphinfo>(remaining, glyphs_len.try_to_usize()?)?;
        let (data, remaining) = remaining.split_at(remaining.len());
        let _ = remaining;
        Ok(AddGlyphsRequest {
            glyphset,
            glyphids: Cow::Owned(glyphids),
            glyphs: Cow::Owned(glyphs),
            data: Cow::Borrowed(data),
        })
    }
    /// Clone all borrowed data in this AddGlyphsRequest.
    pub fn into_owned(self) -> AddGlyphsRequest<'static> {
        AddGlyphsRequest {
            glyphset: self.glyphset,
            glyphids: Cow::Owned(self.glyphids.into_owned()),
            glyphs: Cow::Owned(self.glyphs.into_owned()),
            data: Cow::Owned(self.data.into_owned()),
        }
    }
}
impl<'input> Request for AddGlyphsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for AddGlyphsRequest<'input> {
}

/// Opcode for the FreeGlyphs request
pub const FREE_GLYPHS_REQUEST: u8 = 22;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FreeGlyphsRequest<'input> {
    pub glyphset: Glyphset,
    pub glyphs: Cow<'input, [Glyph]>,
}
impl_debug_if_no_extra_traits!(FreeGlyphsRequest<'_>, "FreeGlyphsRequest");
impl<'input> FreeGlyphsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let glyphset_bytes = self.glyphset.serialize();
        let mut request0 = vec![
            major_opcode,
            FREE_GLYPHS_REQUEST,
            0,
            0,
            glyphset_bytes[0],
            glyphset_bytes[1],
            glyphset_bytes[2],
            glyphset_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let glyphs_bytes = self.glyphs.serialize();
        let length_so_far = length_so_far + glyphs_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), glyphs_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != FREE_GLYPHS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (glyphset, remaining) = Glyphset::try_parse(value)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut glyphs = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Glyph::try_parse(remaining)?;
            remaining = new_remaining;
            glyphs.push(v);
        }
        let _ = remaining;
        Ok(FreeGlyphsRequest {
            glyphset,
            glyphs: Cow::Owned(glyphs),
        })
    }
    /// Clone all borrowed data in this FreeGlyphsRequest.
    pub fn into_owned(self) -> FreeGlyphsRequest<'static> {
        FreeGlyphsRequest {
            glyphset: self.glyphset,
            glyphs: Cow::Owned(self.glyphs.into_owned()),
        }
    }
}
impl<'input> Request for FreeGlyphsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for FreeGlyphsRequest<'input> {
}

/// Opcode for the CompositeGlyphs8 request
pub const COMPOSITE_GLYPHS8_REQUEST: u8 = 23;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CompositeGlyphs8Request<'input> {
    pub op: PictOp,
    pub src: Picture,
    pub dst: Picture,
    pub mask_format: Pictformat,
    pub glyphset: Glyphset,
    pub src_x: i16,
    pub src_y: i16,
    pub glyphcmds: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(CompositeGlyphs8Request<'_>, "CompositeGlyphs8Request");
impl<'input> CompositeGlyphs8Request<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let op_bytes = u8::from(self.op).serialize();
        let src_bytes = self.src.serialize();
        let dst_bytes = self.dst.serialize();
        let mask_format_bytes = self.mask_format.serialize();
        let glyphset_bytes = self.glyphset.serialize();
        let src_x_bytes = self.src_x.serialize();
        let src_y_bytes = self.src_y.serialize();
        let mut request0 = vec![
            major_opcode,
            COMPOSITE_GLYPHS8_REQUEST,
            0,
            0,
            op_bytes[0],
            0,
            0,
            0,
            src_bytes[0],
            src_bytes[1],
            src_bytes[2],
            src_bytes[3],
            dst_bytes[0],
            dst_bytes[1],
            dst_bytes[2],
            dst_bytes[3],
            mask_format_bytes[0],
            mask_format_bytes[1],
            mask_format_bytes[2],
            mask_format_bytes[3],
            glyphset_bytes[0],
            glyphset_bytes[1],
            glyphset_bytes[2],
            glyphset_bytes[3],
            src_x_bytes[0],
            src_x_bytes[1],
            src_y_bytes[0],
            src_y_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.glyphcmds.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.glyphcmds, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != COMPOSITE_GLYPHS8_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (op, remaining) = u8::try_parse(value)?;
        let op = op.into();
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (src, remaining) = Picture::try_parse(remaining)?;
        let (dst, remaining) = Picture::try_parse(remaining)?;
        let (mask_format, remaining) = Pictformat::try_parse(remaining)?;
        let (glyphset, remaining) = Glyphset::try_parse(remaining)?;
        let (src_x, remaining) = i16::try_parse(remaining)?;
        let (src_y, remaining) = i16::try_parse(remaining)?;
        let (glyphcmds, remaining) = remaining.split_at(remaining.len());
        let _ = remaining;
        Ok(CompositeGlyphs8Request {
            op,
            src,
            dst,
            mask_format,
            glyphset,
            src_x,
            src_y,
            glyphcmds: Cow::Borrowed(glyphcmds),
        })
    }
    /// Clone all borrowed data in this CompositeGlyphs8Request.
    pub fn into_owned(self) -> CompositeGlyphs8Request<'static> {
        CompositeGlyphs8Request {
            op: self.op,
            src: self.src,
            dst: self.dst,
            mask_format: self.mask_format,
            glyphset: self.glyphset,
            src_x: self.src_x,
            src_y: self.src_y,
            glyphcmds: Cow::Owned(self.glyphcmds.into_owned()),
        }
    }
}
impl<'input> Request for CompositeGlyphs8Request<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for CompositeGlyphs8Request<'input> {
}

/// Opcode for the CompositeGlyphs16 request
pub const COMPOSITE_GLYPHS16_REQUEST: u8 = 24;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CompositeGlyphs16Request<'input> {
    pub op: PictOp,
    pub src: Picture,
    pub dst: Picture,
    pub mask_format: Pictformat,
    pub glyphset: Glyphset,
    pub src_x: i16,
    pub src_y: i16,
    pub glyphcmds: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(CompositeGlyphs16Request<'_>, "CompositeGlyphs16Request");
impl<'input> CompositeGlyphs16Request<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let op_bytes = u8::from(self.op).serialize();
        let src_bytes = self.src.serialize();
        let dst_bytes = self.dst.serialize();
        let mask_format_bytes = self.mask_format.serialize();
        let glyphset_bytes = self.glyphset.serialize();
        let src_x_bytes = self.src_x.serialize();
        let src_y_bytes = self.src_y.serialize();
        let mut request0 = vec![
            major_opcode,
            COMPOSITE_GLYPHS16_REQUEST,
            0,
            0,
            op_bytes[0],
            0,
            0,
            0,
            src_bytes[0],
            src_bytes[1],
            src_bytes[2],
            src_bytes[3],
            dst_bytes[0],
            dst_bytes[1],
            dst_bytes[2],
            dst_bytes[3],
            mask_format_bytes[0],
            mask_format_bytes[1],
            mask_format_bytes[2],
            mask_format_bytes[3],
            glyphset_bytes[0],
            glyphset_bytes[1],
            glyphset_bytes[2],
            glyphset_bytes[3],
            src_x_bytes[0],
            src_x_bytes[1],
            src_y_bytes[0],
            src_y_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.glyphcmds.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.glyphcmds, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != COMPOSITE_GLYPHS16_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (op, remaining) = u8::try_parse(value)?;
        let op = op.into();
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (src, remaining) = Picture::try_parse(remaining)?;
        let (dst, remaining) = Picture::try_parse(remaining)?;
        let (mask_format, remaining) = Pictformat::try_parse(remaining)?;
        let (glyphset, remaining) = Glyphset::try_parse(remaining)?;
        let (src_x, remaining) = i16::try_parse(remaining)?;
        let (src_y, remaining) = i16::try_parse(remaining)?;
        let (glyphcmds, remaining) = remaining.split_at(remaining.len());
        let _ = remaining;
        Ok(CompositeGlyphs16Request {
            op,
            src,
            dst,
            mask_format,
            glyphset,
            src_x,
            src_y,
            glyphcmds: Cow::Borrowed(glyphcmds),
        })
    }
    /// Clone all borrowed data in this CompositeGlyphs16Request.
    pub fn into_owned(self) -> CompositeGlyphs16Request<'static> {
        CompositeGlyphs16Request {
            op: self.op,
            src: self.src,
            dst: self.dst,
            mask_format: self.mask_format,
            glyphset: self.glyphset,
            src_x: self.src_x,
            src_y: self.src_y,
            glyphcmds: Cow::Owned(self.glyphcmds.into_owned()),
        }
    }
}
impl<'input> Request for CompositeGlyphs16Request<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for CompositeGlyphs16Request<'input> {
}

/// Opcode for the CompositeGlyphs32 request
pub const COMPOSITE_GLYPHS32_REQUEST: u8 = 25;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CompositeGlyphs32Request<'input> {
    pub op: PictOp,
    pub src: Picture,
    pub dst: Picture,
    pub mask_format: Pictformat,
    pub glyphset: Glyphset,
    pub src_x: i16,
    pub src_y: i16,
    pub glyphcmds: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(CompositeGlyphs32Request<'_>, "CompositeGlyphs32Request");
impl<'input> CompositeGlyphs32Request<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let op_bytes = u8::from(self.op).serialize();
        let src_bytes = self.src.serialize();
        let dst_bytes = self.dst.serialize();
        let mask_format_bytes = self.mask_format.serialize();
        let glyphset_bytes = self.glyphset.serialize();
        let src_x_bytes = self.src_x.serialize();
        let src_y_bytes = self.src_y.serialize();
        let mut request0 = vec![
            major_opcode,
            COMPOSITE_GLYPHS32_REQUEST,
            0,
            0,
            op_bytes[0],
            0,
            0,
            0,
            src_bytes[0],
            src_bytes[1],
            src_bytes[2],
            src_bytes[3],
            dst_bytes[0],
            dst_bytes[1],
            dst_bytes[2],
            dst_bytes[3],
            mask_format_bytes[0],
            mask_format_bytes[1],
            mask_format_bytes[2],
            mask_format_bytes[3],
            glyphset_bytes[0],
            glyphset_bytes[1],
            glyphset_bytes[2],
            glyphset_bytes[3],
            src_x_bytes[0],
            src_x_bytes[1],
            src_y_bytes[0],
            src_y_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.glyphcmds.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.glyphcmds, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != COMPOSITE_GLYPHS32_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (op, remaining) = u8::try_parse(value)?;
        let op = op.into();
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (src, remaining) = Picture::try_parse(remaining)?;
        let (dst, remaining) = Picture::try_parse(remaining)?;
        let (mask_format, remaining) = Pictformat::try_parse(remaining)?;
        let (glyphset, remaining) = Glyphset::try_parse(remaining)?;
        let (src_x, remaining) = i16::try_parse(remaining)?;
        let (src_y, remaining) = i16::try_parse(remaining)?;
        let (glyphcmds, remaining) = remaining.split_at(remaining.len());
        let _ = remaining;
        Ok(CompositeGlyphs32Request {
            op,
            src,
            dst,
            mask_format,
            glyphset,
            src_x,
            src_y,
            glyphcmds: Cow::Borrowed(glyphcmds),
        })
    }
    /// Clone all borrowed data in this CompositeGlyphs32Request.
    pub fn into_owned(self) -> CompositeGlyphs32Request<'static> {
        CompositeGlyphs32Request {
            op: self.op,
            src: self.src,
            dst: self.dst,
            mask_format: self.mask_format,
            glyphset: self.glyphset,
            src_x: self.src_x,
            src_y: self.src_y,
            glyphcmds: Cow::Owned(self.glyphcmds.into_owned()),
        }
    }
}
impl<'input> Request for CompositeGlyphs32Request<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for CompositeGlyphs32Request<'input> {
}

/// Opcode for the FillRectangles request
pub const FILL_RECTANGLES_REQUEST: u8 = 26;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FillRectanglesRequest<'input> {
    pub op: PictOp,
    pub dst: Picture,
    pub color: Color,
    pub rects: Cow<'input, [xproto::Rectangle]>,
}
impl_debug_if_no_extra_traits!(FillRectanglesRequest<'_>, "FillRectanglesRequest");
impl<'input> FillRectanglesRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let op_bytes = u8::from(self.op).serialize();
        let dst_bytes = self.dst.serialize();
        let color_bytes = self.color.serialize();
        let mut request0 = vec![
            major_opcode,
            FILL_RECTANGLES_REQUEST,
            0,
            0,
            op_bytes[0],
            0,
            0,
            0,
            dst_bytes[0],
            dst_bytes[1],
            dst_bytes[2],
            dst_bytes[3],
            color_bytes[0],
            color_bytes[1],
            color_bytes[2],
            color_bytes[3],
            color_bytes[4],
            color_bytes[5],
            color_bytes[6],
            color_bytes[7],
        ];
        let length_so_far = length_so_far + request0.len();
        let rects_bytes = self.rects.serialize();
        let length_so_far = length_so_far + rects_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), rects_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != FILL_RECTANGLES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (op, remaining) = u8::try_parse(value)?;
        let op = op.into();
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (dst, remaining) = Picture::try_parse(remaining)?;
        let (color, remaining) = Color::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut rects = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = xproto::Rectangle::try_parse(remaining)?;
            remaining = new_remaining;
            rects.push(v);
        }
        let _ = remaining;
        Ok(FillRectanglesRequest {
            op,
            dst,
            color,
            rects: Cow::Owned(rects),
        })
    }
    /// Clone all borrowed data in this FillRectanglesRequest.
    pub fn into_owned(self) -> FillRectanglesRequest<'static> {
        FillRectanglesRequest {
            op: self.op,
            dst: self.dst,
            color: self.color,
            rects: Cow::Owned(self.rects.into_owned()),
        }
    }
}
impl<'input> Request for FillRectanglesRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for FillRectanglesRequest<'input> {
}

/// Opcode for the CreateCursor request
pub const CREATE_CURSOR_REQUEST: u8 = 27;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateCursorRequest {
    pub cid: xproto::Cursor,
    pub source: Picture,
    pub x: u16,
    pub y: u16,
}
impl_debug_if_no_extra_traits!(CreateCursorRequest, "CreateCursorRequest");
impl CreateCursorRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let cid_bytes = self.cid.serialize();
        let source_bytes = self.source.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_CURSOR_REQUEST,
            0,
            0,
            cid_bytes[0],
            cid_bytes[1],
            cid_bytes[2],
            cid_bytes[3],
            source_bytes[0],
            source_bytes[1],
            source_bytes[2],
            source_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != CREATE_CURSOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (cid, remaining) = xproto::Cursor::try_parse(value)?;
        let (source, remaining) = Picture::try_parse(remaining)?;
        let (x, remaining) = u16::try_parse(remaining)?;
        let (y, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(CreateCursorRequest {
            cid,
            source,
            x,
            y,
        })
    }
}
impl Request for CreateCursorRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CreateCursorRequest {
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Transform {
    pub matrix11: Fixed,
    pub matrix12: Fixed,
    pub matrix13: Fixed,
    pub matrix21: Fixed,
    pub matrix22: Fixed,
    pub matrix23: Fixed,
    pub matrix31: Fixed,
    pub matrix32: Fixed,
    pub matrix33: Fixed,
}
impl_debug_if_no_extra_traits!(Transform, "Transform");
impl TryParse for Transform {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (matrix11, remaining) = Fixed::try_parse(remaining)?;
        let (matrix12, remaining) = Fixed::try_parse(remaining)?;
        let (matrix13, remaining) = Fixed::try_parse(remaining)?;
        let (matrix21, remaining) = Fixed::try_parse(remaining)?;
        let (matrix22, remaining) = Fixed::try_parse(remaining)?;
        let (matrix23, remaining) = Fixed::try_parse(remaining)?;
        let (matrix31, remaining) = Fixed::try_parse(remaining)?;
        let (matrix32, remaining) = Fixed::try_parse(remaining)?;
        let (matrix33, remaining) = Fixed::try_parse(remaining)?;
        let result = Transform { matrix11, matrix12, matrix13, matrix21, matrix22, matrix23, matrix31, matrix32, matrix33 };
        Ok((result, remaining))
    }
}
impl Serialize for Transform {
    type Bytes = [u8; 36];
    fn serialize(&self) -> [u8; 36] {
        let matrix11_bytes = self.matrix11.serialize();
        let matrix12_bytes = self.matrix12.serialize();
        let matrix13_bytes = self.matrix13.serialize();
        let matrix21_bytes = self.matrix21.serialize();
        let matrix22_bytes = self.matrix22.serialize();
        let matrix23_bytes = self.matrix23.serialize();
        let matrix31_bytes = self.matrix31.serialize();
        let matrix32_bytes = self.matrix32.serialize();
        let matrix33_bytes = self.matrix33.serialize();
        [
            matrix11_bytes[0],
            matrix11_bytes[1],
            matrix11_bytes[2],
            matrix11_bytes[3],
            matrix12_bytes[0],
            matrix12_bytes[1],
            matrix12_bytes[2],
            matrix12_bytes[3],
            matrix13_bytes[0],
            matrix13_bytes[1],
            matrix13_bytes[2],
            matrix13_bytes[3],
            matrix21_bytes[0],
            matrix21_bytes[1],
            matrix21_bytes[2],
            matrix21_bytes[3],
            matrix22_bytes[0],
            matrix22_bytes[1],
            matrix22_bytes[2],
            matrix22_bytes[3],
            matrix23_bytes[0],
            matrix23_bytes[1],
            matrix23_bytes[2],
            matrix23_bytes[3],
            matrix31_bytes[0],
            matrix31_bytes[1],
            matrix31_bytes[2],
            matrix31_bytes[3],
            matrix32_bytes[0],
            matrix32_bytes[1],
            matrix32_bytes[2],
            matrix32_bytes[3],
            matrix33_bytes[0],
            matrix33_bytes[1],
            matrix33_bytes[2],
            matrix33_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(36);
        self.matrix11.serialize_into(bytes);
        self.matrix12.serialize_into(bytes);
        self.matrix13.serialize_into(bytes);
        self.matrix21.serialize_into(bytes);
        self.matrix22.serialize_into(bytes);
        self.matrix23.serialize_into(bytes);
        self.matrix31.serialize_into(bytes);
        self.matrix32.serialize_into(bytes);
        self.matrix33.serialize_into(bytes);
    }
}

/// Opcode for the SetPictureTransform request
pub const SET_PICTURE_TRANSFORM_REQUEST: u8 = 28;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetPictureTransformRequest {
    pub picture: Picture,
    pub transform: Transform,
}
impl_debug_if_no_extra_traits!(SetPictureTransformRequest, "SetPictureTransformRequest");
impl SetPictureTransformRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let picture_bytes = self.picture.serialize();
        let transform_bytes = self.transform.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_PICTURE_TRANSFORM_REQUEST,
            0,
            0,
            picture_bytes[0],
            picture_bytes[1],
            picture_bytes[2],
            picture_bytes[3],
            transform_bytes[0],
            transform_bytes[1],
            transform_bytes[2],
            transform_bytes[3],
            transform_bytes[4],
            transform_bytes[5],
            transform_bytes[6],
            transform_bytes[7],
            transform_bytes[8],
            transform_bytes[9],
            transform_bytes[10],
            transform_bytes[11],
            transform_bytes[12],
            transform_bytes[13],
            transform_bytes[14],
            transform_bytes[15],
            transform_bytes[16],
            transform_bytes[17],
            transform_bytes[18],
            transform_bytes[19],
            transform_bytes[20],
            transform_bytes[21],
            transform_bytes[22],
            transform_bytes[23],
            transform_bytes[24],
            transform_bytes[25],
            transform_bytes[26],
            transform_bytes[27],
            transform_bytes[28],
            transform_bytes[29],
            transform_bytes[30],
            transform_bytes[31],
            transform_bytes[32],
            transform_bytes[33],
            transform_bytes[34],
            transform_bytes[35],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_PICTURE_TRANSFORM_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (picture, remaining) = Picture::try_parse(value)?;
        let (transform, remaining) = Transform::try_parse(remaining)?;
        let _ = remaining;
        Ok(SetPictureTransformRequest {
            picture,
            transform,
        })
    }
}
impl Request for SetPictureTransformRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SetPictureTransformRequest {
}

/// Opcode for the QueryFilters request
pub const QUERY_FILTERS_REQUEST: u8 = 29;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryFiltersRequest {
    pub drawable: xproto::Drawable,
}
impl_debug_if_no_extra_traits!(QueryFiltersRequest, "QueryFiltersRequest");
impl QueryFiltersRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_FILTERS_REQUEST,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != QUERY_FILTERS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let _ = remaining;
        Ok(QueryFiltersRequest {
            drawable,
        })
    }
}
impl Request for QueryFiltersRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryFiltersRequest {
    type Reply = QueryFiltersReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryFiltersReply {
    pub sequence: u16,
    pub length: u32,
    pub aliases: Vec<u16>,
    pub filters: Vec<xproto::Str>,
}
impl_debug_if_no_extra_traits!(QueryFiltersReply, "QueryFiltersReply");
impl TryParse for QueryFiltersReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (num_aliases, remaining) = u32::try_parse(remaining)?;
        let (num_filters, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(16..).ok_or(ParseError::InsufficientData)?;
        let (aliases, remaining) = crate::x11_utils::parse_list::<u16>(remaining, num_aliases.try_to_usize()?)?;
        let (filters, remaining) = crate::x11_utils::parse_list::<xproto::Str>(remaining, num_filters.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryFiltersReply { sequence, length, aliases, filters };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryFiltersReply {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        let num_aliases = u32::try_from(self.aliases.len()).expect("`aliases` has too many elements");
        num_aliases.serialize_into(bytes);
        let num_filters = u32::try_from(self.filters.len()).expect("`filters` has too many elements");
        num_filters.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 16]);
        self.aliases.serialize_into(bytes);
        self.filters.serialize_into(bytes);
    }
}
impl QueryFiltersReply {
    /// Get the value of the `num_aliases` field.
    ///
    /// The `num_aliases` field is used as the length field of the `aliases` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_aliases(&self) -> u32 {
        self.aliases.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_filters` field.
    ///
    /// The `num_filters` field is used as the length field of the `filters` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_filters(&self) -> u32 {
        self.filters.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SetPictureFilter request
pub const SET_PICTURE_FILTER_REQUEST: u8 = 30;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetPictureFilterRequest<'input> {
    pub picture: Picture,
    pub filter: Cow<'input, [u8]>,
    pub values: Cow<'input, [Fixed]>,
}
impl_debug_if_no_extra_traits!(SetPictureFilterRequest<'_>, "SetPictureFilterRequest");
impl<'input> SetPictureFilterRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 5]> {
        let length_so_far = 0;
        let picture_bytes = self.picture.serialize();
        let filter_len = u16::try_from(self.filter.len()).expect("`filter` has too many elements");
        let filter_len_bytes = filter_len.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_PICTURE_FILTER_REQUEST,
            0,
            0,
            picture_bytes[0],
            picture_bytes[1],
            picture_bytes[2],
            picture_bytes[3],
            filter_len_bytes[0],
            filter_len_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.filter.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        let values_bytes = self.values.serialize();
        let length_so_far = length_so_far + values_bytes.len();
        let padding1 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding1.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.filter, padding0.into(), values_bytes.into(), padding1.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_PICTURE_FILTER_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (picture, remaining) = Picture::try_parse(value)?;
        let (filter_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (filter, remaining) = crate::x11_utils::parse_u8_list(remaining, filter_len.try_to_usize()?)?;
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut values = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Fixed::try_parse(remaining)?;
            remaining = new_remaining;
            values.push(v);
        }
        let _ = remaining;
        Ok(SetPictureFilterRequest {
            picture,
            filter: Cow::Borrowed(filter),
            values: Cow::Owned(values),
        })
    }
    /// Clone all borrowed data in this SetPictureFilterRequest.
    pub fn into_owned(self) -> SetPictureFilterRequest<'static> {
        SetPictureFilterRequest {
            picture: self.picture,
            filter: Cow::Owned(self.filter.into_owned()),
            values: Cow::Owned(self.values.into_owned()),
        }
    }
}
impl<'input> Request for SetPictureFilterRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetPictureFilterRequest<'input> {
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Animcursorelt {
    pub cursor: xproto::Cursor,
    pub delay: u32,
}
impl_debug_if_no_extra_traits!(Animcursorelt, "Animcursorelt");
impl TryParse for Animcursorelt {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (cursor, remaining) = xproto::Cursor::try_parse(remaining)?;
        let (delay, remaining) = u32::try_parse(remaining)?;
        let result = Animcursorelt { cursor, delay };
        Ok((result, remaining))
    }
}
impl Serialize for Animcursorelt {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let cursor_bytes = self.cursor.serialize();
        let delay_bytes = self.delay.serialize();
        [
            cursor_bytes[0],
            cursor_bytes[1],
            cursor_bytes[2],
            cursor_bytes[3],
            delay_bytes[0],
            delay_bytes[1],
            delay_bytes[2],
            delay_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.cursor.serialize_into(bytes);
        self.delay.serialize_into(bytes);
    }
}

/// Opcode for the CreateAnimCursor request
pub const CREATE_ANIM_CURSOR_REQUEST: u8 = 31;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateAnimCursorRequest<'input> {
    pub cid: xproto::Cursor,
    pub cursors: Cow<'input, [Animcursorelt]>,
}
impl_debug_if_no_extra_traits!(CreateAnimCursorRequest<'_>, "CreateAnimCursorRequest");
impl<'input> CreateAnimCursorRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let cid_bytes = self.cid.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_ANIM_CURSOR_REQUEST,
            0,
            0,
            cid_bytes[0],
            cid_bytes[1],
            cid_bytes[2],
            cid_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let cursors_bytes = self.cursors.serialize();
        let length_so_far = length_so_far + cursors_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), cursors_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != CREATE_ANIM_CURSOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (cid, remaining) = xproto::Cursor::try_parse(value)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut cursors = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Animcursorelt::try_parse(remaining)?;
            remaining = new_remaining;
            cursors.push(v);
        }
        let _ = remaining;
        Ok(CreateAnimCursorRequest {
            cid,
            cursors: Cow::Owned(cursors),
        })
    }
    /// Clone all borrowed data in this CreateAnimCursorRequest.
    pub fn into_owned(self) -> CreateAnimCursorRequest<'static> {
        CreateAnimCursorRequest {
            cid: self.cid,
            cursors: Cow::Owned(self.cursors.into_owned()),
        }
    }
}
impl<'input> Request for CreateAnimCursorRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for CreateAnimCursorRequest<'input> {
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Spanfix {
    pub l: Fixed,
    pub r: Fixed,
    pub y: Fixed,
}
impl_debug_if_no_extra_traits!(Spanfix, "Spanfix");
impl TryParse for Spanfix {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (l, remaining) = Fixed::try_parse(remaining)?;
        let (r, remaining) = Fixed::try_parse(remaining)?;
        let (y, remaining) = Fixed::try_parse(remaining)?;
        let result = Spanfix { l, r, y };
        Ok((result, remaining))
    }
}
impl Serialize for Spanfix {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let l_bytes = self.l.serialize();
        let r_bytes = self.r.serialize();
        let y_bytes = self.y.serialize();
        [
            l_bytes[0],
            l_bytes[1],
            l_bytes[2],
            l_bytes[3],
            r_bytes[0],
            r_bytes[1],
            r_bytes[2],
            r_bytes[3],
            y_bytes[0],
            y_bytes[1],
            y_bytes[2],
            y_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.l.serialize_into(bytes);
        self.r.serialize_into(bytes);
        self.y.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Trap {
    pub top: Spanfix,
    pub bot: Spanfix,
}
impl_debug_if_no_extra_traits!(Trap, "Trap");
impl TryParse for Trap {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (top, remaining) = Spanfix::try_parse(remaining)?;
        let (bot, remaining) = Spanfix::try_parse(remaining)?;
        let result = Trap { top, bot };
        Ok((result, remaining))
    }
}
impl Serialize for Trap {
    type Bytes = [u8; 24];
    fn serialize(&self) -> [u8; 24] {
        let top_bytes = self.top.serialize();
        let bot_bytes = self.bot.serialize();
        [
            top_bytes[0],
            top_bytes[1],
            top_bytes[2],
            top_bytes[3],
            top_bytes[4],
            top_bytes[5],
            top_bytes[6],
            top_bytes[7],
            top_bytes[8],
            top_bytes[9],
            top_bytes[10],
            top_bytes[11],
            bot_bytes[0],
            bot_bytes[1],
            bot_bytes[2],
            bot_bytes[3],
            bot_bytes[4],
            bot_bytes[5],
            bot_bytes[6],
            bot_bytes[7],
            bot_bytes[8],
            bot_bytes[9],
            bot_bytes[10],
            bot_bytes[11],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(24);
        self.top.serialize_into(bytes);
        self.bot.serialize_into(bytes);
    }
}

/// Opcode for the AddTraps request
pub const ADD_TRAPS_REQUEST: u8 = 32;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AddTrapsRequest<'input> {
    pub picture: Picture,
    pub x_off: i16,
    pub y_off: i16,
    pub traps: Cow<'input, [Trap]>,
}
impl_debug_if_no_extra_traits!(AddTrapsRequest<'_>, "AddTrapsRequest");
impl<'input> AddTrapsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let picture_bytes = self.picture.serialize();
        let x_off_bytes = self.x_off.serialize();
        let y_off_bytes = self.y_off.serialize();
        let mut request0 = vec![
            major_opcode,
            ADD_TRAPS_REQUEST,
            0,
            0,
            picture_bytes[0],
            picture_bytes[1],
            picture_bytes[2],
            picture_bytes[3],
            x_off_bytes[0],
            x_off_bytes[1],
            y_off_bytes[0],
            y_off_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let traps_bytes = self.traps.serialize();
        let length_so_far = length_so_far + traps_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), traps_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != ADD_TRAPS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (picture, remaining) = Picture::try_parse(value)?;
        let (x_off, remaining) = i16::try_parse(remaining)?;
        let (y_off, remaining) = i16::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut traps = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Trap::try_parse(remaining)?;
            remaining = new_remaining;
            traps.push(v);
        }
        let _ = remaining;
        Ok(AddTrapsRequest {
            picture,
            x_off,
            y_off,
            traps: Cow::Owned(traps),
        })
    }
    /// Clone all borrowed data in this AddTrapsRequest.
    pub fn into_owned(self) -> AddTrapsRequest<'static> {
        AddTrapsRequest {
            picture: self.picture,
            x_off: self.x_off,
            y_off: self.y_off,
            traps: Cow::Owned(self.traps.into_owned()),
        }
    }
}
impl<'input> Request for AddTrapsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for AddTrapsRequest<'input> {
}

/// Opcode for the CreateSolidFill request
pub const CREATE_SOLID_FILL_REQUEST: u8 = 33;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateSolidFillRequest {
    pub picture: Picture,
    pub color: Color,
}
impl_debug_if_no_extra_traits!(CreateSolidFillRequest, "CreateSolidFillRequest");
impl CreateSolidFillRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let picture_bytes = self.picture.serialize();
        let color_bytes = self.color.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_SOLID_FILL_REQUEST,
            0,
            0,
            picture_bytes[0],
            picture_bytes[1],
            picture_bytes[2],
            picture_bytes[3],
            color_bytes[0],
            color_bytes[1],
            color_bytes[2],
            color_bytes[3],
            color_bytes[4],
            color_bytes[5],
            color_bytes[6],
            color_bytes[7],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != CREATE_SOLID_FILL_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (picture, remaining) = Picture::try_parse(value)?;
        let (color, remaining) = Color::try_parse(remaining)?;
        let _ = remaining;
        Ok(CreateSolidFillRequest {
            picture,
            color,
        })
    }
}
impl Request for CreateSolidFillRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CreateSolidFillRequest {
}

/// Opcode for the CreateLinearGradient request
pub const CREATE_LINEAR_GRADIENT_REQUEST: u8 = 34;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateLinearGradientRequest<'input> {
    pub picture: Picture,
    pub p1: Pointfix,
    pub p2: Pointfix,
    pub stops: Cow<'input, [Fixed]>,
    pub colors: Cow<'input, [Color]>,
}
impl_debug_if_no_extra_traits!(CreateLinearGradientRequest<'_>, "CreateLinearGradientRequest");
impl<'input> CreateLinearGradientRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 4]> {
        let length_so_far = 0;
        let picture_bytes = self.picture.serialize();
        let p1_bytes = self.p1.serialize();
        let p2_bytes = self.p2.serialize();
        let num_stops = u32::try_from(self.stops.len()).expect("`stops` has too many elements");
        let num_stops_bytes = num_stops.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_LINEAR_GRADIENT_REQUEST,
            0,
            0,
            picture_bytes[0],
            picture_bytes[1],
            picture_bytes[2],
            picture_bytes[3],
            p1_bytes[0],
            p1_bytes[1],
            p1_bytes[2],
            p1_bytes[3],
            p1_bytes[4],
            p1_bytes[5],
            p1_bytes[6],
            p1_bytes[7],
            p2_bytes[0],
            p2_bytes[1],
            p2_bytes[2],
            p2_bytes[3],
            p2_bytes[4],
            p2_bytes[5],
            p2_bytes[6],
            p2_bytes[7],
            num_stops_bytes[0],
            num_stops_bytes[1],
            num_stops_bytes[2],
            num_stops_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let stops_bytes = self.stops.serialize();
        let length_so_far = length_so_far + stops_bytes.len();
        assert_eq!(self.colors.len(), usize::try_from(num_stops).unwrap(), "`colors` has an incorrect length");
        let colors_bytes = self.colors.serialize();
        let length_so_far = length_so_far + colors_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), stops_bytes.into(), colors_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != CREATE_LINEAR_GRADIENT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (picture, remaining) = Picture::try_parse(value)?;
        let (p1, remaining) = Pointfix::try_parse(remaining)?;
        let (p2, remaining) = Pointfix::try_parse(remaining)?;
        let (num_stops, remaining) = u32::try_parse(remaining)?;
        let (stops, remaining) = crate::x11_utils::parse_list::<Fixed>(remaining, num_stops.try_to_usize()?)?;
        let (colors, remaining) = crate::x11_utils::parse_list::<Color>(remaining, num_stops.try_to_usize()?)?;
        let _ = remaining;
        Ok(CreateLinearGradientRequest {
            picture,
            p1,
            p2,
            stops: Cow::Owned(stops),
            colors: Cow::Owned(colors),
        })
    }
    /// Clone all borrowed data in this CreateLinearGradientRequest.
    pub fn into_owned(self) -> CreateLinearGradientRequest<'static> {
        CreateLinearGradientRequest {
            picture: self.picture,
            p1: self.p1,
            p2: self.p2,
            stops: Cow::Owned(self.stops.into_owned()),
            colors: Cow::Owned(self.colors.into_owned()),
        }
    }
}
impl<'input> Request for CreateLinearGradientRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for CreateLinearGradientRequest<'input> {
}

/// Opcode for the CreateRadialGradient request
pub const CREATE_RADIAL_GRADIENT_REQUEST: u8 = 35;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateRadialGradientRequest<'input> {
    pub picture: Picture,
    pub inner: Pointfix,
    pub outer: Pointfix,
    pub inner_radius: Fixed,
    pub outer_radius: Fixed,
    pub stops: Cow<'input, [Fixed]>,
    pub colors: Cow<'input, [Color]>,
}
impl_debug_if_no_extra_traits!(CreateRadialGradientRequest<'_>, "CreateRadialGradientRequest");
impl<'input> CreateRadialGradientRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 4]> {
        let length_so_far = 0;
        let picture_bytes = self.picture.serialize();
        let inner_bytes = self.inner.serialize();
        let outer_bytes = self.outer.serialize();
        let inner_radius_bytes = self.inner_radius.serialize();
        let outer_radius_bytes = self.outer_radius.serialize();
        let num_stops = u32::try_from(self.stops.len()).expect("`stops` has too many elements");
        let num_stops_bytes = num_stops.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_RADIAL_GRADIENT_REQUEST,
            0,
            0,
            picture_bytes[0],
            picture_bytes[1],
            picture_bytes[2],
            picture_bytes[3],
            inner_bytes[0],
            inner_bytes[1],
            inner_bytes[2],
            inner_bytes[3],
            inner_bytes[4],
            inner_bytes[5],
            inner_bytes[6],
            inner_bytes[7],
            outer_bytes[0],
            outer_bytes[1],
            outer_bytes[2],
            outer_bytes[3],
            outer_bytes[4],
            outer_bytes[5],
            outer_bytes[6],
            outer_bytes[7],
            inner_radius_bytes[0],
            inner_radius_bytes[1],
            inner_radius_bytes[2],
            inner_radius_bytes[3],
            outer_radius_bytes[0],
            outer_radius_bytes[1],
            outer_radius_bytes[2],
            outer_radius_bytes[3],
            num_stops_bytes[0],
            num_stops_bytes[1],
            num_stops_bytes[2],
            num_stops_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let stops_bytes = self.stops.serialize();
        let length_so_far = length_so_far + stops_bytes.len();
        assert_eq!(self.colors.len(), usize::try_from(num_stops).unwrap(), "`colors` has an incorrect length");
        let colors_bytes = self.colors.serialize();
        let length_so_far = length_so_far + colors_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), stops_bytes.into(), colors_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != CREATE_RADIAL_GRADIENT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (picture, remaining) = Picture::try_parse(value)?;
        let (inner, remaining) = Pointfix::try_parse(remaining)?;
        let (outer, remaining) = Pointfix::try_parse(remaining)?;
        let (inner_radius, remaining) = Fixed::try_parse(remaining)?;
        let (outer_radius, remaining) = Fixed::try_parse(remaining)?;
        let (num_stops, remaining) = u32::try_parse(remaining)?;
        let (stops, remaining) = crate::x11_utils::parse_list::<Fixed>(remaining, num_stops.try_to_usize()?)?;
        let (colors, remaining) = crate::x11_utils::parse_list::<Color>(remaining, num_stops.try_to_usize()?)?;
        let _ = remaining;
        Ok(CreateRadialGradientRequest {
            picture,
            inner,
            outer,
            inner_radius,
            outer_radius,
            stops: Cow::Owned(stops),
            colors: Cow::Owned(colors),
        })
    }
    /// Clone all borrowed data in this CreateRadialGradientRequest.
    pub fn into_owned(self) -> CreateRadialGradientRequest<'static> {
        CreateRadialGradientRequest {
            picture: self.picture,
            inner: self.inner,
            outer: self.outer,
            inner_radius: self.inner_radius,
            outer_radius: self.outer_radius,
            stops: Cow::Owned(self.stops.into_owned()),
            colors: Cow::Owned(self.colors.into_owned()),
        }
    }
}
impl<'input> Request for CreateRadialGradientRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for CreateRadialGradientRequest<'input> {
}

/// Opcode for the CreateConicalGradient request
pub const CREATE_CONICAL_GRADIENT_REQUEST: u8 = 36;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateConicalGradientRequest<'input> {
    pub picture: Picture,
    pub center: Pointfix,
    pub angle: Fixed,
    pub stops: Cow<'input, [Fixed]>,
    pub colors: Cow<'input, [Color]>,
}
impl_debug_if_no_extra_traits!(CreateConicalGradientRequest<'_>, "CreateConicalGradientRequest");
impl<'input> CreateConicalGradientRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 4]> {
        let length_so_far = 0;
        let picture_bytes = self.picture.serialize();
        let center_bytes = self.center.serialize();
        let angle_bytes = self.angle.serialize();
        let num_stops = u32::try_from(self.stops.len()).expect("`stops` has too many elements");
        let num_stops_bytes = num_stops.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_CONICAL_GRADIENT_REQUEST,
            0,
            0,
            picture_bytes[0],
            picture_bytes[1],
            picture_bytes[2],
            picture_bytes[3],
            center_bytes[0],
            center_bytes[1],
            center_bytes[2],
            center_bytes[3],
            center_bytes[4],
            center_bytes[5],
            center_bytes[6],
            center_bytes[7],
            angle_bytes[0],
            angle_bytes[1],
            angle_bytes[2],
            angle_bytes[3],
            num_stops_bytes[0],
            num_stops_bytes[1],
            num_stops_bytes[2],
            num_stops_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let stops_bytes = self.stops.serialize();
        let length_so_far = length_so_far + stops_bytes.len();
        assert_eq!(self.colors.len(), usize::try_from(num_stops).unwrap(), "`colors` has an incorrect length");
        let colors_bytes = self.colors.serialize();
        let length_so_far = length_so_far + colors_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), stops_bytes.into(), colors_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != CREATE_CONICAL_GRADIENT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (picture, remaining) = Picture::try_parse(value)?;
        let (center, remaining) = Pointfix::try_parse(remaining)?;
        let (angle, remaining) = Fixed::try_parse(remaining)?;
        let (num_stops, remaining) = u32::try_parse(remaining)?;
        let (stops, remaining) = crate::x11_utils::parse_list::<Fixed>(remaining, num_stops.try_to_usize()?)?;
        let (colors, remaining) = crate::x11_utils::parse_list::<Color>(remaining, num_stops.try_to_usize()?)?;
        let _ = remaining;
        Ok(CreateConicalGradientRequest {
            picture,
            center,
            angle,
            stops: Cow::Owned(stops),
            colors: Cow::Owned(colors),
        })
    }
    /// Clone all borrowed data in this CreateConicalGradientRequest.
    pub fn into_owned(self) -> CreateConicalGradientRequest<'static> {
        CreateConicalGradientRequest {
            picture: self.picture,
            center: self.center,
            angle: self.angle,
            stops: Cow::Owned(self.stops.into_owned()),
            colors: Cow::Owned(self.colors.into_owned()),
        }
    }
}
impl<'input> Request for CreateConicalGradientRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for CreateConicalGradientRequest<'input> {
}

