# Changelog

## 0.8.0

- Bump Minimal Supported Rust Version to 1.46 due to dependencies.
- Update `uuid` dependency to `1.0`.

## 0.7.3

- Bump Minimal Supported Rust Version to 1.36 due to dependencies.
- Add support for PDB 2.0 format.

## 0.7.2

- Implement stricter and more consistent validation in `FromStr for DebugId`.
- `DebugId::from_breakpad` now properly validates the identifier.
- Remove internal dependencies on `regex` and `lazy_static`.

## 0.7.1

- Remove deprecated implementation of `Error::description` on errors.

## 0.7.0

- Update `uuid` to `0.8.1`.

## 0.6.0

_yanked_

## 0.5.3

- Only allow ASCII hex charactes in code identifiers.
- Implement `AsRef<str>` for `CodeId`.

## 0.5.2

- Implement conversion traits for `CodeId`.
- Always coerce code identifiers to lower case.

## 0.6.0 (yanked)

- Change `CodeId` to be a binary buffer instead of formatted string.

## 0.5.1

- Implement `Display` and `std::error::Error` for `ParseCodeIdError`.

## 0.5.0

- Add `CodeId`, an identifier for code files.
- Add `DebugId::nil` to create an empty id. This is the default.
- Add `DebugId::is_nil` to check whether a debug ID is empty.
- **Breaking Change:** The serde feature is now only called `"serde"`.

## 0.4.0

- **Breaking Change**: Require Rust 1.31.0 (Edition 2018) or newer
- Make parsing from string future-proof

## 0.3.1

Add a conversion from Microsoft GUIDs.

## 0.3.0

Update to `uuid:0.7.0`.

## 0.2.0

Renamed `DebugIdParseError` to `ParseDebugIdError`, according to Rust naming
guidelines.

## 0.1.1

Improved implementation of the `serde` traits.

## 0.1.0

Initial release.
