//! X11 rust bindings.
//!
//! This library allows to interact with an X11 server from rust code. A connection to an X11
//! server is represented by an implementation of the `Connection` trait.
//!
//! The client can interact with the server by sending requests. The server can answer requests and
//! can also generate events.
//!
//! The examples that come with this library might be a good starting point for new users.
//!
//!
//! # Getting started with X11
//!
//! X11 is a big protocol. I would claim that most of it is not actually that complicated, but it
//! is still difficult to get into it. A good starting point might be some [libxcb
//! tutorial](https://www.x.org/releases/X11R7.7/doc/libxcb/tutorial/index.html). This tutorial
//! was adapted in this crate [as an
//! example](https://github.com/psychon/x11rb/blob/master/x11rb/examples/tutorial.rs). A more in-depth
//! look at the X11 protocol can be gained from the [protocol reference
//! manual](https://www.x.org/releases/X11R7.6/doc/xproto/x11protocol.html), but this requires some
//! existing basic understanding of X11. If you want to figure out what some specific request does,
//! be sure to look it up in the specification!
//!
//! Most extensions can be understood by reading their specification. Most of them can be found
//! [here](https://www.x.org/releases/current/doc/index.html#protocol). For example, [the
//! specification of Composite
//! 0.4](https://www.x.org/releases/X11R7.5/doc/compositeproto/compositeproto.txt) consists of
//! about six pages of text.
//!
//! The notable exception is the X keyboard extension, which is documented in a [PDF file with 168
//! pages](https://www.x.org/releases/current/doc/kbproto/xkbproto.pdf) which I am never going to
//! read completely.
//!
//!
//! # Getting started with x11rb
//!
//! Most code in this code is automatically generated from an XML description of the protocol. This
//! is the same approach as taken by [libxcb](https://xcb.freedesktop.org/) (and in fact this uses
//! the same XML description). This means that if you know your way around X11, most things should
//! be obvious to you.
//!
//! For example, here is how to create a new window with x11rb:
//! ```no_run
//! use x11rb::connection::Connection;
//! use x11rb::errors::ReplyOrIdError;
//! use x11rb::protocol::xproto::*;
//! use x11rb::COPY_DEPTH_FROM_PARENT;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let (conn, screen_num) = x11rb::connect(None).unwrap();
//!     let screen = &conn.setup().roots[screen_num];
//!     let win_id = conn.generate_id()?;
//!     conn.create_window(
//!         COPY_DEPTH_FROM_PARENT,
//!         win_id,
//!         screen.root,
//!         0,
//!         0,
//!         100,
//!         100,
//!         0,
//!         WindowClass::INPUT_OUTPUT,
//!         0,
//!         &CreateWindowAux::new().background_pixel(screen.white_pixel),
//!     )?;
//!     conn.map_window(win_id)?;
//!     conn.flush();
//!     loop {
//!         println!("Event: {:?}", conn.wait_for_event()?);
//!     }
//! }
//! ```
//! More examples can be found in the
//! [examples](https://github.com/psychon/x11rb/tree/master/x11rb/examples) directory.
//!
//! ## Feature flags
//!
//! This crate uses [feature
//! flags](https://doc.rust-lang.org/cargo/reference/manifest.html#the-features-section) to reduce
//! the amount of compiled code. There are two kinds of feature flags available:
//!
//! * Feature flags for specific X11 extensions
//! * Feature flags for additional functionality
//!
//! ### Feature flags for specific X11 extensions
//!
//! By default, only the core X11 protocol and X11 extensions that are needed internally are
//! enabled. These are the `bigreq`, `ge` and `xc_misc` extensions. Further extensions need to be
//! explicitly enabled via their feature flag:
//!
//! `composite`, `damage`, `dpms`, `dri2`, `dri3`, `glx`, `present`, `randr`, `record`, `render`,
//! `res`, `screensaver`, `shape`, `shm`, `sync`, `xevie`, `xf86dri`, `xf86vidmode`, `xfixes`,
//! `xinerama`, `xinput`, `xkb`, `xprint`, `xselinux`, `xtest`, `xv`, `xvmc`.
//!
//! If you want to take the "I do not want to think about this"-approach, you can enable the
//! `all-extensions` feature to just enable, well, all extensions.
//!
//! ### Feature flags for additional functionality
//!
//! Additionally, the following flags exist:
//! * `allow-unsafe-code`: Enable features that require `unsafe`. Without this flag,
//!   [`xcb_ffi::XCBConnection`] and some support code for it are unavailable.
//! * `cursor`: Enable the code in [cursor] for loading cursor files.
//! * `resource_manager`: Enable the code in [resource_manager] for loading and querying the
//!   X11 resource database.
//! * `image`: Enable the code in [image] for working with pixel image data.
//! * `dl-libxcb`: Enabling this feature will prevent from libxcb being linked to the
//!   resulting executable. Instead libxcb will be dynamically loaded at runtime.
//!   This feature adds the [`xcb_ffi::load_libxcb`] function, that allows to load
//!   libxcb and check for success or failure.
//! * `extra-traits`: Enable some additional traits for generated code, like `Eq`, `Ord` and
//!   `Hash`. This is not needed by default and adds a large amount of code that bloats codegen
//!   time
//! * `request-parsing`: Add the ability to parse X11 requests. Not normally needed.
//! * `extra-traits`: Implement extra traits for X11 types. This improves the output of the `Debug`
//!   impl and adds `PartialEq`, `Eq`, `PartialOrd`, `Ord`, and `Hash` where possible.
//!
//! # Integrating x11rb with an Event Loop
//!
//! The [event_loop_integration](event_loop_integration/index.html) module contains some hints for
//! integrating x11rb with an event loop as doc comments.

