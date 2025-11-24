//! X11 resource manager library.
//!
//! Usage example (please cache the database returned by [`new_from_default`] in real applications
//! instead of re-opening it whenever a value is needed):
//! ```
//! use x11rb::{connection::Connection, errors::ReplyError, resource_manager::new_from_default};
//! fn get_xft_dpi(conn: &impl Connection) -> Result<Option<u32>, ReplyError> {
//!     let db = new_from_default(conn)?;
//!     let value = db.get_value("Xft.dpi", "");
//!     Ok(value.ok().flatten())
//! }
//! ```
//!
//! This functionality is similar to what is available to C code through xcb-util-xrm and Xlib's
//! `Xrm*` function family. Not all their functionality is available in this library. Please open a
//! feature request if you need something that is not available.
//!
//! The code in this module is only available when the `resource_manager` feature of the library is
//! enabled.

use crate::connection::Connection;
use crate::errors::ReplyError;
use crate::protocol::xproto::GetPropertyReply;

pub use x11rb_protocol::resource_manager::Database;

fn send_request(conn: &impl Connection) -> Result<GetPropertyReply, ReplyError> {
    let mut request = Database::GET_RESOURCE_DATABASE;
    request.window = conn.setup().roots[0].root;
    conn.send_trait_request_with_reply(request)?.reply()
}

/// Create a new X11 resource database from the `RESOURCE_MANAGER` property of the first
/// screen's root window.
///
/// This function returns an error if the `GetProperty` request to get the `RESOURCE_MANAGER`
/// property fails. It returns `Ok(None)` if the property does not exist, has the wrong format,
/// or is empty.
pub fn new_from_resource_manager(conn: &impl Connection) -> Result<Option<Database>, ReplyError> {
    Ok(Database::new_from_get_property_reply(&send_request(conn)?))
}

/// Create a new X11 resource database from the default locations.
///
/// The default location is a combination of two places. First, the following places are
/// searched for data:
/// - The `RESOURCE_MANAGER` property of the first screen's root window (See
///   [`new_from_resource_manager`]).
/// - If not found, the file `$HOME/.Xresources` is loaded.
/// - If not found, the file `$HOME/.Xdefaults` is loaded.
///
/// The result of the above search of the above search is combined with:
/// - The contents of the file `$XENVIRONMENT`, if this environment variable is set.
/// - Otherwise, the contents of `$HOME/.Xdefaults-[hostname]`.
///
/// This function only returns an error if communication with the X11 server fails. All other
/// errors are ignored. It might be that an empty database is returned.
///
/// The behaviour of this function is mostly equivalent to Xlib's `XGetDefault()`. The
/// exception is that `XGetDefault()` does not load `$HOME/.Xresources`.
///
/// The behaviour of this function is equivalent to xcb-util-xrm's
/// `xcb_xrm_database_from_default()`.
pub fn new_from_default(conn: &impl Connection) -> Result<Database, ReplyError> {
    Ok(Database::new_from_default(
        &send_request(conn)?,
        crate::hostname(),
    ))
}
