# Rand

[![Test Status](https://github.com/rust-random/rand/actions/workflows/test.yml/badge.svg?event=push)](https://github.com/rust-random/rand/actions)
[![Crate](https://img.shields.io/crates/v/rand.svg)](https://crates.io/crates/rand)
[![Book](https://img.shields.io/badge/book-master-yellow.svg)](https://rust-random.github.io/book/)
[![API](https://img.shields.io/badge/api-master-yellow.svg)](https://rust-random.github.io/rand/rand)
[![API](https://docs.rs/rand/badge.svg)](https://docs.rs/rand)

Rand is a set of crates supporting (pseudo-)random generators:

-   Built over a standard RNG trait: [`rand_core::RngCore`](https://docs.rs/rand_core/latest/rand_core/trait.RngCore.html)
-   With fast implementations of both [strong](https://rust-random.github.io/book/guide-rngs.html#cryptographically-secure-pseudo-random-number-generators-csprngs) and
    [small](https://rust-random.github.io/book/guide-rngs.html#basic-pseudo-random-number-generators-prngs) generators: [`rand::rngs`](https://docs.rs/rand/latest/rand/rngs/index.html), and more RNGs: [`rand_chacha`](https://docs.rs/rand_chacha), [`rand_xoshiro`](https://docs.rs/rand_xoshiro/), [`rand_pcg`](https://docs.rs/rand_pcg/), [rngs repo](https://github.com/rust-random/rngs/)
-   [`rand::rng`](https://docs.rs/rand/latest/rand/fn.rng.html) is an asymptotically-fast, automatically-seeded and reasonably strong generator available on all `std` targets
-   Direct support for seeding generators from the [getrandom] crate

With broad support for random value generation and random processes:

-   [`StandardUniform`](https://docs.rs/rand/latest/rand/distr/struct.StandardUniform.html) random value sampling,
    [`Uniform`](https://docs.rs/rand/latest/rand/distr/struct.Uniform.html)-ranged value sampling
    and [more](https://docs.rs/rand/latest/rand/distr/index.html)
-   Samplers for a large number of non-uniform random number distributions via our own
    [`rand_distr`](https://docs.rs/rand_distr) and via
    the [`statrs`](https://docs.rs/statrs)
-   Random processes (mostly choose and shuffle) via [`rand::seq`](https://docs.rs/rand/latest/rand/seq/index.html) traits

All with:

-   [Portably reproducible output](https://rust-random.github.io/book/portability.html)
-   `#[no_std]` compatibility (partial)
-   *Many* performance optimisations thanks to contributions from the wide
    user-base

Rand **is not**:

-   Small (LoC). Most low-level crates are small, but the higher-level `rand`
    and `rand_distr` each contain a lot of functionality.
-   Simple (implementation). We have a strong focus on correctness, speed and flexibility, but
    not simplicity. If you prefer a small-and-simple library, there are
    alternatives including [fastrand](https://crates.io/crates/fastrand)
    and [oorandom](https://crates.io/crates/oorandom).
-   Primarily a cryptographic library. `rand` does provide some generators which
    aim to support unpredictable value generation under certain constraints;
    see [SECURITY.md](https://github.com/rust-random/rand/blob/master/SECURITY.md) for details.
    Users are expected to determine for themselves
    whether `rand`'s functionality meets their own security requirements.

Documentation:

-   [The Rust Rand Book](https://rust-random.github.io/book)
-   [API reference (master branch)](https://rust-random.github.io/rand)
-   [API reference (docs.rs)](https://docs.rs/rand)


## Versions

Rand is *mature* (suitable for general usage, with infrequent breaking releases
which minimise breakage) but not yet at 1.0. Current `MAJOR.MINOR` versions are:

-   Version 0.9 was released in January 2025.

See the [CHANGELOG](https://github.com/rust-random/rand/blob/master/CHANGELOG.md) or [Upgrade Guide](https://rust-random.github.io/book/update.html) for more details.

## Crate Features

Rand is built with these features enabled by default:

-   `std` enables functionality dependent on the `std` lib
-   `alloc` (implied by `std`) enables functionality requiring an allocator
-   `os_rng` (implied by `std`) enables `rngs::OsRng`, using the [getrandom] crate
-   `std_rng` enables inclusion of `StdRng`, `ThreadRng`
-   `small_rng` enables inclusion of the `SmallRng` PRNG

Optionally, the following dependencies can be enabled:

-   `log` enables logging via [log](https://crates.io/crates/log)

Additionally, these features configure Rand:

-   `nightly` includes some additions requiring nightly Rust
-   `simd_support` (experimental) enables sampling of SIMD values
    (uniformly random SIMD integers and floats), requiring nightly Rust
-   `unbiased` use unbiased sampling for algorithms supporting this option: Uniform distribution.

    (By default, bias affecting no more than one in  2^48 samples is accepted.)

    Note: enabling this option is expected to affect reproducibility of results.

Note that nightly features are not stable and therefore not all library and
compiler versions will be compatible. This is especially true of Rand's
experimental `simd_support` feature.

Rand supports limited functionality in `no_std` mode (enabled via
`default-features = false`). In this case, `OsRng` and `from_os_rng` are
unavailable (unless `os_rng` is enabled), large parts of `seq` are
unavailable (unless `alloc` is enabled), and `ThreadRng` is unavailable.

## Portability and platform support

Many (but not all) algorithms are intended to have reproducible output. Read more in the book: [Portability](https://rust-random.github.io/book/portability.html).

The Rand library supports a variety of CPU architectures. Platform integration is outsourced to [getrandom].

### WebAssembly support

The [WASI](https://github.com/WebAssembly/WASI/tree/main) and Emscripten
targets are directly supported. The `wasm32-unknown-unknown` target is not
*automatically* supported. To enable support for this target, refer to the
[`getrandom` documentation for WebAssembly](https://docs.rs/getrandom/latest/getrandom/#webassembly-support).
Alternatively, the `os_rng` feature may be disabled.

# License

Rand is distributed under the terms of both the MIT license and the
Apache License (Version 2.0).

See [LICENSE-APACHE](https://github.com/rust-random/rand/blob/master/LICENSE-APACHE) and [LICENSE-MIT](https://github.com/rust-random/rand/blob/master/LICENSE-MIT), and
[COPYRIGHT](https://github.com/rust-random/rand/blob/master/COPYRIGHT) for details.

[getrandom]: https://crates.io/crates/getrandom
