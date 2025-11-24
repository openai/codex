# num-traits

[![crate](https://img.shields.io/crates/v/num-traits.svg)](https://crates.io/crates/num-traits)
[![documentation](https://docs.rs/num-traits/badge.svg)](https://docs.rs/num-traits)
[![minimum rustc 1.60](https://img.shields.io/badge/rustc-1.60+-red.svg)](https://rust-lang.github.io/rfcs/2495-min-rust-version.html)
[![build status](https://github.com/rust-num/num-traits/workflows/master/badge.svg)](https://github.com/rust-num/num-traits/actions)

Numeric traits for generic mathematics in Rust.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
num-traits = "0.2"
```

## Features

This crate can be used without the standard library (`#![no_std]`) by disabling
the default `std` feature. Use this in `Cargo.toml`:

```toml
[dependencies.num-traits]
version = "0.2"
default-features = false
# features = ["libm"]    # <--- Uncomment if you wish to use `Float` and `Real` without `std`
```

The `Float` and `Real` traits are only available when either `std` or `libm` is enabled.

The `FloatCore` trait is always available.  `MulAdd` and `MulAddAssign` for `f32`
and `f64` also require `std` or `libm`, as do implementations of signed and floating-
point exponents in `Pow`.

## Releases

Release notes are available in [RELEASES.md](RELEASES.md).

## Compatibility

The `num-traits` crate is tested for rustc 1.60 and greater.

## License

Licensed under either of

 * [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
 * [MIT license](http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
