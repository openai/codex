// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `DPMS` X11 extension.

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
pub const X11_EXTENSION_NAME: &str = "DPMS";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (1, 2);

/// Opcode for the GetVersion request
pub const GET_VERSION_REQUEST: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetVersionRequest {
    pub client_major_version: u16,
    pub client_minor_version: u16,
}
impl_debug_if_no_extra_traits!(GetVersionRequest, "GetVersionRequest");
impl GetVersionRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let client_major_version_bytes = self.client_major_version.serialize();
        let client_minor_version_bytes = self.client_minor_version.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_VERSION_REQUEST,
            0,
            0,
            client_major_version_bytes[0],
            client_major_version_bytes[1],
            client_minor_version_bytes[0],
            client_minor_version_bytes[1],
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
        let (client_major_version, remaining) = u16::try_parse(value)?;
        let (client_minor_version, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetVersionRequest {
            client_major_version,
            client_minor_version,
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
    pub sequence: u16,
    pub length: u32,
    pub server_major_version: u16,
    pub server_minor_version: u16,
}
impl_debug_if_no_extra_traits!(GetVersionReply, "GetVersionReply");
impl TryParse for GetVersionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (server_major_version, remaining) = u16::try_parse(remaining)?;
        let (server_minor_version, remaining) = u16::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetVersionReply { sequence, length, server_major_version, server_minor_version };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetVersionReply {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let server_major_version_bytes = self.server_major_version.serialize();
        let server_minor_version_bytes = self.server_minor_version.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            server_major_version_bytes[0],
            server_major_version_bytes[1],
            server_minor_version_bytes[0],
            server_minor_version_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.server_major_version.serialize_into(bytes);
        self.server_minor_version.serialize_into(bytes);
    }
}

/// Opcode for the Capable request
pub const CAPABLE_REQUEST: u8 = 1;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapableRequest;
impl_debug_if_no_extra_traits!(CapableRequest, "CapableRequest");
impl CapableRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            CAPABLE_REQUEST,
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
        if header.minor_opcode != CAPABLE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(CapableRequest
        )
    }
}
impl Request for CapableRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for CapableRequest {
    type Reply = CapableReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapableReply {
    pub sequence: u16,
    pub length: u32,
    pub capable: bool,
}
impl_debug_if_no_extra_traits!(CapableReply, "CapableReply");
impl TryParse for CapableReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (capable, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(23..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = CapableReply { sequence, length, capable };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for CapableReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let capable_bytes = self.capable.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            capable_bytes[0],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
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
        self.capable.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 23]);
    }
}

/// Opcode for the GetTimeouts request
pub const GET_TIMEOUTS_REQUEST: u8 = 2;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetTimeoutsRequest;
impl_debug_if_no_extra_traits!(GetTimeoutsRequest, "GetTimeoutsRequest");
impl GetTimeoutsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            GET_TIMEOUTS_REQUEST,
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
        if header.minor_opcode != GET_TIMEOUTS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(GetTimeoutsRequest
        )
    }
}
impl Request for GetTimeoutsRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetTimeoutsRequest {
    type Reply = GetTimeoutsReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetTimeoutsReply {
    pub sequence: u16,
    pub length: u32,
    pub standby_timeout: u16,
    pub suspend_timeout: u16,
    pub off_timeout: u16,
}
impl_debug_if_no_extra_traits!(GetTimeoutsReply, "GetTimeoutsReply");
impl TryParse for GetTimeoutsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (standby_timeout, remaining) = u16::try_parse(remaining)?;
        let (suspend_timeout, remaining) = u16::try_parse(remaining)?;
        let (off_timeout, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(18..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetTimeoutsReply { sequence, length, standby_timeout, suspend_timeout, off_timeout };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetTimeoutsReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let standby_timeout_bytes = self.standby_timeout.serialize();
        let suspend_timeout_bytes = self.suspend_timeout.serialize();
        let off_timeout_bytes = self.off_timeout.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            standby_timeout_bytes[0],
            standby_timeout_bytes[1],
            suspend_timeout_bytes[0],
            suspend_timeout_bytes[1],
            off_timeout_bytes[0],
            off_timeout_bytes[1],
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
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
        self.standby_timeout.serialize_into(bytes);
        self.suspend_timeout.serialize_into(bytes);
        self.off_timeout.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 18]);
    }
}

