[![crates.io](https://img.shields.io/crates/v/wayland-scanner.svg)](https://crates.io/crates/wayland-scanner)
[![docs.rs](https://docs.rs/wayland-scanner/badge.svg)](https://docs.rs/wayland-scanner)
[![Continuous Integration](https://github.com/Smithay/wayland-rs/workflows/Continuous%20Integration/badge.svg)](https://github.com/Smithay/wayland-rs/actions?query=workflow%3A%22Continuous+Integration%22)
[![codecov](https://codecov.io/gh/Smithay/wayland-rs/branch/master/graph/badge.svg)](https://codecov.io/gh/Smithay/wayland-rs)

# wayland-scanner

This crate provides procedural macros for generating the rust code associated with a
Wayland XML protocol specification, for use with the `wayland-client`, `wayland-server`
and `wayland-backend` crates.

Before trying to use this crate, you may check if the protocol extension you want to use
is not already exposed in the `wayland-protocols` crate.