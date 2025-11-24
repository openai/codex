# hex

[![Crates.io: hex](https://img.shields.io/crates/v/hex.svg)](https://crates.io/crates/hex)
[![Documentation](https://docs.rs/hex/badge.svg)](https://docs.rs/hex)
[![Build Status (Github Actions)](https://github.com/KokaKiwi/rust-hex/workflows/Test%20hex/badge.svg?master)](https://github.com/KokaKiwi/rust-hex/actions)

Encoding and decoding data into/from hexadecimal representation.

## Examples

Encoding a `String`

```rust
let hex_string = hex::encode("Hello world!");

println!("{}", hex_string); // Prints "48656c6c6f20776f726c6421"
```

Decoding a `String`

```rust
let decoded_string = hex::decode("48656c6c6f20776f726c6421");

println!("{}", decoded_string); // Prints "Hello world!"
```

You can find the [documentation](https://docs.rs/hex) here.

## Installation

In order to use this crate, you have to add it under `[dependencies]` to your `Cargo.toml`

```toml
[dependencies]
hex = "0.4"
```

By default this will import `std`, if you are working in a
[`no_std`](https://rust-embedded.github.io/book/intro/no-std.html)
environment you can turn this off by adding the following

```toml
[dependencies]
hex = { version = "0.4", default-features = false }
```

## Features

- `std`:
  Enabled by default. Add support for Rust's libstd types.
- `serde`:
  Disabled by default. Add support for `serde` de/serializing library.
  See the `serde` module documentation for usage.

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
