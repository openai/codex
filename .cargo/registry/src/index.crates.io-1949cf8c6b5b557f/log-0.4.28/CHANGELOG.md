# Change Log

## [Unreleased]

## [0.4.28] - 2025-09-02

## What's Changed
* ci: drop really old trick and ensure MSRV for all feature combo by @tisonkun in https://github.com/rust-lang/log/pull/676
* Chore: delete compare_exchange method for AtomicUsize on platforms without atomics  by @HaoliangXu in https://github.com/rust-lang/log/pull/690
* Add `increment_severity()` and `decrement_severity()` methods for `Level` and `LevelFilter` by @nebkor in https://github.com/rust-lang/log/pull/692

## New Contributors
* @xixishidibei made their first contribution in https://github.com/rust-lang/log/pull/677
* @ZylosLumen made their first contribution in https://github.com/rust-lang/log/pull/688
* @HaoliangXu made their first contribution in https://github.com/rust-lang/log/pull/690
* @nebkor made their first contribution in https://github.com/rust-lang/log/pull/692

**Full Changelog**: https://github.com/rust-lang/log/compare/0.4.27...0.4.28

### Notable Changes
* MSRV is bumped to 1.61.0 in https://github.com/rust-lang/log/pull/676

## [0.4.27] - 2025-03-24

### What's Changed
* A few minor lint fixes by @nyurik in https://github.com/rust-lang/log/pull/671
* Enable clippy support for format-like macros by @nyurik in https://github.com/rust-lang/log/pull/665
* Add an optional logger param by @tisonkun in https://github.com/rust-lang/log/pull/664
* Pass global logger by value, supplied logger by ref by @KodrAus in https://github.com/rust-lang/log/pull/673


**Full Changelog**: https://github.com/rust-lang/log/compare/0.4.26...0.4.27


## [0.4.26] - 2025-02-18

### What's Changed
* Derive `Clone` for `kv::Value` by @SpriteOvO in https://github.com/rust-lang/log/pull/668
* Add `spdlog-rs` link to crate doc by @SpriteOvO in https://github.com/rust-lang/log/pull/669


**Full Changelog**: https://github.com/rust-lang/log/compare/0.4.25...0.4.26

## [0.4.25] - 2025-01-14

### What's Changed
* Revert loosening of kv cargo features by @KodrAus in https://github.com/rust-lang/log/pull/662


**Full Changelog**: https://github.com/rust-lang/log/compare/0.4.24...0.4.25

## [0.4.24] - 2025-01-11

### What's Changed
* Fix up kv feature activation by @KodrAus in https://github.com/rust-lang/log/pull/659


**Full Changelog**: https://github.com/rust-lang/log/compare/0.4.23...0.4.24

## [0.4.23] - 2025-01-10 (yanked)

### What's Changed
* Fix some typos by @Kleinmarb in https://github.com/rust-lang/log/pull/637
* Add logforth to implementation by @tisonkun in https://github.com/rust-lang/log/pull/638
* Add `spdlog-rs` link to README by @SpriteOvO in https://github.com/rust-lang/log/pull/639
* Add correct lifetime to kv::Value::to_borrowed_str by @stevenroose in https://github.com/rust-lang/log/pull/643
* docs: Add logforth as an impl by @tisonkun in https://github.com/rust-lang/log/pull/642
* Add clang_log implementation by @DDAN-17 in https://github.com/rust-lang/log/pull/646
* Bind lifetimes of &str returned from Key by the lifetime of 'k rather than the lifetime of the Key struct by @gbbosak in https://github.com/rust-lang/log/pull/648
* Fix up key lifetimes and add method to try get a borrowed key by @KodrAus in https://github.com/rust-lang/log/pull/653
* Add Ftail implementation by @tjardoo in https://github.com/rust-lang/log/pull/652

### New Contributors
* @Kleinmarb made their first contribution in https://github.com/rust-lang/log/pull/637
* @tisonkun made their first contribution in https://github.com/rust-lang/log/pull/638
* @SpriteOvO made their first contribution in https://github.com/rust-lang/log/pull/639
* @stevenroose made their first contribution in https://github.com/rust-lang/log/pull/643
* @DDAN-17 made their first contribution in https://github.com/rust-lang/log/pull/646
* @gbbosak made their first contribution in https://github.com/rust-lang/log/pull/648
* @tjardoo made their first contribution in https://github.com/rust-lang/log/pull/652

