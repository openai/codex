# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.8.4 (2024-02-13)
### Changed
- Assert soundness preconditions for ARMv8 key expansion ([#407], [#408])

[#407]: https://github.com/RustCrypto/block-ciphers/pull/407
[#408]: https://github.com/RustCrypto/block-ciphers/pull/408

## 0.8.3 (2023-06-17)
### Added
- Support `aes_armv8` on Rust 1.61+ using `asm!` ([#365])

[#365]: https://github.com/RustCrypto/block-ciphers/pull/365

## 0.8.2 (2022-10-27)
### Fixed
- Crate documentation around configuration flags ([#343])

[#343]: https://github.com/RustCrypto/block-ciphers/pull/343

## 0.8.1 (2022-02-17)
### Fixed
- Minimal versions build ([#303])

[#303]: https://github.com/RustCrypto/block-ciphers/pull/303

## 0.8.0 (2022-02-10)
### Changed
- Bump `cipher` dependency to v0.4 ([#284])

### Added
- Encrypt-only and decrypt-only cipher types ([#284])

[#284]: https://github.com/RustCrypto/block-ciphers/pull/284

## 0.7.5 (2021-08-26)
### Changed
- Bump `ctr` dependency to v0.8 ([#275])
- Use the `aes` target feature instead of `crypto` on ARMv8 ([#279])
- Use `core::arch::aarch64::vst1q_u8` intrinsic on `armv8` ([#280])
- Bump `cpufeatures` dependency to v0.2 ([#281])

[#275]: https://github.com/RustCrypto/block-ciphers/pull/275
[#279]: https://github.com/RustCrypto/block-ciphers/pull/279
[#280]: https://github.com/RustCrypto/block-ciphers/pull/280
[#281]: https://github.com/RustCrypto/block-ciphers/pull/281

## 0.7.4 (2021-06-01)
### Added
- Soft `hazmat` backend ([#267], [#268])
- Parallel `hazmat` APIs ([#269])

[#267]: https://github.com/RustCrypto/block-ciphers/pull/267
[#268]: https://github.com/RustCrypto/block-ciphers/pull/268
[#269]: https://github.com/RustCrypto/block-ciphers/pull/269

## 0.7.3 (2021-05-26)
### Added
- `hazmat` feature/module providing round function access ([#257], [#259], [#260])
- `BLOCK_SIZE` constant ([#263])

[#257]: https://github.com/RustCrypto/block-ciphers/pull/257
[#259]: https://github.com/RustCrypto/block-ciphers/pull/259
[#260]: https://github.com/RustCrypto/block-ciphers/pull/260
[#263]: https://github.com/RustCrypto/block-ciphers/pull/263

## 0.7.2 (2021-05-17)
### Added
- Nightly-only ARMv8 intrinsics support gated under the `armv8` feature ([#250])

[#250]: https://github.com/RustCrypto/block-ciphers/pull/250

## 0.7.1 (2021-05-09)
### Fixed
- Restore `fixslice64.rs` ([#247])

[#247]: https://github.com/RustCrypto/block-ciphers/pull/247

## 0.7.0 (2021-04-29)
### Added
- Auto-detection support for AES-NI; MSRV 1.49+ ([#208], [#214], [#215], [#216])
- `ctr` feature providing SIMD accelerated AES-CTR ([#200])

### Changed
- Unify the `aes`, `aesni`, `aes-ctr`, and `aes-soft` crates ([#200])
- Use `cfg-if` crate ([#203])
- Rename `semi_fixslice` feature to `compact` ([#204])
- Refactor NI backend ([#224], [#225])
- Bump `cipher` crate dependency to v0.3 ([#235])
- Bump `ctr` crate dependency to v0.7 ([#237])

[#200]: https://github.com/RustCrypto/block-ciphers/pull/200
[#203]: https://github.com/RustCrypto/block-ciphers/pull/203
[#204]: https://github.com/RustCrypto/block-ciphers/pull/204
[#208]: https://github.com/RustCrypto/block-ciphers/pull/208
[#214]: https://github.com/RustCrypto/block-ciphers/pull/214
[#215]: https://github.com/RustCrypto/block-ciphers/pull/215
[#216]: https://github.com/RustCrypto/block-ciphers/pull/216
[#224]: https://github.com/RustCrypto/block-ciphers/pull/224
[#225]: https://github.com/RustCrypto/block-ciphers/pull/225
[#235]: https://github.com/RustCrypto/block-ciphers/pull/235
[#237]: https://github.com/RustCrypto/block-ciphers/pull/237

## 0.6.0 (2020-10-16)
### Changed
- Replace `block-cipher`/`stream-cipher` with `cipher` crate ([#167])

[#167]: https://github.com/RustCrypto/block-ciphers/pull/167

## 0.5.1 (2020-08-25)
### Changed
- Bump `aesni` dependency to v0.9 ([#158])

[#158]: https://github.com/RustCrypto/block-ciphers/pull/158

## 0.5.0 (2020-08-07)
### Changed
- Bump `block-cipher` dependency to v0.8 ([#138])
- Bump `opaque-debug` dependency to v0.3 ([#140])

[#138]: https://github.com/RustCrypto/block-ciphers/pull/138
[#140]: https://github.com/RustCrypto/block-ciphers/pull/140

## 0.4.0 (2020-06-05)
### Changed
- Bump `block-cipher` dependency to v0.7 ([#86], [#122])
- Update to Rust 2018 edition ([#86])

[#121]: https://github.com/RustCrypto/block-ciphers/pull/122 
[#86]: https://github.com/RustCrypto/block-ciphers/pull/86

## 0.3.2 (2018-11-01)

## 0.3.1 (2018-10-04)

## 0.3.0 (2018-10-03)

## 0.2.0 (2018-07-27)

## 0.1.0 (2018-06-22)
