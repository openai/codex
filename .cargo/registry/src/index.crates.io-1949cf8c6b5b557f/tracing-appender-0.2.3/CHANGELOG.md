# 0.2.3 (November 13, 2023)

This release contains several new features. It also increases the
minimum supported Rust version (MSRV) to Rust 1.63.0.

### Added

- **rolling**: add option to automatically delete old log files ([#2323])
- **non_blocking**: allow worker thread name to be configured ([#2365])
- **rolling**: add a builder for constructing `RollingFileAppender`s ([#2227])
- **rolling**: add `Builder::filename_suffix` parameter ([#2225])
- **non_blocking**: remove `Sync` bound from writer for `NonBlocking` ([#2607]) 
- **non_blocking**: name spawned threads ([#2219])

### Fixed

- Fixed several documentation typos and issues ([#2689], [#2375])

### Changed

- Increased minimum supported Rust version (MSRV) to 1.63.0+ ([#2793])
- Updated minimum `tracing-subscriber` version to [0.3.18][subscriber-v0.3.18] ([#2790])

[subscriber-v0.3.18]: https://github.com/tokio-rs/tracing/releases/tag/tracing-subscriber-0.3.18
[#2323]: https://github.com/tokio-rs/tracing/pull/2323
[#2365]: https://github.com/tokio-rs/tracing/pull/2365
[#2227]: https://github.com/tokio-rs/tracing/pull/2227
[#2225]: https://github.com/tokio-rs/tracing/pull/2225
[#2607]: https://github.com/tokio-rs/tracing/pull/2607
[#2219]: https://github.com/tokio-rs/tracing/pull/2219
[#2689]: https://github.com/tokio-rs/tracing/pull/2689
[#2375]: https://github.com/tokio-rs/tracing/pull/2375
[#2793]: https://github.com/tokio-rs/tracing/pull/2793
[#2790]: https://github.com/tokio-rs/tracing/pull/2790

# 0.2.2 (March 17, 2022)

This release fixes a bug in `RollingFileAppender` that could result
in a failure to rotate the log file, or in panics in debug mode.

### Fixed

- **rolling**: Fixed a panic that prohibited rolling files over. ([#1989])

[#1989]: https://github.com/tokio-rs/tracing/pull/1989

# 0.2.1 (February 28, 2022)

This release adds an implementation of the `MakeWriter` trait for
`RollingFileAppender`, allowing it to be used without wrapping in a
`NonBlocking` writer.

This release increases the minimum supported Rust version to 1.53.0.

### Added

- **rolling**: Added `MakeWriter` implementation for `RollingFileAppender`
  ([#1760])

### Changed

- Updated minimum supported Rust version (MSRV) to 1.53.0 ([#1851])
- `parking_lot`: updated to v0.12 ([#1878])

### Fixed

- Fixed several documentation typos and issues ([#1780], [d868054], [#1943])

[#1760]: https://github.com/tokio-rs/tracing/pull/1760
[#1851]: https://github.com/tokio-rs/tracing/pull/1851
[#1878]: https://github.com/tokio-rs/tracing/pull/1878
[#1943]: https://github.com/tokio-rs/tracing/pull/1943
[d868054]: https://github.com/tokio-rs/tracing/commit/d8680547b509978c7113c8f7e19e9b00c789c698

# 0.2.0 (October 22, 2021)

This breaking change release adds support for the new v0.3.x series of
`tracing-subscriber`. In addition, it resolves the security advisory for the
`chrono` crate, [RUSTSEC-2020-0159].

This release increases the minimum supported Rust version to 1.51.0.
### Breaking Changes

- Updated `tracing-subscriber` to v0.3.x ([#1677])
- Changed `NonBlocking::error_counter` to return an `ErrorCounter` type, rather
  than an `Arc<AtomicU64>` ([#1675])

### Changed

- Updated `tracing-subscriber` to v0.3.x ([#1677])

### Fixed

- **non-blocking**: Fixed compilation on 32-bit targets ([#1675])
- **rolling**: Replaced `chrono` dependency with `time` to resolve
  [RUSTSEC-2020-0159] ([#1652])
- **rolling**: Fixed an issue where `RollingFileAppender` would fail to print
  errors that occurred while flushing a previous logfile ([#1604])

Thanks to new contributors @dzvon and @zvkemp for contributing to this release!

[RUSTSEC-2020-0159]: https://rustsec.org/advisories/RUSTSEC-2020-0159.html
[#1677]: https://github.com/tokio-rs/tracing/pull/1677
[#1675]: https://github.com/tokio-rs/tracing/pull/1675
[#1652]: https://github.com/tokio-rs/tracing/pull/1675
[#1604]: https://github.com/tokio-rs/tracing/pull/1604

# 0.1.2 (December 28, 2020)

### Changed

- **non_blocking**: Updated `crossbeam-channel` dependency to 0.5 (#1031)

### Fixed

- **non_blocking**: Fixed a race condition when logging on shutdown (#1125)
- Several documentation improvements (#1109, #1110, #941, #953)

# 0.1.1 (July 20, 2020)

### Added

- **rolling**: `minutely` rotation schedule to rotate the log file once every
  minute (#748)

### Fixed

- Fixed broken links in docs (#718)
- `tracing-appender` now only enables the necessary `tracing-subscriber`'s
  feature flags, rather than all of them (#779) 

Thanks to new contributors @ericjheinz and @sourcefrog for contributing
to this release!

# 0.1.0 (May 5, 2020)

- Initial release