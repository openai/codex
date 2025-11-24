# Adler-32 checksums for Rust

This is a fork of the adler crate as the [original](https://github.com/jonas-schievink/adler) has been archived and is no longer updated by it's author

[![crates.io](https://img.shields.io/crates/v/adler.svg)](https://crates.io/crates/adler)
[![docs.rs](https://docs.rs/adler/badge.svg)](https://docs.rs/adler/)
![CI](https://github.com/jonas-schievink/adler/workflows/CI/badge.svg)

This crate provides a simple implementation of the Adler-32 checksum, used in
the zlib compression format.

Please refer to the [changelog](CHANGELOG.md) to see what changed in the last
releases.

## Features

- Permissively licensed (0BSD) clean-room implementation.
- Zero dependencies.
- Zero `unsafe`.
- Decent performance (3-4 GB/s) (see note).
- Supports `#![no_std]` (with `default-features = false`).

## Usage

Add an entry to your `Cargo.toml`:

```toml
[dependencies]
adler2 = "2.0.0"
```

Check the [API Documentation](https://docs.rs/adler/) for how to use the
crate's functionality.

## Rust version support

Currently, this crate supports all Rust versions starting at Rust 1.56.0.

Bumping the Minimum Supported Rust Version (MSRV) is *not* considered a breaking
change, but will not be done without good reasons. The latest 3 stable Rust
versions will always be supported no matter what.

## Performance

Due to the way the algorithm works this crate and the fact that it's not possible to use explicit simd in safe rust currently, this crate benefits drastically from being compiled with newer cpu instructions enabled (using e.g  ```RUSTFLAGS=-C target-feature'+sse4.1``` or ```-C target-cpu=x86-64-v2```/```-C target-cpu=x86-64-v3``` arguments depending on what cpu support is being targeted.)
Judging by the crate benchmarks, on a Ryzen 5600, compiling with SSE 4.1 (enabled in x86-64-v2 feature level) enabled can give a ~50-150% speedup, enabling the LZCNT instruction (enabled in x86-64-v3 feature level) can give a further ~50% speedup, 
