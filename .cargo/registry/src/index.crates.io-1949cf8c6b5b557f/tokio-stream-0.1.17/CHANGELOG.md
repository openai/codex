# 0.1.17 (December 6th, 2024)

- deps: fix dev-dependency on tokio-test ([#6931], [#7019])
- stream: fix link on `Peekable` ([#6861])
- sync: fix `Stream` link in broadcast docs ([#6873])

[#6861]: https://github.com/tokio-rs/tokio/pull/6861
[#6873]: https://github.com/tokio-rs/tokio/pull/6873
[#6931]: https://github.com/tokio-rs/tokio/pull/6931
[#7019]: https://github.com/tokio-rs/tokio/pull/7019

# 0.1.16 (September 5th, 2024)

This release bumps the MSRV of tokio-stream to 1.70.

- stream: add `next_many` and `poll_next_many` to `StreamMap` ([#6409])
- stream: make stream adapters public ([#6658])
- readme: add readme for tokio-stream ([#6456])

[#6409]: https://github.com/tokio-rs/tokio/pull/6409
[#6658]: https://github.com/tokio-rs/tokio/pull/6658
[#6456]: https://github.com/tokio-rs/tokio/pull/6456

# 0.1.15 (March 14th, 2024)

This release bumps the MSRV of tokio-stream to 1.63.

- docs: fix typo in argument name ([#6389])
- docs: fix typo in peekable docs ([#6130])
- docs: link to latest version of tokio-util docs ([#5694])
- docs: typographic improvements ([#6262])
- stream: add `StreamExt::peekable` ([#6095])

[#5694]: https://github.com/tokio-rs/tokio/pull/5694
[#6095]: https://github.com/tokio-rs/tokio/pull/6095
[#6130]: https://github.com/tokio-rs/tokio/pull/6130
[#6262]: https://github.com/tokio-rs/tokio/pull/6262
[#6389]: https://github.com/tokio-rs/tokio/pull/6389

# 0.1.14 (April 26th, 2023)

This bugfix release bumps the minimum version of Tokio to 1.15, which is
necessary for `timeout_repeating` to compile. ([#5657])

[#5657]: https://github.com/tokio-rs/tokio/pull/5657

# 0.1.13 (April 25th, 2023)

This release bumps the MSRV of tokio-stream to 1.56.

- stream: add "full" feature flag ([#5639])
- stream: add `StreamExt::timeout_repeating` ([#5577])
- stream: add `StreamNotifyClose` ([#4851])

[#4851]: https://github.com/tokio-rs/tokio/pull/4851
[#5577]: https://github.com/tokio-rs/tokio/pull/5577
[#5639]: https://github.com/tokio-rs/tokio/pull/5639

# 0.1.12 (January 20, 2023)

- time: remove `Unpin` bound on `Throttle` methods ([#5105])
- time: document that `throttle` operates on ms granularity ([#5101])
- sync: add `WatchStream::from_changes` ([#5432])

[#5105]: https://github.com/tokio-rs/tokio/pull/5105
[#5101]: https://github.com/tokio-rs/tokio/pull/5101
[#5432]: https://github.com/tokio-rs/tokio/pull/5432

# 0.1.11 (October 11, 2022)

- time: allow `StreamExt::chunks_timeout` outside of a runtime ([#5036])

[#5036]: https://github.com/tokio-rs/tokio/pull/5036

# 0.1.10 (Sept 18, 2022)

- time: add `StreamExt::chunks_timeout` ([#4695])
- stream: add track_caller to public APIs ([#4786])

[#4695]: https://github.com/tokio-rs/tokio/pull/4695
[#4786]: https://github.com/tokio-rs/tokio/pull/4786

# 0.1.9 (June 4, 2022)

- deps: upgrade `tokio-util` dependency to `0.7.x` ([#3762])
- stream: add `StreamExt::map_while` ([#4351])
- stream: add `StreamExt::then` ([#4355])
- stream: add cancel-safety docs to `StreamExt::next` and `try_next` ([#4715])
- stream: expose `Elapsed` error ([#4502])
- stream: expose `Timeout` ([#4601])
- stream: implement `Extend` for `StreamMap` ([#4272])
- sync: add `Clone` to `RecvError` types ([#4560])

[#3762]: https://github.com/tokio-rs/tokio/pull/3762
[#4272]: https://github.com/tokio-rs/tokio/pull/4272
[#4351]: https://github.com/tokio-rs/tokio/pull/4351
[#4355]: https://github.com/tokio-rs/tokio/pull/4355
[#4502]: https://github.com/tokio-rs/tokio/pull/4502
[#4560]: https://github.com/tokio-rs/tokio/pull/4560
[#4601]: https://github.com/tokio-rs/tokio/pull/4601
[#4715]: https://github.com/tokio-rs/tokio/pull/4715

# 0.1.8 (October 29, 2021)

- stream: add `From<Receiver<T>>` impl for receiver streams ([#4080])
- stream: impl `FromIterator` for `StreamMap` ([#4052])
- signal: make windows docs for signal module show up on unix builds ([#3770])

[#3770]: https://github.com/tokio-rs/tokio/pull/3770
[#4052]: https://github.com/tokio-rs/tokio/pull/4052
[#4080]: https://github.com/tokio-rs/tokio/pull/4080

# 0.1.7 (July 7, 2021)

### Fixed

- sync: fix watch wrapper ([#3914])
- time: fix `Timeout::size_hint` ([#3902])

[#3902]: https://github.com/tokio-rs/tokio/pull/3902
[#3914]: https://github.com/tokio-rs/tokio/pull/3914

# 0.1.6 (May 14, 2021)

### Added

- stream: implement `Error` and `Display` for `BroadcastStreamRecvError` ([#3745])

### Fixed

- stream: avoid yielding in `AllFuture` and `AnyFuture` ([#3625])

[#3745]: https://github.com/tokio-rs/tokio/pull/3745
[#3625]: https://github.com/tokio-rs/tokio/pull/3625

# 0.1.5 (March 20, 2021)

### Fixed

- stream: documentation note for throttle `Unpin` ([#3600])

[#3600]: https://github.com/tokio-rs/tokio/pull/3600

# 0.1.4 (March 9, 2021)

Added

- signal: add `Signal` wrapper ([#3510])

Fixed

- stream: remove duplicate `doc_cfg` declaration ([#3561])
- sync: yield initial value in `WatchStream` ([#3576])

[#3510]: https://github.com/tokio-rs/tokio/pull/3510
[#3561]: https://github.com/tokio-rs/tokio/pull/3561
[#3576]: https://github.com/tokio-rs/tokio/pull/3576

# 0.1.3 (February 5, 2021)

Added

 - sync: add wrapper for broadcast and watch ([#3384], [#3504])

[#3384]: https://github.com/tokio-rs/tokio/pull/3384
[#3504]: https://github.com/tokio-rs/tokio/pull/3504

# 0.1.2 (January 12, 2021)

Fixed

 - docs: fix some wrappers missing in documentation ([#3378])

[#3378]: https://github.com/tokio-rs/tokio/pull/3378

# 0.1.1 (January 4, 2021)

Added

 - add `Stream` wrappers ([#3343])

Fixed

 - move `async-stream` to `dev-dependencies` ([#3366])

[#3366]: https://github.com/tokio-rs/tokio/pull/3366
[#3343]: https://github.com/tokio-rs/tokio/pull/3343

# 0.1.0 (December 23, 2020)

 - Initial release
