// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `DRI2` X11 extension.

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
pub const X11_EXTENSION_NAME: &str = "DRI2";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (1, 4);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Attachment(u32);
impl Attachment {
    pub const BUFFER_FRONT_LEFT: Self = Self(0);
    pub const BUFFER_BACK_LEFT: Self = Self(1);
    pub const BUFFER_FRONT_RIGHT: Self = Self(2);
    pub const BUFFER_BACK_RIGHT: Self = Self(3);
    pub const BUFFER_DEPTH: Self = Self(4);
    pub const BUFFER_STENCIL: Self = Self(5);
    pub const BUFFER_ACCUM: Self = Self(6);
    pub const BUFFER_FAKE_FRONT_LEFT: Self = Self(7);
    pub const BUFFER_FAKE_FRONT_RIGHT: Self = Self(8);
    pub const BUFFER_DEPTH_STENCIL: Self = Self(9);
    pub const BUFFER_HIZ: Self = Self(10);
}
impl From<Attachment> for u32 {
    #[inline]
    fn from(input: Attachment) -> Self {
        input.0
    }
}
impl From<Attachment> for Option<u32> {
    #[inline]
    fn from(input: Attachment) -> Self {
        Some(input.0)
    }
}
impl From<u8> for Attachment {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for Attachment {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for Attachment {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Attachment  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::BUFFER_FRONT_LEFT.0, "BUFFER_FRONT_LEFT", "BufferFrontLeft"),
            (Self::BUFFER_BACK_LEFT.0, "BUFFER_BACK_LEFT", "BufferBackLeft"),
            (Self::BUFFER_FRONT_RIGHT.0, "BUFFER_FRONT_RIGHT", "BufferFrontRight"),
            (Self::BUFFER_BACK_RIGHT.0, "BUFFER_BACK_RIGHT", "BufferBackRight"),
            (Self::BUFFER_DEPTH.0, "BUFFER_DEPTH", "BufferDepth"),
            (Self::BUFFER_STENCIL.0, "BUFFER_STENCIL", "BufferStencil"),
            (Self::BUFFER_ACCUM.0, "BUFFER_ACCUM", "BufferAccum"),
            (Self::BUFFER_FAKE_FRONT_LEFT.0, "BUFFER_FAKE_FRONT_LEFT", "BufferFakeFrontLeft"),
            (Self::BUFFER_FAKE_FRONT_RIGHT.0, "BUFFER_FAKE_FRONT_RIGHT", "BufferFakeFrontRight"),
            (Self::BUFFER_DEPTH_STENCIL.0, "BUFFER_DEPTH_STENCIL", "BufferDepthStencil"),
            (Self::BUFFER_HIZ.0, "BUFFER_HIZ", "BufferHiz"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DriverType(u32);
impl DriverType {
    pub const DRI: Self = Self(0);
    pub const VDPAU: Self = Self(1);
}
impl From<DriverType> for u32 {
    #[inline]
    fn from(input: DriverType) -> Self {
        input.0
    }
}
impl From<DriverType> for Option<u32> {
    #[inline]
    fn from(input: DriverType) -> Self {
        Some(input.0)
    }
}
impl From<u8> for DriverType {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for DriverType {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for DriverType {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for DriverType  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::DRI.0, "DRI", "DRI"),
            (Self::VDPAU.0, "VDPAU", "VDPAU"),
        ];
        pretty_print_enum(fmt, self.0, &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EventType(u16);
impl EventType {
    pub const EXCHANGE_COMPLETE: Self = Self(1);
    pub const BLIT_COMPLETE: Self = Self(2);
    pub const FLIP_COMPLETE: Self = Self(3);
}
impl From<EventType> for u16 {
    #[inline]
    fn from(input: EventType) -> Self {
        input.0
    }
}
impl From<EventType> for Option<u16> {
    #[inline]
    fn from(input: EventType) -> Self {
        Some(input.0)
    }
}
impl From<EventType> for u32 {
    #[inline]
    fn from(input: EventType) -> Self {
        u32::from(input.0)
    }
}
impl From<EventType> for Option<u32> {
    #[inline]
    fn from(input: EventType) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for EventType {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for EventType {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for EventType  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::EXCHANGE_COMPLETE.0.into(), "EXCHANGE_COMPLETE", "ExchangeComplete"),
            (Self::BLIT_COMPLETE.0.into(), "BLIT_COMPLETE", "BlitComplete"),
            (Self::FLIP_COMPLETE.0.into(), "FLIP_COMPLETE", "FlipComplete"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DRI2Buffer {
    pub attachment: Attachment,
    pub name: u32,
    pub pitch: u32,
    pub cpp: u32,
    pub flags: u32,
}
impl_debug_if_no_extra_traits!(DRI2Buffer, "DRI2Buffer");
impl TryParse for DRI2Buffer {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (attachment, remaining) = u32::try_parse(remaining)?;
        let (name, remaining) = u32::try_parse(remaining)?;
        let (pitch, remaining) = u32::try_parse(remaining)?;
        let (cpp, remaining) = u32::try_parse(remaining)?;
        let (flags, remaining) = u32::try_parse(remaining)?;
        let attachment = attachment.into();
        let result = DRI2Buffer { attachment, name, pitch, cpp, flags };
        Ok((result, remaining))
    }
}
impl Serialize for DRI2Buffer {
    type Bytes = [u8; 20];
    fn serialize(&self) -> [u8; 20] {
        let attachment_bytes = u32::from(self.attachment).serialize();
        let name_bytes = self.name.serialize();
        let pitch_bytes = self.pitch.serialize();
        let cpp_bytes = self.cpp.serialize();
        let flags_bytes = self.flags.serialize();
        [
            attachment_bytes[0],
            attachment_bytes[1],
            attachment_bytes[2],
            attachment_bytes[3],
            name_bytes[0],
            name_bytes[1],
            name_bytes[2],
            name_bytes[3],
            pitch_bytes[0],
            pitch_bytes[1],
            pitch_bytes[2],
            pitch_bytes[3],
            cpp_bytes[0],
            cpp_bytes[1],
            cpp_bytes[2],
            cpp_bytes[3],
            flags_bytes[0],
            flags_bytes[1],
            flags_bytes[2],
            flags_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(20);
        u32::from(self.attachment).serialize_into(bytes);
        self.name.serialize_into(bytes);
        self.pitch.serialize_into(bytes);
        self.cpp.serialize_into(bytes);
        self.flags.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AttachFormat {
    pub attachment: Attachment,
    pub format: u32,
}
impl_debug_if_no_extra_traits!(AttachFormat, "AttachFormat");
impl TryParse for AttachFormat {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (attachment, remaining) = u32::try_parse(remaining)?;
        let (format, remaining) = u32::try_parse(remaining)?;
        let attachment = attachment.into();
        let result = AttachFormat { attachment, format };
        Ok((result, remaining))
    }
}
impl Serialize for AttachFormat {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let attachment_bytes = u32::from(self.attachment).serialize();
        let format_bytes = self.format.serialize();
        [
            attachment_bytes[0],
            attachment_bytes[1],
            attachment_bytes[2],
            attachment_bytes[3],
            format_bytes[0],
            format_bytes[1],
            format_bytes[2],
            format_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        u32::from(self.attachment).serialize_into(bytes);
        self.format.serialize_into(bytes);
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
    type Bytes = [u8; 16];
    fn serialize(&self) -> [u8; 16] {
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
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.major_version.serialize_into(bytes);
        self.minor_version.serialize_into(bytes);
    }
}

/// Opcode for the Connect request
pub const CONNECT_REQUEST: u8 = 1;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConnectRequest {
    pub window: xproto::Window,
    pub driver_type: DriverType,
}
impl_debug_if_no_extra_traits!(ConnectRequest, "ConnectRequest");
impl ConnectRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let driver_type_bytes = u32::from(self.driver_type).serialize();
        let mut request0 = vec![
            major_opcode,
            CONNECT_REQUEST,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            driver_type_bytes[0],
            driver_type_bytes[1],
            driver_type_bytes[2],
            driver_type_bytes[3],
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
        if header.minor_opcode != CONNECT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (driver_type, remaining) = u32::try_parse(remaining)?;
        let driver_type = driver_type.into();
        let _ = remaining;
        Ok(ConnectRequest {
            window,
            driver_type,
        })
    }
}
impl Request for ConnectRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for ConnectRequest {
    type Reply = ConnectReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConnectReply {
    pub sequence: u16,
    pub length: u32,
    pub driver_name: Vec<u8>,
    pub alignment_pad: Vec<u8>,
    pub device_name: Vec<u8>,
}
impl_debug_if_no_extra_traits!(ConnectReply, "ConnectReply");
impl TryParse for ConnectReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (driver_name_length, remaining) = u32::try_parse(remaining)?;
        let (device_name_length, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(16..).ok_or(ParseError::InsufficientData)?;
        let (driver_name, remaining) = crate::x11_utils::parse_u8_list(remaining, driver_name_length.try_to_usize()?)?;
        let driver_name = driver_name.to_vec();
        let (alignment_pad, remaining) = crate::x11_utils::parse_u8_list(remaining, (u32::from(driver_name_length).checked_add(3u32).ok_or(ParseError::InvalidExpression)? & (!3u32)).checked_sub(u32::from(driver_name_length)).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let alignment_pad = alignment_pad.to_vec();
        let (device_name, remaining) = crate::x11_utils::parse_u8_list(remaining, device_name_length.try_to_usize()?)?;
        let device_name = device_name.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = ConnectReply { sequence, length, driver_name, alignment_pad, device_name };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ConnectReply {
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
        let driver_name_length = u32::try_from(self.driver_name.len()).expect("`driver_name` has too many elements");
        driver_name_length.serialize_into(bytes);
        let device_name_length = u32::try_from(self.device_name.len()).expect("`device_name` has too many elements");
        device_name_length.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 16]);
        bytes.extend_from_slice(&self.driver_name);
        assert_eq!(self.alignment_pad.len(), usize::try_from((u32::from(driver_name_length).checked_add(3u32).unwrap() & (!3u32)).checked_sub(u32::from(driver_name_length)).unwrap()).unwrap(), "`alignment_pad` has an incorrect length");
        bytes.extend_from_slice(&self.alignment_pad);
        bytes.extend_from_slice(&self.device_name);
    }
}
impl ConnectReply {
    /// Get the value of the `driver_name_length` field.
    ///
    /// The `driver_name_length` field is used as the length field of the `driver_name` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn driver_name_length(&self) -> u32 {
        self.driver_name.len()
            .try_into().unwrap()
    }
    /// Get the value of the `device_name_length` field.
    ///
    /// The `device_name_length` field is used as the length field of the `device_name` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn device_name_length(&self) -> u32 {
        self.device_name.len()
            .try_into().unwrap()
    }
}

/// Opcode for the Authenticate request
pub const AUTHENTICATE_REQUEST: u8 = 2;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AuthenticateRequest {
    pub window: xproto::Window,
    pub magic: u32,
}
impl_debug_if_no_extra_traits!(AuthenticateRequest, "AuthenticateRequest");
impl AuthenticateRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let magic_bytes = self.magic.serialize();
        let mut request0 = vec![
            major_opcode,
            AUTHENTICATE_REQUEST,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
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
        if header.minor_opcode != AUTHENTICATE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (magic, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(AuthenticateRequest {
            window,
            magic,
        })
    }
}
impl Request for AuthenticateRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for AuthenticateRequest {
    type Reply = AuthenticateReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AuthenticateReply {
    pub sequence: u16,
    pub length: u32,
    pub authenticated: u32,
}
impl_debug_if_no_extra_traits!(AuthenticateReply, "AuthenticateReply");
impl TryParse for AuthenticateReply {
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
        let result = AuthenticateReply { sequence, length, authenticated };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for AuthenticateReply {
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

/// Opcode for the CreateDrawable request
pub const CREATE_DRAWABLE_REQUEST: u8 = 3;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateDrawableRequest {
    pub drawable: xproto::Drawable,
}
impl_debug_if_no_extra_traits!(CreateDrawableRequest, "CreateDrawableRequest");
impl CreateDrawableRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_DRAWABLE_REQUEST,
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
        if header.minor_opcode != CREATE_DRAWABLE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let _ = remaining;
        Ok(CreateDrawableRequest {
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
impl crate::x11_utils::VoidRequest for CreateDrawableRequest {
}

/// Opcode for the DestroyDrawable request
pub const DESTROY_DRAWABLE_REQUEST: u8 = 4;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DestroyDrawableRequest {
    pub drawable: xproto::Drawable,
}
impl_debug_if_no_extra_traits!(DestroyDrawableRequest, "DestroyDrawableRequest");
impl DestroyDrawableRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let mut request0 = vec![
            major_opcode,
            DESTROY_DRAWABLE_REQUEST,
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
        if header.minor_opcode != DESTROY_DRAWABLE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let _ = remaining;
        Ok(DestroyDrawableRequest {
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

/// Opcode for the GetBuffers request
pub const GET_BUFFERS_REQUEST: u8 = 5;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetBuffersRequest<'input> {
    pub drawable: xproto::Drawable,
    pub count: u32,
    pub attachments: Cow<'input, [u32]>,
}
impl_debug_if_no_extra_traits!(GetBuffersRequest<'_>, "GetBuffersRequest");
impl<'input> GetBuffersRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let count_bytes = self.count.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_BUFFERS_REQUEST,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            count_bytes[0],
            count_bytes[1],
            count_bytes[2],
            count_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let attachments_bytes = self.attachments.serialize();
        let length_so_far = length_so_far + attachments_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), attachments_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != GET_BUFFERS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let (count, remaining) = u32::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut attachments = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = u32::try_parse(remaining)?;
            remaining = new_remaining;
            attachments.push(v);
        }
        let _ = remaining;
        Ok(GetBuffersRequest {
            drawable,
            count,
            attachments: Cow::Owned(attachments),
        })
    }
    /// Clone all borrowed data in this GetBuffersRequest.
    pub fn into_owned(self) -> GetBuffersRequest<'static> {
        GetBuffersRequest {
            drawable: self.drawable,
            count: self.count,
            attachments: Cow::Owned(self.attachments.into_owned()),
        }
    }
}
impl<'input> Request for GetBuffersRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for GetBuffersRequest<'input> {
    type Reply = GetBuffersReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetBuffersReply {
    pub sequence: u16,
    pub length: u32,
    pub width: u32,
    pub height: u32,
    pub buffers: Vec<DRI2Buffer>,
}
impl_debug_if_no_extra_traits!(GetBuffersReply, "GetBuffersReply");
impl TryParse for GetBuffersReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (width, remaining) = u32::try_parse(remaining)?;
        let (height, remaining) = u32::try_parse(remaining)?;
        let (count, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(12..).ok_or(ParseError::InsufficientData)?;
        let (buffers, remaining) = crate::x11_utils::parse_list::<DRI2Buffer>(remaining, count.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetBuffersReply { sequence, length, width, height, buffers };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetBuffersReply {
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
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        let count = u32::try_from(self.buffers.len()).expect("`buffers` has too many elements");
        count.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 12]);
        self.buffers.serialize_into(bytes);
    }
}
impl GetBuffersReply {
    /// Get the value of the `count` field.
    ///
    /// The `count` field is used as the length field of the `buffers` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn count(&self) -> u32 {
        self.buffers.len()
            .try_into().unwrap()
    }
}

/// Opcode for the CopyRegion request
pub const COPY_REGION_REQUEST: u8 = 6;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CopyRegionRequest {
    pub drawable: xproto::Drawable,
    pub region: u32,
    pub dest: u32,
    pub src: u32,
}
impl_debug_if_no_extra_traits!(CopyRegionRequest, "CopyRegionRequest");
impl CopyRegionRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let region_bytes = self.region.serialize();
        let dest_bytes = self.dest.serialize();
        let src_bytes = self.src.serialize();
        let mut request0 = vec![
            major_opcode,
            COPY_REGION_REQUEST,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            region_bytes[0],
            region_bytes[1],
            region_bytes[2],
            region_bytes[3],
            dest_bytes[0],
            dest_bytes[1],
            dest_bytes[2],
            dest_bytes[3],
            src_bytes[0],
            src_bytes[1],
            src_bytes[2],
            src_bytes[3],
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
        if header.minor_opcode != COPY_REGION_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let (region, remaining) = u32::try_parse(remaining)?;
        let (dest, remaining) = u32::try_parse(remaining)?;
        let (src, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(CopyRegionRequest {
            drawable,
            region,
            dest,
            src,
        })
    }
}
impl Request for CopyRegionRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for CopyRegionRequest {
    type Reply = CopyRegionReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CopyRegionReply {
    pub sequence: u16,
    pub length: u32,
}
impl_debug_if_no_extra_traits!(CopyRegionReply, "CopyRegionReply");
impl TryParse for CopyRegionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = CopyRegionReply { sequence, length };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for CopyRegionReply {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
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
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
    }
}

/// Opcode for the GetBuffersWithFormat request
pub const GET_BUFFERS_WITH_FORMAT_REQUEST: u8 = 7;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetBuffersWithFormatRequest<'input> {
    pub drawable: xproto::Drawable,
    pub count: u32,
    pub attachments: Cow<'input, [AttachFormat]>,
}
impl_debug_if_no_extra_traits!(GetBuffersWithFormatRequest<'_>, "GetBuffersWithFormatRequest");
impl<'input> GetBuffersWithFormatRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let count_bytes = self.count.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_BUFFERS_WITH_FORMAT_REQUEST,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            count_bytes[0],
            count_bytes[1],
            count_bytes[2],
            count_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let attachments_bytes = self.attachments.serialize();
        let length_so_far = length_so_far + attachments_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), attachments_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != GET_BUFFERS_WITH_FORMAT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let (count, remaining) = u32::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut attachments = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = AttachFormat::try_parse(remaining)?;
            remaining = new_remaining;
            attachments.push(v);
        }
        let _ = remaining;
        Ok(GetBuffersWithFormatRequest {
            drawable,
            count,
            attachments: Cow::Owned(attachments),
        })
    }
    /// Clone all borrowed data in this GetBuffersWithFormatRequest.
    pub fn into_owned(self) -> GetBuffersWithFormatRequest<'static> {
        GetBuffersWithFormatRequest {
            drawable: self.drawable,
            count: self.count,
            attachments: Cow::Owned(self.attachments.into_owned()),
        }
    }
}
impl<'input> Request for GetBuffersWithFormatRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for GetBuffersWithFormatRequest<'input> {
    type Reply = GetBuffersWithFormatReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetBuffersWithFormatReply {
    pub sequence: u16,
    pub length: u32,
    pub width: u32,
    pub height: u32,
    pub buffers: Vec<DRI2Buffer>,
}
impl_debug_if_no_extra_traits!(GetBuffersWithFormatReply, "GetBuffersWithFormatReply");
impl TryParse for GetBuffersWithFormatReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (width, remaining) = u32::try_parse(remaining)?;
        let (height, remaining) = u32::try_parse(remaining)?;
        let (count, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(12..).ok_or(ParseError::InsufficientData)?;
        let (buffers, remaining) = crate::x11_utils::parse_list::<DRI2Buffer>(remaining, count.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetBuffersWithFormatReply { sequence, length, width, height, buffers };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetBuffersWithFormatReply {
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
        self.width.serialize_into(bytes);
        self.height.serialize_into(bytes);
        let count = u32::try_from(self.buffers.len()).expect("`buffers` has too many elements");
        count.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 12]);
        self.buffers.serialize_into(bytes);
    }
}
impl GetBuffersWithFormatReply {
    /// Get the value of the `count` field.
    ///
    /// The `count` field is used as the length field of the `buffers` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn count(&self) -> u32 {
        self.buffers.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SwapBuffers request
pub const SWAP_BUFFERS_REQUEST: u8 = 8;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SwapBuffersRequest {
    pub drawable: xproto::Drawable,
    pub target_msc_hi: u32,
    pub target_msc_lo: u32,
    pub divisor_hi: u32,
    pub divisor_lo: u32,
    pub remainder_hi: u32,
    pub remainder_lo: u32,
}
impl_debug_if_no_extra_traits!(SwapBuffersRequest, "SwapBuffersRequest");
impl SwapBuffersRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let target_msc_hi_bytes = self.target_msc_hi.serialize();
        let target_msc_lo_bytes = self.target_msc_lo.serialize();
        let divisor_hi_bytes = self.divisor_hi.serialize();
        let divisor_lo_bytes = self.divisor_lo.serialize();
        let remainder_hi_bytes = self.remainder_hi.serialize();
        let remainder_lo_bytes = self.remainder_lo.serialize();
        let mut request0 = vec![
            major_opcode,
            SWAP_BUFFERS_REQUEST,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            target_msc_hi_bytes[0],
            target_msc_hi_bytes[1],
            target_msc_hi_bytes[2],
            target_msc_hi_bytes[3],
            target_msc_lo_bytes[0],
            target_msc_lo_bytes[1],
            target_msc_lo_bytes[2],
            target_msc_lo_bytes[3],
            divisor_hi_bytes[0],
            divisor_hi_bytes[1],
            divisor_hi_bytes[2],
            divisor_hi_bytes[3],
            divisor_lo_bytes[0],
            divisor_lo_bytes[1],
            divisor_lo_bytes[2],
            divisor_lo_bytes[3],
            remainder_hi_bytes[0],
            remainder_hi_bytes[1],
            remainder_hi_bytes[2],
            remainder_hi_bytes[3],
            remainder_lo_bytes[0],
            remainder_lo_bytes[1],
            remainder_lo_bytes[2],
            remainder_lo_bytes[3],
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
        if header.minor_opcode != SWAP_BUFFERS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let (target_msc_hi, remaining) = u32::try_parse(remaining)?;
        let (target_msc_lo, remaining) = u32::try_parse(remaining)?;
        let (divisor_hi, remaining) = u32::try_parse(remaining)?;
        let (divisor_lo, remaining) = u32::try_parse(remaining)?;
        let (remainder_hi, remaining) = u32::try_parse(remaining)?;
        let (remainder_lo, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(SwapBuffersRequest {
            drawable,
            target_msc_hi,
            target_msc_lo,
            divisor_hi,
            divisor_lo,
            remainder_hi,
            remainder_lo,
        })
    }
}
impl Request for SwapBuffersRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for SwapBuffersRequest {
    type Reply = SwapBuffersReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SwapBuffersReply {
    pub sequence: u16,
    pub length: u32,
    pub swap_hi: u32,
    pub swap_lo: u32,
}
impl_debug_if_no_extra_traits!(SwapBuffersReply, "SwapBuffersReply");
impl TryParse for SwapBuffersReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (swap_hi, remaining) = u32::try_parse(remaining)?;
        let (swap_lo, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = SwapBuffersReply { sequence, length, swap_hi, swap_lo };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for SwapBuffersReply {
    type Bytes = [u8; 16];
    fn serialize(&self) -> [u8; 16] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let swap_hi_bytes = self.swap_hi.serialize();
        let swap_lo_bytes = self.swap_lo.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            swap_hi_bytes[0],
            swap_hi_bytes[1],
            swap_hi_bytes[2],
            swap_hi_bytes[3],
            swap_lo_bytes[0],
            swap_lo_bytes[1],
            swap_lo_bytes[2],
            swap_lo_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.swap_hi.serialize_into(bytes);
        self.swap_lo.serialize_into(bytes);
    }
}

/// Opcode for the GetMSC request
pub const GET_MSC_REQUEST: u8 = 9;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetMSCRequest {
    pub drawable: xproto::Drawable,
}
impl_debug_if_no_extra_traits!(GetMSCRequest, "GetMSCRequest");
impl GetMSCRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_MSC_REQUEST,
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
        if header.minor_opcode != GET_MSC_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let _ = remaining;
        Ok(GetMSCRequest {
            drawable,
        })
    }
}
impl Request for GetMSCRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetMSCRequest {
    type Reply = GetMSCReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetMSCReply {
    pub sequence: u16,
    pub length: u32,
    pub ust_hi: u32,
    pub ust_lo: u32,
    pub msc_hi: u32,
    pub msc_lo: u32,
    pub sbc_hi: u32,
    pub sbc_lo: u32,
}
impl_debug_if_no_extra_traits!(GetMSCReply, "GetMSCReply");
impl TryParse for GetMSCReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (ust_hi, remaining) = u32::try_parse(remaining)?;
        let (ust_lo, remaining) = u32::try_parse(remaining)?;
        let (msc_hi, remaining) = u32::try_parse(remaining)?;
        let (msc_lo, remaining) = u32::try_parse(remaining)?;
        let (sbc_hi, remaining) = u32::try_parse(remaining)?;
        let (sbc_lo, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetMSCReply { sequence, length, ust_hi, ust_lo, msc_hi, msc_lo, sbc_hi, sbc_lo };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetMSCReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let ust_hi_bytes = self.ust_hi.serialize();
        let ust_lo_bytes = self.ust_lo.serialize();
        let msc_hi_bytes = self.msc_hi.serialize();
        let msc_lo_bytes = self.msc_lo.serialize();
        let sbc_hi_bytes = self.sbc_hi.serialize();
        let sbc_lo_bytes = self.sbc_lo.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            ust_hi_bytes[0],
            ust_hi_bytes[1],
            ust_hi_bytes[2],
            ust_hi_bytes[3],
            ust_lo_bytes[0],
            ust_lo_bytes[1],
            ust_lo_bytes[2],
            ust_lo_bytes[3],
            msc_hi_bytes[0],
            msc_hi_bytes[1],
            msc_hi_bytes[2],
            msc_hi_bytes[3],
            msc_lo_bytes[0],
            msc_lo_bytes[1],
            msc_lo_bytes[2],
            msc_lo_bytes[3],
            sbc_hi_bytes[0],
            sbc_hi_bytes[1],
            sbc_hi_bytes[2],
            sbc_hi_bytes[3],
            sbc_lo_bytes[0],
            sbc_lo_bytes[1],
            sbc_lo_bytes[2],
            sbc_lo_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.ust_hi.serialize_into(bytes);
        self.ust_lo.serialize_into(bytes);
        self.msc_hi.serialize_into(bytes);
        self.msc_lo.serialize_into(bytes);
        self.sbc_hi.serialize_into(bytes);
        self.sbc_lo.serialize_into(bytes);
    }
}

/// Opcode for the WaitMSC request
pub const WAIT_MSC_REQUEST: u8 = 10;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WaitMSCRequest {
    pub drawable: xproto::Drawable,
    pub target_msc_hi: u32,
    pub target_msc_lo: u32,
    pub divisor_hi: u32,
    pub divisor_lo: u32,
    pub remainder_hi: u32,
    pub remainder_lo: u32,
}
impl_debug_if_no_extra_traits!(WaitMSCRequest, "WaitMSCRequest");
impl WaitMSCRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let target_msc_hi_bytes = self.target_msc_hi.serialize();
        let target_msc_lo_bytes = self.target_msc_lo.serialize();
        let divisor_hi_bytes = self.divisor_hi.serialize();
        let divisor_lo_bytes = self.divisor_lo.serialize();
        let remainder_hi_bytes = self.remainder_hi.serialize();
        let remainder_lo_bytes = self.remainder_lo.serialize();
        let mut request0 = vec![
            major_opcode,
            WAIT_MSC_REQUEST,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            target_msc_hi_bytes[0],
            target_msc_hi_bytes[1],
            target_msc_hi_bytes[2],
            target_msc_hi_bytes[3],
            target_msc_lo_bytes[0],
            target_msc_lo_bytes[1],
            target_msc_lo_bytes[2],
            target_msc_lo_bytes[3],
            divisor_hi_bytes[0],
            divisor_hi_bytes[1],
            divisor_hi_bytes[2],
            divisor_hi_bytes[3],
            divisor_lo_bytes[0],
            divisor_lo_bytes[1],
            divisor_lo_bytes[2],
            divisor_lo_bytes[3],
            remainder_hi_bytes[0],
            remainder_hi_bytes[1],
            remainder_hi_bytes[2],
            remainder_hi_bytes[3],
            remainder_lo_bytes[0],
            remainder_lo_bytes[1],
            remainder_lo_bytes[2],
            remainder_lo_bytes[3],
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
        if header.minor_opcode != WAIT_MSC_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let (target_msc_hi, remaining) = u32::try_parse(remaining)?;
        let (target_msc_lo, remaining) = u32::try_parse(remaining)?;
        let (divisor_hi, remaining) = u32::try_parse(remaining)?;
        let (divisor_lo, remaining) = u32::try_parse(remaining)?;
        let (remainder_hi, remaining) = u32::try_parse(remaining)?;
        let (remainder_lo, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(WaitMSCRequest {
            drawable,
            target_msc_hi,
            target_msc_lo,
            divisor_hi,
            divisor_lo,
            remainder_hi,
            remainder_lo,
        })
    }
}
impl Request for WaitMSCRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for WaitMSCRequest {
    type Reply = WaitMSCReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WaitMSCReply {
    pub sequence: u16,
    pub length: u32,
    pub ust_hi: u32,
    pub ust_lo: u32,
    pub msc_hi: u32,
    pub msc_lo: u32,
    pub sbc_hi: u32,
    pub sbc_lo: u32,
}
impl_debug_if_no_extra_traits!(WaitMSCReply, "WaitMSCReply");
impl TryParse for WaitMSCReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (ust_hi, remaining) = u32::try_parse(remaining)?;
        let (ust_lo, remaining) = u32::try_parse(remaining)?;
        let (msc_hi, remaining) = u32::try_parse(remaining)?;
        let (msc_lo, remaining) = u32::try_parse(remaining)?;
        let (sbc_hi, remaining) = u32::try_parse(remaining)?;
        let (sbc_lo, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = WaitMSCReply { sequence, length, ust_hi, ust_lo, msc_hi, msc_lo, sbc_hi, sbc_lo };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for WaitMSCReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let ust_hi_bytes = self.ust_hi.serialize();
        let ust_lo_bytes = self.ust_lo.serialize();
        let msc_hi_bytes = self.msc_hi.serialize();
        let msc_lo_bytes = self.msc_lo.serialize();
        let sbc_hi_bytes = self.sbc_hi.serialize();
        let sbc_lo_bytes = self.sbc_lo.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            ust_hi_bytes[0],
            ust_hi_bytes[1],
            ust_hi_bytes[2],
            ust_hi_bytes[3],
            ust_lo_bytes[0],
            ust_lo_bytes[1],
            ust_lo_bytes[2],
            ust_lo_bytes[3],
            msc_hi_bytes[0],
            msc_hi_bytes[1],
            msc_hi_bytes[2],
            msc_hi_bytes[3],
            msc_lo_bytes[0],
            msc_lo_bytes[1],
            msc_lo_bytes[2],
            msc_lo_bytes[3],
            sbc_hi_bytes[0],
            sbc_hi_bytes[1],
            sbc_hi_bytes[2],
            sbc_hi_bytes[3],
            sbc_lo_bytes[0],
            sbc_lo_bytes[1],
            sbc_lo_bytes[2],
            sbc_lo_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.ust_hi.serialize_into(bytes);
        self.ust_lo.serialize_into(bytes);
        self.msc_hi.serialize_into(bytes);
        self.msc_lo.serialize_into(bytes);
        self.sbc_hi.serialize_into(bytes);
        self.sbc_lo.serialize_into(bytes);
    }
}

/// Opcode for the WaitSBC request
pub const WAIT_SBC_REQUEST: u8 = 11;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WaitSBCRequest {
    pub drawable: xproto::Drawable,
    pub target_sbc_hi: u32,
    pub target_sbc_lo: u32,
}
impl_debug_if_no_extra_traits!(WaitSBCRequest, "WaitSBCRequest");
impl WaitSBCRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let target_sbc_hi_bytes = self.target_sbc_hi.serialize();
        let target_sbc_lo_bytes = self.target_sbc_lo.serialize();
        let mut request0 = vec![
            major_opcode,
            WAIT_SBC_REQUEST,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            target_sbc_hi_bytes[0],
            target_sbc_hi_bytes[1],
            target_sbc_hi_bytes[2],
            target_sbc_hi_bytes[3],
            target_sbc_lo_bytes[0],
            target_sbc_lo_bytes[1],
            target_sbc_lo_bytes[2],
            target_sbc_lo_bytes[3],
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
        if header.minor_opcode != WAIT_SBC_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let (target_sbc_hi, remaining) = u32::try_parse(remaining)?;
        let (target_sbc_lo, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(WaitSBCRequest {
            drawable,
            target_sbc_hi,
            target_sbc_lo,
        })
    }
}
impl Request for WaitSBCRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for WaitSBCRequest {
    type Reply = WaitSBCReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WaitSBCReply {
    pub sequence: u16,
    pub length: u32,
    pub ust_hi: u32,
    pub ust_lo: u32,
    pub msc_hi: u32,
    pub msc_lo: u32,
    pub sbc_hi: u32,
    pub sbc_lo: u32,
}
impl_debug_if_no_extra_traits!(WaitSBCReply, "WaitSBCReply");
impl TryParse for WaitSBCReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (ust_hi, remaining) = u32::try_parse(remaining)?;
        let (ust_lo, remaining) = u32::try_parse(remaining)?;
        let (msc_hi, remaining) = u32::try_parse(remaining)?;
        let (msc_lo, remaining) = u32::try_parse(remaining)?;
        let (sbc_hi, remaining) = u32::try_parse(remaining)?;
        let (sbc_lo, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = WaitSBCReply { sequence, length, ust_hi, ust_lo, msc_hi, msc_lo, sbc_hi, sbc_lo };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for WaitSBCReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let ust_hi_bytes = self.ust_hi.serialize();
        let ust_lo_bytes = self.ust_lo.serialize();
        let msc_hi_bytes = self.msc_hi.serialize();
        let msc_lo_bytes = self.msc_lo.serialize();
        let sbc_hi_bytes = self.sbc_hi.serialize();
        let sbc_lo_bytes = self.sbc_lo.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            ust_hi_bytes[0],
            ust_hi_bytes[1],
            ust_hi_bytes[2],
            ust_hi_bytes[3],
            ust_lo_bytes[0],
            ust_lo_bytes[1],
            ust_lo_bytes[2],
            ust_lo_bytes[3],
            msc_hi_bytes[0],
            msc_hi_bytes[1],
            msc_hi_bytes[2],
            msc_hi_bytes[3],
            msc_lo_bytes[0],
            msc_lo_bytes[1],
            msc_lo_bytes[2],
            msc_lo_bytes[3],
            sbc_hi_bytes[0],
            sbc_hi_bytes[1],
            sbc_hi_bytes[2],
            sbc_hi_bytes[3],
            sbc_lo_bytes[0],
            sbc_lo_bytes[1],
            sbc_lo_bytes[2],
            sbc_lo_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.ust_hi.serialize_into(bytes);
        self.ust_lo.serialize_into(bytes);
        self.msc_hi.serialize_into(bytes);
        self.msc_lo.serialize_into(bytes);
        self.sbc_hi.serialize_into(bytes);
        self.sbc_lo.serialize_into(bytes);
    }
}

/// Opcode for the SwapInterval request
pub const SWAP_INTERVAL_REQUEST: u8 = 12;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SwapIntervalRequest {
    pub drawable: xproto::Drawable,
    pub interval: u32,
}
impl_debug_if_no_extra_traits!(SwapIntervalRequest, "SwapIntervalRequest");
impl SwapIntervalRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let interval_bytes = self.interval.serialize();
        let mut request0 = vec![
            major_opcode,
            SWAP_INTERVAL_REQUEST,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            interval_bytes[0],
            interval_bytes[1],
            interval_bytes[2],
            interval_bytes[3],
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
        if header.minor_opcode != SWAP_INTERVAL_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let (interval, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(SwapIntervalRequest {
            drawable,
            interval,
        })
    }
}
impl Request for SwapIntervalRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SwapIntervalRequest {
}

/// Opcode for the GetParam request
pub const GET_PARAM_REQUEST: u8 = 13;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetParamRequest {
    pub drawable: xproto::Drawable,
    pub param: u32,
}
impl_debug_if_no_extra_traits!(GetParamRequest, "GetParamRequest");
impl GetParamRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let param_bytes = self.param.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_PARAM_REQUEST,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            param_bytes[0],
            param_bytes[1],
            param_bytes[2],
            param_bytes[3],
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
        if header.minor_opcode != GET_PARAM_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let (param, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetParamRequest {
            drawable,
            param,
        })
    }
}
impl Request for GetParamRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetParamRequest {
    type Reply = GetParamReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetParamReply {
    pub is_param_recognized: bool,
    pub sequence: u16,
    pub length: u32,
    pub value_hi: u32,
    pub value_lo: u32,
}
impl_debug_if_no_extra_traits!(GetParamReply, "GetParamReply");
impl TryParse for GetParamReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (is_param_recognized, remaining) = bool::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (value_hi, remaining) = u32::try_parse(remaining)?;
        let (value_lo, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetParamReply { is_param_recognized, sequence, length, value_hi, value_lo };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetParamReply {
    type Bytes = [u8; 16];
    fn serialize(&self) -> [u8; 16] {
        let response_type_bytes = &[1];
        let is_param_recognized_bytes = self.is_param_recognized.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let value_hi_bytes = self.value_hi.serialize();
        let value_lo_bytes = self.value_lo.serialize();
        [
            response_type_bytes[0],
            is_param_recognized_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            value_hi_bytes[0],
            value_hi_bytes[1],
            value_hi_bytes[2],
            value_hi_bytes[3],
            value_lo_bytes[0],
            value_lo_bytes[1],
            value_lo_bytes[2],
            value_lo_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        self.is_param_recognized.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.value_hi.serialize_into(bytes);
        self.value_lo.serialize_into(bytes);
    }
}

/// Opcode for the BufferSwapComplete event
pub const BUFFER_SWAP_COMPLETE_EVENT: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BufferSwapCompleteEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub event_type: EventType,
    pub drawable: xproto::Drawable,
    pub ust_hi: u32,
    pub ust_lo: u32,
    pub msc_hi: u32,
    pub msc_lo: u32,
    pub sbc: u32,
}
impl_debug_if_no_extra_traits!(BufferSwapCompleteEvent, "BufferSwapCompleteEvent");
impl TryParse for BufferSwapCompleteEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (event_type, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (drawable, remaining) = xproto::Drawable::try_parse(remaining)?;
        let (ust_hi, remaining) = u32::try_parse(remaining)?;
        let (ust_lo, remaining) = u32::try_parse(remaining)?;
        let (msc_hi, remaining) = u32::try_parse(remaining)?;
        let (msc_lo, remaining) = u32::try_parse(remaining)?;
        let (sbc, remaining) = u32::try_parse(remaining)?;
        let event_type = event_type.into();
        let result = BufferSwapCompleteEvent { response_type, sequence, event_type, drawable, ust_hi, ust_lo, msc_hi, msc_lo, sbc };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for BufferSwapCompleteEvent {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let event_type_bytes = u16::from(self.event_type).serialize();
        let drawable_bytes = self.drawable.serialize();
        let ust_hi_bytes = self.ust_hi.serialize();
        let ust_lo_bytes = self.ust_lo.serialize();
        let msc_hi_bytes = self.msc_hi.serialize();
        let msc_lo_bytes = self.msc_lo.serialize();
        let sbc_bytes = self.sbc.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_type_bytes[0],
            event_type_bytes[1],
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            ust_hi_bytes[0],
            ust_hi_bytes[1],
            ust_hi_bytes[2],
            ust_hi_bytes[3],
            ust_lo_bytes[0],
            ust_lo_bytes[1],
            ust_lo_bytes[2],
            ust_lo_bytes[3],
            msc_hi_bytes[0],
            msc_hi_bytes[1],
            msc_hi_bytes[2],
            msc_hi_bytes[3],
            msc_lo_bytes[0],
            msc_lo_bytes[1],
            msc_lo_bytes[2],
            msc_lo_bytes[3],
            sbc_bytes[0],
            sbc_bytes[1],
            sbc_bytes[2],
            sbc_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        u16::from(self.event_type).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.drawable.serialize_into(bytes);
        self.ust_hi.serialize_into(bytes);
        self.ust_lo.serialize_into(bytes);
        self.msc_hi.serialize_into(bytes);
        self.msc_lo.serialize_into(bytes);
        self.sbc.serialize_into(bytes);
    }
}
impl From<&BufferSwapCompleteEvent> for [u8; 32] {
    fn from(input: &BufferSwapCompleteEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let event_type_bytes = u16::from(input.event_type).serialize();
        let drawable_bytes = input.drawable.serialize();
        let ust_hi_bytes = input.ust_hi.serialize();
        let ust_lo_bytes = input.ust_lo.serialize();
        let msc_hi_bytes = input.msc_hi.serialize();
        let msc_lo_bytes = input.msc_lo.serialize();
        let sbc_bytes = input.sbc.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            event_type_bytes[0],
            event_type_bytes[1],
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            ust_hi_bytes[0],
            ust_hi_bytes[1],
            ust_hi_bytes[2],
            ust_hi_bytes[3],
            ust_lo_bytes[0],
            ust_lo_bytes[1],
            ust_lo_bytes[2],
            ust_lo_bytes[3],
            msc_hi_bytes[0],
            msc_hi_bytes[1],
            msc_hi_bytes[2],
            msc_hi_bytes[3],
            msc_lo_bytes[0],
            msc_lo_bytes[1],
            msc_lo_bytes[2],
            msc_lo_bytes[3],
            sbc_bytes[0],
            sbc_bytes[1],
            sbc_bytes[2],
            sbc_bytes[3],
        ]
    }
}
impl From<BufferSwapCompleteEvent> for [u8; 32] {
    fn from(input: BufferSwapCompleteEvent) -> Self {
        Self::from(&input)
    }
}

/// Opcode for the InvalidateBuffers event
pub const INVALIDATE_BUFFERS_EVENT: u8 = 1;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InvalidateBuffersEvent {
    pub response_type: u8,
    pub sequence: u16,
    pub drawable: xproto::Drawable,
}
impl_debug_if_no_extra_traits!(InvalidateBuffersEvent, "InvalidateBuffersEvent");
impl TryParse for InvalidateBuffersEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (drawable, remaining) = xproto::Drawable::try_parse(remaining)?;
        let result = InvalidateBuffersEvent { response_type, sequence, drawable };
        let _ = remaining;
        let remaining = initial_value.get(32..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for InvalidateBuffersEvent {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let response_type_bytes = self.response_type.serialize();
        let sequence_bytes = self.sequence.serialize();
        let drawable_bytes = self.drawable.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.response_type.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.drawable.serialize_into(bytes);
    }
}
impl From<&InvalidateBuffersEvent> for [u8; 32] {
    fn from(input: &InvalidateBuffersEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let sequence_bytes = input.sequence.serialize();
        let drawable_bytes = input.drawable.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
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
impl From<InvalidateBuffersEvent> for [u8; 32] {
    fn from(input: InvalidateBuffersEvent) -> Self {
        Self::from(&input)
    }
}

