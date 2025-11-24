// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Shm` X11 extension.

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

pub use x11rb_protocol::protocol::shm::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

/// Query the version of the MIT-SHM extension..
///
/// This is used to determine the version of the MIT-SHM extension supported by the
/// X server.  Clients MUST NOT make other requests in this extension until a reply
/// to this requests indicates the X server supports them.
pub fn query_version<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, QueryVersionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryVersionRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Attach a System V shared memory segment..
///
/// Attach a System V shared memory segment to the server.  This will fail unless
/// the server has permission to map the segment.  The client may destroy the segment
/// as soon as it receives a XCB_SHM_COMPLETION event with the shmseg value in this
/// request and with the appropriate serial number.
///
/// # Fields
///
/// * `shmseg` - A shared memory segment ID created with xcb_generate_id().
/// * `shmid` - The System V shared memory segment the server should map.
/// * `read_only` - True if the segment shall be mapped read only by the X11 server, otherwise false.
pub fn attach<Conn>(conn: &Conn, shmseg: Seg, shmid: u32, read_only: bool) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = AttachRequest {
        shmseg,
        shmid,
        read_only,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Destroys the specified shared memory segment..
///
/// Destroys the specified shared memory segment.  This will never fail unless the
/// segment number is incorrect.
///
/// # Fields
///
/// * `shmseg` - The segment to be destroyed.
pub fn detach<Conn>(conn: &Conn, shmseg: Seg) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DetachRequest {
        shmseg,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Copy data from the shared memory to the specified drawable..
///
/// Copy data from the shared memory to the specified drawable.  The amount of bytes
/// written to the destination image is always equal to the number of bytes read
/// from the shared memory segment.
///
/// # Fields
///
/// * `drawable` - The drawable to draw to.
/// * `gc` - The graphics context to use.
/// * `total_width` - The total width of the source image.
/// * `total_height` - The total height of the source image.
/// * `src_x` - The source X coordinate of the sub-image to copy.
/// * `src_y` - The source Y coordinate of the sub-image to copy.
/// * `src_width` - The width, in source image coordinates, of the data to copy from the source.
/// The X server will use this to determine the amount of data to copy.  The amount
/// of the destination image that is overwritten is determined automatically.
/// * `src_height` - The height, in source image coordinates, of the data to copy from the source.
/// The X server will use this to determine the amount of data to copy.  The amount
/// of the destination image that is overwritten is determined automatically.
/// * `dst_x` - The X coordinate on the destination drawable to copy to.
/// * `dst_y` - The Y coordinate on the destination drawable to copy to.
/// * `depth` - The depth to use.
/// * `format` - The format of the image being drawn.  If it is XYBitmap, depth must be 1, or a
/// "BadMatch" error results.  The foreground pixel in the GC determines the source
/// for the one bits in the image, and the background pixel determines the source
/// for the zero bits.  For XYPixmap and ZPixmap, the depth must match the depth of
/// the drawable, or a "BadMatch" error results.
/// * `send_event` - True if the server should send an XCB_SHM_COMPLETION event when the blit
/// completes.
/// * `offset` - The offset that the source image starts at.
pub fn put_image<Conn>(conn: &Conn, drawable: xproto::Drawable, gc: xproto::Gcontext, total_width: u16, total_height: u16, src_x: u16, src_y: u16, src_width: u16, src_height: u16, dst_x: i16, dst_y: i16, depth: u8, format: u8, send_event: bool, shmseg: Seg, offset: u32) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PutImageRequest {
        drawable,
        gc,
        total_width,
        total_height,
        src_x,
        src_y,
        src_width,
        src_height,
        dst_x,
        dst_y,
        depth,
        format,
        send_event,
        shmseg,
        offset,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Copies data from the specified drawable to the shared memory segment..
///
/// Copy data from the specified drawable to the shared memory segment.  The amount
/// of bytes written to the destination image is always equal to the number of bytes
/// read from the shared memory segment.
///
/// # Fields
///
/// * `drawable` - The drawable to copy the image out of.
/// * `x` - The X coordinate in the drawable to begin copying at.
/// * `y` - The Y coordinate in the drawable to begin copying at.
/// * `width` - The width of the image to copy.
/// * `height` - The height of the image to copy.
/// * `plane_mask` - A mask that determines which planes are used.
/// * `format` - The format to use for the copy (???).
/// * `shmseg` - The destination shared memory segment.
/// * `offset` - The offset in the shared memory segment to copy data to.
pub fn get_image<Conn>(conn: &Conn, drawable: xproto::Drawable, x: i16, y: i16, width: u16, height: u16, plane_mask: u32, format: u8, shmseg: Seg, offset: u32) -> Result<Cookie<'_, Conn, GetImageReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetImageRequest {
        drawable,
        x,
        y,
        width,
        height,
        plane_mask,
        format,
        shmseg,
        offset,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Create a pixmap backed by shared memory..
///
/// Create a pixmap backed by shared memory.  Writes to the shared memory will be
/// reflected in the contents of the pixmap, and writes to the pixmap will be
/// reflected in the contents of the shared memory.
///
/// # Fields
///
/// * `pid` - A pixmap ID created with xcb_generate_id().
/// * `drawable` - The drawable to create the pixmap in.
/// * `width` - The width of the pixmap to create.  Must be nonzero, or a Value error results.
/// * `height` - The height of the pixmap to create.  Must be nonzero, or a Value error results.
/// * `depth` - The depth of the pixmap to create.  Must be nonzero, or a Value error results.
/// * `shmseg` - The shared memory segment to use to create the pixmap.
/// * `offset` - The offset in the segment to create the pixmap at.
pub fn create_pixmap<Conn>(conn: &Conn, pid: xproto::Pixmap, drawable: xproto::Drawable, width: u16, height: u16, depth: u8, shmseg: Seg, offset: u32) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreatePixmapRequest {
        pid,
        drawable,
        width,
        height,
        depth,
        shmseg,
        offset,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Create a shared memory segment.
///
/// Create a shared memory segment.  The file descriptor will be mapped at offset
/// zero, and the size will be obtained using fstat().  A zero size will result in a
/// Value error.
///
/// # Fields
///
/// * `shmseg` - A shared memory segment ID created with xcb_generate_id().
/// * `shm_fd` - The file descriptor the server should mmap().
/// * `read_only` - True if the segment shall be mapped read only by the X11 server, otherwise false.
pub fn attach_fd<Conn, A>(conn: &Conn, shmseg: Seg, shm_fd: A, read_only: bool) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
    A: Into<RawFdContainer>,
{
    let shm_fd: RawFdContainer = shm_fd.into();
    let request0 = AttachFdRequest {
        shmseg,
        shm_fd,
        read_only,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Asks the server to allocate a shared memory segment..
///
/// Asks the server to allocate a shared memory segment.  The server’s reply will
/// include a file descriptor for the client to pass to mmap().
///
/// # Fields
///
/// * `shmseg` - A shared memory segment ID created with xcb_generate_id().
/// * `size` - The size of the segment to create.
/// * `read_only` - True if the server should map the segment read-only; otherwise false.
pub fn create_segment<Conn>(conn: &Conn, shmseg: Seg, size: u32, read_only: bool) -> Result<CookieWithFds<'_, Conn, CreateSegmentReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateSegmentRequest {
        shmseg,
        size,
        read_only,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply_with_fds(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    /// Query the version of the MIT-SHM extension..
    ///
    /// This is used to determine the version of the MIT-SHM extension supported by the
    /// X server.  Clients MUST NOT make other requests in this extension until a reply
    /// to this requests indicates the X server supports them.
    fn shm_query_version(&self) -> Result<Cookie<'_, Self, QueryVersionReply>, ConnectionError>
    {
        query_version(self)
    }
    /// Attach a System V shared memory segment..
    ///
    /// Attach a System V shared memory segment to the server.  This will fail unless
    /// the server has permission to map the segment.  The client may destroy the segment
    /// as soon as it receives a XCB_SHM_COMPLETION event with the shmseg value in this
    /// request and with the appropriate serial number.
    ///
    /// # Fields
    ///
    /// * `shmseg` - A shared memory segment ID created with xcb_generate_id().
    /// * `shmid` - The System V shared memory segment the server should map.
    /// * `read_only` - True if the segment shall be mapped read only by the X11 server, otherwise false.
    fn shm_attach(&self, shmseg: Seg, shmid: u32, read_only: bool) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        attach(self, shmseg, shmid, read_only)
    }
    /// Destroys the specified shared memory segment..
    ///
    /// Destroys the specified shared memory segment.  This will never fail unless the
    /// segment number is incorrect.
    ///
    /// # Fields
    ///
    /// * `shmseg` - The segment to be destroyed.
    fn shm_detach(&self, shmseg: Seg) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        detach(self, shmseg)
    }
    /// Copy data from the shared memory to the specified drawable..
    ///
    /// Copy data from the shared memory to the specified drawable.  The amount of bytes
    /// written to the destination image is always equal to the number of bytes read
    /// from the shared memory segment.
    ///
    /// # Fields
    ///
    /// * `drawable` - The drawable to draw to.
    /// * `gc` - The graphics context to use.
    /// * `total_width` - The total width of the source image.
    /// * `total_height` - The total height of the source image.
    /// * `src_x` - The source X coordinate of the sub-image to copy.
    /// * `src_y` - The source Y coordinate of the sub-image to copy.
    /// * `src_width` - The width, in source image coordinates, of the data to copy from the source.
    /// The X server will use this to determine the amount of data to copy.  The amount
    /// of the destination image that is overwritten is determined automatically.
    /// * `src_height` - The height, in source image coordinates, of the data to copy from the source.
    /// The X server will use this to determine the amount of data to copy.  The amount
    /// of the destination image that is overwritten is determined automatically.
    /// * `dst_x` - The X coordinate on the destination drawable to copy to.
    /// * `dst_y` - The Y coordinate on the destination drawable to copy to.
    /// * `depth` - The depth to use.
    /// * `format` - The format of the image being drawn.  If it is XYBitmap, depth must be 1, or a
    /// "BadMatch" error results.  The foreground pixel in the GC determines the source
    /// for the one bits in the image, and the background pixel determines the source
    /// for the zero bits.  For XYPixmap and ZPixmap, the depth must match the depth of
    /// the drawable, or a "BadMatch" error results.
    /// * `send_event` - True if the server should send an XCB_SHM_COMPLETION event when the blit
    /// completes.
    /// * `offset` - The offset that the source image starts at.
    fn shm_put_image(&self, drawable: xproto::Drawable, gc: xproto::Gcontext, total_width: u16, total_height: u16, src_x: u16, src_y: u16, src_width: u16, src_height: u16, dst_x: i16, dst_y: i16, depth: u8, format: u8, send_event: bool, shmseg: Seg, offset: u32) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        put_image(self, drawable, gc, total_width, total_height, src_x, src_y, src_width, src_height, dst_x, dst_y, depth, format, send_event, shmseg, offset)
    }
    /// Copies data from the specified drawable to the shared memory segment..
    ///
    /// Copy data from the specified drawable to the shared memory segment.  The amount
    /// of bytes written to the destination image is always equal to the number of bytes
    /// read from the shared memory segment.
    ///
    /// # Fields
    ///
    /// * `drawable` - The drawable to copy the image out of.
    /// * `x` - The X coordinate in the drawable to begin copying at.
    /// * `y` - The Y coordinate in the drawable to begin copying at.
    /// * `width` - The width of the image to copy.
    /// * `height` - The height of the image to copy.
    /// * `plane_mask` - A mask that determines which planes are used.
    /// * `format` - The format to use for the copy (???).
    /// * `shmseg` - The destination shared memory segment.
    /// * `offset` - The offset in the shared memory segment to copy data to.
    fn shm_get_image(&self, drawable: xproto::Drawable, x: i16, y: i16, width: u16, height: u16, plane_mask: u32, format: u8, shmseg: Seg, offset: u32) -> Result<Cookie<'_, Self, GetImageReply>, ConnectionError>
    {
        get_image(self, drawable, x, y, width, height, plane_mask, format, shmseg, offset)
    }
    /// Create a pixmap backed by shared memory..
    ///
    /// Create a pixmap backed by shared memory.  Writes to the shared memory will be
    /// reflected in the contents of the pixmap, and writes to the pixmap will be
    /// reflected in the contents of the shared memory.
    ///
    /// # Fields
    ///
    /// * `pid` - A pixmap ID created with xcb_generate_id().
    /// * `drawable` - The drawable to create the pixmap in.
    /// * `width` - The width of the pixmap to create.  Must be nonzero, or a Value error results.
    /// * `height` - The height of the pixmap to create.  Must be nonzero, or a Value error results.
    /// * `depth` - The depth of the pixmap to create.  Must be nonzero, or a Value error results.
    /// * `shmseg` - The shared memory segment to use to create the pixmap.
    /// * `offset` - The offset in the segment to create the pixmap at.
    fn shm_create_pixmap(&self, pid: xproto::Pixmap, drawable: xproto::Drawable, width: u16, height: u16, depth: u8, shmseg: Seg, offset: u32) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        create_pixmap(self, pid, drawable, width, height, depth, shmseg, offset)
    }
    /// Create a shared memory segment.
    ///
    /// Create a shared memory segment.  The file descriptor will be mapped at offset
    /// zero, and the size will be obtained using fstat().  A zero size will result in a
    /// Value error.
    ///
    /// # Fields
    ///
    /// * `shmseg` - A shared memory segment ID created with xcb_generate_id().
    /// * `shm_fd` - The file descriptor the server should mmap().
    /// * `read_only` - True if the segment shall be mapped read only by the X11 server, otherwise false.
    fn shm_attach_fd<A>(&self, shmseg: Seg, shm_fd: A, read_only: bool) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<RawFdContainer>,
    {
        attach_fd(self, shmseg, shm_fd, read_only)
    }
    /// Asks the server to allocate a shared memory segment..
    ///
    /// Asks the server to allocate a shared memory segment.  The server’s reply will
    /// include a file descriptor for the client to pass to mmap().
    ///
    /// # Fields
    ///
    /// * `shmseg` - A shared memory segment ID created with xcb_generate_id().
    /// * `size` - The size of the segment to create.
    /// * `read_only` - True if the server should map the segment read-only; otherwise false.
    fn shm_create_segment(&self, shmseg: Seg, size: u32, read_only: bool) -> Result<CookieWithFds<'_, Self, CreateSegmentReply>, ConnectionError>
    {
        create_segment(self, shmseg, size, read_only)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}

/// A RAII-like wrapper around a [Seg].
///
/// Instances of this struct represent a Seg that is freed in `Drop`.
///
/// Any errors during `Drop` are silently ignored. Most likely an error here means that your
/// X11 connection is broken and later requests will also fail.
#[derive(Debug)]
pub struct SegWrapper<C: RequestConnection>(C, Seg);

impl<C: RequestConnection> SegWrapper<C>
{
    /// Assume ownership of the given resource and destroy it in `Drop`.
    pub fn for_seg(conn: C, id: Seg) -> Self {
        SegWrapper(conn, id)
    }

    /// Get the XID of the wrapped resource
    pub fn seg(&self) -> Seg {
        self.1
    }

    /// Assume ownership of the XID of the wrapped resource
    ///
    /// This function destroys this wrapper without freeing the underlying resource.
    pub fn into_seg(self) -> Seg {
        let id = self.1;
        std::mem::forget(self);
        id
    }
}

impl<'c, C: X11Connection> SegWrapper<&'c C>
{
    /// Create a new Seg and return a Seg wrapper and a cookie.
    ///
    /// This is a thin wrapper around [attach] that allocates an id for the Seg.
    /// This function returns the resulting `SegWrapper` that owns the created Seg and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [attach].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [attach].
    pub fn attach_and_get_cookie(conn: &'c C, shmid: u32, read_only: bool) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let shmseg = conn.generate_id()?;
        let cookie = attach(conn, shmseg, shmid, read_only)?;
        Ok((Self::for_seg(conn, shmseg), cookie))
    }
}
impl<C: X11Connection> SegWrapper<C>
{
    /// Create a new Seg and return a Seg wrapper
    ///
    /// This is a thin wrapper around [attach] that allocates an id for the Seg.
    /// This function returns the resulting `SegWrapper` that owns the created Seg and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [attach].
    pub fn attach(conn: C, shmid: u32, read_only: bool) -> Result<Self, ReplyOrIdError>
    {
        let shmseg = conn.generate_id()?;
        let _ = attach(&conn, shmseg, shmid, read_only)?;
        Ok(Self::for_seg(conn, shmseg))
    }
}

impl<'c, C: X11Connection> SegWrapper<&'c C>
{
    /// Create a new Seg and return a Seg wrapper and a cookie.
    ///
    /// This is a thin wrapper around [attach_fd] that allocates an id for the Seg.
    /// This function returns the resulting `SegWrapper` that owns the created Seg and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [attach_fd].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [attach_fd].
    pub fn attach_fd_and_get_cookie<A>(conn: &'c C, shm_fd: A, read_only: bool) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    where
        A: Into<RawFdContainer>,
    {
        let shmseg = conn.generate_id()?;
        let cookie = attach_fd(conn, shmseg, shm_fd, read_only)?;
        Ok((Self::for_seg(conn, shmseg), cookie))
    }
}
impl<C: X11Connection> SegWrapper<C>
{
    /// Create a new Seg and return a Seg wrapper
    ///
    /// This is a thin wrapper around [attach_fd] that allocates an id for the Seg.
    /// This function returns the resulting `SegWrapper` that owns the created Seg and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [attach_fd].
    pub fn attach_fd<A>(conn: C, shm_fd: A, read_only: bool) -> Result<Self, ReplyOrIdError>
    where
        A: Into<RawFdContainer>,
    {
        let shmseg = conn.generate_id()?;
        let _ = attach_fd(&conn, shmseg, shm_fd, read_only)?;
        Ok(Self::for_seg(conn, shmseg))
    }
}

impl<C: RequestConnection> From<&SegWrapper<C>> for Seg {
    fn from(from: &SegWrapper<C>) -> Self {
        from.1
    }
}

impl<C: RequestConnection> Drop for SegWrapper<C> {
    fn drop(&mut self) {
        let _ = detach(&self.0, self.1);
    }
}