**Full Changelog**: https://github.com/rust-lang/log/compare/0.4.22...0.4.23

## [0.4.22] - 2024-06-27

### What's Changed
* Add some clarifications to the library docs by @KodrAus in https://github.com/rust-lang/log/pull/620
* Add links to `colog` crate by @chrivers in https://github.com/rust-lang/log/pull/621
* adding line_number test + updating some testing infrastructure by @DIvkov575 in https://github.com/rust-lang/log/pull/619
* Clarify the actual set of functions that can race in _racy variants by @KodrAus in https://github.com/rust-lang/log/pull/623
* Replace deprecated std::sync::atomic::spin_loop_hint() by @Catamantaloedis in https://github.com/rust-lang/log/pull/625
* Check usage of max_level features by @Thomasdezeeuw in https://github.com/rust-lang/log/pull/627
* Remove unneeded import by @Thomasdezeeuw in https://github.com/rust-lang/log/pull/628
* Loosen orderings for logger initialization in https://github.com/rust-lang/log/pull/632. Originally by @pwoolcoc in https://github.com/rust-lang/log/pull/599
* Use Location::caller() for file and line info in https://github.com/rust-lang/log/pull/633. Originally by @Cassy343 in https://github.com/rust-lang/log/pull/520

### New Contributors
* @chrivers made their first contribution in https://github.com/rust-lang/log/pull/621
* @DIvkov575 made their first contribution in https://github.com/rust-lang/log/pull/619
* @Catamantaloedis made their first contribution in https://github.com/rust-lang/log/pull/625

**Full Changelog**: https://github.com/rust-lang/log/compare/0.4.21...0.4.22

## [0.4.21] - 2024-02-27

### What's Changed
* Minor clippy nits by @nyurik in https://github.com/rust-lang/log/pull/578
* Simplify Display impl by @nyurik in https://github.com/rust-lang/log/pull/579
* Set all crates to 2021 edition by @nyurik in https://github.com/rust-lang/log/pull/580
* Various changes based on review by @Thomasdezeeuw in https://github.com/rust-lang/log/pull/583
* Fix typo in file_static() method doc by @dimo414 in https://github.com/rust-lang/log/pull/590
* Specialize empty key value pairs by @EFanZh in https://github.com/rust-lang/log/pull/576
* Fix incorrect lifetime in Value::to_str() by @peterjoel in https://github.com/rust-lang/log/pull/587
* Remove some API of the key-value feature by @Thomasdezeeuw in https://github.com/rust-lang/log/pull/585
* Add logcontrol-log and log-reload by @swsnr in https://github.com/rust-lang/log/pull/595
* Add Serialization section to kv::Value docs by @Thomasdezeeuw in https://github.com/rust-lang/log/pull/593
* Rename Value::to_str to to_cow_str by @Thomasdezeeuw in https://github.com/rust-lang/log/pull/592
* Clarify documentation and simplify initialization of `STATIC_MAX_LEVEL` by @ptosi in https://github.com/rust-lang/log/pull/594
* Update docs to 2021 edition, test by @nyurik in https://github.com/rust-lang/log/pull/577
* Add "alterable_logger" link to README.md by @brummer-simon in https://github.com/rust-lang/log/pull/589
* Normalize line ending by @EFanZh in https://github.com/rust-lang/log/pull/602
* Remove `ok_or` in favor of `Option::ok_or` by @AngelicosPhosphoros in https://github.com/rust-lang/log/pull/607
* Use `Acquire` ordering for initialization check by @AngelicosPhosphoros in https://github.com/rust-lang/log/pull/610
* Get structured logging API ready for stabilization by @KodrAus in https://github.com/rust-lang/log/pull/613

### New Contributors
* @nyurik made their first contribution in https://github.com/rust-lang/log/pull/578
* @dimo414 made their first contribution in https://github.com/rust-lang/log/pull/590
* @peterjoel made their first contribution in https://github.com/rust-lang/log/pull/587
* @ptosi made their first contribution in https://github.com/rust-lang/log/pull/594
* @brummer-simon made their first contribution in https://github.com/rust-lang/log/pull/589
* @AngelicosPhosphoros made their first contribution in https://github.com/rust-lang/log/pull/607

