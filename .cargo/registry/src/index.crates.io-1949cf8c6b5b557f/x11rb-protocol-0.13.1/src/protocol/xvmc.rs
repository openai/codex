// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `XvMC` X11 extension.

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
use super::xv;

/// The X11 name of the extension for QueryExtension
pub const X11_EXTENSION_NAME: &str = "XVideo-MotionCompensation";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (1, 1);

pub type Context = u32;

pub type Surface = u32;

pub type Subpicture = u32;

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SurfaceInfo {
    pub id: Surface,
    pub chroma_format: u16,
    pub pad0: u16,
    pub max_width: u16,
    pub max_height: u16,
    pub subpicture_max_width: u16,
    pub subpicture_max_height: u16,
    pub mc_type: u32,
    pub flags: u32,
}
impl_debug_if_no_extra_traits!(SurfaceInfo, "SurfaceInfo");
impl TryParse for SurfaceInfo {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (id, remaining) = Surface::try_parse(remaining)?;
        let (chroma_format, remaining) = u16::try_parse(remaining)?;
        let (pad0, remaining) = u16::try_parse(remaining)?;
        let (max_width, remaining) = u16::try_parse(remaining)?;
        let (max_height, remaining) = u16::try_parse(remaining)?;
        let (subpicture_max_width, remaining) = u16::try_parse(remaining)?;
        let (subpicture_max_height, remaining) = u16::try_parse(remaining)?;
        let (mc_type, remaining) = u32::try_parse(remaining)?;
        let (flags, remaining) = u32::try_parse(remaining)?;
        let result = SurfaceInfo { id, chroma_format, pad0, max_width, max_height, subpicture_max_width, subpicture_max_height, mc_type, flags };
        Ok((result, remaining))
    }
}
impl Serialize for SurfaceInfo {
    type Bytes = [u8; 24];
    fn serialize(&self) -> [u8; 24] {
        let id_bytes = self.id.serialize();
        let chroma_format_bytes = self.chroma_format.serialize();
        let pad0_bytes = self.pad0.serialize();
        let max_width_bytes = self.max_width.serialize();
        let max_height_bytes = self.max_height.serialize();
        let subpicture_max_width_bytes = self.subpicture_max_width.serialize();
        let subpicture_max_height_bytes = self.subpicture_max_height.serialize();
        let mc_type_bytes = self.mc_type.serialize();
        let flags_bytes = self.flags.serialize();
        [
            id_bytes[0],
            id_bytes[1],
            id_bytes[2],
            id_bytes[3],
            chroma_format_bytes[0],
            chroma_format_bytes[1],
            pad0_bytes[0],
            pad0_bytes[1],
            max_width_bytes[0],
            max_width_bytes[1],
            max_height_bytes[0],
            max_height_bytes[1],
            subpicture_max_width_bytes[0],
            subpicture_max_width_bytes[1],
            subpicture_max_height_bytes[0],
            subpicture_max_height_bytes[1],
            mc_type_bytes[0],
            mc_type_bytes[1],
            mc_type_bytes[2],
            mc_type_bytes[3],
            flags_bytes[0],
            flags_bytes[1],
            flags_bytes[2],
            flags_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(24);
        self.id.serialize_into(bytes);
        self.chroma_format.serialize_into(bytes);
        self.pad0.serialize_into(bytes);
        self.max_width.serialize_into(bytes);
        self.max_height.serialize_into(bytes);
        self.subpicture_max_width.serialize_into(bytes);
        self.subpicture_max_height.serialize_into(bytes);
        self.mc_type.serialize_into(bytes);
        self.flags.serialize_into(bytes);
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
    pub major: u32,
    pub minor: u32,
}
impl_debug_if_no_extra_traits!(QueryVersionReply, "QueryVersionReply");
impl TryParse for QueryVersionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (major, remaining) = u32::try_parse(remaining)?;
        let (minor, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryVersionReply { sequence, length, major, minor };
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
            major_bytes[2],
            major_bytes[3],
            minor_bytes[0],
            minor_bytes[1],
            minor_bytes[2],
            minor_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(16);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.major.serialize_into(bytes);
        self.minor.serialize_into(bytes);
    }
}

/// Opcode for the ListSurfaceTypes request
pub const LIST_SURFACE_TYPES_REQUEST: u8 = 1;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListSurfaceTypesRequest {
    pub port_id: xv::Port,
}
impl_debug_if_no_extra_traits!(ListSurfaceTypesRequest, "ListSurfaceTypesRequest");
impl ListSurfaceTypesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_id_bytes = self.port_id.serialize();
        let mut request0 = vec![
            major_opcode,
            LIST_SURFACE_TYPES_REQUEST,
            0,
            0,
            port_id_bytes[0],
            port_id_bytes[1],
            port_id_bytes[2],
            port_id_bytes[3],
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
        if header.minor_opcode != LIST_SURFACE_TYPES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port_id, remaining) = xv::Port::try_parse(value)?;
        let _ = remaining;
        Ok(ListSurfaceTypesRequest {
            port_id,
        })
    }
}
impl Request for ListSurfaceTypesRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for ListSurfaceTypesRequest {
    type Reply = ListSurfaceTypesReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListSurfaceTypesReply {
    pub sequence: u16,
    pub length: u32,
    pub surfaces: Vec<SurfaceInfo>,
}
impl_debug_if_no_extra_traits!(ListSurfaceTypesReply, "ListSurfaceTypesReply");
impl TryParse for ListSurfaceTypesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (num, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (surfaces, remaining) = crate::x11_utils::parse_list::<SurfaceInfo>(remaining, num.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = ListSurfaceTypesReply { sequence, length, surfaces };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ListSurfaceTypesReply {
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
        let num = u32::try_from(self.surfaces.len()).expect("`surfaces` has too many elements");
        num.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        self.surfaces.serialize_into(bytes);
    }
}
impl ListSurfaceTypesReply {
    /// Get the value of the `num` field.
    ///
    /// The `num` field is used as the length field of the `surfaces` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num(&self) -> u32 {
        self.surfaces.len()
            .try_into().unwrap()
    }
}

/// Opcode for the CreateContext request
pub const CREATE_CONTEXT_REQUEST: u8 = 2;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateContextRequest {
    pub context_id: Context,
    pub port_id: xv::Port,
    pub surface_id: Surface,
    pub width: u16,
    pub height: u16,
    pub flags: u32,
}
impl_debug_if_no_extra_traits!(CreateContextRequest, "CreateContextRequest");
impl CreateContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let context_id_bytes = self.context_id.serialize();
        let port_id_bytes = self.port_id.serialize();
        let surface_id_bytes = self.surface_id.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let flags_bytes = self.flags.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_CONTEXT_REQUEST,
            0,
            0,
            context_id_bytes[0],
            context_id_bytes[1],
            context_id_bytes[2],
            context_id_bytes[3],
            port_id_bytes[0],
            port_id_bytes[1],
            port_id_bytes[2],
            port_id_bytes[3],
            surface_id_bytes[0],
            surface_id_bytes[1],
            surface_id_bytes[2],
            surface_id_bytes[3],
            width_bytes[0],
            width_bytes[1],
            height_bytes[0],
            height_bytes[1],
            flags_bytes[0],
            flags_bytes[1],
            flags_bytes[2],
            flags_bytes[3],
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
        let (context_id, remaining) = Context::try_parse(value)?;
        let (port_id, remaining) = xv::Port::try_parse(remaining)?;
        let (surface_id, remaining) = Surface::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (flags, remaining) = u32::try_parse(remaining)?;
        let _ = remaining;
        Ok(CreateContextRequest {
            context_id,
            port_id,
            surface_id,
            width,
            height,
            flags,
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

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateContextReply {
    pub sequence: u16,
    pub width_actual: u16,
    pub height_actual: u16,
    pub flags_return: u32,
    pub priv_data: Vec<u32>,
}
impl_debug_if_no_extra_traits!(CreateContextReply, "CreateContextReply");
impl TryParse for CreateContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (width_actual, remaining) = u16::try_parse(remaining)?;
        let (height_actual, remaining) = u16::try_parse(remaining)?;
        let (flags_return, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (priv_data, remaining) = crate::x11_utils::parse_list::<u32>(remaining, length.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = CreateContextReply { sequence, width_actual, height_actual, flags_return, priv_data };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for CreateContextReply {
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
        let length = u32::try_from(self.priv_data.len()).expect("`priv_data` has too many elements");
        length.serialize_into(bytes);
        self.width_actual.serialize_into(bytes);
        self.height_actual.serialize_into(bytes);
        self.flags_return.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        self.priv_data.serialize_into(bytes);
    }
}
impl CreateContextReply {
    /// Get the value of the `length` field.
    ///
    /// The `length` field is used as the length field of the `priv_data` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn length(&self) -> u32 {
        self.priv_data.len()
            .try_into().unwrap()
    }
}

/// Opcode for the DestroyContext request
pub const DESTROY_CONTEXT_REQUEST: u8 = 3;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DestroyContextRequest {
    pub context_id: Context,
}
impl_debug_if_no_extra_traits!(DestroyContextRequest, "DestroyContextRequest");
impl DestroyContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let context_id_bytes = self.context_id.serialize();
        let mut request0 = vec![
            major_opcode,
            DESTROY_CONTEXT_REQUEST,
            0,
            0,
            context_id_bytes[0],
            context_id_bytes[1],
            context_id_bytes[2],
            context_id_bytes[3],
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
        let (context_id, remaining) = Context::try_parse(value)?;
        let _ = remaining;
        Ok(DestroyContextRequest {
            context_id,
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

/// Opcode for the CreateSurface request
pub const CREATE_SURFACE_REQUEST: u8 = 4;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateSurfaceRequest {
    pub surface_id: Surface,
    pub context_id: Context,
}
impl_debug_if_no_extra_traits!(CreateSurfaceRequest, "CreateSurfaceRequest");
impl CreateSurfaceRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let surface_id_bytes = self.surface_id.serialize();
        let context_id_bytes = self.context_id.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_SURFACE_REQUEST,
            0,
            0,
            surface_id_bytes[0],
            surface_id_bytes[1],
            surface_id_bytes[2],
            surface_id_bytes[3],
            context_id_bytes[0],
            context_id_bytes[1],
            context_id_bytes[2],
            context_id_bytes[3],
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
        if header.minor_opcode != CREATE_SURFACE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (surface_id, remaining) = Surface::try_parse(value)?;
        let (context_id, remaining) = Context::try_parse(remaining)?;
        let _ = remaining;
        Ok(CreateSurfaceRequest {
            surface_id,
            context_id,
        })
    }
}
impl Request for CreateSurfaceRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for CreateSurfaceRequest {
    type Reply = CreateSurfaceReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateSurfaceReply {
    pub sequence: u16,
    pub priv_data: Vec<u32>,
}
impl_debug_if_no_extra_traits!(CreateSurfaceReply, "CreateSurfaceReply");
impl TryParse for CreateSurfaceReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(24..).ok_or(ParseError::InsufficientData)?;
        let (priv_data, remaining) = crate::x11_utils::parse_list::<u32>(remaining, length.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = CreateSurfaceReply { sequence, priv_data };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for CreateSurfaceReply {
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
        let length = u32::try_from(self.priv_data.len()).expect("`priv_data` has too many elements");
        length.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 24]);
        self.priv_data.serialize_into(bytes);
    }
}
impl CreateSurfaceReply {
    /// Get the value of the `length` field.
    ///
    /// The `length` field is used as the length field of the `priv_data` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn length(&self) -> u32 {
        self.priv_data.len()
            .try_into().unwrap()
    }
}

/// Opcode for the DestroySurface request
pub const DESTROY_SURFACE_REQUEST: u8 = 5;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DestroySurfaceRequest {
    pub surface_id: Surface,
}
impl_debug_if_no_extra_traits!(DestroySurfaceRequest, "DestroySurfaceRequest");
impl DestroySurfaceRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let surface_id_bytes = self.surface_id.serialize();
        let mut request0 = vec![
            major_opcode,
            DESTROY_SURFACE_REQUEST,
            0,
            0,
            surface_id_bytes[0],
            surface_id_bytes[1],
            surface_id_bytes[2],
            surface_id_bytes[3],
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
        if header.minor_opcode != DESTROY_SURFACE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (surface_id, remaining) = Surface::try_parse(value)?;
        let _ = remaining;
        Ok(DestroySurfaceRequest {
            surface_id,
        })
    }
}
impl Request for DestroySurfaceRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DestroySurfaceRequest {
}

/// Opcode for the CreateSubpicture request
pub const CREATE_SUBPICTURE_REQUEST: u8 = 6;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateSubpictureRequest {
    pub subpicture_id: Subpicture,
    pub context: Context,
    pub xvimage_id: u32,
    pub width: u16,
    pub height: u16,
}
impl_debug_if_no_extra_traits!(CreateSubpictureRequest, "CreateSubpictureRequest");
impl CreateSubpictureRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let subpicture_id_bytes = self.subpicture_id.serialize();
        let context_bytes = self.context.serialize();
        let xvimage_id_bytes = self.xvimage_id.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_SUBPICTURE_REQUEST,
            0,
            0,
            subpicture_id_bytes[0],
            subpicture_id_bytes[1],
            subpicture_id_bytes[2],
            subpicture_id_bytes[3],
            context_bytes[0],
            context_bytes[1],
            context_bytes[2],
            context_bytes[3],
            xvimage_id_bytes[0],
            xvimage_id_bytes[1],
            xvimage_id_bytes[2],
            xvimage_id_bytes[3],
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
        if header.minor_opcode != CREATE_SUBPICTURE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (subpicture_id, remaining) = Subpicture::try_parse(value)?;
        let (context, remaining) = Context::try_parse(remaining)?;
        let (xvimage_id, remaining) = u32::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let _ = remaining;
        Ok(CreateSubpictureRequest {
            subpicture_id,
            context,
            xvimage_id,
            width,
            height,
        })
    }
}
impl Request for CreateSubpictureRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for CreateSubpictureRequest {
    type Reply = CreateSubpictureReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateSubpictureReply {
    pub sequence: u16,
    pub width_actual: u16,
    pub height_actual: u16,
    pub num_palette_entries: u16,
    pub entry_bytes: u16,
    pub component_order: [u8; 4],
    pub priv_data: Vec<u32>,
}
impl_debug_if_no_extra_traits!(CreateSubpictureReply, "CreateSubpictureReply");
impl TryParse for CreateSubpictureReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (width_actual, remaining) = u16::try_parse(remaining)?;
        let (height_actual, remaining) = u16::try_parse(remaining)?;
        let (num_palette_entries, remaining) = u16::try_parse(remaining)?;
        let (entry_bytes, remaining) = u16::try_parse(remaining)?;
        let (component_order, remaining) = crate::x11_utils::parse_u8_array::<4>(remaining)?;
        let remaining = remaining.get(12..).ok_or(ParseError::InsufficientData)?;
        let (priv_data, remaining) = crate::x11_utils::parse_list::<u32>(remaining, length.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = CreateSubpictureReply { sequence, width_actual, height_actual, num_palette_entries, entry_bytes, component_order, priv_data };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for CreateSubpictureReply {
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
        let length = u32::try_from(self.priv_data.len()).expect("`priv_data` has too many elements");
        length.serialize_into(bytes);
        self.width_actual.serialize_into(bytes);
        self.height_actual.serialize_into(bytes);
        self.num_palette_entries.serialize_into(bytes);
        self.entry_bytes.serialize_into(bytes);
        bytes.extend_from_slice(&self.component_order);
        bytes.extend_from_slice(&[0; 12]);
        self.priv_data.serialize_into(bytes);
    }
}
impl CreateSubpictureReply {
    /// Get the value of the `length` field.
    ///
    /// The `length` field is used as the length field of the `priv_data` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn length(&self) -> u32 {
        self.priv_data.len()
            .try_into().unwrap()
    }
}

