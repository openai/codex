# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/)
and this project adheres to [Semantic Versioning](https://semver.org/).

<!-- next-header -->
## [Unreleased] - ReleaseDate

## [3.0.11] - 2025-11-13

### Internal

- Allow use of windows-sys 0.61 without bumping MSRV

## [3.0.10] - 2025-08-05

### Internal

- Dependency update

## [3.0.9] - 2025-06-04

## [3.0.8] - 2025-05-22

## [3.0.7] - 2025-01-13

- Restore MSRV for Windows

## [3.0.6] - 2024-10-24

## [3.0.5] - 2024-10-24

### Compatibility

- Update MSRV to 1.66

### Features

- Implement AutoStream for dyn Write + auto traits

## [3.0.4] - 2024-07-25

## [3.0.3] - 2024-05-02

### Fixes

- Drop MSRV to 1.65

## [3.0.2] - 2023-12-04

## [3.0.1] - 2023-09-29

### Features

- Impl `WinconStream` for `Box<dyn Write>`

## [3.0.0] - 2023-09-28

### Breaking Change

- API is rewritten from scratch, just being a `WinconStream::write_colored`

## [2.1.0] - 2023-08-24

### Compatibility

- Update MSRV to 1.70.0

### Features

- Allow querying `is_terminal`

## [2.0.0] - 2023-08-23

### Breaking Change

- Removed support for non-static locked streams
- Removed `Console::map`
- Exposed lower level get/set color APIs

### Features

## [1.0.2] - 2023-08-09

## [1.0.1] - 2023-04-24

### Features

- `std::fs::File` support (writes ANSI to it)

## [1.0.0] - 2023-04-13

### Breaking Change

- Updated `anstyle`

## [0.2.0] - 2023-03-13

### Breaking Change

- Take two at `Console::new` reporting errors

## [0.1.1] - 2023-03-08

### Features

- `Console` now implements `Debug`

## [0.1.0] - 2023-03-08

### Breaking Change

- `anstyle` upgraded
- `Console::new` no longer errors
- `Stream::set_color` and `Stream::get_color` changed their signatures

### Features

- `Console::lock` support
- `Console::into_inner` support
- `Console::map` support
- `Console::flush` support
- An ANSI implementation for windows


## [0.0.1] - 2023-03-07

<!-- next-url -->
[Unreleased]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v3.0.11...HEAD
[3.0.11]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v3.0.10...anstyle-wincon-v3.0.11
[3.0.10]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v3.0.9...anstyle-wincon-v3.0.10
[3.0.9]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v3.0.8...anstyle-wincon-v3.0.9
[3.0.8]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v3.0.7...anstyle-wincon-v3.0.8
[3.0.7]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v3.0.6...anstyle-wincon-v3.0.7
[3.0.6]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v3.0.5...anstyle-wincon-v3.0.6
[3.0.5]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v3.0.4...anstyle-wincon-v3.0.5
[3.0.4]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v3.0.3...anstyle-wincon-v3.0.4
[3.0.3]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v3.0.2...anstyle-wincon-v3.0.3
[3.0.2]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v3.0.1...anstyle-wincon-v3.0.2
[3.0.1]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v3.0.0...anstyle-wincon-v3.0.1
[3.0.0]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v2.1.0...anstyle-wincon-v3.0.0
[2.1.0]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v2.0.0...anstyle-wincon-v2.1.0
[2.0.0]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v1.0.2...anstyle-wincon-v2.0.0
[1.0.2]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v1.0.1...anstyle-wincon-v1.0.2
[1.0.1]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v1.0.0...anstyle-wincon-v1.0.1
[1.0.0]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v0.2.0...anstyle-wincon-v1.0.0
[0.2.0]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v0.1.1...anstyle-wincon-v0.2.0
[0.1.1]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v0.1.0...anstyle-wincon-v0.1.1
[0.1.0]: https://github.com/rust-cli/anstyle/compare/anstyle-wincon-v0.0.1...anstyle-wincon-v0.1.0
[0.0.1]: https://github.com/rust-cli/anstyle/compare/58e49814ccbdbd9cd30862e268a391cd61ce0f89...anstyle-wincon-v0.0.1
