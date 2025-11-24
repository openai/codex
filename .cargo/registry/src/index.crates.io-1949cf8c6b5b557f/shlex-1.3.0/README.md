[![ci badge]][ci link] [![crates.io badge]][crates.io link] [![docs.rs badge]][docs.rs link]

[crates.io badge]: https://img.shields.io/crates/v/shlex.svg?style=flat-square
[crates.io link]: https://crates.io/crates/shlex
[docs.rs badge]: https://img.shields.io/badge/docs-online-dddddd.svg?style=flat-square
[docs.rs link]: https://docs.rs/shlex
[ci badge]: https://img.shields.io/github/actions/workflow/status/comex/rust-shlex/test.yml?branch=master&style=flat-square
[ci link]: https://github.com/comex/rust-shlex/actions

Same idea as (but implementation not directly based on) the Python shlex
module. However, this implementation does not support any of the Python
module's customization because it makes parsing slower and is fairly useless.
You only get the default settings of shlex.split, which mimic the POSIX shell:
<https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html>

This implementation also deviates from the Python version in not treating \r
specially, which I believe is more compliant.

This crate can be used on either normal Rust strings, or on byte strings with
the `bytes` module. The algorithms used are oblivious to UTF-8 high bytes, so
internally they all work on bytes directly as a micro-optimization.

Disabling the `std` feature (which is enabled by default) will allow the crate
to work in `no_std` environments, where the `alloc` crate, and a global
allocator, are available.

# LICENSE

The source code in this repository is Licensed under either of
- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
  https://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  https://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
