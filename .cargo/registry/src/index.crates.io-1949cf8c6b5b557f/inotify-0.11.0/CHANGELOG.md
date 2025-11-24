# Changelog

## v0.11.0 (2024-08-19)

- Fix link in README ([#209])
- **Breaking change:** Make `bits` field of `EventMask`/`WatchMask` inaccessible. You can use the `.bits()` method instead. ([#211], [#218])
- Fix various links in documentation ([#213])
- Bump minimum supported Rust version (MSRV) to 1.70. ([#219])

[#209]: https://github.com/hannobraun/inotify-rs/pull/209
[#211]: https://github.com/hannobraun/inotify-rs/pull/211
[#213]: https://github.com/hannobraun/inotify-rs/pull/213
[#218]: https://github.com/hannobraun/inotify-rs/pull/218
[#219]: https://github.com/hannobraun/inotify-rs/pull/219


## v0.10.2 (2023-07-27)

- Fix broken links to `Watches` in documentation ([#205])

[#205]: https://github.com/hannobraun/inotify-rs/pull/205


## v0.10.1 (2023-06-07)

- Add `WatchDescriptor::get_watch_descriptor_id` ([#193])
- Add `Event::to_owned` ([#196])
- Deprecate `Event::into_owned` ([#196])
- Add `Watches`/`Inotify::watches`/`EventStream::watches` ([#197])
- Deprecate `Inotify::add_watch`/`Inotify::rm_watch` ([#197])
- Add `Inotify::into_event_stream`/`EventStream::into_inotify` ([#199])
- Deprecate `Inotify::event_stream` ([#199])
- Implement `AsFd` and bidirectional conversion to/from `OwnedFd` ([#202])
- Raise Minimum Supported Rust Version (MSRV) to 1.63.0 ([#202])

[#193]: https://github.com/hannobraun/inotify-rs/pull/193
[#196]: https://github.com/hannobraun/inotify-rs/pull/196
[#197]: https://github.com/hannobraun/inotify-rs/pull/197
[#199]: https://github.com/hannobraun/inotify-rs/pull/199
[#202]: https://github.com/hannobraun/inotify-rs/pull/202


## v0.10.0 (2021-12-07)

- **Breaking change:** Remove special handling of `WouldBlock` error ([#190])

[#190]: https://github.com/hannobraun/inotify-rs/pull/190


## v0.9.6 (2021-11-03)

- Fix build status badge in README ([#185])
- Add `get_buffer_size`/`get_absolute_path_buffer_size` ([#187])

[#185]: https://github.com/hannobraun/inotify-rs/pull/185
[#187]: https://github.com/hannobraun/inotify-rs/pull/187


## v0.9.5 (2021-10-07)

- Implement `Ord`/`PartialOrd` for `WatchDescriptor` ([#183])

[#183]: https://github.com/hannobraun/inotify-rs/pull/183


## v0.9.4 (2021-09-22)

- Make `Event::into_owned` always available ([#179])
- Implement missing `Debug` implementations ([#180])

[#179]: https://github.com/hannobraun/inotify-rs/pull/179
[#180]: https://github.com/hannobraun/inotify-rs/pull/180


## v0.9.3 (2021-05-12)

- Improve documentation ([#167], [#169])
- Add missing check for invalid file descriptor ([#168])
- Fix unsound use of buffers due to misalignment ([#171])
- Add missing error checks ([#173])

[#167]: https://github.com/hannobraun/inotify-rs/pull/167
[#168]: https://github.com/hannobraun/inotify-rs/pull/168
[#169]: https://github.com/hannobraun/inotify-rs/pull/169
[#171]: https://github.com/hannobraun/inotify-rs/pull/171
[#173]: https://github.com/hannobraun/inotify-rs/pull/173


## v0.9.2 (2020-12-30)

- Upgrade to Tokio 1.0 ([#165])

[#165]: https://github.com/hannobraun/inotify/pull/165


## v0.9.1 (2020-11-09)

- Fix take wake-up ([#161])

[#161]: https://github.com/hannobraun/inotify/pull/161


## v0.9.0 (2020-11-06)

- Update minimum supported Rust version to version 1.47 ([#154])
- Fix documentation: `Inotify::read_events` doesn't handle all events ([#157])
- Update to tokio 0.3 ([#158])

[#154]: https://github.com/hannobraun/inotify/pull/154
[#157]: https://github.com/hannobraun/inotify/pull/157
[#158]: https://github.com/hannobraun/inotify/pull/158


## v0.8.3 (2020-06-05)

- Avoid using `inotify_init1` ([#146])

[#146]: https://github.com/hannobraun/inotify/pull/146


## v0.8.2 (2020-01-25)

- Ensure file descriptor is closed on drop ([#140])

[#140]: https://github.com/inotify-rs/inotify/pull/140


## v0.8.1 (2020-01-23)

No changes, due to a mistake made while releasing this version.


## v0.8.0 (2019-12-04)

- Update to tokio 0.2 and futures 0.3 ([#134])

[#134]: https://github.com/inotify-rs/inotify/pull/134


## v0.7.1 (2020-06-05)

- backport: Avoid using `inotify_init1` ([#146])

[#146]: https://github.com/hannobraun/inotify/pull/146


## v0.7.0 (2019-02-09)

- Make stream API more flexible in regards to buffers ([ea3e7a394bf34a6ccce4f2136c0991fe7e8f1f42](ea3e7a394bf34a6ccce4f2136c0991fe7e8f1f42)) (breaking change)


## v0.6.1 (2018-08-28)

- Don't return spurious filenames ([2f37560f](2f37560f))


## v0.6.0 (2018-08-16)

- Handle closing of inotify instance better ([824160fe](824160fe))
- Implement `EventStream` using `mio` ([ba4cb8c7](ba4cb8c7))


## v0.5.1 (2018-02-27)

- Add future-based async API ([569e65a7](569e65a7), closes [#49](49))
