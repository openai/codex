// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `SELinux` X11 extension.

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
pub const X11_EXTENSION_NAME: &str = "SELinux";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (1, 0);

/// Opcode for the QueryVersion request
pub const QUERY_VERSION_REQUEST: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryVersionRequest {
    pub client_major: u8,
    pub client_minor: u8,
}
impl_debug_if_no_extra_traits!(QueryVersionRequest, "QueryVersionRequest");
impl QueryVersionRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let client_major_bytes = self.client_major.serialize();
        let client_minor_bytes = self.client_minor.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_VERSION_REQUEST,
            0,
            0,
            client_major_bytes[0],
            client_minor_bytes[0],
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
        let (client_major, remaining) = u8::try_parse(value)?;
        let (client_minor, remaining) = u8::try_parse(remaining)?;
        let _ = remaining;
        Ok(QueryVersionRequest {
            client_major,
            client_minor,
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
    pub server_major: u16,
    pub server_minor: u16,
}
impl_debug_if_no_extra_traits!(QueryVersionReply, "QueryVersionReply");
impl TryParse for QueryVersionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (server_major, remaining) = u16::try_parse(remaining)?;
        let (server_minor, remaining) = u16::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryVersionReply { sequence, length, server_major, server_minor };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryVersionReply {
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let server_major_bytes = self.server_major.serialize();
        let server_minor_bytes = self.server_minor.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            server_major_bytes[0],
            server_major_bytes[1],
            server_minor_bytes[0],
            server_minor_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.server_major.serialize_into(bytes);
        self.server_minor.serialize_into(bytes);
    }
}

/// Opcode for the SetDeviceCreateContext request
pub const SET_DEVICE_CREATE_CONTEXT_REQUEST: u8 = 1;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetDeviceCreateContextRequest<'input> {
    pub context: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(SetDeviceCreateContextRequest<'_>, "SetDeviceCreateContextRequest");
impl<'input> SetDeviceCreateContextRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        let context_len_bytes = context_len.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_DEVICE_CREATE_CONTEXT_REQUEST,
            0,
            0,
            context_len_bytes[0],
            context_len_bytes[1],
            context_len_bytes[2],
            context_len_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.context.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.context, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_DEVICE_CREATE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (context_len, remaining) = u32::try_parse(value)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(SetDeviceCreateContextRequest {
            context: Cow::Borrowed(context),
        })
    }
    /// Clone all borrowed data in this SetDeviceCreateContextRequest.
    pub fn into_owned(self) -> SetDeviceCreateContextRequest<'static> {
        SetDeviceCreateContextRequest {
            context: Cow::Owned(self.context.into_owned()),
        }
    }
}
impl<'input> Request for SetDeviceCreateContextRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetDeviceCreateContextRequest<'input> {
}