/// Opcode for the SetTimeouts request
pub const SET_TIMEOUTS_REQUEST: u8 = 3;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetTimeoutsRequest {
    pub standby_timeout: u16,
    pub suspend_timeout: u16,
    pub off_timeout: u16,
}
impl_debug_if_no_extra_traits!(SetTimeoutsRequest, "SetTimeoutsRequest");
impl SetTimeoutsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let standby_timeout_bytes = self.standby_timeout.serialize();
        let suspend_timeout_bytes = self.suspend_timeout.serialize();
        let off_timeout_bytes = self.off_timeout.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_TIMEOUTS_REQUEST,
            0,
            0,
            standby_timeout_bytes[0],
            standby_timeout_bytes[1],
            suspend_timeout_bytes[0],
            suspend_timeout_bytes[1],
            off_timeout_bytes[0],
            off_timeout_bytes[1],
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
        if header.minor_opcode != SET_TIMEOUTS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (standby_timeout, remaining) = u16::try_parse(value)?;
        let (suspend_timeout, remaining) = u16::try_parse(remaining)?;
        let (off_timeout, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(SetTimeoutsRequest {
            standby_timeout,
            suspend_timeout,
            off_timeout,
        })
    }
}
impl Request for SetTimeoutsRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SetTimeoutsRequest {
}

/// Opcode for the Enable request
pub const ENABLE_REQUEST: u8 = 4;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EnableRequest;
impl_debug_if_no_extra_traits!(EnableRequest, "EnableRequest");
impl EnableRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            ENABLE_REQUEST,
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
        if header.minor_opcode != ENABLE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(EnableRequest
        )
    }
}
impl Request for EnableRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for EnableRequest {
}

/// Opcode for the Disable request
pub const DISABLE_REQUEST: u8 = 5;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DisableRequest;
impl_debug_if_no_extra_traits!(DisableRequest, "DisableRequest");
impl DisableRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            DISABLE_REQUEST,
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
        if header.minor_opcode != DISABLE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(DisableRequest
        )
    }
}
impl Request for DisableRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DisableRequest {
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DPMSMode(u16);
impl DPMSMode {
    pub const ON: Self = Self(0);
    pub const STANDBY: Self = Self(1);
    pub const SUSPEND: Self = Self(2);
    pub const OFF: Self = Self(3);
}
impl From<DPMSMode> for u16 {
    #[inline]
    fn from(input: DPMSMode) -> Self {
        input.0
    }
}
impl From<DPMSMode> for Option<u16> {
    #[inline]
    fn from(input: DPMSMode) -> Self {
        Some(input.0)
    }
}
impl From<DPMSMode> for u32 {
    #[inline]
    fn from(input: DPMSMode) -> Self {
        u32::from(input.0)
    }
}
impl From<DPMSMode> for Option<u32> {
    #[inline]
    fn from(input: DPMSMode) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for DPMSMode {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for DPMSMode {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for DPMSMode  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::ON.0.into(), "ON", "On"),
            (Self::STANDBY.0.into(), "STANDBY", "Standby"),
            (Self::SUSPEND.0.into(), "SUSPEND", "Suspend"),
            (Self::OFF.0.into(), "OFF", "Off"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the ForceLevel request
pub const FORCE_LEVEL_REQUEST: u8 = 6;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ForceLevelRequest {
    pub power_level: DPMSMode,
}
impl_debug_if_no_extra_traits!(ForceLevelRequest, "ForceLevelRequest");
impl ForceLevelRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let power_level_bytes = u16::from(self.power_level).serialize();
        let mut request0 = vec![
            major_opcode,
            FORCE_LEVEL_REQUEST,
            0,
            0,
            power_level_bytes[0],
            power_level_bytes[1],
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
        if header.minor_opcode != FORCE_LEVEL_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (power_level, remaining) = u16::try_parse(value)?;
        let power_level = power_level.into();
        let _ = remaining;
        Ok(ForceLevelRequest {
            power_level,
        })
    }
}
impl Request for ForceLevelRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for ForceLevelRequest {
}

/// Opcode for the Info request
pub const INFO_REQUEST: u8 = 7;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InfoRequest;
impl_debug_if_no_extra_traits!(InfoRequest, "InfoRequest");
impl InfoRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            INFO_REQUEST,
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
        if header.minor_opcode != INFO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(InfoRequest
        )
    }
}
impl Request for InfoRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for InfoRequest {
    type Reply = InfoReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InfoReply {
    pub sequence: u16,
    pub length: u32,
    pub power_level: DPMSMode,
    pub state: bool,
}
impl_debug_if_no_extra_traits!(InfoReply, "InfoReply");
impl TryParse for InfoReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (power_level, remaining) = u16::try_parse(remaining)?;
        let (state, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(21..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let power_level = power_level.into();
        let result = InfoReply { sequence, length, power_level, state };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for InfoReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let power_level_bytes = u16::from(self.power_level).serialize();
        let state_bytes = self.state.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            power_level_bytes[0],
            power_level_bytes[1],
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
            0,
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
        u16::from(self.power_level).serialize_into(bytes);
        self.state.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 21]);
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventMask(u32);
impl EventMask {
    pub const INFO_NOTIFY: Self = Self(1 << 0);
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
            (Self::INFO_NOTIFY.0, "INFO_NOTIFY", "InfoNotify"),
        ];
        pretty_print_bitmask(fmt, self.0, &variants)
    }
}
bitmask_binop!(EventMask, u32);

