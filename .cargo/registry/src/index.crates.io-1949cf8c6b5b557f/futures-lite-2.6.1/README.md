# futures-lite

[![Build](https://github.com/smol-rs/futures-lite/workflows/Build%20and%20test/badge.svg)](
https://github.com/smol-rs/futures-lite/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](
https://github.com/smol-rs/futures-lite)
[![Cargo](https://img.shields.io/crates/v/futures-lite.svg)](
https://crates.io/crates/futures-lite)
[![Documentation](https://docs.rs/futures-lite/badge.svg)](
https://docs.rs/futures-lite)

A lightweight async prelude.

This crate is a subset of [futures] that compiles an order of magnitude faster, fixes minor
warts in its API, fills in some obvious gaps, and removes almost all unsafe code from it.

In short, this crate aims to be more enjoyable than [futures] but still fully compatible with
it.

The API for this crate is intentionally constrained. Please consult the 
[features list] for APIs that are occluded from this crate.

[futures]: https://docs.rs/futures
[features list]: https://github.com/smol-rs/futures-lite/blob/master/FEATURES.md

## Examples

```rust
use futures_lite::future;

fn main() {
    future::block_on(async {
        println!("Hello world!");
    })
}
```

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
