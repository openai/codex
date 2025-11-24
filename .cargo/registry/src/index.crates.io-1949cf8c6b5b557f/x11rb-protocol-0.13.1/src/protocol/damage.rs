// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Damage` X11 extension.

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
use super::xfixes;
#[allow(unused_imports)]
use super::xproto;

/// The X11 name of the extension for QueryExtension
pub const X11_EXTENSION_NAME: &str = "DAMAGE";

/// The version number of this extension that this client library supports.
///
/// This constant contains the version number of this extension that is supported
/// by this build of x11rb. For most things, it does not make sense to use this
/// information. If you need to send a `QueryVersion`, it is recommended to instead
/// send the maximum version of the extension that you need.
pub const X11_XML_VERSION: (u32, u32) = (1, 1);

pub type Damage = u32;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReportLevel(u8);
impl ReportLevel {
    pub const RAW_RECTANGLES: Self = Self(0);
    pub const DELTA_RECTANGLES: Self = Self(1);
    pub const BOUNDING_BOX: Self = Self(2);
    pub const NON_EMPTY: Self = Self(3);
}
impl From<ReportLevel> for u8 {
    #[inline]
    fn from(input: ReportLevel) -> Self {
        input.0
    }
}
impl From<ReportLevel> for Option<u8> {
    #[inline]
    fn from(input: ReportLevel) -> Self {
        Some(input.0)
    }
}
impl From<ReportLevel> for u16 {
    #[inline]
    fn from(input: ReportLevel) -> Self {
        u16::from(input.0)
    }
}
impl From<ReportLevel> for Option<u16> {
    #[inline]
    fn from(input: ReportLevel) -> Self {
        Some(u16::from(input.0))
    }
}
impl From<ReportLevel> for u32 {
    #[inline]
    fn from(input: ReportLevel) -> Self {
        u32::from(input.0)
    }
}
impl From<ReportLevel> for Option<u32> {
    #[inline]
    fn from(input: ReportLevel) -> Self {
        Some(u32::from(input.0))
    }
}
impl From<u8> for ReportLevel {
    #[inline]
    fn from(value: u8) -> Self {
        Self(value)
    }
}
impl core::fmt::Debug for ReportLevel  {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let variants = [
            (Self::RAW_RECTANGLES.0.into(), "RAW_RECTANGLES", "RawRectangles"),
            (Self::DELTA_RECTANGLES.0.into(), "DELTA_RECTANGLES", "DeltaRectangles"),
            (Self::BOUNDING_BOX.0.into(), "BOUNDING_BOX", "BoundingBox"),
            (Self::NON_EMPTY.0.into(), "NON_EMPTY", "NonEmpty"),
        ];
        pretty_print_enum(fmt, self.0.into(), &variants)
    }
}

/// Opcode for the BadDamage error
pub const BAD_DAMAGE_ERROR: u8 = 0;

/// Opcode for the QueryVersion request
pub const QUERY_VERSION_REQUEST: u8 = 0;
/// Negotiate the version of the DAMAGE extension.
///
/// This negotiates the version of the DAMAGE extension.  It must precede any other
/// request using the DAMAGE extension.  Failure to do so will cause a BadRequest
/// error for those requests.
///
/// # Fields
///
/// * `client_major_version` - The major version supported by the client.
/// * `client_minor_version` - The minor version supported by the client.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QueryVersionRequest {
    pub client_major_version: u32,
    pub client_minor_version: u32,
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
            client_major_version_bytes[1],
            client_major_version_bytes[2],
            client_major_version_bytes[3],
            client_minor_version_bytes[0],
            client_minor_version_bytes[1],
            client_minor_version_bytes[2],
            client_minor_version_bytes[3],
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
        let (client_major_version, remaining) = u32::try_parse(value)?;
        let (client_minor_version, remaining) = u32::try_parse(remaining)?;
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

/// The negotiated version of the DAMAGE extension.
///
/// This indicates the version of the DAMAGE extension chosen by the server.
/// It will always be less than or equal to the version offered by the client.
///
/// # Fields
///
/// * `major_version` - The major version chosen by the server.
/// * `minor_version` - The minor version chosen by the server.
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
        let remaining = remaining.get(16..).ok_or(ParseError::InsufficientData)?;
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
            major_version_bytes[1],
            major_version_bytes[2],
            major_version_bytes[3],
            minor_version_bytes[0],
            minor_version_bytes[1],
            minor_version_bytes[2],
            minor_version_bytes[3],
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
        bytes.extend_from_slice(&[0; 16]);
    }
}

