// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `DRI3` X11 extension.

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

pub use x11rb_protocol::protocol::dri3::*;

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

pub fn open<Conn>(conn: &Conn, drawable: xproto::Drawable, provider: u32) -> Result<CookieWithFds<'_, Conn, OpenReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = OpenRequest {
        drawable,
        provider,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply_with_fds(&slices, fds)
}

pub fn pixmap_from_buffer<Conn, A>(conn: &Conn, pixmap: xproto::Pixmap, drawable: xproto::Drawable, size: u32, width: u16, height: u16, stride: u16, depth: u8, bpp: u8, pixmap_fd: A) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<RawFdContainer>,
{
    let pixmap_fd: RawFdContainer = pixmap_fd.into();
    let request0 = PixmapFromBufferRequest {
        pixmap,
        drawable,
        size,
        width,
        height,
        stride,
        depth,
        bpp,
        pixmap_fd,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn buffer_from_pixmap<Conn>(conn: &Conn, pixmap: xproto::Pixmap) -> Result<CookieWithFds<'_, Conn, BufferFromPixmapReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = BufferFromPixmapRequest {
        pixmap,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply_with_fds(&slices, fds)
}

pub fn fence_from_fd<Conn, A>(conn: &Conn, drawable: xproto::Drawable, fence: u32, initially_triggered: bool, fence_fd: A) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<RawFdContainer>,
{
    let fence_fd: RawFdContainer = fence_fd.into();
    let request0 = FenceFromFDRequest {
        drawable,
        fence,
        initially_triggered,
        fence_fd,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn fd_from_fence<Conn>(conn: &Conn, drawable: xproto::Drawable, fence: u32) -> Result<CookieWithFds<'_, Conn, FDFromFenceReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = FDFromFenceRequest {
        drawable,
        fence,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply_with_fds(&slices, fds)
}

pub fn get_supported_modifiers<Conn>(conn: &Conn, window: u32, depth: u8, bpp: u8) -> Result<Cookie<'_, Conn, GetSupportedModifiersReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetSupportedModifiersRequest {
        window,
        depth,
        bpp,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn pixmap_from_buffers<Conn>(conn: &Conn, pixmap: xproto::Pixmap, window: xproto::Window, width: u16, height: u16, stride0: u32, offset0: u32, stride1: u32, offset1: u32, stride2: u32, offset2: u32, stride3: u32, offset3: u32, depth: u8, bpp: u8, modifier: u64, buffers: Vec<RawFdContainer>) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PixmapFromBuffersRequest {
        pixmap,
        window,
        width,
        height,
        stride0,
        offset0,
        stride1,
        offset1,
        stride2,
        offset2,
        stride3,
        offset3,
        depth,
        bpp,
        modifier,
        buffers,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn buffers_from_pixmap<Conn>(conn: &Conn, pixmap: xproto::Pixmap) -> Result<CookieWithFds<'_, Conn, BuffersFromPixmapReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = BuffersFromPixmapRequest {
        pixmap,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply_with_fds(&slices, fds)
}

pub fn set_drm_device_in_use<Conn>(conn: &Conn, window: xproto::Window, drm_major: u32, drm_minor: u32) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetDRMDeviceInUseRequest {
        window,
        drm_major,
        drm_minor,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn import_syncobj<Conn, A>(conn: &Conn, syncobj: Syncobj, drawable: xproto::Drawable, syncobj_fd: A) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<RawFdContainer>,
{
    let syncobj_fd: RawFdContainer = syncobj_fd.into();
    let request0 = ImportSyncobjRequest {
        syncobj,
        drawable,
        syncobj_fd,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn free_syncobj<Conn>(conn: &Conn, syncobj: Syncobj) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = FreeSyncobjRequest {
        syncobj,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn dri3_query_version(&self, major_version: u32, minor_version: u32) -> Result<Cookie<'_, Self, QueryVersionReply>, ConnectionError>
    {
        query_version(self, major_version, minor_version)
    }
    fn dri3_open(&self, drawable: xproto::Drawable, provider: u32) -> Result<CookieWithFds<'_, Self, OpenReply>, ConnectionError>
    {
        open(self, drawable, provider)
    }
    fn dri3_pixmap_from_buffer<A>(&self, pixmap: xproto::Pixmap, drawable: xproto::Drawable, size: u32, width: u16, height: u16, stride: u16, depth: u8, bpp: u8, pixmap_fd: A) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<RawFdContainer>,
    {
        pixmap_from_buffer(self, pixmap, drawable, size, width, height, stride, depth, bpp, pixmap_fd)
    }
    fn dri3_buffer_from_pixmap(&self, pixmap: xproto::Pixmap) -> Result<CookieWithFds<'_, Self, BufferFromPixmapReply>, ConnectionError>
    {
        buffer_from_pixmap(self, pixmap)
    }
    fn dri3_fence_from_fd<A>(&self, drawable: xproto::Drawable, fence: u32, initially_triggered: bool, fence_fd: A) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<RawFdContainer>,
    {
        fence_from_fd(self, drawable, fence, initially_triggered, fence_fd)
    }
    fn dri3_fd_from_fence(&self, drawable: xproto::Drawable, fence: u32) -> Result<CookieWithFds<'_, Self, FDFromFenceReply>, ConnectionError>
    {
        fd_from_fence(self, drawable, fence)
    }
    fn dri3_get_supported_modifiers(&self, window: u32, depth: u8, bpp: u8) -> Result<Cookie<'_, Self, GetSupportedModifiersReply>, ConnectionError>
    {
        get_supported_modifiers(self, window, depth, bpp)
    }
    fn dri3_pixmap_from_buffers(&self, pixmap: xproto::Pixmap, window: xproto::Window, width: u16, height: u16, stride0: u32, offset0: u32, stride1: u32, offset1: u32, stride2: u32, offset2: u32, stride3: u32, offset3: u32, depth: u8, bpp: u8, modifier: u64, buffers: Vec<RawFdContainer>) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        pixmap_from_buffers(self, pixmap, window, width, height, stride0, offset0, stride1, offset1, stride2, offset2, stride3, offset3, depth, bpp, modifier, buffers)
    }
    fn dri3_buffers_from_pixmap(&self, pixmap: xproto::Pixmap) -> Result<CookieWithFds<'_, Self, BuffersFromPixmapReply>, ConnectionError>
    {
        buffers_from_pixmap(self, pixmap)
    }
    fn dri3_set_drm_device_in_use(&self, window: xproto::Window, drm_major: u32, drm_minor: u32) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        set_drm_device_in_use(self, window, drm_major, drm_minor)
    }
    fn dri3_import_syncobj<A>(&self, syncobj: Syncobj, drawable: xproto::Drawable, syncobj_fd: A) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<RawFdContainer>,
    {
        import_syncobj(self, syncobj, drawable, syncobj_fd)
    }
    fn dri3_free_syncobj(&self, syncobj: Syncobj) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        free_syncobj(self, syncobj)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