/// Opcode for the GetDeviceCreateContext request
pub const GET_DEVICE_CREATE_CONTEXT_REQUEST: u8 = 2;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetDeviceCreateContextRequest;
impl_debug_if_no_extra_traits!(GetDeviceCreateContextRequest, "GetDeviceCreateContextRequest");
impl GetDeviceCreateContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            GET_DEVICE_CREATE_CONTEXT_REQUEST,
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
        if header.minor_opcode != GET_DEVICE_CREATE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(GetDeviceCreateContextRequest
        )
    }
}
impl Request for GetDeviceCreateContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetDeviceCreateContextRequest {
    type Reply = GetDeviceCreateContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetDeviceCreateContextReply {
    pub sequence: u16,
    pub length: u32,
    pub context: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetDeviceCreateContextReply, "GetDeviceCreateContextReply");
impl TryParse for GetDeviceCreateContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (context_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let context = context.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetDeviceCreateContextReply { sequence, length, context };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetDeviceCreateContextReply {
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
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        context_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&self.context);
    }
}
impl GetDeviceCreateContextReply {
    /// Get the value of the `context_len` field.
    ///
    /// The `context_len` field is used as the length field of the `context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn context_len(&self) -> u32 {
        self.context.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SetDeviceContext request
pub const SET_DEVICE_CONTEXT_REQUEST: u8 = 3;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetDeviceContextRequest<'input> {
    pub device: u32,
    pub context: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(SetDeviceContextRequest<'_>, "SetDeviceContextRequest");
impl<'input> SetDeviceContextRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let device_bytes = self.device.serialize();
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        let context_len_bytes = context_len.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_DEVICE_CONTEXT_REQUEST,
            0,
            0,
            device_bytes[0],
            device_bytes[1],
            device_bytes[2],
            device_bytes[3],
            context_len_bytes[0],
            context_len_bytes[1],
            context_len_bytes[2],
            context_len_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.context.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.context, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_DEVICE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device, remaining) = u32::try_parse(value)?;
        let (context_len, remaining) = u32::try_parse(remaining)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(SetDeviceContextRequest {
            device,
            context: Cow::Borrowed(context),
        })
    }
    /// Clone all borrowed data in this SetDeviceContextRequest.
    pub fn into_owned(self) -> SetDeviceContextRequest<'static> {
        SetDeviceContextRequest {
            device: self.device,
            context: Cow::Owned(self.context.into_owned()),
        }
    }
}
impl<'input> Request for SetDeviceContextRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetDeviceContextRequest<'input> {
}

/// Opcode for the GetDeviceContext request
pub const GET_DEVICE_CONTEXT_REQUEST: u8 = 4;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetDeviceContextRequest {
    pub device: u32,
}
impl_debug_if_no_extra_traits!(GetDeviceContextRequest, "GetDeviceContextRequest");
impl GetDeviceContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let device_bytes = self.device.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_DEVICE_CONTEXT_REQUEST,
            0,
            0,
            device_bytes[0],
            device_bytes[1],
            device_bytes[2],
            device_bytes[3],
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
        if header.minor_opcode != GET_DEVICE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (device, remaining) = u32::try_parse(value)?;
        let _ = remaining;
        Ok(GetDeviceContextRequest {
            device,
        })
    }
}
impl Request for GetDeviceContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetDeviceContextRequest {
    type Reply = GetDeviceContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetDeviceContextReply {
    pub sequence: u16,
    pub length: u32,
    pub context: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetDeviceContextReply, "GetDeviceContextReply");
impl TryParse for GetDeviceContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (context_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let context = context.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetDeviceContextReply { sequence, length, context };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetDeviceContextReply {
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
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        context_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&self.context);
    }
}
impl GetDeviceContextReply {
    /// Get the value of the `context_len` field.
    ///
    /// The `context_len` field is used as the length field of the `context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn context_len(&self) -> u32 {
        self.context.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SetWindowCreateContext request
pub const SET_WINDOW_CREATE_CONTEXT_REQUEST: u8 = 5;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetWindowCreateContextRequest<'input> {
    pub context: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(SetWindowCreateContextRequest<'_>, "SetWindowCreateContextRequest");
impl<'input> SetWindowCreateContextRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        let context_len_bytes = context_len.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_WINDOW_CREATE_CONTEXT_REQUEST,
            0,
            0,
            context_len_bytes[0],
            context_len_bytes[1],
            context_len_bytes[2],
            context_len_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.context.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.context, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_WINDOW_CREATE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (context_len, remaining) = u32::try_parse(value)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(SetWindowCreateContextRequest {
            context: Cow::Borrowed(context),
        })
    }
    /// Clone all borrowed data in this SetWindowCreateContextRequest.
    pub fn into_owned(self) -> SetWindowCreateContextRequest<'static> {
        SetWindowCreateContextRequest {
            context: Cow::Owned(self.context.into_owned()),
        }
    }
}
impl<'input> Request for SetWindowCreateContextRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetWindowCreateContextRequest<'input> {
}

