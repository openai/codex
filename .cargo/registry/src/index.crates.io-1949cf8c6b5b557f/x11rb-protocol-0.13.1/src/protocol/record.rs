// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Record` X11 extension.

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
pub const X11_EXTENSION_NAME: &str = "RECORD";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (1, 13);

pub type Context = u32;

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Range8 {
    pub first: u8,
    pub last: u8,
}
impl_debug_if_no_extra_traits!(Range8, "Range8");
impl TryParse for Range8 {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (first, remaining) = u8::try_parse(remaining)?;
        let (last, remaining) = u8::try_parse(remaining)?;
        let result = Range8 { first, last };
        Ok((result, remaining))
    }
}
impl Serialize for Range8 {
    type Bytes = [u8; 2];
    fn serialize(&self) -> [u8; 2] {
        let first_bytes = self.first.serialize();
        let last_bytes = self.last.serialize();
        [
            first_bytes[0],
            last_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(2);
        self.first.serialize_into(bytes);
        self.last.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Range16 {
    pub first: u16,
    pub last: u16,
}
impl_debug_if_no_extra_traits!(Range16, "Range16");
impl TryParse for Range16 {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (first, remaining) = u16::try_parse(remaining)?;
        let (last, remaining) = u16::try_parse(remaining)?;
        let result = Range16 { first, last };
        Ok((result, remaining))
    }
}
impl Serialize for Range16 {
    type Bytes = [u8; 4];
    fn serialize(&self) -> [u8; 4] {
        let first_bytes = self.first.serialize();
        let last_bytes = self.last.serialize();
        [
            first_bytes[0],
            first_bytes[1],
            last_bytes[0],
            last_bytes[1],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(4);
        self.first.serialize_into(bytes);
        self.last.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtRange {
    pub major: Range8,
    pub minor: Range16,
}
impl_debug_if_no_extra_traits!(ExtRange, "ExtRange");
impl TryParse for ExtRange {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (major, remaining) = Range8::try_parse(remaining)?;
        let (minor, remaining) = Range16::try_parse(remaining)?;
        let result = ExtRange { major, minor };
        Ok((result, remaining))
    }
}
impl Serialize for ExtRange {
    type Bytes = [u8; 6];
    fn serialize(&self) -> [u8; 6] {
        let major_bytes = self.major.serialize();
        let minor_bytes = self.minor.serialize();
        [
            major_bytes[0],
            major_bytes[1],
            minor_bytes[0],
            minor_bytes[1],
            minor_bytes[2],
            minor_bytes[3],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(6);
        self.major.serialize_into(bytes);
        self.minor.serialize_into(bytes);
    }
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Range {
    pub core_requests: Range8,
    pub core_replies: Range8,
    pub ext_requests: ExtRange,
    pub ext_replies: ExtRange,
    pub delivered_events: Range8,
    pub device_events: Range8,
    pub errors: Range8,
    pub client_started: bool,
    pub client_died: bool,
}
impl_debug_if_no_extra_traits!(Range, "Range");
impl TryParse for Range {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (core_requests, remaining) = Range8::try_parse(remaining)?;
        let (core_replies, remaining) = Range8::try_parse(remaining)?;
        let (ext_requests, remaining) = ExtRange::try_parse(remaining)?;
        let (ext_replies, remaining) = ExtRange::try_parse(remaining)?;
        let (delivered_events, remaining) = Range8::try_parse(remaining)?;
        let (device_events, remaining) = Range8::try_parse(remaining)?;
        let (errors, remaining) = Range8::try_parse(remaining)?;
        let (client_started, remaining) = bool::try_parse(remaining)?;
        let (client_died, remaining) = bool::try_parse(remaining)?;
        let result = Range { core_requests, core_replies, ext_requests, ext_replies, delivered_events, device_events, errors, client_started, client_died };
        Ok((result, remaining))
    }
}
impl Serialize for Range {
    type Bytes = [u8; 24];
    fn serialize(&self) -> [u8; 24] {
        let core_requests_bytes = self.core_requests.serialize();
        let core_replies_bytes = self.core_replies.serialize();
        let ext_requests_bytes = self.ext_requests.serialize();
        let ext_replies_bytes = self.ext_replies.serialize();
        let delivered_events_bytes = self.delivered_events.serialize();
        let device_events_bytes = self.device_events.serialize();
        let errors_bytes = self.errors.serialize();
        let client_started_bytes = self.client_started.serialize();
        let client_died_bytes = self.client_died.serialize();
        [
            core_requests_bytes[0],
            core_requests_bytes[1],
            core_replies_bytes[0],
            core_replies_bytes[1],
            ext_requests_bytes[0],
            ext_requests_bytes[1],
            ext_requests_bytes[2],
            ext_requests_bytes[3],
            ext_requests_bytes[4],
            ext_requests_bytes[5],
            ext_replies_bytes[0],
            ext_replies_bytes[1],
            ext_replies_bytes[2],
            ext_replies_bytes[3],
            ext_replies_bytes[4],
            ext_replies_bytes[5],
            delivered_events_bytes[0],
            delivered_events_bytes[1],
            device_events_bytes[0],
            device_events_bytes[1],
            errors_bytes[0],
            errors_bytes[1],
            client_started_bytes[0],
            client_died_bytes[0],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(24);
        self.core_requests.serialize_into(bytes);
        self.core_replies.serialize_into(bytes);
        self.ext_requests.serialize_into(bytes);
        self.ext_replies.serialize_into(bytes);
        self.delivered_events.serialize_into(bytes);
        self.device_events.serialize_into(bytes);
        self.errors.serialize_into(bytes);
        self.client_started.serialize_into(bytes);
        self.client_died.serialize_into(bytes);
    }
}

pub type ElementHeader = u8;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HType(u8);
impl HType {
    pub const FROM_SERVER_TIME: Self = Self(1 << 0);
    pub const FROM_CLIENT_TIME: Self = Self(1 << 1);
    pub const FROM_CLIENT_SEQUENCE: Self = Self(1 << 2);
}
impl From<HType> for u8 {
    #[inline]
    fn from(input: HType) -> Self {
        input.0
    }
}
impl From<HType> for Option<u8> {
    #[inline]
    fn from(input: HType) -> Self {
        Some(input.0)
    }
}
impl From<HType> for u16 {
    #[inline]
    fn from(input: HType) -> Self {
        u16::from(input.0)
    }
}
impl From<HType> for Option<u16> {
    #[inline]
    fn from(input: HType) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<HType> for u32 {
    #[inline]
    fn from(input: HType) -> Self {
        u32::from(input.0)
    }
}
impl From<HType> for Option<u32> {
    #[inline]
    fn from(input: HType) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for HType {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for HType  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::FROM_SERVER_TIME.0.into(), "FROM_SERVER_TIME", "FromServerTime"),
            (Self::FROM_CLIENT_TIME.0.into(), "FROM_CLIENT_TIME", "FromClientTime"),
            (Self::FROM_CLIENT_SEQUENCE.0.into(), "FROM_CLIENT_SEQUENCE", "FromClientSequence"),
        ];
        pretty_print_bitmask(fmt, self.0.into(), &variants)
    }
}
bitmask_binop!(HType, u8);

pub type ClientSpec = u32;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CS(u8);
impl CS {
    pub const CURRENT_CLIENTS: Self = Self(1);
    pub const FUTURE_CLIENTS: Self = Self(2);
    pub const ALL_CLIENTS: Self = Self(3);
}
impl From<CS> for u8 {
    #[inline]
    fn from(input: CS) -> Self {
        input.0
    }
}
impl From<CS> for Option<u8> {
    #[inline]
    fn from(input: CS) -> Self {
        Some(input.0)
    }
}
impl From<CS> for u16 {
    #[inline]
    fn from(input: CS) -> Self {
        u16::from(input.0)
    }
}
impl From<CS> for Option<u16> {
    #[inline]
    fn from(input: CS) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<CS> for u32 {
    #[inline]
    fn from(input: CS) -> Self {
        u32::from(input.0)
    }
}
impl From<CS> for Option<u32> {
    #[inline]
    fn from(input: CS) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for CS {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for CS  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::CURRENT_CLIENTS.0.into(), "CURRENT_CLIENTS", "CurrentClients"),
            (Self::FUTURE_CLIENTS.0.into(), "FUTURE_CLIENTS", "FutureClients"),
            (Self::ALL_CLIENTS.0.into(), "ALL_CLIENTS", "AllClients"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClientInfo {
    pub client_resource: ClientSpec,
    pub ranges: Vec<Range>,
}
impl_debug_if_no_extra_traits!(ClientInfo, "ClientInfo");
impl TryParse for ClientInfo {
    fn try_parse(remaining: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let (client_resource, remaining) = ClientSpec::try_parse(remaining)?;
        let (num_ranges, remaining) = u32::try_parse(remaining)?;
        let (ranges, remaining) = crate::x11_utils::parse_list::<Range>(remaining, num_ranges.try_to_usize()?)?;
        let result = ClientInfo { client_resource, ranges };
        Ok((result, remaining))
    }
}
impl Serialize for ClientInfo {
    type Bytes = Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let mut result = Vec::new();
        self.serialize_into(&mut result);
        result
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(8);
        self.client_resource.serialize_into(bytes);
        let num_ranges = u32::try_from(self.ranges.len()).expect("`ranges` has too many elements");
        num_ranges.serialize_into(bytes);
        self.ranges.serialize_into(bytes);
    }
}
impl ClientInfo {
    /// Get the value of the `num_ranges` field.
    ///
    /// The `num_ranges` field is used as the length field of the `ranges` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_ranges(&self) -> u32 {
        self.ranges.len()
            .try_into().unwrap()
    }
}

/// Opcode for the BadContext error
pub const BAD_CONTEXT_ERROR: u8 = 0;

/// Opcode for the QueryVersion request
pub const QUERY_VERSION_REQUEST: u8 = 0;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryVersionRequest {
    pub major_version: u16,
    pub minor_version: u16,
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
        if header.minor_opcode != QUERY_VERSION_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (major_version, remaining) = u16::try_parse(value)?;
        let (minor_version, remaining) = u16::try_parse(remaining)?;
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

/// Opcode for the CreateContext request
pub const CREATE_CONTEXT_REQUEST: u8 = 1;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateContextRequest<'input> {
    pub context: Context,
    pub element_header: ElementHeader,
    pub client_specs: Cow<'input, [ClientSpec]>,
    pub ranges: Cow<'input, [Range]>,
}
impl_debug_if_no_extra_traits!(CreateContextRequest<'_>, "CreateContextRequest");
impl<'input> CreateContextRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 4]> {
        let length_so_far = 0;
        let context_bytes = self.context.serialize();
        let element_header_bytes = self.element_header.serialize();
        let num_client_specs = u32::try_from(self.client_specs.len()).expect("`client_specs` has too many elements");
        let num_client_specs_bytes = num_client_specs.serialize();
        let num_ranges = u32::try_from(self.ranges.len()).expect("`ranges` has too many elements");
        let num_ranges_bytes = num_ranges.serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_CONTEXT_REQUEST,
            0,
            0,
            context_bytes[0],
            context_bytes[1],
            context_bytes[2],
            context_bytes[3],
            element_header_bytes[0],
            0,
            0,
            0,
            num_client_specs_bytes[0],
            num_client_specs_bytes[1],
            num_client_specs_bytes[2],
            num_client_specs_bytes[3],
            num_ranges_bytes[0],
            num_ranges_bytes[1],
            num_ranges_bytes[2],
            num_ranges_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let client_specs_bytes = self.client_specs.serialize();
        let length_so_far = length_so_far + client_specs_bytes.len();
        let ranges_bytes = self.ranges.serialize();
        let length_so_far = length_so_far + ranges_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), client_specs_bytes.into(), ranges_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != CREATE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (context, remaining) = Context::try_parse(value)?;
        let (element_header, remaining) = ElementHeader::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (num_client_specs, remaining) = u32::try_parse(remaining)?;
        let (num_ranges, remaining) = u32::try_parse(remaining)?;
        let (client_specs, remaining) = crate::x11_utils::parse_list::<ClientSpec>(remaining, num_client_specs.try_to_usize()?)?;
        let (ranges, remaining) = crate::x11_utils::parse_list::<Range>(remaining, num_ranges.try_to_usize()?)?;
        let _ = remaining;
        Ok(CreateContextRequest {
            context,
            element_header,
            client_specs: Cow::Owned(client_specs),
            ranges: Cow::Owned(ranges),
        })
    }
    /// Clone all borrowed data in this CreateContextRequest.
    pub fn into_owned(self) -> CreateContextRequest<'static> {
        CreateContextRequest {
            context: self.context,
            element_header: self.element_header,
            client_specs: Cow::Owned(self.client_specs.into_owned()),
            ranges: Cow::Owned(self.ranges.into_owned()),
        }
    }
}
impl<'input> Request for CreateContextRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for CreateContextRequest<'input> {
}

/// Opcode for the RegisterClients request
pub const REGISTER_CLIENTS_REQUEST: u8 = 2;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RegisterClientsRequest<'input> {
    pub context: Context,
    pub element_header: ElementHeader,
    pub client_specs: Cow<'input, [ClientSpec]>,
    pub ranges: Cow<'input, [Range]>,
}
impl_debug_if_no_extra_traits!(RegisterClientsRequest<'_>, "RegisterClientsRequest");
impl<'input> RegisterClientsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 4]> {
        let length_so_far = 0;
        let context_bytes = self.context.serialize();
        let element_header_bytes = self.element_header.serialize();
        let num_client_specs = u32::try_from(self.client_specs.len()).expect("`client_specs` has too many elements");
        let num_client_specs_bytes = num_client_specs.serialize();
        let num_ranges = u32::try_from(self.ranges.len()).expect("`ranges` has too many elements");
        let num_ranges_bytes = num_ranges.serialize();
        let mut request0 = vec![
            major_opcode,
            REGISTER_CLIENTS_REQUEST,
            0,
            0,
            context_bytes[0],
            context_bytes[1],
            context_bytes[2],
            context_bytes[3],
            element_header_bytes[0],
            0,
            0,
            0,
            num_client_specs_bytes[0],
            num_client_specs_bytes[1],
            num_client_specs_bytes[2],
            num_client_specs_bytes[3],
            num_ranges_bytes[0],
            num_ranges_bytes[1],
            num_ranges_bytes[2],
            num_ranges_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let client_specs_bytes = self.client_specs.serialize();
        let length_so_far = length_so_far + client_specs_bytes.len();
        let ranges_bytes = self.ranges.serialize();
        let length_so_far = length_so_far + ranges_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), client_specs_bytes.into(), ranges_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != REGISTER_CLIENTS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (context, remaining) = Context::try_parse(value)?;
        let (element_header, remaining) = ElementHeader::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (num_client_specs, remaining) = u32::try_parse(remaining)?;
        let (num_ranges, remaining) = u32::try_parse(remaining)?;
        let (client_specs, remaining) = crate::x11_utils::parse_list::<ClientSpec>(remaining, num_client_specs.try_to_usize()?)?;
        let (ranges, remaining) = crate::x11_utils::parse_list::<Range>(remaining, num_ranges.try_to_usize()?)?;
        let _ = remaining;
        Ok(RegisterClientsRequest {
            context,
            element_header,
            client_specs: Cow::Owned(client_specs),
            ranges: Cow::Owned(ranges),
        })
    }
    /// Clone all borrowed data in this RegisterClientsRequest.
    pub fn into_owned(self) -> RegisterClientsRequest<'static> {
        RegisterClientsRequest {
            context: self.context,
            element_header: self.element_header,
            client_specs: Cow::Owned(self.client_specs.into_owned()),
            ranges: Cow::Owned(self.ranges.into_owned()),
        }
    }
}
impl<'input> Request for RegisterClientsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for RegisterClientsRequest<'input> {
}

