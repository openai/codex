# flate2

[![Crates.io](https://img.shields.io/crates/v/flate2.svg?maxAge=2592000)](https://crates.io/crates/flate2)
[![Documentation](https://docs.rs/flate2/badge.svg)](https://docs.rs/flate2)

A streaming compression/decompression library DEFLATE-based streams in Rust.

This crate by default uses the `miniz_oxide` crate, a port of `miniz.c` to pure
Rust. This crate also supports other [backends](#backends), such as the widely
available zlib library or the high-performance zlib-ng library.

Supported formats:

* deflate
* zlib
* gzip

```toml
# Cargo.toml
[dependencies]
flate2 = "1.0"
```

## MSRV (Minimum Supported Rust Version) Policy

This crate supports the current and previous stable versions of the Rust compiler.
For example, if the current stable is 1.80, this crate supports 1.80 and 1.79.

Other compiler versions may work, but failures may not be treated as a `flate2` bug.

The `Cargo.toml` file specifies a `rust-version` for which builds of the current version
passed at some point. This value is indicative only, and may change at any time.

The `rust-version` is a best-effort measured value and is different to the MSRV. The
`rust-version` can be incremented by a PR in order to pass tests, as long as the MSRV
continues to hold. When the `rust-version` increases, the next release should be a minor
version, to allow any affected users to pin to a previous minor version.

## Compression

```rust
use std::io::prelude::*;
use flate2::Compression;
use flate2::write::ZlibEncoder;

fn main() {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(b"foo");
    e.write_all(b"bar");
    let compressed_bytes = e.finish();
}
```

## Decompression

```rust,no_run
use std::io::prelude::*;
use flate2::read::GzDecoder;

fn main() {
    let mut d = GzDecoder::new("...".as_bytes());
    let mut s = String::new();
    d.read_to_string(&mut s).unwrap();
    println!("{}", s);
}
```

## Backends

The default `miniz_oxide` backend has the advantage of only using safe Rust.

If you want maximum performance while still benefiting from a Rust
implementation at the cost of some `unsafe`, you can use `zlib-rs`:

```toml
[dependencies]
flate2 = { version = "1.0.17", features = ["zlib-rs"], default-features = false }
```

### C backends

While zlib-rs is [the fastest overall](https://trifectatech.org/blog/zlib-rs-is-faster-than-c/),
the zlib-ng C library can be slightly faster in certain cases:

```toml
[dependencies]
flate2 = { version = "1.0.17", features = ["zlib-ng"], default-features = false }
```

Note that the `"zlib-ng"` feature works even if some other part of your crate
graph depends on zlib.

However, if you're already using another C or Rust library that depends on
zlib, and you want to avoid including both zlib and zlib-ng, you can use that
for Rust code as well:

```toml
[dependencies]
flate2 = { version = "1.0.17", features = ["zlib"], default-features = false }
```

Or, if you have C or Rust code that depends on zlib and you want to use zlib-ng
via libz-sys in zlib-compat mode, use:

```toml
[dependencies]
flate2 = { version = "1.0.17", features = ["zlib-ng-compat"], default-features = false }
```

Note that when using the `"zlib-ng-compat"` feature, if any crate in your
dependency graph explicitly requests stock zlib, or uses libz-sys directly
without `default-features = false`, you'll get stock zlib rather than zlib-ng.
See [the libz-sys
README](https://github.com/rust-lang/libz-sys/blob/main/README.md) for details.
To avoid that, use the `"zlib-ng"` feature instead.

For compatibility with previous versions of `flate2`, the Cloudflare optimized
version of zlib is available, via the `cloudflare_zlib` feature. It's not as
fast as zlib-ng, but it's faster than stock zlib. It requires an x86-64 CPU with
SSE 4.2 or ARM64 with NEON & CRC. It does not support 32-bit CPUs at all and is
incompatible with mingw. For more information check the [crate
documentation](https://crates.io/crates/cloudflare-zlib-sys). Note that
`cloudflare_zlib` will cause breakage if any other crate in your crate graph
uses another version of zlib/libz.

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
