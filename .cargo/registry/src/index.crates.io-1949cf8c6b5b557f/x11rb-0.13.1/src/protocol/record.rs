// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Record` X11 extension.

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
use crate::cookie::RecordEnableContextCookie;
use crate::errors::ConnectionError;
#[allow(unused_imports)]
use crate::errors::ReplyOrIdError;

pub use x11rb_protocol::protocol::record::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

pub fn query_version<Conn>(conn: &Conn, major_version: u16, minor_version: u16) -> Result<Cookie<'_, Conn, QueryVersionReply>, ConnectionError>
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

pub fn create_context<'c, 'input, Conn>(conn: &'c Conn, context: Context, element_header: ElementHeader, client_specs: &'input [ClientSpec], ranges: &'input [Range]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateContextRequest {
        context,
        element_header,
        client_specs: Cow::Borrowed(client_specs),
        ranges: Cow::Borrowed(ranges),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn register_clients<'c, 'input, Conn>(conn: &'c Conn, context: Context, element_header: ElementHeader, client_specs: &'input [ClientSpec], ranges: &'input [Range]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = RegisterClientsRequest {
        context,
        element_header,
        client_specs: Cow::Borrowed(client_specs),
        ranges: Cow::Borrowed(ranges),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn unregister_clients<'c, 'input, Conn>(conn: &'c Conn, context: Context, client_specs: &'input [ClientSpec]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = UnregisterClientsRequest {
        context,
        client_specs: Cow::Borrowed(client_specs),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn get_context<Conn>(conn: &Conn, context: Context) -> Result<Cookie<'_, Conn, GetContextReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = GetContextRequest {
        context,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn enable_context<Conn>(conn: &Conn, context: Context) -> Result<RecordEnableContextCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = EnableContextRequest {
        context,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    Ok(RecordEnableContextCookie::new(conn.send_request_with_reply(&slices, fds)?))
}

pub fn disable_context<Conn>(conn: &Conn, context: Context) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = DisableContextRequest {
        context,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn free_context<Conn>(conn: &Conn, context: Context) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = FreeContextRequest {
        context,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn record_query_version(&self, major_version: u16, minor_version: u16) -> Result<Cookie<'_, Self, QueryVersionReply>, ConnectionError>
    {
        query_version(self, major_version, minor_version)
    }
    fn record_create_context<'c, 'input>(&'c self, context: Context, element_header: ElementHeader, client_specs: &'input [ClientSpec], ranges: &'input [Range]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        create_context(self, context, element_header, client_specs, ranges)
    }
    fn record_register_clients<'c, 'input>(&'c self, context: Context, element_header: ElementHeader, client_specs: &'input [ClientSpec], ranges: &'input [Range]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        register_clients(self, context, element_header, client_specs, ranges)
    }
    fn record_unregister_clients<'c, 'input>(&'c self, context: Context, client_specs: &'input [ClientSpec]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        unregister_clients(self, context, client_specs)
    }
    fn record_get_context(&self, context: Context) -> Result<Cookie<'_, Self, GetContextReply>, ConnectionError>
    {
        get_context(self, context)
    }
    fn record_enable_context(&self, context: Context) -> Result<RecordEnableContextCookie<'_, Self>, ConnectionError>
    {
        enable_context(self, context)
    }
    fn record_disable_context(&self, context: Context) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        disable_context(self, context)
    }
    fn record_free_context(&self, context: Context) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        free_context(self, context)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}

/// A RAII-like wrapper around a [Context].
///
/// Instances of this struct represent a Context that is freed in `Drop`.
///
/// Any errors during `Drop` are silently ignored. Most likely an error here means that your
/// X11 connection is broken and later requests will also fail.
#[derive(Debug)]
pub struct ContextWrapper<C: RequestConnection>(C, Context);

impl<C: RequestConnection> ContextWrapper<C>
{
    /// Assume ownership of the given resource and destroy it in `Drop`.
    pub fn for_context(conn: C, id: Context) -> Self {
        ContextWrapper(conn, id)
    }

    /// Get the XID of the wrapped resource
    pub fn context(&self) -> Context {
        self.1
    }

    /// Assume ownership of the XID of the wrapped resource
    ///
    /// This function destroys this wrapper without freeing the underlying resource.
    pub fn into_context(self) -> Context {
        let id = self.1;
        std::mem::forget(self);
        id
    }
}

impl<'c, C: X11Connection> ContextWrapper<&'c C>
{
    /// Create a new Context and return a Context wrapper and a cookie.
    ///
    /// This is a thin wrapper around [create_context] that allocates an id for the Context.
    /// This function returns the resulting `ContextWrapper` that owns the created Context and frees
    /// it in `Drop`. This also returns a `VoidCookie` that comes from the call to
    /// [create_context].
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_context].
    pub fn create_context_and_get_cookie(conn: &'c C, element_header: ElementHeader, client_specs: &[ClientSpec], ranges: &[Range]) -> Result<(Self, VoidCookie<'c, C>), ReplyOrIdError>
    {
        let context = conn.generate_id()?;
        let cookie = create_context(conn, context, element_header, client_specs, ranges)?;
        Ok((Self::for_context(conn, context), cookie))
    }
}
impl<C: X11Connection> ContextWrapper<C>
{
    /// Create a new Context and return a Context wrapper
    ///
    /// This is a thin wrapper around [create_context] that allocates an id for the Context.
    /// This function returns the resulting `ContextWrapper` that owns the created Context and frees
    /// it in `Drop`.
    ///
    /// Errors can come from the call to [X11Connection::generate_id] or [create_context].
    pub fn create_context(conn: C, element_header: ElementHeader, client_specs: &[ClientSpec], ranges: &[Range]) -> Result<Self, ReplyOrIdError>
    {
        let context = conn.generate_id()?;
        let _ = create_context(&conn, context, element_header, client_specs, ranges)?;
        Ok(Self::for_context(conn, context))
    }
}

impl<C: RequestConnection> From<&ContextWrapper<C>> for Context {
    fn from(from: &ContextWrapper<C>) -> Self {
        from.1
    }
}

impl<C: RequestConnection> Drop for ContextWrapper<C> {
    fn drop(&mut self) {
        let _ = free_context(&self.0, self.1);
    }
}
