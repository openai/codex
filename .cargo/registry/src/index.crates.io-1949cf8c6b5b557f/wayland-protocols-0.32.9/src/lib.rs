//! This crate provides bindings to the official wayland protocol extensions
//! provided in <https://gitlab.freedesktop.org/wayland/wayland-protocols>
//!
//! These bindings are built on top of the crates wayland-client and wayland-server.
//!
//! Each protocol module contains a `client` and a `server` submodules, for each side of the
//! protocol. The creation of these modules (and the dependency on the associated crate) is
//! controlled by the two cargo features `client` and `server`.
//!
//! ## Protocol categories
//!
//! The protocols provided in this crate are grouped in 4 main categories:
//!
//! - The [`wp`] module contains general purpose wayland protocols
//! - The [`xdg`] module contains protocols specifically related to window management
//! - The [`xwayland`] module contains protocols used by xwayland.
//! - The [`ext`] module contains protocols that do not fit into the three previous categories.
//!
//! ## Staging protocols
//!
//! The cargo feature `staging` enables the generation of the staging protocols.
//!
//! These protocols are ready for wider adoption and clients and compositors are encouraged to
//! implement staging protocol extensions where a protocol's functionality is desired.
//!
//! Although these protocols should be stable, the protocols may still be completely replaced in a new
//! major version or with a completely different protocol.
//!
//! ## Unstable protocols
//!
//! The `wayland-protocols` project previously had a notion of "unstable protocols" representing protocols
//! that are still being worked on and evolving. These protocols are recognized by the use of the prefix `z`
//! in their interface names.
//!
//! This category has now been deprecated and is no longer supposed to be used, however several protocols
//! are still under that umbrella. We can expect them to be replaced by staging and stable protocols in the
//! long term, but in the meantime you can enable them with the `unstable` cargo feature.
//!
//! ## Other protocols
//!
//! Additionally, more protocol extensions are provided here:
//! - [wayland-protocols-wlr](https://docs.rs/wayland-protocols-wlr)
//! - [wayland-protocols-plasma](https://docs.rs/wayland-protocols-plasma)
//! - [wayland-protocols-misc](https://docs.rs/wayland-protocols-misc)

#![warn(missing_docs)]
#![forbid(improper_ctypes, unsafe_op_in_unsafe_fn)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[macro_use]
mod protocol_macro;

pub mod ext;
pub mod wp;
pub mod xdg;
pub mod xwayland;
