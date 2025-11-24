# Releases

## 2.12.0 (2025-10-17)

- **MSRV**: Rust 1.82.0 or later is now required.
- Updated the `hashbrown` dependency to 0.16 alone.
- Error types now implement `core::error::Error`.
- Added `pop_if` methods to `IndexMap` and `IndexSet`, similar to the
  method for `Vec` added in Rust 1.86.

## 2.11.4 (2025-09-18)

- Updated the `hashbrown` dependency to a range allowing 0.15 or 0.16.

## 2.11.3 (2025-09-15)

- Make the minimum `serde` version only apply when "serde" is enabled.

## 2.11.2 (2025-09-15)

- Switched the "serde" feature to depend on `serde_core`, improving build
  parallelism in cases where other dependents have enabled "serde/derive".

## 2.11.1 (2025-09-08)

- Added a `get_key_value_mut` method to `IndexMap`.
- Removed the unnecessary `Ord` bound on `insert_sorted_by` methods.

## 2.11.0 (2025-08-22)

- Added `insert_sorted_by` and `insert_sorted_by_key` methods to `IndexMap`,
  `IndexSet`, and `VacantEntry`, like customizable versions of `insert_sorted`.
- Added `is_sorted`, `is_sorted_by`, and `is_sorted_by_key` methods to
  `IndexMap` and `IndexSet`, as well as their `Slice` counterparts.
- Added `sort_by_key` and `sort_unstable_by_key` methods to `IndexMap` and
  `IndexSet`, as well as parallel counterparts.
- Added `replace_index` methods to `IndexMap`, `IndexSet`, and `VacantEntry`
  to replace the key (or set value) at a given index.
- Added optional `sval` serialization support.

## 2.10.0 (2025-06-26)

- Added `extract_if` methods to `IndexMap` and `IndexSet`, similar to the
  methods for `HashMap` and `HashSet` with ranges like `Vec::extract_if`.
- Added more `#[track_caller]` annotations to functions that may panic.

## 2.9.0 (2025-04-04)

- Added a `get_disjoint_mut` method to `IndexMap`, matching Rust 1.86's
  `HashMap` method.
- Added a `get_disjoint_indices_mut` method to `IndexMap` and `map::Slice`,
  matching Rust 1.86's `get_disjoint_mut` method on slices.
- Deprecated the `borsh` feature in favor of their own `indexmap` feature,
  solving a cyclic dependency that occurred via `borsh-derive`.

## 2.8.0 (2025-03-10)

- Added `indexmap_with_default!` and `indexset_with_default!` to be used with
  alternative hashers, especially when using the crate without `std`.
- Implemented `PartialEq` between each `Slice` and `[]`/arrays.
- Removed the internal `rustc-rayon` feature and dependency.

## 2.7.1 (2025-01-19)

- Added `#[track_caller]` to functions that may panic.
- Improved memory reservation for `insert_entry`.

## 2.7.0 (2024-11-30)

- Added methods `Entry::insert_entry` and `VacantEntry::insert_entry`, returning
  an `OccupiedEntry` after insertion.

## 2.6.0 (2024-10-01)

- Implemented `Clone` for `map::IntoIter` and `set::IntoIter`.
- Updated the `hashbrown` dependency to version 0.15.

## 2.5.0 (2024-08-30)

- Added an `insert_before` method to `IndexMap` and `IndexSet`, as an
  alternative to `shift_insert` with different behavior on existing entries.
- Added `first_entry` and `last_entry` methods to `IndexMap`.
- Added `From` implementations between `IndexedEntry` and `OccupiedEntry`.

## 2.4.0 (2024-08-13)

- Added methods `IndexMap::append` and `IndexSet::append`, moving all items from
  one map or set into another, and leaving the original capacity for reuse.

## 2.3.0 (2024-07-31)

- Added trait `MutableEntryKey` for opt-in mutable access to map entry keys.
- Added method `MutableKeys::iter_mut2` for opt-in mutable iteration of map
  keys and values.

