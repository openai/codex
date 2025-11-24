// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Shape` X11 extension.

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
pub const X11_EXTENSION_NAME: &str = "SHAPE";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (1, 1);

pub type Op = u8;

pub type Kind = u8;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SO(u8);
impl SO {
    pub const SET: Self = Self(0);
    pub const UNION: Self = Self(1);
    pub const INTERSECT: Self = Self(2);
    pub const SUBTRACT: Self = Self(3);
    pub const INVERT: Self = Self(4);
}
impl From<SO> for u8 {
    #[inline]
    fn from(input: SO) -> Self {
        input.0
    }
}
impl From<SO> for Option<u8> {
    #[inline]
    fn from(input: SO) -> Self {
        Some(input.0)
    }
}
impl From<SO> for u16 {
    #[inline]
    fn from(input: SO) -> Self {
        u16::from(input.0)
    }
}
impl From<SO> for Option<u16> {
    #[inline]
    fn from(input: SO) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SO> for u32 {
    #[inline]
    fn from(input: SO) -> Self {
        u32::from(input.0)
    }
}
impl From<SO> for Option<u32> {
    #[inline]
    fn from(input: SO) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SO {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SO  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::SET.0.into(), "SET", "Set"),
            (Self::UNION.0.into(), "UNION", "Union"),
            (Self::INTERSECT.0.into(), "INTERSECT", "Intersect"),
            (Self::SUBTRACT.0.into(), "SUBTRACT", "Subtract"),
            (Self::INVERT.0.into(), "INVERT", "Invert"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SK(u8);
impl SK {
    pub const BOUNDING: Self = Self(0);
    pub const CLIP: Self = Self(1);
    pub const INPUT: Self = Self(2);
}
impl From<SK> for u8 {
    #[inline]
    fn from(input: SK) -> Self {
        input.0
    }
}
impl From<SK> for Option<u8> {
    #[inline]
    fn from(input: SK) -> Self {
        Some(input.0)
    }
}
impl From<SK> for u16 {
    #[inline]
    fn from(input: SK) -> Self {
        u16::from(input.0)
    }
}
impl From<SK> for Option<u16> {
    #[inline]
    fn from(input: SK) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SK> for u32 {
    #[inline]
    fn from(input: SK) -> Self {
        u32::from(input.0)
    }
}
impl From<SK> for Option<u32> {
    #[inline]
    fn from(input: SK) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SK {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SK  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::BOUNDING.0.into(), "BOUNDING", "Bounding"),
            (Self::CLIP.0.into(), "CLIP", "Clip"),
            (Self::INPUT.0.into(), "INPUT", "Input"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the Notify event
pub const NOTIFY_EVENT: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NotifyEvent {
    pub response_type: u8,
    pub shape_kind: SK,
    pub sequence: u16,
    pub affected_window: xproto::Window,
    pub extents_x: i16,
    pub extents_y: i16,
    pub extents_width: u16,
    pub extents_height: u16,
    pub server_time: xproto::Timestamp,
    pub shaped: bool,
}
impl_debug_if_no_extra_traits!(NotifyEvent, "NotifyEvent");
impl TryParse for NotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (shape_kind, remaining) = Kind::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (affected_window, remaining) = xproto::Window::try_parse(remaining)?;
        let (extents_x, remaining) = i16::try_parse(remaining)?;
        let (extents_y, remaining) = i16::try_parse(remaining)?;
        let (extents_width, remaining) = u16::try_parse(remaining)?;
        let (extents_height, remaining) = u16::try_parse(remaining)?;
        let (server_time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (shaped, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(11..).ok_or(ParseError::InsufficientData)?;
        let shape_kind = shape_kind.into();
        let result = NotifyEvent { response_type, shape_kind, sequence, affected_window, extents_x, extents_y, extents_width, extents_height, server_time, shaped };
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
        let shape_kind_bytes = Kind::from(self.shape_kind).serialize();
        let sequence_bytes = self.sequence.serialize();
        let affected_window_bytes = self.affected_window.serialize();
        let extents_x_bytes = self.extents_x.serialize();
        let extents_y_bytes = self.extents_y.serialize();
        let extents_width_bytes = self.extents_width.serialize();
        let extents_height_bytes = self.extents_height.serialize();
        let server_time_bytes = self.server_time.serialize();
        let shaped_bytes = self.shaped.serialize();
        [
            response_type_bytes[0],
            shape_kind_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            affected_window_bytes[0],
            affected_window_bytes[1],
            affected_window_bytes[2],
            affected_window_bytes[3],
            extents_x_bytes[0],
            extents_x_bytes[1],
            extents_y_bytes[0],
            extents_y_bytes[1],
            extents_width_bytes[0],
            extents_width_bytes[1],
            extents_height_bytes[0],
            extents_height_bytes[1],
            server_time_bytes[0],
            server_time_bytes[1],
            server_time_bytes[2],
            server_time_bytes[3],
            shaped_bytes[0],
            0,
            0,
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
        Kind::from(self.shape_kind).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.affected_window.serialize_into(bytes);
        self.extents_x.serialize_into(bytes);
        self.extents_y.serialize_into(bytes);
        self.extents_width.serialize_into(bytes);
        self.extents_height.serialize_into(bytes);
        self.server_time.serialize_into(bytes);
        self.shaped.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 11]);
    }
}
impl From<&NotifyEvent> for [u8; 32] {
    fn from(input: &NotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let shape_kind_bytes = Kind::from(input.shape_kind).serialize();
        let sequence_bytes = input.sequence.serialize();
        let affected_window_bytes = input.affected_window.serialize();
        let extents_x_bytes = input.extents_x.serialize();
        let extents_y_bytes = input.extents_y.serialize();
        let extents_width_bytes = input.extents_width.serialize();
        let extents_height_bytes = input.extents_height.serialize();
        let server_time_bytes = input.server_time.serialize();
        let shaped_bytes = input.shaped.serialize();
        [
            response_type_bytes[0],
            shape_kind_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            affected_window_bytes[0],
            affected_window_bytes[1],
            affected_window_bytes[2],
            affected_window_bytes[3],
            extents_x_bytes[0],
            extents_x_bytes[1],
            extents_y_bytes[0],
            extents_y_bytes[1],
            extents_width_bytes[0],
            extents_width_bytes[1],
            extents_height_bytes[0],
            extents_height_bytes[1],
            server_time_bytes[0],
            server_time_bytes[1],
            server_time_bytes[2],
            server_time_bytes[3],
            shaped_bytes[0],
            0,
            0,
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
impl From<NotifyEvent> for [u8; 32] {
    fn from(input: NotifyEvent) -> Self {
        Self::from(&input)
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
    pub major_version: u16,
    pub minor_version: u16,
}
impl_debug_if_no_extra_traits!(QueryVersionReply, "QueryVersionReply");
impl TryParse for QueryVersionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (major_version, remaining) = u16::try_parse(remaining)?;
        let (minor_version, remaining) = u16::try_parse(remaining)?;
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
    type Bytes = [u8; 12];
    fn serialize(&self) -> [u8; 12] {
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
            minor_version_bytes[0],
            minor_version_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(12);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.major_version.serialize_into(bytes);
        self.minor_version.serialize_into(bytes);
    }
}

/// Opcode for the Rectangles request
pub const RECTANGLES_REQUEST: u8 = 1;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RectanglesRequest<'input> {
    pub operation: SO,
    pub destination_kind: SK,
    pub ordering: xproto::ClipOrdering,
    pub destination_window: xproto::Window,
    pub x_offset: i16,
    pub y_offset: i16,
    pub rectangles: Cow<'input, [xproto::Rectangle]>,
}
impl_debug_if_no_extra_traits!(RectanglesRequest<'_>, "RectanglesRequest");
impl<'input> RectanglesRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let operation_bytes = Op::from(self.operation).serialize();
        let destination_kind_bytes = Kind::from(self.destination_kind).serialize();
        let ordering_bytes = u8::from(self.ordering).serialize();
        let destination_window_bytes = self.destination_window.serialize();
        let x_offset_bytes = self.x_offset.serialize();
        let y_offset_bytes = self.y_offset.serialize();
        let mut request0 = vec![
            major_opcode,
            RECTANGLES_REQUEST,
            0,
            0,
            operation_bytes[0],
            destination_kind_bytes[0],
            ordering_bytes[0],
            0,
            destination_window_bytes[0],
            destination_window_bytes[1],
            destination_window_bytes[2],
            destination_window_bytes[3],
            x_offset_bytes[0],
            x_offset_bytes[1],
            y_offset_bytes[0],
            y_offset_bytes[1],
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
        if header.minor_opcode != RECTANGLES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (operation, remaining) = Op::try_parse(value)?;
        let operation = operation.into();
        let (destination_kind, remaining) = Kind::try_parse(remaining)?;
        let destination_kind = destination_kind.into();
        let (ordering, remaining) = u8::try_parse(remaining)?;
        let ordering = ordering.into();
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (destination_window, remaining) = xproto::Window::try_parse(remaining)?;
        let (x_offset, remaining) = i16::try_parse(remaining)?;
        let (y_offset, remaining) = i16::try_parse(remaining)?;
        let mut remaining = remaining;
        // Length is 'everything left in the input'
        let mut rectangles = Vec::new();
        while !remaining.is_empty() {
            let (v, new_remaining) = xproto::Rectangle::try_parse(remaining)?;
            remaining = new_remaining;
            rectangles.push(v);
        }
        let _ = remaining;
        Ok(RectanglesRequest {
            operation,
            destination_kind,
            ordering,
            destination_window,
            x_offset,
            y_offset,
            rectangles: Cow::Owned(rectangles),
        })
    }
    /// Clone all borrowed data in this RectanglesRequest.
    pub fn into_owned(self) -> RectanglesRequest<'static> {
        RectanglesRequest {
            operation: self.operation,
            destination_kind: self.destination_kind,
            ordering: self.ordering,
            destination_window: self.destination_window,
            x_offset: self.x_offset,
            y_offset: self.y_offset,
            rectangles: Cow::Owned(self.rectangles.into_owned()),
        }
    }
}
impl<'input> Request for RectanglesRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for RectanglesRequest<'input> {
}

/// Opcode for the Mask request
pub const MASK_REQUEST: u8 = 2;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MaskRequest {
    pub operation: SO,
    pub destination_kind: SK,
    pub destination_window: xproto::Window,
    pub x_offset: i16,
    pub y_offset: i16,
    pub source_bitmap: xproto::Pixmap,
}
impl_debug_if_no_extra_traits!(MaskRequest, "MaskRequest");
impl MaskRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let operation_bytes = Op::from(self.operation).serialize();
        let destination_kind_bytes = Kind::from(self.destination_kind).serialize();
        let destination_window_bytes = self.destination_window.serialize();
        let x_offset_bytes = self.x_offset.serialize();
        let y_offset_bytes = self.y_offset.serialize();
        let source_bitmap_bytes = self.source_bitmap.serialize();
        let mut request0 = vec![
            major_opcode,
            MASK_REQUEST,
            0,
            0,
            operation_bytes[0],
            destination_kind_bytes[0],
            0,
            0,
            destination_window_bytes[0],
            destination_window_bytes[1],
            destination_window_bytes[2],
            destination_window_bytes[3],
            x_offset_bytes[0],
            x_offset_bytes[1],
            y_offset_bytes[0],
            y_offset_bytes[1],
            source_bitmap_bytes[0],
            source_bitmap_bytes[1],
            source_bitmap_bytes[2],
            source_bitmap_bytes[3],
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
        if header.minor_opcode != MASK_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (operation, remaining) = Op::try_parse(value)?;
        let operation = operation.into();
        let (destination_kind, remaining) = Kind::try_parse(remaining)?;
        let destination_kind = destination_kind.into();
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (destination_window, remaining) = xproto::Window::try_parse(remaining)?;
        let (x_offset, remaining) = i16::try_parse(remaining)?;
        let (y_offset, remaining) = i16::try_parse(remaining)?;
        let (source_bitmap, remaining) = xproto::Pixmap::try_parse(remaining)?;
        let _ = remaining;
        Ok(MaskRequest {
            operation,
            destination_kind,
            destination_window,
            x_offset,
            y_offset,
            source_bitmap,
        })
    }
}
impl Request for MaskRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for MaskRequest {
}

/// Opcode for the Combine request
pub const COMBINE_REQUEST: u8 = 3;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CombineRequest {
    pub operation: SO,
    pub destination_kind: SK,
    pub source_kind: SK,
    pub destination_window: xproto::Window,
    pub x_offset: i16,
    pub y_offset: i16,
    pub source_window: xproto::Window,
}
impl_debug_if_no_extra_traits!(CombineRequest, "CombineRequest");
impl CombineRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let operation_bytes = Op::from(self.operation).serialize();
        let destination_kind_bytes = Kind::from(self.destination_kind).serialize();
        let source_kind_bytes = Kind::from(self.source_kind).serialize();
        let destination_window_bytes = self.destination_window.serialize();
        let x_offset_bytes = self.x_offset.serialize();
        let y_offset_bytes = self.y_offset.serialize();
        let source_window_bytes = self.source_window.serialize();
        let mut request0 = vec![
            major_opcode,
            COMBINE_REQUEST,
            0,
            0,
            operation_bytes[0],
            destination_kind_bytes[0],
            source_kind_bytes[0],
            0,
            destination_window_bytes[0],
            destination_window_bytes[1],
            destination_window_bytes[2],
            destination_window_bytes[3],
            x_offset_bytes[0],
            x_offset_bytes[1],
            y_offset_bytes[0],
            y_offset_bytes[1],
            source_window_bytes[0],
            source_window_bytes[1],
            source_window_bytes[2],
            source_window_bytes[3],
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
        if header.minor_opcode != COMBINE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (operation, remaining) = Op::try_parse(value)?;
        let operation = operation.into();
        let (destination_kind, remaining) = Kind::try_parse(remaining)?;
        let destination_kind = destination_kind.into();
        let (source_kind, remaining) = Kind::try_parse(remaining)?;
        let source_kind = source_kind.into();
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (destination_window, remaining) = xproto::Window::try_parse(remaining)?;
        let (x_offset, remaining) = i16::try_parse(remaining)?;
        let (y_offset, remaining) = i16::try_parse(remaining)?;
        let (source_window, remaining) = xproto::Window::try_parse(remaining)?;
        let _ = remaining;
        Ok(CombineRequest {
            operation,
            destination_kind,
            source_kind,
            destination_window,
            x_offset,
            y_offset,
            source_window,
        })
    }
}
impl Request for CombineRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CombineRequest {
}

/// Opcode for the Offset request
pub const OFFSET_REQUEST: u8 = 4;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OffsetRequest {
    pub destination_kind: SK,
    pub destination_window: xproto::Window,
    pub x_offset: i16,
    pub y_offset: i16,
}
impl_debug_if_no_extra_traits!(OffsetRequest, "OffsetRequest");
impl OffsetRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let destination_kind_bytes = Kind::from(self.destination_kind).serialize();
        let destination_window_bytes = self.destination_window.serialize();
        let x_offset_bytes = self.x_offset.serialize();
        let y_offset_bytes = self.y_offset.serialize();
        let mut request0 = vec![
            major_opcode,
            OFFSET_REQUEST,
            0,
            0,
            destination_kind_bytes[0],
            0,
            0,
            0,
            destination_window_bytes[0],
            destination_window_bytes[1],
            destination_window_bytes[2],
            destination_window_bytes[3],
            x_offset_bytes[0],
            x_offset_bytes[1],
            y_offset_bytes[0],
            y_offset_bytes[1],
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
        if header.minor_opcode != OFFSET_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (destination_kind, remaining) = Kind::try_parse(value)?;
        let destination_kind = destination_kind.into();
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (destination_window, remaining) = xproto::Window::try_parse(remaining)?;
        let (x_offset, remaining) = i16::try_parse(remaining)?;
        let (y_offset, remaining) = i16::try_parse(remaining)?;
        let _ = remaining;
        Ok(OffsetRequest {
            destination_kind,
            destination_window,
            x_offset,
            y_offset,
        })
    }
}
impl Request for OffsetRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for OffsetRequest {
}

/// Opcode for the QueryExtents request
pub const QUERY_EXTENTS_REQUEST: u8 = 5;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryExtentsRequest {
    pub destination_window: xproto::Window,
}
impl_debug_if_no_extra_traits!(QueryExtentsRequest, "QueryExtentsRequest");
impl QueryExtentsRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let destination_window_bytes = self.destination_window.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_EXTENTS_REQUEST,
            0,
            0,
            destination_window_bytes[0],
            destination_window_bytes[1],
            destination_window_bytes[2],
            destination_window_bytes[3],
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
        if header.minor_opcode != QUERY_EXTENTS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (destination_window, remaining) = xproto::Window::try_parse(value)?;
        let _ = remaining;
        Ok(QueryExtentsRequest {
            destination_window,
        })
    }
}
impl Request for QueryExtentsRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryExtentsRequest {
    type Reply = QueryExtentsReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryExtentsReply {
    pub sequence: u16,
    pub length: u32,
    pub bounding_shaped: bool,
    pub clip_shaped: bool,
    pub bounding_shape_extents_x: i16,
    pub bounding_shape_extents_y: i16,
    pub bounding_shape_extents_width: u16,
    pub bounding_shape_extents_height: u16,
    pub clip_shape_extents_x: i16,
    pub clip_shape_extents_y: i16,
    pub clip_shape_extents_width: u16,
    pub clip_shape_extents_height: u16,
}
impl_debug_if_no_extra_traits!(QueryExtentsReply, "QueryExtentsReply");
impl TryParse for QueryExtentsReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (bounding_shaped, remaining) = bool::try_parse(remaining)?;
        let (clip_shaped, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (bounding_shape_extents_x, remaining) = i16::try_parse(remaining)?;
        let (bounding_shape_extents_y, remaining) = i16::try_parse(remaining)?;
        let (bounding_shape_extents_width, remaining) = u16::try_parse(remaining)?;
        let (bounding_shape_extents_height, remaining) = u16::try_parse(remaining)?;
        let (clip_shape_extents_x, remaining) = i16::try_parse(remaining)?;
        let (clip_shape_extents_y, remaining) = i16::try_parse(remaining)?;
        let (clip_shape_extents_width, remaining) = u16::try_parse(remaining)?;
        let (clip_shape_extents_height, remaining) = u16::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryExtentsReply { sequence, length, bounding_shaped, clip_shaped, bounding_shape_extents_x, bounding_shape_extents_y, bounding_shape_extents_width, bounding_shape_extents_height, clip_shape_extents_x, clip_shape_extents_y, clip_shape_extents_width, clip_shape_extents_height };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryExtentsReply {
    type Bytes = [u8; 28];
    fn serialize(&self) -> [u8; 28] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let bounding_shaped_bytes = self.bounding_shaped.serialize();
        let clip_shaped_bytes = self.clip_shaped.serialize();
        let bounding_shape_extents_x_bytes = self.bounding_shape_extents_x.serialize();
        let bounding_shape_extents_y_bytes = self.bounding_shape_extents_y.serialize();
        let bounding_shape_extents_width_bytes = self.bounding_shape_extents_width.serialize();
        let bounding_shape_extents_height_bytes = self.bounding_shape_extents_height.serialize();
        let clip_shape_extents_x_bytes = self.clip_shape_extents_x.serialize();
        let clip_shape_extents_y_bytes = self.clip_shape_extents_y.serialize();
        let clip_shape_extents_width_bytes = self.clip_shape_extents_width.serialize();
        let clip_shape_extents_height_bytes = self.clip_shape_extents_height.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            bounding_shaped_bytes[0],
            clip_shaped_bytes[0],
            0,
            0,
            bounding_shape_extents_x_bytes[0],
            bounding_shape_extents_x_bytes[1],
            bounding_shape_extents_y_bytes[0],
            bounding_shape_extents_y_bytes[1],
            bounding_shape_extents_width_bytes[0],
            bounding_shape_extents_width_bytes[1],
            bounding_shape_extents_height_bytes[0],
            bounding_shape_extents_height_bytes[1],
            clip_shape_extents_x_bytes[0],
            clip_shape_extents_x_bytes[1],
            clip_shape_extents_y_bytes[0],
            clip_shape_extents_y_bytes[1],
            clip_shape_extents_width_bytes[0],
            clip_shape_extents_width_bytes[1],
            clip_shape_extents_height_bytes[0],
            clip_shape_extents_height_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(28);
        let response_type_bytes = &[1];
        bytes.push(response_type_bytes[0]);
        bytes.extend_from_slice(&[0; 1]);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.bounding_shaped.serialize_into(bytes);
        self.clip_shaped.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.bounding_shape_extents_x.serialize_into(bytes);
        self.bounding_shape_extents_y.serialize_into(bytes);
        self.bounding_shape_extents_width.serialize_into(bytes);
        self.bounding_shape_extents_height.serialize_into(bytes);
        self.clip_shape_extents_x.serialize_into(bytes);
        self.clip_shape_extents_y.serialize_into(bytes);
        self.clip_shape_extents_width.serialize_into(bytes);
        self.clip_shape_extents_height.serialize_into(bytes);
    }
}

/// Opcode for the SelectInput request
pub const SELECT_INPUT_REQUEST: u8 = 6;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectInputRequest {
    pub destination_window: xproto::Window,
    pub enable: bool,
}
impl_debug_if_no_extra_traits!(SelectInputRequest, "SelectInputRequest");
impl SelectInputRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let destination_window_bytes = self.destination_window.serialize();
        let enable_bytes = self.enable.serialize();
        let mut request0 = vec![
            major_opcode,
            SELECT_INPUT_REQUEST,
            0,
            0,
            destination_window_bytes[0],
            destination_window_bytes[1],
            destination_window_bytes[2],
            destination_window_bytes[3],
            enable_bytes[0],
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
        if header.minor_opcode != SELECT_INPUT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (destination_window, remaining) = xproto::Window::try_parse(value)?;
        let (enable, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(SelectInputRequest {
            destination_window,
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

/// Opcode for the InputSelected request
pub const INPUT_SELECTED_REQUEST: u8 = 7;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InputSelectedRequest {
    pub destination_window: xproto::Window,
}
impl_debug_if_no_extra_traits!(InputSelectedRequest, "InputSelectedRequest");
impl InputSelectedRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let destination_window_bytes = self.destination_window.serialize();
        let mut request0 = vec![
            major_opcode,
            INPUT_SELECTED_REQUEST,
            0,
            0,
            destination_window_bytes[0],
            destination_window_bytes[1],
            destination_window_bytes[2],
            destination_window_bytes[3],
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
        if header.minor_opcode != INPUT_SELECTED_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (destination_window, remaining) = xproto::Window::try_parse(value)?;
        let _ = remaining;
        Ok(InputSelectedRequest {
            destination_window,
        })
    }
}
impl Request for InputSelectedRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for InputSelectedRequest {
    type Reply = InputSelectedReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InputSelectedReply {
    pub enabled: bool,
    pub sequence: u16,
    pub length: u32,
}
impl_debug_if_no_extra_traits!(InputSelectedReply, "InputSelectedReply");
impl TryParse for InputSelectedReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (enabled, remaining) = bool::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = InputSelectedReply { enabled, sequence, length };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for InputSelectedReply {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let response_type_bytes = &[1];
        let enabled_bytes = self.enabled.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        [
            response_type_bytes[0],
            enabled_bytes[0],
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
        self.enabled.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
    }
}

/// Opcode for the GetRectangles request
pub const GET_RECTANGLES_REQUEST: u8 = 8;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetRectanglesRequest {
    pub window: xproto::Window,
    pub source_kind: SK,
}
impl_debug_if_no_extra_traits!(GetRectanglesRequest, "GetRectanglesRequest");
impl GetRectanglesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let source_kind_bytes = Kind::from(self.source_kind).serialize();
        let mut request0 = vec![
            major_opcode,
            GET_RECTANGLES_REQUEST,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            source_kind_bytes[0],
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
        if header.minor_opcode != GET_RECTANGLES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (source_kind, remaining) = Kind::try_parse(remaining)?;
        let source_kind = source_kind.into();
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(GetRectanglesRequest {
            window,
            source_kind,
        })
    }
}
impl Request for GetRectanglesRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetRectanglesRequest {
    type Reply = GetRectanglesReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetRectanglesReply {
    pub ordering: xproto::ClipOrdering,
    pub sequence: u16,
    pub length: u32,
    pub rectangles: Vec<xproto::Rectangle>,
}
impl_debug_if_no_extra_traits!(GetRectanglesReply, "GetRectanglesReply");
impl TryParse for GetRectanglesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (ordering, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (rectangles_len, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (rectangles, remaining) = crate::x11_utils::parse_list::<xproto::Rectangle>(remaining, rectangles_len.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let ordering = ordering.into();
        let result = GetRectanglesReply { ordering, sequence, length, rectangles };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetRectanglesReply {
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
        u8::from(self.ordering).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        let rectangles_len = u32::try_from(self.rectangles.len()).expect("`rectangles` has too many elements");
        rectangles_len.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        self.rectangles.serialize_into(bytes);
    }
}
impl GetRectanglesReply {
    /// Get the value of the `rectangles_len` field.
    ///
    /// The `rectangles_len` field is used as the length field of the `rectangles` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn rectangles_len(&self) -> u32 {
        self.rectangles.len()
            .try_into().unwrap()
    }
}

