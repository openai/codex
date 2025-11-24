# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## `parking_lot` - [0.12.4](https://github.com/Amanieu/parking_lot/compare/parking_lot-v0.12.3...parking_lot-v0.12.4) - 2025-05-29

- Fix parked upgraders potentially not being woken up after a write lock
- Fix clearing `PARKED_WRITER_BIT` after a timeout

## `parking_lot_core` - [0.9.11](https://github.com/Amanieu/parking_lot/compare/parking_lot_core-v0.9.10...parking_lot_core-v0.9.11) - 2025-05-29

- Use Release/Acquire ordering in thread_parker::windows::Backend::create
- Remove warnings due to new lint on unknown cfgs

## `lock_api` - [0.4.13](https://github.com/Amanieu/parking_lot/compare/lock_api-v0.4.12...lock_api-v0.4.13) - 2025-05-29

- Remove warnings due to new lint on unknown cfgs

## parking_lot 0.12.3 (2024-05-24)

- Export types provided by arc_lock feature (#442)

## parking_lot 0.12.2, parking_lot_core 0.9.10, lock_api 0.4.12 (2024-04-15)

- Fixed panic when calling `with_upgraded` twice on a `ArcRwLockUpgradableReadGuard` (#431)
- Fixed `RwLockUpgradeableReadGuard::with_upgraded` 
- Added lock_api::{Mutex, ReentrantMutex, RwLock}::from_raw methods (#429)
- Added Apple visionOS support (#433)

## parking_lot_core 0.9.9, lock_api 0.4.11 (2023-10-18)

- Fixed `RwLockUpgradeableReadGuard::with_upgraded`. (#393)
- Fixed `ReentrantMutex::bump` lock count. (#390)
- Added methods to unsafely create a lock guard out of thin air. (#403)
- Added support for Apple tvOS. (#405)

## parking_lot_core 0.9.8, lock_api 0.4.10 (2023-06-05)

- Mark guards with `#[clippy::has_significant_drop]` (#369, #371)
- Removed windows-sys dependency (#374, #378)
- Add `atomic_usize` default feature to support platforms without atomics. (#380)
- Add with_upgraded API to upgradable read locks (#386)
- Make RwLock guards Sync again (#370)

## parking_lot_core 0.9.7 (2023-02-01)

- Update windows-sys dependency to 0.45. (#368)

## parking_lot_core 0.9.6 (2023-01-11)

- Add support for watchOS. (#367)

## parking_lot_core 0.9.5 (2022-11-29)

- Update use of `libc::timespec` to prepare for future libc version (#363)

## parking_lot_core 0.9.4 (2022-10-18)

- Bump windows-sys dependency to 0.42. (#356)

## lock_api 0.4.9 (2022-09-20)

- Fixed `ReentrantMutexGuard::try_map` signature (#355)

## lock_api 0.4.8 (2022-08-28)

- Fixed unsound `Sync`/`Send` impls for `ArcMutexGuard`. (#349)
- Added `ArcMutexGuard::into_arc`. (#350)

## parking_lot 0.12.1 (2022-05-31)

- Fixed incorrect memory ordering in `RwLock`. (#344)
- Added `Condvar::wait_while` convenience methods (#343)

## parking_lot_core 0.9.3 (2022-04-30)

- Bump windows-sys dependency to 0.36. (#339)

## parking_lot_core 0.9.2, lock_api 0.4.7 (2022-03-25)

- Enable const new() on lock types on stable. (#325)
- Added `MutexGuard::leak` function. (#333)
- Bump windows-sys dependency to 0.34. (#331)
- Bump petgraph dependency to 0.6. (#326)
- Don't use pthread attributes on the espidf platform. (#319)

## parking_lot_core 0.9.1 (2022-02-06)

- Bump windows-sys dependency to 0.32. (#316)

## parking_lot 0.12.0, parking_lot_core 0.9.0, lock_api 0.4.6 (2022-01-28)

- The MSRV is bumped to 1.49.0.
- Disabled eventual fairness on wasm32-unknown-unknown. (#302)
- Added a rwlock method to report if lock is held exclusively. (#303)
- Use new `asm!` macro. (#304)
- Use windows-rs instead of winapi for faster builds. (#311)
- Moved hardware lock elision support to a separate Cargo feature. (#313)
- Removed used of deprecated `spin_loop_hint`. (#314)

## parking_lot 0.11.2, parking_lot_core 0.8.4, lock_api 0.4.5 (2021-08-28)

- Fixed incorrect memory orderings on `RwLock` and `WordLock`. (#294, #292)
- Added `Arc`-based lock guards. (#291)
- Added workaround for TSan's lack of support for `fence`. (#292)

## lock_api 0.4.4 (2021-05-01)

- Update for latest nightly. (#281)

## lock_api 0.4.3 (2021-04-03)

- Added `[Raw]ReentrantMutex::is_owned`. (#280)

## parking_lot_core 0.8.3 (2021-02-12)

- Updated smallvec to 1.6. (#276)

## parking_lot_core 0.8.2 (2020-12-21)

- Fixed assertion failure on OpenBSD. (#270)

## parking_lot_core 0.8.1 (2020-12-04)

- Removed deprecated CloudABI support. (#263)
- Fixed build on wasm32-unknown-unknown. (#265)
- Relaxed dependency on `smallvec`. (#266)

## parking_lot 0.11.1, lock_api 0.4.2 (2020-11-18)

- Fix bounds on Send and Sync impls for lock guards. (#262)
- Fix incorrect memory ordering in `RwLock`. (#260)

## lock_api 0.4.1 (2020-07-06)

- Add `data_ptr` method to lock types to allow unsafely accessing the inner data
  without a guard. (#247)

## parking_lot 0.11.0, parking_lot_core 0.8.0, lock_api 0.4.0 (2020-06-23)

- Add `is_locked` method to mutex types. (#235)
- Make `RawReentrantMutex` public. (#233)
- Allow lock guard to be sent to another thread with the `send_guard` feature. (#240)
- Use `Instant` type from the `instant` crate on wasm32-unknown-unknown. (#231)
- Remove deprecated and unsound `MappedRwLockWriteGuard::downgrade`. (#244)
- Most methods on the `Raw*` traits have been made unsafe since they assume
  the current thread holds the lock. (#243)

## parking_lot_core 0.7.2 (2020-04-21)

- Add support for `wasm32-unknown-unknown` under the "nightly" feature. (#226)

## parking_lot 0.10.2 (2020-04-10)

- Update minimum version of `lock_api`.

## parking_lot 0.10.1, parking_lot_core 0.7.1, lock_api 0.3.4 (2020-04-10)

- Add methods to construct `Mutex`, `RwLock`, etc in a `const` context. (#217)
- Add `FairMutex` which always uses fair unlocking. (#204)
- Fixed panic with deadlock detection on macOS. (#203)
- Fixed incorrect synchronization in `create_hashtable`. (#210)
- Use `llvm_asm!` instead of the deprecated `asm!`. (#223)

## lock_api 0.3.3 (2020-01-04)

- Deprecate unsound `MappedRwLockWriteGuard::downgrade` (#198)

## parking_lot 0.10.0, parking_lot_core 0.7.0, lock_api 0.3.2 (2019-11-25)

- Upgrade smallvec dependency to 1.0 in parking_lot_core.
- Replace all usage of `mem::uninitialized` with `mem::MaybeUninit`.
- The minimum required Rust version is bumped to 1.36. Because of the above two changes.
- Make methods on `WaitTimeoutResult` and `OnceState` take `self` by value instead of reference.

## parking_lot_core 0.6.2 (2019-07-22)

- Fixed compile error on Windows with old cfg_if version. (#164)

## parking_lot_core 0.6.1 (2019-07-17)

- Fixed Android build. (#163)

## parking_lot 0.9.0, parking_lot_core 0.6.0, lock_api 0.3.1 (2019-07-14)

- Re-export lock_api (0.3.1) from parking_lot (#150)
- Removed (non-dev) dependency on rand crate for fairness mechanism, by
  including a simple xorshift PRNG in core (#144)
- Android now uses the futex-based ThreadParker. (#140)
- Fixed CloudABI ThreadParker. (#140)
- Fix race condition in lock_api::ReentrantMutex (da16c2c7)

## lock_api 0.3.0 (2019-07-03, _yanked_)

- Use NonZeroUsize in GetThreadId::nonzero_thread_id (#148)
- Debug assert lock_count in ReentrantMutex (#148)
- Tag as `unsafe` and document some internal methods (#148)
- This release was _yanked_ due to a regression in ReentrantMutex (da16c2c7)

## parking_lot 0.8.1 (2019-07-03, _yanked_)

- Re-export lock_api (0.3.0) from parking_lot (#150)
- This release was _yanked_ from crates.io due to unexpected breakage (#156)

## parking_lot 0.8.0, parking_lot_core 0.5.0, lock_api 0.2.0 (2019-05-04)

- Fix race conditions in deadlock detection.
- Support for more platforms by adding ThreadParker implementations for
  Wasm, Redox, SGX and CloudABI.
- Drop support for older Rust. parking_lot now requires 1.31 and is a
  Rust 2018 edition crate (#122).
- Disable the owning_ref feature by default.
- Fix was_last_thread value in the timeout callback of park() (#129).
- Support single byte Mutex/Once on stable Rust when compiler is at least
  version 1.34.
- Make Condvar::new and Once::new const fns on stable Rust and remove
  ONCE_INIT (#134).
- Add optional Serde support (#135).

## parking_lot 0.7.1 (2019-01-01)

- Fixed potential deadlock when upgrading a RwLock.
- Fixed overflow panic on very long timeouts (#111).

## parking_lot 0.7.0, parking_lot_core 0.4.0 (2018-11-26)

- Return if or how many threads were notified from `Condvar::notify_*`

## parking_lot 0.6.3 (2018-07-18)

- Export `RawMutex`, `RawRwLock` and `RawThreadId`.

## parking_lot 0.6.2 (2018-06-18)

- Enable `lock_api/nightly` feature from `parking_lot/nightly` (#79)

## parking_lot 0.6.1 (2018-06-08)

Added missing typedefs for mapped lock guards:

- `MappedMutexGuard`
- `MappedReentrantMutexGuard`
- `MappedRwLockReadGuard`
- `MappedRwLockWriteGuard`

## parking_lot 0.6.0 (2018-06-08)

This release moves most of the code for type-safe `Mutex` and `RwLock` types
into a separate crate called `lock_api`. This new crate is compatible with
`no_std` and provides `Mutex` and `RwLock` type-safe wrapper types from a raw
mutex type which implements the `RawMutex` or `RawRwLock` trait. The API
provided by the wrapper types can be extended by implementing more traits on
the raw mutex type which provide more functionality (e.g. `RawMutexTimed`). See
the crate documentation for more details.

There are also several major changes:

- The minimum required Rust version is bumped to 1.26.
- All methods on `MutexGuard` (and other guard types) are no longer inherent
  methods and must be called as `MutexGuard::method(self)`. This avoids
  conflicts with methods from the inner type.
- `MutexGuard` (and other guard types) add the `unlocked` method which
  temporarily unlocks a mutex, runs the given closure, and then re-locks the
   mutex.
- `MutexGuard` (and other guard types) add the `bump` method which gives a
  chance for other threads to acquire the mutex by temporarily unlocking it and
  re-locking it. However this is optimized for the common case where there are
  no threads waiting on the lock, in which case no unlocking is performed.
- `MutexGuard` (and other guard types) add the `map` method which returns a
  `MappedMutexGuard` which holds only a subset of the original locked type. The
  `MappedMutexGuard` type is identical to `MutexGuard` except that it does not
  support the `unlocked` and `bump` methods, and can't be used with `CondVar`.
