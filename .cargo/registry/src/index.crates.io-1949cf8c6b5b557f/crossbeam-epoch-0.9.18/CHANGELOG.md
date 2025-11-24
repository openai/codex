# Version 0.9.18

- Remove dependency on `cfg-if`. (#1072)
- Remove dependency on `autocfg`. (#1071)

# Version 0.9.17

- Remove dependency on `memoffset`. (#1058)

# Version 0.9.16

- Bump the minimum supported Rust version to 1.61. (#1037)
- Improve support for targets without atomic CAS. (#1037)
- Remove build script. (#1037)
- Remove dependency on `scopeguard`. (#1045)
- Update `loom` dependency to 0.7.

# Version 0.9.15

- Update `memoffset` to 0.9. (#981)

# Version 0.9.14

- Update `memoffset` to 0.8. (#955)

# Version 0.9.13

- Fix build script bug introduced in 0.9.12. (#932)

# Version 0.9.12

**Note:** This release has been yanked due to regression fixed in 0.9.13.

- Update `memoffset` to 0.7. (#926)
- Improve support for custom targets. (#922)

# Version 0.9.11

- Removes the dependency on the `once_cell` crate to restore the MSRV. (#913)
- Work around [rust-lang#98302](https://github.com/rust-lang/rust/issues/98302), which causes compile error on windows-gnu when LTO is enabled. (#913)

# Version 0.9.10

- Bump the minimum supported Rust version to 1.38. (#877)
- Mitigate the risk of segmentation faults in buggy downstream implementations. (#879)
- Add `{Atomic, Shared}::try_into_owned` (#701)

# Version 0.9.9

- Replace lazy_static with once_cell. (#817)

# Version 0.9.8

- Make `Atomic::null()` const function at 1.61+. (#797)

# Version 0.9.7

- Fix Miri error when `-Zmiri-check-number-validity` is enabled. (#779)

# Version 0.9.6

- Add `Atomic::fetch_update`. (#706)

# Version 0.9.5

- Fix UB in `Pointable` impl of `[MaybeUninit<T>]`. (#694)
- Support targets that do not have atomic CAS on stable Rust. (#698)
- Fix breakage with nightly feature due to rust-lang/rust#84510. (#692)

# Version 0.9.4

**Note**: This release has been yanked. See [#693](https://github.com/crossbeam-rs/crossbeam/issues/693) for details.

- Fix UB in `<[MaybeUninit<T>] as Pointable>::init` when global allocator failed allocation. (#690)
- Bump `loom` dependency to version 0.5. (#686)

# Version 0.9.3

**Note**: This release has been yanked. See [#693](https://github.com/crossbeam-rs/crossbeam/issues/693) for details.

- Make `loom` dependency optional. (#666)

# Version 0.9.2

**Note**: This release has been yanked. See [#693](https://github.com/crossbeam-rs/crossbeam/issues/693) for details.

- Add `Atomic::compare_exchange` and `Atomic::compare_exchange_weak`. (#628)
- Deprecate `Atomic::compare_and_set` and `Atomic::compare_and_set_weak`. Use `Atomic::compare_exchange` or `Atomic::compare_exchange_weak` instead. (#628)
- Make `const_fn` dependency optional. (#611)
- Add unstable support for `loom`. (#487)

# Version 0.9.1

**Note**: This release has been yanked. See [#693](https://github.com/crossbeam-rs/crossbeam/issues/693) for details.

- Bump `memoffset` dependency to version 0.6. (#592)

# Version 0.9.0

**Note**: This release has been yanked. See [#693](https://github.com/crossbeam-rs/crossbeam/issues/693) for details.

- Bump the minimum supported Rust version to 1.36.
- Support dynamically sized types.

# Version 0.8.2

- Fix bug in release (yanking 0.8.1)

# Version 0.8.1

- Bump `autocfg` dependency to version 1.0. (#460)
- Reduce stall in list iteration. (#376)
- Stop stealing from the same deque. (#448)
- Fix unsoundness issues by adopting `MaybeUninit`. (#458)
- Fix use-after-free in lock-free queue. (#466)

# Version 0.8.0

- Bump the minimum required version to 1.28.
- Fix breakage with nightly feature due to rust-lang/rust#65214.
- Make `Atomic::null()` const function at 1.31+.
- Bump `crossbeam-utils` to `0.7`.

# Version 0.7.2

- Add `Atomic::into_owned()`.
- Update `memoffset` dependency.

# Version 0.7.1

- Add `Shared::deref_mut()`.
- Add a Treiber stack to examples.

# Version 0.7.0

- Remove `Guard::clone()`.
- Bump dependencies.

# Version 0.6.1

- Update `crossbeam-utils` to `0.6`.

# Version 0.6.0

- `defer` now requires `F: Send + 'static`.
- Bump the minimum Rust version to 1.26.
- Pinning while TLS is tearing down does not fail anymore.
- Rename `Handle` to `LocalHandle`.
- Add `defer_unchecked` and `defer_destroy`.
- Remove `Clone` impl for `LocalHandle`.

# Version 0.5.2

- Update `crossbeam-utils` to `0.5`.

# Version 0.5.1

- Fix compatibility with the latest Rust nightly.

# Version 0.5.0

- Update `crossbeam-utils` to `0.4`.
- Specify the minimum Rust version to `1.25.0`.

# Version 0.4.3

- Downgrade `crossbeam-utils` to `0.3` because it was a breaking change.

# Version 0.4.2

- Expose the `Pointer` trait.
- Warn missing docs and missing debug impls.
- Update `crossbeam-utils` to `0.4`.

# Version 0.4.1

- Add `Debug` impls for `Collector`, `Handle`, and `Guard`.
- Add `load_consume` to `Atomic`.
- Rename `Collector::handle` to `Collector::register`.
- Remove the `Send` implementation for `Handle` (this was a bug). Only
  `Collector`s can be shared among multiple threads, while `Handle`s and
  `Guard`s must stay within the thread in which they were created.

# Version 0.4.0

- Update dependencies.
- Remove support for Rust 1.13.

# Version 0.3.0

- Add support for Rust 1.13.
- Improve documentation for CAS.

# Version 0.2.0

- Add method `Owned::into_box`.
- Fix a use-after-free bug in `Local::finalize`.
- Fix an ordering bug in `Global::push_bag`.
- Fix a bug in calculating distance between epochs.
- Remove `impl<T> Into<Box<T>> for Owned<T>`.

# Version 0.1.0

- First version of the new epoch-based GC.
