# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] - ReleaseDate

## [0.6.12] - 2024-01-31
### Fixed
- Unsound cast to invalid type during Report downcast [by ten3roberts](https://github.com/eyre-rs/eyre/pull/143)

## [0.6.11] - 2023-12-13
### Fixed
- stale references to `Error` in docstrings [by birkenfeld](https://github.com/eyre-rs/eyre/pull/87)

### Added
- one-argument ensure!($expr) [by sharnoff](https://github.com/eyre-rs/eyre/pull/86)
- documentation on the performance characteristics of `wrap_err` vs `wrap_err_with` [by akshayknarayan](https://github.com/eyre-rs/eyre/pull/93)
    - tl;dr: `wrap_err_with` is faster unless the constructed error object already exists
- ~~automated conversion to external errors for ensure! and bail! [by j-baker](https://github.com/eyre-rs/eyre/pull/95)~~ breaking change: shelved for next major release
- eyre::Ok for generating eyre::Ok() without fully specifying the type [by kylewlacy](https://github.com/eyre-rs/eyre/pull/91)
- `OptionExt::ok_or_eyre` for yielding static `Report`s from `None` [by LeoniePhiline](https://github.com/eyre-rs/eyre/pull/125)

### New Contributors
- @sharnoff made their first contribution in https://github.com/eyre-rs/eyre/pull/86
- @akshayknarayan made their first contribution in https://github.com/eyre-rs/eyre/pull/93
- @j-baker made their first contribution in https://github.com/eyre-rs/eyre/pull/95
- @kylewlacy made their first contribution in https://github.com/eyre-rs/eyre/pull/91
- @LeoniePhiline made their first contribution in https://github.com/eyre-rs/eyre/pull/129

~~## [0.6.10] - 2023-12-07~~ Yanked

## [0.6.9] - 2023-11-17
### Fixed
- stacked borrows when dropping [by TimDiekmann](https://github.com/eyre-rs/eyre/pull/81)
- miri validation errors through now stricter provenance [by ten3roberts](https://github.com/eyre-rs/eyre/pull/103)
- documentation on no_std support [by thenorili](https://github.com/eyre-rs/eyre/pull/111)

### Added
- monorepo for eyre-related crates [by pksunkara](https://github.com/eyre-rs/eyre/pull/104), [[2]](https://github.com/eyre-rs/eyre/pull/105)[[3]](https://github.com/eyre-rs/eyre/pull/107)
- CONTRIBUTING.md [by yaahc](https://github.com/eyre-rs/eyre/pull/99)

## [0.6.8] - 2022-04-04
### Added
- `#[must_use]` to `Report`
- `must-install` feature to help reduce binary sizes when using a custom `EyreHandler`

## [0.6.7] - 2022-02-24
### Fixed
- missing track_caller annotation to new format arg capture constructor

## [0.6.6] - 2022-01-19
### Added
- support for format arguments capture on 1.58 and later

## [0.6.5] - 2021-01-05
### Added
- optional support for converting into `pyo3` exceptions

## [0.6.4] - 2021-01-04
### Fixed
- missing track_caller annotations to `wrap_err` related trait methods

## [0.6.3] - 2020-11-10
### Fixed
- missing track_caller annotation to autoref specialization functions

## [0.6.2] - 2020-10-27
### Fixed
- missing track_caller annotation to new_adhoc function

## [0.6.1] - 2020-09-28
### Added
- support for track_caller on rust versions where it is available


<!-- next-url -->
[Unreleased]: https://github.com/eyre-rs/eyre/compare/v0.6.11...HEAD
[0.6.11]: https://github.com/eyre-rs/eyre/compare/v0.6.9...v0.6.11
[0.6.9]:  https://github.com/eyre-rs/eyre/compare/v0.6.8...v0.6.9
[0.6.8]:  https://github.com/eyre-rs/eyre/compare/v0.6.7...v0.6.8
[0.6.7]:  https://github.com/eyre-rs/eyre/compare/v0.6.6...v0.6.7
[0.6.6]:  https://github.com/eyre-rs/eyre/compare/v0.6.5...v0.6.6
[0.6.5]:  https://github.com/eyre-rs/eyre/compare/v0.6.4...v0.6.5
[0.6.4]:  https://github.com/eyre-rs/eyre/compare/v0.6.3...v0.6.4
[0.6.3]:  https://github.com/eyre-rs/eyre/compare/v0.6.2...v0.6.3
[0.6.2]:  https://github.com/eyre-rs/eyre/compare/v0.6.1...v0.6.2
[0.6.1]:  https://github.com/eyre-rs/eyre/releases/tag/v0.6.1
