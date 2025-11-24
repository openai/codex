# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.12.3 (2022-02-17)
### Fixed
- Minimal versions build ([#63])

[#63]: https://github.com/RustCrypto/KDFs/pull/63

## 0.12.2 (2022-01-27)
### Fixed
- Re-export `InvalidLength` and `InvalidPrkLength` ([#59])

[#59]: https://github.com/RustCrypto/KDFs/pull/59

## 0.12.1 (2022-01-27) [YANKED]
### Added
- Ability to switch HMAC implementation to `SimpleHmac` with respective `SimpleHkdfExtract` and `SimpleHkdf` aliases ([#57])

[#57]: https://github.com/RustCrypto/KDFs/pull/55

## 0.12.0 (2021-12-07)
### Changed
- Bump `hmac` crate dependency to v0.12 and `digest` to v0.10 ([#52])

[#52]: https://github.com/RustCrypto/KDFs/pull/52

## 0.11.0 (2021-04-29)
### Added
- Wycheproof HKDF test vectors ([#49])

### Changed
- Bump `hmac` crate dependency to v0.11 ([#50])

### Fixed
- HKDF-Extract with empty salt ([#46])

[#46]: https://github.com/RustCrypto/KDFs/pull/46
[#49]: https://github.com/RustCrypto/KDFs/pull/49
[#50]: https://github.com/RustCrypto/KDFs/pull/50

## 0.10.0 (2020-10-26)
### Changed
- Bump `hmac` dependency to v0.10 ([#40])

[#40]: https://github.com/RustCrypto/KDFs/pull/40

## 0.9.0 (2020-06-22)
### Added
- Multipart features for HKDF-Extract and HKDF-Expand ([#34])

### Changed
- Bump `digest` v0.9; `hmac` v0.9 ([#35])

[#34]: https://github.com/RustCrypto/KDFs/pull/34
[#35]: https://github.com/RustCrypto/KDFs/pull/35

## 0.8.0 (2019-07-26)
### Added
- `Hkdf::from_prk()`, `Hkdf::extract()`

## 0.7.1 (2019-07-15)

## 0.7.0 (2018-10-16)
### Changed
- Update digest to 0.8 
- Refactor for API changes

### Removed  
- Redundant `generic-array` crate.

## 0.6.0 (2018-08-20)
### Changed
- The `expand` signature has changed.
  
### Removed
- `std` requirement 

## 0.5.0 (2018-05-20)
### Fixed
- Omitting HKDF salt.

### Removed
- Deprecated interface

## 0.4.0 (2018-03-20
### Added
- Benchmarks
- derive `Clone`

### Changed
- RFC-inspired interface 
- Reduce heap allocation
- Bump deps: hex-0.3

### Removed
- Unnecessary mut

## 0.3.0 (2017-11-29)
### Changed
- update dependencies: digest-0.7, hmac-0.5

## 0.2.0 (2017-09-21)
### Fixed
- Support for rustc 1.20.0

## 0.1.2 (2017-09-21)
### Fixed
- Support for rustc 1.5.0

## 0.1.0 (2017-09-21)
- Initial release
