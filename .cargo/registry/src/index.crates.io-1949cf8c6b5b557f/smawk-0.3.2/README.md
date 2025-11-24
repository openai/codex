# SMAWK Algorithm in Rust

[![](https://github.com/mgeisler/smawk/workflows/build/badge.svg)][build-status]
[![](https://codecov.io/gh/mgeisler/smawk/branch/master/graph/badge.svg)][codecov]
[![](https://img.shields.io/crates/v/smawk.svg)][crates-io]
[![](https://docs.rs/smawk/badge.svg)][api-docs]

This crate contains an implementation of the [SMAWK algorithm][smawk] for
finding the smallest element per row in a totally monotone matrix.

The SMAWK algorithm allows you to lower the running time of some algorithms from
O(_n_²) to just O(_n_). In other words, you can turn a quadratic time complexity
(which is often too expensive) into linear time complexity.

Finding optimal line breaks in a paragraph of text is an example of an algorithm
which would normally take O(_n_²) time for _n_ words. With this crate, the
running time becomes linear. Please see the [textwrap crate][textwrap] for an
example of this.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
smawk = "0.3"
```

You can now efficiently find row and column minima. Here is an example where we
find the column minima:

```rust
use smawk::Matrix;

let matrix = vec![
    vec![3, 2, 4, 5, 6],
    vec![2, 1, 3, 3, 4],
    vec![2, 1, 3, 3, 4],
    vec![3, 2, 4, 3, 4],
    vec![4, 3, 2, 1, 1],
];
let minima = vec![1, 1, 4, 4, 4];
assert_eq!(smawk::column_minima(&matrix), minima);
```

The `minima` vector gives the index of the minimum value per column, so
`minima[0] == 1` since the minimum value in the first column is 2 (row 1). Note
that the smallest row index is returned.

### Cargo Features

This crate has an optional dependency on the
[`ndarray` crate](https://docs.rs/ndarray/), which provides an efficient matrix
implementation. Enable the `ndarray` Cargo feature to use it.

## Documentation

**[API documentation][api-docs]**

## Changelog

### Version 0.3.2 (2023-09-17)

This release adds more documentation and renames the top-level SMAWK functions.
The old names have been kept for now to ensure backwards compatibility, but they
will be removed in a future release.

- [#65](https://github.com/mgeisler/smawk/pull/65): Forbid the use of unsafe
  code.
- [#69](https://github.com/mgeisler/smawk/pull/69): Migrate to the Rust 2021
  edition.
- [#73](https://github.com/mgeisler/smawk/pull/73): Add examples to all
  functions.
- [#74](https://github.com/mgeisler/smawk/pull/74): Add “mathematics” as a crate
  category.
- [#75](https://github.com/mgeisler/smawk/pull/75): Remove `smawk_` prefix from
  optimized functions.

### Version 0.3.1 (2021-01-30)

This release relaxes the bounds on the `smawk_row_minima`,
`smawk_column_minima`, and `online_column_minima` functions so that they work on
matrices containing floating point numbers.

- [#55](https://github.com/mgeisler/smawk/pull/55): Relax bounds to `PartialOrd`
  instead of `Ord`.
- [#56](https://github.com/mgeisler/smawk/pull/56): Update dependencies to their
  latest versions.
- [#59](https://github.com/mgeisler/smawk/pull/59): Give an example of what
  SMAWK does in the README.

### Version 0.3.0 (2020-09-02)

This release slims down the crate significantly by making `ndarray` an optional
dependency.

- [#45](https://github.com/mgeisler/smawk/pull/45): Move non-SMAWK code and unit
  tests out of lib and into separate modules.
- [#46](https://github.com/mgeisler/smawk/pull/46): Switch `smawk_row_minima`
  and `smawk_column_minima` functions to a new `Matrix` trait.
- [#47](https://github.com/mgeisler/smawk/pull/47): Make the dependency on the
  `ndarray` crate optional.
- [#48](https://github.com/mgeisler/smawk/pull/48): Let `is_monge` take a
  `Matrix` argument instead of `ndarray::Array2`.
- [#50](https://github.com/mgeisler/smawk/pull/50): Remove mandatory
  dependencies on `rand` and `num-traits` crates.

### Version 0.2.0 (2020-07-29)

This release updates the code to Rust 2018.

- [#18](https://github.com/mgeisler/smawk/pull/18): Make `online_column_minima`
  generic in matrix type.
- [#23](https://github.com/mgeisler/smawk/pull/23): Switch to the
  [Rust 2018][rust-2018] edition. We test against the latest stable and nightly
  version of Rust.
- [#29](https://github.com/mgeisler/smawk/pull/29): Drop strict Rust 2018
  compatibility by not testing with Rust 1.31.0.
- [#32](https://github.com/mgeisler/smawk/pull/32): Fix crash on overflow in
  `is_monge`.
- [#33](https://github.com/mgeisler/smawk/pull/33): Update `rand` dependency to
  latest version and get rid of `rand_derive`.
- [#34](https://github.com/mgeisler/smawk/pull/34): Bump `num-traits` and
  `version-sync` dependencies to latest versions.
- [#35](https://github.com/mgeisler/smawk/pull/35): Drop unnecessary Windows
  tests. The assumption is that the numeric computations we do are
  cross-platform.
- [#36](https://github.com/mgeisler/smawk/pull/36): Update `ndarray` dependency
  to the latest version.
- [#37](https://github.com/mgeisler/smawk/pull/37): Automate publishing new
  releases to crates.io.

### Version 0.1.0 — August 7th, 2018

First release with the classical offline SMAWK algorithm as well as a newer
online version where the matrix entries can depend on previously computed column
minima.

## License

SMAWK can be distributed according to the [MIT license][mit]. Contributions will
be accepted under the same license.

[build-status]: https://github.com/mgeisler/smawk/actions?query=branch%3Amaster+workflow%3Abuild
[crates-io]: https://crates.io/crates/smawk
[codecov]: https://codecov.io/gh/mgeisler/smawk
[textwrap]: https://crates.io/crates/textwrap
[smawk]: https://en.wikipedia.org/wiki/SMAWK_algorithm
[api-docs]: https://docs.rs/smawk/
[rust-2018]: https://doc.rust-lang.org/edition-guide/rust-2018/
[mit]: LICENSE
