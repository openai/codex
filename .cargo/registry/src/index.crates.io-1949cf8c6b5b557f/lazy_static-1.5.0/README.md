lazy-static.rs
==============

A macro for declaring lazily evaluated statics in Rust.

Using this macro, it is possible to have `static`s that require code to be
executed at runtime in order to be initialized.
This includes anything requiring heap allocations, like vectors or hash maps,
as well as anything that requires non-const function calls to be computed.

[![Rust](https://github.com/rust-lang-nursery/lazy-static.rs/actions/workflows/rust.yml/badge.svg)](https://github.com/rust-lang-nursery/lazy-static.rs/actions/workflows/rust.yml)
[![Latest version](https://img.shields.io/crates/v/lazy_static.svg)](https://crates.io/crates/lazy_static)
[![Documentation](https://docs.rs/lazy_static/badge.svg)](https://docs.rs/lazy_static)
[![License](https://img.shields.io/crates/l/lazy_static.svg)](https://github.com/rust-lang-nursery/lazy-static.rs#license)

## Minimum supported `rustc`

`1.40.0+`

This version is explicitly tested in CI and may only be bumped in new minor versions. Any changes to the supported minimum version will be called out in the release notes.


# Getting Started

[lazy-static.rs is available on crates.io](https://crates.io/crates/lazy_static).
It is recommended to look there for the newest released version, as well as links to the newest builds of the docs.

At the point of the last update of this README, the latest published version could be used like this:

Add the following dependency to your Cargo manifest...

```toml
[dependencies]
lazy_static = "1.5.0"
```

...and see the [docs](https://docs.rs/lazy_static) for how to use it.

# Example

```rust
use lazy_static::lazy_static;
use std::collections::HashMap;

lazy_static! {
    static ref HASHMAP: HashMap<u32, &'static str> = {
        let mut m = HashMap::new();
        m.insert(0, "foo");
        m.insert(1, "bar");
        m.insert(2, "baz");
        m
    };
}

fn main() {
    // First access to `HASHMAP` initializes it
    println!("The entry for `0` is \"{}\".", HASHMAP.get(&0).unwrap());

    // Any further access to `HASHMAP` just returns the computed value
    println!("The entry for `1` is \"{}\".", HASHMAP.get(&1).unwrap());
}
```

# Standard library

It is now possible to easily replicate this crate's functionality in Rust's standard library with [`std::sync::OnceLock`](https://doc.rust-lang.org/std/sync/struct.OnceLock.html). The example above could be also be written as:

```rust
use std::collections::HashMap;
use std::sync::OnceLock;

fn hashmap() -> &'static HashMap<u32, &'static str> {
    static HASHMAP: OnceLock<HashMap<u32, &str>> = OnceLock::new();
    HASHMAP.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert(0, "foo");
        m.insert(1, "bar");
        m.insert(2, "baz");
        m
    })
}

fn main() {
    // First access to `HASHMAP` initializes it
    println!("The entry for `0` is \"{}\".", hashmap().get(&0).unwrap());

    // Any further access to `HASHMAP` just returns the computed value
    println!("The entry for `1` is \"{}\".", hashmap().get(&1).unwrap());
}
```

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