## [0.4.20] - 2023-07-11

* Remove rustversion dev-dependency by @Thomasdezeeuw in https://github.com/rust-lang/log/pull/568
* Remove `local_inner_macros` usage by @EFanZh in https://github.com/rust-lang/log/pull/570

## [0.4.19] - 2023-06-10

* Use target_has_atomic instead of the old atomic_cas cfg by @GuillaumeGomez in https://github.com/rust-lang/log/pull/555
* Put MSRV into Cargo.toml by @est31 in https://github.com/rust-lang/log/pull/557

## [0.4.18] - 2023-05-28

* fix Markdown links (again) by @hellow554 in https://github.com/rust-lang/log/pull/513
* add cargo doc to workflow by @hellow554 in https://github.com/rust-lang/log/pull/515
* Apply Clippy lints by @hellow554 in https://github.com/rust-lang/log/pull/516
* Replace ad-hoc eq_ignore_ascii_case with slice::eq_ignore_ascii_case by @glandium in https://github.com/rust-lang/log/pull/519
* fix up windows targets by @KodrAus in https://github.com/rust-lang/log/pull/528
* typo fix by @jiangying000 in https://github.com/rust-lang/log/pull/529
* Remove dependency on cfg_if by @EriKWDev in https://github.com/rust-lang/log/pull/536
* GitHub Workflows security hardening by @sashashura in https://github.com/rust-lang/log/pull/538
* Fix build status badge by @atouchet in https://github.com/rust-lang/log/pull/539
* Add call_logger to the documentation by @a1ecbr0wn in https://github.com/rust-lang/log/pull/547
* Use stable internals for key-value API by @KodrAus in https://github.com/rust-lang/log/pull/550
* Change wording of list of implementations by @Thomasdezeeuw in https://github.com/rust-lang/log/pull/553
* Add std-logger to list of implementations by @Thomasdezeeuw in https://github.com/rust-lang/log/pull/554
* Add `set_max_level_racy` and gate `set_max_level` by @djkoloski in https://github.com/rust-lang/log/pull/544
* [doc] src/lib.rs : prefix an unused variable with an underscore by @OccupyMars2025 in https://github.com/rust-lang/log/pull/561
* [doc] src/macros.rs :  correct  grammar errors of an example in lib documentation by @OccupyMars2025 in https://github.com/rust-lang/log/pull/562

## [0.4.17] - 2022-04-29

* Update `kv_unstable` internal dependencies.

## [0.4.16] - 2022-03-22

* Fix a conflict with unqualified `Option` use in macros.

## [0.4.15] - 2022-02-23

* Silence a warning about the deprecated `spin_loop_hint`.
* Relax ordering in the atomic `set_max_level` call.
* Add thumbv4t-none-eabi to targets that don't support atomics
* Allow levels to be iterated over.
* Implement `Log` on some common wrapper types.
* Improvements to test coverage.
* Improvements to documentation.
* Add key-value support to the `log!` macros.
* Tighten `kv_unstable` internal dependencies, so they don't bump past their current alpha.
* Add a simple visit API to `kv_unstable`.
* Support `NonZero*` integers as values in structured logging
* Support static strings as keys in structured logging

## [0.4.14] - 2021-01-27

* Remove the `__private_api_log_lit` special case.
* Fixed incorrect combination of `kv_unstable` and `std` features causing compile failures.
* Remove unstable `Value::to_*` conversions that were incorrectly using `as`.
* Rename unstable `Value::to_error` to `Value::to_borrowed_error`.

## [0.4.13] - 2021-01-11

* This is the same as `0.4.11`, except with a `kv_unstable_std` feature added to aid migrating current dependents to `0.4.14` (which was originally going to be `0.4.13` until it was decided to create a patch from `0.4.11` to minimize disruption).

## [0.4.12] - 2020-12-24

### New

* Support platforms without atomics by racing instead of failing to compile
* Implement `Log` for `Box<T: Log>`
* Update `cfg-if` to `1.0`
* Internal reworks of the structured logging API. Removed the `Fill` API
and added `source::as_map` and `source::as_list` to easily serialize a `Source`
as either a map of `{key: value, ..}` or as a list of `[(key, value), ..]`.

### Fixed

* Fixed deserialization of `LevelFilter` to use their `u64` index variants

