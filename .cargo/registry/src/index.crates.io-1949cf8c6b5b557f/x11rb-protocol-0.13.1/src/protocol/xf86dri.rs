// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `XF86Dri` X11 extension.

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

/// The X11 name of the extension for QueryExtension
pub const X11_EXTENSION_NAME: &str = "XFree86-DRI";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (4, 1);

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DrmClipRect {
    pub x1: i16,
    pub y1: i16,
    pub x2: i16,
    pub x3: i16,
}
impl_debug_if_no_extra_traits!(DrmClipRect, "DrmClipRect");
impl TryParse for DrmClipRect {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (x1, remaining) = i16::try_parse(remaining)?;
        let (y1, remaining) = i16::try_parse(remaining)?;
        let (x2, remaining) = i16::try_parse(remaining)?;
        let (x3, remaining) = i16::try_parse(remaining)?;
        let result = DrmClipRect { x1, y1, x2, x3 };
        Ok((result, remaining))
    }
}
impl Serialize for DrmClipRect {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let x1_bytes = self.x1.serialize();
        let y1_bytes = self.y1.serialize();
        let x2_bytes = self.x2.serialize();
        let x3_bytes = self.x3.serialize();
        [
            x1_bytes[0],
            x1_bytes[1],
            y1_bytes[0],
            y1_bytes[1],
            x2_bytes[0],
            x2_bytes[1],
            x3_bytes[0],
            x3_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.x1.serialize_into(bytes);
        self.y1.serialize_into(bytes);
        self.x2.serialize_into(bytes);
        self.x3.serialize_into(bytes);
    }
}

/// Opcode for the QueryVersion request
pub const QUERY_VERSION_REQUEST: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryVersionRequest;
impl_debug_if_no_extra_traits!(QueryVersionRequest, "QueryVersionRequest");
impl QueryVersionRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            QUERY_VERSION_REQUEST,
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
        if header.minor_opcode != QUERY_VERSION_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(QueryVersionRequest
        )
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
    pub dri_major_version: u16,
    pub dri_minor_version: u16,
    pub dri_minor_patch: u32,
}
impl_debug_if_no_extra_traits!(QueryVersionReply, "QueryVersionReply");
impl TryParse for QueryVersionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (dri_major_version, remaining) = u16::try_parse(remaining)?;
        let (dri_minor_version, remaining) = u16::try_parse(remaining)?;
        let (dri_minor_patch, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryVersionReply { sequence, length, dri_major_version, dri_minor_version, dri_minor_patch };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryVersionReply {
    type Bytes = [u8; 16];
    fn serialize(&self) -> [u8; 16] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let dri_major_version_bytes = self.dri_major_version.serialize();
        let dri_minor_version_bytes = self.dri_minor_version.serialize();
        let dri_minor_patch_bytes = self.dri_minor_patch.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            dri_major_version_bytes[0],
            dri_major_version_bytes[1],
            dri_minor_version_bytes[0],
            dri_minor_version_bytes[1],
            dri_minor_patch_bytes[0],
            dri_minor_patch_bytes[1],
            dri_minor_patch_bytes[2],
            dri_minor_patch_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.dri_major_version.serialize_into(bytes);
        self.dri_minor_version.serialize_into(bytes);
        self.dri_minor_patch.serialize_into(bytes);
    }
}

/// Opcode for the QueryDirectRenderingCapable request
pub const QUERY_DIRECT_RENDERING_CAPABLE_REQUEST: u8 = 1;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryDirectRenderingCapableRequest {
    pub screen: u32,
}
impl_debug_if_no_extra_traits!(QueryDirectRenderingCapableRequest, "QueryDirectRenderingCapableRequest");
impl QueryDirectRenderingCapableRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let screen_bytes = self.screen.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_DIRECT_RENDERING_CAPABLE_REQUEST,
            0,
            0,
            screen_bytes[0],
            screen_bytes[1],
            screen_bytes[2],
            screen_bytes[3],
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
        if header.minor_opcode != QUERY_DIRECT_RENDERING_CAPABLE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (screen, remaining) = u32::try_parse(value)?;
        let _ = remaining;
        Ok(QueryDirectRenderingCapableRequest {
            screen,
        })
    }
}
impl Request for QueryDirectRenderingCapableRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryDirectRenderingCapableRequest {
    type Reply = QueryDirectRenderingCapableReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryDirectRenderingCapableReply {
    pub sequence: u16,
    pub length: u32,
    pub is_capable: bool,
}
impl_debug_if_no_extra_traits!(QueryDirectRenderingCapableReply, "QueryDirectRenderingCapableReply");
impl TryParse for QueryDirectRenderingCapableReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (is_capable, remaining) = bool::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryDirectRenderingCapableReply { sequence, length, is_capable };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryDirectRenderingCapableReply {
    type Bytes = [u8; 9];
    fn serialize(&self) -> [u8; 9] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let is_capable_bytes = self.is_capable.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            is_capable_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(9);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.is_capable.serialize_into(bytes);
    }
}

