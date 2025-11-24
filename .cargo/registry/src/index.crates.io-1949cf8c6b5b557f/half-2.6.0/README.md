# `f16` and `bf16` floating point types for Rust
[![Crates.io](https://img.shields.io/crates/v/half.svg)](https://crates.io/crates/half/) [![Documentation](https://docs.rs/half/badge.svg)](https://docs.rs/half/) ![Crates.io](https://img.shields.io/crates/l/half) [![Build status](https://github.com/starkat99/half-rs/actions/workflows/rust.yml/badge.svg?branch=main&event=push)](https://github.com/starkat99/half-rs/actions/workflows/rust.yml) [![CircleCI](https://dl.circleci.com/status-badge/img/gh/starkat99/half-rs/tree/main.svg?style=svg)](https://dl.circleci.com/status-badge/redirect/gh/starkat99/half-rs/tree/main)

This crate implements a half-precision floating point `f16` type for Rust implementing the IEEE
754-2008 standard [`binary16`](https://en.wikipedia.org/wiki/Half-precision_floating-point_format)
a.k.a "half" format, as well as a `bf16` type implementing the
[`bfloat16`](https://en.wikipedia.org/wiki/Bfloat16_floating-point_format) format.

## Usage

The `f16` and `bf16` types attempt to match existing Rust floating point type functionality where possible, and provides both conversion operations (such as to/from `f32` and `f64`) and basic
arithmetic operations. Hardware support for these operations will be used whenever hardware support
is available—either through instrinsics or targeted assembly—although a nightly Rust toolchain may
be required for some hardware.

This crate provides [`no_std`](https://rust-embedded.github.io/book/intro/no-std.html) support so can easily be used in embedded code where a smaller float format is most useful.

*Requires Rust 1.81 or greater.* If you need support for older versions of Rust, use previous 
versions of this crate.

See the [crate documentation](https://docs.rs/half/) for more details.

### Optional Features

- **`alloc`** — Enable use of the [`alloc`](https://doc.rust-lang.org/alloc/) crate when not using
  the `std` library.

  This enables the `vec` module, which contains zero-copy conversions for the `Vec` type. This
  allows fast conversion between raw `Vec<u16>` bits and `Vec<f16>` or `Vec<bf16>` arrays, and vice
  versa.

- **`std`** — Enable features that depend on the Rust `std` library, including everything in the
  `alloc` feature.

  Enabling the `std` feature enables runtime CPU feature detection of hardware support.
  Without this feature detection, harware is only used when compiler target supports them.

- **`serde`** - Implement `Serialize` and `Deserialize` traits for `f16` and `bf16`. This adds a
  dependency on the [`serde`](https://crates.io/crates/serde) crate.

- **`num-traits`** — Enable `ToPrimitive`, `FromPrimitive`, `ToBytes`, `FromBytes`, `Num`, `Float`,
  `FloatCore` and `Bounded` trait implementations from the
  [`num-traits`](https://crates.io/crates/num-traits) crate.

- **`bytemuck`** — Enable `Zeroable` and `Pod` trait implementations from the
  [`bytemuck`](https://crates.io/crates/bytemuck) crate.

- **`zerocopy`** — Enable `IntoBytes` and `FromBytes` trait implementations from the 
  [`zerocopy`](https://crates.io/crates/zerocopy) crate.

- **`rand_distr`** — Enable sampling from distributions like `StandardUniform` and `StandardNormal` 
  from the [`rand_distr`](https://crates.io/crates/rand_distr) crate.

- **`rkyv`** -- Enable zero-copy deserializtion with [`rkyv`](https://crates.io/crates/rkyv) crate.

- **`aribtrary`** -- Enable fuzzing support with [`arbitrary`](https://crates.io/crates/arbitrary) 
  crate by implementing `Arbitrary` trait.

### Hardware support

The following list details hardware support for floating point types in this crate. When using `std`
library, runtime CPU target detection will be used. To get the most performance benefits, compile
for specific CPU features which avoids the runtime overhead and works in a `no_std` environment.

| Architecture | CPU Target Feature | Notes |
| ------------ | ------------------ | ----- |
| `x86`/`x86_64` | `f16c` | This supports conversion to/from `f16` only (including vector SIMD) and does not support any `bf16` or arithmetic operations. |
| `aarch64` | `fp16` | This supports all operations on `f16` only. |

### More Documentation

- [Crate API Reference](https://docs.rs/half/)
- [Latest Changes](CHANGELOG.md)

## License

All files in this library are dual-licensed and distributed under the terms of either of:

* [MIT License](LICENSE-MIT)
  ([http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
* [Apache License, Version 2.0](LICENSE-APACHE)
  ([http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

at your option.

### Contributing

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the
work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