## [0.4.11] - 2020-07-09

### New

* Support coercing structured values into concrete types.
* Reference the `win_dbg_logger` in the readme.

### Fixed

* Updates a few deprecated items used internally.
* Fixed issues in docs and expands sections.
* Show the correct build badge in the readme.
* Fix up a possible inference breakage with structured value errors.
* Respect formatting flags in structured value formatting.

## [0.4.10] - 2019-12-16 (yanked)

### Fixed

* Fixed the `log!` macros, so they work in expression context (this regressed in `0.4.9`, which has been yanked).

## [0.4.9] - 2019-12-12 (yanked)

### Minimum Supported Rust Version

This release bumps the minimum compiler version to `1.31.0`. This was mainly needed for `cfg-if`,
but between `1.16.0` and `1.31.0` there are a lot of language and library improvements we now
take advantage of.

### New

* Unstable support for capturing key-value pairs in a record using the `log!` macros

### Improved

* Better documentation for max level filters.
* Internal updates to line up with bumped MSRV

## [0.4.8] - 2019-07-28

### New

* Support attempting to get `Record` fields as static strings.

## [0.4.7] - 2019-07-06

### New

* Support for embedded environments with thread-unsafe initialization.
* Initial unstable support for capturing structured data under the `kv_unstable`
feature gate. This new API doesn't affect existing users and may change in future
patches (so those changes may not appear in the changelog until it stabilizes).

### Improved

* Docs for using `log` with the 2018 edition.
* Error messages for macros missing arguments.

## [0.4.6] - 2018-10-27

### Improved

* Support 2018-style macro import for the `log_enabled!` macro.

## [0.4.5] - 2018-09-03

### Improved

* Make `log`'s internal helper macros less likely to conflict with user-defined
  macros.

## [0.4.4] - 2018-08-17

### Improved

* Support 2018-style imports of the log macros.

## [0.4.3] - 2018-06-29

### Improved

* More code generation improvements.

## [0.4.2] - 2018-06-05

### Improved

* Log invocations now generate less code.

### Fixed

* Example Logger implementations now properly set the max log level.

## [0.4.1] - 2017-12-30

### Fixed

* Some doc links were fixed.

## [0.4.0] - 2017-12-24

The changes in this release include cleanup of some obscure functionality and a more robust public
API designed to support bridges to other logging systems, and provide more flexibility to new
features in the future.

### Compatibility

Vast portions of the Rust ecosystem use the 0.3.x release series of log, and we don't want to force
the community to go through the pain of upgrading every crate to 0.4.x at the exact same time. Along
with 0.4.0, we've published a new 0.3.9 release which acts as a "shim" over 0.4.0. This will allow
crates using either version to coexist without losing messages from one side or the other.

There is one caveat - a log message generated by a crate using 0.4.x but consumed by a logging
implementation using 0.3.x will not have a file name or module path. Applications affected by this
can upgrade their logging implementations to one using 0.4.x to avoid losing this information. The
other direction does not lose any information, fortunately!

**TL;DR** Libraries should feel comfortable upgrading to 0.4.0 without treating that as a breaking
change. Applications may need to update their logging implementation (e.g. env-logger) to a newer
version using log 0.4.x to avoid losing module and file information.

### New

* The crate is now `no_std` by default.
* `Level` and `LevelFilter` now implement `Serialize` and `Deserialize` when the `serde` feature is
    enabled.
* The `Record` and `Metadata` types can now be constructed by third-party code via a builder API.
* The `logger` free function returns a reference to the logger implementation. This, along with the
    ability to construct `Record`s, makes it possible to bridge from another logging framework to
    this one without digging into the private internals of the crate. The standard `error!` `warn!`,
    etc., macros now exclusively use the public API of the crate rather than "secret" internal APIs.
* `Log::flush` has been added to allow crates to tell the logging implementation to ensure that all
    "in flight" log events have been persisted. This can be used, for example, just before an
    application exits to ensure that asynchronous log sinks finish their work.

### Removed

* The `shutdown` and `shutdown_raw` functions have been removed. Supporting shutdown significantly
    complicated the implementation and imposed a performance cost on each logging operation.