/// Opcode for the GetWindowCreateContext request
pub const GET_WINDOW_CREATE_CONTEXT_REQUEST: u8 = 6;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetWindowCreateContextRequest;
impl_debug_if_no_extra_traits!(GetWindowCreateContextRequest, "GetWindowCreateContextRequest");
impl GetWindowCreateContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            GET_WINDOW_CREATE_CONTEXT_REQUEST,
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
        if header.minor_opcode != GET_WINDOW_CREATE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(GetWindowCreateContextRequest
        )
    }
}
impl Request for GetWindowCreateContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetWindowCreateContextRequest {
    type Reply = GetWindowCreateContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetWindowCreateContextReply {
    pub sequence: u16,
    pub length: u32,
    pub context: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetWindowCreateContextReply, "GetWindowCreateContextReply");
impl TryParse for GetWindowCreateContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (context_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let context = context.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetWindowCreateContextReply { sequence, length, context };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetWindowCreateContextReply {
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
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        context_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&self.context);
    }
}
impl GetWindowCreateContextReply {
    /// Get the value of the `context_len` field.
    ///
    /// The `context_len` field is used as the length field of the `context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn context_len(&self) -> u32 {
        self.context.len()
            .try_into().unwrap()
    }
}

/// Opcode for the GetWindowContext request
pub const GET_WINDOW_CONTEXT_REQUEST: u8 = 7;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetWindowContextRequest {
    pub window: xproto::Window,
}
impl_debug_if_no_extra_traits!(GetWindowContextRequest, "GetWindowContextRequest");
impl GetWindowContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_WINDOW_CONTEXT_REQUEST,
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
        if header.minor_opcode != GET_WINDOW_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let _ = remaining;
        Ok(GetWindowContextRequest {
            window,
        })
    }
}
impl Request for GetWindowContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetWindowContextRequest {
    type Reply = GetWindowContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetWindowContextReply {
    pub sequence: u16,
    pub length: u32,
    pub context: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetWindowContextReply, "GetWindowContextReply");
impl TryParse for GetWindowContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (context_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let context = context.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetWindowContextReply { sequence, length, context };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetWindowContextReply {
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
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        context_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&self.context);
    }
}
impl GetWindowContextReply {
    /// Get the value of the `context_len` field.
    ///
    /// The `context_len` field is used as the length field of the `context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn context_len(&self) -> u32 {
        self.context.len()
            .try_into().unwrap()
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListItem {
    pub name: xproto::Atom,
    pub object_context: Vec<u8>,
    pub data_context: Vec<u8>,
}
impl_debug_if_no_extra_traits!(ListItem, "ListItem");
impl TryParse for ListItem {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let value = remaining;
        let (name, remaining) = xproto::Atom::try_parse(remaining)?;
        let (object_context_len, remaining) = u32::try_parse(remaining)?;
        let (data_context_len, remaining) = u32::try_parse(remaining)?;
        let (object_context, remaining) = crate::x11_utils::parse_u8_list(remaining, object_context_len.try_to_usize()?)?;
        let object_context = object_context.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let (data_context, remaining) = crate::x11_utils::parse_u8_list(remaining, data_context_len.try_to_usize()?)?;
        let data_context = data_context.to_vec();
        // Align offset to multiple of 4
        let offset = remaining.as_ptr() as usize - value.as_ptr() as usize;
        let misalignment = (4 - (offset % 4)) % 4;
        let remaining = remaining.get(misalignment..).ok_or(ParseError::InsufficientData)?;
        let result = ListItem { name, object_context, data_context };
        Ok((result, remaining))
    }
}
impl Serialize for ListItem {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        self.name.serialize_into(bytes);
        let object_context_len = u32::try_from(self.object_context.len()).expect("`object_context` has too many elements");
        object_context_len.serialize_into(bytes);
        let data_context_len = u32::try_from(self.data_context.len()).expect("`data_context` has too many elements");
        data_context_len.serialize_into(bytes);
        bytes.extend_from_slice(&self.object_context);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
        bytes.extend_from_slice(&self.data_context);
        bytes.extend_from_slice(&[0; 3][..(4 - (bytes.len() % 4)) % 4]);
    }
}
impl ListItem {
    /// Get the value of the `object_context_len` field.
    ///
    /// The `object_context_len` field is used as the length field of the `object_context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn object_context_len(&self) -> u32 {
        self.object_context.len()
            .try_into().unwrap()
    }
    /// Get the value of the `data_context_len` field.
    ///
    /// The `data_context_len` field is used as the length field of the `data_context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn data_context_len(&self) -> u32 {
        self.data_context.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SetPropertyCreateContext request
