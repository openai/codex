# Change Log

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/)
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [v0.12.3] - 2022-07-17

## Fixed

- Fixed double-drop in `RawTable::clone_from`. (#348)

## [v0.12.2] - 2022-07-09

## Added

- Added `Entry` API for `HashSet`. (#342)
- Added `Extend<&'a (K, V)> for HashMap<K, V, S, A>`. (#340)
- Added length-based short-circuiting for hash table iteration. (#338)
- Added a function to access the `RawTable` of a `HashMap`. (#335)

## Changed

- Edited `do_alloc` to reduce LLVM IR generated. (#341)

## [v0.12.1] - 2022-05-02

## Fixed

- Fixed underflow in `RawIterRange::size_hint`. (#325)
- Fixed the implementation of `Debug` for `ValuesMut` and `IntoValues`. (#325)

## [v0.12.0] - 2022-01-17

## Added

- Added `From<[T; N]>` and `From<[(K, V); N]>` for `HashSet` and `HashMap` respectively. (#297)
- Added an `allocator()` getter to HashMap and HashSet. (#257)
- Added `insert_unique_unchecked` to `HashMap` and `HashSet`. (#293)
- Added `into_keys` and `into_values` to HashMap. (#295)
- Implement `From<array>` on `HashSet` and `HashMap`. (#298)
- Added `entry_ref` API to `HashMap`. (#201)

## Changed

- Bumped minimum Rust version to 1.56.1 and edition to 2021.
- Use u64 for the GroupWord on WebAssembly. (#271)
- Optimized `find`. (#279)
- Made rehashing and resizing less generic to reduce compilation time. (#282)
- Inlined small functions. (#283)
- Use `BuildHasher::hash_one` when `feature = "nightly"` is enabled. (#292)
- Relaxed the bounds on `Debug` for `HashSet`. (#296)
- Rename `get_each_mut` to `get_many_mut` and align API with the stdlib. (#291)
- Don't hash the key when searching in an empty table. (#305)

## Fixed

- Guard against allocations exceeding isize::MAX. (#268)
- Made `RawTable::insert_no_grow` unsafe. (#254)
- Inline `static_empty`. (#280)
- Fixed trait bounds on Send/Sync impls. (#303)

## [v0.11.2] - 2021-03-25

## Fixed

- Added missing allocator type parameter to `HashMap`'s and `HashSet`'s `Clone` impls. (#252)

## [v0.11.1] - 2021-03-20

## Fixed

- Added missing `pub` modifier to `BumpWrapper`. (#251)

## [v0.11.0] - 2021-03-14

## Added
- Added safe `try_insert_no_grow` method to `RawTable`. (#229)
- Added support for `bumpalo` as an allocator without the `nightly` feature. (#231)
- Implemented `Default` for `RawTable`. (#237)
- Added new safe methods `RawTable::get_each_mut`, `HashMap::get_each_mut`, and
  `HashMap::get_each_key_value_mut`. (#239)
- Added `From<HashMap<T, ()>>` for `HashSet<T>`. (#235)
- Added `try_insert` method to `HashMap`. (#247)

## Changed
- The minimum Rust version has been bumped to 1.49.0. (#230)
- Significantly improved compilation times by reducing the amount of generated IR. (#205)

## Removed
- We no longer re-export the unstable allocator items from the standard library, nor the stable shims approximating the same. (#227)
- Removed hasher specialization support from `aHash`, which was resulting in inconsistent hashes being generated for a key. (#248)

## Fixed
- Fixed union length comparison. (#228)

## ~~[v0.10.0] - 2021-01-16~~

This release was _yanked_ due to inconsistent hashes being generated with the `nightly` feature. (#248)

## Changed
- Parametrized `RawTable`, `HashSet` and `HashMap` over an allocator. (#133)
- Improved branch prediction hints on stable. (#209)
- Optimized hashing of primitive types with AHash using specialization. (#207)
- Only instantiate `RawTable`'s reserve functions once per key-value. (#204)

## [v0.9.1] - 2020-09-28

## Added
- Added safe methods to `RawTable` (#202):
  - `get`: `find` and `as_ref`
  - `get_mut`: `find` and `as_mut`
  - `insert_entry`: `insert` and `as_mut`
  - `remove_entry`: `find` and `remove`
  - `erase_entry`: `find` and `erase`

## Changed
- Removed `from_key_hashed_nocheck`'s `Q: Hash`. (#200)
- Made `RawTable::drain` safe. (#201)

## [v0.9.0] - 2020-09-03

### Fixed
- `drain_filter` now removes and yields items that do match the predicate,
  rather than items that don't.  This is a **breaking change** to match the
  behavior of the `drain_filter` methods in `std`. (#187)

### Added
- Added `replace_entry_with` to `OccupiedEntry`, and `and_replace_entry_with` to `Entry`. (#190)
- Implemented `FusedIterator` and `size_hint` for `DrainFilter`. (#188)

### Changed
- The minimum Rust version has been bumped to 1.36 (due to `crossbeam` dependency). (#193)
- Updated `ahash` dependency to 0.4. (#198)
- `HashMap::with_hasher` and `HashSet::with_hasher` are now `const fn`. (#195)
- Removed `T: Hash + Eq` and `S: BuildHasher` bounds on `HashSet::new`,
  `with_capacity`, `with_hasher`, and `with_capacity_and_hasher`.  (#185)

## [v0.8.2] - 2020-08-08

### Changed
- Avoid closures to improve compile times. (#183)
- Do not iterate to drop if empty. (#182)

## [v0.8.1] - 2020-07-16

### Added
- Added `erase` and `remove` to `RawTable`. (#171)
- Added `try_with_capacity` to `RawTable`. (#174)
- Added methods that allow re-using a `RawIter` for `RawDrain`,
  `RawIntoIter`, and `RawParIter`. (#175)
- Added `reflect_remove` and `reflect_insert` to `RawIter`. (#175)
- Added a `drain_filter` function to `HashSet`. (#179)

### Changed
- Deprecated `RawTable::erase_no_drop` in favor of `erase` and `remove`. (#176)
- `insert_no_grow` is now exposed under the `"raw"` feature. (#180)

## [v0.8.0] - 2020-06-18

### Fixed
- Marked `RawTable::par_iter` as `unsafe`. (#157)

### Changed
- Reduced the size of `HashMap`. (#159)
- No longer create tables with a capacity of 1 element. (#162)
- Removed `K: Eq + Hash` bounds on `retain`. (#163)
- Pulled in `HashMap` changes from rust-lang/rust (#164):
  - `extend_one` support on nightly.
  - `CollectionAllocErr` renamed to `TryReserveError`.
  - Added `HashSet::get_or_insert_owned`.
  - `Default` for `HashSet` no longer requires `T: Eq + Hash` and `S: BuildHasher`.

## [v0.7.2] - 2020-04-27

### Added
- Added `or_insert_with_key` to `Entry`. (#152)

### Fixed
- Partially reverted `Clone` optimization which was unsound. (#154)

### Changed
- Disabled use of `const-random` by default, which prevented reproducible builds. (#155)
- Optimized `repeat` function. (#150)
- Use `NonNull` for buckets, which improves codegen for iterators. (#148)

## [v0.7.1] - 2020-03-16

### Added
- Added `HashMap::get_key_value_mut`. (#145)

### Changed
- Optimized `Clone` implementation. (#146)

## [v0.7.0] - 2020-01-31

### Added
- Added a `drain_filter` function to `HashMap`. (#135)

### Changed
- Updated `ahash` dependency to 0.3. (#141)
- Optimized set union and intersection. (#130)
- `raw_entry` can now be used without requiring `S: BuildHasher`. (#123)
- `RawTable::bucket_index` can now be used under the `raw` feature. (#128)

## [v0.6.3] - 2019-10-31

### Added
- Added an `ahash-compile-time-rng` feature (enabled by default) which allows disabling the
  `compile-time-rng` feature in `ahash` to work around a Cargo bug. (#125)

## [v0.6.2] - 2019-10-23

### Added
- Added an `inline-more` feature (enabled by default) which allows choosing a tradeoff between 
  runtime performance and compilation time. (#119)

## [v0.6.1] - 2019-10-04

### Added
- Added `Entry::insert` and `RawEntryMut::insert`. (#118)

### Changed
- `Group::static_empty` was changed from a `const` to a `static` (#116).

## [v0.6.0] - 2019-08-13

### Fixed
- Fixed AHash accidentally depending on `std`. (#110)

### Changed
- The minimum Rust version has been bumped to 1.32 (due to `rand` dependency).

## ~~[v0.5.1] - 2019-08-04~~

This release was _yanked_ due to a breaking change for users of `no-default-features`.

### Added
- The experimental and unsafe `RawTable` API is available under the "raw" feature. (#108)
- Added entry-like methods for `HashSet`. (#98)

### Changed
- Changed the default hasher from FxHash to AHash. (#97)
- `hashbrown` is now fully `no_std` on recent Rust versions (1.36+). (#96)

### Fixed
- We now avoid growing the table during insertions when it wasn't necessary. (#106)
- `RawOccupiedEntryMut` now properly implements `Send` and `Sync`. (#100)
- Relaxed `lazy_static` version. (#92)

## [v0.5.0] - 2019-06-12

### Fixed
- Resize with a more conservative amount of space after deletions. (#86)

### Changed
- Exposed the Layout of the failed allocation in CollectionAllocErr::AllocErr. (#89)

## [v0.4.0] - 2019-05-30

### Fixed
- Fixed `Send` trait bounds on `IterMut` not matching the libstd one. (#82)

## [v0.3.1] - 2019-05-30

### Fixed
- Fixed incorrect use of slice in unsafe code. (#80)

## [v0.3.0] - 2019-04-23

### Changed
- Changed shrink_to to not panic if min_capacity < capacity. (#67)

### Fixed
- Worked around emscripten bug emscripten-core/emscripten-fastcomp#258. (#66)

## [v0.2.2] - 2019-04-16

### Fixed
- Inlined non-nightly lowest_set_bit_nonzero. (#64)
- Fixed build on latest nightly. (#65)

## [v0.2.1] - 2019-04-14

### Changed
- Use for_each in map Extend and FromIterator. (#58)
- Improved worst-case performance of HashSet.is_subset. (#61)

### Fixed
- Removed incorrect debug_assert. (#60)

## [v0.2.0] - 2019-03-31

### Changed
- The code has been updated to Rust 2018 edition. This means that the minimum
  Rust version has been bumped to 1.31 (2018 edition).

### Added
- Added `insert_with_hasher` to the raw_entry API to allow `K: !(Hash + Eq)`. (#54)
- Added support for using hashbrown as the hash table implementation in libstd. (#46)

### Fixed
- Fixed cargo build with minimal-versions. (#45)
- Fixed `#[may_dangle]` attributes to match the libstd `HashMap`. (#46)
- ZST keys and values are now handled properly. (#46)

## [v0.1.8] - 2019-01-14

### Added
- Rayon parallel iterator support (#37)
- `raw_entry` support (#31)
- `#[may_dangle]` on nightly (#31)
- `try_reserve` support (#31)

### Fixed
- Fixed variance on `IterMut`. (#31)

## [v0.1.7] - 2018-12-05

### Fixed
- Fixed non-SSE version of convert_special_to_empty_and_full_to_deleted. (#32)
- Fixed overflow in rehash_in_place. (#33)

## [v0.1.6] - 2018-11-17

### Fixed
- Fixed compile error on nightly. (#29)

## [v0.1.5] - 2018-11-08

### Fixed
- Fixed subtraction overflow in generic::Group::match_byte. (#28)

## [v0.1.4] - 2018-11-04

### Fixed
- Fixed a bug in the `erase_no_drop` implementation. (#26)

## [v0.1.3] - 2018-11-01

### Added
- Serde support. (#14)

### Fixed
- Make the compiler inline functions more aggressively. (#20)

## [v0.1.2] - 2018-10-31

### Fixed
- `clear` segfaults when called on an empty table. (#13)

## [v0.1.1] - 2018-10-30

### Fixed
- `erase_no_drop` optimization not triggering in the SSE2 implementation. (#3)
- Missing `Send` and `Sync` for hash map and iterator types. (#7)
- Bug when inserting into a table smaller than the group width. (#5)

## v0.1.0 - 2018-10-29

- Initial release

[Unreleased]: https://github.com/rust-lang/hashbrown/compare/v0.12.3...HEAD
[v0.12.3]: https://github.com/rust-lang/hashbrown/compare/v0.12.2...v0.12.3
[v0.12.2]: https://github.com/rust-lang/hashbrown/compare/v0.12.1...v0.12.2
[v0.12.1]: https://github.com/rust-lang/hashbrown/compare/v0.12.0...v0.12.1
[v0.12.0]: https://github.com/rust-lang/hashbrown/compare/v0.11.2...v0.12.0
[v0.11.2]: https://github.com/rust-lang/hashbrown/compare/v0.11.1...v0.11.2
[v0.11.1]: https://github.com/rust-lang/hashbrown/compare/v0.11.0...v0.11.1
[v0.11.0]: https://github.com/rust-lang/hashbrown/compare/v0.10.0...v0.11.0
[v0.10.0]: https://github.com/rust-lang/hashbrown/compare/v0.9.1...v0.10.0
[v0.9.1]: https://github.com/rust-lang/hashbrown/compare/v0.9.0...v0.9.1
[v0.9.0]: https://github.com/rust-lang/hashbrown/compare/v0.8.2...v0.9.0
[v0.8.2]: https://github.com/rust-lang/hashbrown/compare/v0.8.1...v0.8.2
[v0.8.1]: https://github.com/rust-lang/hashbrown/compare/v0.8.0...v0.8.1
[v0.8.0]: https://github.com/rust-lang/hashbrown/compare/v0.7.2...v0.8.0
[v0.7.2]: https://github.com/rust-lang/hashbrown/compare/v0.7.1...v0.7.2
[v0.7.1]: https://github.com/rust-lang/hashbrown/compare/v0.7.0...v0.7.1
[v0.7.0]: https://github.com/rust-lang/hashbrown/compare/v0.6.3...v0.7.0
[v0.6.3]: https://github.com/rust-lang/hashbrown/compare/v0.6.2...v0.6.3
[v0.6.2]: https://github.com/rust-lang/hashbrown/compare/v0.6.1...v0.6.2
[v0.6.1]: https://github.com/rust-lang/hashbrown/compare/v0.6.0...v0.6.1
[v0.6.0]: https://github.com/rust-lang/hashbrown/compare/v0.5.1...v0.6.0
[v0.5.1]: https://github.com/rust-lang/hashbrown/compare/v0.5.0...v0.5.1
[v0.5.0]: https://github.com/rust-lang/hashbrown/compare/v0.4.0...v0.5.0
[v0.4.0]: https://github.com/rust-lang/hashbrown/compare/v0.3.1...v0.4.0
[v0.3.1]: https://github.com/rust-lang/hashbrown/compare/v0.3.0...v0.3.1
[v0.3.0]: https://github.com/rust-lang/hashbrown/compare/v0.2.2...v0.3.0
[v0.2.2]: https://github.com/rust-lang/hashbrown/compare/v0.2.1...v0.2.2
[v0.2.1]: https://github.com/rust-lang/hashbrown/compare/v0.2.0...v0.2.1
[v0.2.0]: https://github.com/rust-lang/hashbrown/compare/v0.1.8...v0.2.0
[v0.1.8]: https://github.com/rust-lang/hashbrown/compare/v0.1.7...v0.1.8
[v0.1.7]: https://github.com/rust-lang/hashbrown/compare/v0.1.6...v0.1.7
[v0.1.6]: https://github.com/rust-lang/hashbrown/compare/v0.1.5...v0.1.6
[v0.1.5]: https://github.com/rust-lang/hashbrown/compare/v0.1.4...v0.1.5
[v0.1.4]: https://github.com/rust-lang/hashbrown/compare/v0.1.3...v0.1.4
[v0.1.3]: https://github.com/rust-lang/hashbrown/compare/v0.1.2...v0.1.3
[v0.1.2]: https://github.com/rust-lang/hashbrown/compare/v0.1.1...v0.1.2
[v0.1.1]: https://github.com/rust-lang/hashbrown/compare/v0.1.0...v0.1.1
