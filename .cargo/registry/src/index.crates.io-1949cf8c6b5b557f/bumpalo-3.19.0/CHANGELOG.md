## Unreleased

Released YYYY-MM-DD.

### Added

* TODO (or remove section if none)

### Changed

* TODO (or remove section if none)

### Deprecated

* TODO (or remove section if none)

### Removed

* TODO (or remove section if none)

### Fixed

* TODO (or remove section if none)

### Security

* TODO (or remove section if none)

--------------------------------------------------------------------------------

## 3.19.0

Released 2025-06-24.

### Added

* Added `bumpalo::collections::Vec::retain_mut`, similar to
  `std::vec::Vec::retain_mut`.

--------------------------------------------------------------------------------

## 3.18.1

Released 2025-06-05.

### Removed

* Removed the `allocator-api2` version bump from 3.18.0, as it was not actually
  semver compatible.

--------------------------------------------------------------------------------

## 3.18.0 (yanked)

Released 2025-06-05.

### Added

* Added support for enforcing a minimum alignment on all allocations inside a
  `Bump` arena, which can provide speed ups when allocating objects whose
  alignment is less than or equal to that minimum.
* Added `serde` serialization support for `bumpalo::collections::String`.
* Added some missing fallible slice allocation function variants.

### Changed

* Replaced `extend_from_slice` implementation with a formally-verified version
  that is also faster and more-optimizable for LLVM.
* Updated `allocator-api2` support to version `0.3.*`.

### Fixed

* Fixed a bug where the `allocated_bytes` metrics helper was accidentally
  including the size of `bumpalo`'s footer, rather than just reporting the
  user-allocated bytes.

--------------------------------------------------------------------------------

## 3.17.0

Released 2025-01-28.

### Added

* Added a bunch of `try_` allocation methods for slices and `str`:
  * `try_alloc_slice_fill_default`
  * `try_alloc_slice_fill_iter`
  * `try_alloc_slice_fill_clone`
  * `try_alloc_slice_fill_copy`
  * `try_alloc_slice_fill_with`
  * `try_alloc_str`
  * `try_alloc_slice_clone`
  * `try_alloc_slice_copy`

### Changed

* Minimum supported Rust version reduced to 1.71.1

### Fixed

* Fixed a stacked-borrows MIRI bug in `dealloc`

--------------------------------------------------------------------------------

## 3.16.0

Released 2024-04-08.

### Added

* Added an optional, off-by-default dependency on the `serde` crate. Enabling
  this dependency allows you to serialize Bumpalo's collection and box
  types. Deserialization is not implemented, due to constraints of the
  deserialization trait.

--------------------------------------------------------------------------------

## 3.15.4

Released 2024-03-07.

### Added

* Added the `bumpalo::collections::Vec::extend_from_slices_copy` method, which
  is a faster way to extend a vec from multiple slices when the element is
  `Copy` than calling `extend_from_slice_copy` N times.

--------------------------------------------------------------------------------

## 3.15.3

Released 2024-02-22.

### Added

* Added additional performance improvements to `bumpalo::collections::Vec`
  related to reserving capacity.

--------------------------------------------------------------------------------

## 3.15.2

Released 2024-02-21.

### Added

* Add a `bumpalo::collections::Vec::extend_from_slice_copy` method. This doesn't
  exist on the standard library's `Vec` but they have access to specialization,
  so their regular `extend_from_slice` has a specialization for `Copy`
  types. Using this new method for `Copy` types is a ~80x performance
  improvement over the plain `extend_from_slice` method.

--------------------------------------------------------------------------------

## 3.15.1

Released 2024-02-20.

### Fixed

* Fixed the MSRV listed in `Cargo.toml`, whose update was forgotten when the
  MSRV bumped in release 3.15.0.

--------------------------------------------------------------------------------

## 3.15.0

Released 2024-02-15.

### Changed

* The minimum supported Rust version (MSRV) is now 1.73.0.
* `bumpalo::collections::String::push_str` and
  `bumpalo::collections::String::from_str_in` received significant performance
  improvements.
