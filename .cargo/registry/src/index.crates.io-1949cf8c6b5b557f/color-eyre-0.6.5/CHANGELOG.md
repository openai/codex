# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] - ReleaseDate

## [0.6.2] - 2022-07-11
### Added
- Option to disable display of location section in error reports

## [0.6.1] - 2022-02-24
### Changed
- Collapsed backtrace help text into fewer lines

## [0.6.0] - 2022-01-12
### Changed
- Updated dependencies to match newest tracing versions

## [0.5.11] - 2021-04-13

## [0.5.10] - 2020-12-02
### Added
- Support custom themes

## [0.5.9] - 2020-12-02
### Fixed
- Bumped color-spantrace dependency version to fix a panic

## [0.5.8] - 2020-11-23
### Added
- Exposed internal interfaces for the panic handler so that it can be wrapped
  by consumers to customize the behaviour of the panic hook.

## [0.5.7] - 2020-11-05
### Fixed
- Added missing `cfg`s that caused compiler errors when only enabling the
  `issue-url` feature

## [0.5.6] - 2020-10-02
### Added
- Add support for track caller added in eyre 0.6.1 and print original
  callsites of errors in all `eyre::Reports` by default

## [0.5.5] - 2020-09-21
### Added
- add `issue_filter` method to `HookBuilder` for disabling issue generation
  based on the error encountered.

## [0.5.4] - 2020-09-17
### Added
- Add new "issue-url" feature for generating issue creation links in error
  reports pre-populated with information about the error

## [0.5.3] - 2020-09-14
### Added
- add `panic_section` method to `HookBuilder` for overriding the printer for
  the panic message at the start of panic reports

## [0.5.2] - 2020-08-31
### Added
- make it so all `Section` trait methods can be called on `Report` in
  addition to the already supported usage on `Result<T, E: Into<Report>>`
- panic_section to `HookBuilder` to add custom sections to panic reports
- display_env_section to `HookBuilder` to disable the output indicating what
  environment variables can be set to manipulate the error reports
### Changed
- switched from ansi_term to owo-colors for colorizing output, allowing for
  better compatibility with the Display trait

<!-- next-url -->
[Unreleased]: https://github.com/eyre-rs/color-eyre/compare/v0.6.2...HEAD
[0.6.2]: https://github.com/eyre-rs/color-eyre/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/eyre-rs/color-eyre/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/eyre-rs/color-eyre/compare/v0.5.11...v0.6.0
[0.5.11]: https://github.com/eyre-rs/color-eyre/compare/v0.5.10...v0.5.11
[0.5.10]: https://github.com/eyre-rs/color-eyre/compare/v0.5.9...v0.5.10
[0.5.9]: https://github.com/eyre-rs/color-eyre/compare/v0.5.8...v0.5.9
[0.5.8]: https://github.com/eyre-rs/color-eyre/compare/v0.5.7...v0.5.8
[0.5.7]: https://github.com/eyre-rs/color-eyre/compare/v0.5.6...v0.5.7
[0.5.6]: https://github.com/eyre-rs/color-eyre/compare/v0.5.5...v0.5.6
[0.5.5]: https://github.com/eyre-rs/color-eyre/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/eyre-rs/color-eyre/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/eyre-rs/color-eyre/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/eyre-rs/color-eyre/releases/tag/v0.5.2
