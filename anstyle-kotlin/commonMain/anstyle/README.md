# anstyle

> ANSI text styling

*A portmanteau of "ansi style"*

[![Documentation](https://img.shields.io/badge/docs-master-blue.svg)][Documentation]
![License](https://img.shields.io/crates/l/anstyle.svg)
[![Crates Status](https://img.shields.io/crates/v/anstyle.svg)](https://crates.io/crates/anstyle)

`anstyle` provides core types describing [ANSI styling escape
codes](https://en.wikipedia.org/wiki/ANSI_escape_code) for interoperability
between crates.  For example, this would allow a crate to provide an API for
customizing the colors used without putting the underlying text styling crate
in the API.

For integration with your text styling crate, see:
- [anstyle-termcolor](crates/termcolor)
- [anstyle-owo-colors](crates/owo)
- [anstyle-yansi](crates/yansi)

General utilities:
- [anstyle-git](crates/git): Parse Git style descriptions

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/license/mit>)

at your option.

## [Contribute](../../CONTRIBUTING.md)

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual-licensed as above, without any additional terms or
conditions.

[Crates.io]: https://crates.io/crates/anstyle
[Documentation]: https://docs.rs/anstyle
