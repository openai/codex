hashbrown
=========

[![Build Status](https://github.com/rust-lang/hashbrown/actions/workflows/rust.yml/badge.svg)](https://github.com/rust-lang/hashbrown/actions)
[![Crates.io](https://img.shields.io/crates/v/hashbrown.svg)](https://crates.io/crates/hashbrown)
[![Documentation](https://docs.rs/hashbrown/badge.svg)](https://docs.rs/hashbrown)
[![Rust](https://img.shields.io/badge/rust-1.56.1%2B-blue.svg?maxAge=3600)](https://github.com/rust-lang/hashbrown)

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
- Uses [AHash](https://github.com/tkaitchuck/aHash) as the default hasher, which is much faster than SipHash.
  However, AHash does *not provide the same level of HashDoS resistance* as SipHash, so if that is important to you, you might want to consider using a different hasher.
- Around 2x faster than the previous standard library `HashMap`.
- Lower memory usage: only 1 byte of overhead per entry instead of 8.
- Compatible with `#[no_std]` (but requires a global allocator with the `alloc` crate).
- Empty hash maps do not allocate any memory.
- SIMD lookups to scan multiple hash entries in parallel.

## Performance

Compared to the previous implementation of `std::collections::HashMap` (Rust 1.35).

With the hashbrown default AHash hasher:

| name                    |  oldstdhash ns/iter | hashbrown ns/iter | diff ns/iter |  diff %  | speedup |
|:------------------------|:-------------------:|------------------:|:------------:|---------:|---------|
| insert_ahash_highbits       | 18,865      | 8,020         |               -10,845 | -57.49%  | x 2.35 |
| insert_ahash_random         | 19,711      | 8,019         |               -11,692 | -59.32%  | x 2.46 |
| insert_ahash_serial         | 19,365      | 6,463         |               -12,902 | -66.63%  | x 3.00 |
| insert_erase_ahash_highbits | 51,136      | 17,916        |               -33,220 | -64.96%  | x 2.85 |
| insert_erase_ahash_random   | 51,157      | 17,688        |               -33,469 | -65.42%  | x 2.89 |
| insert_erase_ahash_serial   | 45,479      | 14,895        |               -30,584 | -67.25%  | x 3.05 |
| iter_ahash_highbits         | 1,399       | 1,092         |                  -307 | -21.94%  | x 1.28 |
| iter_ahash_random           | 1,586       | 1,059         |                  -527 | -33.23%  | x 1.50 |
| iter_ahash_serial           | 3,168       | 1,079         |                -2,089 | -65.94%  | x 2.94 |
| lookup_ahash_highbits       | 32,351      | 4,792         |               -27,559 | -85.19%  | x 6.75 |
| lookup_ahash_random         | 17,419      | 4,817         |               -12,602 | -72.35%  | x 3.62 |
| lookup_ahash_serial         | 15,254      | 3,606         |               -11,648 | -76.36%  | x 4.23 |
| lookup_fail_ahash_highbits  | 21,187      | 4,369         |               -16,818 | -79.38%  | x 4.85 |
| lookup_fail_ahash_random    | 21,550      | 4,395         |               -17,155 | -79.61%  | x 4.90 |
| lookup_fail_ahash_serial    | 19,450      | 3,176         |               -16,274 | -83.67%  | x 6.12 |


With the libstd default SipHash hasher:

|name                     |  oldstdhash ns/iter | hashbrown ns/iter | diff ns/iter |  diff %  | speedup |
|:------------------------|:-------------------:|------------------:|:------------:|---------:|---------|
|insert_std_highbits       |19,216      |16,885           |            -2,331    |   -12.13%  | x 1.14 |
|insert_std_random         |19,179      |17,034           |            -2,145    |   -11.18%  | x 1.13 |
|insert_std_serial         |19,462      |17,493           |            -1,969    |   -10.12%  | x 1.11 |
|insert_erase_std_highbits |50,825      |35,847           |            -14,978   |   -29.47%  | x 1.42 |
|insert_erase_std_random   |51,448      |35,392           |            -16,056   |   -31.21%  | x 1.45 |
|insert_erase_std_serial   |87,711      |38,091           |            -49,620   |   -56.57%  | x 2.30 |
|iter_std_highbits         |1,378       |1,159            |            -219      |   -15.89%  | x 1.19 |
|iter_std_random           |1,395       |1,132            |            -263      |   -18.85%  | x 1.23 |
|iter_std_serial           |1,704       |1,105            |            -599      |   -35.15%  | x 1.54 |
|lookup_std_highbits       |17,195      |13,642           |            -3,553    |   -20.66%  | x 1.26 |
|lookup_std_random         |17,181      |13,773           |            -3,408    |   -19.84%  | x 1.25 |
|lookup_std_serial         |15,483      |13,651           |            -1,832    |   -11.83%  | x 1.13 |
|lookup_fail_std_highbits  |20,926      |13,474           |            -7,452    |   -35.61%  | x 1.55 |
|lookup_fail_std_random    |21,766      |13,505           |            -8,261    |   -37.95%  | x 1.61 |
|lookup_fail_std_serial    |19,336      |13,519           |            -5,817    |   -30.08%  | x 1.43 |

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
hashbrown = "0.12"
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
- `raw`: Enables access to the experimental and unsafe `RawTable` API.
- `inline-more`: Adds inline hints to most functions, improving run-time performance at the cost
  of compilation time. (enabled by default)
- `bumpalo`: Provides a `BumpWrapper` type which allows `bumpalo` to be used for memory allocation.
- `ahash`: Compiles with ahash as default hasher. (enabled by default)
- `ahash-compile-time-rng`: Activates the `compile-time-rng` feature of ahash. For targets with no random number generator
this pre-generates seeds at compile time and embeds them as constants. See [aHash's documentation](https://github.com/tkaitchuck/aHash#flags) (disabled by default)

## License

Licensed under either of:

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
