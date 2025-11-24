# Socket2

Socket2 is a crate that provides utilities for creating and using sockets.

The goal of this crate is to create and use a socket using advanced
configuration options (those that are not available in the types in the standard
library) without using any unsafe code.

This crate provides as direct as possible access to the system's functionality
for sockets, this means little effort to provide cross-platform utilities. It is
up to the user to know how to use sockets when using this crate. *If you don't
know how to create a socket using libc/system calls then this crate is not for
you*. Most, if not all, functions directly relate to the equivalent system call
with no error handling applied, so no handling errors such as `EINTR`. As a
result using this crate can be a little wordy, but it should give you maximal
flexibility over configuration of sockets.

See the [API documentation] for more.

[API documentation]: https://docs.rs/socket2

# Branches

Currently Socket2 supports the following versions:
 * v0.6 developed in the master branch
 * v0.5 developed in the [v0.5.x branch]
 * v0.4 developed in the [v0.4.x branch]

[v0.5.x branch]: https://github.com/rust-lang/socket2/tree/v0.5.x
[v0.4.x branch]: https://github.com/rust-lang/socket2/tree/v0.4.x

# OS support

Socket2 attempts to support the same OS/architectures as Rust does, see
https://doc.rust-lang.org/nightly/rustc/platform-support.html. However this is
not always possible, below is current list of support OSs.

*If your favorite OS is not on the list consider contributing it!*

### Tier 1

These OSs are tested with each commit in the CI and must always pass the tests.
All functions/types/etc., excluding ones behind the `all` feature, must work on
these OSs.

* Linux
* macOS
* Windows

### Tier 2

These OSs are currently build in the CI, but not tested. Not all
functions/types/etc. may work on these OSs, even ones **not** behind the `all`
feature flag.

* Android
* FreeBSD
* Fuchsia
* iOS
* illumos
* NetBSD
* Redox
* Solaris
* OpenHarmony

# Minimum Supported Rust Version (MSRV)

Socket2 uses 1.70.0 as MSRV.

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
