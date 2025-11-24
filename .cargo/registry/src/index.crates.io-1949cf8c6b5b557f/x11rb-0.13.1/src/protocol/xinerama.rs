// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Xinerama` X11 extension.

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

pub use x11rb_protocol::protocol::xinerama::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

pub fn query_version<Conn>(conn: &Conn, major: u8, minor: u8) -> Result<Cookie<'_, Conn, QueryVersionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryVersionRequest {
        major,
        minor,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_state<Conn>(conn: &Conn, window: xproto::Window) -> Result<Cookie<'_, Conn, GetStateReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetStateRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_screen_count<Conn>(conn: &Conn, window: xproto::Window) -> Result<Cookie<'_, Conn, GetScreenCountReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetScreenCountRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_screen_size<Conn>(conn: &Conn, window: xproto::Window, screen: u32) -> Result<Cookie<'_, Conn, GetScreenSizeReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetScreenSizeRequest {
        window,
        screen,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn is_active<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, IsActiveReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = IsActiveRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_screens<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, QueryScreensReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryScreensRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn xinerama_query_version(&self, major: u8, minor: u8) -> Result<Cookie<'_, Self, QueryVersionReply>, ConnectionError>
    {
        query_version(self, major, minor)
    }
    fn xinerama_get_state(&self, window: xproto::Window) -> Result<Cookie<'_, Self, GetStateReply>, ConnectionError>
    {
        get_state(self, window)
    }
    fn xinerama_get_screen_count(&self, window: xproto::Window) -> Result<Cookie<'_, Self, GetScreenCountReply>, ConnectionError>
    {
        get_screen_count(self, window)
    }
    fn xinerama_get_screen_size(&self, window: xproto::Window, screen: u32) -> Result<Cookie<'_, Self, GetScreenSizeReply>, ConnectionError>
    {
        get_screen_size(self, window, screen)
    }
    fn xinerama_is_active(&self) -> Result<Cookie<'_, Self, IsActiveReply>, ConnectionError>
    {
        is_active(self)
    }
    fn xinerama_query_screens(&self) -> Result<Cookie<'_, Self, QueryScreensReply>, ConnectionError>
    {
        query_screens(self)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
