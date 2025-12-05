# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/)
and this project adheres to [Semantic Versioning](https://semver.org/).

<!-- next-header -->
## [Unreleased] - ReleaseDate

## [1.0.13] - 2025-09-29

## [1.0.12] - 2025-09-29

### Fixes

- Remove leading zero when rendering single-digit `Ansi256Color`

## [1.0.11] - 2025-06-04

## [1.0.10] - 2024-11-01

### Performance

- Replace some `write!`s with `write_str`

## [1.0.9] - 2024-10-24

### Compatibility

- Update MSRV to 1.66

## [1.0.8] - 2024-07-25

## [1.0.7] - 2024-05-02

### Fixes

- Drop MSRV to 1.65

## [1.0.6] - 2024-02-05

### Fixes

- Ignore `Display` settings like padding (we were inconsistent about that and generally wrong when we did it)

## [1.0.5] - 2024-01-29

### Features

- Implement `Display` for `Style` where the alternate flag is for reset
- Implement `Display` for `Reset`

## [1.0.4] - 2023-09-28

### Features

- Allow `Display` types to be copied

## [1.0.3] - 2023-09-11

### Compatibility

- Update MSRV to 1.70.0

### Features

- Make `Color::on_default` `const`

## [1.0.2] - 2023-08-23

## [1.0.1] - 2023-06-20

## [1.0.0] - 2023-04-13

### Breaking Change

- Removed APIs that assume a color is a foreground color
  - See instead `Color::on_default`

## [0.3.5] - 2023-03-16

### Performance

- Binary size optimizations

## [0.3.4] - 2023-03-16

### Features

- `Anstyle::write_to` to bypass `Display` overhead

### Internal

- Simplified writing of ANSI codes by writing them separately

## [0.3.3] - 2023-03-16

### Features

- `Color::on_default` for more explicit color to style conversion

## [0.3.2] - 2023-03-16

### Documentation

- Fix links

## [0.3.1] - 2023-03-08

### Documentation

- Add more related crates

## [0.3.0] - 2023-03-08

### Compatibility

- Rename `XTermColor` to `Ansi256Color`
- Rename `Color::XTerm` to `Color::Ansi256`
- Replaced `fg_color | bg_color` with `fg_color.on(bg_color)`
- `#[must_use]` was added to calls

### Features

- Added `#[repr(u8)]` to `AnsiColor`

### Fixes

- Help catch API misused with `#[must_use]`

### Performance

- `#[inline]`d some calls

## [0.2.8] - 2023-03-06

### Documentation

- Color clarifications

## [0.2.7] - 2023-03-06

### Documentation

- Fix links

## [0.2.6] - 2023-03-06

### Features

- Alternative underline styles
- `Style::render_reset` for easier use

### Compatibility

MSRV updated to 1.64.0

## [0.2.5] - 2022-11-09

### Fixes

- Fix swapping of foreground and background introduced in 0.2.3

## [0.2.4] - 2022-10-07

## [0.2.3] - 2022-10-07

## [0.2.2] - 2022-08-17

## [0.2.1] - 2022-05-19

### Features

- With `From` and `PartialEq`, colors and `Effects` are considered equivalent of `Style`

## [0.2.0] - 2022-05-19

### Breaking Changes

- `Style::fg_color` / `Style::bg_color` now accept an `Option<Color>`

### Features

- `no_std` support
- `let style = color | effects;` support for easier construction
- Added `Color::from((r, g, b))` for easier creation

### Fixes

- Allow clearing fg/bg colors

## [0.1.1] - 2022-05-18

<!-- next-url -->
[Unreleased]: https://github.com/rust-cli/anstyle/compare/v1.0.13...HEAD
[1.0.13]: https://github.com/rust-cli/anstyle/compare/v1.0.12...v1.0.13
[1.0.12]: https://github.com/rust-cli/anstyle/compare/v1.0.11...v1.0.12
[1.0.11]: https://github.com/rust-cli/anstyle/compare/v1.0.10...v1.0.11
[1.0.10]: https://github.com/rust-cli/anstyle/compare/v1.0.9...v1.0.10
[1.0.9]: https://github.com/rust-cli/anstyle/compare/v1.0.8...v1.0.9
[1.0.8]: https://github.com/rust-cli/anstyle/compare/v1.0.7...v1.0.8
[1.0.7]: https://github.com/rust-cli/anstyle/compare/v1.0.6...v1.0.7
[1.0.6]: https://github.com/rust-cli/anstyle/compare/v1.0.5...v1.0.6
[1.0.5]: https://github.com/rust-cli/anstyle/compare/v1.0.4...v1.0.5
[1.0.4]: https://github.com/rust-cli/anstyle/compare/v1.0.3...v1.0.4
[1.0.3]: https://github.com/rust-cli/anstyle/compare/v1.0.2...v1.0.3
[1.0.2]: https://github.com/rust-cli/anstyle/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/rust-cli/anstyle/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/rust-cli/anstyle/compare/v0.3.5...v1.0.0
[0.3.5]: https://github.com/rust-cli/anstyle/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/rust-cli/anstyle/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/rust-cli/anstyle/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/rust-cli/anstyle/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/rust-cli/anstyle/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/rust-cli/anstyle/compare/v0.2.8...v0.3.0
[0.2.8]: https://github.com/rust-cli/anstyle/compare/v0.2.7...v0.2.8
[0.2.7]: https://github.com/rust-cli/anstyle/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/rust-cli/anstyle/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/rust-cli/anstyle/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/rust-cli/anstyle/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/rust-cli/anstyle/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/rust-cli/anstyle/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/rust-cli/anstyle/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/rust-cli/anstyle/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/rust-cli/anstyle/compare/6644c8911424a1451b483d39a3b415a41abfdf1b...v0.1.1