/// Opcode for the OpenConnection request
pub const OPEN_CONNECTION_REQUEST: u8 = 2;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpenConnectionRequest {
    pub screen: u32,
}
impl_debug_if_no_extra_traits!(OpenConnectionRequest, "OpenConnectionRequest");
impl OpenConnectionRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let screen_bytes = self.screen.serialize();
        let mut request0 = vec![
            major_opcode,
            OPEN_CONNECTION_REQUEST,
            0,
            0,
            screen_bytes[0],
            screen_bytes[1],
            screen_bytes[2],
            screen_bytes[3],
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
        if header.minor_opcode != OPEN_CONNECTION_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (screen, remaining) = u32::try_parse(value)?;
        let _ = remaining;
        Ok(OpenConnectionRequest {
            screen,
        })
    }
}
impl Request for OpenConnectionRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for OpenConnectionRequest {
    type Reply = OpenConnectionReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpenConnectionReply {
    pub sequence: u16,
    pub length: u32,
    pub sarea_handle_low: u32,
    pub sarea_handle_high: u32,
    pub bus_id: Vec<u8>,
}
impl_debug_if_no_extra_traits!(OpenConnectionReply, "OpenConnectionReply");
impl TryParse for OpenConnectionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (sarea_handle_low, remaining) = u32::try_parse(remaining)?;
        let (sarea_handle_high, remaining) = u32::try_parse(remaining)?;
        let (bus_id_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(12..).ok_or(ParseError::InsufficientData)?;
        let (bus_id, remaining) = crate::x11_utils::parse_u8_list(remaining, bus_id_len.try_to_usize()?)?;
        let bus_id = bus_id.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = OpenConnectionReply { sequence, length, sarea_handle_low, sarea_handle_high, bus_id };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for OpenConnectionReply {
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
        self.sarea_handle_low.serialize_into(bytes);
        self.sarea_handle_high.serialize_into(bytes);
        let bus_id_len = u32::try_from(self.bus_id.len()).expect("`bus_id` has too many elements");
        bus_id_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 12]);
        bytes.extend_from_slice(&self.bus_id);
    }
}
impl OpenConnectionReply {
    /// Get the value of the `bus_id_len` field.
    ///
    /// The `bus_id_len` field is used as the length field of the `bus_id` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn bus_id_len(&self) -> u32 {
        self.bus_id.len()
            .try_into().unwrap()
    }
}

/// Opcode for the CloseConnection request
pub const CLOSE_CONNECTION_REQUEST: u8 = 3;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CloseConnectionRequest {
    pub screen: u32,
}
impl_debug_if_no_extra_traits!(CloseConnectionRequest, "CloseConnectionRequest");
impl CloseConnectionRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let screen_bytes = self.screen.serialize();
        let mut request0 = vec![
            major_opcode,
            CLOSE_CONNECTION_REQUEST,
            0,
            0,
            screen_bytes[0],
            screen_bytes[1],
            screen_bytes[2],
            screen_bytes[3],
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
        if header.minor_opcode != CLOSE_CONNECTION_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (screen, remaining) = u32::try_parse(value)?;
        let _ = remaining;
        Ok(CloseConnectionRequest {
            screen,
        })
    }
}
impl Request for CloseConnectionRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CloseConnectionRequest {
}

