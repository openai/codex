// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Xv` X11 extension.

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
use super::shm;
#[allow(unused_imports)]
use super::xproto;

/// The X11 name of the extension for QueryExtension
pub const X11_EXTENSION_NAME: &str = "XVideo";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (2, 2);

pub type Port = u32;

pub type Encoding = u32;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Type(u8);
impl Type {
    pub const INPUT_MASK: Self = Self(1 << 0);
    pub const OUTPUT_MASK: Self = Self(1 << 1);
    pub const VIDEO_MASK: Self = Self(1 << 2);
    pub const STILL_MASK: Self = Self(1 << 3);
    pub const IMAGE_MASK: Self = Self(1 << 4);
}
impl From<Type> for u8 {
    #[inline]
    fn from(input: Type) -> Self {
        input.0
    }
}
impl From<Type> for Option<u8> {
    #[inline]
    fn from(input: Type) -> Self {
        Some(input.0)
    }
}
impl From<Type> for u16 {
    #[inline]
    fn from(input: Type) -> Self {
        u16::from(input.0)
    }
}
impl From<Type> for Option<u16> {
    #[inline]
    fn from(input: Type) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Type> for u32 {
    #[inline]
    fn from(input: Type) -> Self {
        u32::from(input.0)
    }
}
impl From<Type> for Option<u32> {
    #[inline]
    fn from(input: Type) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Type {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Type  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::INPUT_MASK.0.into(), "INPUT_MASK", "InputMask"),
            (Self::OUTPUT_MASK.0.into(), "OUTPUT_MASK", "OutputMask"),
            (Self::VIDEO_MASK.0.into(), "VIDEO_MASK", "VideoMask"),
            (Self::STILL_MASK.0.into(), "STILL_MASK", "StillMask"),
            (Self::IMAGE_MASK.0.into(), "IMAGE_MASK", "ImageMask"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(Type, u8);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageFormatInfoType(u8);
impl ImageFormatInfoType {
    pub const RGB: Self = Self(0);
    pub const YUV: Self = Self(1);
}
impl From<ImageFormatInfoType> for u8 {
    #[inline]
    fn from(input: ImageFormatInfoType) -> Self {
        input.0
    }
}
impl From<ImageFormatInfoType> for Option<u8> {
    #[inline]
    fn from(input: ImageFormatInfoType) -> Self {
        Some(input.0)
    }
}
impl From<ImageFormatInfoType> for u16 {
    #[inline]
    fn from(input: ImageFormatInfoType) -> Self {
        u16::from(input.0)
    }
}
impl From<ImageFormatInfoType> for Option<u16> {
    #[inline]
    fn from(input: ImageFormatInfoType) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<ImageFormatInfoType> for u32 {
    #[inline]
    fn from(input: ImageFormatInfoType) -> Self {
        u32::from(input.0)
    }
}
impl From<ImageFormatInfoType> for Option<u32> {
    #[inline]
    fn from(input: ImageFormatInfoType) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ImageFormatInfoType {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ImageFormatInfoType  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::RGB.0.into(), "RGB", "RGB"),
            (Self::YUV.0.into(), "YUV", "YUV"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageFormatInfoFormat(u8);
impl ImageFormatInfoFormat {
    pub const PACKED: Self = Self(0);
    pub const PLANAR: Self = Self(1);
}
impl From<ImageFormatInfoFormat> for u8 {
    #[inline]
    fn from(input: ImageFormatInfoFormat) -> Self {
        input.0
    }
}
impl From<ImageFormatInfoFormat> for Option<u8> {
    #[inline]
    fn from(input: ImageFormatInfoFormat) -> Self {
        Some(input.0)
    }
}
impl From<ImageFormatInfoFormat> for u16 {
    #[inline]
    fn from(input: ImageFormatInfoFormat) -> Self {
        u16::from(input.0)
    }
}
impl From<ImageFormatInfoFormat> for Option<u16> {
    #[inline]
    fn from(input: ImageFormatInfoFormat) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<ImageFormatInfoFormat> for u32 {
    #[inline]
    fn from(input: ImageFormatInfoFormat) -> Self {
        u32::from(input.0)
    }
}
impl From<ImageFormatInfoFormat> for Option<u32> {
    #[inline]
    fn from(input: ImageFormatInfoFormat) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ImageFormatInfoFormat {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ImageFormatInfoFormat  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::PACKED.0.into(), "PACKED", "Packed"),
            (Self::PLANAR.0.into(), "PLANAR", "Planar"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AttributeFlag(u32);
impl AttributeFlag {
    pub const GETTABLE: Self = Self(1 << 0);
    pub const SETTABLE: Self = Self(1 << 1);
}
impl From<AttributeFlag> for u32 {
    #[inline]
    fn from(input: AttributeFlag) -> Self {
        input.0
    }
}
impl From<AttributeFlag> for Option<u32> {
    #[inline]
    fn from(input: AttributeFlag) -> Self {
        Some(input.0)
    }
}
impl From<u8> for AttributeFlag {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for AttributeFlag {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for AttributeFlag {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for AttributeFlag  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::GETTABLE.0, "GETTABLE", "Gettable"),
            (Self::SETTABLE.0, "SETTABLE", "Settable"),
        ];
        pretty_print_bitmask(fmt, self.0, &variants)
    }
}
bitmask_binop!(AttributeFlag, u32);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VideoNotifyReason(u8);
impl VideoNotifyReason {
    pub const STARTED: Self = Self(0);
    pub const STOPPED: Self = Self(1);
    pub const BUSY: Self = Self(2);
    pub const PREEMPTED: Self = Self(3);
    pub const HARD_ERROR: Self = Self(4);
}
impl From<VideoNotifyReason> for u8 {
    #[inline]
    fn from(input: VideoNotifyReason) -> Self {
        input.0
    }
}
impl From<VideoNotifyReason> for Option<u8> {
    #[inline]
    fn from(input: VideoNotifyReason) -> Self {
        Some(input.0)
    }
}
impl From<VideoNotifyReason> for u16 {
    #[inline]
    fn from(input: VideoNotifyReason) -> Self {
        u16::from(input.0)
    }
}
impl From<VideoNotifyReason> for Option<u16> {
    #[inline]
    fn from(input: VideoNotifyReason) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<VideoNotifyReason> for u32 {
    #[inline]
    fn from(input: VideoNotifyReason) -> Self {
        u32::from(input.0)
    }
}
impl From<VideoNotifyReason> for Option<u32> {
    #[inline]
    fn from(input: VideoNotifyReason) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for VideoNotifyReason {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for VideoNotifyReason  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::STARTED.0.into(), "STARTED", "Started"),
            (Self::STOPPED.0.into(), "STOPPED", "Stopped"),
            (Self::BUSY.0.into(), "BUSY", "Busy"),
            (Self::PREEMPTED.0.into(), "PREEMPTED", "Preempted"),
            (Self::HARD_ERROR.0.into(), "HARD_ERROR", "HardError"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScanlineOrder(u8);
impl ScanlineOrder {
    pub const TOP_TO_BOTTOM: Self = Self(0);
    pub const BOTTOM_TO_TOP: Self = Self(1);
}
impl From<ScanlineOrder> for u8 {
    #[inline]
    fn from(input: ScanlineOrder) -> Self {
        input.0
    }
}
impl From<ScanlineOrder> for Option<u8> {
    #[inline]
    fn from(input: ScanlineOrder) -> Self {
        Some(input.0)
    }
}
impl From<ScanlineOrder> for u16 {
    #[inline]
    fn from(input: ScanlineOrder) -> Self {
        u16::from(input.0)
    }
}
impl From<ScanlineOrder> for Option<u16> {
    #[inline]
    fn from(input: ScanlineOrder) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<ScanlineOrder> for u32 {
    #[inline]
    fn from(input: ScanlineOrder) -> Self {
        u32::from(input.0)
    }
}
impl From<ScanlineOrder> for Option<u32> {
    #[inline]
    fn from(input: ScanlineOrder) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ScanlineOrder {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ScanlineOrder  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::TOP_TO_BOTTOM.0.into(), "TOP_TO_BOTTOM", "TopToBottom"),
            (Self::BOTTOM_TO_TOP.0.into(), "BOTTOM_TO_TOP", "BottomToTop"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrabPortStatus(u8);
impl GrabPortStatus {
    pub const SUCCESS: Self = Self(0);
    pub const BAD_EXTENSION: Self = Self(1);
    pub const ALREADY_GRABBED: Self = Self(2);
    pub const INVALID_TIME: Self = Self(3);
    pub const BAD_REPLY: Self = Self(4);
    pub const BAD_ALLOC: Self = Self(5);
}
impl From<GrabPortStatus> for u8 {
    #[inline]
    fn from(input: GrabPortStatus) -> Self {
        input.0
    }
}
impl From<GrabPortStatus> for Option<u8> {
    #[inline]
    fn from(input: GrabPortStatus) -> Self {
        Some(input.0)
    }
}
impl From<GrabPortStatus> for u16 {
    #[inline]
    fn from(input: GrabPortStatus) -> Self {
        u16::from(input.0)
    }
}
impl From<GrabPortStatus> for Option<u16> {
    #[inline]
    fn from(input: GrabPortStatus) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<GrabPortStatus> for u32 {
    #[inline]
    fn from(input: GrabPortStatus) -> Self {
        u32::from(input.0)
    }
}
impl From<GrabPortStatus> for Option<u32> {
    #[inline]
    fn from(input: GrabPortStatus) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for GrabPortStatus {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for GrabPortStatus  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SUCCESS.0.into(), "SUCCESS", "Success"),
            (Self::BAD_EXTENSION.0.into(), "BAD_EXTENSION", "BadExtension"),
            (Self::ALREADY_GRABBED.0.into(), "ALREADY_GRABBED", "AlreadyGrabbed"),
            (Self::INVALID_TIME.0.into(), "INVALID_TIME", "InvalidTime"),
            (Self::BAD_REPLY.0.into(), "BAD_REPLY", "BadReply"),
            (Self::BAD_ALLOC.0.into(), "BAD_ALLOC", "BadAlloc"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rational {
    pub numerator: i32,
    pub denominator: i32,
}
impl_debug_if_no_extra_traits!(Rational, "Rational");
impl TryParse for Rational {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (numerator, remaining) = i32::try_parse(remaining)?;
        let (denominator, remaining) = i32::try_parse(remaining)?;
        let result = Rational { numerator, denominator };
        Ok((result, remaining))
    }
}
impl Serialize for Rational {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let numerator_bytes = self.numerator.serialize();
        let denominator_bytes = self.denominator.serialize();
        [
            numerator_bytes[0],
            numerator_bytes[1],
            numerator_bytes[2],
            numerator_bytes[3],
            denominator_bytes[0],
            denominator_bytes[1],
            denominator_bytes[2],
            denominator_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.numerator.serialize_into(bytes);
        self.denominator.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Format {
    pub visual: xproto::Visualid,
    pub depth: u8,
}
impl_debug_if_no_extra_traits!(Format, "Format");
impl TryParse for Format {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (visual, remaining) = xproto::Visualid::try_parse(remaining)?;
        let (depth, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let result = Format { visual, depth };
        Ok((result, remaining))
    }
}
impl Serialize for Format {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let visual_bytes = self.visual.serialize();
        let depth_bytes = self.depth.serialize();
        [
            visual_bytes[0],
            visual_bytes[1],
            visual_bytes[2],
            visual_bytes[3],
            depth_bytes[0],
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.visual.serialize_into(bytes);
        self.depth.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AdaptorInfo {
    pub base_id: Port,
    pub num_ports: u16,
    pub type_: Type,
    pub name: Vec<u8>,
    pub formats: Vec<Format>,
}
impl_debug_if_no_extra_traits!(AdaptorInfo, "AdaptorInfo");
impl TryParse for AdaptorInfo {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let value = remaining;
        let (base_id, remaining) = Port::try_parse(remaining)?;
        let (name_size, remaining) = u16::try_parse(remaining)?;
        let (num_ports, remaining) = u16::try_parse(remaining)?;
        let (num_formats, remaining) = u16::try_parse(remaining)?;
        let (type_, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, name_size.try_to_usize()?)?;
        let name = name.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let (formats, remaining) = crate::x11_utils::parse_list::<Format>(remaining, num_formats.try_to_usize()?)?;
        let type_ = type_.into();
        let result = AdaptorInfo { base_id, num_ports, type_, name, formats };
        Ok((result, remaining))
    }
}
impl Serialize for AdaptorInfo {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.base_id.serialize_into(bytes);
        let name_size = u16::try_from(self.name.len()).expect("`name` has too many elements");
        name_size.serialize_into(bytes);
        self.num_ports.serialize_into(bytes);
        let num_formats = u16::try_from(self.formats.len()).expect("`formats` has too many elements");
        num_formats.serialize_into(bytes);
        u8::from(self.type_).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        bytes.extend_from_slice(&self.name);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        self.formats.serialize_into(bytes);
    }
}
impl AdaptorInfo {
    /// Get the value of the `name_size` field.
    ///
    /// The `name_size` field is used as the length field of the `name` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn name_size(&self) -> u16 {
        self.name.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_formats` field.
    ///
    /// The `num_formats` field is used as the length field of the `formats` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_formats(&self) -> u16 {
        self.formats.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EncodingInfo {
    pub encoding: Encoding,
    pub width: u16,
    pub height: u16,
    pub rate: Rational,
    pub name: Vec<u8>,
}
impl_debug_if_no_extra_traits!(EncodingInfo, "EncodingInfo");
impl TryParse for EncodingInfo {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let value = remaining;
        let (encoding, remaining) = Encoding::try_parse(remaining)?;
        let (name_size, remaining) = u16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (rate, remaining) = Rational::try_parse(remaining)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, name_size.try_to_usize()?)?;
        let name = name.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let result = EncodingInfo { encoding, width, height, rate, name };
        Ok((result, remaining))
    }
}
impl Serialize for EncodingInfo {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(20);
        self.encoding.serialize_into(bytes);
        let name_size = u16::try_from(self.name.len()).expect("`name` has too many elements");
        name_size.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.rate.serialize_into(bytes);
        bytes.extend_from_slice(&self.name);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
    }
}
impl EncodingInfo {
    /// Get the value of the `name_size` field.
    ///
    /// The `name_size` field is used as the length field of the `name` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn name_size(&self) -> u16 {
        self.name.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Image {
    pub id: u32,
    pub width: u16,
    pub height: u16,
    pub pitches: Vec<u32>,
    pub offsets: Vec<u32>,
    pub data: Vec<u8>,
}
impl_debug_if_no_extra_traits!(Image, "Image");
impl TryParse for Image {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (id, remaining) = u32::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (data_size, remaining) = u32::try_parse(remaining)?;
        let (num_planes, remaining) = u32::try_parse(remaining)?;
        let (pitches, remaining) = crate::x11_utils::parse_list::<u32>(remaining, num_planes.try_to_usize()?)?;
        let (offsets, remaining) = crate::x11_utils::parse_list::<u32>(remaining, num_planes.try_to_usize()?)?;
        let (data, remaining) = crate::x11_utils::parse_u8_list(remaining, data_size.try_to_usize()?)?;
        let data = data.to_vec();
        let result = Image { id, width, height, pitches, offsets, data };
        Ok((result, remaining))
    }
}
impl Serialize for Image {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        self.id.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        let data_size = u32::try_from(self.data.len()).expect("`data` has too many elements");
        data_size.serialize_into(bytes);
        let num_planes = u32::try_from(self.pitches.len()).expect("`pitches` has too many elements");
        num_planes.serialize_into(bytes);
        self.pitches.serialize_into(bytes);
        assert_eq!(self.offsets.len(), usize::try_from(num_planes).unwrap(), "`offsets` has an incorrect length");
        self.offsets.serialize_into(bytes);
        bytes.extend_from_slice(&self.data);
    }
}
impl Image {
    /// Get the value of the `data_size` field.
    ///
    /// The `data_size` field is used as the length field of the `data` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn data_size(&self) -> u32 {
        self.data.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_planes` field.
    ///
    /// The `num_planes` field is used as the length field of the `pitches` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_planes(&self) -> u32 {
        self.pitches.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AttributeInfo {
    pub flags: AttributeFlag,
    pub min: i32,
    pub max: i32,
    pub name: Vec<u8>,
}
impl_debug_if_no_extra_traits!(AttributeInfo, "AttributeInfo");
impl TryParse for AttributeInfo {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let value = remaining;
        let (flags, remaining) = u32::try_parse(remaining)?;
        let (min, remaining) = i32::try_parse(remaining)?;
        let (max, remaining) = i32::try_parse(remaining)?;
        let (size, remaining) = u32::try_parse(remaining)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, size.try_to_usize()?)?;
        let name = name.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let flags = flags.into();
        let result = AttributeInfo { flags, min, max, name };
        Ok((result, remaining))
    }
}
impl Serialize for AttributeInfo {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        u32::from(self.flags).serialize_into(bytes);
        self.min.serialize_into(bytes);
        self.max.serialize_into(bytes);
        let size = u32::try_from(self.name.len()).expect("`name` has too many elements");
        size.serialize_into(bytes);
        bytes.extend_from_slice(&self.name);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
    }
}
impl AttributeInfo {
    /// Get the value of the `size` field.
    ///
    /// The `size` field is used as the length field of the `name` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn size(&self) -> u32 {
        self.name.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageFormatInfo {
    pub id: u32,
    pub type_: ImageFormatInfoType,
    pub byte_order: xproto::ImageOrder,
    pub guid: [u8; 16],
    pub bpp: u8,
    pub num_planes: u8,
    pub depth: u8,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    pub format: ImageFormatInfoFormat,
    pub y_sample_bits: u32,
    pub u_sample_bits: u32,
    pub v_sample_bits: u32,
    pub vhorz_y_period: u32,
    pub vhorz_u_period: u32,
    pub vhorz_v_period: u32,
    pub vvert_y_period: u32,
    pub vvert_u_period: u32,
    pub vvert_v_period: u32,
    pub vcomp_order: [u8; 32],
    pub vscanline_order: ScanlineOrder,
}
impl_debug_if_no_extra_traits!(ImageFormatInfo, "ImageFormatInfo");
impl TryParse for ImageFormatInfo {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (id, remaining) = u32::try_parse(remaining)?;
        let (type_, remaining) = u8::try_parse(remaining)?;
        let (byte_order, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (guid, remaining) = crate::x11_utils::parse_u8_array::<16>(remaining)?;
        let (bpp, remaining) = u8::try_parse(remaining)?;
        let (num_planes, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (depth, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (red_mask, remaining) = u32::try_parse(remaining)?;
        let (green_mask, remaining) = u32::try_parse(remaining)?;
        let (blue_mask, remaining) = u32::try_parse(remaining)?;
        let (format, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (y_sample_bits, remaining) = u32::try_parse(remaining)?;
        let (u_sample_bits, remaining) = u32::try_parse(remaining)?;
        let (v_sample_bits, remaining) = u32::try_parse(remaining)?;
        let (vhorz_y_period, remaining) = u32::try_parse(remaining)?;
        let (vhorz_u_period, remaining) = u32::try_parse(remaining)?;
        let (vhorz_v_period, remaining) = u32::try_parse(remaining)?;
        let (vvert_y_period, remaining) = u32::try_parse(remaining)?;
        let (vvert_u_period, remaining) = u32::try_parse(remaining)?;
        let (vvert_v_period, remaining) = u32::try_parse(remaining)?;
        let (vcomp_order, remaining) = crate::x11_utils::parse_u8_array::<32>(remaining)?;
        let (vscanline_order, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(11..).ok_or(ParseError::InsufficientData)?;
        let type_ = type_.into();
        let byte_order = byte_order.into();
        let format = format.into();
        let vscanline_order = vscanline_order.into();
        let result = ImageFormatInfo { id, type_, byte_order, guid, bpp, num_planes, depth, red_mask, green_mask, blue_mask, format, y_sample_bits, u_sample_bits, v_sample_bits, vhorz_y_period, vhorz_u_period, vhorz_v_period, vvert_y_period, vvert_u_period, vvert_v_period, vcomp_order, vscanline_order };
        Ok((result, remaining))
    }
}
impl Serialize for ImageFormatInfo {
    type Bytes = [u8; 128];
    fn serialize(&self) -> [u8; 128] {
        let id_bytes = self.id.serialize();
        let type_bytes = u8::from(self.type_).serialize();
        let byte_order_bytes = u8::from(self.byte_order).serialize();
        let bpp_bytes = self.bpp.serialize();
        let num_planes_bytes = self.num_planes.serialize();
        let depth_bytes = self.depth.serialize();
        let red_mask_bytes = self.red_mask.serialize();
        let green_mask_bytes = self.green_mask.serialize();
        let blue_mask_bytes = self.blue_mask.serialize();
        let format_bytes = u8::from(self.format).serialize();
        let y_sample_bits_bytes = self.y_sample_bits.serialize();
        let u_sample_bits_bytes = self.u_sample_bits.serialize();
        let v_sample_bits_bytes = self.v_sample_bits.serialize();
        let vhorz_y_period_bytes = self.vhorz_y_period.serialize();
        let vhorz_u_period_bytes = self.vhorz_u_period.serialize();
        let vhorz_v_period_bytes = self.vhorz_v_period.serialize();
        let vvert_y_period_bytes = self.vvert_y_period.serialize();
        let vvert_u_period_bytes = self.vvert_u_period.serialize();
        let vvert_v_period_bytes = self.vvert_v_period.serialize();
        let vscanline_order_bytes = u8::from(self.vscanline_order).serialize();
        [
            id_bytes[0],
            id_bytes[1],
            id_bytes[2],
            id_bytes[3],
            type_bytes[0],
            byte_order_bytes[0],
            0,
            0,
            self.guid[0],
            self.guid[1],
            self.guid[2],
            self.guid[3],
            self.guid[4],
            self.guid[5],
            self.guid[6],
            self.guid[7],
            self.guid[8],
            self.guid[9],
            self.guid[10],
            self.guid[11],
            self.guid[12],
            self.guid[13],
            self.guid[14],
            self.guid[15],
            bpp_bytes[0],
            num_planes_bytes[0],
            0,
            0,
            depth_bytes[0],
            0,
            0,
            0,
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
            format_bytes[0],
            0,
            0,
            0,
            y_sample_bits_bytes[0],
            y_sample_bits_bytes[1],
            y_sample_bits_bytes[2],
            y_sample_bits_bytes[3],
            u_sample_bits_bytes[0],
            u_sample_bits_bytes[1],
            u_sample_bits_bytes[2],
            u_sample_bits_bytes[3],
            v_sample_bits_bytes[0],
            v_sample_bits_bytes[1],
            v_sample_bits_bytes[2],
            v_sample_bits_bytes[3],
            vhorz_y_period_bytes[0],
            vhorz_y_period_bytes[1],
            vhorz_y_period_bytes[2],
            vhorz_y_period_bytes[3],
            vhorz_u_period_bytes[0],
            vhorz_u_period_bytes[1],
            vhorz_u_period_bytes[2],
            vhorz_u_period_bytes[3],
            vhorz_v_period_bytes[0],
            vhorz_v_period_bytes[1],
            vhorz_v_period_bytes[2],
            vhorz_v_period_bytes[3],
            vvert_y_period_bytes[0],
            vvert_y_period_bytes[1],
            vvert_y_period_bytes[2],
            vvert_y_period_bytes[3],
            vvert_u_period_bytes[0],
            vvert_u_period_bytes[1],
            vvert_u_period_bytes[2],
            vvert_u_period_bytes[3],
            vvert_v_period_bytes[0],
            vvert_v_period_bytes[1],
            vvert_v_period_bytes[2],
            vvert_v_period_bytes[3],
            self.vcomp_order[0],
            self.vcomp_order[1],
            self.vcomp_order[2],
            self.vcomp_order[3],
            self.vcomp_order[4],
            self.vcomp_order[5],
            self.vcomp_order[6],
            self.vcomp_order[7],
            self.vcomp_order[8],
            self.vcomp_order[9],
            self.vcomp_order[10],
            self.vcomp_order[11],
            self.vcomp_order[12],
            self.vcomp_order[13],
            self.vcomp_order[14],
            self.vcomp_order[15],
            self.vcomp_order[16],
            self.vcomp_order[17],
            self.vcomp_order[18],
            self.vcomp_order[19],
            self.vcomp_order[20],
            self.vcomp_order[21],
            self.vcomp_order[22],
            self.vcomp_order[23],
            self.vcomp_order[24],
            self.vcomp_order[25],
            self.vcomp_order[26],
            self.vcomp_order[27],
            self.vcomp_order[28],
            self.vcomp_order[29],
            self.vcomp_order[30],
            self.vcomp_order[31],
            vscanline_order_bytes[0],
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
        bytes.reserve(128);
        self.id.serialize_into(bytes);
        u8::from(self.type_).serialize_into(bytes);
        u8::from(self.byte_order).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        bytes.extend_from_slice(&self.guid);
        self.bpp.serialize_into(bytes);
        self.num_planes.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.depth.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
        self.red_mask.serialize_into(bytes);
        self.green_mask.serialize_into(bytes);
        self.blue_mask.serialize_into(bytes);
        u8::from(self.format).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
        self.y_sample_bits.serialize_into(bytes);
        self.u_sample_bits.serialize_into(bytes);
        self.v_sample_bits.serialize_into(bytes);
        self.vhorz_y_period.serialize_into(bytes);
        self.vhorz_u_period.serialize_into(bytes);
        self.vhorz_v_period.serialize_into(bytes);
        self.vvert_y_period.serialize_into(bytes);
        self.vvert_u_period.serialize_into(bytes);
        self.vvert_v_period.serialize_into(bytes);
        bytes.extend_from_slice(&self.vcomp_order);
        u8::from(self.vscanline_order).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 11]);
    }
}

/// Opcode for the BadPort error
pub const BAD_PORT_ERROR: u8 = 0;

/// Opcode for the BadEncoding error
pub const BAD_ENCODING_ERROR: u8 = 1;

/// Opcode for the BadControl error
pub const BAD_CONTROL_ERROR: u8 = 2;

/// Opcode for the VideoNotify event
pub const VIDEO_NOTIFY_EVENT: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VideoNotifyEvent {
    pub response_type: u8,
    pub reason: VideoNotifyReason,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub drawable: xproto::Drawable,
    pub port: Port,
}
impl_debug_if_no_extra_traits!(VideoNotifyEvent, "VideoNotifyEvent");
impl TryParse for VideoNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (reason, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (drawable, remaining) = xproto::Drawable::try_parse(remaining)?;
        let (port, remaining) = Port::try_parse(remaining)?;
        let reason = reason.into();
        let result = VideoNotifyEvent { response_type, reason, sequence, time, drawable, port };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for VideoNotifyEvent {
    type Bytes = [u8; 16];
    fn serialize(&self) -> [u8; 16] {
        let response_type_bytes = self.response_type.serialize();
        let reason_bytes = u8::from(self.reason).serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let drawable_bytes = self.drawable.serialize();
        let port_bytes = self.port.serialize();
        [
            response_type_bytes[0],
            reason_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        self.response_type.serialize_into(bytes);
        u8::from(self.reason).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.drawable.serialize_into(bytes);
        self.port.serialize_into(bytes);
    }
}
impl From<&VideoNotifyEvent> for [u8; 32] {
    fn from(input: &VideoNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let reason_bytes = u8::from(input.reason).serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let drawable_bytes = input.drawable.serialize();
        let port_bytes = input.port.serialize();
        [
            response_type_bytes[0],
            reason_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
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
impl From<VideoNotifyEvent> for [u8; 32] {
    fn from(input: VideoNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the PortNotify event
pub const PORT_NOTIFY_EVENT: u8 = 1;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PortNotifyEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub port: Port,
    pub attribute: xproto::Atom,
    pub value: i32,
}
impl_debug_if_no_extra_traits!(PortNotifyEvent, "PortNotifyEvent");
impl TryParse for PortNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (port, remaining) = Port::try_parse(remaining)?;
        let (attribute, remaining) = xproto::Atom::try_parse(remaining)?;
        let (value, remaining) = i32::try_parse(remaining)?;
        let result = PortNotifyEvent { response_type, sequence, time, port, attribute, value };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for PortNotifyEvent {
    type Bytes = [u8; 20];
    fn serialize(&self) -> [u8; 20] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let port_bytes = self.port.serialize();
        let attribute_bytes = self.attribute.serialize();
        let value_bytes = self.value.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
            attribute_bytes[0],
            attribute_bytes[1],
            attribute_bytes[2],
            attribute_bytes[3],
            value_bytes[0],
            value_bytes[1],
            value_bytes[2],
            value_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(20);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.port.serialize_into(bytes);
        self.attribute.serialize_into(bytes);
        self.value.serialize_into(bytes);
    }
}
impl From<&PortNotifyEvent> for [u8; 32] {
    fn from(input: &PortNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let port_bytes = input.port.serialize();
        let attribute_bytes = input.attribute.serialize();
        let value_bytes = input.value.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
            attribute_bytes[0],
            attribute_bytes[1],
            attribute_bytes[2],
            attribute_bytes[3],
            value_bytes[0],
            value_bytes[1],
            value_bytes[2],
            value_bytes[3],
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
impl From<PortNotifyEvent> for [u8; 32] {
    fn from(input: PortNotifyEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the QueryExtension request
pub const QUERY_EXTENSION_REQUEST: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryExtensionRequest;
impl_debug_if_no_extra_traits!(QueryExtensionRequest, "QueryExtensionRequest");
impl QueryExtensionRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            QUERY_EXTENSION_REQUEST,
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
        if header.minor_opcode != QUERY_EXTENSION_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(QueryExtensionRequest
        )
    }
}
impl Request for QueryExtensionRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryExtensionRequest {
    type Reply = QueryExtensionReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryExtensionReply {
    pub sequence: u16,
    pub length: u32,
    pub major: u16,
    pub minor: u16,
}
impl_debug_if_no_extra_traits!(QueryExtensionReply, "QueryExtensionReply");
impl TryParse for QueryExtensionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (major, remaining) = u16::try_parse(remaining)?;
        let (minor, remaining) = u16::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryExtensionReply { sequence, length, major, minor };
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
        let major_bytes = self.major.serialize();
        let minor_bytes = self.minor.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            major_bytes[0],
            major_bytes[1],
            minor_bytes[0],
            minor_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.major.serialize_into(bytes);
        self.minor.serialize_into(bytes);
    }
}

/// Opcode for the QueryAdaptors request
pub const QUERY_ADAPTORS_REQUEST: u8 = 1;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryAdaptorsRequest {
    pub window: xproto::Window,
}
impl_debug_if_no_extra_traits!(QueryAdaptorsRequest, "QueryAdaptorsRequest");
impl QueryAdaptorsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_ADAPTORS_REQUEST,
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
        if header.minor_opcode != QUERY_ADAPTORS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let _ = remaining;
        Ok(QueryAdaptorsRequest {
            window,
        })
    }
}
impl Request for QueryAdaptorsRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryAdaptorsRequest {
    type Reply = QueryAdaptorsReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryAdaptorsReply {
    pub sequence: u16,
    pub length: u32,
    pub info: Vec<AdaptorInfo>,
}
impl_debug_if_no_extra_traits!(QueryAdaptorsReply, "QueryAdaptorsReply");
impl TryParse for QueryAdaptorsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (num_adaptors, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
        let (info, remaining) = crate::x11_utils::parse_list::<AdaptorInfo>(remaining, num_adaptors.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryAdaptorsReply { sequence, length, info };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryAdaptorsReply {
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
        let num_adaptors = u16::try_from(self.info.len()).expect("`info` has too many elements");
        num_adaptors.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
        self.info.serialize_into(bytes);
    }
}
impl QueryAdaptorsReply {
    /// Get the value of the `num_adaptors` field.
    ///
    /// The `num_adaptors` field is used as the length field of the `info` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_adaptors(&self) -> u16 {
        self.info.len()
            .try_into().unwrap()
    }
}

/// Opcode for the QueryEncodings request
pub const QUERY_ENCODINGS_REQUEST: u8 = 2;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryEncodingsRequest {
    pub port: Port,
}
impl_debug_if_no_extra_traits!(QueryEncodingsRequest, "QueryEncodingsRequest");
impl QueryEncodingsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_ENCODINGS_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
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
        if header.minor_opcode != QUERY_ENCODINGS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let _ = remaining;
        Ok(QueryEncodingsRequest {
            port,
        })
    }
}
impl Request for QueryEncodingsRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryEncodingsRequest {
    type Reply = QueryEncodingsReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryEncodingsReply {
    pub sequence: u16,
    pub length: u32,
    pub info: Vec<EncodingInfo>,
}
impl_debug_if_no_extra_traits!(QueryEncodingsReply, "QueryEncodingsReply");
impl TryParse for QueryEncodingsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (num_encodings, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
        let (info, remaining) = crate::x11_utils::parse_list::<EncodingInfo>(remaining, num_encodings.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryEncodingsReply { sequence, length, info };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryEncodingsReply {
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
        let num_encodings = u16::try_from(self.info.len()).expect("`info` has too many elements");
        num_encodings.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
        self.info.serialize_into(bytes);
    }
}
impl QueryEncodingsReply {
    /// Get the value of the `num_encodings` field.
    ///
    /// The `num_encodings` field is used as the length field of the `info` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_encodings(&self) -> u16 {
        self.info.len()
            .try_into().unwrap()
    }
}

/// Opcode for the GrabPort request
pub const GRAB_PORT_REQUEST: u8 = 3;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrabPortRequest {
    pub port: Port,
    pub time: xproto::Timestamp,
}
impl_debug_if_no_extra_traits!(GrabPortRequest, "GrabPortRequest");
impl GrabPortRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let time_bytes = self.time.serialize();
        let mut request0 = vec![
            major_opcode,
            GRAB_PORT_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
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
        if header.minor_opcode != GRAB_PORT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let _ = remaining;
        Ok(GrabPortRequest {
            port,
            time,
        })
    }
}
impl Request for GrabPortRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GrabPortRequest {
    type Reply = GrabPortReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrabPortReply {
    pub result: GrabPortStatus,
    pub sequence: u16,
    pub length: u32,
}
impl_debug_if_no_extra_traits!(GrabPortReply, "GrabPortReply");
impl TryParse for GrabPortReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (result, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = result.into();
        let result = GrabPortReply { result, sequence, length };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GrabPortReply {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let response_type_bytes = &[1];
        let result_bytes = u8::from(self.result).serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        [
            response_type_bytes[0],
            result_bytes[0],
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
        u8::from(self.result).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
    }
}

/// Opcode for the UngrabPort request
pub const UNGRAB_PORT_REQUEST: u8 = 4;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UngrabPortRequest {
    pub port: Port,
    pub time: xproto::Timestamp,
}
impl_debug_if_no_extra_traits!(UngrabPortRequest, "UngrabPortRequest");
impl UngrabPortRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let time_bytes = self.time.serialize();
        let mut request0 = vec![
            major_opcode,
            UNGRAB_PORT_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
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
        if header.minor_opcode != UNGRAB_PORT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let _ = remaining;
        Ok(UngrabPortRequest {
            port,
            time,
        })
    }
}
impl Request for UngrabPortRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for UngrabPortRequest {
}

/// Opcode for the PutVideo request
pub const PUT_VIDEO_REQUEST: u8 = 5;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PutVideoRequest {
    pub port: Port,
    pub drawable: xproto::Drawable,
    pub gc: xproto::Gcontext,
    pub vid_x: i16,
    pub vid_y: i16,
    pub vid_w: u16,
    pub vid_h: u16,
    pub drw_x: i16,
    pub drw_y: i16,
    pub drw_w: u16,
    pub drw_h: u16,
}
impl_debug_if_no_extra_traits!(PutVideoRequest, "PutVideoRequest");
impl PutVideoRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let vid_x_bytes = self.vid_x.serialize();
        let vid_y_bytes = self.vid_y.serialize();
        let vid_w_bytes = self.vid_w.serialize();
        let vid_h_bytes = self.vid_h.serialize();
        let drw_x_bytes = self.drw_x.serialize();
        let drw_y_bytes = self.drw_y.serialize();
        let drw_w_bytes = self.drw_w.serialize();
        let drw_h_bytes = self.drw_h.serialize();
        let mut request0 = vec![
            major_opcode,
            PUT_VIDEO_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            vid_x_bytes[0],
            vid_x_bytes[1],
            vid_y_bytes[0],
            vid_y_bytes[1],
            vid_w_bytes[0],
            vid_w_bytes[1],
            vid_h_bytes[0],
            vid_h_bytes[1],
            drw_x_bytes[0],
            drw_x_bytes[1],
            drw_y_bytes[0],
            drw_y_bytes[1],
            drw_w_bytes[0],
            drw_w_bytes[1],
            drw_h_bytes[0],
            drw_h_bytes[1],
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
        if header.minor_opcode != PUT_VIDEO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let (drawable, remaining) = xproto::Drawable::try_parse(remaining)?;
        let (gc, remaining) = xproto::Gcontext::try_parse(remaining)?;
        let (vid_x, remaining) = i16::try_parse(remaining)?;
        let (vid_y, remaining) = i16::try_parse(remaining)?;
        let (vid_w, remaining) = u16::try_parse(remaining)?;
        let (vid_h, remaining) = u16::try_parse(remaining)?;
        let (drw_x, remaining) = i16::try_parse(remaining)?;
        let (drw_y, remaining) = i16::try_parse(remaining)?;
        let (drw_w, remaining) = u16::try_parse(remaining)?;
        let (drw_h, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(PutVideoRequest {
            port,
            drawable,
            gc,
            vid_x,
            vid_y,
            vid_w,
            vid_h,
            drw_x,
            drw_y,
            drw_w,
            drw_h,
        })
    }
}
impl Request for PutVideoRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for PutVideoRequest {
}

/// Opcode for the PutStill request
pub const PUT_STILL_REQUEST: u8 = 6;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PutStillRequest {
    pub port: Port,
    pub drawable: xproto::Drawable,
    pub gc: xproto::Gcontext,
    pub vid_x: i16,
    pub vid_y: i16,
    pub vid_w: u16,
    pub vid_h: u16,
    pub drw_x: i16,
    pub drw_y: i16,
    pub drw_w: u16,
    pub drw_h: u16,
}
impl_debug_if_no_extra_traits!(PutStillRequest, "PutStillRequest");
impl PutStillRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let vid_x_bytes = self.vid_x.serialize();
        let vid_y_bytes = self.vid_y.serialize();
        let vid_w_bytes = self.vid_w.serialize();
        let vid_h_bytes = self.vid_h.serialize();
        let drw_x_bytes = self.drw_x.serialize();
        let drw_y_bytes = self.drw_y.serialize();
        let drw_w_bytes = self.drw_w.serialize();
        let drw_h_bytes = self.drw_h.serialize();
        let mut request0 = vec![
            major_opcode,
            PUT_STILL_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            vid_x_bytes[0],
            vid_x_bytes[1],
            vid_y_bytes[0],
            vid_y_bytes[1],
            vid_w_bytes[0],
            vid_w_bytes[1],
            vid_h_bytes[0],
            vid_h_bytes[1],
            drw_x_bytes[0],
            drw_x_bytes[1],
            drw_y_bytes[0],
            drw_y_bytes[1],
            drw_w_bytes[0],
            drw_w_bytes[1],
            drw_h_bytes[0],
            drw_h_bytes[1],
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
        if header.minor_opcode != PUT_STILL_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let (drawable, remaining) = xproto::Drawable::try_parse(remaining)?;
        let (gc, remaining) = xproto::Gcontext::try_parse(remaining)?;
        let (vid_x, remaining) = i16::try_parse(remaining)?;
        let (vid_y, remaining) = i16::try_parse(remaining)?;
        let (vid_w, remaining) = u16::try_parse(remaining)?;
        let (vid_h, remaining) = u16::try_parse(remaining)?;
        let (drw_x, remaining) = i16::try_parse(remaining)?;
        let (drw_y, remaining) = i16::try_parse(remaining)?;
        let (drw_w, remaining) = u16::try_parse(remaining)?;
        let (drw_h, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(PutStillRequest {
            port,
            drawable,
            gc,
            vid_x,
            vid_y,
            vid_w,
            vid_h,
            drw_x,
            drw_y,
            drw_w,
            drw_h,
        })
    }
}
impl Request for PutStillRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for PutStillRequest {
}

/// Opcode for the GetVideo request
pub const GET_VIDEO_REQUEST: u8 = 7;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetVideoRequest {
    pub port: Port,
    pub drawable: xproto::Drawable,
    pub gc: xproto::Gcontext,
    pub vid_x: i16,
    pub vid_y: i16,
    pub vid_w: u16,
    pub vid_h: u16,
    pub drw_x: i16,
    pub drw_y: i16,
    pub drw_w: u16,
    pub drw_h: u16,
}
impl_debug_if_no_extra_traits!(GetVideoRequest, "GetVideoRequest");
impl GetVideoRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let vid_x_bytes = self.vid_x.serialize();
        let vid_y_bytes = self.vid_y.serialize();
        let vid_w_bytes = self.vid_w.serialize();
        let vid_h_bytes = self.vid_h.serialize();
        let drw_x_bytes = self.drw_x.serialize();
        let drw_y_bytes = self.drw_y.serialize();
        let drw_w_bytes = self.drw_w.serialize();
        let drw_h_bytes = self.drw_h.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_VIDEO_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            vid_x_bytes[0],
            vid_x_bytes[1],
            vid_y_bytes[0],
            vid_y_bytes[1],
            vid_w_bytes[0],
            vid_w_bytes[1],
            vid_h_bytes[0],
            vid_h_bytes[1],
            drw_x_bytes[0],
            drw_x_bytes[1],
            drw_y_bytes[0],
            drw_y_bytes[1],
            drw_w_bytes[0],
            drw_w_bytes[1],
            drw_h_bytes[0],
            drw_h_bytes[1],
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
        if header.minor_opcode != GET_VIDEO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let (drawable, remaining) = xproto::Drawable::try_parse(remaining)?;
        let (gc, remaining) = xproto::Gcontext::try_parse(remaining)?;
        let (vid_x, remaining) = i16::try_parse(remaining)?;
        let (vid_y, remaining) = i16::try_parse(remaining)?;
        let (vid_w, remaining) = u16::try_parse(remaining)?;
        let (vid_h, remaining) = u16::try_parse(remaining)?;
        let (drw_x, remaining) = i16::try_parse(remaining)?;
        let (drw_y, remaining) = i16::try_parse(remaining)?;
        let (drw_w, remaining) = u16::try_parse(remaining)?;
        let (drw_h, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetVideoRequest {
            port,
            drawable,
            gc,
            vid_x,
            vid_y,
            vid_w,
            vid_h,
            drw_x,
            drw_y,
            drw_w,
            drw_h,
        })
    }
}
impl Request for GetVideoRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for GetVideoRequest {
}

/// Opcode for the GetStill request
pub const GET_STILL_REQUEST: u8 = 8;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetStillRequest {
    pub port: Port,
    pub drawable: xproto::Drawable,
    pub gc: xproto::Gcontext,
    pub vid_x: i16,
    pub vid_y: i16,
    pub vid_w: u16,
    pub vid_h: u16,
    pub drw_x: i16,
    pub drw_y: i16,
    pub drw_w: u16,
    pub drw_h: u16,
}
impl_debug_if_no_extra_traits!(GetStillRequest, "GetStillRequest");
impl GetStillRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let vid_x_bytes = self.vid_x.serialize();
        let vid_y_bytes = self.vid_y.serialize();
        let vid_w_bytes = self.vid_w.serialize();
        let vid_h_bytes = self.vid_h.serialize();
        let drw_x_bytes = self.drw_x.serialize();
        let drw_y_bytes = self.drw_y.serialize();
        let drw_w_bytes = self.drw_w.serialize();
        let drw_h_bytes = self.drw_h.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_STILL_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            vid_x_bytes[0],
            vid_x_bytes[1],
            vid_y_bytes[0],
            vid_y_bytes[1],
            vid_w_bytes[0],
            vid_w_bytes[1],
            vid_h_bytes[0],
            vid_h_bytes[1],
            drw_x_bytes[0],
            drw_x_bytes[1],
            drw_y_bytes[0],
            drw_y_bytes[1],
            drw_w_bytes[0],
            drw_w_bytes[1],
            drw_h_bytes[0],
            drw_h_bytes[1],
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
        if header.minor_opcode != GET_STILL_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let (drawable, remaining) = xproto::Drawable::try_parse(remaining)?;
        let (gc, remaining) = xproto::Gcontext::try_parse(remaining)?;
        let (vid_x, remaining) = i16::try_parse(remaining)?;
        let (vid_y, remaining) = i16::try_parse(remaining)?;
        let (vid_w, remaining) = u16::try_parse(remaining)?;
        let (vid_h, remaining) = u16::try_parse(remaining)?;
        let (drw_x, remaining) = i16::try_parse(remaining)?;
        let (drw_y, remaining) = i16::try_parse(remaining)?;
        let (drw_w, remaining) = u16::try_parse(remaining)?;
        let (drw_h, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetStillRequest {
            port,
            drawable,
            gc,
            vid_x,
            vid_y,
            vid_w,
            vid_h,
            drw_x,
            drw_y,
            drw_w,
            drw_h,
        })
    }
}
impl Request for GetStillRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for GetStillRequest {
}

/// Opcode for the StopVideo request
pub const STOP_VIDEO_REQUEST: u8 = 9;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StopVideoRequest {
    pub port: Port,
    pub drawable: xproto::Drawable,
}
impl_debug_if_no_extra_traits!(StopVideoRequest, "StopVideoRequest");
impl StopVideoRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let drawable_bytes = self.drawable.serialize();
        let mut request0 = vec![
            major_opcode,
            STOP_VIDEO_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
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
        if header.minor_opcode != STOP_VIDEO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let (drawable, remaining) = xproto::Drawable::try_parse(remaining)?;
        let _ = remaining;
        Ok(StopVideoRequest {
            port,
            drawable,
        })
    }
}
impl Request for StopVideoRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for StopVideoRequest {
}

/// Opcode for the SelectVideoNotify request
pub const SELECT_VIDEO_NOTIFY_REQUEST: u8 = 10;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectVideoNotifyRequest {
    pub drawable: xproto::Drawable,
    pub onoff: bool,
}
impl_debug_if_no_extra_traits!(SelectVideoNotifyRequest, "SelectVideoNotifyRequest");
impl SelectVideoNotifyRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let onoff_bytes = self.onoff.serialize();
        let mut request0 = vec![
            major_opcode,
            SELECT_VIDEO_NOTIFY_REQUEST,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            onoff_bytes[0],
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
        if header.minor_opcode != SELECT_VIDEO_NOTIFY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let (onoff, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(SelectVideoNotifyRequest {
            drawable,
            onoff,
        })
    }
}
impl Request for SelectVideoNotifyRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SelectVideoNotifyRequest {
}

/// Opcode for the SelectPortNotify request
pub const SELECT_PORT_NOTIFY_REQUEST: u8 = 11;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectPortNotifyRequest {
    pub port: Port,
    pub onoff: bool,
}
impl_debug_if_no_extra_traits!(SelectPortNotifyRequest, "SelectPortNotifyRequest");
impl SelectPortNotifyRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let onoff_bytes = self.onoff.serialize();
        let mut request0 = vec![
            major_opcode,
            SELECT_PORT_NOTIFY_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
            onoff_bytes[0],
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
        if header.minor_opcode != SELECT_PORT_NOTIFY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let (onoff, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(SelectPortNotifyRequest {
            port,
            onoff,
        })
    }
}
impl Request for SelectPortNotifyRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SelectPortNotifyRequest {
}

/// Opcode for the QueryBestSize request
pub const QUERY_BEST_SIZE_REQUEST: u8 = 12;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryBestSizeRequest {
    pub port: Port,
    pub vid_w: u16,
    pub vid_h: u16,
    pub drw_w: u16,
    pub drw_h: u16,
    pub motion: bool,
}
impl_debug_if_no_extra_traits!(QueryBestSizeRequest, "QueryBestSizeRequest");
impl QueryBestSizeRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let vid_w_bytes = self.vid_w.serialize();
        let vid_h_bytes = self.vid_h.serialize();
        let drw_w_bytes = self.drw_w.serialize();
        let drw_h_bytes = self.drw_h.serialize();
        let motion_bytes = self.motion.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_BEST_SIZE_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
            vid_w_bytes[0],
            vid_w_bytes[1],
            vid_h_bytes[0],
            vid_h_bytes[1],
            drw_w_bytes[0],
            drw_w_bytes[1],
            drw_h_bytes[0],
            drw_h_bytes[1],
            motion_bytes[0],
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
        if header.minor_opcode != QUERY_BEST_SIZE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let (vid_w, remaining) = u16::try_parse(remaining)?;
        let (vid_h, remaining) = u16::try_parse(remaining)?;
        let (drw_w, remaining) = u16::try_parse(remaining)?;
        let (drw_h, remaining) = u16::try_parse(remaining)?;
        let (motion, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(QueryBestSizeRequest {
            port,
            vid_w,
            vid_h,
            drw_w,
            drw_h,
            motion,
        })
    }
}
impl Request for QueryBestSizeRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
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
    pub actual_width: u16,
    pub actual_height: u16,
}
impl_debug_if_no_extra_traits!(QueryBestSizeReply, "QueryBestSizeReply");
impl TryParse for QueryBestSizeReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (actual_width, remaining) = u16::try_parse(remaining)?;
        let (actual_height, remaining) = u16::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryBestSizeReply { sequence, length, actual_width, actual_height };
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
        let actual_width_bytes = self.actual_width.serialize();
        let actual_height_bytes = self.actual_height.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            actual_width_bytes[0],
            actual_width_bytes[1],
            actual_height_bytes[0],
            actual_height_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.actual_width.serialize_into(bytes);
        self.actual_height.serialize_into(bytes);
    }
}

/// Opcode for the SetPortAttribute request
pub const SET_PORT_ATTRIBUTE_REQUEST: u8 = 13;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetPortAttributeRequest {
    pub port: Port,
    pub attribute: xproto::Atom,
    pub value: i32,
}
impl_debug_if_no_extra_traits!(SetPortAttributeRequest, "SetPortAttributeRequest");
impl SetPortAttributeRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let attribute_bytes = self.attribute.serialize();
        let value_bytes = self.value.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_PORT_ATTRIBUTE_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
            attribute_bytes[0],
            attribute_bytes[1],
            attribute_bytes[2],
            attribute_bytes[3],
            value_bytes[0],
            value_bytes[1],
            value_bytes[2],
            value_bytes[3],
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
        if header.minor_opcode != SET_PORT_ATTRIBUTE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let (attribute, remaining) = xproto::Atom::try_parse(remaining)?;
        let (value, remaining) = i32::try_parse(remaining)?;
        let _ = remaining;
        Ok(SetPortAttributeRequest {
            port,
            attribute,
            value,
        })
    }
}
impl Request for SetPortAttributeRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SetPortAttributeRequest {
}

/// Opcode for the GetPortAttribute request
pub const GET_PORT_ATTRIBUTE_REQUEST: u8 = 14;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPortAttributeRequest {
    pub port: Port,
    pub attribute: xproto::Atom,
}
impl_debug_if_no_extra_traits!(GetPortAttributeRequest, "GetPortAttributeRequest");
impl GetPortAttributeRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let attribute_bytes = self.attribute.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_PORT_ATTRIBUTE_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
            attribute_bytes[0],
            attribute_bytes[1],
            attribute_bytes[2],
            attribute_bytes[3],
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
        if header.minor_opcode != GET_PORT_ATTRIBUTE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let (attribute, remaining) = xproto::Atom::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetPortAttributeRequest {
            port,
            attribute,
        })
    }
}
impl Request for GetPortAttributeRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetPortAttributeRequest {
    type Reply = GetPortAttributeReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPortAttributeReply {
    pub sequence: u16,
    pub length: u32,
    pub value: i32,
}
impl_debug_if_no_extra_traits!(GetPortAttributeReply, "GetPortAttributeReply");
impl TryParse for GetPortAttributeReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (value, remaining) = i32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetPortAttributeReply { sequence, length, value };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetPortAttributeReply {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let value_bytes = self.value.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            value_bytes[0],
            value_bytes[1],
            value_bytes[2],
            value_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.value.serialize_into(bytes);
    }
}

/// Opcode for the QueryPortAttributes request
pub const QUERY_PORT_ATTRIBUTES_REQUEST: u8 = 15;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryPortAttributesRequest {
    pub port: Port,
}
impl_debug_if_no_extra_traits!(QueryPortAttributesRequest, "QueryPortAttributesRequest");
impl QueryPortAttributesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_PORT_ATTRIBUTES_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
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
        if header.minor_opcode != QUERY_PORT_ATTRIBUTES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let _ = remaining;
        Ok(QueryPortAttributesRequest {
            port,
        })
    }
}
impl Request for QueryPortAttributesRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryPortAttributesRequest {
    type Reply = QueryPortAttributesReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryPortAttributesReply {
    pub sequence: u16,
    pub length: u32,
    pub text_size: u32,
    pub attributes: Vec<AttributeInfo>,
}
impl_debug_if_no_extra_traits!(QueryPortAttributesReply, "QueryPortAttributesReply");
impl TryParse for QueryPortAttributesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (num_attributes, remaining) = u32::try_parse(remaining)?;
        let (text_size, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(16..).ok_or(ParseError::InsufficientData)?;
        let (attributes, remaining) = crate::x11_utils::parse_list::<AttributeInfo>(remaining, num_attributes.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryPortAttributesReply { sequence, length, text_size, attributes };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryPortAttributesReply {
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
        let num_attributes = u32::try_from(self.attributes.len()).expect("`attributes` has too many elements");
        num_attributes.serialize_into(bytes);
        self.text_size.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 16]);
        self.attributes.serialize_into(bytes);
    }
}
impl QueryPortAttributesReply {
    /// Get the value of the `num_attributes` field.
    ///
    /// The `num_attributes` field is used as the length field of the `attributes` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_attributes(&self) -> u32 {
        self.attributes.len()
            .try_into().unwrap()
    }
}

/// Opcode for the ListImageFormats request
pub const LIST_IMAGE_FORMATS_REQUEST: u8 = 16;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListImageFormatsRequest {
    pub port: Port,
}
impl_debug_if_no_extra_traits!(ListImageFormatsRequest, "ListImageFormatsRequest");
impl ListImageFormatsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let mut request0 = vec![
            major_opcode,
            LIST_IMAGE_FORMATS_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
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
        if header.minor_opcode != LIST_IMAGE_FORMATS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let _ = remaining;
        Ok(ListImageFormatsRequest {
            port,
        })
    }
}
impl Request for ListImageFormatsRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for ListImageFormatsRequest {
    type Reply = ListImageFormatsReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListImageFormatsReply {
    pub sequence: u16,
    pub length: u32,
    pub format: Vec<ImageFormatInfo>,
}
impl_debug_if_no_extra_traits!(ListImageFormatsReply, "ListImageFormatsReply");
impl TryParse for ListImageFormatsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (num_formats, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (format, remaining) = crate::x11_utils::parse_list::<ImageFormatInfo>(remaining, num_formats.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = ListImageFormatsReply { sequence, length, format };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ListImageFormatsReply {
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
        let num_formats = u32::try_from(self.format.len()).expect("`format` has too many elements");
        num_formats.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        self.format.serialize_into(bytes);
    }
}
impl ListImageFormatsReply {
    /// Get the value of the `num_formats` field.
    ///
    /// The `num_formats` field is used as the length field of the `format` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_formats(&self) -> u32 {
        self.format.len()
            .try_into().unwrap()
    }
}

/// Opcode for the QueryImageAttributes request
pub const QUERY_IMAGE_ATTRIBUTES_REQUEST: u8 = 17;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryImageAttributesRequest {
    pub port: Port,
    pub id: u32,
    pub width: u16,
    pub height: u16,
}
impl_debug_if_no_extra_traits!(QueryImageAttributesRequest, "QueryImageAttributesRequest");
impl QueryImageAttributesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let id_bytes = self.id.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_IMAGE_ATTRIBUTES_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
            id_bytes[0],
            id_bytes[1],
            id_bytes[2],
            id_bytes[3],
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
        if header.minor_opcode != QUERY_IMAGE_ATTRIBUTES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let (id, remaining) = u32::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(QueryImageAttributesRequest {
            port,
            id,
            width,
            height,
        })
    }
}
impl Request for QueryImageAttributesRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryImageAttributesRequest {
    type Reply = QueryImageAttributesReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryImageAttributesReply {
    pub sequence: u16,
    pub length: u32,
    pub data_size: u32,
    pub width: u16,
    pub height: u16,
    pub pitches: Vec<u32>,
    pub offsets: Vec<u32>,
}
impl_debug_if_no_extra_traits!(QueryImageAttributesReply, "QueryImageAttributesReply");
impl TryParse for QueryImageAttributesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (num_planes, remaining) = u32::try_parse(remaining)?;
        let (data_size, remaining) = u32::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(12..).ok_or(ParseError::InsufficientData)?;
        let (pitches, remaining) = crate::x11_utils::parse_list::<u32>(remaining, num_planes.try_to_usize()?)?;
        let (offsets, remaining) = crate::x11_utils::parse_list::<u32>(remaining, num_planes.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryImageAttributesReply { sequence, length, data_size, width, height, pitches, offsets };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryImageAttributesReply {
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
        let num_planes = u32::try_from(self.pitches.len()).expect("`pitches` has too many elements");
        num_planes.serialize_into(bytes);
        self.data_size.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 12]);
        self.pitches.serialize_into(bytes);
        assert_eq!(self.offsets.len(), usize::try_from(num_planes).unwrap(), "`offsets` has an incorrect length");
        self.offsets.serialize_into(bytes);
    }
}
impl QueryImageAttributesReply {
    /// Get the value of the `num_planes` field.
    ///
    /// The `num_planes` field is used as the length field of the `pitches` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_planes(&self) -> u32 {
        self.pitches.len()
            .try_into().unwrap()
    }
}

/// Opcode for the PutImage request
pub const PUT_IMAGE_REQUEST: u8 = 18;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PutImageRequest<'input> {
    pub port: Port,
    pub drawable: xproto::Drawable,
    pub gc: xproto::Gcontext,
    pub id: u32,
    pub src_x: i16,
    pub src_y: i16,
    pub src_w: u16,
    pub src_h: u16,
    pub drw_x: i16,
    pub drw_y: i16,
    pub drw_w: u16,
    pub drw_h: u16,
    pub width: u16,
    pub height: u16,
    pub data: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(PutImageRequest<'_>, "PutImageRequest");
impl<'input> PutImageRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let id_bytes = self.id.serialize();
        let src_x_bytes = self.src_x.serialize();
        let src_y_bytes = self.src_y.serialize();
        let src_w_bytes = self.src_w.serialize();
        let src_h_bytes = self.src_h.serialize();
        let drw_x_bytes = self.drw_x.serialize();
        let drw_y_bytes = self.drw_y.serialize();
        let drw_w_bytes = self.drw_w.serialize();
        let drw_h_bytes = self.drw_h.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let mut request0 = vec![
            major_opcode,
            PUT_IMAGE_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            id_bytes[0],
            id_bytes[1],
            id_bytes[2],
            id_bytes[3],
            src_x_bytes[0],
            src_x_bytes[1],
            src_y_bytes[0],
            src_y_bytes[1],
            src_w_bytes[0],
            src_w_bytes[1],
            src_h_bytes[0],
            src_h_bytes[1],
            drw_x_bytes[0],
            drw_x_bytes[1],
            drw_y_bytes[0],
            drw_y_bytes[1],
            drw_w_bytes[0],
            drw_w_bytes[1],
            drw_h_bytes[0],
            drw_h_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
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
        if header.minor_opcode != PUT_IMAGE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let (drawable, remaining) = xproto::Drawable::try_parse(remaining)?;
        let (gc, remaining) = xproto::Gcontext::try_parse(remaining)?;
        let (id, remaining) = u32::try_parse(remaining)?;
        let (src_x, remaining) = i16::try_parse(remaining)?;
        let (src_y, remaining) = i16::try_parse(remaining)?;
        let (src_w, remaining) = u16::try_parse(remaining)?;
        let (src_h, remaining) = u16::try_parse(remaining)?;
        let (drw_x, remaining) = i16::try_parse(remaining)?;
        let (drw_y, remaining) = i16::try_parse(remaining)?;
        let (drw_w, remaining) = u16::try_parse(remaining)?;
        let (drw_h, remaining) = u16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (data, remaining) = remaining.split_at(remaining.len());
        let _ = remaining;
        Ok(PutImageRequest {
            port,
            drawable,
            gc,
            id,
            src_x,
            src_y,
            src_w,
            src_h,
            drw_x,
            drw_y,
            drw_w,
            drw_h,
            width,
            height,
            data: Cow::Borrowed(data),
        })
    }
    /// Clone all borrowed data in this PutImageRequest.
    pub fn into_owned(self) -> PutImageRequest<'static> {
        PutImageRequest {
            port: self.port,
            drawable: self.drawable,
            gc: self.gc,
            id: self.id,
            src_x: self.src_x,
            src_y: self.src_y,
            src_w: self.src_w,
            src_h: self.src_h,
            drw_x: self.drw_x,
            drw_y: self.drw_y,
            drw_w: self.drw_w,
            drw_h: self.drw_h,
            width: self.width,
            height: self.height,
            data: Cow::Owned(self.data.into_owned()),
        }
    }
}
impl<'input> Request for PutImageRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for PutImageRequest<'input> {
}

/// Opcode for the ShmPutImage request
pub const SHM_PUT_IMAGE_REQUEST: u8 = 19;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShmPutImageRequest {
    pub port: Port,
    pub drawable: xproto::Drawable,
    pub gc: xproto::Gcontext,
    pub shmseg: shm::Seg,
    pub id: u32,
    pub offset: u32,
    pub src_x: i16,
    pub src_y: i16,
    pub src_w: u16,
    pub src_h: u16,
    pub drw_x: i16,
    pub drw_y: i16,
    pub drw_w: u16,
    pub drw_h: u16,
    pub width: u16,
    pub height: u16,
    pub send_event: u8,
}
impl_debug_if_no_extra_traits!(ShmPutImageRequest, "ShmPutImageRequest");
impl ShmPutImageRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_bytes = self.port.serialize();
        let drawable_bytes = self.drawable.serialize();
        let gc_bytes = self.gc.serialize();
        let shmseg_bytes = self.shmseg.serialize();
        let id_bytes = self.id.serialize();
        let offset_bytes = self.offset.serialize();
        let src_x_bytes = self.src_x.serialize();
        let src_y_bytes = self.src_y.serialize();
        let src_w_bytes = self.src_w.serialize();
        let src_h_bytes = self.src_h.serialize();
        let drw_x_bytes = self.drw_x.serialize();
        let drw_y_bytes = self.drw_y.serialize();
        let drw_w_bytes = self.drw_w.serialize();
        let drw_h_bytes = self.drw_h.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let send_event_bytes = self.send_event.serialize();
        let mut request0 = vec![
            major_opcode,
            SHM_PUT_IMAGE_REQUEST,
            0,
            0,
            port_bytes[0],
            port_bytes[1],
            port_bytes[2],
            port_bytes[3],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            gc_bytes[0],
            gc_bytes[1],
            gc_bytes[2],
            gc_bytes[3],
            shmseg_bytes[0],
            shmseg_bytes[1],
            shmseg_bytes[2],
            shmseg_bytes[3],
            id_bytes[0],
            id_bytes[1],
            id_bytes[2],
            id_bytes[3],
            offset_bytes[0],
            offset_bytes[1],
            offset_bytes[2],
            offset_bytes[3],
            src_x_bytes[0],
            src_x_bytes[1],
            src_y_bytes[0],
            src_y_bytes[1],
            src_w_bytes[0],
            src_w_bytes[1],
            src_h_bytes[0],
            src_h_bytes[1],
            drw_x_bytes[0],
            drw_x_bytes[1],
            drw_y_bytes[0],
            drw_y_bytes[1],
            drw_w_bytes[0],
            drw_w_bytes[1],
            drw_h_bytes[0],
            drw_h_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            send_event_bytes[0],
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
        if header.minor_opcode != SHM_PUT_IMAGE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port, remaining) = Port::try_parse(value)?;
        let (drawable, remaining) = xproto::Drawable::try_parse(remaining)?;
        let (gc, remaining) = xproto::Gcontext::try_parse(remaining)?;
        let (shmseg, remaining) = shm::Seg::try_parse(remaining)?;
        let (id, remaining) = u32::try_parse(remaining)?;
        let (offset, remaining) = u32::try_parse(remaining)?;
        let (src_x, remaining) = i16::try_parse(remaining)?;
        let (src_y, remaining) = i16::try_parse(remaining)?;
        let (src_w, remaining) = u16::try_parse(remaining)?;
        let (src_h, remaining) = u16::try_parse(remaining)?;
        let (drw_x, remaining) = i16::try_parse(remaining)?;
        let (drw_y, remaining) = i16::try_parse(remaining)?;
        let (drw_w, remaining) = u16::try_parse(remaining)?;
        let (drw_h, remaining) = u16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (send_event, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(ShmPutImageRequest {
            port,
            drawable,
            gc,
            shmseg,
            id,
            offset,
            src_x,
            src_y,
            src_w,
            src_h,
            drw_x,
            drw_y,
            drw_w,
            drw_h,
            width,
            height,
            send_event,
        })
    }
}
impl Request for ShmPutImageRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for ShmPutImageRequest {
}

