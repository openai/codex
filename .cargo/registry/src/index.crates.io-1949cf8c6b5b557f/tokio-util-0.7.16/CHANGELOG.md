# 0.7.16 (August 3rd, 2025)

### Added

- codec: add `FramedWrite::with_capacity` ([#7493])
- future: add adapters of `CancellationToken` for `FutureExt` ([#7475])
- sync: add `DropGuardRef` for `CancellationToken` ([#7407])
- task: add `AbortOnDropHandle::detach` ([#7400])
- task: stabilise `JoinMap` ([#7075])

### Changed

- codec: also apply capacity to read buffer in `Framed::with_capacity` ([#7500])
- sync: make `CancellationToken::run_until_cancelled` biased towards the token ([#7462])
- task: remove raw-entry feature from hashbrown dep ([#7252])

### Documented

- compat: add more documentation to `tokio_util::compat` ([#7279])
- sync: improve docs of `tokio_util::sync::CancellationToken` ([#7408])

[#7075]: https://github.com/tokio-rs/tokio/pull/7075
[#7252]: https://github.com/tokio-rs/tokio/pull/7252
[#7279]: https://github.com/tokio-rs/tokio/pull/7279
[#7400]: https://github.com/tokio-rs/tokio/pull/7400
[#7407]: https://github.com/tokio-rs/tokio/pull/7407
[#7408]: https://github.com/tokio-rs/tokio/pull/7408
[#7462]: https://github.com/tokio-rs/tokio/pull/7462
[#7475]: https://github.com/tokio-rs/tokio/pull/7475
[#7493]: https://github.com/tokio-rs/tokio/pull/7493
[#7500]: https://github.com/tokio-rs/tokio/pull/7500

# 0.7.15 (April 23rd, 2025)

### Fixed

- task: properly handle removed entries in `JoinMap` ([#7264])

### Updated

- deps: update hashbrown to 0.15 ([#7219])

### Documented

- task: explicitly state that `TaskTracker` does not abort tasks on Drop ([#7223])

[#7219]: https://github.com/tokio-rs/tokio/pull/7219
[#7223]: https://github.com/tokio-rs/tokio/pull/7223
[#7264]: https://github.com/tokio-rs/tokio/pull/7264

# 0.7.14 (March 12th, 2025)

### Added

- io: add `get_ref` and `get_mut` for `SyncIoBridge` ([#7128])
- io: add `read_exact_arc` ([#7165])
- sync: add `CancellationToken::run_until_cancelled_owned` ([#7081])

### Changed

- codec: optimize buffer reserve for `AnyDelimiterCodec::encode` ([#7188])
- either: enable `Either` to use underlying `AsyncWrite` implementation ([#7025])

### Fixed

- codec: fix typo in API docs ([#7044])
- util: fix example in `StreamReader` docs ([#7167])

### Documented

- io: add docs for `SyncIoBridge` with examples and alternatives ([#6815])

### Internal

- io: clean up buffer casts ([#7142])
- task: run `spawn_pinned` tests with miri ([#7023])

[#6815]: https://github.com/tokio-rs/tokio/pull/6815
[#7023]: https://github.com/tokio-rs/tokio/pull/7023
[#7025]: https://github.com/tokio-rs/tokio/pull/7025
[#7044]: https://github.com/tokio-rs/tokio/pull/7044
[#7081]: https://github.com/tokio-rs/tokio/pull/7081
[#7128]: https://github.com/tokio-rs/tokio/pull/7128
[#7142]: https://github.com/tokio-rs/tokio/pull/7142
[#7165]: https://github.com/tokio-rs/tokio/pull/7165
[#7167]: https://github.com/tokio-rs/tokio/pull/7167
[#7188]: https://github.com/tokio-rs/tokio/pull/7188

# 0.7.13 (December 4th, 2024)

### Fixed

- codec: fix incorrect handling of invalid utf-8 in `LinesCodec::decode_eof` ([#7011])

[#7011]: https://github.com/tokio-rs/tokio/pull/7011

# 0.7.12 (September 5th, 2024)

This release bumps the MSRV to 1.70. ([#6645])

### Added
- sync: Add `run_until_cancelled` to `tokio_util::sync::CancellationToken` ([#6618])
- task: add `AbortOnDropHandle` type ([#6786])

### Changed
- deps: no default features for hashbrown ([#6541])
- time: wake `DelayQueue` when removing last item ([#6752])
- deps: enable the full feature when compiled for the playground ([#6818])

### Documented
- task: fix typo in `TaskTracker` docs ([#6792])

[#6645]: https://github.com/tokio-rs/tokio/pull/6645
[#6541]: https://github.com/tokio-rs/tokio/pull/6541
[#6618]: https://github.com/tokio-rs/tokio/pull/6618
[#6752]: https://github.com/tokio-rs/tokio/pull/6752
[#6786]: https://github.com/tokio-rs/tokio/pull/6786
[#6792]: https://github.com/tokio-rs/tokio/pull/6792
[#6818]: https://github.com/tokio-rs/tokio/pull/6818

# 0.7.11 (May 4th, 2024)

This release updates the MSRV to 1.63. ([#6126])

### Added

- either: implement `Sink` for `Either` ([#6239])
- time: add `DelayQueue::deadline` ([#6163])
- time: add `FutureExt::timeout` ([#6276])

### Changed

- codec: assert compatibility between `LengthDelimitedCodec` options ([#6414])
- codec: make tracing feature optional for codecs ([#6434])
- io: add `T: ?Sized` to `tokio_util::io::poll_read_buf` ([#6441])
- sync: remove `'static` bound on `impl Sink for PollSender` ([#6397])

### Documented

- codec: add examples for `FramedRead` and `FramedWrite` ([#6310])
- codec: document cancel safety of `SinkExt::send` and `StreamExt::next` ([#6417])

[#6126]: https://github.com/tokio-rs/tokio/pull/6126
[#6163]: https://github.com/tokio-rs/tokio/pull/6163
[#6239]: https://github.com/tokio-rs/tokio/pull/6239
[#6276]: https://github.com/tokio-rs/tokio/pull/6276
[#6310]: https://github.com/tokio-rs/tokio/pull/6310
[#6397]: https://github.com/tokio-rs/tokio/pull/6397
[#6414]: https://github.com/tokio-rs/tokio/pull/6414
[#6417]: https://github.com/tokio-rs/tokio/pull/6417
[#6434]: https://github.com/tokio-rs/tokio/pull/6434
[#6441]: https://github.com/tokio-rs/tokio/pull/6441

# 0.7.10 (October 24th, 2023)

### Added

- task: add `TaskTracker` ([#6033])
- task: add `JoinMap::keys` ([#6046])
- io: implement `Seek` for `SyncIoBridge` ([#6058])

### Changed

- deps: update hashbrown to 0.14 ([#6102])

[#6033]: https://github.com/tokio-rs/tokio/pull/6033
[#6046]: https://github.com/tokio-rs/tokio/pull/6046
[#6058]: https://github.com/tokio-rs/tokio/pull/6058
[#6102]: https://github.com/tokio-rs/tokio/pull/6102

# 0.7.9 (September 20th, 2023)

### Added

- io: add passthrough `AsyncRead`/`AsyncWrite` to `InspectWriter`/`InspectReader` ([#5739])
- task: add spawn blocking methods to `JoinMap` ([#5797])
- io: pass through traits for `StreamReader` and `SinkWriter` ([#5941])
- io: add `SyncIoBridge::into_inner` ([#5971])

### Fixed

- sync: handle possibly dangling reference safely ([#5812])
- util: fix broken intra-doc link ([#5849])
- compat: fix clippy warnings ([#5891])

### Documented

- codec: Specify the line ending of `LinesCodec` ([#5982])

[#5739]: https://github.com/tokio-rs/tokio/pull/5739
[#5797]: https://github.com/tokio-rs/tokio/pull/5797
[#5941]: https://github.com/tokio-rs/tokio/pull/5941
[#5971]: https://github.com/tokio-rs/tokio/pull/5971
[#5812]: https://github.com/tokio-rs/tokio/pull/5812
[#5849]: https://github.com/tokio-rs/tokio/pull/5849
[#5891]: https://github.com/tokio-rs/tokio/pull/5891
[#5982]: https://github.com/tokio-rs/tokio/pull/5982

# 0.7.8 (April 25th, 2023)

This release bumps the MSRV of tokio-util to 1.56.

### Added

- time: add `DelayQueue::peek` ([#5569])

### Changed

This release contains one performance improvement:

- sync: try to lock the parent first in `CancellationToken` ([#5561])

### Fixed

- time: fix panic in `DelayQueue` ([#5630])

### Documented

- sync: improve `CancellationToken` doc on child tokens ([#5632])

[#5561]: https://github.com/tokio-rs/tokio/pull/5561
[#5569]: https://github.com/tokio-rs/tokio/pull/5569
[#5630]: https://github.com/tokio-rs/tokio/pull/5630
[#5632]: https://github.com/tokio-rs/tokio/pull/5632

# 0.7.7 (February 12th, 2023)

This release reverts the removal of the `Encoder` bound on the `FramedParts`
constructor from [#5280] since it turned out to be a breaking change. ([#5450])

[#5450]: https://github.com/tokio-rs/tokio/pull/5450

# 0.7.6 (February 10, 2023)

This release fixes a compilation failure in 0.7.5 when it is used together with
Tokio version 1.21 and unstable features are enabled. ([#5445])

[#5445]: https://github.com/tokio-rs/tokio/pull/5445

# 0.7.5 (February 9, 2023)

This release fixes an accidental breaking change where `UnwindSafe` was
accidentally removed from `CancellationToken`.

### Added
- codec: add `Framed::backpressure_boundary` ([#5124])
- io: add `InspectReader` and `InspectWriter` ([#5033])
- io: add `tokio_util::io::{CopyToBytes, SinkWriter}` ([#5070], [#5436])
- io: impl `std::io::BufRead` on `SyncIoBridge` ([#5265])
- sync: add `PollSemaphore::poll_acquire_many` ([#5137])
- sync: add owned future for `CancellationToken` ([#5153])
- time: add `DelayQueue::try_remove` ([#5052])

### Fixed
- codec: fix `LengthDelimitedCodec` buffer over-reservation ([#4997])
- sync: impl `UnwindSafe` on `CancellationToken` ([#5438])
- util: remove `Encoder` bound on `FramedParts` constructor ([#5280])

### Documented
- io: add lines example for `StreamReader` ([#5145])

[#4997]: https://github.com/tokio-rs/tokio/pull/4997
[#5033]: https://github.com/tokio-rs/tokio/pull/5033
[#5052]: https://github.com/tokio-rs/tokio/pull/5052
[#5070]: https://github.com/tokio-rs/tokio/pull/5070
[#5124]: https://github.com/tokio-rs/tokio/pull/5124
[#5137]: https://github.com/tokio-rs/tokio/pull/5137
[#5145]: https://github.com/tokio-rs/tokio/pull/5145
[#5153]: https://github.com/tokio-rs/tokio/pull/5153
[#5265]: https://github.com/tokio-rs/tokio/pull/5265
[#5280]: https://github.com/tokio-rs/tokio/pull/5280
[#5436]: https://github.com/tokio-rs/tokio/pull/5436
[#5438]: https://github.com/tokio-rs/tokio/pull/5438

# 0.7.4 (September 8, 2022)

### Added

- io: add `SyncIoBridge::shutdown()` ([#4938])
- task: improve `LocalPoolHandle` ([#4680])

### Fixed

- util: add `track_caller` to public APIs ([#4785])

### Unstable

- task: fix compilation errors in `JoinMap` with Tokio v1.21.0 ([#4755])
- task: remove the unstable, deprecated `JoinMap::join_one` ([#4920])

[#4680]: https://github.com/tokio-rs/tokio/pull/4680
[#4755]: https://github.com/tokio-rs/tokio/pull/4755
[#4785]: https://github.com/tokio-rs/tokio/pull/4785
[#4920]: https://github.com/tokio-rs/tokio/pull/4920
[#4938]: https://github.com/tokio-rs/tokio/pull/4938

# 0.7.3 (June 4, 2022)

### Changed

- tracing: don't require default tracing features ([#4592])
- util: simplify implementation of `ReusableBoxFuture` ([#4675])

### Added (unstable)

- task: add `JoinMap` ([#4640], [#4697])

[#4592]: https://github.com/tokio-rs/tokio/pull/4592
[#4640]: https://github.com/tokio-rs/tokio/pull/4640
[#4675]: https://github.com/tokio-rs/tokio/pull/4675
[#4697]: https://github.com/tokio-rs/tokio/pull/4697

# 0.7.2 (May 14, 2022)

This release contains a rewrite of `CancellationToken` that fixes a memory leak. ([#4652])

[#4652]: https://github.com/tokio-rs/tokio/pull/4652

# 0.7.1 (February 21, 2022)

### Added

- codec: add `length_field_type` to `LengthDelimitedCodec` builder ([#4508])
- io: add `StreamReader::into_inner_with_chunk()` ([#4559])

### Changed

- switch from log to tracing ([#4539])

### Fixed

- sync: fix waker update condition in `CancellationToken` ([#4497])
- bumped tokio dependency to 1.6 to satisfy minimum requirements ([#4490])

[#4490]: https://github.com/tokio-rs/tokio/pull/4490
[#4497]: https://github.com/tokio-rs/tokio/pull/4497
[#4508]: https://github.com/tokio-rs/tokio/pull/4508
[#4539]: https://github.com/tokio-rs/tokio/pull/4539
[#4559]: https://github.com/tokio-rs/tokio/pull/4559

# 0.7.0 (February 9, 2022)

### Added

- task: add `spawn_pinned` ([#3370])
- time: add `shrink_to_fit` and `compact` methods to `DelayQueue` ([#4170])
- codec: improve `Builder::max_frame_length` docs ([#4352])
- codec: add mutable reference getters for codecs to pinned `Framed` ([#4372])
- net: add generic trait to combine `UnixListener` and `TcpListener` ([#4385])
- codec: implement `Framed::map_codec` ([#4427])
- codec: implement `Encoder<BytesMut>` for `BytesCodec` ([#4465])

### Changed

- sync: add lifetime parameter to `ReusableBoxFuture` ([#3762])
- sync: refactored `PollSender<T>` to fix a subtly broken `Sink<T>` implementation ([#4214])
- time: remove error case from the infallible `DelayQueue::poll_elapsed` ([#4241])

[#3370]: https://github.com/tokio-rs/tokio/pull/3370
[#4170]: https://github.com/tokio-rs/tokio/pull/4170
[#4352]: https://github.com/tokio-rs/tokio/pull/4352
[#4372]: https://github.com/tokio-rs/tokio/pull/4372
[#4385]: https://github.com/tokio-rs/tokio/pull/4385
[#4427]: https://github.com/tokio-rs/tokio/pull/4427
[#4465]: https://github.com/tokio-rs/tokio/pull/4465
[#3762]: https://github.com/tokio-rs/tokio/pull/3762
[#4214]: https://github.com/tokio-rs/tokio/pull/4214
[#4241]: https://github.com/tokio-rs/tokio/pull/4241

# 0.6.10 (May 14, 2021)

This is a backport for the memory leak in `CancellationToken` that was originally fixed in 0.7.2. ([#4652])

[#4652]: https://github.com/tokio-rs/tokio/pull/4652

# 0.6.9 (October 29, 2021)

### Added

- codec: implement `Clone` for `LengthDelimitedCodec` ([#4089])
- io: add `SyncIoBridge`  ([#4146])

### Fixed

- time: update deadline on removal in `DelayQueue` ([#4178])
- codec: Update stream impl for Framed to return None after Err ([#4166])

[#4089]: https://github.com/tokio-rs/tokio/pull/4089
[#4146]: https://github.com/tokio-rs/tokio/pull/4146
[#4166]: https://github.com/tokio-rs/tokio/pull/4166
[#4178]: https://github.com/tokio-rs/tokio/pull/4178

# 0.6.8 (September 3, 2021)

### Added

- sync: add drop guard for `CancellationToken` ([#3839])
- compact: added `AsyncSeek` compat ([#4078])
- time: expose `Key` used in `DelayQueue`'s `Expired` ([#4081])
- io: add `with_capacity` to `ReaderStream` ([#4086])

### Fixed

- codec: remove unnecessary `doc(cfg(...))` ([#3989])

[#3839]: https://github.com/tokio-rs/tokio/pull/3839
[#4078]: https://github.com/tokio-rs/tokio/pull/4078
[#4081]: https://github.com/tokio-rs/tokio/pull/4081
[#4086]: https://github.com/tokio-rs/tokio/pull/4086
[#3989]: https://github.com/tokio-rs/tokio/pull/3989

# 0.6.7 (May 14, 2021)

### Added

- udp: make `UdpFramed` take `Borrow<UdpSocket>` ([#3451])
- compat: implement `AsRawFd`/`AsRawHandle` for `Compat<T>` ([#3765])

[#3451]: https://github.com/tokio-rs/tokio/pull/3451
[#3765]: https://github.com/tokio-rs/tokio/pull/3765

# 0.6.6 (April 12, 2021)

### Added

- util: makes `Framed` and `FramedStream` resumable after eof ([#3272])
- util: add `PollSemaphore::{add_permits, available_permits}` ([#3683])

### Fixed

- chore: avoid allocation if `PollSemaphore` is unused ([#3634])

[#3272]: https://github.com/tokio-rs/tokio/pull/3272
[#3634]: https://github.com/tokio-rs/tokio/pull/3634
[#3683]: https://github.com/tokio-rs/tokio/pull/3683

# 0.6.5 (March 20, 2021)

### Fixed

- util: annotate time module as requiring `time` feature ([#3606])

[#3606]: https://github.com/tokio-rs/tokio/pull/3606

# 0.6.4 (March 9, 2021)

### Added

- codec: `AnyDelimiter` codec ([#3406])
- sync: add pollable `mpsc::Sender` ([#3490])

### Fixed

- codec: `LinesCodec` should only return `MaxLineLengthExceeded` once per line ([#3556])
- sync: fuse PollSemaphore ([#3578])

[#3406]: https://github.com/tokio-rs/tokio/pull/3406
[#3490]: https://github.com/tokio-rs/tokio/pull/3490
[#3556]: https://github.com/tokio-rs/tokio/pull/3556
[#3578]: https://github.com/tokio-rs/tokio/pull/3578

# 0.6.3 (January 31, 2021)

### Added

- sync: add `ReusableBoxFuture` utility ([#3464])

### Changed

- sync: use `ReusableBoxFuture` for `PollSemaphore` ([#3463])
- deps: remove `async-stream` dependency ([#3463])
- deps: remove `tokio-stream` dependency ([#3487])

# 0.6.2 (January 21, 2021)

### Added

- sync: add pollable `Semaphore` ([#3444])

### Fixed

- time: fix panics on updating `DelayQueue` entries ([#3270])

# 0.6.1 (January 12, 2021)

### Added

- codec: `get_ref()`, `get_mut()`, `get_pin_mut()` and `into_inner()` for
  `Framed`, `FramedRead`, `FramedWrite` and `StreamReader` ([#3364]).
- codec: `write_buffer()` and `write_buffer_mut()` for `Framed` and
  `FramedWrite` ([#3387]).

# 0.6.0 (December 23, 2020)

### Changed
- depend on `tokio` 1.0.

### Added
- rt: add constructors to `TokioContext` (#3221).

# 0.5.1 (December 3, 2020)

### Added
- io: `poll_read_buf` util fn (#2972).
- io: `poll_write_buf` util fn with vectored write support (#3156).

# 0.5.0 (October 30, 2020)

### Changed
- io: update `bytes` to 0.6 (#3071).

# 0.4.0 (October 15, 2020)

### Added
- sync: `CancellationToken` for coordinating task cancellation (#2747).
- rt: `TokioContext` sets the Tokio runtime for the duration of a future (#2791)
- io: `StreamReader`/`ReaderStream` map between `AsyncRead` values and `Stream`
  of bytes (#2788).
- time: `DelayQueue` to manage many delays (#2897).

# 0.3.1 (March 18, 2020)

### Fixed

- Adjust minimum-supported Tokio version to v0.2.5 to account for an internal
  dependency on features in that version of Tokio. ([#2326])

# 0.3.0 (March 4, 2020)

### Changed

- **Breaking Change**: Change `Encoder` trait to take a generic `Item` parameter, which allows
  codec writers to pass references into `Framed` and `FramedWrite` types. ([#1746])

### Added

- Add futures-io/tokio::io compatibility layer. ([#2117])
- Add `Framed::with_capacity`. ([#2215])

### Fixed

- Use advance over split_to when data is not needed. ([#2198])

# 0.2.0 (November 26, 2019)

- Initial release

[#3487]: https://github.com/tokio-rs/tokio/pull/3487
[#3464]: https://github.com/tokio-rs/tokio/pull/3464
[#3463]: https://github.com/tokio-rs/tokio/pull/3463
[#3444]: https://github.com/tokio-rs/tokio/pull/3444
[#3387]: https://github.com/tokio-rs/tokio/pull/3387
[#3364]: https://github.com/tokio-rs/tokio/pull/3364
[#3270]: https://github.com/tokio-rs/tokio/pull/3270
[#2326]: https://github.com/tokio-rs/tokio/pull/2326
[#2215]: https://github.com/tokio-rs/tokio/pull/2215
[#2198]: https://github.com/tokio-rs/tokio/pull/2198
[#2117]: https://github.com/tokio-rs/tokio/pull/2117
[#1746]: https://github.com/tokio-rs/tokio/pull/1746
