[![crates.io](https://img.shields.io/crates/v/dirs-sys.svg?style=for-the-badge)](https://crates.io/crates/dirs-sys)
[![API documentation](https://img.shields.io/docsrs/dirs-sys/latest?style=for-the-badge)](https://docs.rs/dirs-sys/)
![as-is](https://img.shields.io/badge/maintenance-as--is-yellow.svg?style=for-the-badge)

# `dirs-sys`

System-level helper functions for the [`dirs`](https://github.com/dirs-dev/dirs-rs)
and [`directories`](https://github.com/dirs-dev/directories-rs) crates.

_Do not use this library directly, use [`dirs`](https://github.com/dirs-dev/dirs-rs)
or [`directories`](https://github.com/dirs-dev/directories-rs)._

## Compatibility

This crate only exists to facilitate code sharing between [`dirs`](https://github.com/dirs-dev/dirs-rs)
and [`directories`](https://github.com/dirs-dev/directories-rs).

There are no compatibility guarantees whatsoever.
Functions may change or disappear without warning or any kind of deprecation period.  

## Platforms

This library is written in Rust, and supports Linux, Redox, macOS and Windows.
Other platforms are also supported; they use the Linux conventions.

## Build

It's possible to cross-compile this library if the necessary toolchains are installed with rustup.
This is helpful to ensure a change has not broken compilation on a different platform.

The following commands will build this library on Linux, macOS and Windows:

```
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
