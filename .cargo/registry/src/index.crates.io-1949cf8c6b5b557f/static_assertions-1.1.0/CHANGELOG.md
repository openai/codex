# Changelog [![Crates.io][crate-badge]][crate]
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog] and this project adheres to
[Semantic Versioning].

## [Unreleased]

## [1.1.0] - 2019-11-03
### Added
- `assert_impl_any!` macro
- `assert_impl_one!` macro
- `assert_trait_sub_all!` macro
- `assert_trait_super_all!` macro
- Frequently asked questions to `README.md`

### Fixed
- `assert_eq_size_val!`, `const_assert_eq!`, and `const_assert_ne!` to export
  their local inner macros. Not having this prevented them from working when
  `use`d or called directly via `static_assertions::macro!(...)`

### Removed
- Unused `_assert_obj_safe!` from pre-1.0

## [1.0.0] - 2019-10-02
### Added
- `assert_eq_align!` macro

### Removed
- **[breaking]** Labels from macros that needed them 🎉
  - Made possible by [`const _`] in Rust 1.37
- **[breaking]** `assert_impl!` macro

### Fixed
- `assert_fields!` now works for `enum` types with multiple variants

### Changed
- **[breaking]** `const_assert!` macro to only take one expression
  - Reasoning: when custom error messages are added in the future (via
    [`assert!`]), having the macro allow for multiple comma-separated
    expressions may lead to ambiguity
- **[breaking]** Trait assertions to use `Type: Trait` syntax
- **[breaking]** Field assertions to use `Type: field1, field2` syntax
- **[breaking]** Renamed `assert_{ne,eq}_type!` to `assert_type_{ne,eq}_all!`

## [0.3.4] - 2019-06-12
### Changed
- Aliased `assert_impl!` to `assert_impl_all!` and deprecated `assert_impl!`

### Added
- `assert_impl_all!` as replacement to `assert_impl!`
- `assert_not_impl_all!` and `assert_not_impl_any!` macro counterparts to
  `assert_impl_all!`

### Fixed
- `assert_eq_type!` now works with types involving lifetimes

## [0.3.3] - 2019-06-12
### Added
- `const_assert_ne!` macro counterpart to `const_assert_eq!`

### Fixed
- `assert_eq_type!` would pass when types can coerce via `Deref`, such as with
  `str` and `String`

## [0.3.2] - 2019-05-15
### Added
- A `assert_eq_type!` macro that allows for checking whether inputs are the same
  concrete type
- A `assert_ne_type!` macro for checking whether inputs all refer to different
  types

### Fixed
- `const_assert!` now only takes `bool` values whereas integer (or other type)
  values could previously be passed

## [0.3.1] - 2018-11-15
### Fixed
- Macros that refer to other internal macros can now be imported when compiling
  for Rust 2018 ([issue
  #10](https://github.com/nvzqz/static-assertions-rs/issues/10))

## [0.3.0] - 2018-11-14
### Changed
- Bumped minimum supported (automatically tested) Rust version to 1.24.0
- Moved message parameter for `assert_cfg!()` to last argument position, making
  it consistent with other macros

### Removed
- No need to use `macro!(label; ...)` syntax when compiling on nightly Rust and
  enabling the `nightly` feature flag

## [0.2.5] - 2017-12-12
### Changed
- `assert_eq_size_ptr` wraps its code inside of a closure, ensuring that the
  unsafe code inside never runs
- Clippy no longer warns about `unneeded_field_pattern` within `assert_fields`

### Added
- Much better documentation with test examples that are guaranteed to fail at
  compile-time

### Removed
- Removed testing features; compile failure tests are now done via doc tests

## [0.2.4] - 2017-12-11
### Removed
- Removed the actual call to `mem::transmute` while still utilizing it for size
  verification ([Simon Sapin], [#5])

### Added
- `assert_cfg` macro that asserts that the given configuration is set
- `assert_fields` macro to assert that a struct type or enum variant has a given
  field

### Fixed
- Allow more generics flexibility in `assert_impl`

## [0.2.3] - 2017-08-24
### Fixed
- Trailing commas are now allowed

### Removed
- Removed clippy warnings

## [0.2.2] - 2017-08-13
### Added
- Added `assert_impl` macro to ensure a type implements a given set of traits

## [0.2.1] - 2017-08-13
### Added
- Added `assert_obj_safe` macro for ensuring that a trait is object-safe

## [0.2.0] - 2017-08-12
### Added
- Added `assert_eq_size_ptr` macro

### Fixed
- Allow `assert_eq_size`, `const_assert`, and `const_assert_eq` in non-function
  contexts via providing a unique label [#1]

### Removed
- **[Breaking]** Semicolon-separated `assert_eq_size` is no longer allowed

## [0.1.1] - 2017-08-12
### Added
- Added `const_assert_eq` macro

## 0.1.0 - 2017-08-12

Initial release

[Simon Sapin]: https://github.com/SimonSapin

[`assert!`]: https://doc.rust-lang.org/stable/std/macro.assert.html
[`const _`]: https://github.com/rust-lang/rfcs/blob/master/text/2526-const-wildcard.md

[#1]: https://github.com/nvzqz/static-assertions-rs/issues/1
[#5]: https://github.com/nvzqz/static-assertions-rs/pull/5

[crate]:       https://crates.io/crates/static_assertions
[crate-badge]: https://img.shields.io/crates/v/static_assertions.svg

[Keep a Changelog]:    http://keepachangelog.com/en/1.0.0/
[Semantic Versioning]: http://semver.org/spec/v2.0.0.html

[Unreleased]: https://github.com/nvzqz/static-assertions-rs/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/nvzqz/static-assertions-rs/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/nvzqz/static-assertions-rs/compare/v0.3.4...v1.0.0
[0.3.4]: https://github.com/nvzqz/static-assertions-rs/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/nvzqz/static-assertions-rs/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/nvzqz/static-assertions-rs/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/nvzqz/static-assertions-rs/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/nvzqz/static-assertions-rs/compare/v0.2.5...v0.3.0
[0.2.5]: https://github.com/nvzqz/static-assertions-rs/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/nvzqz/static-assertions-rs/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/nvzqz/static-assertions-rs/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/nvzqz/static-assertions-rs/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/nvzqz/static-assertions-rs/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/nvzqz/static-assertions-rs/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/nvzqz/static-assertions-rs/compare/v0.1.0...v0.1.1
