# Version 0.8.21

- Improve implementation of `CachePadded`. (#1152)

# Version 0.8.20

- Implement `Display` for `CachePadded`. (#1097)

# Version 0.8.19

- Remove dependency on `cfg-if`. (#1072)

# Version 0.8.18

- Relax the minimum supported Rust version to 1.60. (#1056)
- Improve scalability of `AtomicCell` fallback. (#1055)

# Version 0.8.17

- Bump the minimum supported Rust version to 1.61. (#1037)
- Improve support for targets without atomic CAS or 64-bit atomic. (#1037)
- Always implement `UnwindSafe` and `RefUnwindSafe` for `AtomicCell`. (#1045)
- Improve compatibility with Miri, TSan, and loom. (#995, #1003)
- Improve compatibility with unstable `oom=panic`. (#1045)
- Improve implementation of `CachePadded`. (#1014, #1025)
- Update `loom` dependency to 0.7.

# Version 0.8.16

- Improve implementation of `CachePadded`. (#967)

# Version 0.8.15

- Add `#[clippy::has_significant_drop]` to `ShardedLock{Read,Write}Guard`. (#958)
- Improve handling of very large timeout. (#953)
- Soft-deprecate `thread::scope()` in favor of the more efficient `std::thread::scope` that stabilized in Rust 1.63. (#954)

# Version 0.8.14

- Fix build script bug introduced in 0.8.13. (#932)

# Version 0.8.13

**Note:** This release has been yanked due to regression fixed in 0.8.14.

- Improve support for custom targets. (#922)

# Version 0.8.12

- Removes the dependency on the `once_cell` crate to restore the MSRV. (#913)
- Work around [rust-lang#98302](https://github.com/rust-lang/rust/issues/98302), which causes compile error on windows-gnu when LTO is enabled. (#913)

# Version 0.8.11

- Bump the minimum supported Rust version to 1.38. (#877)

# Version 0.8.10

- Fix unsoundness of `AtomicCell` on types containing niches. (#834)
  This fix contains breaking changes, but they are allowed because this is a soundness bug fix. See #834 for more.

# Version 0.8.9

- Replace lazy_static with once_cell. (#817)

# Version 0.8.8

- Fix a bug when unstable `loom` support is enabled. (#787)

# Version 0.8.7

- Add `AtomicCell<{i*,u*}>::{fetch_max,fetch_min}`. (#785)
- Add `AtomicCell<{i*,u*,bool}>::fetch_nand`. (#785)
- Fix unsoundness of `AtomicCell<{i,u}64>` arithmetics on 32-bit targets that support `Atomic{I,U}64` (#781)

# Version 0.8.6

**Note:** This release has been yanked. See [GHSA-qc84-gqf4-9926](https://github.com/crossbeam-rs/crossbeam/security/advisories/GHSA-qc84-gqf4-9926) for details.

- Re-add `AtomicCell<{i,u}64>::{fetch_add,fetch_sub,fetch_and,fetch_or,fetch_xor}` that were accidentally removed in 0.8.0 on targets that do not support `Atomic{I,U}64`. (#767)
- Re-add `AtomicCell<{i,u}128>::{fetch_add,fetch_sub,fetch_and,fetch_or,fetch_xor}` that were accidentally removed in 0.8.0. (#767)

# Version 0.8.5

**Note:** This release has been yanked. See [GHSA-qc84-gqf4-9926](https://github.com/crossbeam-rs/crossbeam/security/advisories/GHSA-qc84-gqf4-9926) for details.

- Add `AtomicCell::fetch_update`. (#704)
- Support targets that do not have atomic CAS on stable Rust. (#698)

# Version 0.8.4

**Note:** This release has been yanked. See [GHSA-qc84-gqf4-9926](https://github.com/crossbeam-rs/crossbeam/security/advisories/GHSA-qc84-gqf4-9926) for details.

- Bump `loom` dependency to version 0.5. (#686)

# Version 0.8.3

**Note:** This release has been yanked. See [GHSA-qc84-gqf4-9926](https://github.com/crossbeam-rs/crossbeam/security/advisories/GHSA-qc84-gqf4-9926) for details.

- Make `loom` dependency optional. (#666)

# Version 0.8.2

**Note:** This release has been yanked. See [GHSA-qc84-gqf4-9926](https://github.com/crossbeam-rs/crossbeam/security/advisories/GHSA-qc84-gqf4-9926) for details.

- Deprecate `AtomicCell::compare_and_swap`. Use `AtomicCell::compare_exchange` instead. (#619)
- Add `Parker::park_deadline`. (#563)
- Improve implementation of `CachePadded`. (#636)
- Add unstable support for `loom`. (#487)

# Version 0.8.1

**Note:** This release has been yanked. See [GHSA-qc84-gqf4-9926](https://github.com/crossbeam-rs/crossbeam/security/advisories/GHSA-qc84-gqf4-9926) for details.

- Make `AtomicCell::is_lock_free` always const fn. (#600)
- Fix a bug in `seq_lock_wide`. (#596)
- Remove `const_fn` dependency. (#600)
- `crossbeam-utils` no longer fails to compile if unable to determine rustc version. Instead, it now displays a warning. (#604)

# Version 0.8.0

**Note:** This release has been yanked. See [GHSA-qc84-gqf4-9926](https://github.com/crossbeam-rs/crossbeam/security/advisories/GHSA-qc84-gqf4-9926) for details.

- Bump the minimum supported Rust version to 1.36.
- Remove deprecated `AtomicCell::get_mut()` and `Backoff::is_complete()` methods.
- Remove `alloc` feature.
- Make `CachePadded::new()` const function.
- Make `AtomicCell::is_lock_free()` const function at 1.46+.
- Implement `From<T>` for `AtomicCell<T>`.

# Version 0.7.2

- Fix bug in release (yanking 0.7.1)

# Version 0.7.1

- Bump `autocfg` dependency to version 1.0. (#460)
- Make `AtomicCell` lockfree for u8, u16, u32, u64 sized values at 1.34+. (#454)

# Version 0.7.0

- Bump the minimum required version to 1.28.
- Fix breakage with nightly feature due to rust-lang/rust#65214.
- Apply `#[repr(transparent)]` to `AtomicCell`.
- Make `AtomicCell::new()` const function at 1.31+.

# Version 0.6.6

- Add `UnwindSafe` and `RefUnwindSafe` impls for `AtomicCell`.
- Add `AtomicCell::as_ptr()`.
- Add `AtomicCell::take()`.
- Fix a bug in `AtomicCell::compare_exchange()` and `AtomicCell::compare_and_swap()`.
- Various documentation improvements.

# Version 0.6.5

- Rename `Backoff::is_complete()` to `Backoff::is_completed()`.

# Version 0.6.4

- Add `WaitGroup`, `ShardedLock`, and `Backoff`.
- Add `fetch_*` methods for `AtomicCell<i128>` and `AtomicCell<u128>`.
- Expand documentation.

# Version 0.6.3

- Add `AtomicCell`.
- Improve documentation.

# Version 0.6.2

- Add `Parker`.
- Improve documentation.

# Version 0.6.1

- Fix a soundness bug in `Scope::spawn()`.
- Remove the `T: 'scope` bound on `ScopedJoinHandle`.

# Version 0.6.0

- Move `AtomicConsume` to `atomic` module.
- `scope()` returns a `Result` of thread joins.
- Remove `spawn_unchecked`.
- Fix a soundness bug due to incorrect lifetimes.
- Improve documentation.
- Support nested scoped spawns.
- Implement `Copy`, `Hash`, `PartialEq`, and `Eq` for `CachePadded`.
- Add `CachePadded::into_inner()`.

# Version 0.5.0

- Reorganize sub-modules and rename functions.

# Version 0.4.1

- Fix a documentation link.

# Version 0.4.0

- `CachePadded` supports types bigger than 64 bytes.
- Fix a bug in scoped threads where unitialized memory was being dropped.
- Minimum required Rust version is now 1.25.

# Version 0.3.2

- Mark `load_consume` with `#[inline]`.

# Version 0.3.1

- `load_consume` on ARM and AArch64.

# Version 0.3.0

- Add `join` for scoped thread API.
- Add `load_consume` for atomic load-consume memory ordering.
- Remove `AtomicOption`.

# Version 0.2.2

- Support Rust 1.12.1.
- Call `T::clone` when cloning a `CachePadded<T>`.

# Version 0.2.1

- Add `use_std` feature.

# Version 0.2.0

- Add `nightly` feature.
- Use `repr(align(64))` on `CachePadded` with the `nightly` feature.
- Implement `Drop` for `CachePadded<T>`.
- Implement `Clone` for `CachePadded<T>`.
- Implement `From<T>` for `CachePadded<T>`.
- Implement better `Debug` for `CachePadded<T>`.
- Write more tests.
- Add this changelog.
- Change cache line length to 64 bytes.
- Remove `ZerosValid`.

# Version 0.1.0

- Old implementation of `CachePadded` from `crossbeam` version 0.3.0
