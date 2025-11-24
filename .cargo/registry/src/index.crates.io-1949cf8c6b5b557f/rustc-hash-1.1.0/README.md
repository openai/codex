# rustc-hash

[![crates.io](https://img.shields.io/crates/v/rustc-hash.svg)](https://crates.io/crates/rustc-hash)
[![Documentation](https://docs.rs/rustc-hash/badge.svg)](https://docs.rs/rustc-hash)

A speedy hash algorithm used within rustc. The hashmap in liballoc by
default uses SipHash which isn't quite as speedy as we want. In the
compiler we're not really worried about DOS attempts, so we use a fast
non-cryptographic hash.

This is the same as the algorithm used by Firefox -- which is a
homespun one not based on any widely-known algorithm -- though
modified to produce 64-bit hash values instead of 32-bit hash
values. It consistently out-performs an FNV-based hash within rustc
itself -- the collision rate is similar or slightly worse than FNV,
but the speed of the hash function itself is much higher because it
works on up to 8 bytes at a time.

## Usage

```rust
use rustc_hash::FxHashMap;

let mut map: FxHashMap<u32, u32> = FxHashMap::default();
map.insert(22, 44);
```

### `no_std`

This crate can be used as a `no_std` crate by disabling the `std`
feature, which is on by default, as follows:

```toml
rustc-hash = { version = "1.0", default-features = false }
```

In this configuration, `FxHasher` is the only export, and the
`FxHashMap`/`FxHashSet` type aliases are omitted.