pub const SET_PROPERTY_CREATE_CONTEXT_REQUEST: u8 = 8;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetPropertyCreateContextRequest<'input> {
    pub context: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(SetPropertyCreateContextRequest<'_>, "SetPropertyCreateContextRequest");
impl<'input> SetPropertyCreateContextRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        let context_len_bytes = context_len.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_PROPERTY_CREATE_CONTEXT_REQUEST,
            0,
            0,
            context_len_bytes[0],
            context_len_bytes[1],
            context_len_bytes[2],
            context_len_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.context.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.context, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_PROPERTY_CREATE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (context_len, remaining) = u32::try_parse(value)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(SetPropertyCreateContextRequest {
            context: Cow::Borrowed(context),
        })
    }
    /// Clone all borrowed data in this SetPropertyCreateContextRequest.
    pub fn into_owned(self) -> SetPropertyCreateContextRequest<'static> {
        SetPropertyCreateContextRequest {
            context: Cow::Owned(self.context.into_owned()),
        }
    }
}
impl<'input> Request for SetPropertyCreateContextRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetPropertyCreateContextRequest<'input> {
}

/// Opcode for the GetPropertyCreateContext request
pub const GET_PROPERTY_CREATE_CONTEXT_REQUEST: u8 = 9;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPropertyCreateContextRequest;
impl_debug_if_no_extra_traits!(GetPropertyCreateContextRequest, "GetPropertyCreateContextRequest");
impl GetPropertyCreateContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            GET_PROPERTY_CREATE_CONTEXT_REQUEST,
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
        if header.minor_opcode != GET_PROPERTY_CREATE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(GetPropertyCreateContextRequest
        )
    }
}
impl Request for GetPropertyCreateContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetPropertyCreateContextRequest {
    type Reply = GetPropertyCreateContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPropertyCreateContextReply {
    pub sequence: u16,
    pub length: u32,
    pub context: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetPropertyCreateContextReply, "GetPropertyCreateContextReply");
impl TryParse for GetPropertyCreateContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (context_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let context = context.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetPropertyCreateContextReply { sequence, length, context };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetPropertyCreateContextReply {
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
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        context_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&self.context);
    }
}
impl GetPropertyCreateContextReply {
    /// Get the value of the `context_len` field.
    ///
    /// The `context_len` field is used as the length field of the `context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn context_len(&self) -> u32 {
        self.context.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SetPropertyUseContext request
pub const SET_PROPERTY_USE_CONTEXT_REQUEST: u8 = 10;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetPropertyUseContextRequest<'input> {
    pub context: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(SetPropertyUseContextRequest<'_>, "SetPropertyUseContextRequest");
impl<'input> SetPropertyUseContextRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        let context_len_bytes = context_len.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_PROPERTY_USE_CONTEXT_REQUEST,
            0,
            0,
            context_len_bytes[0],
            context_len_bytes[1],
            context_len_bytes[2],
            context_len_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.context.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.context, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_PROPERTY_USE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (context_len, remaining) = u32::try_parse(value)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(SetPropertyUseContextRequest {
            context: Cow::Borrowed(context),
        })
    }
    /// Clone all borrowed data in this SetPropertyUseContextRequest.
    pub fn into_owned(self) -> SetPropertyUseContextRequest<'static> {
        SetPropertyUseContextRequest {
            context: Cow::Owned(self.context.into_owned()),
        }
    }
}
impl<'input> Request for SetPropertyUseContextRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetPropertyUseContextRequest<'input> {
}

