<div align="center">
  <h1>bit-vec</h1>
  <p>
    <strong>A vector of bits.</strong>
  </p>
  <p>

[![crates.io](https://img.shields.io/crates/v/bit-vec?label=latest)](https://crates.io/crates/bit-vec)
[![Documentation](https://docs.rs/bit-vec/badge.svg?version=0.6.2)](https://docs.rs/bit-vec/0.6.2/bit_vec/)
[![Version](https://img.shields.io/badge/rustc-1.42+-ab6000.svg)](https://blog.rust-lang.org/2020/03/12/Rust-1.42.html)
<br />
[![Dependency Status](https://deps.rs/crate/bit-vec/0.6.2/status.svg)](https://deps.rs/crate/bit-vec/0.6.2)
[![Build Status](https://travis-ci.org/contain-rs/bit-vec.svg?branch=master)](https://travis-ci.org/contain-rs/bit-vec)
[![Download Status](https://img.shields.io/crates/d/bit-vec.svg)](https://crates.io/crates/bit-vec)

  </p>
</div>

Documentation is available at https://contain-rs.github.io/bit-vec/bit_vec.

[![Build Status](https://travis-ci.org/contain-rs/bit-vec.svg?branch=master)](https://travis-ci.org/contain-rs/bit-vec)
[![crates.io](http://meritbadge.herokuapp.com/bit-vec)](https://crates.io/crates/bit-vec)

## Usage

Add this to your Cargo.toml:

```toml
[dependencies]
bit-vec = "0.6"
```

and this to your crate root:

```rust
extern crate bit_vec;
```

If you want [serde](https://github.com/serde-rs/serde) support, include the feature like this:

```toml
[dependencies]
bit-vec = { version = "0.6", features = ["serde"] }
```

If you want to use bit-vec in a program that has `#![no_std]`, just drop default features:

```toml
[dependencies]
bit-vec = { version = "0.6", default-features = false }
```
