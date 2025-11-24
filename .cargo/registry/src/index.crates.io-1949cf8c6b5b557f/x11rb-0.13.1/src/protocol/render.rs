// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Render` X11 extension.

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

pub use x11rb_protocol::protocol::render::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

pub fn query_version<Conn>(conn: &Conn, client_major_version: u32, client_minor_version: u32) -> Result<Cookie<'_, Conn, QueryVersionReply>, ConnectionError>
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

pub fn query_pict_formats<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, QueryPictFormatsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryPictFormatsRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_pict_index_values<Conn>(conn: &Conn, format: Pictformat) -> Result<Cookie<'_, Conn, QueryPictIndexValuesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryPictIndexValuesRequest {
        format,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn create_picture<'c, 'input, Conn>(conn: &'c Conn, pid: Picture, drawable: xproto::Drawable, format: Pictformat, value_list: &'input CreatePictureAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreatePictureRequest {
        pid,
        drawable,
        format,
        value_list: Cow::Borrowed(value_list),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn change_picture<'c, 'input, Conn>(conn: &'c Conn, picture: Picture, value_list: &'input ChangePictureAux) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ChangePictureRequest {
        picture,
        value_list: Cow::Borrowed(value_list),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn set_picture_clip_rectangles<'c, 'input, Conn>(conn: &'c Conn, picture: Picture, clip_x_origin: i16, clip_y_origin: i16, rectangles: &'input [xproto::Rectangle]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetPictureClipRectanglesRequest {
        picture,
        clip_x_origin,
        clip_y_origin,
        rectangles: Cow::Borrowed(rectangles),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn free_picture<Conn>(conn: &Conn, picture: Picture) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = FreePictureRequest {
        picture,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn composite<Conn, A>(conn: &Conn, op: PictOp, src: Picture, mask: A, dst: Picture, src_x: i16, src_y: i16, mask_x: i16, mask_y: i16, dst_x: i16, dst_y: i16, width: u16, height: u16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<Picture>,
{
    let mask: Picture = mask.into();
    let request0 = CompositeRequest {
        op,
        src,
        mask,
        dst,
        src_x,
        src_y,
        mask_x,
        mask_y,
        dst_x,
        dst_y,
        width,
        height,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn trapezoids<'c, 'input, Conn>(conn: &'c Conn, op: PictOp, src: Picture, dst: Picture, mask_format: Pictformat, src_x: i16, src_y: i16, traps: &'input [Trapezoid]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = TrapezoidsRequest {
        op,
        src,
        dst,
        mask_format,
        src_x,
        src_y,
        traps: Cow::Borrowed(traps),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn triangles<'c, 'input, Conn>(conn: &'c Conn, op: PictOp, src: Picture, dst: Picture, mask_format: Pictformat, src_x: i16, src_y: i16, triangles: &'input [Triangle]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = TrianglesRequest {
        op,
        src,
        dst,
        mask_format,
        src_x,
        src_y,
        triangles: Cow::Borrowed(triangles),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn tri_strip<'c, 'input, Conn>(conn: &'c Conn, op: PictOp, src: Picture, dst: Picture, mask_format: Pictformat, src_x: i16, src_y: i16, points: &'input [Pointfix]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = TriStripRequest {
        op,
        src,
        dst,
        mask_format,
        src_x,
        src_y,
        points: Cow::Borrowed(points),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn tri_fan<'c, 'input, Conn>(conn: &'c Conn, op: PictOp, src: Picture, dst: Picture, mask_format: Pictformat, src_x: i16, src_y: i16, points: &'input [Pointfix]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = TriFanRequest {
        op,
        src,
        dst,
        mask_format,
        src_x,
        src_y,
        points: Cow::Borrowed(points),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn create_glyph_set<Conn>(conn: &Conn, gsid: Glyphset, format: Pictformat) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateGlyphSetRequest {
        gsid,
        format,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn reference_glyph_set<Conn>(conn: &Conn, gsid: Glyphset, existing: Glyphset) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ReferenceGlyphSetRequest {
        gsid,
        existing,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn free_glyph_set<Conn>(conn: &Conn, glyphset: Glyphset) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = FreeGlyphSetRequest {
        glyphset,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn add_glyphs<'c, 'input, Conn>(conn: &'c Conn, glyphset: Glyphset, glyphids: &'input [u32], glyphs: &'input [Glyphinfo], data: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = AddGlyphsRequest {
        glyphset,
        glyphids: Cow::Borrowed(glyphids),
        glyphs: Cow::Borrowed(glyphs),
        data: Cow::Borrowed(data),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3]), IoSlice::new(&bytes[4])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn free_glyphs<'c, 'input, Conn>(conn: &'c Conn, glyphset: Glyphset, glyphs: &'input [Glyph]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = FreeGlyphsRequest {
        glyphset,
        glyphs: Cow::Borrowed(glyphs),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn composite_glyphs8<'c, 'input, Conn>(conn: &'c Conn, op: PictOp, src: Picture, dst: Picture, mask_format: Pictformat, glyphset: Glyphset, src_x: i16, src_y: i16, glyphcmds: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CompositeGlyphs8Request {
        op,
        src,
        dst,
        mask_format,
        glyphset,
        src_x,
        src_y,
        glyphcmds: Cow::Borrowed(glyphcmds),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn composite_glyphs16<'c, 'input, Conn>(conn: &'c Conn, op: PictOp, src: Picture, dst: Picture, mask_format: Pictformat, glyphset: Glyphset, src_x: i16, src_y: i16, glyphcmds: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CompositeGlyphs16Request {
        op,
        src,
        dst,
        mask_format,
        glyphset,
        src_x,
        src_y,
        glyphcmds: Cow::Borrowed(glyphcmds),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn composite_glyphs32<'c, 'input, Conn>(conn: &'c Conn, op: PictOp, src: Picture, dst: Picture, mask_format: Pictformat, glyphset: Glyphset, src_x: i16, src_y: i16, glyphcmds: &'input [u8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CompositeGlyphs32Request {
        op,
        src,
        dst,
        mask_format,
        glyphset,
        src_x,
        src_y,
        glyphcmds: Cow::Borrowed(glyphcmds),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn fill_rectangles<'c, 'input, Conn>(conn: &'c Conn, op: PictOp, dst: Picture, color: Color, rects: &'input [xproto::Rectangle]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = FillRectanglesRequest {
        op,
        dst,
        color,
        rects: Cow::Borrowed(rects),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn create_cursor<Conn>(conn: &Conn, cid: xproto::Cursor, source: Picture, x: u16, y: u16) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateCursorRequest {
        cid,
        source,
        x,
        y,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn set_picture_transform<Conn>(conn: &Conn, picture: Picture, transform: Transform) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetPictureTransformRequest {
        picture,
        transform,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn query_filters<Conn>(conn: &Conn, drawable: xproto::Drawable) -> Result<Cookie<'_, Conn, QueryFiltersReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryFiltersRequest {
        drawable,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn set_picture_filter<'c, 'input, Conn>(conn: &'c Conn, picture: Picture, filter: &'input [u8], values: &'input [Fixed]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SetPictureFilterRequest {
        picture,
        filter: Cow::Borrowed(filter),
        values: Cow::Borrowed(values),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3]), IoSlice::new(&bytes[4])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn create_anim_cursor<'c, 'input, Conn>(conn: &'c Conn, cid: xproto::Cursor, cursors: &'input [Animcursorelt]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateAnimCursorRequest {
        cid,
        cursors: Cow::Borrowed(cursors),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn add_traps<'c, 'input, Conn>(conn: &'c Conn, picture: Picture, x_off: i16, y_off: i16, traps: &'input [Trap]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = AddTrapsRequest {
        picture,
        x_off,
        y_off,
        traps: Cow::Borrowed(traps),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn create_solid_fill<Conn>(conn: &Conn, picture: Picture, color: Color) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateSolidFillRequest {
        picture,
        color,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn create_linear_gradient<'c, 'input, Conn>(conn: &'c Conn, picture: Picture, p1: Pointfix, p2: Pointfix, stops: &'input [Fixed], colors: &'input [Color]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateLinearGradientRequest {
        picture,
        p1,
        p2,
        stops: Cow::Borrowed(stops),
        colors: Cow::Borrowed(colors),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn create_radial_gradient<'c, 'input, Conn>(conn: &'c Conn, picture: Picture, inner: Pointfix, outer: Pointfix, inner_radius: Fixed, outer_radius: Fixed, stops: &'input [Fixed], colors: &'input [Color]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateRadialGradientRequest {
        picture,
        inner,
        outer,
        inner_radius,
        outer_radius,
        stops: Cow::Borrowed(stops),
        colors: Cow::Borrowed(colors),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn create_conical_gradient<'c, 'input, Conn>(conn: &'c Conn, picture: Picture, center: Pointfix, angle: Fixed, stops: &'input [Fixed], colors: &'input [Color]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateConicalGradientRequest {
        picture,
        center,
        angle,
        stops: Cow::Borrowed(stops),
        colors: Cow::Borrowed(colors),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn render_query_version(&self, client_major_version: u32, client_minor_version: u32) -> Result<Cookie<'_, Self, QueryVersionReply>, ConnectionError>
    {
        query_version(self, client_major_version, client_minor_version)
    }
    fn render_query_pict_formats(&self) -> Result<Cookie<'_, Self, QueryPictFormatsReply>, ConnectionError>
    {
        query_pict_formats(self)
    }
    fn render_query_pict_index_values(&self, format: Pictformat) -> Result<Cookie<'_, Self, QueryPictIndexValuesReply>, ConnectionError>
    {
        query_pict_index_values(self, format)
    }
    fn render_create_picture<'c, 'input>(&'c self, pid: Picture, drawable: xproto::Drawable, format: Pictformat, value_list: &'input CreatePictureAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        create_picture(self, pid, drawable, format, value_list)
    }
    fn render_change_picture<'c, 'input>(&'c self, picture: Picture, value_list: &'input ChangePictureAux) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        change_picture(self, picture, value_list)
    }
    fn render_set_picture_clip_rectangles<'c, 'input>(&'c self, picture: Picture, clip_x_origin: i16, clip_y_origin: i16, rectangles: &'input [xproto::Rectangle]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        set_picture_clip_rectangles(self, picture, clip_x_origin, clip_y_origin, rectangles)
    }
    fn render_free_picture(&self, picture: Picture) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        free_picture(self, picture)
    }
    fn render_composite<A>(&self, op: PictOp, src: Picture, mask: A, dst: Picture, src_x: i16, src_y: i16, mask_x: i16, mask_y: i16, dst_x: i16, dst_y: i16, width: u16, height: u16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Picture>,
    {
        composite(self, op, src, mask, dst, src_x, src_y, mask_x, mask_y, dst_x, dst_y, width, height)
    }
    fn render_trapezoids<'c, 'input>(&'c self, op: PictOp, src: Picture, dst: Picture, mask_format: Pictformat, src_x: i16, src_y: i16, traps: &'input [Trapezoid]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        trapezoids(self, op, src, dst, mask_format, src_x, src_y, traps)
    }
    fn render_triangles<'c, 'input>(&'c self, op: PictOp, src: Picture, dst: Picture, mask_format: Pictformat, src_x: i16, src_y: i16, triangles: &'input [Triangle]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        self::triangles(self, op, src, dst, mask_format, src_x, src_y, triangles)
    }
    fn render_tri_strip<'c, 'input>(&'c self, op: PictOp, src: Picture, dst: Picture, mask_format: Pictformat, src_x: i16, src_y: i16, points: &'input [Pointfix]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        tri_strip(self, op, src, dst, mask_format, src_x, src_y, points)
    }
    fn render_tri_fan<'c, 'input>(&'c self, op: PictOp, src: Picture, dst: Picture, mask_format: Pictformat, src_x: i16, src_y: i16, points: &'input [Pointfix]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        tri_fan(self, op, src, dst, mask_format, src_x, src_y, points)
    }
    fn render_create_glyph_set(&self, gsid: Glyphset, format: Pictformat) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        create_glyph_set(self, gsid, format)
    }
    fn render_reference_glyph_set(&self, gsid: Glyphset, existing: Glyphset) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        reference_glyph_set(self, gsid, existing)
    }
    fn render_free_glyph_set(&self, glyphset: Glyphset) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        free_glyph_set(self, glyphset)
    }
    fn render_add_glyphs<'c, 'input>(&'c self, glyphset: Glyphset, glyphids: &'input [u32], glyphs: &'input [Glyphinfo], data: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        add_glyphs(self, glyphset, glyphids, glyphs, data)
    }
    fn render_free_glyphs<'c, 'input>(&'c self, glyphset: Glyphset, glyphs: &'input [Glyph]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        free_glyphs(self, glyphset, glyphs)
    }
    fn render_composite_glyphs8<'c, 'input>(&'c self, op: PictOp, src: Picture, dst: Picture, mask_format: Pictformat, glyphset: Glyphset, src_x: i16, src_y: i16, glyphcmds: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        composite_glyphs8(self, op, src, dst, mask_format, glyphset, src_x, src_y, glyphcmds)
    }
    fn render_composite_glyphs16<'c, 'input>(&'c self, op: PictOp, src: Picture, dst: Picture, mask_format: Pictformat, glyphset: Glyphset, src_x: i16, src_y: i16, glyphcmds: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        composite_glyphs16(self, op, src, dst, mask_format, glyphset, src_x, src_y, glyphcmds)
    }
    fn render_composite_glyphs32<'c, 'input>(&'c self, op: PictOp, src: Picture, dst: Picture, mask_format: Pictformat, glyphset: Glyphset, src_x: i16, src_y: i16, glyphcmds: &'input [u8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        composite_glyphs32(self, op, src, dst, mask_format, glyphset, src_x, src_y, glyphcmds)
    }
    fn render_fill_rectangles<'c, 'input>(&'c self, op: PictOp, dst: Picture, color: Color, rects: &'input [xproto::Rectangle]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        fill_rectangles(self, op, dst, color, rects)
    }
    fn render_create_cursor(&self, cid: xproto::Cursor, source: Picture, x: u16, y: u16) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        create_cursor(self, cid, source, x, y)
    }
    fn render_set_picture_transform(&self, picture: Picture, transform: Transform) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        set_picture_transform(self, picture, transform)
    }
    fn render_query_filters(&self, drawable: xproto::Drawable) -> Result<Cookie<'_, Self, QueryFiltersReply>, ConnectionError>
    {
        query_filters(self, drawable)
    }
    fn render_set_picture_filter<'c, 'input>(&'c self, picture: Picture, filter: &'input [u8], values: &'input [Fixed]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        set_picture_filter(self, picture, filter, values)
    }
    fn render_create_anim_cursor<'c, 'input>(&'c self, cid: xproto::Cursor, cursors: &'input [Animcursorelt]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        create_anim_cursor(self, cid, cursors)
    }
    fn render_add_traps<'c, 'input>(&'c self, picture: Picture, x_off: i16, y_off: i16, traps: &'input [Trap]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        add_traps(self, picture, x_off, y_off, traps)
    }
    fn render_create_solid_fill(&self, picture: Picture, color: Color) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        create_solid_fill(self, picture, color)
    }
    fn render_create_linear_gradient<'c, 'input>(&'c self, picture: Picture, p1: Pointfix, p2: Pointfix, stops: &'input [Fixed], colors: &'input [Color]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        create_linear_gradient(self, picture, p1, p2, stops, colors)
    }
    fn render_create_radial_gradient<'c, 'input>(&'c self, picture: Picture, inner: Pointfix, outer: Pointfix, inner_radius: Fixed, outer_radius: Fixed, stops: &'input [Fixed], colors: &'input [Color]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        create_radial_gradient(self, picture, inner, outer, inner_radius, outer_radius, stops, colors)
    }
    fn render_create_conical_gradient<'c, 'input>(&'c self, picture: Picture, center: Pointfix, angle: Fixed, stops: &'input [Fixed], colors: &'input [Color]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        create_conical_gradient(self, picture, center, angle, stops, colors)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}

/// A RAII-like wrapper around a [Picture].
///
/// Instances of this struct represent a Picture that is freed in `Drop`.
///
/// Any errors during `Drop` are silently ignored. Most likely an error here means that your
/// X11 connection is broken and later requests will also fail.
#[derive(Debug)]
pub struct PictureWrapper<C: RequestConnection>(C, Picture);

impl<C: RequestConnection> PictureWrapper<C>
{
    /// Assume ownership of the given resource and destroy it in `Drop`.
    pub fn for_picture(conn: C, id: Picture) -> Self {
        PictureWrapper(conn, id)
    }

    /// Get the XID of the wrapped resource
    pub fn picture(&self) -> Picture {
        self.1
    }

    /// Assume ownership of the XID of the wrapped resource
    ///
    /// This function destroys this wrapper without freeing the underlying resource.
    pub fn into_picture(self) -> Picture {
        let id = self.1;
        std::mem::forget(self);
        id
    }
}

impl<'c, C: X11Connection> PictureWrapper<&'c C>
{
    /// Create a new Picture and return a Picture wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_picture] that allocates an id for the Picture.
    /// This function returns the resulting `PictureWrapper` that owns the created Picture and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_picture].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_picture].
    pub fn create_picture_and_get_cookie(conn: &'c C, drawable: xproto::Drawable, format: Pictformat, value_list: &CreatePictureAux) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let pid = conn.generate_id()?;
        let cookie = create_picture(conn, pid, drawable, format, value_list)?;
        Ok((Self::for_picture(conn, pid), cookie))
    }
}
impl<C: X11Connection> PictureWrapper<C>
{
    /// Create a new Picture and return a Picture wrapper
    ///
    /// This is a thin wrapper around [create_picture] that allocates an id for the Picture.
    /// This function returns the resulting `PictureWrapper` that owns the created Picture and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_picture].
    pub fn create_picture(conn: C, drawable: xproto::Drawable, format: Pictformat, value_list: &CreatePictureAux) -> Result<Self, ReplyOrIdError>
    {
        let pid = conn.generate_id()?;
        let _ = create_picture(&conn, pid, drawable, format, value_list)?;
        Ok(Self::for_picture(conn, pid))
    }
}

impl<'c, C: X11Connection> PictureWrapper<&'c C>
{
    /// Create a new Picture and return a Picture wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_solid_fill] that allocates an id for the Picture.
    /// This function returns the resulting `PictureWrapper` that owns the created Picture and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_solid_fill].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_solid_fill].
    pub fn create_solid_fill_and_get_cookie(conn: &'c C, color: Color) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let picture = conn.generate_id()?;
        let cookie = create_solid_fill(conn, picture, color)?;
        Ok((Self::for_picture(conn, picture), cookie))
    }
}
impl<C: X11Connection> PictureWrapper<C>
{
    /// Create a new Picture and return a Picture wrapper
    ///
    /// This is a thin wrapper around [create_solid_fill] that allocates an id for the Picture.
    /// This function returns the resulting `PictureWrapper` that owns the created Picture and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_solid_fill].
    pub fn create_solid_fill(conn: C, color: Color) -> Result<Self, ReplyOrIdError>
    {
        let picture = conn.generate_id()?;
        let _ = create_solid_fill(&conn, picture, color)?;
        Ok(Self::for_picture(conn, picture))
    }
}

impl<'c, C: X11Connection> PictureWrapper<&'c C>
{
    /// Create a new Picture and return a Picture wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_linear_gradient] that allocates an id for the Picture.
    /// This function returns the resulting `PictureWrapper` that owns the created Picture and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_linear_gradient].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_linear_gradient].
    pub fn create_linear_gradient_and_get_cookie(conn: &'c C, p1: Pointfix, p2: Pointfix, stops: &[Fixed], colors: &[Color]) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let picture = conn.generate_id()?;
        let cookie = create_linear_gradient(conn, picture, p1, p2, stops, colors)?;
        Ok((Self::for_picture(conn, picture), cookie))
    }
}
impl<C: X11Connection> PictureWrapper<C>
{
    /// Create a new Picture and return a Picture wrapper
    ///
    /// This is a thin wrapper around [create_linear_gradient] that allocates an id for the Picture.
    /// This function returns the resulting `PictureWrapper` that owns the created Picture and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_linear_gradient].
    pub fn create_linear_gradient(conn: C, p1: Pointfix, p2: Pointfix, stops: &[Fixed], colors: &[Color]) -> Result<Self, ReplyOrIdError>
    {
        let picture = conn.generate_id()?;
        let _ = create_linear_gradient(&conn, picture, p1, p2, stops, colors)?;
        Ok(Self::for_picture(conn, picture))
    }
}

impl<'c, C: X11Connection> PictureWrapper<&'c C>
{
    /// Create a new Picture and return a Picture wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_radial_gradient] that allocates an id for the Picture.
    /// This function returns the resulting `PictureWrapper` that owns the created Picture and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_radial_gradient].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_radial_gradient].
    pub fn create_radial_gradient_and_get_cookie(conn: &'c C, inner: Pointfix, outer: Pointfix, inner_radius: Fixed, outer_radius: Fixed, stops: &[Fixed], colors: &[Color]) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let picture = conn.generate_id()?;
        let cookie = create_radial_gradient(conn, picture, inner, outer, inner_radius, outer_radius, stops, colors)?;
        Ok((Self::for_picture(conn, picture), cookie))
    }
}
impl<C: X11Connection> PictureWrapper<C>
{
    /// Create a new Picture and return a Picture wrapper
    ///
    /// This is a thin wrapper around [create_radial_gradient] that allocates an id for the Picture.
    /// This function returns the resulting `PictureWrapper` that owns the created Picture and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_radial_gradient].
    pub fn create_radial_gradient(conn: C, inner: Pointfix, outer: Pointfix, inner_radius: Fixed, outer_radius: Fixed, stops: &[Fixed], colors: &[Color]) -> Result<Self, ReplyOrIdError>
    {
        let picture = conn.generate_id()?;
        let _ = create_radial_gradient(&conn, picture, inner, outer, inner_radius, outer_radius, stops, colors)?;
        Ok(Self::for_picture(conn, picture))
    }
}

impl<'c, C: X11Connection> PictureWrapper<&'c C>
{
    /// Create a new Picture and return a Picture wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_conical_gradient] that allocates an id for the Picture.
    /// This function returns the resulting `PictureWrapper` that owns the created Picture and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_conical_gradient].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_conical_gradient].
    pub fn create_conical_gradient_and_get_cookie(conn: &'c C, center: Pointfix, angle: Fixed, stops: &[Fixed], colors: &[Color]) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let picture = conn.generate_id()?;
        let cookie = create_conical_gradient(conn, picture, center, angle, stops, colors)?;
        Ok((Self::for_picture(conn, picture), cookie))
    }
}
impl<C: X11Connection> PictureWrapper<C>
{
    /// Create a new Picture and return a Picture wrapper
    ///
    /// This is a thin wrapper around [create_conical_gradient] that allocates an id for the Picture.
    /// This function returns the resulting `PictureWrapper` that owns the created Picture and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_conical_gradient].
    pub fn create_conical_gradient(conn: C, center: Pointfix, angle: Fixed, stops: &[Fixed], colors: &[Color]) -> Result<Self, ReplyOrIdError>
    {
        let picture = conn.generate_id()?;
        let _ = create_conical_gradient(&conn, picture, center, angle, stops, colors)?;
        Ok(Self::for_picture(conn, picture))
    }
}

impl<C: RequestConnection> From<&PictureWrapper<C>> for Picture {
    fn from(from: &PictureWrapper<C>) -> Self {
        from.1
    }
}

impl<C: RequestConnection> Drop for PictureWrapper<C> {
    fn drop(&mut self) {
        let _ = free_picture(&self.0, self.1);
    }
}

/// A RAII-like wrapper around a [Glyphset].
///
/// Instances of this struct represent a Glyphset that is freed in `Drop`.
///
/// Any errors during `Drop` are silently ignored. Most likely an error here means that your
/// X11 connection is broken and later requests will also fail.
#[derive(Debug)]
pub struct GlyphsetWrapper<C: RequestConnection>(C, Glyphset);

impl<C: RequestConnection> GlyphsetWrapper<C>
{
    /// Assume ownership of the given resource and destroy it in `Drop`.
    pub fn for_glyphset(conn: C, id: Glyphset) -> Self {
        GlyphsetWrapper(conn, id)
    }

    /// Get the XID of the wrapped resource
    pub fn glyphset(&self) -> Glyphset {
        self.1
    }

    /// Assume ownership of the XID of the wrapped resource
    ///
    /// This function destroys this wrapper without freeing the underlying resource.
    pub fn into_glyphset(self) -> Glyphset {
        let id = self.1;
        std::mem::forget(self);
        id
    }
}

impl<'c, C: X11Connection> GlyphsetWrapper<&'c C>
{
    /// Create a new Glyphset and return a Glyphset wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_glyph_set] that allocates an id for the Glyphset.
    /// This function returns the resulting `GlyphsetWrapper` that owns the created Glyphset and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_glyph_set].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_glyph_set].
    pub fn create_glyph_set_and_get_cookie(conn: &'c C, format: Pictformat) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let gsid = conn.generate_id()?;
        let cookie = create_glyph_set(conn, gsid, format)?;
        Ok((Self::for_glyphset(conn, gsid), cookie))
    }
}
impl<C: X11Connection> GlyphsetWrapper<C>
{
    /// Create a new Glyphset and return a Glyphset wrapper
    ///
    /// This is a thin wrapper around [create_glyph_set] that allocates an id for the Glyphset.
    /// This function returns the resulting `GlyphsetWrapper` that owns the created Glyphset and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_glyph_set].
    pub fn create_glyph_set(conn: C, format: Pictformat) -> Result<Self, ReplyOrIdError>
    {
        let gsid = conn.generate_id()?;
        let _ = create_glyph_set(&conn, gsid, format)?;
        Ok(Self::for_glyphset(conn, gsid))
    }
}

impl<C: RequestConnection> From<&GlyphsetWrapper<C>> for Glyphset {
    fn from(from: &GlyphsetWrapper<C>) -> Self {
        from.1
    }
}

impl<C: RequestConnection> Drop for GlyphsetWrapper<C> {
    fn drop(&mut self) {
        let _ = free_glyph_set(&self.0, self.1);
    }
}
