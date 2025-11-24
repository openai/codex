[![crates.io](https://img.shields.io/crates/v/wayland-protocols-wlr.svg)](https://crates.io/crates/wayland-protocols-wlr)
[![docs.rs](https://docs.rs/wayland-protocols-wlr/badge.svg)](https://docs.rs/wayland-protocols-wlr)
[![Continuous Integration](https://github.com/Smithay/wayland-rs/workflows/Continuous%20Integration/badge.svg)](https://github.com/Smithay/wayland-rs/actions?query=workflow%3A%22Continuous+Integration%22)
[![codecov](https://codecov.io/gh/Smithay/wayland-rs/branch/master/graph/badge.svg)](https://codecov.io/gh/Smithay/wayland-rs)

# wayland-protocols-wlr

This crate provides Wayland object definitions for the WLR Wayland protocol extensions.
It is meant to be used in addition to `wayland-client` or `wayland-server`.

This crate provides bindings for the ["wlr-protocols"](https://gitlab.freedesktop.org/wlroots/wlr-protocols)
extensions repository.

The provided objects are controlled by the `client` and `server` cargo features, which respectively enable
the generation of client-side and server-side objects