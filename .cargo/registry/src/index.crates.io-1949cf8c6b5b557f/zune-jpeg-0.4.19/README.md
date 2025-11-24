# Zune-JPEG

A fast, correct and safe jpeg decoder in pure Rust.

## Usage

The library provides a simple-to-use API for jpeg decoding
and an ability to add options to influence decoding.

### Example

```Rust
// Import the library
use zune_jpeg::JpegDecoder;
use std::fs::read;

fn main()->Result<(),DecoderErrors> {
    // load some jpeg data
    let data = read("cat.jpg").unwrap();
    // create a decoder
    let mut decoder = JpegDecoder::new(&data);
    // decode the file
    let pixels = decoder.decode()?;
}
```

The decoder supports more manipulations via `DecoderOptions`,
see additional documentation in the library.

## Goals

The implementation aims to have the following goals achieved,
in order of importance

1. Safety - Do not segfault on errors or invalid input. Panics are okay, but
   should be fixed when reported. `unsafe` is only used for SIMD intrinsics,
   and can be turned off entirely both at compile time and at runtime.
2. Speed - Get the data as quickly as possible, which means
    1. Platform intrinsics code where justifiable
    2. Carefully written platform independent code that allows the
       compiler to vectorize it.
    3. Regression tests.
    4. Watch the memory usage of the program
3. Usability - Provide utility functions like different color conversions functions.

## Non-Goals

- Bit identical results with libjpeg/libjpeg-turbo will never be an aim of this library.
  Jpeg is a lossy format with very few parts specified by the standard
  (i.e it doesn't give a reference upsampling and color conversion algorithm)

## Features

- [x] A Pretty fast 8*8 integer IDCT.
- [x] Fast Huffman Decoding
- [x] Fast color convert functions.
- [x] Support for extended colorspaces like GrayScale and RGBA
- [X] Single-threaded decoding.
- [X] Support for four component JPEGs, and esoteric color schemes like CYMK
- [X] Support for `no_std`
- [X] BGR/BGRA decoding support.

## Crate Features

| feature | on  | Capabilities                                                                                |
|---------|-----|---------------------------------------------------------------------------------------------|
| `x86`   | yes | Enables `x86` specific instructions, specifically `avx` and `sse` for accelerated decoding. |
| `std`   | yes | Enable linking to the `std` crate                                                           |

Note that the `x86` features are automatically disabled on platforms that aren't x86 during compile
time hence there is no need to disable them explicitly if you are targeting such a platform.

## Using in a `no_std` environment

The crate can be used in a `no_std` environment with the `alloc` feature.

But one is required to link to a working allocator for whatever environment the decoder
will be running on

## Debug vs release

The decoder heavily relies on platform specific intrinsics, namely AVX2 and SSE to gain speed-ups in decoding,
but they [perform poorly](https://godbolt.org/z/vPq57z13b) in debug builds. To get reasonable performance even
when compiling your program in debug mode, add this to your `Cargo.toml`:

```toml
# `zune-jpeg` package will be always built with optimizations
[profile.dev.package.zune-jpeg]
opt-level = 3
```

## Benchmarks

The library tries to be at fast as [libjpeg-turbo] while being as safe as possible.
Platform specific intrinsics help get speed up intensive operations ensuring we can almost
match [libjpeg-turbo] speeds but speeds are always +- 10 ms of this library.

For more up-to-date benchmarks, see the online repo with
benchmarks [here](https://etemesi254.github.io/assets/criterion/report/index.html)


[libjpeg-turbo]:https://github.com/libjpeg-turbo/libjpeg-turbo/

[image-rs/jpeg-decoder]:https://github.com/image-rs/jpeg-decoder/tree/master/src