/// Opcode for the SelectInput request
pub const SELECT_INPUT_REQUEST: u8 = 8;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectInputRequest {
    pub event_mask: EventMask,
}
impl_debug_if_no_extra_traits!(SelectInputRequest, "SelectInputRequest");
impl SelectInputRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let event_mask_bytes = u32::from(self.event_mask).serialize();
        let mut request0 = vec![
            major_opcode,
            SELECT_INPUT_REQUEST,
            0,
            0,
            event_mask_bytes[0],
            event_mask_bytes[1],
            event_mask_bytes[2],
            event_mask_bytes[3],
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
        let (event_mask, remaining) = u32::try_parse(value)?;
        let event_mask = event_mask.into();
        let _ = remaining;
        Ok(SelectInputRequest {
            event_mask,
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

/// Opcode for the InfoNotify event
pub const INFO_NOTIFY_EVENT: u16 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InfoNotifyEvent {
    pub response_type: u8,
    pub extension: u8,
    pub sequence: u16,
    pub length: u32,
    pub event_type: u16,
    pub timestamp: xproto::Timestamp,
    pub power_level: DPMSMode,
    pub state: bool,
}
impl_debug_if_no_extra_traits!(InfoNotifyEvent, "InfoNotifyEvent");
impl TryParse for InfoNotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (extension, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (event_type, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (power_level, remaining) = u16::try_parse(remaining)?;
        let (state, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(21..).ok_or(ParseError::InsufficientData)?;
        let power_level = power_level.into();
        let result = InfoNotifyEvent { response_type, extension, sequence, length, event_type, timestamp, power_level, state };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for InfoNotifyEvent {
    type Bytes = [u8; 40];
    fn serialize(&self) -> [u8; 40] {
        let response_type_bytes = self.response_type.serialize();
        let extension_bytes = self.extension.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let event_type_bytes = self.event_type.serialize();
        let timestamp_bytes = self.timestamp.serialize();
        let power_level_bytes = u16::from(self.power_level).serialize();
        let state_bytes = self.state.serialize();
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
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            power_level_bytes[0],
            power_level_bytes[1],
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
            0,
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
        bytes.reserve(40);
        self.response_type.serialize_into(bytes);
        self.extension.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.event_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.timestamp.serialize_into(bytes);
        u16::from(self.power_level).serialize_into(bytes);
        self.state.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 21]);
    }
}

