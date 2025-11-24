thread_local
============

[![Build Status](https://github.com/Amanieu/thread_local-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/Amanieu/thread_local-rs/actions) [![crates.io](https://img.shields.io/crates/v/thread_local.svg)](https://crates.io/crates/thread_local)

This library provides the `ThreadLocal` type which allow a separate copy of an
object to be used for each thread. This allows for per-object thread-local
storage, unlike the standard library's `thread_local!` macro which only allows
static thread-local storage.

[Documentation](https://docs.rs/thread_local/)

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
thread_local = "1.1"
```

## Minimum Rust version

This crate's minimum supported Rust version (MSRV) is 1.63.0.

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
