# time

[![minimum rustc: 1.81.0](https://img.shields.io/badge/minimum%20rustc-1.81.0-yellowgreen?logo=rust&style=flat-square)](https://www.whatrustisit.com)
[![version](https://img.shields.io/crates/v/time?color=blue&logo=rust&style=flat-square)](https://crates.io/crates/time)
[![build status](https://img.shields.io/github/actions/workflow/status/time-rs/time/build.yaml?branch=main&style=flat-square)](https://github.com/time-rs/time/actions)
[![codecov](https://codecov.io/gh/time-rs/time/branch/main/graph/badge.svg?token=yt4XSmQNKQ)](https://codecov.io/gh/time-rs/time)

Documentation:

- [latest release](https://docs.rs/time)
- [main branch](https://time-rs.github.io/api/time)
- [book](https://time-rs.github.io/book)

## Minimum Rust version policy

`time` is guaranteed to compile with the latest stable release of Rust in addition to the two prior
minor releases. For example, if the latest stable Rust release is 1.70, then `time` is guaranteed to
compile with Rust 1.68, 1.69, and 1.70.

The minimum supported Rust version may be increased to one of the aforementioned versions if doing
so provides the end user a benefit. However, the minimum supported Rust version may also be bumped
to a version four minor releases prior to the most recent stable release if doing so improves code
quality or maintainability.

For interoperability with third-party crates, it is guaranteed that there exists a version of that
crate that supports the minimum supported Rust version of `time`. This does not mean that the latest
version of the third-party crate supports the minimum supported Rust version of `time`.

## Contributing

Contributions are always welcome! If you have an idea, it's best to float it by me before working on
it to ensure no effort is wasted. If there's already an open issue for it, knock yourself out.
Internal documentation can be viewed [here](https://time-rs.github.io/internal-api/time).

If you have any questions, feel free to use [Discussions]. Don't hesitate to ask questions — that's
what I'm here for!

[Discussions]: https://github.com/time-rs/time/discussions

## License

This project is licensed under either of

- [Apache License, Version 2.0](https://github.com/time-rs/time/blob/main/LICENSE-Apache)
- [MIT license](https://github.com/time-rs/time/blob/main/LICENSE-MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
time by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
