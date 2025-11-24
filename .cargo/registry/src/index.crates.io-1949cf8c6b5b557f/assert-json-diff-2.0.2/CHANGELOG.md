# Change Log

All user visible changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/), as described
for Rust libraries in [RFC #1105](https://github.com/rust-lang/rfcs/blob/master/text/1105-api-evolution.md)

## Unreleased

- None.

### Breaking changes

None.

## 2.0.2 - 2022-06-29

- Don't move the `Value`s being compared in `assert_json_matches`

## 2.0.1 - 2021-02-14

- Add maintenance status to readme and `Cargo.toml`.

## 2.0.0 - 2021-01-23

## Unreleased

- A less strict numeric mode for comparisons is now supported. The `AssumeFloat` mode will make `1 == 1.0`. This mode can be set via `Config::numeric_mode`.
- A panicking `assert_json_matches` macro has been added which takes a `Config`.
- Remove dependency on "extend".

### Breaking changes

- Some breaking changes have been made to support customizing how the JSON values are compared:
    - `assert_json_eq_no_panic` and `assert_json_include_no_panic` have been replaced by `assert_json_matches_no_panic` which takes a `Config` that describes how the comparison should work.
    - This setup will support adding further customizations without more breaking changes.

## 1.1.0 - 2020-07-12

- All methods now accept any `T: Serialize` rather than just `serde_json::Value`.

## 1.0.3 - 2020-02-21

- Introduce non-panicking functions with `assert_json_include_no_panic` and `assert_json_eq_no_panic`.

## 1.0.2 - 2020-02-19

- Internal diffing algorithm simplified. There should be no external changes. Some error messages might have changed, but everything that passed/failed before should still do the same.

## 1.0.1 - 2019-10-24

- Update to 2018 edition

## 1.0.0 - 2019-02-15

### Fixed

- Make macros work with trailing comma

## 0.2.1 - 2018-11-15

### Fixed

- Fix wrong error message when a JSON atom was missing from actual.

## 0.2.0 - 2018-11-16

### Added

- Add `assert_json_include`. It does partial matching the same way the old `assert_json_eq` did.

### Changed

- Change `assert_json_eq` do exact matching. If the two values are not exactly the same, it'll panic.

## 0.1.0 - 2018-10-17

Initial release.