/// Opcode for the GetPropertyUseContext request
pub const GET_PROPERTY_USE_CONTEXT_REQUEST: u8 = 11;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPropertyUseContextRequest;
impl_debug_if_no_extra_traits!(GetPropertyUseContextRequest, "GetPropertyUseContextRequest");
impl GetPropertyUseContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            GET_PROPERTY_USE_CONTEXT_REQUEST,
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
        if header.minor_opcode != GET_PROPERTY_USE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(GetPropertyUseContextRequest
        )
    }
}
impl Request for GetPropertyUseContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetPropertyUseContextRequest {
    type Reply = GetPropertyUseContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPropertyUseContextReply {
    pub sequence: u16,
    pub length: u32,
    pub context: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetPropertyUseContextReply, "GetPropertyUseContextReply");
impl TryParse for GetPropertyUseContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (context_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let context = context.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetPropertyUseContextReply { sequence, length, context };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetPropertyUseContextReply {
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
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        context_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&self.context);
    }
}
impl GetPropertyUseContextReply {
    /// Get the value of the `context_len` field.
    ///
    /// The `context_len` field is used as the length field of the `context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn context_len(&self) -> u32 {
        self.context.len()
            .try_into().unwrap()
    }
}

/// Opcode for the GetPropertyContext request
pub const GET_PROPERTY_CONTEXT_REQUEST: u8 = 12;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPropertyContextRequest {
    pub window: xproto::Window,
    pub property: xproto::Atom,
}
impl_debug_if_no_extra_traits!(GetPropertyContextRequest, "GetPropertyContextRequest");
impl GetPropertyContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let property_bytes = self.property.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_PROPERTY_CONTEXT_REQUEST,
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
        if header.minor_opcode != GET_PROPERTY_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (property, remaining) = xproto::Atom::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetPropertyContextRequest {
            window,
            property,
        })
    }
}
impl Request for GetPropertyContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetPropertyContextRequest {
    type Reply = GetPropertyContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPropertyContextReply {
    pub sequence: u16,
    pub length: u32,
    pub context: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetPropertyContextReply, "GetPropertyContextReply");
impl TryParse for GetPropertyContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (context_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let context = context.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetPropertyContextReply { sequence, length, context };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetPropertyContextReply {
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
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        context_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&self.context);
    }
}
impl GetPropertyContextReply {
    /// Get the value of the `context_len` field.
    ///
    /// The `context_len` field is used as the length field of the `context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn context_len(&self) -> u32 {
        self.context.len()
            .try_into().unwrap()
    }
}

/// Opcode for the GetPropertyDataContext request
pub const GET_PROPERTY_DATA_CONTEXT_REQUEST: u8 = 13;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPropertyDataContextRequest {
    pub window: xproto::Window,
    pub property: xproto::Atom,
}
impl_debug_if_no_extra_traits!(GetPropertyDataContextRequest, "GetPropertyDataContextRequest");
impl GetPropertyDataContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let property_bytes = self.property.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_PROPERTY_DATA_CONTEXT_REQUEST,
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
        if header.minor_opcode != GET_PROPERTY_DATA_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (property, remaining) = xproto::Atom::try_parse(remaining)?;
        let _ = remaining;
        Ok(GetPropertyDataContextRequest {
            window,
            property,
        })
    }
}
impl Request for GetPropertyDataContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetPropertyDataContextRequest {
    type Reply = GetPropertyDataContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetPropertyDataContextReply {
    pub sequence: u16,
    pub length: u32,
    pub context: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetPropertyDataContextReply, "GetPropertyDataContextReply");
impl TryParse for GetPropertyDataContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (context_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let context = context.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetPropertyDataContextReply { sequence, length, context };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetPropertyDataContextReply {
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
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        context_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&self.context);
    }
}
impl GetPropertyDataContextReply {
    /// Get the value of the `context_len` field.
    ///
    /// The `context_len` field is used as the length field of the `context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn context_len(&self) -> u32 {
        self.context.len()
            .try_into().unwrap()
    }
}

