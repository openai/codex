# 0.2.1 (November 29, 2024)

[ [crates.io][crate-0.2.1] ] | [ [docs.rs][docs-0.2.1] ]

### Changed

- Bump MSRV to 1.63 ([#2793])

### Documented

- Use intra-doc links instead of relative file paths ([#2068])
- More intra-doc links ([#2077])
- Add missing backtick to `prelude` docs ([#2120])

[#2068]: https://github.com/tokio-rs/tracing/pull/2068
[#2077]: https://github.com/tokio-rs/tracing/pull/2077
[#2120]: https://github.com/tokio-rs/tracing/pull/2120
[#2793]: https://github.com/tokio-rs/tracing/pull/2793
[docs-0.2.1]: https://docs.rs/tracing-error/0.2.1/tracing_error/
[crate-0.2.1]: https://crates.io/crates/tracing-error/0.2.1

# 0.2.0 (October 23, 2021)

This is a breaking change release in order to update the `tracing-subscriber`
dependency version to [the v0.3.x release series][v03].

### Changed

- Updated `tracing-subscriber` dependency to [v0.3.0][v03] ([#1677])

### Fixed

- Disabled default features of the `tracing` dependency so that proc-macro
  dependencies are not enabled ([#1144])
- Documentation fixes and improvements ([#635], [#695])

### Added

- **SpanTrace**: Added `SpanTrace::new` constructor for constructing a
  `SpanTrace` from a `Span` passed as an argument (rather than capturing the
  current span) ([#1492])

Thanks to @CAD97 for contributing to this release!

[v03]: https://github.com/tokio-rs/tracing/releases/tag/tracing-subscriber-0.3.0
[#635]: https://github.com/tokio-rs/tracing/pull/635
[#695]: https://github.com/tokio-rs/tracing/pull/695
[#1144]: https://github.com/tokio-rs/tracing/pull/1144
[#1492]: https://github.com/tokio-rs/tracing/pull/1492
[#1677]: https://github.com/tokio-rs/tracing/pull/1677

# 0.1.2 (March 3, 2020)

### Added

- **TracedError**: `TracedError`, an error type wrapper that annotates an error
  with the current span.
- **SpanTrace**:`SpanTrace::status` method and `SpanTraceStatus` type for
  determining whether a `SpanTrace` was successfully captured (#614)

### Changed

- **SpanTrace**: Made backtrace formatting more consistent with upstream changes
  to `std::backtrace` (#584)

# 0.1.1 (February 5, 2020)

### Fixed

- Fixed a typo in the crate description

### Changed

- the maintenance badge from active to experimental

# 0.1.0 (February 5, 2020)

- Initial release
