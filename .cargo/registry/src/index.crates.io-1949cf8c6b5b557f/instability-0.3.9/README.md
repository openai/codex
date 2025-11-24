# Instability

Rust API stability attributes for the rest of us.

[![Crate Badge]][Crate] [![Build Badge]][Build] [![Docs Badge]][Docs] [![License Badge]][License]
![MSRV Badge]

## Overview

This crate provides attribute macros for specifying API stability of public API items of a crate. It
is a [fork] of the [Stability] original created by Stephen M. Coakley ([@sagebind]).

## Usage

Add the `instability` crate to your `Cargo.toml` file:

```shell
cargo add instability
```

Then, use the `#[instability::stable]` and `#[instability::unstable]` attributes to specify the
stability of your API items:

```rust
/// This function does something really risky!
#[instability::unstable(feature = "risky-function")]
pub fn risky_function() {
    println!("This function is unstable!");
}

/// This function is safe to use!
#[instability::stable(since = "1.0.0")]
pub fn stable_function() {
    println!("This function is stable!");
}
```

A feature flag prefixed with "unstable-" will be created that can be used to enable unstable items.
The macro will append an extra documentation comment that describes the stability of the item. The
visibility of the item will be changed to `pub(crate)` when the feature is not enabled (or when the
attribute is on an impl block, the entire block will be removed).

Check out the [Docs] for detailed usage. See [instability-example] for a complete example.

## MSRV

The minimum supported Rust version (MSRV) is 1.64.0.

## License

This project's source code and documentation are licensed under the MIT [License].

[Crate Badge]: https://img.shields.io/crates/v/instability
[Build Badge]: https://img.shields.io/github/actions/workflow/status/ratatui/instability/check.yml
[Docs Badge]: https://img.shields.io/docsrs/instability
[License Badge]: https://img.shields.io/crates/l/instability
[MSRV Badge]: https://img.shields.io/crates/msrv/instability
[Crate]: https://crates.io/crates/instability
[Build]: https://github.com/ratatui/instability/actions/workflows/check.yml
[Docs]: https://docs.rs/instability
[License]: ./LICENSE.md
[stability]: https://crates.io/crates/stability
[@Sagebind]: https://github.com/sagebind
[fork]: https://github.com/sagebind/stability/issues/12
[instability-example]: https://crates.io/crates/instability-example
