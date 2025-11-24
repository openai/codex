[![Documentation](https://docs.rs/home/badge.svg)](https://docs.rs/home)
[![crates.io](https://img.shields.io/crates/v/home.svg)](https://crates.io/crates/home)

Canonical definitions of `home_dir`, `cargo_home`, and `rustup_home`.

This provides the definition of `home_dir` used by Cargo and rustup,
as well functions to find the correct value of `CARGO_HOME` and
`RUSTUP_HOME`.

The definition of [`home_dir`] provided by the standard library is
incorrect because it considers the `HOME` environment variable on
Windows. This causes surprising situations where a Rust program will
behave differently depending on whether it is run under a Unix
emulation environment like Cygwin or MinGW. Neither Cargo nor rustup
use the standard library's definition - they use the definition here.

**Note:** This has been fixed in Rust 1.85 to no longer use the `HOME`
environment variable on Windows. If you are still using this crate for the
purpose of getting a home directory, you are strongly encouraged to switch to
using the standard library's [`home_dir`] instead. It is planned to have the
deprecation notice removed in 1.86.

This crate further provides two functions, `cargo_home` and
`rustup_home`, which are the canonical way to determine the location
that Cargo and rustup store their data.

See [rust-lang/rust#43321].

> This crate is maintained by the Cargo team, primarily for use by Cargo and Rustup
> and not intended for external use. This
> crate may make major changes to its APIs or be deprecated without warning.

[rust-lang/rust#43321]: https://github.com/rust-lang/rust/issues/43321
[`home_dir`]: https://doc.rust-lang.org/nightly/std/env/fn.home_dir.html

## License

MIT OR Apache-2.0
