// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Dbe` X11 extension.

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

pub use x11rb_protocol::protocol::dbe::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

/// Queries the version of this extension.
///
/// Queries the version of this extension. You must do this before using any functionality it provides.
///
/// # Fields
///
/// * `major_version` - The major version of the extension. Check that it is compatible with the XCB_DBE_MAJOR_VERSION that your code is compiled with.
/// * `minor_version` - The minor version of the extension. Check that it is compatible with the XCB_DBE_MINOR_VERSION that your code is compiled with.
pub fn query_version<Conn>(conn: &Conn, major_version: u8, minor_version: u8) -> Result<Cookie<'_, Conn, QueryVersionReply>, ConnectionError>
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

/// Allocates a back buffer.
///
/// Associates `buffer` with the back buffer of `window`. Multiple ids may be associated with the back buffer, which is created by the first allocate call and destroyed by the last deallocate.
///
/// # Fields
///
/// * `window` - The window to which to add the back buffer.
/// * `buffer` - The buffer id to associate with the back buffer.
/// * `swap_action` - The swap action most likely to be used to present this back buffer. This is only a hint, and does not preclude the use of other swap actions.
pub fn allocate_back_buffer<Conn>(conn: &Conn, window: xproto::Window, buffer: BackBuffer, swap_action: u8) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = AllocateBackBufferRequest {
        window,
        buffer,
        swap_action,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Deallocates a back buffer.
///
/// Deallocates the given `buffer`. If `buffer` is an invalid id, a `BadBuffer` error is returned. Because a window may have allocated multiple back buffer ids, the back buffer itself is not deleted until all these ids are deallocated by this call.
///
/// # Fields
///
/// * `buffer` - The back buffer to deallocate.
pub fn deallocate_back_buffer<Conn>(conn: &Conn, buffer: BackBuffer) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DeallocateBackBufferRequest {
        buffer,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Swaps front and back buffers.
///
/// Swaps the front and back buffers on the specified windows. The front and back buffers retain their ids, so that the window id continues to refer to the front buffer, while the back buffer id created by this extension continues to refer to the back buffer. Back buffer contents is moved to the front buffer. Back buffer contents after the operation depends on the given swap action. The optimal swap action depends on how each frame is rendered. For example, if the buffer is cleared and fully overwritten on every frame, the "untouched" action, which throws away the buffer contents, would provide the best performance. To eliminate visual artifacts, the swap will occure during the monitor VSync, if the X server supports detecting it.
///
/// # Fields
///
/// * `n_actions` - Number of swap actions in `actions`.
/// * `actions` - List of windows on which to swap buffers.
pub fn swap_buffers<'c, 'input, Conn>(conn: &'c Conn, actions: &'input [SwapInfo]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = SwapBuffersRequest {
        actions: Cow::Borrowed(actions),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Begins a logical swap block.
///
/// Creates a block of operations intended to occur together. This may be needed if window presentation requires changing buffers unknown to this extension, such as depth or stencil buffers.
pub fn begin_idiom<Conn>(conn: &Conn) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = BeginIdiomRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Ends a logical swap block.
pub fn end_idiom<Conn>(conn: &Conn) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = EndIdiomRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Requests visuals that support double buffering.
pub fn get_visual_info<'c, 'input, Conn>(conn: &'c Conn, drawables: &'input [xproto::Drawable]) -> Result<Cookie<'c, Conn, GetVisualInfoReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetVisualInfoRequest {
        drawables: Cow::Borrowed(drawables),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Gets back buffer attributes.
///
/// Returns the attributes of the specified `buffer`.
///
/// # Fields
///
/// * `buffer` - The back buffer to query.
/// * `attributes` - The attributes of `buffer`.
pub fn get_back_buffer_attributes<Conn>(conn: &Conn, buffer: BackBuffer) -> Result<Cookie<'_, Conn, GetBackBufferAttributesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetBackBufferAttributesRequest {
        buffer,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    /// Queries the version of this extension.
    ///
    /// Queries the version of this extension. You must do this before using any functionality it provides.
    ///
    /// # Fields
    ///
    /// * `major_version` - The major version of the extension. Check that it is compatible with the XCB_DBE_MAJOR_VERSION that your code is compiled with.
    /// * `minor_version` - The minor version of the extension. Check that it is compatible with the XCB_DBE_MINOR_VERSION that your code is compiled with.
    fn dbe_query_version(&self, major_version: u8, minor_version: u8) -> Result<Cookie<'_, Self, QueryVersionReply>, ConnectionError>
    {
        query_version(self, major_version, minor_version)
    }
    /// Allocates a back buffer.
    ///
    /// Associates `buffer` with the back buffer of `window`. Multiple ids may be associated with the back buffer, which is created by the first allocate call and destroyed by the last deallocate.
    ///
    /// # Fields
    ///
    /// * `window` - The window to which to add the back buffer.
    /// * `buffer` - The buffer id to associate with the back buffer.
    /// * `swap_action` - The swap action most likely to be used to present this back buffer. This is only a hint, and does not preclude the use of other swap actions.
    fn dbe_allocate_back_buffer(&self, window: xproto::Window, buffer: BackBuffer, swap_action: u8) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        allocate_back_buffer(self, window, buffer, swap_action)
    }
    /// Deallocates a back buffer.
    ///
    /// Deallocates the given `buffer`. If `buffer` is an invalid id, a `BadBuffer` error is returned. Because a window may have allocated multiple back buffer ids, the back buffer itself is not deleted until all these ids are deallocated by this call.
    ///
    /// # Fields
    ///
    /// * `buffer` - The back buffer to deallocate.
    fn dbe_deallocate_back_buffer(&self, buffer: BackBuffer) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        deallocate_back_buffer(self, buffer)
    }
    /// Swaps front and back buffers.
    ///
    /// Swaps the front and back buffers on the specified windows. The front and back buffers retain their ids, so that the window id continues to refer to the front buffer, while the back buffer id created by this extension continues to refer to the back buffer. Back buffer contents is moved to the front buffer. Back buffer contents after the operation depends on the given swap action. The optimal swap action depends on how each frame is rendered. For example, if the buffer is cleared and fully overwritten on every frame, the "untouched" action, which throws away the buffer contents, would provide the best performance. To eliminate visual artifacts, the swap will occure during the monitor VSync, if the X server supports detecting it.
    ///
    /// # Fields
    ///
    /// * `n_actions` - Number of swap actions in `actions`.
    /// * `actions` - List of windows on which to swap buffers.
    fn dbe_swap_buffers<'c, 'input>(&'c self, actions: &'input [SwapInfo]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        swap_buffers(self, actions)
    }
    /// Begins a logical swap block.
    ///
    /// Creates a block of operations intended to occur together. This may be needed if window presentation requires changing buffers unknown to this extension, such as depth or stencil buffers.
    fn dbe_begin_idiom(&self) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        begin_idiom(self)
    }
    /// Ends a logical swap block.
    fn dbe_end_idiom(&self) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        end_idiom(self)
    }
    /// Requests visuals that support double buffering.
    fn dbe_get_visual_info<'c, 'input>(&'c self, drawables: &'input [xproto::Drawable]) -> Result<Cookie<'c, Self, GetVisualInfoReply>, ConnectionError>
    {
        get_visual_info(self, drawables)
    }
    /// Gets back buffer attributes.
    ///
    /// Returns the attributes of the specified `buffer`.
    ///
    /// # Fields
    ///
    /// * `buffer` - The back buffer to query.
    /// * `attributes` - The attributes of `buffer`.
    fn dbe_get_back_buffer_attributes(&self, buffer: BackBuffer) -> Result<Cookie<'_, Self, GetBackBufferAttributesReply>, ConnectionError>
    {
        get_back_buffer_attributes(self, buffer)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
