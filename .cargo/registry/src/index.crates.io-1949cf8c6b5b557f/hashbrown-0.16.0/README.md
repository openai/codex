hashbrown
=========

[![Build Status](https://github.com/rust-lang/hashbrown/actions/workflows/rust.yml/badge.svg)](https://github.com/rust-lang/hashbrown/actions)
[![Crates.io](https://img.shields.io/crates/v/hashbrown.svg)](https://crates.io/crates/hashbrown)
[![Documentation](https://docs.rs/hashbrown/badge.svg)](https://docs.rs/hashbrown)
[![Rust](https://img.shields.io/badge/rust-1.65.0%2B-blue.svg?maxAge=3600)](https://github.com/rust-lang/hashbrown)

This crate is a Rust port of Google's high-performance [SwissTable] hash
map, adapted to make it a drop-in replacement for Rust's standard `HashMap`
and `HashSet` types.

The original C++ version of SwissTable can be found [here], and this
[CppCon talk] gives an overview of how the algorithm works.

Since Rust 1.36, this is now the `HashMap` implementation for the Rust standard
library. However you may still want to use this crate instead since it works
in environments without `std`, such as embedded systems and kernels.

[SwissTable]: https://abseil.io/blog/20180927-swisstables
[here]: https://github.com/abseil/abseil-cpp/blob/master/absl/container/internal/raw_hash_set.h
[CppCon talk]: https://www.youtube.com/watch?v=ncHmEUmJZf4

## [Change log](CHANGELOG.md)

## Features

- Drop-in replacement for the standard library `HashMap` and `HashSet` types.
- Uses [foldhash](https://github.com/orlp/foldhash) as the default hasher, which is much faster than SipHash.
  However, foldhash does *not provide the same level of HashDoS resistance* as SipHash, so if that is important to you, you might want to consider using a different hasher.
- Around 2x faster than the previous standard library `HashMap`.
- Lower memory usage: only 1 byte of overhead per entry instead of 8.
- Compatible with `#[no_std]` (but requires a global allocator with the `alloc` crate).
- Empty hash maps do not allocate any memory.
- SIMD lookups to scan multiple hash entries in parallel.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
hashbrown = "0.15"
```

Then:

```rust
use hashbrown::HashMap;

let mut map = HashMap::new();
map.insert(1, "one");
```
## Flags
This crate has the following Cargo features:

- `nightly`: Enables nightly-only features including: `#[may_dangle]`.
- `serde`: Enables serde serialization support.
- `rayon`: Enables rayon parallel iterator support.
- `equivalent`: Allows comparisons to be customized with the `Equivalent` trait. (enabled by default)
- `raw-entry`: Enables access to the deprecated `RawEntry` API.
- `inline-more`: Adds inline hints to most functions, improving run-time performance at the cost
  of compilation time. (enabled by default)
- `default-hasher`: Compiles with foldhash as default hasher. (enabled by default)
- `allocator-api2`: Enables support for allocators that support `allocator-api2`. (enabled by default)

## License

Licensed under either of:

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
