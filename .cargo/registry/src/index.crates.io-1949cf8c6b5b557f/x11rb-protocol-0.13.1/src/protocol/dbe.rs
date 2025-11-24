// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Dbe` X11 extension.

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
pub const X11_EXTENSION_NAME: &str = "DOUBLE-BUFFER";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (1, 0);

pub type BackBuffer = xproto::Drawable;

/// Specifies what to do with the front buffer after it is swapped with the back buffer.
///
/// # Fields
///
/// * `Undefined` - Discard the buffer. The buffer may be reallocated and end up with random VRAM content.
/// * `Background` - Erase with window background.
/// * `Untouched` - Leave untouched.
/// * `Copied` - Copy the newly displayed front buffer.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SwapAction(u8);
impl SwapAction {
    pub const UNDEFINED: Self = Self(0);
    pub const BACKGROUND: Self = Self(1);
    pub const UNTOUCHED: Self = Self(2);
    pub const COPIED: Self = Self(3);
}
impl From<SwapAction> for u8 {
    #[inline]
    fn from(input: SwapAction) -> Self {
        input.0
    }
}
impl From<SwapAction> for Option<u8> {
    #[inline]
    fn from(input: SwapAction) -> Self {
        Some(input.0)
    }
}
impl From<SwapAction> for u16 {
    #[inline]
    fn from(input: SwapAction) -> Self {
        u16::from(input.0)
    }
}
impl From<SwapAction> for Option<u16> {
    #[inline]
    fn from(input: SwapAction) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<SwapAction> for u32 {
    #[inline]
    fn from(input: SwapAction) -> Self {
        u32::from(input.0)
    }
}
impl From<SwapAction> for Option<u32> {
    #[inline]
    fn from(input: SwapAction) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for SwapAction {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for SwapAction  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::UNDEFINED.0.into(), "UNDEFINED", "Undefined"),
            (Self::BACKGROUND.0.into(), "BACKGROUND", "Background"),
            (Self::UNTOUCHED.0.into(), "UNTOUCHED", "Untouched"),
            (Self::COPIED.0.into(), "COPIED", "Copied"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SwapInfo {
    pub window: xproto::Window,
    pub swap_action: SwapAction,
}
impl_debug_if_no_extra_traits!(SwapInfo, "SwapInfo");
impl TryParse for SwapInfo {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (window, remaining) = xproto::Window::try_parse(remaining)?;
        let (swap_action, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let swap_action = swap_action.into();
        let result = SwapInfo { window, swap_action };
        Ok((result, remaining))
    }
}
impl Serialize for SwapInfo {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let window_bytes = self.window.serialize();
        let swap_action_bytes = u8::from(self.swap_action).serialize();
        [
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            swap_action_bytes[0],
            0,
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.window.serialize_into(bytes);
        u8::from(self.swap_action).serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BufferAttributes {
    pub window: xproto::Window,
}
impl_debug_if_no_extra_traits!(BufferAttributes, "BufferAttributes");
impl TryParse for BufferAttributes {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (window, remaining) = xproto::Window::try_parse(remaining)?;
        let result = BufferAttributes { window };
        Ok((result, remaining))
    }
}
impl Serialize for BufferAttributes {
    type Bytes = [u8; 4];
    fn serialize(&self) -> [u8; 4] {
        let window_bytes = self.window.serialize();
        [
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        self.window.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VisualInfo {
    pub visual_id: xproto::Visualid,
    pub depth: u8,
    pub perf_level: u8,
}
impl_debug_if_no_extra_traits!(VisualInfo, "VisualInfo");
impl TryParse for VisualInfo {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (visual_id, remaining) = xproto::Visualid::try_parse(remaining)?;
        let (depth, remaining) = u8::try_parse(remaining)?;
        let (perf_level, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let result = VisualInfo { visual_id, depth, perf_level };
        Ok((result, remaining))
    }
}
impl Serialize for VisualInfo {
    type Bytes = [u8; 8];
    fn serialize(&self) -> [u8; 8] {
        let visual_id_bytes = self.visual_id.serialize();
        let depth_bytes = self.depth.serialize();
        let perf_level_bytes = self.perf_level.serialize();
        [
            visual_id_bytes[0],
            visual_id_bytes[1],
            visual_id_bytes[2],
            visual_id_bytes[3],
            depth_bytes[0],
            perf_level_bytes[0],
            0,
            0,
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.visual_id.serialize_into(bytes);
        self.depth.serialize_into(bytes);
        self.perf_level.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VisualInfos {
    pub infos: Vec<VisualInfo>,
}
impl_debug_if_no_extra_traits!(VisualInfos, "VisualInfos");
impl TryParse for VisualInfos {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (n_infos, remaining) = u32::try_parse(remaining)?;
        let (infos, remaining) = crate::x11_utils::parse_list::<VisualInfo>(remaining, n_infos.try_to_usize()?)?;
        let result = VisualInfos { infos };
        Ok((result, remaining))
    }
}
impl Serialize for VisualInfos {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        let n_infos = u32::try_from(self.infos.len()).expect("`infos` has too many elements");
        n_infos.serialize_into(bytes);
        self.infos.serialize_into(bytes);
    }
}
impl VisualInfos {
    /// Get the value of the `n_infos` field.
    ///
    /// The `n_infos` field is used as the length field of the `infos` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_infos(&self) -> u32 {
        self.infos.len()
            .try_into().unwrap()
    }
}

/// Opcode for the BadBuffer error
pub const BAD_BUFFER_ERROR: u8 = 0;

/// Opcode for the QueryVersion request
pub const QUERY_VERSION_REQUEST: u8 = 0;
/// Queries the version of this extension.
///
/// Queries the version of this extension. You must do this before using any functionality it provides.
///
/// # Fields
///
/// * `major_version` - The major version of the extension. Check that it is compatible with the XCB_DBE_MAJOR_VERSION that your code is compiled with.
/// * `minor_version` - The minor version of the extension. Check that it is compatible with the XCB_DBE_MINOR_VERSION that your code is compiled with.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryVersionRequest {
    pub major_version: u8,
    pub minor_version: u8,
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
            minor_version_bytes[0],
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
        let (major_version, remaining) = u8::try_parse(value)?;
        let (minor_version, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
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
    pub major_version: u8,
    pub minor_version: u8,
}
impl_debug_if_no_extra_traits!(QueryVersionReply, "QueryVersionReply");
impl TryParse for QueryVersionReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (major_version, remaining) = u8::try_parse(remaining)?;
        let (minor_version, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(22..).ok_or(ParseError::InsufficientData)?;
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
            minor_version_bytes[0],
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
        self.major_version.serialize_into(bytes);
        self.minor_version.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 22]);
    }
}

/// Opcode for the AllocateBackBuffer request
pub const ALLOCATE_BACK_BUFFER_REQUEST: u8 = 1;
/// Allocates a back buffer.
///
/// Associates `buffer` with the back buffer of `window`. Multiple ids may be associated with the back buffer, which is created by the first allocate call and destroyed by the last deallocate.
///
/// # Fields
///
/// * `window` - The window to which to add the back buffer.
/// * `buffer` - The buffer id to associate with the back buffer.
/// * `swap_action` - The swap action most likely to be used to present this back buffer. This is only a hint, and does not preclude the use of other swap actions.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllocateBackBufferRequest {
    pub window: xproto::Window,
    pub buffer: BackBuffer,
    pub swap_action: u8,
}
impl_debug_if_no_extra_traits!(AllocateBackBufferRequest, "AllocateBackBufferRequest");
impl AllocateBackBufferRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let window_bytes = self.window.serialize();
        let buffer_bytes = self.buffer.serialize();
        let swap_action_bytes = self.swap_action.serialize();
        let mut request0 = vec![
            major_opcode,
            ALLOCATE_BACK_BUFFER_REQUEST,
            0,
            0,
            window_bytes[0],
            window_bytes[1],
            window_bytes[2],
            window_bytes[3],
            buffer_bytes[0],
            buffer_bytes[1],
            buffer_bytes[2],
            buffer_bytes[3],
            swap_action_bytes[0],
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
        if header.minor_opcode != ALLOCATE_BACK_BUFFER_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (window, remaining) = xproto::Window::try_parse(value)?;
        let (buffer, remaining) = BackBuffer::try_parse(remaining)?;
        let (swap_action, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(AllocateBackBufferRequest {
            window,
            buffer,
            swap_action,
        })
    }
}
impl Request for AllocateBackBufferRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for AllocateBackBufferRequest {
}

/// Opcode for the DeallocateBackBuffer request
pub const DEALLOCATE_BACK_BUFFER_REQUEST: u8 = 2;
/// Deallocates a back buffer.
///
/// Deallocates the given `buffer`. If `buffer` is an invalid id, a `BadBuffer` error is returned. Because a window may have allocated multiple back buffer ids, the back buffer itself is not deleted until all these ids are deallocated by this call.
///
/// # Fields
///
/// * `buffer` - The back buffer to deallocate.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeallocateBackBufferRequest {
    pub buffer: BackBuffer,
}
impl_debug_if_no_extra_traits!(DeallocateBackBufferRequest, "DeallocateBackBufferRequest");
impl DeallocateBackBufferRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let buffer_bytes = self.buffer.serialize();
        let mut request0 = vec![
            major_opcode,
            DEALLOCATE_BACK_BUFFER_REQUEST,
            0,
            0,
            buffer_bytes[0],
            buffer_bytes[1],
            buffer_bytes[2],
            buffer_bytes[3],
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
        if header.minor_opcode != DEALLOCATE_BACK_BUFFER_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (buffer, remaining) = BackBuffer::try_parse(value)?;
        let _ = remaining;
        Ok(DeallocateBackBufferRequest {
            buffer,
        })
    }
}
impl Request for DeallocateBackBufferRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DeallocateBackBufferRequest {
}

/// Opcode for the SwapBuffers request
pub const SWAP_BUFFERS_REQUEST: u8 = 3;
/// Swaps front and back buffers.
///
/// Swaps the front and back buffers on the specified windows. The front and back buffers retain their ids, so that the window id continues to refer to the front buffer, while the back buffer id created by this extension continues to refer to the back buffer. Back buffer contents is moved to the front buffer. Back buffer contents after the operation depends on the given swap action. The optimal swap action depends on how each frame is rendered. For example, if the buffer is cleared and fully overwritten on every frame, the "untouched" action, which throws away the buffer contents, would provide the best performance. To eliminate visual artifacts, the swap will occure during the monitor VSync, if the X server supports detecting it.
///
/// # Fields
///
/// * `actions` - List of windows on which to swap buffers.
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SwapBuffersRequest<'input> {
    pub actions: Cow<'input, [SwapInfo]>,
}
impl_debug_if_no_extra_traits!(SwapBuffersRequest<'_>, "SwapBuffersRequest");
impl<'input> SwapBuffersRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let n_actions = u32::try_from(self.actions.len()).expect("`actions` has too many elements");
        let n_actions_bytes = n_actions.serialize();
        let mut request0 = vec![
            major_opcode,
            SWAP_BUFFERS_REQUEST,
            0,
            0,
            n_actions_bytes[0],
            n_actions_bytes[1],
            n_actions_bytes[2],
            n_actions_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let actions_bytes = self.actions.serialize();
        let length_so_far = length_so_far + actions_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), actions_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != SWAP_BUFFERS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (n_actions, remaining) = u32::try_parse(value)?;
        let (actions, remaining) = crate::x11_utils::parse_list::<SwapInfo>(remaining, n_actions.try_to_usize()?)?;
        let _ = remaining;
        Ok(SwapBuffersRequest {
            actions: Cow::Owned(actions),
        })
    }
    /// Clone all borrowed data in this SwapBuffersRequest.
    pub fn into_owned(self) -> SwapBuffersRequest<'static> {
        SwapBuffersRequest {
            actions: Cow::Owned(self.actions.into_owned()),
        }
    }
}
impl<'input> Request for SwapBuffersRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for SwapBuffersRequest<'input> {
}

