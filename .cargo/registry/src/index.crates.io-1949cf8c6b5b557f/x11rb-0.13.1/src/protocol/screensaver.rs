// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `ScreenSaver` X11 extension.

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

pub use x11rb_protocol::protocol::screensaver::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

pub fn query_version<Conn>(conn: &Conn, client_major_version: u8, client_minor_version: u8) -> Result<Cookie<'_, Conn, QueryVersionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryVersionRequest {
        client_major_version,
        client_minor_version,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_info<Conn>(conn: &Conn, drawable: xproto::Drawable) -> Result<Cookie<'_, Conn, QueryInfoReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryInfoRequest {
        drawable,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn select_input<Conn>(conn: &Conn, drawable: xproto::Drawable, event_mask: Event) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SelectInputRequest {
        drawable,
        event_mask,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn set_attributes<'c, 'input, Conn>(conn: &'c Conn, drawable: xproto::Drawable, x: i16, y: i16, width: u16, height: u16, border_width: u16, class: xproto::WindowClass, depth: u8, visual: xproto::Visualid, value_list: &'input SetAttributesAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetAttributesRequest {
        drawable,
        x,
        y,
        width,
        height,
        border_width,
        class,
        depth,
        visual,
        value_list: Cow::Borrowed(value_list),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn unset_attributes<Conn>(conn: &Conn, drawable: xproto::Drawable) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = UnsetAttributesRequest {
        drawable,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn suspend<Conn>(conn: &Conn, suspend: u32) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SuspendRequest {
        suspend,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn screensaver_query_version(&self, client_major_version: u8, client_minor_version: u8) -> Result<Cookie<'_, Self, QueryVersionReply>, ConnectionError>
    {
        query_version(self, client_major_version, client_minor_version)
    }
    fn screensaver_query_info(&self, drawable: xproto::Drawable) -> Result<Cookie<'_, Self, QueryInfoReply>, ConnectionError>
    {
        query_info(self, drawable)
    }
    fn screensaver_select_input(&self, drawable: xproto::Drawable, event_mask: Event) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        select_input(self, drawable, event_mask)
    }
    fn screensaver_set_attributes<'c, 'input>(&'c self, drawable: xproto::Drawable, x: i16, y: i16, width: u16, height: u16, border_width: u16, class: xproto::WindowClass, depth: u8, visual: xproto::Visualid, value_list: &'input SetAttributesAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        set_attributes(self, drawable, x, y, width, height, border_width, class, depth, visual, value_list)
    }
    fn screensaver_unset_attributes(&self, drawable: xproto::Drawable) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        unset_attributes(self, drawable)
    }
    fn screensaver_suspend(&self, suspend: u32) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        self::suspend(self, suspend)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
