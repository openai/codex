# Changelog
## [Unreleased]

## [0.1.5] - 2024-09-22

### Bug fixes
* Fix Undefined Behavior in WebAssembly SIMD128 (#86) (thanks @CryZe)
* Documentation and clippy fixes (thanks @rtfeldman, @jqnatividad, @rhysd)

### Performance
* WASM: Don't use u8x16_bitmask for ASCII Check (#79) (thanks @CryZe)

## [0.1.4] - 2022-04-02

### New features
* WASM (wasm32) support

### Improvements
* Make aarch64 SIMD implementation work on Rust 1.59/1.60 with create feature `aarch64_neon`
* For Rust Nightly the aarch64 SIMD implementation is enabled out of the box.
* Starting with Rust 1.61 the aarch64 SIMD implementation is expected to be enabled out of the box as well.

### Performance
* Prefetch was disabled for aarch64 since the requisite intrinsics have not been stabilized.

## [0.1.3] - 2021-05-14
### New features
* Low-level streaming validation API in `simdutf8::basic::imp`

## [0.1.2] - 2021-05-09
### New features
* Aarch64 support (e.g. Apple Silicon, Raspberry Pi 4, ...) with nightly Rust and crate feature `aarch64_neon`

### Performance
* Another speedup on pure ASCII data
* Aligned reads have been removed as the performance was worse overall.
* Prefetch is used selectively on AVX 2, where it provides a slight benefit on some Intel CPUs.

[Comparison vs v0.1.1 on x86-64](https://user-images.githubusercontent.com/3736990/117568946-7a2fdb00-b0c3-11eb-936e-358850f0a9ad.png)

### Other
* Refactored SIMD integration to allow easy implementation for new architectures
* Full test coverage
* Thoroughly fuzz-tested

## [0.1.1] - 2021-04-26
### Performance
* Large speedup on small inputs from delegation to std lib
* Up to 50% better peak throughput on ASCII
* `#[inline]` main entry points for a small general speedup.

[Benchmark against v0.1.0](https://user-images.githubusercontent.com/3736990/116128298-12dc5900-a6c9-11eb-8c23-a117b3e57edb.png)

### Other
* Make both Utf8Error variants implement `std::error::Error`
* Make `basic::Utf8Error` implement `core::fmt::Display`
* Document Minimum Supported Rust Version (1.38.0).
* Reduce package size.
* Documentation updates

## [0.1.0] - 2021-04-21
- Documentation updates only.

0.1.x releases will have API compatibility.

## [0.0.3] - 2021-04-21
- Documentation update only.

## [0.0.2] - 2021-04-20
- Documentation update only.

## [0.0.1] - 2021-04-20
- Initial release.

[Unreleased]: https://github.com/rusticstuff/simdutf8/compare/v0.1.5...HEAD
[0.1.5]: https://github.com/rusticstuff/simdutf8/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/rusticstuff/simdutf8/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/rusticstuff/simdutf8/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/rusticstuff/simdutf8/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/rusticstuff/simdutf8/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/rusticstuff/simdutf8/compare/v0.0.3...v0.1.0
[0.0.3]: https://github.com/rusticstuff/simdutf8/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/rusticstuff/simdutf8/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/rusticstuff/simdutf8/releases/tag/v0.0.1
