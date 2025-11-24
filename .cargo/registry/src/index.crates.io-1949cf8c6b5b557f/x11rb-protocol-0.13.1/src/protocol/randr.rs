// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `RandR` X11 extension.

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
use super::render;
#[allow(unused_imports)]
use super::xproto;

/// The X11 name of the extension for QueryExtension
pub const X11_EXTENSION_NAME: &str = "RANDR";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (1, 6);

pub type Mode = u32;

pub type Crtc = u32;

pub type Output = u32;

pub type Provider = u32;

pub type Lease = u32;

/// Opcode for the BadOutput error
pub const BAD_OUTPUT_ERROR: u8 = 0;

/// Opcode for the BadCrtc error
pub const BAD_CRTC_ERROR: u8 = 1;

/// Opcode for the BadMode error
pub const BAD_MODE_ERROR: u8 = 2;

/// Opcode for the BadProvider error
pub const BAD_PROVIDER_ERROR: u8 = 3;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rotation(u16);
impl Rotation {
    pub const ROTATE0: Self = Self(1 << 0);
    pub const ROTATE90: Self = Self(1 << 1);
    pub const ROTATE180: Self = Self(1 << 2);
    pub const ROTATE270: Self = Self(1 << 3);
    pub const REFLECT_X: Self = Self(1 << 4);
    pub const REFLECT_Y: Self = Self(1 << 5);
}
impl From<Rotation> for u16 {
    #[inline]
    fn from(input: Rotation) -> Self {
        input.0
    }
}
impl From<Rotation> for Option<u16> {
    #[inline]
    fn from(input: Rotation) -> Self {
        Some(input.0)
    }
}
impl From<Rotation> for u32 {
    #[inline]
    fn from(input: Rotation) -> Self {
        u32::from(input.0)
    }
}
impl From<Rotation> for Option<u32> {
    #[inline]
    fn from(input: Rotation) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Rotation {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for Rotation {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Rotation  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ROTATE0.0.into(), "ROTATE0", "Rotate0"),
            (Self::ROTATE90.0.into(), "ROTATE90", "Rotate90"),
            (Self::ROTATE180.0.into(), "ROTATE180", "Rotate180"),
            (Self::ROTATE270.0.into(), "ROTATE270", "Rotate270"),
            (Self::REFLECT_X.0.into(), "REFLECT_X", "ReflectX"),
            (Self::REFLECT_Y.0.into(), "REFLECT_Y", "ReflectY"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(Rotation, u16);

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScreenSize {
    pub width: u16,
    pub height: u16,
    pub mwidth: u16,
    pub mheight: u16,
}
impl_debug_if_no_extra_traits!(ScreenSize, "ScreenSize");
impl TryParse for ScreenSize {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (mwidth, remaining) = u16::try_parse(remaining)?;
        let (mheight, remaining) = u16::try_parse(remaining)?;
        let result = ScreenSize { width, height, mwidth, mheight };
        Ok((result, remaining))
    }
}
impl Serialize for ScreenSize {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let mwidth_bytes = self.mwidth.serialize();
        let mheight_bytes = self.mheight.serialize();
        [
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            mwidth_bytes[0],
            mwidth_bytes[1],
            mheight_bytes[0],
            mheight_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        self.mwidth.serialize_into(bytes);
        self.mheight.serialize_into(bytes);
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RefreshRates {
    pub rates: Vec<u16>,
}
impl_debug_if_no_extra_traits!(RefreshRates, "RefreshRates");
impl TryParse for RefreshRates {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (n_rates, remaining) = u16::try_parse(remaining)?;
        let (rates, remaining) = crate::x11_utils::parse_list::<u16>(remaining, n_rates.try_to_usize()?)?;
        let result = RefreshRates { rates };
        Ok((result, remaining))
    }
}
impl Serialize for RefreshRates {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        let n_rates = u16::try_from(self.rates.len()).expect("`rates` has too many elements");
        n_rates.serialize_into(bytes);
        self.rates.serialize_into(bytes);
    }
}
impl RefreshRates {
    /// Get the value of the `nRates` field.
    ///
    /// The `nRates` field is used as the length field of the `rates` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_rates(&self) -> u16 {
        self.rates.len()
            .try_into().unwrap()
    }
}

/// Opcode for the QueryVersion request
pub const QUERY_VERSION_REQUEST: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryVersionRequest {
    pub major_version: u32,
    pub minor_version: u32,
}
impl_debug_if_no_extra_traits!(QueryVersionRequest, "QueryVersionRequest");
impl QueryVersionRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let major_version_bytes = self.major_version.serialize();
        let minor_version_bytes = self.minor_version.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_VERSION_REQUEST,
            0,
            0,
            major_version_bytes[0],
            major_version_bytes[1],
            major_version_bytes[2],
            major_version_bytes[3],
            minor_version_bytes[0],
            minor_version_bytes[1],
            minor_version_bytes[2],
            minor_version_bytes[3],
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
        let (major_version, remaining) = u32::try_parse(value)?;
        let (minor_version, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(QueryVersionRequest {
            major_version,
            minor_version,
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

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetConfig(u8);
impl SetConfig {
    pub const SUCCESS: Self = Self(0);
    pub const INVALID_CONFIG_TIME: Self = Self(1);
    pub const INVALID_TIME: Self = Self(2);
    pub const FAILED: Self = Self(3);
}
impl From<SetConfig> for u8 {
    #[inline]
    fn from(input: SetConfig) -> Self {
        input.0
    }
}
impl From<SetConfig> for Option<u8> {
    #[inline]
    fn from(input: SetConfig) -> Self {
        Some(input.0)
    }
}
impl From<SetConfig> for u16 {
    #[inline]
    fn from(input: SetConfig) -> Self {
        u16::from(input.0)
    }
}
impl From<SetConfig> for Option<u16> {
    #[inline]
    fn from(input: SetConfig) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SetConfig> for u32 {
    #[inline]
    fn from(input: SetConfig) -> Self {
        u32::from(input.0)
    }
}
impl From<SetConfig> for Option<u32> {
    #[inline]
    fn from(input: SetConfig) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SetConfig {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SetConfig  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SUCCESS.0.into(), "SUCCESS", "Success"),
            (Self::INVALID_CONFIG_TIME.0.into(), "INVALID_CONFIG_TIME", "InvalidConfigTime"),
            (Self::INVALID_TIME.0.into(), "INVALID_TIME", "InvalidTime"),
            (Self::FAILED.0.into(), "FAILED", "Failed"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the SetScreenConfig request
pub const SET_SCREEN_CONFIG_REQUEST: u8 = 2;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetScreenConfigRequest {
    pub window: xproto::Window,
    pub timestamp: xproto::Timestamp,
    pub config_timestamp: xproto::Timestamp,
    pub size_id: u16,
    pub rotation: Rotation,
    pub rate: u16,
}
impl_debug_if_no_extra_traits!(SetScreenConfigRequest, "SetScreenConfigRequest");
impl SetScreenConfigRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let timestamp_bytes = self.timestamp.serialize();
        let config_timestamp_bytes = self.config_timestamp.serialize();
        let size_id_bytes = self.size_id.serialize();
        let rotation_bytes = u16::from(self.rotation).serialize();
        let rate_bytes = self.rate.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_SCREEN_CONFIG_REQUEST,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            config_timestamp_bytes[0],
            config_timestamp_bytes[1],
            config_timestamp_bytes[2],
            config_timestamp_bytes[3],
            size_id_bytes[0],
            size_id_bytes[1],
            rotation_bytes[0],
            rotation_bytes[1],
            rate_bytes[0],
            rate_bytes[1],
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
        if header.minor_opcode != SET_SCREEN_CONFIG_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (config_timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (size_id, remaining) = u16::try_parse(remaining)?;
        let (rotation, remaining) = u16::try_parse(remaining)?;
        let rotation = rotation.into();
        let (rate, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(SetScreenConfigRequest {
            window,
            timestamp,
            config_timestamp,
            size_id,
            rotation,
            rate,
        })
    }
}
impl Request for SetScreenConfigRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for SetScreenConfigRequest {
    type Reply = SetScreenConfigReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetScreenConfigReply {
    pub status: SetConfig,
    pub sequence: u16,
    pub length: u32,
    pub new_timestamp: xproto::Timestamp,
    pub config_timestamp: xproto::Timestamp,
    pub root: xproto::Window,
    pub subpixel_order: render::SubPixel,
}
impl_debug_if_no_extra_traits!(SetScreenConfigReply, "SetScreenConfigReply");
impl TryParse for SetScreenConfigReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (status, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (new_timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (config_timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (root, remaining) = xproto::Window::try_parse(remaining)?;
        let (subpixel_order, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(10..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let status = status.into();
        let subpixel_order = subpixel_order.into();
        let result = SetScreenConfigReply { status, sequence, length, new_timestamp, config_timestamp, root, subpixel_order };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for SetScreenConfigReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let status_bytes = u8::from(self.status).serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let new_timestamp_bytes = self.new_timestamp.serialize();
        let config_timestamp_bytes = self.config_timestamp.serialize();
        let root_bytes = self.root.serialize();
        let subpixel_order_bytes = (u32::from(self.subpixel_order) as u16).serialize();
        [
            response_type_bytes[0],
            status_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            new_timestamp_bytes[0],
            new_timestamp_bytes[1],
            new_timestamp_bytes[2],
            new_timestamp_bytes[3],
            config_timestamp_bytes[0],
            config_timestamp_bytes[1],
            config_timestamp_bytes[2],
            config_timestamp_bytes[3],
            root_bytes[0],
            root_bytes[1],
            root_bytes[2],
            root_bytes[3],
            subpixel_order_bytes[0],
            subpixel_order_bytes[1],
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
        u8::from(self.status).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.new_timestamp.serialize_into(bytes);
        self.config_timestamp.serialize_into(bytes);
        self.root.serialize_into(bytes);
        (u32::from(self.subpixel_order) as u16).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 10]);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NotifyMask(u16);
impl NotifyMask {
    pub const SCREEN_CHANGE: Self = Self(1 << 0);
    pub const CRTC_CHANGE: Self = Self(1 << 1);
    pub const OUTPUT_CHANGE: Self = Self(1 << 2);
    pub const OUTPUT_PROPERTY: Self = Self(1 << 3);
    pub const PROVIDER_CHANGE: Self = Self(1 << 4);
    pub const PROVIDER_PROPERTY: Self = Self(1 << 5);
    pub const RESOURCE_CHANGE: Self = Self(1 << 6);
    pub const LEASE: Self = Self(1 << 7);
}
impl From<NotifyMask> for u16 {
    #[inline]
    fn from(input: NotifyMask) -> Self {
        input.0
    }
}
impl From<NotifyMask> for Option<u16> {
    #[inline]
    fn from(input: NotifyMask) -> Self {
        Some(input.0)
    }
}
impl From<NotifyMask> for u32 {
    #[inline]
    fn from(input: NotifyMask) -> Self {
        u32::from(input.0)
    }
}
impl From<NotifyMask> for Option<u32> {
    #[inline]
    fn from(input: NotifyMask) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for NotifyMask {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for NotifyMask {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for NotifyMask  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SCREEN_CHANGE.0.into(), "SCREEN_CHANGE", "ScreenChange"),
            (Self::CRTC_CHANGE.0.into(), "CRTC_CHANGE", "CrtcChange"),
            (Self::OUTPUT_CHANGE.0.into(), "OUTPUT_CHANGE", "OutputChange"),
            (Self::OUTPUT_PROPERTY.0.into(), "OUTPUT_PROPERTY", "OutputProperty"),
            (Self::PROVIDER_CHANGE.0.into(), "PROVIDER_CHANGE", "ProviderChange"),
            (Self::PROVIDER_PROPERTY.0.into(), "PROVIDER_PROPERTY", "ProviderProperty"),
            (Self::RESOURCE_CHANGE.0.into(), "RESOURCE_CHANGE", "ResourceChange"),
            (Self::LEASE.0.into(), "LEASE", "Lease"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(NotifyMask, u16);

/// Opcode for the SelectInput request
pub const SELECT_INPUT_REQUEST: u8 = 4;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectInputRequest {
    pub window: xproto::Window,
    pub enable: NotifyMask,
}
impl_debug_if_no_extra_traits!(SelectInputRequest, "SelectInputRequest");
impl SelectInputRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let enable_bytes = u16::from(self.enable).serialize();
        let mut request0 = vec![
            major_opcode,
            SELECT_INPUT_REQUEST,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            enable_bytes[0],
            enable_bytes[1],
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
        if header.minor_opcode != SELECT_INPUT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (enable, remaining) = u16::try_parse(remaining)?;
        let enable = enable.into();
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(SelectInputRequest {
            window,
            enable,
        })
    }
}
impl Request for SelectInputRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SelectInputRequest {
}

/// Opcode for the GetScreenInfo request
pub const GET_SCREEN_INFO_REQUEST: u8 = 5;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetScreenInfoRequest {
    pub window: xproto::Window,
}
impl_debug_if_no_extra_traits!(GetScreenInfoRequest, "GetScreenInfoRequest");
impl GetScreenInfoRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_SCREEN_INFO_REQUEST,
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
        if header.minor_opcode != GET_SCREEN_INFO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let _ = remaining;
        Ok(GetScreenInfoRequest {
            window,
        })
    }
}
impl Request for GetScreenInfoRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetScreenInfoRequest {
    type Reply = GetScreenInfoReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetScreenInfoReply {
    pub rotations: Rotation,
    pub sequence: u16,
    pub length: u32,
    pub root: xproto::Window,
    pub timestamp: xproto::Timestamp,
    pub config_timestamp: xproto::Timestamp,
    pub size_id: u16,
    pub rotation: Rotation,
    pub rate: u16,
    pub n_info: u16,
    pub sizes: Vec<ScreenSize>,
    pub rates: Vec<RefreshRates>,
}
impl_debug_if_no_extra_traits!(GetScreenInfoReply, "GetScreenInfoReply");
impl TryParse for GetScreenInfoReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (rotations, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (root, remaining) = xproto::Window::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (config_timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (n_sizes, remaining) = u16::try_parse(remaining)?;
        let (size_id, remaining) = u16::try_parse(remaining)?;
        let (rotation, remaining) = u16::try_parse(remaining)?;
        let (rate, remaining) = u16::try_parse(remaining)?;
        let (n_info, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (sizes, remaining) = crate::x11_utils::parse_list::<ScreenSize>(remaining, n_sizes.try_to_usize()?)?;
        let (rates, remaining) = crate::x11_utils::parse_list::<RefreshRates>(remaining, u32::from(n_info).checked_sub(u32::from(n_sizes)).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let rotations = rotations.into();
        let rotation = rotation.into();
        let result = GetScreenInfoReply { rotations, sequence, length, root, timestamp, config_timestamp, size_id, rotation, rate, n_info, sizes, rates };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetScreenInfoReply {
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
        (u16::from(self.rotations) as u8).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.root.serialize_into(bytes);
        self.timestamp.serialize_into(bytes);
        self.config_timestamp.serialize_into(bytes);
        let n_sizes = u16::try_from(self.sizes.len()).expect("`sizes` has too many elements");
        n_sizes.serialize_into(bytes);
        self.size_id.serialize_into(bytes);
        u16::from(self.rotation).serialize_into(bytes);
        self.rate.serialize_into(bytes);
        self.n_info.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.sizes.serialize_into(bytes);
        assert_eq!(self.rates.len(), usize::try_from(u32::from(self.n_info).checked_sub(u32::from(n_sizes)).unwrap()).unwrap(), "`rates` has an incorrect length");
        self.rates.serialize_into(bytes);
    }
}
impl GetScreenInfoReply {
    /// Get the value of the `nSizes` field.
    ///
    /// The `nSizes` field is used as the length field of the `sizes` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_sizes(&self) -> u16 {
        self.sizes.len()
            .try_into().unwrap()
    }
}

/// Opcode for the GetScreenSizeRange request
pub const GET_SCREEN_SIZE_RANGE_REQUEST: u8 = 6;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetScreenSizeRangeRequest {
    pub window: xproto::Window,
}
impl_debug_if_no_extra_traits!(GetScreenSizeRangeRequest, "GetScreenSizeRangeRequest");
impl GetScreenSizeRangeRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_SCREEN_SIZE_RANGE_REQUEST,
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
        if header.minor_opcode != GET_SCREEN_SIZE_RANGE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let _ = remaining;
        Ok(GetScreenSizeRangeRequest {
            window,
        })
    }
}
impl Request for GetScreenSizeRangeRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetScreenSizeRangeRequest {
    type Reply = GetScreenSizeRangeReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetScreenSizeRangeReply {
    pub sequence: u16,
    pub length: u32,
    pub min_width: u16,
    pub min_height: u16,
    pub max_width: u16,
    pub max_height: u16,
}
impl_debug_if_no_extra_traits!(GetScreenSizeRangeReply, "GetScreenSizeRangeReply");
impl TryParse for GetScreenSizeRangeReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (min_width, remaining) = u16::try_parse(remaining)?;
        let (min_height, remaining) = u16::try_parse(remaining)?;
        let (max_width, remaining) = u16::try_parse(remaining)?;
        let (max_height, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(16..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetScreenSizeRangeReply { sequence, length, min_width, min_height, max_width, max_height };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetScreenSizeRangeReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let min_width_bytes = self.min_width.serialize();
        let min_height_bytes = self.min_height.serialize();
        let max_width_bytes = self.max_width.serialize();
        let max_height_bytes = self.max_height.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            min_width_bytes[0],
            min_width_bytes[1],
            min_height_bytes[0],
            min_height_bytes[1],
            max_width_bytes[0],
            max_width_bytes[1],
            max_height_bytes[0],
            max_height_bytes[1],
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
        self.min_width.serialize_into(bytes);
        self.min_height.serialize_into(bytes);
        self.max_width.serialize_into(bytes);
        self.max_height.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 16]);
    }
}

/// Opcode for the SetScreenSize request
pub const SET_SCREEN_SIZE_REQUEST: u8 = 7;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetScreenSizeRequest {
    pub window: xproto::Window,
    pub width: u16,
    pub height: u16,
    pub mm_width: u32,
    pub mm_height: u32,
}
impl_debug_if_no_extra_traits!(SetScreenSizeRequest, "SetScreenSizeRequest");
impl SetScreenSizeRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let mm_width_bytes = self.mm_width.serialize();
        let mm_height_bytes = self.mm_height.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_SCREEN_SIZE_REQUEST,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            mm_width_bytes[0],
            mm_width_bytes[1],
            mm_width_bytes[2],
            mm_width_bytes[3],
            mm_height_bytes[0],
            mm_height_bytes[1],
            mm_height_bytes[2],
            mm_height_bytes[3],
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
        if header.minor_opcode != SET_SCREEN_SIZE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (mm_width, remaining) = u32::try_parse(remaining)?;
        let (mm_height, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(SetScreenSizeRequest {
            window,
            width,
            height,
            mm_width,
            mm_height,
        })
    }
}
impl Request for SetScreenSizeRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SetScreenSizeRequest {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModeFlag(u32);
impl ModeFlag {
    pub const HSYNC_POSITIVE: Self = Self(1 << 0);
    pub const HSYNC_NEGATIVE: Self = Self(1 << 1);
    pub const VSYNC_POSITIVE: Self = Self(1 << 2);
    pub const VSYNC_NEGATIVE: Self = Self(1 << 3);
    pub const INTERLACE: Self = Self(1 << 4);
    pub const DOUBLE_SCAN: Self = Self(1 << 5);
    pub const CSYNC: Self = Self(1 << 6);
    pub const CSYNC_POSITIVE: Self = Self(1 << 7);
    pub const CSYNC_NEGATIVE: Self = Self(1 << 8);
    pub const HSKEW_PRESENT: Self = Self(1 << 9);
    pub const BCAST: Self = Self(1 << 10);
    pub const PIXEL_MULTIPLEX: Self = Self(1 << 11);
    pub const DOUBLE_CLOCK: Self = Self(1 << 12);
    pub const HALVE_CLOCK: Self = Self(1 << 13);
}
impl From<ModeFlag> for u32 {
    #[inline]
    fn from(input: ModeFlag) -> Self {
        input.0
    }
}
impl From<ModeFlag> for Option<u32> {
    #[inline]
    fn from(input: ModeFlag) -> Self {
        Some(input.0)
    }
}
impl From<u8> for ModeFlag {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for ModeFlag {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for ModeFlag {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ModeFlag  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::HSYNC_POSITIVE.0, "HSYNC_POSITIVE", "HsyncPositive"),
            (Self::HSYNC_NEGATIVE.0, "HSYNC_NEGATIVE", "HsyncNegative"),
            (Self::VSYNC_POSITIVE.0, "VSYNC_POSITIVE", "VsyncPositive"),
            (Self::VSYNC_NEGATIVE.0, "VSYNC_NEGATIVE", "VsyncNegative"),
            (Self::INTERLACE.0, "INTERLACE", "Interlace"),
            (Self::DOUBLE_SCAN.0, "DOUBLE_SCAN", "DoubleScan"),
            (Self::CSYNC.0, "CSYNC", "Csync"),
            (Self::CSYNC_POSITIVE.0, "CSYNC_POSITIVE", "CsyncPositive"),
            (Self::CSYNC_NEGATIVE.0, "CSYNC_NEGATIVE", "CsyncNegative"),
            (Self::HSKEW_PRESENT.0, "HSKEW_PRESENT", "HskewPresent"),
            (Self::BCAST.0, "BCAST", "Bcast"),
            (Self::PIXEL_MULTIPLEX.0, "PIXEL_MULTIPLEX", "PixelMultiplex"),
            (Self::DOUBLE_CLOCK.0, "DOUBLE_CLOCK", "DoubleClock"),
            (Self::HALVE_CLOCK.0, "HALVE_CLOCK", "HalveClock"),
        ];
        pretty_print_bitmask(fmt, self.0, &variants)
    }
}
bitmask_binop!(ModeFlag, u32);

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModeInfo {
    pub id: u32,
    pub width: u16,
    pub height: u16,
    pub dot_clock: u32,
    pub hsync_start: u16,
    pub hsync_end: u16,
    pub htotal: u16,
    pub hskew: u16,
    pub vsync_start: u16,
    pub vsync_end: u16,
    pub vtotal: u16,
    pub name_len: u16,
    pub mode_flags: ModeFlag,
}
impl_debug_if_no_extra_traits!(ModeInfo, "ModeInfo");
impl TryParse for ModeInfo {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (id, remaining) = u32::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (dot_clock, remaining) = u32::try_parse(remaining)?;
        let (hsync_start, remaining) = u16::try_parse(remaining)?;
        let (hsync_end, remaining) = u16::try_parse(remaining)?;
        let (htotal, remaining) = u16::try_parse(remaining)?;
        let (hskew, remaining) = u16::try_parse(remaining)?;
        let (vsync_start, remaining) = u16::try_parse(remaining)?;
        let (vsync_end, remaining) = u16::try_parse(remaining)?;
        let (vtotal, remaining) = u16::try_parse(remaining)?;
        let (name_len, remaining) = u16::try_parse(remaining)?;
        let (mode_flags, remaining) = u32::try_parse(remaining)?;
        let mode_flags = mode_flags.into();
        let result = ModeInfo { id, width, height, dot_clock, hsync_start, hsync_end, htotal, hskew, vsync_start, vsync_end, vtotal, name_len, mode_flags };
        Ok((result, remaining))
    }
}
impl Serialize for ModeInfo {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let id_bytes = self.id.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let dot_clock_bytes = self.dot_clock.serialize();
        let hsync_start_bytes = self.hsync_start.serialize();
        let hsync_end_bytes = self.hsync_end.serialize();
        let htotal_bytes = self.htotal.serialize();
        let hskew_bytes = self.hskew.serialize();
        let vsync_start_bytes = self.vsync_start.serialize();
        let vsync_end_bytes = self.vsync_end.serialize();
        let vtotal_bytes = self.vtotal.serialize();
        let name_len_bytes = self.name_len.serialize();
        let mode_flags_bytes = u32::from(self.mode_flags).serialize();
        [
            id_bytes[0],
            id_bytes[1],
            id_bytes[2],
            id_bytes[3],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            dot_clock_bytes[0],
            dot_clock_bytes[1],
            dot_clock_bytes[2],
            dot_clock_bytes[3],
            hsync_start_bytes[0],
            hsync_start_bytes[1],
            hsync_end_bytes[0],
            hsync_end_bytes[1],
            htotal_bytes[0],
            htotal_bytes[1],
            hskew_bytes[0],
            hskew_bytes[1],
            vsync_start_bytes[0],
            vsync_start_bytes[1],
            vsync_end_bytes[0],
            vsync_end_bytes[1],
            vtotal_bytes[0],
            vtotal_bytes[1],
            name_len_bytes[0],
            name_len_bytes[1],
            mode_flags_bytes[0],
            mode_flags_bytes[1],
            mode_flags_bytes[2],
            mode_flags_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.id.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        self.dot_clock.serialize_into(bytes);
        self.hsync_start.serialize_into(bytes);
        self.hsync_end.serialize_into(bytes);
        self.htotal.serialize_into(bytes);
        self.hskew.serialize_into(bytes);
        self.vsync_start.serialize_into(bytes);
        self.vsync_end.serialize_into(bytes);
        self.vtotal.serialize_into(bytes);
        self.name_len.serialize_into(bytes);
        u32::from(self.mode_flags).serialize_into(bytes);
    }
}

/// Opcode for the GetScreenResources request
pub const GET_SCREEN_RESOURCES_REQUEST: u8 = 8;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetScreenResourcesRequest {
    pub window: xproto::Window,
}
impl_debug_if_no_extra_traits!(GetScreenResourcesRequest, "GetScreenResourcesRequest");
impl GetScreenResourcesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_SCREEN_RESOURCES_REQUEST,
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
        if header.minor_opcode != GET_SCREEN_RESOURCES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let _ = remaining;
        Ok(GetScreenResourcesRequest {
            window,
        })
    }
}
impl Request for GetScreenResourcesRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetScreenResourcesRequest {
    type Reply = GetScreenResourcesReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetScreenResourcesReply {
    pub sequence: u16,
    pub length: u32,
    pub timestamp: xproto::Timestamp,
    pub config_timestamp: xproto::Timestamp,
    pub crtcs: Vec<Crtc>,
    pub outputs: Vec<Output>,
    pub modes: Vec<ModeInfo>,
    pub names: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetScreenResourcesReply, "GetScreenResourcesReply");
impl TryParse for GetScreenResourcesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (config_timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (num_crtcs, remaining) = u16::try_parse(remaining)?;
        let (num_outputs, remaining) = u16::try_parse(remaining)?;
        let (num_modes, remaining) = u16::try_parse(remaining)?;
        let (names_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(8..).ok_or(ParseError::InsufficientData)?;
        let (crtcs, remaining) = crate::x11_utils::parse_list::<Crtc>(remaining, num_crtcs.try_to_usize()?)?;
        let (outputs, remaining) = crate::x11_utils::parse_list::<Output>(remaining, num_outputs.try_to_usize()?)?;
        let (modes, remaining) = crate::x11_utils::parse_list::<ModeInfo>(remaining, num_modes.try_to_usize()?)?;
        let (names, remaining) = crate::x11_utils::parse_u8_list(remaining, names_len.try_to_usize()?)?;
        let names = names.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetScreenResourcesReply { sequence, length, timestamp, config_timestamp, crtcs, outputs, modes, names };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetScreenResourcesReply {
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
        self.timestamp.serialize_into(bytes);
        self.config_timestamp.serialize_into(bytes);
        let num_crtcs = u16::try_from(self.crtcs.len()).expect("`crtcs` has too many elements");
        num_crtcs.serialize_into(bytes);
        let num_outputs = u16::try_from(self.outputs.len()).expect("`outputs` has too many elements");
        num_outputs.serialize_into(bytes);
        let num_modes = u16::try_from(self.modes.len()).expect("`modes` has too many elements");
        num_modes.serialize_into(bytes);
        let names_len = u16::try_from(self.names.len()).expect("`names` has too many elements");
        names_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 8]);
        self.crtcs.serialize_into(bytes);
        self.outputs.serialize_into(bytes);
        self.modes.serialize_into(bytes);
        bytes.extend_from_slice(&self.names);
    }
}
impl GetScreenResourcesReply {
    /// Get the value of the `num_crtcs` field.
    ///
    /// The `num_crtcs` field is used as the length field of the `crtcs` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_crtcs(&self) -> u16 {
        self.crtcs.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_outputs` field.
    ///
    /// The `num_outputs` field is used as the length field of the `outputs` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_outputs(&self) -> u16 {
        self.outputs.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_modes` field.
    ///
    /// The `num_modes` field is used as the length field of the `modes` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_modes(&self) -> u16 {
        self.modes.len()
            .try_into().unwrap()
    }
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

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Connection(u8);
impl Connection {
    pub const CONNECTED: Self = Self(0);
    pub const DISCONNECTED: Self = Self(1);
    pub const UNKNOWN: Self = Self(2);
}
impl From<Connection> for u8 {
    #[inline]
    fn from(input: Connection) -> Self {
        input.0
    }
}
impl From<Connection> for Option<u8> {
    #[inline]
    fn from(input: Connection) -> Self {
        Some(input.0)
    }
}
impl From<Connection> for u16 {
    #[inline]
    fn from(input: Connection) -> Self {
        u16::from(input.0)
    }
}
impl From<Connection> for Option<u16> {
    #[inline]
    fn from(input: Connection) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Connection> for u32 {
    #[inline]
    fn from(input: Connection) -> Self {
        u32::from(input.0)
    }
}
impl From<Connection> for Option<u32> {
    #[inline]
    fn from(input: Connection) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Connection {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Connection  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::CONNECTED.0.into(), "CONNECTED", "Connected"),
            (Self::DISCONNECTED.0.into(), "DISCONNECTED", "Disconnected"),
            (Self::UNKNOWN.0.into(), "UNKNOWN", "Unknown"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the GetOutputInfo request
pub const GET_OUTPUT_INFO_REQUEST: u8 = 9;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetOutputInfoRequest {
    pub output: Output,
    pub config_timestamp: xproto::Timestamp,
}
impl_debug_if_no_extra_traits!(GetOutputInfoRequest, "GetOutputInfoRequest");
impl GetOutputInfoRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let output_bytes = self.output.serialize();
        let config_timestamp_bytes = self.config_timestamp.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_OUTPUT_INFO_REQUEST,
            0,
            0,
            output_bytes[0],
            output_bytes[1],
            output_bytes[2],
            output_bytes[3],
            config_timestamp_bytes[0],
            config_timestamp_bytes[1],
            config_timestamp_bytes[2],
            config_timestamp_bytes[3],
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
        if header.minor_opcode != GET_OUTPUT_INFO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (output, remaining) = Output::try_parse(value)?;
        let (config_timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetOutputInfoRequest {
            output,
            config_timestamp,
        })
    }
}
impl Request for GetOutputInfoRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetOutputInfoRequest {
    type Reply = GetOutputInfoReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetOutputInfoReply {
    pub status: SetConfig,
    pub sequence: u16,
    pub length: u32,
    pub timestamp: xproto::Timestamp,
    pub crtc: Crtc,
    pub mm_width: u32,
    pub mm_height: u32,
    pub connection: Connection,
    pub subpixel_order: render::SubPixel,
    pub num_preferred: u16,
    pub crtcs: Vec<Crtc>,
    pub modes: Vec<Mode>,
    pub clones: Vec<Output>,
    pub name: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetOutputInfoReply, "GetOutputInfoReply");
impl TryParse for GetOutputInfoReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (status, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (crtc, remaining) = Crtc::try_parse(remaining)?;
        let (mm_width, remaining) = u32::try_parse(remaining)?;
        let (mm_height, remaining) = u32::try_parse(remaining)?;
        let (connection, remaining) = u8::try_parse(remaining)?;
        let (subpixel_order, remaining) = u8::try_parse(remaining)?;
        let (num_crtcs, remaining) = u16::try_parse(remaining)?;
        let (num_modes, remaining) = u16::try_parse(remaining)?;
        let (num_preferred, remaining) = u16::try_parse(remaining)?;
        let (num_clones, remaining) = u16::try_parse(remaining)?;
        let (name_len, remaining) = u16::try_parse(remaining)?;
        let (crtcs, remaining) = crate::x11_utils::parse_list::<Crtc>(remaining, num_crtcs.try_to_usize()?)?;
        let (modes, remaining) = crate::x11_utils::parse_list::<Mode>(remaining, num_modes.try_to_usize()?)?;
        let (clones, remaining) = crate::x11_utils::parse_list::<Output>(remaining, num_clones.try_to_usize()?)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, name_len.try_to_usize()?)?;
        let name = name.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let status = status.into();
        let connection = connection.into();
        let subpixel_order = subpixel_order.into();
        let result = GetOutputInfoReply { status, sequence, length, timestamp, crtc, mm_width, mm_height, connection, subpixel_order, num_preferred, crtcs, modes, clones, name };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetOutputInfoReply {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(36);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        u8::from(self.status).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.timestamp.serialize_into(bytes);
        self.crtc.serialize_into(bytes);
        self.mm_width.serialize_into(bytes);
        self.mm_height.serialize_into(bytes);
        u8::from(self.connection).serialize_into(bytes);
        (u32::from(self.subpixel_order) as u8).serialize_into(bytes);
        let num_crtcs = u16::try_from(self.crtcs.len()).expect("`crtcs` has too many elements");
        num_crtcs.serialize_into(bytes);
        let num_modes = u16::try_from(self.modes.len()).expect("`modes` has too many elements");
        num_modes.serialize_into(bytes);
        self.num_preferred.serialize_into(bytes);
        let num_clones = u16::try_from(self.clones.len()).expect("`clones` has too many elements");
        num_clones.serialize_into(bytes);
        let name_len = u16::try_from(self.name.len()).expect("`name` has too many elements");
        name_len.serialize_into(bytes);
        self.crtcs.serialize_into(bytes);
        self.modes.serialize_into(bytes);
        self.clones.serialize_into(bytes);
        bytes.extend_from_slice(&self.name);
    }
}
impl GetOutputInfoReply {
    /// Get the value of the `num_crtcs` field.
    ///
    /// The `num_crtcs` field is used as the length field of the `crtcs` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_crtcs(&self) -> u16 {
        self.crtcs.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_modes` field.
    ///
    /// The `num_modes` field is used as the length field of the `modes` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_modes(&self) -> u16 {
        self.modes.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_clones` field.
    ///
    /// The `num_clones` field is used as the length field of the `clones` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_clones(&self) -> u16 {
        self.clones.len()
            .try_into().unwrap()
    }
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

/// Opcode for the ListOutputProperties request
pub const LIST_OUTPUT_PROPERTIES_REQUEST: u8 = 10;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListOutputPropertiesRequest {
    pub output: Output,
}
impl_debug_if_no_extra_traits!(ListOutputPropertiesRequest, "ListOutputPropertiesRequest");
impl ListOutputPropertiesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let output_bytes = self.output.serialize();
        let mut request0 = vec![
            major_opcode,
            LIST_OUTPUT_PROPERTIES_REQUEST,
            0,
            0,
            output_bytes[0],
            output_bytes[1],
            output_bytes[2],
            output_bytes[3],
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
        if header.minor_opcode != LIST_OUTPUT_PROPERTIES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (output, remaining) = Output::try_parse(value)?;
        let _ = remaining;
        Ok(ListOutputPropertiesRequest {
            output,
        })
    }
}
impl Request for ListOutputPropertiesRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for ListOutputPropertiesRequest {
    type Reply = ListOutputPropertiesReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListOutputPropertiesReply {
    pub sequence: u16,
    pub length: u32,
    pub atoms: Vec<xproto::Atom>,
}
impl_debug_if_no_extra_traits!(ListOutputPropertiesReply, "ListOutputPropertiesReply");
impl TryParse for ListOutputPropertiesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (num_atoms, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
        let (atoms, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, num_atoms.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = ListOutputPropertiesReply { sequence, length, atoms };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ListOutputPropertiesReply {
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
        let num_atoms = u16::try_from(self.atoms.len()).expect("`atoms` has too many elements");
        num_atoms.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
        self.atoms.serialize_into(bytes);
    }
}
impl ListOutputPropertiesReply {
    /// Get the value of the `num_atoms` field.
    ///
    /// The `num_atoms` field is used as the length field of the `atoms` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_atoms(&self) -> u16 {
        self.atoms.len()
            .try_into().unwrap()
    }
}

/// Opcode for the QueryOutputProperty request
pub const QUERY_OUTPUT_PROPERTY_REQUEST: u8 = 11;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryOutputPropertyRequest {
    pub output: Output,
    pub property: xproto::Atom,
}
impl_debug_if_no_extra_traits!(QueryOutputPropertyRequest, "QueryOutputPropertyRequest");
impl QueryOutputPropertyRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let output_bytes = self.output.serialize();
        let property_bytes = self.property.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_OUTPUT_PROPERTY_REQUEST,
            0,
            0,
            output_bytes[0],
            output_bytes[1],
            output_bytes[2],
            output_bytes[3],
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
        if header.minor_opcode != QUERY_OUTPUT_PROPERTY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (output, remaining) = Output::try_parse(value)?;
        let (property, remaining) = xproto::Atom::try_parse(remaining)?;
        let _ = remaining;
        Ok(QueryOutputPropertyRequest {
            output,
            property,
        })
    }
}
impl Request for QueryOutputPropertyRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryOutputPropertyRequest {
    type Reply = QueryOutputPropertyReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryOutputPropertyReply {
    pub sequence: u16,
    pub pending: bool,
    pub range: bool,
    pub immutable: bool,
    pub valid_values: Vec<i32>,
}
impl_debug_if_no_extra_traits!(QueryOutputPropertyReply, "QueryOutputPropertyReply");
impl TryParse for QueryOutputPropertyReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (pending, remaining) = bool::try_parse(remaining)?;
        let (range, remaining) = bool::try_parse(remaining)?;
        let (immutable, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(21..).ok_or(ParseError::InsufficientData)?;
        let (valid_values, remaining) = crate::x11_utils::parse_list::<i32>(remaining, length.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryOutputPropertyReply { sequence, pending, range, immutable, valid_values };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryOutputPropertyReply {
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
        let length = u32::try_from(self.valid_values.len()).expect("`valid_values` has too many elements");
        length.serialize_into(bytes);
        self.pending.serialize_into(bytes);
        self.range.serialize_into(bytes);
        self.immutable.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 21]);
        self.valid_values.serialize_into(bytes);
    }
}
impl QueryOutputPropertyReply {
    /// Get the value of the `length` field.
    ///
    /// The `length` field is used as the length field of the `validValues` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn length(&self) -> u32 {
        self.valid_values.len()
            .try_into().unwrap()
    }
}

/// Opcode for the ConfigureOutputProperty request
pub const CONFIGURE_OUTPUT_PROPERTY_REQUEST: u8 = 12;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConfigureOutputPropertyRequest<'input> {
    pub output: Output,
    pub property: xproto::Atom,
    pub pending: bool,
    pub range: bool,
    pub values: Cow<'input, [i32]>,
}
impl_debug_if_no_extra_traits!(ConfigureOutputPropertyRequest<'_>, "ConfigureOutputPropertyRequest");
impl<'input> ConfigureOutputPropertyRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let output_bytes = self.output.serialize();
        let property_bytes = self.property.serialize();
        let pending_bytes = self.pending.serialize();
        let range_bytes = self.range.serialize();
        let mut request0 = vec![
            major_opcode,
            CONFIGURE_OUTPUT_PROPERTY_REQUEST,
            0,
            0,
            output_bytes[0],
            output_bytes[1],
            output_bytes[2],
            output_bytes[3],
            property_bytes[0],
            property_bytes[1],
            property_bytes[2],
            property_bytes[3],
            pending_bytes[0],
            range_bytes[0],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let values_bytes = self.values.serialize();
        let length_so_far = length_so_far + values_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), values_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != CONFIGURE_OUTPUT_PROPERTY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (output, remaining) = Output::try_parse(value)?;
        let (property, remaining) = xproto::Atom::try_parse(remaining)?;
        let (pending, remaining) = bool::try_parse(remaining)?;
        let (range, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut values = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = i32::try_parse(remaining)?;
            remaining = new_remaining;
            values.push(v);
        }
        let _ = remaining;
        Ok(ConfigureOutputPropertyRequest {
            output,
            property,
            pending,
            range,
            values: Cow::Owned(values),
        })
    }
    /// Clone all borrowed data in this ConfigureOutputPropertyRequest.
    pub fn into_owned(self) -> ConfigureOutputPropertyRequest<'static> {
        ConfigureOutputPropertyRequest {
            output: self.output,
            property: self.property,
            pending: self.pending,
            range: self.range,
            values: Cow::Owned(self.values.into_owned()),
        }
    }
}
impl<'input> Request for ConfigureOutputPropertyRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for ConfigureOutputPropertyRequest<'input> {
}

/// Opcode for the ChangeOutputProperty request
pub const CHANGE_OUTPUT_PROPERTY_REQUEST: u8 = 13;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeOutputPropertyRequest<'input> {
    pub output: Output,
    pub property: xproto::Atom,
    pub type_: xproto::Atom,
    pub format: u8,
    pub mode: xproto::PropMode,
    pub num_units: u32,
    pub data: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(ChangeOutputPropertyRequest<'_>, "ChangeOutputPropertyRequest");
impl<'input> ChangeOutputPropertyRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let output_bytes = self.output.serialize();
        let property_bytes = self.property.serialize();
        let type_bytes = self.type_.serialize();
        let format_bytes = self.format.serialize();
        let mode_bytes = u8::from(self.mode).serialize();
        let num_units_bytes = self.num_units.serialize();
        let mut request0 = vec![
            major_opcode,
            CHANGE_OUTPUT_PROPERTY_REQUEST,
            0,
            0,
            output_bytes[0],
            output_bytes[1],
            output_bytes[2],
            output_bytes[3],
            property_bytes[0],
            property_bytes[1],
            property_bytes[2],
            property_bytes[3],
            type_bytes[0],
            type_bytes[1],
            type_bytes[2],
            type_bytes[3],
            format_bytes[0],
            mode_bytes[0],
            0,
            0,
            num_units_bytes[0],
            num_units_bytes[1],
            num_units_bytes[2],
            num_units_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(self.data.len(), usize::try_from(u32::from(self.num_units).checked_mul(u32::from(self.format)).unwrap().checked_div(8u32).unwrap()).unwrap(), "`data` has an incorrect length");
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
        if header.minor_opcode != CHANGE_OUTPUT_PROPERTY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (output, remaining) = Output::try_parse(value)?;
        let (property, remaining) = xproto::Atom::try_parse(remaining)?;
        let (type_, remaining) = xproto::Atom::try_parse(remaining)?;
        let (format, remaining) = u8::try_parse(remaining)?;
        let (mode, remaining) = u8::try_parse(remaining)?;
        let mode = mode.into();
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (num_units, remaining) = u32::try_parse(remaining)?;
        let (data, remaining) = crate::x11_utils::parse_u8_list(remaining, u32::from(num_units).checked_mul(u32::from(format)).ok_or(ParseError::InvalidExpression)?.checked_div(8u32).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let _ = remaining;
        Ok(ChangeOutputPropertyRequest {
            output,
            property,
            type_,
            format,
            mode,
            num_units,
            data: Cow::Borrowed(data),
        })
    }
    /// Clone all borrowed data in this ChangeOutputPropertyRequest.
    pub fn into_owned(self) -> ChangeOutputPropertyRequest<'static> {
        ChangeOutputPropertyRequest {
            output: self.output,
            property: self.property,
            type_: self.type_,
            format: self.format,
            mode: self.mode,
            num_units: self.num_units,
            data: Cow::Owned(self.data.into_owned()),
        }
    }
}
impl<'input> Request for ChangeOutputPropertyRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for ChangeOutputPropertyRequest<'input> {
}

/// Opcode for the DeleteOutputProperty request
pub const DELETE_OUTPUT_PROPERTY_REQUEST: u8 = 14;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeleteOutputPropertyRequest {
    pub output: Output,
    pub property: xproto::Atom,
}
impl_debug_if_no_extra_traits!(DeleteOutputPropertyRequest, "DeleteOutputPropertyRequest");
impl DeleteOutputPropertyRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let output_bytes = self.output.serialize();
        let property_bytes = self.property.serialize();
        let mut request0 = vec![
            major_opcode,
            DELETE_OUTPUT_PROPERTY_REQUEST,
            0,
            0,
            output_bytes[0],
            output_bytes[1],
            output_bytes[2],
            output_bytes[3],
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
        if header.minor_opcode != DELETE_OUTPUT_PROPERTY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (output, remaining) = Output::try_parse(value)?;
        let (property, remaining) = xproto::Atom::try_parse(remaining)?;
        let _ = remaining;
        Ok(DeleteOutputPropertyRequest {
            output,
            property,
        })
    }
}
impl Request for DeleteOutputPropertyRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DeleteOutputPropertyRequest {
}

/// Opcode for the GetOutputProperty request
pub const GET_OUTPUT_PROPERTY_REQUEST: u8 = 15;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetOutputPropertyRequest {
    pub output: Output,
    pub property: xproto::Atom,
    pub type_: xproto::Atom,
    pub long_offset: u32,
    pub long_length: u32,
    pub delete: bool,
    pub pending: bool,
}
impl_debug_if_no_extra_traits!(GetOutputPropertyRequest, "GetOutputPropertyRequest");
impl GetOutputPropertyRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let output_bytes = self.output.serialize();
        let property_bytes = self.property.serialize();
        let type_bytes = self.type_.serialize();
        let long_offset_bytes = self.long_offset.serialize();
        let long_length_bytes = self.long_length.serialize();
        let delete_bytes = self.delete.serialize();
        let pending_bytes = self.pending.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_OUTPUT_PROPERTY_REQUEST,
            0,
            0,
            output_bytes[0],
            output_bytes[1],
            output_bytes[2],
            output_bytes[3],
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
            delete_bytes[0],
            pending_bytes[0],
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
        if header.minor_opcode != GET_OUTPUT_PROPERTY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (output, remaining) = Output::try_parse(value)?;
        let (property, remaining) = xproto::Atom::try_parse(remaining)?;
        let (type_, remaining) = xproto::Atom::try_parse(remaining)?;
        let (long_offset, remaining) = u32::try_parse(remaining)?;
        let (long_length, remaining) = u32::try_parse(remaining)?;
        let (delete, remaining) = bool::try_parse(remaining)?;
        let (pending, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(GetOutputPropertyRequest {
            output,
            property,
            type_,
            long_offset,
            long_length,
            delete,
            pending,
        })
    }
}
impl Request for GetOutputPropertyRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetOutputPropertyRequest {
    type Reply = GetOutputPropertyReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetOutputPropertyReply {
    pub format: u8,
    pub sequence: u16,
    pub length: u32,
    pub type_: xproto::Atom,
    pub bytes_after: u32,
    pub num_items: u32,
    pub data: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetOutputPropertyReply, "GetOutputPropertyReply");
impl TryParse for GetOutputPropertyReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (format, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (type_, remaining) = xproto::Atom::try_parse(remaining)?;
        let (bytes_after, remaining) = u32::try_parse(remaining)?;
        let (num_items, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(12..).ok_or(ParseError::InsufficientData)?;
        let (data, remaining) = crate::x11_utils::parse_u8_list(remaining, u32::from(num_items).checked_mul(u32::from(format).checked_div(8u32).ok_or(ParseError::InvalidExpression)?).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let data = data.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetOutputPropertyReply { format, sequence, length, type_, bytes_after, num_items, data };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetOutputPropertyReply {
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
        self.num_items.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 12]);
        assert_eq!(self.data.len(), usize::try_from(u32::from(self.num_items).checked_mul(u32::from(self.format).checked_div(8u32).unwrap()).unwrap()).unwrap(), "`data` has an incorrect length");
        bytes.extend_from_slice(&self.data);
    }
}

/// Opcode for the CreateMode request
pub const CREATE_MODE_REQUEST: u8 = 16;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateModeRequest<'input> {
    pub window: xproto::Window,
    pub mode_info: ModeInfo,
    pub name: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(CreateModeRequest<'_>, "CreateModeRequest");
impl<'input> CreateModeRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mode_info_bytes = self.mode_info.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_MODE_REQUEST,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            mode_info_bytes[0],
            mode_info_bytes[1],
            mode_info_bytes[2],
            mode_info_bytes[3],
            mode_info_bytes[4],
            mode_info_bytes[5],
            mode_info_bytes[6],
            mode_info_bytes[7],
            mode_info_bytes[8],
            mode_info_bytes[9],
            mode_info_bytes[10],
            mode_info_bytes[11],
            mode_info_bytes[12],
            mode_info_bytes[13],
            mode_info_bytes[14],
            mode_info_bytes[15],
            mode_info_bytes[16],
            mode_info_bytes[17],
            mode_info_bytes[18],
            mode_info_bytes[19],
            mode_info_bytes[20],
            mode_info_bytes[21],
            mode_info_bytes[22],
            mode_info_bytes[23],
            mode_info_bytes[24],
            mode_info_bytes[25],
            mode_info_bytes[26],
            mode_info_bytes[27],
            mode_info_bytes[28],
            mode_info_bytes[29],
            mode_info_bytes[30],
            mode_info_bytes[31],
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
        if header.minor_opcode != CREATE_MODE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (mode_info, remaining) = ModeInfo::try_parse(remaining)?;
        let (name, remaining) = remaining.split_at(remaining.len());
        let _ = remaining;
        Ok(CreateModeRequest {
            window,
            mode_info,
            name: Cow::Borrowed(name),
        })
    }
    /// Clone all borrowed data in this CreateModeRequest.
    pub fn into_owned(self) -> CreateModeRequest<'static> {
        CreateModeRequest {
            window: self.window,
            mode_info: self.mode_info,
            name: Cow::Owned(self.name.into_owned()),
        }
    }
}
impl<'input> Request for CreateModeRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for CreateModeRequest<'input> {
    type Reply = CreateModeReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateModeReply {
    pub sequence: u16,
    pub length: u32,
    pub mode: Mode,
}
impl_debug_if_no_extra_traits!(CreateModeReply, "CreateModeReply");
impl TryParse for CreateModeReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (mode, remaining) = Mode::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = CreateModeReply { sequence, length, mode };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for CreateModeReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let mode_bytes = self.mode.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            mode_bytes[0],
            mode_bytes[1],
            mode_bytes[2],
            mode_bytes[3],
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
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.mode.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
    }
}

