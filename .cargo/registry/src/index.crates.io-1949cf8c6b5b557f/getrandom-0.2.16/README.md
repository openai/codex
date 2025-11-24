# getrandom

[![Build Status]][GitHub Actions] [![Crate]][crates.io] [![Documentation]][docs.rs] [![Dependency Status]][deps.rs] [![Downloads]][crates.io] [![License]][LICENSE-MIT]

[GitHub Actions]: https://github.com/rust-random/getrandom/actions?query=workflow:Tests+branch:master
[Build Status]: https://github.com/rust-random/getrandom/actions/workflows/tests.yml/badge.svg?branch=master
[crates.io]: https://crates.io/crates/getrandom
[Crate]: https://img.shields.io/crates/v/getrandom
[docs.rs]: https://docs.rs/getrandom
[Documentation]: https://docs.rs/getrandom/badge.svg
[deps.rs]: https://deps.rs/repo/github/rust-random/getrandom
[Dependency Status]: https://deps.rs/repo/github/rust-random/getrandom/status.svg
[Downloads]: https://img.shields.io/crates/d/getrandom
[LICENSE-MIT]: https://raw.githubusercontent.com/rust-random/getrandom/master/LICENSE-MIT
[License]: https://img.shields.io/crates/l/getrandom


A Rust library for retrieving random data from (operating) system sources. It is
assumed that the system always provides high-quality cryptographically secure random
data, ideally backed by hardware entropy sources. This crate derives its name
from Linux's `getrandom` function, but is cross-platform, roughly supporting
the same set of platforms as Rust's `std` lib.

This is a low-level API. Most users should prefer using high-level random-number
library like [`rand`].

[`rand`]: https://crates.io/crates/rand

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
getrandom = "0.2"
```

Then invoke the `getrandom` function:

```rust
fn get_random_buf() -> Result<[u8; 32], getrandom::Error> {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf)?;
    Ok(buf)
}
```

For more information about supported targets, entropy sources, `no_std` targets,
crate features, WASM support and Custom RNGs see the
[`getrandom` documentation](https://docs.rs/getrandom/latest) and
[`getrandom::Error` documentation](https://docs.rs/getrandom/latest/getrandom/struct.Error.html).

## Minimum Supported Rust Version

This crate requires Rust 1.36.0 or later.

## Platform Support

This crate generally supports the same operating system and platform versions that the Rust standard library does. 
Additional targets may be supported using pluggable custom implementations.

This means that as Rust drops support for old versions of operating systems (such as old Linux kernel versions, Android API levels, etc)
in stable releases, `getrandom` may create new patch releases (`0.N.x`) that remove support for outdated platform versions.

## License

The `getrandom` library is distributed under either of

 * [Apache License, Version 2.0][LICENSE-APACHE]
 * [MIT license][LICENSE-MIT]

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[LICENSE-APACHE]: https://github.com/rust-random/getrandom/blob/master/LICENSE-APACHE
[LICENSE-MIT]: https://github.com/rust-random/getrandom/blob/master/LICENSE-MIT
