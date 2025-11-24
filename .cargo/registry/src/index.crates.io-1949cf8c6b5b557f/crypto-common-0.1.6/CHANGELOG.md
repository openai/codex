# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.6 (2022-07-16)
### Added
- Move `ParBlocks`/`ParBlocksSizeUser` from `cipher` crate ([#1052])

[#1052]: https://github.com/RustCrypto/traits/pull/1052

## 0.1.5 (2022-07-09)
### Fixed
- Support on-label MSRV ([#1049])

[#1049]: https://github.com/RustCrypto/traits/pull/1049

## 0.1.4 (2022-07-02)
### Added
- `getrandom` feature ([#1034])

[#1034]: https://github.com/RustCrypto/traits/pull/1034

## 0.1.3 (2022-02-16)
### Fixed
- Minimal versions build ([#940])

[#940]: https://github.com/RustCrypto/traits/pull/940

## 0.1.2 (2022-02-10)
### Added
- Re-export `generic-array` and `typenum`. Enable `more_lengths` feature on
`generic-array`.  Add `key_size`, `iv_size`, `block_size`, and `output_size`
helper methods. ([#849])

[#849]: https://github.com/RustCrypto/traits/pull/849

## 0.1.1 (2021-12-14)
### Added
- `rand_core` re-export and proper exposure of key/IV generation methods on docs.rs ([#847])

[#847]: https://github.com/RustCrypto/traits/pull/847

## 0.1.0 (2021-12-07)
- Initial release
