# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/)
and this project adheres to [Semantic Versioning](https://semver.org/).

<!-- next-header -->
## [Unreleased] - ReleaseDate

## [0.6.21] - 2025-10-02

### Features

- Add `as_inner` functions to streams

## [0.6.20] - 2025-08-05

## [0.6.19] - 2025-06-04

## [0.6.18] - 2024-11-04

### Fixes

- Ensure `test` feature is respected by `print!`

## [0.6.17] - 2024-10-24

## [0.6.16] - 2024-10-24

### Compatibility

- Update MSRV to 1.66

### Features

- Implement AutoStream for dyn Write + auto traits

## [0.6.15] - 2024-07-25

## [0.6.14] - 2024-05-02

### Fixes

- Drop MSRV to 1.65

## [0.6.13] - 2024-02-27

### Fixes

- Put `NO_COLOR` above `CLICOLOR_FORCE` according to latest spec

## [0.6.12] - 2024-02-18

### Fixes

- `WinconBytes` correctly converts more escape codes

## [0.6.11] - 2024-01-18

### Fixes

- Ensure we build without `auto` (regression from 0.6.10)

## [0.6.10] - 2024-01-18

### Features

- Always expose `ColorChoice`
- Always expose `AutoStream::new`

## [0.6.9] - 2024-01-18

### Features

- Expose `Stdout` / `Stderr` types

## [0.6.8] - 2024-01-16

### Fixes

- Loosen `WinconStream` and `StripStream` trait bounds

## [0.6.7] - 2024-01-12

### Fixes

- Remove stray `dbg`

## [0.6.6] - 2024-01-12

### Fixes

- Fix soundness issue where invalid escape sequences can break up UTF-8 characters

## [0.6.5] - 2023-12-08

### Fixes

- *(windows)* Adapt ANSI-256 colors that map to ANSI colors

## [0.6.4] - 2023-09-29

### Fixes

- Compile again when `auto` is disabled

## [0.6.3] - 2023-09-29

### Fixes

- Ensure correct `anstyle-wincon` version is used

## [0.6.2] - 2023-09-29

### Compatibility

- Deprecated `Buffer` in favor of `Vec<u8>`

### Features

- Support `AutoStream` for `Vec<u8>` and `Box<dyn Write>`

## [0.6.1] - 2023-09-28

### Features

- Add `AutoStream::current_choice`

## [0.6.0] - 2023-09-28

### Breaking Change

- Internal traits moved to `stream` mod
- Shift focus from traits to methods for locking / terminal checks

### Fixes

- Capture `print!` et all in tests
- Hold locks over multi-part writes

## [0.5.0] - 2023-08-24

### Compatibility

- Update MSRV to 1.70.0

### Breaking Change

- Removed implementations and use of `is_terminal::IsTerminal`

### Performance

- Improved build times by dropping `is_terminal`

## [0.4.0] - 2023-08-23

### Breaking Change

- Switched from being generic over types to explicit `impl`s for expected types

## [0.3.2] - 2023-05-01

### Fixes

- Reference needed `anstyle-wincon` version

## [0.3.1] - 2023-04-24

### Features

- `std::fs::File` support as a `RawStream`, allowing stripping content written to a file

## [0.3.0] - 2023-04-13

### Breaking Change

- Updated `anstyle`

## [0.2.6] - 2023-03-16

## [0.2.5] - 2023-03-16

### Performance

- Binary size optimizations

## [0.2.4] - 2023-03-16

### Performance

- Remove a duplicated function due to generics

## [0.2.3] - 2023-03-16

## [0.2.2] - 2023-03-14

### Features

- Implement `IsTerminal` for stream types

## [0.2.1] - 2023-03-14

### Features

- `panic!` for styled panic messages
- `AutoStream::choice`

## [0.2.0] - 2023-03-13

### Breaking Change

- `WinconStream` is only exposed on Windows

### Performance

- Build time and binary size improvements for Linux by dropping wincon on non-Windows platforms

## [0.1.2] - 2023-03-13

### Features

- Allow global override of `AutoStream`, meant for CLI use cases
- `AutoStream` no reads
  - `NO_COLOR`
  - `CLICOLOR` / `CLICOLOR_FORCE`
  - `TERM`
  - `CI`

### Fixes

- Compile with `--no-default-features --features wincon`

## [0.1.1] - 2023-03-08

### Features

- Added `AutoStream::auto`
- Streams now implement `Debug`

## [0.1.0] - 2023-03-08

### Breaking Change

- `anstyle` upgraded
- `Stream` was renamed to `AutoStream`
- Replaced all uses of various underlying traits with the sealed `RawStream` trait
- `Lockable::locked` now transfers ownership

### Features

- Wincon support
- `Buffer` as an alternative to `stdout` and `stderr`

### Fixes

- Correctly report how much `StripStream` wrote
- Strip operations now strip `DEL` (0x7f)
- `*Stream::into_inner` support
- Enable windows ANSI support when writing to a terminal for `AutoStream::always_ansi`
- Ensure stale data isn't used when unlocking a stream by making it one-way
- Correctly resume state on partial write

## [0.0.1] - 2023-03-07

<!-- next-url -->
[Unreleased]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.21...HEAD
[0.6.21]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.20...anstream-v0.6.21
[0.6.20]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.19...anstream-v0.6.20
[0.6.19]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.18...anstream-v0.6.19
[0.6.18]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.17...anstream-v0.6.18
[0.6.17]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.16...anstream-v0.6.17
[0.6.16]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.15...anstream-v0.6.16
[0.6.15]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.14...anstream-v0.6.15
[0.6.14]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.13...anstream-v0.6.14
[0.6.13]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.12...anstream-v0.6.13
[0.6.12]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.11...anstream-v0.6.12
[0.6.11]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.10...anstream-v0.6.11
[0.6.10]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.9...anstream-v0.6.10
[0.6.9]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.8...anstream-v0.6.9
[0.6.8]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.7...anstream-v0.6.8
[0.6.7]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.6...anstream-v0.6.7
[0.6.6]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.5...anstream-v0.6.6
[0.6.5]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.4...anstream-v0.6.5
[0.6.4]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.3...anstream-v0.6.4
[0.6.3]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.2...anstream-v0.6.3
[0.6.2]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.1...anstream-v0.6.2
[0.6.1]: https://github.com/rust-cli/anstyle/compare/anstream-v0.6.0...anstream-v0.6.1
[0.6.0]: https://github.com/rust-cli/anstyle/compare/anstream-v0.5.0...anstream-v0.6.0
[0.5.0]: https://github.com/rust-cli/anstyle/compare/anstream-v0.4.0...anstream-v0.5.0
[0.4.0]: https://github.com/rust-cli/anstyle/compare/anstream-v0.3.2...anstream-v0.4.0
[0.3.2]: https://github.com/rust-cli/anstyle/compare/anstream-v0.3.1...anstream-v0.3.2
[0.3.1]: https://github.com/rust-cli/anstyle/compare/anstream-v0.3.0...anstream-v0.3.1
[0.3.0]: https://github.com/rust-cli/anstyle/compare/anstream-v0.2.6...anstream-v0.3.0
[0.2.6]: https://github.com/rust-cli/anstyle/compare/anstream-v0.2.5...anstream-v0.2.6
[0.2.5]: https://github.com/rust-cli/anstyle/compare/anstream-v0.2.4...anstream-v0.2.5
[0.2.4]: https://github.com/rust-cli/anstyle/compare/anstream-v0.2.3...anstream-v0.2.4
[0.2.3]: https://github.com/rust-cli/anstyle/compare/anstyle-stream-v0.2.2...anstream-v0.2.3
[0.2.2]: https://github.com/rust-cli/anstyle/compare/anstyle-stream-v0.2.1...anstyle-stream-v0.2.2
[0.2.1]: https://github.com/rust-cli/anstyle/compare/anstyle-stream-v0.2.0...anstyle-stream-v0.2.1
[0.2.0]: https://github.com/rust-cli/anstyle/compare/anstyle-stream-v0.1.2...anstyle-stream-v0.2.0
[0.1.2]: https://github.com/rust-cli/anstyle/compare/anstyle-stream-v0.1.1...anstyle-stream-v0.1.2
[0.1.1]: https://github.com/rust-cli/anstyle/compare/anstyle-stream-v0.1.0...anstyle-stream-v0.1.1
[0.1.0]: https://github.com/rust-cli/anstyle/compare/anstyle-stream-v0.0.1...anstyle-stream-v0.1.0
[0.0.1]: https://github.com/rust-cli/anstyle/compare/f1a7e73e317f1278be72655f5ce34336ae3d325c...anstyle-stream-v0.0.1