/// Opcode for the DestroyMode request
pub const DESTROY_MODE_REQUEST: u8 = 17;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DestroyModeRequest {
    pub mode: Mode,
}
impl_debug_if_no_extra_traits!(DestroyModeRequest, "DestroyModeRequest");
impl DestroyModeRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mode_bytes = self.mode.serialize();
        let mut request0 = vec![
            major_opcode,
            DESTROY_MODE_REQUEST,
            0,
            0,
            mode_bytes[0],
            mode_bytes[1],
            mode_bytes[2],
            mode_bytes[3],
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
        if header.minor_opcode != DESTROY_MODE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (mode, remaining) = Mode::try_parse(value)?;
        let _ = remaining;
        Ok(DestroyModeRequest {
            mode,
        })
    }
}
impl Request for DestroyModeRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DestroyModeRequest {
}

/// Opcode for the AddOutputMode request
pub const ADD_OUTPUT_MODE_REQUEST: u8 = 18;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AddOutputModeRequest {
    pub output: Output,
    pub mode: Mode,
}
impl_debug_if_no_extra_traits!(AddOutputModeRequest, "AddOutputModeRequest");
impl AddOutputModeRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let output_bytes = self.output.serialize();
        let mode_bytes = self.mode.serialize();
        let mut request0 = vec![
            major_opcode,
            ADD_OUTPUT_MODE_REQUEST,
            0,
            0,
            output_bytes[0],
            output_bytes[1],
            output_bytes[2],
            output_bytes[3],
            mode_bytes[0],
            mode_bytes[1],
            mode_bytes[2],
            mode_bytes[3],
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
        if header.minor_opcode != ADD_OUTPUT_MODE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (output, remaining) = Output::try_parse(value)?;
        let (mode, remaining) = Mode::try_parse(remaining)?;
        let _ = remaining;
        Ok(AddOutputModeRequest {
            output,
            mode,
        })
    }
}
impl Request for AddOutputModeRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for AddOutputModeRequest {
}

