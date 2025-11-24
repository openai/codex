# byteorder-lite

[![crates.io](https://img.shields.io/crates/v/byteorder-lite.svg)](https://crates.io/crates/byteorder-lite)
[![Documentation](https://docs.rs/byteorder-lite/badge.svg)](https://docs.rs/byteorder-lite)
[![Build Status](https://github.com/image-rs/byteorder-lite/workflows/ci/badge.svg)](https://github.com/image-rs/byteorder-lite/actions)

This crate is a fork of the `byteorder` crate which sets
`#![forbid(unsafe_code)]`. It includes all traits and most methods from the
original crate, but the `ReadBytesExt::read_*_into` family of methods had to be
removed because they currently cannot be implemented without unsafe code.

`byteorder-lite` is not affiliated with the main `byteorder` crate.
