# 0.3.20 (August 29, 2025)

[ [crates.io][crate-0.3.20] ] | [ [docs.rs][docs-0.3.20] ]

### Fixed

- Escape ANSI escape sequences in logs

# 0.3.19 (November 29, 2024)

[ [crates.io][crate-0.3.19] ] | [ [docs.rs][docs-0.3.19] ]

This release updates the `tracing` dependency to [v0.1.41][tracing-0.1.41] and
the `tracing-serde` dependency to [v0.2.0][tracing-serde-0.2.0].

### Added

- Add `set_span_events` to `fmt::Subscriber` ([#2962])
- **tracing**: Allow `&[u8]` to be recorded as event/span field ([#2954])

### Changed

- Set `log` max level when reloading ([#1270])
- Bump MSRV to 1.63 ([#2793])
- Use const `thread_local`s when possible ([#2838])
- Don't gate `with_ansi()` on the "ansi" feature ([#3020])
- Updated tracing-serde to 0.2.0 ([#3160])

[#1270]: https://github.com/tokio-rs/tracing/pull/1270
[#2793]: https://github.com/tokio-rs/tracing/pull/2793
[#2838]: https://github.com/tokio-rs/tracing/pull/2838
[#2954]: https://github.com/tokio-rs/tracing/pull/2954
[#2962]: https://github.com/tokio-rs/tracing/pull/2962
[#3020]: https://github.com/tokio-rs/tracing/pull/3020
[#3160]: https://github.com/tokio-rs/tracing/pull/3160
[tracing-0.1.41]:
    https://github.com/tokio-rs/tracing/releases/tag/tracing-0.1.41
[tracing-serde-0.2.0]:
    https://github.com/tokio-rs/tracing/releases/tag/tracing-serde-0.2.0
[docs-0.3.19]: https://docs.rs/tracing-subscriber/0.3.19/tracing_subscriber/
[crate-0.3.19]: https://crates.io/crates/tracing-subscriber/0.3.19

# 0.3.18 (November 13, 2023)

This release of `tracing-subscriber` adds support for the [`NO_COLOR`] environment
variable (an informal standard to disable emitting ANSI color escape codes) in
`fmt::Layer`, reintroduces support for the [`chrono`] crate, and increases the
minimum supported Rust version (MSRV) to Rust 1.63.0.

It also introduces several minor API improvements.

### Added

- **chrono**: Add [`chrono`] implementations of `FormatTime` ([#2690])
- **subscriber**: Add support for the [`NO_COLOR`] environment variable in
`fmt::Layer` ([#2647])
- **fmt**: make `format::Writer::new()` public ([#2680])
- **filter**: Implement `layer::Filter` for `Option<Filter>` ([#2407])

### Changed

- **log**: bump version of `tracing-log` to 0.2 ([#2772])
- Increased minimum supported Rust version (MSRV) to 1.63.0+.

[`chrono`]: https://github.com/chronotope/chrono
[`NO_COLOR`]: https://no-color.org/
[#2690]: https://github.com/tokio-rs/tracing/pull/2690
[#2647]: https://github.com/tokio-rs/tracing/pull/2647
[#2680]: https://github.com/tokio-rs/tracing/pull/2680
[#2407]: https://github.com/tokio-rs/tracing/pull/2407
[#2772]: https://github.com/tokio-rs/tracing/pull/2772

Thanks to @shayne-fletcher, @dmlary, @kaifastromai, and @jsgf for contributing!

# 0.3.17 (April 21, 2023)

This release of `tracing-subscriber` fixes a build error when using `env-filter`
with recent versions of the `regex` crate. It also introduces several minor API
improvements.

### Fixed

- **env-filter**: Add "unicode-case" and "unicode-perl" to the `regex`
  dependency, fixing a build error with recent versions of `regex` ([#2566])
- A number of minor documentation typos and other fixes ([#2384], [#2378],
  [#2368], [#2548])

### Added

- **filter**: Add `fmt::Display` impl for `filter::Targets` ([#2343])
- **fmt**: Made `with_ansi(false)` no longer require the "ansi" feature, so that
  ANSI formatting escapes can be disabled without requiring ANSI-specific
  dependencies ([#2532])

### Changed

- **fmt**: Dim targets in the `Compact` formatter, matching the default
  formatter ([#2409])

Thanks to @keepsimple1, @andrewhalle, @LeoniePhiline, @LukeMathWalker,
@howardjohn, @daxpedda, and @dbidwell94 for contributing to this release!

[#2566]: https://github.com/tokio-rs/tracing/pull/2566
[#2384]: https://github.com/tokio-rs/tracing/pull/2384
[#2378]: https://github.com/tokio-rs/tracing/pull/2378
[#2368]: https://github.com/tokio-rs/tracing/pull/2368
[#2548]: https://github.com/tokio-rs/tracing/pull/2548
[#2343]: https://github.com/tokio-rs/tracing/pull/2343
[#2532]: https://github.com/tokio-rs/tracing/pull/2532
[#2409]: https://github.com/tokio-rs/tracing/pull/2409

# 0.3.16 (October 6, 2022)

This release of `tracing-subscriber` fixes a regression introduced in
[v0.3.15][subscriber-0.3.15] where `Option::None`'s `Layer` implementation would
set the max level hint to `OFF`. In addition, it adds several new APIs,
including the `Filter::event_enabled` method for filtering events based on
fields values, and the ability to log internal errors that occur when writing a
log line.

This release also replaces the dependency on the unmaintained [`ansi-term`]
crate with the [`nu-ansi-term`] crate, resolving an *informational* security
advisory ([RUSTSEC-2021-0139]) for [`ansi-term`]'s maintainance status. This
increases the minimum supported Rust version (MSRV) to Rust 1.50+, although the
crate should still compile for the previous MSRV of Rust 1.49+ when the `ansi`
feature is not enabled.

### Fixed

- **layer**: `Option::None`'s `Layer` impl always setting the `max_level_hint`
  to `LevelFilter::OFF` ([#2321])
- Compilation with `-Z minimal versions` ([#2246])
- **env-filter**: Clarify that disabled level warnings are emitted by
  `tracing-subscriber` ([#2285])

### Added

- **fmt**: Log internal errors to `stderr` if writing a log line fails ([#2102])
- **fmt**: `FmtLayer::log_internal_errors` and
  `FmtSubscriber::log_internal_errors` methods for configuring whether internal
  writer errors are printed to `stderr` ([#2102])
- **fmt**: `#[must_use]` attributes on builders to warn if a `Subscriber` is
  configured but not set as the default subscriber ([#2239])
- **filter**: `Filter::event_enabled` method for filtering an event based on its
  fields ([#2245], [#2251])
- **filter**: `Targets::default_level` accessor ([#2242])

### Changed

- **ansi**: Replaced dependency on unmaintained `ansi-term` crate with
  `nu-ansi-term` (([#2287], fixes informational advisory [RUSTSEC-2021-0139])
- `tracing-core`: updated to [0.1.30][core-0.1.30]
- Minimum Supported Rust Version (MSRV) increased to Rust 1.50+ (when the
  `ansi`) feature flag is enabled ([#2287])

### Documented

- **fmt**: Correct inaccuracies in `fmt::init` documentation ([#2224])
- **filter**: Fix incorrect doc link in `filter::Not` combinator ([#2249])

Thanks to new contributors @cgbur, @DesmondWillowbrook, @RalfJung, and
@poliorcetics, as well as returning contributors @CAD97, @connec, @jswrenn,
@guswynn, and @bryangarza, for contributing to this release!

[nu-ansi-term]: https://github.com/nushell/nu-ansi-term
[ansi_term]: https://github.com/ogham/rust-ansi-term
[RUSTSEC-2021-0139]: https://rustsec.org/advisories/RUSTSEC-2021-0139.html
[core-0.1.30]: https://github.com/tokio-rs/tracing/releases/tag/tracing-core-0.1.30
[subscriber-0.3.15]: https://github.com/tokio-rs/tracing/releases/tag/tracing-subscriber-0.3.15
[#2321]: https://github.com/tokio-rs/tracing/pull/2321
[#2246]: https://github.com/tokio-rs/tracing/pull/2246
[#2285]: https://github.com/tokio-rs/tracing/pull/2285
[#2102]: https://github.com/tokio-rs/tracing/pull/2102
[#2239]: https://github.com/tokio-rs/tracing/pull/2239
[#2245]: https://github.com/tokio-rs/tracing/pull/2245
[#2251]: https://github.com/tokio-rs/tracing/pull/2251
[#2287]: https://github.com/tokio-rs/tracing/pull/2287
[#2224]: https://github.com/tokio-rs/tracing/pull/2224
[#2249]: https://github.com/tokio-rs/tracing/pull/2249

# 0.3.15 (Jul 20, 2022)

This release fixes a bug where the `reload` layer would fail to pass through
`max_level_hint` to the underlying layer, potentially breaking filtering.

### Fixed

- **reload**: pass through `max_level_hint` to the inner `Layer` ([#2204])

Thanks to @guswynn for contributing to this release!

[#2204]: https://github.com/tokio-rs/tracing/pull/2204

# 0.3.14 (Jul 1, 2022)

This release fixes multiple filtering bugs in the `Layer` implementations for
`Option<impl Layer>` and `Vec<impl Layer>`.

### Fixed

- **layer**: `Layer::event_enabled` implementation for `Option<impl Layer<S>>`
  returning `false` when the `Option` is `None`, disabling all events globally
  ([#2193])
- **layer**: `Layer::max_level_hint` implementation for `Option<impl Layer<S>>`
  incorrectly disabling max level filtering when the option is `None` ([#2195])
- **layer**: `Layer::max_level_hint` implementation for `Vec<impl Layer<S>>`
  returning `LevelFilter::ERROR` rather than `LevelFilter::OFF` when the `Vec`
  is empty ([#2195])

Thanks to @CAD97 and @guswynn for contributing to this release!

[#2193]: https://github.com/tokio-rs/tracing/pull/2193
[#2195]: https://github.com/tokio-rs/tracing/pull/2195

# 0.3.13 (Jun 30, 2022) (YANKED)

This release of `tracing-subscriber` fixes a compilation failure due to an
incorrect `tracing-core` dependency that was introduced in v0.3.12.

### Changed

- **tracing_core**: Updated minimum dependency version to 0.1.28 ([#2190])

[#2190]: https://github.com/tokio-rs/tracing/pull/2190

# 0.3.12 (Jun 29, 2022) (YANKED)

This release of `tracing-subscriber` adds a new `Layer::event_enabled` method,
which allows `Layer`s to filter events *after* their field values are recorded;
a `Filter` implementation for `reload::Layer`, to make using `reload` with
per-layer filtering more ergonomic, and additional inherent method downcasting
APIs for the `Layered` type. In addition, it includes dependency updates, and
minor fixes for documentation and feature flagging.

### Added

- **layer**: `Layer::event_enabled` method, which can be implemented to filter
  events based on their field values ([#2008])
- **reload**: `Filter` implementation for `reload::Layer` ([#2159])
- **layer**: `Layered::downcast_ref` and `Layered::is` inherent methods
  ([#2160])

### Changed

- **parking_lot**: Updated dependency on `parking_lot` to 0.13.0 ([#2143])
- Replaced `lazy_static` dependency with `once_cell` ([#2147])

### Fixed

- Don't enable `tracing-core` features by default ([#2107])
- Several documentation link and typo fixes ([#2064], [#2068], #[2077], [#2161],
  [#1088])

Thanks to @ben0x539, @jamesmunns, @georgemp, @james7132, @jswrenn, @CAD97, and
@guswynn for contributing to this release!

[#2008]: https://github.com/tokio-rs/tracing/pull/2008
[#2159]: https://github.com/tokio-rs/tracing/pull/2159
[#2160]: https://github.com/tokio-rs/tracing/pull/2160
[#2143]: https://github.com/tokio-rs/tracing/pull/2143
[#2107]: https://github.com/tokio-rs/tracing/pull/2107
[#2064]: https://github.com/tokio-rs/tracing/pull/2064
[#2068]: https://github.com/tokio-rs/tracing/pull/2068
[#2077]: https://github.com/tokio-rs/tracing/pull/2077
[#2161]: https://github.com/tokio-rs/tracing/pull/2161
[#1088]: https://github.com/tokio-rs/tracing/pull/1088

# 0.3.11 (Apr 9, 2022)

This is a bugfix release for the `Filter` implementation for `EnvFilter` added
in [v0.3.10].

### Fixed

- **env-filter**: Added missing `Filter::on_record` callback to `EnvFilter`'s
  `Filter` impl ([#2058])
- **env-filter**: Fixed method resolution issues when calling `EnvFilter`
  methods with both the `Filter` and `Layer` traits in scope ([#2057])
- **env-filter**: Fixed `EnvFilter::builder().parse()` and other parsing methods
  returning an error when parsing an empty string ([#2052])

Thanks to new contributor @Ma124 for contributing to this release!

[v0.3.10]: https://github.com/tokio-rs/tracing/releases/tag/tracing-subscriber-0.3.10
[#2058]: https://github.com/tokio-rs/tracing/pull/2058
[#2057]: https://github.com/tokio-rs/tracing/pull/2057
[#2052]: https://github.com/tokio-rs/tracing/pull/2052

# 0.3.10 (Apr 1, 2022)

This release adds several new features, including a `Filter` implementation and
new builder API for `EnvFilter`, support for using a `Vec<L> where L: Layer` as
a `Layer`, and a number of smaller API improvements to make working with dynamic
and reloadable layers easier.

### Added

- **registry**: Implement `Filter` for `EnvFilter`, allowing it to be used with
  per-layer filtering ([#1983])
- **registry**: `Filter::on_new_span`, `Filter::on_enter`,
  `Filter::on_exit`, `Filter::on_close` and `Filter::on_record` callbacks to
  allow `Filter`s to track span states internally ([#1973], [#2017], [#2031])
- **registry**: `Filtered::filter` and `Filtered::filter_mut` accessors
  ([#1959])
- **registry**: `Filtered::inner` and `Filtered::inner_mut` accessors to borrow
  the wrapped `Layer` ([#2034])
- **layer**: Implement `Layer` for `Vec<L: Layer>`, to allow composing together
  a dynamically sized list of `Layer`s ([#2027])
- **layer**: `Layer::boxed` method to make type-erasing `Layer`s easier
  ([#2026])
- **fmt**: `fmt::Layer::writer` and `fmt::Layer::writer_mut` accessors ([#2034])
- **fmt**: `fmt::Layer::set_ansi` method to allow changing the ANSI formatting
  configuration at runtime ([#2034])
- **env-filter**: `EnvFilter::builder` to configure a new `EnvFilter` prior to
  parsing it ([#2035])
- Several documentation fixes and improvements ([#1972], [#1971], [#2023],
  [#2023])

### Fixed

- **fmt**: `fmt::Layer`'s auto traits no longer depend on the `Subscriber` type
  parameter's auto traits ([#2025])
- **env-filter**: Fixed missing help text when the `ansi` feature is disabled
  ([#2029])

Thanks to new contributors @TimoFreiberg and @wagenet, as well as @CAD97 for
contributing to this release!

[#1983]: https://github.com/tokio-rs/tracing/pull/1983
[#1973]: https://github.com/tokio-rs/tracing/pull/1973
[#2017]: https://github.com/tokio-rs/tracing/pull/2017
[#2031]: https://github.com/tokio-rs/tracing/pull/2031
[#1959]: https://github.com/tokio-rs/tracing/pull/1959
[#2034]: https://github.com/tokio-rs/tracing/pull/2034
[#2027]: https://github.com/tokio-rs/tracing/pull/2027
[#2026]: https://github.com/tokio-rs/tracing/pull/2026
[#2035]: https://github.com/tokio-rs/tracing/pull/2035
[#1972]: https://github.com/tokio-rs/tracing/pull/1972
[#1971]: https://github.com/tokio-rs/tracing/pull/1971
[#2023]: https://github.com/tokio-rs/tracing/pull/2023
[#2025]: https://github.com/tokio-rs/tracing/pull/2025
[#2029]: https://github.com/tokio-rs/tracing/pull/2029

# 0.3.9 (Feb 17, 2022)

This release updates the minimum supported Rust version (MSRV) to 1.49.0, and
updates the (optional) dependency on `parking_lot` to v0.12.

### Changed

- Updated minimum supported Rust version (MSRV) to 1.49.0 ([#1913])
- `parking_lot`: updated to v0.12 ([008339d])

### Added

- **fmt**: Documentation improvements ([#1926], [#1927])

[#1913]: https://github.com/tokio-rs/tracing/pull/1913
[#1926]: https://github.com/tokio-rs/tracing/pull/1926
[#1927]: https://github.com/tokio-rs/tracing/pull/1927
[008339d]: https://github.com/tokio-rs/tracing/commit/008339d1e8750ffe7b4634fc7789bda0c522424f

# 0.3.8 (Feb 4, 2022)

This release adds *experimental* support for recording structured field
values using the [`valuable`] crate to the `format::Json` formatter. In
particular, user-defined types which are recorded using their
[`valuable::Valuable`] implementations will be serialized as JSON objects,
rather than using their `fmt::Debug` representation. See [this blog post][post]
for details on `valuable`.

Note that `valuable` support currently requires `--cfg tracing_unstable`. See
the documentation for details.

Additionally, this release includes a number of other smaller API improvements.

### Added

- **json**: Experimental support for recording [`valuable`] values as structured
  JSON ([#1862], [#1901])
- **filter**: `Targets::would_enable` method for testing if a `Targets` filter
  would enable a given target ([#1903])
- **fmt**: `map_event_format`, `map_fmt_fields`, and `map_writer` methods to
  `fmt::Layer` and `fmt::SubscriberBuilder` ([#1871])

### Changed

- `tracing-core`: updated to [0.1.22][core-0.1.22]

### Fixed

- Set `smallvec` minimal version to 1.2.0, to fix compilation errors with `-Z
  minimal-versions` ([#1890])
- Minor documentation fixes ([#1902], [#1893])

Thanks to @guswynn, @glts, and @lilyball for contributing to this release!

[`valuable`]: https://crates.io/crates/valuable
[`valuable::Valuable`]: https://docs.rs/valuable/latest/valuable/trait.Valuable.html
[post]: https://tokio.rs/blog/2021-05-valuable
[core-0.1.22]: https://github.com/tokio-rs/tracing/releases/tag/tracing-core-0.1.22
[#1862]: https://github.com/tokio-rs/tracing/pull/1862
[#1901]: https://github.com/tokio-rs/tracing/pull/1901
[#1903]: https://github.com/tokio-rs/tracing/pull/1903
[#1871]: https://github.com/tokio-rs/tracing/pull/1871
[#1890]: https://github.com/tokio-rs/tracing/pull/1890
[#1902]: https://github.com/tokio-rs/tracing/pull/1902
[#1893]: https://github.com/tokio-rs/tracing/pull/1893

# 0.3.7 (Jan 25, 2022)

This release adds combinators for combining filters.

Additionally, this release also updates the `thread-local` crate to v1.1.4,
fixing warnings for the security advisory [RUSTSEC-2022-0006]. Note that
previous versions of `tracing-subscriber` did not use any of the `thread-local`
crate's APIs effected by the vulnerability. However, updating the version fixes
warnings emitted by `cargo audit` and similar tools.

### Added

- **filter**: Added combinators for combining filters ([#1578])

### Fixed

- **registry**: Updated `thread-local` to v1.1.4 ([#1858])

Thanks to new contributor @matze for contributing to this release!

[RUSTSEC-2022-0006]: https://rustsec.org/advisories/RUSTSEC-2022-0006
[#1578]: https://github.com/tokio-rs/tracing/pull/1578
[#1858]: https://github.com/tokio-rs/tracing/pull/1858

# 0.3.6 (Jan 14, 2022)

This release adds configuration options to `tracing_subscriber::fmt` to log
source code locations for events.
### Added

- **fmt**: Added `with_file` and `with_line_number`
  configuration methods to `fmt::Format`, `fmt::SubscriberBuilder`, and
  `fmt::Layer` ([#1773])

### Fixed

- **fmt**: Removed incorrect leading comma from span fields with the `Pretty`
  formatter ([#1833])

### Deprecated

- **fmt**: Deprecated `Pretty::with_source_location`, as it can now be replaced
  by the more general `Format`, `SubscriberBuilder`, and `Layer` methods
  ([#1773])

Thanks to new contributor @renecouto for contributing to this release!

[#1773]: https://github.com/tokio-rs/tracing/pull/1773
[#1833]: https://github.com/tokio-rs/tracing/pull/1833

# 0.3.5 (Dec 29, 2021)

This release re-enables `RUST_LOG` filtering in `tracing_subscriber::fmt`'s
default initialization methods, and adds an `OffsetLocalTime` formatter for
using local timestamps with the `time` crate.

### Added

- **fmt**: Added `OffsetLocalTime` formatter to `fmt::time` for formatting local
  timestamps with a fixed offset ([#1772])

### Fixed

- **fmt**: Added a `Targets` filter to `fmt::init()` and `fmt::try_init()` when
  the "env-filter" feature is disabled, so that `RUST_LOG` is still honored
  ([#1781])

Thanks to @marienz and @ishitatsuyuki for contributing to this release!

[#1772]: https://github.com/tokio-rs/tracing/pull/1772
[#1781]: https://github.com/tokio-rs/tracing/pull/1781

# 0.3.4 (Dec 23, 2021) 

This release contains bugfixes for the `fmt` module, as well as documentation
improvements.

### Fixed

- **fmt**: Fixed `fmt` not emitting log lines when timestamp formatting fails
  ([#1689])
- **fmt**: Fixed double space before thread IDs with `Pretty` formatter
  ([#1778])
- Several documentation improvements ([#1608], [#1699], [#1701])

[#1689]: https://github.com/tokio-rs/tracing/pull/1689
[#1778]: https://github.com/tokio-rs/tracing/pull/1778
[#1608]: https://github.com/tokio-rs/tracing/pull/1608
[#1699]: https://github.com/tokio-rs/tracing/pull/1699
[#1701]: https://github.com/tokio-rs/tracing/pull/1701

Thanks to new contributors @Swatinem and @rukai for contributing to this
release!

# 0.3.3 (Nov 29, 2021)

This release fixes a pair of regressions in `tracing-subscriber`'s `fmt` module.

### Fixed

- **fmt**: Fixed missing event fields with `Compact` formatter ([#1755])
- **fmt**: Fixed `PrettyFields` formatter (and thus `format::Pretty` event
  formatter) ignoring the `fmt::Layer`'s ANSI color code configuration ([#1747])

[#1755]: https://github.com/tokio-rs/tracing/pull/1755
[#1747]: https://github.com/tokio-rs/tracing/pull/1747

# 0.3.2 (Nov 19, 2021)

### Fixed

- **fmt**: Fixed `MakeWriter` filtering not working with `BoxMakeWriter`
  ([#1694])

### Added

- **fmt**: `Writer::has_ansi_escapes` method to check if an output supports ANSI
  terminal formatting escape codes ([#1696])
- **fmt**: Added additional ANSI terminal formatting to field formatters when
  supported ([#1702])
- **fmt**: Added `FmtContext::span_scope`, `FmtContext::event_scope`, and
  `FmtContext::parent_span` methods for accessing the current span and its scope
  when formatting an event ([#1728])
- **fmt**: Improved documentation on implementing event formatters ([#1727])

[#1694]: https://github.com/tokio-rs/tracing/pull/1694
[#1696]: https://github.com/tokio-rs/tracing/pull/1696
[#1702]: https://github.com/tokio-rs/tracing/pull/1702
[#1728]: https://github.com/tokio-rs/tracing/pull/1728
[#1727]: https://github.com/tokio-rs/tracing/pull/1727
# 0.3.1 (Oct 25, 2021)

This release fixes a few issues related to feature flagging.

### Fixed

- **time**: Compilation error when enabling the "time" feature flag without also
  enabling the "local-time" feature flag ([#1685])
- **registry**: Unused method warnings when the "std" feature is enabled but the
  "registry" feature is disabled ([#1686])

[#1685]: https://github.com/tokio-rs/tracing/pull/1685
[#1686]: https://github.com/tokio-rs/tracing/pull/1686

# 0.3.0 (Oct 22, 2021)

This is a breaking release of `tracing-subscriber`. The primary breaking change
in this release is the removal of the dependency on the [`chrono` crate], due to
[RUSTSEC-2020-0159]. To replace `chrono`, support is added for formatting
timestamps using the [`time` crate] instead.

In addition, this release includes a number of other breaking API changes, such
as adding (limited) support for `#![no_std]` targets, removing previously
deprecated APIs, and more.

### Breaking Changes

- Removed APIs deprecated in the v0.2.x release series.
- Renamed `Layer::new_span` to `Layer::on_new_span` ([#1674])
- Removed `Layer` impl for `Arc<L: Layer<S>>` and `Arc<dyn Layer<S> + ...>`
  ([#1649])
- Replaced the [`chrono` crate] with the [`time` crate] for timestamp formatting, to
  resolve [RUSTSEC-2020-0159] ([#1646])
- Removed `json` and `env-filter` from default features. They must now be
  enabled explictly ([#1647]). This means that `RUST_LOG`-based filters _will not_
  work unless the `env-filter` feature is enabled.
- Changed `FormatEvent::format_event` and `FormatFields::format_fields`
  trait methods to take a `Writer` type, rather than a `&mut dyn fmt::Write`
  trait object ([#1661])
- Changed the signature of the `MakeWriter` trait by adding a lifetime parameter
  ([#781])

### Changed

- **layer**: Renamed `Layer::new_span` to `Layer::on_new_span` ([#1674])
- **fmt**: Changed `FormatEvent::format_event` and `FormatFields::format_fields`
  trait methods to take a `Writer` type, rather than a `&mut dyn fmt::Write`
  trait object ([#1661])
- **json**, **env-filter**: `json` and `env-filter` feature flags are no longer
  enabled by default ([#1647])
### Removed

- Removed deprecated `CurrentSpan` type ([#1320])
- **registry**: Removed deprecated `SpanRef::parents` iterator, replaced by
  `SpanRef::scope` in [#1431] ([#1648)])
- **layer**: Removed deprecated `Context::scope` iterator, replaced by
  `Context::span_scope` and `Context::event_scope` in [#1431] and [#1434]
  ([#1648)])
- **layer**: Removed `Layer` impl for `Arc<L: Layer<S>>` and
   `Arc<dyn Layer<S> + ...>`. These interfere with per-layer filtering. ([#1649])
- **fmt**: Removed deprecated `LayerBuilder` type ([#1673])
- **fmt**: Removed `fmt::Layer::on_event` (renamed to `fmt::Layer::fmt_event`)
  ([#1673])
- **fmt**, **chrono**: Removed the `chrono` feature flag and APIs for using the
  [`chrono` crate] for timestamp formatting ([#1646])
### Added

- **fmt**, **time**: `LocalTime` and `UtcTime` types for formatting timestamps
  using the [`time` crate] ([#1646])
- **fmt**: Added a lifetime parameter to the `MakeWriter` trait, allowing it to
  return a borrowed writer. This enables implementations of `MakeWriter` for
  types such as `Mutex<T: io::Write>` and `std::fs::File`. ([#781])
- **env-filter**: Documentation improvements ([#1637])
- Support for some APIs on `#![no_std]` targets, by disabling the `std` feature
  flag ([#1660])

Thanks to @Folyd and @nmathewson for contributing to this release!

[#1320]: https://github.com/tokio-rs/tracing/pull/1320
[#1673]: https://github.com/tokio-rs/tracing/pull/1673
[#1674]: https://github.com/tokio-rs/tracing/pull/1674
[#1646]: https://github.com/tokio-rs/tracing/pull/1646
[#1647]: https://github.com/tokio-rs/tracing/pull/1647
[#1648]: https://github.com/tokio-rs/tracing/pull/1648
[#1649]: https://github.com/tokio-rs/tracing/pull/1649
[#1660]: https://github.com/tokio-rs/tracing/pull/1660
[#1661]: https://github.com/tokio-rs/tracing/pull/1661
[#1431]: https://github.com/tokio-rs/tracing/pull/1431
[#1434]: https://github.com/tokio-rs/tracing/pull/1434
[#781]: https://github.com/tokio-rs/tracing/pull/781

[`chrono` crate]: https://crates.io/crates/chrono
[`time` crate]: https://crates.io/crates/time
[RUSTSEC-2020-0159]: https://rustsec.org/advisories/RUSTSEC-2020-0159.html

# 0.2.25 (October 5, 2021)

This release fixes an issue where a `Layer` implementation's custom
`downcast_raw` implementation was lost when wrapping that layer with a per-layer
filter.

### Fixed

- **registry**: Forward `Filtered::downcast_raw` to wrapped `Layer` ([#1619])

### Added

- Documentation improvements ([#1596], [#1601])

Thanks to @bryanburgers for contributing to this release!

[#1619]: https://github.com/tokio-rs/tracing/pull/1619
[#1601]: https://github.com/tokio-rs/tracing/pull/1601
[#1596]: https://github.com/tokio-rs/tracing/pull/1596

# 0.2.24 (September 19, 2021)

This release contains a number of bug fixes, including a fix for
`tracing-subscriber` failing to compile on the minimum supported Rust version of
1.42.0. It also adds `IntoIterator` implementations for the `Targets` type.

### Fixed

- Fixed compilation on Rust 1.42.0 ([#1580], [#1581])
- **registry**: Ensure per-layer filter `enabled` state is cleared when a global
  filter short-circuits filter evaluation ([#1575])
- **layer**: Fixed `Layer::on_layer` not being called for `Box`ed `Layer`s,
  which broke  per-layer filtering ([#1576])

### Added

- **filter**: Added `Targets::iter`, returning an iterator over the set of
  target-level pairs enabled by a `Targets` filter ([#1574])
- **filter**:  Added `IntoIterator` implementations for `Targets` and `&Targets`
  ([#1574])

Thanks to new contributor @connec for contributing to this release!

[#1580]: https://github.com/tokio-rs/tracing/pull/1580
[#1581]: https://github.com/tokio-rs/tracing/pull/1581
[#1575]: https://github.com/tokio-rs/tracing/pull/1575
[#1576]: https://github.com/tokio-rs/tracing/pull/1576
[#1574]: https://github.com/tokio-rs/tracing/pull/1574

# 0.2.23 (September 16, 2021)

This release fixes a few bugs in the per-layer filtering API added in v0.2.21.

### Fixed

- **env-filter**: Fixed excessive `EnvFilter` memory use ([#1568])
- **filter**: Fixed a panic that may occur in debug mode when using per-layer
  filters together with global filters ([#1569])
- Fixed incorrect documentation formatting ([#1572])

[#1568]: https://github.com/tokio-rs/tracing/pull/1568
[#1569]: https://github.com/tokio-rs/tracing/pull/1569
[#1572]: https://github.com/tokio-rs/tracing/pull/1572

# 0.2.22 (September 13, 2021)

This fixes a regression where the `filter::ParseError` type was accidentally
renamed.

### Fixed

- **filter**: Fix `filter::ParseError` accidentally being renamed to
  `filter::DirectiveParseError` ([#1558])

[#1558]: https://github.com/tokio-rs/tracing/pull/1558

# 0.2.21 (September 12, 2021)

This release introduces the [`Filter`] trait, a new API for [per-layer
filtering][plf]. This allows controlling which spans and events are recorded by
various layers individually, rather than globally.

In addition, it adds a new [`Targets`] filter, which provides a lighter-weight
version of the filtering provided by [`EnvFilter`], as well as other smaller API
improvements and fixes.

### Deprecated

- **registry**: `SpanRef::parent_id`, which cannot properly support per-layer
  filtering. Use `.parent().map(SpanRef::id)` instead. ([#1523])

### Fixed

- **layer** `Context` methods that are provided when the `Subscriber` implements
  `LookupSpan` no longer require the "registry" feature flag ([#1525])
- **layer** `fmt::Debug` implementation for `Layered` no longer requires the `S`
  type parameter to implement `Debug` ([#1528])

### Added

- **registry**: `Filter` trait, `Filtered` type, `Layer::with_filter` method,
  and other APIs for per-layer filtering ([#1523])
- **filter**: `FilterFn` and `DynFilterFn` types that implement global (`Layer`)
  and per-layer (`Filter`) filtering for closures and function pointers
  ([#1523])
- **filter**: `Targets` filter, which implements a lighter-weight form of
  `EnvFilter`-like filtering ([#1550])
- **env-filter**: Added support for filtering on floating-point values ([#1507])
- **layer**: `Layer::on_layer` callback, called when layering the `Layer` onto a
`Subscriber` ([#1523])
- **layer**: `Layer` implementations for `Box<L>` and `Arc<L>` where `L: Layer`
  ([#1536])
- **layer**: `Layer` implementations for `Box<dyn Layer<S> + Send + Sync + 'static>`
  and `Arc<dyn Layer<S> + Send + Sync + 'static>` ([#1536])
- A number of small documentation fixes and improvements ([#1553], [#1544],
  [#1539], [#1524])

Special thanks to new contributors @jsgf and @maxburke for contributing to this
release!

[`Filter`]: https://docs.rs/tracing-subscriber/0.2.21/tracing_subscriber/layer/trait.Filter.html
[plf]: https://docs.rs/tracing-subscriber/0.2.21/tracing_subscriber/layer/index.html#per-layer-filtering
[`Targets`]: https://docs.rs/tracing-subscriber/0.2.21/tracing_subscriber/filter/struct.Targets.html
[`EnvFilter`]: https://docs.rs/tracing-subscriber/0.2.21/tracing_subscriber/filter/struct.EnvFilter.html
[#1507]: https://github.com/tokio-rs/tracing/pull/1507
[#1523]: https://github.com/tokio-rs/tracing/pull/1523
[#1524]: https://github.com/tokio-rs/tracing/pull/1524
[#1525]: https://github.com/tokio-rs/tracing/pull/1525
[#1528]: https://github.com/tokio-rs/tracing/pull/1528
[#1539]: https://github.com/tokio-rs/tracing/pull/1539
[#1544]: https://github.com/tokio-rs/tracing/pull/1544
[#1550]: https://github.com/tokio-rs/tracing/pull/1550
[#1553]: https://github.com/tokio-rs/tracing/pull/1553

# 0.2.20 (August 17, 2021)

### Fixed

- **fmt**: Fixed `fmt` printing only the first `source` for errors with a chain
  of sources ([#1460])
- **fmt**: Fixed missing space between level and event in the `Pretty` formatter
  ([#1498])
- **json**: Fixed `Json` formatter not honoring `without_time` and `with_level`
  configurations ([#1463])

### Added

- **registry**: Improved panic message when cloning a span whose ID doesn't
  exist, to aid in debugging issues with multiple subscribers ([#1483])
- **registry**: Improved documentation on span ID generation ([#1453])

[#1460]: https://github.com/tokio-rs/tracing/pull/1460
[#1483]: https://github.com/tokio-rs/tracing/pull/1483
[#1463]: https://github.com/tokio-rs/tracing/pull/1463
[#1453]: https://github.com/tokio-rs/tracing/pull/1453
[#1498]: https://github.com/tokio-rs/tracing/pull/1498

Thanks to new contributors @joshtriplett and @lerouxrgd, and returning
contributor @teozkr, for contributing to this release!

# 0.2.19 (June 25, 2021)

### Deprecated

- **registry**: `SpanRef::parents`, `SpanRef::from_root`, and `Context::scope`
  iterators, which are replaced by new `SpanRef::scope` and `Scope::from_root`
  iterators ([#1413])

### Added

- **registry**: `SpanRef::scope` method, which returns a leaf-to-root `Iterator`
  including the leaf span ([#1413])
- **registry**: `Scope::from_root` method, which reverses the `scope` iterator
  to iterate root-to-leaf ([#1413])
- **registry**: `Context::event_span` method, which looks up the parent span of
  an event ([#1434])
- **registry**: `Context::event_scope` method, returning a `Scope` iterator over
  the span scope of an event ([#1434])
- **fmt**: `MakeWriter::make_writer_for` method, which allows returning a
  different writer based on a span or event's metadata ([#1141])
- **fmt**: `MakeWriterExt` trait, with `with_max_level`, `with_min_level`,
  `with_filter`, `and`, and `or_else` combinators ([#1274])
- **fmt**: `MakeWriter` implementation for `Arc<W> where &W: io::Write`
  ([#1274])

Thanks to @teozkr and @Folyd for contributing to this release!

[#1413]: https://github.com/tokio-rs/tracing/pull/1413
[#1434]: https://github.com/tokio-rs/tracing/pull/1434
[#1141]: https://github.com/tokio-rs/tracing/pull/1141
[#1274]: https://github.com/tokio-rs/tracing/pull/1274

# 0.2.18 (April 30, 2021)

### Deprecated

- Deprecated the `CurrentSpan` type, which is inefficient and largely superseded
  by the `registry` API ([#1321])

### Fixed

- **json**: Invalid JSON emitted for events in spans with no fields ([#1333])
- **json**: Missing span data for synthesized new span, exit, and close events
  ([#1334])
- **fmt**: Extra space before log lines when timestamps are disabled ([#1355])

### Added

- **env-filter**: Support for filters on spans whose names contain any
  characters other than `{` and `]` ([#1368])

Thanks to @Folyd, and new contributors @akinnane and @aym-v for contributing to
this release!

[#1321]: https://github.com/tokio-rs/tracing/pull/1321
[#1333]: https://github.com/tokio-rs/tracing/pull/1333
[#1334]: https://github.com/tokio-rs/tracing/pull/1334
[#1355]: https://github.com/tokio-rs/tracing/pull/1355
[#1368]: https://github.com/tokio-rs/tracing/pull/1368

# 0.2.17 (March 12, 2021)

### Fixed

- **fmt**: `Pretty` formatter now honors `with_ansi(false)` to disable ANSI
  terminal formatting ([#1240])
- **fmt**: Fixed extra padding when using `Pretty` formatter ([#1275])
- **chrono**: Removed extra trailing space with `ChronoLocal` time formatter
  ([#1103])

### Added

- **fmt**: Added `FmtContext::current_span()` method, returning the current span
  ([#1290])
- **fmt**: `FmtSpan` variants may now be combined using the `|` operator for
  more granular control over what span events are generated ([#1277])

Thanks to new contributors @cratelyn, @dignati, and @zicklag, as well as @Folyd,
@matklad, and @najamelan, for contributing to this release!

[#1240]: https://github.com/tokio-rs/tracing/pull/1240
[#1275]: https://github.com/tokio-rs/tracing/pull/1275
[#1103]: https://github.com/tokio-rs/tracing/pull/1103
[#1290]: https://github.com/tokio-rs/tracing/pull/1290
[#1277]: https://github.com/tokio-rs/tracing/pull/1277

# 0.2.16 (February 19, 2021)

### Fixed

- **env-filter**: Fixed directives where the level is in mixed case (such as
  `Info`) failing to parse ([#1126])
- **fmt**: Fixed `fmt::Subscriber` not providing a max-level hint ([#1251])
- `tracing-subscriber` no longer enables `tracing` and `tracing-core`'s default
  features ([#1144])
  
### Changed

- **chrono**: Updated `chrono` dependency to 0.4.16 ([#1189])
- **log**: Updated `tracing-log` dependency to 0.1.2

Thanks to @salewski, @taiki-e, @davidpdrsn and @markdingram for contributing to
this release!

[#1126]: https://github.com/tokio-rs/tracing/pull/1126
[#1251]: https://github.com/tokio-rs/tracing/pull/1251
[#1144]: https://github.com/tokio-rs/tracing/pull/1144
[#1189]: https://github.com/tokio-rs/tracing/pull/1189

# 0.2.15 (November 2, 2020)

### Fixed

- **fmt**: Fixed wrong lifetime parameters on `FormatFields` impl for
  `FmtContext` ([#1082])

### Added

- **fmt**: `format::Pretty`, an aesthetically pleasing, human-readable event
  formatter for local development and user-facing CLIs ([#1080])
- **fmt**: `FmtContext::field_format`, which returns the subscriber's field
  formatter ([#1082])

[#1082]: https://github.com/tokio-rs/tracing/pull/1082
[#1080]: https://github.com/tokio-rs/tracing/pull/1080

# 0.2.14 (October 22, 2020)

### Fixed

- **registry**: Fixed `Registry::new` allocating an excessively large amount of
  memory, most of which would never be used ([#1064])

### Changed

- **registry**: Improved `new_span` performance by reusing `HashMap` allocations
  for `Extensions` ([#1064])
- **registry**: Significantly improved the performance of `Registry::enter` and
  `Registry::exit` ([#1058])

[#1064]: https://github.com/tokio-rs/tracing/pull/1064
[#1058]: https://github.com/tokio-rs/tracing/pull/1058

# 0.2.13 (October 7, 2020)

### Changed

- Updated `tracing-core` to 0.1.17 ([#992])

### Added

- **env-filter**: Added support for filtering on targets which contain dashes
  ([#1014])
- **env-filter**: Added a warning when creating an `EnvFilter` that contains
  directives that would enable a level disabled by the `tracing` crate's
  `static_max_level` features ([#1021])

Thanks to @jyn514 and @bkchr for contributing to this release!

[#992]: https://github.com/tokio-rs/tracing/pull/992
[#1014]: https://github.com/tokio-rs/tracing/pull/1014
[#1021]: https://github.com/tokio-rs/tracing/pull/1021

# 0.2.12 (September 11, 2020)

### Fixed

- **env-filter**: Fixed a regression where `Option<Level>` lost its
  `Into<LevelFilter>` impl ([#966])
- **env-filter**: Fixed `EnvFilter` enabling spans that should not be enabled
  when multiple subscribers are in use ([#927])

### Changed

- **json**: `format::Json` now outputs fields in a more readable order ([#892])
- Updated `tracing-core` dependency to 0.1.16

### Added

- **fmt**: Add `BoxMakeWriter` for erasing the type of a `MakeWriter`
  implementation ([#958])
- **fmt**: Add `TestWriter` `MakeWriter` implementation to support libtest
  output capturing ([#938])
- **layer**: Add `Layer` impl for `Option<T> where T: Layer` ([#910])
- **env-filter**: Add `From<Level>` impl for `Directive` ([#918])
- Multiple documentation fixes and improvements

Thanks to @Pothulapati, @samrg472, @bryanburgers, @keetonian, and @SriRamanujam
for contributing to this release!

[#927]: https://github.com/tokio-rs/tracing/pull/927
[#966]: https://github.com/tokio-rs/tracing/pull/966
[#958]: https://github.com/tokio-rs/tracing/pull/958
[#892]: https://github.com/tokio-rs/tracing/pull/892
[#938]: https://github.com/tokio-rs/tracing/pull/938
[#910]: https://github.com/tokio-rs/tracing/pull/910
[#918]: https://github.com/tokio-rs/tracing/pull/918

# 0.2.11 (August 10, 2020)

### Fixed

- **env-filter**: Incorrect max level hint when filters involving span field
  values are in use (#907)
- **registry**: Fixed inconsistent span stacks when multiple registries are in
  use on the same thread (#901)

### Changed

- **env-filter**: `regex` dependency enables fewer unused feature flags (#899)

Thanks to @bdonlan and @jeromegn for contributing to this release!

# 0.2.10 (July 31, 2020)

### Fixed

- **docs**: Incorrect formatting (#862)

### Changed

- **filter**: `LevelFilter` is now a re-export of the
  `tracing_core::LevelFilter` type, it can now be used interchangeably with the
  versions in `tracing` and `tracing-core` (#853)
- **filter**: Significant performance improvements when comparing `LevelFilter`s
  and `Level`s (#853)
- Updated the minimum `tracing-core` dependency to 0.1.12 (#853)

### Added

- **filter**: `LevelFilter` and `EnvFilter` now participate in `tracing-core`'s
  max level hinting, improving performance significantly in some use cases where
  levels are disabled globally (#853)

# 0.2.9 (July 23, 2020)

### Fixed

- **fmt**: Fixed compilation failure on MSRV when the `chrono` feature is
  disabled (#844)

### Added

- **fmt**: Span lookup methods defined by `layer::Context` are now also provided
  by `FmtContext` (#834)

# 0.2.8 (July 17, 2020)

### Changed

- **fmt**: When the `chrono` dependency is enabled, the `SystemTime` timestamp
  formatter now emits human-readable timestamps rather than using `SystemTime`'s
  `fmt::Debug`implementation (`chrono` is still required for customized
  timestamp formatting) (#807)
- **ansi**: Updated `ansi_term` dependency to 0.12 (#816)

### Added

- **json**: `with_span_list` method to configure the JSON formatter to include a
  list of all spans in the current trace in formatting events (similarly to the
  text formatter) (#741) 
- **json**: `with_current_span` method to configure the JSON formatter to include
  a field for the _current_ span (the leaf of the trace) in formatted events
  (#741)
- **fmt**: `with_thread_names` and `with_thread_ids` methods to configure
  `fmt::Subscriber`s and `fmt::Layer`s to include the thread name and/or thread ID
  of the current thread when formatting events (#818)
  
Thanks to new contributors @mockersf, @keetonian, and @Pothulapati for
contributing to this release!

# 0.2.7 (July 1, 2020)

### Changed

- **parking_lot**: Updated the optional `parking_lot` dependency to accept the
  latest `parking_lot` version (#774)

### Fixed

- **fmt**: Fixed events with explicitly overridden parent spans being formatted
  as though they were children of the current span (#767)

### Added

- **fmt**: Added the option to print synthesized events when spans are created,
  entered, exited, and closed, including span durations (#761)
- Documentation clarification and improvement (#762, #769)

Thanks to @rkuhn, @greenwoodcm, and @Ralith for contributing to this release!

# 0.2.6 (June 19, 2020)

### Fixed

- **fmt**: Fixed an issue in the JSON formatter where using `Span::record` would
  result in malformed spans (#709)

# 0.2.5 (April 21, 2020)

### Changed

- **fmt**: Bump sharded-slab dependency (#679)

### Fixed

- **fmt**: remove trailing space in `ChronoUtc` `format_time` (#677)

# 0.2.4 (April 6, 2020)

This release includes several API ergonomics improvements, including shorthand
constructors for many types, and an extension trait for initializing subscribers
using method-chaining style. Additionally, several bugs in less commonly used
`fmt` APIs were fixed.

### Added

- **fmt**: Shorthand free functions for constructing most types in `fmt`
  (including `tracing_subscriber::fmt()` to return a `SubscriberBuilder`,
  `tracing_subscriber::fmt::layer()` to return a format `Layer`, etc) (#660)
- **registry**: Shorthand free function `tracing_subscriber::registry()` to
  construct a new registry (#660)
- Added `SubscriberInitExt` extension trait for more ergonomic subscriber
  initialization (#660)
  
### Changed

- **fmt**: Moved `LayerBuilder` methods to `Layer` (#655)

### Deprecated

- **fmt**: `LayerBuilder`, as `Layer` now implements all builder methods (#655)
  
### Fixed

- **fmt**: Fixed `Compact` formatter not omitting levels with
  `with_level(false)` (#657)
- **fmt**: Fixed `fmt::Layer` duplicating the fields for a new span if another
  layer has already formatted its fields (#634)
- **fmt**: Added missing space when using `record` to add new fields to a span
  that already has fields (#659)
- Updated outdated documentation (#647)


# 0.2.3 (March 5, 2020)

### Fixed

- **env-filter**: Regression where filter directives were selected in the order
  they were listed, rather than most specific first (#624)

# 0.2.2 (February 27, 2020)

### Added

- **fmt**: Added `flatten_event` to `SubscriberBuilder` (#599)
- **fmt**: Added `with_level` to `SubscriberBuilder` (#594)

# 0.2.1 (February 13, 2020)

### Changed

- **filter**: `EnvFilter` directive selection now behaves correctly (i.e. like
  `env_logger`) (#583)

### Fixed

- **filter**: Fixed `EnvFilter` incorrectly allowing less-specific filter
  directives to enable events that are disabled by more-specific filters (#583)
- **filter**: Multiple significant `EnvFilter` performance improvements,
  especially when filtering events generated by `log` records (#578, #583)
- **filter**: Replaced `BTreeMap` with `Vec` in `DirectiveSet`, improving
  iteration performance significantly with typical numbers of filter directives
  (#580)

A big thank-you to @samschlegel for lots of help with `EnvFilter` performance
tuning in this release!

# 0.2.0 (February 4, 2020)

### Breaking Changes

- **fmt**: Renamed `Context` to `FmtContext` (#420, #425)
- **fmt**: Renamed `Builder` to `SubscriberBuilder` (#420)
- **filter**: Removed `Filter`. Use `EnvFilter` instead (#434)

### Added

- **registry**: `Registry`, a `Subscriber` implementation that `Layer`s can use
  as a high-performance, in-memory span store. (#420, #425, #432, #433, #435)
- **registry**: Added `LookupSpan` trait, implemented by `Subscriber`s to expose
  stored span data to `Layer`s (#420)
- **fmt**: Added `fmt::Layer`, to allow composing log formatting with other `Layer`s
- **fmt**: Added support for JSON field and event formatting (#377, #415)
- **filter**: Documentation for filtering directives (#554)

### Changed

- **fmt**: Renamed `Context` to `FmtContext` (#420, #425) (BREAKING)
- **fmt**: Renamed `Builder` to `SubscriberBuilder` (#420) (BREAKING)
- **fmt**: Reimplemented `fmt::Subscriber` in terms of the `Registry`
  and `Layer`s (#420)

### Removed

- **filter**: Removed `Filter`. Use `EnvFilter` instead (#434) (BREAKING)

### Fixed

- **fmt**: Fixed memory leaks in the slab used to store per-span data
  (3c35048)
- **fmt**: `fmt::SubscriberBuilder::init` not setting up `log` compatibility
  (#489)
- **fmt**: Spans closed by a child span closing not also closing _their_
  parents (#514)
- **Layer**: Fixed `Layered` subscribers failing to downcast to their own type
  (#549)
- **Layer**: Fixed `Layer::downcast_ref` returning invalid references (#454)

# 0.2.0-alpha.6 (February 3, 2020)

### Fixed

- **fmt**: Fixed empty `{}` printed after spans with no fields (f079f2d)
- **fmt**: Fixed inconsistent formatting when ANSI colors are disabled (506a482)
- **fmt**: Fixed misaligned levels when ANSI colors are disabled (eba1adb)
- Fixed warnings on nightly Rust compilers (#558)

# 0.2.0-alpha.5 (January 31, 2020)

### Added

- **env_filter**: Documentation for filtering directives (#554)
- **registry**, **env_filter**: Updated `smallvec` dependency to 0.1 (#543)

### Fixed

- **registry**: Fixed a memory leak in the slab used to store per-span data
  (3c35048)
- **Layer**: Fixed `Layered` subscribers failing to downcast to their own type
  (#549)
- **fmt**: Fixed a panic when multiple layers insert `FormattedFields`
  extensions from the same formatter type (1c3bb70)
- **fmt**: Fixed `fmt::Layer::on_record` inserting a new `FormattedFields` when
  formatted fields for a span already exist (1c3bb70)

# 0.2.0-alpha.4 (January 11, 2020)

### Fixed

- **registry**: Removed inadvertently committed `dbg!` macros (#533)

# 0.2.0-alpha.3 (January 10, 2020)

### Added

- **fmt**: Public `FormattedFields::new` constructor (#478)
- **fmt**: Added examples to `fmt::Layer` documentation (#510)
- Documentation now shows what feature flags are required by each API item (#525)

### Fixed

- **fmt**: Missing space between timestamp and level (#480)
- **fmt**: Incorrect formatting with `with_target(false)` (#481)
- **fmt**: `fmt::SubscriberBuilder::init` not setting up `log` compatibility
  (#489)
- **registry**: Spans exited out of order not being closed properly on exit
  (#509)
- **registry**: Memory leak when spans are closed by a child span closing (#514)
- **registry**: Spans closed by a child span closing not also closing _their_
  parents (#514)
- Compilation errors with `no-default-features` (#499, #500)

# 0.2.0-alpha.2 (December 8, 2019)

### Added

- `LookupSpans` implementation for `Layered` (#448)
- `SpanRef::from_root` to iterate over a span's parents from the root (#460)
- `Context::scope`, to iterate over the current context from the root (#460)
- `Context::lookup_current`, which returns a `SpanRef` to the current
  span's data (#460)

### Changed

- Lifetimes on some new `Context` methods to be less restrictive (#460)

### Fixed

- `Layer::downcast_ref` returning invalid references (#454)
- Compilation failure on 32-bit platforms (#462)
- Compilation failure with ANSI formatters (#438)

# 0.2.0-alpha.1 (November 18, 2019)

### Added

- `Registry`, a reusable span store that `Layer`s can use a
  high-performance, in-memory store. (#420, #425, #432, #433, #435)
- Reimplemented `fmt::Subscriber` in terms of the `Registry`
  and `Layer`s (#420)
- Add benchmarks for fmt subscriber (#421)
- Add support for JSON field and event formatting (#377, #415)

### Changed

- **BREAKING**: Change `fmt::format::FormatFields` and
  `fmt::format::FormatEvent` to accept a mandatory `FmtContext`. These
  `FormatFields` and `FormatEvent` will likely see additional breaking
  changes in subsequent alpha. (#420, #425)
- **BREAKING**: Removed `Filter`. Use `EnvFilter` instead (#434)

### Contributors

Thanks to all the contributors to this release!

- @pimeys for #377 and #415

# 0.1.6 (October 29, 2019)

### Added

- Add `init` and `try_init` functions to `FmtSubscriber` (#385)
- Add `ChronoUtc` and `ChronoLocal` timers, RFC 3339 support (#387)
- Add `tracing::subscriber::set_default` which sets the default
  subscriber and returns a drop guard. This drop guard will reset the
  dispatch on drop (#388).

### Fixed

- Fix default level for `EnvFilter`. Setting `RUST_LOG=target`
  previously only the `ERROR` level, while it should enable everything.
  `tracing-subscriber` now defaults to `TRACE` if no level is specified
  (#401)
- Fix `tracing-log` feature flag for init + try_init. The feature flag
  `tracing_log` was used instead of the correct `tracing-log`. As a
  result, both `tracing-log` and `tracing_log` needed to be specified in
  order to initialize the global logger. Only `tracing-log` needs to be
  specified now (#400).

### Contributors

Thanks to all the contributors to this release!

- @emschwartz for #385, #387, #400 and #401
- @bIgBV for #388

# 0.1.5 (October 7, 2019)

### Fixed

- Spans not being closed properly when `FmtSubscriber::current_span` is used
  (#371)

# 0.1.4 (September 26, 2019)

### Fixed

- Spans entered twice on the same thread sometimes being completely exited when
  the more deeply-nested entry is exited (#361)
- Setting `with_ansi(false)` on `FmtSubscriber` not disabling ANSI color
  formatting for timestamps (#354)
- Incorrect reference counting in `FmtSubscriber` that could cause spans to not
  be closed when all references are dropped (#366)

# 0.1.3 (September 16, 2019)

### Fixed

- `Layered` subscribers not properly forwarding calls to `current_span`
  (#350)

# 0.1.2 (September 12, 2019)

### Fixed

- `EnvFilter` ignoring directives with targets that are the same number of
  characters (#333)
- `EnvFilter` failing to properly apply filter directives to events generated
  from `log` records by`tracing-log` (#344)

### Changed

- Renamed `Filter` to `EnvFilter`, deprecated `Filter` (#339)
- Renamed "filter" feature flag to "env-filter", deprecated "filter" (#339)
- `FmtSubscriber` now defaults to enabling only the `INFO` level and above when
  a max level filter or `EnvFilter` is not set (#336)
- Made `parking_lot` dependency an opt-in feature flag (#348)

### Added

- `EnvFilter::add_directive` to add new directives to filters after they are
  constructed (#334)
- `fmt::Builder::with_max_level` to set a global level filter for a
  `FmtSubscriber` without requiring the use of `EnvFilter` (#336)
- `Layer` implementation for `LevelFilter` (#336)
- `EnvFilter` now implements `fmt::Display` (#329)

### Removed

- Removed dependency on `crossbeam-util` (#348)

# 0.1.1 (September 4, 2019)

### Fixed

- Potential double panic in `CurrentSpan` (#325)

# 0.1.0 (September 3, 2019)

- Initial release
