# [RustCrypto]: `zeroize_derive`

[![Crate][crate-image]][crate-link]
![Apache 2.0 Licensed/MIT][license-image]
![MSRV][rustc-image]
[![Build Status][build-image]][build-link]

Custom derive support for [zeroize]: a crate for securely zeroing memory
while avoiding compiler optimizations.

This crate isn't intended to be used directly.
See [zeroize] crate for documentation.

## Minimum Supported Rust Version

Rust **1.56** or newer.

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

[crate-image]: https://img.shields.io/crates/v/zeroize_derive.svg
[crate-link]: https://crates.io/crates/zeroize_derive
[license-image]: https://img.shields.io/badge/license-Apache2.0/MIT-blue.svg
[rustc-image]: https://img.shields.io/badge/rustc-1.56+-blue.svg
[build-image]: https://github.com/RustCrypto/utils/actions/workflows/zeroize.yml/badge.svg
[build-link]: https://github.com/RustCrypto/utils/actions/workflows/zeroize.yml

[//]: # (general links)

[RustCrypto]: https://github.com/RustCrypto
[zeroize]: https://github.com/RustCrypto/utils/tree/master/zeroize
