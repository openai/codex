tempfile
========

[![Crate](https://img.shields.io/crates/v/tempfile.svg)](https://crates.io/crates/tempfile)
[![Build Status](https://github.com/Stebalien/tempfile/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/Stebalien/tempfile/actions/workflows/ci.yml?query=branch%3Amaster)

A secure, cross-platform, temporary file library for Rust. In addition to creating
temporary files, this library also allows users to securely open multiple
independent references to the same temporary file (useful for consumer/producer
patterns and surprisingly difficult to implement securely).

[Documentation](https://docs.rs/tempfile/)

Usage
-----

Minimum required Rust version: 1.63.0

Add this to your `Cargo.toml`:

```toml
[dependencies]
tempfile = "3"
```

Example
-------

```rust
use std::fs::File;
use std::io::{Write, Read, Seek, SeekFrom};

fn main() {
    // Write
    let mut tmpfile: File = tempfile::tempfile().unwrap();
    write!(tmpfile, "Hello World!").unwrap();

    // Seek to start
    tmpfile.seek(SeekFrom::Start(0)).unwrap();

    // Read
    let mut buf = String::new();
    tmpfile.read_to_string(&mut buf).unwrap();
    assert_eq!("Hello World!", buf);
}
```
