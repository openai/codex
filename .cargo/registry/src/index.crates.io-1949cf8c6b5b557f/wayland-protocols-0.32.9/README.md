[![crates.io](https://img.shields.io/crates/v/wayland-protocols.svg)](https://crates.io/crates/wayland-protocols)
[![docs.rs](https://docs.rs/wayland-protocols/badge.svg)](https://docs.rs/wayland-protocols)
[![Continuous Integration](https://github.com/Smithay/wayland-rs/workflows/Continuous%20Integration/badge.svg)](https://github.com/Smithay/wayland-rs/actions?query=workflow%3A%22Continuous+Integration%22)
[![codecov](https://codecov.io/gh/Smithay/wayland-rs/branch/master/graph/badge.svg)](https://codecov.io/gh/Smithay/wayland-rs)

# wayland-protocols

This crate provides Wayland object definitions for the official Wayland protocol extensions.
It is meant to be used in addition to `wayland-client` or `wayland-server`.

This crate follows the ["wayland-protocols"](https://gitlab.freedesktop.org/wayland/wayland-protocols)
extensions repository.

The provided objects are controlled by cargo features:

- the `client` and `server` cargo features respectively enable the generation of client-side
  and server-side objects
- the `staging` enable the generation of protocols in the staging process and will soon become stable.
- the `unstable` enable the generation of not-yet-stabilized protocols

For other protocols, see also:

- [wayland-protocols-wlr](https://crates.io/crates/wayland-protocols-wlr) for the WLR set of protocol extensions