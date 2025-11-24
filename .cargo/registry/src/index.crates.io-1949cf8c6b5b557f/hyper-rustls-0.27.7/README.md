# hyper-rustls

This is an integration between the [Rustls TLS stack](https://github.com/rustls/rustls) and the
[hyper HTTP library](https://github.com/hyperium/hyper).

[![Build Status](https://github.com/rustls/hyper-rustls/actions/workflows/build.yml/badge.svg)](https://github.com/rustls/hyper-rustls/actions)
[![Crate](https://img.shields.io/crates/v/hyper-rustls.svg)](https://crates.io/crates/hyper-rustls)
[![Documentation](https://docs.rs/hyper-rustls/badge.svg)](https://docs.rs/hyper-rustls)

# Release history

Release history can be found [on GitHub](https://github.com/rustls/hyper-rustls/releases).

# License

hyper-rustls is distributed under the following three licenses:

- Apache License version 2.0.
- MIT license.
- ISC license.

These are included as LICENSE-APACHE, LICENSE-MIT and LICENSE-ISC respectively. You may use this
software under the terms of any of these licenses, at your option.

## Running examples

### server

```bash
cargo run --example server
```

### client

```bash
cargo run --example client "https://docs.rs/hyper-rustls/latest/hyper_rustls/"
```

## Crate features

This crate exposes a number of features to add support for different portions of `hyper-util`,
`rustls`, and other dependencies.

| Feature flag | Enabled by default | Description |
| ------------ | ------------------ | ----------- |
| `aws-lc-rs`  | **yes** | Enables use of the [AWS-LC][aws-lc-rs] backend for [`rustls`][rustls] |
| `http1` | **yes** | Enables HTTP/1 support in [`hyper-util`][hyper-util] |
| `http2` | **no** | Enables HTTP/2 support in [`hyper-util`][hyper-util] |
| `webpki-tokio` | **no** | Uses a compiled-in set of root certificates trusted by Mozilla (via [`webpki-roots`][webpki-roots]) |
| `native-tokio` | **yes** | Use the platform's native certificate store at runtime (via [`rustls-native-certs`][rustls-native-certs]) |
| `rustls-platform-verifier` | **no** | Use the operating system's verifier for certificate verification (via [`rustls-platform-verifier`][rustls-platform-verifier]) |
| `ring` | **no** | Enables use of the [`ring`][ring] backend for [`rustls`][rustls] |
| `tls12` | **yes** | Enables support for TLS 1.2 (only TLS 1.3 supported when disabled) |
| `logging` | **yes** | Enables logging of protocol-level diagnostics and errors via [`log`][log] |
| `fips` | **no** | Enables support for using a FIPS 140-3 compliant backend via AWS-LC (enables `aws-lc-rs` feature) |

[aws-lc-rs]: https://docs.rs/aws-lc-rs
[rustls]: https://docs.rs/rustls
[hyper-util]: https://docs.rs/hyper-util
[webpki-roots]: https://docs.rs/webpki-roots
[rustls-native-certs]: https://docs.rs/rustls-native-certs
[rustls-platform-verifier]: https://docs.rs/rustls-platform-verifier
[ring]: https://docs.rs/ring
[log]: https://docs.rs/log
