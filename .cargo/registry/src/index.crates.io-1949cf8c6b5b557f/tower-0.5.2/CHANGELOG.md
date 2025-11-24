# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# 0.5.2

### Added

- **util**: Add `BoxCloneSyncService` which is a `Clone + Send + Sync` boxed `Service` ([#777])
- **util**: Add `BoxCloneSyncServiceLayer` which is a `Clone + Send + Sync` boxed `Layer` ([802])

# 0.5.1

### Fixed

- Fix minimum version of `tower-layer` dependency ([#787])

[#787]: https://github.com/tower-rs/tower/pull/787

# 0.5.0

### Fixed

- **util**: `BoxService` is now `Sync` ([#702])

### Changed

- **util**: Removed deprecated `ServiceExt::ready_and` method and `ReadyAnd`
  future ([#652])
- **retry**: **Breaking Change** `retry::Policy::retry` now accepts `&mut Req` and `&mut Res` instead of the previous mutable versions. This
  increases the flexibility of the retry policy. To update, update your method signature to include `mut` for both parameters. ([#584])
- **retry**: **Breaking Change** Change Policy to accept &mut self ([#681])
- **retry**: Add generic backoff utilities ([#685])
- **retry**: Add Budget trait. This allows end-users to implement their own budget and bucket implementations. ([#703])
- **reconnect**: **Breaking Change** Remove unused generic parameter from `Reconnect::new` ([#755])
- **ready-cache**: Allow iteration over ready services ([#700])
- **discover**: Implement `Clone` for Change ([#701])
- **util**: Add a BoxCloneServiceLayer ([#708])
- **rng**: use a simpler random 2-sampler ([#716])
- **filter**: Derive `Clone` for `AsyncFilterLayer` ([#731])
- **general**: Update IndexMap ([#741])
- **MSRV**: Increase MSRV to 1.63.0 ([#741])

[#702]: https://github.com/tower-rs/tower/pull/702
[#652]: https://github.com/tower-rs/tower/pull/652
[#584]: https://github.com/tower-rs/tower/pull/584
[#681]: https://github.com/tower-rs/tower/pull/681
[#685]: https://github.com/tower-rs/tower/pull/685
[#703]: https://github.com/tower-rs/tower/pull/703
[#755]: https://github.com/tower-rs/tower/pull/755
[#700]: https://github.com/tower-rs/tower/pull/700
[#701]: https://github.com/tower-rs/tower/pull/701
[#708]: https://github.com/tower-rs/tower/pull/708
[#716]: https://github.com/tower-rs/tower/pull/716
[#731]: https://github.com/tower-rs/tower/pull/731
[#741]: https://github.com/tower-rs/tower/pull/741

# 0.4.12 (February 16, 2022)

### Fixed

- **hedge**, **load**, **retry**: Fix use of `Instant` operations that can panic
  on platforms where `Instant` is not monotonic ([#633])
- Disable `attributes` feature on `tracing` dependency ([#623])
- Remove unused dependencies and dependency features with some feature
  combinations ([#603], [#602])
- **docs**: Fix a typo in the RustDoc for `Buffer` ([#622])

### Changed

- Updated minimum supported Rust version (MSRV) to 1.49.0.
- **hedge**: Updated `hdrhistogram` dependency to v7.0 ([#602])
- Updated `tokio-util` dependency to v0.7 ([#638])

[#633]: https://github.com/tower-rs/tower/pull/633
[#623]: https://github.com/tower-rs/tower/pull/623
[#603]: https://github.com/tower-rs/tower/pull/603
[#602]: https://github.com/tower-rs/tower/pull/602
[#622]: https://github.com/tower-rs/tower/pull/622
[#638]: https://github.com/tower-rs/tower/pull/638

# 0.4.11 (November 18, 2021)

### Added

- **util**: Add `BoxCloneService` which is a `Clone + Send` boxed `Service` ([#615])
- **util**: Add `ServiceExt::boxed` and `ServiceExt::boxed_clone` for applying the
  `BoxService` and `BoxCloneService` middleware ([#616])
- **builder**: Add `ServiceBuilder::boxed` and `ServiceBuilder::boxed_clone` for
  applying `BoxService` and `BoxCloneService` layers ([#616])

### Fixed

- **util**: Remove redundant `F: Clone` bound from `ServiceExt::map_request` ([#607])
- **util**: Remove unnecessary `Debug` bounds from `impl Debug for BoxService` ([#617])
- **util**: Remove unnecessary `Debug` bounds from `impl Debug for UnsyncBoxService` ([#617])
- **balance**: Remove redundant `Req: Clone` bound from `Clone` impls
  for `MakeBalance`, and `MakeBalanceLayer` ([#607])
- **balance**: Remove redundant `Req: Debug` bound from `Debug` impls
  for `MakeBalance`, `MakeFuture`, `Balance`, and `Pool` ([#607])
- **ready-cache**: Remove redundant `Req: Debug` bound from `Debug` impl
  for `ReadyCache` ([#607])
- **steer**: Remove redundant `Req: Debug` bound from `Debug` impl
  for `Steer` ([#607])
- **docs**: Fix `doc(cfg(...))` attributes
  of `PeakEwmaDiscover`, and `PendingRequestsDiscover` ([#610])

[#607]: https://github.com/tower-rs/tower/pull/607
[#610]: https://github.com/tower-rs/tower/pull/610
[#615]: https://github.com/tower-rs/tower/pull/615
[#616]: https://github.com/tower-rs/tower/pull/616
[#617]: https://github.com/tower-rs/tower/pull/617

# 0.4.10 (October 19, 2021)

- Fix accidental breaking change when using the
  `rustdoc::broken_intra_doc_links` lint ([#605])
- Clarify that tower's minimum supported rust version is 1.46 ([#605])

[#605]: https://github.com/tower-rs/tower/pull/605

# 0.4.9 (October 13, 2021)

- Migrate to [pin-project-lite] ([#595])
- **builder**: Implement `Layer` for `ServiceBuilder` ([#600])
- **builder**: Add `ServiceBuilder::and_then` analogous to
  `ServiceExt::and_then` ([#601])

[#600]: https://github.com/tower-rs/tower/pull/600
[#601]: https://github.com/tower-rs/tower/pull/601
[#595]: https://github.com/tower-rs/tower/pull/595
[pin-project-lite]: https://crates.io/crates/pin-project-lite

# 0.4.8 (May 28, 2021)

- **builder**: Add `ServiceBuilder::map_result` analogous to
  `ServiceExt::map_result` ([#583]) 
- **limit**: Add `GlobalConcurrencyLimitLayer` to allow reusing a concurrency
  limit across multiple services ([#574])

[#574]: https://github.com/tower-rs/tower/pull/574
[#583]: https://github.com/tower-rs/tower/pull/583

# 0.4.7 (April 27, 2021)

### Added

- **builder**: Add `ServiceBuilder::check_service` to check the request,
    response, and error types of the output service. ([#576])
- **builder**: Add `ServiceBuilder::check_service_clone` to check the output
    service can be cloned. ([#576])

### Fixed

- **spawn_ready**: Abort spawned background tasks when the `SpawnReady` service
  is dropped, fixing a potential task/resource leak (#[581])
- Fixed broken documentation links ([#578])

[#576]: https://github.com/tower-rs/tower/pull/576
[#578]: https://github.com/tower-rs/tower/pull/578
[#581]: https://github.com/tower-rs/tower/pull/581

# 0.4.6 (February 26, 2021)

### Deprecated

- **util**: Deprecated `ServiceExt::ready_and` (renamed to `ServiceExt::ready`).
  ([#567])
- **util**: Deprecated `ReadyAnd` future (renamed to `Ready`). ([#567])
### Added

- **builder**: Add `ServiceBuilder::layer_fn` to add a layer built from a
  function. ([#560])
- **builder**: Add `ServiceBuilder::map_future` for transforming the futures
  produced by a service. ([#559])
- **builder**: Add `ServiceBuilder::service_fn` for applying `Layer`s to an
  async function using `util::service_fn`. ([#564])
- **util**: Add example for `service_fn`. ([#563])
- **util**: Add `BoxLayer` for creating boxed `Layer` trait objects. ([#569])

[#567]: https://github.com/tower-rs/tower/pull/567
[#560]: https://github.com/tower-rs/tower/pull/560
[#559]: https://github.com/tower-rs/tower/pull/559
[#564]: https://github.com/tower-rs/tower/pull/564
[#563]: https://github.com/tower-rs/tower/pull/563
[#569]: https://github.com/tower-rs/tower/pull/569

# 0.4.5 (February 10, 2021)

### Added

- **util**: Add `ServiceExt::map_future`. ([#542])
- **builder**: Add `ServiceBuilder::option_layer` to optionally add a layer. ([#555])
- **make**: Add `Shared` which lets you implement `MakeService` by cloning a
  service. ([#533])

### Fixed

- **util**: Make combinators that contain closures implement `Debug`. They
  previously wouldn't since closures never implement `Debug`. ([#552])
- **steer**: Implement `Clone` for `Steer`. ([#554])
- **spawn-ready**: SpawnReady now propagates the current `tracing` span to
  spawned tasks ([#557])
- Only pull in `tracing` for the features that need it. ([#551])

[#542]: https://github.com/tower-rs/tower/pull/542
[#555]: https://github.com/tower-rs/tower/pull/555
[#557]: https://github.com/tower-rs/tower/pull/557
[#533]: https://github.com/tower-rs/tower/pull/533
[#551]: https://github.com/tower-rs/tower/pull/551
[#554]: https://github.com/tower-rs/tower/pull/554
[#552]: https://github.com/tower-rs/tower/pull/552

# 0.4.4 (January 20, 2021)

### Added

- **util**: Implement `Layer` for `Either<A, B>`. ([#531])
- **util**: Implement `Clone` for `FilterLayer`. ([#535])
- **timeout**: Implement `Clone` for `TimeoutLayer`. ([#535])
- **limit**: Implement `Clone` for `RateLimitLayer`. ([#535])

### Fixed

- Added "full" feature which turns on all other features. ([#532])
- **spawn-ready**: Avoid oneshot allocations. ([#538])

[#531]: https://github.com/tower-rs/tower/pull/531
[#532]: https://github.com/tower-rs/tower/pull/532
[#535]: https://github.com/tower-rs/tower/pull/535
[#538]: https://github.com/tower-rs/tower/pull/538

# 0.4.3 (January 13, 2021)

### Added

- **filter**: `Filter::check` and `AsyncFilter::check` methods which check a
  request against the filter's `Predicate` ([#521])
- **filter**: Added `get_ref`, `get_mut`, and `into_inner` methods to `Filter`
  and `AsyncFilter`, allowing access to the wrapped service ([#522])
- **util**: Added `layer` associated function to `AndThen`, `Then`,
  `MapRequest`, `MapResponse`, and `MapResult` types. These return a `Layer`
  that produces middleware of that type, as a convenience to avoid having to
  import the `Layer` type separately. ([#524])
- **util**: Added missing `Clone` impls to `AndThenLayer`, `MapRequestLayer`,
  and `MapErrLayer`, when the mapped function implements `Clone` ([#525])
- **util**: Added `FutureService::new` constructor, with less restrictive bounds
  than the `future_service` free function ([#523])

[#521]: https://github.com/tower-rs/tower/pull/521
[#522]: https://github.com/tower-rs/tower/pull/522
[#523]: https://github.com/tower-rs/tower/pull/523
[#524]: https://github.com/tower-rs/tower/pull/524
[#525]: https://github.com/tower-rs/tower/pull/525

# 0.4.2 (January 11, 2021)

### Added

- Export `layer_fn` and `LayerFn` from the `tower::layer` module. ([#516])

### Fixed

- Fix missing `Sync` implementation for `Buffer` and `ConcurrencyLimit` ([#518])

[#518]: https://github.com/tower-rs/tower/pull/518
[#516]: https://github.com/tower-rs/tower/pull/516

# 0.4.1 (January 7, 2021)

### Fixed

- Updated `tower-layer` to 0.3.1 to fix broken re-exports.

# 0.4.0 (January 7, 2021)

This is a major breaking release including a large number of changes. In
particular, this release updates `tower` to depend on Tokio 1.0, and moves all
middleware into the `tower` crate. In addition, Tower 0.4 reworks several
middleware APIs, as well as introducing new ones. 

This release does *not* change the core `Service` or `Layer` traits, so `tower`
0.4 still depends on `tower-service` 0.3 and `tower-layer` 0.3. This means that
`tower` 0.4 is still compatible with libraries that depend on those crates.

### Added

- **make**: Added `MakeService::into_service` and `MakeService::as_service` for
  converting `MakeService`s into `Service`s ([#492])
- **steer**: Added `steer` middleware for routing requests to one of a set of
  services ([#426])
- **util**: Added `MapRequest` middleware and `ServiceExt::map_request`, for
  applying a function to a request before passing it to the inner service
  ([#435])
- **util**: Added `MapResponse` middleware and `ServiceExt::map_response`, for
  applying a function to the `Response` type of an inner service after its
  future completes ([#435])
- **util**: Added `MapErr` middleware and `ServiceExt::map_err`, for
  applying a function to the `Error` returned by an inner service if it fails
  ([#396])
- **util**: Added `MapResult` middleware and `ServiceExt::map_result`, for
  applying a function to the `Result` returned by an inner service's future
  regardless of  whether it succeeds or fails ([#499])
- **util**: Added `Then` middleware and `ServiceExt::then`, for chaining another
  future after an inner service's future completes (with a `Response` or an
  `Error`) ([#500])
- **util**: Added `AndThen` middleware and `ServiceExt::and_then`, for
  chaining another future after an inner service's future completes successfully
  ([#485])
- **util**: Added `layer_fn`, for constructing a `Layer` from a function taking
  a `Service` and returning a different `Service` ([#491])
- **util**: Added `FutureService`, which implements `Service` for a
  `Future` whose `Output` type is a `Service` ([#496])
- **util**: Added `BoxService::layer` and `UnsyncBoxService::layer`, to make
  constructing layers more ergonomic ([#503])
- **layer**: Added `Layer` impl for `&Layer` ([#446])
- **retry**: Added `Retry::get_ref`, `Retry::get_mut`, and `Retry::into_inner`
  to access the inner service ([#463])
- **timeout**: Added `Timeout::get_ref`, `Timeout::get_mut`, and
  `Timeout::into_inner` to access the inner service ([#463])
- **buffer**: Added `Clone` and `Copy` impls for `BufferLayer` (#[493])
- Several documentation improvements ([#442], [#444], [#445], [#449], [#487],
  [#490], [#506]])

### Changed

- All middleware `tower-*` crates were merged into `tower` and placed
  behind feature flags ([#432])
- Updated Tokio dependency to 1.0 ([#489])
- **builder**: Make `ServiceBuilder::service` take `self` by reference rather
  than by value ([#504])
- **reconnect**: Return errors from `MakeService` in the response future, rather than
  in `poll_ready`, allowing the reconnect service to be reused when a reconnect
  fails ([#386], [#437])
- **discover**: Changed `Discover` to be a sealed trait alias for a
  `TryStream<Item = Change>`. `Discover` implementations are now written by
  implementing `Stream`. ([#443])
- **load**: Renamed the `Instrument` trait to `TrackCompletion` ([#445])
- **load**: Renamed `NoInstrument` to `CompleteOnResponse` ([#445])
- **balance**: Renamed `BalanceLayer` to `MakeBalanceLayer` ([#449])
- **balance**: Renamed `BalanceMake` to `MakeBalance` ([#449])
- **ready-cache**: Changed `ready_cache::error::Failed`'s `fmt::Debug` impl to
  require the key type to also implement `fmt::Debug` ([#467])
- **filter**: Changed `Filter` and `Predicate` to use a synchronous function as
  a predicate ([#508])
- **filter**: Renamed the previous `Filter` and `Predicate` (where `Predicate`s
  returned a `Future`) to `AsyncFilter` and `AsyncPredicate` ([#508])
- **filter**: `Predicate`s now take a `Request` type by value and may return a
  new request, potentially of a different type ([#508])
- **filter**: `Predicate`s may now return an error of any type ([#508])

### Fixed

- **limit**: Fixed an issue where `RateLimit` services do not reset the remaining
  count when rate limiting ([#438], [#439])
- **util**: Fixed a bug where `oneshot` futures panic if the service does not
  immediately become ready ([#447])
- **ready-cache**: Fixed `ready_cache::error::Failed` not returning inner error types
  via `Error::source` ([#467])
- **hedge**: Fixed an interaction with `buffer` where `buffer` slots were
  eagerly reserved for hedge requests even if they were not sent ([#472])
- **hedge**: Fixed the use of a fixed 10 second bound on the hedge latency
  histogram resulting on errors with longer-lived requests. The latency
  histogram now automatically resizes ([#484])
- **buffer**: Fixed an issue where tasks waiting for buffer capacity were not
  woken when a buffer is dropped, potentially resulting in a task leak ([#480])

### Removed

- Remove `ServiceExt::ready`.
- **discover**: Removed `discover::stream` module, since `Discover` is now an
  alias for `Stream` ([#443])
- **buffer**: Removed `MakeBalance::from_rng`, which caused all balancers to use
  the same RNG ([#497])

[#432]: https://github.com/tower-rs/tower/pull/432
[#426]: https://github.com/tower-rs/tower/pull/426
[#435]: https://github.com/tower-rs/tower/pull/435
[#499]: https://github.com/tower-rs/tower/pull/499
[#386]: https://github.com/tower-rs/tower/pull/386
[#437]: https://github.com/tower-rs/tower/pull/487
[#438]: https://github.com/tower-rs/tower/pull/438
[#437]: https://github.com/tower-rs/tower/pull/439
[#443]: https://github.com/tower-rs/tower/pull/443
[#442]: https://github.com/tower-rs/tower/pull/442
[#444]: https://github.com/tower-rs/tower/pull/444
[#445]: https://github.com/tower-rs/tower/pull/445
[#446]: https://github.com/tower-rs/tower/pull/446
[#447]: https://github.com/tower-rs/tower/pull/447
[#449]: https://github.com/tower-rs/tower/pull/449
[#463]: https://github.com/tower-rs/tower/pull/463
[#396]: https://github.com/tower-rs/tower/pull/396
[#467]: https://github.com/tower-rs/tower/pull/467
[#472]: https://github.com/tower-rs/tower/pull/472
[#480]: https://github.com/tower-rs/tower/pull/480
[#484]: https://github.com/tower-rs/tower/pull/484
[#489]: https://github.com/tower-rs/tower/pull/489
[#497]: https://github.com/tower-rs/tower/pull/497
[#487]: https://github.com/tower-rs/tower/pull/487
[#493]: https://github.com/tower-rs/tower/pull/493
[#491]: https://github.com/tower-rs/tower/pull/491
[#495]: https://github.com/tower-rs/tower/pull/495
[#503]: https://github.com/tower-rs/tower/pull/503
[#504]: https://github.com/tower-rs/tower/pull/504
[#492]: https://github.com/tower-rs/tower/pull/492
[#500]: https://github.com/tower-rs/tower/pull/500
[#490]: https://github.com/tower-rs/tower/pull/490
[#506]: https://github.com/tower-rs/tower/pull/506
[#508]: https://github.com/tower-rs/tower/pull/508
[#485]: https://github.com/tower-rs/tower/pull/485

# 0.3.1 (January 17, 2020)

- Allow opting out of tracing/log (#410).

# 0.3.0 (December 19, 2019)

- Update all tower based crates to `0.3`.
- Update to `tokio 0.2`
- Update to `futures 0.3`

# 0.3.0-alpha.2 (September 30, 2019)

- Move to `futures-*-preview 0.3.0-alpha.19`
- Move to `pin-project 0.4`

# 0.3.0-alpha.1a (September 13, 2019)

- Update `tower-buffer` to `0.3.0-alpha.1b`

# 0.3.0-alpha.1 (September 11, 2019)

- Move to `std::future`

# 0.1.1 (July 19, 2019)

- Add `ServiceBuilder::into_inner`

# 0.1.0 (April 26, 2019)

- Initial release