/// Opcode for the BeginIdiom request
pub const BEGIN_IDIOM_REQUEST: u8 = 4;
/// Begins a logical swap block.
///
/// Creates a block of operations intended to occur together. This may be needed if window presentation requires changing buffers unknown to this extension, such as depth or stencil buffers.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BeginIdiomRequest;
impl_debug_if_no_extra_traits!(BeginIdiomRequest, "BeginIdiomRequest");
impl BeginIdiomRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            BEGIN_IDIOM_REQUEST,
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
        if header.minor_opcode != BEGIN_IDIOM_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(BeginIdiomRequest
        )
    }
}
impl Request for BeginIdiomRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for BeginIdiomRequest {
}

/// Opcode for the EndIdiom request
pub const END_IDIOM_REQUEST: u8 = 5;
/// Ends a logical swap block.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EndIdiomRequest;
impl_debug_if_no_extra_traits!(EndIdiomRequest, "EndIdiomRequest");
impl EndIdiomRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let mut request0 = vec![
            major_opcode,
            END_IDIOM_REQUEST,
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
        if header.minor_opcode != END_IDIOM_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let _ = value;
        Ok(EndIdiomRequest
        )
    }
}
impl Request for EndIdiomRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for EndIdiomRequest {
}

/// Opcode for the GetVisualInfo request
pub const GET_VISUAL_INFO_REQUEST: u8 = 6;
/// Requests visuals that support double buffering.
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetVisualInfoRequest<'input> {
    pub drawables: Cow<'input, [xproto::Drawable]>,
}
impl_debug_if_no_extra_traits!(GetVisualInfoRequest<'_>, "GetVisualInfoRequest");
impl<'input> GetVisualInfoRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let n_drawables = u32::try_from(self.drawables.len()).expect("`drawables` has too many elements");
        let n_drawables_bytes = n_drawables.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_VISUAL_INFO_REQUEST,
            0,
            0,
            n_drawables_bytes[0],
            n_drawables_bytes[1],
            n_drawables_bytes[2],
            n_drawables_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let drawables_bytes = self.drawables.serialize();
        let length_so_far = length_so_far + drawables_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), drawables_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != GET_VISUAL_INFO_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (n_drawables, remaining) = u32::try_parse(value)?;
        let (drawables, remaining) = crate::x11_utils::parse_list::<xproto::Drawable>(remaining, n_drawables.try_to_usize()?)?;
        let _ = remaining;
        Ok(GetVisualInfoRequest {
            drawables: Cow::Owned(drawables),
        })
    }
    /// Clone all borrowed data in this GetVisualInfoRequest.
    pub fn into_owned(self) -> GetVisualInfoRequest<'static> {
        GetVisualInfoRequest {
            drawables: Cow::Owned(self.drawables.into_owned()),
        }
    }
}
impl<'input> Request for GetVisualInfoRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::ReplyRequest for GetVisualInfoRequest<'input> {
    type Reply = GetVisualInfoReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetVisualInfoReply {
    pub sequence: u16,
    pub length: u32,
    pub supported_visuals: Vec<VisualInfos>,
}
impl_debug_if_no_extra_traits!(GetVisualInfoReply, "GetVisualInfoReply");
impl TryParse for GetVisualInfoReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (n_supported_visuals, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        let (supported_visuals, remaining) = crate::x11_utils::parse_list::<VisualInfos>(remaining, n_supported_visuals.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetVisualInfoReply { sequence, length, supported_visuals };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetVisualInfoReply {
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
        let n_supported_visuals = u32::try_from(self.supported_visuals.len()).expect("`supported_visuals` has too many elements");
        n_supported_visuals.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
        self.supported_visuals.serialize_into(bytes);
    }
}
impl GetVisualInfoReply {
    /// Get the value of the `n_supported_visuals` field.
    ///
    /// The `n_supported_visuals` field is used as the length field of the `supported_visuals` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn n_supported_visuals(&self) -> u32 {
        self.supported_visuals.len()
            .try_into().unwrap()
    }
}

/// Opcode for the GetBackBufferAttributes request
pub const GET_BACK_BUFFER_ATTRIBUTES_REQUEST: u8 = 7;
/// Gets back buffer attributes.
///
/// Returns the attributes of the specified `buffer`.
///
/// # Fields
///
/// * `buffer` - The back buffer to query.
/// * `attributes` - The attributes of `buffer`.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetBackBufferAttributesRequest {
    pub buffer: BackBuffer,
}
impl_debug_if_no_extra_traits!(GetBackBufferAttributesRequest, "GetBackBufferAttributesRequest");
impl GetBackBufferAttributesRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let buffer_bytes = self.buffer.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_BACK_BUFFER_ATTRIBUTES_REQUEST,
            0,
            0,
            buffer_bytes[0],
            buffer_bytes[1],
            buffer_bytes[2],
            buffer_bytes[3],
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
        if header.minor_opcode != GET_BACK_BUFFER_ATTRIBUTES_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (buffer, remaining) = BackBuffer::try_parse(value)?;
        let _ = remaining;
        Ok(GetBackBufferAttributesRequest {
            buffer,
        })
    }
}
impl Request for GetBackBufferAttributesRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetBackBufferAttributesRequest {
    type Reply = GetBackBufferAttributesReply;
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetBackBufferAttributesReply {
    pub sequence: u16,
    pub length: u32,
    pub attributes: BufferAttributes,
}
impl_debug_if_no_extra_traits!(GetBackBufferAttributesReply, "GetBackBufferAttributesReply");
impl TryParse for GetBackBufferAttributesReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let remaining = remaining.get(1..).ok_or(ParseError::InsufficientData)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (attributes, remaining) = BufferAttributes::try_parse(remaining)?;
        let remaining = remaining.get(20..).ok_or(ParseError::InsufficientData)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetBackBufferAttributesReply { sequence, length, attributes };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetBackBufferAttributesReply {
    type Bytes = [u8; 32];
    fn serialize(&self) -> [u8; 32] {
        let response_type_bytes = &[1];
        let sequence_bytes = self.sequence.serialize();
        let length_bytes = self.length.serialize();
        let attributes_bytes = self.attributes.serialize();
        [
            response_type_bytes[0],
            0,
            sequence_bytes[0],
            sequence_bytes[1],
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
            attributes_bytes[0],
            attributes_bytes[1],
            attributes_bytes[2],
            attributes_bytes[3],
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
        self.attributes.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 20]);
    }
}