/// Opcode for the ListProperties request
pub const LIST_PROPERTIES_REQUEST: u8 = 14;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListPropertiesRequest {
    pub window: xproto::Window,
}
impl_debug_if_no_extra_traits!(ListPropertiesRequest, "ListPropertiesRequest");
impl ListPropertiesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let mut request0 = vec![
            major_opcode,
            LIST_PROPERTIES_REQUEST,
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
        if header.minor_opcode != LIST_PROPERTIES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let _ = remaining;
        Ok(ListPropertiesRequest {
            window,
        })
    }
}
impl Request for ListPropertiesRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
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
    pub properties: Vec<ListItem>,
}
impl_debug_if_no_extra_traits!(ListPropertiesReply, "ListPropertiesReply");
impl TryParse for ListPropertiesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (properties_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (properties, remaining) = crate::x11_utils::parse_list::<ListItem>(remaining, properties_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = ListPropertiesReply { sequence, length, properties };
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
        let properties_len = u32::try_from(self.properties.len()).expect("`properties` has too many elements");
        properties_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        self.properties.serialize_into(bytes);
    }
}
impl ListPropertiesReply {
    /// Get the value of the `properties_len` field.
    ///
    /// The `properties_len` field is used as the length field of the `properties` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn properties_len(&self) -> u32 {
        self.properties.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SetSelectionCreateContext request
pub const SET_SELECTION_CREATE_CONTEXT_REQUEST: u8 = 15;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetSelectionCreateContextRequest<'input> {
    pub context: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(SetSelectionCreateContextRequest<'_>, "SetSelectionCreateContextRequest");
impl<'input> SetSelectionCreateContextRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        let context_len_bytes = context_len.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_SELECTION_CREATE_CONTEXT_REQUEST,
            0,
            0,
            context_len_bytes[0],
            context_len_bytes[1],
            context_len_bytes[2],
            context_len_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.context.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.context, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_SELECTION_CREATE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (context_len, remaining) = u32::try_parse(value)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(SetSelectionCreateContextRequest {
            context: Cow::Borrowed(context),
        })
    }
    /// Clone all borrowed data in this SetSelectionCreateContextRequest.
    pub fn into_owned(self) -> SetSelectionCreateContextRequest<'static> {
        SetSelectionCreateContextRequest {
            context: Cow::Owned(self.context.into_owned()),
        }
    }
}
impl<'input> Request for SetSelectionCreateContextRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetSelectionCreateContextRequest<'input> {
}

/// Opcode for the GetSelectionCreateContext request
pub const GET_SELECTION_CREATE_CONTEXT_REQUEST: u8 = 16;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetSelectionCreateContextRequest;
impl_debug_if_no_extra_traits!(GetSelectionCreateContextRequest, "GetSelectionCreateContextRequest");
impl GetSelectionCreateContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            GET_SELECTION_CREATE_CONTEXT_REQUEST,
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
        if header.minor_opcode != GET_SELECTION_CREATE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(GetSelectionCreateContextRequest
        )
    }
}
impl Request for GetSelectionCreateContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetSelectionCreateContextRequest {
    type Reply = GetSelectionCreateContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetSelectionCreateContextReply {
    pub sequence: u16,
    pub length: u32,
    pub context: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetSelectionCreateContextReply, "GetSelectionCreateContextReply");
impl TryParse for GetSelectionCreateContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (context_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let context = context.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetSelectionCreateContextReply { sequence, length, context };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetSelectionCreateContextReply {
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
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        context_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&self.context);
    }
}
impl GetSelectionCreateContextReply {
    /// Get the value of the `context_len` field.
    ///
    /// The `context_len` field is used as the length field of the `context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn context_len(&self) -> u32 {
        self.context.len()
            .try_into().unwrap()
    }
}

