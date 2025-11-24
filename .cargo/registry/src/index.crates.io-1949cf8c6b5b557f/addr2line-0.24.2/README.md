# addr2line

[![](https://img.shields.io/crates/v/addr2line.svg)](https://crates.io/crates/addr2line)
[![](https://img.shields.io/docsrs/addr2line.svg)](https://docs.rs/addr2line)
[![Coverage Status](https://coveralls.io/repos/github/gimli-rs/addr2line/badge.svg?branch=master)](https://coveralls.io/github/gimli-rs/addr2line?branch=master)

`addr2line` provides a cross-platform library for retrieving per-address debug information
from files with DWARF debug information. Given an address, it can return the file name,
line number, and function name associated with that address, as well as the inline call
stack leading to that address.

The crate has a CLI wrapper around the library which provides some of
the functionality of the `addr2line` command line tool distributed with
[GNU binutils](https://sourceware.org/binutils/docs/binutils/addr2line.html).

# Quickstart
 - Add the [`addr2line` crate](https://crates.io/crates/addr2line) to your `Cargo.toml`.
 - Call [`addr2line::Loader::new`](https://docs.rs/addr2line/*/addr2line/struct.Loader.html#method.new) with the file path.
 - Use [`addr2line::Loader::find_location`](https://docs.rs/addr2line/*/addr2line/struct.Loader.html#method.find_location)
   or [`addr2line::Loader::find_frames`](https://docs.rs/addr2line/*/addr2line/struct.Loader.html#method.find_frames)
   to look up debug information for an address.

If you want to provide your own file loading and memory management, use
[`addr2line::Context`](https://docs.rs/addr2line/*/addr2line/struct.Context.html)
instead of `addr2line::Loader`.

# Performance

`addr2line` optimizes for speed over memory by caching parsed information.
The DWARF information is parsed lazily where possible.

The library aims to perform similarly to equivalent existing tools such
as `addr2line` from binutils, `eu-addr2line` from elfutils, and
`llvm-addr2line` from the llvm project. Current benchmarks show a performance
improvement in all cases:

![addr2line runtime](benchmark-time.svg)

## License

Licensed under either of

  * Apache License, Version 2.0 ([`LICENSE-APACHE`](./LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
  * MIT license ([`LICENSE-MIT`](./LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
