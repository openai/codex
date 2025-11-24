- 1.9.3

  - Bump the `rustc-rayon` dependency, for compiler use only.

- 1.9.2

  - `IndexMap` and `IndexSet` both implement `arbitrary::Arbitrary<'_>` and
    `quickcheck::Arbitrary` if those optional dependency features are enabled.

- 1.9.1

  - The MSRV now allows Rust 1.56.0 as well. However, currently `hashbrown`
    0.12.1 requires 1.56.1, so users on 1.56.0 should downgrade that to 0.12.0
    until there is a later published version relaxing its requirement.

- 1.9.0

  - **MSRV**: Rust 1.56.1 or later is now required.

  - The `hashbrown` dependency has been updated to version 0.12.

  - `IterMut` and `ValuesMut` now implement `Debug`.

  - The new `IndexMap::shrink_to` and `IndexSet::shrink_to` methods shrink
    the capacity with a lower bound.

  - The new `IndexMap::move_index` and `IndexSet::move_index` methods change
    the position of an item from one index to another, shifting the items
    between to accommodate the move.

- 1.8.2

  - Bump the `rustc-rayon` dependency, for compiler use only.

- 1.8.1

  - The new `IndexSet::replace_full` will return the index of the item along
    with the replaced value, if any, by @zakcutner in PR [222].

[222]: https://github.com/bluss/indexmap/pull/222

- 1.8.0

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

[195]: https://github.com/bluss/indexmap/pull/195
[196]: https://github.com/bluss/indexmap/pull/196
[197]: https://github.com/bluss/indexmap/pull/197
[203]: https://github.com/bluss/indexmap/pull/203
[205]: https://github.com/bluss/indexmap/pull/205
[211]: https://github.com/bluss/indexmap/pull/211

- 1.7.0

  - **MSRV**: Rust 1.49 or later is now required.

  - The `hashbrown` dependency has been updated to version 0.11.

- 1.6.2

  - Fixed to match `std` behavior, `OccupiedEntry::key` now references the
    existing key in the map instead of the lookup key, by @cuviper in PR [170].

  - The new `Entry::or_insert_with_key` matches Rust 1.50's `Entry` method,
    passing `&K` to the callback to create a value, by @cuviper in PR [175].

[170]: https://github.com/bluss/indexmap/pull/170
[175]: https://github.com/bluss/indexmap/pull/175

- 1.6.1

  - The new `serde_seq` module implements `IndexMap` serialization as a
    sequence to ensure order is preserved, by @cuviper in PR [158].

  - New methods on maps and sets work like the `Vec`/slice methods by the same name:
    `truncate`, `split_off`, `first`, `first_mut`, `last`, `last_mut`, and
    `swap_indices`, by @cuviper in PR [160].

[158]: https://github.com/bluss/indexmap/pull/158
[160]: https://github.com/bluss/indexmap/pull/160

- 1.6.0

  - **MSRV**: Rust 1.36 or later is now required.

  - The `hashbrown` dependency has been updated to version 0.9.

- 1.5.2

  - The new "std" feature will force the use of `std` for users that explicitly
    want the default `S = RandomState`, bypassing the autodetection added in 1.3.0,
    by @cuviper in PR [145].

[145]: https://github.com/bluss/indexmap/pull/145

- 1.5.1

  - Values can now be indexed by their `usize` position by @cuviper in PR [132].

  - Some of the generic bounds have been relaxed to match `std` by @cuviper in PR [141].

  - `drain` now accepts any `R: RangeBounds<usize>` by @cuviper in PR [142].

[132]: https://github.com/bluss/indexmap/pull/132
[141]: https://github.com/bluss/indexmap/pull/141
[142]: https://github.com/bluss/indexmap/pull/142

- 1.5.0

  - **MSRV**: Rust 1.32 or later is now required.

  - The inner hash table is now based on `hashbrown` by @cuviper in PR [131].
    This also completes the method `reserve` and adds `shrink_to_fit`.

  - Add new methods `get_key_value`, `remove_entry`, `swap_remove_entry`,
    and `shift_remove_entry`, by @cuviper in PR [136]

  - `Clone::clone_from` reuses allocations by @cuviper in PR [125]

  - Add new method `reverse` by @linclelinkpart5 in PR [128]

[125]: https://github.com/bluss/indexmap/pull/125
[128]: https://github.com/bluss/indexmap/pull/128
[131]: https://github.com/bluss/indexmap/pull/131
[136]: https://github.com/bluss/indexmap/pull/136

- 1.4.0

  - Add new method `get_index_of` by @Thermatrix in PR [115] and [120]

  - Fix build script rebuild-if-changed configuration to use "build.rs";
    fixes issue [123]. Fix by @cuviper.

  - Dev-dependencies (rand and quickcheck) have been updated. The crate's tests
    now run using Rust 1.32 or later (MSRV for building the crate has not changed).
    by @kjeremy and @bluss

[123]: https://github.com/bluss/indexmap/issues/123
[115]: https://github.com/bluss/indexmap/pull/115
[120]: https://github.com/bluss/indexmap/pull/120

- 1.3.2

  - Maintenance update to regenerate the published `Cargo.toml`.

- 1.3.1

  - Maintenance update for formatting and `autocfg` 1.0.

