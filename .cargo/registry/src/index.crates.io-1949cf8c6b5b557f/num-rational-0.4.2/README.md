# num-rational

[![crate](https://img.shields.io/crates/v/num-rational.svg)](https://crates.io/crates/num-rational)
[![documentation](https://docs.rs/num-rational/badge.svg)](https://docs.rs/num-rational)
[![minimum rustc 1.60](https://img.shields.io/badge/rustc-1.60+-red.svg)](https://rust-lang.github.io/rfcs/2495-min-rust-version.html)
[![build status](https://github.com/rust-num/num-rational/workflows/master/badge.svg)](https://github.com/rust-num/num-rational/actions)

Generic `Rational` numbers (aka fractions) for Rust.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
num-rational = "0.4"
```

## Features

This crate can be used without the standard library (`#![no_std]`) by disabling
the default `std` feature.  Use this in `Cargo.toml`:

```toml
[dependencies.num-rational]
version = "0.4"
default-features = false
```

## Releases

Release notes are available in [RELEASES.md](RELEASES.md).

## Compatibility

The `num-rational` crate is tested for rustc 1.60 and greater.

## License

Licensed under either of

 * [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
 * [MIT license](http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