## 2.2.6 (2024-03-22)

- Added trait `MutableValues` for opt-in mutable access to set values.

## 2.2.5 (2024-02-29)

- Added optional `borsh` serialization support.

## 2.2.4 (2024-02-28)

- Added an `insert_sorted` method on `IndexMap`, `IndexSet`, and `VacantEntry`.
- Avoid hashing for lookups in single-entry maps.
- Limit preallocated memory in `serde` deserializers.

## 2.2.3 (2024-02-11)

- Added `move_index` and `swap_indices` methods to `IndexedEntry`,
  `OccupiedEntry`, and `RawOccupiedEntryMut`, functioning like the existing
  methods on `IndexMap`.
- Added `shift_insert` methods on `VacantEntry` and `RawVacantEntryMut`, as
  well as `shift_insert_hashed_nocheck` on the latter, to insert the new entry
  at a particular index.
- Added `shift_insert` methods on `IndexMap` and `IndexSet` to insert a new
  entry at a particular index, or else move an existing entry there.

## 2.2.2 (2024-01-31)

- Added indexing methods to raw entries: `RawEntryBuilder::from_hash_full`,
  `RawEntryBuilder::index_from_hash`, and `RawEntryMut::index`.

## 2.2.1 (2024-01-28)

- Corrected the signature of `RawOccupiedEntryMut::into_key(self) -> &'a mut K`,
  This a breaking change from 2.2.0, but that version was published for less
  than a day and has now been yanked.

## 2.2.0 (2024-01-28)

- The new `IndexMap::get_index_entry` method finds an entry by its index for
  in-place manipulation.

- The `Keys` iterator now implements `Index<usize>` for quick access to the
  entry's key, compared to indexing the map to get the value.

- The new `IndexMap::splice` and `IndexSet::splice` methods will drain the
  given range as an iterator, and then replace that range with entries from
  an input iterator.

- The new trait `RawEntryApiV1` offers opt-in access to a raw entry API for
  `IndexMap`, corresponding to the unstable API on `HashSet` as of Rust 1.75.

- Many `IndexMap` and `IndexSet` methods have relaxed their type constraints,
  e.g. removing `K: Hash` on methods that don't actually need to hash.

- Removal methods `remove`, `remove_entry`, and `take` are now deprecated
  in favor of their `shift_` or `swap_` prefixed variants, which are more
  explicit about their effect on the index and order of remaining items.
  The deprecated methods will remain to guide drop-in replacements from
  `HashMap` and `HashSet` toward the prefixed methods.

## 2.1.0 (2023-10-31)

- Empty slices can now be created with `map::Slice::{new, new_mut}` and
  `set::Slice::new`. In addition, `Slice::new`, `len`, and `is_empty` are
  now `const` functions on both types.

- `IndexMap`, `IndexSet`, and their respective `Slice`s all have binary
  search methods for sorted data: map `binary_search_keys` and set
  `binary_search` for plain comparison, `binary_search_by` for custom
  comparators, `binary_search_by_key` for key extraction, and
  `partition_point` for boolean conditions.

## 2.0.2 (2023-09-29)

- The `hashbrown` dependency has been updated to version 0.14.1 to
  complete the support for Rust 1.63.

## 2.0.1 (2023-09-27)

- **MSRV**: Rust 1.63.0 is now supported as well, pending publication of
  `hashbrown`'s relaxed MSRV (or use cargo `--ignore-rust-version`).

## 2.0.0 (2023-06-23)

- **MSRV**: Rust 1.64.0 or later is now required.

- The `"std"` feature is no longer auto-detected. It is included in the
  default feature set, or else can be enabled like any other Cargo feature.

- The `"serde-1"` feature has been removed, leaving just the optional
  `"serde"` dependency to be enabled like a feature itself.

- `IndexMap::get_index_mut` now returns `Option<(&K, &mut V)>`, changing
  the key part from `&mut K` to `&K`. There is also a new alternative
  `MutableKeys::get_index_mut2` to access the former behavior.

