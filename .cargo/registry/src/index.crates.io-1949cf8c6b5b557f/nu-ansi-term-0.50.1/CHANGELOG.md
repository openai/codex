# Changelog

## v0.47.0 (2023-03-13)

### Breaking changes

- Bumped minimum supported Rust version (MSRV) to 1.62.1
- Change of `Color::default()` value to the ANSI default color `Color::Default` (code `39` and `49` for foreground and background respectively). This replaces `Color::White` as the default value.

### Other changes

- `const`ification of several functions and methods.
- Improved CI workflow.
- Updated to Rust edition 2021.
- Replaced `winapi` dependency with `windows-sys`.
- Removed `overload` dependency.
- Added `AnsiGenericString::as_str()` to allow access to the underlying string.
- Fixed typos in README.
- Added `CHANGELOG.md` for changes since forking `ansi_term`.

## v0.46.0 (2022-06-03)

- Removed `impl Deref` for `AnsiGenericString`.
- Improved README headings.

## v0.45.1 (2022-03-27)

- Added `Color::Default` enum variant with ansi codes `39` and `49` for foreground & background.

## v0.45.0 (2022-03-16)

- Fixed examples in README.
- Fixed typos in documentation.
- Renamed `ANSIByteStrings` to `AnsiByteStrings`.
- Added GitHub Actions workflow.
- Changed authors metadata.
- Updated license

## v0.43.0 (2022-01-18)

- Fixed clippy warning.

## v0.40.0 (2021-11-16)

- Fixed clippy warning.

## v0.38.0 (2021-10-05)

- Removed `itertools` dependency.

## v0.37.0 (2021-09-14)

- Fixed clippy warnings.

## v0.31.0 (2021-05-11)

- Implemented `Default` trait for `Color` returning `Color::White`.
- Added helpers for gradients.
- Fixed clippy warning.

## v0.30.0 (2021-04-21)

- Export `ansi` module to expose `ansi::RESET`.

## v0.29.0 (2021-03-30)

- Renamed `Color::RGB` enum variant to `Color::Rgb`.
- Renamed `ANSIByteString` to `AnsiByteString`.
- Renamed `ANSIGenericString` to `AnsiGenericString`.
- Renamed `ANSIGenericStrings` to `AnsiGenericStrings`.
- Renamed `ANSIString` to `AnsiString`.
- Renamed `ANSIStrings` to `AnsiStrings`.

## v0.28.0 (2021-03-09)

- Forked `ansi_term` as `nu-ansi-term`.
- Added Nushell project contributors to the authors.
- Updated README.md.
- Renamed `Colour` to `Color`.
- Renamed some files ending in `colour` to `color`.
- Added "bright" colors ansi 90-97 (foreground) and 100-107 (background).
- Ran cargo fmt