/// Opcode for the UnregisterClients request
pub const UNREGISTER_CLIENTS_REQUEST: u8 = 3;
#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnregisterClientsRequest<'input> {
    pub context: Context,
    pub client_specs: Cow<'input, [ClientSpec]>,
}
impl_debug_if_no_extra_traits!(UnregisterClientsRequest<'_>, "UnregisterClientsRequest");
impl<'input> UnregisterClientsRequest<'input> {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'input, [u8]>; 3]> {
        let length_so_far = 0;
        let context_bytes = self.context.serialize();
        let num_client_specs = u32::try_from(self.client_specs.len()).expect("`client_specs` has too many elements");
        let num_client_specs_bytes = num_client_specs.serialize();
        let mut request0 = vec![
            major_opcode,
            UNREGISTER_CLIENTS_REQUEST,
            0,
            0,
            context_bytes[0],
            context_bytes[1],
            context_bytes[2],
            context_bytes[3],
            num_client_specs_bytes[0],
            num_client_specs_bytes[1],
            num_client_specs_bytes[2],
            num_client_specs_bytes[3],
        ];
        let length_so_far = length_so_far + request0.len();
        let client_specs_bytes = self.client_specs.serialize();
        let length_so_far = length_so_far + client_specs_bytes.len();
        let padding0 = &[0; 3][..(4 - (length_so_far % 4)) % 4];
        let length_so_far = length_so_far + padding0.len();
        assert_eq!(length_so_far % 4, 0);
        let length = u16::try_from(length_so_far / 4).unwrap_or(0);
        request0[2..4].copy_from_slice(&length.to_ne_bytes());
        ([request0.into(), client_specs_bytes.into(), padding0.into()], vec![])
    }
    /// Parse this request given its header, its body, and any fds that go along with it
    #[cfg(feature = "request-parsing")]
    pub fn try_parse_request(header: RequestHeader, value: &'input [u8]) -> Result<Self, ParseError> {
        if header.minor_opcode != UNREGISTER_CLIENTS_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (context, remaining) = Context::try_parse(value)?;
        let (num_client_specs, remaining) = u32::try_parse(remaining)?;
        let (client_specs, remaining) = crate::x11_utils::parse_list::<ClientSpec>(remaining, num_client_specs.try_to_usize()?)?;
        let _ = remaining;
        Ok(UnregisterClientsRequest {
            context,
            client_specs: Cow::Owned(client_specs),
        })
    }
    /// Clone all borrowed data in this UnregisterClientsRequest.
    pub fn into_owned(self) -> UnregisterClientsRequest<'static> {
        UnregisterClientsRequest {
            context: self.context,
            client_specs: Cow::Owned(self.client_specs.into_owned()),
        }
    }
}
impl<'input> Request for UnregisterClientsRequest<'input> {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl<'input> crate::x11_utils::VoidRequest for UnregisterClientsRequest<'input> {
}

/// Opcode for the GetContext request
pub const GET_CONTEXT_REQUEST: u8 = 4;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetContextRequest {
    pub context: Context,
}
impl_debug_if_no_extra_traits!(GetContextRequest, "GetContextRequest");
impl GetContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let context_bytes = self.context.serialize();
        let mut request0 = vec![
            major_opcode,
            GET_CONTEXT_REQUEST,
            0,
            0,
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
        if header.minor_opcode != GET_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (context, remaining) = Context::try_parse(value)?;
        let _ = remaining;
        Ok(GetContextRequest {
            context,
        })
    }
}
impl Request for GetContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for GetContextRequest {
    type Reply = GetContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GetContextReply {
    pub enabled: bool,
    pub sequence: u16,
    pub length: u32,
    pub element_header: ElementHeader,
    pub intercepted_clients: Vec<ClientInfo>,
}
impl_debug_if_no_extra_traits!(GetContextReply, "GetContextReply");
impl TryParse for GetContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (enabled, remaining) = bool::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (element_header, remaining) = ElementHeader::try_parse(remaining)?;
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let (num_intercepted_clients, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(16..).ok_or(ParseError::InsufficientData)?;
        let (intercepted_clients, remaining) = crate::x11_utils::parse_list::<ClientInfo>(remaining, num_intercepted_clients.try_to_usize()?)?;
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = GetContextReply { enabled, sequence, length, element_header, intercepted_clients };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for GetContextReply {
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
        self.enabled.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.length.serialize_into(bytes);
        self.element_header.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 3]);
        let num_intercepted_clients = u32::try_from(self.intercepted_clients.len()).expect("`intercepted_clients` has too many elements");
        num_intercepted_clients.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 16]);
        self.intercepted_clients.serialize_into(bytes);
    }
}
impl GetContextReply {
    /// Get the value of the `num_intercepted_clients` field.
    ///
    /// The `num_intercepted_clients` field is used as the length field of the `intercepted_clients` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn num_intercepted_clients(&self) -> u32 {
        self.intercepted_clients.len()
            .try_into().unwrap()
    }
}