* Allocator trait methods are now marked `#[inline]`, increasing performance for
  some callers.

### Fixed

* Fixed an edge-case bug in the `Allocator::shrink` method.

--------------------------------------------------------------------------------

## 3.14.0

Released 2023-09-14.

### Added

* Added the `std` cargo feature, which enables implementations of `std` traits
  for various things. Right now that is just `std::io::Write` for
  `bumpalo::collections::Vec`, but could be more in the future.

--------------------------------------------------------------------------------

## 3.13.0

Released 2023-05-22.

### Added

* New `"allocator-api2"` feature enables the use of the allocator API on
  stable. This feature uses a crate that mirrors the API of the unstable Rust
  `allocator_api` feature. If the feature is enabled, references to `Bump` will
  implement `allocator_api2::Allocator`. This allows `Bump` to be used as an
  allocator for collection types from `allocator-api2` and any other crates that
  support `allocator-api2`.

### Changed

* The minimum supported Rust version (MSRV) is now 1.63.0.

--------------------------------------------------------------------------------

## 3.12.2

Released 2023-05-09.

### Changed

* Added `rust-version` metadata to `Cargo.toml` which helps `cargo` with version
  resolution.

--------------------------------------------------------------------------------

## 3.12.1

Released 2023-04-21.

### Fixed

* Fixed a bug where `Bump::try_with_capacity(n)` where `n > isize::MAX` could
  lead to attempts to create invalid `Layout`s.

--------------------------------------------------------------------------------

## 3.12.0

Released 2023-01-17.

### Added

* Added the `bumpalo::boxed::Box::bump` and `bumpalo::collections::String::bump`
  getters to get the underlying `Bump` that a string or box was allocated into.

### Changed

* Some uses of `Box` that MIRI did not previously consider as UB are now
  reported as UB, and `bumpalo`'s internals have been adjusted to avoid the new
  UB.

--------------------------------------------------------------------------------

## 3.11.1

Released 2022-10-18.

### Security

* Fixed a bug where when `std::vec::IntoIter` was ported to
  `bumpalo::collections::vec::IntoIter`, it didn't get its underlying `Bump`'s
  lifetime threaded through. This meant that `rustc` was not checking the
  borrows for `bumpalo::collections::IntoIter` and this could result in
  use-after-free bugs.

--------------------------------------------------------------------------------

## 3.11.0

Released 2022-08-17.

### Added

* Added support for per-`Bump` allocation limits. These are enforced only in the
  slow path when allocating new chunks in the `Bump`, not in the bump allocation
  hot path, and therefore impose near zero overhead.
* Added the `bumpalo::boxed::Box::into_inner` method.

### Changed

* Updated to Rust 2021 edition.
* The minimum supported Rust version (MSRV) is now 1.56.0.

--------------------------------------------------------------------------------

## 3.10.0

Released 2022-06-01.

### Added

* Implement `bumpalo::collections::FromIteratorIn` for `Option` and `Result`,
  just like `core` does for `FromIterator`.
* Implement `bumpalo::collections::FromIteratorIn` for `bumpalo::boxed::Box<'a,
  [T]>`.
* Added running tests under MIRI in CI for additional confidence in unsafe code.
* Publicly exposed `bumpalo::collections::Vec::drain_filter` since the
  corresponding `std::vec::Vec` method has stabilized.

### Changed

* `Bump::new` will not allocate a backing chunk until the first allocation
  inside the bump arena now.

### Fixed

* Properly account for alignment changes when growing or shrinking an existing
  allocation.
* Removed all internal integer-to-pointer casts, to play better with UB checkers
  like MIRI.

--------------------------------------------------------------------------------

## 3.9.1

Released 2022-01-06.

### Fixed

* Fixed link to logo in docs and README.md

--------------------------------------------------------------------------------

## 3.9.0

Released 2022-01-05.

### Changed