/// Opcode for the GetClientDriverName request
pub const GET_CLIENT_DRIVER_NAME_REQUEST: u8 = 4;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetClientDriverNameRequest {
    pub screen: u32,
}
impl_debug_if_no_extra_traits!(GetClientDriverNameRequest, "GetClientDriverNameRequest");
impl GetClientDriverNameRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let screen_bytes = self.screen.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_CLIENT_DRIVER_NAME_REQUEST,
            0,
            0,
            screen_bytes[0],
            screen_bytes[1],
            screen_bytes[2],
            screen_bytes[3],
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
        if header.minor_opcode != GET_CLIENT_DRIVER_NAME_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (screen, remaining) = u32::try_parse(value)?;
        let _ = remaining;
        Ok(GetClientDriverNameRequest {
            screen,
        })
    }
}
impl Request for GetClientDriverNameRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetClientDriverNameRequest {
    type Reply = GetClientDriverNameReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetClientDriverNameReply {
    pub sequence: u16,
    pub length: u32,
    pub client_driver_major_version: u32,
    pub client_driver_minor_version: u32,
    pub client_driver_patch_version: u32,
    pub client_driver_name: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetClientDriverNameReply, "GetClientDriverNameReply");
impl TryParse for GetClientDriverNameReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (client_driver_major_version, remaining) = u32::try_parse(remaining)?;
        let (client_driver_minor_version, remaining) = u32::try_parse(remaining)?;
        let (client_driver_patch_version, remaining) = u32::try_parse(remaining)?;
        let (client_driver_name_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(8..).ok_or(ParseError::InsufficientData)?;
        let (client_driver_name, remaining) = crate::x11_utils::parse_u8_list(remaining, client_driver_name_len.try_to_usize()?)?;
        let client_driver_name = client_driver_name.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetClientDriverNameReply { sequence, length, client_driver_major_version, client_driver_minor_version, client_driver_patch_version, client_driver_name };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetClientDriverNameReply {
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
        self.client_driver_major_version.serialize_into(bytes);
        self.client_driver_minor_version.serialize_into(bytes);
        self.client_driver_patch_version.serialize_into(bytes);
        let client_driver_name_len = u32::try_from(self.client_driver_name.len()).expect("`client_driver_name` has too many elements");
        client_driver_name_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 8]);
        bytes.extend_from_slice(&self.client_driver_name);
    }
}
impl GetClientDriverNameReply {
    /// Get the value of the `client_driver_name_len` field.
    ///
    /// The `client_driver_name_len` field is used as the length field of the `client_driver_name` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn client_driver_name_len(&self) -> u32 {
        self.client_driver_name.len()
            .try_into().unwrap()
    }
}

