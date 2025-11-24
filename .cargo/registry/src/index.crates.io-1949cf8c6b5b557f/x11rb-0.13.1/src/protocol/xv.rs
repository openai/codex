// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Xv` X11 extension.

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
use super::shm;
#[allow(unused_imports)]
use super::xproto;

pub use x11rb_protocol::protocol::xv::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

pub fn query_extension<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, QueryExtensionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryExtensionRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_adaptors<Conn>(conn: &Conn, window: xproto::Window) -> Result<Cookie<'_, Conn, QueryAdaptorsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryAdaptorsRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_encodings<Conn>(conn: &Conn, port: Port) -> Result<Cookie<'_, Conn, QueryEncodingsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryEncodingsRequest {
        port,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn grab_port<Conn, A>(conn: &Conn, port: Port, time: A) -> Result<Cookie<'_, Conn, GrabPortReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<xproto::Timestamp>,
{
    let time: xproto::Timestamp = time.into();
    let request0 = GrabPortRequest {
        port,
        time,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn ungrab_port<Conn, A>(conn: &Conn, port: Port, time: A) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<xproto::Timestamp>,
{
    let time: xproto::Timestamp = time.into();
    let request0 = UngrabPortRequest {
        port,
        time,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn put_video<Conn>(conn: &Conn, port: Port, drawable: xproto::Drawable, gc: xproto::Gcontext, vid_x: i16, vid_y: i16, vid_w: u16, vid_h: u16, drw_x: i16, drw_y: i16, drw_w: u16, drw_h: u16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PutVideoRequest {
        port,
        drawable,
        gc,
        vid_x,
        vid_y,
        vid_w,
        vid_h,
        drw_x,
        drw_y,
        drw_w,
        drw_h,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn put_still<Conn>(conn: &Conn, port: Port, drawable: xproto::Drawable, gc: xproto::Gcontext, vid_x: i16, vid_y: i16, vid_w: u16, vid_h: u16, drw_x: i16, drw_y: i16, drw_w: u16, drw_h: u16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PutStillRequest {
        port,
        drawable,
        gc,
        vid_x,
        vid_y,
        vid_w,
        vid_h,
        drw_x,
        drw_y,
        drw_w,
        drw_h,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_video<Conn>(conn: &Conn, port: Port, drawable: xproto::Drawable, gc: xproto::Gcontext, vid_x: i16, vid_y: i16, vid_w: u16, vid_h: u16, drw_x: i16, drw_y: i16, drw_w: u16, drw_h: u16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetVideoRequest {
        port,
        drawable,
        gc,
        vid_x,
        vid_y,
        vid_w,
        vid_h,
        drw_x,
        drw_y,
        drw_w,
        drw_h,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_still<Conn>(conn: &Conn, port: Port, drawable: xproto::Drawable, gc: xproto::Gcontext, vid_x: i16, vid_y: i16, vid_w: u16, vid_h: u16, drw_x: i16, drw_y: i16, drw_w: u16, drw_h: u16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetStillRequest {
        port,
        drawable,
        gc,
        vid_x,
        vid_y,
        vid_w,
        vid_h,
        drw_x,
        drw_y,
        drw_w,
        drw_h,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn stop_video<Conn>(conn: &Conn, port: Port, drawable: xproto::Drawable) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = StopVideoRequest {
        port,
        drawable,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn select_video_notify<Conn>(conn: &Conn, drawable: xproto::Drawable, onoff: bool) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SelectVideoNotifyRequest {
        drawable,
        onoff,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn select_port_notify<Conn>(conn: &Conn, port: Port, onoff: bool) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SelectPortNotifyRequest {
        port,
        onoff,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn query_best_size<Conn>(conn: &Conn, port: Port, vid_w: u16, vid_h: u16, drw_w: u16, drw_h: u16, motion: bool) -> Result<Cookie<'_, Conn, QueryBestSizeReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryBestSizeRequest {
        port,
        vid_w,
        vid_h,
        drw_w,
        drw_h,
        motion,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_port_attribute<Conn>(conn: &Conn, port: Port, attribute: xproto::Atom, value: i32) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetPortAttributeRequest {
        port,
        attribute,
        value,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_port_attribute<Conn>(conn: &Conn, port: Port, attribute: xproto::Atom) -> Result<Cookie<'_, Conn, GetPortAttributeReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetPortAttributeRequest {
        port,
        attribute,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_port_attributes<Conn>(conn: &Conn, port: Port) -> Result<Cookie<'_, Conn, QueryPortAttributesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryPortAttributesRequest {
        port,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn list_image_formats<Conn>(conn: &Conn, port: Port) -> Result<Cookie<'_, Conn, ListImageFormatsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ListImageFormatsRequest {
        port,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_image_attributes<Conn>(conn: &Conn, port: Port, id: u32, width: u16, height: u16) -> Result<Cookie<'_, Conn, QueryImageAttributesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryImageAttributesRequest {
        port,
        id,
        width,
        height,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn put_image<'c, 'input, Conn>(conn: &'c Conn, port: Port, drawable: xproto::Drawable, gc: xproto::Gcontext, id: u32, src_x: i16, src_y: i16, src_w: u16, src_h: u16, drw_x: i16, drw_y: i16, drw_w: u16, drw_h: u16, width: u16, height: u16, data: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PutImageRequest {
        port,
        drawable,
        gc,
        id,
        src_x,
        src_y,
        src_w,
        src_h,
        drw_x,
        drw_y,
        drw_w,
        drw_h,
        width,
        height,
        data: Cow::Borrowed(data),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn shm_put_image<Conn>(conn: &Conn, port: Port, drawable: xproto::Drawable, gc: xproto::Gcontext, shmseg: shm::Seg, id: u32, offset: u32, src_x: i16, src_y: i16, src_w: u16, src_h: u16, drw_x: i16, drw_y: i16, drw_w: u16, drw_h: u16, width: u16, height: u16, send_event: u8) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ShmPutImageRequest {
        port,
        drawable,
        gc,
        shmseg,
        id,
        offset,
        src_x,
        src_y,
        src_w,
        src_h,
        drw_x,
        drw_y,
        drw_w,
        drw_h,
        width,
        height,
        send_event,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn xv_query_extension(&self) -> Result<Cookie<'_, Self, QueryExtensionReply>, ConnectionError>
    {
        query_extension(self)
    }
    fn xv_query_adaptors(&self, window: xproto::Window) -> Result<Cookie<'_, Self, QueryAdaptorsReply>, ConnectionError>
    {
        query_adaptors(self, window)
    }
    fn xv_query_encodings(&self, port: Port) -> Result<Cookie<'_, Self, QueryEncodingsReply>, ConnectionError>
    {
        query_encodings(self, port)
    }
    fn xv_grab_port<A>(&self, port: Port, time: A) -> Result<Cookie<'_, Self, GrabPortReply>, ConnectionError>
    where
        A: Into<xproto::Timestamp>,
    {
        grab_port(self, port, time)
    }
    fn xv_ungrab_port<A>(&self, port: Port, time: A) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<xproto::Timestamp>,
    {
        ungrab_port(self, port, time)
    }
    fn xv_put_video(&self, port: Port, drawable: xproto::Drawable, gc: xproto::Gcontext, vid_x: i16, vid_y: i16, vid_w: u16, vid_h: u16, drw_x: i16, drw_y: i16, drw_w: u16, drw_h: u16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        put_video(self, port, drawable, gc, vid_x, vid_y, vid_w, vid_h, drw_x, drw_y, drw_w, drw_h)
    }
    fn xv_put_still(&self, port: Port, drawable: xproto::Drawable, gc: xproto::Gcontext, vid_x: i16, vid_y: i16, vid_w: u16, vid_h: u16, drw_x: i16, drw_y: i16, drw_w: u16, drw_h: u16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        put_still(self, port, drawable, gc, vid_x, vid_y, vid_w, vid_h, drw_x, drw_y, drw_w, drw_h)
    }
    fn xv_get_video(&self, port: Port, drawable: xproto::Drawable, gc: xproto::Gcontext, vid_x: i16, vid_y: i16, vid_w: u16, vid_h: u16, drw_x: i16, drw_y: i16, drw_w: u16, drw_h: u16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        get_video(self, port, drawable, gc, vid_x, vid_y, vid_w, vid_h, drw_x, drw_y, drw_w, drw_h)
    }
    fn xv_get_still(&self, port: Port, drawable: xproto::Drawable, gc: xproto::Gcontext, vid_x: i16, vid_y: i16, vid_w: u16, vid_h: u16, drw_x: i16, drw_y: i16, drw_w: u16, drw_h: u16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        get_still(self, port, drawable, gc, vid_x, vid_y, vid_w, vid_h, drw_x, drw_y, drw_w, drw_h)
    }
    fn xv_stop_video(&self, port: Port, drawable: xproto::Drawable) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        stop_video(self, port, drawable)
    }
    fn xv_select_video_notify(&self, drawable: xproto::Drawable, onoff: bool) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        select_video_notify(self, drawable, onoff)
    }
    fn xv_select_port_notify(&self, port: Port, onoff: bool) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        select_port_notify(self, port, onoff)
    }
    fn xv_query_best_size(&self, port: Port, vid_w: u16, vid_h: u16, drw_w: u16, drw_h: u16, motion: bool) -> Result<Cookie<'_, Self, QueryBestSizeReply>, ConnectionError>
    {
        query_best_size(self, port, vid_w, vid_h, drw_w, drw_h, motion)
    }
    fn xv_set_port_attribute(&self, port: Port, attribute: xproto::Atom, value: i32) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        set_port_attribute(self, port, attribute, value)
    }
    fn xv_get_port_attribute(&self, port: Port, attribute: xproto::Atom) -> Result<Cookie<'_, Self, GetPortAttributeReply>, ConnectionError>
    {
        get_port_attribute(self, port, attribute)
    }
    fn xv_query_port_attributes(&self, port: Port) -> Result<Cookie<'_, Self, QueryPortAttributesReply>, ConnectionError>
    {
        query_port_attributes(self, port)
    }
    fn xv_list_image_formats(&self, port: Port) -> Result<Cookie<'_, Self, ListImageFormatsReply>, ConnectionError>
    {
        list_image_formats(self, port)
    }
    fn xv_query_image_attributes(&self, port: Port, id: u32, width: u16, height: u16) -> Result<Cookie<'_, Self, QueryImageAttributesReply>, ConnectionError>
    {
        query_image_attributes(self, port, id, width, height)
    }
    fn xv_put_image<'c, 'input>(&'c self, port: Port, drawable: xproto::Drawable, gc: xproto::Gcontext, id: u32, src_x: i16, src_y: i16, src_w: u16, src_h: u16, drw_x: i16, drw_y: i16, drw_w: u16, drw_h: u16, width: u16, height: u16, data: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        put_image(self, port, drawable, gc, id, src_x, src_y, src_w, src_h, drw_x, drw_y, drw_w, drw_h, width, height, data)
    }
    fn xv_shm_put_image(&self, port: Port, drawable: xproto::Drawable, gc: xproto::Gcontext, shmseg: shm::Seg, id: u32, offset: u32, src_x: i16, src_y: i16, src_w: u16, src_h: u16, drw_x: i16, drw_y: i16, drw_w: u16, drw_h: u16, width: u16, height: u16, send_event: u8) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        shm_put_image(self, port, drawable, gc, shmseg, id, offset, src_x, src_y, src_w, src_h, drw_x, drw_y, drw_w, drw_h, width, height, send_event)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
