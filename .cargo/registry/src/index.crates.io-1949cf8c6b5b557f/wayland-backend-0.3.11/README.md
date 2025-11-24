[![crates.io](https://img.shields.io/crates/v/wayland-backend.svg)](https://crates.io/crates/wayland-backend)
[![docs.rs](https://docs.rs/wayland-backend/badge.svg)](https://docs.rs/wayland-backend)
[![Continuous Integration](https://github.com/Smithay/wayland-rs/workflows/Continuous%20Integration/badge.svg)](https://github.com/Smithay/wayland-rs/actions?query=workflow%3A%22Continuous+Integration%22)
[![codecov](https://codecov.io/gh/Smithay/wayland-rs/branch/master/graph/badge.svg)](https://codecov.io/gh/Smithay/wayland-rs)

# wayland-backend

Backend API for wayland crates

This crate provide low-level APIs for interacting with the Wayland protocol, both client-side
and server-side. For higher-level interfaces, see the `wayland-client` and `wayland-server` crates.

Two possible backends are provided by this crate: the system backend ([`sys`] module)
which relies on the system-provided wayland libraries, and the rust backend ([`rs`] module)
which is an alternative rust implementation of the protocol. The rust backend is always
available, and the system backend is controlled by the `client_system` and `server_system`
cargo features. The `dlopen` cargo feature ensures that the system wayland libraries are loaded
dynamically at runtime, so that your executable does not link them and can gracefully handle
their absence (for example by falling back to X11).

Additionnaly the default backends are reexported as toplevel `client` and `server` modules
in this crate. For both client and server, the default backend is the system one if the
associated cargo feature is enabled, and the rust one otherwise. Using these reexports is the
recommended way to use the crate.

Both backends have the exact same API, except that the system backend additionnaly provides
functions related to FFI.