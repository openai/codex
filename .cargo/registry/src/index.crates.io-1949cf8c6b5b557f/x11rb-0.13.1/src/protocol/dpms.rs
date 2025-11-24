// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `DPMS` X11 extension.

#![allow(clippy::too_many_arguments)]

#[allow(unused_imports)]
use std::borrow::Cow;
#[allow(unused_imports)]
use std::convert::TryInto;
#[allow(unused_imports)]
use crate::utils::RawFdContainer;
#[allow(unused_imports)]
use crate::x11_utils::{Request, RequestHeader, Serialize, TryParse, TryParseFd};
use std::io::IoSlice;
use crate::connection::RequestConnection;
#[allow(unused_imports)]
use crate::connection::Connection as X11Connection;
#[allow(unused_imports)]
use crate::cookie::{Cookie, CookieWithFds, VoidCookie};
use crate::errors::ConnectionError;
#[allow(unused_imports)]
use crate::errors::ReplyOrIdError;
#[allow(unused_imports)]
use super::xproto;

pub use x11rb_protocol::protocol::dpms::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

pub fn get_version<Conn>(conn: &Conn, client_major_version: u16, client_minor_version: u16) -> Result<Cookie<'_, Conn, GetVersionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetVersionRequest {
        client_major_version,
        client_minor_version,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn capable<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, CapableReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CapableRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_timeouts<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, GetTimeoutsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetTimeoutsRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_timeouts<Conn>(conn: &Conn, standby_timeout: u16, suspend_timeout: u16, off_timeout: u16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetTimeoutsRequest {
        standby_timeout,
        suspend_timeout,
        off_timeout,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn enable<Conn>(conn: &Conn) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = EnableRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn disable<Conn>(conn: &Conn) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DisableRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn force_level<Conn>(conn: &Conn, power_level: DPMSMode) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ForceLevelRequest {
        power_level,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn info<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, InfoReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = InfoRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn select_input<Conn>(conn: &Conn, event_mask: EventMask) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SelectInputRequest {
        event_mask,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn dpms_get_version(&self, client_major_version: u16, client_minor_version: u16) -> Result<Cookie<'_, Self, GetVersionReply>, ConnectionError>
    {
        get_version(self, client_major_version, client_minor_version)
    }
    fn dpms_capable(&self) -> Result<Cookie<'_, Self, CapableReply>, ConnectionError>
    {
        capable(self)
    }
    fn dpms_get_timeouts(&self) -> Result<Cookie<'_, Self, GetTimeoutsReply>, ConnectionError>
    {
        get_timeouts(self)
    }
    fn dpms_set_timeouts(&self, standby_timeout: u16, suspend_timeout: u16, off_timeout: u16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        set_timeouts(self, standby_timeout, suspend_timeout, off_timeout)
    }
    fn dpms_enable(&self) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        enable(self)
    }
    fn dpms_disable(&self) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        disable(self)
    }
    fn dpms_force_level(&self, power_level: DPMSMode) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        force_level(self, power_level)
    }
    fn dpms_info(&self) -> Result<Cookie<'_, Self, InfoReply>, ConnectionError>
    {
        info(self)
    }
    fn dpms_select_input(&self, event_mask: EventMask) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        select_input(self, event_mask)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
