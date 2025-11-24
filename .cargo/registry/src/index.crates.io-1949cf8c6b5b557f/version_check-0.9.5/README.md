# version\_check

[![Build Status](https://github.com/SergioBenitez/version_check/workflows/CI/badge.svg)](https://github.com/SergioBenitez/version_check/actions)
[![Current Crates.io Version](https://img.shields.io/crates/v/version_check.svg)](https://crates.io/crates/version_check)
[![rustdocs on docs.rs](https://docs.rs/version_check/badge.svg)](https://docs.rs/version_check)

This tiny crate checks that the running or installed `rustc` meets some version
requirements. The version is queried by calling the Rust compiler with
`--version`. The path to the compiler is determined first via the `RUSTC`
environment variable. If it is not set, then `rustc` is used. If that fails, no
determination is made, and calls return `None`.

## Usage

Add to your `Cargo.toml` file, typically as a build dependency:

```toml
[build-dependencies]
version_check = "0.9"
```

`version_check` is compatible and compiles with Rust 1.0.0 and beyond.

## Examples

Set a `cfg` flag in `build.rs` if the running compiler was determined to be
at least version `1.13.0`:

```rust
extern crate version_check as rustc;

if rustc::is_min_version("1.13.0").unwrap_or(false) {
    println!("cargo:rustc-cfg=question_mark_operator");
}
```

Check that the running compiler was released on or after `2018-12-18`:

```rust
extern crate version_check as rustc;

match rustc::is_min_date("2018-12-18") {
    Some(true) => "Yep! It's recent!",
    Some(false) => "No, it's older.",
    None => "Couldn't determine the rustc version."
};
```

Check that the running compiler supports feature flags:

```rust
extern crate version_check as rustc;

match rustc::is_feature_flaggable() {
    Some(true) => "Yes! It's a dev or nightly release!",
    Some(false) => "No, it's stable or beta.",
    None => "Couldn't determine the rustc version."
};
```

See the [rustdocs](https://docs.rs/version_check) for more examples and complete
documentation.

## Alternatives

This crate is dead simple with no dependencies. If you need something more and
don't care about panicking if the version cannot be obtained, or if you don't
mind adding dependencies, see [rustc_version]. If you'd instead prefer a feature
detection library that works by dynamically invoking `rustc` with a
representative code sample, see [autocfg].

[rustc_version]: https://crates.io/crates/rustc_version
[autocfg]: https://crates.io/crates/autocfg

## License

`version_check` is licensed under either of the following, at your option:

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
