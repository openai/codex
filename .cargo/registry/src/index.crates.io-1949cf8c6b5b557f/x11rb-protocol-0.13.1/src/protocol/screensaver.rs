// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `ScreenSaver` X11 extension.

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
pub const X11_EXTENSION_NAME: &str = "MIT-SCREEN-SAVER";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (1, 1);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Kind(u8);
impl Kind {
    pub const BLANKED: Self = Self(0);
    pub const INTERNAL: Self = Self(1);
    pub const EXTERNAL: Self = Self(2);
}
impl From<Kind> for u8 {
    #[inline]
    fn from(input: Kind) -> Self {
        input.0
    }
}
impl From<Kind> for Option<u8> {
    #[inline]
    fn from(input: Kind) -> Self {
        Some(input.0)
    }
}
impl From<Kind> for u16 {
    #[inline]
    fn from(input: Kind) -> Self {
        u16::from(input.0)
    }
}
impl From<Kind> for Option<u16> {
    #[inline]
    fn from(input: Kind) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<Kind> for u32 {
    #[inline]
    fn from(input: Kind) -> Self {
        u32::from(input.0)
    }
}
impl From<Kind> for Option<u32> {
    #[inline]
    fn from(input: Kind) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for Kind {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Kind  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::BLANKED.0.into(), "BLANKED", "Blanked"),
            (Self::INTERNAL.0.into(), "INTERNAL", "Internal"),
            (Self::EXTERNAL.0.into(), "EXTERNAL", "External"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Event(u32);
impl Event {
    pub const NOTIFY_MASK: Self = Self(1 << 0);
    pub const CYCLE_MASK: Self = Self(1 << 1);
}
impl From<Event> for u32 {
    #[inline]
    fn from(input: Event) -> Self {
        input.0
    }
}
impl From<Event> for Option<u32> {
    #[inline]
    fn from(input: Event) -> Self {
        Some(input.0)
    }
}
impl From<u8> for Event {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value.into())
    }
}
impl From<u16> for Event {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value.into())
    }
}
impl From<u32> for Event {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for Event  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::NOTIFY_MASK.0, "NOTIFY_MASK", "NotifyMask"),
            (Self::CYCLE_MASK.0, "CYCLE_MASK", "CycleMask"),
        ];
        pretty_print_bitmask(fmt, self.0, &variants)
    }
}
bitmask_binop!(Event, u32);

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct State(u8);
impl State {
    pub const OFF: Self = Self(0);
    pub const ON: Self = Self(1);
    pub const CYCLE: Self = Self(2);
    pub const DISABLED: Self = Self(3);
}
impl From<State> for u8 {
    #[inline]
    fn from(input: State) -> Self {
        input.0
    }
}
impl From<State> for Option<u8> {
    #[inline]
    fn from(input: State) -> Self {
        Some(input.0)
    }
}
impl From<State> for u16 {
    #[inline]
    fn from(input: State) -> Self {
        u16::from(input.0)
    }
}
impl From<State> for Option<u16> {
    #[inline]
    fn from(input: State) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<State> for u32 {
    #[inline]
    fn from(input: State) -> Self {
        u32::from(input.0)
    }
}
impl From<State> for Option<u32> {
    #[inline]
    fn from(input: State) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for State {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for State  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::OFF.0.into(), "OFF", "Off"),
            (Self::ON.0.into(), "ON", "On"),
            (Self::CYCLE.0.into(), "CYCLE", "Cycle"),
            (Self::DISABLED.0.into(), "DISABLED", "Disabled"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the QueryVersion request
pub const QUERY_VERSION_REQUEST: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryVersionRequest {
    pub client_major_version: u8,
    pub client_minor_version: u8,
}
impl_debug_if_no_extra_traits!(QueryVersionRequest, "QueryVersionRequest");
impl QueryVersionRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let client_major_version_bytes = self.client_major_version.serialize();
        let client_minor_version_bytes = self.client_minor_version.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_VERSION_REQUEST,
            0,
            0,
            client_major_version_bytes[0],
            client_minor_version_bytes[0],
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
        let (client_major_version, remaining) = u8::try_parse(value)?;
        let (client_minor_version, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(QueryVersionRequest {
            client_major_version,
            client_minor_version,
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
    pub server_major_version: u16,
    pub server_minor_version: u16,
}
impl_debug_if_no_extra_traits!(QueryVersionReply, "QueryVersionReply");
impl TryParse for QueryVersionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (server_major_version, remaining) = u16::try_parse(remaining)?;
        let (server_minor_version, remaining) = u16::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = QueryVersionReply { sequence, length, server_major_version, server_minor_version };
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
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
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
        self.server_major_version.serialize_into(bytes);
        self.server_minor_version.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
    }
}

/// Opcode for the QueryInfo request
pub const QUERY_INFO_REQUEST: u8 = 1;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryInfoRequest {
    pub drawable: xproto::Drawable,
}
impl_debug_if_no_extra_traits!(QueryInfoRequest, "QueryInfoRequest");
impl QueryInfoRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let mut request0 = vec![
            major_opcode,
            QUERY_INFO_REQUEST,
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
        if header.minor_opcode != QUERY_INFO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let _ = remaining;
        Ok(QueryInfoRequest {
            drawable,
        })
    }
}
impl Request for QueryInfoRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for QueryInfoRequest {
    type Reply = QueryInfoReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryInfoReply {
    pub state: u8,
    pub sequence: u16,
    pub length: u32,
    pub saver_window: xproto::Window,
    pub ms_until_server: u32,
    pub ms_since_user_input: u32,
    pub event_mask: u32,
    pub kind: Kind,
}
impl_debug_if_no_extra_traits!(QueryInfoReply, "QueryInfoReply");
impl TryParse for QueryInfoReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (state, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (saver_window, remaining) = xproto::Window::try_parse(remaining)?;
        let (ms_until_server, remaining) = u32::try_parse(remaining)?;
        let (ms_since_user_input, remaining) = u32::try_parse(remaining)?;
        let (event_mask, remaining) = u32::try_parse(remaining)?;
        let (kind, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(7..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let kind = kind.into();
        let result = QueryInfoReply { state, sequence, length, saver_window, ms_until_server, ms_since_user_input, event_mask, kind };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for QueryInfoReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let state_bytes = self.state.serialize();
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let saver_window_bytes = self.saver_window.serialize();
        let ms_until_server_bytes = self.ms_until_server.serialize();
        let ms_since_user_input_bytes = self.ms_since_user_input.serialize();
        let event_mask_bytes = self.event_mask.serialize();
        let kind_bytes = u8::from(self.kind).serialize();
        [
            response_type_bytes[0],
            state_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            saver_window_bytes[0],
            saver_window_bytes[1],
            saver_window_bytes[2],
            saver_window_bytes[3],
            ms_until_server_bytes[0],
            ms_until_server_bytes[1],
            ms_until_server_bytes[2],
            ms_until_server_bytes[3],
            ms_since_user_input_bytes[0],
            ms_since_user_input_bytes[1],
            ms_since_user_input_bytes[2],
            ms_since_user_input_bytes[3],
            event_mask_bytes[0],
            event_mask_bytes[1],
            event_mask_bytes[2],
            event_mask_bytes[3],
            kind_bytes[0],
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
        self.state.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.saver_window.serialize_into(bytes);
        self.ms_until_server.serialize_into(bytes);
        self.ms_since_user_input.serialize_into(bytes);
        self.event_mask.serialize_into(bytes);
        u8::from(self.kind).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 7]);
    }
}

/// Opcode for the SelectInput request
pub const SELECT_INPUT_REQUEST: u8 = 2;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SelectInputRequest {
    pub drawable: xproto::Drawable,
    pub event_mask: Event,
}
impl_debug_if_no_extra_traits!(SelectInputRequest, "SelectInputRequest");
impl SelectInputRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let event_mask_bytes = u32::from(self.event_mask).serialize();
        let mut request0 = vec![
            major_opcode,
            SELECT_INPUT_REQUEST,
            0,
            0,
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
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
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let (event_mask, remaining) = u32::try_parse(remaining)?;
        let event_mask = event_mask.into();
        let _ = remaining;
        Ok(SelectInputRequest {
            drawable,
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

/// Auxiliary and optional information for the `set_attributes` function
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetAttributesAux {
    pub background_pixmap: Option<xproto::Pixmap>,
    pub background_pixel: Option<u32>,
    pub border_pixmap: Option<xproto::Pixmap>,
    pub border_pixel: Option<u32>,
    pub bit_gravity: Option<xproto::Gravity>,
    pub win_gravity: Option<xproto::Gravity>,
    pub backing_store: Option<xproto::BackingStore>,
    pub backing_planes: Option<u32>,
    pub backing_pixel: Option<u32>,
    pub override_redirect: Option<xproto::Bool32>,
    pub save_under: Option<xproto::Bool32>,
    pub event_mask: Option<xproto::EventMask>,
    pub do_not_propogate_mask: Option<xproto::EventMask>,
    pub colormap: Option<xproto::Colormap>,
    pub cursor: Option<xproto::Cursor>,
}
impl_debug_if_no_extra_traits!(SetAttributesAux, "SetAttributesAux");
impl SetAttributesAux {
    #[cfg_attr(not(feature = "request-parsing"), allow(dead_code))]
    fn try_parse(value: &[u8], value_mask: u32) -> Result<(Self, &[u8]), ParseError> {
        let switch_expr = u32::from(value_mask);
        let mut outer_remaining = value;
        let background_pixmap = if switch_expr & u32::from(xproto::CW::BACK_PIXMAP) != 0 {
            let remaining = outer_remaining;
            let (background_pixmap, remaining) = xproto::Pixmap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(background_pixmap)
        } else {
            None
        };
        let background_pixel = if switch_expr & u32::from(xproto::CW::BACK_PIXEL) != 0 {
            let remaining = outer_remaining;
            let (background_pixel, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(background_pixel)
        } else {
            None
        };
        let border_pixmap = if switch_expr & u32::from(xproto::CW::BORDER_PIXMAP) != 0 {
            let remaining = outer_remaining;
            let (border_pixmap, remaining) = xproto::Pixmap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(border_pixmap)
        } else {
            None
        };
        let border_pixel = if switch_expr & u32::from(xproto::CW::BORDER_PIXEL) != 0 {
            let remaining = outer_remaining;
            let (border_pixel, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(border_pixel)
        } else {
            None
        };
        let bit_gravity = if switch_expr & u32::from(xproto::CW::BIT_GRAVITY) != 0 {
            let remaining = outer_remaining;
            let (bit_gravity, remaining) = u32::try_parse(remaining)?;
            let bit_gravity = bit_gravity.into();
            outer_remaining = remaining;
            Some(bit_gravity)
        } else {
            None
        };
        let win_gravity = if switch_expr & u32::from(xproto::CW::WIN_GRAVITY) != 0 {
            let remaining = outer_remaining;
            let (win_gravity, remaining) = u32::try_parse(remaining)?;
            let win_gravity = win_gravity.into();
            outer_remaining = remaining;
            Some(win_gravity)
        } else {
            None
        };
        let backing_store = if switch_expr & u32::from(xproto::CW::BACKING_STORE) != 0 {
            let remaining = outer_remaining;
            let (backing_store, remaining) = u32::try_parse(remaining)?;
            let backing_store = backing_store.into();
            outer_remaining = remaining;
            Some(backing_store)
        } else {
            None
        };
        let backing_planes = if switch_expr & u32::from(xproto::CW::BACKING_PLANES) != 0 {
            let remaining = outer_remaining;
            let (backing_planes, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(backing_planes)
        } else {
            None
        };
        let backing_pixel = if switch_expr & u32::from(xproto::CW::BACKING_PIXEL) != 0 {
            let remaining = outer_remaining;
            let (backing_pixel, remaining) = u32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(backing_pixel)
        } else {
            None
        };
        let override_redirect = if switch_expr & u32::from(xproto::CW::OVERRIDE_REDIRECT) != 0 {
            let remaining = outer_remaining;
            let (override_redirect, remaining) = xproto::Bool32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(override_redirect)
        } else {
            None
        };
        let save_under = if switch_expr & u32::from(xproto::CW::SAVE_UNDER) != 0 {
            let remaining = outer_remaining;
            let (save_under, remaining) = xproto::Bool32::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(save_under)
        } else {
            None
        };
        let event_mask = if switch_expr & u32::from(xproto::CW::EVENT_MASK) != 0 {
            let remaining = outer_remaining;
            let (event_mask, remaining) = u32::try_parse(remaining)?;
            let event_mask = event_mask.into();
            outer_remaining = remaining;
            Some(event_mask)
        } else {
            None
        };
        let do_not_propogate_mask = if switch_expr & u32::from(xproto::CW::DONT_PROPAGATE) != 0 {
            let remaining = outer_remaining;
            let (do_not_propogate_mask, remaining) = u32::try_parse(remaining)?;
            let do_not_propogate_mask = do_not_propogate_mask.into();
            outer_remaining = remaining;
            Some(do_not_propogate_mask)
        } else {
            None
        };
        let colormap = if switch_expr & u32::from(xproto::CW::COLORMAP) != 0 {
            let remaining = outer_remaining;
            let (colormap, remaining) = xproto::Colormap::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(colormap)
        } else {
            None
        };
        let cursor = if switch_expr & u32::from(xproto::CW::CURSOR) != 0 {
            let remaining = outer_remaining;
            let (cursor, remaining) = xproto::Cursor::try_parse(remaining)?;
            outer_remaining = remaining;
            Some(cursor)
        } else {
            None
        };
        let result = SetAttributesAux { background_pixmap, background_pixel, border_pixmap, border_pixel, bit_gravity, win_gravity, backing_store, backing_planes, backing_pixel, override_redirect, save_under, event_mask, do_not_propogate_mask, colormap, cursor };
        Ok((result, outer_remaining))
    }
}
impl SetAttributesAux {
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
impl SetAttributesAux {
    fn switch_expr(&self) -> u32 {
        let mut expr_value = 0;
        if self.background_pixmap.is_some() {
            expr_value |= u32::from(xproto::CW::BACK_PIXMAP);
        }
        if self.background_pixel.is_some() {
            expr_value |= u32::from(xproto::CW::BACK_PIXEL);
        }
        if self.border_pixmap.is_some() {
            expr_value |= u32::from(xproto::CW::BORDER_PIXMAP);
        }
        if self.border_pixel.is_some() {
            expr_value |= u32::from(xproto::CW::BORDER_PIXEL);
        }
        if self.bit_gravity.is_some() {
            expr_value |= u32::from(xproto::CW::BIT_GRAVITY);
        }
        if self.win_gravity.is_some() {
            expr_value |= u32::from(xproto::CW::WIN_GRAVITY);
        }
        if self.backing_store.is_some() {
            expr_value |= u32::from(xproto::CW::BACKING_STORE);
        }
        if self.backing_planes.is_some() {
            expr_value |= u32::from(xproto::CW::BACKING_PLANES);
        }
        if self.backing_pixel.is_some() {
            expr_value |= u32::from(xproto::CW::BACKING_PIXEL);
        }
        if self.override_redirect.is_some() {
            expr_value |= u32::from(xproto::CW::OVERRIDE_REDIRECT);
        }
        if self.save_under.is_some() {
            expr_value |= u32::from(xproto::CW::SAVE_UNDER);
        }
        if self.event_mask.is_some() {
            expr_value |= u32::from(xproto::CW::EVENT_MASK);
        }
        if self.do_not_propogate_mask.is_some() {
            expr_value |= u32::from(xproto::CW::DONT_PROPAGATE);
        }
        if self.colormap.is_some() {
            expr_value |= u32::from(xproto::CW::COLORMAP);
        }
        if self.cursor.is_some() {
            expr_value |= u32::from(xproto::CW::CURSOR);
        }
        expr_value
    }
}
impl SetAttributesAux {
    /// Create a new instance with all fields unset / not present.
    pub fn new() -> Self {
        Default::default()
    }
    /// Set the `background_pixmap` field of this structure.
    #[must_use]
    pub fn background_pixmap<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Pixmap>> {
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
    pub fn border_pixmap<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Pixmap>> {
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
    pub fn bit_gravity<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Gravity>> {
        self.bit_gravity = value.into();
        self
    }
    /// Set the `win_gravity` field of this structure.
    #[must_use]
    pub fn win_gravity<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Gravity>> {
        self.win_gravity = value.into();
        self
    }
    /// Set the `backing_store` field of this structure.
    #[must_use]
    pub fn backing_store<I>(mut self, value: I) -> Self where I: Into<Option<xproto::BackingStore>> {
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
    pub fn override_redirect<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Bool32>> {
        self.override_redirect = value.into();
        self
    }
    /// Set the `save_under` field of this structure.
    #[must_use]
    pub fn save_under<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Bool32>> {
        self.save_under = value.into();
        self
    }
    /// Set the `event_mask` field of this structure.
    #[must_use]
    pub fn event_mask<I>(mut self, value: I) -> Self where I: Into<Option<xproto::EventMask>> {
        self.event_mask = value.into();
        self
    }
    /// Set the `do_not_propogate_mask` field of this structure.
    #[must_use]
    pub fn do_not_propogate_mask<I>(mut self, value: I) -> Self where I: Into<Option<xproto::EventMask>> {
        self.do_not_propogate_mask = value.into();
        self
    }
    /// Set the `colormap` field of this structure.
    #[must_use]
    pub fn colormap<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Colormap>> {
        self.colormap = value.into();
        self
    }
    /// Set the `cursor` field of this structure.
    #[must_use]
    pub fn cursor<I>(mut self, value: I) -> Self where I: Into<Option<xproto::Cursor>> {
        self.cursor = value.into();
        self
    }
}

/// Opcode for the SetAttributes request
pub const SET_ATTRIBUTES_REQUEST: u8 = 3;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SetAttributesRequest<'input> {
    pub drawable: xproto::Drawable,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub border_width: u16,
    pub class: xproto::WindowClass,
    pub depth: u8,
    pub visual: xproto::Visualid,
    pub value_list: Cow<'input, SetAttributesAux>,
}
impl_debug_if_no_extra_traits!(SetAttributesRequest<'_>, "SetAttributesRequest");
impl<'input> SetAttributesRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let x_bytes = self.x.serialize();
        let y_bytes = self.y.serialize();
        let width_bytes = self.width.serialize();
        let height_bytes = self.height.serialize();
        let border_width_bytes = self.border_width.serialize();
        let class_bytes = (u16::from(self.class) as u8).serialize();
        let depth_bytes = self.depth.serialize();
        let visual_bytes = self.visual.serialize();
        let value_mask: u32 = self.value_list.switch_expr();
        let value_mask_bytes = value_mask.serialize();
        let mut request0 = vec![
            major_opcode,
            SET_ATTRIBUTES_REQUEST,
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
            border_width_bytes[0],
            border_width_bytes[1],
            class_bytes[0],
            depth_bytes[0],
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
        if header.minor_opcode != SET_ATTRIBUTES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let (x, remaining) = i16::try_parse(remaining)?;
        let (y, remaining) = i16::try_parse(remaining)?;
        let (width, remaining) = u16::try_parse(remaining)?;
        let (height, remaining) = u16::try_parse(remaining)?;
        let (border_width, remaining) = u16::try_parse(remaining)?;
        let (class, remaining) = u8::try_parse(remaining)?;
        let class = class.into();
        let (depth, remaining) = u8::try_parse(remaining)?;
        let (visual, remaining) = xproto::Visualid::try_parse(remaining)?;
        let (value_mask, remaining) = u32::try_parse(remaining)?;
        let (value_list, remaining) = SetAttributesAux::try_parse(remaining, u32::from(value_mask))?;
        let _ = remaining;
        Ok(SetAttributesRequest {
            drawable,
            x,
            y,
            width,
            height,
            border_width,
            class,
            depth,
            visual,
            value_list: Cow::Owned(value_list),
        })
    }
    /// Clone all borrowed data in this SetAttributesRequest.
    pub fn into_owned(self) -> SetAttributesRequest<'static> {
        SetAttributesRequest {
            drawable: self.drawable,
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            border_width: self.border_width,
            class: self.class,
            depth: self.depth,
            visual: self.visual,
            value_list: Cow::Owned(self.value_list.into_owned()),
        }
    }
}
impl<'input> Request for SetAttributesRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SetAttributesRequest<'input> {
}

/// Opcode for the UnsetAttributes request
pub const UNSET_ATTRIBUTES_REQUEST: u8 = 4;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnsetAttributesRequest {
    pub drawable: xproto::Drawable,
}
impl_debug_if_no_extra_traits!(UnsetAttributesRequest, "UnsetAttributesRequest");
impl UnsetAttributesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let mut request0 = vec![
            major_opcode,
            UNSET_ATTRIBUTES_REQUEST,
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
        if header.minor_opcode != UNSET_ATTRIBUTES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let _ = remaining;
        Ok(UnsetAttributesRequest {
            drawable,
        })
    }
}
impl Request for UnsetAttributesRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for UnsetAttributesRequest {
}

/// Opcode for the Suspend request
pub const SUSPEND_REQUEST: u8 = 5;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SuspendRequest {
    pub suspend: u32,
}
impl_debug_if_no_extra_traits!(SuspendRequest, "SuspendRequest");
impl SuspendRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let suspend_bytes = self.suspend.serialize();
        let mut request0 = vec![
            major_opcode,
            SUSPEND_REQUEST,
            0,
            0,
            suspend_bytes[0],
            suspend_bytes[1],
            suspend_bytes[2],
            suspend_bytes[3],
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
        if header.minor_opcode != SUSPEND_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (suspend, remaining) = u32::try_parse(value)?;
        let _ = remaining;
        Ok(SuspendRequest {
            suspend,
        })
    }
}
impl Request for SuspendRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SuspendRequest {
}

/// Opcode for the Notify event
pub const NOTIFY_EVENT: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NotifyEvent {
    pub response_type: u8,
    pub state: State,
    pub sequence: u16,
    pub time: xproto::Timestamp,
    pub root: xproto::Window,
    pub window: xproto::Window,
    pub kind: Kind,
    pub forced: bool,
}
impl_debug_if_no_extra_traits!(NotifyEvent, "NotifyEvent");
impl TryParse for NotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (state, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (time, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (root, remaining) = xproto::Window::try_parse(remaining)?;
        let (window, remaining) = xproto::Window::try_parse(remaining)?;
        let (kind, remaining) = u8::try_parse(remaining)?;
        let (forced, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(14..).ok_or(ParseError::InsufficientData)?;
        let state = state.into();
        let kind = kind.into();
        let result = NotifyEvent { response_type, state, sequence, time, root, window, kind, forced };
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
        let state_bytes = u8::from(self.state).serialize();
        let sequence_bytes = self.sequence.serialize();
        let time_bytes = self.time.serialize();
        let root_bytes = self.root.serialize();
        let window_bytes = self.window.serialize();
        let kind_bytes = u8::from(self.kind).serialize();
        let forced_bytes = self.forced.serialize();
        [
            response_type_bytes[0],
            state_bytes[0],
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
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            kind_bytes[0],
            forced_bytes[0],
            0,
            0,
            0,
            0,
            0,
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
        u8::from(self.state).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.time.serialize_into(bytes);
        self.root.serialize_into(bytes);
        self.window.serialize_into(bytes);
        u8::from(self.kind).serialize_into(bytes);
        self.forced.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 14]);
    }
}
impl From<&NotifyEvent> for [u8; 32] {
    fn from(input: &NotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let state_bytes = u8::from(input.state).serialize();
        let sequence_bytes = input.sequence.serialize();
        let time_bytes = input.time.serialize();
        let root_bytes = input.root.serialize();
        let window_bytes = input.window.serialize();
        let kind_bytes = u8::from(input.kind).serialize();
        let forced_bytes = input.forced.serialize();
        [
            response_type_bytes[0],
            state_bytes[0],
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
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            kind_bytes[0],
            forced_bytes[0],
            0,
            0,
            0,
            0,
            0,
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