/// Opcode for the EnableContext request
pub const ENABLE_CONTEXT_REQUEST: u8 = 5;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EnableContextRequest {
    pub context: Context,
}
impl_debug_if_no_extra_traits!(EnableContextRequest, "EnableContextRequest");
impl EnableContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let context_bytes = self.context.serialize();
        let mut request0 = vec![
            major_opcode,
            ENABLE_CONTEXT_REQUEST,
            0,
            0,
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
        if header.minor_opcode != ENABLE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (context, remaining) = Context::try_parse(value)?;
        let _ = remaining;
        Ok(EnableContextRequest {
            context,
        })
    }
}
impl Request for EnableContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::ReplyRequest for EnableContextRequest {
    type Reply = EnableContextReply;
}

#[derive(Clone, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EnableContextReply {
    pub category: u8,
    pub sequence: u16,
    pub element_header: ElementHeader,
    pub client_swapped: bool,
    pub xid_base: u32,
    pub server_time: u32,
    pub rec_sequence_num: u32,
    pub data: Vec<u8>,
}
impl_debug_if_no_extra_traits!(EnableContextReply, "EnableContextReply");
impl TryParse for EnableContextReply {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (category, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (length, remaining) = u32::try_parse(remaining)?;
        let (element_header, remaining) = ElementHeader::try_parse(remaining)?;
        let (client_swapped, remaining) = bool::try_parse(remaining)?;
        let remaining = remaining.get(2..).ok_or(ParseError::InsufficientData)?;
        let (xid_base, remaining) = u32::try_parse(remaining)?;
        let (server_time, remaining) = u32::try_parse(remaining)?;
        let (rec_sequence_num, remaining) = u32::try_parse(remaining)?;
        let remaining = remaining.get(8..).ok_or(ParseError::InsufficientData)?;
        let (data, remaining) = crate::x11_utils::parse_u8_list(remaining, u32::from(length).checked_mul(4u32).ok_or(ParseError::InvalidExpression)?.try_to_usize()?)?;
        let data = data.to_vec();
        if response_type != 1 {
            return Err(ParseError::InvalidValue);
        }
        let result = EnableContextReply { category, sequence, element_header, client_swapped, xid_base, server_time, rec_sequence_num, data };
        let _ = remaining;
        let remaining = initial_value.get(32 + length as usize * 4..)
            .ok_or(ParseError::InsufficientData)?;
        Ok((result, remaining))
    }
}
impl Serialize for EnableContextReply {
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
        self.category.serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        assert_eq!(self.data.len() % 4, 0, "`data` has an incorrect length, must be a multiple of 4");
        let length = u32::try_from(self.data.len() / 4).expect("`data` has too many elements");
        length.serialize_into(bytes);
        self.element_header.serialize_into(bytes);
        self.client_swapped.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 2]);
        self.xid_base.serialize_into(bytes);
        self.server_time.serialize_into(bytes);
        self.rec_sequence_num.serialize_into(bytes);
        bytes.extend_from_slice(&[0; 8]);
        bytes.extend_from_slice(&self.data);
    }
}
impl EnableContextReply {
    /// Get the value of the `length` field.
    ///
    /// The `length` field is used as the length field of the `data` field.
    /// This function computes the field's value again based on the length of the list.
    ///
    /// # Panics
    ///
    /// Panics if the value cannot be represented in the target type. This
    /// cannot happen with values of the struct received from the X11 server.
    pub fn length(&self) -> u32 {
        self.data.len()
            .checked_div(4).unwrap()
            .try_into().unwrap()
    }
}