/// Opcode for the SetSelectionUseContext request
pub const SET_SELECTION_USE_CONTEXT_REQUEST: u8 = 17;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetSelectionUseContextRequest<'input> {
    pub context: Cow<'input, [u8]>,
}
impl_debug_if_no_extra_traits!(SetSelectionUseContextRequest<'_>, "SetSelectionUseContextRequest");
impl<'input> SetSelectionUseContextRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        let context_len_bytes = context_len.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_SELECTION_USE_CONTEXT_REQUEST,
            0,
            0,
            context_len_bytes[0],
            context_len_bytes[1],
            context_len_bytes[2],
            context_len_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let length_so_far = length_so_far + self.context.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), self.context, padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SET_SELECTION_USE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (context_len, remaining) = u32::try_parse(value)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let _ = remaining;
        Ok(SetSelectionUseContextRequest {
            context: Cow::Borrowed(context),
        })
    }
    /// Clone all borrowed data in this SetSelectionUseContextRequest.
    pub fn into_owned(self) -> SetSelectionUseContextRequest<'static> {
        SetSelectionUseContextRequest {
            context: Cow::Owned(self.context.into_owned()),
        }
    }
}
impl<'input> Request for SetSelectionUseContextRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetSelectionUseContextRequest<'input> {
}

/// Opcode for the GetSelectionUseContext request
pub const GET_SELECTION_USE_CONTEXT_REQUEST: u8 = 18;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetSelectionUseContextRequest;
impl_debug_if_no_extra_traits!(GetSelectionUseContextRequest, "GetSelectionUseContextRequest");
impl GetSelectionUseContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            GET_SELECTION_USE_CONTEXT_REQUEST,
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
        if header.minor_opcode != GET_SELECTION_USE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(GetSelectionUseContextRequest
        )
    }
}
impl Request for GetSelectionUseContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetSelectionUseContextRequest {
    type Reply = GetSelectionUseContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetSelectionUseContextReply {
    pub sequence: u16,
    pub length: u32,
    pub context: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetSelectionUseContextReply, "GetSelectionUseContextReply");
impl TryParse for GetSelectionUseContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (context_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let context = context.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetSelectionUseContextReply { sequence, length, context };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetSelectionUseContextReply {
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
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        context_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&self.context);
    }
}
impl GetSelectionUseContextReply {
    /// Get the value of the `context_len` field.
    ///
    /// The `context_len` field is used as the length field of the `context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn context_len(&self) -> u32 {
        self.context.len()
            .try_into().unwrap()
    }
}

/// Opcode for the GetSelectionContext request
pub const GET_SELECTION_CONTEXT_REQUEST: u8 = 19;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetSelectionContextRequest {
    pub selection: xproto::Atom,
}
impl_debug_if_no_extra_traits!(GetSelectionContextRequest, "GetSelectionContextRequest");
impl GetSelectionContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let selection_bytes = self.selection.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_SELECTION_CONTEXT_REQUEST,
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
        if header.minor_opcode != GET_SELECTION_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (selection, remaining) = xproto::Atom::try_parse(value)?;
        let _ = remaining;
        Ok(GetSelectionContextRequest {
            selection,
        })
    }
}
impl Request for GetSelectionContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetSelectionContextRequest {
    type Reply = GetSelectionContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetSelectionContextReply {
    pub sequence: u16,
    pub length: u32,
    pub context: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetSelectionContextReply, "GetSelectionContextReply");
impl TryParse for GetSelectionContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (context_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let context = context.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetSelectionContextReply { sequence, length, context };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetSelectionContextReply {
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
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        context_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&self.context);
    }
}
impl GetSelectionContextReply {
    /// Get the value of the `context_len` field.
    ///
    /// The `context_len` field is used as the length field of the `context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn context_len(&self) -> u32 {
        self.context.len()
            .try_into().unwrap()
    }
}