/// Opcode for the Create request
pub const CREATE_REQUEST: u8 = 1;
/// Creates a Damage object to monitor changes to a drawable..
///
/// This creates a Damage object to monitor changes to a drawable, and specifies
/// the level of detail to be reported for changes.
///
/// We call changes made to pixel contents of windows and pixmaps 'damage'
/// throughout this extension.
///
/// Damage accumulates as drawing occurs in the drawable.  Each drawing operation
/// 'damages' one or more rectangular areas within the drawable.  The rectangles
/// are guaranteed to include the set of pixels modified by each operation, but
/// may include significantly more than just those pixels.  The desire is for
/// the damage to strike a balance between the number of rectangles reported and
/// the extraneous area included.  A reasonable goal is for each primitive
/// object drawn (line, string, rectangle) to be represented as a single
/// rectangle and for the damage area of the operation to be the union of these
/// rectangles.
///
/// The DAMAGE extension allows applications to either receive the raw
/// rectangles as a stream of events, or to have them partially processed within
/// the X server to reduce the amount of data transmitted as well as reduce the
/// processing latency once the repaint operation has started.
///
/// The Damage object holds any accumulated damage region and reflects the
/// relationship between the drawable selected for damage notification and the
/// drawable for which damage is tracked.
///
/// # Fields
///
/// * `damage` - The ID with which you will refer to the new Damage object, created by
/// `xcb_generate_id`.
/// * `drawable` - The ID of the drawable to be monitored.
/// * `level` - The level of detail to be provided in Damage events.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreateRequest {
    pub damage: Damage,
    pub drawable: xproto::Drawable,
    pub level: ReportLevel,
}
impl_debug_if_no_extra_traits!(CreateRequest, "CreateRequest");
impl CreateRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let damage_bytes = self.damage.serialize();
        let drawable_bytes = self.drawable.serialize();
        let level_bytes = u8::from(self.level).serialize();
        let mut request0 = vec![
            major_opcode,
            CREATE_REQUEST,
            0,
            0,
            damage_bytes[0],
            damage_bytes[1],
            damage_bytes[2],
            damage_bytes[3],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            level_bytes[0],
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
        if header.minor_opcode != CREATE_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (damage, remaining) = Damage::try_parse(value)?;
        let (drawable, remaining) = xproto::Drawable::try_parse(remaining)?;
        let (level, remaining) = u8::try_parse(remaining)?;
        let level = level.into();
        let remaining = remaining.get(3..).ok_or(ParseError::InsufficientData)?;
        let _ = remaining;
        Ok(CreateRequest {
            damage,
            drawable,
            level,
        })
    }
}
impl Request for CreateRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for CreateRequest {
}

/// Opcode for the Destroy request
pub const DESTROY_REQUEST: u8 = 2;
/// Destroys a previously created Damage object..
///
/// This destroys a Damage object and requests the X server stop reporting
/// the changes it was tracking.
///
/// # Fields
///
/// * `damage` - The ID you provided to `xcb_create_damage`.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DestroyRequest {
    pub damage: Damage,
}
impl_debug_if_no_extra_traits!(DestroyRequest, "DestroyRequest");
impl DestroyRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let damage_bytes = self.damage.serialize();
        let mut request0 = vec![
            major_opcode,
            DESTROY_REQUEST,
            0,
            0,
            damage_bytes[0],
            damage_bytes[1],
            damage_bytes[2],
            damage_bytes[3],
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
        if header.minor_opcode != DESTROY_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (damage, remaining) = Damage::try_parse(value)?;
        let _ = remaining;
        Ok(DestroyRequest {
            damage,
        })
    }
}
impl Request for DestroyRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for DestroyRequest {
}

