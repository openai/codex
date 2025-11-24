# Castaway

Safe, zero-cost downcasting for limited compile-time specialization.

[![Crates.io](https://img.shields.io/crates/v/castaway.svg)](https://crates.io/crates/castaway)
[![Documentation](https://docs.rs/castaway/badge.svg)][documentation]
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Minimum supported Rust version](https://img.shields.io/badge/rustc-1.38+-yellow.svg)](#minimum-supported-rust-version)
[![Build](https://github.com/sagebind/castaway/workflows/ci/badge.svg)](https://github.com/sagebind/castaway/actions)

## [Documentation]

Please check out the [documentation] for details how to use Castaway and its limitations. To get you started, here is a really simple, complete example of Castaway in action:

```rust
use std::fmt::Display;
use castaway::cast;

/// Like `std::string::ToString`, but with an optimization when `Self` is
/// already a `String`.
///
/// Since the standard library is allowed to use unstable features,
/// `ToString` already has this optimization using the `specialization`
/// feature, but this isn't something normal crates can do.
pub trait FastToString {
    fn fast_to_string(&self) -> String;
}

impl<T: Display> FastToString for T {
    fn fast_to_string(&self) -> String {
        // If `T` is already a string, then take a different code path.
        // After monomorphization, this check will be completely optimized
        // away.
        if let Ok(string) = cast!(self, &String) {
            // Don't invoke the std::fmt machinery, just clone the string.
            string.to_owned()
        } else {
            // Make use of `Display` for any other `T`.
            format!("{}", self)
        }
    }
}

fn main() {
    println!("specialized: {}", String::from("hello").fast_to_string());
    println!("default: {}", "hello".fast_to_string());
}
```

## Minimum supported Rust version

The minimum supported Rust version (or _MSRV_) for Castaway is **stable Rust 1.38 or greater**, meaning we only guarantee that Castaway will compile if you use a rustc version of at least 1.38. Any changes to the supported minimum version will be called out in the release notes.

## What is this?

This is an experimental library that implements zero-cost downcasting of types that works on stable Rust. It began as a thought experiment after I had read [this pull request](https://github.com/hyperium/http/pull/369) and wondered if it would be possible to alter the behavior of a generic function based on a concrete type without using trait objects. I stumbled on the "zero-cost"-ness of my findings by accident while playing around with different implementations and examining the generated assembly of example programs.

The API is somewhat similar to [`Any`](https://doc.rust-lang.org/stable/std/any/trait.Any.html) in the standard library, but Castaway is instead focused on ergonomic compile-time downcasting rather than runtime downcasting. Unlike `Any`, Castaway _does_ support safely casting non-`'static` references in limited scenarios. If you need to store one or more `Box<?>` objects implementing some trait with the option of downcasting, you are much better off using `Any`.

## License

This project's source code and documentation is licensed under the MIT license. See the [LICENSE](LICENSE) file for details.


[documentation]: https://docs.rs/castaway
