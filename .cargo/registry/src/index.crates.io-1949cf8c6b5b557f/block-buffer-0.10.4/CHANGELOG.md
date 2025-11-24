# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.10.4 (2023-03-09)
### Fixed
- Unsoundness triggered by zero block size  ([#844])

[#844]: https://github.com/RustCrypto/utils/pull/844

## 0.10.3 (2022-09-04)
### Added
- `try_new` method ([#799])

[#799]: https://github.com/RustCrypto/utils/pull/799

## 0.10.2 (2021-02-08)
### Fixed
- Eliminate unreachable panic in `LazyBuffer::digest_blocks` ([#731])

[#731]: https://github.com/RustCrypto/utils/pull/731

## 0.10.1 (2021-02-05)
### Fixed
- Use `as_mut_ptr` to get a pointer for mutation in the `set_data` method ([#728])

[#728]: https://github.com/RustCrypto/utils/pull/728

## 0.10.0 (2020-12-07) [YANKED]
### Changed
- Significant reduction of number of unreachable panics. ([#671])
- Added buffer kind type parameter to `BlockBuffer`, respective marker types, and type aliases. ([#671])
- Various `BlockBuffer` method changes. ([#671])

### Removed
- `pad_with` method and dependency on `block-padding`. ([#671])

[#671]: https://github.com/RustCrypto/utils/pull/671

## 0.10.0 (2020-12-08)
### Changed
- Rename `input_block(s)` methods to `digest_block(s)`. ([#113])
- Upgrade the `block-padding` dependency to v0.3. ([#113])

### Added
- `par_xor_data`, `xor_data`, and `set_data` methods. ([#113])

### Removed
- The `input_lazy` method. ([#113])

[#113]: https://github.com/RustCrypto/utils/pull/113
