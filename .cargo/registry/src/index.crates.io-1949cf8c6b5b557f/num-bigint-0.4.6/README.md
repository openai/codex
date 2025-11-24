# num-bigint

[![crate](https://img.shields.io/crates/v/num-bigint.svg)](https://crates.io/crates/num-bigint)
[![documentation](https://docs.rs/num-bigint/badge.svg)](https://docs.rs/num-bigint)
[![minimum rustc 1.60](https://img.shields.io/badge/rustc-1.60+-red.svg)](https://rust-lang.github.io/rfcs/2495-min-rust-version.html)
[![build status](https://github.com/rust-num/num-bigint/workflows/master/badge.svg)](https://github.com/rust-num/num-bigint/actions)

Big integer types for Rust, `BigInt` and `BigUint`.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
num-bigint = "0.4"
```

## Features

The `std` crate feature is enabled by default, and is mandatory before Rust
1.36 and the stabilized `alloc` crate.  If you depend on `num-bigint` with
`default-features = false`, you must manually enable the `std` feature yourself
if your compiler is not new enough.

### Random Generation

`num-bigint` supports the generation of random big integers when the `rand`
feature is enabled. To enable it include rand as

```toml
rand = "0.8"
num-bigint = { version = "0.4", features = ["rand"] }
```

Note that you must use the version of `rand` that `num-bigint` is compatible
with: `0.8`.

## Releases

Release notes are available in [RELEASES.md](RELEASES.md).

## Compatibility

The `num-bigint` crate is tested for rustc 1.60 and greater.

## Alternatives

While `num-bigint` strives for good performance in pure Rust code, other
crates may offer better performance with different trade-offs.  The following
table offers a brief comparison to a few alternatives.

| Crate             | License        | Min rustc | Implementation | Features |
| :---------------  | :------------- | :-------- | :------------- | :------- |
| **`num-bigint`**  | MIT/Apache-2.0 | 1.60      | pure rust | dynamic width, number theoretical functions |
| [`awint`]         | MIT/Apache-2.0 | 1.66      | pure rust | fixed width, heap or stack, concatenation macros |
| [`bnum`]          | MIT/Apache-2.0 | 1.65      | pure rust | fixed width, parity with Rust primitives including floats |
| [`crypto-bigint`] | MIT/Apache-2.0 | 1.73      | pure rust | fixed width, stack only |
| [`ibig`]          | MIT/Apache-2.0 | 1.49      | pure rust | dynamic width, number theoretical functions |
| [`rug`]           | LGPL-3.0+      | 1.65      | bundles [GMP] via [`gmp-mpfr-sys`] | all the features of GMP, MPFR, and MPC |

[`awint`]: https://crates.io/crates/awint
[`bnum`]: https://crates.io/crates/bnum
[`crypto-bigint`]: https://crates.io/crates/crypto-bigint
[`ibig`]: https://crates.io/crates/ibig
[`rug`]: https://crates.io/crates/rug

[GMP]: https://gmplib.org/
[`gmp-mpfr-sys`]: https://crates.io/crates/gmp-mpfr-sys

## License

Licensed under either of

 * [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
 * [MIT license](http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
