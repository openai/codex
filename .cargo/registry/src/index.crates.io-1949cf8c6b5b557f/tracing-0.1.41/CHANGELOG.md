# 0.1.41 (November 27, 2024)

[ [crates.io][crate-0.1.41] ] | [ [docs.rs][docs-0.1.41] ]

This release updates the `tracing-core` dependency to [v0.1.33][core-0.1.33] and
the `tracing-attributes` dependency to [v0.1.28][attrs-0.1.28].

### Added

- **core**: Add index API for `Field` ([#2820])
- **core**: Allow `&[u8]` to be recorded as event/span field ([#2954])

### Changed

- Bump MSRV to 1.63 ([#2793])
- **core**: Use const `thread_local`s when possible ([#2838])

### Fixed

- Removed core imports in macros ([#2762])
- **attributes**: Added missing RecordTypes for instrument ([#2781])
- **attributes**: Change order of async and unsafe modifier ([#2864])
- Fix missing field prefixes ([#2878])
- **attributes**: Extract match scrutinee ([#2880])
- Fix non-simple macro usage without message ([#2879])
- Fix event macros with constant field names in the first position ([#2883])
- Allow field path segments to be keywords ([#2925])
- **core**: Fix missed `register_callsite` error ([#2938])
- **attributes**: Support const values for `target` and `name` ([#2941])
- Prefix macro calls with ::core to avoid clashing with local macros ([#3024])

[#2762]: https://github.com/tokio-rs/tracing/pull/2762
[#2781]: https://github.com/tokio-rs/tracing/pull/2781
[#2793]: https://github.com/tokio-rs/tracing/pull/2793
[#2820]: https://github.com/tokio-rs/tracing/pull/2820
[#2838]: https://github.com/tokio-rs/tracing/pull/2838
[#2864]: https://github.com/tokio-rs/tracing/pull/2864
[#2878]: https://github.com/tokio-rs/tracing/pull/2878
[#2879]: https://github.com/tokio-rs/tracing/pull/2879
[#2880]: https://github.com/tokio-rs/tracing/pull/2880
[#2883]: https://github.com/tokio-rs/tracing/pull/2883
[#2925]: https://github.com/tokio-rs/tracing/pull/2925
[#2938]: https://github.com/tokio-rs/tracing/pull/2938
[#2941]: https://github.com/tokio-rs/tracing/pull/2941
[#2954]: https://github.com/tokio-rs/tracing/pull/2954
[#3024]: https://github.com/tokio-rs/tracing/pull/3024
[attrs-0.1.28]:
    https://github.com/tokio-rs/tracing/releases/tag/tracing-attributes-0.1.28
[core-0.1.33]:
    https://github.com/tokio-rs/tracing/releases/tag/tracing-core-0.1.33
[docs-0.1.41]: https://docs.rs/tracing/0.1.41/tracing/
[crate-0.1.41]: https://crates.io/crates/tracing/0.1.41

# 0.1.40 (October 19, 2023)

This release fixes a potential stack use-after-free in the
`Instrument::into_inner` method. Only uses of this method are affected by this
bug.

### Fixed

- Use `mem::ManuallyDrop` instead of `mem::forget` in `Instrument::into_inner`
  ([#2765])

[#2765]: https://github.com/tokio-rs/tracing/pull/2765

Thanks to @cramertj and @manishearth for finding and fixing this issue!

# 0.1.39 (October 12, 2023)

This release adds several additional features to the `tracing` macros. In
addition, it updates the `tracing-core` dependency to [v0.1.32][core-0.1.32] and
the `tracing-attributes` dependency to [v0.1.27][attrs-0.1.27].

### Added

- Allow constant field names in macros ([#2617])
- Allow setting event names in macros ([#2699])
- **core**: Allow `ValueSet`s of any length ([#2508])

### Changed

- `tracing-attributes`: updated to [0.1.27][attrs-0.1.27]
- `tracing-core`: updated to [0.1.32][core-0.1.32]
- **attributes**: Bump minimum version of proc-macro2 to 1.0.60 ([#2732])
-  **attributes**: Generate less dead code for async block return type hint ([#2709])

### Fixed

- Use fully qualified names in macros for items exported from std prelude
  ([#2621], [#2757])
- **attributes**: Allow [`clippy::let_with_type_underscore`] in macro-generated
  code ([#2609])
- **attributes**: Allow `unknown_lints` in macro-generated code ([#2626])
- **attributes**: Fix a compilation error in `#[instrument]` when the `"log"`
  feature is enabled ([#2599])

### Documented

- Add `axum-insights` to relevant crates. ([#2713])
- Fix link to RAI pattern crate documentation ([#2612])
- Fix docs typos and warnings ([#2581])
- Add `clippy-tracing` to related crates ([#2628])
- Add `tracing-cloudwatch` to related crates ([#2667])
- Fix deadlink to `tracing-etw` repo ([#2602])

[#2617]: https://github.com/tokio-rs/tracing/pull/2617
[#2699]: https://github.com/tokio-rs/tracing/pull/2699
[#2508]: https://github.com/tokio-rs/tracing/pull/2508
[#2621]: https://github.com/tokio-rs/tracing/pull/2621
[#2713]: https://github.com/tokio-rs/tracing/pull/2713
[#2581]: https://github.com/tokio-rs/tracing/pull/2581
[#2628]: https://github.com/tokio-rs/tracing/pull/2628
[#2667]: https://github.com/tokio-rs/tracing/pull/2667
[#2602]: https://github.com/tokio-rs/tracing/pull/2602
[#2626]: https://github.com/tokio-rs/tracing/pull/2626
[#2757]: https://github.com/tokio-rs/tracing/pull/2757
[#2732]: https://github.com/tokio-rs/tracing/pull/2732
[#2709]: https://github.com/tokio-rs/tracing/pull/2709
[#2599]: https://github.com/tokio-rs/tracing/pull/2599
[`let_with_type_underscore`]: http://rust-lang.github.io/rust-clippy/rust-1.70.0/index.html#let_with_type_underscore
[attrs-0.1.27]:
    https://github.com/tokio-rs/tracing/releases/tag/tracing-attributes-0.1.27
[core-0.1.32]:
    https://github.com/tokio-rs/tracing/releases/tag/tracing-core-0.1.32

# 0.1.38 (April 25th, 2023)

This `tracing` release changes the `Drop` implementation for `Instrumented`
`Future`s so that the attached `Span` is entered when dropping the `Future`. This
means that events emitted by the `Future`'s `Drop` implementation will now be
recorded within its `Span`. It also adds `#[inline]` hints to methods called in
the `event!` macro's expansion, for an improvement in both binary size and
performance.

Additionally, this release updates the `tracing-attributes` dependency to
[v0.1.24][attrs-0.1.24], which updates the [`syn`] dependency to v2.x.x.
`tracing-attributes` v0.1.24 also includes improvements to the `#[instrument]`
macro; see [the `tracing-attributes` 0.1.24 release notes][attrs-0.1.24] for
details.

### Added

- `Instrumented` futures will now enter the attached `Span` in their `Drop`
  implementation, allowing events emitted when dropping the future to occur
  within the span ([#2562])
- `#[inline]` attributes for methods called by the `event!` macros, making
  generated code smaller ([#2555])
- **attributes**: `level` argument to `#[instrument(err)]` and
  `#[instrument(ret)]` to override the level of
  the generated return value event ([#2335])
- **attributes**: Improved compiler error message when `#[instrument]` is added to a `const fn`
  ([#2418])

### Changed

- `tracing-attributes`: updated to [0.1.24][attrs-0.1.24]
- Removed unneeded `cfg-if` dependency ([#2553])
- **attributes**: Updated [`syn`] dependency to 2.0 ([#2516])

### Fixed

- **attributes**: Fix `clippy::unreachable` warnings in `#[instrument]`-generated code ([#2356])
- **attributes**: Removed unused "visit" feature flag from `syn` dependency ([#2530])

### Documented

- **attributes**: Documented default level for `#[instrument(err)]` ([#2433])
- **attributes**: Improved documentation for levels in `#[instrument]` ([#2350])

Thanks to @nitnelave, @jsgf, @Abhicodes-crypto, @LukeMathWalker, @andrewpollack,
@quad, @klensy, @davidpdrsn, @dbidwell94, @ldm0, @NobodyXu, @ilsv, and @daxpedda
for contributing to this release!

[`syn`]: https://crates.io/crates/syn
[attrs-0.1.24]:
    https://github.com/tokio-rs/tracing/releases/tag/tracing-attributes-0.1.24
[#2565]: https://github.com/tokio-rs/tracing/pull/2565
[#2555]: https://github.com/tokio-rs/tracing/pull/2555
[#2553]: https://github.com/tokio-rs/tracing/pull/2553
[#2335]: https://github.com/tokio-rs/tracing/pull/2335
[#2418]: https://github.com/tokio-rs/tracing/pull/2418
[#2516]: https://github.com/tokio-rs/tracing/pull/2516
[#2356]: https://github.com/tokio-rs/tracing/pull/2356
[#2530]: https://github.com/tokio-rs/tracing/pull/2530
[#2433]: https://github.com/tokio-rs/tracing/pull/2433
[#2350]: https://github.com/tokio-rs/tracing/pull/2350

# 0.1.37 (October 6, 2022)

This release of `tracing` incorporates changes from `tracing-core`
[v0.1.30][core-0.1.30] and `tracing-attributes` [v0.1.23][attrs-0.1.23],
including the new `Subscriber::on_register_dispatch` method for performing late
initialization after a `Subscriber` is registered as a `Dispatch`, and bugfixes
for the `#[instrument]` attribute. Additionally, it fixes instances of the
`bare_trait_objects` lint, which is now a warning on `tracing`'s MSRV and will
become an error in the next edition.

### Fixed

- **attributes**: Incorrect handling of inner attributes in `#[instrument]`ed
  functions ([#2307])
- **attributes**: Incorrect location of compiler diagnostic spans generated for
  type errors in `#[instrument]`ed `async fn`s ([#2270])
- **attributes**: Updated `syn` dependency to fix compilation with `-Z
  minimal-versions` ([#2246])
- `bare_trait_objects` warning in `valueset!` macro expansion ([#2308])

### Added

- **core**: `Subscriber::on_register_dispatch` method ([#2269])
- **core**: `WeakDispatch` type and `Dispatch::downgrade()` function ([#2293])

### Changed

- `tracing-core`: updated to [0.1.30][core-0.1.30]
- `tracing-attributes`: updated to [0.1.23][attrs-0.1.23]

### Documented

- Added [`tracing-web`] and [`reqwest-tracing`] to related crates ([#2283],
  [#2331])

Thanks to new contributors @compiler-errors, @e-nomem, @WorldSEnder, @Xiami2012,
and @tl-rodrigo-gryzinski, as well as @jswrenn and @CAD97, for contributing to
this release!

[core-0.1.30]: https://github.com/tokio-rs/tracing/releases/tag/tracing-core-0.1.30
[attrs-0.1.23]: https://github.com/tokio-rs/tracing/releases/tag/tracing-attributes-0.1.23
[`tracing-web`]: https://crates.io/crates/tracing-web/
[`reqwest-tracing`]: https://crates.io/crates/reqwest-tracing/
[#2246]: https://github.com/tokio-rs/tracing/pull/2246
[#2269]: https://github.com/tokio-rs/tracing/pull/2269
[#2283]: https://github.com/tokio-rs/tracing/pull/2283
[#2270]: https://github.com/tokio-rs/tracing/pull/2270
[#2293]: https://github.com/tokio-rs/tracing/pull/2293
[#2307]: https://github.com/tokio-rs/tracing/pull/2307
[#2308]: https://github.com/tokio-rs/tracing/pull/2308
[#2331]: https://github.com/tokio-rs/tracing/pull/2331

# 0.1.36 (July 29, 2022)

This release adds support for owned values and fat pointers as arguments to the
`Span::record` method, as well as updating the minimum `tracing-core` version
and several documentation improvements.

### Fixed

- Incorrect docs in `dispatcher::set_default` ([#2220])
- Compilation with `-Z minimal-versions` ([#2246])

### Added

- Support for owned values and fat pointers in `Span::record` ([#2212])
- Documentation improvements ([#2208], [#2163])

### Changed

- `tracing-core`: updated to [0.1.29][core-0.1.29]

Thanks to @fredr, @cgbur, @jyn514, @matklad, and @CAD97 for contributing to this
release!

[core-0.1.29]: https://github.com/tokio-rs/tracing/releases/tag/tracing-core-0.1.29
[#2220]: https://github.com/tokio-rs/tracing/pull/2220
[#2246]: https://github.com/tokio-rs/tracing/pull/2246
[#2212]: https://github.com/tokio-rs/tracing/pull/2212
[#2208]: https://github.com/tokio-rs/tracing/pull/2208
[#2163]: https://github.com/tokio-rs/tracing/pull/2163

# 0.1.35 (June 8, 2022)

This release reduces the overhead of callsite registration by using new
`tracing-core` APIs.

### Added

- Use `DefaultCallsite` to reduce callsite registration overhead ([#2083])

### Changed

- `tracing-core`: updated to [0.1.27][core-0.1.27]

[core-0.1.27]: https://github.com/tokio-rs/tracing/releases/tag/tracing-core-0.1.27
[#2088]: https://github.com/tokio-rs/tracing/pull/2083

# 0.1.34 (April 14, 2022)

This release includes bug fixes for the "log" support feature and for the use of
both scoped and global default dispatchers in the same program.

### Fixed

- Failure to use the global default dispatcher when a thread sets a local
  default dispatcher before the global default is set ([#2065])
- **log**: Compilation errors due to `async` block/fn futures becoming `!Send`
  when the "log" feature flag is enabled ([#2073])
- Broken links in documentation ([#2068])

Thanks to @ben0x539 for contributing to this release!

[#2065]: https://github.com/tokio-rs/tracing/pull/2065
[#2073]: https://github.com/tokio-rs/tracing/pull/2073
[#2068]: https://github.com/tokio-rs/tracing/pull/2068

# 0.1.33 (April 9, 2022)

This release adds new `span_enabled!` and `event_enabled!` variants of the
`enabled!` macro, for testing whether a subscriber would specifically enable a
span or an event.

### Added

- `span_enabled!` and `event_enabled!` macros ([#1900])
- Several documentation improvements ([#2010], [#2012])

### Fixed

- Compilation warning when compiling for <=32-bit targets (including `wasm32`)
  ([#2060])

Thanks to @guswynn, @arifd, @hrxi, @CAD97, and @name1e5s for contributing to
this release!

[#1900]: https://github.com/tokio-rs/tracing/pull/1900
[#2010]: https://github.com/tokio-rs/tracing/pull/2010
[#2012]: https://github.com/tokio-rs/tracing/pull/2012
[#2060]: https://github.com/tokio-rs/tracing/pull/2060

# 0.1.32 (March 8th, 2022)

This release reduces the overhead of creating and dropping disabled
spans significantly, which should improve performance when no `tracing`
subscriber is in use or when spans are disabled by a filter.

### Fixed

- **attributes**: Compilation failure with `--minimal-versions` due to a
  too-permissive `syn` dependency ([#1960])

### Changed

- Reduced `Drop` overhead for disabled spans ([#1974])
- `tracing-attributes`: updated to [0.1.20][attributes-0.1.20]

[#1974]: https://github.com/tokio-rs/tracing/pull/1974
[#1960]: https://github.com/tokio-rs/tracing/pull/1960
[attributes-0.1.20]: https://github.com/tokio-rs/tracing/releases/tag/tracing-attributes-0.1.20

# 0.1.31 (February 17th, 2022)

This release increases the minimum supported Rust version (MSRV) to 1.49.0. In
addition, it fixes some relatively rare macro bugs.

### Added

- Added `tracing-forest` to the list of related crates ([#1935])

### Changed

- Updated minimum supported Rust version (MSRV) to 1.49.0 ([#1913])

### Fixed

- Fixed the `warn!` macro incorrectly generating an event with the `TRACE` level
  ([#1930])
- Fixed macro hygiene issues when used in a crate that defines its own `concat!`
  macro, for real this time ([#1918])

Thanks to @QnnOkabayashi, @nicolaasg, and @teohhanhui for contributing to this
release!

[#1935]: https://github.com/tokio-rs/tracing/pull/1935
[#1913]: https://github.com/tokio-rs/tracing/pull/1913
[#1930]: https://github.com/tokio-rs/tracing/pull/1930
[#1918]: https://github.com/tokio-rs/tracing/pull/1918

# 0.1.30 (February 3rd, 2022)

This release adds *experimental* support for recording structured field
values using the [`valuable`] crate. See [this blog post][post] for
details on `valuable`.

Note that `valuable` support currently requires `--cfg tracing_unstable`. See
the documentation for details.

This release also adds a new `enabled!` macro for testing if a span or event
would be enabled.

### Added

- **field**: Experimental support for recording field values using the
  [`valuable`] crate ([#1608], [#1888], [#1887])
- `enabled!` macro for testing if a span or event is enabled ([#1882])

### Changed

- `tracing-core`: updated to [0.1.22][core-0.1.22]
- `tracing-attributes`: updated to [0.1.19][attributes-0.1.19]

### Fixed

- **log**: Fixed "use of moved value" compiler error when the "log" feature is
  enabled ([#1823])
- Fixed macro hygiene issues when used in a crate that defines its own `concat!`
  macro ([#1842])
- A very large number of documentation fixes and improvements.

Thanks to @@Vlad-Scherbina, @Skepfyr, @Swatinem, @guswynn, @teohhanhui,
@xd009642, @tobz, @d-e-s-o@0b01, and @nickelc for contributing to this release!

[`valuable`]: https://crates.io/crates/valuable
[post]: https://tokio.rs/blog/2021-05-valuable
[core-0.1.22]: https://github.com/tokio-rs/tracing/releases/tag/tracing-core-0.1.22
[attributes-0.1.19]: https://github.com/tokio-rs/tracing/releases/tag/tracing-attributes-0.1.19
[#1608]: https://github.com/tokio-rs/tracing/pull/1608
[#1888]: https://github.com/tokio-rs/tracing/pull/1888
[#1887]: https://github.com/tokio-rs/tracing/pull/1887
[#1882]: https://github.com/tokio-rs/tracing/pull/1882
[#1823]: https://github.com/tokio-rs/tracing/pull/1823
[#1842]: https://github.com/tokio-rs/tracing/pull/1842

# 0.1.29 (October 5th, 2021)

This release adds support for recording `Option<T> where T: Value` as typed
`tracing` field values. It also includes significant performance improvements
for functions annotated with the `#[instrument]` attribute when the generated
span is disabled.

### Changed

- `tracing-core`: updated to v0.1.21
- `tracing-attributes`: updated to v0.1.18

### Added

- **field**: `Value` impl for `Option<T> where T: Value` ([#1585])
- **attributes**: - improved performance when skipping `#[instrument]`-generated
  spans below the max level ([#1600], [#1605], [#1614], [#1616], [#1617])

### Fixed

- **instrument**: added missing `Future` implementation for `WithSubscriber`,
  making the `WithDispatch` extension trait actually useable ([#1602])
- Documentation fixes and improvements ([#1595], [#1601], [#1597])

Thanks to @brianburgers, @mattiast, @DCjanus, @oli-obk, and @matklad for
contributing to this release!

[#1585]: https://github.com/tokio-rs/tracing/pull/1585
[#1595]: https://github.com/tokio-rs/tracing/pull/1596
[#1597]: https://github.com/tokio-rs/tracing/pull/1597
[#1600]: https://github.com/tokio-rs/tracing/pull/1600
[#1601]: https://github.com/tokio-rs/tracing/pull/1601
[#1602]: https://github.com/tokio-rs/tracing/pull/1602
[#1605]: https://github.com/tokio-rs/tracing/pull/1605
[#1614]: https://github.com/tokio-rs/tracing/pull/1614
[#1616]: https://github.com/tokio-rs/tracing/pull/1616
[#1617]: https://github.com/tokio-rs/tracing/pull/1617

# 0.1.28 (September 17th, 2021)

This release fixes an issue where the RustDoc documentation was rendered
incorrectly. It doesn't include any actual code changes, and is very boring and
can be ignored.

### Fixed

- **docs**: Incorrect documentation rendering due to unclosed `<div>` tag
  ([#1572])

[#1572]: https://github.com/tokio-rs/tracing/pull/1572

# 0.1.27 (September 13, 2021)

This release adds a new [`Span::or_current`] method to aid in efficiently
propagating span contexts to spawned threads or tasks. Additionally, it updates
the [`tracing-core`] version to [0.1.20] and the [`tracing-attributes`] version to
[0.1.16], ensuring that a number of new features in those crates are present.

### Fixed

- **instrument**: Added missing `WithSubscriber` implementations for futures and
  other types ([#1424])

### Added

- `Span::or_current` method, to help with efficient span context propagation
  ([#1538])
- **attributes**: add `skip_all` option to `#[instrument]` ([#1548])
- **attributes**: record primitive types as primitive values rather than as
  `fmt::Debug` ([#1378])
- **core**: `NoSubscriber`, a no-op `Subscriber` implementation
  ([#1549])
- **core**: Added `Visit::record_f64` and support for recording floating-point
  values ([#1507], [#1522])
- A large number of documentation improvements and fixes ([#1369], [#1398],
  [#1435], [#1442], [#1524], [#1556])

Thanks to new contributors @dzvon and @mbergkvist, as well as @teozkr,
@maxburke, @LukeMathWalker, and @jsgf, for contributing to this
release!

[`Span::or_current`]: https://docs.rs/tracing/0.1.27/tracing/struct.Span.html#method.or_current
[`tracing-core`]: https://crates.io/crates/tracing-core
[`tracing-attributes`]: https://crates.io/crates/tracing-attributes
[`tracing-core`]: https://crates.io/crates/tracing-core
[0.1.20]: https://github.com/tokio-rs/tracing/releases/tag/tracing-core-0.1.20
[0.1.16]: https://github.com/tokio-rs/tracing/releases/tag/tracing-attributes-0.1.16
[#1424]: https://github.com/tokio-rs/tracing/pull/1424
[#1538]: https://github.com/tokio-rs/tracing/pull/1538
[#1548]: https://github.com/tokio-rs/tracing/pull/1548
[#1378]: https://github.com/tokio-rs/tracing/pull/1378
[#1507]: https://github.com/tokio-rs/tracing/pull/1507
[#1522]: https://github.com/tokio-rs/tracing/pull/1522
[#1369]: https://github.com/tokio-rs/tracing/pull/1369
[#1398]: https://github.com/tokio-rs/tracing/pull/1398
[#1435]: https://github.com/tokio-rs/tracing/pull/1435
[#1442]: https://github.com/tokio-rs/tracing/pull/1442
[#1524]: https://github.com/tokio-rs/tracing/pull/1524
[#1556]: https://github.com/tokio-rs/tracing/pull/1556

# 0.1.26 (April 30, 2021)

### Fixed

- **attributes**: Compatibility between `#[instrument]` and `async-trait`
  v0.1.43 and newer ([#1228])
- Several documentation fixes ([#1305], [#1344])
### Added

- `Subscriber` impl for `Box<dyn Subscriber + Send + Sync + 'static>` ([#1358])
- `Subscriber` impl for `Arc<dyn Subscriber + Send + Sync + 'static>` ([#1374])
- Symmetric `From` impls for existing `Into` impls on `span::Current`, `Span`,
  and `Option<Id>` ([#1335], [#1338])
- `From<EnteredSpan>` implementation for `Option<Id>`, allowing `EnteredSpan` to
  be used in a `span!` macro's `parent:` field ([#1325])
- `Attributes::fields` accessor that returns the set of fields defined on a
  span's `Attributes` ([#1331])


Thanks to @Folyd, @nightmared, and new contributors @rmsc and @Fishrock123 for
contributing to this release!

[#1227]: https://github.com/tokio-rs/tracing/pull/1228
[#1305]: https://github.com/tokio-rs/tracing/pull/1305
[#1325]: https://github.com/tokio-rs/tracing/pull/1325
[#1338]: https://github.com/tokio-rs/tracing/pull/1338
[#1344]: https://github.com/tokio-rs/tracing/pull/1344
[#1358]: https://github.com/tokio-rs/tracing/pull/1358
[#1374]: https://github.com/tokio-rs/tracing/pull/1374
[#1335]: https://github.com/tokio-rs/tracing/pull/1335
[#1331]: https://github.com/tokio-rs/tracing/pull/1331

# 0.1.25 (February 23, 2021)

### Added

- `Span::entered` method for entering a span and moving it into a guard by value
  rather than borrowing it ([#1252])

Thanks to @matklad for contributing to this release!

[#1252]: https://github.com/tokio-rs/tracing/pull/1252

# 0.1.24 (February 17, 2021)

### Fixed

- **attributes**: Compiler error when using `#[instrument(err)]` on functions
  which return `impl Trait` ([#1236])
- Fixed broken match arms in event macros ([#1239])
- Documentation improvements ([#1232])

Thanks to @bkchr and @lfranke for contributing to this release!

[#1236]: https://github.com/tokio-rs/tracing/pull/1236
[#1239]: https://github.com/tokio-rs/tracing/pull/1239
[#1232]: https://github.com/tokio-rs/tracing/pull/1232

# 0.1.23 (February 4, 2021)

### Fixed

- **attributes**: Compiler error when using `#[instrument(err)]` on functions
  with mutable parameters ([#1167])
- **attributes**: Missing function visibility modifier when using
  `#[instrument]` with `async-trait` ([#977])
- **attributes** Removed unused `syn` features ([#928])
- **log**: Fixed an issue where the `tracing` macros would generate code for
  events whose levels are disabled statically by the `log` crate's
  `static_max_level_XXX` features ([#1175])
- Fixed deprecations and clippy lints ([#1195])
- Several documentation fixes and improvements ([#941], [#965], [#981], [#1146],
  [#1215])
  
### Changed

- **attributes**: `tracing-futures` dependency is no longer required when using
  `#[instrument]` on async functions ([#808])
- **attributes**: Updated `tracing-attributes` minimum dependency to v0.1.12
  ([#1222])

Thanks to @nagisa, @Txuritan, @TaKO8Ki, @okready, and @krojew for contributing
to this release!

[#1167]: https://github.com/tokio-rs/tracing/pull/1167
[#977]: https://github.com/tokio-rs/tracing/pull/977
[#965]: https://github.com/tokio-rs/tracing/pull/965
[#981]: https://github.com/tokio-rs/tracing/pull/981
[#1215]: https://github.com/tokio-rs/tracing/pull/1215
[#808]: https://github.com/tokio-rs/tracing/pull/808
[#941]: https://github.com/tokio-rs/tracing/pull/941
[#1146]: https://github.com/tokio-rs/tracing/pull/1146
[#1175]: https://github.com/tokio-rs/tracing/pull/1175
[#1195]: https://github.com/tokio-rs/tracing/pull/1195
[#1222]: https://github.com/tokio-rs/tracing/pull/1222

# 0.1.22 (November 23, 2020)

### Changed

- Updated `pin-project-lite` dependency to 0.2 ([#1108])

[#1108]: https://github.com/tokio-rs/tracing/pull/1108

# 0.1.21 (September 28, 2020)

### Fixed

- Incorrect inlining of `Span::new`, `Span::new_root`, and `Span::new_child_of`,
  which could result in  `dispatcher::get_default` being inlined at the callsite
  ([#994])
- Regression where using a struct field as a span or event field when other
  fields on that struct are borrowed mutably would fail to compile ([#987])

### Changed

- Updated `tracing-core` to 0.1.17 ([#992])

### Added

- `Instrument` trait and `Instrumented` type for attaching a `Span` to a
  `Future` ([#808])
- `Copy` implementations for `Level` and `LevelFilter` ([#992])
- Multiple documentation fixes and improvements ([#964], [#980], [#981])

Thanks to @nagisa, and new contributors @SecurityInsanity, @froydnj, @jyn514 and
@TaKO8Ki for contributing to this release!

[#994]: https://github.com/tokio-rs/tracing/pull/994
[#992]: https://github.com/tokio-rs/tracing/pull/992
[#987]: https://github.com/tokio-rs/tracing/pull/987
[#980]: https://github.com/tokio-rs/tracing/pull/980
[#981]: https://github.com/tokio-rs/tracing/pull/981
[#964]: https://github.com/tokio-rs/tracing/pull/964
[#808]: https://github.com/tokio-rs/tracing/pull/808

# 0.1.20 (August 24, 2020)

### Changed

- Significantly reduced assembly generated by macro invocations (#943)
- Updated `tracing-core` to 0.1.15 (#943)

### Added 

- Documented minimum supported Rust version policy (#941)

# 0.1.19 (August 10, 2020)

### Fixed

- Updated `tracing-core` to fix incorrect calculation of the global max level
  filter (#908)

### Added

- **attributes**: Support for using `self` in field expressions when
  instrumenting `async-trait` functions (#875)
- Several documentation improvements (#832, #881, #896, #897, #911, #913)

Thanks to @anton-dutov, @nightmared, @mystor, and @toshokan for contributing to
this release!
  
# 0.1.18 (July 31, 2020)

### Fixed

- Fixed a bug where `LevelFilter::OFF` (and thus also the `static_max_level_off`
  feature flag) would enable *all* traces, rather than *none* (#853)
- **log**: Fixed `tracing` macros and `Span`s not checking `log::max_level`
  before emitting `log` records (#870)
  
### Changed

- **macros**: Macros now check the global max level (`LevelFilter::current`)
  before the per-callsite cache when determining if a span or event is enabled.
  This significantly improves performance in some use cases (#853)
- **macros**: Simplified the code generated by macro expansion significantly,
  which may improve compile times and/or `rustc` optimizatation of surrounding
  code (#869, #869)
- **macros**: Macros now check the static max level before checking any runtime
  filtering, improving performance when a span or event is disabled by a
  `static_max_level_XXX` feature flag (#868) 
- `LevelFilter` is now a re-export of the `tracing_core::LevelFilter` type, it
  can now be used interchangeably with the versions in `tracing-core` and
  `tracing-subscriber` (#853)
- Significant performance improvements when comparing `LevelFilter`s and
  `Level`s (#853)
- Updated the minimum `tracing-core` dependency to 0.1.12 (#853)

### Added

- **macros**: Quoted string literals may now be used as field names, to allow
  fields whose names are not valid Rust identifiers (#790)
- **docs**: Several documentation improvements (#850, #857, #841)
- `LevelFilter::current()` function, which returns the highest level that any
  subscriber will enable (#853)
- `Subscriber::max_level_hint` optional trait method, for setting the value
  returned by `LevelFilter::current()` (#853)
  
Thanks to new contributors @cuviper, @ethanboxx, @ben0x539, @dignati,
@colelawrence, and @rbtcollins for helping out with this release!

# 0.1.17 (July 22, 2020)

### Changed

- **log**: Moved verbose span enter/exit log records to "tracing::span::active"
  target, allowing them to be filtered separately (#833)
- **log**: All span lifecycle log records without fields now have the `Trace`
  log filter, to guard against `log` users enabling them by default with blanket
  level filtering (#833)
  
### Fixed

- **log**/**macros**: Fixed missing implicit imports of the
  `tracing::field::debug` and `tracing::field::display` functions inside the
  macros when the "log" feature is enabled (#835)

# 0.1.16 (July 8, 2020)

### Added

- **attributes**: Support for arbitrary expressions as fields in `#[instrument]` (#672)
- **attributes**: `#[instrument]` now emits a compiler warning when ignoring unrecognized
  input (#672, #786)
- Improved documentation on using `tracing` in async code (#769)

### Changed

- Updated `tracing-core` dependency to 0.1.11

### Fixed

- **macros**: Excessive monomorphization in macros, which could lead to
  longer compilation times (#787) 
- **log**: Compiler warnings in macros when `log` or `log-always` features
  are enabled (#753)
- Compiler error when `tracing-core/std` feature is enabled but `tracing/std` is
  not (#760)

Thanks to @nagisa for contributing to this release!

# 0.1.15 (June 2, 2020)

### Changed

- **macros**: Replaced use of legacy `local_inner_macros` with `$crate::` (#740)

### Added

- Docs fixes and improvements (#742, #731, #730)

Thanks to @bnjjj, @blaenk, and @LukeMathWalker for contributing to this release!

# 0.1.14 (May 14, 2020)

### Added

- **log**: When using the [`log`] compatibility feature alongside a `tracing`
  `Subscriber`, log records for spans now include span IDs (#613)
- **attributes**: Support for using `#[instrument]` on methods that are part of
  [`async-trait`] trait implementations (#711)
- **attributes**: Optional `#[instrument(err)]` argument to automatically emit
  an event if an instrumented function returns `Err` (#637) 
- Added `#[must_use]` attribute to the guard returned by
  `subscriber::set_default` (#685)
  
### Changed

- **log**: Made [`log`] records emitted by spans much less noisy when span IDs are
 not available (#613)
 
### Fixed

- Several typos in the documentation (#656, #710, #715)

Thanks to @FintanH, @shepmaster, @inanna-malick, @zekisharif, @bkchr, @majecty,
@ilana and @nightmared for contributing to this release! 

[`async-trait`]: https://crates.io/crates/async-trait 
[`log`]: https://crates.io/crates/log

# 0.1.13 (February 26, 2019)

### Added

- **field**: `field::Empty` type for declaring empty fields whose values will be
  recorded later (#548)
- **field**: `field::Value` implementations for `Wrapping` and `NonZero*`
  numbers (#538)
- **attributes**: Support for adding arbitrary literal fields to spans generated
  by `#[instrument]` (#569)
- **attributes**: `#[instrument]` now emits a helpful compiler error when
  attempting to skip a function parameter (#600)

### Changed

- **attributes**: The `#[instrument]` attribute was placed under an on-by-default
  feature flag "attributes" (#603)

### Fixed

- Broken and unresolvable links in RustDoc (#595)

Thanks to @oli-cosmian and @Kobzol for contributing to this release!

# 0.1.12 (January 11, 2019)

### Added

- `Span::with_subscriber` method to access the subscriber that tracks a `Span`
  (#503)
- API documentation now shows which features are required by feature-flagged
  items (#523)
- Improved README examples (#496)
- Documentation links to related crates (#507)

# 0.1.11 (December 20, 2019)

### Added

- `Span::is_none` method (#475)
- `LevelFilter::into_level` method (#470)
- `LevelFilter::from_level` function and `From<Level>` impl (#471)
- Documented minimum supported Rust version (#482)

### Fixed

- Incorrect parameter type to `Span::follows_from` that made it impossible to
  call (#467)
- Missing whitespace in `log` records generated when enabling the `log` feature
  flag (#484)
- Typos and missing links in documentation (#405, #423, #439)

# 0.1.10 (October 23, 2019)

### Added

- Support for destructuring in arguments to `#[instrument]`ed functions (#397)
- Generated field for `self` parameters when `#[instrument]`ing methods (#397)
- Optional `skip` argument to `#[instrument]` for excluding function parameters
  from generated spans (#359)
- Added `dispatcher::set_default` and `subscriber::set_default` APIs, which
  return a drop guard (#388)

### Fixed

- Some minor documentation errors (#356, #370)

# 0.1.9 (September 13, 2019)

### Fixed

- Fixed `#[instrument]`ed async functions not compiling on `nightly-2019-09-11`
  or newer (#342)

### Changed

- Significantly reduced performance impact of skipped spans and events when a
  `Subscriber` is not in use (#326)
- The `log` feature will now only cause `tracing` spans and events to emit log
  records when a `Subscriber` is not in use (#346)

### Added

- Added support for overriding the name of the span generated by `#[instrument]`
  (#330)
- `log-always` feature flag to emit log records even when a `Subscriber` is set
  (#346)

# 0.1.8 (September 3, 2019)

### Changed

- Reorganized and improved API documentation (#317)

### Removed

- Dev-dependencies on `ansi_term` and `humantime` crates, which were used only
  for examples (#316)

# 0.1.7 (August 30, 2019)

### Changed

- New (curly-brace free) event message syntax to place the message in the first
  field rather than the last (#309)

### Fixed

- Fixed a regression causing macro stack exhaustion when the `log` feature flag
  is enabled (#304)

# 0.1.6 (August 20, 2019)

### Added

- `std::error::Error` as a new primitive type (#277)
- Support for mixing key-value fields and `format_args` messages without curly
  braces as delimiters (#288)

### Changed

- `tracing-core` dependency to 0.1.5 (#294)
- `tracing-attributes` dependency to 0.1.2 (#297)

# 0.1.5 (August 9, 2019)

### Added

- Support for `no-std` + `liballoc` (#263)

### Changed

- Using the `#[instrument]` attribute on `async fn`s no longer requires a
  feature flag (#258)

### Fixed

- The `#[instrument]` macro now works on generic functions (#262)

# 0.1.4 (August 8, 2019)

### Added

- `#[instrument]` attribute for automatically adding spans to functions (#253)

# 0.1.3 (July 11, 2019)

### Added

- Log messages when a subscriber indicates that a span has closed, when the
  `log` feature flag is enabled (#180).

### Changed

- `tracing-core` minimum dependency version to 0.1.2 (#174).

### Fixed

- Fixed an issue where event macro invocations with a single field, using local
  variable shorthand, would recur infinitely (#166).
- Fixed uses of deprecated `tracing-core` APIs (#174).

# 0.1.2 (July 6, 2019)

### Added

- `Span::none()` constructor, which does not require metadata and
  returns a completely empty span (#147).
- `Span::current()` function, returning the current span if it is
  known to the subscriber (#148).

### Fixed

- Broken macro imports when used prefixed with `tracing::` (#152).

# 0.1.1 (July 3, 2019)

### Changed

- `cfg_if` dependency to 0.1.9.

### Fixed

- Compilation errors when the `log` feature is enabled (#131).
- Unclear wording and typos in documentation (#124, #128, #142).

# 0.1.0 (June 27, 2019)

- Initial release
