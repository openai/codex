# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.63] - 2025-03-31
### Changes
- Bump MSRV (minimum supported rust version) to 1.62 ([#131](https://github.com/strawlab/iana-time-zone/pull/131))
- Bump `windows-core` to `0.56-0.61` range ([#131](https://github.com/strawlab/iana-time-zone/pull/131), [#133](https://github.com/strawlab/iana-time-zone/pull/133))

## [0.1.62] - 2025-03-24
### Changed
- Bump MSRV (minimum supported rust version) to 1.61 ([#157](https://github.com/strawlab/iana-time-zone/pull/157))
- Update to rust edition 2021 ([#161](https://github.com/strawlab/iana-time-zone/pull/161))
- Address high and medium severity zizmor findings ([#163](https://github.com/strawlab/iana-time-zone/pull/163))

### Added
- Added support for tvOS, watchOS and visionOS ([#146](https://github.com/strawlab/iana-time-zone/pull/146)).
- implement OpenHarmony support ([#150](https://github.com/strawlab/iana-time-zone/pull/150))

## [0.1.61] - 2024-09-16
### Changed

- Depend on wasm-bindgen 0.2.89 or higher ([#134](https://github.com/strawlab/iana-time-zone/pull/134))
- Do not use wasm_bindgen in wasm32-unknown-emscripten environment ([#130](https://github.com/strawlab/iana-time-zone/pull/130))

## [0.1.60] - 2024-02-03
### Changed
- correct `windows-core` dependency version ([#127](https://github.com/strawlab/iana-time-zone/pull/127))

## [0.1.59] - 2023-12-30
### Changed
- update `windows` dependency ([#125](https://github.com/strawlab/iana-time-zone/pull/125))

## [0.1.58] - 2023-10-17
### Added
- use windows-core with embedded bindings via windows-bindgen ([#117](https://github.com/strawlab/iana-time-zone/pull/117))
- implement GNU Hurd support ([#121](https://github.com/strawlab/iana-time-zone/pull/121))
- implement AIX support ([#57](https://github.com/strawlab/iana-time-zone/pull/57))

## [0.1.57] - 2023-06-07
### Added
- implement OpenWRT support ([#109](https://github.com/strawlab/iana-time-zone/pull/109))

## [0.1.56] - 2023-04-03
### Changed
- update `windows` dependency ([#102](https://github.com/strawlab/iana-time-zone/pull/102))

## [0.1.55] - 2023-03-30
### Changed
- update `windows` dependency ([#101](https://github.com/strawlab/iana-time-zone/pull/101))

## [0.1.54] - 2023-03-21
### Changed
- replace `winapi` dependency with `windows` ([#97](https://github.com/strawlab/iana-time-zone/pull/97))
- bump msrv to 1.48 ([#91](https://github.com/strawlab/iana-time-zone/pull/91))

## [0.1.53] - 2022-10-28
### Fixed
- remove lint causing breakage on rust 1.45-1.51 ([#84](https://github.com/strawlab/iana-time-zone/pull/84))

## [0.1.52] - 2022-10-28
### Fixed
- fix for NixOS ([#81](https://github.com/strawlab/iana-time-zone/pull/81))

### Changed
- allow building the haiku crate on other hosts([#75](https://github.com/strawlab/iana-time-zone/pull/75))
- various improvements in continuous integration and source quality
  ([#76](https://github.com/strawlab/iana-time-zone/pull/76)),
  ([#77](https://github.com/strawlab/iana-time-zone/pull/77)),
  ([#78](https://github.com/strawlab/iana-time-zone/pull/78)),
  ([#81](https://github.com/strawlab/iana-time-zone/pull/81))

## [0.1.51] - 2022-10-08
### Changed
- bump MSRV to 1.38 ([#70](https://github.com/strawlab/iana-time-zone/pull/70))
- Refactor Android property key CStr construction to add tests ([#69](https://github.com/strawlab/iana-time-zone/pull/69))
- Refactor MacOS implementation a lot ([#67](https://github.com/strawlab/iana-time-zone/pull/67))

### Added
- Implement for Haiku ([#66](https://github.com/strawlab/iana-time-zone/pull/66))

### Fixed
- Fix spelling of 'initialized' in sync::Once statics ([#63](https://github.com/strawlab/iana-time-zone/pull/63))

## [0.1.50] - 2022-09-23
### Fixed
- Reduce MSRV for Android again ([#62](https://github.com/strawlab/iana-time-zone/pull/62))

## [0.1.49] - 2022-09-22
### Changed
- `once_cell` dependency is not needed ([#61](https://github.com/strawlab/iana-time-zone/pull/61))

## [0.1.48] - 2022-09-12
### Changed
- Downgrade requirements for WASM dependencies ([#58](https://github.com/strawlab/iana-time-zone/pull/58))
- Reduce MSRV for Tier 1 platforms to 1.31 ([#59](https://github.com/strawlab/iana-time-zone/pull/59))

## [0.1.47] - 2022-08-30
### Changed
- Update `android_system_properties` to v0.1.5 to run 9786% faster (YMMV) ([#56](https://github.com/strawlab/iana-time-zone/pull/56))

## [0.1.46] - 2022-08-18
### Added
- Implement for Solaris ([#55](https://github.com/strawlab/iana-time-zone/pull/55))

## [0.1.45] - 2022-08-16
### Fixed
- Fix potential use after free in MacOS / iOS ([#54](https://github.com/strawlab/iana-time-zone/pull/54), [RUSTSEC-2022-0049](https://rustsec.org/advisories/RUSTSEC-2022-0049.html))
- Fix typos in README ([#53](https://github.com/strawlab/iana-time-zone/pull/53))

## [0.1.44] - 2022-08-11
### Fixed
- "/etc/localtime" may be relative link ([#49](https://github.com/strawlab/iana-time-zone/pull/49))

## [0.1.43] - 2022-08-11
### Changed
- Use `core-foundation-sys` instead of `core-foundation` ([#50](https://github.com/strawlab/iana-time-zone/pull/50))

## [0.1.42] - 2022-08-10
### Fixed
- Fix implementation for Redhat based distros ([#48](https://github.com/strawlab/iana-time-zone/pull/48))

## [0.1.41] - 2022-08-02
### Added
- Add `fallback` feature ([#46](https://github.com/strawlab/iana-time-zone/pull/46))

## [0.1.40] - 2022-07-29
### Added
- Implement for Android ([#45](https://github.com/strawlab/iana-time-zone/pull/45))

## [0.1.38] - 2022-07-27
### Added
- Implement illumos ([#44](https://github.com/strawlab/iana-time-zone/pull/44))
### Changed
- Update examples in README

## [0.1.37] - 2022-07-23
### Added
- Support iOS ([#41](https://github.com/strawlab/iana-time-zone/pull/41))
### Changed
- Implement `std::err::source()`, format `IoError` ([#42](https://github.com/strawlab/iana-time-zone/pull/42))

## [0.1.36] - 2022-07-21
### Fixed
- Fail to compile for WASI ([#40](https://github.com/strawlab/iana-time-zone/pull/40))

## [0.1.35] - 2022-06-29
### Added
- Implement for FreeBSD, NetBSD, OpenBSD and Dragonfly ([#39](https://github.com/strawlab/iana-time-zone/pull/39))

## [0.1.34] - 2022-06-29
### Added
- Implement for wasm32 ([#38](https://github.com/strawlab/iana-time-zone/pull/38))

## [0.1.33] - 2022-04-15
### Changed
- Use `winapi` crate instead of `windows` crate ([#35](https://github.com/strawlab/iana-time-zone/pull/35))

## [0.1.32] - 2022-04-06
### Changed
- Update `windows` requirement from 0.34 to 0.35 ([#34](https://github.com/strawlab/iana-time-zone/pull/34))

## [0.1.31] - 2022-03-16
### Changed
- Update `windows` requirement from 0.33 to 0.34 ([#33](https://github.com/strawlab/iana-time-zone/pull/33))

## [0.1.30] - 2022-02-28
### Changed
- Fewer string allocations ([#32](https://github.com/strawlab/iana-time-zone/pull/32))

## [0.1.29] - 2022-02-25
### Changed
- Update `windows` requirement from 0.32 to 0.33 ([#31](https://github.com/strawlab/iana-time-zone/pull/31))

## [0.1.28] - 2022-02-04
### Changed
- Update `windows` requirement from 0.30 to 0.32 ([#30](https://github.com/strawlab/iana-time-zone/pull/30))

## [0.1.27] - 2022-01-14
### Changed
- Update `windows` requirement from 0.29 to 0.30 ([#29](https://github.com/strawlab/iana-time-zone/pull/29))

## [0.1.26] - 2021-12-23
### Changed
- Update `windows` requirement from 0.28 to 0.29 ([#28](https://github.com/strawlab/iana-time-zone/pull/28))

## [0.1.25] - 2021-11-18
### Changed
- Update `windows` requirement from 0.27 to 0.28 ([#27](https://github.com/strawlab/iana-time-zone/pull/27))

## [0.1.24] - 2021-11-16
### Changed
- Update `windows` requirement from 0.26 to 0.27 ([#26](https://github.com/strawlab/iana-time-zone/pull/26))

## [0.1.23] - 2021-11-12
### Changed
- Update `windows` requirement from 0.25 to 0.26 ([#25](https://github.com/strawlab/iana-time-zone/pull/25))

## [0.1.22] - 2021-11-08
### Changed
- Update `windows` requirement from 0.24 to 0.25 ([#24](https://github.com/strawlab/iana-time-zone/pull/24))

## [0.1.21] - 2021-11-02
### Changed
- Update `windows` requirement from 0.23 to 0.24 ([#23](https://github.com/strawlab/iana-time-zone/pull/23))

## [0.1.20] - 2021-10-29
### Changed
- Update `windows` requirement from 0.21 to 0.23 ([#22](https://github.com/strawlab/iana-time-zone/pull/22))

## [0.1.19] - 2021-09-27
### Changed
- Update `windows` requirement from 0.19 to 0.21 ([#18](https://github.com/strawlab/iana-time-zone/pull/18), [#20](https://github.com/strawlab/iana-time-zone/pull/20))
- Update `chrono-tz` requirement from 0.5 to 0.6 ([#19](https://github.com/strawlab/iana-time-zone/pull/19))

## [0.1.18] - 2021-08-23
### Changed
- Update `windows` requirement from 0.18 to 0.19 ([#17](https://github.com/strawlab/iana-time-zone/pull/17))

## [0.1.16] - 2021-07-26
### Changed
- Update `windows` requirement from 0.17 to 0.18 ([#16](https://github.com/strawlab/iana-time-zone/pull/16))

## [0.1.15] - 2021-07-08
### Changed
- Update `windows` requirement from 0.14 to 0.17 ([#15](https://github.com/strawlab/iana-time-zone/pull/15))

## [0.1.14] - 2021-07-07
### Changed
- Update `windows` requirement from 0.13 to 0.14 ([#14](https://github.com/strawlab/iana-time-zone/pull/14))

## [0.1.13] - 2021-06-28
### Changed
- Update `windows` requirement from 0.12 to 0.13 ([#13](https://github.com/strawlab/iana-time-zone/pull/13))

## [0.1.12] - 2021-06-28
### Changed
- Update `windows` requirement from 0.11 to 0.12 ([#12](https://github.com/strawlab/iana-time-zone/pull/12))

## [0.1.11] - 2021-06-12
### Changed
- Update `windows` requirement from 0.10 to 0.11 ([#11](https://github.com/strawlab/iana-time-zone/pull/11))

## [0.1.10] - 2021-05-13
### Changed
- Update `windows` requirement from 0.9 to 0.10 ([#10](https://github.com/strawlab/iana-time-zone/pull/10))

## [0.1.9] - 2021-04-28
### Changed
- Update `windows` requirement from 0.8 to 0.9 ([#8](https://github.com/strawlab/iana-time-zone/pull/8))

## [0.1.8] - 2021-04-13
### Changed
- Update `windows` requirement from 0.7 to 0.8 ([#7](https://github.com/strawlab/iana-time-zone/pull/7))

## [0.1.7] - 2021-03-30
### Changed
- Update `windows` requirement from 0.6 to 0.7 ([#6](https://github.com/strawlab/iana-time-zone/pull/6))

## [0.1.6] - 2021-03-24
### Changed
- Update `windows` requirement from 0.5 to 0.6 ([#5](https://github.com/strawlab/iana-time-zone/pull/5))

## [0.1.5] - 2021-03-20
### Changed
- Update `windows` requirement from 0.4 to 0.5 ([#4](https://github.com/strawlab/iana-time-zone/pull/4))

## [0.1.4] - 2021-03-11
### Changed
- Update `windows` requirement from 0.3 to 0.4 ([#3](https://github.com/strawlab/iana-time-zone/pull/3))

## [0.1.3] - 2021-02-22
### Changed
- Use `windows` crate instead of `winrt`

## [0.1.2] - 2020-10-09
### Changed
- Update `core-foundation` requirement from 0.7 to 0.9 ([#1](https://github.com/strawlab/iana-time-zone/pull/1))

## [0.1.1] - 2020-06-27
### Changed
- Update `core-foundation` requirement from 0.5 to 0.7

## [0.1.0] - 2020-06-27
### Added
- Implement for Linux, Windows, MacOS

[0.1.63]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.63
[0.1.62]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.62
[0.1.61]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.61
[0.1.60]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.60
[0.1.59]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.59
[0.1.58]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.58
[0.1.57]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.57
[0.1.56]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.56
[0.1.55]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.55
[0.1.54]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.54
[0.1.53]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.53
[0.1.52]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.52
[0.1.51]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.51
[0.1.50]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.50
[0.1.49]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.49
[0.1.48]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.48
[0.1.47]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.47
[0.1.46]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.46
[0.1.45]: https://github.com/strawlab/iana-time-zone/releases/tag/v0.1.45
[0.1.44]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.44
[0.1.43]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.43
[0.1.42]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.42
[0.1.41]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.41
[0.1.40]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.40
[0.1.39]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.39
[0.1.38]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.38
[0.1.37]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.37
[0.1.36]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.36
[0.1.35]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.35
[0.1.34]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.34
[0.1.33]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.33
[0.1.32]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.32
[0.1.31]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.31
[0.1.30]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.30
[0.1.29]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.29
[0.1.28]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.28
[0.1.27]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.27
[0.1.26]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.26
[0.1.25]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.25
[0.1.24]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.24
[0.1.23]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.23
[0.1.22]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.22
[0.1.21]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.21
[0.1.20]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.20
[0.1.19]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.19
[0.1.18]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.18
[0.1.17]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.17
[0.1.16]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.16
[0.1.15]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.15
[0.1.14]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.14
[0.1.13]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.13
[0.1.12]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.12
[0.1.11]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.11
[0.1.10]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.10
[0.1.9]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.9
[0.1.8]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.8
[0.1.7]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.7
[0.1.6]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.6
[0.1.5]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.5
[0.1.4]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.4
[0.1.3]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.3
[0.1.2]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.2
[0.1.1]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.1
[0.1.0]: https://github.com/strawlab/iana-time-zone/releases/tag/0.1.0
