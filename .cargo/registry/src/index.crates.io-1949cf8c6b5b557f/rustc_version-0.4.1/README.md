rustc-version-rs
================

[![Documentation](https://docs.rs/rustc_version/badge.svg)](https://docs.rs/rustc_version/)
[![Crates.io](https://img.shields.io/crates/v/rustc_version.svg)](https://crates.io/crates/rustc_version)
[![Build status](https://github.com/djc/rustc-version-rs/workflows/CI/badge.svg)](https://github.com/djc/rustc-version-rs/actions?query=workflow%3ACI)

A library for querying the version of a `rustc` compiler.

This can be used by build scripts or other tools dealing with Rust sources
to make decisions based on the version of the compiler. Current MSRV is 1.32.0.

If this is of interest, also consider looking at these other crates:

* [autocfg](https://crates.io/crates/autocfg/), which helps with feature detection instead of depending on compiler versions
* [rustversion](https://github.com/dtolnay/rustversion) provides a procedural macro with no other dependencies

# Getting Started

[rustc-version-rs is available on crates.io](https://crates.io/crates/rustc_version).
It is recommended to look there for the newest released version, as well as links to the newest builds of the docs.

At the point of the last update of this README, the latest published version could be used like this:

Add the following dependency to your Cargo manifest...

```toml
[build-dependencies]
rustc_version = "0.2"
```

... and see the [docs](https://docs.rs/rustc_version) for how to use it.

# Example

```rust
// This could be a cargo build script

use rustc_version::{version, version_meta, Channel, Version};

fn main() {
    // Assert we haven't travelled back in time
    assert!(version().unwrap().major >= 1);

    // Set cfg flags depending on release channel
    match version_meta().unwrap().channel {
        Channel::Stable => {
            println!("cargo:rustc-cfg=RUSTC_IS_STABLE");
        }
        Channel::Beta => {
            println!("cargo:rustc-cfg=RUSTC_IS_BETA");
        }
        Channel::Nightly => {
            println!("cargo:rustc-cfg=RUSTC_IS_NIGHTLY");
        }
        Channel::Dev => {
            println!("cargo:rustc-cfg=RUSTC_IS_DEV");
        }
    }

    // Check for a minimum version
    if version().unwrap() >= Version::parse("1.4.0").unwrap() {
        println!("cargo:rustc-cfg=compiler_has_important_bugfix");
    }
}
```

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
