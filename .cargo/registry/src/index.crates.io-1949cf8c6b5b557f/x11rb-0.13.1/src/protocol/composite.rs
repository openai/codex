// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Composite` X11 extension.

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
use super::xfixes;
#[allow(unused_imports)]
use super::xproto;

pub use x11rb_protocol::protocol::composite::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

/// Negotiate the version of Composite.
///
/// This negotiates the version of the Composite extension.  It must be precede all
/// other requests using Composite.  Failure to do so will cause a BadRequest error.
///
/// # Fields
///
/// * `client_major_version` - The major version supported by the client.
/// * `client_minor_version` - The minor version supported by the client.
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

/// Redirect the hierarchy starting at "window" to off-screen storage..
///
/// The hierarchy starting at 'window' is directed to off-screen
/// storage.  When all clients enabling redirection terminate,
/// the redirection will automatically be disabled.
///
/// The root window may not be redirected. Doing so results in a Match
/// error.
///
/// # Fields
///
/// * `window` - The root of the hierarchy to redirect to off-screen storage.
/// * `update` - Whether contents are automatically mirrored to the parent window.  If one client
/// already specifies an update type of Manual, any attempt by another to specify a
/// mode of Manual so will result in an Access error.
pub fn redirect_window<Conn>(conn: &Conn, window: xproto::Window, update: Redirect) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = RedirectWindowRequest {
        window,
        update,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Redirect all current and future children of ‘window’.
///
/// Hierarchies starting at all current and future children of window
/// will be redirected as in RedirectWindow. If update is Manual,
/// then painting of the window background during window manipulation
/// and ClearArea requests is inhibited.
///
/// # Fields
///
/// * `window` - The root of the hierarchy to redirect to off-screen storage.
/// * `update` - Whether contents are automatically mirrored to the parent window.  If one client
/// already specifies an update type of Manual, any attempt by another to specify a
/// mode of Manual so will result in an Access error.
pub fn redirect_subwindows<Conn>(conn: &Conn, window: xproto::Window, update: Redirect) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = RedirectSubwindowsRequest {
        window,
        update,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Terminate redirection of the specified window..
///
/// Redirection of the specified window will be terminated.  This cannot be
/// used if the window was redirected with RedirectSubwindows.
///
/// # Fields
///
/// * `window` - The window to terminate redirection of.  Must be redirected by the
/// current client, or a Value error results.
/// * `update` - The update type passed to RedirectWindows.  If this does not match the
/// previously requested update type, a Value error results.
pub fn unredirect_window<Conn>(conn: &Conn, window: xproto::Window, update: Redirect) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = UnredirectWindowRequest {
        window,
        update,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Terminate redirection of the specified window’s children.
///
/// Redirection of all children of window will be terminated.
///
/// # Fields
///
/// * `window` - The window to terminate redirection of.  Must have previously been
/// selected for sub-redirection by the current client, or a Value error
/// results.
/// * `update` - The update type passed to RedirectSubWindows.  If this does not match
/// the previously requested update type, a Value error results.
pub fn unredirect_subwindows<Conn>(conn: &Conn, window: xproto::Window, update: Redirect) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = UnredirectSubwindowsRequest {
        window,
        update,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn create_region_from_border_clip<Conn>(conn: &Conn, region: xfixes::Region, window: xproto::Window) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateRegionFromBorderClipRequest {
        region,
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn name_window_pixmap<Conn>(conn: &Conn, window: xproto::Window, pixmap: xproto::Pixmap) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = NameWindowPixmapRequest {
        window,
        pixmap,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_overlay_window<Conn>(conn: &Conn, window: xproto::Window) -> Result<Cookie<'_, Conn, GetOverlayWindowReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetOverlayWindowRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn release_overlay_window<Conn>(conn: &Conn, window: xproto::Window) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = ReleaseOverlayWindowRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    /// Negotiate the version of Composite.
    ///
    /// This negotiates the version of the Composite extension.  It must be precede all
    /// other requests using Composite.  Failure to do so will cause a BadRequest error.
    ///
    /// # Fields
    ///
    /// * `client_major_version` - The major version supported by the client.
    /// * `client_minor_version` - The minor version supported by the client.
    fn composite_query_version(&self, client_major_version: u32, client_minor_version: u32) -> Result<Cookie<'_, Self, QueryVersionReply>, ConnectionError>
    {
        query_version(self, client_major_version, client_minor_version)
    }
    /// Redirect the hierarchy starting at "window" to off-screen storage..
    ///
    /// The hierarchy starting at 'window' is directed to off-screen
    /// storage.  When all clients enabling redirection terminate,
    /// the redirection will automatically be disabled.
    ///
    /// The root window may not be redirected. Doing so results in a Match
    /// error.
    ///
    /// # Fields
    ///
    /// * `window` - The root of the hierarchy to redirect to off-screen storage.
    /// * `update` - Whether contents are automatically mirrored to the parent window.  If one client
    /// already specifies an update type of Manual, any attempt by another to specify a
    /// mode of Manual so will result in an Access error.
    fn composite_redirect_window(&self, window: xproto::Window, update: Redirect) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        redirect_window(self, window, update)
    }
    /// Redirect all current and future children of ‘window’.
    ///
    /// Hierarchies starting at all current and future children of window
    /// will be redirected as in RedirectWindow. If update is Manual,
    /// then painting of the window background during window manipulation
    /// and ClearArea requests is inhibited.
    ///
    /// # Fields
    ///
    /// * `window` - The root of the hierarchy to redirect to off-screen storage.
    /// * `update` - Whether contents are automatically mirrored to the parent window.  If one client
    /// already specifies an update type of Manual, any attempt by another to specify a
    /// mode of Manual so will result in an Access error.
    fn composite_redirect_subwindows(&self, window: xproto::Window, update: Redirect) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        redirect_subwindows(self, window, update)
    }
    /// Terminate redirection of the specified window..
    ///
    /// Redirection of the specified window will be terminated.  This cannot be
    /// used if the window was redirected with RedirectSubwindows.
    ///
    /// # Fields
    ///
    /// * `window` - The window to terminate redirection of.  Must be redirected by the
    /// current client, or a Value error results.
    /// * `update` - The update type passed to RedirectWindows.  If this does not match the
    /// previously requested update type, a Value error results.
    fn composite_unredirect_window(&self, window: xproto::Window, update: Redirect) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        unredirect_window(self, window, update)
    }
    /// Terminate redirection of the specified window’s children.
    ///
    /// Redirection of all children of window will be terminated.
    ///
    /// # Fields
    ///
    /// * `window` - The window to terminate redirection of.  Must have previously been
    /// selected for sub-redirection by the current client, or a Value error
    /// results.
    /// * `update` - The update type passed to RedirectSubWindows.  If this does not match
    /// the previously requested update type, a Value error results.
    fn composite_unredirect_subwindows(&self, window: xproto::Window, update: Redirect) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        unredirect_subwindows(self, window, update)
    }
    fn composite_create_region_from_border_clip(&self, region: xfixes::Region, window: xproto::Window) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        create_region_from_border_clip(self, region, window)
    }
    fn composite_name_window_pixmap(&self, window: xproto::Window, pixmap: xproto::Pixmap) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        name_window_pixmap(self, window, pixmap)
    }
    fn composite_get_overlay_window(&self, window: xproto::Window) -> Result<Cookie<'_, Self, GetOverlayWindowReply>, ConnectionError>
    {
        get_overlay_window(self, window)
    }
    fn composite_release_overlay_window(&self, window: xproto::Window) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        release_overlay_window(self, window)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