* The minimum supported Rust version (MSRV) has been raised to Rust 1.54.0.

* `bumpalo::collections::Vec<T>` implements relevant traits for all arrays of
  any size `N` via const generics. Previously, it was just arrays up to length
  32. Similar for `bumpalo::boxed::Box<[T; N]>`.

--------------------------------------------------------------------------------

## 3.8.0

Released 2021-10-19.

### Added

* Added the `CollectIn` and `FromIteratorIn` traits to make building a
  collection from an iterator easier. These new traits live in the
  `bumpalo::collections` module and are implemented by
  `bumpalo::collections::{String,Vec}`.

* Added the `Bump::iter_allocated_chunks_raw` method, which is an `unsafe`, raw
  version of `Bump::iter_allocated_chunks`. The new method does not take an
  exclusive borrow of the `Bump` and yields raw pointer-and-length pairs for
  each chunk in the bump. It is the caller's responsibility to ensure that no
  allocation happens in the `Bump` while iterating over chunks and that there
  are no active borrows of allocated data if they want to turn any
  pointer-and-length pairs into slices.

--------------------------------------------------------------------------------

## 3.7.1

Released 2021-09-17.

### Changed

* The packaged crate uploaded to crates.io when `bumpalo` is published is now
  smaller, thanks to excluding unnecessary files.

--------------------------------------------------------------------------------

## 3.7.0

Released 2020-05-28.

### Added