/// Opcode for the Subtract request
pub const SUBTRACT_REQUEST: u8 = 3;
/// Remove regions from a previously created Damage object..
///
/// This updates the regions of damage recorded in a a Damage object.
/// See <https://www.x.org/releases/current/doc/damageproto/damageproto.txt>
/// for details.
///
/// # Fields
///
/// * `damage` - The ID you provided to `xcb_create_damage`.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SubtractRequest {
    pub damage: Damage,
    pub repair: xfixes::Region,
    pub parts: xfixes::Region,
}
impl_debug_if_no_extra_traits!(SubtractRequest, "SubtractRequest");
impl SubtractRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let damage_bytes = self.damage.serialize();
        let repair_bytes = self.repair.serialize();
        let parts_bytes = self.parts.serialize();
        let mut request0 = vec![
            major_opcode,
            SUBTRACT_REQUEST,
            0,
            0,
            damage_bytes[0],
            damage_bytes[1],
            damage_bytes[2],
            damage_bytes[3],
            repair_bytes[0],
            repair_bytes[1],
            repair_bytes[2],
            repair_bytes[3],
            parts_bytes[0],
            parts_bytes[1],
            parts_bytes[2],
            parts_bytes[3],
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
        if header.minor_opcode != SUBTRACT_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (damage, remaining) = Damage::try_parse(value)?;
        let (repair, remaining) = xfixes::Region::try_parse(remaining)?;
        let (parts, remaining) = xfixes::Region::try_parse(remaining)?;
        let _ = remaining;
        Ok(SubtractRequest {
            damage,
            repair,
            parts,
        })
    }
}
impl Request for SubtractRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for SubtractRequest {
}

/// Opcode for the Add request
pub const ADD_REQUEST: u8 = 4;
/// Add a region to a previously created Damage object..
///
/// This updates the regions of damage recorded in a a Damage object.
/// See <https://www.x.org/releases/current/doc/damageproto/damageproto.txt>
/// for details.
///
/// # Fields
///
/// * `damage` - The ID you provided to `xcb_create_damage`.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AddRequest {
    pub drawable: xproto::Drawable,
    pub region: xfixes::Region,
}
impl_debug_if_no_extra_traits!(AddRequest, "AddRequest");
impl AddRequest {
    /// Serialize this request into bytes for the provided connection
    pub fn serialize(self, major_opcode: u8) -> BufWithFds<[Cow<'static, [u8]>; 1]> {
        let length_so_far = 0;
        let drawable_bytes = self.drawable.serialize();
        let region_bytes = self.region.serialize();
        let mut request0 = vec![
            major_opcode,
            ADD_REQUEST,
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
        if header.minor_opcode != ADD_REQUEST {
            return Err(ParseError::InvalidValue);
        }
        let (drawable, remaining) = xproto::Drawable::try_parse(value)?;
        let (region, remaining) = xfixes::Region::try_parse(remaining)?;
        let _ = remaining;
        Ok(AddRequest {
            drawable,
            region,
        })
    }
}
impl Request for AddRequest {
    const EXTENSION_NAME: Option<&'static str> = Some(X11_EXTENSION_NAME);

