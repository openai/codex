// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `DRI2` X11 extension.

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

pub use x11rb_protocol::protocol::dri2::*;

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

pub fn connect<Conn>(conn: &Conn, window: xproto::Window, driver_type: DriverType) -> Result<Cookie<'_, Conn, ConnectReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ConnectRequest {
        window,
        driver_type,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn authenticate<Conn>(conn: &Conn, window: xproto::Window, magic: u32) -> Result<Cookie<'_, Conn, AuthenticateReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = AuthenticateRequest {
        window,
        magic,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn create_drawable<Conn>(conn: &Conn, drawable: xproto::Drawable) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateDrawableRequest {
        drawable,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn destroy_drawable<Conn>(conn: &Conn, drawable: xproto::Drawable) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DestroyDrawableRequest {
        drawable,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_buffers<'c, 'input, Conn>(conn: &'c Conn, drawable: xproto::Drawable, count: u32, attachments: &'input [u32]) -> Result<Cookie<'c, Conn, GetBuffersReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetBuffersRequest {
        drawable,
        count,
        attachments: Cow::Borrowed(attachments),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn copy_region<Conn>(conn: &Conn, drawable: xproto::Drawable, region: u32, dest: u32, src: u32) -> Result<Cookie<'_, Conn, CopyRegionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CopyRegionRequest {
        drawable,
        region,
        dest,
        src,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_buffers_with_format<'c, 'input, Conn>(conn: &'c Conn, drawable: xproto::Drawable, count: u32, attachments: &'input [AttachFormat]) -> Result<Cookie<'c, Conn, GetBuffersWithFormatReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetBuffersWithFormatRequest {
        drawable,
        count,
        attachments: Cow::Borrowed(attachments),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn swap_buffers<Conn>(conn: &Conn, drawable: xproto::Drawable, target_msc_hi: u32, target_msc_lo: u32, divisor_hi: u32, divisor_lo: u32, remainder_hi: u32, remainder_lo: u32) -> Result<Cookie<'_, Conn, SwapBuffersReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SwapBuffersRequest {
        drawable,
        target_msc_hi,
        target_msc_lo,
        divisor_hi,
        divisor_lo,
        remainder_hi,
        remainder_lo,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn get_msc<Conn>(conn: &Conn, drawable: xproto::Drawable) -> Result<Cookie<'_, Conn, GetMSCReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetMSCRequest {
        drawable,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn wait_msc<Conn>(conn: &Conn, drawable: xproto::Drawable, target_msc_hi: u32, target_msc_lo: u32, divisor_hi: u32, divisor_lo: u32, remainder_hi: u32, remainder_lo: u32) -> Result<Cookie<'_, Conn, WaitMSCReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = WaitMSCRequest {
        drawable,
        target_msc_hi,
        target_msc_lo,
        divisor_hi,
        divisor_lo,
        remainder_hi,
        remainder_lo,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn wait_sbc<Conn>(conn: &Conn, drawable: xproto::Drawable, target_sbc_hi: u32, target_sbc_lo: u32) -> Result<Cookie<'_, Conn, WaitSBCReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = WaitSBCRequest {
        drawable,
        target_sbc_hi,
        target_sbc_lo,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn swap_interval<Conn>(conn: &Conn, drawable: xproto::Drawable, interval: u32) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SwapIntervalRequest {
        drawable,
        interval,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_param<Conn>(conn: &Conn, drawable: xproto::Drawable, param: u32) -> Result<Cookie<'_, Conn, GetParamReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetParamRequest {
        drawable,
        param,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn dri2_query_version(&self, major_version: u32, minor_version: u32) -> Result<Cookie<'_, Self, QueryVersionReply>, ConnectionError>
    {
        query_version(self, major_version, minor_version)
    }
    fn dri2_connect(&self, window: xproto::Window, driver_type: DriverType) -> Result<Cookie<'_, Self, ConnectReply>, ConnectionError>
    {
        connect(self, window, driver_type)
    }
    fn dri2_authenticate(&self, window: xproto::Window, magic: u32) -> Result<Cookie<'_, Self, AuthenticateReply>, ConnectionError>
    {
        authenticate(self, window, magic)
    }
    fn dri2_create_drawable(&self, drawable: xproto::Drawable) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        create_drawable(self, drawable)
    }
    fn dri2_destroy_drawable(&self, drawable: xproto::Drawable) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        destroy_drawable(self, drawable)
    }
    fn dri2_get_buffers<'c, 'input>(&'c self, drawable: xproto::Drawable, count: u32, attachments: &'input [u32]) -> Result<Cookie<'c, Self, GetBuffersReply>, ConnectionError>
    {
        get_buffers(self, drawable, count, attachments)
    }
    fn dri2_copy_region(&self, drawable: xproto::Drawable, region: u32, dest: u32, src: u32) -> Result<Cookie<'_, Self, CopyRegionReply>, ConnectionError>
    {
        copy_region(self, drawable, region, dest, src)
    }
    fn dri2_get_buffers_with_format<'c, 'input>(&'c self, drawable: xproto::Drawable, count: u32, attachments: &'input [AttachFormat]) -> Result<Cookie<'c, Self, GetBuffersWithFormatReply>, ConnectionError>
    {
        get_buffers_with_format(self, drawable, count, attachments)
    }
    fn dri2_swap_buffers(&self, drawable: xproto::Drawable, target_msc_hi: u32, target_msc_lo: u32, divisor_hi: u32, divisor_lo: u32, remainder_hi: u32, remainder_lo: u32) -> Result<Cookie<'_, Self, SwapBuffersReply>, ConnectionError>
    {
        swap_buffers(self, drawable, target_msc_hi, target_msc_lo, divisor_hi, divisor_lo, remainder_hi, remainder_lo)
    }
    fn dri2_get_msc(&self, drawable: xproto::Drawable) -> Result<Cookie<'_, Self, GetMSCReply>, ConnectionError>
    {
        get_msc(self, drawable)
    }
    fn dri2_wait_msc(&self, drawable: xproto::Drawable, target_msc_hi: u32, target_msc_lo: u32, divisor_hi: u32, divisor_lo: u32, remainder_hi: u32, remainder_lo: u32) -> Result<Cookie<'_, Self, WaitMSCReply>, ConnectionError>
    {
        wait_msc(self, drawable, target_msc_hi, target_msc_lo, divisor_hi, divisor_lo, remainder_hi, remainder_lo)
    }
    fn dri2_wait_sbc(&self, drawable: xproto::Drawable, target_sbc_hi: u32, target_sbc_lo: u32) -> Result<Cookie<'_, Self, WaitSBCReply>, ConnectionError>
    {
        wait_sbc(self, drawable, target_sbc_hi, target_sbc_lo)
    }
    fn dri2_swap_interval(&self, drawable: xproto::Drawable, interval: u32) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        swap_interval(self, drawable, interval)
    }
    fn dri2_get_param(&self, drawable: xproto::Drawable, param: u32) -> Result<Cookie<'_, Self, GetParamReply>, ConnectionError>
    {
        get_param(self, drawable, param)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