// A list of lints that are only #![deny] and not the stronger #![forbid]. Each one has a comment
// explaining why it gets the weaker treatment.
#![deny(
    // Contains unreachable_code and "?" generates an #[allow] for this
    unused,
    // #[derive] generates an #[allow] for this; not part of "unused"
    unused_qualifications,
    // Not everything in x11rb::protocol has doc comments
    missing_docs,
)]
#![forbid(
    missing_copy_implementations,
    missing_debug_implementations,
    rustdoc::private_doc_tests,
    rust_2018_idioms,
    //single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unused_must_use,
    unused_results,
    clippy::cast_lossless,
    clippy::needless_pass_by_value,
)]
#![cfg_attr(not(feature = "allow-unsafe-code"), forbid(unsafe_code))]

// Only contains documentation, but no "actual rust"
pub mod event_loop_integration;

/// Reexports of dependencies
pub mod reexports {
    pub use x11rb_protocol;
}

mod tracing;
pub(crate) use crate::tracing::*;

pub mod utils;
#[cfg(feature = "allow-unsafe-code")]
pub mod xcb_ffi;
#[macro_use]
pub mod x11_utils;
pub mod connection;
pub mod cookie;
#[cfg(feature = "cursor")]
pub mod cursor;
pub mod errors;
pub mod extension_manager;
#[cfg(feature = "image")]
pub mod image;
pub mod properties;
pub mod rust_connection;
pub mod wrapper;
#[rustfmt::skip]
#[allow(missing_docs)]
pub mod protocol;
#[cfg(feature = "resource_manager")]
pub mod resource_manager;
#[cfg(test)]
mod test;

use errors::ConnectError;
use protocol::xproto::{Keysym, Timestamp};
use std::ffi::OsString;

/// Establish a new connection to an X11 server.
///
/// This function is identical to
/// [RustConnection::connect](crate::rust_connection::RustConnection::connect).
pub fn connect(
    dpy_name: Option<&str>,
) -> Result<(rust_connection::RustConnection, usize), ConnectError> {
    rust_connection::RustConnection::connect(dpy_name)
}

/// The universal null resource or null atom parameter value for many core X requests
pub const NONE: u32 = 0;

/// This constant can be used for many parameters in `create_window`
pub const COPY_FROM_PARENT: u32 = 0;

/// This constant can be used for the depth parameter in `create_window`. It indicates to use the
/// parent window's depth.
pub const COPY_DEPTH_FROM_PARENT: u8 = 0;

/// This constant can be used for the class parameter in `create_window`. It indicates to use the
/// parent window's class.
pub const COPY_CLASS_FROM_PARENT: u16 = 0;

/// This constant can be used in most request that take a timestamp argument
pub const CURRENT_TIME: Timestamp = 0;

/// This constant can be used to fill unused entries in `Keysym` tables
pub const NO_SYMBOL: Keysym = 0;

#[cfg(not(unix))]
fn hostname() -> OsString {
    gethostname::gethostname()
}

#[cfg(unix)]
fn hostname() -> OsString {
    use std::os::unix::ffi::OsStringExt;

    OsString::from_vec(rustix::system::uname().nodename().to_bytes().to_vec())
}