* The `log_panics` function and its associated `nightly` Cargo feature have been removed. Use the
    [log-panics](https://crates.io/crates/log-panics) instead.

### Changed

* The `Log` prefix has been removed from type names. For example, `LogLevelFilter` is now
    `LevelFilter`, and `LogRecord` is now `Record`.
* The `MaxLogLevelFilter` object has been removed in favor of a `set_max_level` free function.
* The `set_logger` free functions have been restructured. The logger is now directly passed to the
    functions rather than a closure which returns the logger. `set_logger` now takes a `&'static
    Log` and is usable in `no_std` contexts in place of the old `set_logger_raw`. `set_boxed_logger`
    is a convenience function which takes a `Box<Log>` but otherwise acts like `set_logger`. It
    requires the `std` feature.
* The `file` and `module_path` values in `Record` no longer have the `'static` lifetime to support
    integration with other logging frameworks that don't provide a `'static` lifetime for the
    equivalent values.
* The `file`, `line`, and `module_path` values in `Record` are now `Option`s to support integration
    with other logging frameworks that don't provide those values.

### In the Future

* We're looking to add support for *structured* logging - the inclusion of extra key-value pairs of
    information in a log event in addition to the normal string message. This should be able to be
    added in a backwards compatible manner to the 0.4.x series when the design is worked out.

## Older

Look at the [release tags] for information about older releases.

[Unreleased]: https://github.com/rust-lang-nursery/log/compare/0.4.28...HEAD
[0.4.28]: https://github.com/rust-lang/log/compare/0.4.27...0.4.28
[0.4.27]: https://github.com/rust-lang/log/compare/0.4.26...0.4.27
[0.4.26]: https://github.com/rust-lang/log/compare/0.4.25...0.4.26
[0.4.25]: https://github.com/rust-lang/log/compare/0.4.24...0.4.25
[0.4.24]: https://github.com/rust-lang/log/compare/0.4.23...0.4.24
[0.4.23]: https://github.com/rust-lang/log/compare/0.4.22...0.4.23
[0.4.22]: https://github.com/rust-lang/log/compare/0.4.21...0.4.22
[0.4.21]: https://github.com/rust-lang/log/compare/0.4.20...0.4.21
[0.4.20]: https://github.com/rust-lang-nursery/log/compare/0.4.19...0.4.20
[0.4.19]: https://github.com/rust-lang-nursery/log/compare/0.4.18...0.4.19
[0.4.18]: https://github.com/rust-lang-nursery/log/compare/0.4.17...0.4.18
[0.4.17]: https://github.com/rust-lang-nursery/log/compare/0.4.16...0.4.17
[0.4.16]: https://github.com/rust-lang-nursery/log/compare/0.4.15...0.4.16
[0.4.15]: https://github.com/rust-lang-nursery/log/compare/0.4.13...0.4.15
[0.4.14]: https://github.com/rust-lang-nursery/log/compare/0.4.13...0.4.14
[0.4.13]: https://github.com/rust-lang-nursery/log/compare/0.4.11...0.4.13
[0.4.12]: https://github.com/rust-lang-nursery/log/compare/0.4.11...0.4.12
[0.4.11]: https://github.com/rust-lang-nursery/log/compare/0.4.10...0.4.11
[0.4.10]: https://github.com/rust-lang-nursery/log/compare/0.4.9...0.4.10
[0.4.9]: https://github.com/rust-lang-nursery/log/compare/0.4.8...0.4.9
[0.4.8]: https://github.com/rust-lang-nursery/log/compare/0.4.7...0.4.8
[0.4.7]: https://github.com/rust-lang-nursery/log/compare/0.4.6...0.4.7
[0.4.6]: https://github.com/rust-lang-nursery/log/compare/0.4.5...0.4.6
[0.4.5]: https://github.com/rust-lang-nursery/log/compare/0.4.4...0.4.5
[0.4.4]: https://github.com/rust-lang-nursery/log/compare/0.4.3...0.4.4
[0.4.3]: https://github.com/rust-lang-nursery/log/compare/0.4.2...0.4.3
[0.4.2]: https://github.com/rust-lang-nursery/log/compare/0.4.1...0.4.2
[0.4.1]: https://github.com/rust-lang-nursery/log/compare/0.4.0...0.4.1
[0.4.0]: https://github.com/rust-lang-nursery/log/compare/0.3.8...0.4.0
[release tags]: https://github.com/rust-lang-nursery/log/releases
