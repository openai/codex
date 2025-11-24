# fdeflate

[![crates.io](https://img.shields.io/crates/v/fdeflate.svg)](https://crates.io/crates/fdeflate)
[![Documentation](https://docs.rs/fdeflate/badge.svg)](https://docs.rs/fdeflate)
[![Build Status](https://img.shields.io/github/actions/workflow/status/image-rs/fdeflate/rust.yml?label=Rust%20CI)](https://github.com/image-rs/fdeflate/actions)

A fast and safe deflate implementation for PNG.

This crate contains an optimized implementation of the [deflate algorithm](https://en.wikipedia.org/wiki/Deflate) tuned for PNG images.

At least on PNG data, our decoder rivals the performance of `zlib-ng` and `zlib-rs` without using any `unsafe` code.

When compressing it makes a bunch of simplifying assumptions that
drastically improve encoding speed while still being compatible with zlib:

- Exactly one block per deflate stream.
- No distance codes except for run length encoding of zeros.
- A single fixed huffman tree trained on a large corpus of PNG images.
- All huffman codes are <= 12 bits.

### Inspiration

The algorithms in this crate take inspiration from multiple sources:
* [fpnge](https://github.com/veluca93/fpnge)
* [zune-inflate](https://github.com/etemesi254/zune-image/tree/main/zune-inflate)
* [RealTime Data Compression blog](https://fastcompression.blogspot.com/2015/10/huffman-revisited-part-4-multi-bytes.html)