- The new `map::Slice<K, V>` and `set::Slice<T>` offer a linear view of maps
  and sets, behaving a lot like normal `[(K, V)]` and `[T]` slices. Notably,
  comparison traits like `Eq` only consider items in order, rather than hash
  lookups, and slices even implement `Hash`.

- `IndexMap` and `IndexSet` now have `sort_by_cached_key` and
  `par_sort_by_cached_key` methods which perform stable sorts in place
  using a key extraction function.

- `IndexMap` and `IndexSet` now have `reserve_exact`, `try_reserve`, and
  `try_reserve_exact` methods that correspond to the same methods on `Vec`.
  However, exactness only applies to the direct capacity for items, while the
  raw hash table still follows its own rules for capacity and load factor.

- The `Equivalent` trait is now re-exported from the `equivalent` crate,
  intended as a common base to allow types to work with multiple map types.

- The `hashbrown` dependency has been updated to version 0.14.

- The `serde_seq` module has been moved from the crate root to below the
  `map` module.

## 1.9.3 (2023-03-24)

- Bump the `rustc-rayon` dependency, for compiler use only.

## 1.9.2 (2022-11-17)

- `IndexMap` and `IndexSet` both implement `arbitrary::Arbitrary<'_>` and
  `quickcheck::Arbitrary` if those optional dependency features are enabled.

## 1.9.1 (2022-06-21)

- The MSRV now allows Rust 1.56.0 as well. However, currently `hashbrown`
  0.12.1 requires 1.56.1, so users on 1.56.0 should downgrade that to 0.12.0
  until there is a later published version relaxing its requirement.

## 1.9.0 (2022-06-16)

- **MSRV**: Rust 1.56.1 or later is now required.

- The `hashbrown` dependency has been updated to version 0.12.

- `IterMut` and `ValuesMut` now implement `Debug`.

- The new `IndexMap::shrink_to` and `IndexSet::shrink_to` methods shrink
  the capacity with a lower bound.

- The new `IndexMap::move_index` and `IndexSet::move_index` methods change
  the position of an item from one index to another, shifting the items
  between to accommodate the move.

## 1.8.2 (2022-05-27)

- Bump the `rustc-rayon` dependency, for compiler use only.

## 1.8.1 (2022-03-29)

- The new `IndexSet::replace_full` will return the index of the item along
  with the replaced value, if any, by @zakcutner in PR [222].

[222]: https://github.com/indexmap-rs/indexmap/pull/222

## 1.8.0 (2022-01-07)

- The new `IndexMap::into_keys` and `IndexMap::into_values` will consume
  the map into keys or values, respectively, matching Rust 1.54's `HashMap`
  methods, by @taiki-e in PR [195].

- More of the iterator types implement `Debug`, `ExactSizeIterator`, and
  `FusedIterator`, by @cuviper in PR [196].

- `IndexMap` and `IndexSet` now implement rayon's `ParallelDrainRange`,
  by @cuviper in PR [197].

- `IndexMap::with_hasher` and `IndexSet::with_hasher` are now `const`
  functions, allowing static maps and sets, by @mwillsey in PR [203].

- `IndexMap` and `IndexSet` now implement `From` for arrays, matching
  Rust 1.56's implementation for `HashMap`, by @rouge8 in PR [205].

- `IndexMap` and `IndexSet` now have methods `sort_unstable_keys`,
  `sort_unstable_by`, `sorted_unstable_by`, and `par_*` equivalents,
  which sort in-place without preserving the order of equal items, by
  @bhgomes in PR [211].

[195]: https://github.com/indexmap-rs/indexmap/pull/195
[196]: https://github.com/indexmap-rs/indexmap/pull/196
[197]: https://github.com/indexmap-rs/indexmap/pull/197
[203]: https://github.com/indexmap-rs/indexmap/pull/203
[205]: https://github.com/indexmap-rs/indexmap/pull/205
[211]: https://github.com/indexmap-rs/indexmap/pull/211

