# rand_chacha

[![Test Status](https://github.com/rust-random/rand/workflows/Tests/badge.svg?event=push)](https://github.com/rust-random/rand/actions)
[![Latest version](https://img.shields.io/crates/v/rand_chacha.svg)](https://crates.io/crates/rand_chacha)
[![Book](https://img.shields.io/badge/book-master-yellow.svg)](https://rust-random.github.io/book/)
[![API](https://img.shields.io/badge/api-master-yellow.svg)](https://rust-random.github.io/rand/rand_chacha)
[![API](https://docs.rs/rand_chacha/badge.svg)](https://docs.rs/rand_chacha)
[![Minimum rustc version](https://img.shields.io/badge/rustc-1.36+-lightgray.svg)](https://github.com/rust-random/rand#rust-version-requirements)

A cryptographically secure random number generator that uses the ChaCha
algorithm.

ChaCha is a stream cipher designed by Daniel J. Bernstein[^1], that we use
as an RNG. It is an improved variant of the Salsa20 cipher family, which was
selected as one of the "stream ciphers suitable for widespread adoption" by
eSTREAM[^2].

The RNGs provided by this crate are implemented via the fast stream ciphers of
the [`c2-chacha`](https://crates.io/crates/c2-chacha) crate.

Links:

-   [API documentation (master)](https://rust-random.github.io/rand/rand_chacha)
-   [API documentation (docs.rs)](https://docs.rs/rand_chacha)
-   [Changelog](https://github.com/rust-random/rand/blob/master/rand_chacha/CHANGELOG.md)

[rand]: https://crates.io/crates/rand
[^1]: D. J. Bernstein, [*ChaCha, a variant of Salsa20*](
      https://cr.yp.to/chacha.html)

[^2]: [eSTREAM: the ECRYPT Stream Cipher Project](
      http://www.ecrypt.eu.org/stream/)


## Crate Features

`rand_chacha` is `no_std` compatible when disabling default features; the `std`
feature can be explicitly required to re-enable `std` support. Using `std`
allows detection of CPU features and thus better optimisation.


# License

`rand_chacha` is distributed under the terms of both the MIT license and the
Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT), and
[COPYRIGHT](COPYRIGHT) for details.
