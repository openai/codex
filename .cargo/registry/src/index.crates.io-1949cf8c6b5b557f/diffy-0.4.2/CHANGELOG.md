# Changelog

## [0.4.2] - 2025-01-29

### Added
- [#37](https://github.com/bmwill/diffy/pull/37) Allow configuring the "No
  newline at end of file" message from being printed when formatting a patch.
- [#38](https://github.com/bmwill/diffy/pull/38) Add support for configuring
  `suppress_blank_empty`.

## [0.4.1] - 2025-01-29

### Added
- [#36](https://github.com/bmwill/diffy/pull/36) Add ability to configure
  filenames when creating a patch with `DiffOptions`.

## [0.4.0] - 2024-06-14

### Fixed
- [#28](https://github.com/bmwill/diffy/issues/28) Fixed an issue where
  conflicts were being omitted from merges.

### Added
- [#26](https://github.com/bmwill/diffy/pull/26) Add ability to reverse a
  patch.

### Changed
- [#29](https://github.com/bmwill/diffy/pull/29) Bump minimum supported rust
  version (msrv) to 1.62.1.
- [#22](https://github.com/bmwill/diffy/pull/22) update nu-ansi-term dependency
  to 0.50.

## [0.3.0] - 2022-08-29

### Fixed
- [#17](https://github.com/bmwill/diffy/issues/17) Fix an issue which resulted
  in a large slowdown when applying a patch with incorrect hunk headers.
- [#18](https://github.com/bmwill/diffy/pull/18) Replace unmaintained ansi_term
  dependency with nu_ansi_term in order to address
  [RUSTSEC-2021-0139](https://rustsec.org/advisories/RUSTSEC-2021-0139).

### Changed
- [#19](https://github.com/bmwill/diffy/pull/19) Bump minimum supported rust
  version (msrv) to 1.51.0.

## [0.2.2] - 2022-01-31

### Fixed
- [#16](https://github.com/bmwill/diffy/issues/16) Fix an issue where patch
  files failed to parse when they contained hunks which were adjacent to one
  another.

## [0.2.1] - 2021-01-27

### Fixed
- [#9](https://github.com/bmwill/diffy/issues/9) Fix an issue where the incorrect
  range was being used to index an array when calculating a merge resulting in a
  panic in some cases.

## [0.2.0] - 2020-07-07
### Added
- Support for working with potentially non-utf8 data with the addition of
  various `*_bytes` functions.
- Support for writing both utf8 and non-utf8 patches into a writer `W: io::write`
- Support for a minimum supported rust version (msrv) of 1.36.0.

### Changed
- The `Patch` type is now generic across the text type, either `str` for utf8
  text and `[u8]` for potentially non-utf8 texts.
- The filenames for the original and modified files of a patch are now
  optional. This means that patches which don't include filename headers
  (only include hunks) can now properly be parsed.

### Fixed
- Quoted filenames which include escaped characters are now properly parsed.

## [0.1.1] - 2020-07-01
### Added
- `Patch`es can now be parsed from strings with `Patch::from_str`
- A `Patch` can now be applied to a base image with `apply`

## [0.1.0] - 2020-06-30
- Initial release.

[0.4.2]: https://github.com/bmwill/diffy/releases/tag/0.4.2
[0.4.1]: https://github.com/bmwill/diffy/releases/tag/0.4.1
[0.4.0]: https://github.com/bmwill/diffy/releases/tag/0.4.0
[0.3.0]: https://github.com/bmwill/diffy/releases/tag/0.3.0
[0.2.2]: https://github.com/bmwill/diffy/releases/tag/0.2.2
[0.2.1]: https://github.com/bmwill/diffy/releases/tag/0.2.1
[0.2.0]: https://github.com/bmwill/diffy/releases/tag/0.2.0
[0.1.1]: https://github.com/bmwill/diffy/releases/tag/0.1.1
[0.1.0]: https://github.com/bmwill/diffy/releases/tag/0.1.0
