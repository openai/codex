[![crates.io](https://img.shields.io/crates/v/wayland-sys.svg)](https://crates.io/crates/wayland-sys)
[![docs.rs](https://docs.rs/wayland-sys/badge.svg)](https://docs.rs/wayland-sys)
[![Continuous Integration](https://github.com/Smithay/wayland-rs/workflows/Continuous%20Integration/badge.svg)](https://github.com/Smithay/wayland-rs/actions?query=workflow%3A%22Continuous+Integration%22)
[![codecov](https://codecov.io/gh/Smithay/wayland-rs/branch/master/graph/badge.svg)](https://codecov.io/gh/Smithay/wayland-rs)

# wayland-sys

This crate provides raw bindings to the system `libwayland-*.so` libraries. If you are
looking for a Rust API over the Wayland protocol, see the `wayland-client` or `wayland-server`
crates instead.

Bindings to the different libraries are enabled by the different cargo features:

- `client` for bindings to `libwayland-client.so`
- `server` for bindings to `libwayland-server.so`
- `cursor` for bindings to `libwayland-cursor.so`
- `egl` for bindings to `libwayland-egl.so`

Furthermore, the `dlopen` cargo feature will switch the library to a mode where, instead
of directly linking to these system libraries, it'll instead try to open them at runtime.
This allows to create binaries that can gracefully handle being run on non-Wayland
environments. In that case the crate should be used with its provided `ffi_dispatch!()`
macro, to support both modes seamlessly.
