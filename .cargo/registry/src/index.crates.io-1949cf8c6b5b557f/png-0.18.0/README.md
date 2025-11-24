# PNG Decoder/Encoder
[![Build Status](https://github.com/image-rs/image-png/workflows/Rust%20CI/badge.svg)](https://github.com/image-rs/image-png/actions)
[![Documentation](https://docs.rs/png/badge.svg)](https://docs.rs/png)
[![Crates.io](https://img.shields.io/crates/v/png.svg)](https://crates.io/crates/png)
[![License](https://img.shields.io/crates/l/png.svg)](https://github.com/image-rs/image-png)

Robust and performant PNG decoder/encoder in pure Rust. Also supports [APNG](https://en.wikipedia.org/wiki/APNG).

No `unsafe` code, battle-tested, and fuzzed on [OSS-fuzz](https://github.com/google/oss-fuzz).

## Performance

Performance is typically on par with or better than libpng.

Includes a fast encoding mode powered by [fdeflate](https://crates.io/crates/fdeflate) that is dramatically faster than the fastest mode of libpng while *simultaneously* providing better compression ratio.

On nightly Rust compiler you can slightly speed up decoding of some images by enabling the `unstable` feature of this crate.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
