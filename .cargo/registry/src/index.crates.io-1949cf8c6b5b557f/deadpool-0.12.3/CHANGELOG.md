# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [0.12.3] - 2025-08-19

- Add unique `id` to objects which can be read via the `Object::id` method
- Add `WeakPool<T>` and `Pool::weak()` for non-owning, upgradeable pool references.
- Use `num_cpus::get() * 2` as default pool size instead of `num_cpu::get_physical() * 4`
  which should result in the same pool size for most systems with hyper threading but fixes
  the default pool size in containerized environments.
- Cache `num_cpus::get()` result to improve `Pool` creation performance.

## [0.12.2] - 2025-02-02

- Update `itertools` dependency to version `0.13.0`
- Change predicate parameter of `Pool::retain` method to `FnMut`
- Add `RetainResult` as return value of `Pool::retain` method
- Fix panic in `Pool::resize` method caused by shrinking and
  growing the pool in quick succession.

## [0.12.1] - 2024-05-07

- Add WASM support

## [0.12.0] - 2024-05-04

- Add `Send` to `Manager::Type` and `Manager::Error` associated types
- Add `Send` to `Manager::create` and `Manager::recycle` return types

## [0.11.2] - 2024-04-10

- Make `Timeouts::new` and `Timeouts::wait_millis` functions const fns

## [0.11.1] - 2024-04-06

- Remove unused `console` dependency

## [0.11.0] - 2024-04-01

- Remove `async_trait` dependency
- Bump up MSRV to `1.75`

## [0.10.0] - 2023-09-25

- Remove unreachable enum variant `BuildError::Backend`
- Split `Status.available` into `available` and `waiting`.
- Add `QueueMode` configuration option for choosing between
  a `FIFO` (default) and `LIFO` queue.
- Remove `HookError::Continue` and `HookError::Abort` variants
  replacing it with the contents of `HookErrorCause`. Returning
  a `HookError` from a `post_create` hook causes the `Pool::get`
  operation to fail while returning it from a `pre_recycle` or
  `post_recycle` hook the operation continues.
- Add `metrics` argument to `Manager::recycle` method.
- Remove deprecated `managed::sync` module.
- Remove deprecated `managed::Pool::try_get` method.
- Bump up MSRV to `1.63` to match the one of `tokio`.

## [0.9.5] - 2022-05-20

- Fix bug causing the pool to exceed its `max_size` in the
  case of a recycling error.
- Fix panic caused by an integer overflow in the case of
  a failing `post_create` hook.

## [0.9.4] - 2022-04-27

- Fix `HookError` and `HookErrorCause` in re-exports

## [0.9.3] - 2022-04-12

- Add `Pool::retain` method
- Fix `Pool::get_timeouts` method
- Deprecate `managed::Pool::try_get`
- Add `Pool::timeouts` method

## [0.9.2] - 2021-11-15

- `PoolConfig` now implements `Serialize`

## [0.9.1] - 2021-10-26

- Deprecate `managed::sync` module in favor of `deadpool-sync` crate
- Extract `runtime` module as separate `deadpool-runtime` crate

## [0.9.0] - 2021-10-18

- __Breaking:__ Replace `config` feature with `serde` (opted out by default)
- Fix `std::error::Error::source` implementations for library errors
- Add `Runtime::spawn_blocking` method
- Add `Runtime::spawn_blocking_background` method
- Remove `Runtime::None` in favor of `Option<Runtime>`
- Remove `Pool::new` method
- Add `Pool::builder` method and `PoolBuilder` struct
- Add `Object::metrics` method and `Metrics` struct
- Update `tokio` dependency to version `1.5.0`
- Add `post_create`, `pre_recycle` and `post_recycle` hooks
- Add `Pool::resize` method
- Add `managed_reexports` macro

## [0.8.2] - 2021-07-16

