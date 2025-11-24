# Crossbeam Epoch

[![Build Status](https://github.com/crossbeam-rs/crossbeam/workflows/CI/badge.svg)](
https://github.com/crossbeam-rs/crossbeam/actions)
[![License](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-blue.svg)](
https://github.com/crossbeam-rs/crossbeam/tree/master/crossbeam-epoch#license)
[![Cargo](https://img.shields.io/crates/v/crossbeam-epoch.svg)](
https://crates.io/crates/crossbeam-epoch)
[![Documentation](https://docs.rs/crossbeam-epoch/badge.svg)](
https://docs.rs/crossbeam-epoch)
[![Rust 1.61+](https://img.shields.io/badge/rust-1.61+-lightgray.svg)](
https://www.rust-lang.org)
[![chat](https://img.shields.io/discord/569610676205781012.svg?logo=discord)](https://discord.com/invite/JXYwgWZ)

This crate provides epoch-based garbage collection for building concurrent data structures.

When a thread removes an object from a concurrent data structure, other threads
may be still using pointers to it at the same time, so it cannot be destroyed
immediately. Epoch-based GC is an efficient mechanism for deferring destruction of
shared objects until no pointers to them can exist.

Everything in this crate except the global GC can be used in `no_std` environments, provided that
`alloc` feature is enabled.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
crossbeam-epoch = "0.9"
```

## Compatibility

Crossbeam Epoch supports stable Rust releases going back at least six months,
and every time the minimum supported Rust version is increased, a new minor
version is released. Currently, the minimum supported Rust version is 1.61.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
