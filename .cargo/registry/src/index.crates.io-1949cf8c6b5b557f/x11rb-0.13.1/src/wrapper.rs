//! Some wrappers around the generated code to simplify use.

use super::cookie::VoidCookie;
use super::errors::{ConnectionError, ReplyError};
use super::protocol::xproto::{Atom, ConnectionExt as XProtoConnectionExt, PropMode, Window};

/// Extension trait that simplifies API use
pub trait ConnectionExt: XProtoConnectionExt {
    /// Change a property on a window with format 8.
    fn change_property8<A, B>(
        &self,
        mode: PropMode,
        window: Window,
        property: A,
        type_: B,
        data: &[u8],
    ) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Atom>,
        B: Into<Atom>,
    {
        self.change_property(
            mode,
            window,
            property,
            type_,
            8,
            data.len().try_into().expect("`data` has too many elements"),
            data,
        )
    }

    /// Change a property on a window with format 16.
    fn change_property16<A, B>(
        &self,
        mode: PropMode,
        window: Window,
        property: A,
        type_: B,
        data: &[u16],
    ) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Atom>,
        B: Into<Atom>,
    {
        let mut data_u8 = Vec::with_capacity(data.len() * 2);
        for item in data {
            data_u8.extend(item.to_ne_bytes());
        }
        self.change_property(
            mode,
            window,
            property,
            type_,
            16,
            data.len().try_into().expect("`data` has too many elements"),
            &data_u8,
        )
    }

    /// Change a property on a window with format 32.
    fn change_property32<A, B>(
        &self,
        mode: PropMode,
        window: Window,
        property: A,
        type_: B,
        data: &[u32],
    ) -> Result<VoidCookie<'_, Self>, ConnectionError>
    where
        A: Into<Atom>,
        B: Into<Atom>,
    {
        let mut data_u8 = Vec::with_capacity(data.len() * 4);
        for item in data {
            data_u8.extend(item.to_ne_bytes());
        }
        self.change_property(
            mode,
            window,
            property,
            type_,
            32,
            data.len().try_into().expect("`data` has too many elements"),
            &data_u8,
        )
    }

    /// Synchronise with the X11 server.
    ///
    /// This function synchronises with the X11 server. This means that all requests that are still
    /// in the output buffer are sent to the server. Then, we wait until the X11 server processed
    /// all requests.
    fn sync(&self) -> Result<(), ReplyError> {
        // When a new request is generated, it is appended to the output buffer. Thus, this causes
        // all previous requests to be sent.
        // The X11 server is single-threaded and processes requests in-order. Thus, it will only
        // reply to our GetInputFocus after everything before was processed.
        self.get_input_focus()?.reply().and(Ok(()))
    }
}
impl<C: XProtoConnectionExt + ?Sized> ConnectionExt for C {}

/// A RAII-like wrapper around [super::protocol::xproto::grab_server] and
/// [super::protocol::xproto::ungrab_server].
///
/// Instances of this struct represent that we sent a [super::protocol::xproto::grab_server]
/// request. When this struct is dropped, an [super::protocol::xproto::ungrab_server] request is
/// sent.
///
/// Any errors during `Drop` are silently ignored. Most likely an error here means that your
/// X11 connection is broken and later requests will also fail.
#[derive(Debug)]
pub struct GrabServer<'c, C: XProtoConnectionExt>(&'c C);

impl<'c, C: XProtoConnectionExt> GrabServer<'c, C> {
    /// Grab the server by sending a [super::protocol::xproto::grab_server] request.
    ///
    /// The returned type will call [super::protocol::xproto::ungrab_server] when it is dropped.
    pub fn grab(conn: &'c C) -> Result<Self, ConnectionError> {
        // Grab the server, return any errors, ignore the resulting VoidCookie
        drop(conn.grab_server()?);
        Ok(Self(conn))
    }
}

impl<C: XProtoConnectionExt> Drop for GrabServer<'_, C> {
    fn drop(&mut self) {
        let _ = (self.0).ungrab_server();
    }
}