- 1.3.0

  - The deprecation messages in the previous version have been removed.
    (The methods have not otherwise changed.) Docs for removal methods have been
    improved.
  - From Rust 1.36, this crate supports being built **without std**, requiring
    `alloc` instead. This is enabled automatically when it is detected that
    `std` is not available. There is no crate feature to enable/disable to
    trigger this. The new build-dep `autocfg` enables this.

- 1.2.0

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

- 1.1.0

  - Added optional feature `"rayon"` that adds parallel iterator support
    to `IndexMap` and `IndexSet` using Rayon. This includes all the regular
    iterators in parallel versions, and parallel sort.

  - Implemented `Clone` for `map::{Iter, Keys, Values}` and
    `set::{Difference, Intersection, Iter, SymmetricDifference, Union}`

  - Implemented `Debug` for `map::{Entry, IntoIter, Iter, Keys, Values}` and
    `set::{Difference, Intersection, IntoIter, Iter, SymmetricDifference, Union}`

  - Serde trait `IntoDeserializer` are implemented for `IndexMap` and `IndexSet`.

  - Minimum Rust version requirement increased to Rust 1.30 for development builds.

- 1.0.2

  - The new methods `IndexMap::insert_full` and `IndexSet::insert_full` are
    both like `insert` with the index included in the return value.

  - The new method `Entry::and_modify` can be used to modify occupied
    entries, matching the new methods of `std` maps in Rust 1.26.

  - The new method `Entry::or_default` inserts a default value in unoccupied
    entries, matching the new methods of `std` maps in Rust 1.28.

- 1.0.1

  - Document Rust version policy for the crate (see rustdoc)

- 1.0.0

  - This is the 1.0 release for `indexmap`! (the crate and datastructure
    formerly known as “ordermap”)
  - `OccupiedEntry::insert` changed its signature, to use `&mut self` for
    the method receiver, matching the equivalent method for a standard
    `HashMap`.  Thanks to @dtolnay for finding this bug.
  - The deprecated old names from ordermap were removed: `OrderMap`,
    `OrderSet`, `ordermap!{}`, `orderset!{}`. Use the new `IndexMap`
    etc names instead.

- 0.4.1

  - Renamed crate to `indexmap`; the `ordermap` crate is now deprecated
    and the types `OrderMap/Set` now have a deprecation notice.

- 0.4.0

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

- 0.3.5

  - Documentation improvements

- 0.3.4

  - The `.retain()` methods for `OrderMap` and `OrderSet` now
    traverse the elements in order, and the retained elements **keep their order**
  - Added new methods `.sort_by()`, `.sort_keys()` to `OrderMap` and
    `.sort_by()`, `.sort()` to `OrderSet`. These methods allow you to
    sort the maps in place efficiently.

- 0.3.3

  - Document insertion behaviour better by @lucab
  - Updated dependences (no feature changes) by @ignatenkobrain

- 0.3.2

  - Add `OrderSet` by @cuviper!
  - `OrderMap::drain` is now (too) a double ended iterator.

- 0.3.1

  - In all ordermap iterators, forward the `collect` method to the underlying
    iterator as well.
  - Add crates.io categories.

- 0.3.0

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

[#10]: https://github.com/bluss/ordermap/pull/10

- 0.2.13

  - Fix deserialization to support custom hashers by @Techcable.
  - Add methods `.index()` on the entry types by @garro95.

- 0.2.12

  - Add methods `.with_hasher()`, `.hasher()`.

- 0.2.11

  - Support `ExactSizeIterator` for the iterators. By @Binero.
  - Use `Box<[Pos]>` internally, saving a word in the `OrderMap` struct.
  - Serde support, with crate feature `"serde-1"`. By @xfix.

- 0.2.10

  - Add iterator `.drain(..)` by @stevej.

- 0.2.9

  - Add method `.is_empty()` by @overvenus.
  - Implement `PartialEq, Eq` by @overvenus.
  - Add method `.sorted_by()`.

- 0.2.8

  - Add iterators `.values()` and `.values_mut()`.
  - Fix compatibility with 32-bit platforms.

- 0.2.7

  - Add `.retain()`.

- 0.2.6

  - Add `OccupiedEntry::remove_entry` and other minor entry methods,
    so that it now has all the features of `HashMap`'s entries.

- 0.2.5

  - Improved `.pop()` slightly.

- 0.2.4

  - Improved performance of `.insert()` ([#3]) by @pczarn.

[#3]: https://github.com/bluss/ordermap/pull/3

- 0.2.3

  - Generalize `Entry` for now, so that it works on hashmaps with non-default
    hasher. However, there's a lingering compat issue since libstd `HashMap`
    does not parameterize its entries by the hasher (`S` typarm).
  - Special case some iterator methods like `.nth()`.

- 0.2.2

  - Disable the verbose `Debug` impl by default.

- 0.2.1

  - Fix doc links and clarify docs.

- 0.2.0

  - Add more `HashMap` methods & compat with its API.
  - Experimental support for `.entry()` (the simplest parts of the API).
  - Add `.reserve()` (placeholder impl).
  - Add `.remove()` as synonym for `.swap_remove()`.
  - Changed `.insert()` to swap value if the entry already exists, and
    return `Option`.
  - Experimental support as an *indexed* hash map! Added methods
    `.get_index()`, `.get_index_mut()`, `.swap_remove_index()`,
    `.get_pair_index()`, `.get_pair_index_mut()`.

- 0.1.2

  - Implement the 32/32 split idea for `Pos` which improves cache utilization
    and lookup performance.

- 0.1.1

  - Initial release.
