# [RustCrypto]: zeroize

[![Crate][crate-image]][crate-link]
[![Docs][docs-image]][docs-link]
![Apache 2.0/MIT Licensed][license-image]
![MSRV][rustc-image]
[![Build Status][build-image]][build-link]

Securely zero memory (a.k.a. [zeroize]) while avoiding compiler optimizations.

This crate implements a portable approach to securely zeroing memory using
techniques which guarantee they won't be "optimized away" by the compiler.

The [`Zeroize` trait] is the crate's primary API.

[Documentation]

## About

[Zeroing memory securely is hard] - compilers optimize for performance, and
in doing so they love to "optimize away" unnecessary zeroing calls. There are
many documented "tricks" to attempt to avoid these optimizations and ensure
that a zeroing routine is performed reliably.

This crate isn't about tricks: it uses [core::ptr::write_volatile]
and [core::sync::atomic] memory fences to provide easy-to-use, portable
zeroing behavior which works on all of Rust's core number types and slices
thereof, implemented in pure Rust with no usage of FFI or assembly.

- No insecure fallbacks!
- No dependencies!
- No FFI or inline assembly! **WASM friendly** (and tested)!
- `#![no_std]` i.e. **embedded-friendly**!
- No functionality besides securely zeroing memory!
- (Optional) Custom derive support for zeroing complex structures

## Minimum Supported Rust Version

Rust **1.60** or newer.

In the future, we reserve the right to change MSRV (i.e. MSRV is out-of-scope
for this crate's SemVer guarantees), however when we do it will be accompanied by
a minor version bump.

## License

Licensed under either of:

* [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
* [MIT license](http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[//]: # (badges)

[crate-image]: https://img.shields.io/crates/v/zeroize.svg
[crate-link]: https://crates.io/crates/zeroize
[docs-image]: https://docs.rs/zeroize/badge.svg
[docs-link]: https://docs.rs/zeroize/
[license-image]: https://img.shields.io/badge/license-Apache2.0/MIT-blue.svg
[rustc-image]: https://img.shields.io/badge/rustc-1.60+-blue.svg
[build-image]: https://github.com/RustCrypto/utils/actions/workflows/zeroize.yml/badge.svg
[build-link]: https://github.com/RustCrypto/utils/actions/workflows/zeroize.yml

[//]: # (general links)

[RustCrypto]: https://github.com/RustCrypto
[zeroize]: https://en.wikipedia.org/wiki/Zeroisation
[`Zeroize` trait]: https://docs.rs/zeroize/latest/zeroize/trait.Zeroize.html
[Documentation]: https://docs.rs/zeroize/
[Zeroing memory securely is hard]: http://www.daemonology.net/blog/2014-09-04-how-to-zero-a-buffer.html
[core::ptr::write_volatile]: https://doc.rust-lang.org/core/ptr/fn.write_volatile.html
[core::sync::atomic]: https://doc.rust-lang.org/stable/core/sync/atomic/index.html
[good cryptographic hygiene]: https://github.com/veorq/cryptocoding#clean-memory-of-secret-data
