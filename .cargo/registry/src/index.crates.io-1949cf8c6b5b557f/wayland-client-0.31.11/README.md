[![crates.io](https://img.shields.io/crates/v/wayland-client.svg)](https://crates.io/crates/wayland-client)
[![docs.rs](https://docs.rs/wayland-client/badge.svg)](https://docs.rs/wayland-client)
[![Continuous Integration](https://github.com/Smithay/wayland-rs/workflows/Continuous%20Integration/badge.svg)](https://github.com/Smithay/wayland-rs/actions?query=workflow%3A%22Continuous+Integration%22)
[![codecov](https://codecov.io/gh/Smithay/wayland-rs/branch/master/graph/badge.svg)](https://codecov.io/gh/Smithay/wayland-rs)

# wayland-client

Client side API for the Wayland protocol. This crate provides infrastructure for manipulating
Wayland objects, as well as object definitions for the core Wayland protocol. Protocol extensions
can be supported as well by combining this crate with `wayland-protocols`, which provides object
definitions for a large set of extensions.

See the [crate-level documentation](https://docs.rs/wayland-client) for usage explanations.

**Note:** This crate is a low-level interface to the Wayland protocol. If you are looking for a more
battery-included toolkit for writing a Wayland client app, you may consider
[Smithay's Client Toolkit](https://crates.io/crates/smithay-client-toolkit), which is built on top
of it.