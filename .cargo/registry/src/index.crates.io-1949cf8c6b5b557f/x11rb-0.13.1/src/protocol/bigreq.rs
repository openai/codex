// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `BigRequests` X11 extension.

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

pub use x11rb_protocol::protocol::bigreq::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

/// Enable the BIG-REQUESTS extension.
///
/// This enables the BIG-REQUESTS extension, which allows for requests larger than
/// 262140 bytes in length.  When enabled, if the 16-bit length field is zero, it
/// is immediately followed by a 32-bit length field specifying the length of the
/// request in 4-byte units.
pub fn enable<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, EnableReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = EnableRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    /// Enable the BIG-REQUESTS extension.
    ///
    /// This enables the BIG-REQUESTS extension, which allows for requests larger than
    /// 262140 bytes in length.  When enabled, if the 16-bit length field is zero, it
    /// is immediately followed by a 32-bit length field specifying the length of the
    /// request in 4-byte units.
    fn bigreq_enable(&self) -> Result<Cookie<'_, Self, EnableReply>, ConnectionError>
    {
        enable(self)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
