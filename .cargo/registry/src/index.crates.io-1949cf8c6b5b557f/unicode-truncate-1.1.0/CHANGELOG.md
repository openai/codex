# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.1.0](https://github.com/Aetf/unicode-truncate/compare/v1.0.0...v1.1.0) - 2024-07-08

### Added
- segment by graphemes ([#11](https://github.com/Aetf/unicode-truncate/pull/11))

### Fixed
- *(deps)* update rust crate itertools to 0.13 ([#20](https://github.com/Aetf/unicode-truncate/pull/20))
- fixed typos in the `renovate.json` ([#17](https://github.com/Aetf/unicode-truncate/pull/17))
- Treat control characters as width 1, fixes [#16](https://github.com/Aetf/unicode-truncate/pull/16) ([#19](https://github.com/Aetf/unicode-truncate/pull/19))

### Other
- Removed unnessary debug-assertions setting
- tweak renovate configs ([#13](https://github.com/Aetf/unicode-truncate/pull/13))

## [1.0.0](https://github.com/Aetf/unicode-truncate/compare/v0.2.0...v1.0.0) - 2024-04-26

It's about time for a 1.0 release. This crate has been stable over the years.

### Breaking changes
Formally define MSRV (minimum supported rust version) to `1.63`.

### Added
- new method `unicode_truncate_centered`. Thanks @EdJoPaTo (#2, #3)

### Fixed
- prevent arithmetic side effects ([#7](https://github.com/Aetf/unicode-truncate/pull/7))
- do not include zero-width characters at boundaries when truncate_start

### Other
- use release-plz
- use renovate (#4)
- move from Travis to Github Action
- update Rust crate criterion to 0.5 ([#5](https://github.com/Aetf/unicode-truncate/pull/5))
- fix broken links
- Check in Cargo.lock per the latest guide
