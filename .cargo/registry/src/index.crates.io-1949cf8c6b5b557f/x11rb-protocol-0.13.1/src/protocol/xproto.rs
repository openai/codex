// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the core X11 protocol.
//!
//! For more documentation on the X11 protocol, see the
//! [protocol reference manual](https://www.x.org/releases/X11R7.6/doc/xproto/x11protocol.html).
//! This is especially recommended for looking up the exact semantics of
//! specific errors, events, or requests.

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

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Char2b {
    pub byte1: u8,
    pub byte2: u8,
}
impl_debug_if_no_extra_traits!(Char2b, "Char2b");
impl TryParse for Char2b {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (byte1, remaining) = u8::try_parse(remaining)?;
        let (byte2, remaining) = u8::try_parse(remaining)?;
        let result = Char2b { byte1, byte2 };
        Ok((result, remaining))
    }
}
impl Serialize for Char2b {
    type Bytes = [u8; 2];
    fn serialize(&self) -> [u8; 2] {
        let byte1_bytes = self.byte1.serialize();
        let byte2_bytes = self.byte2.serialize();
        [
            byte1_bytes[0],
            byte2_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(2);
        self.byte1.serialize_into(bytes);
        self.byte2.serialize_into(bytes);
    }
}

pub type Window = u32;

pub type Pixmap = u32;

pub type Cursor = u32;

pub type Font = u32;

pub type Gcontext = u32;

pub type Colormap = u32;

pub type Atom = u32;

pub type Drawable = u32;

pub type Fontable = u32;

pub type Bool32 = u32;

pub type Visualid = u32;

pub type Timestamp = u32;

pub type Keysym = u32;

pub type Keycode = u8;

pub type Keycode32 = u32;

pub type Button = u8;

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Point {
    pub x: i16,
    pub y: i16,
}
impl_debug_if_no_extra_traits!(Point, "Point");
impl TryParse for Point {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let result = Point { x, y };
        Ok((result, remaining))
    }
}
impl Serialize for Point {
    type Bytes = [u8; 4];
    fn serialize(&self) -> [u8; 4] {
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        [
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rectangle {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}
impl_debug_if_no_extra_traits!(Rectangle, "Rectangle");
impl TryParse for Rectangle {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let result = Rectangle { x, y, width, height };
        Ok((result, remaining))
    }
}
impl Serialize for Rectangle {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        [
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Arc {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub angle1: i16,
    pub angle2: i16,
}
impl_debug_if_no_extra_traits!(Arc, "Arc");
impl TryParse for Arc {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (angle1, remaining) = i16::try_parse(remaining)?;
        let (angle2, remaining) = i16::try_parse(remaining)?;
        let result = Arc { x, y, width, height, angle1, angle2 };
        Ok((result, remaining))
    }
}
impl Serialize for Arc {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let angle1_bytes = self.angle1.serialize();
        let angle2_bytes = self.angle2.serialize();
        [
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            angle1_bytes[0],
            angle1_bytes[1],
            angle2_bytes[0],
            angle2_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        self.angle1.serialize_into(bytes);
        self.angle2.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Format {
    pub depth: u8,
    pub bits_per_pixel: u8,
    pub scanline_pad: u8,
}
impl_debug_if_no_extra_traits!(Format, "Format");
impl TryParse for Format {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (depth, remaining) = u8::try_parse(remaining)?;
        let (bits_per_pixel, remaining) = u8::try_parse(remaining)?;
        let (scanline_pad, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(5..).ok_or(ParseError::InsufficientData)?;
        let result = Format { depth, bits_per_pixel, scanline_pad };
        Ok((result, remaining))
    }
}
impl Serialize for Format {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let depth_bytes = self.depth.serialize();
        let bits_per_pixel_bytes = self.bits_per_pixel.serialize();
        let scanline_pad_bytes = self.scanline_pad.serialize();
        [
            depth_bytes[0],
            bits_per_pixel_bytes[0],
            scanline_pad_bytes[0],
            0,
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.depth.serialize_into(bytes);
        self.bits_per_pixel.serialize_into(bytes);
        self.scanline_pad.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 5]);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VisualClass(u8);
impl VisualClass {
    pub const STATIC_GRAY: Self = Self(0);
    pub const GRAY_SCALE: Self = Self(1);
    pub const STATIC_COLOR: Self = Self(2);
    pub const PSEUDO_COLOR: Self = Self(3);
    pub const TRUE_COLOR: Self = Self(4);
    pub const DIRECT_COLOR: Self = Self(5);
}
impl From<VisualClass> for u8 {
    #[inline]
    fn from(input: VisualClass) -> Self {
        input.0
    }
}
impl From<VisualClass> for Option<u8> {
    #[inline]
    fn from(input: VisualClass) -> Self {
        Some(input.0)
    }
}
impl From<VisualClass> for u16 {
    #[inline]
    fn from(input: VisualClass) -> Self {
        u16::from(input.0)
    }
}
impl From<VisualClass> for Option<u16> {
    #[inline]
    fn from(input: VisualClass) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<VisualClass> for u32 {
    #[inline]
    fn from(input: VisualClass) -> Self {
        u32::from(input.0)
    }
}
impl From<VisualClass> for Option<u32> {
    #[inline]
    fn from(input: VisualClass) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for VisualClass {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for VisualClass  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::STATIC_GRAY.0.into(), "STATIC_GRAY", "StaticGray"),
            (Self::GRAY_SCALE.0.into(), "GRAY_SCALE", "GrayScale"),
            (Self::STATIC_COLOR.0.into(), "STATIC_COLOR", "StaticColor"),
            (Self::PSEUDO_COLOR.0.into(), "PSEUDO_COLOR", "PseudoColor"),
            (Self::TRUE_COLOR.0.into(), "TRUE_COLOR", "TrueColor"),
            (Self::DIRECT_COLOR.0.into(), "DIRECT_COLOR", "DirectColor"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Visualtype {
    pub visual_id: Visualid,
    pub class: VisualClass,
    pub bits_per_rgb_value: u8,
    pub colormap_entries: u16,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
}
impl_debug_if_no_extra_traits!(Visualtype, "Visualtype");
impl TryParse for Visualtype {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (visual_id, remaining) = Visualid::try_parse(remaining)?;
        let (class, remaining) = u8::try_parse(remaining)?;
        let (bits_per_rgb_value, remaining) = u8::try_parse(remaining)?;
        let (colormap_entries, remaining) = u16::try_parse(remaining)?;
        let (red_mask, remaining) = u32::try_parse(remaining)?;
        let (green_mask, remaining) = u32::try_parse(remaining)?;
        let (blue_mask, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let class = class.into();
        let result = Visualtype { visual_id, class, bits_per_rgb_value, colormap_entries, red_mask, green_mask, blue_mask };
        Ok((result, remaining))
    }
}
impl Serialize for Visualtype {
    type Bytes = [u8; 24];
    fn serialize(&self) -> [u8; 24] {
        let visual_id_bytes = self.visual_id.serialize();
        let class_bytes = u8::from(self.class).serialize();
        let bits_per_rgb_value_bytes = self.bits_per_rgb_value.serialize();
        let colormap_entries_bytes = self.colormap_entries.serialize();
        let red_mask_bytes = self.red_mask.serialize();
        let green_mask_bytes = self.green_mask.serialize();
        let blue_mask_bytes = self.blue_mask.serialize();
        [
            visual_id_bytes[0],
            visual_id_bytes[1],
            visual_id_bytes[2],
            visual_id_bytes[3],
            class_bytes[0],
            bits_per_rgb_value_bytes[0],
            colormap_entries_bytes[0],
            colormap_entries_bytes[1],
            red_mask_bytes[0],
            red_mask_bytes[1],
            red_mask_bytes[2],
            red_mask_bytes[3],
            green_mask_bytes[0],
            green_mask_bytes[1],
            green_mask_bytes[2],
            green_mask_bytes[3],
            blue_mask_bytes[0],
            blue_mask_bytes[1],
            blue_mask_bytes[2],
            blue_mask_bytes[3],
            0,
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(24);
        self.visual_id.serialize_into(bytes);
        u8::from(self.class).serialize_into(bytes);
        self.bits_per_rgb_value.serialize_into(bytes);
        self.colormap_entries.serialize_into(bytes);
        self.red_mask.serialize_into(bytes);
        self.green_mask.serialize_into(bytes);
        self.blue_mask.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Depth {
    pub depth: u8,
    pub visuals: Vec<Visualtype>,
}
impl_debug_if_no_extra_traits!(Depth, "Depth");
impl TryParse for Depth {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (depth, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (visuals_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let (visuals, remaining) = crate::x11_utils::parse_list::<Visualtype>(remaining, visuals_len.try_to_usize()?)?;
        let result = Depth { depth, visuals };
        Ok((result, remaining))
    }
}
impl Serialize for Depth {
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
        let visuals_len = u16::try_from(self.visuals.len()).expect("`visuals` has too many elements");
        visuals_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
        self.visuals.serialize_into(bytes);
    }
}
impl Depth {
    /// Get the value of the `visuals_len` field.
    ///
    /// The `visuals_len` field is used as the length field of the `visuals` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn visuals_len(&self) -> u16 {
        self.visuals.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventMask(u32);
impl EventMask {
    pub const NO_EVENT: Self = Self(0);
    pub const KEY_PRESS: Self = Self(1 << 0);
    pub const KEY_RELEASE: Self = Self(1 << 1);
    pub const BUTTON_PRESS: Self = Self(1 << 2);
    pub const BUTTON_RELEASE: Self = Self(1 << 3);
    pub const ENTER_WINDOW: Self = Self(1 << 4);
    pub const LEAVE_WINDOW: Self = Self(1 << 5);
    pub const POINTER_MOTION: Self = Self(1 << 6);
    pub const POINTER_MOTION_HINT: Self = Self(1 << 7);
    pub const BUTTON1_MOTION: Self = Self(1 << 8);
    pub const BUTTON2_MOTION: Self = Self(1 << 9);
    pub const BUTTON3_MOTION: Self = Self(1 << 10);
    pub const BUTTON4_MOTION: Self = Self(1 << 11);
    pub const BUTTON5_MOTION: Self = Self(1 << 12);
    pub const BUTTON_MOTION: Self = Self(1 << 13);
    pub const KEYMAP_STATE: Self = Self(1 << 14);
    pub const EXPOSURE: Self = Self(1 << 15);
    pub const VISIBILITY_CHANGE: Self = Self(1 << 16);
    pub const STRUCTURE_NOTIFY: Self = Self(1 << 17);
    pub const RESIZE_REDIRECT: Self = Self(1 << 18);
    pub const SUBSTRUCTURE_NOTIFY: Self = Self(1 << 19);
    pub const SUBSTRUCTURE_REDIRECT: Self = Self(1 << 20);
    pub const FOCUS_CHANGE: Self = Self(1 << 21);
    pub const PROPERTY_CHANGE: Self = Self(1 << 22);
    pub const COLOR_MAP_CHANGE: Self = Self(1 << 23);
    pub const OWNER_GRAB_BUTTON: Self = Self(1 << 24);
}
impl From<EventMask> for u32 {
    #[inline]
    fn from(input: EventMask) -> Self {
        input.0
    }
}
impl From<EventMask> for Option<u32> {
    #[inline]
    fn from(input: EventMask) -> Self {
        Some(input.0)
    }
}
impl From<u8> for EventMask {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for EventMask {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for EventMask {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for EventMask  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NO_EVENT.0, "NO_EVENT", "NoEvent"),
            (Self::KEY_PRESS.0, "KEY_PRESS", "KeyPress"),
            (Self::KEY_RELEASE.0, "KEY_RELEASE", "KeyRelease"),
            (Self::BUTTON_PRESS.0, "BUTTON_PRESS", "ButtonPress"),
            (Self::BUTTON_RELEASE.0, "BUTTON_RELEASE", "ButtonRelease"),
            (Self::ENTER_WINDOW.0, "ENTER_WINDOW", "EnterWindow"),
            (Self::LEAVE_WINDOW.0, "LEAVE_WINDOW", "LeaveWindow"),
            (Self::POINTER_MOTION.0, "POINTER_MOTION", "PointerMotion"),
            (Self::POINTER_MOTION_HINT.0, "POINTER_MOTION_HINT", "PointerMotionHint"),
            (Self::BUTTON1_MOTION.0, "BUTTON1_MOTION", "Button1Motion"),
            (Self::BUTTON2_MOTION.0, "BUTTON2_MOTION", "Button2Motion"),
            (Self::BUTTON3_MOTION.0, "BUTTON3_MOTION", "Button3Motion"),
            (Self::BUTTON4_MOTION.0, "BUTTON4_MOTION", "Button4Motion"),
            (Self::BUTTON5_MOTION.0, "BUTTON5_MOTION", "Button5Motion"),
            (Self::BUTTON_MOTION.0, "BUTTON_MOTION", "ButtonMotion"),
            (Self::KEYMAP_STATE.0, "KEYMAP_STATE", "KeymapState"),
            (Self::EXPOSURE.0, "EXPOSURE", "Exposure"),
            (Self::VISIBILITY_CHANGE.0, "VISIBILITY_CHANGE", "VisibilityChange"),
            (Self::STRUCTURE_NOTIFY.0, "STRUCTURE_NOTIFY", "StructureNotify"),
            (Self::RESIZE_REDIRECT.0, "RESIZE_REDIRECT", "ResizeRedirect"),
            (Self::SUBSTRUCTURE_NOTIFY.0, "SUBSTRUCTURE_NOTIFY", "SubstructureNotify"),
            (Self::SUBSTRUCTURE_REDIRECT.0, "SUBSTRUCTURE_REDIRECT", "SubstructureRedirect"),
            (Self::FOCUS_CHANGE.0, "FOCUS_CHANGE", "FocusChange"),
            (Self::PROPERTY_CHANGE.0, "PROPERTY_CHANGE", "PropertyChange"),
            (Self::COLOR_MAP_CHANGE.0, "COLOR_MAP_CHANGE", "ColorMapChange"),
            (Self::OWNER_GRAB_BUTTON.0, "OWNER_GRAB_BUTTON", "OwnerGrabButton"),
        ];
        pretty_print_bitmask(fmt, self.0, &variants)
    }
}
bitmask_binop!(EventMask, u32);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BackingStore(u32);
impl BackingStore {
    pub const NOT_USEFUL: Self = Self(0);
    pub const WHEN_MAPPED: Self = Self(1);
    pub const ALWAYS: Self = Self(2);
}
impl From<BackingStore> for u32 {
    #[inline]
    fn from(input: BackingStore) -> Self {
        input.0
    }
}
impl From<BackingStore> for Option<u32> {
    #[inline]
    fn from(input: BackingStore) -> Self {
        Some(input.0)
    }
}
impl From<u8> for BackingStore {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for BackingStore {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for BackingStore {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for BackingStore  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NOT_USEFUL.0, "NOT_USEFUL", "NotUseful"),
            (Self::WHEN_MAPPED.0, "WHEN_MAPPED", "WhenMapped"),
            (Self::ALWAYS.0, "ALWAYS", "Always"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Screen {
    pub root: Window,
    pub default_colormap: Colormap,
    pub white_pixel: u32,
    pub black_pixel: u32,
    pub current_input_masks: EventMask,
    pub width_in_pixels: u16,
    pub height_in_pixels: u16,
    pub width_in_millimeters: u16,
    pub height_in_millimeters: u16,
    pub min_installed_maps: u16,
    pub max_installed_maps: u16,
    pub root_visual: Visualid,
    pub backing_stores: BackingStore,
    pub save_unders: bool,
    pub root_depth: u8,
    pub allowed_depths: Vec<Depth>,
}
impl_debug_if_no_extra_traits!(Screen, "Screen");
impl TryParse for Screen {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (root, remaining) = Window::try_parse(remaining)?;
        let (default_colormap, remaining) = Colormap::try_parse(remaining)?;
        let (white_pixel, remaining) = u32::try_parse(remaining)?;
        let (black_pixel, remaining) = u32::try_parse(remaining)?;
        let (current_input_masks, remaining) = u32::try_parse(remaining)?;
        let (width_in_pixels, remaining) = u16::try_parse(remaining)?;
        let (height_in_pixels, remaining) = u16::try_parse(remaining)?;
        let (width_in_millimeters, remaining) = u16::try_parse(remaining)?;
        let (height_in_millimeters, remaining) = u16::try_parse(remaining)?;
        let (min_installed_maps, remaining) = u16::try_parse(remaining)?;
        let (max_installed_maps, remaining) = u16::try_parse(remaining)?;
        let (root_visual, remaining) = Visualid::try_parse(remaining)?;
        let (backing_stores, remaining) = u8::try_parse(remaining)?;
        let (save_unders, remaining) = bool::try_parse(remaining)?;
        let (root_depth, remaining) = u8::try_parse(remaining)?;
        let (allowed_depths_len, remaining) = u8::try_parse(remaining)?;
        let (allowed_depths, remaining) = crate::x11_utils::parse_list::<Depth>(remaining, allowed_depths_len.try_to_usize()?)?;
        let current_input_masks = current_input_masks.into();
        let backing_stores = backing_stores.into();
        let result = Screen { root, default_colormap, white_pixel, black_pixel, current_input_masks, width_in_pixels, height_in_pixels, width_in_millimeters, height_in_millimeters, min_installed_maps, max_installed_maps, root_visual, backing_stores, save_unders, root_depth, allowed_depths };
        Ok((result, remaining))
    }
}
impl Serialize for Screen {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(40);
        self.root.serialize_into(bytes);
        self.default_colormap.serialize_into(bytes);
        self.white_pixel.serialize_into(bytes);
        self.black_pixel.serialize_into(bytes);
        u32::from(self.current_input_masks).serialize_into(bytes);
        self.width_in_pixels.serialize_into(bytes);
        self.height_in_pixels.serialize_into(bytes);
        self.width_in_millimeters.serialize_into(bytes);
        self.height_in_millimeters.serialize_into(bytes);
        self.min_installed_maps.serialize_into(bytes);
        self.max_installed_maps.serialize_into(bytes);
        self.root_visual.serialize_into(bytes);
        (u32::from(self.backing_stores) as u8).serialize_into(bytes);
        self.save_unders.serialize_into(bytes);
        self.root_depth.serialize_into(bytes);
        let allowed_depths_len = u8::try_from(self.allowed_depths.len()).expect("`allowed_depths` has too many elements");
        allowed_depths_len.serialize_into(bytes);
        self.allowed_depths.serialize_into(bytes);
    }
}
impl Screen {
    /// Get the value of the `allowed_depths_len` field.
    ///
    /// The `allowed_depths_len` field is used as the length field of the `allowed_depths` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn allowed_depths_len(&self) -> u8 {
        self.allowed_depths.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetupRequest {
    pub byte_order: u8,
    pub protocol_major_version: u16,
    pub protocol_minor_version: u16,
    pub authorization_protocol_name: Vec<u8>,
    pub authorization_protocol_data: Vec<u8>,
}
impl_debug_if_no_extra_traits!(SetupRequest, "SetupRequest");
impl TryParse for SetupRequest {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let value = remaining;
        let (byte_order, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (protocol_major_version, remaining) = u16::try_parse(remaining)?;
        let (protocol_minor_version, remaining) = u16::try_parse(remaining)?;
        let (authorization_protocol_name_len, remaining) = u16::try_parse(remaining)?;
        let (authorization_protocol_data_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (authorization_protocol_name, remaining) = crate::x11_utils::parse_u8_list(remaining, authorization_protocol_name_len.try_to_usize()?)?;
        let authorization_protocol_name = authorization_protocol_name.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let (authorization_protocol_data, remaining) = crate::x11_utils::parse_u8_list(remaining, authorization_protocol_data_len.try_to_usize()?)?;
        let authorization_protocol_data = authorization_protocol_data.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let result = SetupRequest { byte_order, protocol_major_version, protocol_minor_version, authorization_protocol_name, authorization_protocol_data };
        Ok((result, remaining))
    }
}
impl Serialize for SetupRequest {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.byte_order.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.protocol_major_version.serialize_into(bytes);
        self.protocol_minor_version.serialize_into(bytes);
        let authorization_protocol_name_len = u16::try_from(self.authorization_protocol_name.len()).expect("`authorization_protocol_name` has too many elements");
        authorization_protocol_name_len.serialize_into(bytes);
        let authorization_protocol_data_len = u16::try_from(self.authorization_protocol_data.len()).expect("`authorization_protocol_data` has too many elements");
        authorization_protocol_data_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        bytes.extend_from_slice(&self.authorization_protocol_name);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        bytes.extend_from_slice(&self.authorization_protocol_data);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
    }
}
impl SetupRequest {
    /// Get the value of the `authorization_protocol_name_len` field.
    ///
    /// The `authorization_protocol_name_len` field is used as the length field of the `authorization_protocol_name` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn authorization_protocol_name_len(&self) -> u16 {
        self.authorization_protocol_name.len()
            .try_into().unwrap()
    }
    /// Get the value of the `authorization_protocol_data_len` field.
    ///
    /// The `authorization_protocol_data_len` field is used as the length field of the `authorization_protocol_data` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn authorization_protocol_data_len(&self) -> u16 {
        self.authorization_protocol_data.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetupFailed {
    pub status: u8,
    pub protocol_major_version: u16,
    pub protocol_minor_version: u16,
    pub length: u16,
    pub reason: Vec<u8>,
}
impl_debug_if_no_extra_traits!(SetupFailed, "SetupFailed");
impl TryParse for SetupFailed {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (status, remaining) = u8::try_parse(remaining)?;
        let (reason_len, remaining) = u8::try_parse(remaining)?;
        let (protocol_major_version, remaining) = u16::try_parse(remaining)?;
        let (protocol_minor_version, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u16::try_parse(remaining)?;
        let (reason, remaining) = crate::x11_utils::parse_u8_list(remaining, reason_len.try_to_usize()?)?;
        let reason = reason.to_vec();
        let result = SetupFailed { status, protocol_major_version, protocol_minor_version, length, reason };
        Ok((result, remaining))
    }
}
impl Serialize for SetupFailed {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.status.serialize_into(bytes);
        let reason_len = u8::try_from(self.reason.len()).expect("`reason` has too many elements");
        reason_len.serialize_into(bytes);
        self.protocol_major_version.serialize_into(bytes);
        self.protocol_minor_version.serialize_into(bytes);
        self.length.serialize_into(bytes);
        bytes.extend_from_slice(&self.reason);
    }
}
impl SetupFailed {
    /// Get the value of the `reason_len` field.
    ///
    /// The `reason_len` field is used as the length field of the `reason` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn reason_len(&self) -> u8 {
        self.reason.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetupAuthenticate {
    pub status: u8,
    pub reason: Vec<u8>,
}
impl_debug_if_no_extra_traits!(SetupAuthenticate, "SetupAuthenticate");
impl TryParse for SetupAuthenticate {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (status, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(5..).ok_or(ParseError::InsufficientData)?;
        let (length, remaining) = u16::try_parse(remaining)?;
        let (reason, remaining) = crate::x11_utils::parse_u8_list(remaining, u32::from(length).checked_mul(4u32).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let reason = reason.to_vec();
        let result = SetupAuthenticate { status, reason };
        Ok((result, remaining))
    }
}
impl Serialize for SetupAuthenticate {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.status.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 5]);
        assert_eq!(self.reason.len() % 4, 0, "`reason` has an incorrect length, must be a multiple of 4");
        let length = u16::try_from(self.reason.len() / 4).expect("`reason` has too many elements");
        length.serialize_into(bytes);
        bytes.extend_from_slice(&self.reason);
    }
}
impl SetupAuthenticate {
    /// Get the value of the `length` field.
    ///
    /// The `length` field is used as the length field of the `reason` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn length(&self) -> u16 {
        self.reason.len()
            .checked_div(4).unwrap()
            .try_into().unwrap()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageOrder(u8);
impl ImageOrder {
    pub const LSB_FIRST: Self = Self(0);
    pub const MSB_FIRST: Self = Self(1);
}
impl From<ImageOrder> for u8 {
    #[inline]
    fn from(input: ImageOrder) -> Self {
        input.0
    }
}
impl From<ImageOrder> for Option<u8> {
    #[inline]
    fn from(input: ImageOrder) -> Self {
        Some(input.0)
    }
}
impl From<ImageOrder> for u16 {
    #[inline]
    fn from(input: ImageOrder) -> Self {
        u16::from(input.0)
    }
}
impl From<ImageOrder> for Option<u16> {
    #[inline]
    fn from(input: ImageOrder) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<ImageOrder> for u32 {
    #[inline]
    fn from(input: ImageOrder) -> Self {
        u32::from(input.0)
    }
}
impl From<ImageOrder> for Option<u32> {
    #[inline]
    fn from(input: ImageOrder) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ImageOrder {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ImageOrder  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::LSB_FIRST.0.into(), "LSB_FIRST", "LSBFirst"),
            (Self::MSB_FIRST.0.into(), "MSB_FIRST", "MSBFirst"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Setup {
    pub status: u8,
    pub protocol_major_version: u16,
    pub protocol_minor_version: u16,
    pub length: u16,
    pub release_number: u32,
    pub resource_id_base: u32,
    pub resource_id_mask: u32,
    pub motion_buffer_size: u32,
    pub maximum_request_length: u16,
    pub image_byte_order: ImageOrder,
    pub bitmap_format_bit_order: ImageOrder,
    pub bitmap_format_scanline_unit: u8,
    pub bitmap_format_scanline_pad: u8,
    pub min_keycode: Keycode,
    pub max_keycode: Keycode,
    pub vendor: Vec<u8>,
    pub pixmap_formats: Vec<Format>,
    pub roots: Vec<Screen>,
}
impl_debug_if_no_extra_traits!(Setup, "Setup");
impl TryParse for Setup {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let value = remaining;
        let (status, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (protocol_major_version, remaining) = u16::try_parse(remaining)?;
        let (protocol_minor_version, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u16::try_parse(remaining)?;
        let (release_number, remaining) = u32::try_parse(remaining)?;
        let (resource_id_base, remaining) = u32::try_parse(remaining)?;
        let (resource_id_mask, remaining) = u32::try_parse(remaining)?;
        let (motion_buffer_size, remaining) = u32::try_parse(remaining)?;
        let (vendor_len, remaining) = u16::try_parse(remaining)?;
        let (maximum_request_length, remaining) = u16::try_parse(remaining)?;
        let (roots_len, remaining) = u8::try_parse(remaining)?;
        let (pixmap_formats_len, remaining) = u8::try_parse(remaining)?;
        let (image_byte_order, remaining) = u8::try_parse(remaining)?;
        let (bitmap_format_bit_order, remaining) = u8::try_parse(remaining)?;
        let (bitmap_format_scanline_unit, remaining) = u8::try_parse(remaining)?;
        let (bitmap_format_scanline_pad, remaining) = u8::try_parse(remaining)?;
        let (min_keycode, remaining) = Keycode::try_parse(remaining)?;
        let (max_keycode, remaining) = Keycode::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let (vendor, remaining) = crate::x11_utils::parse_u8_list(remaining, vendor_len.try_to_usize()?)?;
        let vendor = vendor.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let (pixmap_formats, remaining) = crate::x11_utils::parse_list::<Format>(remaining, pixmap_formats_len.try_to_usize()?)?;
        let (roots, remaining) = crate::x11_utils::parse_list::<Screen>(remaining, roots_len.try_to_usize()?)?;
        let image_byte_order = image_byte_order.into();
        let bitmap_format_bit_order = bitmap_format_bit_order.into();
        let result = Setup { status, protocol_major_version, protocol_minor_version, length, release_number, resource_id_base, resource_id_mask, motion_buffer_size, maximum_request_length, image_byte_order, bitmap_format_bit_order, bitmap_format_scanline_unit, bitmap_format_scanline_pad, min_keycode, max_keycode, vendor, pixmap_formats, roots };
        Ok((result, remaining))
    }
}
impl Serialize for Setup {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(40);
        self.status.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.protocol_major_version.serialize_into(bytes);
        self.protocol_minor_version.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.release_number.serialize_into(bytes);
        self.resource_id_base.serialize_into(bytes);
        self.resource_id_mask.serialize_into(bytes);
        self.motion_buffer_size.serialize_into(bytes);
        let vendor_len = u16::try_from(self.vendor.len()).expect("`vendor` has too many elements");
        vendor_len.serialize_into(bytes);
        self.maximum_request_length.serialize_into(bytes);
        let roots_len = u8::try_from(self.roots.len()).expect("`roots` has too many elements");
        roots_len.serialize_into(bytes);
        let pixmap_formats_len = u8::try_from(self.pixmap_formats.len()).expect("`pixmap_formats` has too many elements");
        pixmap_formats_len.serialize_into(bytes);
        u8::from(self.image_byte_order).serialize_into(bytes);
        u8::from(self.bitmap_format_bit_order).serialize_into(bytes);
        self.bitmap_format_scanline_unit.serialize_into(bytes);
        self.bitmap_format_scanline_pad.serialize_into(bytes);
        self.min_keycode.serialize_into(bytes);
        self.max_keycode.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&self.vendor);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        self.pixmap_formats.serialize_into(bytes);
        self.roots.serialize_into(bytes);
    }
}
impl Setup {
    /// Get the value of the `vendor_len` field.
    ///
    /// The `vendor_len` field is used as the length field of the `vendor` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn vendor_len(&self) -> u16 {
        self.vendor.len()
            .try_into().unwrap()
    }
    /// Get the value of the `roots_len` field.
    ///
    /// The `roots_len` field is used as the length field of the `roots` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn roots_len(&self) -> u8 {
        self.roots.len()
            .try_into().unwrap()
    }
    /// Get the value of the `pixmap_formats_len` field.
    ///
    /// The `pixmap_formats_len` field is used as the length field of the `pixmap_formats` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn pixmap_formats_len(&self) -> u8 {
        self.pixmap_formats.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModMask(u16);
impl ModMask {
    pub const SHIFT: Self = Self(1 << 0);
    pub const LOCK: Self = Self(1 << 1);
    pub const CONTROL: Self = Self(1 << 2);
    pub const M1: Self = Self(1 << 3);
    pub const M2: Self = Self(1 << 4);
    pub const M3: Self = Self(1 << 5);
    pub const M4: Self = Self(1 << 6);
    pub const M5: Self = Self(1 << 7);
    pub const ANY: Self = Self(1 << 15);
}
impl From<ModMask> for u16 {
    #[inline]
    fn from(input: ModMask) -> Self {
        input.0
    }
}
impl From<ModMask> for Option<u16> {
    #[inline]
    fn from(input: ModMask) -> Self {
        Some(input.0)
    }
}
impl From<ModMask> for u32 {
    #[inline]
    fn from(input: ModMask) -> Self {
        u32::from(input.0)
    }
}
impl From<ModMask> for Option<u32> {
    #[inline]
    fn from(input: ModMask) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ModMask {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for ModMask {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ModMask  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SHIFT.0.into(), "SHIFT", "Shift"),
            (Self::LOCK.0.into(), "LOCK", "Lock"),
            (Self::CONTROL.0.into(), "CONTROL", "Control"),
            (Self::M1.0.into(), "M1", "M1"),
            (Self::M2.0.into(), "M2", "M2"),
            (Self::M3.0.into(), "M3", "M3"),
            (Self::M4.0.into(), "M4", "M4"),
            (Self::M5.0.into(), "M5", "M5"),
            (Self::ANY.0.into(), "ANY", "Any"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(ModMask, u16);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyButMask(u16);
impl KeyButMask {
    pub const SHIFT: Self = Self(1 << 0);
    pub const LOCK: Self = Self(1 << 1);
    pub const CONTROL: Self = Self(1 << 2);
    pub const MOD1: Self = Self(1 << 3);
    pub const MOD2: Self = Self(1 << 4);
    pub const MOD3: Self = Self(1 << 5);
    pub const MOD4: Self = Self(1 << 6);
    pub const MOD5: Self = Self(1 << 7);
    pub const BUTTON1: Self = Self(1 << 8);
    pub const BUTTON2: Self = Self(1 << 9);
    pub const BUTTON3: Self = Self(1 << 10);
    pub const BUTTON4: Self = Self(1 << 11);
    pub const BUTTON5: Self = Self(1 << 12);
}
impl From<KeyButMask> for u16 {
    #[inline]
    fn from(input: KeyButMask) -> Self {
        input.0
    }
}
impl From<KeyButMask> for Option<u16> {
    #[inline]
    fn from(input: KeyButMask) -> Self {
        Some(input.0)
    }
}
impl From<KeyButMask> for u32 {
    #[inline]
    fn from(input: KeyButMask) -> Self {
        u32::from(input.0)
    }
}
impl From<KeyButMask> for Option<u32> {
    #[inline]
    fn from(input: KeyButMask) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for KeyButMask {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for KeyButMask {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for KeyButMask  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SHIFT.0.into(), "SHIFT", "Shift"),
            (Self::LOCK.0.into(), "LOCK", "Lock"),
            (Self::CONTROL.0.into(), "CONTROL", "Control"),
            (Self::MOD1.0.into(), "MOD1", "Mod1"),
            (Self::MOD2.0.into(), "MOD2", "Mod2"),
            (Self::MOD3.0.into(), "MOD3", "Mod3"),
            (Self::MOD4.0.into(), "MOD4", "Mod4"),
            (Self::MOD5.0.into(), "MOD5", "Mod5"),
            (Self::BUTTON1.0.into(), "BUTTON1", "Button1"),
            (Self::BUTTON2.0.into(), "BUTTON2", "Button2"),
            (Self::BUTTON3.0.into(), "BUTTON3", "Button3"),
            (Self::BUTTON4.0.into(), "BUTTON4", "Button4"),
            (Self::BUTTON5.0.into(), "BUTTON5", "Button5"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(KeyButMask, u16);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WindowEnum(u8);
impl WindowEnum {
    pub const NONE: Self = Self(0);
}
impl From<WindowEnum> for u8 {
    #[inline]
    fn from(input: WindowEnum) -> Self {
        input.0
    }
}
impl From<WindowEnum> for Option<u8> {
    #[inline]
    fn from(input: WindowEnum) -> Self {
        Some(input.0)
    }
}
impl From<WindowEnum> for u16 {
    #[inline]
    fn from(input: WindowEnum) -> Self {
        u16::from(input.0)
    }
}
impl From<WindowEnum> for Option<u16> {
    #[inline]
    fn from(input: WindowEnum) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<WindowEnum> for u32 {
    #[inline]
    fn from(input: WindowEnum) -> Self {
        u32::from(input.0)
    }
}
impl From<WindowEnum> for Option<u32> {
    #[inline]
    fn from(input: WindowEnum) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for WindowEnum {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for WindowEnum  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NONE.0.into(), "NONE", "None"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the KeyPress event
pub const KEY_PRESS_EVENT: u8 = 2;
/// a key was pressed/released.
///
/// # Fields
///
/// * `detail` - The keycode (a number representing a physical key on the keyboard) of the key
/// which was pressed.
/// * `time` - Time when the event was generated (in milliseconds).
/// * `root` - The root window of `child`.
/// * `same_screen` - Whether the `event` window is on the same screen as the `root` window.
/// * `event_x` - If `same_screen` is true, this is the X coordinate relative to the `event`
/// window's origin. Otherwise, `event_x` will be set to zero.
/// * `event_y` - If `same_screen` is true, this is the Y coordinate relative to the `event`
/// window's origin. Otherwise, `event_y` will be set to zero.
/// * `root_x` - The X coordinate of the pointer relative to the `root` window at the time of
/// the event.
/// * `root_y` - The Y coordinate of the pointer relative to the `root` window at the time of
/// the event.
/// * `state` - The logical state of the pointer buttons and modifier keys just prior to the
/// event.
///
/// # See
///
/// * `GrabKey`: request
/// * `GrabKeyboard`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyPressEvent {
    pub response_type: u8,
    pub detail: Keycode,
    pub sequence: u16,
    pub time: Timestamp,
    pub root: Window,
    pub event: Window,
    pub child: Window,
    pub root_x: i16,
    pub root_y: i16,
    pub event_x: i16,
    pub event_y: i16,
    pub state: KeyButMask,
    pub same_screen: bool,
}
impl_debug_if_no_extra_traits!(KeyPressEvent, "KeyPressEvent");
impl TryParse for KeyPressEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (detail, remaining) = Keycode::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let (root, remaining) = Window::try_parse(remaining)?;
        let (event, remaining) = Window::try_parse(remaining)?;
        let (child, remaining) = Window::try_parse(remaining)?;
        let (root_x, remaining) = i16::try_parse(remaining)?;
        let (root_y, remaining) = i16::try_parse(remaining)?;
        let (event_x, remaining) = i16::try_parse(remaining)?;
        let (event_y, remaining) = i16::try_parse(remaining)?;
        let (state, remaining) = u16::try_parse(remaining)?;
        let (same_screen, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let state = state.into();
        let result = KeyPressEvent { response_type, detail, sequence, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for KeyPressEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let detail_bytes = self.detail.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let root_bytes = self.root.serialize();
        let event_bytes = self.event.serialize();
        let child_bytes = self.child.serialize();
        let root_x_bytes = self.root_x.serialize();
        let root_y_bytes = self.root_y.serialize();
        let event_x_bytes = self.event_x.serialize();
        let event_y_bytes = self.event_y.serialize();
        let state_bytes = u16::from(self.state).serialize();
        let same_screen_bytes = self.same_screen.serialize();
        [
            response_type_bytes[0],
            detail_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            root_bytes[0],
            root_bytes[1],
            root_bytes[2],
            root_bytes[3],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            child_bytes[0],
            child_bytes[1],
            child_bytes[2],
            child_bytes[3],
            root_x_bytes[0],
            root_x_bytes[1],
            root_y_bytes[0],
            root_y_bytes[1],
            event_x_bytes[0],
            event_x_bytes[1],
            event_y_bytes[0],
            event_y_bytes[1],
            state_bytes[0],
            state_bytes[1],
            same_screen_bytes[0],
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.detail.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.root.serialize_into(bytes);
        self.event.serialize_into(bytes);
        self.child.serialize_into(bytes);
        self.root_x.serialize_into(bytes);
        self.root_y.serialize_into(bytes);
        self.event_x.serialize_into(bytes);
        self.event_y.serialize_into(bytes);
        u16::from(self.state).serialize_into(bytes);
        self.same_screen.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
    }
}
impl From<&KeyPressEvent> for [u8; 32] {
    fn from(input: &KeyPressEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let detail_bytes = input.detail.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let root_bytes = input.root.serialize();
        let event_bytes = input.event.serialize();
        let child_bytes = input.child.serialize();
        let root_x_bytes = input.root_x.serialize();
        let root_y_bytes = input.root_y.serialize();
        let event_x_bytes = input.event_x.serialize();
        let event_y_bytes = input.event_y.serialize();
        let state_bytes = u16::from(input.state).serialize();
        let same_screen_bytes = input.same_screen.serialize();
        [
            response_type_bytes[0],
            detail_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            root_bytes[0],
            root_bytes[1],
            root_bytes[2],
            root_bytes[3],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            child_bytes[0],
            child_bytes[1],
            child_bytes[2],
            child_bytes[3],
            root_x_bytes[0],
            root_x_bytes[1],
            root_y_bytes[0],
            root_y_bytes[1],
            event_x_bytes[0],
            event_x_bytes[1],
            event_y_bytes[0],
            event_y_bytes[1],
            state_bytes[0],
            state_bytes[1],
            same_screen_bytes[0],
            0,
        ]
    }
}
impl From<KeyPressEvent> for [u8; 32] {
    fn from(input: KeyPressEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the KeyRelease event
pub const KEY_RELEASE_EVENT: u8 = 3;
pub type KeyReleaseEvent = KeyPressEvent;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ButtonMask(u16);
impl ButtonMask {
    pub const M1: Self = Self(1 << 8);
    pub const M2: Self = Self(1 << 9);
    pub const M3: Self = Self(1 << 10);
    pub const M4: Self = Self(1 << 11);
    pub const M5: Self = Self(1 << 12);
    pub const ANY: Self = Self(1 << 15);
}
impl From<ButtonMask> for u16 {
    #[inline]
    fn from(input: ButtonMask) -> Self {
        input.0
    }
}
impl From<ButtonMask> for Option<u16> {
    #[inline]
    fn from(input: ButtonMask) -> Self {
        Some(input.0)
    }
}
impl From<ButtonMask> for u32 {
    #[inline]
    fn from(input: ButtonMask) -> Self {
        u32::from(input.0)
    }
}
impl From<ButtonMask> for Option<u32> {
    #[inline]
    fn from(input: ButtonMask) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ButtonMask {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for ButtonMask {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ButtonMask  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::M1.0.into(), "M1", "M1"),
            (Self::M2.0.into(), "M2", "M2"),
            (Self::M3.0.into(), "M3", "M3"),
            (Self::M4.0.into(), "M4", "M4"),
            (Self::M5.0.into(), "M5", "M5"),
            (Self::ANY.0.into(), "ANY", "Any"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(ButtonMask, u16);

/// Opcode for the ButtonPress event
pub const BUTTON_PRESS_EVENT: u8 = 4;
/// a mouse button was pressed/released.
///
/// # Fields
///
/// * `detail` - The keycode (a number representing a physical key on the keyboard) of the key
/// which was pressed.
/// * `time` - Time when the event was generated (in milliseconds).
/// * `root` - The root window of `child`.
/// * `same_screen` - Whether the `event` window is on the same screen as the `root` window.
/// * `event_x` - If `same_screen` is true, this is the X coordinate relative to the `event`
/// window's origin. Otherwise, `event_x` will be set to zero.
/// * `event_y` - If `same_screen` is true, this is the Y coordinate relative to the `event`
/// window's origin. Otherwise, `event_y` will be set to zero.
/// * `root_x` - The X coordinate of the pointer relative to the `root` window at the time of
/// the event.
/// * `root_y` - The Y coordinate of the pointer relative to the `root` window at the time of
/// the event.
/// * `state` - The logical state of the pointer buttons and modifier keys just prior to the
/// event.
///
/// # See
///
/// * `GrabButton`: request
/// * `GrabPointer`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ButtonPressEvent {
    pub response_type: u8,
    pub detail: Button,
    pub sequence: u16,
    pub time: Timestamp,
    pub root: Window,
    pub event: Window,
    pub child: Window,
    pub root_x: i16,
    pub root_y: i16,
    pub event_x: i16,
    pub event_y: i16,
    pub state: KeyButMask,
    pub same_screen: bool,
}
impl_debug_if_no_extra_traits!(ButtonPressEvent, "ButtonPressEvent");
impl TryParse for ButtonPressEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (detail, remaining) = Button::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let (root, remaining) = Window::try_parse(remaining)?;
        let (event, remaining) = Window::try_parse(remaining)?;
        let (child, remaining) = Window::try_parse(remaining)?;
        let (root_x, remaining) = i16::try_parse(remaining)?;
        let (root_y, remaining) = i16::try_parse(remaining)?;
        let (event_x, remaining) = i16::try_parse(remaining)?;
        let (event_y, remaining) = i16::try_parse(remaining)?;
        let (state, remaining) = u16::try_parse(remaining)?;
        let (same_screen, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let state = state.into();
        let result = ButtonPressEvent { response_type, detail, sequence, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ButtonPressEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let detail_bytes = self.detail.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let root_bytes = self.root.serialize();
        let event_bytes = self.event.serialize();
        let child_bytes = self.child.serialize();
        let root_x_bytes = self.root_x.serialize();
        let root_y_bytes = self.root_y.serialize();
        let event_x_bytes = self.event_x.serialize();
        let event_y_bytes = self.event_y.serialize();
        let state_bytes = u16::from(self.state).serialize();
        let same_screen_bytes = self.same_screen.serialize();
        [
            response_type_bytes[0],
            detail_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            root_bytes[0],
            root_bytes[1],
            root_bytes[2],
            root_bytes[3],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            child_bytes[0],
            child_bytes[1],
            child_bytes[2],
            child_bytes[3],
            root_x_bytes[0],
            root_x_bytes[1],
            root_y_bytes[0],
            root_y_bytes[1],
            event_x_bytes[0],
            event_x_bytes[1],
            event_y_bytes[0],
            event_y_bytes[1],
            state_bytes[0],
            state_bytes[1],
            same_screen_bytes[0],
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.detail.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.root.serialize_into(bytes);
        self.event.serialize_into(bytes);
        self.child.serialize_into(bytes);
        self.root_x.serialize_into(bytes);
        self.root_y.serialize_into(bytes);
        self.event_x.serialize_into(bytes);
        self.event_y.serialize_into(bytes);
        u16::from(self.state).serialize_into(bytes);
        self.same_screen.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
    }
}
impl From<&ButtonPressEvent> for [u8; 32] {
    fn from(input: &ButtonPressEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let detail_bytes = input.detail.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let root_bytes = input.root.serialize();
        let event_bytes = input.event.serialize();
        let child_bytes = input.child.serialize();
        let root_x_bytes = input.root_x.serialize();
        let root_y_bytes = input.root_y.serialize();
        let event_x_bytes = input.event_x.serialize();
        let event_y_bytes = input.event_y.serialize();
        let state_bytes = u16::from(input.state).serialize();
        let same_screen_bytes = input.same_screen.serialize();
        [
            response_type_bytes[0],
            detail_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            root_bytes[0],
            root_bytes[1],
            root_bytes[2],
            root_bytes[3],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            child_bytes[0],
            child_bytes[1],
            child_bytes[2],
            child_bytes[3],
            root_x_bytes[0],
            root_x_bytes[1],
            root_y_bytes[0],
            root_y_bytes[1],
            event_x_bytes[0],
            event_x_bytes[1],
            event_y_bytes[0],
            event_y_bytes[1],
            state_bytes[0],
            state_bytes[1],
            same_screen_bytes[0],
            0,
        ]
    }
}
impl From<ButtonPressEvent> for [u8; 32] {
    fn from(input: ButtonPressEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the ButtonRelease event
pub const BUTTON_RELEASE_EVENT: u8 = 5;
pub type ButtonReleaseEvent = ButtonPressEvent;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Motion(u8);
impl Motion {
    pub const NORMAL: Self = Self(0);
    pub const HINT: Self = Self(1);
}
impl From<Motion> for u8 {
    #[inline]
    fn from(input: Motion) -> Self {
        input.0
    }
}
impl From<Motion> for Option<u8> {
    #[inline]
    fn from(input: Motion) -> Self {
        Some(input.0)
    }
}
impl From<Motion> for u16 {
    #[inline]
    fn from(input: Motion) -> Self {
        u16::from(input.0)
    }
}
impl From<Motion> for Option<u16> {
    #[inline]
    fn from(input: Motion) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Motion> for u32 {
    #[inline]
    fn from(input: Motion) -> Self {
        u32::from(input.0)
    }
}
impl From<Motion> for Option<u32> {
    #[inline]
    fn from(input: Motion) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Motion {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Motion  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NORMAL.0.into(), "NORMAL", "Normal"),
            (Self::HINT.0.into(), "HINT", "Hint"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the MotionNotify event
pub const MOTION_NOTIFY_EVENT: u8 = 6;
/// a key was pressed.
///
/// # Fields
///
/// * `detail` - The keycode (a number representing a physical key on the keyboard) of the key
/// which was pressed.
/// * `time` - Time when the event was generated (in milliseconds).
/// * `root` - The root window of `child`.
/// * `same_screen` - Whether the `event` window is on the same screen as the `root` window.
/// * `event_x` - If `same_screen` is true, this is the X coordinate relative to the `event`
/// window's origin. Otherwise, `event_x` will be set to zero.
/// * `event_y` - If `same_screen` is true, this is the Y coordinate relative to the `event`
/// window's origin. Otherwise, `event_y` will be set to zero.
/// * `root_x` - The X coordinate of the pointer relative to the `root` window at the time of
/// the event.
/// * `root_y` - The Y coordinate of the pointer relative to the `root` window at the time of
/// the event.
/// * `state` - The logical state of the pointer buttons and modifier keys just prior to the
/// event.
///
/// # See
///
/// * `GrabKey`: request
/// * `GrabKeyboard`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MotionNotifyEvent {
    pub response_type: u8,
    pub detail: Motion,
    pub sequence: u16,
    pub time: Timestamp,
    pub root: Window,
    pub event: Window,
    pub child: Window,
    pub root_x: i16,
    pub root_y: i16,
    pub event_x: i16,
    pub event_y: i16,
    pub state: KeyButMask,
    pub same_screen: bool,
}
impl_debug_if_no_extra_traits!(MotionNotifyEvent, "MotionNotifyEvent");
impl TryParse for MotionNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (detail, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let (root, remaining) = Window::try_parse(remaining)?;
        let (event, remaining) = Window::try_parse(remaining)?;
        let (child, remaining) = Window::try_parse(remaining)?;
        let (root_x, remaining) = i16::try_parse(remaining)?;
        let (root_y, remaining) = i16::try_parse(remaining)?;
        let (event_x, remaining) = i16::try_parse(remaining)?;
        let (event_y, remaining) = i16::try_parse(remaining)?;
        let (state, remaining) = u16::try_parse(remaining)?;
        let (same_screen, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let detail = detail.into();
        let state = state.into();
        let result = MotionNotifyEvent { response_type, detail, sequence, time, root, event, child, root_x, root_y, event_x, event_y, state, same_screen };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for MotionNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let detail_bytes = u8::from(self.detail).serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let root_bytes = self.root.serialize();
        let event_bytes = self.event.serialize();
        let child_bytes = self.child.serialize();
        let root_x_bytes = self.root_x.serialize();
        let root_y_bytes = self.root_y.serialize();
        let event_x_bytes = self.event_x.serialize();
        let event_y_bytes = self.event_y.serialize();
        let state_bytes = u16::from(self.state).serialize();
        let same_screen_bytes = self.same_screen.serialize();
        [
            response_type_bytes[0],
            detail_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            root_bytes[0],
            root_bytes[1],
            root_bytes[2],
            root_bytes[3],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            child_bytes[0],
            child_bytes[1],
            child_bytes[2],
            child_bytes[3],
            root_x_bytes[0],
            root_x_bytes[1],
            root_y_bytes[0],
            root_y_bytes[1],
            event_x_bytes[0],
            event_x_bytes[1],
            event_y_bytes[0],
            event_y_bytes[1],
            state_bytes[0],
            state_bytes[1],
            same_screen_bytes[0],
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        u8::from(self.detail).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.root.serialize_into(bytes);
        self.event.serialize_into(bytes);
        self.child.serialize_into(bytes);
        self.root_x.serialize_into(bytes);
        self.root_y.serialize_into(bytes);
        self.event_x.serialize_into(bytes);
        self.event_y.serialize_into(bytes);
        u16::from(self.state).serialize_into(bytes);
        self.same_screen.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
    }
}
impl From<&MotionNotifyEvent> for [u8; 32] {
    fn from(input: &MotionNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let detail_bytes = u8::from(input.detail).serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let root_bytes = input.root.serialize();
        let event_bytes = input.event.serialize();
        let child_bytes = input.child.serialize();
        let root_x_bytes = input.root_x.serialize();
        let root_y_bytes = input.root_y.serialize();
        let event_x_bytes = input.event_x.serialize();
        let event_y_bytes = input.event_y.serialize();
        let state_bytes = u16::from(input.state).serialize();
        let same_screen_bytes = input.same_screen.serialize();
        [
            response_type_bytes[0],
            detail_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            root_bytes[0],
            root_bytes[1],
            root_bytes[2],
            root_bytes[3],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            child_bytes[0],
            child_bytes[1],
            child_bytes[2],
            child_bytes[3],
            root_x_bytes[0],
            root_x_bytes[1],
            root_y_bytes[0],
            root_y_bytes[1],
            event_x_bytes[0],
            event_x_bytes[1],
            event_y_bytes[0],
            event_y_bytes[1],
            state_bytes[0],
            state_bytes[1],
            same_screen_bytes[0],
            0,
        ]
    }
}
impl From<MotionNotifyEvent> for [u8; 32] {
    fn from(input: MotionNotifyEvent) -> Self {
        Self::from(&input)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NotifyDetail(u8);
impl NotifyDetail {
    pub const ANCESTOR: Self = Self(0);
    pub const VIRTUAL: Self = Self(1);
    pub const INFERIOR: Self = Self(2);
    pub const NONLINEAR: Self = Self(3);
    pub const NONLINEAR_VIRTUAL: Self = Self(4);
    pub const POINTER: Self = Self(5);
    pub const POINTER_ROOT: Self = Self(6);
    pub const NONE: Self = Self(7);
}
impl From<NotifyDetail> for u8 {
    #[inline]
    fn from(input: NotifyDetail) -> Self {
        input.0
    }
}
impl From<NotifyDetail> for Option<u8> {
    #[inline]
    fn from(input: NotifyDetail) -> Self {
        Some(input.0)
    }
}
impl From<NotifyDetail> for u16 {
    #[inline]
    fn from(input: NotifyDetail) -> Self {
        u16::from(input.0)
    }
}
impl From<NotifyDetail> for Option<u16> {
    #[inline]
    fn from(input: NotifyDetail) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<NotifyDetail> for u32 {
    #[inline]
    fn from(input: NotifyDetail) -> Self {
        u32::from(input.0)
    }
}
impl From<NotifyDetail> for Option<u32> {
    #[inline]
    fn from(input: NotifyDetail) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for NotifyDetail {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for NotifyDetail  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ANCESTOR.0.into(), "ANCESTOR", "Ancestor"),
            (Self::VIRTUAL.0.into(), "VIRTUAL", "Virtual"),
            (Self::INFERIOR.0.into(), "INFERIOR", "Inferior"),
            (Self::NONLINEAR.0.into(), "NONLINEAR", "Nonlinear"),
            (Self::NONLINEAR_VIRTUAL.0.into(), "NONLINEAR_VIRTUAL", "NonlinearVirtual"),
            (Self::POINTER.0.into(), "POINTER", "Pointer"),
            (Self::POINTER_ROOT.0.into(), "POINTER_ROOT", "PointerRoot"),
            (Self::NONE.0.into(), "NONE", "None"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NotifyMode(u8);
impl NotifyMode {
    pub const NORMAL: Self = Self(0);
    pub const GRAB: Self = Self(1);
    pub const UNGRAB: Self = Self(2);
    pub const WHILE_GRABBED: Self = Self(3);
}
impl From<NotifyMode> for u8 {
    #[inline]
    fn from(input: NotifyMode) -> Self {
        input.0
    }
}
impl From<NotifyMode> for Option<u8> {
    #[inline]
    fn from(input: NotifyMode) -> Self {
        Some(input.0)
    }
}
impl From<NotifyMode> for u16 {
    #[inline]
    fn from(input: NotifyMode) -> Self {
        u16::from(input.0)
    }
}
impl From<NotifyMode> for Option<u16> {
    #[inline]
    fn from(input: NotifyMode) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<NotifyMode> for u32 {
    #[inline]
    fn from(input: NotifyMode) -> Self {
        u32::from(input.0)
    }
}
impl From<NotifyMode> for Option<u32> {
    #[inline]
    fn from(input: NotifyMode) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for NotifyMode {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for NotifyMode  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NORMAL.0.into(), "NORMAL", "Normal"),
            (Self::GRAB.0.into(), "GRAB", "Grab"),
            (Self::UNGRAB.0.into(), "UNGRAB", "Ungrab"),
            (Self::WHILE_GRABBED.0.into(), "WHILE_GRABBED", "WhileGrabbed"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the EnterNotify event
pub const ENTER_NOTIFY_EVENT: u8 = 7;
/// the pointer is in a different window.
///
/// # Fields
///
/// * `event` - The window on which the event was generated.
/// * `child` - If the `event` window has subwindows and the final pointer position is in one
/// of them, then `child` is set to that subwindow, `XCB_WINDOW_NONE` otherwise.
/// * `root` - The root window for the final cursor position.
/// * `root_x` - The pointer X coordinate relative to `root`'s origin at the time of the event.
/// * `root_y` - The pointer Y coordinate relative to `root`'s origin at the time of the event.
/// * `event_x` - If `event` is on the same screen as `root`, this is the pointer X coordinate
/// relative to the event window's origin.
/// * `event_y` - If `event` is on the same screen as `root`, this is the pointer Y coordinate
/// relative to the event window's origin.
/// * `mode` -
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EnterNotifyEvent {
    pub response_type: u8,
    pub detail: NotifyDetail,
    pub sequence: u16,
    pub time: Timestamp,
    pub root: Window,
    pub event: Window,
    pub child: Window,
    pub root_x: i16,
    pub root_y: i16,
    pub event_x: i16,
    pub event_y: i16,
    pub state: KeyButMask,
    pub mode: NotifyMode,
    pub same_screen_focus: u8,
}
impl_debug_if_no_extra_traits!(EnterNotifyEvent, "EnterNotifyEvent");
impl TryParse for EnterNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (detail, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let (root, remaining) = Window::try_parse(remaining)?;
        let (event, remaining) = Window::try_parse(remaining)?;
        let (child, remaining) = Window::try_parse(remaining)?;
        let (root_x, remaining) = i16::try_parse(remaining)?;
        let (root_y, remaining) = i16::try_parse(remaining)?;
        let (event_x, remaining) = i16::try_parse(remaining)?;
        let (event_y, remaining) = i16::try_parse(remaining)?;
        let (state, remaining) = u16::try_parse(remaining)?;
        let (mode, remaining) = u8::try_parse(remaining)?;
        let (same_screen_focus, remaining) = u8::try_parse(remaining)?;
        let detail = detail.into();
        let state = state.into();
        let mode = mode.into();
        let result = EnterNotifyEvent { response_type, detail, sequence, time, root, event, child, root_x, root_y, event_x, event_y, state, mode, same_screen_focus };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for EnterNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let detail_bytes = u8::from(self.detail).serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let root_bytes = self.root.serialize();
        let event_bytes = self.event.serialize();
        let child_bytes = self.child.serialize();
        let root_x_bytes = self.root_x.serialize();
        let root_y_bytes = self.root_y.serialize();
        let event_x_bytes = self.event_x.serialize();
        let event_y_bytes = self.event_y.serialize();
        let state_bytes = u16::from(self.state).serialize();
        let mode_bytes = u8::from(self.mode).serialize();
        let same_screen_focus_bytes = self.same_screen_focus.serialize();
        [
            response_type_bytes[0],
            detail_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            root_bytes[0],
            root_bytes[1],
            root_bytes[2],
            root_bytes[3],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            child_bytes[0],
            child_bytes[1],
            child_bytes[2],
            child_bytes[3],
            root_x_bytes[0],
            root_x_bytes[1],
            root_y_bytes[0],
            root_y_bytes[1],
            event_x_bytes[0],
            event_x_bytes[1],
            event_y_bytes[0],
            event_y_bytes[1],
            state_bytes[0],
            state_bytes[1],
            mode_bytes[0],
            same_screen_focus_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        u8::from(self.detail).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.root.serialize_into(bytes);
        self.event.serialize_into(bytes);
        self.child.serialize_into(bytes);
        self.root_x.serialize_into(bytes);
        self.root_y.serialize_into(bytes);
        self.event_x.serialize_into(bytes);
        self.event_y.serialize_into(bytes);
        u16::from(self.state).serialize_into(bytes);
        u8::from(self.mode).serialize_into(bytes);
        self.same_screen_focus.serialize_into(bytes);
    }
}
impl From<&EnterNotifyEvent> for [u8; 32] {
    fn from(input: &EnterNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let detail_bytes = u8::from(input.detail).serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let root_bytes = input.root.serialize();
        let event_bytes = input.event.serialize();
        let child_bytes = input.child.serialize();
        let root_x_bytes = input.root_x.serialize();
        let root_y_bytes = input.root_y.serialize();
        let event_x_bytes = input.event_x.serialize();
        let event_y_bytes = input.event_y.serialize();
        let state_bytes = u16::from(input.state).serialize();
        let mode_bytes = u8::from(input.mode).serialize();
        let same_screen_focus_bytes = input.same_screen_focus.serialize();
        [
            response_type_bytes[0],
            detail_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            root_bytes[0],
            root_bytes[1],
            root_bytes[2],
            root_bytes[3],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            child_bytes[0],
            child_bytes[1],
            child_bytes[2],
            child_bytes[3],
            root_x_bytes[0],
            root_x_bytes[1],
            root_y_bytes[0],
            root_y_bytes[1],
            event_x_bytes[0],
            event_x_bytes[1],
            event_y_bytes[0],
            event_y_bytes[1],
            state_bytes[0],
            state_bytes[1],
            mode_bytes[0],
            same_screen_focus_bytes[0],
        ]
    }
}
impl From<EnterNotifyEvent> for [u8; 32] {
    fn from(input: EnterNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the LeaveNotify event
pub const LEAVE_NOTIFY_EVENT: u8 = 8;
pub type LeaveNotifyEvent = EnterNotifyEvent;

/// Opcode for the FocusIn event
pub const FOCUS_IN_EVENT: u8 = 9;
/// NOT YET DOCUMENTED.
///
/// # Fields
///
/// * `event` - The window on which the focus event was generated. This is the window used by
/// the X server to report the event.
/// * `detail` -
/// * `mode` -
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FocusInEvent {
    pub response_type: u8,
    pub detail: NotifyDetail,
    pub sequence: u16,
    pub event: Window,
    pub mode: NotifyMode,
}
impl_debug_if_no_extra_traits!(FocusInEvent, "FocusInEvent");
impl TryParse for FocusInEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (detail, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (event, remaining) = Window::try_parse(remaining)?;
        let (mode, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let detail = detail.into();
        let mode = mode.into();
        let result = FocusInEvent { response_type, detail, sequence, event, mode };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for FocusInEvent {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = self.response_type.serialize();
        let detail_bytes = u8::from(self.detail).serialize();
        let sequence_bytes = self.sequence.serialize();
        let event_bytes = self.event.serialize();
        let mode_bytes = u8::from(self.mode).serialize();
        [
            response_type_bytes[0],
            detail_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            mode_bytes[0],
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.response_type.serialize_into(bytes);
        u8::from(self.detail).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.event.serialize_into(bytes);
        u8::from(self.mode).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
    }
}
impl From<&FocusInEvent> for [u8; 32] {
    fn from(input: &FocusInEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let detail_bytes = u8::from(input.detail).serialize();
        let sequence_bytes = input.sequence.serialize();
        let event_bytes = input.event.serialize();
        let mode_bytes = u8::from(input.mode).serialize();
        [
            response_type_bytes[0],
            detail_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            mode_bytes[0],
            0,
            0,
            0,
            // trailing padding
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
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<FocusInEvent> for [u8; 32] {
    fn from(input: FocusInEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the FocusOut event
pub const FOCUS_OUT_EVENT: u8 = 10;
pub type FocusOutEvent = FocusInEvent;

/// Opcode for the KeymapNotify event
pub const KEYMAP_NOTIFY_EVENT: u8 = 11;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeymapNotifyEvent {
    pub response_type: u8,
    pub keys: [u8; 31],
}
impl_debug_if_no_extra_traits!(KeymapNotifyEvent, "KeymapNotifyEvent");
impl TryParse for KeymapNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (keys, remaining) = crate::x11_utils::parse_u8_array::<31>(remaining)?;
        let result = KeymapNotifyEvent { response_type, keys };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for KeymapNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        [
            response_type_bytes[0],
            self.keys[0],
            self.keys[1],
            self.keys[2],
            self.keys[3],
            self.keys[4],
            self.keys[5],
            self.keys[6],
            self.keys[7],
            self.keys[8],
            self.keys[9],
            self.keys[10],
            self.keys[11],
            self.keys[12],
            self.keys[13],
            self.keys[14],
            self.keys[15],
            self.keys[16],
            self.keys[17],
            self.keys[18],
            self.keys[19],
            self.keys[20],
            self.keys[21],
            self.keys[22],
            self.keys[23],
            self.keys[24],
            self.keys[25],
            self.keys[26],
            self.keys[27],
            self.keys[28],
            self.keys[29],
            self.keys[30],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&self.keys);
    }
}
impl From<&KeymapNotifyEvent> for [u8; 32] {
    fn from(input: &KeymapNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        [
            response_type_bytes[0],
            input.keys[0],
            input.keys[1],
            input.keys[2],
            input.keys[3],
            input.keys[4],
            input.keys[5],
            input.keys[6],
            input.keys[7],
            input.keys[8],
            input.keys[9],
            input.keys[10],
            input.keys[11],
            input.keys[12],
            input.keys[13],
            input.keys[14],
            input.keys[15],
            input.keys[16],
            input.keys[17],
            input.keys[18],
            input.keys[19],
            input.keys[20],
            input.keys[21],
            input.keys[22],
            input.keys[23],
            input.keys[24],
            input.keys[25],
            input.keys[26],
            input.keys[27],
            input.keys[28],
            input.keys[29],
            input.keys[30],
        ]
    }
}
impl From<KeymapNotifyEvent> for [u8; 32] {
    fn from(input: KeymapNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the Expose event
pub const EXPOSE_EVENT: u8 = 12;
/// NOT YET DOCUMENTED.
///
/// # Fields
///
/// * `window` - The exposed (damaged) window.
/// * `x` - The X coordinate of the left-upper corner of the exposed rectangle, relative to
/// the `window`'s origin.
/// * `y` - The Y coordinate of the left-upper corner of the exposed rectangle, relative to
/// the `window`'s origin.
/// * `width` - The width of the exposed rectangle.
/// * `height` - The height of the exposed rectangle.
/// * `count` - The amount of `Expose` events following this one. Simple applications that do
/// not want to optimize redisplay by distinguishing between subareas of its window
/// can just ignore all Expose events with nonzero counts and perform full
/// redisplays on events with zero counts.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExposeEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub window: Window,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub count: u16,
}
impl_debug_if_no_extra_traits!(ExposeEvent, "ExposeEvent");
impl TryParse for ExposeEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let (x, remaining) = u16::try_parse(remaining)?;
        let (y, remaining) = u16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (count, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let result = ExposeEvent { response_type, sequence, window, x, y, width, height, count };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ExposeEvent {
    type Bytes = [u8; 20];
    fn serialize(&self) -> [u8; 20] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let window_bytes = self.window.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let count_bytes = self.count.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            count_bytes[0],
            count_bytes[1],
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(20);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        self.count.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
    }
}
impl From<&ExposeEvent> for [u8; 32] {
    fn from(input: &ExposeEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let window_bytes = input.window.serialize();
        let x_bytes = input.x.serialize();
        let y_bytes = input.y.serialize();
        let width_bytes = input.width.serialize();
        let height_bytes = input.height.serialize();
        let count_bytes = input.count.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            count_bytes[0],
            count_bytes[1],
            0,
            0,
            // trailing padding
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
}
impl From<ExposeEvent> for [u8; 32] {
    fn from(input: ExposeEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the GraphicsExposure event
pub const GRAPHICS_EXPOSURE_EVENT: u8 = 13;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphicsExposureEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub drawable: Drawable,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub minor_opcode: u16,
    pub count: u16,
    pub major_opcode: u8,
}
impl_debug_if_no_extra_traits!(GraphicsExposureEvent, "GraphicsExposureEvent");
impl TryParse for GraphicsExposureEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (drawable, remaining) = Drawable::try_parse(remaining)?;
        let (x, remaining) = u16::try_parse(remaining)?;
        let (y, remaining) = u16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (minor_opcode, remaining) = u16::try_parse(remaining)?;
        let (count, remaining) = u16::try_parse(remaining)?;
        let (major_opcode, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let result = GraphicsExposureEvent { response_type, sequence, drawable, x, y, width, height, minor_opcode, count, major_opcode };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GraphicsExposureEvent {
    type Bytes = [u8; 24];
    fn serialize(&self) -> [u8; 24] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let drawable_bytes = self.drawable.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let minor_opcode_bytes = self.minor_opcode.serialize();
        let count_bytes = self.count.serialize();
        let major_opcode_bytes = self.major_opcode.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            minor_opcode_bytes[0],
            minor_opcode_bytes[1],
            count_bytes[0],
            count_bytes[1],
            major_opcode_bytes[0],
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(24);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.drawable.serialize_into(bytes);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        self.minor_opcode.serialize_into(bytes);
        self.count.serialize_into(bytes);
        self.major_opcode.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
    }
}
impl From<&GraphicsExposureEvent> for [u8; 32] {
    fn from(input: &GraphicsExposureEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let drawable_bytes = input.drawable.serialize();
        let x_bytes = input.x.serialize();
        let y_bytes = input.y.serialize();
        let width_bytes = input.width.serialize();
        let height_bytes = input.height.serialize();
        let minor_opcode_bytes = input.minor_opcode.serialize();
        let count_bytes = input.count.serialize();
        let major_opcode_bytes = input.major_opcode.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            minor_opcode_bytes[0],
            minor_opcode_bytes[1],
            count_bytes[0],
            count_bytes[1],
            major_opcode_bytes[0],
            0,
            0,
            0,
            // trailing padding
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
}
impl From<GraphicsExposureEvent> for [u8; 32] {
    fn from(input: GraphicsExposureEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the NoExposure event
pub const NO_EXPOSURE_EVENT: u8 = 14;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NoExposureEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub drawable: Drawable,
    pub minor_opcode: u16,
    pub major_opcode: u8,
}
impl_debug_if_no_extra_traits!(NoExposureEvent, "NoExposureEvent");
impl TryParse for NoExposureEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (drawable, remaining) = Drawable::try_parse(remaining)?;
        let (minor_opcode, remaining) = u16::try_parse(remaining)?;
        let (major_opcode, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let result = NoExposureEvent { response_type, sequence, drawable, minor_opcode, major_opcode };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for NoExposureEvent {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let drawable_bytes = self.drawable.serialize();
        let minor_opcode_bytes = self.minor_opcode.serialize();
        let major_opcode_bytes = self.major_opcode.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            minor_opcode_bytes[0],
            minor_opcode_bytes[1],
            major_opcode_bytes[0],
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.drawable.serialize_into(bytes);
        self.minor_opcode.serialize_into(bytes);
        self.major_opcode.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
    }
}
impl From<&NoExposureEvent> for [u8; 32] {
    fn from(input: &NoExposureEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let drawable_bytes = input.drawable.serialize();
        let minor_opcode_bytes = input.minor_opcode.serialize();
        let major_opcode_bytes = input.major_opcode.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            minor_opcode_bytes[0],
            minor_opcode_bytes[1],
            major_opcode_bytes[0],
            0,
            // trailing padding
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
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<NoExposureEvent> for [u8; 32] {
    fn from(input: NoExposureEvent) -> Self {
        Self::from(&input)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Visibility(u8);
impl Visibility {
    pub const UNOBSCURED: Self = Self(0);
    pub const PARTIALLY_OBSCURED: Self = Self(1);
    pub const FULLY_OBSCURED: Self = Self(2);
}
impl From<Visibility> for u8 {
    #[inline]
    fn from(input: Visibility) -> Self {
        input.0
    }
}
impl From<Visibility> for Option<u8> {
    #[inline]
    fn from(input: Visibility) -> Self {
        Some(input.0)
    }
}
impl From<Visibility> for u16 {
    #[inline]
    fn from(input: Visibility) -> Self {
        u16::from(input.0)
    }
}
impl From<Visibility> for Option<u16> {
    #[inline]
    fn from(input: Visibility) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Visibility> for u32 {
    #[inline]
    fn from(input: Visibility) -> Self {
        u32::from(input.0)
    }
}
impl From<Visibility> for Option<u32> {
    #[inline]
    fn from(input: Visibility) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Visibility {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Visibility  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::UNOBSCURED.0.into(), "UNOBSCURED", "Unobscured"),
            (Self::PARTIALLY_OBSCURED.0.into(), "PARTIALLY_OBSCURED", "PartiallyObscured"),
            (Self::FULLY_OBSCURED.0.into(), "FULLY_OBSCURED", "FullyObscured"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the VisibilityNotify event
pub const VISIBILITY_NOTIFY_EVENT: u8 = 15;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VisibilityNotifyEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub window: Window,
    pub state: Visibility,
}
impl_debug_if_no_extra_traits!(VisibilityNotifyEvent, "VisibilityNotifyEvent");
impl TryParse for VisibilityNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let (state, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let state = state.into();
        let result = VisibilityNotifyEvent { response_type, sequence, window, state };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for VisibilityNotifyEvent {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let window_bytes = self.window.serialize();
        let state_bytes = u8::from(self.state).serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            state_bytes[0],
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.window.serialize_into(bytes);
        u8::from(self.state).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
    }
}
impl From<&VisibilityNotifyEvent> for [u8; 32] {
    fn from(input: &VisibilityNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let window_bytes = input.window.serialize();
        let state_bytes = u8::from(input.state).serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            state_bytes[0],
            0,
            0,
            0,
            // trailing padding
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
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<VisibilityNotifyEvent> for [u8; 32] {
    fn from(input: VisibilityNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the CreateNotify event
pub const CREATE_NOTIFY_EVENT: u8 = 16;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateNotifyEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub parent: Window,
    pub window: Window,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub border_width: u16,
    pub override_redirect: bool,
}
impl_debug_if_no_extra_traits!(CreateNotifyEvent, "CreateNotifyEvent");
impl TryParse for CreateNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (parent, remaining) = Window::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (border_width, remaining) = u16::try_parse(remaining)?;
        let (override_redirect, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let result = CreateNotifyEvent { response_type, sequence, parent, window, x, y, width, height, border_width, override_redirect };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for CreateNotifyEvent {
    type Bytes = [u8; 24];
    fn serialize(&self) -> [u8; 24] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let parent_bytes = self.parent.serialize();
        let window_bytes = self.window.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let border_width_bytes = self.border_width.serialize();
        let override_redirect_bytes = self.override_redirect.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            parent_bytes[0],
            parent_bytes[1],
            parent_bytes[2],
            parent_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            border_width_bytes[0],
            border_width_bytes[1],
            override_redirect_bytes[0],
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(24);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.parent.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        self.border_width.serialize_into(bytes);
        self.override_redirect.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
    }
}
impl From<&CreateNotifyEvent> for [u8; 32] {
    fn from(input: &CreateNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let parent_bytes = input.parent.serialize();
        let window_bytes = input.window.serialize();
        let x_bytes = input.x.serialize();
        let y_bytes = input.y.serialize();
        let width_bytes = input.width.serialize();
        let height_bytes = input.height.serialize();
        let border_width_bytes = input.border_width.serialize();
        let override_redirect_bytes = input.override_redirect.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            parent_bytes[0],
            parent_bytes[1],
            parent_bytes[2],
            parent_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            border_width_bytes[0],
            border_width_bytes[1],
            override_redirect_bytes[0],
            0,
            // trailing padding
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
}
impl From<CreateNotifyEvent> for [u8; 32] {
    fn from(input: CreateNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the DestroyNotify event
pub const DESTROY_NOTIFY_EVENT: u8 = 17;
/// a window is destroyed.
///
/// # Fields
///
/// * `event` - The reconfigured window or its parent, depending on whether `StructureNotify`
/// or `SubstructureNotify` was selected.
/// * `window` - The window that is destroyed.
///
/// # See
///
/// * `DestroyWindow`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DestroyNotifyEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub event: Window,
    pub window: Window,
}
impl_debug_if_no_extra_traits!(DestroyNotifyEvent, "DestroyNotifyEvent");
impl TryParse for DestroyNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (event, remaining) = Window::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let result = DestroyNotifyEvent { response_type, sequence, event, window };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for DestroyNotifyEvent {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let event_bytes = self.event.serialize();
        let window_bytes = self.window.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.event.serialize_into(bytes);
        self.window.serialize_into(bytes);
    }
}
impl From<&DestroyNotifyEvent> for [u8; 32] {
    fn from(input: &DestroyNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let event_bytes = input.event.serialize();
        let window_bytes = input.window.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            // trailing padding
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
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<DestroyNotifyEvent> for [u8; 32] {
    fn from(input: DestroyNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the UnmapNotify event
pub const UNMAP_NOTIFY_EVENT: u8 = 18;
/// a window is unmapped.
///
/// # Fields
///
/// * `event` - The reconfigured window or its parent, depending on whether `StructureNotify`
/// or `SubstructureNotify` was selected.
/// * `window` - The window that was unmapped.
/// * `from_configure` - Set to 1 if the event was generated as a result of a resizing of the window's
/// parent when `window` had a win_gravity of `UnmapGravity`.
///
/// # See
///
/// * `UnmapWindow`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnmapNotifyEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub event: Window,
    pub window: Window,
    pub from_configure: bool,
}
impl_debug_if_no_extra_traits!(UnmapNotifyEvent, "UnmapNotifyEvent");
impl TryParse for UnmapNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (event, remaining) = Window::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let (from_configure, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let result = UnmapNotifyEvent { response_type, sequence, event, window, from_configure };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for UnmapNotifyEvent {
    type Bytes = [u8; 16];
    fn serialize(&self) -> [u8; 16] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let event_bytes = self.event.serialize();
        let window_bytes = self.window.serialize();
        let from_configure_bytes = self.from_configure.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            from_configure_bytes[0],
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.event.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.from_configure.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
    }
}
impl From<&UnmapNotifyEvent> for [u8; 32] {
    fn from(input: &UnmapNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let event_bytes = input.event.serialize();
        let window_bytes = input.window.serialize();
        let from_configure_bytes = input.from_configure.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            from_configure_bytes[0],
            0,
            0,
            0,
            // trailing padding
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
}
impl From<UnmapNotifyEvent> for [u8; 32] {
    fn from(input: UnmapNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the MapNotify event
pub const MAP_NOTIFY_EVENT: u8 = 19;
/// a window was mapped.
///
/// # Fields
///
/// * `event` - The window which was mapped or its parent, depending on whether
/// `StructureNotify` or `SubstructureNotify` was selected.
/// * `window` - The window that was mapped.
/// * `override_redirect` - Window managers should ignore this window if `override_redirect` is 1.
///
/// # See
///
/// * `MapWindow`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapNotifyEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub event: Window,
    pub window: Window,
    pub override_redirect: bool,
}
impl_debug_if_no_extra_traits!(MapNotifyEvent, "MapNotifyEvent");
impl TryParse for MapNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (event, remaining) = Window::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let (override_redirect, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let result = MapNotifyEvent { response_type, sequence, event, window, override_redirect };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for MapNotifyEvent {
    type Bytes = [u8; 16];
    fn serialize(&self) -> [u8; 16] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let event_bytes = self.event.serialize();
        let window_bytes = self.window.serialize();
        let override_redirect_bytes = self.override_redirect.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            override_redirect_bytes[0],
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.event.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.override_redirect.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
    }
}
impl From<&MapNotifyEvent> for [u8; 32] {
    fn from(input: &MapNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let event_bytes = input.event.serialize();
        let window_bytes = input.window.serialize();
        let override_redirect_bytes = input.override_redirect.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            override_redirect_bytes[0],
            0,
            0,
            0,
            // trailing padding
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
}
impl From<MapNotifyEvent> for [u8; 32] {
    fn from(input: MapNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the MapRequest event
pub const MAP_REQUEST_EVENT: u8 = 20;
/// window wants to be mapped.
///
/// # Fields
///
/// * `parent` - The parent of `window`.
/// * `window` - The window to be mapped.
///
/// # See
///
/// * `MapWindow`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapRequestEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub parent: Window,
    pub window: Window,
}
impl_debug_if_no_extra_traits!(MapRequestEvent, "MapRequestEvent");
impl TryParse for MapRequestEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (parent, remaining) = Window::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let result = MapRequestEvent { response_type, sequence, parent, window };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for MapRequestEvent {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let parent_bytes = self.parent.serialize();
        let window_bytes = self.window.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            parent_bytes[0],
            parent_bytes[1],
            parent_bytes[2],
            parent_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.parent.serialize_into(bytes);
        self.window.serialize_into(bytes);
    }
}
impl From<&MapRequestEvent> for [u8; 32] {
    fn from(input: &MapRequestEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let parent_bytes = input.parent.serialize();
        let window_bytes = input.window.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            parent_bytes[0],
            parent_bytes[1],
            parent_bytes[2],
            parent_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            // trailing padding
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
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<MapRequestEvent> for [u8; 32] {
    fn from(input: MapRequestEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the ReparentNotify event
pub const REPARENT_NOTIFY_EVENT: u8 = 21;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReparentNotifyEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub event: Window,
    pub window: Window,
    pub parent: Window,
    pub x: i16,
    pub y: i16,
    pub override_redirect: bool,
}
impl_debug_if_no_extra_traits!(ReparentNotifyEvent, "ReparentNotifyEvent");
impl TryParse for ReparentNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (event, remaining) = Window::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let (parent, remaining) = Window::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (override_redirect, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let result = ReparentNotifyEvent { response_type, sequence, event, window, parent, x, y, override_redirect };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ReparentNotifyEvent {
    type Bytes = [u8; 24];
    fn serialize(&self) -> [u8; 24] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let event_bytes = self.event.serialize();
        let window_bytes = self.window.serialize();
        let parent_bytes = self.parent.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let override_redirect_bytes = self.override_redirect.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            parent_bytes[0],
            parent_bytes[1],
            parent_bytes[2],
            parent_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            override_redirect_bytes[0],
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(24);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.event.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.parent.serialize_into(bytes);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
        self.override_redirect.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
    }
}
impl From<&ReparentNotifyEvent> for [u8; 32] {
    fn from(input: &ReparentNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let event_bytes = input.event.serialize();
        let window_bytes = input.window.serialize();
        let parent_bytes = input.parent.serialize();
        let x_bytes = input.x.serialize();
        let y_bytes = input.y.serialize();
        let override_redirect_bytes = input.override_redirect.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            parent_bytes[0],
            parent_bytes[1],
            parent_bytes[2],
            parent_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            override_redirect_bytes[0],
            0,
            0,
            0,
            // trailing padding
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
}
impl From<ReparentNotifyEvent> for [u8; 32] {
    fn from(input: ReparentNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the ConfigureNotify event
pub const CONFIGURE_NOTIFY_EVENT: u8 = 22;
/// NOT YET DOCUMENTED.
///
/// # Fields
///
/// * `event` - The reconfigured window or its parent, depending on whether `StructureNotify`
/// or `SubstructureNotify` was selected.
/// * `window` - The window whose size, position, border, and/or stacking order was changed.
/// * `above_sibling` - If `XCB_NONE`, the `window` is on the bottom of the stack with respect to
/// sibling windows. However, if set to a sibling window, the `window` is placed on
/// top of this sibling window.
/// * `x` - The X coordinate of the upper-left outside corner of `window`, relative to the
/// parent window's origin.
/// * `y` - The Y coordinate of the upper-left outside corner of `window`, relative to the
/// parent window's origin.
/// * `width` - The inside width of `window`, not including the border.
/// * `height` - The inside height of `window`, not including the border.
/// * `border_width` - The border width of `window`.
/// * `override_redirect` - Window managers should ignore this window if `override_redirect` is 1.
///
/// # See
///
/// * `FreeColormap`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConfigureNotifyEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub event: Window,
    pub window: Window,
    pub above_sibling: Window,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub border_width: u16,
    pub override_redirect: bool,
}
impl_debug_if_no_extra_traits!(ConfigureNotifyEvent, "ConfigureNotifyEvent");
impl TryParse for ConfigureNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (event, remaining) = Window::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let (above_sibling, remaining) = Window::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (border_width, remaining) = u16::try_parse(remaining)?;
        let (override_redirect, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let result = ConfigureNotifyEvent { response_type, sequence, event, window, above_sibling, x, y, width, height, border_width, override_redirect };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ConfigureNotifyEvent {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let event_bytes = self.event.serialize();
        let window_bytes = self.window.serialize();
        let above_sibling_bytes = self.above_sibling.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let border_width_bytes = self.border_width.serialize();
        let override_redirect_bytes = self.override_redirect.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            above_sibling_bytes[0],
            above_sibling_bytes[1],
            above_sibling_bytes[2],
            above_sibling_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            border_width_bytes[0],
            border_width_bytes[1],
            override_redirect_bytes[0],
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(28);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.event.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.above_sibling.serialize_into(bytes);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        self.border_width.serialize_into(bytes);
        self.override_redirect.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
    }
}
impl From<&ConfigureNotifyEvent> for [u8; 32] {
    fn from(input: &ConfigureNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let event_bytes = input.event.serialize();
        let window_bytes = input.window.serialize();
        let above_sibling_bytes = input.above_sibling.serialize();
        let x_bytes = input.x.serialize();
        let y_bytes = input.y.serialize();
        let width_bytes = input.width.serialize();
        let height_bytes = input.height.serialize();
        let border_width_bytes = input.border_width.serialize();
        let override_redirect_bytes = input.override_redirect.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            above_sibling_bytes[0],
            above_sibling_bytes[1],
            above_sibling_bytes[2],
            above_sibling_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            border_width_bytes[0],
            border_width_bytes[1],
            override_redirect_bytes[0],
            0,
            // trailing padding
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<ConfigureNotifyEvent> for [u8; 32] {
    fn from(input: ConfigureNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the ConfigureRequest event
pub const CONFIGURE_REQUEST_EVENT: u8 = 23;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConfigureRequestEvent {
    pub response_type: u8,
    pub stack_mode: StackMode,
    pub sequence: u16,
    pub parent: Window,
    pub window: Window,
    pub sibling: Window,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub border_width: u16,
    pub value_mask: ConfigWindow,
}
impl_debug_if_no_extra_traits!(ConfigureRequestEvent, "ConfigureRequestEvent");
impl TryParse for ConfigureRequestEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (stack_mode, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (parent, remaining) = Window::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let (sibling, remaining) = Window::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (border_width, remaining) = u16::try_parse(remaining)?;
        let (value_mask, remaining) = u16::try_parse(remaining)?;
        let stack_mode = stack_mode.into();
        let value_mask = value_mask.into();
        let result = ConfigureRequestEvent { response_type, stack_mode, sequence, parent, window, sibling, x, y, width, height, border_width, value_mask };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ConfigureRequestEvent {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        let response_type_bytes = self.response_type.serialize();
        let stack_mode_bytes = (u32::from(self.stack_mode) as u8).serialize();
        let sequence_bytes = self.sequence.serialize();
        let parent_bytes = self.parent.serialize();
        let window_bytes = self.window.serialize();
        let sibling_bytes = self.sibling.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let border_width_bytes = self.border_width.serialize();
        let value_mask_bytes = u16::from(self.value_mask).serialize();
        [
            response_type_bytes[0],
            stack_mode_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            parent_bytes[0],
            parent_bytes[1],
            parent_bytes[2],
            parent_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            sibling_bytes[0],
            sibling_bytes[1],
            sibling_bytes[2],
            sibling_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            border_width_bytes[0],
            border_width_bytes[1],
            value_mask_bytes[0],
            value_mask_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(28);
        self.response_type.serialize_into(bytes);
        (u32::from(self.stack_mode) as u8).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.parent.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.sibling.serialize_into(bytes);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        self.border_width.serialize_into(bytes);
        u16::from(self.value_mask).serialize_into(bytes);
    }
}
impl From<&ConfigureRequestEvent> for [u8; 32] {
    fn from(input: &ConfigureRequestEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let stack_mode_bytes = (u32::from(input.stack_mode) as u8).serialize();
        let sequence_bytes = input.sequence.serialize();
        let parent_bytes = input.parent.serialize();
        let window_bytes = input.window.serialize();
        let sibling_bytes = input.sibling.serialize();
        let x_bytes = input.x.serialize();
        let y_bytes = input.y.serialize();
        let width_bytes = input.width.serialize();
        let height_bytes = input.height.serialize();
        let border_width_bytes = input.border_width.serialize();
        let value_mask_bytes = u16::from(input.value_mask).serialize();
        [
            response_type_bytes[0],
            stack_mode_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            parent_bytes[0],
            parent_bytes[1],
            parent_bytes[2],
            parent_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            sibling_bytes[0],
            sibling_bytes[1],
            sibling_bytes[2],
            sibling_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            border_width_bytes[0],
            border_width_bytes[1],
            value_mask_bytes[0],
            value_mask_bytes[1],
            // trailing padding
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<ConfigureRequestEvent> for [u8; 32] {
    fn from(input: ConfigureRequestEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the GravityNotify event
pub const GRAVITY_NOTIFY_EVENT: u8 = 24;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GravityNotifyEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub event: Window,
    pub window: Window,
    pub x: i16,
    pub y: i16,
}
impl_debug_if_no_extra_traits!(GravityNotifyEvent, "GravityNotifyEvent");
impl TryParse for GravityNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (event, remaining) = Window::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let result = GravityNotifyEvent { response_type, sequence, event, window, x, y };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GravityNotifyEvent {
    type Bytes = [u8; 16];
    fn serialize(&self) -> [u8; 16] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let event_bytes = self.event.serialize();
        let window_bytes = self.window.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.event.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
    }
}
impl From<&GravityNotifyEvent> for [u8; 32] {
    fn from(input: &GravityNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let event_bytes = input.event.serialize();
        let window_bytes = input.window.serialize();
        let x_bytes = input.x.serialize();
        let y_bytes = input.y.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            // trailing padding
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
}
impl From<GravityNotifyEvent> for [u8; 32] {
    fn from(input: GravityNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the ResizeRequest event
pub const RESIZE_REQUEST_EVENT: u8 = 25;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResizeRequestEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub window: Window,
    pub width: u16,
    pub height: u16,
}
impl_debug_if_no_extra_traits!(ResizeRequestEvent, "ResizeRequestEvent");
impl TryParse for ResizeRequestEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let result = ResizeRequestEvent { response_type, sequence, window, width, height };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ResizeRequestEvent {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let window_bytes = self.window.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
    }
}
impl From<&ResizeRequestEvent> for [u8; 32] {
    fn from(input: &ResizeRequestEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let window_bytes = input.window.serialize();
        let width_bytes = input.width.serialize();
        let height_bytes = input.height.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            // trailing padding
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
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<ResizeRequestEvent> for [u8; 32] {
    fn from(input: ResizeRequestEvent) -> Self {
        Self::from(&input)
    }
}

/// # Fields
///
/// * `OnTop` - The window is now on top of all siblings.
/// * `OnBottom` - The window is now below all siblings.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Place(u8);
impl Place {
    pub const ON_TOP: Self = Self(0);
    pub const ON_BOTTOM: Self = Self(1);
}
impl From<Place> for u8 {
    #[inline]
    fn from(input: Place) -> Self {
        input.0
    }
}
impl From<Place> for Option<u8> {
    #[inline]
    fn from(input: Place) -> Self {
        Some(input.0)
    }
}
impl From<Place> for u16 {
    #[inline]
    fn from(input: Place) -> Self {
        u16::from(input.0)
    }
}
impl From<Place> for Option<u16> {
    #[inline]
    fn from(input: Place) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Place> for u32 {
    #[inline]
    fn from(input: Place) -> Self {
        u32::from(input.0)
    }
}
impl From<Place> for Option<u32> {
    #[inline]
    fn from(input: Place) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Place {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Place  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ON_TOP.0.into(), "ON_TOP", "OnTop"),
            (Self::ON_BOTTOM.0.into(), "ON_BOTTOM", "OnBottom"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the CirculateNotify event
pub const CIRCULATE_NOTIFY_EVENT: u8 = 26;
/// NOT YET DOCUMENTED.
///
/// # Fields
///
/// * `event` - Either the restacked window or its parent, depending on whether
/// `StructureNotify` or `SubstructureNotify` was selected.
/// * `window` - The restacked window.
/// * `place` -
///
/// # See
///
/// * `CirculateWindow`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CirculateNotifyEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub event: Window,
    pub window: Window,
    pub place: Place,
}
impl_debug_if_no_extra_traits!(CirculateNotifyEvent, "CirculateNotifyEvent");
impl TryParse for CirculateNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (event, remaining) = Window::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let (place, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let place = place.into();
        let result = CirculateNotifyEvent { response_type, sequence, event, window, place };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for CirculateNotifyEvent {
    type Bytes = [u8; 20];
    fn serialize(&self) -> [u8; 20] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let event_bytes = self.event.serialize();
        let window_bytes = self.window.serialize();
        let place_bytes = u8::from(self.place).serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            0,
            0,
            0,
            0,
            place_bytes[0],
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(20);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.event.serialize_into(bytes);
        self.window.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
        u8::from(self.place).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
    }
}
impl From<&CirculateNotifyEvent> for [u8; 32] {
    fn from(input: &CirculateNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let event_bytes = input.event.serialize();
        let window_bytes = input.window.serialize();
        let place_bytes = u8::from(input.place).serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_bytes[0],
            event_bytes[1],
            event_bytes[2],
            event_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            0,
            0,
            0,
            0,
            place_bytes[0],
            0,
            0,
            0,
            // trailing padding
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
}
impl From<CirculateNotifyEvent> for [u8; 32] {
    fn from(input: CirculateNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the CirculateRequest event
pub const CIRCULATE_REQUEST_EVENT: u8 = 27;
pub type CirculateRequestEvent = CirculateNotifyEvent;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Property(u8);
impl Property {
    pub const NEW_VALUE: Self = Self(0);
    pub const DELETE: Self = Self(1);
}
impl From<Property> for u8 {
    #[inline]
    fn from(input: Property) -> Self {
        input.0
    }
}
impl From<Property> for Option<u8> {
    #[inline]
    fn from(input: Property) -> Self {
        Some(input.0)
    }
}
impl From<Property> for u16 {
    #[inline]
    fn from(input: Property) -> Self {
        u16::from(input.0)
    }
}
impl From<Property> for Option<u16> {
    #[inline]
    fn from(input: Property) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Property> for u32 {
    #[inline]
    fn from(input: Property) -> Self {
        u32::from(input.0)
    }
}
impl From<Property> for Option<u32> {
    #[inline]
    fn from(input: Property) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Property {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Property  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NEW_VALUE.0.into(), "NEW_VALUE", "NewValue"),
            (Self::DELETE.0.into(), "DELETE", "Delete"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the PropertyNotify event
pub const PROPERTY_NOTIFY_EVENT: u8 = 28;
/// a window property changed.
///
/// # Fields
///
/// * `window` - The window whose associated property was changed.
/// * `atom` - The property's atom, to indicate which property was changed.
/// * `time` - A timestamp of the server time when the property was changed.
/// * `state` -
///
/// # See
///
/// * `ChangeProperty`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PropertyNotifyEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub window: Window,
    pub atom: Atom,
    pub time: Timestamp,
    pub state: Property,
}
impl_debug_if_no_extra_traits!(PropertyNotifyEvent, "PropertyNotifyEvent");
impl TryParse for PropertyNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let (atom, remaining) = Atom::try_parse(remaining)?;
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let (state, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let state = state.into();
        let result = PropertyNotifyEvent { response_type, sequence, window, atom, time, state };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for PropertyNotifyEvent {
    type Bytes = [u8; 20];
    fn serialize(&self) -> [u8; 20] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let window_bytes = self.window.serialize();
        let atom_bytes = self.atom.serialize();
        let time_bytes = self.time.serialize();
        let state_bytes = u8::from(self.state).serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            atom_bytes[0],
            atom_bytes[1],
            atom_bytes[2],
            atom_bytes[3],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            state_bytes[0],
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(20);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.atom.serialize_into(bytes);
        self.time.serialize_into(bytes);
        u8::from(self.state).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
    }
}
impl From<&PropertyNotifyEvent> for [u8; 32] {
    fn from(input: &PropertyNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let window_bytes = input.window.serialize();
        let atom_bytes = input.atom.serialize();
        let time_bytes = input.time.serialize();
        let state_bytes = u8::from(input.state).serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            atom_bytes[0],
            atom_bytes[1],
            atom_bytes[2],
            atom_bytes[3],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            state_bytes[0],
            0,
            0,
            0,
            // trailing padding
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
}
impl From<PropertyNotifyEvent> for [u8; 32] {
    fn from(input: PropertyNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the SelectionClear event
pub const SELECTION_CLEAR_EVENT: u8 = 29;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectionClearEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub time: Timestamp,
    pub owner: Window,
    pub selection: Atom,
}
impl_debug_if_no_extra_traits!(SelectionClearEvent, "SelectionClearEvent");
impl TryParse for SelectionClearEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let (owner, remaining) = Window::try_parse(remaining)?;
        let (selection, remaining) = Atom::try_parse(remaining)?;
        let result = SelectionClearEvent { response_type, sequence, time, owner, selection };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for SelectionClearEvent {
    type Bytes = [u8; 16];
    fn serialize(&self) -> [u8; 16] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let owner_bytes = self.owner.serialize();
        let selection_bytes = self.selection.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            owner_bytes[0],
            owner_bytes[1],
            owner_bytes[2],
            owner_bytes[3],
            selection_bytes[0],
            selection_bytes[1],
            selection_bytes[2],
            selection_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.owner.serialize_into(bytes);
        self.selection.serialize_into(bytes);
    }
}
impl From<&SelectionClearEvent> for [u8; 32] {
    fn from(input: &SelectionClearEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let owner_bytes = input.owner.serialize();
        let selection_bytes = input.selection.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            owner_bytes[0],
            owner_bytes[1],
            owner_bytes[2],
            owner_bytes[3],
            selection_bytes[0],
            selection_bytes[1],
            selection_bytes[2],
            selection_bytes[3],
            // trailing padding
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
}
impl From<SelectionClearEvent> for [u8; 32] {
    fn from(input: SelectionClearEvent) -> Self {
        Self::from(&input)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Time(u8);
impl Time {
    pub const CURRENT_TIME: Self = Self(0);
}
impl From<Time> for u8 {
    #[inline]
    fn from(input: Time) -> Self {
        input.0
    }
}
impl From<Time> for Option<u8> {
    #[inline]
    fn from(input: Time) -> Self {
        Some(input.0)
    }
}
impl From<Time> for u16 {
    #[inline]
    fn from(input: Time) -> Self {
        u16::from(input.0)
    }
}
impl From<Time> for Option<u16> {
    #[inline]
    fn from(input: Time) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Time> for u32 {
    #[inline]
    fn from(input: Time) -> Self {
        u32::from(input.0)
    }
}
impl From<Time> for Option<u32> {
    #[inline]
    fn from(input: Time) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Time {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Time  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::CURRENT_TIME.0.into(), "CURRENT_TIME", "CurrentTime"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AtomEnum(u8);
impl AtomEnum {
    pub const NONE: Self = Self(0);
    pub const ANY: Self = Self(0);
    pub const PRIMARY: Self = Self(1);
    pub const SECONDARY: Self = Self(2);
    pub const ARC: Self = Self(3);
    pub const ATOM: Self = Self(4);
    pub const BITMAP: Self = Self(5);
    pub const CARDINAL: Self = Self(6);
    pub const COLORMAP: Self = Self(7);
    pub const CURSOR: Self = Self(8);
    pub const CUT_BUFFE_R0: Self = Self(9);
    pub const CUT_BUFFE_R1: Self = Self(10);
    pub const CUT_BUFFE_R2: Self = Self(11);
    pub const CUT_BUFFE_R3: Self = Self(12);
    pub const CUT_BUFFE_R4: Self = Self(13);
    pub const CUT_BUFFE_R5: Self = Self(14);
    pub const CUT_BUFFE_R6: Self = Self(15);
    pub const CUT_BUFFE_R7: Self = Self(16);
    pub const DRAWABLE: Self = Self(17);
    pub const FONT: Self = Self(18);
    pub const INTEGER: Self = Self(19);
    pub const PIXMAP: Self = Self(20);
    pub const POINT: Self = Self(21);
    pub const RECTANGLE: Self = Self(22);
    pub const RESOURCE_MANAGER: Self = Self(23);
    pub const RGB_COLOR_MAP: Self = Self(24);
    pub const RGB_BEST_MAP: Self = Self(25);
    pub const RGB_BLUE_MAP: Self = Self(26);
    pub const RGB_DEFAULT_MAP: Self = Self(27);
    pub const RGB_GRAY_MAP: Self = Self(28);
    pub const RGB_GREEN_MAP: Self = Self(29);
    pub const RGB_RED_MAP: Self = Self(30);
    pub const STRING: Self = Self(31);
    pub const VISUALID: Self = Self(32);
    pub const WINDOW: Self = Self(33);
    pub const WM_COMMAND: Self = Self(34);
    pub const WM_HINTS: Self = Self(35);
    pub const WM_CLIENT_MACHINE: Self = Self(36);
    pub const WM_ICON_NAME: Self = Self(37);
    pub const WM_ICON_SIZE: Self = Self(38);
    pub const WM_NAME: Self = Self(39);
    pub const WM_NORMAL_HINTS: Self = Self(40);
    pub const WM_SIZE_HINTS: Self = Self(41);
    pub const WM_ZOOM_HINTS: Self = Self(42);
    pub const MIN_SPACE: Self = Self(43);
    pub const NORM_SPACE: Self = Self(44);
    pub const MAX_SPACE: Self = Self(45);
    pub const END_SPACE: Self = Self(46);
    pub const SUPERSCRIPT_X: Self = Self(47);
    pub const SUPERSCRIPT_Y: Self = Self(48);
    pub const SUBSCRIPT_X: Self = Self(49);
    pub const SUBSCRIPT_Y: Self = Self(50);
    pub const UNDERLINE_POSITION: Self = Self(51);
    pub const UNDERLINE_THICKNESS: Self = Self(52);
    pub const STRIKEOUT_ASCENT: Self = Self(53);
    pub const STRIKEOUT_DESCENT: Self = Self(54);
    pub const ITALIC_ANGLE: Self = Self(55);
    pub const X_HEIGHT: Self = Self(56);
    pub const QUAD_WIDTH: Self = Self(57);
    pub const WEIGHT: Self = Self(58);
    pub const POINT_SIZE: Self = Self(59);
    pub const RESOLUTION: Self = Self(60);
    pub const COPYRIGHT: Self = Self(61);
    pub const NOTICE: Self = Self(62);
    pub const FONT_NAME: Self = Self(63);
    pub const FAMILY_NAME: Self = Self(64);
    pub const FULL_NAME: Self = Self(65);
    pub const CAP_HEIGHT: Self = Self(66);
    pub const WM_CLASS: Self = Self(67);
    pub const WM_TRANSIENT_FOR: Self = Self(68);
}
impl From<AtomEnum> for u8 {
    #[inline]
    fn from(input: AtomEnum) -> Self {
        input.0
    }
}
impl From<AtomEnum> for Option<u8> {
    #[inline]
    fn from(input: AtomEnum) -> Self {
        Some(input.0)
    }
}
impl From<AtomEnum> for u16 {
    #[inline]
    fn from(input: AtomEnum) -> Self {
        u16::from(input.0)
    }
}
impl From<AtomEnum> for Option<u16> {
    #[inline]
    fn from(input: AtomEnum) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<AtomEnum> for u32 {
    #[inline]
    fn from(input: AtomEnum) -> Self {
        u32::from(input.0)
    }
}
impl From<AtomEnum> for Option<u32> {
    #[inline]
    fn from(input: AtomEnum) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for AtomEnum {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for AtomEnum  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NONE.0.into(), "NONE", "None"),
            (Self::ANY.0.into(), "ANY", "Any"),
            (Self::PRIMARY.0.into(), "PRIMARY", "PRIMARY"),
            (Self::SECONDARY.0.into(), "SECONDARY", "SECONDARY"),
            (Self::ARC.0.into(), "ARC", "ARC"),
            (Self::ATOM.0.into(), "ATOM", "ATOM"),
            (Self::BITMAP.0.into(), "BITMAP", "BITMAP"),
            (Self::CARDINAL.0.into(), "CARDINAL", "CARDINAL"),
            (Self::COLORMAP.0.into(), "COLORMAP", "COLORMAP"),
            (Self::CURSOR.0.into(), "CURSOR", "CURSOR"),
            (Self::CUT_BUFFE_R0.0.into(), "CUT_BUFFE_R0", "CUT_BUFFER0"),
            (Self::CUT_BUFFE_R1.0.into(), "CUT_BUFFE_R1", "CUT_BUFFER1"),
            (Self::CUT_BUFFE_R2.0.into(), "CUT_BUFFE_R2", "CUT_BUFFER2"),
            (Self::CUT_BUFFE_R3.0.into(), "CUT_BUFFE_R3", "CUT_BUFFER3"),
            (Self::CUT_BUFFE_R4.0.into(), "CUT_BUFFE_R4", "CUT_BUFFER4"),
            (Self::CUT_BUFFE_R5.0.into(), "CUT_BUFFE_R5", "CUT_BUFFER5"),
            (Self::CUT_BUFFE_R6.0.into(), "CUT_BUFFE_R6", "CUT_BUFFER6"),
            (Self::CUT_BUFFE_R7.0.into(), "CUT_BUFFE_R7", "CUT_BUFFER7"),
            (Self::DRAWABLE.0.into(), "DRAWABLE", "DRAWABLE"),
            (Self::FONT.0.into(), "FONT", "FONT"),
            (Self::INTEGER.0.into(), "INTEGER", "INTEGER"),
            (Self::PIXMAP.0.into(), "PIXMAP", "PIXMAP"),
            (Self::POINT.0.into(), "POINT", "POINT"),
            (Self::RECTANGLE.0.into(), "RECTANGLE", "RECTANGLE"),
            (Self::RESOURCE_MANAGER.0.into(), "RESOURCE_MANAGER", "RESOURCE_MANAGER"),
            (Self::RGB_COLOR_MAP.0.into(), "RGB_COLOR_MAP", "RGB_COLOR_MAP"),
            (Self::RGB_BEST_MAP.0.into(), "RGB_BEST_MAP", "RGB_BEST_MAP"),
            (Self::RGB_BLUE_MAP.0.into(), "RGB_BLUE_MAP", "RGB_BLUE_MAP"),
            (Self::RGB_DEFAULT_MAP.0.into(), "RGB_DEFAULT_MAP", "RGB_DEFAULT_MAP"),
            (Self::RGB_GRAY_MAP.0.into(), "RGB_GRAY_MAP", "RGB_GRAY_MAP"),
            (Self::RGB_GREEN_MAP.0.into(), "RGB_GREEN_MAP", "RGB_GREEN_MAP"),
            (Self::RGB_RED_MAP.0.into(), "RGB_RED_MAP", "RGB_RED_MAP"),
            (Self::STRING.0.into(), "STRING", "STRING"),
            (Self::VISUALID.0.into(), "VISUALID", "VISUALID"),
            (Self::WINDOW.0.into(), "WINDOW", "WINDOW"),
            (Self::WM_COMMAND.0.into(), "WM_COMMAND", "WM_COMMAND"),
            (Self::WM_HINTS.0.into(), "WM_HINTS", "WM_HINTS"),
            (Self::WM_CLIENT_MACHINE.0.into(), "WM_CLIENT_MACHINE", "WM_CLIENT_MACHINE"),
            (Self::WM_ICON_NAME.0.into(), "WM_ICON_NAME", "WM_ICON_NAME"),
            (Self::WM_ICON_SIZE.0.into(), "WM_ICON_SIZE", "WM_ICON_SIZE"),
            (Self::WM_NAME.0.into(), "WM_NAME", "WM_NAME"),
            (Self::WM_NORMAL_HINTS.0.into(), "WM_NORMAL_HINTS", "WM_NORMAL_HINTS"),
            (Self::WM_SIZE_HINTS.0.into(), "WM_SIZE_HINTS", "WM_SIZE_HINTS"),
            (Self::WM_ZOOM_HINTS.0.into(), "WM_ZOOM_HINTS", "WM_ZOOM_HINTS"),
            (Self::MIN_SPACE.0.into(), "MIN_SPACE", "MIN_SPACE"),
            (Self::NORM_SPACE.0.into(), "NORM_SPACE", "NORM_SPACE"),
            (Self::MAX_SPACE.0.into(), "MAX_SPACE", "MAX_SPACE"),
            (Self::END_SPACE.0.into(), "END_SPACE", "END_SPACE"),
            (Self::SUPERSCRIPT_X.0.into(), "SUPERSCRIPT_X", "SUPERSCRIPT_X"),
            (Self::SUPERSCRIPT_Y.0.into(), "SUPERSCRIPT_Y", "SUPERSCRIPT_Y"),
            (Self::SUBSCRIPT_X.0.into(), "SUBSCRIPT_X", "SUBSCRIPT_X"),
            (Self::SUBSCRIPT_Y.0.into(), "SUBSCRIPT_Y", "SUBSCRIPT_Y"),
            (Self::UNDERLINE_POSITION.0.into(), "UNDERLINE_POSITION", "UNDERLINE_POSITION"),
            (Self::UNDERLINE_THICKNESS.0.into(), "UNDERLINE_THICKNESS", "UNDERLINE_THICKNESS"),
            (Self::STRIKEOUT_ASCENT.0.into(), "STRIKEOUT_ASCENT", "STRIKEOUT_ASCENT"),
            (Self::STRIKEOUT_DESCENT.0.into(), "STRIKEOUT_DESCENT", "STRIKEOUT_DESCENT"),
            (Self::ITALIC_ANGLE.0.into(), "ITALIC_ANGLE", "ITALIC_ANGLE"),
            (Self::X_HEIGHT.0.into(), "X_HEIGHT", "X_HEIGHT"),
            (Self::QUAD_WIDTH.0.into(), "QUAD_WIDTH", "QUAD_WIDTH"),
            (Self::WEIGHT.0.into(), "WEIGHT", "WEIGHT"),
            (Self::POINT_SIZE.0.into(), "POINT_SIZE", "POINT_SIZE"),
            (Self::RESOLUTION.0.into(), "RESOLUTION", "RESOLUTION"),
            (Self::COPYRIGHT.0.into(), "COPYRIGHT", "COPYRIGHT"),
            (Self::NOTICE.0.into(), "NOTICE", "NOTICE"),
            (Self::FONT_NAME.0.into(), "FONT_NAME", "FONT_NAME"),
            (Self::FAMILY_NAME.0.into(), "FAMILY_NAME", "FAMILY_NAME"),
            (Self::FULL_NAME.0.into(), "FULL_NAME", "FULL_NAME"),
            (Self::CAP_HEIGHT.0.into(), "CAP_HEIGHT", "CAP_HEIGHT"),
            (Self::WM_CLASS.0.into(), "WM_CLASS", "WM_CLASS"),
            (Self::WM_TRANSIENT_FOR.0.into(), "WM_TRANSIENT_FOR", "WM_TRANSIENT_FOR"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the SelectionRequest event
pub const SELECTION_REQUEST_EVENT: u8 = 30;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectionRequestEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub time: Timestamp,
    pub owner: Window,
    pub requestor: Window,
    pub selection: Atom,
    pub target: Atom,
    pub property: Atom,
}
impl_debug_if_no_extra_traits!(SelectionRequestEvent, "SelectionRequestEvent");
impl TryParse for SelectionRequestEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let (owner, remaining) = Window::try_parse(remaining)?;
        let (requestor, remaining) = Window::try_parse(remaining)?;
        let (selection, remaining) = Atom::try_parse(remaining)?;
        let (target, remaining) = Atom::try_parse(remaining)?;
        let (property, remaining) = Atom::try_parse(remaining)?;
        let result = SelectionRequestEvent { response_type, sequence, time, owner, requestor, selection, target, property };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for SelectionRequestEvent {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let owner_bytes = self.owner.serialize();
        let requestor_bytes = self.requestor.serialize();
        let selection_bytes = self.selection.serialize();
        let target_bytes = self.target.serialize();
        let property_bytes = self.property.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            owner_bytes[0],
            owner_bytes[1],
            owner_bytes[2],
            owner_bytes[3],
            requestor_bytes[0],
            requestor_bytes[1],
            requestor_bytes[2],
            requestor_bytes[3],
            selection_bytes[0],
            selection_bytes[1],
            selection_bytes[2],
            selection_bytes[3],
            target_bytes[0],
            target_bytes[1],
            target_bytes[2],
            target_bytes[3],
            property_bytes[0],
            property_bytes[1],
            property_bytes[2],
            property_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(28);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.owner.serialize_into(bytes);
        self.requestor.serialize_into(bytes);
        self.selection.serialize_into(bytes);
        self.target.serialize_into(bytes);
        self.property.serialize_into(bytes);
    }
}
impl From<&SelectionRequestEvent> for [u8; 32] {
    fn from(input: &SelectionRequestEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let owner_bytes = input.owner.serialize();
        let requestor_bytes = input.requestor.serialize();
        let selection_bytes = input.selection.serialize();
        let target_bytes = input.target.serialize();
        let property_bytes = input.property.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            owner_bytes[0],
            owner_bytes[1],
            owner_bytes[2],
            owner_bytes[3],
            requestor_bytes[0],
            requestor_bytes[1],
            requestor_bytes[2],
            requestor_bytes[3],
            selection_bytes[0],
            selection_bytes[1],
            selection_bytes[2],
            selection_bytes[3],
            target_bytes[0],
            target_bytes[1],
            target_bytes[2],
            target_bytes[3],
            property_bytes[0],
            property_bytes[1],
            property_bytes[2],
            property_bytes[3],
            // trailing padding
            0,
            0,
            0,
            0,
        ]
    }
}
impl From<SelectionRequestEvent> for [u8; 32] {
    fn from(input: SelectionRequestEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the SelectionNotify event
pub const SELECTION_NOTIFY_EVENT: u8 = 31;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectionNotifyEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub time: Timestamp,
    pub requestor: Window,
    pub selection: Atom,
    pub target: Atom,
    pub property: Atom,
}
impl_debug_if_no_extra_traits!(SelectionNotifyEvent, "SelectionNotifyEvent");
impl TryParse for SelectionNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let (requestor, remaining) = Window::try_parse(remaining)?;
        let (selection, remaining) = Atom::try_parse(remaining)?;
        let (target, remaining) = Atom::try_parse(remaining)?;
        let (property, remaining) = Atom::try_parse(remaining)?;
        let result = SelectionNotifyEvent { response_type, sequence, time, requestor, selection, target, property };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for SelectionNotifyEvent {
    type Bytes = [u8; 24];
    fn serialize(&self) -> [u8; 24] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let requestor_bytes = self.requestor.serialize();
        let selection_bytes = self.selection.serialize();
        let target_bytes = self.target.serialize();
        let property_bytes = self.property.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            requestor_bytes[0],
            requestor_bytes[1],
            requestor_bytes[2],
            requestor_bytes[3],
            selection_bytes[0],
            selection_bytes[1],
            selection_bytes[2],
            selection_bytes[3],
            target_bytes[0],
            target_bytes[1],
            target_bytes[2],
            target_bytes[3],
            property_bytes[0],
            property_bytes[1],
            property_bytes[2],
            property_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(24);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.requestor.serialize_into(bytes);
        self.selection.serialize_into(bytes);
        self.target.serialize_into(bytes);
        self.property.serialize_into(bytes);
    }
}
impl From<&SelectionNotifyEvent> for [u8; 32] {
    fn from(input: &SelectionNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let requestor_bytes = input.requestor.serialize();
        let selection_bytes = input.selection.serialize();
        let target_bytes = input.target.serialize();
        let property_bytes = input.property.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            requestor_bytes[0],
            requestor_bytes[1],
            requestor_bytes[2],
            requestor_bytes[3],
            selection_bytes[0],
            selection_bytes[1],
            selection_bytes[2],
            selection_bytes[3],
            target_bytes[0],
            target_bytes[1],
            target_bytes[2],
            target_bytes[3],
            property_bytes[0],
            property_bytes[1],
            property_bytes[2],
            property_bytes[3],
            // trailing padding
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
}
impl From<SelectionNotifyEvent> for [u8; 32] {
    fn from(input: SelectionNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// # Fields
///
/// * `Uninstalled` - The colormap was uninstalled.
/// * `Installed` - The colormap was installed.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ColormapState(u8);
impl ColormapState {
    pub const UNINSTALLED: Self = Self(0);
    pub const INSTALLED: Self = Self(1);
}
impl From<ColormapState> for u8 {
    #[inline]
    fn from(input: ColormapState) -> Self {
        input.0
    }
}
impl From<ColormapState> for Option<u8> {
    #[inline]
    fn from(input: ColormapState) -> Self {
        Some(input.0)
    }
}
impl From<ColormapState> for u16 {
    #[inline]
    fn from(input: ColormapState) -> Self {
        u16::from(input.0)
    }
}
impl From<ColormapState> for Option<u16> {
    #[inline]
    fn from(input: ColormapState) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<ColormapState> for u32 {
    #[inline]
    fn from(input: ColormapState) -> Self {
        u32::from(input.0)
    }
}
impl From<ColormapState> for Option<u32> {
    #[inline]
    fn from(input: ColormapState) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ColormapState {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ColormapState  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::UNINSTALLED.0.into(), "UNINSTALLED", "Uninstalled"),
            (Self::INSTALLED.0.into(), "INSTALLED", "Installed"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ColormapEnum(u8);
impl ColormapEnum {
    pub const NONE: Self = Self(0);
}
impl From<ColormapEnum> for u8 {
    #[inline]
    fn from(input: ColormapEnum) -> Self {
        input.0
    }
}
impl From<ColormapEnum> for Option<u8> {
    #[inline]
    fn from(input: ColormapEnum) -> Self {
        Some(input.0)
    }
}
impl From<ColormapEnum> for u16 {
    #[inline]
    fn from(input: ColormapEnum) -> Self {
        u16::from(input.0)
    }
}
impl From<ColormapEnum> for Option<u16> {
    #[inline]
    fn from(input: ColormapEnum) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<ColormapEnum> for u32 {
    #[inline]
    fn from(input: ColormapEnum) -> Self {
        u32::from(input.0)
    }
}
impl From<ColormapEnum> for Option<u32> {
    #[inline]
    fn from(input: ColormapEnum) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ColormapEnum {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ColormapEnum  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NONE.0.into(), "NONE", "None"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the ColormapNotify event
pub const COLORMAP_NOTIFY_EVENT: u8 = 32;
/// the colormap for some window changed.
///
/// # Fields
///
/// * `window` - The window whose associated colormap is changed, installed or uninstalled.
/// * `colormap` - The colormap which is changed, installed or uninstalled. This is `XCB_NONE`
/// when the colormap is changed by a call to `FreeColormap`.
/// * `_new` - Indicates whether the colormap was changed (1) or installed/uninstalled (0).
/// * `state` -
///
/// # See
///
/// * `FreeColormap`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ColormapNotifyEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub window: Window,
    pub colormap: Colormap,
    pub new: bool,
    pub state: ColormapState,
}
impl_debug_if_no_extra_traits!(ColormapNotifyEvent, "ColormapNotifyEvent");
impl TryParse for ColormapNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let (colormap, remaining) = Colormap::try_parse(remaining)?;
        let (new, remaining) = bool::try_parse(remaining)?;
        let (state, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let state = state.into();
        let result = ColormapNotifyEvent { response_type, sequence, window, colormap, new, state };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ColormapNotifyEvent {
    type Bytes = [u8; 16];
    fn serialize(&self) -> [u8; 16] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let window_bytes = self.window.serialize();
        let colormap_bytes = self.colormap.serialize();
        let new_bytes = self.new.serialize();
        let state_bytes = u8::from(self.state).serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            colormap_bytes[0],
            colormap_bytes[1],
            colormap_bytes[2],
            colormap_bytes[3],
            new_bytes[0],
            state_bytes[0],
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.colormap.serialize_into(bytes);
        self.new.serialize_into(bytes);
        u8::from(self.state).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
    }
}
impl From<&ColormapNotifyEvent> for [u8; 32] {
    fn from(input: &ColormapNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let window_bytes = input.window.serialize();
        let colormap_bytes = input.colormap.serialize();
        let new_bytes = input.new.serialize();
        let state_bytes = u8::from(input.state).serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            colormap_bytes[0],
            colormap_bytes[1],
            colormap_bytes[2],
            colormap_bytes[3],
            new_bytes[0],
            state_bytes[0],
            0,
            0,
            // trailing padding
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
}
impl From<ColormapNotifyEvent> for [u8; 32] {
    fn from(input: ColormapNotifyEvent) -> Self {
        Self::from(&input)
    }
}

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClientMessageData([u8; 20]);
impl ClientMessageData {
    pub fn as_data8(&self) -> [u8; 20] {
        fn do_the_parse(remaining: &[u8]) -> Result<[u8; 20], ParseError> {
            let (data8, remaining) = crate::x11_utils::parse_u8_array::<20>(remaining)?;
            let _ = remaining;
            Ok(data8)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_data16(&self) -> [u16; 10] {
        fn do_the_parse(remaining: &[u8]) -> Result<[u16; 10], ParseError> {
            let (data16_0, remaining) = u16::try_parse(remaining)?;
            let (data16_1, remaining) = u16::try_parse(remaining)?;
            let (data16_2, remaining) = u16::try_parse(remaining)?;
            let (data16_3, remaining) = u16::try_parse(remaining)?;
            let (data16_4, remaining) = u16::try_parse(remaining)?;
            let (data16_5, remaining) = u16::try_parse(remaining)?;
            let (data16_6, remaining) = u16::try_parse(remaining)?;
            let (data16_7, remaining) = u16::try_parse(remaining)?;
            let (data16_8, remaining) = u16::try_parse(remaining)?;
            let (data16_9, remaining) = u16::try_parse(remaining)?;
            let data16 = [
                data16_0,
                data16_1,
                data16_2,
                data16_3,
                data16_4,
                data16_5,
                data16_6,
                data16_7,
                data16_8,
                data16_9,
            ];
            let _ = remaining;
            Ok(data16)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_data32(&self) -> [u32; 5] {
        fn do_the_parse(remaining: &[u8]) -> Result<[u32; 5], ParseError> {
            let (data32_0, remaining) = u32::try_parse(remaining)?;
            let (data32_1, remaining) = u32::try_parse(remaining)?;
            let (data32_2, remaining) = u32::try_parse(remaining)?;
            let (data32_3, remaining) = u32::try_parse(remaining)?;
            let (data32_4, remaining) = u32::try_parse(remaining)?;
            let data32 = [
                data32_0,
                data32_1,
                data32_2,
                data32_3,
                data32_4,
            ];
            let _ = remaining;
            Ok(data32)
        }
        do_the_parse(&self.0).unwrap()
    }
}
impl Serialize for ClientMessageData {
    type Bytes = [u8; 20];
    fn serialize(&self) -> [u8; 20] {
        self.0
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.0);
    }
}
impl TryParse for ClientMessageData {
    fn try_parse(value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let inner: [u8; 20] = value.get(..20)
            .ok_or(ParseError::InsufficientData)?
            .try_into()
            .unwrap();
        let result = ClientMessageData(inner);
        Ok((result, &value[20..]))
    }
}
impl From<[u8; 20]> for ClientMessageData {
    fn from(data8: [u8; 20]) -> Self {
        Self(data8)
    }
}
impl From<[u16; 10]> for ClientMessageData {
    fn from(data16: [u16; 10]) -> Self {
        let data16_0_bytes = data16[0].serialize();
        let data16_1_bytes = data16[1].serialize();
        let data16_2_bytes = data16[2].serialize();
        let data16_3_bytes = data16[3].serialize();
        let data16_4_bytes = data16[4].serialize();
        let data16_5_bytes = data16[5].serialize();
        let data16_6_bytes = data16[6].serialize();
        let data16_7_bytes = data16[7].serialize();
        let data16_8_bytes = data16[8].serialize();
        let data16_9_bytes = data16[9].serialize();
        let value = [
            data16_0_bytes[0],
            data16_0_bytes[1],
            data16_1_bytes[0],
            data16_1_bytes[1],
            data16_2_bytes[0],
            data16_2_bytes[1],
            data16_3_bytes[0],
            data16_3_bytes[1],
            data16_4_bytes[0],
            data16_4_bytes[1],
            data16_5_bytes[0],
            data16_5_bytes[1],
            data16_6_bytes[0],
            data16_6_bytes[1],
            data16_7_bytes[0],
            data16_7_bytes[1],
            data16_8_bytes[0],
            data16_8_bytes[1],
            data16_9_bytes[0],
            data16_9_bytes[1],
        ];
        Self(value)
    }
}
impl From<[u32; 5]> for ClientMessageData {
    fn from(data32: [u32; 5]) -> Self {
        let data32_0_bytes = data32[0].serialize();
        let data32_1_bytes = data32[1].serialize();
        let data32_2_bytes = data32[2].serialize();
        let data32_3_bytes = data32[3].serialize();
        let data32_4_bytes = data32[4].serialize();
        let value = [
            data32_0_bytes[0],
            data32_0_bytes[1],
            data32_0_bytes[2],
            data32_0_bytes[3],
            data32_1_bytes[0],
            data32_1_bytes[1],
            data32_1_bytes[2],
            data32_1_bytes[3],
            data32_2_bytes[0],
            data32_2_bytes[1],
            data32_2_bytes[2],
            data32_2_bytes[3],
            data32_3_bytes[0],
            data32_3_bytes[1],
            data32_3_bytes[2],
            data32_3_bytes[3],
            data32_4_bytes[0],
            data32_4_bytes[1],
            data32_4_bytes[2],
            data32_4_bytes[3],
        ];
        Self(value)
    }
}

/// Opcode for the ClientMessage event
pub const CLIENT_MESSAGE_EVENT: u8 = 33;
/// NOT YET DOCUMENTED.
///
/// This event represents a ClientMessage, sent by another X11 client. An example
/// is a client sending the `_NET_WM_STATE` ClientMessage to the root window
/// to indicate the fullscreen window state, effectively requesting that the window
/// manager puts it into fullscreen mode.
///
/// # Fields
///
/// * `format` - Specifies how to interpret `data`. Can be either 8, 16 or 32.
/// * `type` - An atom which indicates how the data should be interpreted by the receiving
/// client.
/// * `data` - The data itself (20 bytes max).
///
/// # See
///
/// * `SendEvent`: request
#[derive(Clone, Copy)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClientMessageEvent {
    pub response_type: u8,
    pub format: u8,
    pub sequence: u16,
    pub window: Window,
    pub type_: Atom,
    pub data: ClientMessageData,
}
impl_debug_if_no_extra_traits!(ClientMessageEvent, "ClientMessageEvent");
impl TryParse for ClientMessageEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (format, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let (type_, remaining) = Atom::try_parse(remaining)?;
        let (data, remaining) = ClientMessageData::try_parse(remaining)?;
        let result = ClientMessageEvent { response_type, format, sequence, window, type_, data };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ClientMessageEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let format_bytes = self.format.serialize();
        let sequence_bytes = self.sequence.serialize();
        let window_bytes = self.window.serialize();
        let type_bytes = self.type_.serialize();
        let data_bytes = self.data.serialize();
        [
            response_type_bytes[0],
            format_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            type_bytes[0],
            type_bytes[1],
            type_bytes[2],
            type_bytes[3],
            data_bytes[0],
            data_bytes[1],
            data_bytes[2],
            data_bytes[3],
            data_bytes[4],
            data_bytes[5],
            data_bytes[6],
            data_bytes[7],
            data_bytes[8],
            data_bytes[9],
            data_bytes[10],
            data_bytes[11],
            data_bytes[12],
            data_bytes[13],
            data_bytes[14],
            data_bytes[15],
            data_bytes[16],
            data_bytes[17],
            data_bytes[18],
            data_bytes[19],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        self.format.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.type_.serialize_into(bytes);
        self.data.serialize_into(bytes);
    }
}
impl From<&ClientMessageEvent> for [u8; 32] {
    fn from(input: &ClientMessageEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let format_bytes = input.format.serialize();
        let sequence_bytes = input.sequence.serialize();
        let window_bytes = input.window.serialize();
        let type_bytes = input.type_.serialize();
        let data_bytes = input.data.serialize();
        [
            response_type_bytes[0],
            format_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            type_bytes[0],
            type_bytes[1],
            type_bytes[2],
            type_bytes[3],
            data_bytes[0],
            data_bytes[1],
            data_bytes[2],
            data_bytes[3],
            data_bytes[4],
            data_bytes[5],
            data_bytes[6],
            data_bytes[7],
            data_bytes[8],
            data_bytes[9],
            data_bytes[10],
            data_bytes[11],
            data_bytes[12],
            data_bytes[13],
            data_bytes[14],
            data_bytes[15],
            data_bytes[16],
            data_bytes[17],
            data_bytes[18],
            data_bytes[19],
        ]
    }
}
impl From<ClientMessageEvent> for [u8; 32] {
    fn from(input: ClientMessageEvent) -> Self {
        Self::from(&input)
    }
}
impl ClientMessageEvent {
    /// Create a new `ClientMessageEvent`.
    ///
    /// This function simplifies the creation of a `ClientMessageEvent` by applying
    /// some useful defaults:
    /// - `response_type = CLIENT_MESSAGE_EVENT`
    /// - `sequence = 0`
    ///
    /// The other fields are set from the parameters given to this function.
    pub fn new(format: u8, window: Window, type_: impl Into<Atom>, data: impl Into<ClientMessageData>) -> Self {
        Self {
            response_type: CLIENT_MESSAGE_EVENT,
            format,
            sequence: 0,
            window,
            type_: type_.into(),
            data: data.into(),
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mapping(u8);
impl Mapping {
    pub const MODIFIER: Self = Self(0);
    pub const KEYBOARD: Self = Self(1);
    pub const POINTER: Self = Self(2);
}
impl From<Mapping> for u8 {
    #[inline]
    fn from(input: Mapping) -> Self {
        input.0
    }
}
impl From<Mapping> for Option<u8> {
    #[inline]
    fn from(input: Mapping) -> Self {
        Some(input.0)
    }
}
impl From<Mapping> for u16 {
    #[inline]
    fn from(input: Mapping) -> Self {
        u16::from(input.0)
    }
}
impl From<Mapping> for Option<u16> {
    #[inline]
    fn from(input: Mapping) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Mapping> for u32 {
    #[inline]
    fn from(input: Mapping) -> Self {
        u32::from(input.0)
    }
}
impl From<Mapping> for Option<u32> {
    #[inline]
    fn from(input: Mapping) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Mapping {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Mapping  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::MODIFIER.0.into(), "MODIFIER", "Modifier"),
            (Self::KEYBOARD.0.into(), "KEYBOARD", "Keyboard"),
            (Self::POINTER.0.into(), "POINTER", "Pointer"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the MappingNotify event
pub const MAPPING_NOTIFY_EVENT: u8 = 34;
/// keyboard mapping changed.
///
/// # Fields
///
/// * `request` -
/// * `first_keycode` - The first number in the range of the altered mapping.
/// * `count` - The number of keycodes altered.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MappingNotifyEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub request: Mapping,
    pub first_keycode: Keycode,
    pub count: u8,
}
impl_debug_if_no_extra_traits!(MappingNotifyEvent, "MappingNotifyEvent");
impl TryParse for MappingNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (request, remaining) = u8::try_parse(remaining)?;
        let (first_keycode, remaining) = Keycode::try_parse(remaining)?;
        let (count, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let request = request.into();
        let result = MappingNotifyEvent { response_type, sequence, request, first_keycode, count };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for MappingNotifyEvent {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let request_bytes = u8::from(self.request).serialize();
        let first_keycode_bytes = self.first_keycode.serialize();
        let count_bytes = self.count.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            request_bytes[0],
            first_keycode_bytes[0],
            count_bytes[0],
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        u8::from(self.request).serialize_into(bytes);
        self.first_keycode.serialize_into(bytes);
        self.count.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
    }
}
impl From<&MappingNotifyEvent> for [u8; 32] {
    fn from(input: &MappingNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let request_bytes = u8::from(input.request).serialize();
        let first_keycode_bytes = input.first_keycode.serialize();
        let count_bytes = input.count.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            request_bytes[0],
            first_keycode_bytes[0],
            count_bytes[0],
            0,
            // trailing padding
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
}
impl From<MappingNotifyEvent> for [u8; 32] {
    fn from(input: MappingNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the GeGeneric event
pub const GE_GENERIC_EVENT: u8 = 35;
/// generic event (with length).
///
/// # Fields
///
/// * `extension` - The major opcode of the extension creating this event
/// * `length` - The amount (in 4-byte units) of data beyond 32 bytes
/// * `evtype` - The extension-specific event type
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeGenericEvent {
    pub response_type: u8,
    pub extension: u8,
    pub sequence: u16,
    pub length: u32,
    pub event_type: u16,
}
impl_debug_if_no_extra_traits!(GeGenericEvent, "GeGenericEvent");
impl TryParse for GeGenericEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (extension, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (event_type, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
        let result = GeGenericEvent { response_type, extension, sequence, length, event_type };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GeGenericEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let extension_bytes = self.extension.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let event_type_bytes = self.event_type.serialize();
        [
            response_type_bytes[0],
            extension_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            event_type_bytes[0],
            event_type_bytes[1],
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
        self.response_type.serialize_into(bytes);
        self.extension.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.event_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
    }
}

/// Opcode for the Request error
pub const REQUEST_ERROR: u8 = 1;

/// Opcode for the Value error
pub const VALUE_ERROR: u8 = 2;

/// Opcode for the Window error
pub const WINDOW_ERROR: u8 = 3;

/// Opcode for the Pixmap error
pub const PIXMAP_ERROR: u8 = 4;

/// Opcode for the Atom error
pub const ATOM_ERROR: u8 = 5;

/// Opcode for the Cursor error
pub const CURSOR_ERROR: u8 = 6;

/// Opcode for the Font error
pub const FONT_ERROR: u8 = 7;

/// Opcode for the Match error
pub const MATCH_ERROR: u8 = 8;

/// Opcode for the Drawable error
pub const DRAWABLE_ERROR: u8 = 9;

/// Opcode for the Access error
pub const ACCESS_ERROR: u8 = 10;

/// Opcode for the Alloc error
pub const ALLOC_ERROR: u8 = 11;

/// Opcode for the Colormap error
pub const COLORMAP_ERROR: u8 = 12;

/// Opcode for the GContext error
pub const G_CONTEXT_ERROR: u8 = 13;

/// Opcode for the IDChoice error
pub const ID_CHOICE_ERROR: u8 = 14;

/// Opcode for the Name error
pub const NAME_ERROR: u8 = 15;

/// Opcode for the Length error
pub const LENGTH_ERROR: u8 = 16;

/// Opcode for the Implementation error
pub const IMPLEMENTATION_ERROR: u8 = 17;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WindowClass(u16);
impl WindowClass {
    pub const COPY_FROM_PARENT: Self = Self(0);
    pub const INPUT_OUTPUT: Self = Self(1);
    pub const INPUT_ONLY: Self = Self(2);
}
impl From<WindowClass> for u16 {
    #[inline]
    fn from(input: WindowClass) -> Self {
        input.0
    }
}
impl From<WindowClass> for Option<u16> {
    #[inline]
    fn from(input: WindowClass) -> Self {
        Some(input.0)
    }
}
impl From<WindowClass> for u32 {
    #[inline]
    fn from(input: WindowClass) -> Self {
        u32::from(input.0)
    }
}
impl From<WindowClass> for Option<u32> {
    #[inline]
    fn from(input: WindowClass) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for WindowClass {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for WindowClass {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for WindowClass  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::COPY_FROM_PARENT.0.into(), "COPY_FROM_PARENT", "CopyFromParent"),
            (Self::INPUT_OUTPUT.0.into(), "INPUT_OUTPUT", "InputOutput"),
            (Self::INPUT_ONLY.0.into(), "INPUT_ONLY", "InputOnly"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// # Fields
///
/// * `BackPixmap` - Overrides the default background-pixmap. The background pixmap and window must
/// have the same root and same depth. Any size pixmap can be used, although some
/// sizes may be faster than others.
///
/// If `XCB_BACK_PIXMAP_NONE` is specified, the window has no defined background.
/// The server may fill the contents with the previous screen contents or with
/// contents of its own choosing.
///
/// If `XCB_BACK_PIXMAP_PARENT_RELATIVE` is specified, the parent's background is
/// used, but the window must have the same depth as the parent (or a Match error
/// results).   The parent's background is tracked, and the current version is
/// used each time the window background is required.
/// * `BackPixel` - Overrides `BackPixmap`. A pixmap of undefined size filled with the specified
/// background pixel is used for the background. Range-checking is not performed,
/// the background pixel is truncated to the appropriate number of bits.
/// * `BorderPixmap` - Overrides the default border-pixmap. The border pixmap and window must have the
/// same root and the same depth. Any size pixmap can be used, although some sizes
/// may be faster than others.
///
/// The special value `XCB_COPY_FROM_PARENT` means the parent's border pixmap is
/// copied (subsequent changes to the parent's border attribute do not affect the
/// child), but the window must have the same depth as the parent.
/// * `BorderPixel` - Overrides `BorderPixmap`. A pixmap of undefined size filled with the specified
/// border pixel is used for the border. Range checking is not performed on the
/// border-pixel value, it is truncated to the appropriate number of bits.
/// * `BitGravity` - Defines which region of the window should be retained if the window is resized.
/// * `WinGravity` - Defines how the window should be repositioned if the parent is resized (see
/// `ConfigureWindow`).
/// * `BackingStore` - A backing-store of `WhenMapped` advises the server that maintaining contents of
/// obscured regions when the window is mapped would be beneficial. A backing-store
/// of `Always` advises the server that maintaining contents even when the window
/// is unmapped would be beneficial. In this case, the server may generate an
/// exposure event when the window is created. A value of `NotUseful` advises the
/// server that maintaining contents is unnecessary, although a server may still
/// choose to maintain contents while the window is mapped. Note that if the server
/// maintains contents, then the server should maintain complete contents not just
/// the region within the parent boundaries, even if the window is larger than its
/// parent. While the server maintains contents, exposure events will not normally
/// be generated, but the server may stop maintaining contents at any time.
/// * `BackingPlanes` - The backing-planes indicates (with bits set to 1) which bit planes of the
/// window hold dynamic data that must be preserved in backing-stores and during
/// save-unders.
/// * `BackingPixel` - The backing-pixel specifies what value to use in planes not covered by
/// backing-planes. The server is free to save only the specified bit planes in the
/// backing-store or save-under and regenerate the remaining planes with the
/// specified pixel value. Any bits beyond the specified depth of the window in
/// these values are simply ignored.
/// * `OverrideRedirect` - The override-redirect specifies whether map and configure requests on this
/// window should override a SubstructureRedirect on the parent, typically to
/// inform a window manager not to tamper with the window.
/// * `SaveUnder` - If 1, the server is advised that when this window is mapped, saving the
/// contents of windows it obscures would be beneficial.
/// * `EventMask` - The event-mask defines which events the client is interested in for this window
/// (or for some event types, inferiors of the window).
/// * `DontPropagate` - The do-not-propagate-mask defines which events should not be propagated to
/// ancestor windows when no client has the event type selected in this window.
/// * `Colormap` - The colormap specifies the colormap that best reflects the true colors of the window. Servers
/// capable of supporting multiple hardware colormaps may use this information, and window man-
/// agers may use it for InstallColormap requests. The colormap must have the same visual type
/// and root as the window (or a Match error results). If CopyFromParent is specified, the parent's
/// colormap is copied (subsequent changes to the parent's colormap attribute do not affect the child).
/// However, the window must have the same visual type as the parent (or a Match error results),
/// and the parent must not have a colormap of None (or a Match error results). For an explanation
/// of None, see FreeColormap request. The colormap is copied by sharing the colormap object
/// between the child and the parent, not by making a complete copy of the colormap contents.
/// * `Cursor` - If a cursor is specified, it will be used whenever the pointer is in the window. If None is speci-
/// fied, the parent's cursor will be used when the pointer is in the window, and any change in the
/// parent's cursor will cause an immediate change in the displayed cursor.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CW(u32);
impl CW {
    pub const BACK_PIXMAP: Self = Self(1 << 0);
    pub const BACK_PIXEL: Self = Self(1 << 1);
    pub const BORDER_PIXMAP: Self = Self(1 << 2);
    pub const BORDER_PIXEL: Self = Self(1 << 3);
    pub const BIT_GRAVITY: Self = Self(1 << 4);
    pub const WIN_GRAVITY: Self = Self(1 << 5);
    pub const BACKING_STORE: Self = Self(1 << 6);
    pub const BACKING_PLANES: Self = Self(1 << 7);
    pub const BACKING_PIXEL: Self = Self(1 << 8);
    pub const OVERRIDE_REDIRECT: Self = Self(1 << 9);
    pub const SAVE_UNDER: Self = Self(1 << 10);
    pub const EVENT_MASK: Self = Self(1 << 11);
    pub const DONT_PROPAGATE: Self = Self(1 << 12);
    pub const COLORMAP: Self = Self(1 << 13);
    pub const CURSOR: Self = Self(1 << 14);
}
impl From<CW> for u32 {
    #[inline]
    fn from(input: CW) -> Self {
        input.0
    }
}
impl From<CW> for Option<u32> {
    #[inline]
    fn from(input: CW) -> Self {
        Some(input.0)
    }
}
impl From<u8> for CW {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for CW {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for CW {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for CW  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::BACK_PIXMAP.0, "BACK_PIXMAP", "BackPixmap"),
            (Self::BACK_PIXEL.0, "BACK_PIXEL", "BackPixel"),
            (Self::BORDER_PIXMAP.0, "BORDER_PIXMAP", "BorderPixmap"),
            (Self::BORDER_PIXEL.0, "BORDER_PIXEL", "BorderPixel"),
            (Self::BIT_GRAVITY.0, "BIT_GRAVITY", "BitGravity"),
            (Self::WIN_GRAVITY.0, "WIN_GRAVITY", "WinGravity"),
            (Self::BACKING_STORE.0, "BACKING_STORE", "BackingStore"),
            (Self::BACKING_PLANES.0, "BACKING_PLANES", "BackingPlanes"),
            (Self::BACKING_PIXEL.0, "BACKING_PIXEL", "BackingPixel"),
            (Self::OVERRIDE_REDIRECT.0, "OVERRIDE_REDIRECT", "OverrideRedirect"),
            (Self::SAVE_UNDER.0, "SAVE_UNDER", "SaveUnder"),
            (Self::EVENT_MASK.0, "EVENT_MASK", "EventMask"),
            (Self::DONT_PROPAGATE.0, "DONT_PROPAGATE", "DontPropagate"),
            (Self::COLORMAP.0, "COLORMAP", "Colormap"),
            (Self::CURSOR.0, "CURSOR", "Cursor"),
        ];
        pretty_print_bitmask(fmt, self.0, &variants)
    }
}
bitmask_binop!(CW, u32);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BackPixmap(bool);
impl BackPixmap {
    pub const NONE: Self = Self(false);
    pub const PARENT_RELATIVE: Self = Self(true);
}
impl From<BackPixmap> for bool {
    #[inline]
    fn from(input: BackPixmap) -> Self {
        input.0
    }
}
impl From<BackPixmap> for Option<bool> {
    #[inline]
    fn from(input: BackPixmap) -> Self {
        Some(input.0)
    }
}
impl From<BackPixmap> for u8 {
    #[inline]
    fn from(input: BackPixmap) -> Self {
        u8::from(input.0)
    }
}
impl From<BackPixmap> for Option<u8> {
    #[inline]
    fn from(input: BackPixmap) -> Self {
        Some(u8::from(input.0))
    }
}
impl From<BackPixmap> for u16 {
    #[inline]
    fn from(input: BackPixmap) -> Self {
        u16::from(input.0)
    }
}
impl From<BackPixmap> for Option<u16> {
    #[inline]
    fn from(input: BackPixmap) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<BackPixmap> for u32 {
    #[inline]
    fn from(input: BackPixmap) -> Self {
        u32::from(input.0)
    }
}
impl From<BackPixmap> for Option<u32> {
    #[inline]
    fn from(input: BackPixmap) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<bool> for BackPixmap {
    #[inline]
    fn from(value: bool) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for BackPixmap  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NONE.0.into(), "NONE", "None"),
            (Self::PARENT_RELATIVE.0.into(), "PARENT_RELATIVE", "ParentRelative"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Gravity(u32);
impl Gravity {
    pub const BIT_FORGET: Self = Self(0);
    pub const WIN_UNMAP: Self = Self(0);
    pub const NORTH_WEST: Self = Self(1);
    pub const NORTH: Self = Self(2);
    pub const NORTH_EAST: Self = Self(3);
    pub const WEST: Self = Self(4);
    pub const CENTER: Self = Self(5);
    pub const EAST: Self = Self(6);
    pub const SOUTH_WEST: Self = Self(7);
    pub const SOUTH: Self = Self(8);
    pub const SOUTH_EAST: Self = Self(9);
    pub const STATIC: Self = Self(10);
}
impl From<Gravity> for u32 {
    #[inline]
    fn from(input: Gravity) -> Self {
        input.0
    }
}
impl From<Gravity> for Option<u32> {
    #[inline]
    fn from(input: Gravity) -> Self {
        Some(input.0)
    }
}
impl From<u8> for Gravity {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for Gravity {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for Gravity {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Gravity  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::BIT_FORGET.0, "BIT_FORGET", "BitForget"),
            (Self::WIN_UNMAP.0, "WIN_UNMAP", "WinUnmap"),
            (Self::NORTH_WEST.0, "NORTH_WEST", "NorthWest"),
            (Self::NORTH.0, "NORTH", "North"),
            (Self::NORTH_EAST.0, "NORTH_EAST", "NorthEast"),
            (Self::WEST.0, "WEST", "West"),
            (Self::CENTER.0, "CENTER", "Center"),
            (Self::EAST.0, "EAST", "East"),
            (Self::SOUTH_WEST.0, "SOUTH_WEST", "SouthWest"),
            (Self::SOUTH.0, "SOUTH", "South"),
            (Self::SOUTH_EAST.0, "SOUTH_EAST", "SouthEast"),
            (Self::STATIC.0, "STATIC", "Static"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

/// Auxiliary and optional information for the `create_window` function
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateWindowAux {
    pub background_pixmap: Option<Pixmap>,
    pub background_pixel: Option<u32>,
    pub border_pixmap: Option<Pixmap>,
    pub border_pixel: Option<u32>,
    pub bit_gravity: Option<Gravity>,
    pub win_gravity: Option<Gravity>,
    pub backing_store: Option<BackingStore>,
    pub backing_planes: Option<u32>,
    pub backing_pixel: Option<u32>,
    pub override_redirect: Option<Bool32>,
    pub save_under: Option<Bool32>,
    pub event_mask: Option<EventMask>,
    pub do_not_propogate_mask: Option<EventMask>,
    pub colormap: Option<Colormap>,
    pub cursor: Option<Cursor>,
}
impl_debug_if_no_extra_traits!(CreateWindowAux, "CreateWindowAux");
impl CreateWindowAux {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], value_mask: u32) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u32::from(value_mask);
        let mut outer_remaining = value;
        let background_pixmap = if switch_expr & u32::from(CW::BACK_PIXMAP) != 0 {
            let remaining = outer_remaining;
            let (background_pixmap, remaining) = Pixmap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(background_pixmap)
        } else {
            None
        };
        let background_pixel = if switch_expr & u32::from(CW::BACK_PIXEL) != 0 {
            let remaining = outer_remaining;
            let (background_pixel, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(background_pixel)
        } else {
            None
        };
        let border_pixmap = if switch_expr & u32::from(CW::BORDER_PIXMAP) != 0 {
            let remaining = outer_remaining;
            let (border_pixmap, remaining) = Pixmap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(border_pixmap)
        } else {
            None
        };
        let border_pixel = if switch_expr & u32::from(CW::BORDER_PIXEL) != 0 {
            let remaining = outer_remaining;
            let (border_pixel, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(border_pixel)
        } else {
            None
        };
        let bit_gravity = if switch_expr & u32::from(CW::BIT_GRAVITY) != 0 {
            let remaining = outer_remaining;
            let (bit_gravity, remaining) = u32::try_parse(remaining)?;
            let bit_gravity = bit_gravity.into();
            outer_remaining = remaining;
            Some(bit_gravity)
        } else {
            None
        };
        let win_gravity = if switch_expr & u32::from(CW::WIN_GRAVITY) != 0 {
            let remaining = outer_remaining;
            let (win_gravity, remaining) = u32::try_parse(remaining)?;
            let win_gravity = win_gravity.into();
            outer_remaining = remaining;
            Some(win_gravity)
        } else {
            None
        };
        let backing_store = if switch_expr & u32::from(CW::BACKING_STORE) != 0 {
            let remaining = outer_remaining;
            let (backing_store, remaining) = u32::try_parse(remaining)?;
            let backing_store = backing_store.into();
            outer_remaining = remaining;
            Some(backing_store)
        } else {
            None
        };
        let backing_planes = if switch_expr & u32::from(CW::BACKING_PLANES) != 0 {
            let remaining = outer_remaining;
            let (backing_planes, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(backing_planes)
        } else {
            None
        };
        let backing_pixel = if switch_expr & u32::from(CW::BACKING_PIXEL) != 0 {
            let remaining = outer_remaining;
            let (backing_pixel, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(backing_pixel)
        } else {
            None
        };
        let override_redirect = if switch_expr & u32::from(CW::OVERRIDE_REDIRECT) != 0 {
            let remaining = outer_remaining;
            let (override_redirect, remaining) = Bool32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(override_redirect)
        } else {
            None
        };
        let save_under = if switch_expr & u32::from(CW::SAVE_UNDER) != 0 {
            let remaining = outer_remaining;
            let (save_under, remaining) = Bool32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(save_under)
        } else {
            None
        };
        let event_mask = if switch_expr & u32::from(CW::EVENT_MASK) != 0 {
            let remaining = outer_remaining;
            let (event_mask, remaining) = u32::try_parse(remaining)?;
            let event_mask = event_mask.into();
            outer_remaining = remaining;
            Some(event_mask)
        } else {
            None
        };
        let do_not_propogate_mask = if switch_expr & u32::from(CW::DONT_PROPAGATE) != 0 {
            let remaining = outer_remaining;
            let (do_not_propogate_mask, remaining) = u32::try_parse(remaining)?;
            let do_not_propogate_mask = do_not_propogate_mask.into();
            outer_remaining = remaining;
            Some(do_not_propogate_mask)
        } else {
            None
        };
        let colormap = if switch_expr & u32::from(CW::COLORMAP) != 0 {
            let remaining = outer_remaining;
            let (colormap, remaining) = Colormap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(colormap)
        } else {
            None
        };
        let cursor = if switch_expr & u32::from(CW::CURSOR) != 0 {
            let remaining = outer_remaining;
            let (cursor, remaining) = Cursor::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(cursor)
        } else {
            None
        };
        let result = CreateWindowAux { background_pixmap, background_pixel, border_pixmap, border_pixel, bit_gravity, win_gravity, backing_store, backing_planes, backing_pixel, override_redirect, save_under, event_mask, do_not_propogate_mask, colormap, cursor };
        Ok((result, outer_remaining))
    }
}
impl CreateWindowAux {
    #[allow(dead_code)]
    fn serialize(&self, value_mask: u32) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u32::from(value_mask));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, value_mask: u32) {
        assert_eq!(self.switch_expr(), u32::from(value_mask), "switch `value_list` has an inconsistent discriminant");
        if let Some(background_pixmap) = self.background_pixmap {
            background_pixmap.serialize_into(bytes);
        }
        if let Some(background_pixel) = self.background_pixel {
            background_pixel.serialize_into(bytes);
        }
        if let Some(border_pixmap) = self.border_pixmap {
            border_pixmap.serialize_into(bytes);
        }
        if let Some(border_pixel) = self.border_pixel {
            border_pixel.serialize_into(bytes);
        }
        if let Some(bit_gravity) = self.bit_gravity {
            u32::from(bit_gravity).serialize_into(bytes);
        }
        if let Some(win_gravity) = self.win_gravity {
            u32::from(win_gravity).serialize_into(bytes);
        }
        if let Some(backing_store) = self.backing_store {
            u32::from(backing_store).serialize_into(bytes);
        }
        if let Some(backing_planes) = self.backing_planes {
            backing_planes.serialize_into(bytes);
        }
        if let Some(backing_pixel) = self.backing_pixel {
            backing_pixel.serialize_into(bytes);
        }
        if let Some(override_redirect) = self.override_redirect {
            override_redirect.serialize_into(bytes);
        }
        if let Some(save_under) = self.save_under {
            save_under.serialize_into(bytes);
        }
        if let Some(event_mask) = self.event_mask {
            u32::from(event_mask).serialize_into(bytes);
        }
        if let Some(do_not_propogate_mask) = self.do_not_propogate_mask {
            u32::from(do_not_propogate_mask).serialize_into(bytes);
        }
        if let Some(colormap) = self.colormap {
            colormap.serialize_into(bytes);
        }
        if let Some(cursor) = self.cursor {
            cursor.serialize_into(bytes);
        }
    }
}
impl CreateWindowAux {
    fn switch_expr(&self) -> u32 {
        let mut expr_value = 0;
        if self.background_pixmap.is_some() {
            expr_value |= u32::from(CW::BACK_PIXMAP);
        }
        if self.background_pixel.is_some() {
            expr_value |= u32::from(CW::BACK_PIXEL);
        }
        if self.border_pixmap.is_some() {
            expr_value |= u32::from(CW::BORDER_PIXMAP);
        }
        if self.border_pixel.is_some() {
            expr_value |= u32::from(CW::BORDER_PIXEL);
        }
        if self.bit_gravity.is_some() {
            expr_value |= u32::from(CW::BIT_GRAVITY);
        }
        if self.win_gravity.is_some() {
            expr_value |= u32::from(CW::WIN_GRAVITY);
        }
        if self.backing_store.is_some() {
            expr_value |= u32::from(CW::BACKING_STORE);
        }
        if self.backing_planes.is_some() {
            expr_value |= u32::from(CW::BACKING_PLANES);
        }
        if self.backing_pixel.is_some() {
            expr_value |= u32::from(CW::BACKING_PIXEL);
        }
        if self.override_redirect.is_some() {
            expr_value |= u32::from(CW::OVERRIDE_REDIRECT);
        }
        if self.save_under.is_some() {
            expr_value |= u32::from(CW::SAVE_UNDER);
        }
        if self.event_mask.is_some() {
            expr_value |= u32::from(CW::EVENT_MASK);
        }
        if self.do_not_propogate_mask.is_some() {
            expr_value |= u32::from(CW::DONT_PROPAGATE);
        }
        if self.colormap.is_some() {
            expr_value |= u32::from(CW::COLORMAP);
        }
        if self.cursor.is_some() {
            expr_value |= u32::from(CW::CURSOR);
        }
        expr_value
    }
}
impl CreateWindowAux {
    /// Create a new instance with all fields unset / not present.
    pub fn new() -> Self {
        Default::default()
    }
    /// Set the `background_pixmap` field of this structure.
    #[must_use]
    pub fn background_pixmap<I>(mut self, value: I) -> Self where I: Into<Option<Pixmap>> {
        self.background_pixmap = value.into();
        self
    }
    /// Set the `background_pixel` field of this structure.
    #[must_use]
    pub fn background_pixel<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.background_pixel = value.into();
        self
    }
    /// Set the `border_pixmap` field of this structure.
    #[must_use]
    pub fn border_pixmap<I>(mut self, value: I) -> Self where I: Into<Option<Pixmap>> {
        self.border_pixmap = value.into();
        self
    }
    /// Set the `border_pixel` field of this structure.
    #[must_use]
    pub fn border_pixel<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.border_pixel = value.into();
        self
    }
    /// Set the `bit_gravity` field of this structure.
    #[must_use]
    pub fn bit_gravity<I>(mut self, value: I) -> Self where I: Into<Option<Gravity>> {
        self.bit_gravity = value.into();
        self
    }
    /// Set the `win_gravity` field of this structure.
    #[must_use]
    pub fn win_gravity<I>(mut self, value: I) -> Self where I: Into<Option<Gravity>> {
        self.win_gravity = value.into();
        self
    }
    /// Set the `backing_store` field of this structure.
    #[must_use]
    pub fn backing_store<I>(mut self, value: I) -> Self where I: Into<Option<BackingStore>> {
        self.backing_store = value.into();
        self
    }
    /// Set the `backing_planes` field of this structure.
    #[must_use]
    pub fn backing_planes<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.backing_planes = value.into();
        self
    }
    /// Set the `backing_pixel` field of this structure.
    #[must_use]
    pub fn backing_pixel<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.backing_pixel = value.into();
        self
    }
    /// Set the `override_redirect` field of this structure.
    #[must_use]
    pub fn override_redirect<I>(mut self, value: I) -> Self where I: Into<Option<Bool32>> {
        self.override_redirect = value.into();
        self
    }
    /// Set the `save_under` field of this structure.
    #[must_use]
    pub fn save_under<I>(mut self, value: I) -> Self where I: Into<Option<Bool32>> {
        self.save_under = value.into();
        self
    }
    /// Set the `event_mask` field of this structure.
    #[must_use]
    pub fn event_mask<I>(mut self, value: I) -> Self where I: Into<Option<EventMask>> {
        self.event_mask = value.into();
        self
    }
    /// Set the `do_not_propogate_mask` field of this structure.
    #[must_use]
    pub fn do_not_propogate_mask<I>(mut self, value: I) -> Self where I: Into<Option<EventMask>> {
        self.do_not_propogate_mask = value.into();
        self
    }
    /// Set the `colormap` field of this structure.
    #[must_use]
    pub fn colormap<I>(mut self, value: I) -> Self where I: Into<Option<Colormap>> {
        self.colormap = value.into();
        self
    }
    /// Set the `cursor` field of this structure.
    #[must_use]
    pub fn cursor<I>(mut self, value: I) -> Self where I: Into<Option<Cursor>> {
        self.cursor = value.into();
        self
    }
}

/// Opcode for the CreateWindow request
pub const CREATE_WINDOW_REQUEST: u8 = 1;
/// Creates a window.
///
/// Creates an unmapped window as child of the specified `parent` window. A
/// CreateNotify event will be generated. The new window is placed on top in the
/// stacking order with respect to siblings.
///
/// The coordinate system has the X axis horizontal and the Y axis vertical with
/// the origin [0, 0] at the upper-left corner. Coordinates are integral, in terms
/// of pixels, and coincide with pixel centers. Each window and pixmap has its own
/// coordinate system. For a window, the origin is inside the border at the inside,
/// upper-left corner.
///
/// The created window is not yet displayed (mapped), call `xcb_map_window` to
/// display it.
///
/// The created window will initially use the same cursor as its parent.
///
/// # Fields
///
/// * `wid` - The ID with which you will refer to the new window, created by
/// `xcb_generate_id`.
/// * `depth` - Specifies the new window's depth (TODO: what unit?).
///
/// The special value `XCB_COPY_FROM_PARENT` means the depth is taken from the
/// `parent` window.
/// * `visual` - Specifies the id for the new window's visual.
///
/// The special value `XCB_COPY_FROM_PARENT` means the visual is taken from the
/// `parent` window.
/// * `class` -
/// * `parent` - The parent window of the new window.
/// * `border_width` - TODO:
///
/// Must be zero if the `class` is `InputOnly` or a `xcb_match_error_t` occurs.
/// * `x` - The X coordinate of the new window.
/// * `y` - The Y coordinate of the new window.
/// * `width` - The width of the new window.
/// * `height` - The height of the new window.
///
/// # Errors
///
/// * `Colormap` - TODO: reasons?
/// * `Match` - TODO: reasons?
/// * `Cursor` - TODO: reasons?
/// * `Pixmap` - TODO: reasons?
/// * `Value` - TODO: reasons?
/// * `Window` - TODO: reasons?
/// * `Alloc` - The X server could not allocate the requested resources (no memory?).
///
/// # See
///
/// * `xcb_generate_id`: function
/// * `MapWindow`: request
/// * `CreateNotify`: event
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateWindowRequest<'input> {
    pub depth: u8,
    pub wid: Window,
    pub parent: Window,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub border_width: u16,
    pub class: WindowClass,
    pub visual: Visualid,
    pub value_list: Cow<'input, CreateWindowAux>,
}
impl_debug_if_no_extra_traits!(CreateWindowRequest<'_>, "CreateWindowRequest");
impl<'input> CreateWindowRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let depth_bytes = self.depth.serialize();
        let wid_bytes = self.wid.serialize();
        let parent_bytes = self.parent.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let border_width_bytes = self.border_width.serialize();
        let class_bytes = u16::from(self.class).serialize();
        let visual_bytes = self.visual.serialize();
        let value_mask: u32 = self.value_list.switch_expr();
        let value_mask_bytes = value_mask.serialize();
        let mut request0 = vec![
            CREATE_WINDOW_REQUEST,
            depth_bytes[0],
            0,
            0,
            wid_bytes[0],
            wid_bytes[1],
            wid_bytes[2],
            wid_bytes[3],
            parent_bytes[0],
            parent_bytes[1],
            parent_bytes[2],
            parent_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            border_width_bytes[0],
            border_width_bytes[1],
            class_bytes[0],
            class_bytes[1],
            visual_bytes[0],
            visual_bytes[1],
            visual_bytes[2],
            visual_bytes[3],
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
        if header.major_opcode != CREATE_WINDOW_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (depth, remaining) = u8::try_parse(remaining)?;
        let _ = remaining;
        let (wid, remaining) = Window::try_parse(value)?;
        let (parent, remaining) = Window::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (border_width, remaining) = u16::try_parse(remaining)?;
        let (class, remaining) = u16::try_parse(remaining)?;
        let class = class.into();
        let (visual, remaining) = Visualid::try_parse(remaining)?;
        let (value_mask, remaining) = u32::try_parse(remaining)?;
        let (value_list, remaining) = CreateWindowAux::try_parse(remaining, u32::from(value_mask))?;
        let _ = remaining;
        Ok(CreateWindowRequest {
            depth,
            wid,
            parent,
            x,
            y,
            width,
            height,
            border_width,
            class,
            visual,
            value_list: Cow::Owned(value_list),
        })
    }
    /// Clone all borrowed data in this CreateWindowRequest.
    pub fn into_owned(self) -> CreateWindowRequest<'static> {
        CreateWindowRequest {
            depth: self.depth,
            wid: self.wid,
            parent: self.parent,
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            border_width: self.border_width,
            class: self.class,
            visual: self.visual,
            value_list: Cow::Owned(self.value_list.into_owned()),
        }
    }
}
impl<'input> Request for CreateWindowRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for CreateWindowRequest<'input> {
}

/// Auxiliary and optional information for the `change_window_attributes` function
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeWindowAttributesAux {
    pub background_pixmap: Option<Pixmap>,
    pub background_pixel: Option<u32>,
    pub border_pixmap: Option<Pixmap>,
    pub border_pixel: Option<u32>,
    pub bit_gravity: Option<Gravity>,
    pub win_gravity: Option<Gravity>,
    pub backing_store: Option<BackingStore>,
    pub backing_planes: Option<u32>,
    pub backing_pixel: Option<u32>,
    pub override_redirect: Option<Bool32>,
    pub save_under: Option<Bool32>,
    pub event_mask: Option<EventMask>,
    pub do_not_propogate_mask: Option<EventMask>,
    pub colormap: Option<Colormap>,
    pub cursor: Option<Cursor>,
}
impl_debug_if_no_extra_traits!(ChangeWindowAttributesAux, "ChangeWindowAttributesAux");
impl ChangeWindowAttributesAux {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], value_mask: u32) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u32::from(value_mask);
        let mut outer_remaining = value;
        let background_pixmap = if switch_expr & u32::from(CW::BACK_PIXMAP) != 0 {
            let remaining = outer_remaining;
            let (background_pixmap, remaining) = Pixmap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(background_pixmap)
        } else {
            None
        };
        let background_pixel = if switch_expr & u32::from(CW::BACK_PIXEL) != 0 {
            let remaining = outer_remaining;
            let (background_pixel, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(background_pixel)
        } else {
            None
        };
        let border_pixmap = if switch_expr & u32::from(CW::BORDER_PIXMAP) != 0 {
            let remaining = outer_remaining;
            let (border_pixmap, remaining) = Pixmap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(border_pixmap)
        } else {
            None
        };
        let border_pixel = if switch_expr & u32::from(CW::BORDER_PIXEL) != 0 {
            let remaining = outer_remaining;
            let (border_pixel, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(border_pixel)
        } else {
            None
        };
        let bit_gravity = if switch_expr & u32::from(CW::BIT_GRAVITY) != 0 {
            let remaining = outer_remaining;
            let (bit_gravity, remaining) = u32::try_parse(remaining)?;
            let bit_gravity = bit_gravity.into();
            outer_remaining = remaining;
            Some(bit_gravity)
        } else {
            None
        };
        let win_gravity = if switch_expr & u32::from(CW::WIN_GRAVITY) != 0 {
            let remaining = outer_remaining;
            let (win_gravity, remaining) = u32::try_parse(remaining)?;
            let win_gravity = win_gravity.into();
            outer_remaining = remaining;
            Some(win_gravity)
        } else {
            None
        };
        let backing_store = if switch_expr & u32::from(CW::BACKING_STORE) != 0 {
            let remaining = outer_remaining;
            let (backing_store, remaining) = u32::try_parse(remaining)?;
            let backing_store = backing_store.into();
            outer_remaining = remaining;
            Some(backing_store)
        } else {
            None
        };
        let backing_planes = if switch_expr & u32::from(CW::BACKING_PLANES) != 0 {
            let remaining = outer_remaining;
            let (backing_planes, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(backing_planes)
        } else {
            None
        };
        let backing_pixel = if switch_expr & u32::from(CW::BACKING_PIXEL) != 0 {
            let remaining = outer_remaining;
            let (backing_pixel, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(backing_pixel)
        } else {
            None
        };
        let override_redirect = if switch_expr & u32::from(CW::OVERRIDE_REDIRECT) != 0 {
            let remaining = outer_remaining;
            let (override_redirect, remaining) = Bool32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(override_redirect)
        } else {
            None
        };
        let save_under = if switch_expr & u32::from(CW::SAVE_UNDER) != 0 {
            let remaining = outer_remaining;
            let (save_under, remaining) = Bool32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(save_under)
        } else {
            None
        };
        let event_mask = if switch_expr & u32::from(CW::EVENT_MASK) != 0 {
            let remaining = outer_remaining;
            let (event_mask, remaining) = u32::try_parse(remaining)?;
            let event_mask = event_mask.into();
            outer_remaining = remaining;
            Some(event_mask)
        } else {
            None
        };
        let do_not_propogate_mask = if switch_expr & u32::from(CW::DONT_PROPAGATE) != 0 {
            let remaining = outer_remaining;
            let (do_not_propogate_mask, remaining) = u32::try_parse(remaining)?;
            let do_not_propogate_mask = do_not_propogate_mask.into();
            outer_remaining = remaining;
            Some(do_not_propogate_mask)
        } else {
            None
        };
        let colormap = if switch_expr & u32::from(CW::COLORMAP) != 0 {
            let remaining = outer_remaining;
            let (colormap, remaining) = Colormap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(colormap)
        } else {
            None
        };
        let cursor = if switch_expr & u32::from(CW::CURSOR) != 0 {
            let remaining = outer_remaining;
            let (cursor, remaining) = Cursor::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(cursor)
        } else {
            None
        };
        let result = ChangeWindowAttributesAux { background_pixmap, background_pixel, border_pixmap, border_pixel, bit_gravity, win_gravity, backing_store, backing_planes, backing_pixel, override_redirect, save_under, event_mask, do_not_propogate_mask, colormap, cursor };
        Ok((result, outer_remaining))
    }
}
impl ChangeWindowAttributesAux {
    #[allow(dead_code)]
    fn serialize(&self, value_mask: u32) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u32::from(value_mask));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, value_mask: u32) {
        assert_eq!(self.switch_expr(), u32::from(value_mask), "switch `value_list` has an inconsistent discriminant");
        if let Some(background_pixmap) = self.background_pixmap {
            background_pixmap.serialize_into(bytes);
        }
        if let Some(background_pixel) = self.background_pixel {
            background_pixel.serialize_into(bytes);
        }
        if let Some(border_pixmap) = self.border_pixmap {
            border_pixmap.serialize_into(bytes);
        }
        if let Some(border_pixel) = self.border_pixel {
            border_pixel.serialize_into(bytes);
        }
        if let Some(bit_gravity) = self.bit_gravity {
            u32::from(bit_gravity).serialize_into(bytes);
        }
        if let Some(win_gravity) = self.win_gravity {
            u32::from(win_gravity).serialize_into(bytes);
        }
        if let Some(backing_store) = self.backing_store {
            u32::from(backing_store).serialize_into(bytes);
        }
        if let Some(backing_planes) = self.backing_planes {
            backing_planes.serialize_into(bytes);
        }
        if let Some(backing_pixel) = self.backing_pixel {
            backing_pixel.serialize_into(bytes);
        }
        if let Some(override_redirect) = self.override_redirect {
            override_redirect.serialize_into(bytes);
        }
        if let Some(save_under) = self.save_under {
            save_under.serialize_into(bytes);
        }
        if let Some(event_mask) = self.event_mask {
            u32::from(event_mask).serialize_into(bytes);
        }
        if let Some(do_not_propogate_mask) = self.do_not_propogate_mask {
            u32::from(do_not_propogate_mask).serialize_into(bytes);
        }
        if let Some(colormap) = self.colormap {
            colormap.serialize_into(bytes);
        }
        if let Some(cursor) = self.cursor {
            cursor.serialize_into(bytes);
        }
    }
}
impl ChangeWindowAttributesAux {
    fn switch_expr(&self) -> u32 {
        let mut expr_value = 0;
        if self.background_pixmap.is_some() {
            expr_value |= u32::from(CW::BACK_PIXMAP);
        }
        if self.background_pixel.is_some() {
            expr_value |= u32::from(CW::BACK_PIXEL);
        }
        if self.border_pixmap.is_some() {
            expr_value |= u32::from(CW::BORDER_PIXMAP);
        }
        if self.border_pixel.is_some() {
            expr_value |= u32::from(CW::BORDER_PIXEL);
        }
        if self.bit_gravity.is_some() {
            expr_value |= u32::from(CW::BIT_GRAVITY);
        }
        if self.win_gravity.is_some() {
            expr_value |= u32::from(CW::WIN_GRAVITY);
        }
        if self.backing_store.is_some() {
            expr_value |= u32::from(CW::BACKING_STORE);
        }
        if self.backing_planes.is_some() {
            expr_value |= u32::from(CW::BACKING_PLANES);
        }
        if self.backing_pixel.is_some() {
            expr_value |= u32::from(CW::BACKING_PIXEL);
        }
        if self.override_redirect.is_some() {
            expr_value |= u32::from(CW::OVERRIDE_REDIRECT);
        }
        if self.save_under.is_some() {
            expr_value |= u32::from(CW::SAVE_UNDER);
        }
        if self.event_mask.is_some() {
            expr_value |= u32::from(CW::EVENT_MASK);
        }
        if self.do_not_propogate_mask.is_some() {
            expr_value |= u32::from(CW::DONT_PROPAGATE);
        }
        if self.colormap.is_some() {
            expr_value |= u32::from(CW::COLORMAP);
        }
        if self.cursor.is_some() {
            expr_value |= u32::from(CW::CURSOR);
        }
        expr_value
    }
}
impl ChangeWindowAttributesAux {
    /// Create a new instance with all fields unset / not present.
    pub fn new() -> Self {
        Default::default()
    }
    /// Set the `background_pixmap` field of this structure.
    #[must_use]
    pub fn background_pixmap<I>(mut self, value: I) -> Self where I: Into<Option<Pixmap>> {
        self.background_pixmap = value.into();
        self
    }
    /// Set the `background_pixel` field of this structure.
    #[must_use]
    pub fn background_pixel<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.background_pixel = value.into();
        self
    }
    /// Set the `border_pixmap` field of this structure.
    #[must_use]
    pub fn border_pixmap<I>(mut self, value: I) -> Self where I: Into<Option<Pixmap>> {
        self.border_pixmap = value.into();
        self
    }
    /// Set the `border_pixel` field of this structure.
    #[must_use]
    pub fn border_pixel<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.border_pixel = value.into();
        self
    }
    /// Set the `bit_gravity` field of this structure.
    #[must_use]
    pub fn bit_gravity<I>(mut self, value: I) -> Self where I: Into<Option<Gravity>> {
        self.bit_gravity = value.into();
        self
    }
    /// Set the `win_gravity` field of this structure.
    #[must_use]
    pub fn win_gravity<I>(mut self, value: I) -> Self where I: Into<Option<Gravity>> {
        self.win_gravity = value.into();
        self
    }
    /// Set the `backing_store` field of this structure.
    #[must_use]
    pub fn backing_store<I>(mut self, value: I) -> Self where I: Into<Option<BackingStore>> {
        self.backing_store = value.into();
        self
    }
    /// Set the `backing_planes` field of this structure.
    #[must_use]
    pub fn backing_planes<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.backing_planes = value.into();
        self
    }
    /// Set the `backing_pixel` field of this structure.
    #[must_use]
    pub fn backing_pixel<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.backing_pixel = value.into();
        self
    }
    /// Set the `override_redirect` field of this structure.
    #[must_use]
    pub fn override_redirect<I>(mut self, value: I) -> Self where I: Into<Option<Bool32>> {
        self.override_redirect = value.into();
        self
    }
    /// Set the `save_under` field of this structure.
    #[must_use]
    pub fn save_under<I>(mut self, value: I) -> Self where I: Into<Option<Bool32>> {
        self.save_under = value.into();
        self
    }
    /// Set the `event_mask` field of this structure.
    #[must_use]
    pub fn event_mask<I>(mut self, value: I) -> Self where I: Into<Option<EventMask>> {
        self.event_mask = value.into();
        self
    }
    /// Set the `do_not_propogate_mask` field of this structure.
    #[must_use]
    pub fn do_not_propogate_mask<I>(mut self, value: I) -> Self where I: Into<Option<EventMask>> {
        self.do_not_propogate_mask = value.into();
        self
    }
    /// Set the `colormap` field of this structure.
    #[must_use]
    pub fn colormap<I>(mut self, value: I) -> Self where I: Into<Option<Colormap>> {
        self.colormap = value.into();
        self
    }
    /// Set the `cursor` field of this structure.
    #[must_use]
    pub fn cursor<I>(mut self, value: I) -> Self where I: Into<Option<Cursor>> {
        self.cursor = value.into();
        self
    }
}

/// Opcode for the ChangeWindowAttributes request
pub const CHANGE_WINDOW_ATTRIBUTES_REQUEST: u8 = 2;
/// change window attributes.
///
/// Changes the attributes specified by `value_mask` for the specified `window`.
///
/// # Fields
///
/// * `window` - The window to change.
/// * `value_list` - Values for each of the attributes specified in the bitmask `value_mask`. The
/// order has to correspond to the order of possible `value_mask` bits. See the
/// example.
///
/// # Errors
///
/// * `Access` - TODO: reasons?
/// * `Colormap` - TODO: reasons?
/// * `Cursor` - TODO: reasons?
/// * `Match` - TODO: reasons?
/// * `Pixmap` - TODO: reasons?
/// * `Value` - TODO: reasons?
/// * `Window` - The specified `window` does not exist.
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeWindowAttributesRequest<'input> {
    pub window: Window,
    pub value_list: Cow<'input, ChangeWindowAttributesAux>,
}
impl_debug_if_no_extra_traits!(ChangeWindowAttributesRequest<'_>, "ChangeWindowAttributesRequest");
impl<'input> ChangeWindowAttributesRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let value_mask: u32 = self.value_list.switch_expr();
        let value_mask_bytes = value_mask.serialize();
        let mut request0 = vec![
            CHANGE_WINDOW_ATTRIBUTES_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.major_opcode != CHANGE_WINDOW_ATTRIBUTES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let (value_mask, remaining) = u32::try_parse(remaining)?;
        let (value_list, remaining) = ChangeWindowAttributesAux::try_parse(remaining, u32::from(value_mask))?;
        let _ = remaining;
        Ok(ChangeWindowAttributesRequest {
            window,
            value_list: Cow::Owned(value_list),
        })
    }
    /// Clone all borrowed data in this ChangeWindowAttributesRequest.
    pub fn into_owned(self) -> ChangeWindowAttributesRequest<'static> {
        ChangeWindowAttributesRequest {
            window: self.window,
            value_list: Cow::Owned(self.value_list.into_owned()),
        }
    }
}
impl<'input> Request for ChangeWindowAttributesRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for ChangeWindowAttributesRequest<'input> {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapState(u8);
impl MapState {
    pub const UNMAPPED: Self = Self(0);
    pub const UNVIEWABLE: Self = Self(1);
    pub const VIEWABLE: Self = Self(2);
}
impl From<MapState> for u8 {
    #[inline]
    fn from(input: MapState) -> Self {
        input.0
    }
}
impl From<MapState> for Option<u8> {
    #[inline]
    fn from(input: MapState) -> Self {
        Some(input.0)
    }
}
impl From<MapState> for u16 {
    #[inline]
    fn from(input: MapState) -> Self {
        u16::from(input.0)
    }
}
impl From<MapState> for Option<u16> {
    #[inline]
    fn from(input: MapState) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<MapState> for u32 {
    #[inline]
    fn from(input: MapState) -> Self {
        u32::from(input.0)
    }
}
impl From<MapState> for Option<u32> {
    #[inline]
    fn from(input: MapState) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for MapState {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for MapState  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::UNMAPPED.0.into(), "UNMAPPED", "Unmapped"),
            (Self::UNVIEWABLE.0.into(), "UNVIEWABLE", "Unviewable"),
            (Self::VIEWABLE.0.into(), "VIEWABLE", "Viewable"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the GetWindowAttributes request
pub const GET_WINDOW_ATTRIBUTES_REQUEST: u8 = 3;
/// Gets window attributes.
///
/// Gets the current attributes for the specified `window`.
///
/// # Fields
///
/// * `window` - The window to get the attributes from.
///
/// # Errors
///
/// * `Window` - The specified `window` does not exist.
/// * `Drawable` - TODO: reasons?
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetWindowAttributesRequest {
    pub window: Window,
}
impl_debug_if_no_extra_traits!(GetWindowAttributesRequest, "GetWindowAttributesRequest");
impl GetWindowAttributesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            GET_WINDOW_ATTRIBUTES_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.major_opcode != GET_WINDOW_ATTRIBUTES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let _ = remaining;
        Ok(GetWindowAttributesRequest {
            window,
        })
    }
}
impl Request for GetWindowAttributesRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetWindowAttributesRequest {
    type Reply = GetWindowAttributesReply;
}

/// # Fields
///
/// * `override_redirect` - Window managers should ignore this window if `override_redirect` is 1.
/// * `visual` - The associated visual structure of `window`.
/// * `backing_planes` - Planes to be preserved if possible.
/// * `backing_pixel` - Value to be used when restoring planes.
/// * `save_under` - Boolean, should bits under be saved?
/// * `colormap` - Color map to be associated with window.
/// * `all_event_masks` - Set of events all people have interest in.
/// * `your_event_mask` - My event mask.
/// * `do_not_propagate_mask` - Set of events that should not propagate.
/// * `backing_store` -
/// * `class` -
/// * `bit_gravity` -
/// * `win_gravity` -
/// * `map_state` -
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetWindowAttributesReply {
    pub backing_store: BackingStore,
    pub sequence: u16,
    pub length: u32,
    pub visual: Visualid,
    pub class: WindowClass,
    pub bit_gravity: Gravity,
    pub win_gravity: Gravity,
    pub backing_planes: u32,
    pub backing_pixel: u32,
    pub save_under: bool,
    pub map_is_installed: bool,
    pub map_state: MapState,
    pub override_redirect: bool,
    pub colormap: Colormap,
    pub all_event_masks: EventMask,
    pub your_event_mask: EventMask,
    pub do_not_propagate_mask: EventMask,
}
impl_debug_if_no_extra_traits!(GetWindowAttributesReply, "GetWindowAttributesReply");
impl TryParse for GetWindowAttributesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (backing_store, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (visual, remaining) = Visualid::try_parse(remaining)?;
        let (class, remaining) = u16::try_parse(remaining)?;
        let (bit_gravity, remaining) = u8::try_parse(remaining)?;
        let (win_gravity, remaining) = u8::try_parse(remaining)?;
        let (backing_planes, remaining) = u32::try_parse(remaining)?;
        let (backing_pixel, remaining) = u32::try_parse(remaining)?;
        let (save_under, remaining) = bool::try_parse(remaining)?;
        let (map_is_installed, remaining) = bool::try_parse(remaining)?;
        let (map_state, remaining) = u8::try_parse(remaining)?;
        let (override_redirect, remaining) = bool::try_parse(remaining)?;
        let (colormap, remaining) = Colormap::try_parse(remaining)?;
        let (all_event_masks, remaining) = u32::try_parse(remaining)?;
        let (your_event_mask, remaining) = u32::try_parse(remaining)?;
        let (do_not_propagate_mask, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let backing_store = backing_store.into();
        let class = class.into();
        let bit_gravity = bit_gravity.into();
        let win_gravity = win_gravity.into();
        let map_state = map_state.into();
        let all_event_masks = all_event_masks.into();
        let your_event_mask = your_event_mask.into();
        let do_not_propagate_mask = do_not_propagate_mask.into();
        let result = GetWindowAttributesReply { backing_store, sequence, length, visual, class, bit_gravity, win_gravity, backing_planes, backing_pixel, save_under, map_is_installed, map_state, override_redirect, colormap, all_event_masks, your_event_mask, do_not_propagate_mask };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetWindowAttributesReply {
    type Bytes = [u8; 44];
    fn serialize(&self) -> [u8; 44] {
        let response_type_bytes = &[1];
        let backing_store_bytes = (u32::from(self.backing_store) as u8).serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let visual_bytes = self.visual.serialize();
        let class_bytes = u16::from(self.class).serialize();
        let bit_gravity_bytes = (u32::from(self.bit_gravity) as u8).serialize();
        let win_gravity_bytes = (u32::from(self.win_gravity) as u8).serialize();
        let backing_planes_bytes = self.backing_planes.serialize();
        let backing_pixel_bytes = self.backing_pixel.serialize();
        let save_under_bytes = self.save_under.serialize();
        let map_is_installed_bytes = self.map_is_installed.serialize();
        let map_state_bytes = u8::from(self.map_state).serialize();
        let override_redirect_bytes = self.override_redirect.serialize();
        let colormap_bytes = self.colormap.serialize();
        let all_event_masks_bytes = u32::from(self.all_event_masks).serialize();
        let your_event_mask_bytes = u32::from(self.your_event_mask).serialize();
        let do_not_propagate_mask_bytes = (u32::from(self.do_not_propagate_mask) as u16).serialize();
        [
            response_type_bytes[0],
            backing_store_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            visual_bytes[0],
            visual_bytes[1],
            visual_bytes[2],
            visual_bytes[3],
            class_bytes[0],
            class_bytes[1],
            bit_gravity_bytes[0],
            win_gravity_bytes[0],
            backing_planes_bytes[0],
            backing_planes_bytes[1],
            backing_planes_bytes[2],
            backing_planes_bytes[3],
            backing_pixel_bytes[0],
            backing_pixel_bytes[1],
            backing_pixel_bytes[2],
            backing_pixel_bytes[3],
            save_under_bytes[0],
            map_is_installed_bytes[0],
            map_state_bytes[0],
            override_redirect_bytes[0],
            colormap_bytes[0],
            colormap_bytes[1],
            colormap_bytes[2],
            colormap_bytes[3],
            all_event_masks_bytes[0],
            all_event_masks_bytes[1],
            all_event_masks_bytes[2],
            all_event_masks_bytes[3],
            your_event_mask_bytes[0],
            your_event_mask_bytes[1],
            your_event_mask_bytes[2],
            your_event_mask_bytes[3],
            do_not_propagate_mask_bytes[0],
            do_not_propagate_mask_bytes[1],
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(44);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        (u32::from(self.backing_store) as u8).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.visual.serialize_into(bytes);
        u16::from(self.class).serialize_into(bytes);
        (u32::from(self.bit_gravity) as u8).serialize_into(bytes);
        (u32::from(self.win_gravity) as u8).serialize_into(bytes);
        self.backing_planes.serialize_into(bytes);
        self.backing_pixel.serialize_into(bytes);
        self.save_under.serialize_into(bytes);
        self.map_is_installed.serialize_into(bytes);
        u8::from(self.map_state).serialize_into(bytes);
        self.override_redirect.serialize_into(bytes);
        self.colormap.serialize_into(bytes);
        u32::from(self.all_event_masks).serialize_into(bytes);
        u32::from(self.your_event_mask).serialize_into(bytes);
        (u32::from(self.do_not_propagate_mask) as u16).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
    }
}

/// Opcode for the DestroyWindow request
pub const DESTROY_WINDOW_REQUEST: u8 = 4;
/// Destroys a window.
///
/// Destroys the specified window and all of its subwindows. A DestroyNotify event
/// is generated for each destroyed window (a DestroyNotify event is first generated
/// for any given window's inferiors). If the window was mapped, it will be
/// automatically unmapped before destroying.
///
/// Calling DestroyWindow on the root window will do nothing.
///
/// # Fields
///
/// * `window` - The window to destroy.
///
/// # Errors
///
/// * `Window` - The specified window does not exist.
///
/// # See
///
/// * `DestroyNotify`: event
/// * `MapWindow`: request
/// * `UnmapWindow`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DestroyWindowRequest {
    pub window: Window,
}
impl_debug_if_no_extra_traits!(DestroyWindowRequest, "DestroyWindowRequest");
impl DestroyWindowRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            DESTROY_WINDOW_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.major_opcode != DESTROY_WINDOW_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let _ = remaining;
        Ok(DestroyWindowRequest {
            window,
        })
    }
}
impl Request for DestroyWindowRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DestroyWindowRequest {
}

/// Opcode for the DestroySubwindows request
pub const DESTROY_SUBWINDOWS_REQUEST: u8 = 5;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DestroySubwindowsRequest {
    pub window: Window,
}
impl_debug_if_no_extra_traits!(DestroySubwindowsRequest, "DestroySubwindowsRequest");
impl DestroySubwindowsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            DESTROY_SUBWINDOWS_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.major_opcode != DESTROY_SUBWINDOWS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let _ = remaining;
        Ok(DestroySubwindowsRequest {
            window,
        })
    }
}
impl Request for DestroySubwindowsRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DestroySubwindowsRequest {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetMode(u8);
impl SetMode {
    pub const INSERT: Self = Self(0);
    pub const DELETE: Self = Self(1);
}
impl From<SetMode> for u8 {
    #[inline]
    fn from(input: SetMode) -> Self {
        input.0
    }
}
impl From<SetMode> for Option<u8> {
    #[inline]
    fn from(input: SetMode) -> Self {
        Some(input.0)
    }
}
impl From<SetMode> for u16 {
    #[inline]
    fn from(input: SetMode) -> Self {
        u16::from(input.0)
    }
}
impl From<SetMode> for Option<u16> {
    #[inline]
    fn from(input: SetMode) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SetMode> for u32 {
    #[inline]
    fn from(input: SetMode) -> Self {
        u32::from(input.0)
    }
}
impl From<SetMode> for Option<u32> {
    #[inline]
    fn from(input: SetMode) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SetMode {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SetMode  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::INSERT.0.into(), "INSERT", "Insert"),
            (Self::DELETE.0.into(), "DELETE", "Delete"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the ChangeSaveSet request
pub const CHANGE_SAVE_SET_REQUEST: u8 = 6;
/// Changes a client's save set.
///
/// TODO: explain what the save set is for.
///
/// This function either adds or removes the specified window to the client's (your
/// application's) save set.
///
/// # Fields
///
/// * `mode` - Insert to add the specified window to the save set or Delete to delete it from the save set.
/// * `window` - The window to add or delete to/from your save set.
///
/// # Errors
///
/// * `Match` - You created the specified window. This does not make sense, you can only add
/// windows created by other clients to your save set.
/// * `Value` - You specified an invalid mode.
/// * `Window` - The specified window does not exist.
///
/// # See
///
/// * `ReparentWindow`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeSaveSetRequest {
    pub mode: SetMode,
    pub window: Window,
}
impl_debug_if_no_extra_traits!(ChangeSaveSetRequest, "ChangeSaveSetRequest");
impl ChangeSaveSetRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mode_bytes = u8::from(self.mode).serialize();
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            CHANGE_SAVE_SET_REQUEST,
            mode_bytes[0],
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.major_opcode != CHANGE_SAVE_SET_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (mode, remaining) = u8::try_parse(remaining)?;
        let mode = mode.into();
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let _ = remaining;
        Ok(ChangeSaveSetRequest {
            mode,
            window,
        })
    }
}
impl Request for ChangeSaveSetRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for ChangeSaveSetRequest {
}

/// Opcode for the ReparentWindow request
pub const REPARENT_WINDOW_REQUEST: u8 = 7;
/// Reparents a window.
///
/// Makes the specified window a child of the specified parent window. If the
/// window is mapped, it will automatically be unmapped before reparenting and
/// re-mapped after reparenting. The window is placed in the stacking order on top
/// with respect to sibling windows.
///
/// After reparenting, a ReparentNotify event is generated.
///
/// # Fields
///
/// * `window` - The window to reparent.
/// * `parent` - The new parent of the window.
/// * `x` - The X position of the window within its new parent.
/// * `y` - The Y position of the window within its new parent.
///
/// # Errors
///
/// * `Match` - The new parent window is not on the same screen as the old parent window.
/// 
/// The new parent window is the specified window or an inferior of the specified window.
/// 
/// The new parent is InputOnly and the window is not.
/// 
/// The specified window has a ParentRelative background and the new parent window is not the same depth as the specified window.
/// * `Window` - The specified window does not exist.
///
/// # See
///
/// * `ReparentNotify`: event
/// * `MapWindow`: request
/// * `UnmapWindow`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReparentWindowRequest {
    pub window: Window,
    pub parent: Window,
    pub x: i16,
    pub y: i16,
}
impl_debug_if_no_extra_traits!(ReparentWindowRequest, "ReparentWindowRequest");
impl ReparentWindowRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let parent_bytes = self.parent.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let mut request0 = vec![
            REPARENT_WINDOW_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            parent_bytes[0],
            parent_bytes[1],
            parent_bytes[2],
            parent_bytes[3],
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
        if header.major_opcode != REPARENT_WINDOW_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let (parent, remaining) = Window::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let _ = remaining;
        Ok(ReparentWindowRequest {
            window,
            parent,
            x,
            y,
        })
    }
}
impl Request for ReparentWindowRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for ReparentWindowRequest {
}

/// Opcode for the MapWindow request
pub const MAP_WINDOW_REQUEST: u8 = 8;
/// Makes a window visible.
///
/// Maps the specified window. This means making the window visible (as long as its
/// parent is visible).
///
/// This MapWindow request will be translated to a MapRequest request if a window
/// manager is running. The window manager then decides to either map the window or
/// not. Set the override-redirect window attribute to true if you want to bypass
/// this mechanism.
///
/// If the window manager decides to map the window (or if no window manager is
/// running), a MapNotify event is generated.
///
/// If the window becomes viewable and no earlier contents for it are remembered,
/// the X server tiles the window with its background. If the window's background
/// is undefined, the existing screen contents are not altered, and the X server
/// generates zero or more Expose events.
///
/// If the window type is InputOutput, an Expose event will be generated when the
/// window becomes visible. The normal response to an Expose event should be to
/// repaint the window.
///
/// # Fields
///
/// * `window` - The window to make visible.
///
/// # Errors
///
/// * `Match` - The specified window does not exist.
///
/// # See
///
/// * `MapNotify`: event
/// * `Expose`: event
/// * `UnmapWindow`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapWindowRequest {
    pub window: Window,
}
impl_debug_if_no_extra_traits!(MapWindowRequest, "MapWindowRequest");
impl MapWindowRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            MAP_WINDOW_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.major_opcode != MAP_WINDOW_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let _ = remaining;
        Ok(MapWindowRequest {
            window,
        })
    }
}
impl Request for MapWindowRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for MapWindowRequest {
}

/// Opcode for the MapSubwindows request
pub const MAP_SUBWINDOWS_REQUEST: u8 = 9;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapSubwindowsRequest {
    pub window: Window,
}
impl_debug_if_no_extra_traits!(MapSubwindowsRequest, "MapSubwindowsRequest");
impl MapSubwindowsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            MAP_SUBWINDOWS_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.major_opcode != MAP_SUBWINDOWS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let _ = remaining;
        Ok(MapSubwindowsRequest {
            window,
        })
    }
}
impl Request for MapSubwindowsRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for MapSubwindowsRequest {
}

/// Opcode for the UnmapWindow request
pub const UNMAP_WINDOW_REQUEST: u8 = 10;
/// Makes a window invisible.
///
/// Unmaps the specified window. This means making the window invisible (and all
/// its child windows).
///
/// Unmapping a window leads to the `UnmapNotify` event being generated. Also,
/// `Expose` events are generated for formerly obscured windows.
///
/// # Fields
///
/// * `window` - The window to make invisible.
///
/// # Errors
///
/// * `Window` - The specified window does not exist.
///
/// # See
///
/// * `UnmapNotify`: event
/// * `Expose`: event
/// * `MapWindow`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnmapWindowRequest {
    pub window: Window,
}
impl_debug_if_no_extra_traits!(UnmapWindowRequest, "UnmapWindowRequest");
impl UnmapWindowRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            UNMAP_WINDOW_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.major_opcode != UNMAP_WINDOW_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let _ = remaining;
        Ok(UnmapWindowRequest {
            window,
        })
    }
}
impl Request for UnmapWindowRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for UnmapWindowRequest {
}

/// Opcode for the UnmapSubwindows request
pub const UNMAP_SUBWINDOWS_REQUEST: u8 = 11;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnmapSubwindowsRequest {
    pub window: Window,
}
impl_debug_if_no_extra_traits!(UnmapSubwindowsRequest, "UnmapSubwindowsRequest");
impl UnmapSubwindowsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            UNMAP_SUBWINDOWS_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.major_opcode != UNMAP_SUBWINDOWS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let _ = remaining;
        Ok(UnmapSubwindowsRequest {
            window,
        })
    }
}
impl Request for UnmapSubwindowsRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for UnmapSubwindowsRequest {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConfigWindow(u16);
impl ConfigWindow {
    pub const X: Self = Self(1 << 0);
    pub const Y: Self = Self(1 << 1);
    pub const WIDTH: Self = Self(1 << 2);
    pub const HEIGHT: Self = Self(1 << 3);
    pub const BORDER_WIDTH: Self = Self(1 << 4);
    pub const SIBLING: Self = Self(1 << 5);
    pub const STACK_MODE: Self = Self(1 << 6);
}
impl From<ConfigWindow> for u16 {
    #[inline]
    fn from(input: ConfigWindow) -> Self {
        input.0
    }
}
impl From<ConfigWindow> for Option<u16> {
    #[inline]
    fn from(input: ConfigWindow) -> Self {
        Some(input.0)
    }
}
impl From<ConfigWindow> for u32 {
    #[inline]
    fn from(input: ConfigWindow) -> Self {
        u32::from(input.0)
    }
}
impl From<ConfigWindow> for Option<u32> {
    #[inline]
    fn from(input: ConfigWindow) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ConfigWindow {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for ConfigWindow {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ConfigWindow  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::X.0.into(), "X", "X"),
            (Self::Y.0.into(), "Y", "Y"),
            (Self::WIDTH.0.into(), "WIDTH", "Width"),
            (Self::HEIGHT.0.into(), "HEIGHT", "Height"),
            (Self::BORDER_WIDTH.0.into(), "BORDER_WIDTH", "BorderWidth"),
            (Self::SIBLING.0.into(), "SIBLING", "Sibling"),
            (Self::STACK_MODE.0.into(), "STACK_MODE", "StackMode"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(ConfigWindow, u16);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StackMode(u32);
impl StackMode {
    pub const ABOVE: Self = Self(0);
    pub const BELOW: Self = Self(1);
    pub const TOP_IF: Self = Self(2);
    pub const BOTTOM_IF: Self = Self(3);
    pub const OPPOSITE: Self = Self(4);
}
impl From<StackMode> for u32 {
    #[inline]
    fn from(input: StackMode) -> Self {
        input.0
    }
}
impl From<StackMode> for Option<u32> {
    #[inline]
    fn from(input: StackMode) -> Self {
        Some(input.0)
    }
}
impl From<u8> for StackMode {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for StackMode {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for StackMode {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for StackMode  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ABOVE.0, "ABOVE", "Above"),
            (Self::BELOW.0, "BELOW", "Below"),
            (Self::TOP_IF.0, "TOP_IF", "TopIf"),
            (Self::BOTTOM_IF.0, "BOTTOM_IF", "BottomIf"),
            (Self::OPPOSITE.0, "OPPOSITE", "Opposite"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

/// Auxiliary and optional information for the `configure_window` function
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConfigureWindowAux {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub border_width: Option<u32>,
    pub sibling: Option<Window>,
    pub stack_mode: Option<StackMode>,
}
impl_debug_if_no_extra_traits!(ConfigureWindowAux, "ConfigureWindowAux");
impl ConfigureWindowAux {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], value_mask: u16) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u16::from(value_mask);
        let mut outer_remaining = value;
        let x = if switch_expr & u16::from(ConfigWindow::X) != 0 {
            let remaining = outer_remaining;
            let (x, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(x)
        } else {
            None
        };
        let y = if switch_expr & u16::from(ConfigWindow::Y) != 0 {
            let remaining = outer_remaining;
            let (y, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(y)
        } else {
            None
        };
        let width = if switch_expr & u16::from(ConfigWindow::WIDTH) != 0 {
            let remaining = outer_remaining;
            let (width, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(width)
        } else {
            None
        };
        let height = if switch_expr & u16::from(ConfigWindow::HEIGHT) != 0 {
            let remaining = outer_remaining;
            let (height, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(height)
        } else {
            None
        };
        let border_width = if switch_expr & u16::from(ConfigWindow::BORDER_WIDTH) != 0 {
            let remaining = outer_remaining;
            let (border_width, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(border_width)
        } else {
            None
        };
        let sibling = if switch_expr & u16::from(ConfigWindow::SIBLING) != 0 {
            let remaining = outer_remaining;
            let (sibling, remaining) = Window::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(sibling)
        } else {
            None
        };
        let stack_mode = if switch_expr & u16::from(ConfigWindow::STACK_MODE) != 0 {
            let remaining = outer_remaining;
            let (stack_mode, remaining) = u32::try_parse(remaining)?;
            let stack_mode = stack_mode.into();
            outer_remaining = remaining;
            Some(stack_mode)
        } else {
            None
        };
        let result = ConfigureWindowAux { x, y, width, height, border_width, sibling, stack_mode };
        Ok((result, outer_remaining))
    }
}
impl ConfigureWindowAux {
    #[allow(dead_code)]
    fn serialize(&self, value_mask: u16) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u16::from(value_mask));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, value_mask: u16) {
        assert_eq!(self.switch_expr(), u16::from(value_mask), "switch `value_list` has an inconsistent discriminant");
        if let Some(x) = self.x {
            x.serialize_into(bytes);
        }
        if let Some(y) = self.y {
            y.serialize_into(bytes);
        }
        if let Some(width) = self.width {
            width.serialize_into(bytes);
        }
        if let Some(height) = self.height {
            height.serialize_into(bytes);
        }
        if let Some(border_width) = self.border_width {
            border_width.serialize_into(bytes);
        }
        if let Some(sibling) = self.sibling {
            sibling.serialize_into(bytes);
        }
        if let Some(stack_mode) = self.stack_mode {
            u32::from(stack_mode).serialize_into(bytes);
        }
    }
}
impl ConfigureWindowAux {
    fn switch_expr(&self) -> u16 {
        let mut expr_value = 0;
        if self.x.is_some() {
            expr_value |= u16::from(ConfigWindow::X);
        }
        if self.y.is_some() {
            expr_value |= u16::from(ConfigWindow::Y);
        }
        if self.width.is_some() {
            expr_value |= u16::from(ConfigWindow::WIDTH);
        }
        if self.height.is_some() {
            expr_value |= u16::from(ConfigWindow::HEIGHT);
        }
        if self.border_width.is_some() {
            expr_value |= u16::from(ConfigWindow::BORDER_WIDTH);
        }
        if self.sibling.is_some() {
            expr_value |= u16::from(ConfigWindow::SIBLING);
        }
        if self.stack_mode.is_some() {
            expr_value |= u16::from(ConfigWindow::STACK_MODE);
        }
        expr_value
    }
}
impl ConfigureWindowAux {
    /// Create a new instance with all fields unset / not present.
    pub fn new() -> Self {
        Default::default()
    }
    /// Set the `x` field of this structure.
    #[must_use]
    pub fn x<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.x = value.into();
        self
    }
    /// Set the `y` field of this structure.
    #[must_use]
    pub fn y<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.y = value.into();
        self
    }
    /// Set the `width` field of this structure.
    #[must_use]
    pub fn width<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.width = value.into();
        self
    }
    /// Set the `height` field of this structure.
    #[must_use]
    pub fn height<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.height = value.into();
        self
    }
    /// Set the `border_width` field of this structure.
    #[must_use]
    pub fn border_width<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.border_width = value.into();
        self
    }
    /// Set the `sibling` field of this structure.
    #[must_use]
    pub fn sibling<I>(mut self, value: I) -> Self where I: Into<Option<Window>> {
        self.sibling = value.into();
        self
    }
    /// Set the `stack_mode` field of this structure.
    #[must_use]
    pub fn stack_mode<I>(mut self, value: I) -> Self where I: Into<Option<StackMode>> {
        self.stack_mode = value.into();
        self
    }
}
impl ConfigureWindowAux {
    /// Construct from a [`ConfigureRequestEvent`].
    ///
    /// This function construct a new `ConfigureWindowAux` instance by accepting all requested
    /// changes from a `ConfigureRequestEvent`. This function is useful for window managers that want
    /// to handle `ConfigureRequestEvent`s.
    pub fn from_configure_request(event: &ConfigureRequestEvent) -> Self {
        let mut result = Self::new();
        let value_mask = u16::from(event.value_mask);
        if value_mask & u16::from(ConfigWindow::X) != 0 {
            result = result.x(i32::from(event.x));
        }
        if value_mask & u16::from(ConfigWindow::Y) != 0 {
            result = result.y(i32::from(event.y));
        }
        if value_mask & u16::from(ConfigWindow::WIDTH) != 0 {
            result = result.width(u32::from(event.width));
        }
        if value_mask & u16::from(ConfigWindow::HEIGHT) != 0 {
            result = result.height(u32::from(event.height));
        }
        if value_mask & u16::from(ConfigWindow::BORDER_WIDTH) != 0 {
            result = result.border_width(u32::from(event.border_width));
        }
        if value_mask & u16::from(ConfigWindow::SIBLING) != 0 {
            result = result.sibling(event.sibling);
        }
        if value_mask & u16::from(ConfigWindow::STACK_MODE) != 0 {
            result = result.stack_mode(event.stack_mode);
        }
        result
    }
}

/// Opcode for the ConfigureWindow request
pub const CONFIGURE_WINDOW_REQUEST: u8 = 12;
/// Configures window attributes.
///
/// Configures a window's size, position, border width and stacking order.
///
/// # Fields
///
/// * `window` - The window to configure.
/// * `value_list` - New values, corresponding to the attributes in value_mask. The order has to
/// correspond to the order of possible `value_mask` bits. See the example.
///
/// # Errors
///
/// * `Match` - You specified a Sibling without also specifying StackMode or the window is not
/// actually a Sibling.
/// * `Window` - The specified window does not exist. TODO: any other reason?
/// * `Value` - TODO: reasons?
///
/// # See
///
/// * `MapNotify`: event
/// * `Expose`: event
///
/// # Example
///
/// ```text
/// /*
///  * Configures the given window to the left upper corner
///  * with a size of 1024x768 pixels.
///  *
///  */
/// void my_example(xcb_connection_t *c, xcb_window_t window) {
///     uint16_t mask = 0;
///
///     mask |= XCB_CONFIG_WINDOW_X;
///     mask |= XCB_CONFIG_WINDOW_Y;
///     mask |= XCB_CONFIG_WINDOW_WIDTH;
///     mask |= XCB_CONFIG_WINDOW_HEIGHT;
///
///     const uint32_t values[] = {
///         0,    /* x */
///         0,    /* y */
///         1024, /* width */
///         768   /* height */
///     };
///
///     xcb_configure_window(c, window, mask, values);
///     xcb_flush(c);
/// }
/// ```
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConfigureWindowRequest<'input> {
    pub window: Window,
    pub value_list: Cow<'input, ConfigureWindowAux>,
}
impl_debug_if_no_extra_traits!(ConfigureWindowRequest<'_>, "ConfigureWindowRequest");
impl<'input> ConfigureWindowRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let value_mask: u16 = self.value_list.switch_expr();
        let value_mask_bytes = value_mask.serialize();
        let mut request0 = vec![
            CONFIGURE_WINDOW_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            value_mask_bytes[0],
            value_mask_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let value_list_bytes = self.value_list.serialize(u16::from(value_mask));
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
        if header.major_opcode != CONFIGURE_WINDOW_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let (value_mask, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (value_list, remaining) = ConfigureWindowAux::try_parse(remaining, u16::from(value_mask))?;
        let _ = remaining;
        Ok(ConfigureWindowRequest {
            window,
            value_list: Cow::Owned(value_list),
        })
    }
    /// Clone all borrowed data in this ConfigureWindowRequest.
    pub fn into_owned(self) -> ConfigureWindowRequest<'static> {
        ConfigureWindowRequest {
            window: self.window,
            value_list: Cow::Owned(self.value_list.into_owned()),
        }
    }
}
impl<'input> Request for ConfigureWindowRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for ConfigureWindowRequest<'input> {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Circulate(u8);
impl Circulate {
    pub const RAISE_LOWEST: Self = Self(0);
    pub const LOWER_HIGHEST: Self = Self(1);
}
impl From<Circulate> for u8 {
    #[inline]
    fn from(input: Circulate) -> Self {
        input.0
    }
}
impl From<Circulate> for Option<u8> {
    #[inline]
    fn from(input: Circulate) -> Self {
        Some(input.0)
    }
}
impl From<Circulate> for u16 {
    #[inline]
    fn from(input: Circulate) -> Self {
        u16::from(input.0)
    }
}
impl From<Circulate> for Option<u16> {
    #[inline]
    fn from(input: Circulate) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Circulate> for u32 {
    #[inline]
    fn from(input: Circulate) -> Self {
        u32::from(input.0)
    }
}
impl From<Circulate> for Option<u32> {
    #[inline]
    fn from(input: Circulate) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Circulate {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Circulate  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::RAISE_LOWEST.0.into(), "RAISE_LOWEST", "RaiseLowest"),
            (Self::LOWER_HIGHEST.0.into(), "LOWER_HIGHEST", "LowerHighest"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the CirculateWindow request
pub const CIRCULATE_WINDOW_REQUEST: u8 = 13;
/// Change window stacking order.
///
/// If `direction` is `XCB_CIRCULATE_RAISE_LOWEST`, the lowest mapped child (if
/// any) will be raised to the top of the stack.
///
/// If `direction` is `XCB_CIRCULATE_LOWER_HIGHEST`, the highest mapped child will
/// be lowered to the bottom of the stack.
///
/// # Fields
///
/// * `direction` -
/// * `window` - The window to raise/lower (depending on `direction`).
///
/// # Errors
///
/// * `Window` - The specified `window` does not exist.
/// * `Value` - The specified `direction` is invalid.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CirculateWindowRequest {
    pub direction: Circulate,
    pub window: Window,
}
impl_debug_if_no_extra_traits!(CirculateWindowRequest, "CirculateWindowRequest");
impl CirculateWindowRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let direction_bytes = u8::from(self.direction).serialize();
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            CIRCULATE_WINDOW_REQUEST,
            direction_bytes[0],
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.major_opcode != CIRCULATE_WINDOW_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (direction, remaining) = u8::try_parse(remaining)?;
        let direction = direction.into();
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let _ = remaining;
        Ok(CirculateWindowRequest {
            direction,
            window,
        })
    }
}
impl Request for CirculateWindowRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CirculateWindowRequest {
}

/// Opcode for the GetGeometry request
pub const GET_GEOMETRY_REQUEST: u8 = 14;
/// Get current window geometry.
///
/// Gets the current geometry of the specified drawable (either `Window` or `Pixmap`).
///
/// # Fields
///
/// * `drawable` - The drawable (`Window` or `Pixmap`) of which the geometry will be received.
///
/// # Errors
///
/// * `Drawable` - TODO: reasons?
/// * `Window` - TODO: reasons?
///
/// # See
///
/// * `xwininfo`: program
///
/// # Example
///
/// ```text
/// /*
///  * Displays the x and y position of the given window.
///  *
///  */
/// void my_example(xcb_connection_t *c, xcb_window_t window) {
///     xcb_get_geometry_cookie_t cookie;
///     xcb_get_geometry_reply_t *reply;
///
///     cookie = xcb_get_geometry(c, window);
///     /* ... do other work here if possible ... */
///     if ((reply = xcb_get_geometry_reply(c, cookie, NULL))) {
///         printf("This window is at %d, %d\\n", reply->x, reply->y);
///     }
///     free(reply);
/// }
/// ```
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetGeometryRequest {
    pub drawable: Drawable,
}
impl_debug_if_no_extra_traits!(GetGeometryRequest, "GetGeometryRequest");
impl GetGeometryRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let mut request0 = vec![
            GET_GEOMETRY_REQUEST,
            0,
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
        if header.major_opcode != GET_GEOMETRY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let _ = remaining;
        Ok(GetGeometryRequest {
            drawable,
        })
    }
}
impl Request for GetGeometryRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetGeometryRequest {
    type Reply = GetGeometryReply;
}

/// # Fields
///
/// * `root` - Root window of the screen containing `drawable`.
/// * `x` - The X coordinate of `drawable`. If `drawable` is a window, the coordinate
/// specifies the upper-left outer corner relative to its parent's origin. If
/// `drawable` is a pixmap, the X coordinate is always 0.
/// * `y` - The Y coordinate of `drawable`. If `drawable` is a window, the coordinate
/// specifies the upper-left outer corner relative to its parent's origin. If
/// `drawable` is a pixmap, the Y coordinate is always 0.
/// * `width` - The width of `drawable`.
/// * `height` - The height of `drawable`.
/// * `border_width` - The border width (in pixels).
/// * `depth` - The depth of the drawable (bits per pixel for the object).
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetGeometryReply {
    pub depth: u8,
    pub sequence: u16,
    pub length: u32,
    pub root: Window,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub border_width: u16,
}
impl_debug_if_no_extra_traits!(GetGeometryReply, "GetGeometryReply");
impl TryParse for GetGeometryReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (depth, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (root, remaining) = Window::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (border_width, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetGeometryReply { depth, sequence, length, root, x, y, width, height, border_width };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetGeometryReply {
    type Bytes = [u8; 24];
    fn serialize(&self) -> [u8; 24] {
        let response_type_bytes = &[1];
        let depth_bytes = self.depth.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let root_bytes = self.root.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let border_width_bytes = self.border_width.serialize();
        [
            response_type_bytes[0],
            depth_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            root_bytes[0],
            root_bytes[1],
            root_bytes[2],
            root_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            border_width_bytes[0],
            border_width_bytes[1],
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(24);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.depth.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.root.serialize_into(bytes);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        self.border_width.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
    }
}

/// Opcode for the QueryTree request
pub const QUERY_TREE_REQUEST: u8 = 15;
/// query the window tree.
///
/// Gets the root window ID, parent window ID and list of children windows for the
/// specified `window`. The children are listed in bottom-to-top stacking order.
///
/// # Fields
///
/// * `window` - The `window` to query.
///
/// # See
///
/// * `xwininfo`: program
///
/// # Example
///
/// ```text
/// /*
///  * Displays the root, parent and children of the specified window.
///  *
///  */
/// void my_example(xcb_connection_t *conn, xcb_window_t window) {
///     xcb_query_tree_cookie_t cookie;
///     xcb_query_tree_reply_t *reply;
///
///     cookie = xcb_query_tree(conn, window);
///     if ((reply = xcb_query_tree_reply(conn, cookie, NULL))) {
///         printf("root = 0x%08x\\n", reply->root);
///         printf("parent = 0x%08x\\n", reply->parent);
///
///         xcb_window_t *children = xcb_query_tree_children(reply);
///         for (int i = 0; i < xcb_query_tree_children_length(reply); i++)
///             printf("child window = 0x%08x\\n", children[i]);
///
///         free(reply);
///     }
/// }
/// ```
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryTreeRequest {
    pub window: Window,
}
impl_debug_if_no_extra_traits!(QueryTreeRequest, "QueryTreeRequest");
impl QueryTreeRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            QUERY_TREE_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.major_opcode != QUERY_TREE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let _ = remaining;
        Ok(QueryTreeRequest {
            window,
        })
    }
}
impl Request for QueryTreeRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryTreeRequest {
    type Reply = QueryTreeReply;
}

/// # Fields
///
/// * `root` - The root window of `window`.
/// * `parent` - The parent window of `window`.
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryTreeReply {
    pub sequence: u16,
    pub length: u32,
    pub root: Window,
    pub parent: Window,
    pub children: Vec<Window>,
}
impl_debug_if_no_extra_traits!(QueryTreeReply, "QueryTreeReply");
impl TryParse for QueryTreeReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (root, remaining) = Window::try_parse(remaining)?;
        let (parent, remaining) = Window::try_parse(remaining)?;
        let (children_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(14..).ok_or(ParseError::InsufficientData)?;
        let (children, remaining) = crate::x11_utils::parse_list::<Window>(remaining, children_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryTreeReply { sequence, length, root, parent, children };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryTreeReply {
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
        self.root.serialize_into(bytes);
        self.parent.serialize_into(bytes);
        let children_len = u16::try_from(self.children.len()).expect("`children` has too many elements");
        children_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 14]);
        self.children.serialize_into(bytes);
    }
}
impl QueryTreeReply {
    /// Get the value of the `children_len` field.
    ///
    /// The `children_len` field is used as the length field of the `children` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn children_len(&self) -> u16 {
        self.children.len()
            .try_into().unwrap()
    }
}

/// Opcode for the InternAtom request
pub const INTERN_ATOM_REQUEST: u8 = 16;
/// Get atom identifier by name.
///
/// Retrieves the identifier (xcb_atom_t TODO) for the atom with the specified
/// name. Atoms are used in protocols like EWMH, for example to store window titles
/// (`_NET_WM_NAME` atom) as property of a window.
///
/// If `only_if_exists` is 0, the atom will be created if it does not already exist.
/// If `only_if_exists` is 1, `XCB_ATOM_NONE` will be returned if the atom does
/// not yet exist.
///
/// # Fields
///
/// * `name` - The name of the atom.
/// * `only_if_exists` - Return a valid atom id only if the atom already exists.
///
/// # Errors
///
/// * `Alloc` - TODO: reasons?
/// * `Value` - A value other than 0 or 1 was specified for `only_if_exists`.
///
/// # See
///
/// * `xlsatoms`: program
/// * `GetAtomName`: request
///
/// # Example
///
/// ```text
/// /*
///  * Resolves the _NET_WM_NAME atom.
///  *
///  */
/// void my_example(xcb_connection_t *c) {
///     xcb_intern_atom_cookie_t cookie;
///     xcb_intern_atom_reply_t *reply;
///
///     cookie = xcb_intern_atom(c, 0, strlen("_NET_WM_NAME"), "_NET_WM_NAME");
///     /* ... do other work here if possible ... */
///     if ((reply = xcb_intern_atom_reply(c, cookie, NULL))) {
///         printf("The _NET_WM_NAME atom has ID %u\n", reply->atom);
///         free(reply);
///     }
/// }
/// ```
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InternAtomRequest<'input> {
    pub only_if_exists: bool,
    pub name: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(InternAtomRequest<'_>, "InternAtomRequest");
impl<'input> InternAtomRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let only_if_exists_bytes = self.only_if_exists.serialize();
        let name_len = u16::try_from(self.name.len()).expect("`name` has too many elements");
        let name_len_bytes = name_len.serialize();
        let mut request0 = vec![
            INTERN_ATOM_REQUEST,
            only_if_exists_bytes[0],
            0,
            0,
            name_len_bytes[0],
            name_len_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.name.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.name, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != INTERN_ATOM_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (only_if_exists, remaining) = bool::try_parse(remaining)?;
        let _ = remaining;
        let (name_len, remaining) = u16::try_parse(value)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, name_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(InternAtomRequest {
            only_if_exists,
            name: Cow::Borrowed(name),
        })
    }
    /// Clone all borrowed data in this InternAtomRequest.
    pub fn into_owned(self) -> InternAtomRequest<'static> {
        InternAtomRequest {
            only_if_exists: self.only_if_exists,
            name: Cow::Owned(self.name.into_owned()),
        }
    }
}
impl<'input> Request for InternAtomRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for InternAtomRequest<'input> {
    type Reply = InternAtomReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InternAtomReply {
    pub sequence: u16,
    pub length: u32,
    pub atom: Atom,
}
impl_debug_if_no_extra_traits!(InternAtomReply, "InternAtomReply");
impl TryParse for InternAtomReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (atom, remaining) = Atom::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = InternAtomReply { sequence, length, atom };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for InternAtomReply {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let atom_bytes = self.atom.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            atom_bytes[0],
            atom_bytes[1],
            atom_bytes[2],
            atom_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.atom.serialize_into(bytes);
    }
}

/// Opcode for the GetAtomName request
pub const GET_ATOM_NAME_REQUEST: u8 = 17;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetAtomNameRequest {
    pub atom: Atom,
}
impl_debug_if_no_extra_traits!(GetAtomNameRequest, "GetAtomNameRequest");
impl GetAtomNameRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let atom_bytes = self.atom.serialize();
        let mut request0 = vec![
            GET_ATOM_NAME_REQUEST,
            0,
            0,
            0,
            atom_bytes[0],
            atom_bytes[1],
            atom_bytes[2],
            atom_bytes[3],
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
        if header.major_opcode != GET_ATOM_NAME_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (atom, remaining) = Atom::try_parse(value)?;
        let _ = remaining;
        Ok(GetAtomNameRequest {
            atom,
        })
    }
}
impl Request for GetAtomNameRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetAtomNameRequest {
    type Reply = GetAtomNameReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetAtomNameReply {
    pub sequence: u16,
    pub length: u32,
    pub name: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetAtomNameReply, "GetAtomNameReply");
impl TryParse for GetAtomNameReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (name_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, name_len.try_to_usize()?)?;
        let name = name.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetAtomNameReply { sequence, length, name };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetAtomNameReply {
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
        let name_len = u16::try_from(self.name.len()).expect("`name` has too many elements");
        name_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
        bytes.extend_from_slice(&self.name);
    }
}
impl GetAtomNameReply {
    /// Get the value of the `name_len` field.
    ///
    /// The `name_len` field is used as the length field of the `name` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn name_len(&self) -> u16 {
        self.name.len()
            .try_into().unwrap()
    }
}

/// # Fields
///
/// * `Replace` - Discard the previous property value and store the new data.
/// * `Prepend` - Insert the new data before the beginning of existing data. The `format` must
/// match existing property value. If the property is undefined, it is treated as
/// defined with the correct type and format with zero-length data.
/// * `Append` - Insert the new data after the beginning of existing data. The `format` must
/// match existing property value. If the property is undefined, it is treated as
/// defined with the correct type and format with zero-length data.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PropMode(u8);
impl PropMode {
    pub const REPLACE: Self = Self(0);
    pub const PREPEND: Self = Self(1);
    pub const APPEND: Self = Self(2);
}
impl From<PropMode> for u8 {
    #[inline]
    fn from(input: PropMode) -> Self {
        input.0
    }
}
impl From<PropMode> for Option<u8> {
    #[inline]
    fn from(input: PropMode) -> Self {
        Some(input.0)
    }
}
impl From<PropMode> for u16 {
    #[inline]
    fn from(input: PropMode) -> Self {
        u16::from(input.0)
    }
}
impl From<PropMode> for Option<u16> {
    #[inline]
    fn from(input: PropMode) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<PropMode> for u32 {
    #[inline]
    fn from(input: PropMode) -> Self {
        u32::from(input.0)
    }
}
impl From<PropMode> for Option<u32> {
    #[inline]
    fn from(input: PropMode) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for PropMode {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for PropMode  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::REPLACE.0.into(), "REPLACE", "Replace"),
            (Self::PREPEND.0.into(), "PREPEND", "Prepend"),
            (Self::APPEND.0.into(), "APPEND", "Append"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the ChangeProperty request
pub const CHANGE_PROPERTY_REQUEST: u8 = 18;
/// Changes a window property.
///
/// Sets or updates a property on the specified `window`. Properties are for
/// example the window title (`WM_NAME`) or its minimum size (`WM_NORMAL_HINTS`).
/// Protocols such as EWMH also use properties - for example EWMH defines the
/// window title, encoded as UTF-8 string, in the `_NET_WM_NAME` property.
///
/// # Fields
///
/// * `window` - The window whose property you want to change.
/// * `mode` -
/// * `property` - The property you want to change (an atom).
/// * `type` - The type of the property you want to change (an atom).
/// * `format` - Specifies whether the data should be viewed as a list of 8-bit, 16-bit or
/// 32-bit quantities. Possible values are 8, 16 and 32. This information allows
/// the X server to correctly perform byte-swap operations as necessary.
/// * `data_len` - Specifies the number of elements (see `format`).
/// * `data` - The property data.
///
/// # Errors
///
/// * `Match` - TODO: reasons?
/// * `Value` - TODO: reasons?
/// * `Window` - The specified `window` does not exist.
/// * `Atom` - `property` or `type` do not refer to a valid atom.
/// * `Alloc` - The X server could not store the property (no memory?).
///
/// # See
///
/// * `InternAtom`: request
/// * `xprop`: program
///
/// # Example
///
/// ```text
/// /*
///  * Sets the WM_NAME property of the window to "XCB Example".
///  *
///  */
/// void my_example(xcb_connection_t *conn, xcb_window_t window) {
///     xcb_change_property(conn,
///         XCB_PROP_MODE_REPLACE,
///         window,
///         XCB_ATOM_WM_NAME,
///         XCB_ATOM_STRING,
///         8,
///         strlen("XCB Example"),
///         "XCB Example");
///     xcb_flush(conn);
/// }
/// ```
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangePropertyRequest<'input> {
    pub mode: PropMode,
    pub window: Window,
    pub property: Atom,
    pub type_: Atom,
    pub format: u8,
    pub data_len: u32,
    pub data: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(ChangePropertyRequest<'_>, "ChangePropertyRequest");
impl<'input> ChangePropertyRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let mode_bytes = u8::from(self.mode).serialize();
        let window_bytes = self.window.serialize();
        let property_bytes = self.property.serialize();
        let type_bytes = self.type_.serialize();
        let format_bytes = self.format.serialize();
        let data_len_bytes = self.data_len.serialize();
        let mut request0 = vec![
            CHANGE_PROPERTY_REQUEST,
            mode_bytes[0],
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            property_bytes[0],
            property_bytes[1],
            property_bytes[2],
            property_bytes[3],
            type_bytes[0],
            type_bytes[1],
            type_bytes[2],
            type_bytes[3],
            format_bytes[0],
            0,
            0,
            0,
            data_len_bytes[0],
            data_len_bytes[1],
            data_len_bytes[2],
            data_len_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(self.data.len(), usize::try_from(u32::from(self.data_len).checked_mul(u32::from(self.format)).unwrap().checked_div(8u32).unwrap()).unwrap(), "`data` has an incorrect length");
        let length_so_far = length_so_far + self.data.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.data, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != CHANGE_PROPERTY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (mode, remaining) = u8::try_parse(remaining)?;
        let mode = mode.into();
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let (property, remaining) = Atom::try_parse(remaining)?;
        let (type_, remaining) = Atom::try_parse(remaining)?;
        let (format, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (data_len, remaining) = u32::try_parse(remaining)?;
        let (data, remaining) = crate::x11_utils::parse_u8_list(remaining, u32::from(data_len).checked_mul(u32::from(format)).ok_or(ParseError::InvalidExpression)?.checked_div(8u32).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let _ = remaining;
        Ok(ChangePropertyRequest {
            mode,
            window,
            property,
            type_,
            format,
            data_len,
            data: Cow::Borrowed(data),
        })
    }
    /// Clone all borrowed data in this ChangePropertyRequest.
    pub fn into_owned(self) -> ChangePropertyRequest<'static> {
        ChangePropertyRequest {
            mode: self.mode,
            window: self.window,
            property: self.property,
            type_: self.type_,
            format: self.format,
            data_len: self.data_len,
            data: Cow::Owned(self.data.into_owned()),
        }
    }
}
impl<'input> Request for ChangePropertyRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for ChangePropertyRequest<'input> {
}

/// Opcode for the DeleteProperty request
pub const DELETE_PROPERTY_REQUEST: u8 = 19;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeletePropertyRequest {
    pub window: Window,
    pub property: Atom,
}
impl_debug_if_no_extra_traits!(DeletePropertyRequest, "DeletePropertyRequest");
impl DeletePropertyRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let property_bytes = self.property.serialize();
        let mut request0 = vec![
            DELETE_PROPERTY_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            property_bytes[0],
            property_bytes[1],
            property_bytes[2],
            property_bytes[3],
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
        if header.major_opcode != DELETE_PROPERTY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let (property, remaining) = Atom::try_parse(remaining)?;
        let _ = remaining;
        Ok(DeletePropertyRequest {
            window,
            property,
        })
    }
}
impl Request for DeletePropertyRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DeletePropertyRequest {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPropertyType(u8);
impl GetPropertyType {
    pub const ANY: Self = Self(0);
}
impl From<GetPropertyType> for u8 {
    #[inline]
    fn from(input: GetPropertyType) -> Self {
        input.0
    }
}
impl From<GetPropertyType> for Option<u8> {
    #[inline]
    fn from(input: GetPropertyType) -> Self {
        Some(input.0)
    }
}
impl From<GetPropertyType> for u16 {
    #[inline]
    fn from(input: GetPropertyType) -> Self {
        u16::from(input.0)
    }
}
impl From<GetPropertyType> for Option<u16> {
    #[inline]
    fn from(input: GetPropertyType) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<GetPropertyType> for u32 {
    #[inline]
    fn from(input: GetPropertyType) -> Self {
        u32::from(input.0)
    }
}
impl From<GetPropertyType> for Option<u32> {
    #[inline]
    fn from(input: GetPropertyType) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for GetPropertyType {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for GetPropertyType  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ANY.0.into(), "ANY", "Any"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the GetProperty request
pub const GET_PROPERTY_REQUEST: u8 = 20;
/// Gets a window property.
///
/// Gets the specified `property` from the specified `window`. Properties are for
/// example the window title (`WM_NAME`) or its minimum size (`WM_NORMAL_HINTS`).
/// Protocols such as EWMH also use properties - for example EWMH defines the
/// window title, encoded as UTF-8 string, in the `_NET_WM_NAME` property.
///
/// TODO: talk about `type`
///
/// TODO: talk about `delete`
///
/// TODO: talk about the offset/length thing. what's a valid use case?
///
/// # Fields
///
/// * `window` - The window whose property you want to get.
/// * `delete` - Whether the property should actually be deleted. For deleting a property, the
/// specified `type` has to match the actual property type.
/// * `property` - The property you want to get (an atom).
/// * `type` - The type of the property you want to get (an atom).
/// * `long_offset` - Specifies the offset (in 32-bit multiples) in the specified property where the
/// data is to be retrieved.
/// * `long_length` - Specifies how many 32-bit multiples of data should be retrieved (e.g. if you
/// set `long_length` to 4, you will receive 16 bytes of data).
///
/// # Errors
///
/// * `Window` - The specified `window` does not exist.
/// * `Atom` - `property` or `type` do not refer to a valid atom.
/// * `Value` - The specified `long_offset` is beyond the actual property length (e.g. the
/// property has a length of 3 bytes and you are setting `long_offset` to 1,
/// resulting in a byte offset of 4).
///
/// # See
///
/// * `InternAtom`: request
/// * `xprop`: program
///
/// # Example
///
/// ```text
/// /*
///  * Prints the WM_NAME property of the window.
///  *
///  */
/// void my_example(xcb_connection_t *c, xcb_window_t window) {
///     xcb_get_property_cookie_t cookie;
///     xcb_get_property_reply_t *reply;
///
///     /* These atoms are predefined in the X11 protocol. */
///     xcb_atom_t property = XCB_ATOM_WM_NAME;
///     xcb_atom_t type = XCB_ATOM_STRING;
///
///     // TODO: a reasonable long_length for WM_NAME?
///     cookie = xcb_get_property(c, 0, window, property, type, 0, 0);
///     if ((reply = xcb_get_property_reply(c, cookie, NULL))) {
///         int len = xcb_get_property_value_length(reply);
///         if (len == 0) {
///             printf("TODO\\n");
///             free(reply);
///             return;
///         }
///         printf("WM_NAME is %.*s\\n", len,
///                (char*)xcb_get_property_value(reply));
///     }
///     free(reply);
/// }
/// ```
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPropertyRequest {
    pub delete: bool,
    pub window: Window,
    pub property: Atom,
    pub type_: Atom,
    pub long_offset: u32,
    pub long_length: u32,
}
impl_debug_if_no_extra_traits!(GetPropertyRequest, "GetPropertyRequest");
impl GetPropertyRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let delete_bytes = self.delete.serialize();
        let window_bytes = self.window.serialize();
        let property_bytes = self.property.serialize();
        let type_bytes = self.type_.serialize();
        let long_offset_bytes = self.long_offset.serialize();
        let long_length_bytes = self.long_length.serialize();
        let mut request0 = vec![
            GET_PROPERTY_REQUEST,
            delete_bytes[0],
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            property_bytes[0],
            property_bytes[1],
            property_bytes[2],
            property_bytes[3],
            type_bytes[0],
            type_bytes[1],
            type_bytes[2],
            type_bytes[3],
            long_offset_bytes[0],
            long_offset_bytes[1],
            long_offset_bytes[2],
            long_offset_bytes[3],
            long_length_bytes[0],
            long_length_bytes[1],
            long_length_bytes[2],
            long_length_bytes[3],
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
        if header.major_opcode != GET_PROPERTY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (delete, remaining) = bool::try_parse(remaining)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let (property, remaining) = Atom::try_parse(remaining)?;
        let (type_, remaining) = Atom::try_parse(remaining)?;
        let (long_offset, remaining) = u32::try_parse(remaining)?;
        let (long_length, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetPropertyRequest {
            delete,
            window,
            property,
            type_,
            long_offset,
            long_length,
        })
    }
}
impl Request for GetPropertyRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetPropertyRequest {
    type Reply = GetPropertyReply;
}
impl GetPropertyReply {
    /// Iterate over the contained value if its format is 8.
    ///
    /// This function checks if the `format` member of the reply
    /// is 8. If it it is not, `None` is returned. Otherwise
    /// and iterator is returned that interprets the value in
    /// this reply as type `u8`.
    ///
    /// # Examples
    ///
    /// Successfully iterate over the value:
    /// ```
    /// // First, we have to 'invent' a GetPropertyReply.
    /// let reply = x11rb_protocol::protocol::xproto::GetPropertyReply {
    ///     format: 8,
    ///     sequence: 0,
    ///     length: 0, // This value is incorrect
    ///     type_: 0, // This value is incorrect
    ///     bytes_after: 0,
    ///     value_len: 4,
    ///     value: vec![1, 2, 3, 4],
    /// };
    ///
    /// // This is the actual example: Iterate over the value.
    /// let mut iter = reply.value8().unwrap();
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), Some(4));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// An iterator is only returned when the `format` is correct.
    /// The following example shows this.
    /// ```
    /// // First, we have to 'invent' a GetPropertyReply.
    /// let reply = x11rb_protocol::protocol::xproto::GetPropertyReply {
    ///     format: 42, // Not allowed in X11, but used for the example
    ///     sequence: 0,
    ///     length: 0, // This value is incorrect
    ///     type_: 0, // This value is incorrect
    ///     bytes_after: 0,
    ///     value_len: 4,
    ///     value: vec![1, 2, 3, 4],
    /// };
    /// assert!(reply.value8().is_none());
    /// ```
    pub fn value8(&self) -> Option<impl Iterator<Item=u8> + '_> {
        if self.format == 8 {
            Some(crate::wrapper::PropertyIterator::new(&self.value))
        } else {
            None
        }
    }
    /// Iterate over the contained value if its format is 16.
    ///
    /// This function checks if the `format` member of the reply
    /// is 16. If it it is not, `None` is returned. Otherwise
    /// and iterator is returned that interprets the value in
    /// this reply as type `u16`.
    ///
    /// # Examples
    ///
    /// Successfully iterate over the value:
    /// ```
    /// // First, we have to 'invent' a GetPropertyReply.
    /// let reply = x11rb_protocol::protocol::xproto::GetPropertyReply {
    ///     format: 16,
    ///     sequence: 0,
    ///     length: 0, // This value is incorrect
    ///     type_: 0, // This value is incorrect
    ///     bytes_after: 0,
    ///     value_len: 4,
    ///     value: vec![1, 1, 2, 2],
    /// };
    ///
    /// // This is the actual example: Iterate over the value.
    /// let mut iter = reply.value16().unwrap();
    /// assert_eq!(iter.next(), Some(257));
    /// assert_eq!(iter.next(), Some(514));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// An iterator is only returned when the `format` is correct.
    /// The following example shows this.
    /// ```
    /// // First, we have to 'invent' a GetPropertyReply.
    /// let reply = x11rb_protocol::protocol::xproto::GetPropertyReply {
    ///     format: 42, // Not allowed in X11, but used for the example
    ///     sequence: 0,
    ///     length: 0, // This value is incorrect
    ///     type_: 0, // This value is incorrect
    ///     bytes_after: 0,
    ///     value_len: 4,
    ///     value: vec![1, 2, 3, 4],
    /// };
    /// assert!(reply.value16().is_none());
    /// ```
    pub fn value16(&self) -> Option<impl Iterator<Item=u16> + '_> {
        if self.format == 16 {
            Some(crate::wrapper::PropertyIterator::new(&self.value))
        } else {
            None
        }
    }
    /// Iterate over the contained value if its format is 32.
    ///
    /// This function checks if the `format` member of the reply
    /// is 32. If it it is not, `None` is returned. Otherwise
    /// and iterator is returned that interprets the value in
    /// this reply as type `u32`.
    ///
    /// # Examples
    ///
    /// Successfully iterate over the value:
    /// ```
    /// // First, we have to 'invent' a GetPropertyReply.
    /// let reply = x11rb_protocol::protocol::xproto::GetPropertyReply {
    ///     format: 32,
    ///     sequence: 0,
    ///     length: 0, // This value is incorrect
    ///     type_: 0, // This value is incorrect
    ///     bytes_after: 0,
    ///     value_len: 4,
    ///     value: vec![1, 2, 2, 1],
    /// };
    ///
    /// // This is the actual example: Iterate over the value.
    /// let mut iter = reply.value32().unwrap();
    /// assert_eq!(iter.next(), Some(16908801));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// An iterator is only returned when the `format` is correct.
    /// The following example shows this.
    /// ```
    /// // First, we have to 'invent' a GetPropertyReply.
    /// let reply = x11rb_protocol::protocol::xproto::GetPropertyReply {
    ///     format: 42, // Not allowed in X11, but used for the example
    ///     sequence: 0,
    ///     length: 0, // This value is incorrect
    ///     type_: 0, // This value is incorrect
    ///     bytes_after: 0,
    ///     value_len: 4,
    ///     value: vec![1, 2, 3, 4],
    /// };
    /// assert!(reply.value32().is_none());
    /// ```
    pub fn value32(&self) -> Option<impl Iterator<Item=u32> + '_> {
        if self.format == 32 {
            Some(crate::wrapper::PropertyIterator::new(&self.value))
        } else {
            None
        }
    }
}


/// # Fields
///
/// * `format` - Specifies whether the data should be viewed as a list of 8-bit, 16-bit, or
/// 32-bit quantities. Possible values are 8, 16, and 32. This information allows
/// the X server to correctly perform byte-swap operations as necessary.
/// * `type` - The actual type of the property (an atom).
/// * `bytes_after` - The number of bytes remaining to be read in the property if a partial read was
/// performed.
/// * `value_len` - The length of value. You should use the corresponding accessor instead of this
/// field.
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPropertyReply {
    pub format: u8,
    pub sequence: u16,
    pub length: u32,
    pub type_: Atom,
    pub bytes_after: u32,
    pub value_len: u32,
    pub value: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetPropertyReply, "GetPropertyReply");
impl TryParse for GetPropertyReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (format, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (type_, remaining) = Atom::try_parse(remaining)?;
        let (bytes_after, remaining) = u32::try_parse(remaining)?;
        let (value_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(12..).ok_or(ParseError::InsufficientData)?;
        let (value, remaining) = crate::x11_utils::parse_u8_list(remaining, u32::from(value_len).checked_mul(u32::from(format).checked_div(8u32).ok_or(ParseError::InvalidExpression)?).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let value = value.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetPropertyReply { format, sequence, length, type_, bytes_after, value_len, value };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetPropertyReply {
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
        self.format.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.type_.serialize_into(bytes);
        self.bytes_after.serialize_into(bytes);
        self.value_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 12]);
        assert_eq!(self.value.len(), usize::try_from(u32::from(self.value_len).checked_mul(u32::from(self.format).checked_div(8u32).unwrap()).unwrap()).unwrap(), "`value` has an incorrect length");
        bytes.extend_from_slice(&self.value);
    }
}

/// Opcode for the ListProperties request
pub const LIST_PROPERTIES_REQUEST: u8 = 21;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListPropertiesRequest {
    pub window: Window,
}
impl_debug_if_no_extra_traits!(ListPropertiesRequest, "ListPropertiesRequest");
impl ListPropertiesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            LIST_PROPERTIES_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.major_opcode != LIST_PROPERTIES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let _ = remaining;
        Ok(ListPropertiesRequest {
            window,
        })
    }
}
impl Request for ListPropertiesRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for ListPropertiesRequest {
    type Reply = ListPropertiesReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListPropertiesReply {
    pub sequence: u16,
    pub length: u32,
    pub atoms: Vec<Atom>,
}
impl_debug_if_no_extra_traits!(ListPropertiesReply, "ListPropertiesReply");
impl TryParse for ListPropertiesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (atoms_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
        let (atoms, remaining) = crate::x11_utils::parse_list::<Atom>(remaining, atoms_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = ListPropertiesReply { sequence, length, atoms };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ListPropertiesReply {
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
        let atoms_len = u16::try_from(self.atoms.len()).expect("`atoms` has too many elements");
        atoms_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
        self.atoms.serialize_into(bytes);
    }
}
impl ListPropertiesReply {
    /// Get the value of the `atoms_len` field.
    ///
    /// The `atoms_len` field is used as the length field of the `atoms` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn atoms_len(&self) -> u16 {
        self.atoms.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SetSelectionOwner request
pub const SET_SELECTION_OWNER_REQUEST: u8 = 22;
/// Sets the owner of a selection.
///
/// Makes `window` the owner of the selection `selection` and updates the
/// last-change time of the specified selection.
///
/// TODO: briefly explain what a selection is.
///
/// # Fields
///
/// * `selection` - The selection.
/// * `owner` - The new owner of the selection.
///
/// The special value `XCB_NONE` means that the selection will have no owner.
/// * `time` - Timestamp to avoid race conditions when running X over the network.
///
/// The selection will not be changed if `time` is earlier than the current
/// last-change time of the `selection` or is later than the current X server time.
/// Otherwise, the last-change time is set to the specified time.
///
/// The special value `XCB_CURRENT_TIME` will be replaced with the current server
/// time.
///
/// # Errors
///
/// * `Atom` - `selection` does not refer to a valid atom.
///
/// # See
///
/// * `SetSelectionOwner`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetSelectionOwnerRequest {
    pub owner: Window,
    pub selection: Atom,
    pub time: Timestamp,
}
impl_debug_if_no_extra_traits!(SetSelectionOwnerRequest, "SetSelectionOwnerRequest");
impl SetSelectionOwnerRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let owner_bytes = self.owner.serialize();
        let selection_bytes = self.selection.serialize();
        let time_bytes = self.time.serialize();
        let mut request0 = vec![
            SET_SELECTION_OWNER_REQUEST,
            0,
            0,
            0,
            owner_bytes[0],
            owner_bytes[1],
            owner_bytes[2],
            owner_bytes[3],
            selection_bytes[0],
            selection_bytes[1],
            selection_bytes[2],
            selection_bytes[3],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
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
        if header.major_opcode != SET_SELECTION_OWNER_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (owner, remaining) = Window::try_parse(value)?;
        let (selection, remaining) = Atom::try_parse(remaining)?;
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let _ = remaining;
        Ok(SetSelectionOwnerRequest {
            owner,
            selection,
            time,
        })
    }
}
impl Request for SetSelectionOwnerRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SetSelectionOwnerRequest {
}

/// Opcode for the GetSelectionOwner request
pub const GET_SELECTION_OWNER_REQUEST: u8 = 23;
/// Gets the owner of a selection.
///
/// Gets the owner of the specified selection.
///
/// TODO: briefly explain what a selection is.
///
/// # Fields
///
/// * `selection` - The selection.
///
/// # Errors
///
/// * `Atom` - `selection` does not refer to a valid atom.
///
/// # See
///
/// * `SetSelectionOwner`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetSelectionOwnerRequest {
    pub selection: Atom,
}
impl_debug_if_no_extra_traits!(GetSelectionOwnerRequest, "GetSelectionOwnerRequest");
impl GetSelectionOwnerRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let selection_bytes = self.selection.serialize();
        let mut request0 = vec![
            GET_SELECTION_OWNER_REQUEST,
            0,
            0,
            0,
            selection_bytes[0],
            selection_bytes[1],
            selection_bytes[2],
            selection_bytes[3],
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
        if header.major_opcode != GET_SELECTION_OWNER_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (selection, remaining) = Atom::try_parse(value)?;
        let _ = remaining;
        Ok(GetSelectionOwnerRequest {
            selection,
        })
    }
}
impl Request for GetSelectionOwnerRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetSelectionOwnerRequest {
    type Reply = GetSelectionOwnerReply;
}

/// # Fields
///
/// * `owner` - The current selection owner window.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetSelectionOwnerReply {
    pub sequence: u16,
    pub length: u32,
    pub owner: Window,
}
impl_debug_if_no_extra_traits!(GetSelectionOwnerReply, "GetSelectionOwnerReply");
impl TryParse for GetSelectionOwnerReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (owner, remaining) = Window::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetSelectionOwnerReply { sequence, length, owner };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetSelectionOwnerReply {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let owner_bytes = self.owner.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            owner_bytes[0],
            owner_bytes[1],
            owner_bytes[2],
            owner_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.owner.serialize_into(bytes);
    }
}

/// Opcode for the ConvertSelection request
pub const CONVERT_SELECTION_REQUEST: u8 = 24;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConvertSelectionRequest {
    pub requestor: Window,
    pub selection: Atom,
    pub target: Atom,
    pub property: Atom,
    pub time: Timestamp,
}
impl_debug_if_no_extra_traits!(ConvertSelectionRequest, "ConvertSelectionRequest");
impl ConvertSelectionRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let requestor_bytes = self.requestor.serialize();
        let selection_bytes = self.selection.serialize();
        let target_bytes = self.target.serialize();
        let property_bytes = self.property.serialize();
        let time_bytes = self.time.serialize();
        let mut request0 = vec![
            CONVERT_SELECTION_REQUEST,
            0,
            0,
            0,
            requestor_bytes[0],
            requestor_bytes[1],
            requestor_bytes[2],
            requestor_bytes[3],
            selection_bytes[0],
            selection_bytes[1],
            selection_bytes[2],
            selection_bytes[3],
            target_bytes[0],
            target_bytes[1],
            target_bytes[2],
            target_bytes[3],
            property_bytes[0],
            property_bytes[1],
            property_bytes[2],
            property_bytes[3],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
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
        if header.major_opcode != CONVERT_SELECTION_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (requestor, remaining) = Window::try_parse(value)?;
        let (selection, remaining) = Atom::try_parse(remaining)?;
        let (target, remaining) = Atom::try_parse(remaining)?;
        let (property, remaining) = Atom::try_parse(remaining)?;
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let _ = remaining;
        Ok(ConvertSelectionRequest {
            requestor,
            selection,
            target,
            property,
            time,
        })
    }
}
impl Request for ConvertSelectionRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for ConvertSelectionRequest {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SendEventDest(bool);
impl SendEventDest {
    pub const POINTER_WINDOW: Self = Self(false);
    pub const ITEM_FOCUS: Self = Self(true);
}
impl From<SendEventDest> for bool {
    #[inline]
    fn from(input: SendEventDest) -> Self {
        input.0
    }
}
impl From<SendEventDest> for Option<bool> {
    #[inline]
    fn from(input: SendEventDest) -> Self {
        Some(input.0)
    }
}
impl From<SendEventDest> for u8 {
    #[inline]
    fn from(input: SendEventDest) -> Self {
        u8::from(input.0)
    }
}
impl From<SendEventDest> for Option<u8> {
    #[inline]
    fn from(input: SendEventDest) -> Self {
        Some(u8::from(input.0))
    }
}
impl From<SendEventDest> for u16 {
    #[inline]
    fn from(input: SendEventDest) -> Self {
        u16::from(input.0)
    }
}
impl From<SendEventDest> for Option<u16> {
    #[inline]
    fn from(input: SendEventDest) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SendEventDest> for u32 {
    #[inline]
    fn from(input: SendEventDest) -> Self {
        u32::from(input.0)
    }
}
impl From<SendEventDest> for Option<u32> {
    #[inline]
    fn from(input: SendEventDest) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<bool> for SendEventDest {
    #[inline]
    fn from(value: bool) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SendEventDest  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::POINTER_WINDOW.0.into(), "POINTER_WINDOW", "PointerWindow"),
            (Self::ITEM_FOCUS.0.into(), "ITEM_FOCUS", "ItemFocus"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the SendEvent request
pub const SEND_EVENT_REQUEST: u8 = 25;
/// send an event.
///
/// Identifies the `destination` window, determines which clients should receive
/// the specified event and ignores any active grabs.
///
/// The `event` must be one of the core events or an event defined by an extension,
/// so that the X server can correctly byte-swap the contents as necessary. The
/// contents of `event` are otherwise unaltered and unchecked except for the
/// `send_event` field which is forced to 'true'.
///
/// # Fields
///
/// * `destination` - The window to send this event to. Every client which selects any event within
/// `event_mask` on `destination` will get the event.
///
/// The special value `XCB_SEND_EVENT_DEST_POINTER_WINDOW` refers to the window
/// that contains the mouse pointer.
///
/// The special value `XCB_SEND_EVENT_DEST_ITEM_FOCUS` refers to the window which
/// has the keyboard focus.
/// * `event_mask` - Event_mask for determining which clients should receive the specified event.
/// See `destination` and `propagate`.
/// * `propagate` - If `propagate` is true and no clients have selected any event on `destination`,
/// the destination is replaced with the closest ancestor of `destination` for
/// which some client has selected a type in `event_mask` and for which no
/// intervening window has that type in its do-not-propagate-mask. If no such
/// window exists or if the window is an ancestor of the focus window and
/// `InputFocus` was originally specified as the destination, the event is not sent
/// to any clients. Otherwise, the event is reported to every client selecting on
/// the final destination any of the types specified in `event_mask`.
/// * `event` - The event to send to the specified `destination`.
///
/// # Errors
///
/// * `Window` - The specified `destination` window does not exist.
/// * `Value` - The given `event` is neither a core event nor an event defined by an extension.
///
/// # See
///
/// * `ConfigureNotify`: event
///
/// # Example
///
/// ```text
/// /*
///  * Tell the given window that it was configured to a size of 800x600 pixels.
///  *
///  */
/// void my_example(xcb_connection_t *conn, xcb_window_t window) {
///     /* Every X11 event is 32 bytes long. Therefore, XCB will copy 32 bytes.
///      * In order to properly initialize these bytes, we allocate 32 bytes even
///      * though we only need less for an xcb_configure_notify_event_t */
///     xcb_configure_notify_event_t *event = calloc(32, 1);
///
///     event->event = window;
///     event->window = window;
///     event->response_type = XCB_CONFIGURE_NOTIFY;
///
///     event->x = 0;
///     event->y = 0;
///     event->width = 800;
///     event->height = 600;
///
///     event->border_width = 0;
///     event->above_sibling = XCB_NONE;
///     event->override_redirect = false;
///
///     xcb_send_event(conn, false, window, XCB_EVENT_MASK_STRUCTURE_NOTIFY,
///                    (char*)event);
///     xcb_flush(conn);
///     free(event);
/// }
/// ```
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SendEventRequest<'input> {
    pub propagate: bool,
    pub destination: Window,
    pub event_mask: EventMask,
    pub event: Cow<'input, [u8; 32]>,
}
impl_debug_if_no_extra_traits!(SendEventRequest<'_>, "SendEventRequest");
impl<'input> SendEventRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 2]> {
        let length_so_far = 0;
        let propagate_bytes = self.propagate.serialize();
        let destination_bytes = self.destination.serialize();
        let event_mask_bytes = u32::from(self.event_mask).serialize();
        let mut request0 = vec![
            SEND_EVENT_REQUEST,
            propagate_bytes[0],
            0,
            0,
            destination_bytes[0],
            destination_bytes[1],
            destination_bytes[2],
            destination_bytes[3],
            event_mask_bytes[0],
            event_mask_bytes[1],
            event_mask_bytes[2],
            event_mask_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.event.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), Cow::Owned(self.event.to_vec())], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != SEND_EVENT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (propagate, remaining) = bool::try_parse(remaining)?;
        let _ = remaining;
        let (destination, remaining) = Window::try_parse(value)?;
        let (event_mask, remaining) = u32::try_parse(remaining)?;
        let event_mask = event_mask.into();
        let (event, remaining) = crate::x11_utils::parse_u8_array_ref::<32>(remaining)?;
        let _ = remaining;
        Ok(SendEventRequest {
            propagate,
            destination,
            event_mask,
            event: Cow::Borrowed(event),
        })
    }
    /// Clone all borrowed data in this SendEventRequest.
    pub fn into_owned(self) -> SendEventRequest<'static> {
        SendEventRequest {
            propagate: self.propagate,
            destination: self.destination,
            event_mask: self.event_mask,
            event: Cow::Owned(self.event.into_owned()),
        }
    }
}
impl<'input> Request for SendEventRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SendEventRequest<'input> {
}

/// # Fields
///
/// * `Sync` - The state of the keyboard appears to freeze: No further keyboard events are
/// generated by the server until the grabbing client issues a releasing
/// `AllowEvents` request or until the keyboard grab is released.
/// * `Async` - Keyboard event processing continues normally.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrabMode(u8);
impl GrabMode {
    pub const SYNC: Self = Self(0);
    pub const ASYNC: Self = Self(1);
}
impl From<GrabMode> for u8 {
    #[inline]
    fn from(input: GrabMode) -> Self {
        input.0
    }
}
impl From<GrabMode> for Option<u8> {
    #[inline]
    fn from(input: GrabMode) -> Self {
        Some(input.0)
    }
}
impl From<GrabMode> for u16 {
    #[inline]
    fn from(input: GrabMode) -> Self {
        u16::from(input.0)
    }
}
impl From<GrabMode> for Option<u16> {
    #[inline]
    fn from(input: GrabMode) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<GrabMode> for u32 {
    #[inline]
    fn from(input: GrabMode) -> Self {
        u32::from(input.0)
    }
}
impl From<GrabMode> for Option<u32> {
    #[inline]
    fn from(input: GrabMode) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for GrabMode {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for GrabMode  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SYNC.0.into(), "SYNC", "Sync"),
            (Self::ASYNC.0.into(), "ASYNC", "Async"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrabStatus(u8);
impl GrabStatus {
    pub const SUCCESS: Self = Self(0);
    pub const ALREADY_GRABBED: Self = Self(1);
    pub const INVALID_TIME: Self = Self(2);
    pub const NOT_VIEWABLE: Self = Self(3);
    pub const FROZEN: Self = Self(4);
}
impl From<GrabStatus> for u8 {
    #[inline]
    fn from(input: GrabStatus) -> Self {
        input.0
    }
}
impl From<GrabStatus> for Option<u8> {
    #[inline]
    fn from(input: GrabStatus) -> Self {
        Some(input.0)
    }
}
impl From<GrabStatus> for u16 {
    #[inline]
    fn from(input: GrabStatus) -> Self {
        u16::from(input.0)
    }
}
impl From<GrabStatus> for Option<u16> {
    #[inline]
    fn from(input: GrabStatus) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<GrabStatus> for u32 {
    #[inline]
    fn from(input: GrabStatus) -> Self {
        u32::from(input.0)
    }
}
impl From<GrabStatus> for Option<u32> {
    #[inline]
    fn from(input: GrabStatus) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for GrabStatus {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for GrabStatus  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SUCCESS.0.into(), "SUCCESS", "Success"),
            (Self::ALREADY_GRABBED.0.into(), "ALREADY_GRABBED", "AlreadyGrabbed"),
            (Self::INVALID_TIME.0.into(), "INVALID_TIME", "InvalidTime"),
            (Self::NOT_VIEWABLE.0.into(), "NOT_VIEWABLE", "NotViewable"),
            (Self::FROZEN.0.into(), "FROZEN", "Frozen"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CursorEnum(u8);
impl CursorEnum {
    pub const NONE: Self = Self(0);
}
impl From<CursorEnum> for u8 {
    #[inline]
    fn from(input: CursorEnum) -> Self {
        input.0
    }
}
impl From<CursorEnum> for Option<u8> {
    #[inline]
    fn from(input: CursorEnum) -> Self {
        Some(input.0)
    }
}
impl From<CursorEnum> for u16 {
    #[inline]
    fn from(input: CursorEnum) -> Self {
        u16::from(input.0)
    }
}
impl From<CursorEnum> for Option<u16> {
    #[inline]
    fn from(input: CursorEnum) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<CursorEnum> for u32 {
    #[inline]
    fn from(input: CursorEnum) -> Self {
        u32::from(input.0)
    }
}
impl From<CursorEnum> for Option<u32> {
    #[inline]
    fn from(input: CursorEnum) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for CursorEnum {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for CursorEnum  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NONE.0.into(), "NONE", "None"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the GrabPointer request
pub const GRAB_POINTER_REQUEST: u8 = 26;
/// Grab the pointer.
///
/// Actively grabs control of the pointer. Further pointer events are reported only to the grabbing client. Overrides any active pointer grab by this client.
///
/// # Fields
///
/// * `event_mask` - Specifies which pointer events are reported to the client.
///
/// TODO: which values?
/// * `confine_to` - Specifies the window to confine the pointer in (the user will not be able to
/// move the pointer out of that window).
///
/// The special value `XCB_NONE` means don't confine the pointer.
/// * `cursor` - Specifies the cursor that should be displayed or `XCB_NONE` to not change the
/// cursor.
/// * `owner_events` - If 1, the `grab_window` will still get the pointer events. If 0, events are not
/// reported to the `grab_window`.
/// * `grab_window` - Specifies the window on which the pointer should be grabbed.
/// * `time` - The time argument allows you to avoid certain circumstances that come up if
/// applications take a long time to respond or if there are long network delays.
/// Consider a situation where you have two applications, both of which normally
/// grab the pointer when clicked on. If both applications specify the timestamp
/// from the event, the second application may wake up faster and successfully grab
/// the pointer before the first application. The first application then will get
/// an indication that the other application grabbed the pointer before its request
/// was processed.
///
/// The special value `XCB_CURRENT_TIME` will be replaced with the current server
/// time.
/// * `pointer_mode` -
/// * `keyboard_mode` -
///
/// # Errors
///
/// * `Value` - TODO: reasons?
/// * `Window` - The specified `window` does not exist.
///
/// # See
///
/// * `GrabKeyboard`: request
///
/// # Example
///
/// ```text
/// /*
///  * Grabs the pointer actively
///  *
///  */
/// void my_example(xcb_connection_t *conn, xcb_screen_t *screen, xcb_cursor_t cursor) {
///     xcb_grab_pointer_cookie_t cookie;
///     xcb_grab_pointer_reply_t *reply;
///
///     cookie = xcb_grab_pointer(
///         conn,
///         false,               /* get all pointer events specified by the following mask */
///         screen->root,        /* grab the root window */
///         XCB_NONE,            /* which events to let through */
///         XCB_GRAB_MODE_ASYNC, /* pointer events should continue as normal */
///         XCB_GRAB_MODE_ASYNC, /* keyboard mode */
///         XCB_NONE,            /* confine_to = in which window should the cursor stay */
///         cursor,              /* we change the cursor to whatever the user wanted */
///         XCB_CURRENT_TIME
///     );
///
///     if ((reply = xcb_grab_pointer_reply(conn, cookie, NULL))) {
///         if (reply->status == XCB_GRAB_STATUS_SUCCESS)
///             printf("successfully grabbed the pointer\\n");
///         free(reply);
///     }
/// }
/// ```
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrabPointerRequest {
    pub owner_events: bool,
    pub grab_window: Window,
    pub event_mask: EventMask,
    pub pointer_mode: GrabMode,
    pub keyboard_mode: GrabMode,
    pub confine_to: Window,
    pub cursor: Cursor,
    pub time: Timestamp,
}
impl_debug_if_no_extra_traits!(GrabPointerRequest, "GrabPointerRequest");
impl GrabPointerRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let owner_events_bytes = self.owner_events.serialize();
        let grab_window_bytes = self.grab_window.serialize();
        let event_mask_bytes = (u32::from(self.event_mask) as u16).serialize();
        let pointer_mode_bytes = u8::from(self.pointer_mode).serialize();
        let keyboard_mode_bytes = u8::from(self.keyboard_mode).serialize();
        let confine_to_bytes = self.confine_to.serialize();
        let cursor_bytes = self.cursor.serialize();
        let time_bytes = self.time.serialize();
        let mut request0 = vec![
            GRAB_POINTER_REQUEST,
            owner_events_bytes[0],
            0,
            0,
            grab_window_bytes[0],
            grab_window_bytes[1],
            grab_window_bytes[2],
            grab_window_bytes[3],
            event_mask_bytes[0],
            event_mask_bytes[1],
            pointer_mode_bytes[0],
            keyboard_mode_bytes[0],
            confine_to_bytes[0],
            confine_to_bytes[1],
            confine_to_bytes[2],
            confine_to_bytes[3],
            cursor_bytes[0],
            cursor_bytes[1],
            cursor_bytes[2],
            cursor_bytes[3],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
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
        if header.major_opcode != GRAB_POINTER_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (owner_events, remaining) = bool::try_parse(remaining)?;
        let _ = remaining;
        let (grab_window, remaining) = Window::try_parse(value)?;
        let (event_mask, remaining) = u16::try_parse(remaining)?;
        let event_mask = event_mask.into();
        let (pointer_mode, remaining) = u8::try_parse(remaining)?;
        let pointer_mode = pointer_mode.into();
        let (keyboard_mode, remaining) = u8::try_parse(remaining)?;
        let keyboard_mode = keyboard_mode.into();
        let (confine_to, remaining) = Window::try_parse(remaining)?;
        let (cursor, remaining) = Cursor::try_parse(remaining)?;
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let _ = remaining;
        Ok(GrabPointerRequest {
            owner_events,
            grab_window,
            event_mask,
            pointer_mode,
            keyboard_mode,
            confine_to,
            cursor,
            time,
        })
    }
}
impl Request for GrabPointerRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GrabPointerRequest {
    type Reply = GrabPointerReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrabPointerReply {
    pub status: GrabStatus,
    pub sequence: u16,
    pub length: u32,
}
impl_debug_if_no_extra_traits!(GrabPointerReply, "GrabPointerReply");
impl TryParse for GrabPointerReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (status, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let status = status.into();
        let result = GrabPointerReply { status, sequence, length };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GrabPointerReply {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let response_type_bytes = &[1];
        let status_bytes = u8::from(self.status).serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        [
            response_type_bytes[0],
            status_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        u8::from(self.status).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
    }
}

/// Opcode for the UngrabPointer request
pub const UNGRAB_POINTER_REQUEST: u8 = 27;
/// release the pointer.
///
/// Releases the pointer and any queued events if you actively grabbed the pointer
/// before using `xcb_grab_pointer`, `xcb_grab_button` or within a normal button
/// press.
///
/// EnterNotify and LeaveNotify events are generated.
///
/// # Fields
///
/// * `time` - Timestamp to avoid race conditions when running X over the network.
///
/// The pointer will not be released if `time` is earlier than the
/// last-pointer-grab time or later than the current X server time.
/// * `name_len` - Length (in bytes) of `name`.
/// * `name` - A pattern describing an X core font.
///
/// # See
///
/// * `GrabPointer`: request
/// * `GrabButton`: request
/// * `EnterNotify`: event
/// * `LeaveNotify`: event
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UngrabPointerRequest {
    pub time: Timestamp,
}
impl_debug_if_no_extra_traits!(UngrabPointerRequest, "UngrabPointerRequest");
impl UngrabPointerRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let time_bytes = self.time.serialize();
        let mut request0 = vec![
            UNGRAB_POINTER_REQUEST,
            0,
            0,
            0,
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
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
        if header.major_opcode != UNGRAB_POINTER_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (time, remaining) = Timestamp::try_parse(value)?;
        let _ = remaining;
        Ok(UngrabPointerRequest {
            time,
        })
    }
}
impl Request for UngrabPointerRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for UngrabPointerRequest {
}

/// # Fields
///
/// * `Any` - Any of the following (or none):
/// * `1` - The left mouse button.
/// * `2` - The right mouse button.
/// * `3` - The middle mouse button.
/// * `4` - Scroll wheel. TODO: direction?
/// * `5` - Scroll wheel. TODO: direction?
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ButtonIndex(u8);
impl ButtonIndex {
    pub const ANY: Self = Self(0);
    pub const M1: Self = Self(1);
    pub const M2: Self = Self(2);
    pub const M3: Self = Self(3);
    pub const M4: Self = Self(4);
    pub const M5: Self = Self(5);
}
impl From<ButtonIndex> for u8 {
    #[inline]
    fn from(input: ButtonIndex) -> Self {
        input.0
    }
}
impl From<ButtonIndex> for Option<u8> {
    #[inline]
    fn from(input: ButtonIndex) -> Self {
        Some(input.0)
    }
}
impl From<ButtonIndex> for u16 {
    #[inline]
    fn from(input: ButtonIndex) -> Self {
        u16::from(input.0)
    }
}
impl From<ButtonIndex> for Option<u16> {
    #[inline]
    fn from(input: ButtonIndex) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<ButtonIndex> for u32 {
    #[inline]
    fn from(input: ButtonIndex) -> Self {
        u32::from(input.0)
    }
}
impl From<ButtonIndex> for Option<u32> {
    #[inline]
    fn from(input: ButtonIndex) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ButtonIndex {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ButtonIndex  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ANY.0.into(), "ANY", "Any"),
            (Self::M1.0.into(), "M1", "M1"),
            (Self::M2.0.into(), "M2", "M2"),
            (Self::M3.0.into(), "M3", "M3"),
            (Self::M4.0.into(), "M4", "M4"),
            (Self::M5.0.into(), "M5", "M5"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the GrabButton request
pub const GRAB_BUTTON_REQUEST: u8 = 28;
/// Grab pointer button(s).
///
/// This request establishes a passive grab. The pointer is actively grabbed as
/// described in GrabPointer, the last-pointer-grab time is set to the time at
/// which the button was pressed (as transmitted in the ButtonPress event), and the
/// ButtonPress event is reported if all of the following conditions are true:
///
/// The pointer is not grabbed and the specified button is logically pressed when
/// the specified modifier keys are logically down, and no other buttons or
/// modifier keys are logically down.
///
/// The grab-window contains the pointer.
///
/// The confine-to window (if any) is viewable.
///
/// A passive grab on the same button/key combination does not exist on any
/// ancestor of grab-window.
///
/// The interpretation of the remaining arguments is the same as for GrabPointer.
/// The active grab is terminated automatically when the logical state of the
/// pointer has all buttons released, independent of the logical state of modifier
/// keys. Note that the logical state of a device (as seen by means of the
/// protocol) may lag the physical state if device event processing is frozen. This
/// request overrides all previous passive grabs by the same client on the same
/// button/key combinations on the same window. A modifier of AnyModifier is
/// equivalent to issuing the request for all possible modifier combinations
/// (including the combination of no modifiers). It is not required that all
/// specified modifiers have currently assigned keycodes. A button of AnyButton is
/// equivalent to issuing the request for all possible buttons. Otherwise, it is
/// not required that the button specified currently be assigned to a physical
/// button.
///
/// An Access error is generated if some other client has already issued a
/// GrabButton request with the same button/key combination on the same window.
/// When using AnyModifier or AnyButton, the request fails completely (no grabs are
/// established), and an Access error is generated if there is a conflicting grab
/// for any combination. The request has no effect on an active grab.
///
/// # Fields
///
/// * `owner_events` - If 1, the `grab_window` will still get the pointer events. If 0, events are not
/// reported to the `grab_window`.
/// * `grab_window` - Specifies the window on which the pointer should be grabbed.
/// * `event_mask` - Specifies which pointer events are reported to the client.
///
/// TODO: which values?
/// * `confine_to` - Specifies the window to confine the pointer in (the user will not be able to
/// move the pointer out of that window).
///
/// The special value `XCB_NONE` means don't confine the pointer.
/// * `cursor` - Specifies the cursor that should be displayed or `XCB_NONE` to not change the
/// cursor.
/// * `modifiers` - The modifiers to grab.
///
/// Using the special value `XCB_MOD_MASK_ANY` means grab the pointer with all
/// possible modifier combinations.
/// * `pointer_mode` -
/// * `keyboard_mode` -
/// * `button` -
///
/// # Errors
///
/// * `Access` - Another client has already issued a GrabButton with the same button/key
/// combination on the same window.
/// * `Value` - TODO: reasons?
/// * `Cursor` - The specified `cursor` does not exist.
/// * `Window` - The specified `window` does not exist.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrabButtonRequest {
    pub owner_events: bool,
    pub grab_window: Window,
    pub event_mask: EventMask,
    pub pointer_mode: GrabMode,
    pub keyboard_mode: GrabMode,
    pub confine_to: Window,
    pub cursor: Cursor,
    pub button: ButtonIndex,
    pub modifiers: ModMask,
}
impl_debug_if_no_extra_traits!(GrabButtonRequest, "GrabButtonRequest");
impl GrabButtonRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let owner_events_bytes = self.owner_events.serialize();
        let grab_window_bytes = self.grab_window.serialize();
        let event_mask_bytes = (u32::from(self.event_mask) as u16).serialize();
        let pointer_mode_bytes = u8::from(self.pointer_mode).serialize();
        let keyboard_mode_bytes = u8::from(self.keyboard_mode).serialize();
        let confine_to_bytes = self.confine_to.serialize();
        let cursor_bytes = self.cursor.serialize();
        let button_bytes = u8::from(self.button).serialize();
        let modifiers_bytes = u16::from(self.modifiers).serialize();
        let mut request0 = vec![
            GRAB_BUTTON_REQUEST,
            owner_events_bytes[0],
            0,
            0,
            grab_window_bytes[0],
            grab_window_bytes[1],
            grab_window_bytes[2],
            grab_window_bytes[3],
            event_mask_bytes[0],
            event_mask_bytes[1],
            pointer_mode_bytes[0],
            keyboard_mode_bytes[0],
            confine_to_bytes[0],
            confine_to_bytes[1],
            confine_to_bytes[2],
            confine_to_bytes[3],
            cursor_bytes[0],
            cursor_bytes[1],
            cursor_bytes[2],
            cursor_bytes[3],
            button_bytes[0],
            0,
            modifiers_bytes[0],
            modifiers_bytes[1],
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
        if header.major_opcode != GRAB_BUTTON_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (owner_events, remaining) = bool::try_parse(remaining)?;
        let _ = remaining;
        let (grab_window, remaining) = Window::try_parse(value)?;
        let (event_mask, remaining) = u16::try_parse(remaining)?;
        let event_mask = event_mask.into();
        let (pointer_mode, remaining) = u8::try_parse(remaining)?;
        let pointer_mode = pointer_mode.into();
        let (keyboard_mode, remaining) = u8::try_parse(remaining)?;
        let keyboard_mode = keyboard_mode.into();
        let (confine_to, remaining) = Window::try_parse(remaining)?;
        let (cursor, remaining) = Cursor::try_parse(remaining)?;
        let (button, remaining) = u8::try_parse(remaining)?;
        let button = button.into();
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (modifiers, remaining) = u16::try_parse(remaining)?;
        let modifiers = modifiers.into();
        let _ = remaining;
        Ok(GrabButtonRequest {
            owner_events,
            grab_window,
            event_mask,
            pointer_mode,
            keyboard_mode,
            confine_to,
            cursor,
            button,
            modifiers,
        })
    }
}
impl Request for GrabButtonRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for GrabButtonRequest {
}

/// Opcode for the UngrabButton request
pub const UNGRAB_BUTTON_REQUEST: u8 = 29;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UngrabButtonRequest {
    pub button: ButtonIndex,
    pub grab_window: Window,
    pub modifiers: ModMask,
}
impl_debug_if_no_extra_traits!(UngrabButtonRequest, "UngrabButtonRequest");
impl UngrabButtonRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let button_bytes = u8::from(self.button).serialize();
        let grab_window_bytes = self.grab_window.serialize();
        let modifiers_bytes = u16::from(self.modifiers).serialize();
        let mut request0 = vec![
            UNGRAB_BUTTON_REQUEST,
            button_bytes[0],
            0,
            0,
            grab_window_bytes[0],
            grab_window_bytes[1],
            grab_window_bytes[2],
            grab_window_bytes[3],
            modifiers_bytes[0],
            modifiers_bytes[1],
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
        if header.major_opcode != UNGRAB_BUTTON_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (button, remaining) = u8::try_parse(remaining)?;
        let button = button.into();
        let _ = remaining;
        let (grab_window, remaining) = Window::try_parse(value)?;
        let (modifiers, remaining) = u16::try_parse(remaining)?;
        let modifiers = modifiers.into();
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(UngrabButtonRequest {
            button,
            grab_window,
            modifiers,
        })
    }
}
impl Request for UngrabButtonRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for UngrabButtonRequest {
}

/// Opcode for the ChangeActivePointerGrab request
pub const CHANGE_ACTIVE_POINTER_GRAB_REQUEST: u8 = 30;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeActivePointerGrabRequest {
    pub cursor: Cursor,
    pub time: Timestamp,
    pub event_mask: EventMask,
}
impl_debug_if_no_extra_traits!(ChangeActivePointerGrabRequest, "ChangeActivePointerGrabRequest");
impl ChangeActivePointerGrabRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let cursor_bytes = self.cursor.serialize();
        let time_bytes = self.time.serialize();
        let event_mask_bytes = (u32::from(self.event_mask) as u16).serialize();
        let mut request0 = vec![
            CHANGE_ACTIVE_POINTER_GRAB_REQUEST,
            0,
            0,
            0,
            cursor_bytes[0],
            cursor_bytes[1],
            cursor_bytes[2],
            cursor_bytes[3],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            event_mask_bytes[0],
            event_mask_bytes[1],
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
        if header.major_opcode != CHANGE_ACTIVE_POINTER_GRAB_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cursor, remaining) = Cursor::try_parse(value)?;
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let (event_mask, remaining) = u16::try_parse(remaining)?;
        let event_mask = event_mask.into();
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(ChangeActivePointerGrabRequest {
            cursor,
            time,
            event_mask,
        })
    }
}
impl Request for ChangeActivePointerGrabRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for ChangeActivePointerGrabRequest {
}

/// Opcode for the GrabKeyboard request
pub const GRAB_KEYBOARD_REQUEST: u8 = 31;
/// Grab the keyboard.
///
/// Actively grabs control of the keyboard and generates FocusIn and FocusOut
/// events. Further key events are reported only to the grabbing client.
///
/// Any active keyboard grab by this client is overridden. If the keyboard is
/// actively grabbed by some other client, `AlreadyGrabbed` is returned. If
/// `grab_window` is not viewable, `GrabNotViewable` is returned. If the keyboard
/// is frozen by an active grab of another client, `GrabFrozen` is returned. If the
/// specified `time` is earlier than the last-keyboard-grab time or later than the
/// current X server time, `GrabInvalidTime` is returned. Otherwise, the
/// last-keyboard-grab time is set to the specified time.
///
/// # Fields
///
/// * `owner_events` - If 1, the `grab_window` will still get the pointer events. If 0, events are not
/// reported to the `grab_window`.
/// * `grab_window` - Specifies the window on which the pointer should be grabbed.
/// * `time` - Timestamp to avoid race conditions when running X over the network.
///
/// The special value `XCB_CURRENT_TIME` will be replaced with the current server
/// time.
/// * `pointer_mode` -
/// * `keyboard_mode` -
///
/// # Errors
///
/// * `Value` - TODO: reasons?
/// * `Window` - The specified `window` does not exist.
///
/// # See
///
/// * `GrabPointer`: request
///
/// # Example
///
/// ```text
/// /*
///  * Grabs the keyboard actively
///  *
///  */
/// void my_example(xcb_connection_t *conn, xcb_screen_t *screen) {
///     xcb_grab_keyboard_cookie_t cookie;
///     xcb_grab_keyboard_reply_t *reply;
///
///     cookie = xcb_grab_keyboard(
///         conn,
///         true,                /* report events */
///         screen->root,        /* grab the root window */
///         XCB_CURRENT_TIME,
///         XCB_GRAB_MODE_ASYNC, /* process events as normal, do not require sync */
///         XCB_GRAB_MODE_ASYNC
///     );
///
///     if ((reply = xcb_grab_keyboard_reply(conn, cookie, NULL))) {
///         if (reply->status == XCB_GRAB_STATUS_SUCCESS)
///             printf("successfully grabbed the keyboard\\n");
///
///         free(reply);
///     }
/// }
/// ```
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrabKeyboardRequest {
    pub owner_events: bool,
    pub grab_window: Window,
    pub time: Timestamp,
    pub pointer_mode: GrabMode,
    pub keyboard_mode: GrabMode,
}
impl_debug_if_no_extra_traits!(GrabKeyboardRequest, "GrabKeyboardRequest");
impl GrabKeyboardRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let owner_events_bytes = self.owner_events.serialize();
        let grab_window_bytes = self.grab_window.serialize();
        let time_bytes = self.time.serialize();
        let pointer_mode_bytes = u8::from(self.pointer_mode).serialize();
        let keyboard_mode_bytes = u8::from(self.keyboard_mode).serialize();
        let mut request0 = vec![
            GRAB_KEYBOARD_REQUEST,
            owner_events_bytes[0],
            0,
            0,
            grab_window_bytes[0],
            grab_window_bytes[1],
            grab_window_bytes[2],
            grab_window_bytes[3],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            pointer_mode_bytes[0],
            keyboard_mode_bytes[0],
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
        if header.major_opcode != GRAB_KEYBOARD_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (owner_events, remaining) = bool::try_parse(remaining)?;
        let _ = remaining;
        let (grab_window, remaining) = Window::try_parse(value)?;
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let (pointer_mode, remaining) = u8::try_parse(remaining)?;
        let pointer_mode = pointer_mode.into();
        let (keyboard_mode, remaining) = u8::try_parse(remaining)?;
        let keyboard_mode = keyboard_mode.into();
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(GrabKeyboardRequest {
            owner_events,
            grab_window,
            time,
            pointer_mode,
            keyboard_mode,
        })
    }
}
impl Request for GrabKeyboardRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GrabKeyboardRequest {
    type Reply = GrabKeyboardReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrabKeyboardReply {
    pub status: GrabStatus,
    pub sequence: u16,
    pub length: u32,
}
impl_debug_if_no_extra_traits!(GrabKeyboardReply, "GrabKeyboardReply");
impl TryParse for GrabKeyboardReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (status, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let status = status.into();
        let result = GrabKeyboardReply { status, sequence, length };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GrabKeyboardReply {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let response_type_bytes = &[1];
        let status_bytes = u8::from(self.status).serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        [
            response_type_bytes[0],
            status_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        u8::from(self.status).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
    }
}

/// Opcode for the UngrabKeyboard request
pub const UNGRAB_KEYBOARD_REQUEST: u8 = 32;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UngrabKeyboardRequest {
    pub time: Timestamp,
}
impl_debug_if_no_extra_traits!(UngrabKeyboardRequest, "UngrabKeyboardRequest");
impl UngrabKeyboardRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let time_bytes = self.time.serialize();
        let mut request0 = vec![
            UNGRAB_KEYBOARD_REQUEST,
            0,
            0,
            0,
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
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
        if header.major_opcode != UNGRAB_KEYBOARD_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (time, remaining) = Timestamp::try_parse(value)?;
        let _ = remaining;
        Ok(UngrabKeyboardRequest {
            time,
        })
    }
}
impl Request for UngrabKeyboardRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for UngrabKeyboardRequest {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Grab(u8);
impl Grab {
    pub const ANY: Self = Self(0);
}
impl From<Grab> for u8 {
    #[inline]
    fn from(input: Grab) -> Self {
        input.0
    }
}
impl From<Grab> for Option<u8> {
    #[inline]
    fn from(input: Grab) -> Self {
        Some(input.0)
    }
}
impl From<Grab> for u16 {
    #[inline]
    fn from(input: Grab) -> Self {
        u16::from(input.0)
    }
}
impl From<Grab> for Option<u16> {
    #[inline]
    fn from(input: Grab) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Grab> for u32 {
    #[inline]
    fn from(input: Grab) -> Self {
        u32::from(input.0)
    }
}
impl From<Grab> for Option<u32> {
    #[inline]
    fn from(input: Grab) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Grab {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Grab  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ANY.0.into(), "ANY", "Any"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the GrabKey request
pub const GRAB_KEY_REQUEST: u8 = 33;
/// Grab keyboard key(s).
///
/// Establishes a passive grab on the keyboard. In the future, the keyboard is
/// actively grabbed (as for `GrabKeyboard`), the last-keyboard-grab time is set to
/// the time at which the key was pressed (as transmitted in the KeyPress event),
/// and the KeyPress event is reported if all of the following conditions are true:
///
/// The keyboard is not grabbed and the specified key (which can itself be a
/// modifier key) is logically pressed when the specified modifier keys are
/// logically down, and no other modifier keys are logically down.
///
/// Either the grab_window is an ancestor of (or is) the focus window, or the
/// grab_window is a descendant of the focus window and contains the pointer.
///
/// A passive grab on the same key combination does not exist on any ancestor of
/// grab_window.
///
/// The interpretation of the remaining arguments is as for XGrabKeyboard.  The active grab is terminated
/// automatically when the logical state of the keyboard has the specified key released (independent of the
/// logical state of the modifier keys), at which point a KeyRelease event is reported to the grabbing window.
///
/// Note that the logical state of a device (as seen by client applications) may lag the physical state if
/// device event processing is frozen.
///
/// A modifiers argument of AnyModifier is equivalent to issuing the request for all possible modifier combinations (including the combination of no modifiers).  It is not required that all modifiers specified
/// have currently assigned KeyCodes.  A keycode argument of AnyKey is equivalent to issuing the request for
/// all possible KeyCodes.  Otherwise, the specified keycode must be in the range specified by min_keycode
/// and max_keycode in the connection setup, or a BadValue error results.
///
/// If some other client has issued a XGrabKey with the same key combination on the same window, a BadAccess
/// error results.  When using AnyModifier or AnyKey, the request fails completely, and a BadAccess error
/// results (no grabs are established) if there is a conflicting grab for any combination.
///
/// # Fields
///
/// * `owner_events` - If 1, the `grab_window` will still get the key events. If 0, events are not
/// reported to the `grab_window`.
/// * `grab_window` - Specifies the window on which the key should be grabbed.
/// * `key` - The keycode of the key to grab.
///
/// The special value `XCB_GRAB_ANY` means grab any key.
/// * `modifiers` - The modifiers to grab.
///
/// Using the special value `XCB_MOD_MASK_ANY` means grab the key with all
/// possible modifier combinations.
/// * `pointer_mode` -
/// * `keyboard_mode` -
///
/// # Errors
///
/// * `Access` - Another client has already issued a GrabKey with the same button/key
/// combination on the same window.
/// * `Value` - The key is not `XCB_GRAB_ANY` and not in the range specified by `min_keycode`
/// and `max_keycode` in the connection setup.
/// * `Window` - The specified `window` does not exist.
///
/// # See
///
/// * `GrabKeyboard`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrabKeyRequest {
    pub owner_events: bool,
    pub grab_window: Window,
    pub modifiers: ModMask,
    pub key: Keycode,
    pub pointer_mode: GrabMode,
    pub keyboard_mode: GrabMode,
}
impl_debug_if_no_extra_traits!(GrabKeyRequest, "GrabKeyRequest");
impl GrabKeyRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let owner_events_bytes = self.owner_events.serialize();
        let grab_window_bytes = self.grab_window.serialize();
        let modifiers_bytes = u16::from(self.modifiers).serialize();
        let key_bytes = self.key.serialize();
        let pointer_mode_bytes = u8::from(self.pointer_mode).serialize();
        let keyboard_mode_bytes = u8::from(self.keyboard_mode).serialize();
        let mut request0 = vec![
            GRAB_KEY_REQUEST,
            owner_events_bytes[0],
            0,
            0,
            grab_window_bytes[0],
            grab_window_bytes[1],
            grab_window_bytes[2],
            grab_window_bytes[3],
            modifiers_bytes[0],
            modifiers_bytes[1],
            key_bytes[0],
            pointer_mode_bytes[0],
            keyboard_mode_bytes[0],
            0,
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
        if header.major_opcode != GRAB_KEY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (owner_events, remaining) = bool::try_parse(remaining)?;
        let _ = remaining;
        let (grab_window, remaining) = Window::try_parse(value)?;
        let (modifiers, remaining) = u16::try_parse(remaining)?;
        let modifiers = modifiers.into();
        let (key, remaining) = Keycode::try_parse(remaining)?;
        let (pointer_mode, remaining) = u8::try_parse(remaining)?;
        let pointer_mode = pointer_mode.into();
        let (keyboard_mode, remaining) = u8::try_parse(remaining)?;
        let keyboard_mode = keyboard_mode.into();
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(GrabKeyRequest {
            owner_events,
            grab_window,
            modifiers,
            key,
            pointer_mode,
            keyboard_mode,
        })
    }
}
impl Request for GrabKeyRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for GrabKeyRequest {
}

/// Opcode for the UngrabKey request
pub const UNGRAB_KEY_REQUEST: u8 = 34;
/// release a key combination.
///
/// Releases the key combination on `grab_window` if you grabbed it using
/// `xcb_grab_key` before.
///
/// # Fields
///
/// * `key` - The keycode of the specified key combination.
///
/// Using the special value `XCB_GRAB_ANY` means releasing all possible key codes.
/// * `grab_window` - The window on which the grabbed key combination will be released.
/// * `modifiers` - The modifiers of the specified key combination.
///
/// Using the special value `XCB_MOD_MASK_ANY` means releasing the key combination
/// with every possible modifier combination.
///
/// # Errors
///
/// * `Window` - The specified `grab_window` does not exist.
/// * `Value` - TODO: reasons?
///
/// # See
///
/// * `GrabKey`: request
/// * `xev`: program
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UngrabKeyRequest {
    pub key: Keycode,
    pub grab_window: Window,
    pub modifiers: ModMask,
}
impl_debug_if_no_extra_traits!(UngrabKeyRequest, "UngrabKeyRequest");
impl UngrabKeyRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let key_bytes = self.key.serialize();
        let grab_window_bytes = self.grab_window.serialize();
        let modifiers_bytes = u16::from(self.modifiers).serialize();
        let mut request0 = vec![
            UNGRAB_KEY_REQUEST,
            key_bytes[0],
            0,
            0,
            grab_window_bytes[0],
            grab_window_bytes[1],
            grab_window_bytes[2],
            grab_window_bytes[3],
            modifiers_bytes[0],
            modifiers_bytes[1],
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
        if header.major_opcode != UNGRAB_KEY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (key, remaining) = Keycode::try_parse(remaining)?;
        let _ = remaining;
        let (grab_window, remaining) = Window::try_parse(value)?;
        let (modifiers, remaining) = u16::try_parse(remaining)?;
        let modifiers = modifiers.into();
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(UngrabKeyRequest {
            key,
            grab_window,
            modifiers,
        })
    }
}
impl Request for UngrabKeyRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for UngrabKeyRequest {
}

/// # Fields
///
/// * `AsyncPointer` - For AsyncPointer, if the pointer is frozen by the client, pointer event
/// processing continues normally. If the pointer is frozen twice by the client on
/// behalf of two separate grabs, AsyncPointer thaws for both. AsyncPointer has no
/// effect if the pointer is not frozen by the client, but the pointer need not be
/// grabbed by the client.
///
/// TODO: rewrite this in more understandable terms.
/// * `SyncPointer` - For SyncPointer, if the pointer is frozen and actively grabbed by the client,
/// pointer event processing continues normally until the next ButtonPress or
/// ButtonRelease event is reported to the client, at which time the pointer again
/// appears to freeze. However, if the reported event causes the pointer grab to be
/// released, then the pointer does not freeze. SyncPointer has no effect if the
/// pointer is not frozen by the client or if the pointer is not grabbed by the
/// client.
/// * `ReplayPointer` - For ReplayPointer, if the pointer is actively grabbed by the client and is
/// frozen as the result of an event having been sent to the client (either from
/// the activation of a GrabButton or from a previous AllowEvents with mode
/// SyncPointer but not from a GrabPointer), then the pointer grab is released and
/// that event is completely reprocessed, this time ignoring any passive grabs at
/// or above (towards the root) the grab-window of the grab just released. The
/// request has no effect if the pointer is not grabbed by the client or if the
/// pointer is not frozen as the result of an event.
/// * `AsyncKeyboard` - For AsyncKeyboard, if the keyboard is frozen by the client, keyboard event
/// processing continues normally. If the keyboard is frozen twice by the client on
/// behalf of two separate grabs, AsyncKeyboard thaws for both. AsyncKeyboard has
/// no effect if the keyboard is not frozen by the client, but the keyboard need
/// not be grabbed by the client.
/// * `SyncKeyboard` - For SyncKeyboard, if the keyboard is frozen and actively grabbed by the client,
/// keyboard event processing continues normally until the next KeyPress or
/// KeyRelease event is reported to the client, at which time the keyboard again
/// appears to freeze. However, if the reported event causes the keyboard grab to
/// be released, then the keyboard does not freeze. SyncKeyboard has no effect if
/// the keyboard is not frozen by the client or if the keyboard is not grabbed by
/// the client.
/// * `ReplayKeyboard` - For ReplayKeyboard, if the keyboard is actively grabbed by the client and is
/// frozen as the result of an event having been sent to the client (either from
/// the activation of a GrabKey or from a previous AllowEvents with mode
/// SyncKeyboard but not from a GrabKeyboard), then the keyboard grab is released
/// and that event is completely reprocessed, this time ignoring any passive grabs
/// at or above (towards the root) the grab-window of the grab just released. The
/// request has no effect if the keyboard is not grabbed by the client or if the
/// keyboard is not frozen as the result of an event.
/// * `SyncBoth` - For SyncBoth, if both pointer and keyboard are frozen by the client, event
/// processing (for both devices) continues normally until the next ButtonPress,
/// ButtonRelease, KeyPress, or KeyRelease event is reported to the client for a
/// grabbed device (button event for the pointer, key event for the keyboard), at
/// which time the devices again appear to freeze. However, if the reported event
/// causes the grab to be released, then the devices do not freeze (but if the
/// other device is still grabbed, then a subsequent event for it will still cause
/// both devices to freeze). SyncBoth has no effect unless both pointer and
/// keyboard are frozen by the client. If the pointer or keyboard is frozen twice
/// by the client on behalf of two separate grabs, SyncBoth thaws for both (but a
/// subsequent freeze for SyncBoth will only freeze each device once).
/// * `AsyncBoth` - For AsyncBoth, if the pointer and the keyboard are frozen by the client, event
/// processing for both devices continues normally. If a device is frozen twice by
/// the client on behalf of two separate grabs, AsyncBoth thaws for both. AsyncBoth
/// has no effect unless both pointer and keyboard are frozen by the client.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Allow(u8);
impl Allow {
    pub const ASYNC_POINTER: Self = Self(0);
    pub const SYNC_POINTER: Self = Self(1);
    pub const REPLAY_POINTER: Self = Self(2);
    pub const ASYNC_KEYBOARD: Self = Self(3);
    pub const SYNC_KEYBOARD: Self = Self(4);
    pub const REPLAY_KEYBOARD: Self = Self(5);
    pub const ASYNC_BOTH: Self = Self(6);
    pub const SYNC_BOTH: Self = Self(7);
}
impl From<Allow> for u8 {
    #[inline]
    fn from(input: Allow) -> Self {
        input.0
    }
}
impl From<Allow> for Option<u8> {
    #[inline]
    fn from(input: Allow) -> Self {
        Some(input.0)
    }
}
impl From<Allow> for u16 {
    #[inline]
    fn from(input: Allow) -> Self {
        u16::from(input.0)
    }
}
impl From<Allow> for Option<u16> {
    #[inline]
    fn from(input: Allow) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Allow> for u32 {
    #[inline]
    fn from(input: Allow) -> Self {
        u32::from(input.0)
    }
}
impl From<Allow> for Option<u32> {
    #[inline]
    fn from(input: Allow) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Allow {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Allow  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ASYNC_POINTER.0.into(), "ASYNC_POINTER", "AsyncPointer"),
            (Self::SYNC_POINTER.0.into(), "SYNC_POINTER", "SyncPointer"),
            (Self::REPLAY_POINTER.0.into(), "REPLAY_POINTER", "ReplayPointer"),
            (Self::ASYNC_KEYBOARD.0.into(), "ASYNC_KEYBOARD", "AsyncKeyboard"),
            (Self::SYNC_KEYBOARD.0.into(), "SYNC_KEYBOARD", "SyncKeyboard"),
            (Self::REPLAY_KEYBOARD.0.into(), "REPLAY_KEYBOARD", "ReplayKeyboard"),
            (Self::ASYNC_BOTH.0.into(), "ASYNC_BOTH", "AsyncBoth"),
            (Self::SYNC_BOTH.0.into(), "SYNC_BOTH", "SyncBoth"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the AllowEvents request
pub const ALLOW_EVENTS_REQUEST: u8 = 35;
/// release queued events.
///
/// Releases queued events if the client has caused a device (pointer/keyboard) to
/// freeze due to grabbing it actively. This request has no effect if `time` is
/// earlier than the last-grab time of the most recent active grab for this client
/// or if `time` is later than the current X server time.
///
/// # Fields
///
/// * `mode` -
/// * `time` - Timestamp to avoid race conditions when running X over the network.
///
/// The special value `XCB_CURRENT_TIME` will be replaced with the current server
/// time.
///
/// # Errors
///
/// * `Value` - You specified an invalid `mode`.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllowEventsRequest {
    pub mode: Allow,
    pub time: Timestamp,
}
impl_debug_if_no_extra_traits!(AllowEventsRequest, "AllowEventsRequest");
impl AllowEventsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mode_bytes = u8::from(self.mode).serialize();
        let time_bytes = self.time.serialize();
        let mut request0 = vec![
            ALLOW_EVENTS_REQUEST,
            mode_bytes[0],
            0,
            0,
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
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
        if header.major_opcode != ALLOW_EVENTS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (mode, remaining) = u8::try_parse(remaining)?;
        let mode = mode.into();
        let _ = remaining;
        let (time, remaining) = Timestamp::try_parse(value)?;
        let _ = remaining;
        Ok(AllowEventsRequest {
            mode,
            time,
        })
    }
}
impl Request for AllowEventsRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for AllowEventsRequest {
}

/// Opcode for the GrabServer request
pub const GRAB_SERVER_REQUEST: u8 = 36;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrabServerRequest;
impl_debug_if_no_extra_traits!(GrabServerRequest, "GrabServerRequest");
impl GrabServerRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            GRAB_SERVER_REQUEST,
            0,
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
        if header.major_opcode != GRAB_SERVER_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let _ = value;
        Ok(GrabServerRequest
        )
    }
}
impl Request for GrabServerRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for GrabServerRequest {
}

/// Opcode for the UngrabServer request
pub const UNGRAB_SERVER_REQUEST: u8 = 37;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UngrabServerRequest;
impl_debug_if_no_extra_traits!(UngrabServerRequest, "UngrabServerRequest");
impl UngrabServerRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            UNGRAB_SERVER_REQUEST,
            0,
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
        if header.major_opcode != UNGRAB_SERVER_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let _ = value;
        Ok(UngrabServerRequest
        )
    }
}
impl Request for UngrabServerRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for UngrabServerRequest {
}

/// Opcode for the QueryPointer request
pub const QUERY_POINTER_REQUEST: u8 = 38;
/// get pointer coordinates.
///
/// Gets the root window the pointer is logically on and the pointer coordinates
/// relative to the root window's origin.
///
/// # Fields
///
/// * `window` - A window to check if the pointer is on the same screen as `window` (see the
/// `same_screen` field in the reply).
///
/// # Errors
///
/// * `Window` - The specified `window` does not exist.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryPointerRequest {
    pub window: Window,
}
impl_debug_if_no_extra_traits!(QueryPointerRequest, "QueryPointerRequest");
impl QueryPointerRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            QUERY_POINTER_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.major_opcode != QUERY_POINTER_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let _ = remaining;
        Ok(QueryPointerRequest {
            window,
        })
    }
}
impl Request for QueryPointerRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryPointerRequest {
    type Reply = QueryPointerReply;
}

/// # Fields
///
/// * `same_screen` - If `same_screen` is False, then the pointer is not on the same screen as the
/// argument window, `child` is None, and `win_x` and `win_y` are zero. If
/// `same_screen` is True, then `win_x` and `win_y` are the pointer coordinates
/// relative to the argument window's origin, and child is the child containing the
/// pointer, if any.
/// * `root` - The root window the pointer is logically on.
/// * `child` - The child window containing the pointer, if any, if `same_screen` is true. If
/// `same_screen` is false, `XCB_NONE` is returned.
/// * `root_x` - The pointer X position, relative to `root`.
/// * `root_y` - The pointer Y position, relative to `root`.
/// * `win_x` - The pointer X coordinate, relative to `child`, if `same_screen` is true. Zero
/// otherwise.
/// * `win_y` - The pointer Y coordinate, relative to `child`, if `same_screen` is true. Zero
/// otherwise.
/// * `mask` - The current logical state of the modifier keys and the buttons. Note that the
/// logical state of a device (as seen by means of the protocol) may lag the
/// physical state if device event processing is frozen.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryPointerReply {
    pub same_screen: bool,
    pub sequence: u16,
    pub length: u32,
    pub root: Window,
    pub child: Window,
    pub root_x: i16,
    pub root_y: i16,
    pub win_x: i16,
    pub win_y: i16,
    pub mask: KeyButMask,
}
impl_debug_if_no_extra_traits!(QueryPointerReply, "QueryPointerReply");
impl TryParse for QueryPointerReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (same_screen, remaining) = bool::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (root, remaining) = Window::try_parse(remaining)?;
        let (child, remaining) = Window::try_parse(remaining)?;
        let (root_x, remaining) = i16::try_parse(remaining)?;
        let (root_y, remaining) = i16::try_parse(remaining)?;
        let (win_x, remaining) = i16::try_parse(remaining)?;
        let (win_y, remaining) = i16::try_parse(remaining)?;
        let (mask, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let mask = mask.into();
        let result = QueryPointerReply { same_screen, sequence, length, root, child, root_x, root_y, win_x, win_y, mask };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryPointerReply {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        let response_type_bytes = &[1];
        let same_screen_bytes = self.same_screen.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let root_bytes = self.root.serialize();
        let child_bytes = self.child.serialize();
        let root_x_bytes = self.root_x.serialize();
        let root_y_bytes = self.root_y.serialize();
        let win_x_bytes = self.win_x.serialize();
        let win_y_bytes = self.win_y.serialize();
        let mask_bytes = u16::from(self.mask).serialize();
        [
            response_type_bytes[0],
            same_screen_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            root_bytes[0],
            root_bytes[1],
            root_bytes[2],
            root_bytes[3],
            child_bytes[0],
            child_bytes[1],
            child_bytes[2],
            child_bytes[3],
            root_x_bytes[0],
            root_x_bytes[1],
            root_y_bytes[0],
            root_y_bytes[1],
            win_x_bytes[0],
            win_x_bytes[1],
            win_y_bytes[0],
            win_y_bytes[1],
            mask_bytes[0],
            mask_bytes[1],
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(28);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.same_screen.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.root.serialize_into(bytes);
        self.child.serialize_into(bytes);
        self.root_x.serialize_into(bytes);
        self.root_y.serialize_into(bytes);
        self.win_x.serialize_into(bytes);
        self.win_y.serialize_into(bytes);
        u16::from(self.mask).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Timecoord {
    pub time: Timestamp,
    pub x: i16,
    pub y: i16,
}
impl_debug_if_no_extra_traits!(Timecoord, "Timecoord");
impl TryParse for Timecoord {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let result = Timecoord { time, x, y };
        Ok((result, remaining))
    }
}
impl Serialize for Timecoord {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let time_bytes = self.time.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        [
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.time.serialize_into(bytes);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
    }
}

/// Opcode for the GetMotionEvents request
pub const GET_MOTION_EVENTS_REQUEST: u8 = 39;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetMotionEventsRequest {
    pub window: Window,
    pub start: Timestamp,
    pub stop: Timestamp,
}
impl_debug_if_no_extra_traits!(GetMotionEventsRequest, "GetMotionEventsRequest");
impl GetMotionEventsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let start_bytes = self.start.serialize();
        let stop_bytes = self.stop.serialize();
        let mut request0 = vec![
            GET_MOTION_EVENTS_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            start_bytes[0],
            start_bytes[1],
            start_bytes[2],
            start_bytes[3],
            stop_bytes[0],
            stop_bytes[1],
            stop_bytes[2],
            stop_bytes[3],
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
        if header.major_opcode != GET_MOTION_EVENTS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let (start, remaining) = Timestamp::try_parse(remaining)?;
        let (stop, remaining) = Timestamp::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetMotionEventsRequest {
            window,
            start,
            stop,
        })
    }
}
impl Request for GetMotionEventsRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetMotionEventsRequest {
    type Reply = GetMotionEventsReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetMotionEventsReply {
    pub sequence: u16,
    pub length: u32,
    pub events: Vec<Timecoord>,
}
impl_debug_if_no_extra_traits!(GetMotionEventsReply, "GetMotionEventsReply");
impl TryParse for GetMotionEventsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (events_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (events, remaining) = crate::x11_utils::parse_list::<Timecoord>(remaining, events_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetMotionEventsReply { sequence, length, events };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetMotionEventsReply {
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
        let events_len = u32::try_from(self.events.len()).expect("`events` has too many elements");
        events_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        self.events.serialize_into(bytes);
    }
}
impl GetMotionEventsReply {
    /// Get the value of the `events_len` field.
    ///
    /// The `events_len` field is used as the length field of the `events` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn events_len(&self) -> u32 {
        self.events.len()
            .try_into().unwrap()
    }
}

/// Opcode for the TranslateCoordinates request
pub const TRANSLATE_COORDINATES_REQUEST: u8 = 40;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TranslateCoordinatesRequest {
    pub src_window: Window,
    pub dst_window: Window,
    pub src_x: i16,
    pub src_y: i16,
}
impl_debug_if_no_extra_traits!(TranslateCoordinatesRequest, "TranslateCoordinatesRequest");
impl TranslateCoordinatesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let src_window_bytes = self.src_window.serialize();
        let dst_window_bytes = self.dst_window.serialize();
        let src_x_bytes = self.src_x.serialize();
        let src_y_bytes = self.src_y.serialize();
        let mut request0 = vec![
            TRANSLATE_COORDINATES_REQUEST,
            0,
            0,
            0,
            src_window_bytes[0],
            src_window_bytes[1],
            src_window_bytes[2],
            src_window_bytes[3],
            dst_window_bytes[0],
            dst_window_bytes[1],
            dst_window_bytes[2],
            dst_window_bytes[3],
            src_x_bytes[0],
            src_x_bytes[1],
            src_y_bytes[0],
            src_y_bytes[1],
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
        if header.major_opcode != TRANSLATE_COORDINATES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (src_window, remaining) = Window::try_parse(value)?;
        let (dst_window, remaining) = Window::try_parse(remaining)?;
        let (src_x, remaining) = i16::try_parse(remaining)?;
        let (src_y, remaining) = i16::try_parse(remaining)?;
        let _ = remaining;
        Ok(TranslateCoordinatesRequest {
            src_window,
            dst_window,
            src_x,
            src_y,
        })
    }
}
impl Request for TranslateCoordinatesRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for TranslateCoordinatesRequest {
    type Reply = TranslateCoordinatesReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TranslateCoordinatesReply {
    pub same_screen: bool,
    pub sequence: u16,
    pub length: u32,
    pub child: Window,
    pub dst_x: i16,
    pub dst_y: i16,
}
impl_debug_if_no_extra_traits!(TranslateCoordinatesReply, "TranslateCoordinatesReply");
impl TryParse for TranslateCoordinatesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (same_screen, remaining) = bool::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (child, remaining) = Window::try_parse(remaining)?;
        let (dst_x, remaining) = i16::try_parse(remaining)?;
        let (dst_y, remaining) = i16::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = TranslateCoordinatesReply { same_screen, sequence, length, child, dst_x, dst_y };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for TranslateCoordinatesReply {
    type Bytes = [u8; 16];
    fn serialize(&self) -> [u8; 16] {
        let response_type_bytes = &[1];
        let same_screen_bytes = self.same_screen.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let child_bytes = self.child.serialize();
        let dst_x_bytes = self.dst_x.serialize();
        let dst_y_bytes = self.dst_y.serialize();
        [
            response_type_bytes[0],
            same_screen_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            child_bytes[0],
            child_bytes[1],
            child_bytes[2],
            child_bytes[3],
            dst_x_bytes[0],
            dst_x_bytes[1],
            dst_y_bytes[0],
            dst_y_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.same_screen.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.child.serialize_into(bytes);
        self.dst_x.serialize_into(bytes);
        self.dst_y.serialize_into(bytes);
    }
}

/// Opcode for the WarpPointer request
pub const WARP_POINTER_REQUEST: u8 = 41;
/// move mouse pointer.
///
/// Moves the mouse pointer to the specified position.
///
/// If `src_window` is not `XCB_NONE` (TODO), the move will only take place if the
/// pointer is inside `src_window` and within the rectangle specified by (`src_x`,
/// `src_y`, `src_width`, `src_height`). The rectangle coordinates are relative to
/// `src_window`.
///
/// If `dst_window` is not `XCB_NONE` (TODO), the pointer will be moved to the
/// offsets (`dst_x`, `dst_y`) relative to `dst_window`. If `dst_window` is
/// `XCB_NONE` (TODO), the pointer will be moved by the offsets (`dst_x`, `dst_y`)
/// relative to the current position of the pointer.
///
/// # Fields
///
/// * `src_window` - If `src_window` is not `XCB_NONE` (TODO), the move will only take place if the
/// pointer is inside `src_window` and within the rectangle specified by (`src_x`,
/// `src_y`, `src_width`, `src_height`). The rectangle coordinates are relative to
/// `src_window`.
/// * `dst_window` - If `dst_window` is not `XCB_NONE` (TODO), the pointer will be moved to the
/// offsets (`dst_x`, `dst_y`) relative to `dst_window`. If `dst_window` is
/// `XCB_NONE` (TODO), the pointer will be moved by the offsets (`dst_x`, `dst_y`)
/// relative to the current position of the pointer.
///
/// # Errors
///
/// * `Window` - TODO: reasons?
///
/// # See
///
/// * `SetInputFocus`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WarpPointerRequest {
    pub src_window: Window,
    pub dst_window: Window,
    pub src_x: i16,
    pub src_y: i16,
    pub src_width: u16,
    pub src_height: u16,
    pub dst_x: i16,
    pub dst_y: i16,
}
impl_debug_if_no_extra_traits!(WarpPointerRequest, "WarpPointerRequest");
impl WarpPointerRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let src_window_bytes = self.src_window.serialize();
        let dst_window_bytes = self.dst_window.serialize();
        let src_x_bytes = self.src_x.serialize();
        let src_y_bytes = self.src_y.serialize();
        let src_width_bytes = self.src_width.serialize();
        let src_height_bytes = self.src_height.serialize();
        let dst_x_bytes = self.dst_x.serialize();
        let dst_y_bytes = self.dst_y.serialize();
        let mut request0 = vec![
            WARP_POINTER_REQUEST,
            0,
            0,
            0,
            src_window_bytes[0],
            src_window_bytes[1],
            src_window_bytes[2],
            src_window_bytes[3],
            dst_window_bytes[0],
            dst_window_bytes[1],
            dst_window_bytes[2],
            dst_window_bytes[3],
            src_x_bytes[0],
            src_x_bytes[1],
            src_y_bytes[0],
            src_y_bytes[1],
            src_width_bytes[0],
            src_width_bytes[1],
            src_height_bytes[0],
            src_height_bytes[1],
            dst_x_bytes[0],
            dst_x_bytes[1],
            dst_y_bytes[0],
            dst_y_bytes[1],
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
        if header.major_opcode != WARP_POINTER_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (src_window, remaining) = Window::try_parse(value)?;
        let (dst_window, remaining) = Window::try_parse(remaining)?;
        let (src_x, remaining) = i16::try_parse(remaining)?;
        let (src_y, remaining) = i16::try_parse(remaining)?;
        let (src_width, remaining) = u16::try_parse(remaining)?;
        let (src_height, remaining) = u16::try_parse(remaining)?;
        let (dst_x, remaining) = i16::try_parse(remaining)?;
        let (dst_y, remaining) = i16::try_parse(remaining)?;
        let _ = remaining;
        Ok(WarpPointerRequest {
            src_window,
            dst_window,
            src_x,
            src_y,
            src_width,
            src_height,
            dst_x,
            dst_y,
        })
    }
}
impl Request for WarpPointerRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for WarpPointerRequest {
}

/// # Fields
///
/// * `None` - The focus reverts to `XCB_NONE`, so no window will have the input focus.
/// * `PointerRoot` - The focus reverts to `XCB_POINTER_ROOT` respectively. When the focus reverts,
/// FocusIn and FocusOut events are generated, but the last-focus-change time is
/// not changed.
/// * `Parent` - The focus reverts to the parent (or closest viewable ancestor) and the new
/// revert_to value is `XCB_INPUT_FOCUS_NONE`.
/// * `FollowKeyboard` - NOT YET DOCUMENTED. Only relevant for the xinput extension.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InputFocus(u8);
impl InputFocus {
    pub const NONE: Self = Self(0);
    pub const POINTER_ROOT: Self = Self(1);
    pub const PARENT: Self = Self(2);
    pub const FOLLOW_KEYBOARD: Self = Self(3);
}
impl From<InputFocus> for u8 {
    #[inline]
    fn from(input: InputFocus) -> Self {
        input.0
    }
}
impl From<InputFocus> for Option<u8> {
    #[inline]
    fn from(input: InputFocus) -> Self {
        Some(input.0)
    }
}
impl From<InputFocus> for u16 {
    #[inline]
    fn from(input: InputFocus) -> Self {
        u16::from(input.0)
    }
}
impl From<InputFocus> for Option<u16> {
    #[inline]
    fn from(input: InputFocus) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<InputFocus> for u32 {
    #[inline]
    fn from(input: InputFocus) -> Self {
        u32::from(input.0)
    }
}
impl From<InputFocus> for Option<u32> {
    #[inline]
    fn from(input: InputFocus) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for InputFocus {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for InputFocus  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NONE.0.into(), "NONE", "None"),
            (Self::POINTER_ROOT.0.into(), "POINTER_ROOT", "PointerRoot"),
            (Self::PARENT.0.into(), "PARENT", "Parent"),
            (Self::FOLLOW_KEYBOARD.0.into(), "FOLLOW_KEYBOARD", "FollowKeyboard"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the SetInputFocus request
pub const SET_INPUT_FOCUS_REQUEST: u8 = 42;
/// Sets input focus.
///
/// Changes the input focus and the last-focus-change time. If the specified `time`
/// is earlier than the current last-focus-change time, the request is ignored (to
/// avoid race conditions when running X over the network).
///
/// A FocusIn and FocusOut event is generated when focus is changed.
///
/// # Fields
///
/// * `focus` - The window to focus. All keyboard events will be reported to this window. The
/// window must be viewable (TODO), or a `xcb_match_error_t` occurs (TODO).
///
/// If `focus` is `XCB_NONE` (TODO), all keyboard events are
/// discarded until a new focus window is set.
///
/// If `focus` is `XCB_POINTER_ROOT` (TODO), focus is on the root window of the
/// screen on which the pointer is on currently.
/// * `time` - Timestamp to avoid race conditions when running X over the network.
///
/// The special value `XCB_CURRENT_TIME` will be replaced with the current server
/// time.
/// * `revert_to` - Specifies what happens when the `focus` window becomes unviewable (if `focus`
/// is neither `XCB_NONE` nor `XCB_POINTER_ROOT`).
///
/// # Errors
///
/// * `Window` - The specified `focus` window does not exist.
/// * `Match` - The specified `focus` window is not viewable.
/// * `Value` - TODO: Reasons?
///
/// # See
///
/// * `FocusIn`: event
/// * `FocusOut`: event
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetInputFocusRequest {
    pub revert_to: InputFocus,
    pub focus: Window,
    pub time: Timestamp,
}
impl_debug_if_no_extra_traits!(SetInputFocusRequest, "SetInputFocusRequest");
impl SetInputFocusRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let revert_to_bytes = u8::from(self.revert_to).serialize();
        let focus_bytes = self.focus.serialize();
        let time_bytes = self.time.serialize();
        let mut request0 = vec![
            SET_INPUT_FOCUS_REQUEST,
            revert_to_bytes[0],
            0,
            0,
            focus_bytes[0],
            focus_bytes[1],
            focus_bytes[2],
            focus_bytes[3],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
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
        if header.major_opcode != SET_INPUT_FOCUS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (revert_to, remaining) = u8::try_parse(remaining)?;
        let revert_to = revert_to.into();
        let _ = remaining;
        let (focus, remaining) = Window::try_parse(value)?;
        let (time, remaining) = Timestamp::try_parse(remaining)?;
        let _ = remaining;
        Ok(SetInputFocusRequest {
            revert_to,
            focus,
            time,
        })
    }
}
impl Request for SetInputFocusRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SetInputFocusRequest {
}

/// Opcode for the GetInputFocus request
pub const GET_INPUT_FOCUS_REQUEST: u8 = 43;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetInputFocusRequest;
impl_debug_if_no_extra_traits!(GetInputFocusRequest, "GetInputFocusRequest");
impl GetInputFocusRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            GET_INPUT_FOCUS_REQUEST,
            0,
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
        if header.major_opcode != GET_INPUT_FOCUS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let _ = value;
        Ok(GetInputFocusRequest
        )
    }
}
impl Request for GetInputFocusRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetInputFocusRequest {
    type Reply = GetInputFocusReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetInputFocusReply {
    pub revert_to: InputFocus,
    pub sequence: u16,
    pub length: u32,
    pub focus: Window,
}
impl_debug_if_no_extra_traits!(GetInputFocusReply, "GetInputFocusReply");
impl TryParse for GetInputFocusReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (revert_to, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (focus, remaining) = Window::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let revert_to = revert_to.into();
        let result = GetInputFocusReply { revert_to, sequence, length, focus };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetInputFocusReply {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = &[1];
        let revert_to_bytes = u8::from(self.revert_to).serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let focus_bytes = self.focus.serialize();
        [
            response_type_bytes[0],
            revert_to_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            focus_bytes[0],
            focus_bytes[1],
            focus_bytes[2],
            focus_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        u8::from(self.revert_to).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.focus.serialize_into(bytes);
    }
}

/// Opcode for the QueryKeymap request
pub const QUERY_KEYMAP_REQUEST: u8 = 44;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryKeymapRequest;
impl_debug_if_no_extra_traits!(QueryKeymapRequest, "QueryKeymapRequest");
impl QueryKeymapRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            QUERY_KEYMAP_REQUEST,
            0,
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
        if header.major_opcode != QUERY_KEYMAP_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let _ = value;
        Ok(QueryKeymapRequest
        )
    }
}
impl Request for QueryKeymapRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryKeymapRequest {
    type Reply = QueryKeymapReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryKeymapReply {
    pub sequence: u16,
    pub length: u32,
    pub keys: [u8; 32],
}
impl_debug_if_no_extra_traits!(QueryKeymapReply, "QueryKeymapReply");
impl TryParse for QueryKeymapReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (keys, remaining) = crate::x11_utils::parse_u8_array::<32>(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryKeymapReply { sequence, length, keys };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryKeymapReply {
    type Bytes = [u8; 40];
    fn serialize(&self) -> [u8; 40] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            self.keys[0],
            self.keys[1],
            self.keys[2],
            self.keys[3],
            self.keys[4],
            self.keys[5],
            self.keys[6],
            self.keys[7],
            self.keys[8],
            self.keys[9],
            self.keys[10],
            self.keys[11],
            self.keys[12],
            self.keys[13],
            self.keys[14],
            self.keys[15],
            self.keys[16],
            self.keys[17],
            self.keys[18],
            self.keys[19],
            self.keys[20],
            self.keys[21],
            self.keys[22],
            self.keys[23],
            self.keys[24],
            self.keys[25],
            self.keys[26],
            self.keys[27],
            self.keys[28],
            self.keys[29],
            self.keys[30],
            self.keys[31],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(40);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        bytes.extend_from_slice(&self.keys);
    }
}

/// Opcode for the OpenFont request
pub const OPEN_FONT_REQUEST: u8 = 45;
/// opens a font.
///
/// Opens any X core font matching the given `name` (for example "-misc-fixed-*").
///
/// Note that X core fonts are deprecated (but still supported) in favor of
/// client-side rendering using Xft.
///
/// # Fields
///
/// * `fid` - The ID with which you will refer to the font, created by `xcb_generate_id`.
/// * `name` - A pattern describing an X core font.
///
/// # Errors
///
/// * `Name` - No font matches the given `name`.
///
/// # See
///
/// * `xcb_generate_id`: function
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpenFontRequest<'input> {
    pub fid: Font,
    pub name: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(OpenFontRequest<'_>, "OpenFontRequest");
impl<'input> OpenFontRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let fid_bytes = self.fid.serialize();
        let name_len = u16::try_from(self.name.len()).expect("`name` has too many elements");
        let name_len_bytes = name_len.serialize();
        let mut request0 = vec![
            OPEN_FONT_REQUEST,
            0,
            0,
            0,
            fid_bytes[0],
            fid_bytes[1],
            fid_bytes[2],
            fid_bytes[3],
            name_len_bytes[0],
            name_len_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.name.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.name, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != OPEN_FONT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (fid, remaining) = Font::try_parse(value)?;
        let (name_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, name_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(OpenFontRequest {
            fid,
            name: Cow::Borrowed(name),
        })
    }
    /// Clone all borrowed data in this OpenFontRequest.
    pub fn into_owned(self) -> OpenFontRequest<'static> {
        OpenFontRequest {
            fid: self.fid,
            name: Cow::Owned(self.name.into_owned()),
        }
    }
}
impl<'input> Request for OpenFontRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for OpenFontRequest<'input> {
}

/// Opcode for the CloseFont request
pub const CLOSE_FONT_REQUEST: u8 = 46;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CloseFontRequest {
    pub font: Font,
}
impl_debug_if_no_extra_traits!(CloseFontRequest, "CloseFontRequest");
impl CloseFontRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let font_bytes = self.font.serialize();
        let mut request0 = vec![
            CLOSE_FONT_REQUEST,
            0,
            0,
            0,
            font_bytes[0],
            font_bytes[1],
            font_bytes[2],
            font_bytes[3],
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
        if header.major_opcode != CLOSE_FONT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (font, remaining) = Font::try_parse(value)?;
        let _ = remaining;
        Ok(CloseFontRequest {
            font,
        })
    }
}
impl Request for CloseFontRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CloseFontRequest {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FontDraw(u8);
impl FontDraw {
    pub const LEFT_TO_RIGHT: Self = Self(0);
    pub const RIGHT_TO_LEFT: Self = Self(1);
}
impl From<FontDraw> for u8 {
    #[inline]
    fn from(input: FontDraw) -> Self {
        input.0
    }
}
impl From<FontDraw> for Option<u8> {
    #[inline]
    fn from(input: FontDraw) -> Self {
        Some(input.0)
    }
}
impl From<FontDraw> for u16 {
    #[inline]
    fn from(input: FontDraw) -> Self {
        u16::from(input.0)
    }
}
impl From<FontDraw> for Option<u16> {
    #[inline]
    fn from(input: FontDraw) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<FontDraw> for u32 {
    #[inline]
    fn from(input: FontDraw) -> Self {
        u32::from(input.0)
    }
}
impl From<FontDraw> for Option<u32> {
    #[inline]
    fn from(input: FontDraw) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for FontDraw {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for FontDraw  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::LEFT_TO_RIGHT.0.into(), "LEFT_TO_RIGHT", "LeftToRight"),
            (Self::RIGHT_TO_LEFT.0.into(), "RIGHT_TO_LEFT", "RightToLeft"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fontprop {
    pub name: Atom,
    pub value: u32,
}
impl_debug_if_no_extra_traits!(Fontprop, "Fontprop");
impl TryParse for Fontprop {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (name, remaining) = Atom::try_parse(remaining)?;
        let (value, remaining) = u32::try_parse(remaining)?;
        let result = Fontprop { name, value };
        Ok((result, remaining))
    }
}
impl Serialize for Fontprop {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let name_bytes = self.name.serialize();
        let value_bytes = self.value.serialize();
        [
            name_bytes[0],
            name_bytes[1],
            name_bytes[2],
            name_bytes[3],
            value_bytes[0],
            value_bytes[1],
            value_bytes[2],
            value_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.name.serialize_into(bytes);
        self.value.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Charinfo {
    pub left_side_bearing: i16,
    pub right_side_bearing: i16,
    pub character_width: i16,
    pub ascent: i16,
    pub descent: i16,
    pub attributes: u16,
}
impl_debug_if_no_extra_traits!(Charinfo, "Charinfo");
impl TryParse for Charinfo {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (left_side_bearing, remaining) = i16::try_parse(remaining)?;
        let (right_side_bearing, remaining) = i16::try_parse(remaining)?;
        let (character_width, remaining) = i16::try_parse(remaining)?;
        let (ascent, remaining) = i16::try_parse(remaining)?;
        let (descent, remaining) = i16::try_parse(remaining)?;
        let (attributes, remaining) = u16::try_parse(remaining)?;
        let result = Charinfo { left_side_bearing, right_side_bearing, character_width, ascent, descent, attributes };
        Ok((result, remaining))
    }
}
impl Serialize for Charinfo {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let left_side_bearing_bytes = self.left_side_bearing.serialize();
        let right_side_bearing_bytes = self.right_side_bearing.serialize();
        let character_width_bytes = self.character_width.serialize();
        let ascent_bytes = self.ascent.serialize();
        let descent_bytes = self.descent.serialize();
        let attributes_bytes = self.attributes.serialize();
        [
            left_side_bearing_bytes[0],
            left_side_bearing_bytes[1],
            right_side_bearing_bytes[0],
            right_side_bearing_bytes[1],
            character_width_bytes[0],
            character_width_bytes[1],
            ascent_bytes[0],
            ascent_bytes[1],
            descent_bytes[0],
            descent_bytes[1],
            attributes_bytes[0],
            attributes_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.left_side_bearing.serialize_into(bytes);
        self.right_side_bearing.serialize_into(bytes);
        self.character_width.serialize_into(bytes);
        self.ascent.serialize_into(bytes);
        self.descent.serialize_into(bytes);
        self.attributes.serialize_into(bytes);
    }
}

/// Opcode for the QueryFont request
pub const QUERY_FONT_REQUEST: u8 = 47;
/// query font metrics.
///
/// Queries information associated with the font.
///
/// # Fields
///
/// * `font` - The fontable (Font or Graphics Context) to query.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryFontRequest {
    pub font: Fontable,
}
impl_debug_if_no_extra_traits!(QueryFontRequest, "QueryFontRequest");
impl QueryFontRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let font_bytes = self.font.serialize();
        let mut request0 = vec![
            QUERY_FONT_REQUEST,
            0,
            0,
            0,
            font_bytes[0],
            font_bytes[1],
            font_bytes[2],
            font_bytes[3],
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
        if header.major_opcode != QUERY_FONT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (font, remaining) = Fontable::try_parse(value)?;
        let _ = remaining;
        Ok(QueryFontRequest {
            font,
        })
    }
}
impl Request for QueryFontRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryFontRequest {
    type Reply = QueryFontReply;
}

/// # Fields
///
/// * `min_bounds` - minimum bounds over all existing char
/// * `max_bounds` - maximum bounds over all existing char
/// * `min_char_or_byte2` - first character
/// * `max_char_or_byte2` - last character
/// * `default_char` - char to print for undefined character
/// * `all_chars_exist` - flag if all characters have nonzero size
/// * `font_ascent` - baseline to top edge of raster
/// * `font_descent` - baseline to bottom edge of raster
/// * `draw_direction` -
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryFontReply {
    pub sequence: u16,
    pub length: u32,
    pub min_bounds: Charinfo,
    pub max_bounds: Charinfo,
    pub min_char_or_byte2: u16,
    pub max_char_or_byte2: u16,
    pub default_char: u16,
    pub draw_direction: FontDraw,
    pub min_byte1: u8,
    pub max_byte1: u8,
    pub all_chars_exist: bool,
    pub font_ascent: i16,
    pub font_descent: i16,
    pub properties: Vec<Fontprop>,
    pub char_infos: Vec<Charinfo>,
}
impl_debug_if_no_extra_traits!(QueryFontReply, "QueryFontReply");
impl TryParse for QueryFontReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (min_bounds, remaining) = Charinfo::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let (max_bounds, remaining) = Charinfo::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let (min_char_or_byte2, remaining) = u16::try_parse(remaining)?;
        let (max_char_or_byte2, remaining) = u16::try_parse(remaining)?;
        let (default_char, remaining) = u16::try_parse(remaining)?;
        let (properties_len, remaining) = u16::try_parse(remaining)?;
        let (draw_direction, remaining) = u8::try_parse(remaining)?;
        let (min_byte1, remaining) = u8::try_parse(remaining)?;
        let (max_byte1, remaining) = u8::try_parse(remaining)?;
        let (all_chars_exist, remaining) = bool::try_parse(remaining)?;
        let (font_ascent, remaining) = i16::try_parse(remaining)?;
        let (font_descent, remaining) = i16::try_parse(remaining)?;
        let (char_infos_len, remaining) = u32::try_parse(remaining)?;
        let (properties, remaining) = crate::x11_utils::parse_list::<Fontprop>(remaining, properties_len.try_to_usize()?)?;
        let (char_infos, remaining) = crate::x11_utils::parse_list::<Charinfo>(remaining, char_infos_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let draw_direction = draw_direction.into();
        let result = QueryFontReply { sequence, length, min_bounds, max_bounds, min_char_or_byte2, max_char_or_byte2, default_char, draw_direction, min_byte1, max_byte1, all_chars_exist, font_ascent, font_descent, properties, char_infos };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryFontReply {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(60);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.min_bounds.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
        self.max_bounds.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
        self.min_char_or_byte2.serialize_into(bytes);
        self.max_char_or_byte2.serialize_into(bytes);
        self.default_char.serialize_into(bytes);
        let properties_len = u16::try_from(self.properties.len()).expect("`properties` has too many elements");
        properties_len.serialize_into(bytes);
        u8::from(self.draw_direction).serialize_into(bytes);
        self.min_byte1.serialize_into(bytes);
        self.max_byte1.serialize_into(bytes);
        self.all_chars_exist.serialize_into(bytes);
        self.font_ascent.serialize_into(bytes);
        self.font_descent.serialize_into(bytes);
        let char_infos_len = u32::try_from(self.char_infos.len()).expect("`char_infos` has too many elements");
        char_infos_len.serialize_into(bytes);
        self.properties.serialize_into(bytes);
        self.char_infos.serialize_into(bytes);
    }
}
impl QueryFontReply {
    /// Get the value of the `properties_len` field.
    ///
    /// The `properties_len` field is used as the length field of the `properties` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn properties_len(&self) -> u16 {
        self.properties.len()
            .try_into().unwrap()
    }
    /// Get the value of the `char_infos_len` field.
    ///
    /// The `char_infos_len` field is used as the length field of the `char_infos` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn char_infos_len(&self) -> u32 {
        self.char_infos.len()
            .try_into().unwrap()
    }
}

/// Opcode for the QueryTextExtents request
pub const QUERY_TEXT_EXTENTS_REQUEST: u8 = 48;
/// get text extents.
///
/// Query text extents from the X11 server. This request returns the bounding box
/// of the specified 16-bit character string in the specified `font` or the font
/// contained in the specified graphics context.
///
/// `font_ascent` is set to the maximum of the ascent metrics of all characters in
/// the string. `font_descent` is set to the maximum of the descent metrics.
/// `overall_width` is set to the sum of the character-width metrics of all
/// characters in the string. For each character in the string, let W be the sum of
/// the character-width metrics of all characters preceding it in the string. Let L
/// be the left-side-bearing metric of the character plus W. Let R be the
/// right-side-bearing metric of the character plus W. The lbearing member is set
/// to the minimum L of all characters in the string. The rbearing member is set to
/// the maximum R.
///
/// For fonts defined with linear indexing rather than 2-byte matrix indexing, each
/// `xcb_char2b_t` structure is interpreted as a 16-bit number with byte1 as the
/// most significant byte. If the font has no defined default character, undefined
/// characters in the string are taken to have all zero metrics.
///
/// Characters with all zero metrics are ignored. If the font has no defined
/// default_char, the undefined characters in the string are also ignored.
///
/// # Fields
///
/// * `font` - The `font` to calculate text extents in. You can also pass a graphics context.
/// * `string_len` - The number of characters in `string`.
/// * `string` - The text to get text extents for.
///
/// # Errors
///
/// * `GContext` - The specified graphics context does not exist.
/// * `Font` - The specified `font` does not exist.
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryTextExtentsRequest<'input> {
    pub font: Fontable,
    pub string: Cow<'input, [Char2b]>,
}
impl_debug_if_no_extra_traits!(QueryTextExtentsRequest<'_>, "QueryTextExtentsRequest");
impl<'input> QueryTextExtentsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let string_len = u32::try_from(self.string.len()).unwrap();
        let length_so_far = 0;
        let odd_length = (u32::from(string_len) & 1u32) != 0;
        let odd_length_bytes = odd_length.serialize();
        let font_bytes = self.font.serialize();
        let mut request0 = vec![
            QUERY_TEXT_EXTENTS_REQUEST,
            odd_length_bytes[0],
            0,
            0,
            font_bytes[0],
            font_bytes[1],
            font_bytes[2],
            font_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let string_bytes = self.string.serialize();
        let length_so_far = length_so_far + string_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), string_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != QUERY_TEXT_EXTENTS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (odd_length, remaining) = bool::try_parse(remaining)?;
        let _ = remaining;
        let (font, remaining) = Fontable::try_parse(value)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut string = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Char2b::try_parse(remaining)?;
            remaining = new_remaining;
            string.push(v);
        }
        let _ = remaining;
        if odd_length {
            if string.is_empty() {
                return Err(ParseError::InvalidValue);
            }
            string.truncate(string.len() - 1);
        }
        Ok(QueryTextExtentsRequest {
            font,
            string: Cow::Owned(string),
        })
    }
    /// Clone all borrowed data in this QueryTextExtentsRequest.
    pub fn into_owned(self) -> QueryTextExtentsRequest<'static> {
        QueryTextExtentsRequest {
            font: self.font,
            string: Cow::Owned(self.string.into_owned()),
        }
    }
}
impl<'input> Request for QueryTextExtentsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for QueryTextExtentsRequest<'input> {
    type Reply = QueryTextExtentsReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryTextExtentsReply {
    pub draw_direction: FontDraw,
    pub sequence: u16,
    pub length: u32,
    pub font_ascent: i16,
    pub font_descent: i16,
    pub overall_ascent: i16,
    pub overall_descent: i16,
    pub overall_width: i32,
    pub overall_left: i32,
    pub overall_right: i32,
}
impl_debug_if_no_extra_traits!(QueryTextExtentsReply, "QueryTextExtentsReply");
impl TryParse for QueryTextExtentsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (draw_direction, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (font_ascent, remaining) = i16::try_parse(remaining)?;
        let (font_descent, remaining) = i16::try_parse(remaining)?;
        let (overall_ascent, remaining) = i16::try_parse(remaining)?;
        let (overall_descent, remaining) = i16::try_parse(remaining)?;
        let (overall_width, remaining) = i32::try_parse(remaining)?;
        let (overall_left, remaining) = i32::try_parse(remaining)?;
        let (overall_right, remaining) = i32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let draw_direction = draw_direction.into();
        let result = QueryTextExtentsReply { draw_direction, sequence, length, font_ascent, font_descent, overall_ascent, overall_descent, overall_width, overall_left, overall_right };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryTextExtentsReply {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        let response_type_bytes = &[1];
        let draw_direction_bytes = u8::from(self.draw_direction).serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let font_ascent_bytes = self.font_ascent.serialize();
        let font_descent_bytes = self.font_descent.serialize();
        let overall_ascent_bytes = self.overall_ascent.serialize();
        let overall_descent_bytes = self.overall_descent.serialize();
        let overall_width_bytes = self.overall_width.serialize();
        let overall_left_bytes = self.overall_left.serialize();
        let overall_right_bytes = self.overall_right.serialize();
        [
            response_type_bytes[0],
            draw_direction_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            font_ascent_bytes[0],
            font_ascent_bytes[1],
            font_descent_bytes[0],
            font_descent_bytes[1],
            overall_ascent_bytes[0],
            overall_ascent_bytes[1],
            overall_descent_bytes[0],
            overall_descent_bytes[1],
            overall_width_bytes[0],
            overall_width_bytes[1],
            overall_width_bytes[2],
            overall_width_bytes[3],
            overall_left_bytes[0],
            overall_left_bytes[1],
            overall_left_bytes[2],
            overall_left_bytes[3],
            overall_right_bytes[0],
            overall_right_bytes[1],
            overall_right_bytes[2],
            overall_right_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(28);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        u8::from(self.draw_direction).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.font_ascent.serialize_into(bytes);
        self.font_descent.serialize_into(bytes);
        self.overall_ascent.serialize_into(bytes);
        self.overall_descent.serialize_into(bytes);
        self.overall_width.serialize_into(bytes);
        self.overall_left.serialize_into(bytes);
        self.overall_right.serialize_into(bytes);
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Str {
    pub name: Vec<u8>,
}
impl_debug_if_no_extra_traits!(Str, "Str");
impl TryParse for Str {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (name_len, remaining) = u8::try_parse(remaining)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, name_len.try_to_usize()?)?;
        let name = name.to_vec();
        let result = Str { name };
        Ok((result, remaining))
    }
}
impl Serialize for Str {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        let name_len = u8::try_from(self.name.len()).expect("`name` has too many elements");
        name_len.serialize_into(bytes);
        bytes.extend_from_slice(&self.name);
    }
}
impl Str {
    /// Get the value of the `name_len` field.
    ///
    /// The `name_len` field is used as the length field of the `name` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn name_len(&self) -> u8 {
        self.name.len()
            .try_into().unwrap()
    }
}

/// Opcode for the ListFonts request
pub const LIST_FONTS_REQUEST: u8 = 49;
/// get matching font names.
///
/// Gets a list of available font names which match the given `pattern`.
///
/// # Fields
///
/// * `pattern` - A font pattern, for example "-misc-fixed-*".
///
/// The asterisk (*) is a wildcard for any number of characters. The question mark
/// (?) is a wildcard for a single character. Use of uppercase or lowercase does
/// not matter.
/// * `max_names` - The maximum number of fonts to be returned.
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListFontsRequest<'input> {
    pub max_names: u16,
    pub pattern: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(ListFontsRequest<'_>, "ListFontsRequest");
impl<'input> ListFontsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let max_names_bytes = self.max_names.serialize();
        let pattern_len = u16::try_from(self.pattern.len()).expect("`pattern` has too many elements");
        let pattern_len_bytes = pattern_len.serialize();
        let mut request0 = vec![
            LIST_FONTS_REQUEST,
            0,
            0,
            0,
            max_names_bytes[0],
            max_names_bytes[1],
            pattern_len_bytes[0],
            pattern_len_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.pattern.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.pattern, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != LIST_FONTS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (max_names, remaining) = u16::try_parse(value)?;
        let (pattern_len, remaining) = u16::try_parse(remaining)?;
        let (pattern, remaining) = crate::x11_utils::parse_u8_list(remaining, pattern_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(ListFontsRequest {
            max_names,
            pattern: Cow::Borrowed(pattern),
        })
    }
    /// Clone all borrowed data in this ListFontsRequest.
    pub fn into_owned(self) -> ListFontsRequest<'static> {
        ListFontsRequest {
            max_names: self.max_names,
            pattern: Cow::Owned(self.pattern.into_owned()),
        }
    }
}
impl<'input> Request for ListFontsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for ListFontsRequest<'input> {
    type Reply = ListFontsReply;
}

/// # Fields
///
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListFontsReply {
    pub sequence: u16,
    pub length: u32,
    pub names: Vec<Str>,
}
impl_debug_if_no_extra_traits!(ListFontsReply, "ListFontsReply");
impl TryParse for ListFontsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (names_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
        let (names, remaining) = crate::x11_utils::parse_list::<Str>(remaining, names_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = ListFontsReply { sequence, length, names };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ListFontsReply {
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
        let names_len = u16::try_from(self.names.len()).expect("`names` has too many elements");
        names_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
        self.names.serialize_into(bytes);
    }
}
impl ListFontsReply {
    /// Get the value of the `names_len` field.
    ///
    /// The `names_len` field is used as the length field of the `names` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn names_len(&self) -> u16 {
        self.names.len()
            .try_into().unwrap()
    }
}

/// Opcode for the ListFontsWithInfo request
pub const LIST_FONTS_WITH_INFO_REQUEST: u8 = 50;
/// get matching font names and information.
///
/// Gets a list of available font names which match the given `pattern`.
///
/// # Fields
///
/// * `pattern` - A font pattern, for example "-misc-fixed-*".
///
/// The asterisk (*) is a wildcard for any number of characters. The question mark
/// (?) is a wildcard for a single character. Use of uppercase or lowercase does
/// not matter.
/// * `max_names` - The maximum number of fonts to be returned.
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListFontsWithInfoRequest<'input> {
    pub max_names: u16,
    pub pattern: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(ListFontsWithInfoRequest<'_>, "ListFontsWithInfoRequest");
impl<'input> ListFontsWithInfoRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let max_names_bytes = self.max_names.serialize();
        let pattern_len = u16::try_from(self.pattern.len()).expect("`pattern` has too many elements");
        let pattern_len_bytes = pattern_len.serialize();
        let mut request0 = vec![
            LIST_FONTS_WITH_INFO_REQUEST,
            0,
            0,
            0,
            max_names_bytes[0],
            max_names_bytes[1],
            pattern_len_bytes[0],
            pattern_len_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.pattern.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.pattern, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != LIST_FONTS_WITH_INFO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (max_names, remaining) = u16::try_parse(value)?;
        let (pattern_len, remaining) = u16::try_parse(remaining)?;
        let (pattern, remaining) = crate::x11_utils::parse_u8_list(remaining, pattern_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(ListFontsWithInfoRequest {
            max_names,
            pattern: Cow::Borrowed(pattern),
        })
    }
    /// Clone all borrowed data in this ListFontsWithInfoRequest.
    pub fn into_owned(self) -> ListFontsWithInfoRequest<'static> {
        ListFontsWithInfoRequest {
            max_names: self.max_names,
            pattern: Cow::Owned(self.pattern.into_owned()),
        }
    }
}
impl<'input> Request for ListFontsWithInfoRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for ListFontsWithInfoRequest<'input> {
    type Reply = ListFontsWithInfoReply;
}

/// # Fields
///
/// * `min_bounds` - minimum bounds over all existing char
/// * `max_bounds` - maximum bounds over all existing char
/// * `min_char_or_byte2` - first character
/// * `max_char_or_byte2` - last character
/// * `default_char` - char to print for undefined character
/// * `all_chars_exist` - flag if all characters have nonzero size
/// * `font_ascent` - baseline to top edge of raster
/// * `font_descent` - baseline to bottom edge of raster
/// * `replies_hint` - An indication of how many more fonts will be returned. This is only a hint and
/// may be larger or smaller than the number of fonts actually returned. A zero
/// value does not guarantee that no more fonts will be returned.
/// * `draw_direction` -
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListFontsWithInfoReply {
    pub sequence: u16,
    pub length: u32,
    pub min_bounds: Charinfo,
    pub max_bounds: Charinfo,
    pub min_char_or_byte2: u16,
    pub max_char_or_byte2: u16,
    pub default_char: u16,
    pub draw_direction: FontDraw,
    pub min_byte1: u8,
    pub max_byte1: u8,
    pub all_chars_exist: bool,
    pub font_ascent: i16,
    pub font_descent: i16,
    pub replies_hint: u32,
    pub properties: Vec<Fontprop>,
    pub name: Vec<u8>,
}
impl_debug_if_no_extra_traits!(ListFontsWithInfoReply, "ListFontsWithInfoReply");
impl TryParse for ListFontsWithInfoReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (name_len, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (min_bounds, remaining) = Charinfo::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let (max_bounds, remaining) = Charinfo::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let (min_char_or_byte2, remaining) = u16::try_parse(remaining)?;
        let (max_char_or_byte2, remaining) = u16::try_parse(remaining)?;
        let (default_char, remaining) = u16::try_parse(remaining)?;
        let (properties_len, remaining) = u16::try_parse(remaining)?;
        let (draw_direction, remaining) = u8::try_parse(remaining)?;
        let (min_byte1, remaining) = u8::try_parse(remaining)?;
        let (max_byte1, remaining) = u8::try_parse(remaining)?;
        let (all_chars_exist, remaining) = bool::try_parse(remaining)?;
        let (font_ascent, remaining) = i16::try_parse(remaining)?;
        let (font_descent, remaining) = i16::try_parse(remaining)?;
        let (replies_hint, remaining) = u32::try_parse(remaining)?;
        let (properties, remaining) = crate::x11_utils::parse_list::<Fontprop>(remaining, properties_len.try_to_usize()?)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, name_len.try_to_usize()?)?;
        let name = name.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let draw_direction = draw_direction.into();
        let result = ListFontsWithInfoReply { sequence, length, min_bounds, max_bounds, min_char_or_byte2, max_char_or_byte2, default_char, draw_direction, min_byte1, max_byte1, all_chars_exist, font_ascent, font_descent, replies_hint, properties, name };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ListFontsWithInfoReply {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(60);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        let name_len = u8::try_from(self.name.len()).expect("`name` has too many elements");
        name_len.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.min_bounds.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
        self.max_bounds.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
        self.min_char_or_byte2.serialize_into(bytes);
        self.max_char_or_byte2.serialize_into(bytes);
        self.default_char.serialize_into(bytes);
        let properties_len = u16::try_from(self.properties.len()).expect("`properties` has too many elements");
        properties_len.serialize_into(bytes);
        u8::from(self.draw_direction).serialize_into(bytes);
        self.min_byte1.serialize_into(bytes);
        self.max_byte1.serialize_into(bytes);
        self.all_chars_exist.serialize_into(bytes);
        self.font_ascent.serialize_into(bytes);
        self.font_descent.serialize_into(bytes);
        self.replies_hint.serialize_into(bytes);
        self.properties.serialize_into(bytes);
        bytes.extend_from_slice(&self.name);
    }
}
impl ListFontsWithInfoReply {
    /// Get the value of the `name_len` field.
    ///
    /// The `name_len` field is used as the length field of the `name` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn name_len(&self) -> u8 {
        self.name.len()
            .try_into().unwrap()
    }
    /// Get the value of the `properties_len` field.
    ///
    /// The `properties_len` field is used as the length field of the `properties` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn properties_len(&self) -> u16 {
        self.properties.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SetFontPath request
pub const SET_FONT_PATH_REQUEST: u8 = 51;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetFontPathRequest<'input> {
    pub font: Cow<'input, [Str]>,
}
impl_debug_if_no_extra_traits!(SetFontPathRequest<'_>, "SetFontPathRequest");
impl<'input> SetFontPathRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let font_qty = u16::try_from(self.font.len()).expect("`font` has too many elements");
        let font_qty_bytes = font_qty.serialize();
        let mut request0 = vec![
            SET_FONT_PATH_REQUEST,
            0,
            0,
            0,
            font_qty_bytes[0],
            font_qty_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let font_bytes = self.font.serialize();
        let length_so_far = length_so_far + font_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), font_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != SET_FONT_PATH_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (font_qty, remaining) = u16::try_parse(value)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (font, remaining) = crate::x11_utils::parse_list::<Str>(remaining, font_qty.try_to_usize()?)?;
        let _ = remaining;
        Ok(SetFontPathRequest {
            font: Cow::Owned(font),
        })
    }
    /// Clone all borrowed data in this SetFontPathRequest.
    pub fn into_owned(self) -> SetFontPathRequest<'static> {
        SetFontPathRequest {
            font: Cow::Owned(self.font.into_owned()),
        }
    }
}
impl<'input> Request for SetFontPathRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetFontPathRequest<'input> {
}

/// Opcode for the GetFontPath request
pub const GET_FONT_PATH_REQUEST: u8 = 52;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetFontPathRequest;
impl_debug_if_no_extra_traits!(GetFontPathRequest, "GetFontPathRequest");
impl GetFontPathRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            GET_FONT_PATH_REQUEST,
            0,
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
        if header.major_opcode != GET_FONT_PATH_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let _ = value;
        Ok(GetFontPathRequest
        )
    }
}
impl Request for GetFontPathRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetFontPathRequest {
    type Reply = GetFontPathReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetFontPathReply {
    pub sequence: u16,
    pub length: u32,
    pub path: Vec<Str>,
}
impl_debug_if_no_extra_traits!(GetFontPathReply, "GetFontPathReply");
impl TryParse for GetFontPathReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (path_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
        let (path, remaining) = crate::x11_utils::parse_list::<Str>(remaining, path_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetFontPathReply { sequence, length, path };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetFontPathReply {
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
        let path_len = u16::try_from(self.path.len()).expect("`path` has too many elements");
        path_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
        self.path.serialize_into(bytes);
    }
}
impl GetFontPathReply {
    /// Get the value of the `path_len` field.
    ///
    /// The `path_len` field is used as the length field of the `path` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn path_len(&self) -> u16 {
        self.path.len()
            .try_into().unwrap()
    }
}

/// Opcode for the CreatePixmap request
pub const CREATE_PIXMAP_REQUEST: u8 = 53;
/// Creates a pixmap.
///
/// Creates a pixmap. The pixmap can only be used on the same screen as `drawable`
/// is on and only with drawables of the same `depth`.
///
/// # Fields
///
/// * `depth` - TODO
/// * `pid` - The ID with which you will refer to the new pixmap, created by
/// `xcb_generate_id`.
/// * `drawable` - Drawable to get the screen from.
/// * `width` - The width of the new pixmap.
/// * `height` - The height of the new pixmap.
///
/// # Errors
///
/// * `Value` - TODO: reasons?
/// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
/// * `Alloc` - The X server could not allocate the requested resources (no memory?).
///
/// # See
///
/// * `xcb_generate_id`: function
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreatePixmapRequest {
    pub depth: u8,
    pub pid: Pixmap,
    pub drawable: Drawable,
    pub width: u16,
    pub height: u16,
}
impl_debug_if_no_extra_traits!(CreatePixmapRequest, "CreatePixmapRequest");
impl CreatePixmapRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let depth_bytes = self.depth.serialize();
        let pid_bytes = self.pid.serialize();
        let drawable_bytes = self.drawable.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let mut request0 = vec![
            CREATE_PIXMAP_REQUEST,
            depth_bytes[0],
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
        if header.major_opcode != CREATE_PIXMAP_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (depth, remaining) = u8::try_parse(remaining)?;
        let _ = remaining;
        let (pid, remaining) = Pixmap::try_parse(value)?;
        let (drawable, remaining) = Drawable::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(CreatePixmapRequest {
            depth,
            pid,
            drawable,
            width,
            height,
        })
    }
}
impl Request for CreatePixmapRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CreatePixmapRequest {
}

/// Opcode for the FreePixmap request
pub const FREE_PIXMAP_REQUEST: u8 = 54;
/// Destroys a pixmap.
///
/// Deletes the association between the pixmap ID and the pixmap. The pixmap
/// storage will be freed when there are no more references to it.
///
/// # Fields
///
/// * `pixmap` - The pixmap to destroy.
///
/// # Errors
///
/// * `Pixmap` - The specified pixmap does not exist.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FreePixmapRequest {
    pub pixmap: Pixmap,
}
impl_debug_if_no_extra_traits!(FreePixmapRequest, "FreePixmapRequest");
impl FreePixmapRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let pixmap_bytes = self.pixmap.serialize();
        let mut request0 = vec![
            FREE_PIXMAP_REQUEST,
            0,
            0,
            0,
            pixmap_bytes[0],
            pixmap_bytes[1],
            pixmap_bytes[2],
            pixmap_bytes[3],
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
        if header.major_opcode != FREE_PIXMAP_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (pixmap, remaining) = Pixmap::try_parse(value)?;
        let _ = remaining;
        Ok(FreePixmapRequest {
            pixmap,
        })
    }
}
impl Request for FreePixmapRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for FreePixmapRequest {
}

/// # Fields
///
/// * `Function` - TODO: Refer to GX
/// * `PlaneMask` - In graphics operations, given a source and destination pixel, the result is
/// computed bitwise on corresponding bits of the pixels; that is, a Boolean
/// operation is performed in each bit plane. The plane-mask restricts the
/// operation to a subset of planes, so the result is:
///
/// ((src FUNC dst) AND plane-mask) OR (dst AND (NOT plane-mask))
/// * `Foreground` - Foreground colorpixel.
/// * `Background` - Background colorpixel.
/// * `LineWidth` - The line-width is measured in pixels and can be greater than or equal to one, a wide line, or the
/// special value zero, a thin line.
/// * `LineStyle` - The line-style defines which sections of a line are drawn:
/// Solid                The full path of the line is drawn.
/// DoubleDash           The full path of the line is drawn, but the even dashes are filled differently
/// than the odd dashes (see fill-style), with Butt cap-style used where even and
/// odd dashes meet.
/// OnOffDash            Only the even dashes are drawn, and cap-style applies to all internal ends of
/// the individual dashes (except NotLast is treated as Butt).
/// * `CapStyle` - The cap-style defines how the endpoints of a path are drawn:
/// NotLast    The result is equivalent to Butt, except that for a line-width of zero the final
/// endpoint is not drawn.
/// Butt       The result is square at the endpoint (perpendicular to the slope of the line)
/// with no projection beyond.
/// Round      The result is a circular arc with its diameter equal to the line-width, centered
/// on the endpoint; it is equivalent to Butt for line-width zero.
/// Projecting The result is square at the end, but the path continues beyond the endpoint for
/// a distance equal to half the line-width; it is equivalent to Butt for line-width
/// zero.
/// * `JoinStyle` - The join-style defines how corners are drawn for wide lines:
/// Miter               The outer edges of the two lines extend to meet at an angle. However, if the
/// angle is less than 11 degrees, a Bevel join-style is used instead.
/// Round               The result is a circular arc with a diameter equal to the line-width, centered
/// on the joinpoint.
/// Bevel               The result is Butt endpoint styles, and then the triangular notch is filled.
/// * `FillStyle` - The fill-style defines the contents of the source for line, text, and fill requests. For all text and fill
/// requests (for example, PolyText8, PolyText16, PolyFillRectangle, FillPoly, and PolyFillArc)
/// as well as for line requests with line-style Solid, (for example, PolyLine, PolySegment,
/// PolyRectangle, PolyArc) and for the even dashes for line requests with line-style OnOffDash
/// or DoubleDash:
/// Solid                     Foreground
/// Tiled                     Tile
/// OpaqueStippled            A tile with the same width and height as stipple but with background
/// everywhere stipple has a zero and with foreground everywhere stipple
/// has a one
/// Stippled                  Foreground masked by stipple
/// For the odd dashes for line requests with line-style DoubleDash:
/// Solid                     Background
/// Tiled                     Same as for even dashes
/// OpaqueStippled            Same as for even dashes
/// Stippled                  Background masked by stipple
/// * `FillRule` -
/// * `Tile` - The tile/stipple represents an infinite two-dimensional plane with the tile/stipple replicated in all
/// dimensions. When that plane is superimposed on the drawable for use in a graphics operation,
/// the upper-left corner of some instance of the tile/stipple is at the coordinates within the drawable
/// specified by the tile/stipple origin. The tile/stipple and clip origins are interpreted relative to the
/// origin of whatever destination drawable is specified in a graphics request.
/// The tile pixmap must have the same root and depth as the gcontext (or a Match error results).
/// The stipple pixmap must have depth one and must have the same root as the gcontext (or a
/// Match error results). For fill-style Stippled (but not fill-style
/// OpaqueStippled), the stipple pattern is tiled in a single plane and acts as an
/// additional clip mask to be ANDed with the clip-mask.
/// Any size pixmap can be used for tiling or stippling, although some sizes may be faster to use than
/// others.
/// * `Stipple` - The tile/stipple represents an infinite two-dimensional plane with the tile/stipple replicated in all
/// dimensions. When that plane is superimposed on the drawable for use in a graphics operation,
/// the upper-left corner of some instance of the tile/stipple is at the coordinates within the drawable
/// specified by the tile/stipple origin. The tile/stipple and clip origins are interpreted relative to the
/// origin of whatever destination drawable is specified in a graphics request.
/// The tile pixmap must have the same root and depth as the gcontext (or a Match error results).
/// The stipple pixmap must have depth one and must have the same root as the gcontext (or a
/// Match error results). For fill-style Stippled (but not fill-style
/// OpaqueStippled), the stipple pattern is tiled in a single plane and acts as an
/// additional clip mask to be ANDed with the clip-mask.
/// Any size pixmap can be used for tiling or stippling, although some sizes may be faster to use than
/// others.
/// * `TileStippleOriginX` - TODO
/// * `TileStippleOriginY` - TODO
/// * `Font` - Which font to use for the `ImageText8` and `ImageText16` requests.
/// * `SubwindowMode` - For ClipByChildren, both source and destination windows are additionally
/// clipped by all viewable InputOutput children. For IncludeInferiors, neither
/// source nor destination window is
/// clipped by inferiors. This will result in including subwindow contents in the source and drawing
/// through subwindow boundaries of the destination. The use of IncludeInferiors with a source or
/// destination window of one depth with mapped inferiors of differing depth is not illegal, but the
/// semantics is undefined by the core protocol.
/// * `GraphicsExposures` - Whether ExposureEvents should be generated (1) or not (0).
///
/// The default is 1.
/// * `ClipOriginX` - TODO
/// * `ClipOriginY` - TODO
/// * `ClipMask` - The clip-mask restricts writes to the destination drawable. Only pixels where the clip-mask has
/// bits set to 1 are drawn. Pixels are not drawn outside the area covered by the clip-mask or where
/// the clip-mask has bits set to 0. The clip-mask affects all graphics requests, but it does not clip
/// sources. The clip-mask origin is interpreted relative to the origin of whatever destination drawable is specified in a graphics request. If a pixmap is specified as the clip-mask, it must have
/// depth 1 and have the same root as the gcontext (or a Match error results). If clip-mask is None,
/// then pixels are always drawn, regardless of the clip origin. The clip-mask can also be set with the
/// SetClipRectangles request.
/// * `DashOffset` - TODO
/// * `DashList` - TODO
/// * `ArcMode` - TODO
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GC(u32);
impl GC {
    pub const FUNCTION: Self = Self(1 << 0);
    pub const PLANE_MASK: Self = Self(1 << 1);
    pub const FOREGROUND: Self = Self(1 << 2);
    pub const BACKGROUND: Self = Self(1 << 3);
    pub const LINE_WIDTH: Self = Self(1 << 4);
    pub const LINE_STYLE: Self = Self(1 << 5);
    pub const CAP_STYLE: Self = Self(1 << 6);
    pub const JOIN_STYLE: Self = Self(1 << 7);
    pub const FILL_STYLE: Self = Self(1 << 8);
    pub const FILL_RULE: Self = Self(1 << 9);
    pub const TILE: Self = Self(1 << 10);
    pub const STIPPLE: Self = Self(1 << 11);
    pub const TILE_STIPPLE_ORIGIN_X: Self = Self(1 << 12);
    pub const TILE_STIPPLE_ORIGIN_Y: Self = Self(1 << 13);
    pub const FONT: Self = Self(1 << 14);
    pub const SUBWINDOW_MODE: Self = Self(1 << 15);
    pub const GRAPHICS_EXPOSURES: Self = Self(1 << 16);
    pub const CLIP_ORIGIN_X: Self = Self(1 << 17);
    pub const CLIP_ORIGIN_Y: Self = Self(1 << 18);
    pub const CLIP_MASK: Self = Self(1 << 19);
    pub const DASH_OFFSET: Self = Self(1 << 20);
    pub const DASH_LIST: Self = Self(1 << 21);
    pub const ARC_MODE: Self = Self(1 << 22);
}
impl From<GC> for u32 {
    #[inline]
    fn from(input: GC) -> Self {
        input.0
    }
}
impl From<GC> for Option<u32> {
    #[inline]
    fn from(input: GC) -> Self {
        Some(input.0)
    }
}
impl From<u8> for GC {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for GC {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for GC {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for GC  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::FUNCTION.0, "FUNCTION", "Function"),
            (Self::PLANE_MASK.0, "PLANE_MASK", "PlaneMask"),
            (Self::FOREGROUND.0, "FOREGROUND", "Foreground"),
            (Self::BACKGROUND.0, "BACKGROUND", "Background"),
            (Self::LINE_WIDTH.0, "LINE_WIDTH", "LineWidth"),
            (Self::LINE_STYLE.0, "LINE_STYLE", "LineStyle"),
            (Self::CAP_STYLE.0, "CAP_STYLE", "CapStyle"),
            (Self::JOIN_STYLE.0, "JOIN_STYLE", "JoinStyle"),
            (Self::FILL_STYLE.0, "FILL_STYLE", "FillStyle"),
            (Self::FILL_RULE.0, "FILL_RULE", "FillRule"),
            (Self::TILE.0, "TILE", "Tile"),
            (Self::STIPPLE.0, "STIPPLE", "Stipple"),
            (Self::TILE_STIPPLE_ORIGIN_X.0, "TILE_STIPPLE_ORIGIN_X", "TileStippleOriginX"),
            (Self::TILE_STIPPLE_ORIGIN_Y.0, "TILE_STIPPLE_ORIGIN_Y", "TileStippleOriginY"),
            (Self::FONT.0, "FONT", "Font"),
            (Self::SUBWINDOW_MODE.0, "SUBWINDOW_MODE", "SubwindowMode"),
            (Self::GRAPHICS_EXPOSURES.0, "GRAPHICS_EXPOSURES", "GraphicsExposures"),
            (Self::CLIP_ORIGIN_X.0, "CLIP_ORIGIN_X", "ClipOriginX"),
            (Self::CLIP_ORIGIN_Y.0, "CLIP_ORIGIN_Y", "ClipOriginY"),
            (Self::CLIP_MASK.0, "CLIP_MASK", "ClipMask"),
            (Self::DASH_OFFSET.0, "DASH_OFFSET", "DashOffset"),
            (Self::DASH_LIST.0, "DASH_LIST", "DashList"),
            (Self::ARC_MODE.0, "ARC_MODE", "ArcMode"),
        ];
        pretty_print_bitmask(fmt, self.0, &variants)
    }
}
bitmask_binop!(GC, u32);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GX(u32);
impl GX {
    pub const CLEAR: Self = Self(0);
    pub const AND: Self = Self(1);
    pub const AND_REVERSE: Self = Self(2);
    pub const COPY: Self = Self(3);
    pub const AND_INVERTED: Self = Self(4);
    pub const NOOP: Self = Self(5);
    pub const XOR: Self = Self(6);
    pub const OR: Self = Self(7);
    pub const NOR: Self = Self(8);
    pub const EQUIV: Self = Self(9);
    pub const INVERT: Self = Self(10);
    pub const OR_REVERSE: Self = Self(11);
    pub const COPY_INVERTED: Self = Self(12);
    pub const OR_INVERTED: Self = Self(13);
    pub const NAND: Self = Self(14);
    pub const SET: Self = Self(15);
}
impl From<GX> for u32 {
    #[inline]
    fn from(input: GX) -> Self {
        input.0
    }
}
impl From<GX> for Option<u32> {
    #[inline]
    fn from(input: GX) -> Self {
        Some(input.0)
    }
}
impl From<u8> for GX {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for GX {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for GX {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for GX  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::CLEAR.0, "CLEAR", "Clear"),
            (Self::AND.0, "AND", "And"),
            (Self::AND_REVERSE.0, "AND_REVERSE", "AndReverse"),
            (Self::COPY.0, "COPY", "Copy"),
            (Self::AND_INVERTED.0, "AND_INVERTED", "AndInverted"),
            (Self::NOOP.0, "NOOP", "Noop"),
            (Self::XOR.0, "XOR", "Xor"),
            (Self::OR.0, "OR", "Or"),
            (Self::NOR.0, "NOR", "Nor"),
            (Self::EQUIV.0, "EQUIV", "Equiv"),
            (Self::INVERT.0, "INVERT", "Invert"),
            (Self::OR_REVERSE.0, "OR_REVERSE", "OrReverse"),
            (Self::COPY_INVERTED.0, "COPY_INVERTED", "CopyInverted"),
            (Self::OR_INVERTED.0, "OR_INVERTED", "OrInverted"),
            (Self::NAND.0, "NAND", "Nand"),
            (Self::SET.0, "SET", "Set"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LineStyle(u32);
impl LineStyle {
    pub const SOLID: Self = Self(0);
    pub const ON_OFF_DASH: Self = Self(1);
    pub const DOUBLE_DASH: Self = Self(2);
}
impl From<LineStyle> for u32 {
    #[inline]
    fn from(input: LineStyle) -> Self {
        input.0
    }
}
impl From<LineStyle> for Option<u32> {
    #[inline]
    fn from(input: LineStyle) -> Self {
        Some(input.0)
    }
}
impl From<u8> for LineStyle {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for LineStyle {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for LineStyle {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for LineStyle  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SOLID.0, "SOLID", "Solid"),
            (Self::ON_OFF_DASH.0, "ON_OFF_DASH", "OnOffDash"),
            (Self::DOUBLE_DASH.0, "DOUBLE_DASH", "DoubleDash"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapStyle(u32);
impl CapStyle {
    pub const NOT_LAST: Self = Self(0);
    pub const BUTT: Self = Self(1);
    pub const ROUND: Self = Self(2);
    pub const PROJECTING: Self = Self(3);
}
impl From<CapStyle> for u32 {
    #[inline]
    fn from(input: CapStyle) -> Self {
        input.0
    }
}
impl From<CapStyle> for Option<u32> {
    #[inline]
    fn from(input: CapStyle) -> Self {
        Some(input.0)
    }
}
impl From<u8> for CapStyle {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for CapStyle {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for CapStyle {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for CapStyle  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NOT_LAST.0, "NOT_LAST", "NotLast"),
            (Self::BUTT.0, "BUTT", "Butt"),
            (Self::ROUND.0, "ROUND", "Round"),
            (Self::PROJECTING.0, "PROJECTING", "Projecting"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct JoinStyle(u32);
impl JoinStyle {
    pub const MITER: Self = Self(0);
    pub const ROUND: Self = Self(1);
    pub const BEVEL: Self = Self(2);
}
impl From<JoinStyle> for u32 {
    #[inline]
    fn from(input: JoinStyle) -> Self {
        input.0
    }
}
impl From<JoinStyle> for Option<u32> {
    #[inline]
    fn from(input: JoinStyle) -> Self {
        Some(input.0)
    }
}
impl From<u8> for JoinStyle {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for JoinStyle {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for JoinStyle {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for JoinStyle  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::MITER.0, "MITER", "Miter"),
            (Self::ROUND.0, "ROUND", "Round"),
            (Self::BEVEL.0, "BEVEL", "Bevel"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FillStyle(u32);
impl FillStyle {
    pub const SOLID: Self = Self(0);
    pub const TILED: Self = Self(1);
    pub const STIPPLED: Self = Self(2);
    pub const OPAQUE_STIPPLED: Self = Self(3);
}
impl From<FillStyle> for u32 {
    #[inline]
    fn from(input: FillStyle) -> Self {
        input.0
    }
}
impl From<FillStyle> for Option<u32> {
    #[inline]
    fn from(input: FillStyle) -> Self {
        Some(input.0)
    }
}
impl From<u8> for FillStyle {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for FillStyle {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for FillStyle {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for FillStyle  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SOLID.0, "SOLID", "Solid"),
            (Self::TILED.0, "TILED", "Tiled"),
            (Self::STIPPLED.0, "STIPPLED", "Stippled"),
            (Self::OPAQUE_STIPPLED.0, "OPAQUE_STIPPLED", "OpaqueStippled"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FillRule(u32);
impl FillRule {
    pub const EVEN_ODD: Self = Self(0);
    pub const WINDING: Self = Self(1);
}
impl From<FillRule> for u32 {
    #[inline]
    fn from(input: FillRule) -> Self {
        input.0
    }
}
impl From<FillRule> for Option<u32> {
    #[inline]
    fn from(input: FillRule) -> Self {
        Some(input.0)
    }
}
impl From<u8> for FillRule {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for FillRule {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for FillRule {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for FillRule  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::EVEN_ODD.0, "EVEN_ODD", "EvenOdd"),
            (Self::WINDING.0, "WINDING", "Winding"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SubwindowMode(u32);
impl SubwindowMode {
    pub const CLIP_BY_CHILDREN: Self = Self(0);
    pub const INCLUDE_INFERIORS: Self = Self(1);
}
impl From<SubwindowMode> for u32 {
    #[inline]
    fn from(input: SubwindowMode) -> Self {
        input.0
    }
}
impl From<SubwindowMode> for Option<u32> {
    #[inline]
    fn from(input: SubwindowMode) -> Self {
        Some(input.0)
    }
}
impl From<u8> for SubwindowMode {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for SubwindowMode {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for SubwindowMode {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SubwindowMode  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::CLIP_BY_CHILDREN.0, "CLIP_BY_CHILDREN", "ClipByChildren"),
            (Self::INCLUDE_INFERIORS.0, "INCLUDE_INFERIORS", "IncludeInferiors"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArcMode(u32);
impl ArcMode {
    pub const CHORD: Self = Self(0);
    pub const PIE_SLICE: Self = Self(1);
}
impl From<ArcMode> for u32 {
    #[inline]
    fn from(input: ArcMode) -> Self {
        input.0
    }
}
impl From<ArcMode> for Option<u32> {
    #[inline]
    fn from(input: ArcMode) -> Self {
        Some(input.0)
    }
}
impl From<u8> for ArcMode {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for ArcMode {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for ArcMode {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ArcMode  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::CHORD.0, "CHORD", "Chord"),
            (Self::PIE_SLICE.0, "PIE_SLICE", "PieSlice"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

/// Auxiliary and optional information for the `create_gc` function
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateGCAux {
    pub function: Option<GX>,
    pub plane_mask: Option<u32>,
    pub foreground: Option<u32>,
    pub background: Option<u32>,
    pub line_width: Option<u32>,
    pub line_style: Option<LineStyle>,
    pub cap_style: Option<CapStyle>,
    pub join_style: Option<JoinStyle>,
    pub fill_style: Option<FillStyle>,
    pub fill_rule: Option<FillRule>,
    pub tile: Option<Pixmap>,
    pub stipple: Option<Pixmap>,
    pub tile_stipple_x_origin: Option<i32>,
    pub tile_stipple_y_origin: Option<i32>,
    pub font: Option<Font>,
    pub subwindow_mode: Option<SubwindowMode>,
    pub graphics_exposures: Option<Bool32>,
    pub clip_x_origin: Option<i32>,
    pub clip_y_origin: Option<i32>,
    pub clip_mask: Option<Pixmap>,
    pub dash_offset: Option<u32>,
    pub dashes: Option<u32>,
    pub arc_mode: Option<ArcMode>,
}
impl_debug_if_no_extra_traits!(CreateGCAux, "CreateGCAux");
impl CreateGCAux {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], value_mask: u32) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u32::from(value_mask);
        let mut outer_remaining = value;
        let function = if switch_expr & u32::from(GC::FUNCTION) != 0 {
            let remaining = outer_remaining;
            let (function, remaining) = u32::try_parse(remaining)?;
            let function = function.into();
            outer_remaining = remaining;
            Some(function)
        } else {
            None
        };
        let plane_mask = if switch_expr & u32::from(GC::PLANE_MASK) != 0 {
            let remaining = outer_remaining;
            let (plane_mask, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(plane_mask)
        } else {
            None
        };
        let foreground = if switch_expr & u32::from(GC::FOREGROUND) != 0 {
            let remaining = outer_remaining;
            let (foreground, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(foreground)
        } else {
            None
        };
        let background = if switch_expr & u32::from(GC::BACKGROUND) != 0 {
            let remaining = outer_remaining;
            let (background, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(background)
        } else {
            None
        };
        let line_width = if switch_expr & u32::from(GC::LINE_WIDTH) != 0 {
            let remaining = outer_remaining;
            let (line_width, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(line_width)
        } else {
            None
        };
        let line_style = if switch_expr & u32::from(GC::LINE_STYLE) != 0 {
            let remaining = outer_remaining;
            let (line_style, remaining) = u32::try_parse(remaining)?;
            let line_style = line_style.into();
            outer_remaining = remaining;
            Some(line_style)
        } else {
            None
        };
        let cap_style = if switch_expr & u32::from(GC::CAP_STYLE) != 0 {
            let remaining = outer_remaining;
            let (cap_style, remaining) = u32::try_parse(remaining)?;
            let cap_style = cap_style.into();
            outer_remaining = remaining;
            Some(cap_style)
        } else {
            None
        };
        let join_style = if switch_expr & u32::from(GC::JOIN_STYLE) != 0 {
            let remaining = outer_remaining;
            let (join_style, remaining) = u32::try_parse(remaining)?;
            let join_style = join_style.into();
            outer_remaining = remaining;
            Some(join_style)
        } else {
            None
        };
        let fill_style = if switch_expr & u32::from(GC::FILL_STYLE) != 0 {
            let remaining = outer_remaining;
            let (fill_style, remaining) = u32::try_parse(remaining)?;
            let fill_style = fill_style.into();
            outer_remaining = remaining;
            Some(fill_style)
        } else {
            None
        };
        let fill_rule = if switch_expr & u32::from(GC::FILL_RULE) != 0 {
            let remaining = outer_remaining;
            let (fill_rule, remaining) = u32::try_parse(remaining)?;
            let fill_rule = fill_rule.into();
            outer_remaining = remaining;
            Some(fill_rule)
        } else {
            None
        };
        let tile = if switch_expr & u32::from(GC::TILE) != 0 {
            let remaining = outer_remaining;
            let (tile, remaining) = Pixmap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(tile)
        } else {
            None
        };
        let stipple = if switch_expr & u32::from(GC::STIPPLE) != 0 {
            let remaining = outer_remaining;
            let (stipple, remaining) = Pixmap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(stipple)
        } else {
            None
        };
        let tile_stipple_x_origin = if switch_expr & u32::from(GC::TILE_STIPPLE_ORIGIN_X) != 0 {
            let remaining = outer_remaining;
            let (tile_stipple_x_origin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(tile_stipple_x_origin)
        } else {
            None
        };
        let tile_stipple_y_origin = if switch_expr & u32::from(GC::TILE_STIPPLE_ORIGIN_Y) != 0 {
            let remaining = outer_remaining;
            let (tile_stipple_y_origin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(tile_stipple_y_origin)
        } else {
            None
        };
        let font = if switch_expr & u32::from(GC::FONT) != 0 {
            let remaining = outer_remaining;
            let (font, remaining) = Font::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(font)
        } else {
            None
        };
        let subwindow_mode = if switch_expr & u32::from(GC::SUBWINDOW_MODE) != 0 {
            let remaining = outer_remaining;
            let (subwindow_mode, remaining) = u32::try_parse(remaining)?;
            let subwindow_mode = subwindow_mode.into();
            outer_remaining = remaining;
            Some(subwindow_mode)
        } else {
            None
        };
        let graphics_exposures = if switch_expr & u32::from(GC::GRAPHICS_EXPOSURES) != 0 {
            let remaining = outer_remaining;
            let (graphics_exposures, remaining) = Bool32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(graphics_exposures)
        } else {
            None
        };
        let clip_x_origin = if switch_expr & u32::from(GC::CLIP_ORIGIN_X) != 0 {
            let remaining = outer_remaining;
            let (clip_x_origin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(clip_x_origin)
        } else {
            None
        };
        let clip_y_origin = if switch_expr & u32::from(GC::CLIP_ORIGIN_Y) != 0 {
            let remaining = outer_remaining;
            let (clip_y_origin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(clip_y_origin)
        } else {
            None
        };
        let clip_mask = if switch_expr & u32::from(GC::CLIP_MASK) != 0 {
            let remaining = outer_remaining;
            let (clip_mask, remaining) = Pixmap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(clip_mask)
        } else {
            None
        };
        let dash_offset = if switch_expr & u32::from(GC::DASH_OFFSET) != 0 {
            let remaining = outer_remaining;
            let (dash_offset, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(dash_offset)
        } else {
            None
        };
        let dashes = if switch_expr & u32::from(GC::DASH_LIST) != 0 {
            let remaining = outer_remaining;
            let (dashes, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(dashes)
        } else {
            None
        };
        let arc_mode = if switch_expr & u32::from(GC::ARC_MODE) != 0 {
            let remaining = outer_remaining;
            let (arc_mode, remaining) = u32::try_parse(remaining)?;
            let arc_mode = arc_mode.into();
            outer_remaining = remaining;
            Some(arc_mode)
        } else {
            None
        };
        let result = CreateGCAux { function, plane_mask, foreground, background, line_width, line_style, cap_style, join_style, fill_style, fill_rule, tile, stipple, tile_stipple_x_origin, tile_stipple_y_origin, font, subwindow_mode, graphics_exposures, clip_x_origin, clip_y_origin, clip_mask, dash_offset, dashes, arc_mode };
        Ok((result, outer_remaining))
    }
}
impl CreateGCAux {
    #[allow(dead_code)]
    fn serialize(&self, value_mask: u32) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u32::from(value_mask));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, value_mask: u32) {
        assert_eq!(self.switch_expr(), u32::from(value_mask), "switch `value_list` has an inconsistent discriminant");
        if let Some(function) = self.function {
            u32::from(function).serialize_into(bytes);
        }
        if let Some(plane_mask) = self.plane_mask {
            plane_mask.serialize_into(bytes);
        }
        if let Some(foreground) = self.foreground {
            foreground.serialize_into(bytes);
        }
        if let Some(background) = self.background {
            background.serialize_into(bytes);
        }
        if let Some(line_width) = self.line_width {
            line_width.serialize_into(bytes);
        }
        if let Some(line_style) = self.line_style {
            u32::from(line_style).serialize_into(bytes);
        }
        if let Some(cap_style) = self.cap_style {
            u32::from(cap_style).serialize_into(bytes);
        }
        if let Some(join_style) = self.join_style {
            u32::from(join_style).serialize_into(bytes);
        }
        if let Some(fill_style) = self.fill_style {
            u32::from(fill_style).serialize_into(bytes);
        }
        if let Some(fill_rule) = self.fill_rule {
            u32::from(fill_rule).serialize_into(bytes);
        }
        if let Some(tile) = self.tile {
            tile.serialize_into(bytes);
        }
        if let Some(stipple) = self.stipple {
            stipple.serialize_into(bytes);
        }
        if let Some(tile_stipple_x_origin) = self.tile_stipple_x_origin {
            tile_stipple_x_origin.serialize_into(bytes);
        }
        if let Some(tile_stipple_y_origin) = self.tile_stipple_y_origin {
            tile_stipple_y_origin.serialize_into(bytes);
        }
        if let Some(font) = self.font {
            font.serialize_into(bytes);
        }
        if let Some(subwindow_mode) = self.subwindow_mode {
            u32::from(subwindow_mode).serialize_into(bytes);
        }
        if let Some(graphics_exposures) = self.graphics_exposures {
            graphics_exposures.serialize_into(bytes);
        }
        if let Some(clip_x_origin) = self.clip_x_origin {
            clip_x_origin.serialize_into(bytes);
        }
        if let Some(clip_y_origin) = self.clip_y_origin {
            clip_y_origin.serialize_into(bytes);
        }
        if let Some(clip_mask) = self.clip_mask {
            clip_mask.serialize_into(bytes);
        }
        if let Some(dash_offset) = self.dash_offset {
            dash_offset.serialize_into(bytes);
        }
        if let Some(dashes) = self.dashes {
            dashes.serialize_into(bytes);
        }
        if let Some(arc_mode) = self.arc_mode {
            u32::from(arc_mode).serialize_into(bytes);
        }
    }
}
impl CreateGCAux {
    fn switch_expr(&self) -> u32 {
        let mut expr_value = 0;
        if self.function.is_some() {
            expr_value |= u32::from(GC::FUNCTION);
        }
        if self.plane_mask.is_some() {
            expr_value |= u32::from(GC::PLANE_MASK);
        }
        if self.foreground.is_some() {
            expr_value |= u32::from(GC::FOREGROUND);
        }
        if self.background.is_some() {
            expr_value |= u32::from(GC::BACKGROUND);
        }
        if self.line_width.is_some() {
            expr_value |= u32::from(GC::LINE_WIDTH);
        }
        if self.line_style.is_some() {
            expr_value |= u32::from(GC::LINE_STYLE);
        }
        if self.cap_style.is_some() {
            expr_value |= u32::from(GC::CAP_STYLE);
        }
        if self.join_style.is_some() {
            expr_value |= u32::from(GC::JOIN_STYLE);
        }
        if self.fill_style.is_some() {
            expr_value |= u32::from(GC::FILL_STYLE);
        }
        if self.fill_rule.is_some() {
            expr_value |= u32::from(GC::FILL_RULE);
        }
        if self.tile.is_some() {
            expr_value |= u32::from(GC::TILE);
        }
        if self.stipple.is_some() {
            expr_value |= u32::from(GC::STIPPLE);
        }
        if self.tile_stipple_x_origin.is_some() {
            expr_value |= u32::from(GC::TILE_STIPPLE_ORIGIN_X);
        }
        if self.tile_stipple_y_origin.is_some() {
            expr_value |= u32::from(GC::TILE_STIPPLE_ORIGIN_Y);
        }
        if self.font.is_some() {
            expr_value |= u32::from(GC::FONT);
        }
        if self.subwindow_mode.is_some() {
            expr_value |= u32::from(GC::SUBWINDOW_MODE);
        }
        if self.graphics_exposures.is_some() {
            expr_value |= u32::from(GC::GRAPHICS_EXPOSURES);
        }
        if self.clip_x_origin.is_some() {
            expr_value |= u32::from(GC::CLIP_ORIGIN_X);
        }
        if self.clip_y_origin.is_some() {
            expr_value |= u32::from(GC::CLIP_ORIGIN_Y);
        }
        if self.clip_mask.is_some() {
            expr_value |= u32::from(GC::CLIP_MASK);
        }
        if self.dash_offset.is_some() {
            expr_value |= u32::from(GC::DASH_OFFSET);
        }
        if self.dashes.is_some() {
            expr_value |= u32::from(GC::DASH_LIST);
        }
        if self.arc_mode.is_some() {
            expr_value |= u32::from(GC::ARC_MODE);
        }
        expr_value
    }
}
impl CreateGCAux {
    /// Create a new instance with all fields unset / not present.
    pub fn new() -> Self {
        Default::default()
    }
    /// Set the `function` field of this structure.
    #[must_use]
    pub fn function<I>(mut self, value: I) -> Self where I: Into<Option<GX>> {
        self.function = value.into();
        self
    }
    /// Set the `plane_mask` field of this structure.
    #[must_use]
    pub fn plane_mask<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.plane_mask = value.into();
        self
    }
    /// Set the `foreground` field of this structure.
    #[must_use]
    pub fn foreground<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.foreground = value.into();
        self
    }
    /// Set the `background` field of this structure.
    #[must_use]
    pub fn background<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.background = value.into();
        self
    }
    /// Set the `line_width` field of this structure.
    #[must_use]
    pub fn line_width<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.line_width = value.into();
        self
    }
    /// Set the `line_style` field of this structure.
    #[must_use]
    pub fn line_style<I>(mut self, value: I) -> Self where I: Into<Option<LineStyle>> {
        self.line_style = value.into();
        self
    }
    /// Set the `cap_style` field of this structure.
    #[must_use]
    pub fn cap_style<I>(mut self, value: I) -> Self where I: Into<Option<CapStyle>> {
        self.cap_style = value.into();
        self
    }
    /// Set the `join_style` field of this structure.
    #[must_use]
    pub fn join_style<I>(mut self, value: I) -> Self where I: Into<Option<JoinStyle>> {
        self.join_style = value.into();
        self
    }
    /// Set the `fill_style` field of this structure.
    #[must_use]
    pub fn fill_style<I>(mut self, value: I) -> Self where I: Into<Option<FillStyle>> {
        self.fill_style = value.into();
        self
    }
    /// Set the `fill_rule` field of this structure.
    #[must_use]
    pub fn fill_rule<I>(mut self, value: I) -> Self where I: Into<Option<FillRule>> {
        self.fill_rule = value.into();
        self
    }
    /// Set the `tile` field of this structure.
    #[must_use]
    pub fn tile<I>(mut self, value: I) -> Self where I: Into<Option<Pixmap>> {
        self.tile = value.into();
        self
    }
    /// Set the `stipple` field of this structure.
    #[must_use]
    pub fn stipple<I>(mut self, value: I) -> Self where I: Into<Option<Pixmap>> {
        self.stipple = value.into();
        self
    }
    /// Set the `tile_stipple_x_origin` field of this structure.
    #[must_use]
    pub fn tile_stipple_x_origin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.tile_stipple_x_origin = value.into();
        self
    }
    /// Set the `tile_stipple_y_origin` field of this structure.
    #[must_use]
    pub fn tile_stipple_y_origin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.tile_stipple_y_origin = value.into();
        self
    }
    /// Set the `font` field of this structure.
    #[must_use]
    pub fn font<I>(mut self, value: I) -> Self where I: Into<Option<Font>> {
        self.font = value.into();
        self
    }
    /// Set the `subwindow_mode` field of this structure.
    #[must_use]
    pub fn subwindow_mode<I>(mut self, value: I) -> Self where I: Into<Option<SubwindowMode>> {
        self.subwindow_mode = value.into();
        self
    }
    /// Set the `graphics_exposures` field of this structure.
    #[must_use]
    pub fn graphics_exposures<I>(mut self, value: I) -> Self where I: Into<Option<Bool32>> {
        self.graphics_exposures = value.into();
        self
    }
    /// Set the `clip_x_origin` field of this structure.
    #[must_use]
    pub fn clip_x_origin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.clip_x_origin = value.into();
        self
    }
    /// Set the `clip_y_origin` field of this structure.
    #[must_use]
    pub fn clip_y_origin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.clip_y_origin = value.into();
        self
    }
    /// Set the `clip_mask` field of this structure.
    #[must_use]
    pub fn clip_mask<I>(mut self, value: I) -> Self where I: Into<Option<Pixmap>> {
        self.clip_mask = value.into();
        self
    }
    /// Set the `dash_offset` field of this structure.
    #[must_use]
    pub fn dash_offset<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.dash_offset = value.into();
        self
    }
    /// Set the `dashes` field of this structure.
    #[must_use]
    pub fn dashes<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.dashes = value.into();
        self
    }
    /// Set the `arc_mode` field of this structure.
    #[must_use]
    pub fn arc_mode<I>(mut self, value: I) -> Self where I: Into<Option<ArcMode>> {
        self.arc_mode = value.into();
        self
    }
}

/// Opcode for the CreateGC request
pub const CREATE_GC_REQUEST: u8 = 55;
/// Creates a graphics context.
///
/// Creates a graphics context. The graphics context can be used with any drawable
/// that has the same root and depth as the specified drawable.
///
/// # Fields
///
/// * `cid` - The ID with which you will refer to the graphics context, created by
/// `xcb_generate_id`.
/// * `drawable` - Drawable to get the root/depth from.
///
/// # Errors
///
/// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
/// * `Match` - TODO: reasons?
/// * `Font` - TODO: reasons?
/// * `Pixmap` - TODO: reasons?
/// * `Value` - TODO: reasons?
/// * `Alloc` - The X server could not allocate the requested resources (no memory?).
///
/// # See
///
/// * `xcb_generate_id`: function
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateGCRequest<'input> {
    pub cid: Gcontext,
    pub drawable: Drawable,
    pub value_list: Cow<'input, CreateGCAux>,
}
impl_debug_if_no_extra_traits!(CreateGCRequest<'_>, "CreateGCRequest");
impl<'input> CreateGCRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let cid_bytes = self.cid.serialize();
        let drawable_bytes = self.drawable.serialize();
        let value_mask: u32 = self.value_list.switch_expr();
        let value_mask_bytes = value_mask.serialize();
        let mut request0 = vec![
            CREATE_GC_REQUEST,
            0,
            0,
            0,
            cid_bytes[0],
            cid_bytes[1],
            cid_bytes[2],
            cid_bytes[3],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
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
        if header.major_opcode != CREATE_GC_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cid, remaining) = Gcontext::try_parse(value)?;
        let (drawable, remaining) = Drawable::try_parse(remaining)?;
        let (value_mask, remaining) = u32::try_parse(remaining)?;
        let (value_list, remaining) = CreateGCAux::try_parse(remaining, u32::from(value_mask))?;
        let _ = remaining;
        Ok(CreateGCRequest {
            cid,
            drawable,
            value_list: Cow::Owned(value_list),
        })
    }
    /// Clone all borrowed data in this CreateGCRequest.
    pub fn into_owned(self) -> CreateGCRequest<'static> {
        CreateGCRequest {
            cid: self.cid,
            drawable: self.drawable,
            value_list: Cow::Owned(self.value_list.into_owned()),
        }
    }
}
impl<'input> Request for CreateGCRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for CreateGCRequest<'input> {
}

/// Auxiliary and optional information for the `change_gc` function
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeGCAux {
    pub function: Option<GX>,
    pub plane_mask: Option<u32>,
    pub foreground: Option<u32>,
    pub background: Option<u32>,
    pub line_width: Option<u32>,
    pub line_style: Option<LineStyle>,
    pub cap_style: Option<CapStyle>,
    pub join_style: Option<JoinStyle>,
    pub fill_style: Option<FillStyle>,
    pub fill_rule: Option<FillRule>,
    pub tile: Option<Pixmap>,
    pub stipple: Option<Pixmap>,
    pub tile_stipple_x_origin: Option<i32>,
    pub tile_stipple_y_origin: Option<i32>,
    pub font: Option<Font>,
    pub subwindow_mode: Option<SubwindowMode>,
    pub graphics_exposures: Option<Bool32>,
    pub clip_x_origin: Option<i32>,
    pub clip_y_origin: Option<i32>,
    pub clip_mask: Option<Pixmap>,
    pub dash_offset: Option<u32>,
    pub dashes: Option<u32>,
    pub arc_mode: Option<ArcMode>,
}
impl_debug_if_no_extra_traits!(ChangeGCAux, "ChangeGCAux");
impl ChangeGCAux {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], value_mask: u32) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u32::from(value_mask);
        let mut outer_remaining = value;
        let function = if switch_expr & u32::from(GC::FUNCTION) != 0 {
            let remaining = outer_remaining;
            let (function, remaining) = u32::try_parse(remaining)?;
            let function = function.into();
            outer_remaining = remaining;
            Some(function)
        } else {
            None
        };
        let plane_mask = if switch_expr & u32::from(GC::PLANE_MASK) != 0 {
            let remaining = outer_remaining;
            let (plane_mask, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(plane_mask)
        } else {
            None
        };
        let foreground = if switch_expr & u32::from(GC::FOREGROUND) != 0 {
            let remaining = outer_remaining;
            let (foreground, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(foreground)
        } else {
            None
        };
        let background = if switch_expr & u32::from(GC::BACKGROUND) != 0 {
            let remaining = outer_remaining;
            let (background, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(background)
        } else {
            None
        };
        let line_width = if switch_expr & u32::from(GC::LINE_WIDTH) != 0 {
            let remaining = outer_remaining;
            let (line_width, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(line_width)
        } else {
            None
        };
        let line_style = if switch_expr & u32::from(GC::LINE_STYLE) != 0 {
            let remaining = outer_remaining;
            let (line_style, remaining) = u32::try_parse(remaining)?;
            let line_style = line_style.into();
            outer_remaining = remaining;
            Some(line_style)
        } else {
            None
        };
        let cap_style = if switch_expr & u32::from(GC::CAP_STYLE) != 0 {
            let remaining = outer_remaining;
            let (cap_style, remaining) = u32::try_parse(remaining)?;
            let cap_style = cap_style.into();
            outer_remaining = remaining;
            Some(cap_style)
        } else {
            None
        };
        let join_style = if switch_expr & u32::from(GC::JOIN_STYLE) != 0 {
            let remaining = outer_remaining;
            let (join_style, remaining) = u32::try_parse(remaining)?;
            let join_style = join_style.into();
            outer_remaining = remaining;
            Some(join_style)
        } else {
            None
        };
        let fill_style = if switch_expr & u32::from(GC::FILL_STYLE) != 0 {
            let remaining = outer_remaining;
            let (fill_style, remaining) = u32::try_parse(remaining)?;
            let fill_style = fill_style.into();
            outer_remaining = remaining;
            Some(fill_style)
        } else {
            None
        };
        let fill_rule = if switch_expr & u32::from(GC::FILL_RULE) != 0 {
            let remaining = outer_remaining;
            let (fill_rule, remaining) = u32::try_parse(remaining)?;
            let fill_rule = fill_rule.into();
            outer_remaining = remaining;
            Some(fill_rule)
        } else {
            None
        };
        let tile = if switch_expr & u32::from(GC::TILE) != 0 {
            let remaining = outer_remaining;
            let (tile, remaining) = Pixmap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(tile)
        } else {
            None
        };
        let stipple = if switch_expr & u32::from(GC::STIPPLE) != 0 {
            let remaining = outer_remaining;
            let (stipple, remaining) = Pixmap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(stipple)
        } else {
            None
        };
        let tile_stipple_x_origin = if switch_expr & u32::from(GC::TILE_STIPPLE_ORIGIN_X) != 0 {
            let remaining = outer_remaining;
            let (tile_stipple_x_origin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(tile_stipple_x_origin)
        } else {
            None
        };
        let tile_stipple_y_origin = if switch_expr & u32::from(GC::TILE_STIPPLE_ORIGIN_Y) != 0 {
            let remaining = outer_remaining;
            let (tile_stipple_y_origin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(tile_stipple_y_origin)
        } else {
            None
        };
        let font = if switch_expr & u32::from(GC::FONT) != 0 {
            let remaining = outer_remaining;
            let (font, remaining) = Font::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(font)
        } else {
            None
        };
        let subwindow_mode = if switch_expr & u32::from(GC::SUBWINDOW_MODE) != 0 {
            let remaining = outer_remaining;
            let (subwindow_mode, remaining) = u32::try_parse(remaining)?;
            let subwindow_mode = subwindow_mode.into();
            outer_remaining = remaining;
            Some(subwindow_mode)
        } else {
            None
        };
        let graphics_exposures = if switch_expr & u32::from(GC::GRAPHICS_EXPOSURES) != 0 {
            let remaining = outer_remaining;
            let (graphics_exposures, remaining) = Bool32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(graphics_exposures)
        } else {
            None
        };
        let clip_x_origin = if switch_expr & u32::from(GC::CLIP_ORIGIN_X) != 0 {
            let remaining = outer_remaining;
            let (clip_x_origin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(clip_x_origin)
        } else {
            None
        };
        let clip_y_origin = if switch_expr & u32::from(GC::CLIP_ORIGIN_Y) != 0 {
            let remaining = outer_remaining;
            let (clip_y_origin, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(clip_y_origin)
        } else {
            None
        };
        let clip_mask = if switch_expr & u32::from(GC::CLIP_MASK) != 0 {
            let remaining = outer_remaining;
            let (clip_mask, remaining) = Pixmap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(clip_mask)
        } else {
            None
        };
        let dash_offset = if switch_expr & u32::from(GC::DASH_OFFSET) != 0 {
            let remaining = outer_remaining;
            let (dash_offset, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(dash_offset)
        } else {
            None
        };
        let dashes = if switch_expr & u32::from(GC::DASH_LIST) != 0 {
            let remaining = outer_remaining;
            let (dashes, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(dashes)
        } else {
            None
        };
        let arc_mode = if switch_expr & u32::from(GC::ARC_MODE) != 0 {
            let remaining = outer_remaining;
            let (arc_mode, remaining) = u32::try_parse(remaining)?;
            let arc_mode = arc_mode.into();
            outer_remaining = remaining;
            Some(arc_mode)
        } else {
            None
        };
        let result = ChangeGCAux { function, plane_mask, foreground, background, line_width, line_style, cap_style, join_style, fill_style, fill_rule, tile, stipple, tile_stipple_x_origin, tile_stipple_y_origin, font, subwindow_mode, graphics_exposures, clip_x_origin, clip_y_origin, clip_mask, dash_offset, dashes, arc_mode };
        Ok((result, outer_remaining))
    }
}
impl ChangeGCAux {
    #[allow(dead_code)]
    fn serialize(&self, value_mask: u32) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u32::from(value_mask));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, value_mask: u32) {
        assert_eq!(self.switch_expr(), u32::from(value_mask), "switch `value_list` has an inconsistent discriminant");
        if let Some(function) = self.function {
            u32::from(function).serialize_into(bytes);
        }
        if let Some(plane_mask) = self.plane_mask {
            plane_mask.serialize_into(bytes);
        }
        if let Some(foreground) = self.foreground {
            foreground.serialize_into(bytes);
        }
        if let Some(background) = self.background {
            background.serialize_into(bytes);
        }
        if let Some(line_width) = self.line_width {
            line_width.serialize_into(bytes);
        }
        if let Some(line_style) = self.line_style {
            u32::from(line_style).serialize_into(bytes);
        }
        if let Some(cap_style) = self.cap_style {
            u32::from(cap_style).serialize_into(bytes);
        }
        if let Some(join_style) = self.join_style {
            u32::from(join_style).serialize_into(bytes);
        }
        if let Some(fill_style) = self.fill_style {
            u32::from(fill_style).serialize_into(bytes);
        }
        if let Some(fill_rule) = self.fill_rule {
            u32::from(fill_rule).serialize_into(bytes);
        }
        if let Some(tile) = self.tile {
            tile.serialize_into(bytes);
        }
        if let Some(stipple) = self.stipple {
            stipple.serialize_into(bytes);
        }
        if let Some(tile_stipple_x_origin) = self.tile_stipple_x_origin {
            tile_stipple_x_origin.serialize_into(bytes);
        }
        if let Some(tile_stipple_y_origin) = self.tile_stipple_y_origin {
            tile_stipple_y_origin.serialize_into(bytes);
        }
        if let Some(font) = self.font {
            font.serialize_into(bytes);
        }
        if let Some(subwindow_mode) = self.subwindow_mode {
            u32::from(subwindow_mode).serialize_into(bytes);
        }
        if let Some(graphics_exposures) = self.graphics_exposures {
            graphics_exposures.serialize_into(bytes);
        }
        if let Some(clip_x_origin) = self.clip_x_origin {
            clip_x_origin.serialize_into(bytes);
        }
        if let Some(clip_y_origin) = self.clip_y_origin {
            clip_y_origin.serialize_into(bytes);
        }
        if let Some(clip_mask) = self.clip_mask {
            clip_mask.serialize_into(bytes);
        }
        if let Some(dash_offset) = self.dash_offset {
            dash_offset.serialize_into(bytes);
        }
        if let Some(dashes) = self.dashes {
            dashes.serialize_into(bytes);
        }
        if let Some(arc_mode) = self.arc_mode {
            u32::from(arc_mode).serialize_into(bytes);
        }
    }
}
impl ChangeGCAux {
    fn switch_expr(&self) -> u32 {
        let mut expr_value = 0;
        if self.function.is_some() {
            expr_value |= u32::from(GC::FUNCTION);
        }
        if self.plane_mask.is_some() {
            expr_value |= u32::from(GC::PLANE_MASK);
        }
        if self.foreground.is_some() {
            expr_value |= u32::from(GC::FOREGROUND);
        }
        if self.background.is_some() {
            expr_value |= u32::from(GC::BACKGROUND);
        }
        if self.line_width.is_some() {
            expr_value |= u32::from(GC::LINE_WIDTH);
        }
        if self.line_style.is_some() {
            expr_value |= u32::from(GC::LINE_STYLE);
        }
        if self.cap_style.is_some() {
            expr_value |= u32::from(GC::CAP_STYLE);
        }
        if self.join_style.is_some() {
            expr_value |= u32::from(GC::JOIN_STYLE);
        }
        if self.fill_style.is_some() {
            expr_value |= u32::from(GC::FILL_STYLE);
        }
        if self.fill_rule.is_some() {
            expr_value |= u32::from(GC::FILL_RULE);
        }
        if self.tile.is_some() {
            expr_value |= u32::from(GC::TILE);
        }
        if self.stipple.is_some() {
            expr_value |= u32::from(GC::STIPPLE);
        }
        if self.tile_stipple_x_origin.is_some() {
            expr_value |= u32::from(GC::TILE_STIPPLE_ORIGIN_X);
        }
        if self.tile_stipple_y_origin.is_some() {
            expr_value |= u32::from(GC::TILE_STIPPLE_ORIGIN_Y);
        }
        if self.font.is_some() {
            expr_value |= u32::from(GC::FONT);
        }
        if self.subwindow_mode.is_some() {
            expr_value |= u32::from(GC::SUBWINDOW_MODE);
        }
        if self.graphics_exposures.is_some() {
            expr_value |= u32::from(GC::GRAPHICS_EXPOSURES);
        }
        if self.clip_x_origin.is_some() {
            expr_value |= u32::from(GC::CLIP_ORIGIN_X);
        }
        if self.clip_y_origin.is_some() {
            expr_value |= u32::from(GC::CLIP_ORIGIN_Y);
        }
        if self.clip_mask.is_some() {
            expr_value |= u32::from(GC::CLIP_MASK);
        }
        if self.dash_offset.is_some() {
            expr_value |= u32::from(GC::DASH_OFFSET);
        }
        if self.dashes.is_some() {
            expr_value |= u32::from(GC::DASH_LIST);
        }
        if self.arc_mode.is_some() {
            expr_value |= u32::from(GC::ARC_MODE);
        }
        expr_value
    }
}
impl ChangeGCAux {
    /// Create a new instance with all fields unset / not present.
    pub fn new() -> Self {
        Default::default()
    }
    /// Set the `function` field of this structure.
    #[must_use]
    pub fn function<I>(mut self, value: I) -> Self where I: Into<Option<GX>> {
        self.function = value.into();
        self
    }
    /// Set the `plane_mask` field of this structure.
    #[must_use]
    pub fn plane_mask<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.plane_mask = value.into();
        self
    }
    /// Set the `foreground` field of this structure.
    #[must_use]
    pub fn foreground<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.foreground = value.into();
        self
    }
    /// Set the `background` field of this structure.
    #[must_use]
    pub fn background<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.background = value.into();
        self
    }
    /// Set the `line_width` field of this structure.
    #[must_use]
    pub fn line_width<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.line_width = value.into();
        self
    }
    /// Set the `line_style` field of this structure.
    #[must_use]
    pub fn line_style<I>(mut self, value: I) -> Self where I: Into<Option<LineStyle>> {
        self.line_style = value.into();
        self
    }
    /// Set the `cap_style` field of this structure.
    #[must_use]
    pub fn cap_style<I>(mut self, value: I) -> Self where I: Into<Option<CapStyle>> {
        self.cap_style = value.into();
        self
    }
    /// Set the `join_style` field of this structure.
    #[must_use]
    pub fn join_style<I>(mut self, value: I) -> Self where I: Into<Option<JoinStyle>> {
        self.join_style = value.into();
        self
    }
    /// Set the `fill_style` field of this structure.
    #[must_use]
    pub fn fill_style<I>(mut self, value: I) -> Self where I: Into<Option<FillStyle>> {
        self.fill_style = value.into();
        self
    }
    /// Set the `fill_rule` field of this structure.
    #[must_use]
    pub fn fill_rule<I>(mut self, value: I) -> Self where I: Into<Option<FillRule>> {
        self.fill_rule = value.into();
        self
    }
    /// Set the `tile` field of this structure.
    #[must_use]
    pub fn tile<I>(mut self, value: I) -> Self where I: Into<Option<Pixmap>> {
        self.tile = value.into();
        self
    }
    /// Set the `stipple` field of this structure.
    #[must_use]
    pub fn stipple<I>(mut self, value: I) -> Self where I: Into<Option<Pixmap>> {
        self.stipple = value.into();
        self
    }
    /// Set the `tile_stipple_x_origin` field of this structure.
    #[must_use]
    pub fn tile_stipple_x_origin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.tile_stipple_x_origin = value.into();
        self
    }
    /// Set the `tile_stipple_y_origin` field of this structure.
    #[must_use]
    pub fn tile_stipple_y_origin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.tile_stipple_y_origin = value.into();
        self
    }
    /// Set the `font` field of this structure.
    #[must_use]
    pub fn font<I>(mut self, value: I) -> Self where I: Into<Option<Font>> {
        self.font = value.into();
        self
    }
    /// Set the `subwindow_mode` field of this structure.
    #[must_use]
    pub fn subwindow_mode<I>(mut self, value: I) -> Self where I: Into<Option<SubwindowMode>> {
        self.subwindow_mode = value.into();
        self
    }
    /// Set the `graphics_exposures` field of this structure.
    #[must_use]
    pub fn graphics_exposures<I>(mut self, value: I) -> Self where I: Into<Option<Bool32>> {
        self.graphics_exposures = value.into();
        self
    }
    /// Set the `clip_x_origin` field of this structure.
    #[must_use]
    pub fn clip_x_origin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.clip_x_origin = value.into();
        self
    }
    /// Set the `clip_y_origin` field of this structure.
    #[must_use]
    pub fn clip_y_origin<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.clip_y_origin = value.into();
        self
    }
    /// Set the `clip_mask` field of this structure.
    #[must_use]
    pub fn clip_mask<I>(mut self, value: I) -> Self where I: Into<Option<Pixmap>> {
        self.clip_mask = value.into();
        self
    }
    /// Set the `dash_offset` field of this structure.
    #[must_use]
    pub fn dash_offset<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.dash_offset = value.into();
        self
    }
    /// Set the `dashes` field of this structure.
    #[must_use]
    pub fn dashes<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.dashes = value.into();
        self
    }
    /// Set the `arc_mode` field of this structure.
    #[must_use]
    pub fn arc_mode<I>(mut self, value: I) -> Self where I: Into<Option<ArcMode>> {
        self.arc_mode = value.into();
        self
    }
}

/// Opcode for the ChangeGC request
pub const CHANGE_GC_REQUEST: u8 = 56;
/// change graphics context components.
///
/// Changes the components specified by `value_mask` for the specified graphics context.
///
/// # Fields
///
/// * `gc` - The graphics context to change.
/// * `value_list` - Values for each of the components specified in the bitmask `value_mask`. The
/// order has to correspond to the order of possible `value_mask` bits. See the
/// example.
///
/// # Errors
///
/// * `Font` - TODO: reasons?
/// * `GContext` - TODO: reasons?
/// * `Match` - TODO: reasons?
/// * `Pixmap` - TODO: reasons?
/// * `Value` - TODO: reasons?
/// * `Alloc` - The X server could not allocate the requested resources (no memory?).
///
/// # Example
///
/// ```text
/// /*
///  * Changes the foreground color component of the specified graphics context.
///  *
///  */
/// void my_example(xcb_connection_t *conn, xcb_gcontext_t gc, uint32_t fg, uint32_t bg) {
///     /* C99 allows us to use a compact way of changing a single component: */
///     xcb_change_gc(conn, gc, XCB_GC_FOREGROUND, (uint32_t[]){ fg });
///
///     /* The more explicit way. Beware that the order of values is important! */
///     uint32_t mask = 0;
///     mask |= XCB_GC_FOREGROUND;
///     mask |= XCB_GC_BACKGROUND;
///
///     uint32_t values[] = {
///         fg,
///         bg
///     };
///     xcb_change_gc(conn, gc, mask, values);
///     xcb_flush(conn);
/// }
/// ```
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeGCRequest<'input> {
    pub gc: Gcontext,
    pub value_list: Cow<'input, ChangeGCAux>,
}
impl_debug_if_no_extra_traits!(ChangeGCRequest<'_>, "ChangeGCRequest");
impl<'input> ChangeGCRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let gc_bytes = self.gc.serialize();
        let value_mask: u32 = self.value_list.switch_expr();
        let value_mask_bytes = value_mask.serialize();
        let mut request0 = vec![
            CHANGE_GC_REQUEST,
            0,
            0,
            0,
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
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
        if header.major_opcode != CHANGE_GC_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (gc, remaining) = Gcontext::try_parse(value)?;
        let (value_mask, remaining) = u32::try_parse(remaining)?;
        let (value_list, remaining) = ChangeGCAux::try_parse(remaining, u32::from(value_mask))?;
        let _ = remaining;
        Ok(ChangeGCRequest {
            gc,
            value_list: Cow::Owned(value_list),
        })
    }
    /// Clone all borrowed data in this ChangeGCRequest.
    pub fn into_owned(self) -> ChangeGCRequest<'static> {
        ChangeGCRequest {
            gc: self.gc,
            value_list: Cow::Owned(self.value_list.into_owned()),
        }
    }
}
impl<'input> Request for ChangeGCRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for ChangeGCRequest<'input> {
}

/// Opcode for the CopyGC request
pub const COPY_GC_REQUEST: u8 = 57;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CopyGCRequest {
    pub src_gc: Gcontext,
    pub dst_gc: Gcontext,
    pub value_mask: GC,
}
impl_debug_if_no_extra_traits!(CopyGCRequest, "CopyGCRequest");
impl CopyGCRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let src_gc_bytes = self.src_gc.serialize();
        let dst_gc_bytes = self.dst_gc.serialize();
        let value_mask_bytes = u32::from(self.value_mask).serialize();
        let mut request0 = vec![
            COPY_GC_REQUEST,
            0,
            0,
            0,
            src_gc_bytes[0],
            src_gc_bytes[1],
            src_gc_bytes[2],
            src_gc_bytes[3],
            dst_gc_bytes[0],
            dst_gc_bytes[1],
            dst_gc_bytes[2],
            dst_gc_bytes[3],
            value_mask_bytes[0],
            value_mask_bytes[1],
            value_mask_bytes[2],
            value_mask_bytes[3],
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
        if header.major_opcode != COPY_GC_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (src_gc, remaining) = Gcontext::try_parse(value)?;
        let (dst_gc, remaining) = Gcontext::try_parse(remaining)?;
        let (value_mask, remaining) = u32::try_parse(remaining)?;
        let value_mask = value_mask.into();
        let _ = remaining;
        Ok(CopyGCRequest {
            src_gc,
            dst_gc,
            value_mask,
        })
    }
}
impl Request for CopyGCRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CopyGCRequest {
}

/// Opcode for the SetDashes request
pub const SET_DASHES_REQUEST: u8 = 58;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetDashesRequest<'input> {
    pub gc: Gcontext,
    pub dash_offset: u16,
    pub dashes: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(SetDashesRequest<'_>, "SetDashesRequest");
impl<'input> SetDashesRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let gc_bytes = self.gc.serialize();
        let dash_offset_bytes = self.dash_offset.serialize();
        let dashes_len = u16::try_from(self.dashes.len()).expect("`dashes` has too many elements");
        let dashes_len_bytes = dashes_len.serialize();
        let mut request0 = vec![
            SET_DASHES_REQUEST,
            0,
            0,
            0,
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            dash_offset_bytes[0],
            dash_offset_bytes[1],
            dashes_len_bytes[0],
            dashes_len_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.dashes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.dashes, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != SET_DASHES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (gc, remaining) = Gcontext::try_parse(value)?;
        let (dash_offset, remaining) = u16::try_parse(remaining)?;
        let (dashes_len, remaining) = u16::try_parse(remaining)?;
        let (dashes, remaining) = crate::x11_utils::parse_u8_list(remaining, dashes_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(SetDashesRequest {
            gc,
            dash_offset,
            dashes: Cow::Borrowed(dashes),
        })
    }
    /// Clone all borrowed data in this SetDashesRequest.
    pub fn into_owned(self) -> SetDashesRequest<'static> {
        SetDashesRequest {
            gc: self.gc,
            dash_offset: self.dash_offset,
            dashes: Cow::Owned(self.dashes.into_owned()),
        }
    }
}
impl<'input> Request for SetDashesRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetDashesRequest<'input> {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClipOrdering(u8);
impl ClipOrdering {
    pub const UNSORTED: Self = Self(0);
    pub const Y_SORTED: Self = Self(1);
    pub const YX_SORTED: Self = Self(2);
    pub const YX_BANDED: Self = Self(3);
}
impl From<ClipOrdering> for u8 {
    #[inline]
    fn from(input: ClipOrdering) -> Self {
        input.0
    }
}
impl From<ClipOrdering> for Option<u8> {
    #[inline]
    fn from(input: ClipOrdering) -> Self {
        Some(input.0)
    }
}
impl From<ClipOrdering> for u16 {
    #[inline]
    fn from(input: ClipOrdering) -> Self {
        u16::from(input.0)
    }
}
impl From<ClipOrdering> for Option<u16> {
    #[inline]
    fn from(input: ClipOrdering) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<ClipOrdering> for u32 {
    #[inline]
    fn from(input: ClipOrdering) -> Self {
        u32::from(input.0)
    }
}
impl From<ClipOrdering> for Option<u32> {
    #[inline]
    fn from(input: ClipOrdering) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ClipOrdering {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ClipOrdering  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::UNSORTED.0.into(), "UNSORTED", "Unsorted"),
            (Self::Y_SORTED.0.into(), "Y_SORTED", "YSorted"),
            (Self::YX_SORTED.0.into(), "YX_SORTED", "YXSorted"),
            (Self::YX_BANDED.0.into(), "YX_BANDED", "YXBanded"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the SetClipRectangles request
pub const SET_CLIP_RECTANGLES_REQUEST: u8 = 59;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetClipRectanglesRequest<'input> {
    pub ordering: ClipOrdering,
    pub gc: Gcontext,
    pub clip_x_origin: i16,
    pub clip_y_origin: i16,
    pub rectangles: Cow<'input, [Rectangle]>,
}
impl_debug_if_no_extra_traits!(SetClipRectanglesRequest<'_>, "SetClipRectanglesRequest");
impl<'input> SetClipRectanglesRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let ordering_bytes = u8::from(self.ordering).serialize();
        let gc_bytes = self.gc.serialize();
        let clip_x_origin_bytes = self.clip_x_origin.serialize();
        let clip_y_origin_bytes = self.clip_y_origin.serialize();
        let mut request0 = vec![
            SET_CLIP_RECTANGLES_REQUEST,
            ordering_bytes[0],
            0,
            0,
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
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
        if header.major_opcode != SET_CLIP_RECTANGLES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (ordering, remaining) = u8::try_parse(remaining)?;
        let ordering = ordering.into();
        let _ = remaining;
        let (gc, remaining) = Gcontext::try_parse(value)?;
        let (clip_x_origin, remaining) = i16::try_parse(remaining)?;
        let (clip_y_origin, remaining) = i16::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut rectangles = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Rectangle::try_parse(remaining)?;
            remaining = new_remaining;
            rectangles.push(v);
        }
        let _ = remaining;
        Ok(SetClipRectanglesRequest {
            ordering,
            gc,
            clip_x_origin,
            clip_y_origin,
            rectangles: Cow::Owned(rectangles),
        })
    }
    /// Clone all borrowed data in this SetClipRectanglesRequest.
    pub fn into_owned(self) -> SetClipRectanglesRequest<'static> {
        SetClipRectanglesRequest {
            ordering: self.ordering,
            gc: self.gc,
            clip_x_origin: self.clip_x_origin,
            clip_y_origin: self.clip_y_origin,
            rectangles: Cow::Owned(self.rectangles.into_owned()),
        }
    }
}
impl<'input> Request for SetClipRectanglesRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetClipRectanglesRequest<'input> {
}

/// Opcode for the FreeGC request
pub const FREE_GC_REQUEST: u8 = 60;
/// Destroys a graphics context.
///
/// Destroys the specified `gc` and all associated storage.
///
/// # Fields
///
/// * `gc` - The graphics context to destroy.
///
/// # Errors
///
/// * `GContext` - The specified graphics context does not exist.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FreeGCRequest {
    pub gc: Gcontext,
}
impl_debug_if_no_extra_traits!(FreeGCRequest, "FreeGCRequest");
impl FreeGCRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let gc_bytes = self.gc.serialize();
        let mut request0 = vec![
            FREE_GC_REQUEST,
            0,
            0,
            0,
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
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
        if header.major_opcode != FREE_GC_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (gc, remaining) = Gcontext::try_parse(value)?;
        let _ = remaining;
        Ok(FreeGCRequest {
            gc,
        })
    }
}
impl Request for FreeGCRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for FreeGCRequest {
}

/// Opcode for the ClearArea request
pub const CLEAR_AREA_REQUEST: u8 = 61;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClearAreaRequest {
    pub exposures: bool,
    pub window: Window,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}
impl_debug_if_no_extra_traits!(ClearAreaRequest, "ClearAreaRequest");
impl ClearAreaRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let exposures_bytes = self.exposures.serialize();
        let window_bytes = self.window.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let mut request0 = vec![
            CLEAR_AREA_REQUEST,
            exposures_bytes[0],
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
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
        if header.major_opcode != CLEAR_AREA_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (exposures, remaining) = bool::try_parse(remaining)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(ClearAreaRequest {
            exposures,
            window,
            x,
            y,
            width,
            height,
        })
    }
}
impl Request for ClearAreaRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for ClearAreaRequest {
}

/// Opcode for the CopyArea request
pub const COPY_AREA_REQUEST: u8 = 62;
/// copy areas.
///
/// Copies the specified rectangle from `src_drawable` to `dst_drawable`.
///
/// # Fields
///
/// * `dst_drawable` - The destination drawable (Window or Pixmap).
/// * `src_drawable` - The source drawable (Window or Pixmap).
/// * `gc` - The graphics context to use.
/// * `src_x` - The source X coordinate.
/// * `src_y` - The source Y coordinate.
/// * `dst_x` - The destination X coordinate.
/// * `dst_y` - The destination Y coordinate.
/// * `width` - The width of the area to copy (in pixels).
/// * `height` - The height of the area to copy (in pixels).
///
/// # Errors
///
/// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
/// * `GContext` - The specified graphics context does not exist.
/// * `Match` - `src_drawable` has a different root or depth than `dst_drawable`.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CopyAreaRequest {
    pub src_drawable: Drawable,
    pub dst_drawable: Drawable,
    pub gc: Gcontext,
    pub src_x: i16,
    pub src_y: i16,
    pub dst_x: i16,
    pub dst_y: i16,
    pub width: u16,
    pub height: u16,
}
impl_debug_if_no_extra_traits!(CopyAreaRequest, "CopyAreaRequest");
impl CopyAreaRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let src_drawable_bytes = self.src_drawable.serialize();
        let dst_drawable_bytes = self.dst_drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let src_x_bytes = self.src_x.serialize();
        let src_y_bytes = self.src_y.serialize();
        let dst_x_bytes = self.dst_x.serialize();
        let dst_y_bytes = self.dst_y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let mut request0 = vec![
            COPY_AREA_REQUEST,
            0,
            0,
            0,
            src_drawable_bytes[0],
            src_drawable_bytes[1],
            src_drawable_bytes[2],
            src_drawable_bytes[3],
            dst_drawable_bytes[0],
            dst_drawable_bytes[1],
            dst_drawable_bytes[2],
            dst_drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            src_x_bytes[0],
            src_x_bytes[1],
            src_y_bytes[0],
            src_y_bytes[1],
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
        if header.major_opcode != COPY_AREA_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (src_drawable, remaining) = Drawable::try_parse(value)?;
        let (dst_drawable, remaining) = Drawable::try_parse(remaining)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let (src_x, remaining) = i16::try_parse(remaining)?;
        let (src_y, remaining) = i16::try_parse(remaining)?;
        let (dst_x, remaining) = i16::try_parse(remaining)?;
        let (dst_y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(CopyAreaRequest {
            src_drawable,
            dst_drawable,
            gc,
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
        })
    }
}
impl Request for CopyAreaRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CopyAreaRequest {
}

/// Opcode for the CopyPlane request
pub const COPY_PLANE_REQUEST: u8 = 63;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CopyPlaneRequest {
    pub src_drawable: Drawable,
    pub dst_drawable: Drawable,
    pub gc: Gcontext,
    pub src_x: i16,
    pub src_y: i16,
    pub dst_x: i16,
    pub dst_y: i16,
    pub width: u16,
    pub height: u16,
    pub bit_plane: u32,
}
impl_debug_if_no_extra_traits!(CopyPlaneRequest, "CopyPlaneRequest");
impl CopyPlaneRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let src_drawable_bytes = self.src_drawable.serialize();
        let dst_drawable_bytes = self.dst_drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let src_x_bytes = self.src_x.serialize();
        let src_y_bytes = self.src_y.serialize();
        let dst_x_bytes = self.dst_x.serialize();
        let dst_y_bytes = self.dst_y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let bit_plane_bytes = self.bit_plane.serialize();
        let mut request0 = vec![
            COPY_PLANE_REQUEST,
            0,
            0,
            0,
            src_drawable_bytes[0],
            src_drawable_bytes[1],
            src_drawable_bytes[2],
            src_drawable_bytes[3],
            dst_drawable_bytes[0],
            dst_drawable_bytes[1],
            dst_drawable_bytes[2],
            dst_drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            src_x_bytes[0],
            src_x_bytes[1],
            src_y_bytes[0],
            src_y_bytes[1],
            dst_x_bytes[0],
            dst_x_bytes[1],
            dst_y_bytes[0],
            dst_y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            bit_plane_bytes[0],
            bit_plane_bytes[1],
            bit_plane_bytes[2],
            bit_plane_bytes[3],
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
        if header.major_opcode != COPY_PLANE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (src_drawable, remaining) = Drawable::try_parse(value)?;
        let (dst_drawable, remaining) = Drawable::try_parse(remaining)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let (src_x, remaining) = i16::try_parse(remaining)?;
        let (src_y, remaining) = i16::try_parse(remaining)?;
        let (dst_x, remaining) = i16::try_parse(remaining)?;
        let (dst_y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (bit_plane, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(CopyPlaneRequest {
            src_drawable,
            dst_drawable,
            gc,
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
            bit_plane,
        })
    }
}
impl Request for CopyPlaneRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CopyPlaneRequest {
}

/// # Fields
///
/// * `Origin` - Treats all coordinates as relative to the origin.
/// * `Previous` - Treats all coordinates after the first as relative to the previous coordinate.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoordMode(u8);
impl CoordMode {
    pub const ORIGIN: Self = Self(0);
    pub const PREVIOUS: Self = Self(1);
}
impl From<CoordMode> for u8 {
    #[inline]
    fn from(input: CoordMode) -> Self {
        input.0
    }
}
impl From<CoordMode> for Option<u8> {
    #[inline]
    fn from(input: CoordMode) -> Self {
        Some(input.0)
    }
}
impl From<CoordMode> for u16 {
    #[inline]
    fn from(input: CoordMode) -> Self {
        u16::from(input.0)
    }
}
impl From<CoordMode> for Option<u16> {
    #[inline]
    fn from(input: CoordMode) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<CoordMode> for u32 {
    #[inline]
    fn from(input: CoordMode) -> Self {
        u32::from(input.0)
    }
}
impl From<CoordMode> for Option<u32> {
    #[inline]
    fn from(input: CoordMode) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for CoordMode {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for CoordMode  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ORIGIN.0.into(), "ORIGIN", "Origin"),
            (Self::PREVIOUS.0.into(), "PREVIOUS", "Previous"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the PolyPoint request
pub const POLY_POINT_REQUEST: u8 = 64;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolyPointRequest<'input> {
    pub coordinate_mode: CoordMode,
    pub drawable: Drawable,
    pub gc: Gcontext,
    pub points: Cow<'input, [Point]>,
}
impl_debug_if_no_extra_traits!(PolyPointRequest<'_>, "PolyPointRequest");
impl<'input> PolyPointRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let coordinate_mode_bytes = u8::from(self.coordinate_mode).serialize();
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let mut request0 = vec![
            POLY_POINT_REQUEST,
            coordinate_mode_bytes[0],
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
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
        if header.major_opcode != POLY_POINT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (coordinate_mode, remaining) = u8::try_parse(remaining)?;
        let coordinate_mode = coordinate_mode.into();
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut points = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Point::try_parse(remaining)?;
            remaining = new_remaining;
            points.push(v);
        }
        let _ = remaining;
        Ok(PolyPointRequest {
            coordinate_mode,
            drawable,
            gc,
            points: Cow::Owned(points),
        })
    }
    /// Clone all borrowed data in this PolyPointRequest.
    pub fn into_owned(self) -> PolyPointRequest<'static> {
        PolyPointRequest {
            coordinate_mode: self.coordinate_mode,
            drawable: self.drawable,
            gc: self.gc,
            points: Cow::Owned(self.points.into_owned()),
        }
    }
}
impl<'input> Request for PolyPointRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for PolyPointRequest<'input> {
}

/// Opcode for the PolyLine request
pub const POLY_LINE_REQUEST: u8 = 65;
/// draw lines.
///
/// Draws `points_len`-1 lines between each pair of points (point[i], point[i+1])
/// in the `points` array. The lines are drawn in the order listed in the array.
/// They join correctly at all intermediate points, and if the first and last
/// points coincide, the first and last lines also join correctly. For any given
/// line, a pixel is not drawn more than once. If thin (zero line-width) lines
/// intersect, the intersecting pixels are drawn multiple times. If wide lines
/// intersect, the intersecting pixels are drawn only once, as though the entire
/// request were a single, filled shape.
///
/// # Fields
///
/// * `drawable` - The drawable to draw the line(s) on.
/// * `gc` - The graphics context to use.
/// * `points_len` - The number of `xcb_point_t` structures in `points`.
/// * `points` - An array of points.
/// * `coordinate_mode` -
///
/// # Errors
///
/// * `Drawable` - TODO: reasons?
/// * `GContext` - TODO: reasons?
/// * `Match` - TODO: reasons?
/// * `Value` - TODO: reasons?
///
/// # Example
///
/// ```text
/// /*
///  * Draw a straight line.
///  *
///  */
/// void my_example(xcb_connection_t *conn, xcb_drawable_t drawable, xcb_gcontext_t gc) {
///     xcb_poly_line(conn, XCB_COORD_MODE_ORIGIN, drawable, gc, 2,
///                   (xcb_point_t[]) { {10, 10}, {100, 10} });
///     xcb_flush(conn);
/// }
/// ```
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolyLineRequest<'input> {
    pub coordinate_mode: CoordMode,
    pub drawable: Drawable,
    pub gc: Gcontext,
    pub points: Cow<'input, [Point]>,
}
impl_debug_if_no_extra_traits!(PolyLineRequest<'_>, "PolyLineRequest");
impl<'input> PolyLineRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let coordinate_mode_bytes = u8::from(self.coordinate_mode).serialize();
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let mut request0 = vec![
            POLY_LINE_REQUEST,
            coordinate_mode_bytes[0],
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
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
        if header.major_opcode != POLY_LINE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (coordinate_mode, remaining) = u8::try_parse(remaining)?;
        let coordinate_mode = coordinate_mode.into();
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut points = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Point::try_parse(remaining)?;
            remaining = new_remaining;
            points.push(v);
        }
        let _ = remaining;
        Ok(PolyLineRequest {
            coordinate_mode,
            drawable,
            gc,
            points: Cow::Owned(points),
        })
    }
    /// Clone all borrowed data in this PolyLineRequest.
    pub fn into_owned(self) -> PolyLineRequest<'static> {
        PolyLineRequest {
            coordinate_mode: self.coordinate_mode,
            drawable: self.drawable,
            gc: self.gc,
            points: Cow::Owned(self.points.into_owned()),
        }
    }
}
impl<'input> Request for PolyLineRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for PolyLineRequest<'input> {
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Segment {
    pub x1: i16,
    pub y1: i16,
    pub x2: i16,
    pub y2: i16,
}
impl_debug_if_no_extra_traits!(Segment, "Segment");
impl TryParse for Segment {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (x1, remaining) = i16::try_parse(remaining)?;
        let (y1, remaining) = i16::try_parse(remaining)?;
        let (x2, remaining) = i16::try_parse(remaining)?;
        let (y2, remaining) = i16::try_parse(remaining)?;
        let result = Segment { x1, y1, x2, y2 };
        Ok((result, remaining))
    }
}
impl Serialize for Segment {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let x1_bytes = self.x1.serialize();
        let y1_bytes = self.y1.serialize();
        let x2_bytes = self.x2.serialize();
        let y2_bytes = self.y2.serialize();
        [
            x1_bytes[0],
            x1_bytes[1],
            y1_bytes[0],
            y1_bytes[1],
            x2_bytes[0],
            x2_bytes[1],
            y2_bytes[0],
            y2_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.x1.serialize_into(bytes);
        self.y1.serialize_into(bytes);
        self.x2.serialize_into(bytes);
        self.y2.serialize_into(bytes);
    }
}

/// Opcode for the PolySegment request
pub const POLY_SEGMENT_REQUEST: u8 = 66;
/// draw lines.
///
/// Draws multiple, unconnected lines. For each segment, a line is drawn between
/// (x1, y1) and (x2, y2). The lines are drawn in the order listed in the array of
/// `xcb_segment_t` structures and does not perform joining at coincident
/// endpoints. For any given line, a pixel is not drawn more than once. If lines
/// intersect, the intersecting pixels are drawn multiple times.
///
/// TODO: include the xcb_segment_t data structure
///
/// TODO: an example
///
/// # Fields
///
/// * `drawable` - A drawable (Window or Pixmap) to draw on.
/// * `gc` - The graphics context to use.
///
/// TODO: document which attributes of a gc are used
/// * `segments_len` - The number of `xcb_segment_t` structures in `segments`.
/// * `segments` - An array of `xcb_segment_t` structures.
///
/// # Errors
///
/// * `Drawable` - The specified `drawable` does not exist.
/// * `GContext` - The specified `gc` does not exist.
/// * `Match` - TODO: reasons?
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolySegmentRequest<'input> {
    pub drawable: Drawable,
    pub gc: Gcontext,
    pub segments: Cow<'input, [Segment]>,
}
impl_debug_if_no_extra_traits!(PolySegmentRequest<'_>, "PolySegmentRequest");
impl<'input> PolySegmentRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let mut request0 = vec![
            POLY_SEGMENT_REQUEST,
            0,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let segments_bytes = self.segments.serialize();
        let length_so_far = length_so_far + segments_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), segments_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != POLY_SEGMENT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut segments = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Segment::try_parse(remaining)?;
            remaining = new_remaining;
            segments.push(v);
        }
        let _ = remaining;
        Ok(PolySegmentRequest {
            drawable,
            gc,
            segments: Cow::Owned(segments),
        })
    }
    /// Clone all borrowed data in this PolySegmentRequest.
    pub fn into_owned(self) -> PolySegmentRequest<'static> {
        PolySegmentRequest {
            drawable: self.drawable,
            gc: self.gc,
            segments: Cow::Owned(self.segments.into_owned()),
        }
    }
}
impl<'input> Request for PolySegmentRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for PolySegmentRequest<'input> {
}

/// Opcode for the PolyRectangle request
pub const POLY_RECTANGLE_REQUEST: u8 = 67;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolyRectangleRequest<'input> {
    pub drawable: Drawable,
    pub gc: Gcontext,
    pub rectangles: Cow<'input, [Rectangle]>,
}
impl_debug_if_no_extra_traits!(PolyRectangleRequest<'_>, "PolyRectangleRequest");
impl<'input> PolyRectangleRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let mut request0 = vec![
            POLY_RECTANGLE_REQUEST,
            0,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
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
        if header.major_opcode != POLY_RECTANGLE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut rectangles = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Rectangle::try_parse(remaining)?;
            remaining = new_remaining;
            rectangles.push(v);
        }
        let _ = remaining;
        Ok(PolyRectangleRequest {
            drawable,
            gc,
            rectangles: Cow::Owned(rectangles),
        })
    }
    /// Clone all borrowed data in this PolyRectangleRequest.
    pub fn into_owned(self) -> PolyRectangleRequest<'static> {
        PolyRectangleRequest {
            drawable: self.drawable,
            gc: self.gc,
            rectangles: Cow::Owned(self.rectangles.into_owned()),
        }
    }
}
impl<'input> Request for PolyRectangleRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for PolyRectangleRequest<'input> {
}

/// Opcode for the PolyArc request
pub const POLY_ARC_REQUEST: u8 = 68;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolyArcRequest<'input> {
    pub drawable: Drawable,
    pub gc: Gcontext,
    pub arcs: Cow<'input, [Arc]>,
}
impl_debug_if_no_extra_traits!(PolyArcRequest<'_>, "PolyArcRequest");
impl<'input> PolyArcRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let mut request0 = vec![
            POLY_ARC_REQUEST,
            0,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let arcs_bytes = self.arcs.serialize();
        let length_so_far = length_so_far + arcs_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), arcs_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != POLY_ARC_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut arcs = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Arc::try_parse(remaining)?;
            remaining = new_remaining;
            arcs.push(v);
        }
        let _ = remaining;
        Ok(PolyArcRequest {
            drawable,
            gc,
            arcs: Cow::Owned(arcs),
        })
    }
    /// Clone all borrowed data in this PolyArcRequest.
    pub fn into_owned(self) -> PolyArcRequest<'static> {
        PolyArcRequest {
            drawable: self.drawable,
            gc: self.gc,
            arcs: Cow::Owned(self.arcs.into_owned()),
        }
    }
}
impl<'input> Request for PolyArcRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for PolyArcRequest<'input> {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolyShape(u8);
impl PolyShape {
    pub const COMPLEX: Self = Self(0);
    pub const NONCONVEX: Self = Self(1);
    pub const CONVEX: Self = Self(2);
}
impl From<PolyShape> for u8 {
    #[inline]
    fn from(input: PolyShape) -> Self {
        input.0
    }
}
impl From<PolyShape> for Option<u8> {
    #[inline]
    fn from(input: PolyShape) -> Self {
        Some(input.0)
    }
}
impl From<PolyShape> for u16 {
    #[inline]
    fn from(input: PolyShape) -> Self {
        u16::from(input.0)
    }
}
impl From<PolyShape> for Option<u16> {
    #[inline]
    fn from(input: PolyShape) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<PolyShape> for u32 {
    #[inline]
    fn from(input: PolyShape) -> Self {
        u32::from(input.0)
    }
}
impl From<PolyShape> for Option<u32> {
    #[inline]
    fn from(input: PolyShape) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for PolyShape {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for PolyShape  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::COMPLEX.0.into(), "COMPLEX", "Complex"),
            (Self::NONCONVEX.0.into(), "NONCONVEX", "Nonconvex"),
            (Self::CONVEX.0.into(), "CONVEX", "Convex"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the FillPoly request
pub const FILL_POLY_REQUEST: u8 = 69;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FillPolyRequest<'input> {
    pub drawable: Drawable,
    pub gc: Gcontext,
    pub shape: PolyShape,
    pub coordinate_mode: CoordMode,
    pub points: Cow<'input, [Point]>,
}
impl_debug_if_no_extra_traits!(FillPolyRequest<'_>, "FillPolyRequest");
impl<'input> FillPolyRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let shape_bytes = u8::from(self.shape).serialize();
        let coordinate_mode_bytes = u8::from(self.coordinate_mode).serialize();
        let mut request0 = vec![
            FILL_POLY_REQUEST,
            0,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            shape_bytes[0],
            coordinate_mode_bytes[0],
            0,
            0,
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
        if header.major_opcode != FILL_POLY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let (shape, remaining) = u8::try_parse(remaining)?;
        let shape = shape.into();
        let (coordinate_mode, remaining) = u8::try_parse(remaining)?;
        let coordinate_mode = coordinate_mode.into();
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut points = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Point::try_parse(remaining)?;
            remaining = new_remaining;
            points.push(v);
        }
        let _ = remaining;
        Ok(FillPolyRequest {
            drawable,
            gc,
            shape,
            coordinate_mode,
            points: Cow::Owned(points),
        })
    }
    /// Clone all borrowed data in this FillPolyRequest.
    pub fn into_owned(self) -> FillPolyRequest<'static> {
        FillPolyRequest {
            drawable: self.drawable,
            gc: self.gc,
            shape: self.shape,
            coordinate_mode: self.coordinate_mode,
            points: Cow::Owned(self.points.into_owned()),
        }
    }
}
impl<'input> Request for FillPolyRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for FillPolyRequest<'input> {
}

/// Opcode for the PolyFillRectangle request
pub const POLY_FILL_RECTANGLE_REQUEST: u8 = 70;
/// Fills rectangles.
///
/// Fills the specified rectangle(s) in the order listed in the array. For any
/// given rectangle, each pixel is not drawn more than once. If rectangles
/// intersect, the intersecting pixels are drawn multiple times.
///
/// # Fields
///
/// * `drawable` - The drawable (Window or Pixmap) to draw on.
/// * `gc` - The graphics context to use.
///
/// The following graphics context components are used: function, plane-mask,
/// fill-style, subwindow-mode, clip-x-origin, clip-y-origin, and clip-mask.
///
/// The following graphics context mode-dependent components are used:
/// foreground, background, tile, stipple, tile-stipple-x-origin, and
/// tile-stipple-y-origin.
/// * `rectangles_len` - The number of `xcb_rectangle_t` structures in `rectangles`.
/// * `rectangles` - The rectangles to fill.
///
/// # Errors
///
/// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
/// * `GContext` - The specified graphics context does not exist.
/// * `Match` - TODO: reasons?
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolyFillRectangleRequest<'input> {
    pub drawable: Drawable,
    pub gc: Gcontext,
    pub rectangles: Cow<'input, [Rectangle]>,
}
impl_debug_if_no_extra_traits!(PolyFillRectangleRequest<'_>, "PolyFillRectangleRequest");
impl<'input> PolyFillRectangleRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let mut request0 = vec![
            POLY_FILL_RECTANGLE_REQUEST,
            0,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
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
        if header.major_opcode != POLY_FILL_RECTANGLE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut rectangles = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Rectangle::try_parse(remaining)?;
            remaining = new_remaining;
            rectangles.push(v);
        }
        let _ = remaining;
        Ok(PolyFillRectangleRequest {
            drawable,
            gc,
            rectangles: Cow::Owned(rectangles),
        })
    }
    /// Clone all borrowed data in this PolyFillRectangleRequest.
    pub fn into_owned(self) -> PolyFillRectangleRequest<'static> {
        PolyFillRectangleRequest {
            drawable: self.drawable,
            gc: self.gc,
            rectangles: Cow::Owned(self.rectangles.into_owned()),
        }
    }
}
impl<'input> Request for PolyFillRectangleRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for PolyFillRectangleRequest<'input> {
}

/// Opcode for the PolyFillArc request
pub const POLY_FILL_ARC_REQUEST: u8 = 71;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolyFillArcRequest<'input> {
    pub drawable: Drawable,
    pub gc: Gcontext,
    pub arcs: Cow<'input, [Arc]>,
}
impl_debug_if_no_extra_traits!(PolyFillArcRequest<'_>, "PolyFillArcRequest");
impl<'input> PolyFillArcRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let mut request0 = vec![
            POLY_FILL_ARC_REQUEST,
            0,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let arcs_bytes = self.arcs.serialize();
        let length_so_far = length_so_far + arcs_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), arcs_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != POLY_FILL_ARC_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut arcs = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Arc::try_parse(remaining)?;
            remaining = new_remaining;
            arcs.push(v);
        }
        let _ = remaining;
        Ok(PolyFillArcRequest {
            drawable,
            gc,
            arcs: Cow::Owned(arcs),
        })
    }
    /// Clone all borrowed data in this PolyFillArcRequest.
    pub fn into_owned(self) -> PolyFillArcRequest<'static> {
        PolyFillArcRequest {
            drawable: self.drawable,
            gc: self.gc,
            arcs: Cow::Owned(self.arcs.into_owned()),
        }
    }
}
impl<'input> Request for PolyFillArcRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for PolyFillArcRequest<'input> {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageFormat(u8);
impl ImageFormat {
    pub const XY_BITMAP: Self = Self(0);
    pub const XY_PIXMAP: Self = Self(1);
    pub const Z_PIXMAP: Self = Self(2);
}
impl From<ImageFormat> for u8 {
    #[inline]
    fn from(input: ImageFormat) -> Self {
        input.0
    }
}
impl From<ImageFormat> for Option<u8> {
    #[inline]
    fn from(input: ImageFormat) -> Self {
        Some(input.0)
    }
}
impl From<ImageFormat> for u16 {
    #[inline]
    fn from(input: ImageFormat) -> Self {
        u16::from(input.0)
    }
}
impl From<ImageFormat> for Option<u16> {
    #[inline]
    fn from(input: ImageFormat) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<ImageFormat> for u32 {
    #[inline]
    fn from(input: ImageFormat) -> Self {
        u32::from(input.0)
    }
}
impl From<ImageFormat> for Option<u32> {
    #[inline]
    fn from(input: ImageFormat) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ImageFormat {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ImageFormat  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::XY_BITMAP.0.into(), "XY_BITMAP", "XYBitmap"),
            (Self::XY_PIXMAP.0.into(), "XY_PIXMAP", "XYPixmap"),
            (Self::Z_PIXMAP.0.into(), "Z_PIXMAP", "ZPixmap"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the PutImage request
pub const PUT_IMAGE_REQUEST: u8 = 72;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PutImageRequest<'input> {
    pub format: ImageFormat,
    pub drawable: Drawable,
    pub gc: Gcontext,
    pub width: u16,
    pub height: u16,
    pub dst_x: i16,
    pub dst_y: i16,
    pub left_pad: u8,
    pub depth: u8,
    pub data: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(PutImageRequest<'_>, "PutImageRequest");
impl<'input> PutImageRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let format_bytes = u8::from(self.format).serialize();
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let dst_x_bytes = self.dst_x.serialize();
        let dst_y_bytes = self.dst_y.serialize();
        let left_pad_bytes = self.left_pad.serialize();
        let depth_bytes = self.depth.serialize();
        let mut request0 = vec![
            PUT_IMAGE_REQUEST,
            format_bytes[0],
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            dst_x_bytes[0],
            dst_x_bytes[1],
            dst_y_bytes[0],
            dst_y_bytes[1],
            left_pad_bytes[0],
            depth_bytes[0],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.data.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.data, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != PUT_IMAGE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (format, remaining) = u8::try_parse(remaining)?;
        let format = format.into();
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (dst_x, remaining) = i16::try_parse(remaining)?;
        let (dst_y, remaining) = i16::try_parse(remaining)?;
        let (left_pad, remaining) = u8::try_parse(remaining)?;
        let (depth, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (data, remaining) = remaining.split_at(remaining.len());
        let _ = remaining;
        Ok(PutImageRequest {
            format,
            drawable,
            gc,
            width,
            height,
            dst_x,
            dst_y,
            left_pad,
            depth,
            data: Cow::Borrowed(data),
        })
    }
    /// Clone all borrowed data in this PutImageRequest.
    pub fn into_owned(self) -> PutImageRequest<'static> {
        PutImageRequest {
            format: self.format,
            drawable: self.drawable,
            gc: self.gc,
            width: self.width,
            height: self.height,
            dst_x: self.dst_x,
            dst_y: self.dst_y,
            left_pad: self.left_pad,
            depth: self.depth,
            data: Cow::Owned(self.data.into_owned()),
        }
    }
}
impl<'input> Request for PutImageRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for PutImageRequest<'input> {
}

/// Opcode for the GetImage request
pub const GET_IMAGE_REQUEST: u8 = 73;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetImageRequest {
    pub format: ImageFormat,
    pub drawable: Drawable,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub plane_mask: u32,
}
impl_debug_if_no_extra_traits!(GetImageRequest, "GetImageRequest");
impl GetImageRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let format_bytes = u8::from(self.format).serialize();
        let drawable_bytes = self.drawable.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let plane_mask_bytes = self.plane_mask.serialize();
        let mut request0 = vec![
            GET_IMAGE_REQUEST,
            format_bytes[0],
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            plane_mask_bytes[0],
            plane_mask_bytes[1],
            plane_mask_bytes[2],
            plane_mask_bytes[3],
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
        if header.major_opcode != GET_IMAGE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (format, remaining) = u8::try_parse(remaining)?;
        let format = format.into();
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (plane_mask, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetImageRequest {
            format,
            drawable,
            x,
            y,
            width,
            height,
            plane_mask,
        })
    }
}
impl Request for GetImageRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetImageRequest {
    type Reply = GetImageReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetImageReply {
    pub depth: u8,
    pub sequence: u16,
    pub visual: Visualid,
    pub data: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetImageReply, "GetImageReply");
impl TryParse for GetImageReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (depth, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (visual, remaining) = Visualid::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (data, remaining) = crate::x11_utils::parse_u8_list(remaining, u32::from(length).checked_mul(4u32).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let data = data.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetImageReply { depth, sequence, visual, data };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetImageReply {
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
        self.depth.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        assert_eq!(self.data.len() % 4, 0, "`data` has an incorrect length, must be a multiple of 4");
        let length = u32::try_from(self.data.len() / 4).expect("`data` has too many elements");
        length.serialize_into(bytes);
        self.visual.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&self.data);
    }
}
impl GetImageReply {
    /// Get the value of the `length` field.
    ///
    /// The `length` field is used as the length field of the `data` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn length(&self) -> u32 {
        self.data.len()
            .checked_div(4).unwrap()
            .try_into().unwrap()
    }
}

/// Opcode for the PolyText8 request
pub const POLY_TEXT8_REQUEST: u8 = 74;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolyText8Request<'input> {
    pub drawable: Drawable,
    pub gc: Gcontext,
    pub x: i16,
    pub y: i16,
    pub items: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(PolyText8Request<'_>, "PolyText8Request");
impl<'input> PolyText8Request<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let mut request0 = vec![
            POLY_TEXT8_REQUEST,
            0,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.items.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.items, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != POLY_TEXT8_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (items, remaining) = remaining.split_at(remaining.len());
        let _ = remaining;
        Ok(PolyText8Request {
            drawable,
            gc,
            x,
            y,
            items: Cow::Borrowed(items),
        })
    }
    /// Clone all borrowed data in this PolyText8Request.
    pub fn into_owned(self) -> PolyText8Request<'static> {
        PolyText8Request {
            drawable: self.drawable,
            gc: self.gc,
            x: self.x,
            y: self.y,
            items: Cow::Owned(self.items.into_owned()),
        }
    }
}
impl<'input> Request for PolyText8Request<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for PolyText8Request<'input> {
}

/// Opcode for the PolyText16 request
pub const POLY_TEXT16_REQUEST: u8 = 75;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolyText16Request<'input> {
    pub drawable: Drawable,
    pub gc: Gcontext,
    pub x: i16,
    pub y: i16,
    pub items: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(PolyText16Request<'_>, "PolyText16Request");
impl<'input> PolyText16Request<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let mut request0 = vec![
            POLY_TEXT16_REQUEST,
            0,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.items.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.items, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != POLY_TEXT16_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (items, remaining) = remaining.split_at(remaining.len());
        let _ = remaining;
        Ok(PolyText16Request {
            drawable,
            gc,
            x,
            y,
            items: Cow::Borrowed(items),
        })
    }
    /// Clone all borrowed data in this PolyText16Request.
    pub fn into_owned(self) -> PolyText16Request<'static> {
        PolyText16Request {
            drawable: self.drawable,
            gc: self.gc,
            x: self.x,
            y: self.y,
            items: Cow::Owned(self.items.into_owned()),
        }
    }
}
impl<'input> Request for PolyText16Request<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for PolyText16Request<'input> {
}

/// Opcode for the ImageText8 request
pub const IMAGE_TEXT8_REQUEST: u8 = 76;
/// Draws text.
///
/// Fills the destination rectangle with the background pixel from `gc`, then
/// paints the text with the foreground pixel from `gc`. The upper-left corner of
/// the filled rectangle is at [x, y - font-ascent]. The width is overall-width,
/// the height is font-ascent + font-descent. The overall-width, font-ascent and
/// font-descent are as returned by `xcb_query_text_extents` (TODO).
///
/// Note that using X core fonts is deprecated (but still supported) in favor of
/// client-side rendering using Xft.
///
/// # Fields
///
/// * `drawable` - The drawable (Window or Pixmap) to draw text on.
/// * `string` - The string to draw. Only the first 255 characters are relevant due to the data
/// type of `string_len`.
/// * `x` - The x coordinate of the first character, relative to the origin of `drawable`.
/// * `y` - The y coordinate of the first character, relative to the origin of `drawable`.
/// * `gc` - The graphics context to use.
///
/// The following graphics context components are used: plane-mask, foreground,
/// background, font, subwindow-mode, clip-x-origin, clip-y-origin, and clip-mask.
///
/// # Errors
///
/// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
/// * `GContext` - The specified graphics context does not exist.
/// * `Match` - TODO: reasons?
///
/// # See
///
/// * `ImageText16`: request
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageText8Request<'input> {
    pub drawable: Drawable,
    pub gc: Gcontext,
    pub x: i16,
    pub y: i16,
    pub string: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(ImageText8Request<'_>, "ImageText8Request");
impl<'input> ImageText8Request<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let string_len = u8::try_from(self.string.len()).expect("`string` has too many elements");
        let string_len_bytes = string_len.serialize();
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let mut request0 = vec![
            IMAGE_TEXT8_REQUEST,
            string_len_bytes[0],
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.string.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.string, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != IMAGE_TEXT8_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (string_len, remaining) = u8::try_parse(remaining)?;
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (string, remaining) = crate::x11_utils::parse_u8_list(remaining, string_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(ImageText8Request {
            drawable,
            gc,
            x,
            y,
            string: Cow::Borrowed(string),
        })
    }
    /// Clone all borrowed data in this ImageText8Request.
    pub fn into_owned(self) -> ImageText8Request<'static> {
        ImageText8Request {
            drawable: self.drawable,
            gc: self.gc,
            x: self.x,
            y: self.y,
            string: Cow::Owned(self.string.into_owned()),
        }
    }
}
impl<'input> Request for ImageText8Request<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for ImageText8Request<'input> {
}

/// Opcode for the ImageText16 request
pub const IMAGE_TEXT16_REQUEST: u8 = 77;
/// Draws text.
///
/// Fills the destination rectangle with the background pixel from `gc`, then
/// paints the text with the foreground pixel from `gc`. The upper-left corner of
/// the filled rectangle is at [x, y - font-ascent]. The width is overall-width,
/// the height is font-ascent + font-descent. The overall-width, font-ascent and
/// font-descent are as returned by `xcb_query_text_extents` (TODO).
///
/// Note that using X core fonts is deprecated (but still supported) in favor of
/// client-side rendering using Xft.
///
/// # Fields
///
/// * `drawable` - The drawable (Window or Pixmap) to draw text on.
/// * `string` - The string to draw. Only the first 255 characters are relevant due to the data
/// type of `string_len`. Every character uses 2 bytes (hence the 16 in this
/// request's name).
/// * `x` - The x coordinate of the first character, relative to the origin of `drawable`.
/// * `y` - The y coordinate of the first character, relative to the origin of `drawable`.
/// * `gc` - The graphics context to use.
///
/// The following graphics context components are used: plane-mask, foreground,
/// background, font, subwindow-mode, clip-x-origin, clip-y-origin, and clip-mask.
///
/// # Errors
///
/// * `Drawable` - The specified `drawable` (Window or Pixmap) does not exist.
/// * `GContext` - The specified graphics context does not exist.
/// * `Match` - TODO: reasons?
///
/// # See
///
/// * `ImageText8`: request
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageText16Request<'input> {
    pub drawable: Drawable,
    pub gc: Gcontext,
    pub x: i16,
    pub y: i16,
    pub string: Cow<'input, [Char2b]>,
}
impl_debug_if_no_extra_traits!(ImageText16Request<'_>, "ImageText16Request");
impl<'input> ImageText16Request<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let string_len = u8::try_from(self.string.len()).expect("`string` has too many elements");
        let string_len_bytes = string_len.serialize();
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let mut request0 = vec![
            IMAGE_TEXT16_REQUEST,
            string_len_bytes[0],
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let string_bytes = self.string.serialize();
        let length_so_far = length_so_far + string_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), string_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != IMAGE_TEXT16_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (string_len, remaining) = u8::try_parse(remaining)?;
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (gc, remaining) = Gcontext::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (string, remaining) = crate::x11_utils::parse_list::<Char2b>(remaining, string_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(ImageText16Request {
            drawable,
            gc,
            x,
            y,
            string: Cow::Owned(string),
        })
    }
    /// Clone all borrowed data in this ImageText16Request.
    pub fn into_owned(self) -> ImageText16Request<'static> {
        ImageText16Request {
            drawable: self.drawable,
            gc: self.gc,
            x: self.x,
            y: self.y,
            string: Cow::Owned(self.string.into_owned()),
        }
    }
}
impl<'input> Request for ImageText16Request<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for ImageText16Request<'input> {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ColormapAlloc(u8);
impl ColormapAlloc {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(1);
}
impl From<ColormapAlloc> for u8 {
    #[inline]
    fn from(input: ColormapAlloc) -> Self {
        input.0
    }
}
impl From<ColormapAlloc> for Option<u8> {
    #[inline]
    fn from(input: ColormapAlloc) -> Self {
        Some(input.0)
    }
}
impl From<ColormapAlloc> for u16 {
    #[inline]
    fn from(input: ColormapAlloc) -> Self {
        u16::from(input.0)
    }
}
impl From<ColormapAlloc> for Option<u16> {
    #[inline]
    fn from(input: ColormapAlloc) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<ColormapAlloc> for u32 {
    #[inline]
    fn from(input: ColormapAlloc) -> Self {
        u32::from(input.0)
    }
}
impl From<ColormapAlloc> for Option<u32> {
    #[inline]
    fn from(input: ColormapAlloc) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ColormapAlloc {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ColormapAlloc  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NONE.0.into(), "NONE", "None"),
            (Self::ALL.0.into(), "ALL", "All"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the CreateColormap request
pub const CREATE_COLORMAP_REQUEST: u8 = 78;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateColormapRequest {
    pub alloc: ColormapAlloc,
    pub mid: Colormap,
    pub window: Window,
    pub visual: Visualid,
}
impl_debug_if_no_extra_traits!(CreateColormapRequest, "CreateColormapRequest");
impl CreateColormapRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let alloc_bytes = u8::from(self.alloc).serialize();
        let mid_bytes = self.mid.serialize();
        let window_bytes = self.window.serialize();
        let visual_bytes = self.visual.serialize();
        let mut request0 = vec![
            CREATE_COLORMAP_REQUEST,
            alloc_bytes[0],
            0,
            0,
            mid_bytes[0],
            mid_bytes[1],
            mid_bytes[2],
            mid_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            visual_bytes[0],
            visual_bytes[1],
            visual_bytes[2],
            visual_bytes[3],
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
        if header.major_opcode != CREATE_COLORMAP_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (alloc, remaining) = u8::try_parse(remaining)?;
        let alloc = alloc.into();
        let _ = remaining;
        let (mid, remaining) = Colormap::try_parse(value)?;
        let (window, remaining) = Window::try_parse(remaining)?;
        let (visual, remaining) = Visualid::try_parse(remaining)?;
        let _ = remaining;
        Ok(CreateColormapRequest {
            alloc,
            mid,
            window,
            visual,
        })
    }
}
impl Request for CreateColormapRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CreateColormapRequest {
}

/// Opcode for the FreeColormap request
pub const FREE_COLORMAP_REQUEST: u8 = 79;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FreeColormapRequest {
    pub cmap: Colormap,
}
impl_debug_if_no_extra_traits!(FreeColormapRequest, "FreeColormapRequest");
impl FreeColormapRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let cmap_bytes = self.cmap.serialize();
        let mut request0 = vec![
            FREE_COLORMAP_REQUEST,
            0,
            0,
            0,
            cmap_bytes[0],
            cmap_bytes[1],
            cmap_bytes[2],
            cmap_bytes[3],
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
        if header.major_opcode != FREE_COLORMAP_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cmap, remaining) = Colormap::try_parse(value)?;
        let _ = remaining;
        Ok(FreeColormapRequest {
            cmap,
        })
    }
}
impl Request for FreeColormapRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for FreeColormapRequest {
}

/// Opcode for the CopyColormapAndFree request
pub const COPY_COLORMAP_AND_FREE_REQUEST: u8 = 80;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CopyColormapAndFreeRequest {
    pub mid: Colormap,
    pub src_cmap: Colormap,
}
impl_debug_if_no_extra_traits!(CopyColormapAndFreeRequest, "CopyColormapAndFreeRequest");
impl CopyColormapAndFreeRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mid_bytes = self.mid.serialize();
        let src_cmap_bytes = self.src_cmap.serialize();
        let mut request0 = vec![
            COPY_COLORMAP_AND_FREE_REQUEST,
            0,
            0,
            0,
            mid_bytes[0],
            mid_bytes[1],
            mid_bytes[2],
            mid_bytes[3],
            src_cmap_bytes[0],
            src_cmap_bytes[1],
            src_cmap_bytes[2],
            src_cmap_bytes[3],
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
        if header.major_opcode != COPY_COLORMAP_AND_FREE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (mid, remaining) = Colormap::try_parse(value)?;
        let (src_cmap, remaining) = Colormap::try_parse(remaining)?;
        let _ = remaining;
        Ok(CopyColormapAndFreeRequest {
            mid,
            src_cmap,
        })
    }
}
impl Request for CopyColormapAndFreeRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CopyColormapAndFreeRequest {
}

/// Opcode for the InstallColormap request
pub const INSTALL_COLORMAP_REQUEST: u8 = 81;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InstallColormapRequest {
    pub cmap: Colormap,
}
impl_debug_if_no_extra_traits!(InstallColormapRequest, "InstallColormapRequest");
impl InstallColormapRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let cmap_bytes = self.cmap.serialize();
        let mut request0 = vec![
            INSTALL_COLORMAP_REQUEST,
            0,
            0,
            0,
            cmap_bytes[0],
            cmap_bytes[1],
            cmap_bytes[2],
            cmap_bytes[3],
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
        if header.major_opcode != INSTALL_COLORMAP_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cmap, remaining) = Colormap::try_parse(value)?;
        let _ = remaining;
        Ok(InstallColormapRequest {
            cmap,
        })
    }
}
impl Request for InstallColormapRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for InstallColormapRequest {
}

/// Opcode for the UninstallColormap request
pub const UNINSTALL_COLORMAP_REQUEST: u8 = 82;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UninstallColormapRequest {
    pub cmap: Colormap,
}
impl_debug_if_no_extra_traits!(UninstallColormapRequest, "UninstallColormapRequest");
impl UninstallColormapRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let cmap_bytes = self.cmap.serialize();
        let mut request0 = vec![
            UNINSTALL_COLORMAP_REQUEST,
            0,
            0,
            0,
            cmap_bytes[0],
            cmap_bytes[1],
            cmap_bytes[2],
            cmap_bytes[3],
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
        if header.major_opcode != UNINSTALL_COLORMAP_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cmap, remaining) = Colormap::try_parse(value)?;
        let _ = remaining;
        Ok(UninstallColormapRequest {
            cmap,
        })
    }
}
impl Request for UninstallColormapRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for UninstallColormapRequest {
}

/// Opcode for the ListInstalledColormaps request
pub const LIST_INSTALLED_COLORMAPS_REQUEST: u8 = 83;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListInstalledColormapsRequest {
    pub window: Window,
}
impl_debug_if_no_extra_traits!(ListInstalledColormapsRequest, "ListInstalledColormapsRequest");
impl ListInstalledColormapsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            LIST_INSTALLED_COLORMAPS_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.major_opcode != LIST_INSTALLED_COLORMAPS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let _ = remaining;
        Ok(ListInstalledColormapsRequest {
            window,
        })
    }
}
impl Request for ListInstalledColormapsRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for ListInstalledColormapsRequest {
    type Reply = ListInstalledColormapsReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListInstalledColormapsReply {
    pub sequence: u16,
    pub length: u32,
    pub cmaps: Vec<Colormap>,
}
impl_debug_if_no_extra_traits!(ListInstalledColormapsReply, "ListInstalledColormapsReply");
impl TryParse for ListInstalledColormapsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (cmaps_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
        let (cmaps, remaining) = crate::x11_utils::parse_list::<Colormap>(remaining, cmaps_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = ListInstalledColormapsReply { sequence, length, cmaps };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ListInstalledColormapsReply {
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
        let cmaps_len = u16::try_from(self.cmaps.len()).expect("`cmaps` has too many elements");
        cmaps_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
        self.cmaps.serialize_into(bytes);
    }
}
impl ListInstalledColormapsReply {
    /// Get the value of the `cmaps_len` field.
    ///
    /// The `cmaps_len` field is used as the length field of the `cmaps` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn cmaps_len(&self) -> u16 {
        self.cmaps.len()
            .try_into().unwrap()
    }
}

/// Opcode for the AllocColor request
pub const ALLOC_COLOR_REQUEST: u8 = 84;
/// Allocate a color.
///
/// Allocates a read-only colormap entry corresponding to the closest RGB value
/// supported by the hardware. If you are using TrueColor, you can take a shortcut
/// and directly calculate the color pixel value to avoid the round trip. But, for
/// example, on 16-bit color setups (VNC), you can easily get the closest supported
/// RGB value to the RGB value you are specifying.
///
/// # Fields
///
/// * `cmap` - TODO
/// * `red` - The red value of your color.
/// * `green` - The green value of your color.
/// * `blue` - The blue value of your color.
///
/// # Errors
///
/// * `Colormap` - The specified colormap `cmap` does not exist.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllocColorRequest {
    pub cmap: Colormap,
    pub red: u16,
    pub green: u16,
    pub blue: u16,
}
impl_debug_if_no_extra_traits!(AllocColorRequest, "AllocColorRequest");
impl AllocColorRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let cmap_bytes = self.cmap.serialize();
        let red_bytes = self.red.serialize();
        let green_bytes = self.green.serialize();
        let blue_bytes = self.blue.serialize();
        let mut request0 = vec![
            ALLOC_COLOR_REQUEST,
            0,
            0,
            0,
            cmap_bytes[0],
            cmap_bytes[1],
            cmap_bytes[2],
            cmap_bytes[3],
            red_bytes[0],
            red_bytes[1],
            green_bytes[0],
            green_bytes[1],
            blue_bytes[0],
            blue_bytes[1],
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
        if header.major_opcode != ALLOC_COLOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cmap, remaining) = Colormap::try_parse(value)?;
        let (red, remaining) = u16::try_parse(remaining)?;
        let (green, remaining) = u16::try_parse(remaining)?;
        let (blue, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(AllocColorRequest {
            cmap,
            red,
            green,
            blue,
        })
    }
}
impl Request for AllocColorRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for AllocColorRequest {
    type Reply = AllocColorReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllocColorReply {
    pub sequence: u16,
    pub length: u32,
    pub red: u16,
    pub green: u16,
    pub blue: u16,
    pub pixel: u32,
}
impl_debug_if_no_extra_traits!(AllocColorReply, "AllocColorReply");
impl TryParse for AllocColorReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (red, remaining) = u16::try_parse(remaining)?;
        let (green, remaining) = u16::try_parse(remaining)?;
        let (blue, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (pixel, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = AllocColorReply { sequence, length, red, green, blue, pixel };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for AllocColorReply {
    type Bytes = [u8; 20];
    fn serialize(&self) -> [u8; 20] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let red_bytes = self.red.serialize();
        let green_bytes = self.green.serialize();
        let blue_bytes = self.blue.serialize();
        let pixel_bytes = self.pixel.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            red_bytes[0],
            red_bytes[1],
            green_bytes[0],
            green_bytes[1],
            blue_bytes[0],
            blue_bytes[1],
            0,
            0,
            pixel_bytes[0],
            pixel_bytes[1],
            pixel_bytes[2],
            pixel_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(20);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.red.serialize_into(bytes);
        self.green.serialize_into(bytes);
        self.blue.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.pixel.serialize_into(bytes);
    }
}

/// Opcode for the AllocNamedColor request
pub const ALLOC_NAMED_COLOR_REQUEST: u8 = 85;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllocNamedColorRequest<'input> {
    pub cmap: Colormap,
    pub name: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(AllocNamedColorRequest<'_>, "AllocNamedColorRequest");
impl<'input> AllocNamedColorRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let cmap_bytes = self.cmap.serialize();
        let name_len = u16::try_from(self.name.len()).expect("`name` has too many elements");
        let name_len_bytes = name_len.serialize();
        let mut request0 = vec![
            ALLOC_NAMED_COLOR_REQUEST,
            0,
            0,
            0,
            cmap_bytes[0],
            cmap_bytes[1],
            cmap_bytes[2],
            cmap_bytes[3],
            name_len_bytes[0],
            name_len_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.name.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.name, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != ALLOC_NAMED_COLOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cmap, remaining) = Colormap::try_parse(value)?;
        let (name_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, name_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(AllocNamedColorRequest {
            cmap,
            name: Cow::Borrowed(name),
        })
    }
    /// Clone all borrowed data in this AllocNamedColorRequest.
    pub fn into_owned(self) -> AllocNamedColorRequest<'static> {
        AllocNamedColorRequest {
            cmap: self.cmap,
            name: Cow::Owned(self.name.into_owned()),
        }
    }
}
impl<'input> Request for AllocNamedColorRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for AllocNamedColorRequest<'input> {
    type Reply = AllocNamedColorReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllocNamedColorReply {
    pub sequence: u16,
    pub length: u32,
    pub pixel: u32,
    pub exact_red: u16,
    pub exact_green: u16,
    pub exact_blue: u16,
    pub visual_red: u16,
    pub visual_green: u16,
    pub visual_blue: u16,
}
impl_debug_if_no_extra_traits!(AllocNamedColorReply, "AllocNamedColorReply");
impl TryParse for AllocNamedColorReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (pixel, remaining) = u32::try_parse(remaining)?;
        let (exact_red, remaining) = u16::try_parse(remaining)?;
        let (exact_green, remaining) = u16::try_parse(remaining)?;
        let (exact_blue, remaining) = u16::try_parse(remaining)?;
        let (visual_red, remaining) = u16::try_parse(remaining)?;
        let (visual_green, remaining) = u16::try_parse(remaining)?;
        let (visual_blue, remaining) = u16::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = AllocNamedColorReply { sequence, length, pixel, exact_red, exact_green, exact_blue, visual_red, visual_green, visual_blue };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for AllocNamedColorReply {
    type Bytes = [u8; 24];
    fn serialize(&self) -> [u8; 24] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let pixel_bytes = self.pixel.serialize();
        let exact_red_bytes = self.exact_red.serialize();
        let exact_green_bytes = self.exact_green.serialize();
        let exact_blue_bytes = self.exact_blue.serialize();
        let visual_red_bytes = self.visual_red.serialize();
        let visual_green_bytes = self.visual_green.serialize();
        let visual_blue_bytes = self.visual_blue.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            pixel_bytes[0],
            pixel_bytes[1],
            pixel_bytes[2],
            pixel_bytes[3],
            exact_red_bytes[0],
            exact_red_bytes[1],
            exact_green_bytes[0],
            exact_green_bytes[1],
            exact_blue_bytes[0],
            exact_blue_bytes[1],
            visual_red_bytes[0],
            visual_red_bytes[1],
            visual_green_bytes[0],
            visual_green_bytes[1],
            visual_blue_bytes[0],
            visual_blue_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(24);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.pixel.serialize_into(bytes);
        self.exact_red.serialize_into(bytes);
        self.exact_green.serialize_into(bytes);
        self.exact_blue.serialize_into(bytes);
        self.visual_red.serialize_into(bytes);
        self.visual_green.serialize_into(bytes);
        self.visual_blue.serialize_into(bytes);
    }
}

/// Opcode for the AllocColorCells request
pub const ALLOC_COLOR_CELLS_REQUEST: u8 = 86;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllocColorCellsRequest {
    pub contiguous: bool,
    pub cmap: Colormap,
    pub colors: u16,
    pub planes: u16,
}
impl_debug_if_no_extra_traits!(AllocColorCellsRequest, "AllocColorCellsRequest");
impl AllocColorCellsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let contiguous_bytes = self.contiguous.serialize();
        let cmap_bytes = self.cmap.serialize();
        let colors_bytes = self.colors.serialize();
        let planes_bytes = self.planes.serialize();
        let mut request0 = vec![
            ALLOC_COLOR_CELLS_REQUEST,
            contiguous_bytes[0],
            0,
            0,
            cmap_bytes[0],
            cmap_bytes[1],
            cmap_bytes[2],
            cmap_bytes[3],
            colors_bytes[0],
            colors_bytes[1],
            planes_bytes[0],
            planes_bytes[1],
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
        if header.major_opcode != ALLOC_COLOR_CELLS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (contiguous, remaining) = bool::try_parse(remaining)?;
        let _ = remaining;
        let (cmap, remaining) = Colormap::try_parse(value)?;
        let (colors, remaining) = u16::try_parse(remaining)?;
        let (planes, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(AllocColorCellsRequest {
            contiguous,
            cmap,
            colors,
            planes,
        })
    }
}
impl Request for AllocColorCellsRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for AllocColorCellsRequest {
    type Reply = AllocColorCellsReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllocColorCellsReply {
    pub sequence: u16,
    pub length: u32,
    pub pixels: Vec<u32>,
    pub masks: Vec<u32>,
}
impl_debug_if_no_extra_traits!(AllocColorCellsReply, "AllocColorCellsReply");
impl TryParse for AllocColorCellsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (pixels_len, remaining) = u16::try_parse(remaining)?;
        let (masks_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (pixels, remaining) = crate::x11_utils::parse_list::<u32>(remaining, pixels_len.try_to_usize()?)?;
        let (masks, remaining) = crate::x11_utils::parse_list::<u32>(remaining, masks_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = AllocColorCellsReply { sequence, length, pixels, masks };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for AllocColorCellsReply {
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
        let pixels_len = u16::try_from(self.pixels.len()).expect("`pixels` has too many elements");
        pixels_len.serialize_into(bytes);
        let masks_len = u16::try_from(self.masks.len()).expect("`masks` has too many elements");
        masks_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        self.pixels.serialize_into(bytes);
        self.masks.serialize_into(bytes);
    }
}
impl AllocColorCellsReply {
    /// Get the value of the `pixels_len` field.
    ///
    /// The `pixels_len` field is used as the length field of the `pixels` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn pixels_len(&self) -> u16 {
        self.pixels.len()
            .try_into().unwrap()
    }
    /// Get the value of the `masks_len` field.
    ///
    /// The `masks_len` field is used as the length field of the `masks` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn masks_len(&self) -> u16 {
        self.masks.len()
            .try_into().unwrap()
    }
}

/// Opcode for the AllocColorPlanes request
pub const ALLOC_COLOR_PLANES_REQUEST: u8 = 87;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllocColorPlanesRequest {
    pub contiguous: bool,
    pub cmap: Colormap,
    pub colors: u16,
    pub reds: u16,
    pub greens: u16,
    pub blues: u16,
}
impl_debug_if_no_extra_traits!(AllocColorPlanesRequest, "AllocColorPlanesRequest");
impl AllocColorPlanesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let contiguous_bytes = self.contiguous.serialize();
        let cmap_bytes = self.cmap.serialize();
        let colors_bytes = self.colors.serialize();
        let reds_bytes = self.reds.serialize();
        let greens_bytes = self.greens.serialize();
        let blues_bytes = self.blues.serialize();
        let mut request0 = vec![
            ALLOC_COLOR_PLANES_REQUEST,
            contiguous_bytes[0],
            0,
            0,
            cmap_bytes[0],
            cmap_bytes[1],
            cmap_bytes[2],
            cmap_bytes[3],
            colors_bytes[0],
            colors_bytes[1],
            reds_bytes[0],
            reds_bytes[1],
            greens_bytes[0],
            greens_bytes[1],
            blues_bytes[0],
            blues_bytes[1],
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
        if header.major_opcode != ALLOC_COLOR_PLANES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (contiguous, remaining) = bool::try_parse(remaining)?;
        let _ = remaining;
        let (cmap, remaining) = Colormap::try_parse(value)?;
        let (colors, remaining) = u16::try_parse(remaining)?;
        let (reds, remaining) = u16::try_parse(remaining)?;
        let (greens, remaining) = u16::try_parse(remaining)?;
        let (blues, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(AllocColorPlanesRequest {
            contiguous,
            cmap,
            colors,
            reds,
            greens,
            blues,
        })
    }
}
impl Request for AllocColorPlanesRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for AllocColorPlanesRequest {
    type Reply = AllocColorPlanesReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllocColorPlanesReply {
    pub sequence: u16,
    pub length: u32,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    pub pixels: Vec<u32>,
}
impl_debug_if_no_extra_traits!(AllocColorPlanesReply, "AllocColorPlanesReply");
impl TryParse for AllocColorPlanesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (pixels_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (red_mask, remaining) = u32::try_parse(remaining)?;
        let (green_mask, remaining) = u32::try_parse(remaining)?;
        let (blue_mask, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(8..).ok_or(ParseError::InsufficientData)?;
        let (pixels, remaining) = crate::x11_utils::parse_list::<u32>(remaining, pixels_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = AllocColorPlanesReply { sequence, length, red_mask, green_mask, blue_mask, pixels };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for AllocColorPlanesReply {
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
        let pixels_len = u16::try_from(self.pixels.len()).expect("`pixels` has too many elements");
        pixels_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.red_mask.serialize_into(bytes);
        self.green_mask.serialize_into(bytes);
        self.blue_mask.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 8]);
        self.pixels.serialize_into(bytes);
    }
}
impl AllocColorPlanesReply {
    /// Get the value of the `pixels_len` field.
    ///
    /// The `pixels_len` field is used as the length field of the `pixels` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn pixels_len(&self) -> u16 {
        self.pixels.len()
            .try_into().unwrap()
    }
}

/// Opcode for the FreeColors request
pub const FREE_COLORS_REQUEST: u8 = 88;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FreeColorsRequest<'input> {
    pub cmap: Colormap,
    pub plane_mask: u32,
    pub pixels: Cow<'input, [u32]>,
}
impl_debug_if_no_extra_traits!(FreeColorsRequest<'_>, "FreeColorsRequest");
impl<'input> FreeColorsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let cmap_bytes = self.cmap.serialize();
        let plane_mask_bytes = self.plane_mask.serialize();
        let mut request0 = vec![
            FREE_COLORS_REQUEST,
            0,
            0,
            0,
            cmap_bytes[0],
            cmap_bytes[1],
            cmap_bytes[2],
            cmap_bytes[3],
            plane_mask_bytes[0],
            plane_mask_bytes[1],
            plane_mask_bytes[2],
            plane_mask_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let pixels_bytes = self.pixels.serialize();
        let length_so_far = length_so_far + pixels_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), pixels_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != FREE_COLORS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cmap, remaining) = Colormap::try_parse(value)?;
        let (plane_mask, remaining) = u32::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut pixels = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = u32::try_parse(remaining)?;
            remaining = new_remaining;
            pixels.push(v);
        }
        let _ = remaining;
        Ok(FreeColorsRequest {
            cmap,
            plane_mask,
            pixels: Cow::Owned(pixels),
        })
    }
    /// Clone all borrowed data in this FreeColorsRequest.
    pub fn into_owned(self) -> FreeColorsRequest<'static> {
        FreeColorsRequest {
            cmap: self.cmap,
            plane_mask: self.plane_mask,
            pixels: Cow::Owned(self.pixels.into_owned()),
        }
    }
}
impl<'input> Request for FreeColorsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for FreeColorsRequest<'input> {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ColorFlag(u8);
impl ColorFlag {
    pub const RED: Self = Self(1 << 0);
    pub const GREEN: Self = Self(1 << 1);
    pub const BLUE: Self = Self(1 << 2);
}
impl From<ColorFlag> for u8 {
    #[inline]
    fn from(input: ColorFlag) -> Self {
        input.0
    }
}
impl From<ColorFlag> for Option<u8> {
    #[inline]
    fn from(input: ColorFlag) -> Self {
        Some(input.0)
    }
}
impl From<ColorFlag> for u16 {
    #[inline]
    fn from(input: ColorFlag) -> Self {
        u16::from(input.0)
    }
}
impl From<ColorFlag> for Option<u16> {
    #[inline]
    fn from(input: ColorFlag) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<ColorFlag> for u32 {
    #[inline]
    fn from(input: ColorFlag) -> Self {
        u32::from(input.0)
    }
}
impl From<ColorFlag> for Option<u32> {
    #[inline]
    fn from(input: ColorFlag) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ColorFlag {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ColorFlag  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::RED.0.into(), "RED", "Red"),
            (Self::GREEN.0.into(), "GREEN", "Green"),
            (Self::BLUE.0.into(), "BLUE", "Blue"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(ColorFlag, u8);

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Coloritem {
    pub pixel: u32,
    pub red: u16,
    pub green: u16,
    pub blue: u16,
    pub flags: ColorFlag,
}
impl_debug_if_no_extra_traits!(Coloritem, "Coloritem");
impl TryParse for Coloritem {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (pixel, remaining) = u32::try_parse(remaining)?;
        let (red, remaining) = u16::try_parse(remaining)?;
        let (green, remaining) = u16::try_parse(remaining)?;
        let (blue, remaining) = u16::try_parse(remaining)?;
        let (flags, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let flags = flags.into();
        let result = Coloritem { pixel, red, green, blue, flags };
        Ok((result, remaining))
    }
}
impl Serialize for Coloritem {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let pixel_bytes = self.pixel.serialize();
        let red_bytes = self.red.serialize();
        let green_bytes = self.green.serialize();
        let blue_bytes = self.blue.serialize();
        let flags_bytes = u8::from(self.flags).serialize();
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
            flags_bytes[0],
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.pixel.serialize_into(bytes);
        self.red.serialize_into(bytes);
        self.green.serialize_into(bytes);
        self.blue.serialize_into(bytes);
        u8::from(self.flags).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
    }
}

/// Opcode for the StoreColors request
pub const STORE_COLORS_REQUEST: u8 = 89;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StoreColorsRequest<'input> {
    pub cmap: Colormap,
    pub items: Cow<'input, [Coloritem]>,
}
impl_debug_if_no_extra_traits!(StoreColorsRequest<'_>, "StoreColorsRequest");
impl<'input> StoreColorsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let cmap_bytes = self.cmap.serialize();
        let mut request0 = vec![
            STORE_COLORS_REQUEST,
            0,
            0,
            0,
            cmap_bytes[0],
            cmap_bytes[1],
            cmap_bytes[2],
            cmap_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let items_bytes = self.items.serialize();
        let length_so_far = length_so_far + items_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), items_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != STORE_COLORS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cmap, remaining) = Colormap::try_parse(value)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut items = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Coloritem::try_parse(remaining)?;
            remaining = new_remaining;
            items.push(v);
        }
        let _ = remaining;
        Ok(StoreColorsRequest {
            cmap,
            items: Cow::Owned(items),
        })
    }
    /// Clone all borrowed data in this StoreColorsRequest.
    pub fn into_owned(self) -> StoreColorsRequest<'static> {
        StoreColorsRequest {
            cmap: self.cmap,
            items: Cow::Owned(self.items.into_owned()),
        }
    }
}
impl<'input> Request for StoreColorsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for StoreColorsRequest<'input> {
}

/// Opcode for the StoreNamedColor request
pub const STORE_NAMED_COLOR_REQUEST: u8 = 90;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StoreNamedColorRequest<'input> {
    pub flags: ColorFlag,
    pub cmap: Colormap,
    pub pixel: u32,
    pub name: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(StoreNamedColorRequest<'_>, "StoreNamedColorRequest");
impl<'input> StoreNamedColorRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let flags_bytes = u8::from(self.flags).serialize();
        let cmap_bytes = self.cmap.serialize();
        let pixel_bytes = self.pixel.serialize();
        let name_len = u16::try_from(self.name.len()).expect("`name` has too many elements");
        let name_len_bytes = name_len.serialize();
        let mut request0 = vec![
            STORE_NAMED_COLOR_REQUEST,
            flags_bytes[0],
            0,
            0,
            cmap_bytes[0],
            cmap_bytes[1],
            cmap_bytes[2],
            cmap_bytes[3],
            pixel_bytes[0],
            pixel_bytes[1],
            pixel_bytes[2],
            pixel_bytes[3],
            name_len_bytes[0],
            name_len_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.name.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.name, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != STORE_NAMED_COLOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (flags, remaining) = u8::try_parse(remaining)?;
        let flags = flags.into();
        let _ = remaining;
        let (cmap, remaining) = Colormap::try_parse(value)?;
        let (pixel, remaining) = u32::try_parse(remaining)?;
        let (name_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, name_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(StoreNamedColorRequest {
            flags,
            cmap,
            pixel,
            name: Cow::Borrowed(name),
        })
    }
    /// Clone all borrowed data in this StoreNamedColorRequest.
    pub fn into_owned(self) -> StoreNamedColorRequest<'static> {
        StoreNamedColorRequest {
            flags: self.flags,
            cmap: self.cmap,
            pixel: self.pixel,
            name: Cow::Owned(self.name.into_owned()),
        }
    }
}
impl<'input> Request for StoreNamedColorRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for StoreNamedColorRequest<'input> {
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rgb {
    pub red: u16,
    pub green: u16,
    pub blue: u16,
}
impl_debug_if_no_extra_traits!(Rgb, "Rgb");
impl TryParse for Rgb {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (red, remaining) = u16::try_parse(remaining)?;
        let (green, remaining) = u16::try_parse(remaining)?;
        let (blue, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let result = Rgb { red, green, blue };
        Ok((result, remaining))
    }
}
impl Serialize for Rgb {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let red_bytes = self.red.serialize();
        let green_bytes = self.green.serialize();
        let blue_bytes = self.blue.serialize();
        [
            red_bytes[0],
            red_bytes[1],
            green_bytes[0],
            green_bytes[1],
            blue_bytes[0],
            blue_bytes[1],
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.red.serialize_into(bytes);
        self.green.serialize_into(bytes);
        self.blue.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
    }
}

/// Opcode for the QueryColors request
pub const QUERY_COLORS_REQUEST: u8 = 91;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryColorsRequest<'input> {
    pub cmap: Colormap,
    pub pixels: Cow<'input, [u32]>,
}
impl_debug_if_no_extra_traits!(QueryColorsRequest<'_>, "QueryColorsRequest");
impl<'input> QueryColorsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let cmap_bytes = self.cmap.serialize();
        let mut request0 = vec![
            QUERY_COLORS_REQUEST,
            0,
            0,
            0,
            cmap_bytes[0],
            cmap_bytes[1],
            cmap_bytes[2],
            cmap_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let pixels_bytes = self.pixels.serialize();
        let length_so_far = length_so_far + pixels_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), pixels_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != QUERY_COLORS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cmap, remaining) = Colormap::try_parse(value)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut pixels = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = u32::try_parse(remaining)?;
            remaining = new_remaining;
            pixels.push(v);
        }
        let _ = remaining;
        Ok(QueryColorsRequest {
            cmap,
            pixels: Cow::Owned(pixels),
        })
    }
    /// Clone all borrowed data in this QueryColorsRequest.
    pub fn into_owned(self) -> QueryColorsRequest<'static> {
        QueryColorsRequest {
            cmap: self.cmap,
            pixels: Cow::Owned(self.pixels.into_owned()),
        }
    }
}
impl<'input> Request for QueryColorsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for QueryColorsRequest<'input> {
    type Reply = QueryColorsReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryColorsReply {
    pub sequence: u16,
    pub length: u32,
    pub colors: Vec<Rgb>,
}
impl_debug_if_no_extra_traits!(QueryColorsReply, "QueryColorsReply");
impl TryParse for QueryColorsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (colors_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
        let (colors, remaining) = crate::x11_utils::parse_list::<Rgb>(remaining, colors_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryColorsReply { sequence, length, colors };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryColorsReply {
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
        let colors_len = u16::try_from(self.colors.len()).expect("`colors` has too many elements");
        colors_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
        self.colors.serialize_into(bytes);
    }
}
impl QueryColorsReply {
    /// Get the value of the `colors_len` field.
    ///
    /// The `colors_len` field is used as the length field of the `colors` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn colors_len(&self) -> u16 {
        self.colors.len()
            .try_into().unwrap()
    }
}

/// Opcode for the LookupColor request
pub const LOOKUP_COLOR_REQUEST: u8 = 92;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LookupColorRequest<'input> {
    pub cmap: Colormap,
    pub name: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(LookupColorRequest<'_>, "LookupColorRequest");
impl<'input> LookupColorRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let cmap_bytes = self.cmap.serialize();
        let name_len = u16::try_from(self.name.len()).expect("`name` has too many elements");
        let name_len_bytes = name_len.serialize();
        let mut request0 = vec![
            LOOKUP_COLOR_REQUEST,
            0,
            0,
            0,
            cmap_bytes[0],
            cmap_bytes[1],
            cmap_bytes[2],
            cmap_bytes[3],
            name_len_bytes[0],
            name_len_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.name.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.name, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != LOOKUP_COLOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cmap, remaining) = Colormap::try_parse(value)?;
        let (name_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, name_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(LookupColorRequest {
            cmap,
            name: Cow::Borrowed(name),
        })
    }
    /// Clone all borrowed data in this LookupColorRequest.
    pub fn into_owned(self) -> LookupColorRequest<'static> {
        LookupColorRequest {
            cmap: self.cmap,
            name: Cow::Owned(self.name.into_owned()),
        }
    }
}
impl<'input> Request for LookupColorRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for LookupColorRequest<'input> {
    type Reply = LookupColorReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LookupColorReply {
    pub sequence: u16,
    pub length: u32,
    pub exact_red: u16,
    pub exact_green: u16,
    pub exact_blue: u16,
    pub visual_red: u16,
    pub visual_green: u16,
    pub visual_blue: u16,
}
impl_debug_if_no_extra_traits!(LookupColorReply, "LookupColorReply");
impl TryParse for LookupColorReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (exact_red, remaining) = u16::try_parse(remaining)?;
        let (exact_green, remaining) = u16::try_parse(remaining)?;
        let (exact_blue, remaining) = u16::try_parse(remaining)?;
        let (visual_red, remaining) = u16::try_parse(remaining)?;
        let (visual_green, remaining) = u16::try_parse(remaining)?;
        let (visual_blue, remaining) = u16::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = LookupColorReply { sequence, length, exact_red, exact_green, exact_blue, visual_red, visual_green, visual_blue };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for LookupColorReply {
    type Bytes = [u8; 20];
    fn serialize(&self) -> [u8; 20] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let exact_red_bytes = self.exact_red.serialize();
        let exact_green_bytes = self.exact_green.serialize();
        let exact_blue_bytes = self.exact_blue.serialize();
        let visual_red_bytes = self.visual_red.serialize();
        let visual_green_bytes = self.visual_green.serialize();
        let visual_blue_bytes = self.visual_blue.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            exact_red_bytes[0],
            exact_red_bytes[1],
            exact_green_bytes[0],
            exact_green_bytes[1],
            exact_blue_bytes[0],
            exact_blue_bytes[1],
            visual_red_bytes[0],
            visual_red_bytes[1],
            visual_green_bytes[0],
            visual_green_bytes[1],
            visual_blue_bytes[0],
            visual_blue_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(20);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.exact_red.serialize_into(bytes);
        self.exact_green.serialize_into(bytes);
        self.exact_blue.serialize_into(bytes);
        self.visual_red.serialize_into(bytes);
        self.visual_green.serialize_into(bytes);
        self.visual_blue.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PixmapEnum(u8);
impl PixmapEnum {
    pub const NONE: Self = Self(0);
}
impl From<PixmapEnum> for u8 {
    #[inline]
    fn from(input: PixmapEnum) -> Self {
        input.0
    }
}
impl From<PixmapEnum> for Option<u8> {
    #[inline]
    fn from(input: PixmapEnum) -> Self {
        Some(input.0)
    }
}
impl From<PixmapEnum> for u16 {
    #[inline]
    fn from(input: PixmapEnum) -> Self {
        u16::from(input.0)
    }
}
impl From<PixmapEnum> for Option<u16> {
    #[inline]
    fn from(input: PixmapEnum) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<PixmapEnum> for u32 {
    #[inline]
    fn from(input: PixmapEnum) -> Self {
        u32::from(input.0)
    }
}
impl From<PixmapEnum> for Option<u32> {
    #[inline]
    fn from(input: PixmapEnum) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for PixmapEnum {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for PixmapEnum  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NONE.0.into(), "NONE", "None"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the CreateCursor request
pub const CREATE_CURSOR_REQUEST: u8 = 93;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateCursorRequest {
    pub cid: Cursor,
    pub source: Pixmap,
    pub mask: Pixmap,
    pub fore_red: u16,
    pub fore_green: u16,
    pub fore_blue: u16,
    pub back_red: u16,
    pub back_green: u16,
    pub back_blue: u16,
    pub x: u16,
    pub y: u16,
}
impl_debug_if_no_extra_traits!(CreateCursorRequest, "CreateCursorRequest");
impl CreateCursorRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let cid_bytes = self.cid.serialize();
        let source_bytes = self.source.serialize();
        let mask_bytes = self.mask.serialize();
        let fore_red_bytes = self.fore_red.serialize();
        let fore_green_bytes = self.fore_green.serialize();
        let fore_blue_bytes = self.fore_blue.serialize();
        let back_red_bytes = self.back_red.serialize();
        let back_green_bytes = self.back_green.serialize();
        let back_blue_bytes = self.back_blue.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let mut request0 = vec![
            CREATE_CURSOR_REQUEST,
            0,
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
            mask_bytes[0],
            mask_bytes[1],
            mask_bytes[2],
            mask_bytes[3],
            fore_red_bytes[0],
            fore_red_bytes[1],
            fore_green_bytes[0],
            fore_green_bytes[1],
            fore_blue_bytes[0],
            fore_blue_bytes[1],
            back_red_bytes[0],
            back_red_bytes[1],
            back_green_bytes[0],
            back_green_bytes[1],
            back_blue_bytes[0],
            back_blue_bytes[1],
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
        if header.major_opcode != CREATE_CURSOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cid, remaining) = Cursor::try_parse(value)?;
        let (source, remaining) = Pixmap::try_parse(remaining)?;
        let (mask, remaining) = Pixmap::try_parse(remaining)?;
        let (fore_red, remaining) = u16::try_parse(remaining)?;
        let (fore_green, remaining) = u16::try_parse(remaining)?;
        let (fore_blue, remaining) = u16::try_parse(remaining)?;
        let (back_red, remaining) = u16::try_parse(remaining)?;
        let (back_green, remaining) = u16::try_parse(remaining)?;
        let (back_blue, remaining) = u16::try_parse(remaining)?;
        let (x, remaining) = u16::try_parse(remaining)?;
        let (y, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(CreateCursorRequest {
            cid,
            source,
            mask,
            fore_red,
            fore_green,
            fore_blue,
            back_red,
            back_green,
            back_blue,
            x,
            y,
        })
    }
}
impl Request for CreateCursorRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CreateCursorRequest {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FontEnum(u8);
impl FontEnum {
    pub const NONE: Self = Self(0);
}
impl From<FontEnum> for u8 {
    #[inline]
    fn from(input: FontEnum) -> Self {
        input.0
    }
}
impl From<FontEnum> for Option<u8> {
    #[inline]
    fn from(input: FontEnum) -> Self {
        Some(input.0)
    }
}
impl From<FontEnum> for u16 {
    #[inline]
    fn from(input: FontEnum) -> Self {
        u16::from(input.0)
    }
}
impl From<FontEnum> for Option<u16> {
    #[inline]
    fn from(input: FontEnum) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<FontEnum> for u32 {
    #[inline]
    fn from(input: FontEnum) -> Self {
        u32::from(input.0)
    }
}
impl From<FontEnum> for Option<u32> {
    #[inline]
    fn from(input: FontEnum) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for FontEnum {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for FontEnum  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NONE.0.into(), "NONE", "None"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the CreateGlyphCursor request
pub const CREATE_GLYPH_CURSOR_REQUEST: u8 = 94;
/// create cursor.
///
/// Creates a cursor from a font glyph. X provides a set of standard cursor shapes
/// in a special font named cursor. Applications are encouraged to use this
/// interface for their cursors because the font can be customized for the
/// individual display type.
///
/// All pixels which are set to 1 in the source will use the foreground color (as
/// specified by `fore_red`, `fore_green` and `fore_blue`). All pixels set to 0
/// will use the background color (as specified by `back_red`, `back_green` and
/// `back_blue`).
///
/// # Fields
///
/// * `cid` - The ID with which you will refer to the cursor, created by `xcb_generate_id`.
/// * `source_font` - In which font to look for the cursor glyph.
/// * `mask_font` - In which font to look for the mask glyph.
/// * `source_char` - The glyph of `source_font` to use.
/// * `mask_char` - The glyph of `mask_font` to use as a mask: Pixels which are set to 1 define
/// which source pixels are displayed. All pixels which are set to 0 are not
/// displayed.
/// * `fore_red` - The red value of the foreground color.
/// * `fore_green` - The green value of the foreground color.
/// * `fore_blue` - The blue value of the foreground color.
/// * `back_red` - The red value of the background color.
/// * `back_green` - The green value of the background color.
/// * `back_blue` - The blue value of the background color.
///
/// # Errors
///
/// * `Alloc` - The X server could not allocate the requested resources (no memory?).
/// * `Font` - The specified `source_font` or `mask_font` does not exist.
/// * `Value` - Either `source_char` or `mask_char` are not defined in `source_font` or `mask_font`, respectively.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateGlyphCursorRequest {
    pub cid: Cursor,
    pub source_font: Font,
    pub mask_font: Font,
    pub source_char: u16,
    pub mask_char: u16,
    pub fore_red: u16,
    pub fore_green: u16,
    pub fore_blue: u16,
    pub back_red: u16,
    pub back_green: u16,
    pub back_blue: u16,
}
impl_debug_if_no_extra_traits!(CreateGlyphCursorRequest, "CreateGlyphCursorRequest");
impl CreateGlyphCursorRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let cid_bytes = self.cid.serialize();
        let source_font_bytes = self.source_font.serialize();
        let mask_font_bytes = self.mask_font.serialize();
        let source_char_bytes = self.source_char.serialize();
        let mask_char_bytes = self.mask_char.serialize();
        let fore_red_bytes = self.fore_red.serialize();
        let fore_green_bytes = self.fore_green.serialize();
        let fore_blue_bytes = self.fore_blue.serialize();
        let back_red_bytes = self.back_red.serialize();
        let back_green_bytes = self.back_green.serialize();
        let back_blue_bytes = self.back_blue.serialize();
        let mut request0 = vec![
            CREATE_GLYPH_CURSOR_REQUEST,
            0,
            0,
            0,
            cid_bytes[0],
            cid_bytes[1],
            cid_bytes[2],
            cid_bytes[3],
            source_font_bytes[0],
            source_font_bytes[1],
            source_font_bytes[2],
            source_font_bytes[3],
            mask_font_bytes[0],
            mask_font_bytes[1],
            mask_font_bytes[2],
            mask_font_bytes[3],
            source_char_bytes[0],
            source_char_bytes[1],
            mask_char_bytes[0],
            mask_char_bytes[1],
            fore_red_bytes[0],
            fore_red_bytes[1],
            fore_green_bytes[0],
            fore_green_bytes[1],
            fore_blue_bytes[0],
            fore_blue_bytes[1],
            back_red_bytes[0],
            back_red_bytes[1],
            back_green_bytes[0],
            back_green_bytes[1],
            back_blue_bytes[0],
            back_blue_bytes[1],
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
        if header.major_opcode != CREATE_GLYPH_CURSOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cid, remaining) = Cursor::try_parse(value)?;
        let (source_font, remaining) = Font::try_parse(remaining)?;
        let (mask_font, remaining) = Font::try_parse(remaining)?;
        let (source_char, remaining) = u16::try_parse(remaining)?;
        let (mask_char, remaining) = u16::try_parse(remaining)?;
        let (fore_red, remaining) = u16::try_parse(remaining)?;
        let (fore_green, remaining) = u16::try_parse(remaining)?;
        let (fore_blue, remaining) = u16::try_parse(remaining)?;
        let (back_red, remaining) = u16::try_parse(remaining)?;
        let (back_green, remaining) = u16::try_parse(remaining)?;
        let (back_blue, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(CreateGlyphCursorRequest {
            cid,
            source_font,
            mask_font,
            source_char,
            mask_char,
            fore_red,
            fore_green,
            fore_blue,
            back_red,
            back_green,
            back_blue,
        })
    }
}
impl Request for CreateGlyphCursorRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CreateGlyphCursorRequest {
}

/// Opcode for the FreeCursor request
pub const FREE_CURSOR_REQUEST: u8 = 95;
/// Deletes a cursor.
///
/// Deletes the association between the cursor resource ID and the specified
/// cursor. The cursor is freed when no other resource references it.
///
/// # Fields
///
/// * `cursor` - The cursor to destroy.
///
/// # Errors
///
/// * `Cursor` - The specified cursor does not exist.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FreeCursorRequest {
    pub cursor: Cursor,
}
impl_debug_if_no_extra_traits!(FreeCursorRequest, "FreeCursorRequest");
impl FreeCursorRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let cursor_bytes = self.cursor.serialize();
        let mut request0 = vec![
            FREE_CURSOR_REQUEST,
            0,
            0,
            0,
            cursor_bytes[0],
            cursor_bytes[1],
            cursor_bytes[2],
            cursor_bytes[3],
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
        if header.major_opcode != FREE_CURSOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cursor, remaining) = Cursor::try_parse(value)?;
        let _ = remaining;
        Ok(FreeCursorRequest {
            cursor,
        })
    }
}
impl Request for FreeCursorRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for FreeCursorRequest {
}

/// Opcode for the RecolorCursor request
pub const RECOLOR_CURSOR_REQUEST: u8 = 96;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RecolorCursorRequest {
    pub cursor: Cursor,
    pub fore_red: u16,
    pub fore_green: u16,
    pub fore_blue: u16,
    pub back_red: u16,
    pub back_green: u16,
    pub back_blue: u16,
}
impl_debug_if_no_extra_traits!(RecolorCursorRequest, "RecolorCursorRequest");
impl RecolorCursorRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let cursor_bytes = self.cursor.serialize();
        let fore_red_bytes = self.fore_red.serialize();
        let fore_green_bytes = self.fore_green.serialize();
        let fore_blue_bytes = self.fore_blue.serialize();
        let back_red_bytes = self.back_red.serialize();
        let back_green_bytes = self.back_green.serialize();
        let back_blue_bytes = self.back_blue.serialize();
        let mut request0 = vec![
            RECOLOR_CURSOR_REQUEST,
            0,
            0,
            0,
            cursor_bytes[0],
            cursor_bytes[1],
            cursor_bytes[2],
            cursor_bytes[3],
            fore_red_bytes[0],
            fore_red_bytes[1],
            fore_green_bytes[0],
            fore_green_bytes[1],
            fore_blue_bytes[0],
            fore_blue_bytes[1],
            back_red_bytes[0],
            back_red_bytes[1],
            back_green_bytes[0],
            back_green_bytes[1],
            back_blue_bytes[0],
            back_blue_bytes[1],
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
        if header.major_opcode != RECOLOR_CURSOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (cursor, remaining) = Cursor::try_parse(value)?;
        let (fore_red, remaining) = u16::try_parse(remaining)?;
        let (fore_green, remaining) = u16::try_parse(remaining)?;
        let (fore_blue, remaining) = u16::try_parse(remaining)?;
        let (back_red, remaining) = u16::try_parse(remaining)?;
        let (back_green, remaining) = u16::try_parse(remaining)?;
        let (back_blue, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(RecolorCursorRequest {
            cursor,
            fore_red,
            fore_green,
            fore_blue,
            back_red,
            back_green,
            back_blue,
        })
    }
}
impl Request for RecolorCursorRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for RecolorCursorRequest {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryShapeOf(u8);
impl QueryShapeOf {
    pub const LARGEST_CURSOR: Self = Self(0);
    pub const FASTEST_TILE: Self = Self(1);
    pub const FASTEST_STIPPLE: Self = Self(2);
}
impl From<QueryShapeOf> for u8 {
    #[inline]
    fn from(input: QueryShapeOf) -> Self {
        input.0
    }
}
impl From<QueryShapeOf> for Option<u8> {
    #[inline]
    fn from(input: QueryShapeOf) -> Self {
        Some(input.0)
    }
}
impl From<QueryShapeOf> for u16 {
    #[inline]
    fn from(input: QueryShapeOf) -> Self {
        u16::from(input.0)
    }
}
impl From<QueryShapeOf> for Option<u16> {
    #[inline]
    fn from(input: QueryShapeOf) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<QueryShapeOf> for u32 {
    #[inline]
    fn from(input: QueryShapeOf) -> Self {
        u32::from(input.0)
    }
}
impl From<QueryShapeOf> for Option<u32> {
    #[inline]
    fn from(input: QueryShapeOf) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for QueryShapeOf {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for QueryShapeOf  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::LARGEST_CURSOR.0.into(), "LARGEST_CURSOR", "LargestCursor"),
            (Self::FASTEST_TILE.0.into(), "FASTEST_TILE", "FastestTile"),
            (Self::FASTEST_STIPPLE.0.into(), "FASTEST_STIPPLE", "FastestStipple"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the QueryBestSize request
pub const QUERY_BEST_SIZE_REQUEST: u8 = 97;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryBestSizeRequest {
    pub class: QueryShapeOf,
    pub drawable: Drawable,
    pub width: u16,
    pub height: u16,
}
impl_debug_if_no_extra_traits!(QueryBestSizeRequest, "QueryBestSizeRequest");
impl QueryBestSizeRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let class_bytes = u8::from(self.class).serialize();
        let drawable_bytes = self.drawable.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let mut request0 = vec![
            QUERY_BEST_SIZE_REQUEST,
            class_bytes[0],
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
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
        if header.major_opcode != QUERY_BEST_SIZE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (class, remaining) = u8::try_parse(remaining)?;
        let class = class.into();
        let _ = remaining;
        let (drawable, remaining) = Drawable::try_parse(value)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(QueryBestSizeRequest {
            class,
            drawable,
            width,
            height,
        })
    }
}
impl Request for QueryBestSizeRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryBestSizeRequest {
    type Reply = QueryBestSizeReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryBestSizeReply {
    pub sequence: u16,
    pub length: u32,
    pub width: u16,
    pub height: u16,
}
impl_debug_if_no_extra_traits!(QueryBestSizeReply, "QueryBestSizeReply");
impl TryParse for QueryBestSizeReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryBestSizeReply { sequence, length, width, height };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryBestSizeReply {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
    }
}

/// Opcode for the QueryExtension request
pub const QUERY_EXTENSION_REQUEST: u8 = 98;
/// check if extension is present.
///
/// Determines if the specified extension is present on this X11 server.
///
/// Every extension has a unique `major_opcode` to identify requests, the minor
/// opcodes and request formats are extension-specific. If the extension provides
/// events and errors, the `first_event` and `first_error` fields in the reply are
/// set accordingly.
///
/// There should rarely be a need to use this request directly, XCB provides the
/// `xcb_get_extension_data` function instead.
///
/// # Fields
///
/// * `name` - The name of the extension to query, for example "RANDR". This is case
/// sensitive!
///
/// # See
///
/// * `xdpyinfo`: program
/// * `xcb_get_extension_data`: function
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryExtensionRequest<'input> {
    pub name: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(QueryExtensionRequest<'_>, "QueryExtensionRequest");
impl<'input> QueryExtensionRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let name_len = u16::try_from(self.name.len()).expect("`name` has too many elements");
        let name_len_bytes = name_len.serialize();
        let mut request0 = vec![
            QUERY_EXTENSION_REQUEST,
            0,
            0,
            0,
            name_len_bytes[0],
            name_len_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.name.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.name, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != QUERY_EXTENSION_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (name_len, remaining) = u16::try_parse(value)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, name_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(QueryExtensionRequest {
            name: Cow::Borrowed(name),
        })
    }
    /// Clone all borrowed data in this QueryExtensionRequest.
    pub fn into_owned(self) -> QueryExtensionRequest<'static> {
        QueryExtensionRequest {
            name: Cow::Owned(self.name.into_owned()),
        }
    }
}
impl<'input> Request for QueryExtensionRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for QueryExtensionRequest<'input> {
    type Reply = QueryExtensionReply;
}

/// # Fields
///
/// * `present` - Whether the extension is present on this X11 server.
/// * `major_opcode` - The major opcode for requests.
/// * `first_event` - The first event code, if any.
/// * `first_error` - The first error code, if any.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryExtensionReply {
    pub sequence: u16,
    pub length: u32,
    pub present: bool,
    pub major_opcode: u8,
    pub first_event: u8,
    pub first_error: u8,
}
impl_debug_if_no_extra_traits!(QueryExtensionReply, "QueryExtensionReply");
impl TryParse for QueryExtensionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (present, remaining) = bool::try_parse(remaining)?;
        let (major_opcode, remaining) = u8::try_parse(remaining)?;
        let (first_event, remaining) = u8::try_parse(remaining)?;
        let (first_error, remaining) = u8::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryExtensionReply { sequence, length, present, major_opcode, first_event, first_error };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryExtensionReply {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let present_bytes = self.present.serialize();
        let major_opcode_bytes = self.major_opcode.serialize();
        let first_event_bytes = self.first_event.serialize();
        let first_error_bytes = self.first_error.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            present_bytes[0],
            major_opcode_bytes[0],
            first_event_bytes[0],
            first_error_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.present.serialize_into(bytes);
        self.major_opcode.serialize_into(bytes);
        self.first_event.serialize_into(bytes);
        self.first_error.serialize_into(bytes);
    }
}

/// Opcode for the ListExtensions request
pub const LIST_EXTENSIONS_REQUEST: u8 = 99;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListExtensionsRequest;
impl_debug_if_no_extra_traits!(ListExtensionsRequest, "ListExtensionsRequest");
impl ListExtensionsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            LIST_EXTENSIONS_REQUEST,
            0,
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
        if header.major_opcode != LIST_EXTENSIONS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let _ = value;
        Ok(ListExtensionsRequest
        )
    }
}
impl Request for ListExtensionsRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for ListExtensionsRequest {
    type Reply = ListExtensionsReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListExtensionsReply {
    pub sequence: u16,
    pub length: u32,
    pub names: Vec<Str>,
}
impl_debug_if_no_extra_traits!(ListExtensionsReply, "ListExtensionsReply");
impl TryParse for ListExtensionsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (names_len, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(24..).ok_or(ParseError::InsufficientData)?;
        let (names, remaining) = crate::x11_utils::parse_list::<Str>(remaining, names_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = ListExtensionsReply { sequence, length, names };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ListExtensionsReply {
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
        let names_len = u8::try_from(self.names.len()).expect("`names` has too many elements");
        names_len.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 24]);
        self.names.serialize_into(bytes);
    }
}
impl ListExtensionsReply {
    /// Get the value of the `names_len` field.
    ///
    /// The `names_len` field is used as the length field of the `names` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn names_len(&self) -> u8 {
        self.names.len()
            .try_into().unwrap()
    }
}

/// Opcode for the ChangeKeyboardMapping request
pub const CHANGE_KEYBOARD_MAPPING_REQUEST: u8 = 100;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeKeyboardMappingRequest<'input> {
    pub keycode_count: u8,
    pub first_keycode: Keycode,
    pub keysyms_per_keycode: u8,
    pub keysyms: Cow<'input, [Keysym]>,
}
impl_debug_if_no_extra_traits!(ChangeKeyboardMappingRequest<'_>, "ChangeKeyboardMappingRequest");
impl<'input> ChangeKeyboardMappingRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let keycode_count_bytes = self.keycode_count.serialize();
        let first_keycode_bytes = self.first_keycode.serialize();
        let keysyms_per_keycode_bytes = self.keysyms_per_keycode.serialize();
        let mut request0 = vec![
            CHANGE_KEYBOARD_MAPPING_REQUEST,
            keycode_count_bytes[0],
            0,
            0,
            first_keycode_bytes[0],
            keysyms_per_keycode_bytes[0],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(self.keysyms.len(), usize::try_from(u32::from(self.keycode_count).checked_mul(u32::from(self.keysyms_per_keycode)).unwrap()).unwrap(), "`keysyms` has an incorrect length");
        let keysyms_bytes = self.keysyms.serialize();
        let length_so_far = length_so_far + keysyms_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), keysyms_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != CHANGE_KEYBOARD_MAPPING_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (keycode_count, remaining) = u8::try_parse(remaining)?;
        let _ = remaining;
        let (first_keycode, remaining) = Keycode::try_parse(value)?;
        let (keysyms_per_keycode, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (keysyms, remaining) = crate::x11_utils::parse_list::<Keysym>(remaining, u32::from(keycode_count).checked_mul(u32::from(keysyms_per_keycode)).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let _ = remaining;
        Ok(ChangeKeyboardMappingRequest {
            keycode_count,
            first_keycode,
            keysyms_per_keycode,
            keysyms: Cow::Owned(keysyms),
        })
    }
    /// Clone all borrowed data in this ChangeKeyboardMappingRequest.
    pub fn into_owned(self) -> ChangeKeyboardMappingRequest<'static> {
        ChangeKeyboardMappingRequest {
            keycode_count: self.keycode_count,
            first_keycode: self.first_keycode,
            keysyms_per_keycode: self.keysyms_per_keycode,
            keysyms: Cow::Owned(self.keysyms.into_owned()),
        }
    }
}
impl<'input> Request for ChangeKeyboardMappingRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for ChangeKeyboardMappingRequest<'input> {
}

/// Opcode for the GetKeyboardMapping request
pub const GET_KEYBOARD_MAPPING_REQUEST: u8 = 101;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKeyboardMappingRequest {
    pub first_keycode: Keycode,
    pub count: u8,
}
impl_debug_if_no_extra_traits!(GetKeyboardMappingRequest, "GetKeyboardMappingRequest");
impl GetKeyboardMappingRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let first_keycode_bytes = self.first_keycode.serialize();
        let count_bytes = self.count.serialize();
        let mut request0 = vec![
            GET_KEYBOARD_MAPPING_REQUEST,
            0,
            0,
            0,
            first_keycode_bytes[0],
            count_bytes[0],
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
        if header.major_opcode != GET_KEYBOARD_MAPPING_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (first_keycode, remaining) = Keycode::try_parse(value)?;
        let (count, remaining) = u8::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetKeyboardMappingRequest {
            first_keycode,
            count,
        })
    }
}
impl Request for GetKeyboardMappingRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetKeyboardMappingRequest {
    type Reply = GetKeyboardMappingReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKeyboardMappingReply {
    pub keysyms_per_keycode: u8,
    pub sequence: u16,
    pub keysyms: Vec<Keysym>,
}
impl_debug_if_no_extra_traits!(GetKeyboardMappingReply, "GetKeyboardMappingReply");
impl TryParse for GetKeyboardMappingReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (keysyms_per_keycode, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(24..).ok_or(ParseError::InsufficientData)?;
        let (keysyms, remaining) = crate::x11_utils::parse_list::<Keysym>(remaining, length.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetKeyboardMappingReply { keysyms_per_keycode, sequence, keysyms };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetKeyboardMappingReply {
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
        self.keysyms_per_keycode.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        let length = u32::try_from(self.keysyms.len()).expect("`keysyms` has too many elements");
        length.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 24]);
        self.keysyms.serialize_into(bytes);
    }
}
impl GetKeyboardMappingReply {
    /// Get the value of the `length` field.
    ///
    /// The `length` field is used as the length field of the `keysyms` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn length(&self) -> u32 {
        self.keysyms.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KB(u32);
impl KB {
    pub const KEY_CLICK_PERCENT: Self = Self(1 << 0);
    pub const BELL_PERCENT: Self = Self(1 << 1);
    pub const BELL_PITCH: Self = Self(1 << 2);
    pub const BELL_DURATION: Self = Self(1 << 3);
    pub const LED: Self = Self(1 << 4);
    pub const LED_MODE: Self = Self(1 << 5);
    pub const KEY: Self = Self(1 << 6);
    pub const AUTO_REPEAT_MODE: Self = Self(1 << 7);
}
impl From<KB> for u32 {
    #[inline]
    fn from(input: KB) -> Self {
        input.0
    }
}
impl From<KB> for Option<u32> {
    #[inline]
    fn from(input: KB) -> Self {
        Some(input.0)
    }
}
impl From<u8> for KB {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for KB {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for KB {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for KB  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::KEY_CLICK_PERCENT.0, "KEY_CLICK_PERCENT", "KeyClickPercent"),
            (Self::BELL_PERCENT.0, "BELL_PERCENT", "BellPercent"),
            (Self::BELL_PITCH.0, "BELL_PITCH", "BellPitch"),
            (Self::BELL_DURATION.0, "BELL_DURATION", "BellDuration"),
            (Self::LED.0, "LED", "Led"),
            (Self::LED_MODE.0, "LED_MODE", "LedMode"),
            (Self::KEY.0, "KEY", "Key"),
            (Self::AUTO_REPEAT_MODE.0, "AUTO_REPEAT_MODE", "AutoRepeatMode"),
        ];
        pretty_print_bitmask(fmt, self.0, &variants)
    }
}
bitmask_binop!(KB, u32);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LedMode(u32);
impl LedMode {
    pub const OFF: Self = Self(0);
    pub const ON: Self = Self(1);
}
impl From<LedMode> for u32 {
    #[inline]
    fn from(input: LedMode) -> Self {
        input.0
    }
}
impl From<LedMode> for Option<u32> {
    #[inline]
    fn from(input: LedMode) -> Self {
        Some(input.0)
    }
}
impl From<u8> for LedMode {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for LedMode {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for LedMode {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for LedMode  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::OFF.0, "OFF", "Off"),
            (Self::ON.0, "ON", "On"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AutoRepeatMode(u32);
impl AutoRepeatMode {
    pub const OFF: Self = Self(0);
    pub const ON: Self = Self(1);
    pub const DEFAULT: Self = Self(2);
}
impl From<AutoRepeatMode> for u32 {
    #[inline]
    fn from(input: AutoRepeatMode) -> Self {
        input.0
    }
}
impl From<AutoRepeatMode> for Option<u32> {
    #[inline]
    fn from(input: AutoRepeatMode) -> Self {
        Some(input.0)
    }
}
impl From<u8> for AutoRepeatMode {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for AutoRepeatMode {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for AutoRepeatMode {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for AutoRepeatMode  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::OFF.0, "OFF", "Off"),
            (Self::ON.0, "ON", "On"),
            (Self::DEFAULT.0, "DEFAULT", "Default"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

/// Auxiliary and optional information for the `change_keyboard_control` function
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeKeyboardControlAux {
    pub key_click_percent: Option<i32>,
    pub bell_percent: Option<i32>,
    pub bell_pitch: Option<i32>,
    pub bell_duration: Option<i32>,
    pub led: Option<u32>,
    pub led_mode: Option<LedMode>,
    pub key: Option<Keycode32>,
    pub auto_repeat_mode: Option<AutoRepeatMode>,
}
impl_debug_if_no_extra_traits!(ChangeKeyboardControlAux, "ChangeKeyboardControlAux");
impl ChangeKeyboardControlAux {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], value_mask: u32) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u32::from(value_mask);
        let mut outer_remaining = value;
        let key_click_percent = if switch_expr & u32::from(KB::KEY_CLICK_PERCENT) != 0 {
            let remaining = outer_remaining;
            let (key_click_percent, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(key_click_percent)
        } else {
            None
        };
        let bell_percent = if switch_expr & u32::from(KB::BELL_PERCENT) != 0 {
            let remaining = outer_remaining;
            let (bell_percent, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(bell_percent)
        } else {
            None
        };
        let bell_pitch = if switch_expr & u32::from(KB::BELL_PITCH) != 0 {
            let remaining = outer_remaining;
            let (bell_pitch, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(bell_pitch)
        } else {
            None
        };
        let bell_duration = if switch_expr & u32::from(KB::BELL_DURATION) != 0 {
            let remaining = outer_remaining;
            let (bell_duration, remaining) = i32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(bell_duration)
        } else {
            None
        };
        let led = if switch_expr & u32::from(KB::LED) != 0 {
            let remaining = outer_remaining;
            let (led, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(led)
        } else {
            None
        };
        let led_mode = if switch_expr & u32::from(KB::LED_MODE) != 0 {
            let remaining = outer_remaining;
            let (led_mode, remaining) = u32::try_parse(remaining)?;
            let led_mode = led_mode.into();
            outer_remaining = remaining;
            Some(led_mode)
        } else {
            None
        };
        let key = if switch_expr & u32::from(KB::KEY) != 0 {
            let remaining = outer_remaining;
            let (key, remaining) = Keycode32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(key)
        } else {
            None
        };
        let auto_repeat_mode = if switch_expr & u32::from(KB::AUTO_REPEAT_MODE) != 0 {
            let remaining = outer_remaining;
            let (auto_repeat_mode, remaining) = u32::try_parse(remaining)?;
            let auto_repeat_mode = auto_repeat_mode.into();
            outer_remaining = remaining;
            Some(auto_repeat_mode)
        } else {
            None
        };
        let result = ChangeKeyboardControlAux { key_click_percent, bell_percent, bell_pitch, bell_duration, led, led_mode, key, auto_repeat_mode };
        Ok((result, outer_remaining))
    }
}
impl ChangeKeyboardControlAux {
    #[allow(dead_code)]
    fn serialize(&self, value_mask: u32) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result, u32::from(value_mask));
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>, value_mask: u32) {
        assert_eq!(self.switch_expr(), u32::from(value_mask), "switch `value_list` has an inconsistent discriminant");
        if let Some(key_click_percent) = self.key_click_percent {
            key_click_percent.serialize_into(bytes);
        }
        if let Some(bell_percent) = self.bell_percent {
            bell_percent.serialize_into(bytes);
        }
        if let Some(bell_pitch) = self.bell_pitch {
            bell_pitch.serialize_into(bytes);
        }
        if let Some(bell_duration) = self.bell_duration {
            bell_duration.serialize_into(bytes);
        }
        if let Some(led) = self.led {
            led.serialize_into(bytes);
        }
        if let Some(led_mode) = self.led_mode {
            u32::from(led_mode).serialize_into(bytes);
        }
        if let Some(key) = self.key {
            key.serialize_into(bytes);
        }
        if let Some(auto_repeat_mode) = self.auto_repeat_mode {
            u32::from(auto_repeat_mode).serialize_into(bytes);
        }
    }
}
impl ChangeKeyboardControlAux {
    fn switch_expr(&self) -> u32 {
        let mut expr_value = 0;
        if self.key_click_percent.is_some() {
            expr_value |= u32::from(KB::KEY_CLICK_PERCENT);
        }
        if self.bell_percent.is_some() {
            expr_value |= u32::from(KB::BELL_PERCENT);
        }
        if self.bell_pitch.is_some() {
            expr_value |= u32::from(KB::BELL_PITCH);
        }
        if self.bell_duration.is_some() {
            expr_value |= u32::from(KB::BELL_DURATION);
        }
        if self.led.is_some() {
            expr_value |= u32::from(KB::LED);
        }
        if self.led_mode.is_some() {
            expr_value |= u32::from(KB::LED_MODE);
        }
        if self.key.is_some() {
            expr_value |= u32::from(KB::KEY);
        }
        if self.auto_repeat_mode.is_some() {
            expr_value |= u32::from(KB::AUTO_REPEAT_MODE);
        }
        expr_value
    }
}
impl ChangeKeyboardControlAux {
    /// Create a new instance with all fields unset / not present.
    pub fn new() -> Self {
        Default::default()
    }
    /// Set the `key_click_percent` field of this structure.
    #[must_use]
    pub fn key_click_percent<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.key_click_percent = value.into();
        self
    }
    /// Set the `bell_percent` field of this structure.
    #[must_use]
    pub fn bell_percent<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.bell_percent = value.into();
        self
    }
    /// Set the `bell_pitch` field of this structure.
    #[must_use]
    pub fn bell_pitch<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.bell_pitch = value.into();
        self
    }
    /// Set the `bell_duration` field of this structure.
    #[must_use]
    pub fn bell_duration<I>(mut self, value: I) -> Self where I: Into<Option<i32>> {
        self.bell_duration = value.into();
        self
    }
    /// Set the `led` field of this structure.
    #[must_use]
    pub fn led<I>(mut self, value: I) -> Self where I: Into<Option<u32>> {
        self.led = value.into();
        self
    }
    /// Set the `led_mode` field of this structure.
    #[must_use]
    pub fn led_mode<I>(mut self, value: I) -> Self where I: Into<Option<LedMode>> {
        self.led_mode = value.into();
        self
    }
    /// Set the `key` field of this structure.
    #[must_use]
    pub fn key<I>(mut self, value: I) -> Self where I: Into<Option<Keycode32>> {
        self.key = value.into();
        self
    }
    /// Set the `auto_repeat_mode` field of this structure.
    #[must_use]
    pub fn auto_repeat_mode<I>(mut self, value: I) -> Self where I: Into<Option<AutoRepeatMode>> {
        self.auto_repeat_mode = value.into();
        self
    }
}

/// Opcode for the ChangeKeyboardControl request
pub const CHANGE_KEYBOARD_CONTROL_REQUEST: u8 = 102;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeKeyboardControlRequest<'input> {
    pub value_list: Cow<'input, ChangeKeyboardControlAux>,
}
impl_debug_if_no_extra_traits!(ChangeKeyboardControlRequest<'_>, "ChangeKeyboardControlRequest");
impl<'input> ChangeKeyboardControlRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let value_mask: u32 = self.value_list.switch_expr();
        let value_mask_bytes = value_mask.serialize();
        let mut request0 = vec![
            CHANGE_KEYBOARD_CONTROL_REQUEST,
            0,
            0,
            0,
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
        if header.major_opcode != CHANGE_KEYBOARD_CONTROL_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (value_mask, remaining) = u32::try_parse(value)?;
        let (value_list, remaining) = ChangeKeyboardControlAux::try_parse(remaining, u32::from(value_mask))?;
        let _ = remaining;
        Ok(ChangeKeyboardControlRequest {
            value_list: Cow::Owned(value_list),
        })
    }
    /// Clone all borrowed data in this ChangeKeyboardControlRequest.
    pub fn into_owned(self) -> ChangeKeyboardControlRequest<'static> {
        ChangeKeyboardControlRequest {
            value_list: Cow::Owned(self.value_list.into_owned()),
        }
    }
}
impl<'input> Request for ChangeKeyboardControlRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for ChangeKeyboardControlRequest<'input> {
}

/// Opcode for the GetKeyboardControl request
pub const GET_KEYBOARD_CONTROL_REQUEST: u8 = 103;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKeyboardControlRequest;
impl_debug_if_no_extra_traits!(GetKeyboardControlRequest, "GetKeyboardControlRequest");
impl GetKeyboardControlRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            GET_KEYBOARD_CONTROL_REQUEST,
            0,
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
        if header.major_opcode != GET_KEYBOARD_CONTROL_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let _ = value;
        Ok(GetKeyboardControlRequest
        )
    }
}
impl Request for GetKeyboardControlRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetKeyboardControlRequest {
    type Reply = GetKeyboardControlReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetKeyboardControlReply {
    pub global_auto_repeat: AutoRepeatMode,
    pub sequence: u16,
    pub length: u32,
    pub led_mask: u32,
    pub key_click_percent: u8,
    pub bell_percent: u8,
    pub bell_pitch: u16,
    pub bell_duration: u16,
    pub auto_repeats: [u8; 32],
}
impl_debug_if_no_extra_traits!(GetKeyboardControlReply, "GetKeyboardControlReply");
impl TryParse for GetKeyboardControlReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (global_auto_repeat, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (led_mask, remaining) = u32::try_parse(remaining)?;
        let (key_click_percent, remaining) = u8::try_parse(remaining)?;
        let (bell_percent, remaining) = u8::try_parse(remaining)?;
        let (bell_pitch, remaining) = u16::try_parse(remaining)?;
        let (bell_duration, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (auto_repeats, remaining) = crate::x11_utils::parse_u8_array::<32>(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let global_auto_repeat = global_auto_repeat.into();
        let result = GetKeyboardControlReply { global_auto_repeat, sequence, length, led_mask, key_click_percent, bell_percent, bell_pitch, bell_duration, auto_repeats };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetKeyboardControlReply {
    type Bytes = [u8; 52];
    fn serialize(&self) -> [u8; 52] {
        let response_type_bytes = &[1];
        let global_auto_repeat_bytes = (u32::from(self.global_auto_repeat) as u8).serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let led_mask_bytes = self.led_mask.serialize();
        let key_click_percent_bytes = self.key_click_percent.serialize();
        let bell_percent_bytes = self.bell_percent.serialize();
        let bell_pitch_bytes = self.bell_pitch.serialize();
        let bell_duration_bytes = self.bell_duration.serialize();
        [
            response_type_bytes[0],
            global_auto_repeat_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            led_mask_bytes[0],
            led_mask_bytes[1],
            led_mask_bytes[2],
            led_mask_bytes[3],
            key_click_percent_bytes[0],
            bell_percent_bytes[0],
            bell_pitch_bytes[0],
            bell_pitch_bytes[1],
            bell_duration_bytes[0],
            bell_duration_bytes[1],
            0,
            0,
            self.auto_repeats[0],
            self.auto_repeats[1],
            self.auto_repeats[2],
            self.auto_repeats[3],
            self.auto_repeats[4],
            self.auto_repeats[5],
            self.auto_repeats[6],
            self.auto_repeats[7],
            self.auto_repeats[8],
            self.auto_repeats[9],
            self.auto_repeats[10],
            self.auto_repeats[11],
            self.auto_repeats[12],
            self.auto_repeats[13],
            self.auto_repeats[14],
            self.auto_repeats[15],
            self.auto_repeats[16],
            self.auto_repeats[17],
            self.auto_repeats[18],
            self.auto_repeats[19],
            self.auto_repeats[20],
            self.auto_repeats[21],
            self.auto_repeats[22],
            self.auto_repeats[23],
            self.auto_repeats[24],
            self.auto_repeats[25],
            self.auto_repeats[26],
            self.auto_repeats[27],
            self.auto_repeats[28],
            self.auto_repeats[29],
            self.auto_repeats[30],
            self.auto_repeats[31],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(52);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        (u32::from(self.global_auto_repeat) as u8).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.led_mask.serialize_into(bytes);
        self.key_click_percent.serialize_into(bytes);
        self.bell_percent.serialize_into(bytes);
        self.bell_pitch.serialize_into(bytes);
        self.bell_duration.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        bytes.extend_from_slice(&self.auto_repeats);
    }
}

/// Opcode for the Bell request
pub const BELL_REQUEST: u8 = 104;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BellRequest {
    pub percent: i8,
}
impl_debug_if_no_extra_traits!(BellRequest, "BellRequest");
impl BellRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let percent_bytes = self.percent.serialize();
        let mut request0 = vec![
            BELL_REQUEST,
            percent_bytes[0],
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
        if header.major_opcode != BELL_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (percent, remaining) = i8::try_parse(remaining)?;
        let _ = remaining;
        let _ = value;
        Ok(BellRequest {
            percent,
        })
    }
}
impl Request for BellRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for BellRequest {
}

/// Opcode for the ChangePointerControl request
pub const CHANGE_POINTER_CONTROL_REQUEST: u8 = 105;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangePointerControlRequest {
    pub acceleration_numerator: i16,
    pub acceleration_denominator: i16,
    pub threshold: i16,
    pub do_acceleration: bool,
    pub do_threshold: bool,
}
impl_debug_if_no_extra_traits!(ChangePointerControlRequest, "ChangePointerControlRequest");
impl ChangePointerControlRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let acceleration_numerator_bytes = self.acceleration_numerator.serialize();
        let acceleration_denominator_bytes = self.acceleration_denominator.serialize();
        let threshold_bytes = self.threshold.serialize();
        let do_acceleration_bytes = self.do_acceleration.serialize();
        let do_threshold_bytes = self.do_threshold.serialize();
        let mut request0 = vec![
            CHANGE_POINTER_CONTROL_REQUEST,
            0,
            0,
            0,
            acceleration_numerator_bytes[0],
            acceleration_numerator_bytes[1],
            acceleration_denominator_bytes[0],
            acceleration_denominator_bytes[1],
            threshold_bytes[0],
            threshold_bytes[1],
            do_acceleration_bytes[0],
            do_threshold_bytes[0],
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
        if header.major_opcode != CHANGE_POINTER_CONTROL_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (acceleration_numerator, remaining) = i16::try_parse(value)?;
        let (acceleration_denominator, remaining) = i16::try_parse(remaining)?;
        let (threshold, remaining) = i16::try_parse(remaining)?;
        let (do_acceleration, remaining) = bool::try_parse(remaining)?;
        let (do_threshold, remaining) = bool::try_parse(remaining)?;
        let _ = remaining;
        Ok(ChangePointerControlRequest {
            acceleration_numerator,
            acceleration_denominator,
            threshold,
            do_acceleration,
            do_threshold,
        })
    }
}
impl Request for ChangePointerControlRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for ChangePointerControlRequest {
}

/// Opcode for the GetPointerControl request
pub const GET_POINTER_CONTROL_REQUEST: u8 = 106;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPointerControlRequest;
impl_debug_if_no_extra_traits!(GetPointerControlRequest, "GetPointerControlRequest");
impl GetPointerControlRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            GET_POINTER_CONTROL_REQUEST,
            0,
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
        if header.major_opcode != GET_POINTER_CONTROL_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let _ = value;
        Ok(GetPointerControlRequest
        )
    }
}
impl Request for GetPointerControlRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetPointerControlRequest {
    type Reply = GetPointerControlReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPointerControlReply {
    pub sequence: u16,
    pub length: u32,
    pub acceleration_numerator: u16,
    pub acceleration_denominator: u16,
    pub threshold: u16,
}
impl_debug_if_no_extra_traits!(GetPointerControlReply, "GetPointerControlReply");
impl TryParse for GetPointerControlReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (acceleration_numerator, remaining) = u16::try_parse(remaining)?;
        let (acceleration_denominator, remaining) = u16::try_parse(remaining)?;
        let (threshold, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(18..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetPointerControlReply { sequence, length, acceleration_numerator, acceleration_denominator, threshold };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetPointerControlReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let acceleration_numerator_bytes = self.acceleration_numerator.serialize();
        let acceleration_denominator_bytes = self.acceleration_denominator.serialize();
        let threshold_bytes = self.threshold.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            acceleration_numerator_bytes[0],
            acceleration_numerator_bytes[1],
            acceleration_denominator_bytes[0],
            acceleration_denominator_bytes[1],
            threshold_bytes[0],
            threshold_bytes[1],
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
        self.acceleration_numerator.serialize_into(bytes);
        self.acceleration_denominator.serialize_into(bytes);
        self.threshold.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 18]);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Blanking(u8);
impl Blanking {
    pub const NOT_PREFERRED: Self = Self(0);
    pub const PREFERRED: Self = Self(1);
    pub const DEFAULT: Self = Self(2);
}
impl From<Blanking> for u8 {
    #[inline]
    fn from(input: Blanking) -> Self {
        input.0
    }
}
impl From<Blanking> for Option<u8> {
    #[inline]
    fn from(input: Blanking) -> Self {
        Some(input.0)
    }
}
impl From<Blanking> for u16 {
    #[inline]
    fn from(input: Blanking) -> Self {
        u16::from(input.0)
    }
}
impl From<Blanking> for Option<u16> {
    #[inline]
    fn from(input: Blanking) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Blanking> for u32 {
    #[inline]
    fn from(input: Blanking) -> Self {
        u32::from(input.0)
    }
}
impl From<Blanking> for Option<u32> {
    #[inline]
    fn from(input: Blanking) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Blanking {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Blanking  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NOT_PREFERRED.0.into(), "NOT_PREFERRED", "NotPreferred"),
            (Self::PREFERRED.0.into(), "PREFERRED", "Preferred"),
            (Self::DEFAULT.0.into(), "DEFAULT", "Default"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Exposures(u8);
impl Exposures {
    pub const NOT_ALLOWED: Self = Self(0);
    pub const ALLOWED: Self = Self(1);
    pub const DEFAULT: Self = Self(2);
}
impl From<Exposures> for u8 {
    #[inline]
    fn from(input: Exposures) -> Self {
        input.0
    }
}
impl From<Exposures> for Option<u8> {
    #[inline]
    fn from(input: Exposures) -> Self {
        Some(input.0)
    }
}
impl From<Exposures> for u16 {
    #[inline]
    fn from(input: Exposures) -> Self {
        u16::from(input.0)
    }
}
impl From<Exposures> for Option<u16> {
    #[inline]
    fn from(input: Exposures) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Exposures> for u32 {
    #[inline]
    fn from(input: Exposures) -> Self {
        u32::from(input.0)
    }
}
impl From<Exposures> for Option<u32> {
    #[inline]
    fn from(input: Exposures) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Exposures {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Exposures  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NOT_ALLOWED.0.into(), "NOT_ALLOWED", "NotAllowed"),
            (Self::ALLOWED.0.into(), "ALLOWED", "Allowed"),
            (Self::DEFAULT.0.into(), "DEFAULT", "Default"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the SetScreenSaver request
pub const SET_SCREEN_SAVER_REQUEST: u8 = 107;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetScreenSaverRequest {
    pub timeout: i16,
    pub interval: i16,
    pub prefer_blanking: Blanking,
    pub allow_exposures: Exposures,
}
impl_debug_if_no_extra_traits!(SetScreenSaverRequest, "SetScreenSaverRequest");
impl SetScreenSaverRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let timeout_bytes = self.timeout.serialize();
        let interval_bytes = self.interval.serialize();
        let prefer_blanking_bytes = u8::from(self.prefer_blanking).serialize();
        let allow_exposures_bytes = u8::from(self.allow_exposures).serialize();
        let mut request0 = vec![
            SET_SCREEN_SAVER_REQUEST,
            0,
            0,
            0,
            timeout_bytes[0],
            timeout_bytes[1],
            interval_bytes[0],
            interval_bytes[1],
            prefer_blanking_bytes[0],
            allow_exposures_bytes[0],
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
        if header.major_opcode != SET_SCREEN_SAVER_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (timeout, remaining) = i16::try_parse(value)?;
        let (interval, remaining) = i16::try_parse(remaining)?;
        let (prefer_blanking, remaining) = u8::try_parse(remaining)?;
        let prefer_blanking = prefer_blanking.into();
        let (allow_exposures, remaining) = u8::try_parse(remaining)?;
        let allow_exposures = allow_exposures.into();
        let _ = remaining;
        Ok(SetScreenSaverRequest {
            timeout,
            interval,
            prefer_blanking,
            allow_exposures,
        })
    }
}
impl Request for SetScreenSaverRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SetScreenSaverRequest {
}

/// Opcode for the GetScreenSaver request
pub const GET_SCREEN_SAVER_REQUEST: u8 = 108;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetScreenSaverRequest;
impl_debug_if_no_extra_traits!(GetScreenSaverRequest, "GetScreenSaverRequest");
impl GetScreenSaverRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            GET_SCREEN_SAVER_REQUEST,
            0,
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
        if header.major_opcode != GET_SCREEN_SAVER_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let _ = value;
        Ok(GetScreenSaverRequest
        )
    }
}
impl Request for GetScreenSaverRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetScreenSaverRequest {
    type Reply = GetScreenSaverReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetScreenSaverReply {
    pub sequence: u16,
    pub length: u32,
    pub timeout: u16,
    pub interval: u16,
    pub prefer_blanking: Blanking,
    pub allow_exposures: Exposures,
}
impl_debug_if_no_extra_traits!(GetScreenSaverReply, "GetScreenSaverReply");
impl TryParse for GetScreenSaverReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (timeout, remaining) = u16::try_parse(remaining)?;
        let (interval, remaining) = u16::try_parse(remaining)?;
        let (prefer_blanking, remaining) = u8::try_parse(remaining)?;
        let (allow_exposures, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(18..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let prefer_blanking = prefer_blanking.into();
        let allow_exposures = allow_exposures.into();
        let result = GetScreenSaverReply { sequence, length, timeout, interval, prefer_blanking, allow_exposures };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetScreenSaverReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let timeout_bytes = self.timeout.serialize();
        let interval_bytes = self.interval.serialize();
        let prefer_blanking_bytes = u8::from(self.prefer_blanking).serialize();
        let allow_exposures_bytes = u8::from(self.allow_exposures).serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            timeout_bytes[0],
            timeout_bytes[1],
            interval_bytes[0],
            interval_bytes[1],
            prefer_blanking_bytes[0],
            allow_exposures_bytes[0],
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
        self.timeout.serialize_into(bytes);
        self.interval.serialize_into(bytes);
        u8::from(self.prefer_blanking).serialize_into(bytes);
        u8::from(self.allow_exposures).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 18]);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HostMode(u8);
impl HostMode {
    pub const INSERT: Self = Self(0);
    pub const DELETE: Self = Self(1);
}
impl From<HostMode> for u8 {
    #[inline]
    fn from(input: HostMode) -> Self {
        input.0
    }
}
impl From<HostMode> for Option<u8> {
    #[inline]
    fn from(input: HostMode) -> Self {
        Some(input.0)
    }
}
impl From<HostMode> for u16 {
    #[inline]
    fn from(input: HostMode) -> Self {
        u16::from(input.0)
    }
}
impl From<HostMode> for Option<u16> {
    #[inline]
    fn from(input: HostMode) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<HostMode> for u32 {
    #[inline]
    fn from(input: HostMode) -> Self {
        u32::from(input.0)
    }
}
impl From<HostMode> for Option<u32> {
    #[inline]
    fn from(input: HostMode) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for HostMode {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for HostMode  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::INSERT.0.into(), "INSERT", "Insert"),
            (Self::DELETE.0.into(), "DELETE", "Delete"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Family(u8);
impl Family {
    pub const INTERNET: Self = Self(0);
    pub const DEC_NET: Self = Self(1);
    pub const CHAOS: Self = Self(2);
    pub const SERVER_INTERPRETED: Self = Self(5);
    pub const INTERNET6: Self = Self(6);
}
impl From<Family> for u8 {
    #[inline]
    fn from(input: Family) -> Self {
        input.0
    }
}
impl From<Family> for Option<u8> {
    #[inline]
    fn from(input: Family) -> Self {
        Some(input.0)
    }
}
impl From<Family> for u16 {
    #[inline]
    fn from(input: Family) -> Self {
        u16::from(input.0)
    }
}
impl From<Family> for Option<u16> {
    #[inline]
    fn from(input: Family) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Family> for u32 {
    #[inline]
    fn from(input: Family) -> Self {
        u32::from(input.0)
    }
}
impl From<Family> for Option<u32> {
    #[inline]
    fn from(input: Family) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Family {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Family  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::INTERNET.0.into(), "INTERNET", "Internet"),
            (Self::DEC_NET.0.into(), "DEC_NET", "DECnet"),
            (Self::CHAOS.0.into(), "CHAOS", "Chaos"),
            (Self::SERVER_INTERPRETED.0.into(), "SERVER_INTERPRETED", "ServerInterpreted"),
            (Self::INTERNET6.0.into(), "INTERNET6", "Internet6"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the ChangeHosts request
pub const CHANGE_HOSTS_REQUEST: u8 = 109;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeHostsRequest<'input> {
    pub mode: HostMode,
    pub family: Family,
    pub address: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(ChangeHostsRequest<'_>, "ChangeHostsRequest");
impl<'input> ChangeHostsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let mode_bytes = u8::from(self.mode).serialize();
        let family_bytes = u8::from(self.family).serialize();
        let address_len = u16::try_from(self.address.len()).expect("`address` has too many elements");
        let address_len_bytes = address_len.serialize();
        let mut request0 = vec![
            CHANGE_HOSTS_REQUEST,
            mode_bytes[0],
            0,
            0,
            family_bytes[0],
            0,
            address_len_bytes[0],
            address_len_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.address.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.address, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != CHANGE_HOSTS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (mode, remaining) = u8::try_parse(remaining)?;
        let mode = mode.into();
        let _ = remaining;
        let (family, remaining) = u8::try_parse(value)?;
        let family = family.into();
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (address_len, remaining) = u16::try_parse(remaining)?;
        let (address, remaining) = crate::x11_utils::parse_u8_list(remaining, address_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(ChangeHostsRequest {
            mode,
            family,
            address: Cow::Borrowed(address),
        })
    }
    /// Clone all borrowed data in this ChangeHostsRequest.
    pub fn into_owned(self) -> ChangeHostsRequest<'static> {
        ChangeHostsRequest {
            mode: self.mode,
            family: self.family,
            address: Cow::Owned(self.address.into_owned()),
        }
    }
}
impl<'input> Request for ChangeHostsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for ChangeHostsRequest<'input> {
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Host {
    pub family: Family,
    pub address: Vec<u8>,
}
impl_debug_if_no_extra_traits!(Host, "Host");
impl TryParse for Host {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let value = remaining;
        let (family, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (address_len, remaining) = u16::try_parse(remaining)?;
        let (address, remaining) = crate::x11_utils::parse_u8_list(remaining, address_len.try_to_usize()?)?;
        let address = address.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let family = family.into();
        let result = Host { family, address };
        Ok((result, remaining))
    }
}
impl Serialize for Host {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        u8::from(self.family).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        let address_len = u16::try_from(self.address.len()).expect("`address` has too many elements");
        address_len.serialize_into(bytes);
        bytes.extend_from_slice(&self.address);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
    }
}
impl Host {
    /// Get the value of the `address_len` field.
    ///
    /// The `address_len` field is used as the length field of the `address` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn address_len(&self) -> u16 {
        self.address.len()
            .try_into().unwrap()
    }
}

/// Opcode for the ListHosts request
pub const LIST_HOSTS_REQUEST: u8 = 110;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListHostsRequest;
impl_debug_if_no_extra_traits!(ListHostsRequest, "ListHostsRequest");
impl ListHostsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            LIST_HOSTS_REQUEST,
            0,
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
        if header.major_opcode != LIST_HOSTS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let _ = value;
        Ok(ListHostsRequest
        )
    }
}
impl Request for ListHostsRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for ListHostsRequest {
    type Reply = ListHostsReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListHostsReply {
    pub mode: AccessControl,
    pub sequence: u16,
    pub length: u32,
    pub hosts: Vec<Host>,
}
impl_debug_if_no_extra_traits!(ListHostsReply, "ListHostsReply");
impl TryParse for ListHostsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (mode, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (hosts_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
        let (hosts, remaining) = crate::x11_utils::parse_list::<Host>(remaining, hosts_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let mode = mode.into();
        let result = ListHostsReply { mode, sequence, length, hosts };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ListHostsReply {
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
        u8::from(self.mode).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        let hosts_len = u16::try_from(self.hosts.len()).expect("`hosts` has too many elements");
        hosts_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
        self.hosts.serialize_into(bytes);
    }
}
impl ListHostsReply {
    /// Get the value of the `hosts_len` field.
    ///
    /// The `hosts_len` field is used as the length field of the `hosts` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn hosts_len(&self) -> u16 {
        self.hosts.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AccessControl(u8);
impl AccessControl {
    pub const DISABLE: Self = Self(0);
    pub const ENABLE: Self = Self(1);
}
impl From<AccessControl> for u8 {
    #[inline]
    fn from(input: AccessControl) -> Self {
        input.0
    }
}
impl From<AccessControl> for Option<u8> {
    #[inline]
    fn from(input: AccessControl) -> Self {
        Some(input.0)
    }
}
impl From<AccessControl> for u16 {
    #[inline]
    fn from(input: AccessControl) -> Self {
        u16::from(input.0)
    }
}
impl From<AccessControl> for Option<u16> {
    #[inline]
    fn from(input: AccessControl) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<AccessControl> for u32 {
    #[inline]
    fn from(input: AccessControl) -> Self {
        u32::from(input.0)
    }
}
impl From<AccessControl> for Option<u32> {
    #[inline]
    fn from(input: AccessControl) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for AccessControl {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for AccessControl  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::DISABLE.0.into(), "DISABLE", "Disable"),
            (Self::ENABLE.0.into(), "ENABLE", "Enable"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the SetAccessControl request
pub const SET_ACCESS_CONTROL_REQUEST: u8 = 111;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetAccessControlRequest {
    pub mode: AccessControl,
}
impl_debug_if_no_extra_traits!(SetAccessControlRequest, "SetAccessControlRequest");
impl SetAccessControlRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mode_bytes = u8::from(self.mode).serialize();
        let mut request0 = vec![
            SET_ACCESS_CONTROL_REQUEST,
            mode_bytes[0],
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
        if header.major_opcode != SET_ACCESS_CONTROL_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (mode, remaining) = u8::try_parse(remaining)?;
        let mode = mode.into();
        let _ = remaining;
        let _ = value;
        Ok(SetAccessControlRequest {
            mode,
        })
    }
}
impl Request for SetAccessControlRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SetAccessControlRequest {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CloseDown(u8);
impl CloseDown {
    pub const DESTROY_ALL: Self = Self(0);
    pub const RETAIN_PERMANENT: Self = Self(1);
    pub const RETAIN_TEMPORARY: Self = Self(2);
}
impl From<CloseDown> for u8 {
    #[inline]
    fn from(input: CloseDown) -> Self {
        input.0
    }
}
impl From<CloseDown> for Option<u8> {
    #[inline]
    fn from(input: CloseDown) -> Self {
        Some(input.0)
    }
}
impl From<CloseDown> for u16 {
    #[inline]
    fn from(input: CloseDown) -> Self {
        u16::from(input.0)
    }
}
impl From<CloseDown> for Option<u16> {
    #[inline]
    fn from(input: CloseDown) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<CloseDown> for u32 {
    #[inline]
    fn from(input: CloseDown) -> Self {
        u32::from(input.0)
    }
}
impl From<CloseDown> for Option<u32> {
    #[inline]
    fn from(input: CloseDown) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for CloseDown {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for CloseDown  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::DESTROY_ALL.0.into(), "DESTROY_ALL", "DestroyAll"),
            (Self::RETAIN_PERMANENT.0.into(), "RETAIN_PERMANENT", "RetainPermanent"),
            (Self::RETAIN_TEMPORARY.0.into(), "RETAIN_TEMPORARY", "RetainTemporary"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the SetCloseDownMode request
pub const SET_CLOSE_DOWN_MODE_REQUEST: u8 = 112;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetCloseDownModeRequest {
    pub mode: CloseDown,
}
impl_debug_if_no_extra_traits!(SetCloseDownModeRequest, "SetCloseDownModeRequest");
impl SetCloseDownModeRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mode_bytes = u8::from(self.mode).serialize();
        let mut request0 = vec![
            SET_CLOSE_DOWN_MODE_REQUEST,
            mode_bytes[0],
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
        if header.major_opcode != SET_CLOSE_DOWN_MODE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (mode, remaining) = u8::try_parse(remaining)?;
        let mode = mode.into();
        let _ = remaining;
        let _ = value;
        Ok(SetCloseDownModeRequest {
            mode,
        })
    }
}
impl Request for SetCloseDownModeRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SetCloseDownModeRequest {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Kill(u8);
impl Kill {
    pub const ALL_TEMPORARY: Self = Self(0);
}
impl From<Kill> for u8 {
    #[inline]
    fn from(input: Kill) -> Self {
        input.0
    }
}
impl From<Kill> for Option<u8> {
    #[inline]
    fn from(input: Kill) -> Self {
        Some(input.0)
    }
}
impl From<Kill> for u16 {
    #[inline]
    fn from(input: Kill) -> Self {
        u16::from(input.0)
    }
}
impl From<Kill> for Option<u16> {
    #[inline]
    fn from(input: Kill) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Kill> for u32 {
    #[inline]
    fn from(input: Kill) -> Self {
        u32::from(input.0)
    }
}
impl From<Kill> for Option<u32> {
    #[inline]
    fn from(input: Kill) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Kill {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Kill  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ALL_TEMPORARY.0.into(), "ALL_TEMPORARY", "AllTemporary"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the KillClient request
pub const KILL_CLIENT_REQUEST: u8 = 113;
/// kills a client.
///
/// Forces a close down of the client that created the specified `resource`.
///
/// # Fields
///
/// * `resource` - Any resource belonging to the client (for example a Window), used to identify
/// the client connection.
///
/// The special value of `XCB_KILL_ALL_TEMPORARY`, the resources of all clients
/// that have terminated in `RetainTemporary` (TODO) are destroyed.
///
/// # Errors
///
/// * `Value` - The specified `resource` does not exist.
///
/// # See
///
/// * `xkill`: program
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KillClientRequest {
    pub resource: u32,
}
impl_debug_if_no_extra_traits!(KillClientRequest, "KillClientRequest");
impl KillClientRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let resource_bytes = self.resource.serialize();
        let mut request0 = vec![
            KILL_CLIENT_REQUEST,
            0,
            0,
            0,
            resource_bytes[0],
            resource_bytes[1],
            resource_bytes[2],
            resource_bytes[3],
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
        if header.major_opcode != KILL_CLIENT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (resource, remaining) = u32::try_parse(value)?;
        let _ = remaining;
        Ok(KillClientRequest {
            resource,
        })
    }
}
impl Request for KillClientRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for KillClientRequest {
}

/// Opcode for the RotateProperties request
pub const ROTATE_PROPERTIES_REQUEST: u8 = 114;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RotatePropertiesRequest<'input> {
    pub window: Window,
    pub delta: i16,
    pub atoms: Cow<'input, [Atom]>,
}
impl_debug_if_no_extra_traits!(RotatePropertiesRequest<'_>, "RotatePropertiesRequest");
impl<'input> RotatePropertiesRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let atoms_len = u16::try_from(self.atoms.len()).expect("`atoms` has too many elements");
        let atoms_len_bytes = atoms_len.serialize();
        let delta_bytes = self.delta.serialize();
        let mut request0 = vec![
            ROTATE_PROPERTIES_REQUEST,
            0,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            atoms_len_bytes[0],
            atoms_len_bytes[1],
            delta_bytes[0],
            delta_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let atoms_bytes = self.atoms.serialize();
        let length_so_far = length_so_far + atoms_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), atoms_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != ROTATE_PROPERTIES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let (window, remaining) = Window::try_parse(value)?;
        let (atoms_len, remaining) = u16::try_parse(remaining)?;
        let (delta, remaining) = i16::try_parse(remaining)?;
        let (atoms, remaining) = crate::x11_utils::parse_list::<Atom>(remaining, atoms_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(RotatePropertiesRequest {
            window,
            delta,
            atoms: Cow::Owned(atoms),
        })
    }
    /// Clone all borrowed data in this RotatePropertiesRequest.
    pub fn into_owned(self) -> RotatePropertiesRequest<'static> {
        RotatePropertiesRequest {
            window: self.window,
            delta: self.delta,
            atoms: Cow::Owned(self.atoms.into_owned()),
        }
    }
}
impl<'input> Request for RotatePropertiesRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for RotatePropertiesRequest<'input> {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScreenSaver(u8);
impl ScreenSaver {
    pub const RESET: Self = Self(0);
    pub const ACTIVE: Self = Self(1);
}
impl From<ScreenSaver> for u8 {
    #[inline]
    fn from(input: ScreenSaver) -> Self {
        input.0
    }
}
impl From<ScreenSaver> for Option<u8> {
    #[inline]
    fn from(input: ScreenSaver) -> Self {
        Some(input.0)
    }
}
impl From<ScreenSaver> for u16 {
    #[inline]
    fn from(input: ScreenSaver) -> Self {
        u16::from(input.0)
    }
}
impl From<ScreenSaver> for Option<u16> {
    #[inline]
    fn from(input: ScreenSaver) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<ScreenSaver> for u32 {
    #[inline]
    fn from(input: ScreenSaver) -> Self {
        u32::from(input.0)
    }
}
impl From<ScreenSaver> for Option<u32> {
    #[inline]
    fn from(input: ScreenSaver) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ScreenSaver {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ScreenSaver  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::RESET.0.into(), "RESET", "Reset"),
            (Self::ACTIVE.0.into(), "ACTIVE", "Active"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the ForceScreenSaver request
pub const FORCE_SCREEN_SAVER_REQUEST: u8 = 115;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ForceScreenSaverRequest {
    pub mode: ScreenSaver,
}
impl_debug_if_no_extra_traits!(ForceScreenSaverRequest, "ForceScreenSaverRequest");
impl ForceScreenSaverRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mode_bytes = u8::from(self.mode).serialize();
        let mut request0 = vec![
            FORCE_SCREEN_SAVER_REQUEST,
            mode_bytes[0],
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
        if header.major_opcode != FORCE_SCREEN_SAVER_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (mode, remaining) = u8::try_parse(remaining)?;
        let mode = mode.into();
        let _ = remaining;
        let _ = value;
        Ok(ForceScreenSaverRequest {
            mode,
        })
    }
}
impl Request for ForceScreenSaverRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for ForceScreenSaverRequest {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MappingStatus(u8);
impl MappingStatus {
    pub const SUCCESS: Self = Self(0);
    pub const BUSY: Self = Self(1);
    pub const FAILURE: Self = Self(2);
}
impl From<MappingStatus> for u8 {
    #[inline]
    fn from(input: MappingStatus) -> Self {
        input.0
    }
}
impl From<MappingStatus> for Option<u8> {
    #[inline]
    fn from(input: MappingStatus) -> Self {
        Some(input.0)
    }
}
impl From<MappingStatus> for u16 {
    #[inline]
    fn from(input: MappingStatus) -> Self {
        u16::from(input.0)
    }
}
impl From<MappingStatus> for Option<u16> {
    #[inline]
    fn from(input: MappingStatus) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<MappingStatus> for u32 {
    #[inline]
    fn from(input: MappingStatus) -> Self {
        u32::from(input.0)
    }
}
impl From<MappingStatus> for Option<u32> {
    #[inline]
    fn from(input: MappingStatus) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for MappingStatus {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for MappingStatus  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SUCCESS.0.into(), "SUCCESS", "Success"),
            (Self::BUSY.0.into(), "BUSY", "Busy"),
            (Self::FAILURE.0.into(), "FAILURE", "Failure"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the SetPointerMapping request
pub const SET_POINTER_MAPPING_REQUEST: u8 = 116;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetPointerMappingRequest<'input> {
    pub map: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(SetPointerMappingRequest<'_>, "SetPointerMappingRequest");
impl<'input> SetPointerMappingRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let map_len = u8::try_from(self.map.len()).expect("`map` has too many elements");
        let map_len_bytes = map_len.serialize();
        let mut request0 = vec![
            SET_POINTER_MAPPING_REQUEST,
            map_len_bytes[0],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.map.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.map, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != SET_POINTER_MAPPING_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (map_len, remaining) = u8::try_parse(remaining)?;
        let _ = remaining;
        let (map, remaining) = crate::x11_utils::parse_u8_list(value, map_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(SetPointerMappingRequest {
            map: Cow::Borrowed(map),
        })
    }
    /// Clone all borrowed data in this SetPointerMappingRequest.
    pub fn into_owned(self) -> SetPointerMappingRequest<'static> {
        SetPointerMappingRequest {
            map: Cow::Owned(self.map.into_owned()),
        }
    }
}
impl<'input> Request for SetPointerMappingRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for SetPointerMappingRequest<'input> {
    type Reply = SetPointerMappingReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetPointerMappingReply {
    pub status: MappingStatus,
    pub sequence: u16,
    pub length: u32,
}
impl_debug_if_no_extra_traits!(SetPointerMappingReply, "SetPointerMappingReply");
impl TryParse for SetPointerMappingReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (status, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let status = status.into();
        let result = SetPointerMappingReply { status, sequence, length };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for SetPointerMappingReply {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let response_type_bytes = &[1];
        let status_bytes = u8::from(self.status).serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        [
            response_type_bytes[0],
            status_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        u8::from(self.status).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
    }
}

/// Opcode for the GetPointerMapping request
pub const GET_POINTER_MAPPING_REQUEST: u8 = 117;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPointerMappingRequest;
impl_debug_if_no_extra_traits!(GetPointerMappingRequest, "GetPointerMappingRequest");
impl GetPointerMappingRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            GET_POINTER_MAPPING_REQUEST,
            0,
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
        if header.major_opcode != GET_POINTER_MAPPING_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let _ = value;
        Ok(GetPointerMappingRequest
        )
    }
}
impl Request for GetPointerMappingRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetPointerMappingRequest {
    type Reply = GetPointerMappingReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPointerMappingReply {
    pub sequence: u16,
    pub length: u32,
    pub map: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetPointerMappingReply, "GetPointerMappingReply");
impl TryParse for GetPointerMappingReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (map_len, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(24..).ok_or(ParseError::InsufficientData)?;
        let (map, remaining) = crate::x11_utils::parse_u8_list(remaining, map_len.try_to_usize()?)?;
        let map = map.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetPointerMappingReply { sequence, length, map };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetPointerMappingReply {
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
        let map_len = u8::try_from(self.map.len()).expect("`map` has too many elements");
        map_len.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 24]);
        bytes.extend_from_slice(&self.map);
    }
}
impl GetPointerMappingReply {
    /// Get the value of the `map_len` field.
    ///
    /// The `map_len` field is used as the length field of the `map` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn map_len(&self) -> u8 {
        self.map.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapIndex(u8);
impl MapIndex {
    pub const SHIFT: Self = Self(0);
    pub const LOCK: Self = Self(1);
    pub const CONTROL: Self = Self(2);
    pub const M1: Self = Self(3);
    pub const M2: Self = Self(4);
    pub const M3: Self = Self(5);
    pub const M4: Self = Self(6);
    pub const M5: Self = Self(7);
}
impl From<MapIndex> for u8 {
    #[inline]
    fn from(input: MapIndex) -> Self {
        input.0
    }
}
impl From<MapIndex> for Option<u8> {
    #[inline]
    fn from(input: MapIndex) -> Self {
        Some(input.0)
    }
}
impl From<MapIndex> for u16 {
    #[inline]
    fn from(input: MapIndex) -> Self {
        u16::from(input.0)
    }
}
impl From<MapIndex> for Option<u16> {
    #[inline]
    fn from(input: MapIndex) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<MapIndex> for u32 {
    #[inline]
    fn from(input: MapIndex) -> Self {
        u32::from(input.0)
    }
}
impl From<MapIndex> for Option<u32> {
    #[inline]
    fn from(input: MapIndex) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for MapIndex {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for MapIndex  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SHIFT.0.into(), "SHIFT", "Shift"),
            (Self::LOCK.0.into(), "LOCK", "Lock"),
            (Self::CONTROL.0.into(), "CONTROL", "Control"),
            (Self::M1.0.into(), "M1", "M1"),
            (Self::M2.0.into(), "M2", "M2"),
            (Self::M3.0.into(), "M3", "M3"),
            (Self::M4.0.into(), "M4", "M4"),
            (Self::M5.0.into(), "M5", "M5"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the SetModifierMapping request
pub const SET_MODIFIER_MAPPING_REQUEST: u8 = 118;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetModifierMappingRequest<'input> {
    pub keycodes: Cow<'input, [Keycode]>,
}
impl_debug_if_no_extra_traits!(SetModifierMappingRequest<'_>, "SetModifierMappingRequest");
impl<'input> SetModifierMappingRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        assert_eq!(self.keycodes.len() % 8, 0, "`keycodes` has an incorrect length, must be a multiple of 8");
        let keycodes_per_modifier = u8::try_from(self.keycodes.len() / 8).expect("`keycodes` has too many elements");
        let keycodes_per_modifier_bytes = keycodes_per_modifier.serialize();
        let mut request0 = vec![
            SET_MODIFIER_MAPPING_REQUEST,
            keycodes_per_modifier_bytes[0],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.keycodes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.keycodes, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.major_opcode != SET_MODIFIER_MAPPING_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let (keycodes_per_modifier, remaining) = u8::try_parse(remaining)?;
        let _ = remaining;
        let (keycodes, remaining) = crate::x11_utils::parse_u8_list(value, u32::from(keycodes_per_modifier).checked_mul(8u32).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let _ = remaining;
        Ok(SetModifierMappingRequest {
            keycodes: Cow::Borrowed(keycodes),
        })
    }
    /// Clone all borrowed data in this SetModifierMappingRequest.
    pub fn into_owned(self) -> SetModifierMappingRequest<'static> {
        SetModifierMappingRequest {
            keycodes: Cow::Owned(self.keycodes.into_owned()),
        }
    }
}
impl<'input> Request for SetModifierMappingRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for SetModifierMappingRequest<'input> {
    type Reply = SetModifierMappingReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetModifierMappingReply {
    pub status: MappingStatus,
    pub sequence: u16,
    pub length: u32,
}
impl_debug_if_no_extra_traits!(SetModifierMappingReply, "SetModifierMappingReply");
impl TryParse for SetModifierMappingReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (status, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let status = status.into();
        let result = SetModifierMappingReply { status, sequence, length };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for SetModifierMappingReply {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let response_type_bytes = &[1];
        let status_bytes = u8::from(self.status).serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        [
            response_type_bytes[0],
            status_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        u8::from(self.status).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
    }
}

/// Opcode for the GetModifierMapping request
pub const GET_MODIFIER_MAPPING_REQUEST: u8 = 119;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetModifierMappingRequest;
impl_debug_if_no_extra_traits!(GetModifierMappingRequest, "GetModifierMappingRequest");
impl GetModifierMappingRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            GET_MODIFIER_MAPPING_REQUEST,
            0,
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
        if header.major_opcode != GET_MODIFIER_MAPPING_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let _ = value;
        Ok(GetModifierMappingRequest
        )
    }
}
impl Request for GetModifierMappingRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetModifierMappingRequest {
    type Reply = GetModifierMappingReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetModifierMappingReply {
    pub sequence: u16,
    pub length: u32,
    pub keycodes: Vec<Keycode>,
}
impl_debug_if_no_extra_traits!(GetModifierMappingReply, "GetModifierMappingReply");
impl TryParse for GetModifierMappingReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (keycodes_per_modifier, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(24..).ok_or(ParseError::InsufficientData)?;
        let (keycodes, remaining) = crate::x11_utils::parse_u8_list(remaining, u32::from(keycodes_per_modifier).checked_mul(8u32).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let keycodes = keycodes.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetModifierMappingReply { sequence, length, keycodes };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetModifierMappingReply {
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
        assert_eq!(self.keycodes.len() % 8, 0, "`keycodes` has an incorrect length, must be a multiple of 8");
        let keycodes_per_modifier = u8::try_from(self.keycodes.len() / 8).expect("`keycodes` has too many elements");
        keycodes_per_modifier.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 24]);
        bytes.extend_from_slice(&self.keycodes);
    }
}
impl GetModifierMappingReply {
    /// Get the value of the `keycodes_per_modifier` field.
    ///
    /// The `keycodes_per_modifier` field is used as the length field of the `keycodes` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn keycodes_per_modifier(&self) -> u8 {
        self.keycodes.len()
            .checked_div(8).unwrap()
            .try_into().unwrap()
    }
}

/// Opcode for the NoOperation request
pub const NO_OPERATION_REQUEST: u8 = 127;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NoOperationRequest;
impl_debug_if_no_extra_traits!(NoOperationRequest, "NoOperationRequest");
impl NoOperationRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            NO_OPERATION_REQUEST,
            0,
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
        if header.major_opcode != NO_OPERATION_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let remaining = &[header.minor_opcode];
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        let _ = value;
        Ok(NoOperationRequest
        )
    }
}
impl Request for NoOperationRequest {
    const EXTENSION_NAME: Option<&'static str> = None;

    fn serialize(self, _major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize();
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for NoOperationRequest {
}

