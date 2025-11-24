[![crates.io](https://img.shields.io/crates/v/dirs-sys-next.svg)](https://crates.io/crates/dirs-sys-next)
[![API documentation](https://docs.rs/dirs-sys-next/badge.svg)](https://docs.rs/dirs-sys-next/)

# `dirs-sys-next`

**NOTE**: This crate is a fork of once-abandoned `dirs-sys` crate.

_Do not use this library directly, use [`dirs-next`] or [`directories-next`]._

## Compatibility

This crate only exists to facilitate code sharing between [`dirs-next`]
and [`directories-next`].

There are no compatibility guarantees whatsoever.
Functions may change or disappear without warning or any kind of deprecation period.

## Platforms

This library is written in Rust, and supports Linux, Redox, macOS and Windows.
Other platforms are also supported; they use the Linux conventions.

## Minimum Rust version policy

The minimal required version of Rust is `1.34.0`^.

We may bump the Rust version in major and minor releases (`x`/`y` in `x.y.z`).
Changing the Rust version will be written in the CHANGELOG.

^ Except for Redox, where the Rust version depends on the
[`redox_users`](https://crates.io/crates/redox_users) crate.

## Build

It's possible to cross-compile this library if the necessary toolchains are installed with rustup.
This is helpful to ensure a change has not broken compilation on a different platform.

The following commands will build this library on Linux, macOS and Windows:

```console
cargo build --target=x86_64-unknown-linux-gnu
cargo build --target=x86_64-pc-windows-gnu
cargo build --target=x86_64-apple-darwin
cargo build --target=x86_64-unknown-redox
```

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[`dirs-next`]: https://github.com/xdg-rs/dirs
[`directories-next`]: https://github.com/xdg-rs/dirs/tree/master/directories
