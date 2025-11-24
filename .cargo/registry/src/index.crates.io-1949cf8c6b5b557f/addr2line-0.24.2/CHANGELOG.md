# `addr2line` Change Log

--------------------------------------------------------------------------------

## 0.24.2 (2024/10/04)

### Changed

* Enabled caching of DWARF abbreviations.
  [#318](https://github.com/gimli-rs/addr2line/pull/318)

* Changed the `addr2line` binary to prefer symbol names over DWARF names.
  [#332](https://github.com/gimli-rs/addr2line/pull/332)

* Updated `gimli` dependency.

### Added

* Added `Context::from_arc_dwarf`.
  [#327](https://github.com/gimli-rs/addr2line/pull/327)

* Added benchmark comparison.
  [#315](https://github.com/gimli-rs/addr2line/pull/315)
  [#321](https://github.com/gimli-rs/addr2line/pull/321)
  [#322](https://github.com/gimli-rs/addr2line/pull/322)
  [#325](https://github.com/gimli-rs/addr2line/pull/325)

* Added more tests.
  [#328](https://github.com/gimli-rs/addr2line/pull/328)
  [#330](https://github.com/gimli-rs/addr2line/pull/330)
  [#331](https://github.com/gimli-rs/addr2line/pull/331)
  [#333](https://github.com/gimli-rs/addr2line/pull/333)

--------------------------------------------------------------------------------

## 0.24.1 (2024/07/26)

### Changed

* Fixed parsing of partial units, which are found in supplementary object files.
  [#313](https://github.com/gimli-rs/addr2line/pull/313)

--------------------------------------------------------------------------------

## 0.24.0 (2024/07/16)

### Breaking changes

* Updated `gimli` dependency.

### Changed

* Changed the order of ranges returned by `Context::find_location_range`, and
  fixed handling in rare situations.
  [#303](https://github.com/gimli-rs/addr2line/pull/303)
  [#304](https://github.com/gimli-rs/addr2line/pull/304)
  [#306](https://github.com/gimli-rs/addr2line/pull/306)

* Improved the performance of `Context::find_location`.
  [#305](https://github.com/gimli-rs/addr2line/pull/305)

### Added

* Added `LoaderReader`.
  [#307](https://github.com/gimli-rs/addr2line/pull/307)

* Added `--all` option to `addr2line`.
  [#307](https://github.com/gimli-rs/addr2line/pull/307)

--------------------------------------------------------------------------------

## 0.23.0 (2024/05/26)

### Breaking changes

* Updated `gimli` dependency.

* Deleted `Context::new`, `Context::new_with_sup`, and `builtin_split_dwarf_loader`.
  Use `Context::from_dwarf` or `Loader::new` instead.
  This removes `object` from the public API.
  [#296](https://github.com/gimli-rs/addr2line/pull/296)

### Changed

* Fixed handling of column 0 in the line table.
  [#290](https://github.com/gimli-rs/addr2line/pull/290)

* Moved `addr2line` from `examples` to `bin`. Requires the `bin` feature.
  [#291](https://github.com/gimli-rs/addr2line/pull/291)

* Split up `lib.rs` into smaller modules.
  [#292](https://github.com/gimli-rs/addr2line/pull/292)

### Added

* Added `Loader`. Requires the `loader` feature.
  [#296](https://github.com/gimli-rs/addr2line/pull/296)
  [#297](https://github.com/gimli-rs/addr2line/pull/297)

* Added unpacked Mach-O support to `Loader`.
  [#298](https://github.com/gimli-rs/addr2line/pull/298)

--------------------------------------------------------------------------------

## 0.22.0 (2024/04/11)

### Breaking changes

* Updated `gimli` and `object` dependencies.

--------------------------------------------------------------------------------

## 0.21.0 (2023/08/12)

### Breaking changes

* Updated `gimli`, `object`, and `fallible-iterator` dependencies.

### Changed

* The minimum supported rust version is 1.65.0.

* Store boxed slices instead of `Vec` objects in `Context`.
  [#278](https://github.com/gimli-rs/addr2line/pull/278)

--------------------------------------------------------------------------------

## 0.20.0 (2023/04/15)

### Breaking changes

* The minimum supported rust version is 1.58.0.

* Changed `Context::find_frames` to return `LookupResult`.
  Use `LookupResult::skip_all_loads` to obtain the result without loading split DWARF.
  [#260](https://github.com/gimli-rs/addr2line/pull/260)

* Replaced `Context::find_dwarf_unit` with `Context::find_dwarf_and_unit`.
  [#260](https://github.com/gimli-rs/addr2line/pull/260)

* Updated `object` dependency.

### Changed

* Fix handling of file index 0 for DWARF 5.
  [#264](https://github.com/gimli-rs/addr2line/pull/264)

### Added

* Added types and methods to support loading split DWARF:
  `LookupResult`, `SplitDwarfLoad`, `SplitDwarfLoader`, `Context::preload_units`.
  [#260](https://github.com/gimli-rs/addr2line/pull/260)
  [#262](https://github.com/gimli-rs/addr2line/pull/262)
  [#263](https://github.com/gimli-rs/addr2line/pull/263)

--------------------------------------------------------------------------------

## 0.19.0 (2022/11/24)

### Breaking changes

* Updated `gimli` and `object` dependencies.

--------------------------------------------------------------------------------

## 0.18.0 (2022/07/16)

### Breaking changes

* Updated `object` dependency.

### Changed

* Fixed handling of relative path for `DW_AT_comp_dir`.
  [#239](https://github.com/gimli-rs/addr2line/pull/239)

* Fixed handling of `DW_FORM_addrx` for DWARF 5 support.
  [#243](https://github.com/gimli-rs/addr2line/pull/243)

* Fixed handling of units that are missing range information.
  [#249](https://github.com/gimli-rs/addr2line/pull/249)

--------------------------------------------------------------------------------

## 0.17.0 (2021/10/24)

### Breaking changes

* Updated `gimli` and `object` dependencies.

### Changed

* Use `skip_attributes` to improve performance.
  [#236](https://github.com/gimli-rs/addr2line/pull/236)

--------------------------------------------------------------------------------

## 0.16.0 (2021/07/26)

### Breaking changes

* Updated `gimli` and `object` dependencies.

--------------------------------------------------------------------------------

## 0.15.2 (2021/06/04)

### Fixed

* Allow `Context` to be `Send`.
  [#219](https://github.com/gimli-rs/addr2line/pull/219)

--------------------------------------------------------------------------------

## 0.15.1 (2021/05/02)

### Fixed

* Don't ignore aranges with address 0.
  [#217](https://github.com/gimli-rs/addr2line/pull/217)

--------------------------------------------------------------------------------

## 0.15.0 (2021/05/02)

### Breaking changes

* Updated `gimli` and `object` dependencies.
  [#215](https://github.com/gimli-rs/addr2line/pull/215)

* Added `debug_aranges` parameter to `Context::from_sections`.
  [#200](https://github.com/gimli-rs/addr2line/pull/200)

### Added

* Added `.debug_aranges` support.
  [#200](https://github.com/gimli-rs/addr2line/pull/200)

* Added supplementary object file support.
  [#208](https://github.com/gimli-rs/addr2line/pull/208)

### Fixed

* Fixed handling of Windows paths in locations.
  [#209](https://github.com/gimli-rs/addr2line/pull/209)

* examples/addr2line: Flush stdout after each response.
  [#210](https://github.com/gimli-rs/addr2line/pull/210)

* examples/addr2line: Avoid copying every section.
  [#213](https://github.com/gimli-rs/addr2line/pull/213)

--------------------------------------------------------------------------------

## 0.14.1 (2020/12/31)

### Fixed

* Fix location lookup for skeleton units.
  [#201](https://github.com/gimli-rs/addr2line/pull/201)

### Added

* Added `Context::find_location_range`.
  [#196](https://github.com/gimli-rs/addr2line/pull/196)
  [#199](https://github.com/gimli-rs/addr2line/pull/199)

--------------------------------------------------------------------------------

## 0.14.0 (2020/10/27)

### Breaking changes

* Updated `gimli` and `object` dependencies.

### Fixed

* Handle units that only have line information.
  [#188](https://github.com/gimli-rs/addr2line/pull/188)

* Handle DWARF units with version <= 4 and no `DW_AT_name`.
  [#191](https://github.com/gimli-rs/addr2line/pull/191)

* Fix handling of `DW_FORM_ref_addr`.
  [#193](https://github.com/gimli-rs/addr2line/pull/193)

--------------------------------------------------------------------------------

## 0.13.0 (2020/07/07)

### Breaking changes

* Updated `gimli` and `object` dependencies.

* Added `rustc-dep-of-std` feature.
  [#166](https://github.com/gimli-rs/addr2line/pull/166)

### Changed

* Improve performance by parsing function contents lazily.
  [#178](https://github.com/gimli-rs/addr2line/pull/178)

* Don't skip `.debug_info` and `.debug_line` entries with a zero address.
  [#182](https://github.com/gimli-rs/addr2line/pull/182)

--------------------------------------------------------------------------------

## 0.12.2 (2020/06/21)

### Fixed

* Avoid linear search for `DW_FORM_ref_addr`.
  [#175](https://github.com/gimli-rs/addr2line/pull/175)

--------------------------------------------------------------------------------

## 0.12.1 (2020/05/19)

### Fixed

* Handle units with overlapping address ranges.
  [#163](https://github.com/gimli-rs/addr2line/pull/163)

* Don't assert for functions with overlapping address ranges.
  [#168](https://github.com/gimli-rs/addr2line/pull/168)

--------------------------------------------------------------------------------

## 0.12.0 (2020/05/12)

### Breaking changes

* Updated `gimli` and `object` dependencies.

* Added more optional features: `smallvec` and `fallible-iterator`.
  [#160](https://github.com/gimli-rs/addr2line/pull/160)

### Added

*  Added `Context::dwarf` and `Context::find_dwarf_unit`.
  [#159](https://github.com/gimli-rs/addr2line/pull/159)

### Changed

* Removed `lazycell` dependency.
  [#160](https://github.com/gimli-rs/addr2line/pull/160)

--------------------------------------------------------------------------------

## 0.11.0 (2020/01/11)

### Breaking changes

* Updated `gimli` and `object` dependencies.

* [#130](https://github.com/gimli-rs/addr2line/pull/130)
  Changed `Location::file` from `Option<String>` to `Option<&str>`.
  This required adding lifetime parameters to `Location` and other structs that
  contain it.

* [#152](https://github.com/gimli-rs/addr2line/pull/152)
  Changed `Location::line` and `Location::column` from `Option<u64>`to `Option<u32>`.

* [#156](https://github.com/gimli-rs/addr2line/pull/156)
  Deleted `alloc` feature, and fixed `no-std` builds with stable rust.
  Removed default `Reader` parameter for `Context`, and added `ObjectContext` instead.

### Added

* [#134](https://github.com/gimli-rs/addr2line/pull/134)
  Added `Context::from_dwarf`.

### Changed

* [#133](https://github.com/gimli-rs/addr2line/pull/133)
  Fixed handling of units that can't be parsed.

* [#155](https://github.com/gimli-rs/addr2line/pull/155)
  Fixed `addr2line` output to match binutils.

* [#130](https://github.com/gimli-rs/addr2line/pull/130)
  Improved `.debug_line` parsing performance.

* [#148](https://github.com/gimli-rs/addr2line/pull/148)
  [#150](https://github.com/gimli-rs/addr2line/pull/150)
  [#151](https://github.com/gimli-rs/addr2line/pull/151)
  [#152](https://github.com/gimli-rs/addr2line/pull/152)
  Improved `.debug_info` parsing performance.

* [#137](https://github.com/gimli-rs/addr2line/pull/137)
  [#138](https://github.com/gimli-rs/addr2line/pull/138)
  [#139](https://github.com/gimli-rs/addr2line/pull/139)
  [#140](https://github.com/gimli-rs/addr2line/pull/140)
  [#146](https://github.com/gimli-rs/addr2line/pull/146)
  Improved benchmarks.

--------------------------------------------------------------------------------

## 0.10.0 (2019/07/07)

### Breaking changes

* [#127](https://github.com/gimli-rs/addr2line/pull/127)
  Update `gimli`.

--------------------------------------------------------------------------------

## 0.9.0 (2019/05/02)

### Breaking changes

* [#121](https://github.com/gimli-rs/addr2line/pull/121)
  Update `gimli`, `object`, and `fallible-iterator` dependencies.

### Added

* [#121](https://github.com/gimli-rs/addr2line/pull/121)
  Reexport `gimli`, `object`, and `fallible-iterator`.

--------------------------------------------------------------------------------

## 0.8.0 (2019/02/06)

### Breaking changes

* [#107](https://github.com/gimli-rs/addr2line/pull/107)
  Update `object` dependency to 0.11. This is part of the public API.

### Added

* [#101](https://github.com/gimli-rs/addr2line/pull/101)
  Add `object` feature (enabled by default). Disable this feature to remove
  the `object` dependency and `Context::new` API.

* [#102](https://github.com/gimli-rs/addr2line/pull/102)
  Add `std` (enabled by default) and `alloc` features.

### Changed

* [#108](https://github.com/gimli-rs/addr2line/issues/108)
  `demangle` no longer outputs the hash for rust symbols.

* [#109](https://github.com/gimli-rs/addr2line/issues/109)
  Set default `R` for `Context<R>`.
