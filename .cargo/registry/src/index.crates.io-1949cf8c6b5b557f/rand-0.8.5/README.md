# Rand

[![Test Status](https://github.com/rust-random/rand/workflows/Tests/badge.svg?event=push)](https://github.com/rust-random/rand/actions)
[![Crate](https://img.shields.io/crates/v/rand.svg)](https://crates.io/crates/rand)
[![Book](https://img.shields.io/badge/book-master-yellow.svg)](https://rust-random.github.io/book/)
[![API](https://img.shields.io/badge/api-master-yellow.svg)](https://rust-random.github.io/rand/rand)
[![API](https://docs.rs/rand/badge.svg)](https://docs.rs/rand)
[![Minimum rustc version](https://img.shields.io/badge/rustc-1.36+-lightgray.svg)](https://github.com/rust-random/rand#rust-version-requirements)

A Rust library for random number generation, featuring:

-   Easy random value generation and usage via the [`Rng`](https://docs.rs/rand/*/rand/trait.Rng.html),
    [`SliceRandom`](https://docs.rs/rand/*/rand/seq/trait.SliceRandom.html) and
    [`IteratorRandom`](https://docs.rs/rand/*/rand/seq/trait.IteratorRandom.html) traits
-   Secure seeding via the [`getrandom` crate](https://crates.io/crates/getrandom)
    and fast, convenient generation via [`thread_rng`](https://docs.rs/rand/*/rand/fn.thread_rng.html)
-   A modular design built over [`rand_core`](https://crates.io/crates/rand_core)
    ([see the book](https://rust-random.github.io/book/crates.html))
-   Fast implementations of the best-in-class [cryptographic](https://rust-random.github.io/book/guide-rngs.html#cryptographically-secure-pseudo-random-number-generators-csprngs) and
    [non-cryptographic](https://rust-random.github.io/book/guide-rngs.html#basic-pseudo-random-number-generators-prngs) generators
-   A flexible [`distributions`](https://docs.rs/rand/*/rand/distributions/index.html) module
-   Samplers for a large number of random number distributions via our own
    [`rand_distr`](https://docs.rs/rand_distr) and via
    the [`statrs`](https://docs.rs/statrs/0.13.0/statrs/)
-   [Portably reproducible output](https://rust-random.github.io/book/portability.html)
-   `#[no_std]` compatibility (partial)
-   *Many* performance optimisations

It's also worth pointing out what `rand` *is not*:

-   Small. Most low-level crates are small, but the higher-level `rand` and
    `rand_distr` each contain a lot of functionality.
-   Simple (implementation). We have a strong focus on correctness, speed and flexibility, but
    not simplicity. If you prefer a small-and-simple library, there are
    alternatives including [fastrand](https://crates.io/crates/fastrand)
    and [oorandom](https://crates.io/crates/oorandom).
-   Slow. We take performance seriously, with considerations also for set-up
    time of new distributions, commonly-used parameters, and parameters of the
    current sampler.

Documentation:

-   [The Rust Rand Book](https://rust-random.github.io/book)
-   [API reference (master branch)](https://rust-random.github.io/rand)
-   [API reference (docs.rs)](https://docs.rs/rand)


## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
rand = "0.8.4"
```

To get started using Rand, see [The Book](https://rust-random.github.io/book).


## Versions

Rand is *mature* (suitable for general usage, with infrequent breaking releases
which minimise breakage) but not yet at 1.0. We maintain compatibility with
pinned versions of the Rust compiler (see below).

Current Rand versions are:

-   Version 0.7 was released in June 2019, moving most non-uniform distributions
    to an external crate, moving `from_entropy` to `SeedableRng`, and many small
    changes and fixes.
-   Version 0.8 was released in December 2020 with many small changes.

A detailed [changelog](CHANGELOG.md) is available for releases.

When upgrading to the next minor series (especially 0.4 → 0.5), we recommend
reading the [Upgrade Guide](https://rust-random.github.io/book/update.html).

Rand has not yet reached 1.0 implying some breaking changes may arrive in the
future ([SemVer](https://semver.org/) allows each 0.x.0 release to include
breaking changes), but is considered *mature*: breaking changes are minimised
and breaking releases are infrequent.

Rand libs have inter-dependencies and make use of the
[semver trick](https://github.com/dtolnay/semver-trick/) in order to make traits
compatible across crate versions. (This is especially important for `RngCore`
and `SeedableRng`.) A few crate releases are thus compatibility shims,
depending on the *next* lib version (e.g. `rand_core` versions `0.2.2` and
`0.3.1`). This means, for example, that `rand_core_0_4_0::SeedableRng` and
`rand_core_0_3_0::SeedableRng` are distinct, incompatible traits, which can
cause build errors. Usually, running `cargo update` is enough to fix any issues.

### Yanked versions

Some versions of Rand crates have been yanked ("unreleased"). Where this occurs,
the crate's CHANGELOG *should* be updated with a rationale, and a search on the
issue tracker with the keyword `yank` *should* uncover the motivation.

### Rust version requirements

Since version 0.8, Rand requires **Rustc version 1.36 or greater**.
Rand 0.7 requires Rustc 1.32 or greater while versions 0.5 require Rustc 1.22 or
greater, and 0.4 and 0.3 (since approx. June 2017) require Rustc version 1.15 or
greater. Subsets of the Rand code may work with older Rust versions, but this is
not supported.

Continuous Integration (CI) will always test the minimum supported Rustc version
(the MSRV). The current policy is that this can be updated in any
Rand release if required, but the change must be noted in the changelog.

## Crate Features

Rand is built with these features enabled by default:

-   `std` enables functionality dependent on the `std` lib
-   `alloc` (implied by `std`) enables functionality requiring an allocator
-   `getrandom` (implied by `std`) is an optional dependency providing the code
    behind `rngs::OsRng`
-   `std_rng` enables inclusion of `StdRng`, `thread_rng` and `random`
    (the latter two *also* require that `std` be enabled)

Optionally, the following dependencies can be enabled:

-   `log` enables logging via the `log` crate

Additionally, these features configure Rand:

-   `small_rng` enables inclusion of the `SmallRng` PRNG
-   `nightly` enables some optimizations requiring nightly Rust
-   `simd_support` (experimental) enables sampling of SIMD values
    (uniformly random SIMD integers and floats), requiring nightly Rust
-   `min_const_gen` enables generating random arrays of 
    any size using min-const-generics, requiring Rust ≥ 1.51.

Note that nightly features are not stable and therefore not all library and
compiler versions will be compatible. This is especially true of Rand's
experimental `simd_support` feature.

Rand supports limited functionality in `no_std` mode (enabled via
`default-features = false`). In this case, `OsRng` and `from_entropy` are
unavailable (unless `getrandom` is enabled), large parts of `seq` are
unavailable (unless `alloc` is enabled), and `thread_rng` and `random` are
unavailable.

### WASM support

The WASM target `wasm32-unknown-unknown` is not *automatically* supported by
`rand` or `getrandom`. To solve this, either use a different target such as
`wasm32-wasi` or add a direct dependency on `getrandom` with the `js` feature
(if the target supports JavaScript). See
[getrandom#WebAssembly support](https://docs.rs/getrandom/latest/getrandom/#webassembly-support).

# License

Rand is distributed under the terms of both the MIT license and the
Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT), and
[COPYRIGHT](COPYRIGHT) for details.
