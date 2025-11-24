// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `Res` X11 extension.

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

pub use x11rb_protocol::protocol::res::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

pub fn query_version<Conn>(conn: &Conn, client_major: u8, client_minor: u8) -> Result<Cookie<'_, Conn, QueryVersionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryVersionRequest {
        client_major,
        client_minor,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_clients<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, QueryClientsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryClientsRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_client_resources<Conn>(conn: &Conn, xid: u32) -> Result<Cookie<'_, Conn, QueryClientResourcesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryClientResourcesRequest {
        xid,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_client_pixmap_bytes<Conn>(conn: &Conn, xid: u32) -> Result<Cookie<'_, Conn, QueryClientPixmapBytesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryClientPixmapBytesRequest {
        xid,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_client_ids<'c, 'input, Conn>(conn: &'c Conn, specs: &'input [ClientIdSpec]) -> Result<Cookie<'c, Conn, QueryClientIdsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryClientIdsRequest {
        specs: Cow::Borrowed(specs),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn query_resource_bytes<'c, 'input, Conn>(conn: &'c Conn, client: u32, specs: &'input [ResourceIdSpec]) -> Result<Cookie<'c, Conn, QueryResourceBytesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = QueryResourceBytesRequest {
        client,
        specs: Cow::Borrowed(specs),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn res_query_version(&self, client_major: u8, client_minor: u8) -> Result<Cookie<'_, Self, QueryVersionReply>, ConnectionError>
    {
        query_version(self, client_major, client_minor)
    }
    fn res_query_clients(&self) -> Result<Cookie<'_, Self, QueryClientsReply>, ConnectionError>
    {
        query_clients(self)
    }
    fn res_query_client_resources(&self, xid: u32) -> Result<Cookie<'_, Self, QueryClientResourcesReply>, ConnectionError>
    {
        query_client_resources(self, xid)
    }
    fn res_query_client_pixmap_bytes(&self, xid: u32) -> Result<Cookie<'_, Self, QueryClientPixmapBytesReply>, ConnectionError>
    {
        query_client_pixmap_bytes(self, xid)
    }
    fn res_query_client_ids<'c, 'input>(&'c self, specs: &'input [ClientIdSpec]) -> Result<Cookie<'c, Self, QueryClientIdsReply>, ConnectionError>
    {
        query_client_ids(self, specs)
    }
    fn res_query_resource_bytes<'c, 'input>(&'c self, client: u32, specs: &'input [ResourceIdSpec]) -> Result<Cookie<'c, Self, QueryResourceBytesReply>, ConnectionError>
    {
        query_resource_bytes(self, client, specs)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
