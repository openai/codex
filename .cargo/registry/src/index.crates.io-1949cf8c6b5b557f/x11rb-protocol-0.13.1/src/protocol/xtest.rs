// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Test` X11 extension.

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
pub const X11_EXTENSION_NAME: &str = "XTEST";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (2, 2);

/// Opcode for the GetVersion request
pub const GET_VERSION_REQUEST: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetVersionRequest {
    pub major_version: u8,
    pub minor_version: u16,
}
impl_debug_if_no_extra_traits!(GetVersionRequest, "GetVersionRequest");
impl GetVersionRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let major_version_bytes = self.major_version.serialize();
        let minor_version_bytes = self.minor_version.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_VERSION_REQUEST,
            0,
            0,
            major_version_bytes[0],
            0,
            minor_version_bytes[0],
            minor_version_bytes[1],
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
        if header.minor_opcode != GET_VERSION_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (major_version, remaining) = u8::try_parse(value)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (minor_version, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetVersionRequest {
            major_version,
            minor_version,
        })
    }
}
impl Request for GetVersionRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetVersionRequest {
    type Reply = GetVersionReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetVersionReply {
    pub major_version: u8,
    pub sequence: u16,
    pub length: u32,
    pub minor_version: u16,
}
impl_debug_if_no_extra_traits!(GetVersionReply, "GetVersionReply");
impl TryParse for GetVersionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (major_version, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (minor_version, remaining) = u16::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetVersionReply { major_version, sequence, length, minor_version };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetVersionReply {
    type Bytes = [u8; 10];
    fn serialize(&self) -> [u8; 10] {
        let response_type_bytes = &[1];
        let major_version_bytes = self.major_version.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let minor_version_bytes = self.minor_version.serialize();
        [
            response_type_bytes[0],
            major_version_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            minor_version_bytes[0],
            minor_version_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(10);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.major_version.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.minor_version.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cursor(bool);
impl Cursor {
    pub const NONE: Self = Self(false);
    pub const CURRENT: Self = Self(true);
}
impl From<Cursor> for bool {
    #[inline]
    fn from(input: Cursor) -> Self {
        input.0
    }
}
impl From<Cursor> for Option<bool> {
    #[inline]
    fn from(input: Cursor) -> Self {
        Some(input.0)
    }
}
impl From<Cursor> for u8 {
    #[inline]
    fn from(input: Cursor) -> Self {
        u8::from(input.0)
    }
}
impl From<Cursor> for Option<u8> {
    #[inline]
    fn from(input: Cursor) -> Self {
        Some(u8::from(input.0))
    }
}
impl From<Cursor> for u16 {
    #[inline]
    fn from(input: Cursor) -> Self {
        u16::from(input.0)
    }
}
impl From<Cursor> for Option<u16> {
    #[inline]
    fn from(input: Cursor) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Cursor> for u32 {
    #[inline]
    fn from(input: Cursor) -> Self {
        u32::from(input.0)
    }
}
impl From<Cursor> for Option<u32> {
    #[inline]
    fn from(input: Cursor) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<bool> for Cursor {
    #[inline]
    fn from(value: bool) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Cursor  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NONE.0.into(), "NONE", "None"),
            (Self::CURRENT.0.into(), "CURRENT", "Current"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the CompareCursor request
pub const COMPARE_CURSOR_REQUEST: u8 = 1;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CompareCursorRequest {
    pub window: xproto::Window,
    pub cursor: xproto::Cursor,
}
impl_debug_if_no_extra_traits!(CompareCursorRequest, "CompareCursorRequest");
impl CompareCursorRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let cursor_bytes = self.cursor.serialize();
        let mut request0 = vec![
            major_opcode,
            COMPARE_CURSOR_REQUEST,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.minor_opcode != COMPARE_CURSOR_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (cursor, remaining) = xproto::Cursor::try_parse(remaining)?;
        let _ = remaining;
        Ok(CompareCursorRequest {
            window,
            cursor,
        })
    }
}
impl Request for CompareCursorRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for CompareCursorRequest {
    type Reply = CompareCursorReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CompareCursorReply {
    pub same: bool,
    pub sequence: u16,
    pub length: u32,
}
impl_debug_if_no_extra_traits!(CompareCursorReply, "CompareCursorReply");
impl TryParse for CompareCursorReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (same, remaining) = bool::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = CompareCursorReply { same, sequence, length };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for CompareCursorReply {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let response_type_bytes = &[1];
        let same_bytes = self.same.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        [
            response_type_bytes[0],
            same_bytes[0],
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
        self.same.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
    }
}

/// Opcode for the FakeInput request
pub const FAKE_INPUT_REQUEST: u8 = 2;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FakeInputRequest {
    pub type_: u8,
    pub detail: u8,
    pub time: u32,
    pub root: xproto::Window,
    pub root_x: i16,
    pub root_y: i16,
    pub deviceid: u8,
}
impl_debug_if_no_extra_traits!(FakeInputRequest, "FakeInputRequest");
impl FakeInputRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let type_bytes = self.type_.serialize();
        let detail_bytes = self.detail.serialize();
        let time_bytes = self.time.serialize();
        let root_bytes = self.root.serialize();
        let root_x_bytes = self.root_x.serialize();
        let root_y_bytes = self.root_y.serialize();
        let deviceid_bytes = self.deviceid.serialize();
        let mut request0 = vec![
            major_opcode,
            FAKE_INPUT_REQUEST,
            0,
            0,
            type_bytes[0],
            detail_bytes[0],
            0,
            0,
            time_bytes[0],
            time_bytes[1],
            time_bytes[2],
            time_bytes[3],
            root_bytes[0],
            root_bytes[1],
            root_bytes[2],
            root_bytes[3],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            root_x_bytes[0],
            root_x_bytes[1],
            root_y_bytes[0],
            root_y_bytes[1],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            deviceid_bytes[0],
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
        if header.minor_opcode != FAKE_INPUT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (type_, remaining) = u8::try_parse(value)?;
        let (detail, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (time, remaining) = u32::try_parse(remaining)?;
        let (root, remaining) = xproto::Window::try_parse(remaining)?;
        let remaining = remaining.get(8..).ok_or(ParseError::InsufficientData)?;
        let (root_x, remaining) = i16::try_parse(remaining)?;
        let (root_y, remaining) = i16::try_parse(remaining)?;
        let remaining = remaining.get(7..).ok_or(ParseError::InsufficientData)?;
        let (deviceid, remaining) = u8::try_parse(remaining)?;
        let _ = remaining;
        Ok(FakeInputRequest {
            type_,
            detail,
            time,
            root,
            root_x,
            root_y,
            deviceid,
        })
    }
}
impl Request for FakeInputRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for FakeInputRequest {
}

/// Opcode for the GrabControl request
pub const GRAB_CONTROL_REQUEST: u8 = 3;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GrabControlRequest {
    pub impervious: bool,
}
impl_debug_if_no_extra_traits!(GrabControlRequest, "GrabControlRequest");
impl GrabControlRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let impervious_bytes = self.impervious.serialize();
        let mut request0 = vec![
            major_opcode,
            GRAB_CONTROL_REQUEST,
            0,
            0,
            impervious_bytes[0],
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
        if header.minor_opcode != GRAB_CONTROL_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (impervious, remaining) = bool::try_parse(value)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(GrabControlRequest {
            impervious,
        })
    }
}
impl Request for GrabControlRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for GrabControlRequest {
}