/// Opcode for the DestroySubpicture request
pub const DESTROY_SUBPICTURE_REQUEST: u8 = 7;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DestroySubpictureRequest {
    pub subpicture_id: Subpicture,
}
impl_debug_if_no_extra_traits!(DestroySubpictureRequest, "DestroySubpictureRequest");
impl DestroySubpictureRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let subpicture_id_bytes = self.subpicture_id.serialize();
        let mut request0 = vec![
            major_opcode,
            DESTROY_SUBPICTURE_REQUEST,
            0,
            0,
            subpicture_id_bytes[0],
            subpicture_id_bytes[1],
            subpicture_id_bytes[2],
            subpicture_id_bytes[3],
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
        if header.minor_opcode != DESTROY_SUBPICTURE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (subpicture_id, remaining) = Subpicture::try_parse(value)?;
        let _ = remaining;
        Ok(DestroySubpictureRequest {
            subpicture_id,
        })
    }
}
impl Request for DestroySubpictureRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DestroySubpictureRequest {
}

/// Opcode for the ListSubpictureTypes request
pub const LIST_SUBPICTURE_TYPES_REQUEST: u8 = 8;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListSubpictureTypesRequest {
    pub port_id: xv::Port,
    pub surface_id: Surface,
}
impl_debug_if_no_extra_traits!(ListSubpictureTypesRequest, "ListSubpictureTypesRequest");
impl ListSubpictureTypesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let port_id_bytes = self.port_id.serialize();
        let surface_id_bytes = self.surface_id.serialize();
        let mut request0 = vec![
            major_opcode,
            LIST_SUBPICTURE_TYPES_REQUEST,
            0,
            0,
            port_id_bytes[0],
            port_id_bytes[1],
            port_id_bytes[2],
            port_id_bytes[3],
            surface_id_bytes[0],
            surface_id_bytes[1],
            surface_id_bytes[2],
            surface_id_bytes[3],
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
        if header.minor_opcode != LIST_SUBPICTURE_TYPES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (port_id, remaining) = xv::Port::try_parse(value)?;
        let (surface_id, remaining) = Surface::try_parse(remaining)?;
        let _ = remaining;
        Ok(ListSubpictureTypesRequest {
            port_id,
            surface_id,
        })
    }
}
impl Request for ListSubpictureTypesRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for ListSubpictureTypesRequest {
    type Reply = ListSubpictureTypesReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListSubpictureTypesReply {
    pub sequence: u16,
    pub length: u32,
    pub types: Vec<xv::ImageFormatInfo>,
}
impl_debug_if_no_extra_traits!(ListSubpictureTypesReply, "ListSubpictureTypesReply");
impl TryParse for ListSubpictureTypesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (num, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (types, remaining) = crate::x11_utils::parse_list::<xv::ImageFormatInfo>(remaining, num.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = ListSubpictureTypesReply { sequence, length, types };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for ListSubpictureTypesReply {
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
        let num = u32::try_from(self.types.len()).expect("`types` has too many elements");
        num.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        self.types.serialize_into(bytes);
    }
}
impl ListSubpictureTypesReply {
    /// Get the value of the `num` field.
    ///
    /// The `num` field is used as the length field of the `types` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num(&self) -> u32 {
        self.types.len()
            .try_into().unwrap()
    }
}

