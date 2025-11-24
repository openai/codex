# 2.5.0 (Jan 8th, 2025)

- macros: suppress `clippy::needless_return` in `#[tokio::main]` ([#6874])

[#6874]: https://github.com/tokio-rs/tokio/pull/6874

# 2.4.0 (July 22nd, 2024)

- msrv: increase MSRV to 1.70 ([#6645])
- macros: allow `unhandled_panic` behavior for `#[tokio::main]` and `#[tokio::test]` ([#6593])

[#6593]: https://github.com/tokio-rs/tokio/pull/6593
[#6645]: https://github.com/tokio-rs/tokio/pull/6645

# 2.3.0 (May 30th, 2024)

- macros: make `#[tokio::test]` append `#[test]` at the end of the attribute list ([#6497])

[#6497]: https://github.com/tokio-rs/tokio/pull/6497

# 2.2.0 (November 19th, 2023)

### Changed

- use `::core` qualified imports instead of `::std` inside `tokio::test` macro ([#5973])

[#5973]: https://github.com/tokio-rs/tokio/pull/5973

# 2.1.0 (April 25th, 2023)

- macros: fix typo in `#[tokio::test]` docs ([#5636])
- macros: make entrypoints more efficient ([#5621])

[#5621]: https://github.com/tokio-rs/tokio/pull/5621
[#5636]: https://github.com/tokio-rs/tokio/pull/5636

# 2.0.0 (March 24th, 2023)

This major release updates the dependency on the syn crate to 2.0.0, and
increases the MSRV to 1.56.

As part of this release, we are adopting a policy of depending on a specific minor
release of tokio-macros. This prevents Tokio from being able to pull in many different
versions of tokio-macros.

- macros: update `syn` ([#5572])
- macros: accept path as crate rename ([#5557])

[#5572]: https://github.com/tokio-rs/tokio/pull/5572
[#5557]: https://github.com/tokio-rs/tokio/pull/5557

# 1.8.2 (November 30th, 2022)

- fix a regression introduced in 1.8.1 ([#5244])

[#5244]: https://github.com/tokio-rs/tokio/pull/5244

# 1.8.1 (November 29th, 2022)

(yanked)

- macros: Pin Futures in `#[tokio::test]` to stack ([#5205])
- macros: Reduce usage of last statement spans in proc-macros ([#5092])
- macros: Improve the documentation for `#[tokio::test]` ([#4761])

[#5205]: https://github.com/tokio-rs/tokio/pull/5205
[#5092]: https://github.com/tokio-rs/tokio/pull/5092
[#4761]: https://github.com/tokio-rs/tokio/pull/4761

# 1.8.0 (June 4th, 2022)

- macros: always emit return statement ([#4636])
- macros: support setting a custom crate name for `#[tokio::main]` and `#[tokio::test]` ([#4613])

[#4613]: https://github.com/tokio-rs/tokio/pull/4613
[#4636]: https://github.com/tokio-rs/tokio/pull/4636

# 1.7.0 (December 15th, 2021)

- macros: address remaining `clippy::semicolon_if_nothing_returned` warning ([#4252])

[#4252]: https://github.com/tokio-rs/tokio/pull/4252

# 1.6.0 (November 16th, 2021)

- macros: fix mut patterns in `select!` macro ([#4211])

[#4211]: https://github.com/tokio-rs/tokio/pull/4211

# 1.5.1 (October 29th, 2021)

- macros: fix type resolution error in `#[tokio::main]` ([#4176])

[#4176]: https://github.com/tokio-rs/tokio/pull/4176

# 1.5.0 (October 13th, 2021)

- macros: make tokio-macros attributes more IDE friendly ([#4162])

[#4162]: https://github.com/tokio-rs/tokio/pull/4162

# 1.4.1 (September 30th, 2021)

Reverted: run `current_thread` inside `LocalSet` ([#4027])

# 1.4.0 (September 29th, 2021)

(yanked)

### Changed

- macros: run `current_thread` inside `LocalSet` ([#4027])
- macros: explicitly relaxed clippy lint for `.expect()` in runtime entry macro ([#4030])

### Fixed

- macros: fix invalid error messages in functions wrapped with `#[main]` or `#[test]` ([#4067])

[#4027]: https://github.com/tokio-rs/tokio/pull/4027
[#4030]: https://github.com/tokio-rs/tokio/pull/4030
[#4067]: https://github.com/tokio-rs/tokio/pull/4067

# 1.3.0 (July 7, 2021)

- macros: don't trigger `clippy::unwrap_used` ([#3926])

[#3926]: https://github.com/tokio-rs/tokio/pull/3926

# 1.2.0 (May 14, 2021)

- macros: forward input arguments in `#[tokio::test]` ([#3691])
- macros: improve diagnostics on type mismatch ([#3766])
- macros: various error message improvements ([#3677])

[#3677]: https://github.com/tokio-rs/tokio/pull/3677
[#3691]: https://github.com/tokio-rs/tokio/pull/3691
[#3766]: https://github.com/tokio-rs/tokio/pull/3766

# 1.1.0 (February 5, 2021)

- add `start_paused` option to macros ([#3492])

# 1.0.0 (December 23, 2020)

- track `tokio` 1.0 release.

# 0.3.1 (October 25, 2020)

### Fixed

- fix incorrect docs regarding `max_threads` option ([#3038])

# 0.3.0 (October 15, 2020)

- Track `tokio` 0.3 release.

### Changed
- options are renamed to track `tokio` runtime builder fn names.
- `#[tokio::main]` macro requires `rt-multi-thread` when no `flavor` is specified.

# 0.2.5 (February 27, 2019)

### Fixed
- doc improvements ([#2225]).

# 0.2.4 (January 27, 2019)

### Fixed
- generics on `#[tokio::main]` function ([#2177]).

### Added
- support for `tokio::select!` ([#2152]).

# 0.2.3 (January 7, 2019)

### Fixed
- Revert breaking change.

# 0.2.2 (January 7, 2019)

### Added
- General refactoring and inclusion of additional runtime options ([#2022] and [#2038])

# 0.2.1 (December 18, 2019)

### Fixes
- inherit visibility when wrapping async fn ([#1954]).

# 0.2.0 (November 26, 2019)

- Initial release

[#1954]: https://github.com/tokio-rs/tokio/pull/1954
[#2022]: https://github.com/tokio-rs/tokio/pull/2022
[#2038]: https://github.com/tokio-rs/tokio/pull/2038
[#2152]: https://github.com/tokio-rs/tokio/pull/2152
[#2177]: https://github.com/tokio-rs/tokio/pull/2177
[#2225]: https://github.com/tokio-rs/tokio/pull/2225
[#3038]: https://github.com/tokio-rs/tokio/pull/3038
[#3492]: https://github.com/tokio-rs/tokio/pull/3492
