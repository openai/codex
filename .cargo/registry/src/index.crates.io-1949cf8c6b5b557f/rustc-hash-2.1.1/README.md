# rustc-hash

[![crates.io](https://img.shields.io/crates/v/rustc-hash.svg)](https://crates.io/crates/rustc-hash)
[![Documentation](https://docs.rs/rustc-hash/badge.svg)](https://docs.rs/rustc-hash)

A speedy, non-cryptographic hashing algorithm used by `rustc`.
The [hash map in `std`](https://doc.rust-lang.org/std/collections/struct.HashMap.html) uses SipHash by default, which provides resistance against DOS attacks.
These attacks aren't a concern in the compiler so we prefer to use a quicker,
non-cryptographic hash algorithm.

The original hash algorithm provided by this crate was one taken from Firefox,
hence the hasher it provides is called FxHasher. This name is kept for backwards
compatibility, but the underlying hash has since been replaced. The current
design for the hasher is a polynomial hash finished with a single bit rotation,
together with a wyhash-inspired compression function for strings/slices, both
designed by Orson Peters.

For `rustc` we have tried many different hashing algorithms. Hashing speed is
critical, especially for single integers. Spending more CPU cycles on a higher
quality hash does not reduce hash collisions enough to make the compiler faster
on real-world benchmarks.

## Usage

This crate provides `FxHashMap` and `FxHashSet` as collections.
They are simply type aliases for their `std::collection` counterparts using the Fx hasher.

```rust
use rustc_hash::FxHashMap;

let mut map: FxHashMap<u32, u32> = FxHashMap::default();
map.insert(22, 44);
```

### `no_std`

The `std` feature is on by default to enable collections.
It can be turned off in `Cargo.toml` like so:

```toml
rustc-hash = { version = "2.1", default-features = false }
```