/// Opcode for the DeleteOutputMode request
pub const DELETE_OUTPUT_MODE_REQUEST: u8 = 19;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeleteOutputModeRequest {
    pub output: Output,
    pub mode: Mode,
}
impl_debug_if_no_extra_traits!(DeleteOutputModeRequest, "DeleteOutputModeRequest");
impl DeleteOutputModeRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let output_bytes = self.output.serialize();
        let mode_bytes = self.mode.serialize();
        let mut request0 = vec![
            major_opcode,
            DELETE_OUTPUT_MODE_REQUEST,
            0,
            0,
            output_bytes[0],
            output_bytes[1],
            output_bytes[2],
            output_bytes[3],
            mode_bytes[0],
            mode_bytes[1],
            mode_bytes[2],
            mode_bytes[3],
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
        if header.minor_opcode != DELETE_OUTPUT_MODE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (output, remaining) = Output::try_parse(value)?;
        let (mode, remaining) = Mode::try_parse(remaining)?;
        let _ = remaining;
        Ok(DeleteOutputModeRequest {
            output,
            mode,
        })
    }
}
impl Request for DeleteOutputModeRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DeleteOutputModeRequest {
}

/// Opcode for the GetCrtcInfo request
pub const GET_CRTC_INFO_REQUEST: u8 = 20;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetCrtcInfoRequest {
    pub crtc: Crtc,
    pub config_timestamp: xproto::Timestamp,
}
impl_debug_if_no_extra_traits!(GetCrtcInfoRequest, "GetCrtcInfoRequest");
impl GetCrtcInfoRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let crtc_bytes = self.crtc.serialize();
        let config_timestamp_bytes = self.config_timestamp.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_CRTC_INFO_REQUEST,
            0,
            0,
            crtc_bytes[0],
            crtc_bytes[1],
            crtc_bytes[2],
            crtc_bytes[3],
            config_timestamp_bytes[0],
            config_timestamp_bytes[1],
            config_timestamp_bytes[2],
            config_timestamp_bytes[3],
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
        if header.minor_opcode != GET_CRTC_INFO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (crtc, remaining) = Crtc::try_parse(value)?;
        let (config_timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetCrtcInfoRequest {
            crtc,
            config_timestamp,
        })
    }
}
impl Request for GetCrtcInfoRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetCrtcInfoRequest {
    type Reply = GetCrtcInfoReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetCrtcInfoReply {
    pub status: SetConfig,
    pub sequence: u16,
    pub length: u32,
    pub timestamp: xproto::Timestamp,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub mode: Mode,
    pub rotation: Rotation,
    pub rotations: Rotation,
    pub outputs: Vec<Output>,
    pub possible: Vec<Output>,
}
impl_debug_if_no_extra_traits!(GetCrtcInfoReply, "GetCrtcInfoReply");
impl TryParse for GetCrtcInfoReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (status, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (mode, remaining) = Mode::try_parse(remaining)?;
        let (rotation, remaining) = u16::try_parse(remaining)?;
        let (rotations, remaining) = u16::try_parse(remaining)?;
        let (num_outputs, remaining) = u16::try_parse(remaining)?;
        let (num_possible_outputs, remaining) = u16::try_parse(remaining)?;
        let (outputs, remaining) = crate::x11_utils::parse_list::<Output>(remaining, num_outputs.try_to_usize()?)?;
        let (possible, remaining) = crate::x11_utils::parse_list::<Output>(remaining, num_possible_outputs.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let status = status.into();
        let rotation = rotation.into();
        let rotations = rotations.into();
        let result = GetCrtcInfoReply { status, sequence, length, timestamp, x, y, width, height, mode, rotation, rotations, outputs, possible };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetCrtcInfoReply {
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
        u8::from(self.status).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.timestamp.serialize_into(bytes);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        self.mode.serialize_into(bytes);
        u16::from(self.rotation).serialize_into(bytes);
        u16::from(self.rotations).serialize_into(bytes);
        let num_outputs = u16::try_from(self.outputs.len()).expect("`outputs` has too many elements");
        num_outputs.serialize_into(bytes);
        let num_possible_outputs = u16::try_from(self.possible.len()).expect("`possible` has too many elements");
        num_possible_outputs.serialize_into(bytes);
        self.outputs.serialize_into(bytes);
        self.possible.serialize_into(bytes);
    }
}
impl GetCrtcInfoReply {
    /// Get the value of the `num_outputs` field.
    ///
    /// The `num_outputs` field is used as the length field of the `outputs` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_outputs(&self) -> u16 {
        self.outputs.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_possible_outputs` field.
    ///
    /// The `num_possible_outputs` field is used as the length field of the `possible` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_possible_outputs(&self) -> u16 {
        self.possible.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SetCrtcConfig request
pub const SET_CRTC_CONFIG_REQUEST: u8 = 21;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetCrtcConfigRequest<'input> {
    pub crtc: Crtc,
    pub timestamp: xproto::Timestamp,
    pub config_timestamp: xproto::Timestamp,
    pub x: i16,
    pub y: i16,
    pub mode: Mode,
    pub rotation: Rotation,
    pub outputs: Cow<'input, [Output]>,
}
impl_debug_if_no_extra_traits!(SetCrtcConfigRequest<'_>, "SetCrtcConfigRequest");
impl<'input> SetCrtcConfigRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let crtc_bytes = self.crtc.serialize();
        let timestamp_bytes = self.timestamp.serialize();
        let config_timestamp_bytes = self.config_timestamp.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let mode_bytes = self.mode.serialize();
        let rotation_bytes = u16::from(self.rotation).serialize();
        let mut request0 = vec![
            major_opcode,
            SET_CRTC_CONFIG_REQUEST,
            0,
            0,
            crtc_bytes[0],
            crtc_bytes[1],
            crtc_bytes[2],
            crtc_bytes[3],
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            config_timestamp_bytes[0],
            config_timestamp_bytes[1],
            config_timestamp_bytes[2],
            config_timestamp_bytes[3],
            x_bytes[0],
            x_bytes[1],
            y_bytes[0],
            y_bytes[1],
            mode_bytes[0],
            mode_bytes[1],
            mode_bytes[2],
            mode_bytes[3],
            rotation_bytes[0],
            rotation_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let outputs_bytes = self.outputs.serialize();
        let length_so_far = length_so_far + outputs_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), outputs_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_CRTC_CONFIG_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (crtc, remaining) = Crtc::try_parse(value)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (config_timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (mode, remaining) = Mode::try_parse(remaining)?;
        let (rotation, remaining) = u16::try_parse(remaining)?;
        let rotation = rotation.into();
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut outputs = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = Output::try_parse(remaining)?;
            remaining = new_remaining;
            outputs.push(v);
        }
        let _ = remaining;
        Ok(SetCrtcConfigRequest {
            crtc,
            timestamp,
            config_timestamp,
            x,
            y,
            mode,
            rotation,
            outputs: Cow::Owned(outputs),
        })
    }
    /// Clone all borrowed data in this SetCrtcConfigRequest.
    pub fn into_owned(self) -> SetCrtcConfigRequest<'static> {
        SetCrtcConfigRequest {
            crtc: self.crtc,
            timestamp: self.timestamp,
            config_timestamp: self.config_timestamp,
            x: self.x,
            y: self.y,
            mode: self.mode,
            rotation: self.rotation,
            outputs: Cow::Owned(self.outputs.into_owned()),
        }
    }
}
impl<'input> Request for SetCrtcConfigRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for SetCrtcConfigRequest<'input> {
    type Reply = SetCrtcConfigReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetCrtcConfigReply {
    pub status: SetConfig,
    pub sequence: u16,
    pub length: u32,
    pub timestamp: xproto::Timestamp,
}
impl_debug_if_no_extra_traits!(SetCrtcConfigReply, "SetCrtcConfigReply");
impl TryParse for SetCrtcConfigReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (status, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let status = status.into();
        let result = SetCrtcConfigReply { status, sequence, length, timestamp };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for SetCrtcConfigReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let status_bytes = u8::from(self.status).serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let timestamp_bytes = self.timestamp.serialize();
        [
            response_type_bytes[0],
            status_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
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
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        u8::from(self.status).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.timestamp.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
    }
}

/// Opcode for the GetCrtcGammaSize request
pub const GET_CRTC_GAMMA_SIZE_REQUEST: u8 = 22;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetCrtcGammaSizeRequest {
    pub crtc: Crtc,
}
impl_debug_if_no_extra_traits!(GetCrtcGammaSizeRequest, "GetCrtcGammaSizeRequest");
impl GetCrtcGammaSizeRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let crtc_bytes = self.crtc.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_CRTC_GAMMA_SIZE_REQUEST,
            0,
            0,
            crtc_bytes[0],
            crtc_bytes[1],
            crtc_bytes[2],
            crtc_bytes[3],
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
        if header.minor_opcode != GET_CRTC_GAMMA_SIZE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (crtc, remaining) = Crtc::try_parse(value)?;
        let _ = remaining;
        Ok(GetCrtcGammaSizeRequest {
            crtc,
        })
    }
}
impl Request for GetCrtcGammaSizeRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetCrtcGammaSizeRequest {
    type Reply = GetCrtcGammaSizeReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetCrtcGammaSizeReply {
    pub sequence: u16,
    pub length: u32,
    pub size: u16,
}
impl_debug_if_no_extra_traits!(GetCrtcGammaSizeReply, "GetCrtcGammaSizeReply");
impl TryParse for GetCrtcGammaSizeReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (size, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetCrtcGammaSizeReply { sequence, length, size };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetCrtcGammaSizeReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let size_bytes = self.size.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            size_bytes[0],
            size_bytes[1],
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
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.size.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
    }
}

/// Opcode for the GetCrtcGamma request
pub const GET_CRTC_GAMMA_REQUEST: u8 = 23;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetCrtcGammaRequest {
    pub crtc: Crtc,
}
impl_debug_if_no_extra_traits!(GetCrtcGammaRequest, "GetCrtcGammaRequest");
impl GetCrtcGammaRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let crtc_bytes = self.crtc.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_CRTC_GAMMA_REQUEST,
            0,
            0,
            crtc_bytes[0],
            crtc_bytes[1],
            crtc_bytes[2],
            crtc_bytes[3],
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
        if header.minor_opcode != GET_CRTC_GAMMA_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (crtc, remaining) = Crtc::try_parse(value)?;
        let _ = remaining;
        Ok(GetCrtcGammaRequest {
            crtc,
        })
    }
}
impl Request for GetCrtcGammaRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetCrtcGammaRequest {
    type Reply = GetCrtcGammaReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetCrtcGammaReply {
    pub sequence: u16,
    pub length: u32,
    pub red: Vec<u16>,
    pub green: Vec<u16>,
    pub blue: Vec<u16>,
}
impl_debug_if_no_extra_traits!(GetCrtcGammaReply, "GetCrtcGammaReply");
impl TryParse for GetCrtcGammaReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (size, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
        let (red, remaining) = crate::x11_utils::parse_list::<u16>(remaining, size.try_to_usize()?)?;
        let (green, remaining) = crate::x11_utils::parse_list::<u16>(remaining, size.try_to_usize()?)?;
        let (blue, remaining) = crate::x11_utils::parse_list::<u16>(remaining, size.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetCrtcGammaReply { sequence, length, red, green, blue };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetCrtcGammaReply {
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
        let size = u16::try_from(self.red.len()).expect("`red` has too many elements");
        size.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
        self.red.serialize_into(bytes);
        assert_eq!(self.green.len(), usize::try_from(size).unwrap(), "`green` has an incorrect length");
        self.green.serialize_into(bytes);
        assert_eq!(self.blue.len(), usize::try_from(size).unwrap(), "`blue` has an incorrect length");
        self.blue.serialize_into(bytes);
    }
}
impl GetCrtcGammaReply {
    /// Get the value of the `size` field.
    ///
    /// The `size` field is used as the length field of the `red` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn size(&self) -> u16 {
        self.red.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SetCrtcGamma request
pub const SET_CRTC_GAMMA_REQUEST: u8 = 24;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetCrtcGammaRequest<'input> {
    pub crtc: Crtc,
    pub red: Cow<'input, [u16]>,
    pub green: Cow<'input, [u16]>,
    pub blue: Cow<'input, [u16]>,
}
impl_debug_if_no_extra_traits!(SetCrtcGammaRequest<'_>, "SetCrtcGammaRequest");
impl<'input> SetCrtcGammaRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 5]> {
        let length_so_far = 0;
        let crtc_bytes = self.crtc.serialize();
        let size = u16::try_from(self.red.len()).expect("`red` has too many elements");
        let size_bytes = size.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_CRTC_GAMMA_REQUEST,
            0,
            0,
            crtc_bytes[0],
            crtc_bytes[1],
            crtc_bytes[2],
            crtc_bytes[3],
            size_bytes[0],
            size_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let red_bytes = self.red.serialize();
        let length_so_far = length_so_far + red_bytes.len();
        assert_eq!(self.green.len(), usize::try_from(size).unwrap(), "`green` has an incorrect length");
        let green_bytes = self.green.serialize();
        let length_so_far = length_so_far + green_bytes.len();
        assert_eq!(self.blue.len(), usize::try_from(size).unwrap(), "`blue` has an incorrect length");
        let blue_bytes = self.blue.serialize();
        let length_so_far = length_so_far + blue_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), red_bytes.into(), green_bytes.into(), blue_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_CRTC_GAMMA_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (crtc, remaining) = Crtc::try_parse(value)?;
        let (size, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (red, remaining) = crate::x11_utils::parse_list::<u16>(remaining, size.try_to_usize()?)?;
        let (green, remaining) = crate::x11_utils::parse_list::<u16>(remaining, size.try_to_usize()?)?;
        let (blue, remaining) = crate::x11_utils::parse_list::<u16>(remaining, size.try_to_usize()?)?;
        let _ = remaining;
        Ok(SetCrtcGammaRequest {
            crtc,
            red: Cow::Owned(red),
            green: Cow::Owned(green),
            blue: Cow::Owned(blue),
        })
    }
    /// Clone all borrowed data in this SetCrtcGammaRequest.
    pub fn into_owned(self) -> SetCrtcGammaRequest<'static> {
        SetCrtcGammaRequest {
            crtc: self.crtc,
            red: Cow::Owned(self.red.into_owned()),
            green: Cow::Owned(self.green.into_owned()),
            blue: Cow::Owned(self.blue.into_owned()),
        }
    }
}
impl<'input> Request for SetCrtcGammaRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetCrtcGammaRequest<'input> {
}

/// Opcode for the GetScreenResourcesCurrent request
pub const GET_SCREEN_RESOURCES_CURRENT_REQUEST: u8 = 25;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetScreenResourcesCurrentRequest {
    pub window: xproto::Window,
}
impl_debug_if_no_extra_traits!(GetScreenResourcesCurrentRequest, "GetScreenResourcesCurrentRequest");
impl GetScreenResourcesCurrentRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_SCREEN_RESOURCES_CURRENT_REQUEST,
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
        if header.minor_opcode != GET_SCREEN_RESOURCES_CURRENT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let _ = remaining;
        Ok(GetScreenResourcesCurrentRequest {
            window,
        })
    }
}
impl Request for GetScreenResourcesCurrentRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetScreenResourcesCurrentRequest {
    type Reply = GetScreenResourcesCurrentReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetScreenResourcesCurrentReply {
    pub sequence: u16,
    pub length: u32,
    pub timestamp: xproto::Timestamp,
    pub config_timestamp: xproto::Timestamp,
    pub crtcs: Vec<Crtc>,
    pub outputs: Vec<Output>,
    pub modes: Vec<ModeInfo>,
    pub names: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetScreenResourcesCurrentReply, "GetScreenResourcesCurrentReply");
impl TryParse for GetScreenResourcesCurrentReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (config_timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (num_crtcs, remaining) = u16::try_parse(remaining)?;
        let (num_outputs, remaining) = u16::try_parse(remaining)?;
        let (num_modes, remaining) = u16::try_parse(remaining)?;
        let (names_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(8..).ok_or(ParseError::InsufficientData)?;
        let (crtcs, remaining) = crate::x11_utils::parse_list::<Crtc>(remaining, num_crtcs.try_to_usize()?)?;
        let (outputs, remaining) = crate::x11_utils::parse_list::<Output>(remaining, num_outputs.try_to_usize()?)?;
        let (modes, remaining) = crate::x11_utils::parse_list::<ModeInfo>(remaining, num_modes.try_to_usize()?)?;
        let (names, remaining) = crate::x11_utils::parse_u8_list(remaining, names_len.try_to_usize()?)?;
        let names = names.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetScreenResourcesCurrentReply { sequence, length, timestamp, config_timestamp, crtcs, outputs, modes, names };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetScreenResourcesCurrentReply {
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
        self.timestamp.serialize_into(bytes);
        self.config_timestamp.serialize_into(bytes);
        let num_crtcs = u16::try_from(self.crtcs.len()).expect("`crtcs` has too many elements");
        num_crtcs.serialize_into(bytes);
        let num_outputs = u16::try_from(self.outputs.len()).expect("`outputs` has too many elements");
        num_outputs.serialize_into(bytes);
        let num_modes = u16::try_from(self.modes.len()).expect("`modes` has too many elements");
        num_modes.serialize_into(bytes);
        let names_len = u16::try_from(self.names.len()).expect("`names` has too many elements");
        names_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 8]);
        self.crtcs.serialize_into(bytes);
        self.outputs.serialize_into(bytes);
        self.modes.serialize_into(bytes);
        bytes.extend_from_slice(&self.names);
    }
}
impl GetScreenResourcesCurrentReply {
    /// Get the value of the `num_crtcs` field.
    ///
    /// The `num_crtcs` field is used as the length field of the `crtcs` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_crtcs(&self) -> u16 {
        self.crtcs.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_outputs` field.
    ///
    /// The `num_outputs` field is used as the length field of the `outputs` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_outputs(&self) -> u16 {
        self.outputs.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_modes` field.
    ///
    /// The `num_modes` field is used as the length field of the `modes` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_modes(&self) -> u16 {
        self.modes.len()
            .try_into().unwrap()
    }
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

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Transform(u8);
impl Transform {
    pub const UNIT: Self = Self(1 << 0);
    pub const SCALE_UP: Self = Self(1 << 1);
    pub const SCALE_DOWN: Self = Self(1 << 2);
    pub const PROJECTIVE: Self = Self(1 << 3);
}
impl From<Transform> for u8 {
    #[inline]
    fn from(input: Transform) -> Self {
        input.0
    }
}
impl From<Transform> for Option<u8> {
    #[inline]
    fn from(input: Transform) -> Self {
        Some(input.0)
    }
}
impl From<Transform> for u16 {
    #[inline]
    fn from(input: Transform) -> Self {
        u16::from(input.0)
    }
}
impl From<Transform> for Option<u16> {
    #[inline]
    fn from(input: Transform) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Transform> for u32 {
    #[inline]
    fn from(input: Transform) -> Self {
        u32::from(input.0)
    }
}
impl From<Transform> for Option<u32> {
    #[inline]
    fn from(input: Transform) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Transform {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Transform  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::UNIT.0.into(), "UNIT", "Unit"),
            (Self::SCALE_UP.0.into(), "SCALE_UP", "ScaleUp"),
            (Self::SCALE_DOWN.0.into(), "SCALE_DOWN", "ScaleDown"),
            (Self::PROJECTIVE.0.into(), "PROJECTIVE", "Projective"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(Transform, u8);

/// Opcode for the SetCrtcTransform request
pub const SET_CRTC_TRANSFORM_REQUEST: u8 = 26;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetCrtcTransformRequest<'input> {
    pub crtc: Crtc,
    pub transform: render::Transform,
    pub filter_name: Cow<'input, [u8]>,
    pub filter_params: Cow<'input, [render::Fixed]>,
}
impl_debug_if_no_extra_traits!(SetCrtcTransformRequest<'_>, "SetCrtcTransformRequest");
impl<'input> SetCrtcTransformRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 5]> {
        let length_so_far = 0;
        let crtc_bytes = self.crtc.serialize();
        let transform_bytes = self.transform.serialize();
        let filter_len = u16::try_from(self.filter_name.len()).expect("`filter_name` has too many elements");
        let filter_len_bytes = filter_len.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_CRTC_TRANSFORM_REQUEST,
            0,
            0,
            crtc_bytes[0],
            crtc_bytes[1],
            crtc_bytes[2],
            crtc_bytes[3],
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
            filter_len_bytes[0],
            filter_len_bytes[1],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.filter_name.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        let filter_params_bytes = self.filter_params.serialize();
        let length_so_far = length_so_far + filter_params_bytes.len();
        let padding1 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding1.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.filter_name, padding0.into(), filter_params_bytes.into(), padding1.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_CRTC_TRANSFORM_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (crtc, remaining) = Crtc::try_parse(value)?;
        let (transform, remaining) = render::Transform::try_parse(remaining)?;
        let (filter_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (filter_name, remaining) = crate::x11_utils::parse_u8_list(remaining, filter_len.try_to_usize()?)?;
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut filter_params = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = render::Fixed::try_parse(remaining)?;
            remaining = new_remaining;
            filter_params.push(v);
        }
        let _ = remaining;
        Ok(SetCrtcTransformRequest {
            crtc,
            transform,
            filter_name: Cow::Borrowed(filter_name),
            filter_params: Cow::Owned(filter_params),
        })
    }
    /// Clone all borrowed data in this SetCrtcTransformRequest.
    pub fn into_owned(self) -> SetCrtcTransformRequest<'static> {
        SetCrtcTransformRequest {
            crtc: self.crtc,
            transform: self.transform,
            filter_name: Cow::Owned(self.filter_name.into_owned()),
            filter_params: Cow::Owned(self.filter_params.into_owned()),
        }
    }
}
impl<'input> Request for SetCrtcTransformRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetCrtcTransformRequest<'input> {
}

/// Opcode for the GetCrtcTransform request
pub const GET_CRTC_TRANSFORM_REQUEST: u8 = 27;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetCrtcTransformRequest {
    pub crtc: Crtc,
}
impl_debug_if_no_extra_traits!(GetCrtcTransformRequest, "GetCrtcTransformRequest");
impl GetCrtcTransformRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let crtc_bytes = self.crtc.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_CRTC_TRANSFORM_REQUEST,
            0,
            0,
            crtc_bytes[0],
            crtc_bytes[1],
            crtc_bytes[2],
            crtc_bytes[3],
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
        if header.minor_opcode != GET_CRTC_TRANSFORM_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (crtc, remaining) = Crtc::try_parse(value)?;
        let _ = remaining;
        Ok(GetCrtcTransformRequest {
            crtc,
        })
    }
}
impl Request for GetCrtcTransformRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetCrtcTransformRequest {
    type Reply = GetCrtcTransformReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetCrtcTransformReply {
    pub sequence: u16,
    pub length: u32,
    pub pending_transform: render::Transform,
    pub has_transforms: bool,
    pub current_transform: render::Transform,
    pub pending_filter_name: Vec<u8>,
    pub pending_params: Vec<render::Fixed>,
    pub current_filter_name: Vec<u8>,
    pub current_params: Vec<render::Fixed>,
}
impl_debug_if_no_extra_traits!(GetCrtcTransformReply, "GetCrtcTransformReply");
impl TryParse for GetCrtcTransformReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let value = remaining;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (pending_transform, remaining) = render::Transform::try_parse(remaining)?;
        let (has_transforms, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (current_transform, remaining) = render::Transform::try_parse(remaining)?;
        let remaining = remaining.get(4..).ok_or(ParseError::InsufficientData)?;
        let (pending_len, remaining) = u16::try_parse(remaining)?;
        let (pending_nparams, remaining) = u16::try_parse(remaining)?;
        let (current_len, remaining) = u16::try_parse(remaining)?;
        let (current_nparams, remaining) = u16::try_parse(remaining)?;
        let (pending_filter_name, remaining) = crate::x11_utils::parse_u8_list(remaining, pending_len.try_to_usize()?)?;
        let pending_filter_name = pending_filter_name.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let (pending_params, remaining) = crate::x11_utils::parse_list::<render::Fixed>(remaining, pending_nparams.try_to_usize()?)?;
        let (current_filter_name, remaining) = crate::x11_utils::parse_u8_list(remaining, current_len.try_to_usize()?)?;
        let current_filter_name = current_filter_name.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let (current_params, remaining) = crate::x11_utils::parse_list::<render::Fixed>(remaining, current_nparams.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetCrtcTransformReply { sequence, length, pending_transform, has_transforms, current_transform, pending_filter_name, pending_params, current_filter_name, current_params };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetCrtcTransformReply {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(96);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.pending_transform.serialize_into(bytes);
        self.has_transforms.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
        self.current_transform.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 4]);
        let pending_len = u16::try_from(self.pending_filter_name.len()).expect("`pending_filter_name` has too many elements");
        pending_len.serialize_into(bytes);
        let pending_nparams = u16::try_from(self.pending_params.len()).expect("`pending_params` has too many elements");
        pending_nparams.serialize_into(bytes);
        let current_len = u16::try_from(self.current_filter_name.len()).expect("`current_filter_name` has too many elements");
        current_len.serialize_into(bytes);
        let current_nparams = u16::try_from(self.current_params.len()).expect("`current_params` has too many elements");
        current_nparams.serialize_into(bytes);
        bytes.extend_from_slice(&self.pending_filter_name);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        self.pending_params.serialize_into(bytes);
        bytes.extend_from_slice(&self.current_filter_name);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        self.current_params.serialize_into(bytes);
    }
}
impl GetCrtcTransformReply {
    /// Get the value of the `pending_len` field.
    ///
    /// The `pending_len` field is used as the length field of the `pending_filter_name` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn pending_len(&self) -> u16 {
        self.pending_filter_name.len()
            .try_into().unwrap()
    }
    /// Get the value of the `pending_nparams` field.
    ///
    /// The `pending_nparams` field is used as the length field of the `pending_params` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn pending_nparams(&self) -> u16 {
        self.pending_params.len()
            .try_into().unwrap()
    }
    /// Get the value of the `current_len` field.
    ///
    /// The `current_len` field is used as the length field of the `current_filter_name` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn current_len(&self) -> u16 {
        self.current_filter_name.len()
            .try_into().unwrap()
    }
    /// Get the value of the `current_nparams` field.
    ///
    /// The `current_nparams` field is used as the length field of the `current_params` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn current_nparams(&self) -> u16 {
        self.current_params.len()
            .try_into().unwrap()
    }
}

/// Opcode for the GetPanning request
pub const GET_PANNING_REQUEST: u8 = 28;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPanningRequest {
    pub crtc: Crtc,
}
impl_debug_if_no_extra_traits!(GetPanningRequest, "GetPanningRequest");
impl GetPanningRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let crtc_bytes = self.crtc.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_PANNING_REQUEST,
            0,
            0,
            crtc_bytes[0],
            crtc_bytes[1],
            crtc_bytes[2],
            crtc_bytes[3],
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
        if header.minor_opcode != GET_PANNING_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (crtc, remaining) = Crtc::try_parse(value)?;
        let _ = remaining;
        Ok(GetPanningRequest {
            crtc,
        })
    }
}
impl Request for GetPanningRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetPanningRequest {
    type Reply = GetPanningReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPanningReply {
    pub status: SetConfig,
    pub sequence: u16,
    pub length: u32,
    pub timestamp: xproto::Timestamp,
    pub left: u16,
    pub top: u16,
    pub width: u16,
    pub height: u16,
    pub track_left: u16,
    pub track_top: u16,
    pub track_width: u16,
    pub track_height: u16,
    pub border_left: i16,
    pub border_top: i16,
    pub border_right: i16,
    pub border_bottom: i16,
}
impl_debug_if_no_extra_traits!(GetPanningReply, "GetPanningReply");
impl TryParse for GetPanningReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (status, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (left, remaining) = u16::try_parse(remaining)?;
        let (top, remaining) = u16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (track_left, remaining) = u16::try_parse(remaining)?;
        let (track_top, remaining) = u16::try_parse(remaining)?;
        let (track_width, remaining) = u16::try_parse(remaining)?;
        let (track_height, remaining) = u16::try_parse(remaining)?;
        let (border_left, remaining) = i16::try_parse(remaining)?;
        let (border_top, remaining) = i16::try_parse(remaining)?;
        let (border_right, remaining) = i16::try_parse(remaining)?;
        let (border_bottom, remaining) = i16::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let status = status.into();
        let result = GetPanningReply { status, sequence, length, timestamp, left, top, width, height, track_left, track_top, track_width, track_height, border_left, border_top, border_right, border_bottom };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetPanningReply {
    type Bytes = [u8; 36];
    fn serialize(&self) -> [u8; 36] {
        let response_type_bytes = &[1];
        let status_bytes = u8::from(self.status).serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let timestamp_bytes = self.timestamp.serialize();
        let left_bytes = self.left.serialize();
        let top_bytes = self.top.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let track_left_bytes = self.track_left.serialize();
        let track_top_bytes = self.track_top.serialize();
        let track_width_bytes = self.track_width.serialize();
        let track_height_bytes = self.track_height.serialize();
        let border_left_bytes = self.border_left.serialize();
        let border_top_bytes = self.border_top.serialize();
        let border_right_bytes = self.border_right.serialize();
        let border_bottom_bytes = self.border_bottom.serialize();
        [
            response_type_bytes[0],
            status_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            left_bytes[0],
            left_bytes[1],
            top_bytes[0],
            top_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            track_left_bytes[0],
            track_left_bytes[1],
            track_top_bytes[0],
            track_top_bytes[1],
            track_width_bytes[0],
            track_width_bytes[1],
            track_height_bytes[0],
            track_height_bytes[1],
            border_left_bytes[0],
            border_left_bytes[1],
            border_top_bytes[0],
            border_top_bytes[1],
            border_right_bytes[0],
            border_right_bytes[1],
            border_bottom_bytes[0],
            border_bottom_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(36);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        u8::from(self.status).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.timestamp.serialize_into(bytes);
        self.left.serialize_into(bytes);
        self.top.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        self.track_left.serialize_into(bytes);
        self.track_top.serialize_into(bytes);
        self.track_width.serialize_into(bytes);
        self.track_height.serialize_into(bytes);
        self.border_left.serialize_into(bytes);
        self.border_top.serialize_into(bytes);
        self.border_right.serialize_into(bytes);
        self.border_bottom.serialize_into(bytes);
    }
}

/// Opcode for the SetPanning request
pub const SET_PANNING_REQUEST: u8 = 29;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetPanningRequest {
    pub crtc: Crtc,
    pub timestamp: xproto::Timestamp,
    pub left: u16,
    pub top: u16,
    pub width: u16,
    pub height: u16,
    pub track_left: u16,
    pub track_top: u16,
    pub track_width: u16,
    pub track_height: u16,
    pub border_left: i16,
    pub border_top: i16,
    pub border_right: i16,
    pub border_bottom: i16,
}
impl_debug_if_no_extra_traits!(SetPanningRequest, "SetPanningRequest");
impl SetPanningRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let crtc_bytes = self.crtc.serialize();
        let timestamp_bytes = self.timestamp.serialize();
        let left_bytes = self.left.serialize();
        let top_bytes = self.top.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let track_left_bytes = self.track_left.serialize();
        let track_top_bytes = self.track_top.serialize();
        let track_width_bytes = self.track_width.serialize();
        let track_height_bytes = self.track_height.serialize();
        let border_left_bytes = self.border_left.serialize();
        let border_top_bytes = self.border_top.serialize();
        let border_right_bytes = self.border_right.serialize();
        let border_bottom_bytes = self.border_bottom.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_PANNING_REQUEST,
            0,
            0,
            crtc_bytes[0],
            crtc_bytes[1],
            crtc_bytes[2],
            crtc_bytes[3],
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            left_bytes[0],
            left_bytes[1],
            top_bytes[0],
            top_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            track_left_bytes[0],
            track_left_bytes[1],
            track_top_bytes[0],
            track_top_bytes[1],
            track_width_bytes[0],
            track_width_bytes[1],
            track_height_bytes[0],
            track_height_bytes[1],
            border_left_bytes[0],
            border_left_bytes[1],
            border_top_bytes[0],
            border_top_bytes[1],
            border_right_bytes[0],
            border_right_bytes[1],
            border_bottom_bytes[0],
            border_bottom_bytes[1],
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
        if header.minor_opcode != SET_PANNING_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (crtc, remaining) = Crtc::try_parse(value)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (left, remaining) = u16::try_parse(remaining)?;
        let (top, remaining) = u16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (track_left, remaining) = u16::try_parse(remaining)?;
        let (track_top, remaining) = u16::try_parse(remaining)?;
        let (track_width, remaining) = u16::try_parse(remaining)?;
        let (track_height, remaining) = u16::try_parse(remaining)?;
        let (border_left, remaining) = i16::try_parse(remaining)?;
        let (border_top, remaining) = i16::try_parse(remaining)?;
        let (border_right, remaining) = i16::try_parse(remaining)?;
        let (border_bottom, remaining) = i16::try_parse(remaining)?;
        let _ = remaining;
        Ok(SetPanningRequest {
            crtc,
            timestamp,
            left,
            top,
            width,
            height,
            track_left,
            track_top,
            track_width,
            track_height,
            border_left,
            border_top,
            border_right,
            border_bottom,
        })
    }
}
impl Request for SetPanningRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for SetPanningRequest {
    type Reply = SetPanningReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetPanningReply {
    pub status: SetConfig,
    pub sequence: u16,
    pub length: u32,
    pub timestamp: xproto::Timestamp,
}
impl_debug_if_no_extra_traits!(SetPanningReply, "SetPanningReply");
impl TryParse for SetPanningReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (status, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let status = status.into();
        let result = SetPanningReply { status, sequence, length, timestamp };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for SetPanningReply {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = &[1];
        let status_bytes = u8::from(self.status).serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let timestamp_bytes = self.timestamp.serialize();
        [
            response_type_bytes[0],
            status_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        u8::from(self.status).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.timestamp.serialize_into(bytes);
    }
}

/// Opcode for the SetOutputPrimary request
pub const SET_OUTPUT_PRIMARY_REQUEST: u8 = 30;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetOutputPrimaryRequest {
    pub window: xproto::Window,
    pub output: Output,
}
impl_debug_if_no_extra_traits!(SetOutputPrimaryRequest, "SetOutputPrimaryRequest");
impl SetOutputPrimaryRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let output_bytes = self.output.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_OUTPUT_PRIMARY_REQUEST,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            output_bytes[0],
            output_bytes[1],
            output_bytes[2],
            output_bytes[3],
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
        if header.minor_opcode != SET_OUTPUT_PRIMARY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (output, remaining) = Output::try_parse(remaining)?;
        let _ = remaining;
        Ok(SetOutputPrimaryRequest {
            window,
            output,
        })
    }
}
impl Request for SetOutputPrimaryRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SetOutputPrimaryRequest {
}

/// Opcode for the GetOutputPrimary request
pub const GET_OUTPUT_PRIMARY_REQUEST: u8 = 31;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetOutputPrimaryRequest {
    pub window: xproto::Window,
}
impl_debug_if_no_extra_traits!(GetOutputPrimaryRequest, "GetOutputPrimaryRequest");
impl GetOutputPrimaryRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_OUTPUT_PRIMARY_REQUEST,
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
        if header.minor_opcode != GET_OUTPUT_PRIMARY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let _ = remaining;
        Ok(GetOutputPrimaryRequest {
            window,
        })
    }
}
impl Request for GetOutputPrimaryRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetOutputPrimaryRequest {
    type Reply = GetOutputPrimaryReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetOutputPrimaryReply {
    pub sequence: u16,
    pub length: u32,
    pub output: Output,
}
impl_debug_if_no_extra_traits!(GetOutputPrimaryReply, "GetOutputPrimaryReply");
impl TryParse for GetOutputPrimaryReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (output, remaining) = Output::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetOutputPrimaryReply { sequence, length, output };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetOutputPrimaryReply {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let output_bytes = self.output.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            output_bytes[0],
            output_bytes[1],
            output_bytes[2],
            output_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.output.serialize_into(bytes);
    }
}

/// Opcode for the GetProviders request
pub const GET_PROVIDERS_REQUEST: u8 = 32;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetProvidersRequest {
    pub window: xproto::Window,
}
impl_debug_if_no_extra_traits!(GetProvidersRequest, "GetProvidersRequest");
impl GetProvidersRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_PROVIDERS_REQUEST,
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
        if header.minor_opcode != GET_PROVIDERS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let _ = remaining;
        Ok(GetProvidersRequest {
            window,
        })
    }
}
impl Request for GetProvidersRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetProvidersRequest {
    type Reply = GetProvidersReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetProvidersReply {
    pub sequence: u16,
    pub length: u32,
    pub timestamp: xproto::Timestamp,
    pub providers: Vec<Provider>,
}
impl_debug_if_no_extra_traits!(GetProvidersReply, "GetProvidersReply");
impl TryParse for GetProvidersReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (num_providers, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(18..).ok_or(ParseError::InsufficientData)?;
        let (providers, remaining) = crate::x11_utils::parse_list::<Provider>(remaining, num_providers.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetProvidersReply { sequence, length, timestamp, providers };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetProvidersReply {
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
        self.timestamp.serialize_into(bytes);
        let num_providers = u16::try_from(self.providers.len()).expect("`providers` has too many elements");
        num_providers.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 18]);
        self.providers.serialize_into(bytes);
    }
}
impl GetProvidersReply {
    /// Get the value of the `num_providers` field.
    ///
    /// The `num_providers` field is used as the length field of the `providers` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_providers(&self) -> u16 {
        self.providers.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProviderCapability(u32);
impl ProviderCapability {
    pub const SOURCE_OUTPUT: Self = Self(1 << 0);
    pub const SINK_OUTPUT: Self = Self(1 << 1);
    pub const SOURCE_OFFLOAD: Self = Self(1 << 2);
    pub const SINK_OFFLOAD: Self = Self(1 << 3);
}
impl From<ProviderCapability> for u32 {
    #[inline]
    fn from(input: ProviderCapability) -> Self {
        input.0
    }
}
impl From<ProviderCapability> for Option<u32> {
    #[inline]
    fn from(input: ProviderCapability) -> Self {
        Some(input.0)
    }
}
impl From<u8> for ProviderCapability {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for ProviderCapability {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for ProviderCapability {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ProviderCapability  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SOURCE_OUTPUT.0, "SOURCE_OUTPUT", "SourceOutput"),
            (Self::SINK_OUTPUT.0, "SINK_OUTPUT", "SinkOutput"),
            (Self::SOURCE_OFFLOAD.0, "SOURCE_OFFLOAD", "SourceOffload"),
            (Self::SINK_OFFLOAD.0, "SINK_OFFLOAD", "SinkOffload"),
        ];
        pretty_print_bitmask(fmt, self.0, &variants)
    }
}
bitmask_binop!(ProviderCapability, u32);

/// Opcode for the GetProviderInfo request
pub const GET_PROVIDER_INFO_REQUEST: u8 = 33;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetProviderInfoRequest {
    pub provider: Provider,
    pub config_timestamp: xproto::Timestamp,
}
impl_debug_if_no_extra_traits!(GetProviderInfoRequest, "GetProviderInfoRequest");
impl GetProviderInfoRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let provider_bytes = self.provider.serialize();
        let config_timestamp_bytes = self.config_timestamp.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_PROVIDER_INFO_REQUEST,
            0,
            0,
            provider_bytes[0],
            provider_bytes[1],
            provider_bytes[2],
            provider_bytes[3],
            config_timestamp_bytes[0],
            config_timestamp_bytes[1],
            config_timestamp_bytes[2],
            config_timestamp_bytes[3],
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
        if header.minor_opcode != GET_PROVIDER_INFO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (provider, remaining) = Provider::try_parse(value)?;
        let (config_timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetProviderInfoRequest {
            provider,
            config_timestamp,
        })
    }
}
impl Request for GetProviderInfoRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetProviderInfoRequest {
    type Reply = GetProviderInfoReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetProviderInfoReply {
    pub status: u8,
    pub sequence: u16,
    pub length: u32,
    pub timestamp: xproto::Timestamp,
    pub capabilities: ProviderCapability,
    pub crtcs: Vec<Crtc>,
    pub outputs: Vec<Output>,
    pub associated_providers: Vec<Provider>,
    pub associated_capability: Vec<u32>,
    pub name: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetProviderInfoReply, "GetProviderInfoReply");
impl TryParse for GetProviderInfoReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (status, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (capabilities, remaining) = u32::try_parse(remaining)?;
        let (num_crtcs, remaining) = u16::try_parse(remaining)?;
        let (num_outputs, remaining) = u16::try_parse(remaining)?;
        let (num_associated_providers, remaining) = u16::try_parse(remaining)?;
        let (name_len, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(8..).ok_or(ParseError::InsufficientData)?;
        let (crtcs, remaining) = crate::x11_utils::parse_list::<Crtc>(remaining, num_crtcs.try_to_usize()?)?;
        let (outputs, remaining) = crate::x11_utils::parse_list::<Output>(remaining, num_outputs.try_to_usize()?)?;
        let (associated_providers, remaining) = crate::x11_utils::parse_list::<Provider>(remaining, num_associated_providers.try_to_usize()?)?;
        let (associated_capability, remaining) = crate::x11_utils::parse_list::<u32>(remaining, num_associated_providers.try_to_usize()?)?;
        let (name, remaining) = crate::x11_utils::parse_u8_list(remaining, name_len.try_to_usize()?)?;
        let name = name.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let capabilities = capabilities.into();
        let result = GetProviderInfoReply { status, sequence, length, timestamp, capabilities, crtcs, outputs, associated_providers, associated_capability, name };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetProviderInfoReply {
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
        self.status.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.timestamp.serialize_into(bytes);
        u32::from(self.capabilities).serialize_into(bytes);
        let num_crtcs = u16::try_from(self.crtcs.len()).expect("`crtcs` has too many elements");
        num_crtcs.serialize_into(bytes);
        let num_outputs = u16::try_from(self.outputs.len()).expect("`outputs` has too many elements");
        num_outputs.serialize_into(bytes);
        let num_associated_providers = u16::try_from(self.associated_providers.len()).expect("`associated_providers` has too many elements");
        num_associated_providers.serialize_into(bytes);
        let name_len = u16::try_from(self.name.len()).expect("`name` has too many elements");
        name_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 8]);
        self.crtcs.serialize_into(bytes);
        self.outputs.serialize_into(bytes);
        self.associated_providers.serialize_into(bytes);
        assert_eq!(self.associated_capability.len(), usize::try_from(num_associated_providers).unwrap(), "`associated_capability` has an incorrect length");
        self.associated_capability.serialize_into(bytes);
        bytes.extend_from_slice(&self.name);
    }
}
impl GetProviderInfoReply {
    /// Get the value of the `num_crtcs` field.
    ///
    /// The `num_crtcs` field is used as the length field of the `crtcs` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_crtcs(&self) -> u16 {
        self.crtcs.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_outputs` field.
    ///
    /// The `num_outputs` field is used as the length field of the `outputs` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_outputs(&self) -> u16 {
        self.outputs.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_associated_providers` field.
    ///
    /// The `num_associated_providers` field is used as the length field of the `associated_providers` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_associated_providers(&self) -> u16 {
        self.associated_providers.len()
            .try_into().unwrap()
    }
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

/// Opcode for the SetProviderOffloadSink request
pub const SET_PROVIDER_OFFLOAD_SINK_REQUEST: u8 = 34;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetProviderOffloadSinkRequest {
    pub provider: Provider,
    pub sink_provider: Provider,
    pub config_timestamp: xproto::Timestamp,
}
impl_debug_if_no_extra_traits!(SetProviderOffloadSinkRequest, "SetProviderOffloadSinkRequest");
impl SetProviderOffloadSinkRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let provider_bytes = self.provider.serialize();
        let sink_provider_bytes = self.sink_provider.serialize();
        let config_timestamp_bytes = self.config_timestamp.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_PROVIDER_OFFLOAD_SINK_REQUEST,
            0,
            0,
            provider_bytes[0],
            provider_bytes[1],
            provider_bytes[2],
            provider_bytes[3],
            sink_provider_bytes[0],
            sink_provider_bytes[1],
            sink_provider_bytes[2],
            sink_provider_bytes[3],
            config_timestamp_bytes[0],
            config_timestamp_bytes[1],
            config_timestamp_bytes[2],
            config_timestamp_bytes[3],
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
        if header.minor_opcode != SET_PROVIDER_OFFLOAD_SINK_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (provider, remaining) = Provider::try_parse(value)?;
        let (sink_provider, remaining) = Provider::try_parse(remaining)?;
        let (config_timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let _ = remaining;
        Ok(SetProviderOffloadSinkRequest {
            provider,
            sink_provider,
            config_timestamp,
        })
    }
}
impl Request for SetProviderOffloadSinkRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SetProviderOffloadSinkRequest {
}

/// Opcode for the SetProviderOutputSource request
pub const SET_PROVIDER_OUTPUT_SOURCE_REQUEST: u8 = 35;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetProviderOutputSourceRequest {
    pub provider: Provider,
    pub source_provider: Provider,
    pub config_timestamp: xproto::Timestamp,
}
impl_debug_if_no_extra_traits!(SetProviderOutputSourceRequest, "SetProviderOutputSourceRequest");
impl SetProviderOutputSourceRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let provider_bytes = self.provider.serialize();
        let source_provider_bytes = self.source_provider.serialize();
        let config_timestamp_bytes = self.config_timestamp.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_PROVIDER_OUTPUT_SOURCE_REQUEST,
            0,
            0,
            provider_bytes[0],
            provider_bytes[1],
            provider_bytes[2],
            provider_bytes[3],
            source_provider_bytes[0],
            source_provider_bytes[1],
            source_provider_bytes[2],
            source_provider_bytes[3],
            config_timestamp_bytes[0],
            config_timestamp_bytes[1],
            config_timestamp_bytes[2],
            config_timestamp_bytes[3],
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
        if header.minor_opcode != SET_PROVIDER_OUTPUT_SOURCE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (provider, remaining) = Provider::try_parse(value)?;
        let (source_provider, remaining) = Provider::try_parse(remaining)?;
        let (config_timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let _ = remaining;
        Ok(SetProviderOutputSourceRequest {
            provider,
            source_provider,
            config_timestamp,
        })
    }
}
impl Request for SetProviderOutputSourceRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SetProviderOutputSourceRequest {
}

/// Opcode for the ListProviderProperties request
pub const LIST_PROVIDER_PROPERTIES_REQUEST: u8 = 36;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListProviderPropertiesRequest {
    pub provider: Provider,
}
impl_debug_if_no_extra_traits!(ListProviderPropertiesRequest, "ListProviderPropertiesRequest");
impl ListProviderPropertiesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let provider_bytes = self.provider.serialize();
        let mut request0 = vec![
            major_opcode,
            LIST_PROVIDER_PROPERTIES_REQUEST,
            0,
            0,
            provider_bytes[0],
            provider_bytes[1],
            provider_bytes[2],
            provider_bytes[3],
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
        if header.minor_opcode != LIST_PROVIDER_PROPERTIES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (provider, remaining) = Provider::try_parse(value)?;
        let _ = remaining;
        Ok(ListProviderPropertiesRequest {
            provider,
        })
    }
}
impl Request for ListProviderPropertiesRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for ListProviderPropertiesRequest {
    type Reply = ListProviderPropertiesReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListProviderPropertiesReply {
    pub sequence: u16,
    pub length: u32,
    pub atoms: Vec<xproto::Atom>,
}
impl_debug_if_no_extra_traits!(ListProviderPropertiesReply, "ListProviderPropertiesReply");
impl TryParse for ListProviderPropertiesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (num_atoms, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
        let (atoms, remaining) = crate::x11_utils::parse_list::<xproto::Atom>(remaining, num_atoms.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = ListProviderPropertiesReply { sequence, length, atoms };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ListProviderPropertiesReply {
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
        let num_atoms = u16::try_from(self.atoms.len()).expect("`atoms` has too many elements");
        num_atoms.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
        self.atoms.serialize_into(bytes);
    }
}
impl ListProviderPropertiesReply {
    /// Get the value of the `num_atoms` field.
    ///
    /// The `num_atoms` field is used as the length field of the `atoms` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_atoms(&self) -> u16 {
        self.atoms.len()
            .try_into().unwrap()
    }
}

/// Opcode for the QueryProviderProperty request
pub const QUERY_PROVIDER_PROPERTY_REQUEST: u8 = 37;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryProviderPropertyRequest {
    pub provider: Provider,
    pub property: xproto::Atom,
}
impl_debug_if_no_extra_traits!(QueryProviderPropertyRequest, "QueryProviderPropertyRequest");
impl QueryProviderPropertyRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let provider_bytes = self.provider.serialize();
        let property_bytes = self.property.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_PROVIDER_PROPERTY_REQUEST,
            0,
            0,
            provider_bytes[0],
            provider_bytes[1],
            provider_bytes[2],
            provider_bytes[3],
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
        if header.minor_opcode != QUERY_PROVIDER_PROPERTY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (provider, remaining) = Provider::try_parse(value)?;
        let (property, remaining) = xproto::Atom::try_parse(remaining)?;
        let _ = remaining;
        Ok(QueryProviderPropertyRequest {
            provider,
            property,
        })
    }
}
impl Request for QueryProviderPropertyRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryProviderPropertyRequest {
    type Reply = QueryProviderPropertyReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryProviderPropertyReply {
    pub sequence: u16,
    pub pending: bool,
    pub range: bool,
    pub immutable: bool,
    pub valid_values: Vec<i32>,
}
impl_debug_if_no_extra_traits!(QueryProviderPropertyReply, "QueryProviderPropertyReply");
impl TryParse for QueryProviderPropertyReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (pending, remaining) = bool::try_parse(remaining)?;
        let (range, remaining) = bool::try_parse(remaining)?;
        let (immutable, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(21..).ok_or(ParseError::InsufficientData)?;
        let (valid_values, remaining) = crate::x11_utils::parse_list::<i32>(remaining, length.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryProviderPropertyReply { sequence, pending, range, immutable, valid_values };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryProviderPropertyReply {
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
        let length = u32::try_from(self.valid_values.len()).expect("`valid_values` has too many elements");
        length.serialize_into(bytes);
        self.pending.serialize_into(bytes);
        self.range.serialize_into(bytes);
        self.immutable.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 21]);
        self.valid_values.serialize_into(bytes);
    }
}
impl QueryProviderPropertyReply {
    /// Get the value of the `length` field.
    ///
    /// The `length` field is used as the length field of the `valid_values` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn length(&self) -> u32 {
        self.valid_values.len()
            .try_into().unwrap()
    }
}

/// Opcode for the ConfigureProviderProperty request
pub const CONFIGURE_PROVIDER_PROPERTY_REQUEST: u8 = 38;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConfigureProviderPropertyRequest<'input> {
    pub provider: Provider,
    pub property: xproto::Atom,
    pub pending: bool,
    pub range: bool,
    pub values: Cow<'input, [i32]>,
}
impl_debug_if_no_extra_traits!(ConfigureProviderPropertyRequest<'_>, "ConfigureProviderPropertyRequest");
impl<'input> ConfigureProviderPropertyRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let provider_bytes = self.provider.serialize();
        let property_bytes = self.property.serialize();
        let pending_bytes = self.pending.serialize();
        let range_bytes = self.range.serialize();
        let mut request0 = vec![
            major_opcode,
            CONFIGURE_PROVIDER_PROPERTY_REQUEST,
            0,
            0,
            provider_bytes[0],
            provider_bytes[1],
            provider_bytes[2],
            provider_bytes[3],
            property_bytes[0],
            property_bytes[1],
            property_bytes[2],
            property_bytes[3],
            pending_bytes[0],
            range_bytes[0],
            0,
            0,
        ];
        let length_so_far = length_so_far + request0.len();
        let values_bytes = self.values.serialize();
        let length_so_far = length_so_far + values_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), values_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != CONFIGURE_PROVIDER_PROPERTY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (provider, remaining) = Provider::try_parse(value)?;
        let (property, remaining) = xproto::Atom::try_parse(remaining)?;
        let (pending, remaining) = bool::try_parse(remaining)?;
        let (range, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut values = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = i32::try_parse(remaining)?;
            remaining = new_remaining;
            values.push(v);
        }
        let _ = remaining;
        Ok(ConfigureProviderPropertyRequest {
            provider,
            property,
            pending,
            range,
            values: Cow::Owned(values),
        })
    }
    /// Clone all borrowed data in this ConfigureProviderPropertyRequest.
    pub fn into_owned(self) -> ConfigureProviderPropertyRequest<'static> {
        ConfigureProviderPropertyRequest {
            provider: self.provider,
            property: self.property,
            pending: self.pending,
            range: self.range,
            values: Cow::Owned(self.values.into_owned()),
        }
    }
}
impl<'input> Request for ConfigureProviderPropertyRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for ConfigureProviderPropertyRequest<'input> {
}

/// Opcode for the ChangeProviderProperty request
pub const CHANGE_PROVIDER_PROPERTY_REQUEST: u8 = 39;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChangeProviderPropertyRequest<'input> {
    pub provider: Provider,
    pub property: xproto::Atom,
    pub type_: xproto::Atom,
    pub format: u8,
    pub mode: u8,
    pub num_items: u32,
    pub data: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(ChangeProviderPropertyRequest<'_>, "ChangeProviderPropertyRequest");
impl<'input> ChangeProviderPropertyRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let provider_bytes = self.provider.serialize();
        let property_bytes = self.property.serialize();
        let type_bytes = self.type_.serialize();
        let format_bytes = self.format.serialize();
        let mode_bytes = self.mode.serialize();
        let num_items_bytes = self.num_items.serialize();
        let mut request0 = vec![
            major_opcode,
            CHANGE_PROVIDER_PROPERTY_REQUEST,
            0,
            0,
            provider_bytes[0],
            provider_bytes[1],
            provider_bytes[2],
            provider_bytes[3],
            property_bytes[0],
            property_bytes[1],
            property_bytes[2],
            property_bytes[3],
            type_bytes[0],
            type_bytes[1],
            type_bytes[2],
            type_bytes[3],
            format_bytes[0],
            mode_bytes[0],
            0,
            0,
            num_items_bytes[0],
            num_items_bytes[1],
            num_items_bytes[2],
            num_items_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        assert_eq!(self.data.len(), usize::try_from(u32::from(self.num_items).checked_mul(u32::from(self.format).checked_div(8u32).unwrap()).unwrap()).unwrap(), "`data` has an incorrect length");
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
        if header.minor_opcode != CHANGE_PROVIDER_PROPERTY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (provider, remaining) = Provider::try_parse(value)?;
        let (property, remaining) = xproto::Atom::try_parse(remaining)?;
        let (type_, remaining) = xproto::Atom::try_parse(remaining)?;
        let (format, remaining) = u8::try_parse(remaining)?;
        let (mode, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (num_items, remaining) = u32::try_parse(remaining)?;
        let (data, remaining) = crate::x11_utils::parse_u8_list(remaining, u32::from(num_items).checked_mul(u32::from(format).checked_div(8u32).ok_or(ParseError::InvalidExpression)?).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let _ = remaining;
        Ok(ChangeProviderPropertyRequest {
            provider,
            property,
            type_,
            format,
            mode,
            num_items,
            data: Cow::Borrowed(data),
        })
    }
    /// Clone all borrowed data in this ChangeProviderPropertyRequest.
    pub fn into_owned(self) -> ChangeProviderPropertyRequest<'static> {
        ChangeProviderPropertyRequest {
            provider: self.provider,
            property: self.property,
            type_: self.type_,
            format: self.format,
            mode: self.mode,
            num_items: self.num_items,
            data: Cow::Owned(self.data.into_owned()),
        }
    }
}
impl<'input> Request for ChangeProviderPropertyRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for ChangeProviderPropertyRequest<'input> {
}

/// Opcode for the DeleteProviderProperty request
pub const DELETE_PROVIDER_PROPERTY_REQUEST: u8 = 40;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeleteProviderPropertyRequest {
    pub provider: Provider,
    pub property: xproto::Atom,
}
impl_debug_if_no_extra_traits!(DeleteProviderPropertyRequest, "DeleteProviderPropertyRequest");
impl DeleteProviderPropertyRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let provider_bytes = self.provider.serialize();
        let property_bytes = self.property.serialize();
        let mut request0 = vec![
            major_opcode,
            DELETE_PROVIDER_PROPERTY_REQUEST,
            0,
            0,
            provider_bytes[0],
            provider_bytes[1],
            provider_bytes[2],
            provider_bytes[3],
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
        if header.minor_opcode != DELETE_PROVIDER_PROPERTY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (provider, remaining) = Provider::try_parse(value)?;
        let (property, remaining) = xproto::Atom::try_parse(remaining)?;
        let _ = remaining;
        Ok(DeleteProviderPropertyRequest {
            provider,
            property,
        })
    }
}
impl Request for DeleteProviderPropertyRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DeleteProviderPropertyRequest {
}

/// Opcode for the GetProviderProperty request
pub const GET_PROVIDER_PROPERTY_REQUEST: u8 = 41;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetProviderPropertyRequest {
    pub provider: Provider,
    pub property: xproto::Atom,
    pub type_: xproto::Atom,
    pub long_offset: u32,
    pub long_length: u32,
    pub delete: bool,
    pub pending: bool,
}
impl_debug_if_no_extra_traits!(GetProviderPropertyRequest, "GetProviderPropertyRequest");
impl GetProviderPropertyRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let provider_bytes = self.provider.serialize();
        let property_bytes = self.property.serialize();
        let type_bytes = self.type_.serialize();
        let long_offset_bytes = self.long_offset.serialize();
        let long_length_bytes = self.long_length.serialize();
        let delete_bytes = self.delete.serialize();
        let pending_bytes = self.pending.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_PROVIDER_PROPERTY_REQUEST,
            0,
            0,
            provider_bytes[0],
            provider_bytes[1],
            provider_bytes[2],
            provider_bytes[3],
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
            delete_bytes[0],
            pending_bytes[0],
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
        if header.minor_opcode != GET_PROVIDER_PROPERTY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (provider, remaining) = Provider::try_parse(value)?;
        let (property, remaining) = xproto::Atom::try_parse(remaining)?;
        let (type_, remaining) = xproto::Atom::try_parse(remaining)?;
        let (long_offset, remaining) = u32::try_parse(remaining)?;
        let (long_length, remaining) = u32::try_parse(remaining)?;
        let (delete, remaining) = bool::try_parse(remaining)?;
        let (pending, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(GetProviderPropertyRequest {
            provider,
            property,
            type_,
            long_offset,
            long_length,
            delete,
            pending,
        })
    }
}
impl Request for GetProviderPropertyRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetProviderPropertyRequest {
    type Reply = GetProviderPropertyReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetProviderPropertyReply {
    pub format: u8,
    pub sequence: u16,
    pub length: u32,
    pub type_: xproto::Atom,
    pub bytes_after: u32,
    pub num_items: u32,
    pub data: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetProviderPropertyReply, "GetProviderPropertyReply");
impl TryParse for GetProviderPropertyReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (format, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (type_, remaining) = xproto::Atom::try_parse(remaining)?;
        let (bytes_after, remaining) = u32::try_parse(remaining)?;
        let (num_items, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(12..).ok_or(ParseError::InsufficientData)?;
        let (data, remaining) = crate::x11_utils::parse_u8_list(remaining, u32::from(num_items).checked_mul(u32::from(format).checked_div(8u32).ok_or(ParseError::InvalidExpression)?).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let data = data.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetProviderPropertyReply { format, sequence, length, type_, bytes_after, num_items, data };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetProviderPropertyReply {
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
        self.num_items.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 12]);
        assert_eq!(self.data.len(), usize::try_from(u32::from(self.num_items).checked_mul(u32::from(self.format).checked_div(8u32).unwrap()).unwrap()).unwrap(), "`data` has an incorrect length");
        bytes.extend_from_slice(&self.data);
    }
}

/// Opcode for the ScreenChangeNotify event
pub const SCREEN_CHANGE_NOTIFY_EVENT: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScreenChangeNotifyEvent {
    pub response_type: u8,
    pub rotation: Rotation,
    pub sequence: u16,
    pub timestamp: xproto::Timestamp,
    pub config_timestamp: xproto::Timestamp,
    pub root: xproto::Window,
    pub request_window: xproto::Window,
    pub size_id: u16,
    pub subpixel_order: render::SubPixel,
    pub width: u16,
    pub height: u16,
    pub mwidth: u16,
    pub mheight: u16,
}
impl_debug_if_no_extra_traits!(ScreenChangeNotifyEvent, "ScreenChangeNotifyEvent");
impl TryParse for ScreenChangeNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (rotation, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (config_timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (root, remaining) = xproto::Window::try_parse(remaining)?;
        let (request_window, remaining) = xproto::Window::try_parse(remaining)?;
        let (size_id, remaining) = u16::try_parse(remaining)?;
        let (subpixel_order, remaining) = u16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (mwidth, remaining) = u16::try_parse(remaining)?;
        let (mheight, remaining) = u16::try_parse(remaining)?;
        let rotation = rotation.into();
        let subpixel_order = subpixel_order.into();
        let result = ScreenChangeNotifyEvent { response_type, rotation, sequence, timestamp, config_timestamp, root, request_window, size_id, subpixel_order, width, height, mwidth, mheight };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ScreenChangeNotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let rotation_bytes = (u16::from(self.rotation) as u8).serialize();
        let sequence_bytes = self.sequence.serialize();
        let timestamp_bytes = self.timestamp.serialize();
        let config_timestamp_bytes = self.config_timestamp.serialize();
        let root_bytes = self.root.serialize();
        let request_window_bytes = self.request_window.serialize();
        let size_id_bytes = self.size_id.serialize();
        let subpixel_order_bytes = (u32::from(self.subpixel_order) as u16).serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let mwidth_bytes = self.mwidth.serialize();
        let mheight_bytes = self.mheight.serialize();
        [
            response_type_bytes[0],
            rotation_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            config_timestamp_bytes[0],
            config_timestamp_bytes[1],
            config_timestamp_bytes[2],
            config_timestamp_bytes[3],
            root_bytes[0],
            root_bytes[1],
            root_bytes[2],
            root_bytes[3],
            request_window_bytes[0],
            request_window_bytes[1],
            request_window_bytes[2],
            request_window_bytes[3],
            size_id_bytes[0],
            size_id_bytes[1],
            subpixel_order_bytes[0],
            subpixel_order_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            mwidth_bytes[0],
            mwidth_bytes[1],
            mheight_bytes[0],
            mheight_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        (u16::from(self.rotation) as u8).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.timestamp.serialize_into(bytes);
        self.config_timestamp.serialize_into(bytes);
        self.root.serialize_into(bytes);
        self.request_window.serialize_into(bytes);
        self.size_id.serialize_into(bytes);
        (u32::from(self.subpixel_order) as u16).serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        self.mwidth.serialize_into(bytes);
        self.mheight.serialize_into(bytes);
    }
}
impl From<&ScreenChangeNotifyEvent> for [u8; 32] {
    fn from(input: &ScreenChangeNotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let rotation_bytes = (u16::from(input.rotation) as u8).serialize();
        let sequence_bytes = input.sequence.serialize();
        let timestamp_bytes = input.timestamp.serialize();
        let config_timestamp_bytes = input.config_timestamp.serialize();
        let root_bytes = input.root.serialize();
        let request_window_bytes = input.request_window.serialize();
        let size_id_bytes = input.size_id.serialize();
        let subpixel_order_bytes = (u32::from(input.subpixel_order) as u16).serialize();
        let width_bytes = input.width.serialize();
        let height_bytes = input.height.serialize();
        let mwidth_bytes = input.mwidth.serialize();
        let mheight_bytes = input.mheight.serialize();
        [
            response_type_bytes[0],
            rotation_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            config_timestamp_bytes[0],
            config_timestamp_bytes[1],
            config_timestamp_bytes[2],
            config_timestamp_bytes[3],
            root_bytes[0],
            root_bytes[1],
            root_bytes[2],
            root_bytes[3],
            request_window_bytes[0],
            request_window_bytes[1],
            request_window_bytes[2],
            request_window_bytes[3],
            size_id_bytes[0],
            size_id_bytes[1],
            subpixel_order_bytes[0],
            subpixel_order_bytes[1],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            mwidth_bytes[0],
            mwidth_bytes[1],
            mheight_bytes[0],
            mheight_bytes[1],
        ]
    }
}
impl From<ScreenChangeNotifyEvent> for [u8; 32] {
    fn from(input: ScreenChangeNotifyEvent) -> Self {
        Self::from(&input)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Notify(u8);
impl Notify {
    pub const CRTC_CHANGE: Self = Self(0);
    pub const OUTPUT_CHANGE: Self = Self(1);
    pub const OUTPUT_PROPERTY: Self = Self(2);
    pub const PROVIDER_CHANGE: Self = Self(3);
    pub const PROVIDER_PROPERTY: Self = Self(4);
    pub const RESOURCE_CHANGE: Self = Self(5);
    pub const LEASE: Self = Self(6);
}
impl From<Notify> for u8 {
    #[inline]
    fn from(input: Notify) -> Self {
        input.0
    }
}
impl From<Notify> for Option<u8> {
    #[inline]
    fn from(input: Notify) -> Self {
        Some(input.0)
    }
}
impl From<Notify> for u16 {
    #[inline]
    fn from(input: Notify) -> Self {
        u16::from(input.0)
    }
}
impl From<Notify> for Option<u16> {
    #[inline]
    fn from(input: Notify) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Notify> for u32 {
    #[inline]
    fn from(input: Notify) -> Self {
        u32::from(input.0)
    }
}
impl From<Notify> for Option<u32> {
    #[inline]
    fn from(input: Notify) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Notify {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Notify  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::CRTC_CHANGE.0.into(), "CRTC_CHANGE", "CrtcChange"),
            (Self::OUTPUT_CHANGE.0.into(), "OUTPUT_CHANGE", "OutputChange"),
            (Self::OUTPUT_PROPERTY.0.into(), "OUTPUT_PROPERTY", "OutputProperty"),
            (Self::PROVIDER_CHANGE.0.into(), "PROVIDER_CHANGE", "ProviderChange"),
            (Self::PROVIDER_PROPERTY.0.into(), "PROVIDER_PROPERTY", "ProviderProperty"),
            (Self::RESOURCE_CHANGE.0.into(), "RESOURCE_CHANGE", "ResourceChange"),
            (Self::LEASE.0.into(), "LEASE", "Lease"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CrtcChange {
    pub timestamp: xproto::Timestamp,
    pub window: xproto::Window,
    pub crtc: Crtc,
    pub mode: Mode,
    pub rotation: Rotation,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}
impl_debug_if_no_extra_traits!(CrtcChange, "CrtcChange");
impl TryParse for CrtcChange {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (window, remaining) = xproto::Window::try_parse(remaining)?;
        let (crtc, remaining) = Crtc::try_parse(remaining)?;
        let (mode, remaining) = Mode::try_parse(remaining)?;
        let (rotation, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let rotation = rotation.into();
        let result = CrtcChange { timestamp, window, crtc, mode, rotation, x, y, width, height };
        Ok((result, remaining))
    }
}
impl Serialize for CrtcChange {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        let timestamp_bytes = self.timestamp.serialize();
        let window_bytes = self.window.serialize();
        let crtc_bytes = self.crtc.serialize();
        let mode_bytes = self.mode.serialize();
        let rotation_bytes = u16::from(self.rotation).serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        [
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            crtc_bytes[0],
            crtc_bytes[1],
            crtc_bytes[2],
            crtc_bytes[3],
            mode_bytes[0],
            mode_bytes[1],
            mode_bytes[2],
            mode_bytes[3],
            rotation_bytes[0],
            rotation_bytes[1],
            0,
            0,
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
        bytes.reserve(28);
        self.timestamp.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.crtc.serialize_into(bytes);
        self.mode.serialize_into(bytes);
        u16::from(self.rotation).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OutputChange {
    pub timestamp: xproto::Timestamp,
    pub config_timestamp: xproto::Timestamp,
    pub window: xproto::Window,
    pub output: Output,
    pub crtc: Crtc,
    pub mode: Mode,
    pub rotation: Rotation,
    pub connection: Connection,
    pub subpixel_order: render::SubPixel,
}
impl_debug_if_no_extra_traits!(OutputChange, "OutputChange");
impl TryParse for OutputChange {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (config_timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (window, remaining) = xproto::Window::try_parse(remaining)?;
        let (output, remaining) = Output::try_parse(remaining)?;
        let (crtc, remaining) = Crtc::try_parse(remaining)?;
        let (mode, remaining) = Mode::try_parse(remaining)?;
        let (rotation, remaining) = u16::try_parse(remaining)?;
        let (connection, remaining) = u8::try_parse(remaining)?;
        let (subpixel_order, remaining) = u8::try_parse(remaining)?;
        let rotation = rotation.into();
        let connection = connection.into();
        let subpixel_order = subpixel_order.into();
        let result = OutputChange { timestamp, config_timestamp, window, output, crtc, mode, rotation, connection, subpixel_order };
        Ok((result, remaining))
    }
}
impl Serialize for OutputChange {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        let timestamp_bytes = self.timestamp.serialize();
        let config_timestamp_bytes = self.config_timestamp.serialize();
        let window_bytes = self.window.serialize();
        let output_bytes = self.output.serialize();
        let crtc_bytes = self.crtc.serialize();
        let mode_bytes = self.mode.serialize();
        let rotation_bytes = u16::from(self.rotation).serialize();
        let connection_bytes = u8::from(self.connection).serialize();
        let subpixel_order_bytes = (u32::from(self.subpixel_order) as u8).serialize();
        [
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            config_timestamp_bytes[0],
            config_timestamp_bytes[1],
            config_timestamp_bytes[2],
            config_timestamp_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            output_bytes[0],
            output_bytes[1],
            output_bytes[2],
            output_bytes[3],
            crtc_bytes[0],
            crtc_bytes[1],
            crtc_bytes[2],
            crtc_bytes[3],
            mode_bytes[0],
            mode_bytes[1],
            mode_bytes[2],
            mode_bytes[3],
            rotation_bytes[0],
            rotation_bytes[1],
            connection_bytes[0],
            subpixel_order_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(28);
        self.timestamp.serialize_into(bytes);
        self.config_timestamp.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.output.serialize_into(bytes);
        self.crtc.serialize_into(bytes);
        self.mode.serialize_into(bytes);
        u16::from(self.rotation).serialize_into(bytes);
        u8::from(self.connection).serialize_into(bytes);
        (u32::from(self.subpixel_order) as u8).serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OutputProperty {
    pub window: xproto::Window,
    pub output: Output,
    pub atom: xproto::Atom,
    pub timestamp: xproto::Timestamp,
    pub status: xproto::Property,
}
impl_debug_if_no_extra_traits!(OutputProperty, "OutputProperty");
impl TryParse for OutputProperty {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (window, remaining) = xproto::Window::try_parse(remaining)?;
        let (output, remaining) = Output::try_parse(remaining)?;
        let (atom, remaining) = xproto::Atom::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (status, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(11..).ok_or(ParseError::InsufficientData)?;
        let status = status.into();
        let result = OutputProperty { window, output, atom, timestamp, status };
        Ok((result, remaining))
    }
}
impl Serialize for OutputProperty {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        let window_bytes = self.window.serialize();
        let output_bytes = self.output.serialize();
        let atom_bytes = self.atom.serialize();
        let timestamp_bytes = self.timestamp.serialize();
        let status_bytes = u8::from(self.status).serialize();
        [
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            output_bytes[0],
            output_bytes[1],
            output_bytes[2],
            output_bytes[3],
            atom_bytes[0],
            atom_bytes[1],
            atom_bytes[2],
            atom_bytes[3],
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            status_bytes[0],
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
        bytes.reserve(28);
        self.window.serialize_into(bytes);
        self.output.serialize_into(bytes);
        self.atom.serialize_into(bytes);
        self.timestamp.serialize_into(bytes);
        u8::from(self.status).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 11]);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProviderChange {
    pub timestamp: xproto::Timestamp,
    pub window: xproto::Window,
    pub provider: Provider,
}
impl_debug_if_no_extra_traits!(ProviderChange, "ProviderChange");
impl TryParse for ProviderChange {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (window, remaining) = xproto::Window::try_parse(remaining)?;
        let (provider, remaining) = Provider::try_parse(remaining)?;
        let remaining = remaining.get(16..).ok_or(ParseError::InsufficientData)?;
        let result = ProviderChange { timestamp, window, provider };
        Ok((result, remaining))
    }
}
impl Serialize for ProviderChange {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        let timestamp_bytes = self.timestamp.serialize();
        let window_bytes = self.window.serialize();
        let provider_bytes = self.provider.serialize();
        [
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            provider_bytes[0],
            provider_bytes[1],
            provider_bytes[2],
            provider_bytes[3],
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
        bytes.reserve(28);
        self.timestamp.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.provider.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 16]);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProviderProperty {
    pub window: xproto::Window,
    pub provider: Provider,
    pub atom: xproto::Atom,
    pub timestamp: xproto::Timestamp,
    pub state: u8,
}
impl_debug_if_no_extra_traits!(ProviderProperty, "ProviderProperty");
impl TryParse for ProviderProperty {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (window, remaining) = xproto::Window::try_parse(remaining)?;
        let (provider, remaining) = Provider::try_parse(remaining)?;
        let (atom, remaining) = xproto::Atom::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (state, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(11..).ok_or(ParseError::InsufficientData)?;
        let result = ProviderProperty { window, provider, atom, timestamp, state };
        Ok((result, remaining))
    }
}
impl Serialize for ProviderProperty {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        let window_bytes = self.window.serialize();
        let provider_bytes = self.provider.serialize();
        let atom_bytes = self.atom.serialize();
        let timestamp_bytes = self.timestamp.serialize();
        let state_bytes = self.state.serialize();
        [
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            provider_bytes[0],
            provider_bytes[1],
            provider_bytes[2],
            provider_bytes[3],
            atom_bytes[0],
            atom_bytes[1],
            atom_bytes[2],
            atom_bytes[3],
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            state_bytes[0],
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
        bytes.reserve(28);
        self.window.serialize_into(bytes);
        self.provider.serialize_into(bytes);
        self.atom.serialize_into(bytes);
        self.timestamp.serialize_into(bytes);
        self.state.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 11]);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResourceChange {
    pub timestamp: xproto::Timestamp,
    pub window: xproto::Window,
}
impl_debug_if_no_extra_traits!(ResourceChange, "ResourceChange");
impl TryParse for ResourceChange {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (window, remaining) = xproto::Window::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let result = ResourceChange { timestamp, window };
        Ok((result, remaining))
    }
}
impl Serialize for ResourceChange {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        let timestamp_bytes = self.timestamp.serialize();
        let window_bytes = self.window.serialize();
        [
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        bytes.reserve(28);
        self.timestamp.serialize_into(bytes);
        self.window.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MonitorInfo {
    pub name: xproto::Atom,
    pub primary: bool,
    pub automatic: bool,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub width_in_millimeters: u32,
    pub height_in_millimeters: u32,
    pub outputs: Vec<Output>,
}
impl_debug_if_no_extra_traits!(MonitorInfo, "MonitorInfo");
impl TryParse for MonitorInfo {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (name, remaining) = xproto::Atom::try_parse(remaining)?;
        let (primary, remaining) = bool::try_parse(remaining)?;
        let (automatic, remaining) = bool::try_parse(remaining)?;
        let (n_output, remaining) = u16::try_parse(remaining)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (width_in_millimeters, remaining) = u32::try_parse(remaining)?;
        let (height_in_millimeters, remaining) = u32::try_parse(remaining)?;
        let (outputs, remaining) = crate::x11_utils::parse_list::<Output>(remaining, n_output.try_to_usize()?)?;
        let result = MonitorInfo { name, primary, automatic, x, y, width, height, width_in_millimeters, height_in_millimeters, outputs };
        Ok((result, remaining))
    }
}
impl Serialize for MonitorInfo {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(24);
        self.name.serialize_into(bytes);
        self.primary.serialize_into(bytes);
        self.automatic.serialize_into(bytes);
        let n_output = u16::try_from(self.outputs.len()).expect("`outputs` has too many elements");
        n_output.serialize_into(bytes);
        self.x.serialize_into(bytes);
        self.y.serialize_into(bytes);
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        self.width_in_millimeters.serialize_into(bytes);
        self.height_in_millimeters.serialize_into(bytes);
        self.outputs.serialize_into(bytes);
    }
}
impl MonitorInfo {
    /// Get the value of the `nOutput` field.
    ///
    /// The `nOutput` field is used as the length field of the `outputs` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_output(&self) -> u16 {
        self.outputs.len()
            .try_into().unwrap()
    }
}

/// Opcode for the GetMonitors request
pub const GET_MONITORS_REQUEST: u8 = 42;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetMonitorsRequest {
    pub window: xproto::Window,
    pub get_active: bool,
}
impl_debug_if_no_extra_traits!(GetMonitorsRequest, "GetMonitorsRequest");
impl GetMonitorsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let get_active_bytes = self.get_active.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_MONITORS_REQUEST,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            get_active_bytes[0],
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
        if header.minor_opcode != GET_MONITORS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (get_active, remaining) = bool::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetMonitorsRequest {
            window,
            get_active,
        })
    }
}
impl Request for GetMonitorsRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetMonitorsRequest {
    type Reply = GetMonitorsReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetMonitorsReply {
    pub sequence: u16,
    pub length: u32,
    pub timestamp: xproto::Timestamp,
    pub n_outputs: u32,
    pub monitors: Vec<MonitorInfo>,
}
impl_debug_if_no_extra_traits!(GetMonitorsReply, "GetMonitorsReply");
impl TryParse for GetMonitorsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (n_monitors, remaining) = u32::try_parse(remaining)?;
        let (n_outputs, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(12..).ok_or(ParseError::InsufficientData)?;
        let (monitors, remaining) = crate::x11_utils::parse_list::<MonitorInfo>(remaining, n_monitors.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetMonitorsReply { sequence, length, timestamp, n_outputs, monitors };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetMonitorsReply {
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
        self.timestamp.serialize_into(bytes);
        let n_monitors = u32::try_from(self.monitors.len()).expect("`monitors` has too many elements");
        n_monitors.serialize_into(bytes);
        self.n_outputs.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 12]);
        self.monitors.serialize_into(bytes);
    }
}
impl GetMonitorsReply {
    /// Get the value of the `nMonitors` field.
    ///
    /// The `nMonitors` field is used as the length field of the `monitors` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_monitors(&self) -> u32 {
        self.monitors.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SetMonitor request
pub const SET_MONITOR_REQUEST: u8 = 43;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetMonitorRequest {
    pub window: xproto::Window,
    pub monitorinfo: MonitorInfo,
}
impl_debug_if_no_extra_traits!(SetMonitorRequest, "SetMonitorRequest");
impl SetMonitorRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 3]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_MONITOR_REQUEST,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let monitorinfo_bytes = self.monitorinfo.serialize();
        let length_so_far = length_so_far + monitorinfo_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), monitorinfo_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &[u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_MONITOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (monitorinfo, remaining) = MonitorInfo::try_parse(remaining)?;
        let _ = remaining;
        Ok(SetMonitorRequest {
            window,
            monitorinfo,
        })
    }
}
impl Request for SetMonitorRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SetMonitorRequest {
}

/// Opcode for the DeleteMonitor request
pub const DELETE_MONITOR_REQUEST: u8 = 44;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeleteMonitorRequest {
    pub window: xproto::Window,
    pub name: xproto::Atom,
}
impl_debug_if_no_extra_traits!(DeleteMonitorRequest, "DeleteMonitorRequest");
impl DeleteMonitorRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let name_bytes = self.name.serialize();
        let mut request0 = vec![
            major_opcode,
            DELETE_MONITOR_REQUEST,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            name_bytes[0],
            name_bytes[1],
            name_bytes[2],
            name_bytes[3],
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
        if header.minor_opcode != DELETE_MONITOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (name, remaining) = xproto::Atom::try_parse(remaining)?;
        let _ = remaining;
        Ok(DeleteMonitorRequest {
            window,
            name,
        })
    }
}
impl Request for DeleteMonitorRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DeleteMonitorRequest {
}

/// Opcode for the CreateLease request
pub const CREATE_LEASE_REQUEST: u8 = 45;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateLeaseRequest<'input> {
    pub window: xproto::Window,
    pub lid: Lease,
    pub crtcs: Cow<'input, [Crtc]>,
    pub outputs: Cow<'input, [Output]>,
}
impl_debug_if_no_extra_traits!(CreateLeaseRequest<'_>, "CreateLeaseRequest");
impl<'input> CreateLeaseRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 4]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let lid_bytes = self.lid.serialize();
        let num_crtcs = u16::try_from(self.crtcs.len()).expect("`crtcs` has too many elements");
        let num_crtcs_bytes = num_crtcs.serialize();
        let num_outputs = u16::try_from(self.outputs.len()).expect("`outputs` has too many elements");
        let num_outputs_bytes = num_outputs.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_LEASE_REQUEST,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            lid_bytes[0],
            lid_bytes[1],
            lid_bytes[2],
            lid_bytes[3],
            num_crtcs_bytes[0],
            num_crtcs_bytes[1],
            num_outputs_bytes[0],
            num_outputs_bytes[1],
        ];
        let length_so_far = length_so_far + request0.len();
        let crtcs_bytes = self.crtcs.serialize();
        let length_so_far = length_so_far + crtcs_bytes.len();
        let outputs_bytes = self.outputs.serialize();
        let length_so_far = length_so_far + outputs_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), crtcs_bytes.into(), outputs_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != CREATE_LEASE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (lid, remaining) = Lease::try_parse(remaining)?;
        let (num_crtcs, remaining) = u16::try_parse(remaining)?;
        let (num_outputs, remaining) = u16::try_parse(remaining)?;
        let (crtcs, remaining) = crate::x11_utils::parse_list::<Crtc>(remaining, num_crtcs.try_to_usize()?)?;
        let (outputs, remaining) = crate::x11_utils::parse_list::<Output>(remaining, num_outputs.try_to_usize()?)?;
        let _ = remaining;
        Ok(CreateLeaseRequest {
            window,
            lid,
            crtcs: Cow::Owned(crtcs),
            outputs: Cow::Owned(outputs),
        })
    }
    /// Clone all borrowed data in this CreateLeaseRequest.
    pub fn into_owned(self) -> CreateLeaseRequest<'static> {
        CreateLeaseRequest {
            window: self.window,
            lid: self.lid,
            crtcs: Cow::Owned(self.crtcs.into_owned()),
            outputs: Cow::Owned(self.outputs.into_owned()),
        }
    }
}
impl<'input> Request for CreateLeaseRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyFDsRequest for CreateLeaseRequest<'input> {
    type Reply = CreateLeaseReply;
}

#[cfg_attr(feature = "extra-traits", derive(Debug))]
pub struct CreateLeaseReply {
    pub nfd: u8,
    pub sequence: u16,
    pub length: u32,
    pub master_fd: RawFdContainer,
}
impl_debug_if_no_extra_traits!(CreateLeaseReply, "CreateLeaseReply");
impl TryParseFd for CreateLeaseReply {
    fn try_parse_fd<'a>(initial_value: &'a [u8], fds: &mut Vec<RawFdContainer>) -> Result<(Self, &'a [u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (nfd, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        if fds.is_empty() { return Err(ParseError::MissingFileDescriptors) }
        let master_fd = fds.remove(0);
        let remaining = remaining.get(24..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = CreateLeaseReply { nfd, sequence, length, master_fd };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for CreateLeaseReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let nfd_bytes = self.nfd.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        [
            response_type_bytes[0],
            nfd_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
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
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.nfd.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 24]);
    }
}

/// Opcode for the FreeLease request
pub const FREE_LEASE_REQUEST: u8 = 46;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FreeLeaseRequest {
    pub lid: Lease,
    pub terminate: u8,
}
impl_debug_if_no_extra_traits!(FreeLeaseRequest, "FreeLeaseRequest");
impl FreeLeaseRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let lid_bytes = self.lid.serialize();
        let terminate_bytes = self.terminate.serialize();
        let mut request0 = vec![
            major_opcode,
            FREE_LEASE_REQUEST,
            0,
            0,
            lid_bytes[0],
            lid_bytes[1],
            lid_bytes[2],
            lid_bytes[3],
            terminate_bytes[0],
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
        if header.minor_opcode != FREE_LEASE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (lid, remaining) = Lease::try_parse(value)?;
        let (terminate, remaining) = u8::try_parse(remaining)?;
        let _ = remaining;
        Ok(FreeLeaseRequest {
            lid,
            terminate,
        })
    }
}
impl Request for FreeLeaseRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for FreeLeaseRequest {
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LeaseNotify {
    pub timestamp: xproto::Timestamp,
    pub window: xproto::Window,
    pub lease: Lease,
    pub created: u8,
}
impl_debug_if_no_extra_traits!(LeaseNotify, "LeaseNotify");
impl TryParse for LeaseNotify {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (window, remaining) = xproto::Window::try_parse(remaining)?;
        let (lease, remaining) = Lease::try_parse(remaining)?;
        let (created, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(15..).ok_or(ParseError::InsufficientData)?;
        let result = LeaseNotify { timestamp, window, lease, created };
        Ok((result, remaining))
    }
}
impl Serialize for LeaseNotify {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        let timestamp_bytes = self.timestamp.serialize();
        let window_bytes = self.window.serialize();
        let lease_bytes = self.lease.serialize();
        let created_bytes = self.created.serialize();
        [
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            lease_bytes[0],
            lease_bytes[1],
            lease_bytes[2],
            lease_bytes[3],
            created_bytes[0],
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
        bytes.reserve(28);
        self.timestamp.serialize_into(bytes);
        self.window.serialize_into(bytes);
        self.lease.serialize_into(bytes);
        self.created.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 15]);
    }
}

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NotifyData([u8; 28]);
impl NotifyData {
    pub fn as_cc(&self) -> CrtcChange {
        fn do_the_parse(remaining: &[u8]) -> Result<CrtcChange, ParseError> {
            let (cc, remaining) = CrtcChange::try_parse(remaining)?;
            let _ = remaining;
            Ok(cc)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_oc(&self) -> OutputChange {
        fn do_the_parse(remaining: &[u8]) -> Result<OutputChange, ParseError> {
            let (oc, remaining) = OutputChange::try_parse(remaining)?;
            let _ = remaining;
            Ok(oc)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_op(&self) -> OutputProperty {
        fn do_the_parse(remaining: &[u8]) -> Result<OutputProperty, ParseError> {
            let (op, remaining) = OutputProperty::try_parse(remaining)?;
            let _ = remaining;
            Ok(op)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_pc(&self) -> ProviderChange {
        fn do_the_parse(remaining: &[u8]) -> Result<ProviderChange, ParseError> {
            let (pc, remaining) = ProviderChange::try_parse(remaining)?;
            let _ = remaining;
            Ok(pc)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_pp(&self) -> ProviderProperty {
        fn do_the_parse(remaining: &[u8]) -> Result<ProviderProperty, ParseError> {
            let (pp, remaining) = ProviderProperty::try_parse(remaining)?;
            let _ = remaining;
            Ok(pp)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_rc(&self) -> ResourceChange {
        fn do_the_parse(remaining: &[u8]) -> Result<ResourceChange, ParseError> {
            let (rc, remaining) = ResourceChange::try_parse(remaining)?;
            let _ = remaining;
            Ok(rc)
        }
        do_the_parse(&self.0).unwrap()
    }
    pub fn as_lc(&self) -> LeaseNotify {
        fn do_the_parse(remaining: &[u8]) -> Result<LeaseNotify, ParseError> {
            let (lc, remaining) = LeaseNotify::try_parse(remaining)?;
            let _ = remaining;
            Ok(lc)
        }
        do_the_parse(&self.0).unwrap()
    }
}
impl Serialize for NotifyData {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        self.0
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.0);
    }
}
impl TryParse for NotifyData {
    fn try_parse(value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let inner: [u8; 28] = value.get(..28)
            .ok_or(ParseError::InsufficientData)?
            .try_into()
            .unwrap();
        let result = NotifyData(inner);
        Ok((result, &value[28..]))
    }
}
impl From<CrtcChange> for NotifyData {
    fn from(cc: CrtcChange) -> Self {
        let cc_bytes = cc.serialize();
        Self(cc_bytes)
    }
}
impl From<OutputChange> for NotifyData {
    fn from(oc: OutputChange) -> Self {
        let oc_bytes = oc.serialize();
        Self(oc_bytes)
    }
}
impl From<OutputProperty> for NotifyData {
    fn from(op: OutputProperty) -> Self {
        let op_bytes = op.serialize();
        Self(op_bytes)
    }
}
impl From<ProviderChange> for NotifyData {
    fn from(pc: ProviderChange) -> Self {
        let pc_bytes = pc.serialize();
        Self(pc_bytes)
    }
}
impl From<ProviderProperty> for NotifyData {
    fn from(pp: ProviderProperty) -> Self {
        let pp_bytes = pp.serialize();
        Self(pp_bytes)
    }
}
impl From<ResourceChange> for NotifyData {
    fn from(rc: ResourceChange) -> Self {
        let rc_bytes = rc.serialize();
        Self(rc_bytes)
    }
}
impl From<LeaseNotify> for NotifyData {
    fn from(lc: LeaseNotify) -> Self {
        let lc_bytes = lc.serialize();
        Self(lc_bytes)
    }
}

/// Opcode for the Notify event
pub const NOTIFY_EVENT: u8 = 1;
#[derive(Clone, Copy)]
#[cfg_attr(feature = "extra-traits", derive(Debug))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NotifyEvent {
    pub response_type: u8,
    pub sub_code: Notify,
    pub sequence: u16,
    pub u: NotifyData,
}
impl_debug_if_no_extra_traits!(NotifyEvent, "NotifyEvent");
impl TryParse for NotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (sub_code, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (u, remaining) = NotifyData::try_parse(remaining)?;
        let sub_code = sub_code.into();
        let result = NotifyEvent { response_type, sub_code, sequence, u };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for NotifyEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let sub_code_bytes = u8::from(self.sub_code).serialize();
        let sequence_bytes = self.sequence.serialize();
        let u_bytes = self.u.serialize();
        [
            response_type_bytes[0],
            sub_code_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            u_bytes[0],
            u_bytes[1],
            u_bytes[2],
            u_bytes[3],
            u_bytes[4],
            u_bytes[5],
            u_bytes[6],
            u_bytes[7],
            u_bytes[8],
            u_bytes[9],
            u_bytes[10],
            u_bytes[11],
            u_bytes[12],
            u_bytes[13],
            u_bytes[14],
            u_bytes[15],
            u_bytes[16],
            u_bytes[17],
            u_bytes[18],
            u_bytes[19],
            u_bytes[20],
            u_bytes[21],
            u_bytes[22],
            u_bytes[23],
            u_bytes[24],
            u_bytes[25],
            u_bytes[26],
            u_bytes[27],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        u8::from(self.sub_code).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.u.serialize_into(bytes);
    }
}
impl From<&NotifyEvent> for [u8; 32] {
    fn from(input: &NotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sub_code_bytes = u8::from(input.sub_code).serialize();
        let sequence_bytes = input.sequence.serialize();
        let u_bytes = input.u.serialize();
        [
            response_type_bytes[0],
            sub_code_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            u_bytes[0],
            u_bytes[1],
            u_bytes[2],
            u_bytes[3],
            u_bytes[4],
            u_bytes[5],
            u_bytes[6],
            u_bytes[7],
            u_bytes[8],
            u_bytes[9],
            u_bytes[10],
            u_bytes[11],
            u_bytes[12],
            u_bytes[13],
            u_bytes[14],
            u_bytes[15],
            u_bytes[16],
            u_bytes[17],
            u_bytes[18],
            u_bytes[19],
            u_bytes[20],
            u_bytes[21],
            u_bytes[22],
            u_bytes[23],
            u_bytes[24],
            u_bytes[25],
            u_bytes[26],
            u_bytes[27],
        ]
    }
}
impl From<NotifyEvent> for [u8; 32] {
    fn from(input: NotifyEvent) -> Self {
        Self::from(&input)
    }
}