    fn serialize(self, major_opcode: u8) -> BufWithFds<Vec<u8>> {
        let (bufs, fds) = self.serialize(major_opcode);
        // Flatten the buffers into a single vector
        let buf = bufs.iter().flat_map(|buf| buf.iter().copied()).collect();
        (buf, fds)
    }
}
impl crate::x11_utils::VoidRequest for AddRequest {
}

/// Opcode for the Notify event
pub const NOTIFY_EVENT: u8 = 0;
/// the contents of the monitored drawable have changed.
///
/// # Fields
///
/// * `level` - The level of the damage being reported.
/// If the 0x80 bit is set, indicates there are subsequent Damage events
/// being delivered immediately as part of a larger Damage region.
/// * `drawable` - The drawable for which damage is being reported.
/// * `damage` - The Damage object being used to track the damage.
/// * `timestamp` - Time when the event was generated (in milliseconds).
/// * `area` - Damaged area of the drawable.
/// * `geometry` - Total area of the drawable.
///
/// # See
///
/// * `Create`: request
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "extra-traits", derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NotifyEvent {
    pub response_type: u8,
    pub level: ReportLevel,
    pub sequence: u16,
    pub drawable: xproto::Drawable,
    pub damage: Damage,
    pub timestamp: xproto::Timestamp,
    pub area: xproto::Rectangle,
    pub geometry: xproto::Rectangle,
}
impl_debug_if_no_extra_traits!(NotifyEvent, "NotifyEvent");
impl TryParse for NotifyEvent {
    fn try_parse(initial_value: &[u8]) -> Result<(Self, &[u8]), ParseError> {
        let remaining = initial_value;
        let (response_type, remaining) = u8::try_parse(remaining)?;
        let (level, remaining) = u8::try_parse(remaining)?;
        let (sequence, remaining) = u16::try_parse(remaining)?;
        let (drawable, remaining) = xproto::Drawable::try_parse(remaining)?;
        let (damage, remaining) = Damage::try_parse(remaining)?;
        let (timestamp, remaining) = xproto::Timestamp::try_parse(remaining)?;
        let (area, remaining) = xproto::Rectangle::try_parse(remaining)?;
        let (geometry, remaining) = xproto::Rectangle::try_parse(remaining)?;
        let level = level.into();
        let result = NotifyEvent { response_type, level, sequence, drawable, damage, timestamp, area, geometry };
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
        let level_bytes = u8::from(self.level).serialize();
        let sequence_bytes = self.sequence.serialize();
        let drawable_bytes = self.drawable.serialize();
        let damage_bytes = self.damage.serialize();
        let timestamp_bytes = self.timestamp.serialize();
        let area_bytes = self.area.serialize();
        let geometry_bytes = self.geometry.serialize();
        [
            response_type_bytes[0],
            level_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            damage_bytes[0],
            damage_bytes[1],
            damage_bytes[2],
            damage_bytes[3],
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            area_bytes[0],
            area_bytes[1],
            area_bytes[2],
            area_bytes[3],
            area_bytes[4],
            area_bytes[5],
            area_bytes[6],
            area_bytes[7],
            geometry_bytes[0],
            geometry_bytes[1],
            geometry_bytes[2],
            geometry_bytes[3],
            geometry_bytes[4],
            geometry_bytes[5],
            geometry_bytes[6],
            geometry_bytes[7],
        ]
    }
    fn serialize_into(&self, bytes: &mut Vec<u8>) {
        bytes.reserve(32);
        self.response_type.serialize_into(bytes);
        u8::from(self.level).serialize_into(bytes);
        self.sequence.serialize_into(bytes);
        self.drawable.serialize_into(bytes);
        self.damage.serialize_into(bytes);
        self.timestamp.serialize_into(bytes);
        self.area.serialize_into(bytes);
        self.geometry.serialize_into(bytes);
    }
}
impl From<&NotifyEvent> for [u8; 32] {
    fn from(input: &NotifyEvent) -> Self {
        let response_type_bytes = input.response_type.serialize();
        let level_bytes = u8::from(input.level).serialize();
        let sequence_bytes = input.sequence.serialize();
        let drawable_bytes = input.drawable.serialize();
        let damage_bytes = input.damage.serialize();
        let timestamp_bytes = input.timestamp.serialize();
        let area_bytes = input.area.serialize();
        let geometry_bytes = input.geometry.serialize();
        [
            response_type_bytes[0],
            level_bytes[0],
            sequence_bytes[0],
            sequence_bytes[1],
            drawable_bytes[0],
            drawable_bytes[1],
            drawable_bytes[2],
            drawable_bytes[3],
            damage_bytes[0],
            damage_bytes[1],
            damage_bytes[2],
            damage_bytes[3],
            timestamp_bytes[0],
            timestamp_bytes[1],
            timestamp_bytes[2],
            timestamp_bytes[3],
            area_bytes[0],
            area_bytes[1],
            area_bytes[2],
            area_bytes[3],
            area_bytes[4],
            area_bytes[5],
            area_bytes[6],
            area_bytes[7],
            geometry_bytes[0],
            geometry_bytes[1],
            geometry_bytes[2],
            geometry_bytes[3],
            geometry_bytes[4],
            geometry_bytes[5],
            geometry_bytes[6],
            geometry_bytes[7],
        ]
    }
}
impl From<NotifyEvent> for [u8; 32] {
    fn from(input: NotifyEvent) -> Self {
        Self::from(&input)
    }
}

