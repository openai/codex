# linux-keyutils
[![cargo-badge-lib][]][cargo-lib] [![docs-badge-lib][]][docs-lib] [![license-badge][]][license] [![rust-version-badge][]][rust-version] [![build][]][build-url] [![codecov][]][codecov-url]

Rust interface to the Linux key-management facility. Provides a safe interface around the raw system calls allowing user-space programs to perform key manipulation.

There is a good [cloudflare blog](https://blog.cloudflare.com/the-linux-kernel-key-retention-service-and-why-you-should-use-it-in-your-next-application/) discussing why it should be used.

## Basic Usage

To use `linux-keyutils`, first add this to your `Cargo.toml`:

```toml
[dependencies]
linux-keyutils = "0.2"
```

For more information please view the full [documentation](https://docs.rs/linux-keyutils). There is also a small example program in the [examples directory](examples/keyctl.rs).

## Features

* `#![no_std]` by default.
* For std programs `KeyError` implements `std::error::Error` when the `std` feature of this crate enabled.
* Small footprint, the library only relies on the `libc` and `bitflags` crates.

## License

Licensed under either of the following at your discretion:

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you shall be dual licensed as above, without any
additional terms or conditions.

[//]: # (badges)
[license-badge]: https://img.shields.io/badge/license-MIT/Apache--2.0-lightgray.svg?style=flat-square
[license]: #license
[rust-version-badge]: https://img.shields.io/badge/rust-latest%20stable-blue.svg?style=flat-square
[rust-version]: #rust-version-policy
[cargo-badge-lib]: https://img.shields.io/crates/v/linux-keyutils.svg?style=flat-square&label=linux-keyutils
[cargo-lib]: https://crates.io/crates/linux-keyutils
[docs-badge-lib]: https://img.shields.io/docsrs/linux-keyutils/latest?style=flat-square
[docs-lib]: https://docs.rs/linux-keyutils
[codecov]: https://img.shields.io/codecov/c/github/landhb/linux-keyutils?style=flat-square
[codecov-url]: https://codecov.io/gh/landhb/linux-keyutils
[build]: https://img.shields.io/github/actions/workflow/status/landhb/linux-keyutils/checks.yml?branch=main&style=flat-square
[build-url]: https://github.com/landhb/linux-keyutils/actions?query=workflow%3Achecks
