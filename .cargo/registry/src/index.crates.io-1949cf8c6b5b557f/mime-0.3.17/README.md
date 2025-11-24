# mime

[![Build Status](https://travis-ci.org/hyperium/mime.svg?branch=master)](https://travis-ci.org/hyperium/mime)
[![crates.io](https://img.shields.io/crates/v/mime.svg)](https://crates.io/crates/mime)
[![docs.rs](https://docs.rs/mime/badge.svg)](https://docs.rs/mime)

Support MIME (Media Types) as strong types in Rust.

[Documentation](https://docs.rs/mime)

## Usage

```rust
extern crate mime;

// common types are constants
let text = mime::TEXT_PLAIN;

// deconstruct Mimes to match on them
match (text.type_(), text.subtype()) {
    (mime::TEXT, mime::PLAIN) => {
        // plain text!
    },
    (mime::TEXT, _) => {
        // structured text!
    },
    _ => {
        // not text!
    }
}
```
