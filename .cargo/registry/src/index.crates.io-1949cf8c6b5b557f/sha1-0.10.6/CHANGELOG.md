# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.10.6 (2023-09-21)
### Added
- `asm!`-based backend for LoongArch64 targets gated behind `loongarch64_asm` feature [#504]

[#504]: https://github.com/RustCrypto/hashes/pull/504

## 0.10.5 (2022-09-16)
### Added
- Feature-gated OID support ([#405])

[#405]: https://github.com/RustCrypto/hashes/pull/405

## 0.10.4 (2022-09-02)
### Fixed
- MSRV issue which was not resolved by v0.10.3 ([#401])

[#401]: https://github.com/RustCrypto/hashes/pull/401

## 0.10.3 (2022-09-02)
### Fixed
- MSRV issue caused by publishing v0.10.2 using a buggy Nightly toolchain ([#399])

[#399]: https://github.com/RustCrypto/hashes/pull/399

## 0.10.2 (2022-08-30)
### Changed
- Ignore `asm` feature on unsupported targets ([#388])

[#388]: https://github.com/RustCrypto/hashes/pull/388

## 0.10.1 (2022-02-17)
### Fixed
- Minimal versions build ([#363])

[#363]: https://github.com/RustCrypto/hashes/pull/363

## 0.10.0 (2022-01-17)
### Changed
- The crate is transferred to the RustCrypto organization. New implementation is identical to the `sha-1 v0.10.0` crate and expressed in terms of traits from the `digest` crate. ([#350])

[#350]: https://github.com/RustCrypto/hashes/pull/350