## 1.7.0 (2021-06-29)

- **MSRV**: Rust 1.49 or later is now required.

- The `hashbrown` dependency has been updated to version 0.11.

## 1.6.2 (2021-03-05)

- Fixed to match `std` behavior, `OccupiedEntry::key` now references the
  existing key in the map instead of the lookup key, by @cuviper in PR [170].

- The new `Entry::or_insert_with_key` matches Rust 1.50's `Entry` method,
  passing `&K` to the callback to create a value, by @cuviper in PR [175].

[170]: https://github.com/indexmap-rs/indexmap/pull/170
[175]: https://github.com/indexmap-rs/indexmap/pull/175

## 1.6.1 (2020-12-14)

- The new `serde_seq` module implements `IndexMap` serialization as a
  sequence to ensure order is preserved, by @cuviper in PR [158].

- New methods on maps and sets work like the `Vec`/slice methods by the same name:
  `truncate`, `split_off`, `first`, `first_mut`, `last`, `last_mut`, and
  `swap_indices`, by @cuviper in PR [160].

[158]: https://github.com/indexmap-rs/indexmap/pull/158
[160]: https://github.com/indexmap-rs/indexmap/pull/160

## 1.6.0 (2020-09-05)

- **MSRV**: Rust 1.36 or later is now required.

- The `hashbrown` dependency has been updated to version 0.9.

## 1.5.2 (2020-09-01)

- The new "std" feature will force the use of `std` for users that explicitly
  want the default `S = RandomState`, bypassing the autodetection added in 1.3.0,
  by @cuviper in PR [145].

[145]: https://github.com/indexmap-rs/indexmap/pull/145

## 1.5.1 (2020-08-07)

- Values can now be indexed by their `usize` position by @cuviper in PR [132].

- Some of the generic bounds have been relaxed to match `std` by @cuviper in PR [141].

- `drain` now accepts any `R: RangeBounds<usize>` by @cuviper in PR [142].

[132]: https://github.com/indexmap-rs/indexmap/pull/132
[141]: https://github.com/indexmap-rs/indexmap/pull/141
[142]: https://github.com/indexmap-rs/indexmap/pull/142

## 1.5.0 (2020-07-17)

- **MSRV**: Rust 1.32 or later is now required.

- The inner hash table is now based on `hashbrown` by @cuviper in PR [131].
  This also completes the method `reserve` and adds `shrink_to_fit`.

- Add new methods `get_key_value`, `remove_entry`, `swap_remove_entry`,
  and `shift_remove_entry`, by @cuviper in PR [136]

- `Clone::clone_from` reuses allocations by @cuviper in PR [125]

- Add new method `reverse` by @linclelinkpart5 in PR [128]

[125]: https://github.com/indexmap-rs/indexmap/pull/125
[128]: https://github.com/indexmap-rs/indexmap/pull/128
[131]: https://github.com/indexmap-rs/indexmap/pull/131
[136]: https://github.com/indexmap-rs/indexmap/pull/136

## 1.4.0 (2020-06-01)

- Add new method `get_index_of` by @Thermatrix in PR [115] and [120]

- Fix build script rebuild-if-changed configuration to use "build.rs";
  fixes issue [123]. Fix by @cuviper.

- Dev-dependencies (rand and quickcheck) have been updated. The crate's tests
  now run using Rust 1.32 or later (MSRV for building the crate has not changed).
  by @kjeremy and @bluss

[123]: https://github.com/indexmap-rs/indexmap/issues/123
[115]: https://github.com/indexmap-rs/indexmap/pull/115
[120]: https://github.com/indexmap-rs/indexmap/pull/120

## 1.3.2 (2020-02-05)

- Maintenance update to regenerate the published `Cargo.toml`.

## 1.3.1 (2020-01-15)

- Maintenance update for formatting and `autocfg` 1.0.

## 1.3.0 (2019-10-18)

- The deprecation messages in the previous version have been removed.
  (The methods have not otherwise changed.) Docs for removal methods have been
  improved.
