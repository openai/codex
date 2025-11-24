<p align="center">
  <img alt="futures-rs" src="https://raw.githubusercontent.com/rust-lang/futures-rs/gh-pages/assets/images/futures-rs-logo.svg?sanitize=true" width="400">
</p>

<p align="center">
  Zero-cost asynchronous programming in Rust
</p>

<p align="center">
  <a href="https://github.com/rust-lang/futures-rs/actions?query=branch%3Amaster">
    <img alt="Build Status" src="https://img.shields.io/github/actions/workflow/status/rust-lang/futures-rs/ci.yml?branch=master">
  </a>

  <a href="https://crates.io/crates/futures">
    <img alt="crates.io" src="https://img.shields.io/crates/v/futures.svg">
  </a>
</p>

<p align="center">
  <a href="https://docs.rs/futures">
    Documentation
  </a> | <a href="https://rust-lang.github.io/futures-rs/">
    Website
  </a>
</p>

`futures-rs` is a library providing the foundations for asynchronous programming in Rust.
It includes key trait definitions like `Stream`, as well as utilities like `join!`,
`select!`, and various futures combinator methods which enable expressive asynchronous
control flow.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
futures = "0.3"
```

The current `futures` requires Rust 1.56 or later.

### Feature `std`

Futures-rs works without the standard library, such as in bare metal environments.
However, it has a significantly reduced API surface. To use futures-rs in
a `#[no_std]` environment, use:

```toml
[dependencies]
futures = { version = "0.3", default-features = false }
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