/// Opcode for the CreateContext request
pub const CREATE_CONTEXT_REQUEST: u8 = 5;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateContextRequest {
    pub screen: u32,
    pub visual: u32,
    pub context: u32,
}
impl_debug_if_no_extra_traits!(CreateContextRequest, "CreateContextRequest");
impl CreateContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let screen_bytes = self.screen.serialize();
        let visual_bytes = self.visual.serialize();
        let context_bytes = self.context.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_CONTEXT_REQUEST,
            0,
            0,
            screen_bytes[0],
            screen_bytes[1],
            screen_bytes[2],
            screen_bytes[3],
            visual_bytes[0],
            visual_bytes[1],
            visual_bytes[2],
            visual_bytes[3],
            context_bytes[0],
            context_bytes[1],
            context_bytes[2],
            context_bytes[3],
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
        if header.minor_opcode != CREATE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (screen, remaining) = u32::try_parse(value)?;
        let (visual, remaining) = u32::try_parse(remaining)?;
        let (context, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(CreateContextRequest {
            screen,
            visual,
            context,
        })
    }
}
impl Request for CreateContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for CreateContextRequest {
    type Reply = CreateContextReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateContextReply {
    pub sequence: u16,
    pub length: u32,
    pub hw_context: u32,
}
impl_debug_if_no_extra_traits!(CreateContextReply, "CreateContextReply");
impl TryParse for CreateContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (hw_context, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = CreateContextReply { sequence, length, hw_context };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for CreateContextReply {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let hw_context_bytes = self.hw_context.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            hw_context_bytes[0],
            hw_context_bytes[1],
            hw_context_bytes[2],
            hw_context_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.hw_context.serialize_into(bytes);
    }
}

/// Opcode for the DestroyContext request
pub const DESTROY_CONTEXT_REQUEST: u8 = 6;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DestroyContextRequest {
    pub screen: u32,
    pub context: u32,
}
impl_debug_if_no_extra_traits!(DestroyContextRequest, "DestroyContextRequest");
impl DestroyContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let screen_bytes = self.screen.serialize();
        let context_bytes = self.context.serialize();
        let mut request0 = vec![
            major_opcode,
            DESTROY_CONTEXT_REQUEST,
            0,
            0,
            screen_bytes[0],
            screen_bytes[1],
            screen_bytes[2],
            screen_bytes[3],
            context_bytes[0],
            context_bytes[1],
            context_bytes[2],
            context_bytes[3],
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
        if header.minor_opcode != DESTROY_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (screen, remaining) = u32::try_parse(value)?;
        let (context, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(DestroyContextRequest {
            screen,
            context,
        })
    }
}
impl Request for DestroyContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DestroyContextRequest {
}

/// Opcode for the CreateDrawable request
pub const CREATE_DRAWABLE_REQUEST: u8 = 7;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateDrawableRequest {
    pub screen: u32,
    pub drawable: u32,
}
impl_debug_if_no_extra_traits!(CreateDrawableRequest, "CreateDrawableRequest");
impl CreateDrawableRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let screen_bytes = self.screen.serialize();
        let drawable_bytes = self.drawable.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_DRAWABLE_REQUEST,
            0,
            0,
            screen_bytes[0],
            screen_bytes[1],
            screen_bytes[2],
            screen_bytes[3],
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
        if header.minor_opcode != CREATE_DRAWABLE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (screen, remaining) = u32::try_parse(value)?;
        let (drawable, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(CreateDrawableRequest {
            screen,
            drawable,
        })
    }
}
impl Request for CreateDrawableRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for CreateDrawableRequest {
    type Reply = CreateDrawableReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateDrawableReply {
    pub sequence: u16,
    pub length: u32,
    pub hw_drawable_handle: u32,
}
impl_debug_if_no_extra_traits!(CreateDrawableReply, "CreateDrawableReply");
impl TryParse for CreateDrawableReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (hw_drawable_handle, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = CreateDrawableReply { sequence, length, hw_drawable_handle };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for CreateDrawableReply {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let hw_drawable_handle_bytes = self.hw_drawable_handle.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            hw_drawable_handle_bytes[0],
            hw_drawable_handle_bytes[1],
            hw_drawable_handle_bytes[2],
            hw_drawable_handle_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.hw_drawable_handle.serialize_into(bytes);
    }
}

/// Opcode for the DestroyDrawable request
pub const DESTROY_DRAWABLE_REQUEST: u8 = 8;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DestroyDrawableRequest {
    pub screen: u32,
    pub drawable: u32,
}
impl_debug_if_no_extra_traits!(DestroyDrawableRequest, "DestroyDrawableRequest");
impl DestroyDrawableRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let screen_bytes = self.screen.serialize();
        let drawable_bytes = self.drawable.serialize();
        let mut request0 = vec![
            major_opcode,
            DESTROY_DRAWABLE_REQUEST,
            0,
            0,
            screen_bytes[0],
            screen_bytes[1],
            screen_bytes[2],
            screen_bytes[3],
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
        if header.minor_opcode != DESTROY_DRAWABLE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (screen, remaining) = u32::try_parse(value)?;
        let (drawable, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(DestroyDrawableRequest {
            screen,
            drawable,
        })
    }
}
impl Request for DestroyDrawableRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DestroyDrawableRequest {
}

/// Opcode for the GetDrawableInfo request
pub const GET_DRAWABLE_INFO_REQUEST: u8 = 9;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetDrawableInfoRequest {
    pub screen: u32,
    pub drawable: u32,
}
impl_debug_if_no_extra_traits!(GetDrawableInfoRequest, "GetDrawableInfoRequest");
impl GetDrawableInfoRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let screen_bytes = self.screen.serialize();
        let drawable_bytes = self.drawable.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_DRAWABLE_INFO_REQUEST,
            0,
            0,
            screen_bytes[0],
            screen_bytes[1],
            screen_bytes[2],
            screen_bytes[3],
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
        if header.minor_opcode != GET_DRAWABLE_INFO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (screen, remaining) = u32::try_parse(value)?;
        let (drawable, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetDrawableInfoRequest {
            screen,
            drawable,
        })
    }
}
impl Request for GetDrawableInfoRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetDrawableInfoRequest {
    type Reply = GetDrawableInfoReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetDrawableInfoReply {
    pub sequence: u16,
    pub length: u32,
    pub drawable_table_index: u32,
    pub drawable_table_stamp: u32,
    pub drawable_origin_x: i16,
    pub drawable_origin_y: i16,
    pub drawable_size_w: i16,
    pub drawable_size_h: i16,
    pub back_x: i16,
    pub back_y: i16,
    pub clip_rects: Vec<DrmClipRect>,
    pub back_clip_rects: Vec<DrmClipRect>,
}
impl_debug_if_no_extra_traits!(GetDrawableInfoReply, "GetDrawableInfoReply");
impl TryParse for GetDrawableInfoReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (drawable_table_index, remaining) = u32::try_parse(remaining)?;
        let (drawable_table_stamp, remaining) = u32::try_parse(remaining)?;
        let (drawable_origin_x, remaining) = i16::try_parse(remaining)?;
        let (drawable_origin_y, remaining) = i16::try_parse(remaining)?;
        let (drawable_size_w, remaining) = i16::try_parse(remaining)?;
        let (drawable_size_h, remaining) = i16::try_parse(remaining)?;
        let (num_clip_rects, remaining) = u32::try_parse(remaining)?;
        let (back_x, remaining) = i16::try_parse(remaining)?;
        let (back_y, remaining) = i16::try_parse(remaining)?;
        let (num_back_clip_rects, remaining) = u32::try_parse(remaining)?;
        let (clip_rects, remaining) = crate::x11_utils::parse_list::<DrmClipRect>(remaining, num_clip_rects.try_to_usize()?)?;
        let (back_clip_rects, remaining) = crate::x11_utils::parse_list::<DrmClipRect>(remaining, num_back_clip_rects.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetDrawableInfoReply { sequence, length, drawable_table_index, drawable_table_stamp, drawable_origin_x, drawable_origin_y, drawable_size_w, drawable_size_h, back_x, back_y, clip_rects, back_clip_rects };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetDrawableInfoReply {
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
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.drawable_table_index.serialize_into(bytes);
        self.drawable_table_stamp.serialize_into(bytes);
        self.drawable_origin_x.serialize_into(bytes);
        self.drawable_origin_y.serialize_into(bytes);
        self.drawable_size_w.serialize_into(bytes);
        self.drawable_size_h.serialize_into(bytes);
        let num_clip_rects = u32::try_from(self.clip_rects.len()).expect("`clip_rects` has too many elements");
        num_clip_rects.serialize_into(bytes);
        self.back_x.serialize_into(bytes);
        self.back_y.serialize_into(bytes);
        let num_back_clip_rects = u32::try_from(self.back_clip_rects.len()).expect("`back_clip_rects` has too many elements");
        num_back_clip_rects.serialize_into(bytes);
        self.clip_rects.serialize_into(bytes);
        self.back_clip_rects.serialize_into(bytes);
    }
}
impl GetDrawableInfoReply {
    /// Get the value of the `num_clip_rects` field.
    ///
    /// The `num_clip_rects` field is used as the length field of the `clip_rects` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_clip_rects(&self) -> u32 {
        self.clip_rects.len()
            .try_into().unwrap()
    }
    /// Get the value of the `num_back_clip_rects` field.
    ///
    /// The `num_back_clip_rects` field is used as the length field of the `back_clip_rects` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_back_clip_rects(&self) -> u32 {
        self.back_clip_rects.len()
            .try_into().unwrap()
    }
}