- From Rust 1.36, this crate supports being built **without std**, requiring
  `alloc` instead. This is enabled automatically when it is detected that
  `std` is not available. There is no crate feature to enable/disable to
  trigger this. The new build-dep `autocfg` enables this.

## 1.2.0 (2019-09-08)

- Plain `.remove()` now has a deprecation message, it informs the user
  about picking one of the removal functions `swap_remove` and `shift_remove`
  which have different performance and order semantics.
  Plain `.remove()` will not be removed, the warning message and method
  will remain until further.

- Add new method `shift_remove` for order preserving removal on the map,
  and `shift_take` for the corresponding operation on the set.

- Add methods `swap_remove`, `swap_remove_entry` to `Entry`.

- Fix indexset/indexmap to support full paths, like `indexmap::indexmap!()`

- Internal improvements: fix warnings, deprecations and style lints

## 1.1.0 (2019-08-20)

- Added optional feature `"rayon"` that adds parallel iterator support
  to `IndexMap` and `IndexSet` using Rayon. This includes all the regular
  iterators in parallel versions, and parallel sort.

- Implemented `Clone` for `map::{Iter, Keys, Values}` and
  `set::{Difference, Intersection, Iter, SymmetricDifference, Union}`

- Implemented `Debug` for `map::{Entry, IntoIter, Iter, Keys, Values}` and
  `set::{Difference, Intersection, IntoIter, Iter, SymmetricDifference, Union}`

- Serde trait `IntoDeserializer` are implemented for `IndexMap` and `IndexSet`.

- Minimum Rust version requirement increased to Rust 1.30 for development builds.

## 1.0.2 (2018-10-22)

- The new methods `IndexMap::insert_full` and `IndexSet::insert_full` are
  both like `insert` with the index included in the return value.

- The new method `Entry::and_modify` can be used to modify occupied
  entries, matching the new methods of `std` maps in Rust 1.26.

- The new method `Entry::or_default` inserts a default value in unoccupied
  entries, matching the new methods of `std` maps in Rust 1.28.

## 1.0.1 (2018-03-24)

- Document Rust version policy for the crate (see rustdoc)

## 1.0.0 (2018-03-11)

- This is the 1.0 release for `indexmap`! (the crate and datastructure
  formerly known as “ordermap”)
- `OccupiedEntry::insert` changed its signature, to use `&mut self` for
  the method receiver, matching the equivalent method for a standard
  `HashMap`.  Thanks to @dtolnay for finding this bug.
- The deprecated old names from ordermap were removed: `OrderMap`,
  `OrderSet`, `ordermap!{}`, `orderset!{}`. Use the new `IndexMap`
  etc names instead.

## 0.4.1 (2018-02-14)

- Renamed crate to `indexmap`; the `ordermap` crate is now deprecated
  and the types `OrderMap/Set` now have a deprecation notice.

## 0.4.0 (2018-02-02)

- This is the last release series for this `ordermap` under that name,
  because the crate is **going to be renamed** to `indexmap` (with types
  `IndexMap`, `IndexSet`) and no change in functionality!
- The map and its associated structs moved into the `map` submodule of the
  crate, so that the map and set are symmetric

    + The iterators, `Entry` and other structs are now under `ordermap::map::`

- Internally refactored `OrderMap<K, V, S>` so that all the main algorithms
  (insertion, lookup, removal etc) that don't use the `S` parameter (the
  hasher) are compiled without depending on `S`, which reduces generics bloat.

- `Entry<K, V>` no longer has a type parameter `S`, which is just like
  the standard `HashMap`'s entry.

- Minimum Rust version requirement increased to Rust 1.18

## 0.3.5 (2018-01-14)

- Documentation improvements

## 0.3.4 (2018-01-04)

- The `.retain()` methods for `OrderMap` and `OrderSet` now
  traverse the elements in order, and the retained elements **keep their order**