/// Opcode for the DisableContext request
pub const DISABLE_CONTEXT_REQUEST: u8 = 6;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DisableContextRequest {
    pub context: Context,
}
impl_debug_if_no_extra_traits!(DisableContextRequest, "DisableContextRequest");
impl DisableContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let context_bytes = self.context.serialize();
        let mut request0 = vec![
            major_opcode,
            DISABLE_CONTEXT_REQUEST,
            0,
            0,
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
        if header.minor_opcode != DISABLE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (context, remaining) = Context::try_parse(value)?;
        let _ = remaining;
        Ok(DisableContextRequest {
            context,
        })
    }
}
impl Request for DisableContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DisableContextRequest {
}

/// Opcode for the FreeContext request
pub const FREE_CONTEXT_REQUEST: u8 = 7;
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FreeContextRequest {
    pub context: Context,
}
impl_debug_if_no_extra_traits!(FreeContextRequest, "FreeContextRequest");
impl FreeContextRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let context_bytes = self.context.serialize();
        let mut request0 = vec![
            major_opcode,
            FREE_CONTEXT_REQUEST,
            0,
            0,
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
        if header.minor_opcode != FREE_CONTEXT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (context, remaining) = Context::try_parse(value)?;
        let _ = remaining;
        Ok(FreeContextRequest {
            context,
        })
    }
}
impl Request for FreeContextRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for FreeContextRequest {
}

