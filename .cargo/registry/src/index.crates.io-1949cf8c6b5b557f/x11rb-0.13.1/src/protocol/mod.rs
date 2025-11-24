// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the X11 protocol.
//!
//! Each sub-module of this module corresponds to one X11 extension. It contains all the
//! definitions from that extension. The core X11 protocol is in [`xproto`](xproto/index.html).

// Clippy does not like some names from the XML.
#![allow(clippy::upper_case_acronyms)]
// This is not easy to fix, so ignore it.
#![allow(clippy::needless_borrow, clippy::needless_lifetimes)]
pub mod xproto;
pub mod bigreq;
#[cfg(feature = "composite")]
pub mod composite;
#[cfg(feature = "damage")]
pub mod damage;
#[cfg(feature = "dbe")]
pub mod dbe;
#[cfg(feature = "dpms")]
pub mod dpms;
#[cfg(feature = "dri2")]
pub mod dri2;
#[cfg(feature = "dri3")]
pub mod dri3;
pub mod ge;
#[cfg(feature = "glx")]
pub mod glx;
#[cfg(feature = "present")]
pub mod present;
#[cfg(feature = "randr")]
pub mod randr;
#[cfg(feature = "record")]
pub mod record;
#[cfg(feature = "render")]
pub mod render;
#[cfg(feature = "res")]
pub mod res;
#[cfg(feature = "screensaver")]
pub mod screensaver;
#[cfg(feature = "shape")]
pub mod shape;
#[cfg(feature = "shm")]
pub mod shm;
#[cfg(feature = "sync")]
pub mod sync;
pub mod xc_misc;
#[cfg(feature = "xevie")]
pub mod xevie;
#[cfg(feature = "xf86dri")]
pub mod xf86dri;
#[cfg(feature = "xf86vidmode")]
pub mod xf86vidmode;
#[cfg(feature = "xfixes")]
pub mod xfixes;
#[cfg(feature = "xinerama")]
pub mod xinerama;
#[cfg(feature = "xinput")]
pub mod xinput;
#[cfg(feature = "xkb")]
pub mod xkb;
#[cfg(feature = "xprint")]
pub mod xprint;
#[cfg(feature = "xselinux")]
pub mod xselinux;
#[cfg(feature = "xtest")]
pub mod xtest;
#[cfg(feature = "xv")]
pub mod xv;
#[cfg(feature = "xvmc")]
pub mod xvmc;

pub use x11rb_protocol::protocol::Request;
pub use x11rb_protocol::protocol::Reply;
pub use x11rb_protocol::protocol::ErrorKind;
pub use x11rb_protocol::protocol::Event;
