# sys-locale

[![crates.io version](https://img.shields.io/crates/v/sys-locale.svg)](https://crates.io/crates/sys-locale)
[![crate documentation](https://docs.rs/sys-locale/badge.svg)](https://docs.rs/sys-locale)
![MSRV](https://img.shields.io/badge/rustc-1.56+-blue.svg)
[![crates.io downloads](https://img.shields.io/crates/d/sys-locale.svg)](https://crates.io/crates/sys-locale)
![CI](https://github.com/1Password/sys-locale/workflows/CI/badge.svg)

A small and lightweight Rust library to get the current active locale on the system.

`sys-locale` is small library to get the current locale set for the system or application with the relevant platform APIs. The library is also `no_std` compatible, relying only on `alloc`, except on Linux and BSD.

Platform support currently includes:
- Android
- iOS (and derivatives such as watchOS, tvOS, and visionOS)
- macOS
- Linux, BSD, and other UNIX variations
- WebAssembly, for the following platforms:
    - Inside of a web browser (via the `js` feature)
    - Emscripten (via the `UNIX` backend)
    Further support for other WASM targets is dependent on upstream
    support in those target's runtimes and specifications.
- Windows

```rust
use sys_locale::get_locale;

let locale = get_locale().unwrap_or_else(|| String::from("en-US"));

println!("The current locale is {}", locale);
```

## MSRV

The Minimum Supported Rust Version is currently 1.56.0. This will be bumped to a newer stable version of Rust when needed.

## Credits

Made with ❤️ by the [1Password](https://1password.com/) team.

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