/// Opcode for the GetSelectionDataContext request
pub const GET_SELECTION_DATA_CONTEXT_REQUEST: u8 = 20;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetSelectionDataContextRequest {
    pub selection: xproto::Atom,
}
impl_debug_if_no_extra_traits!(GetSelectionDataContextRequest, "GetSelectionDataContextRequest");
impl GetSelectionDataContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let selection_bytes = self.selection.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_SELECTION_DATA_CONTEXT_REQUEST,
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
        if header.minor_opcode != GET_SELECTION_DATA_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (selection, remaining) = xproto::Atom::try_parse(value)?;
        let _ = remaining;
        Ok(GetSelectionDataContextRequest {
            selection,
        })
    }
}
impl Request for GetSelectionDataContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetSelectionDataContextRequest {
    type Reply = GetSelectionDataContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetSelectionDataContextReply {
    pub sequence: u16,
    pub length: u32,
    pub context: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetSelectionDataContextReply, "GetSelectionDataContextReply");
impl TryParse for GetSelectionDataContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (context_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let context = context.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetSelectionDataContextReply { sequence, length, context };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetSelectionDataContextReply {
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
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        context_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&self.context);
    }
}
impl GetSelectionDataContextReply {
    /// Get the value of the `context_len` field.
    ///
    /// The `context_len` field is used as the length field of the `context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn context_len(&self) -> u32 {
        self.context.len()
            .try_into().unwrap()
    }
}

/// Opcode for the ListSelections request
pub const LIST_SELECTIONS_REQUEST: u8 = 21;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListSelectionsRequest;
impl_debug_if_no_extra_traits!(ListSelectionsRequest, "ListSelectionsRequest");
impl ListSelectionsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            LIST_SELECTIONS_REQUEST,
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
        if header.minor_opcode != LIST_SELECTIONS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(ListSelectionsRequest
        )
    }
}
impl Request for ListSelectionsRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for ListSelectionsRequest {
    type Reply = ListSelectionsReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListSelectionsReply {
    pub sequence: u16,
    pub length: u32,
    pub selections: Vec<ListItem>,
}
impl_debug_if_no_extra_traits!(ListSelectionsReply, "ListSelectionsReply");
impl TryParse for ListSelectionsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (selections_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (selections, remaining) = crate::x11_utils::parse_list::<ListItem>(remaining, selections_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = ListSelectionsReply { sequence, length, selections };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ListSelectionsReply {
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
        let selections_len = u32::try_from(self.selections.len()).expect("`selections` has too many elements");
        selections_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        self.selections.serialize_into(bytes);
    }
}
impl ListSelectionsReply {
    /// Get the value of the `selections_len` field.
    ///
    /// The `selections_len` field is used as the length field of the `selections` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn selections_len(&self) -> u32 {
        self.selections.len()
            .try_into().unwrap()
    }
}

/// Opcode for the GetClientContext request
pub const GET_CLIENT_CONTEXT_REQUEST: u8 = 22;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetClientContextRequest {
    pub resource: u32,
}
impl_debug_if_no_extra_traits!(GetClientContextRequest, "GetClientContextRequest");
impl GetClientContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let resource_bytes = self.resource.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_CLIENT_CONTEXT_REQUEST,
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
        if header.minor_opcode != GET_CLIENT_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (resource, remaining) = u32::try_parse(value)?;
        let _ = remaining;
        Ok(GetClientContextRequest {
            resource,
        })
    }
}
impl Request for GetClientContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetClientContextRequest {
    type Reply = GetClientContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetClientContextReply {
    pub sequence: u16,
    pub length: u32,
    pub context: Vec<u8>,
}
impl_debug_if_no_extra_traits!(GetClientContextReply, "GetClientContextReply");
impl TryParse for GetClientContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (context_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (context, remaining) = crate::x11_utils::parse_u8_list(remaining, context_len.try_to_usize()?)?;
        let context = context.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetClientContextReply { sequence, length, context };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetClientContextReply {
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
        let context_len = u32::try_from(self.context.len()).expect("`context` has too many elements");
        context_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        bytes.extend_from_slice(&self.context);
    }
}
impl GetClientContextReply {
    /// Get the value of the `context_len` field.
    ///
    /// The `context_len` field is used as the length field of the `context` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn context_len(&self) -> u32 {
        self.context.len()
            .try_into().unwrap()
    }
}

