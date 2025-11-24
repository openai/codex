# Changelog

## Unreleased

## v0.9.1 (26/03/2024)
### Added
 - Added changelog

### Changed
 - Clarify documentation about macro indirection
 - Turn the crate into a thin stdlib wrapper on rustc>=1.77
 - Turn `unstable_offset_of` and `unstable_const` into NOPs; they are not needed any more on recent nightlies

## v0.9.0 (18/05/2023)
### Added
 - Cargo feature `unstable_offset_of` which turns the crate into a stdlib polyfill

## v0.8.0 (15/12/2022)
### Changed
 - Constant-evaluation is automatically enabled

## v0.7.1 (17/10/2022)
### Changed
 - Version in `README.md`

## v0.7.0 (17/10/2022)
### Added
 - `offset_of_union!`

## v0.6.5 (03/12/2021)
### Removed
 - [nightly] `#![feature(const_raw_ptr_deref, const_maybe_uninit_as_ptr)]`
