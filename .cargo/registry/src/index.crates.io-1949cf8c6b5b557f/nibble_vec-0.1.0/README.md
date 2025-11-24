NibbleVec
====

[![Build Status](https://travis-ci.org/michaelsproul/rust_nibble_vec.svg)](https://travis-ci.org/michaelsproul/rust_nibble_vec)

Data-structure for storing a sequence of half-bytes.

Wraps a `Vec<u8>`, providing safe and memory-efficient storage of 4-bit values.

In terms of supported operations, the structure behaves kind of like a fixed length array, in that insertions into the middle of the vector are difficult (and unimplemented at present).

## Usage

This code is available on the Rust package host:

https://crates.io/crates/nibble_vec

You can use it in your own projects by adding `nibble_vec` as a dependency in your `Cargo.toml`.

```toml
[dependencies]
nibble_vec = "0.0.5"
```

## Documentation

API documentation is available here:

https://docs.rs/nibble_vec

## Contributors

Made by:

* Alexander Andreev ([@andreevlex](https://github.com/andreevlex))
* Devin Ragotzy ([@DevinR528](https://github.com/DevinR528))
* Michael Sproul ([@michaelsproul](https://github.com/michaelsproul))
* Vinzent Steinberg ([@vks](https://github.com/vks))

## License

MIT License. Copyright © Michael Sproul and contributors 2015-present.
