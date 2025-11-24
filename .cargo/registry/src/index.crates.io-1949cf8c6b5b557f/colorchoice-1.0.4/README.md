# colorchoice

> Global override of color control

[![Documentation](https://img.shields.io/badge/docs-master-blue.svg)][Documentation]
![License](https://img.shields.io/crates/l/colorchoice.svg)
[![Crates Status](https://img.shields.io/crates/v/colorchoice.svg)](https://crates.io/crates/colorchoice)

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## [Contribute](../../CONTRIBUTING.md)

Special note: to be successful, this crate **cannot** break compatibility or
else different crates in the hierarchy will be reading different globals.
While end users can work around this, it isn't ideal.    If we need a new API, we can make
the old API an adapter to the new logic.

Similarly, we should strive to reduce **risk** of breaking compatibility by
exposing as little as possible.  Anything more should be broken out into a
separate crate that this crate can call into.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual-licensed as above, without any additional terms or
conditions.

[Documentation]: https://docs.rs/colorchoice
