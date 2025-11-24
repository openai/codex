# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.17 (2025-01-24)
### Fixed
- Don't link `std` when linking `libc` ([#1142])

[#1142]: https://github.com/RustCrypto/utils/pull/1142

## 0.2.16 (2024-11-22)
### Fixed
- `cfg` for freestanding x86 targets ([#1137])

[#1137]: https://github.com/RustCrypto/utils/pull/1137

## 0.2.15 (2024-11-11)
### Fixed
- Relax XSAVE checks ([#1130])

[#1130]: https://github.com/RustCrypto/utils/pull/1130

## 0.2.14 (2024-09-05)
### Added
- Support for detecting Data Independent Timing (DIT) on AArch64 ([#1100], [#1101])

[#1100]: https://github.com/RustCrypto/utils/pull/1100
[#1101]: https://github.com/RustCrypto/utils/pull/1101

## 0.2.13 (2024-08-12)
### Changed
- Use `#[cold]` for initialization code ([#1096])

[#1096]: https://github.com/RustCrypto/utils/pull/1096

## 0.2.12 (2024-01-04)
### Added
- Support for x86-64 AVX-512 target features: `gfni`, `vaes`, `vpclmulqdq`, `avx512bitalg`, `avx512vpopcntdq` ([#1035])

[#1035]: https://github.com/RustCrypto/utils/pull/1035

## 0.2.11 (2023-10-26)
### Added
- Support for AArch64's `sm4` target feature ([#972])

[#972]: https://github.com/RustCrypto/utils/pull/972

## 0.2.10 (2023-10-20)
### Added
- LoongArch64 target support ([#955])

[#955]: https://github.com/RustCrypto/utils/pull/955

## 0.2.9 (2023-07-05)
### Added
- Support for `avx512vbmi` and `avx512vbmi2` target features ([#926])

[#926]: https://github.com/RustCrypto/utils/pull/926

## 0.2.8 (2023-06-15)
### Fixed
- Check OS register support on x86 targets ([#919])

[#919]: https://github.com/RustCrypto/utils/issues/919

## 0.2.7 (2023-04-20)
### Added
- Support freestanding/UEFI `x86` targets ([#821])

[#821]: https://github.com/RustCrypto/utils/issues/821

## 0.2.6 (2023-03-24)
### Added
- Support dynamic feature detection on iOS and derivative platforms ([#848])
- Support for detecting AVX-512 target features ([#862])

[#848]: https://github.com/RustCrypto/utils/issues/848
[#862]: https://github.com/RustCrypto/utils/pull/862

## 0.2.5 (2022-09-04)
### Fixed
- Add workaround for [CPUID bug] in `std` ([#800])

[CPUID bug]: https://github.com/rust-lang/rust/issues/101346
[#800]: https://github.com/RustCrypto/utils/pull/800

## 0.2.4 (2022-08-22) [YANKED]
- Re-release v0.2.3 without any changes to fix [#795] ([#796])

[#795]: https://github.com/RustCrypto/utils/issues/795
[#796]: https://github.com/RustCrypto/utils/pull/796

## 0.2.3 (2022-08-18) [YANKED]
### Changed
- Update `libc` version to v0.2.95 ([#789])
- Disable all target features under MIRI ([#779])
- Check AVX availability when detecting AVX2 and FMA ([#792])

[#779]: https://github.com/RustCrypto/utils/pull/779
[#789]: https://github.com/RustCrypto/utils/pull/789
[#792]: https://github.com/RustCrypto/utils/pull/792

## 0.2.2 (2022-03-18) [YANKED]
### Added
- Support for Android on `aarch64` ([#752])

### Removed
- Vestigial code around `crypto` target feature ([#600])

[#600]: https://github.com/RustCrypto/utils/pull/600
[#752]: https://github.com/RustCrypto/utils/pull/752

## 0.2.1 (2021-08-26) [YANKED]
### Changed
- Revert [#583] "Use from_bytes_with_nul for string check" ([#597])

[#583]: https://github.com/RustCrypto/utils/pull/583
[#597]: https://github.com/RustCrypto/utils/pull/597

## 0.2.0 (2021-08-26) [YANKED]
### Removed
- AArch64 `crypto` target feature ([#594])

[#594]: https://github.com/RustCrypto/utils/pull/594

## 0.1.5 (2021-06-21)
### Added
- iOS support ([#435], [#501])

### Changed
- Map `aarch64` HWCAPs to target features; add `crypto` ([#456])

[#435]: https://github.com/RustCrypto/utils/pull/435
[#456]: https://github.com/RustCrypto/utils/pull/456
[#501]: https://github.com/RustCrypto/utils/pull/501

## 0.1.4 (2021-05-14)
### Added
- Support compiling on non-Linux/macOS aarch64 targets ([#408])

[#408]: https://github.com/RustCrypto/utils/pull/408

## 0.1.3 (2021-05-13)
### Removed
- `neon` on `aarch64` targets: already enabled by default ([#406])

[#406]: https://github.com/RustCrypto/utils/pull/406

## 0.1.2 (2021-05-13) [YANKED]
### Added
- `neon` feature detection on `aarch64` targets ([#403])

### Fixed
- Support for `musl`-based targets ([#403])

[#403]: https://github.com/RustCrypto/utils/pull/403

## 0.1.1 (2021-05-06)
### Added
- `aarch64` support for Linux and macOS/M4 targets ([#393])

[#393]: https://github.com/RustCrypto/utils/pull/393

## 0.1.0 (2021-04-29)
- Initial release
