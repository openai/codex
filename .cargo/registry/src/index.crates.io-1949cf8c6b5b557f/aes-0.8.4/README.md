# RustCrypto: Advanced Encryption Standard (AES)

[![crate][crate-image]][crate-link]
[![Docs][docs-image]][docs-link]
![Apache2/MIT licensed][license-image]
![Rust Version][rustc-image]
[![Project Chat][chat-image]][chat-link]
[![Build Status][build-image]][build-link]
[![Downloads][downloads-image]][crate-link]
[![HAZMAT][hazmat-image]][hazmat-link]

Pure Rust implementation of the [Advanced Encryption Standard (AES)][1].

This crate implements the low-level AES block function, and is intended
for use for implementing higher-level constructions *only*. It is NOT
intended for direct use in applications.

[Documentation][docs-link]

<img src="https://raw.githubusercontent.com/RustCrypto/media/85f62bb/img/block-ciphers/aes-round.svg" width="480px">

## Security

### ⚠️ Warning: [Hazmat!][hazmat-link]

This crate does not ensure ciphertexts are authentic (i.e. by using a MAC to
verify ciphertext integrity), which can lead to serious vulnerabilities
if used incorrectly!

To avoid this, use an [AEAD][2] mode based on AES, such as [AES-GCM][3] or [AES-GCM-SIV][4].
See the [RustCrypto/AEADs][5] repository for more information.

USE AT YOUR OWN RISK!

### Notes

This crate has received one [security audit by NCC Group][6], with no significant
findings. We would like to thank [MobileCoin][7] for funding the audit.

All implementations contained in the crate are designed to execute in constant
time, either by relying on hardware intrinsics (i.e. AES-NI on x86/x86_64), or
using a portable implementation based on bitslicing.

## Minimum Supported Rust Version

Rust **1.56** or higher.

Minimum supported Rust version can be changed in future releases, but it will
be done with a minor version bump.

## SemVer Policy

- All on-by-default features of this library are covered by SemVer
- MSRV is considered exempt from SemVer as noted above

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

[crate-image]: https://img.shields.io/crates/v/aes.svg
[crate-link]: https://crates.io/crates/aes
[docs-image]: https://docs.rs/aes/badge.svg
[docs-link]: https://docs.rs/aes/
[license-image]: https://img.shields.io/badge/license-Apache2.0/MIT-blue.svg
[rustc-image]: https://img.shields.io/badge/rustc-1.56+-blue.svg
[chat-image]: https://img.shields.io/badge/zulip-join_chat-blue.svg
[chat-link]: https://rustcrypto.zulipchat.com/#narrow/stream/260039-block-ciphers
[build-image]: https://github.com/RustCrypto/block-ciphers/workflows/aes/badge.svg?branch=master&event=push
[build-link]: https://github.com/RustCrypto/block-ciphers/actions?query=workflow%3Aaes
[downloads-image]: https://img.shields.io/crates/d/aes.svg
[hazmat-image]: https://img.shields.io/badge/crypto-hazmat%E2%9A%A0-red.svg
[hazmat-link]: https://github.com/RustCrypto/meta/blob/master/HAZMAT.md

[//]: # (general links)

[1]: https://en.wikipedia.org/wiki/Advanced_Encryption_Standard
[2]: https://en.wikipedia.org/wiki/Authenticated_encryption
[3]: https://github.com/RustCrypto/AEADs/tree/master/aes-gcm
[4]: https://github.com/RustCrypto/AEADs/tree/master/aes-gcm-siv
[5]: https://github.com/RustCrypto/AEADs
[6]: https://research.nccgroup.com/2020/02/26/public-report-rustcrypto-aes-gcm-and-chacha20poly1305-implementation-review/
[7]: https://www.mobilecoin.com/