* Added `Borrow` and `BorrowMut` trait implementations for
  `bumpalo::collections::Vec` and
  `bumpalo::collections::String`. [#108](https://github.com/fitzgen/bumpalo/pull/108)

### Changed

* When allocating a new chunk fails, don't immediately give up. Instead, try
  allocating a chunk that is half that size, and if that fails, then try half of
  *that* size, etc until either we successfully allocate a chunk or we fail to
  allocate the minimum chunk size and then finally give
  up. [#111](https://github.com/fitzgen/bumpalo/pull/111)

--------------------------------------------------------------------------------

## 3.6.1

Released 2020-02-18.

### Added

* Improved performance of `Bump`'s `Allocator::grow_zeroed` trait method
  implementation. [#99](https://github.com/fitzgen/bumpalo/pull/99)

--------------------------------------------------------------------------------

## 3.6.0

Released 2020-01-29.

### Added

* Added a few new flavors of allocation:

  * `try_alloc` for fallible, by-value allocation

  * `try_alloc_with` for fallible allocation with an infallible initializer
    function

  * `alloc_try_with` for infallible allocation with a fallible initializer
    function

  * `try_alloc_try_with` method for fallible allocation with a fallible
    initializer function

  We already have infallible, by-value allocation (`alloc`) and infallible
  allocation with an infallible initializer (`alloc_with`). With these new
  methods, we now have every combination covered.

  Thanks to [Tamme Schichler](https://github.com/Tamschi) for contributing these
  methods!

--------------------------------------------------------------------------------

## 3.5.0

Released 2020-01-22.

### Added

* Added experimental, unstable support for the unstable, nightly Rust
  `allocator_api` feature.

  The `allocator_api` feature defines an `Allocator` trait and exposes custom
  allocators for `std` types. Bumpalo has a matching `allocator_api` cargo
  feature to enable implementing `Allocator` and using `Bump` with `std`
  collections.

  First, enable the `allocator_api` feature in your `Cargo.toml`:

  ```toml
  [dependencies]
  bumpalo = { version = "3.5", features = ["allocator_api"] }
  ```

  Next, enable the `allocator_api` nightly Rust feature in your `src/lib.rs` or `src/main.rs`:

  ```rust
  # #[cfg(feature = "allocator_api")]
  # {
  #![feature(allocator_api)]
  # }
  ```

  Finally, use `std` collections with `Bump`, so that their internal heap
  allocations are made within the given bump arena:

  ```
  # #![cfg_attr(feature = "allocator_api", feature(allocator_api))]
  # #[cfg(feature = "allocator_api")]
  # {
  #![feature(allocator_api)]
  use bumpalo::Bump;

  // Create a new bump arena.
  let bump = Bump::new();

  // Create a `Vec` whose elements are allocated within the bump arena.
  let mut v = Vec::new_in(&bump);
  v.push(0);
  v.push(1);
  v.push(2);
  # }
  ```

  I'm very excited to see custom allocators in `std` coming along! Thanks to
  Arthur Gautier for implementing support for the `allocator_api` feature for
  Bumpalo.

--------------------------------------------------------------------------------

## 3.4.0

Released 2020-06-01.

### Added

* Added the `bumpalo::boxed::Box<T>` type. It is an owned pointer referencing a
  bump-allocated value, and it runs `T`'s `Drop` implementation on the
  referenced value when dropped. This type can be used by enabling the `"boxed"`
  cargo feature flag.

--------------------------------------------------------------------------------

## 3.3.0

Released 2020-05-13.

### Added

* Added fallible allocation methods to `Bump`: `try_new`, `try_with_capacity`,
  and `try_alloc_layout`.

* Added `Bump::chunk_capacity`

* Added `bumpalo::collections::Vec::try_reserve[_exact]`

--------------------------------------------------------------------------------

## 3.2.1

Released 2020-03-24.

### Security

* When `realloc`ing, if we allocate new space, we need to copy the old
  allocation's bytes into the new space. There are `old_size` number of bytes in
  the old allocation, but we were accidentally copying `new_size` number of
  bytes, which could lead to copying bytes into the realloc'd space from past
  the chunk that we're bump allocating out of, from unknown memory.

  If an attacker can cause `realloc`s, and can read the `realoc`ed data back,
  this could allow them to read things from other regions of memory that they
  shouldn't be able to. For example, if some crypto keys happened to live in
  memory right after a chunk we were bump allocating out of, this could allow
  the attacker to read the crypto keys.

  Beyond just fixing the bug and adding a regression test, I've also taken two
  additional steps:

  1. While we were already running the testsuite under `valgrind` in CI, because
     `valgrind` exits with the same code that the program did, if there are
     invalid reads/writes that happen not to trigger a segfault, the program can
     still exit OK and we will be none the wiser. I've enabled the
     `--error-exitcode=1` flag for `valgrind` in CI so that tests eagerly fail
     in these scenarios.

  2. I've written a quickcheck test to exercise `realloc`. Without the bug fix
     in this patch, this quickcheck immediately triggers invalid reads when run
     under `valgrind`. We didn't previously have quickchecks that exercised
     `realloc` because `realloc` isn't publicly exposed directly, and instead
     can only be indirectly called. This new quickcheck test exercises `realloc`
     via `bumpalo::collections::Vec::resize` and
     `bumpalo::collections::Vec::shrink_to_fit` calls.

  This bug was introduced in version 3.0.0.

  See [#69](https://github.com/fitzgen/bumpalo/issues/69) for details.

--------------------------------------------------------------------------------

## 3.2.0

Released 2020-02-07.

### Added

* Added the `bumpalo::collections::Vec::into_bump_slice_mut` method to turn a
  `bumpalo::collections::Vec<'bump, T>` into a `&'bump mut [T]`.

--------------------------------------------------------------------------------

## 3.1.2

Released 2020-01-07.

### Fixed

* The `bumpalo::collections::format!` macro did not used to accept a trailing
  comma like `format!(in bump; "{}", 1,)`, but it does now.

--------------------------------------------------------------------------------

## 3.1.1

Released 2020-01-03.

### Fixed

* The `bumpalo::collections::vec!` macro did not used to accept a trailing
  comma like `vec![in bump; 1, 2,]`, but it does now.

--------------------------------------------------------------------------------

## 3.1.0

Released 2019-12-27.

### Added

* Added the `Bump::allocated_bytes` diagnostic method for counting the total
  number of bytes a `Bump` has allocated.

--------------------------------------------------------------------------------

# 3.0.0

Released 2019-12-20.

## Added

* Added `Bump::alloc_str` for copying string slices into a `Bump`.

* Added `Bump::alloc_slice_copy` and `Bump::alloc_slice_clone` for copying or
  cloning slices into a `Bump`.

* Added `Bump::alloc_slice_fill_iter` for allocating a slice in the `Bump` from
  an iterator.

* Added `Bump::alloc_slice_fill_copy` and `Bump::alloc_slice_fill_clone` for
  creating slices of length `n` that are filled with copies or clones of an
  initial element.

* Added `Bump::alloc_slice_fill_default` for creating slices of length `n` with
  the element type's default instance.

* Added `Bump::alloc_slice_fill_with` for creating slices of length `n` whose
  elements are initialized with a function or closure.

* Added `Bump::iter_allocated_chunks` as a replacement for the old
  `Bump::each_allocated_chunk`. The `iter_allocated_chunks` version returns an
  iterator, which is more idiomatic than its old, callback-taking counterpart.
  Additionally, `iter_allocated_chunks` exposes the chunks as `MaybeUninit`s
  instead of slices, which makes it usable in more situations without triggering
  undefined behavior. See also the note about bump direction in the "changed"
  section; if you're iterating chunks, you're likely affected by that change!

* Added `Bump::with_capacity` so that you can pre-allocate a chunk with the
  requested space.

### Changed

* **BREAKING:** The direction we allocate within a chunk has changed. It used to
  be "upwards", from low addresses within a chunk towards high addresses. It is
  now "downwards", from high addresses towards lower addresses.

  Additionally, the order in which we iterate over allocated chunks has changed!
  We used to iterate over chunks from oldest chunk to youngest chunk, and now we
  do the opposite: the youngest chunks are iterated over first, and the oldest
  chunks are iterated over last.

  If you were using `Bump::each_allocated_chunk` to iterate over data that you
  had previously allocated, and *you want to iterate in order of
  oldest-to-youngest allocation*, you need to reverse the chunks iterator and
  also reverse the order in which you loop through the data within a chunk!

  For example, if you had this code:

  ```rust
  unsafe {
      bump.each_allocated_chunk(|chunk| {
          for byte in chunk {
              // Touch each byte in oldest-to-youngest allocation order...
          }
      });
  }
  ```

  It should become this code:

  ```rust
  let mut chunks: Vec<_> = bump.iter_allocated_chunks().collect();
  chunks.reverse();
  for chunk in chunks {
      for byte in chunk.iter().rev() {
          let byte = unsafe { byte.assume_init() };
          // Touch each byte in oldest-to-youngest allocation order...
      }
  }
  ```

  The good news is that this change yielded a *speed up in allocation throughput
  of 3-19%!*

  See https://github.com/fitzgen/bumpalo/pull/37 and
  https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html for details.

* **BREAKING:** The `collections` cargo feature is no longer on by default. You
  must explicitly turn it on if you intend to use the `bumpalo::collections`
  module.

* `Bump::reset` will now retain only the last allocated chunk (the biggest),
  rather than only the first allocated chunk (the smallest). This should enable
  `Bump` to better adapt to workload sizes and quickly reach a steady state
  where new chunks are not requested from the global allocator.

### Removed

* The `Bump::each_allocated_chunk` method is removed in favor of
  `Bump::iter_allocated_chunks`. Note that its safety requirements for reading
  from the allocated chunks are slightly different from the old
  `each_allocated_chunk`: only up to 16-byte alignment is supported now. If you
  allocate anything with greater alignment than that into the bump arena, there
  might be uninitialized padding inserted in the chunks, and therefore it is no
  longer safe to read them via `MaybeUninit::assume_init`. See also the note
  about bump direction in the "changed" section; if you're iterating chunks,
  you're likely affected by that change!

* The `std` cargo feature has been removed, since this crate is now always
  no-std.

## Fixed

* Fixed a bug involving potential integer overflows with large requested
  allocation sizes.

--------------------------------------------------------------------------------

# 2.6.0

Released 2019-08-19.

* Implement `Send` for `Bump`.

--------------------------------------------------------------------------------

# 2.5.0

Released 2019-07-01.

* Add `alloc_slice_copy` and `alloc_slice_clone` methods that allocate space for
  slices and either copy (with bound `T: Copy`) or clone (with bound `T: Clone`)
  the provided slice's data into the newly allocated space.

--------------------------------------------------------------------------------

# 2.4.3

Released 2019-05-20.

* Fixed a bug where chunks were always deallocated with the default chunk
  layout, not the layout that the chunk was actually allocated with (i.e. if we
  started growing larger chunks with larger layouts, we would deallocate those
  chunks with an incorrect layout).

--------------------------------------------------------------------------------

# 2.4.2

Released 2019-05-17.

* Added an implementation `Default` for `Bump`.
* Made it so that if bump allocation within a chunk overflows, we still try to
  allocate a new chunk to bump out of for the requested allocation. This can
  avoid some OOMs in scenarios where the chunk we are currently allocating out
  of is very near the high end of the address space, and there is still
  available address space lower down for new chunks.

--------------------------------------------------------------------------------

# 2.4.1

Released 2019-04-19.

* Added readme metadata to Cargo.toml so it shows up on crates.io

--------------------------------------------------------------------------------

# 2.4.0

Released 2019-04-19.

* Added support for `realloc`ing in-place when the pointer being `realloc`ed is
  the last allocation made from the bump arena. This should speed up various
  `String`, `Vec`, and `format!` operations in many cases.

--------------------------------------------------------------------------------

# 2.3.0

Released 2019-03-26.

* Add the `alloc_with` method, that (usually) avoids stack-allocating the
  allocated value and then moving it into the bump arena. This avoids potential
  stack overflows in release mode when allocating very large objects, and also
  some `memcpy` calls. This is similar to the `copyless` crate. Read [the
  `alloc_with` doc comments][alloc-with-doc-comments] and [the original issue
  proposing this API][issue-proposing-alloc-with] for more.

[alloc-with-doc-comments]: https://github.com/fitzgen/bumpalo/blob/9f47aee8a6839ba65c073b9ad5372aacbbd02352/src/lib.rs#L436-L475
[issue-proposing-alloc-with]: https://github.com/fitzgen/bumpalo/issues/10

--------------------------------------------------------------------------------

# 2.2.2

Released 2019-03-18.

* Fix a regression from 2.2.1 where chunks were not always aligned to the chunk
  footer's alignment.

--------------------------------------------------------------------------------

# 2.2.1

Released 2019-03-18.

* Fix a regression in 2.2.0 where newly allocated bump chunks could fail to have
  capacity for a large requested bump allocation in some corner cases.

--------------------------------------------------------------------------------

# 2.2.0

Released 2019-03-15.

* Chunks in an arena now start out small, and double in size as more chunks are
  requested.

--------------------------------------------------------------------------------

# 2.1.0

Released 2019-02-12.

* Added the `into_bump_slice` method on `bumpalo::collections::Vec<T>`.

--------------------------------------------------------------------------------

# 2.0.0

Released 2019-02-11.

* Removed the `BumpAllocSafe` trait.
* Correctly detect overflows from large allocations and panic.

--------------------------------------------------------------------------------

# 1.2.0

Released 2019-01-15.

* Fixed an overly-aggressive `debug_assert!` that had false positives.
* Ported to Rust 2018 edition.

--------------------------------------------------------------------------------

# 1.1.0

Released 2018-11-28.

* Added the `collections` module, which contains ports of `std`'s collection
  types that are compatible with backing their storage in `Bump` arenas.
* Lifted the limits on size and alignment of allocations.

--------------------------------------------------------------------------------

# 1.0.2

--------------------------------------------------------------------------------

# 1.0.1

--------------------------------------------------------------------------------

# 1.0.0
