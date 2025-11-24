bitflags
========

[![Rust](https://github.com/bitflags/bitflags/workflows/Rust/badge.svg)](https://github.com/bitflags/bitflags/actions)
[![Join the chat at https://gitter.im/bitflags/Lobby](https://badges.gitter.im/Join%20Chat.svg)](https://gitter.im/bitflags/Lobby?utm_source=badge&utm_medium=badge&utm_content=badge)
[![Latest version](https://img.shields.io/crates/v/bitflags.svg)](https://crates.io/crates/bitflags)
[![Documentation](https://docs.rs/bitflags/badge.svg)](https://docs.rs/bitflags)
![License](https://img.shields.io/crates/l/bitflags.svg)

A Rust macro to generate structures which behave like a set of bitflags

- [Documentation](https://docs.rs/bitflags)
- [Release notes](https://github.com/bitflags/bitflags/releases)

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
bitflags = "1.3"
```

and this to your source code:

```rust
use bitflags::bitflags;
```

## Rust Version Support

The minimum supported Rust version is 1.46 due to use of associated constants and const functions.
