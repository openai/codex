# Changelog

## 1.21.3

- Outline more initialization in `race`: [#284](https://github.com/matklad/once_cell/pull/284),
  [#285](https://github.com/matklad/once_cell/pull/285).

## 1.21.2
- Relax success ordering from AcqRel to Release in `race`: [#278](https://github.com/matklad/once_cell/pull/278).

## 1.21.1
- Reduce MSRV to 1.65: [#277](https://github.com/matklad/once_cell/pull/277).

## 1.21.0

- Outline initialization in `race`: [#273](https://github.com/matklad/once_cell/pull/273).
- Add `OnceNonZereUsize::get_unchecked`: [#274](https://github.com/matklad/once_cell/pull/274).
- Add `OnceBox::clone` and `OnceBox::with_value`: [#275](https://github.com/matklad/once_cell/pull/275).
- Increase MSRV to 1.70

## 1.20.2

- Remove `portable_atomic` from Cargo.lock if it is not, in fact, used: [#267](https://github.com/matklad/once_cell/pull/267)
  This is a work-around for this cargo bug: https://github.com/rust-lang/cargo/issues/10801.

## 1.20.1

- Allow using `race` module using just `portable_atomic`, without `critical_section` and provide
  better error messages on targets without atomic CAS instruction,
  [#265](https://github.com/matklad/once_cell/pull/265).

## 1.19.0

- Use `portable-atomic` instead of `atomic-polyfill`, [#251](https://github.com/matklad/once_cell/pull/251).

## 1.18.0

- `MSRV` is updated to 1.60.0 to take advantage of `dep:` syntax for cargo features,
  removing "implementation details" from publicly visible surface.

## 1.17.2

- Avoid unnecessary synchronization in `Lazy::{force,deref}_mut()`, [#231](https://github.com/matklad/once_cell/pull/231).

## 1.17.1

- Make `OnceRef` implementation compliant with [strict provenance](https://github.com/rust-lang/rust/issues/95228).

## 1.17.0

- Add `race::OnceRef` for storing a `&'a T`.

## 1.16.0

- Add `no_std` implementation based on `critical-section`,
  [#195](https://github.com/matklad/once_cell/pull/195).
- Deprecate `atomic-polyfill` feature (use the new `critical-section` instead)

## 1.15.0

- Increase minimal supported Rust version to 1.56.0.
- Implement `UnwindSafe` even if the `std` feature is disabled.

## 1.14.0

- Add extension to `unsync` and `sync` `Lazy` mut API:
  - `force_mut`
  - `get_mut`


## 1.13.1

- Make implementation compliant with [strict provenance](https://github.com/rust-lang/rust/issues/95228).
- Upgrade `atomic-polyfill` to `1.0`

## 1.13.0

- Add `Lazy::get`, similar to `OnceCell::get`.

## 1.12.1

- Remove incorrect `debug_assert`.

## 1.12.0

- Add `OnceCell::wait`, a blocking variant of `get`.

## 1.11.0

- Add `OnceCell::with_value` to create initialized `OnceCell` in `const` context.
- Improve `Clone` implementation for `OnceCell`.
- Rewrite `parking_lot` version on top of `parking_lot_core`, for even smaller cells!

## 1.10.0

- upgrade `parking_lot` to `0.12.0` (note that this bumps MSRV with `parking_lot` feature enabled to `1.49.0`).

## 1.9.0

- Added an `atomic-polyfill` optional dependency to compile `race` on platforms without atomics

## 1.8.0

- Add `try_insert` API -- a version of `set` that returns a reference.

## 1.7.2

- Improve code size when using parking_lot feature.

## 1.7.1

- Fix `race::OnceBox<T>` to also impl `Default` even if `T` doesn't impl `Default`.

## 1.7.0

- Hide the `race` module behind (default) `race` feature.
  Turns out that adding `race` by default was a breaking change on some platforms without atomics.
  In this release, we make the module opt-out.
  Technically, this is a breaking change for those who use `race` with `no_default_features`.
  Given that the `race` module itself only several days old, the breakage is deemed acceptable.

## 1.6.0

- Add `Lazy::into_value`
- Stabilize `once_cell::race` module for "first one wins" no_std-compatible initialization flavor.
- Migrate from deprecated `compare_and_swap` to `compare_exchange`.

## 1.5.2

- `OnceBox` API uses `Box<T>`.
  This a breaking change to unstable API.

## 1.5.1

- MSRV is increased to `1.36.0`.
- document `once_cell::race` module.
- introduce `alloc` feature for `OnceBox`.
- fix `OnceBox::set`.

## 1.5.0

- add new `once_cell::race` module for "first one wins" no_std-compatible initialization flavor.
  The API is provisional, subject to change and is gated by the `unstable` cargo feature.

## 1.4.1

- upgrade `parking_lot` to `0.11.0`
- make `sync::OnceCell<T>` pass https://doc.rust-lang.org/nomicon/dropck.html#an-escape-hatch[dropck] with `parking_lot` feature enabled.
  This fixes a (minor) semver-incompatible changed introduced in `1.4.0`

## 1.4.0

- upgrade `parking_lot` to `0.10` (note that this bumps MSRV with `parking_lot` feature enabled to `1.36.0`).
- add `OnceCell::take`.
- upgrade crossbeam utils (private dependency) to `0.7`.

## 1.3.1

- remove unnecessary `F: fmt::Debug` bound from `impl fmt::Debug for Lazy<T, F>`.

## 1.3.0

- `Lazy<T>` now implements `DerefMut`.
- update implementation according to the latest changes in `std`.

## 1.2.0

- add `sync::OnceCell::get_unchecked`.

## 1.1.0

- implement `Default` for `Lazy`: it creates an empty `Lazy<T>` which is initialized with `T::default` on first access.
- add `OnceCell::get_mut`.

## 1.0.2

- actually add `#![no_std]` attribute if std feature is not enabled.

## 1.0.1

- fix unsoundness in `Lazy<T>` if the initializing function panics. Thanks [@xfix](https://github.com/xfix)!
- implement `RefUnwindSafe` for `Lazy`.
- share more code between `std` and `parking_lot` implementations.
- add F.A.Q section to the docs.

## 1.0.0

- remove `parking_lot` from the list of default features.
- add `std` default feature. Without `std`, only `unsync` module is supported.
- implement `Eq` for `OnceCell`.
- fix wrong `Sync` bound on `sync::Lazy`.
- run the whole test suite with miri.

## 0.2.7

- New implementation of `sync::OnceCell` if `parking_lot` feature is disabled.
  It now employs a hand-rolled variant of `std::sync::Once`.
- `sync::OnceCell::get_or_try_init` works without `parking_lot` as well!
- document the effects of `parking_lot` feature: same performance but smaller types.

## 0.2.6

- Updated `Lazy`'s `Deref` impl to requires only `FnOnce` instead of `Fn`

## 0.2.5

- `Lazy` requires only `FnOnce` instead of `Fn`

## 0.2.4

- nicer `fmt::Debug` implementation

## 0.2.3

- update `parking_lot` to `0.9.0`
- fix stacked borrows violation in `unsync::OnceCell::get`
- implement `Clone` for `sync::OnceCell<T> where T: Clone`

## 0.2.2

- add `OnceCell::into_inner` which consumes a cell and returns an option

## 0.2.1

- implement `sync::OnceCell::get_or_try_init` if `parking_lot` feature is enabled
- switch internal `unsafe` implementation of `sync::OnceCell` from `Once` to `Mutex`
- `sync::OnceCell::get_or_init` is twice as fast if cell is already initialized
- implement `std::panic::RefUnwindSafe` and `std::panic::UnwindSafe` for `OnceCell`
- better document behavior around panics

## 0.2.0

- MSRV is now 1.31.1
- `Lazy::new` and `OnceCell::new` are now const-fns
- `unsync_lazy` and `sync_lazy` macros are removed

## 0.1.8

- update crossbeam-utils to 0.6
- enable bors-ng

## 0.1.7

- cells implement `PartialEq` and `From`
- MSRV is down to 1.24.1
- update `parking_lot` to `0.7.1`

## 0.1.6

- `unsync::OnceCell<T>` is `Clone` if `T` is `Clone`.

## 0.1.5

- No changelog until this point :(