/// Opcode for the GetDeviceInfo request
pub const GET_DEVICE_INFO_REQUEST: u8 = 10;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetDeviceInfoRequest {
    pub screen: u32,
}
impl_debug_if_no_extra_traits!(GetDeviceInfoRequest, "GetDeviceInfoRequest");
impl GetDeviceInfoRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let screen_bytes = self.screen.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_DEVICE_INFO_REQUEST,
            0,
            0,
            screen_bytes[0],
            screen_bytes[1],
            screen_bytes[2],
            screen_bytes[3],
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
        if header.minor_opcode != GET_DEVICE_INFO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (screen, remaining) = u32::try_parse(value)?;
        let _ = remaining;
        Ok(GetDeviceInfoRequest {
            screen,
        })
    }
}
impl Request for GetDeviceInfoRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetDeviceInfoRequest {
    type Reply = GetDeviceInfoReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetDeviceInfoReply {
    pub sequence: u16,
    pub length: u32,
    pub framebuffer_handle_low: u32,
    pub framebuffer_handle_high: u32,
    pub framebuffer_origin_offset: u32,
    pub framebuffer_size: u32,
    pub framebuffer_stride: u32,
    pub device_private: Vec<u32>,
}
impl_debug_if_no_extra_traits!(GetDeviceInfoReply, "GetDeviceInfoReply");
impl TryParse for GetDeviceInfoReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (framebuffer_handle_low, remaining) = u32::try_parse(remaining)?;
        let (framebuffer_handle_high, remaining) = u32::try_parse(remaining)?;
        let (framebuffer_origin_offset, remaining) = u32::try_parse(remaining)?;
        let (framebuffer_size, remaining) = u32::try_parse(remaining)?;
        let (framebuffer_stride, remaining) = u32::try_parse(remaining)?;
        let (device_private_size, remaining) = u32::try_parse(remaining)?;
        let (device_private, remaining) = crate::x11_utils::parse_list::<u32>(remaining, device_private_size.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetDeviceInfoReply { sequence, length, framebuffer_handle_low, framebuffer_handle_high, framebuffer_origin_offset, framebuffer_size, framebuffer_stride, device_private };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetDeviceInfoReply {
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
        self.framebuffer_handle_low.serialize_into(bytes);
        self.framebuffer_handle_high.serialize_into(bytes);
        self.framebuffer_origin_offset.serialize_into(bytes);
        self.framebuffer_size.serialize_into(bytes);
        self.framebuffer_stride.serialize_into(bytes);
        let device_private_size = u32::try_from(self.device_private.len()).expect("`device_private` has too many elements");
        device_private_size.serialize_into(bytes);
        self.device_private.serialize_into(bytes);
    }
}
impl GetDeviceInfoReply {
    /// Get the value of the `device_private_size` field.
    ///
    /// The `device_private_size` field is used as the length field of the `device_private` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn device_private_size(&self) -> u32 {
        self.device_private.len()
            .try_into().unwrap()
    }
}

