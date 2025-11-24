# allocator-api2

[![crates](https://img.shields.io/crates/v/allocator-api2.svg?style=for-the-badge&label=allocator-api2)](https://crates.io/crates/allocator-api2)
[![docs](https://img.shields.io/badge/docs.rs-allocator--api2-66c2a5?style=for-the-badge&labelColor=555555&logoColor=white)](https://docs.rs/allocator-api2)
[![actions](https://img.shields.io/github/actions/workflow/status/zakarumych/allocator-api2/badge.yml?branch=main&style=for-the-badge)](https://github.com/zakarumych/allocator-api2/actions/workflows/badge.yml)
[![MIT/Apache](https://img.shields.io/badge/license-MIT%2FApache-blue.svg?style=for-the-badge)](COPYING)
![loc](https://img.shields.io/tokei/lines/github/zakarumych/allocator-api2?style=for-the-badge)

This crate mirrors types and traits from Rust's unstable [`allocator_api`]
The intention of this crate is to serve as substitution for actual thing
for libs when build on stable and beta channels.
The target users are library authors who implement allocators or collection types
that use allocators, or anyone else who wants using [`allocator_api`]

The crate should be frequently updated with minor version bump.
When [`allocator_api`] is stable this crate will get version `1.0` and simply
re-export from `core`, `alloc` and `std`.

The code is mostly verbatim copy from rust repository.
Mostly attributes are removed.

## Usage

This paragraph describes how to use this crate correctly to ensure
compatibility and interoperability on both stable and nightly channels.

If you are writing a library that interacts with allocators API, you can
add this crate as a dependency and use the types and traits from this
crate instead of the ones in `core` or `alloc`.
This will allow your library to compile on stable and beta channels.

Your library *MAY* provide a feature that will enable "allocator-api2/nightly".
When this feature is enabled, your library *MUST* enable
unstable `#![feature(allocator_api)]` or it may not compile.
If feature is not provided, your library may not be compatible with the
rest of the users and cause compilation errors on nightly channel
when some other crate enables "allocator-api2/nightly" feature.

# Minimal Supported Rust Version (MSRV)

This crate is guaranteed to compile on stable Rust 1.63 and up.
A feature "fresh-rust" bumps the MSRV to unspecified higher version, but should be compatible with
at least few latest stable releases. The feature enables some additional functionality:

* `CStr` without "std" feature

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.


[`allocator_api`]: https://doc.rust-lang.org/unstable-book/library-features/allocator-api.html
