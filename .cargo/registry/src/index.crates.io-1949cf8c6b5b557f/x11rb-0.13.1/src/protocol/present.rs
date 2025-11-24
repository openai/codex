// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Present` X11 extension.

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
use super::dri3;
#[allow(unused_imports)]
use super::randr;
#[allow(unused_imports)]
use super::sync;
#[allow(unused_imports)]
use super::xfixes;
#[allow(unused_imports)]
use super::xproto;

pub use x11rb_protocol::protocol::present::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

pub fn query_version<Conn>(conn: &Conn, major_version: u32, minor_version: u32) -> Result<Cookie<'_, Conn, QueryVersionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryVersionRequest {
        major_version,
        minor_version,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn pixmap<'c, 'input, Conn>(conn: &'c Conn, window: xproto::Window, pixmap: xproto::Pixmap, serial: u32, valid: xfixes::Region, update: xfixes::Region, x_off: i16, y_off: i16, target_crtc: randr::Crtc, wait_fence: sync::Fence, idle_fence: sync::Fence, options: u32, target_msc: u64, divisor: u64, remainder: u64, notifies: &'input [Notify]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PixmapRequest {
        window,
        pixmap,
        serial,
        valid,
        update,
        x_off,
        y_off,
        target_crtc,
        wait_fence,
        idle_fence,
        options,
        target_msc,
        divisor,
        remainder,
        notifies: Cow::Borrowed(notifies),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn notify_msc<Conn>(conn: &Conn, window: xproto::Window, serial: u32, target_msc: u64, divisor: u64, remainder: u64) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = NotifyMSCRequest {
        window,
        serial,
        target_msc,
        divisor,
        remainder,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn select_input<Conn>(conn: &Conn, eid: Event, window: xproto::Window, event_mask: EventMask) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SelectInputRequest {
        eid,
        window,
        event_mask,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn query_capabilities<Conn>(conn: &Conn, target: u32) -> Result<Cookie<'_, Conn, QueryCapabilitiesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryCapabilitiesRequest {
        target,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn pixmap_synced<'c, 'input, Conn>(conn: &'c Conn, window: xproto::Window, pixmap: xproto::Pixmap, serial: u32, valid: xfixes::Region, update: xfixes::Region, x_off: i16, y_off: i16, target_crtc: randr::Crtc, acquire_syncobj: dri3::Syncobj, release_syncobj: dri3::Syncobj, acquire_point: u64, release_point: u64, options: u32, target_msc: u64, divisor: u64, remainder: u64, notifies: &'input [Notify]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PixmapSyncedRequest {
        window,
        pixmap,
        serial,
        valid,
        update,
        x_off,
        y_off,
        target_crtc,
        acquire_syncobj,
        release_syncobj,
        acquire_point,
        release_point,
        options,
        target_msc,
        divisor,
        remainder,
        notifies: Cow::Borrowed(notifies),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn present_query_version(&self, major_version: u32, minor_version: u32) -> Result<Cookie<'_, Self, QueryVersionReply>, ConnectionError>
    {
        query_version(self, major_version, minor_version)
    }
    fn present_pixmap<'c, 'input>(&'c self, window: xproto::Window, pixmap: xproto::Pixmap, serial: u32, valid: xfixes::Region, update: xfixes::Region, x_off: i16, y_off: i16, target_crtc: randr::Crtc, wait_fence: sync::Fence, idle_fence: sync::Fence, options: u32, target_msc: u64, divisor: u64, remainder: u64, notifies: &'input [Notify]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        self::pixmap(self, window, pixmap, serial, valid, update, x_off, y_off, target_crtc, wait_fence, idle_fence, options, target_msc, divisor, remainder, notifies)
    }
    fn present_notify_msc(&self, window: xproto::Window, serial: u32, target_msc: u64, divisor: u64, remainder: u64) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        notify_msc(self, window, serial, target_msc, divisor, remainder)
    }
    fn present_select_input(&self, eid: Event, window: xproto::Window, event_mask: EventMask) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        select_input(self, eid, window, event_mask)
    }
    fn present_query_capabilities(&self, target: u32) -> Result<Cookie<'_, Self, QueryCapabilitiesReply>, ConnectionError>
    {
        query_capabilities(self, target)
    }
    fn present_pixmap_synced<'c, 'input>(&'c self, window: xproto::Window, pixmap: xproto::Pixmap, serial: u32, valid: xfixes::Region, update: xfixes::Region, x_off: i16, y_off: i16, target_crtc: randr::Crtc, acquire_syncobj: dri3::Syncobj, release_syncobj: dri3::Syncobj, acquire_point: u64, release_point: u64, options: u32, target_msc: u64, divisor: u64, remainder: u64, notifies: &'input [Notify]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        pixmap_synced(self, window, pixmap, serial, valid, update, x_off, y_off, target_crtc, acquire_syncobj, release_syncobj, acquire_point, release_point, options, target_msc, divisor, remainder, notifies)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
