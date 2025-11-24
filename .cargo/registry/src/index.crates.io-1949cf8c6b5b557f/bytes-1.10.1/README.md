# Bytes

A utility library for working with bytes.

[![Crates.io][crates-badge]][crates-url]
[![Build Status][ci-badge]][ci-url]

[crates-badge]: https://img.shields.io/crates/v/bytes.svg
[crates-url]: https://crates.io/crates/bytes
[ci-badge]: https://github.com/tokio-rs/bytes/workflows/CI/badge.svg
[ci-url]: https://github.com/tokio-rs/bytes/actions

[Documentation](https://docs.rs/bytes)

## Usage

To use `bytes`, first add this to your `Cargo.toml`:

```toml
[dependencies]
bytes = "1"
```

Next, add this to your crate:

```rust
use bytes::{Bytes, BytesMut, Buf, BufMut};
```

## no_std support

To use `bytes` with no_std environment, disable the (enabled by default) `std` feature.

```toml
[dependencies]
bytes = { version = "1", default-features = false }
```

To use `bytes` with no_std environment without atomic CAS, such as thumbv6m, you also need to enable
the `extra-platforms` feature. See the [documentation for the `portable-atomic`
crate](https://docs.rs/portable-atomic) for more information.

The MSRV when `extra-platforms` feature is enabled depends on the MSRV of `portable-atomic`.

## Serde support

Serde support is optional and disabled by default. To enable use the feature `serde`.

```toml
[dependencies]
bytes = { version = "1", features = ["serde"] }
```

The MSRV when `serde` feature is enabled depends on the MSRV of `serde`.

## Building documentation

When building the `bytes` documentation the `docsrs` option should be used, otherwise
feature gates will not be shown. This requires a nightly toolchain:

```
RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc
```

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `bytes` by you, shall be licensed as MIT, without any additional
terms or conditions.