- Added new methods `.sort_by()`, `.sort_keys()` to `OrderMap` and
  `.sort_by()`, `.sort()` to `OrderSet`. These methods allow you to
  sort the maps in place efficiently.

## 0.3.3 (2017-12-28)

- Document insertion behaviour better by @lucab
- Updated dependences (no feature changes) by @ignatenkobrain

## 0.3.2 (2017-11-25)

- Add `OrderSet` by @cuviper!
- `OrderMap::drain` is now (too) a double ended iterator.

## 0.3.1 (2017-11-19)

- In all ordermap iterators, forward the `collect` method to the underlying
  iterator as well.
- Add crates.io categories.

## 0.3.0 (2017-10-07)

- The methods `get_pair`, `get_pair_index` were both replaced by
  `get_full` (and the same for the mutable case).
- Method `swap_remove_pair` replaced by `swap_remove_full`.
- Add trait `MutableKeys` for opt-in mutable key access. Mutable key access
  is only possible through the methods of this extension trait.
- Add new trait `Equivalent` for key equivalence. This extends the
  `Borrow` trait mechanism for `OrderMap::get` in a backwards compatible
  way, just some minor type inference related issues may become apparent.
  See [#10] for more information.
- Implement `Extend<(&K, &V)>` by @xfix.

[#10]: https://github.com/indexmap-rs/indexmap/pull/10

## 0.2.13 (2017-09-30)

- Fix deserialization to support custom hashers by @Techcable.
- Add methods `.index()` on the entry types by @garro95.

## 0.2.12 (2017-09-11)

- Add methods `.with_hasher()`, `.hasher()`.

## 0.2.11 (2017-08-29)

- Support `ExactSizeIterator` for the iterators. By @Binero.
- Use `Box<[Pos]>` internally, saving a word in the `OrderMap` struct.
- Serde support, with crate feature `"serde-1"`. By @xfix.

## 0.2.10 (2017-04-29)

- Add iterator `.drain(..)` by @stevej.

## 0.2.9 (2017-03-26)

- Add method `.is_empty()` by @overvenus.
- Implement `PartialEq, Eq` by @overvenus.
- Add method `.sorted_by()`.

## 0.2.8 (2017-03-01)

- Add iterators `.values()` and `.values_mut()`.
- Fix compatibility with 32-bit platforms.

## 0.2.7 (2016-11-02)

- Add `.retain()`.

## 0.2.6 (2016-11-02)

- Add `OccupiedEntry::remove_entry` and other minor entry methods,
  so that it now has all the features of `HashMap`'s entries.

## 0.2.5 (2016-10-31)

- Improved `.pop()` slightly.

## 0.2.4 (2016-10-22)

- Improved performance of `.insert()` ([#3]) by @pczarn.

[#3]: https://github.com/indexmap-rs/indexmap/pull/3

## 0.2.3 (2016-10-11)

- Generalize `Entry` for now, so that it works on hashmaps with non-default
  hasher. However, there's a lingering compat issue since libstd `HashMap`
  does not parameterize its entries by the hasher (`S` typarm).
- Special case some iterator methods like `.nth()`.

## 0.2.2 (2016-10-02)

- Disable the verbose `Debug` impl by default.

## 0.2.1 (2016-10-02)

- Fix doc links and clarify docs.

## 0.2.0 (2016-10-01)

- Add more `HashMap` methods & compat with its API.
- Experimental support for `.entry()` (the simplest parts of the API).
- Add `.reserve()` (placeholder impl).
- Add `.remove()` as synonym for `.swap_remove()`.
- Changed `.insert()` to swap value if the entry already exists, and
  return `Option`.
- Experimental support as an *indexed* hash map! Added methods
  `.get_index()`, `.get_index_mut()`, `.swap_remove_index()`,
  `.get_pair_index()`, `.get_pair_index_mut()`.

## 0.1.2 (2016-09-19)

- Implement the 32/32 split idea for `Pos` which improves cache utilization
  and lookup performance.

## 0.1.1 (2016-09-16)

- Initial release.
