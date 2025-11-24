# miniz_oxide

A fully safe, pure rust port and replacement for the [miniz](https://github.com/richgel999/miniz) DEFLATE/zlib encoder/decoder originally written by Rich Geldreich. The main intention of this crate is to be used as a back-end for the [flate2](https://github.com/rust-lang/flate2-rs), but it can also be used on its own. Using flate2 with the default ```rust_backend``` feature provides an easy to use streaming API for miniz_oxide.

The library is fully [no_std](https://docs.rust-embedded.org/book/intro/no-std.html). By default, the `with-alloc` feature is enabled, which requires the use of the `alloc` and `collection` crates as it allocates memory.

The `std` feature additionally turns on things only available if `no_std` is not used. Currently this only means implementing [Error](https://doc.rust-lang.org/stable/std/error/trait.Error.html) for the `DecompressError` error struct returned by the simple decompression functions if enabled together with `with-alloc`.

Using the library with `default-features = false` removes the dependency on `alloc`
and `collection` crates, making it suitable for systems without an allocator.
Running without allocation reduces crate functionality:

- The `deflate` module is removed completely
- Some `inflate` functions which return a `Vec` are removed

miniz_oxide 0.8.x currently requires at least Rust 1.56.0, though to leave some room for future internal improvements the minimum version might be raised in the future though it never be made incompatible with anything more recent than the last 4 rust versions and in all likelyhood not require anything even remotely that recent unless there is a very good reason for it.

IMPORTANT! Versions prior to 0.8.4 have a [massive](https://github.com/Frommi/miniz_oxide/issues/163) regression in compression performance in versions of rust 1.81 and newer due to a upstream regression. If miniz_oxide is part of your dependencies, please update to the latest version to avoid performance regressions!

miniz_oxide features no use of unsafe code.

miniz_oxide can optionally be made to use a simd-accelerated version of adler32 via the [simd-adler32](https://crates.io/crates/simd-adler32) crate by enabling the 'simd' feature which will give a noticeable speedup on decoding, and a smaller speedup during encoding, if the data is encoded with a zlib header. Due to the increase in performance this is recommended, though not enabled by default for compatability reasons. Additionally, due to the use of simd intrinsics, the simd-adler32 has to use unsafe. (Due to limitations in the rust standard library simd-adler32 only has explicit SIMD implementations on stable rust for x86 platforms currently but this may change in the future.)

simd-adler32 requires std support (and it's 'std' feature to be enabled, which it is by default) for runtime feature detection to work though this does *not* require the 'std' feature in miniz_oxide to be enabled.

The default setup uses the [adler2](https://crates.io/crates/adler2) crate which features no unsafe code. (a fork of the [adler](https://github.com/jonas-schievink/adler) crate as that crate is archived and no longer maintained.)

The 'serde' feature enables serialization of the decompressor struct, or a subset of it at block boundaries, allowing compression to be suspended and resumed. This is still an experimental feature that may be expanded in the future the format may still change.

## Usage
Simple compression/decompression:
```rust

use miniz_oxide::deflate::compress_to_vec;
use miniz_oxide::inflate::decompress_to_vec_with_limit;

fn roundtrip(data: &[u8]) {
    // Compress the input
    let compressed = compress_to_vec(data, 6);
    // Decompress the compressed input and limit max output size to avoid going out of memory on large/malformed input.
    let decompressed = decompress_to_vec_with_limit(compressed.as_slice(), 60000).expect("Failed to decompress!");
    // Check roundtrip succeeded
    assert_eq!(data, decompressed);
}

fn main() {
    roundtrip("Hello, world!".as_bytes());
}

```
These simple functions will do everything in one go and are thus not recommended for use cases outside of prototyping/testing as real world data can have any size and thus result in very large memory allocations for the output Vector. Consider using miniz_oxide via [flate2](https://github.com/rust-lang/flate2-rs) which makes it easy to do streaming (de)compression or the low-level streaming functions instead.
