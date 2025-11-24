# [Unreleased]

# [0.3.13] - 2025-06-19

- Update windows-sys requirement from >=0.52, <=0.59 to >=0.52, <=0.60
  [#113](https://github.com/lambda-fairy/rust-errno/pull/113)

# [0.3.12] - 2025-05-12

- Issue a better error message if the target is unsupported.
  [#110](https://github.com/lambda-fairy/rust-errno/pull/110)

# [0.3.11] - 2025-04-04

- Add VxWorks support
  [#105](https://github.com/lambda-fairy/rust-errno/pull/105)

- Add cygwin support
  [#106](https://github.com/lambda-fairy/rust-errno/pull/106)

# [0.3.10] - 2024-11-29

- Update to windows-sys 0.59
  [#98](https://github.com/lambda-fairy/rust-errno/pull/98)

- Support emscripten
  [#100](https://github.com/lambda-fairy/rust-errno/pull/100)

- Remove Bitrig support
  [#99](https://github.com/lambda-fairy/rust-errno/pull/99)

# [0.3.9] - 2024-05-07

- Add visionOS support
  [#95](https://github.com/lambda-fairy/rust-errno/pull/95)

# [0.3.8] - 2023-11-27

- Update to windows-sys 0.52.
  [#91](https://github.com/lambda-fairy/rust-errno/pull/91)

- Update minimum Rust version to 1.56
  [#91](https://github.com/lambda-fairy/rust-errno/pull/91)

# [0.3.7] - 2023-11-15

- Fix `to_string()` handling for unknown error codes
  [#88](https://github.com/lambda-fairy/rust-errno/pull/88)

# [0.3.6] - 2023-11-07

- Add support for tvOS and watchOS
  [#84](https://github.com/lambda-fairy/rust-errno/pull/84)

- Added support for vita target
  [#86](https://github.com/lambda-fairy/rust-errno/pull/86)

# [0.3.5] - 2023-10-08

- Use __errno_location on DragonFly BSD
  [#82](https://github.com/lambda-fairy/rust-errno/pull/82)

# [0.3.4] - 2023-10-01

- Add GNU/Hurd support
  [#80](https://github.com/lambda-fairy/rust-errno/pull/80)

# [0.3.3] - 2023-08-28

- Disable "libc/std" in no-std configurations.
  [#77](https://github.com/lambda-fairy/rust-errno/pull/77)

- Bump errno-dragonfly to 0.1.2
  [#75](https://github.com/lambda-fairy/rust-errno/pull/75)

- Support for the ESP-IDF framework
  [#74](https://github.com/lambda-fairy/rust-errno/pull/74)

# [0.3.2] - 2023-07-30

- Fix build on Hermit
  [#73](https://github.com/lambda-fairy/rust-errno/pull/73)

- Add support for QNX Neutrino
  [#72](https://github.com/lambda-fairy/rust-errno/pull/72)

# [0.3.1] - 2023-04-08

- Correct link name on redox
  [#69](https://github.com/lambda-fairy/rust-errno/pull/69)

- Update windows-sys requirement from 0.45 to 0.48
  [#70](https://github.com/lambda-fairy/rust-errno/pull/70)

# [0.3.0] - 2023-02-12

- Add haiku support
  [#42](https://github.com/lambda-fairy/rust-errno/pull/42)

- Add AIX support
  [#54](https://github.com/lambda-fairy/rust-errno/pull/54)

- Add formatting with `#![no_std]`
  [#44](https://github.com/lambda-fairy/rust-errno/pull/44)

- Switch from `winapi` to `windows-sys` [#55](https://github.com/lambda-fairy/rust-errno/pull/55)

- Update minimum Rust version to 1.48
  [#48](https://github.com/lambda-fairy/rust-errno/pull/48) [#55](https://github.com/lambda-fairy/rust-errno/pull/55)

- Upgrade to Rust 2018 edition [#59](https://github.com/lambda-fairy/rust-errno/pull/59)

- wasm32-wasi: Use `__errno_location` instead of `feature(thread_local)`. [#66](https://github.com/lambda-fairy/rust-errno/pull/66)

# [0.2.8] - 2021-10-27

- Optionally support no_std
  [#31](https://github.com/lambda-fairy/rust-errno/pull/31)

[Unreleased]: https://github.com/lambda-fairy/rust-errno/compare/v0.3.10...HEAD
[0.3.10]: https://github.com/lambda-fairy/rust-errno/compare/v0.3.9...v0.3.10
[0.3.9]: https://github.com/lambda-fairy/rust-errno/compare/v0.3.8...v0.3.9
[0.3.8]: https://github.com/lambda-fairy/rust-errno/compare/v0.3.7...v0.3.8
[0.3.7]: https://github.com/lambda-fairy/rust-errno/compare/v0.3.6...v0.3.7
[0.3.6]: https://github.com/lambda-fairy/rust-errno/compare/v0.3.5...v0.3.6
[0.3.5]: https://github.com/lambda-fairy/rust-errno/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/lambda-fairy/rust-errno/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/lambda-fairy/rust-errno/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/lambda-fairy/rust-errno/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/lambda-fairy/rust-errno/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/lambda-fairy/rust-errno/compare/v0.2.8...v0.3.0
[0.2.8]: https://github.com/lambda-fairy/rust-errno/compare/v0.2.7...v0.2.8