/// Opcode for the AuthConnection request
pub const AUTH_CONNECTION_REQUEST: u8 = 11;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AuthConnectionRequest {
    pub screen: u32,
    pub magic: u32,
}
impl_debug_if_no_extra_traits!(AuthConnectionRequest, "AuthConnectionRequest");
impl AuthConnectionRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let screen_bytes = self.screen.serialize();
        let magic_bytes = self.magic.serialize();
        let mut request0 = vec![
            major_opcode,
            AUTH_CONNECTION_REQUEST,
            0,
            0,
            screen_bytes[0],
            screen_bytes[1],
            screen_bytes[2],
            screen_bytes[3],
            magic_bytes[0],
            magic_bytes[1],
            magic_bytes[2],
            magic_bytes[3],
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
        if header.minor_opcode != AUTH_CONNECTION_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (screen, remaining) = u32::try_parse(value)?;
        let (magic, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(AuthConnectionRequest {
            screen,
            magic,
        })
    }
}
impl Request for AuthConnectionRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for AuthConnectionRequest {
    type Reply = AuthConnectionReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AuthConnectionReply {
    pub sequence: u16,
    pub length: u32,
    pub authenticated: u32,
}
impl_debug_if_no_extra_traits!(AuthConnectionReply, "AuthConnectionReply");
impl TryParse for AuthConnectionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (authenticated, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = AuthConnectionReply { sequence, length, authenticated };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for AuthConnectionReply {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let authenticated_bytes = self.authenticated.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            authenticated_bytes[0],
            authenticated_bytes[1],
            authenticated_bytes[2],
            authenticated_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.authenticated.serialize_into(bytes);
    }
}