- Add `deadpool-diesel` to README
- Add `Sync + Send` as supertrait to `Manager`
- Fix usage of `PhantomData` in `Pool` struct: `Pool is now `Sync` regardless of the wrapper.

## [0.8.1] - 2021-07-04

- Add `Object::pool` method

## [0.8.0] - 2021-05-21

- Add support for closing pools
- Replace `crossbeam-queue` by `Mutex<VecDeque<_>>`
- Fix invalid `size` and `available` counts when recycling fails
- Update `config` dependency to version `0.11`
- Remove deprecated `from_env` methods
- Add support for wrappers returned by the pool
- Use associated types for traits

## [0.7.0] - 2020-12-26

- Update `tokio` dependency to version `1`

## [0.6.0] - 2020-11-04

- Update `tokio` dependency to version `0.3`
- Update `crossbeam-queue` dependency to version `0.3`
- Remove deprecated `deadpool::*` types
- Add `deadpool-memcached` to README

## [0.5.2] - 2020-07-14

- Deprecate `managed::Config::from_env`
- Deprecate `unmanaged::Config::from_env`

## [0.5.1] - 2020-01-18

- Add `managed::Object::take` method

## [0.5.0] - 2020-01-16

- Move current pool implementation into `managed` module
- Add unmanaged version of the `Pool` which does not use a `Manager`
  to create and recycle objects.
- Add feature flags `"managed"` and `"unmanaged"` to enable only parts
  of this crate.
- Add `max_size` to pool `Status`
- Add support for `config` crate

## [0.4.3] - 2019-12-23

- Add `std::error::Error` implementation for `PoolError` and `RecycleError`.
  This makes it more convenient to use the `?` operator.

## [0.4.2] - 2019-12-23

- Replace `tokio::sync::mpsc::channel` by `crossbeam_queue::ArrayQueue`
  which gets rid of the mutex when fetching an object from the pool.

## [0.4.1] - 2019-12-19

- Make `Pool::timeout_get` public

## [0.4.0] - 2019-12-19

- Add support for timeouts
- Make fields of pool status public
- Fix possible deadlock and make implementation a lot simpler by using
  the new tokio `Semaphore` and `Receiver::try_recv`.
- Add `Pool::try_get` and `Pool::timeout_get` functions

## [0.3.0] - 2019-12-13

- Add `deadpool-lapin` to README
- Add `deadpool-redis` to README
- Fix possible stale state and deadlock if a future calling `Pool::get` is
  aborted. This is related to <https://github.com/tokio-rs/tokio/issues/1898>
- Make recycling more robust by changing the `Manager::recycle` to a non
  consuming API.

## [0.2.3] - 2019-12-02

- Add documentation for `docs.rs`
- Remove `PoolInner` and `PoolSize` struct from public interface
- Improve example in `README.md` and crate root

## [0.2.2] - 2019-12-02

- Update to `tokio 0.2`

## 0.2.1

- Version skipped; only `tokio-postgres` was updated.

## [0.2.0] - 2019-11-14

- Split `deadpool` and `deadpool-postgres` in separate crates instead of
    one with feature flags.

## [0.1.0] - 2019-11-14

- First release

<!-- next-url -->
[Unreleased]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.12.3...HEAD
[0.12.3]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.12.2...deadpool-v0.12.3
[0.12.2]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.12.1...deadpool-v0.12.2
[0.12.1]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.12.0...deadpool-v0.12.1
[0.12.0]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.11.2...deadpool-v0.12.0
[0.11.2]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.11.1...deadpool-v0.11.2
[0.11.1]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.11.0...deadpool-v0.11.1
[0.11.0]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.10.0...deadpool-v0.11.0
[0.10.0]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.9.5...deadpool-v0.10.0
[0.9.5]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.9.4...deadpool-v0.9.5
[0.9.4]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.9.3...deadpool-v0.9.4
[0.9.3]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.9.2...deadpool-v0.9.3
[0.9.2]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.9.1...deadpool-v0.9.2
[0.9.1]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.9.0...deadpool-v0.9.1
[0.9.0]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.8.2...deadpool-v0.9.0
[0.8.2]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.8.1...deadpool-v0.8.2
[0.8.1]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.8.0...deadpool-v0.8.1
[0.8.0]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.7.0...deadpool-v0.8.0
[0.7.0]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.6.0...deadpool-v0.7.0
[0.6.0]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.5.2...deadpool-v0.6.0
[0.5.2]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.5.1...deadpool-v0.5.2
[0.5.1]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.5.0...deadpool-v0.5.1
[0.5.0]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.4.4...deadpool-v0.5.0
[0.4.4]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.4.3...deadpool-v0.4.4
[0.4.3]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.4.2...deadpool-v0.4.3
[0.4.2]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.4.1...deadpool-v0.4.2
[0.4.1]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.4.0...deadpool-v0.4.1
[0.4.0]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.3.0...deadpool-v0.4.0
[0.3.0]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.2.3...deadpool-v0.3.0
[0.2.3]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.2.2...deadpool-v0.2.3
[0.2.2]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.2.1...deadpool-v0.2.2
[0.2.0]: https://github.com/bikeshedder/deadpool/compare/deadpool-v0.1.0...deadpool-v0.2.0
[0.1.0]: https://github.com/bikeshedder/deadpool/releases/tag/deadpool-v0.1.0
