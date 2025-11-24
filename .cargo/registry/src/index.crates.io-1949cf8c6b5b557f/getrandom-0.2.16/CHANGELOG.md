# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.16] - 2025-04-22
### Added
- Cygwin support (backport of [#626]) [#654]

[#626]: https://github.com/rust-random/getrandom/pull/626
[#654]: https://github.com/rust-random/getrandom/pull/654

## [0.2.15] - 2024-05-06
### Added
- Apple visionOS support [#410]

### Changed
- Use `libc::getrandom` on DragonflyBSD, FreeBSD, illumos, and Solaris [#411] [#416] [#417] [#420]
- Unify `libc::getentropy`-based implementations [#418]

[#410]: https://github.com/rust-random/getrandom/pull/410
[#411]: https://github.com/rust-random/getrandom/pull/411
[#416]: https://github.com/rust-random/getrandom/pull/416
[#417]: https://github.com/rust-random/getrandom/pull/417
[#418]: https://github.com/rust-random/getrandom/pull/418
[#420]: https://github.com/rust-random/getrandom/pull/420

## [0.2.14] - 2024-04-08
### Fixed
- Enable `/dev/urandom` fallback for MUSL-based Linux targets [#408]

[#408]: https://github.com/rust-random/getrandom/pull/408

## [0.2.13] - 2024-04-06
### Added
- `linux_disable_fallback` crate feature to disable `/dev/urandom`-based fallback on Linux and
  Android targets. Enabling this feature bumps minimum supported Linux kernel version to 3.17 and
  Android API level to 23 (Marshmallow). [#396]

### Changed
- Disable `/dev/urandom` fallback for Linux targets outside of the following `target_arch`es:
  `aarch64`, `arm`, `powerpc`, `powerpc64`, `s390x`, `x86`, `x86_64` [#396]
- Do not catch `EPERM` error code on Android while checking availability of
  the `getrandom` syscall [#396]

[#396]: https://github.com/rust-random/getrandom/pull/396

## [0.2.12] - 2024-01-09
### Fixed
- Custom backend for targets without atomics [#385]

### Changed
- Improve robustness of the Hermit backend and `sys_fill_exact` [#386]
- Raise minimum supported Apple OS versions to macOS 10.12 and iOS 10 [#388]

### Added
- Document platform support policy [#387]

[#385]: https://github.com/rust-random/getrandom/pull/385
[#386]: https://github.com/rust-random/getrandom/pull/386
[#387]: https://github.com/rust-random/getrandom/pull/387
[#388]: https://github.com/rust-random/getrandom/pull/388

## [0.2.11] - 2023-11-08
### Added
- GNU/Hurd support [#370]

### Changed
- Renamed `__getrandom_internal` to `__GETRANDOM_INTERNAL`  [#369]
- Updated link to Hermit docs [#374]

[#369]: https://github.com/rust-random/getrandom/pull/369
[#370]: https://github.com/rust-random/getrandom/pull/370
[#374]: https://github.com/rust-random/getrandom/pull/374

## [0.2.10] - 2023-06-06
### Added
- Support for PS Vita (`armv7-sony-vita-newlibeabihf`) [#359]

### Changed
- Use getentropy from libc on Emscripten targets [#362]

[#359]: https://github.com/rust-random/getrandom/pull/359
[#362]: https://github.com/rust-random/getrandom/pull/362

## [0.2.9] - 2023-04-06
### Added
- AIX support [#282]
- `getrandom_uninit` function [#291]
- `wasm64-unknown-unknown` support [#303]
- tvOS and watchOS support [#317]
- QNX/nto support [#325]
- Support for `getrandom` syscall on NetBSD ≥ 10.0 [#331]
- `RtlGenRandom` fallback for non-UWP Windows [#337]

### Breaking Changes
- Update MSRV to 1.36 [#291]

### Fixed
- Solaris/OpenBSD/Dragonfly build [#301]

### Changed
- Update MSRV to 1.36 [#291]
- Use getentropy on Emscripten [#307]
- Solaris: consistantly use `/dev/random` source [#310]
- Move 3ds selection above rdrand/js/custom fallback [#312]
- Remove buffer zeroing from Node.js implementation [#315]
- Use `open` instead of `open64` [#326]
- Remove #cfg from bsd_arandom.rs [#332]
- Hermit: use `sys_read_entropy` syscall [#333]
- Eliminate potential panic in sys_fill_exact [#334]
- rdrand: Remove checking for 0 and !0 and instead check CPU family and do a self-test [#335]
- Move `__getrandom_custom` definition into a const block [#344]
- Switch the custom backend to Rust ABI [#347]

[#282]: https://github.com/rust-random/getrandom/pull/282
[#291]: https://github.com/rust-random/getrandom/pull/291
[#301]: https://github.com/rust-random/getrandom/pull/301
[#303]: https://github.com/rust-random/getrandom/pull/303
[#307]: https://github.com/rust-random/getrandom/pull/307
[#310]: https://github.com/rust-random/getrandom/pull/310
[#312]: https://github.com/rust-random/getrandom/pull/312
[#315]: https://github.com/rust-random/getrandom/pull/315
[#317]: https://github.com/rust-random/getrandom/pull/317
[#325]: https://github.com/rust-random/getrandom/pull/325
[#326]: https://github.com/rust-random/getrandom/pull/326
[#331]: https://github.com/rust-random/getrandom/pull/331
[#332]: https://github.com/rust-random/getrandom/pull/332
[#333]: https://github.com/rust-random/getrandom/pull/333
[#334]: https://github.com/rust-random/getrandom/pull/334
[#335]: https://github.com/rust-random/getrandom/pull/335
[#337]: https://github.com/rust-random/getrandom/pull/337
[#344]: https://github.com/rust-random/getrandom/pull/344
[#347]: https://github.com/rust-random/getrandom/pull/347

## [0.2.8] - 2022-10-20
### Changed
- The [Web Cryptography API] will now be preferred on `wasm32-unknown-unknown`
  when using the `"js"` feature, even on Node.js [#284] [#295]

### Added
- Added benchmarks to track buffer initialization cost [#272]

### Fixed
- Use `$crate` in `register_custom_getrandom!` [#270]

### Documentation
- Add information about enabling `"js"` feature [#280]
- Fix link to `wasm-bindgen` [#278]
- Document the varied implementations for underlying randomness sources [#276]

[Web Cryptography API]: https://developer.mozilla.org/en-US/docs/Web/API/Web_Crypto_API
[#284]: https://github.com/rust-random/getrandom/pull/284
[#295]: https://github.com/rust-random/getrandom/pull/295
[#272]: https://github.com/rust-random/getrandom/pull/272
[#270]: https://github.com/rust-random/getrandom/pull/270
[#280]: https://github.com/rust-random/getrandom/pull/280
[#278]: https://github.com/rust-random/getrandom/pull/278
[#276]: https://github.com/rust-random/getrandom/pull/276

## [0.2.7] - 2022-06-14
### Changed
- Update `wasi` dependency to `0.11` [#253]

### Fixed
- Use `AtomicPtr` instead of `AtomicUsize` for Strict Provenance compatibility. [#263]

### Documentation
- Add comments explaining use of fallback mechanisms [#257] [#260]

[#263]: https://github.com/rust-random/getrandom/pull/263
[#260]: https://github.com/rust-random/getrandom/pull/260
[#253]: https://github.com/rust-random/getrandom/pull/253
[#257]: https://github.com/rust-random/getrandom/pull/257

## [0.2.6] - 2022-03-28
### Added
- Nintendo 3DS (`armv6k-nintendo-3ds`) support [#248]

### Changed
- Retry `open` when interrupted [#252]

[#248]: https://github.com/rust-random/getrandom/pull/248
[#252]: https://github.com/rust-random/getrandom/pull/252

## [0.2.5] - 2022-02-22
### Added
- ESP-IDF targets (`*‑espidf`) support [#245]

### Fixed
- Webpack warning caused by dynamic require [#234]
- Error checking on iOS for `SecRandomCopyBytes` [#244]

[#234]: https://github.com/rust-random/getrandom/pull/234
[#244]: https://github.com/rust-random/getrandom/pull/244
[#245]: https://github.com/rust-random/getrandom/pull/245

## [0.2.4] - 2021-12-13
### Changed
- Use explicit imports in the `js` backend [#220]
- Use `/dev/urandom` on Redox instead of `rand:` [#222]
- Use `NonZeroU32::new_unchecked` to convert wasi error [#233]

### Added
- SOLID targets (`*-kmc-solid_*`) support [#235]
- Limited Hermit (`x86_64-unknown-hermit`) support [#236]

[#220]: https://github.com/rust-random/getrandom/pull/220
[#222]: https://github.com/rust-random/getrandom/pull/222
[#233]: https://github.com/rust-random/getrandom/pull/233
[#235]: https://github.com/rust-random/getrandom/pull/235
[#236]: https://github.com/rust-random/getrandom/pull/236

## [0.2.3] - 2021-04-10
### Changed
- Replace build.rs with link attributes. [#205]
- Add support for getrandom syscall on DragonFly BSD. [#210]
- Improve Node.js detection. [#215]

[#205]: https://github.com/rust-random/getrandom/pull/205
[#210]: https://github.com/rust-random/getrandom/pull/210
[#215]: https://github.com/rust-random/getrandom/pull/215

## [0.2.2] - 2021-01-19
### Changed
- Forward `rustc-dep-of-std` to dependencies. [#198]
- Highlight feature-dependent functionality in documentation using the `doc_cfg` feature. [#200]

[#198]: https://github.com/rust-random/getrandom/pull/198
[#200]: https://github.com/rust-random/getrandom/pull/200

## [0.2.1] - 2021-01-03
### Changed
- Update `cfg-if` to v1.0. [#166]
- Update `wasi` to v0.10. [#167]

### Fixed
- Multithreaded WASM support. [#165]

### Removed
- Windows XP support. [#177]
- Direct `stdweb` support. [#178]
- CloudABI support. [#184]

[#165]: https://github.com/rust-random/getrandom/pull/165
[#166]: https://github.com/rust-random/getrandom/pull/166
[#167]: https://github.com/rust-random/getrandom/pull/167
[#177]: https://github.com/rust-random/getrandom/pull/177
[#178]: https://github.com/rust-random/getrandom/pull/178
[#184]: https://github.com/rust-random/getrandom/pull/184

## [0.2.0] - 2020-09-10
### Features for using getrandom on unsupported targets

The following (off by default) Cargo features have been added:
- `"rdrand"` - use the RDRAND instruction on `no_std` `x86`/`x86_64` targets [#133]
- `"js"` - use JavaScript calls on `wasm32-unknown-unknown` [#149]
  - Replaces the `stdweb` and `wasm-bindgen` features (which are removed)
- `"custom"` - allows a user to specify a custom implementation [#109]

### Breaking Changes
- Unsupported targets no longer compile [#107]
- Change/Add `Error` constants [#120]
- Only impl `std` traits when the `"std"` Cargo feature is specified [#106]
- Remove official support for Hermit, L4Re, and UEFI [#133]
- Remove optional `"log"` dependency [#131]
- Update minimum supported Linux kernel to 2.6.32 [#153]
- Update MSRV to 1.34 [#159]

[#106]: https://github.com/rust-random/getrandom/pull/106
[#107]: https://github.com/rust-random/getrandom/pull/107
[#109]: https://github.com/rust-random/getrandom/pull/109
[#120]: https://github.com/rust-random/getrandom/pull/120
[#131]: https://github.com/rust-random/getrandom/pull/131
[#133]: https://github.com/rust-random/getrandom/pull/133
[#149]: https://github.com/rust-random/getrandom/pull/149
[#153]: https://github.com/rust-random/getrandom/pull/153
[#159]: https://github.com/rust-random/getrandom/pull/159

## [0.1.16] - 2020-12-31
### Changed
- Update `cfg-if` to v1.0. [#173]
- Implement `std::error::Error` for the `Error` type on additional targets. [#169]

### Fixed
- Multithreaded WASM support. [#171]

[#173]: https://github.com/rust-random/getrandom/pull/173
[#171]: https://github.com/rust-random/getrandom/pull/171
[#169]: https://github.com/rust-random/getrandom/pull/169

## [0.1.15] - 2020-09-10
### Changed
- Added support for Internet Explorer 11 [#139]
- Fix Webpack require warning with `wasm-bindgen` [#137]

[#137]: https://github.com/rust-random/getrandom/pull/137
[#139]: https://github.com/rust-random/getrandom/pull/139

## [0.1.14] - 2020-01-07
### Changed
- Remove use of spin-locks in the `use_file` module. [#125]
- Update `wasi` to v0.9. [#126]
- Do not read errno value on DragonFlyBSD to fix compilation failure. [#129]

[#125]: https://github.com/rust-random/getrandom/pull/125
[#126]: https://github.com/rust-random/getrandom/pull/126
[#129]: https://github.com/rust-random/getrandom/pull/129

## [0.1.13] - 2019-08-25
### Added
- VxWorks targets support. [#86]

### Changed
- If zero-length slice is passed to the `getrandom` function, always return
`Ok(())` immediately without doing any calls to the underlying operating
system. [#104]
- Use the `kern.arandom` sysctl on NetBSD. [#115]

### Fixed
- Bump `cfg-if` minimum version from 0.1.0 to 0.1.2. [#112]
- Typos and bad doc links. [#117]

[#86]: https://github.com/rust-random/getrandom/pull/86
[#104]: https://github.com/rust-random/getrandom/pull/104
[#112]: https://github.com/rust-random/getrandom/pull/112
[#115]: https://github.com/rust-random/getrandom/pull/115
[#117]: https://github.com/rust-random/getrandom/pull/117

## [0.1.12] - 2019-08-18
### Changed
- Update wasi dependency from v0.5 to v0.7. [#100]

[#100]: https://github.com/rust-random/getrandom/pull/100

## [0.1.11] - 2019-08-25
### Fixed
- Implement `std`-dependent traits for selected targets even if `std`
feature is disabled. (backward compatibility with v0.1.8) [#96]

[#96]: https://github.com/rust-random/getrandom/pull/96

## [0.1.10] - 2019-08-18 [YANKED]
### Changed
- Use the dummy implementation on `wasm32-unknown-unknown` even with the
disabled `dummy` feature. [#90]

### Fixed
- Fix CSP error for `wasm-bindgen`. [#92]

[#90]: https://github.com/rust-random/getrandom/pull/90
[#92]: https://github.com/rust-random/getrandom/pull/92

## [0.1.9] - 2019-08-14 [YANKED]
### Changed
- Remove `std` dependency for opening and reading files. [#58]
- Use `wasi` instead of `libc` on WASI target. [#64]
- By default emit a compile-time error when built for an unsupported target.
This behaviour can be disabled by using the `dummy` feature. [#71]

### Added
- Add support for UWP targets. [#69]
- Add unstable `rustc-dep-of-std` feature. [#78]

[#58]: https://github.com/rust-random/getrandom/pull/58
[#64]: https://github.com/rust-random/getrandom/pull/64
[#69]: https://github.com/rust-random/getrandom/pull/69
[#71]: https://github.com/rust-random/getrandom/pull/71
[#78]: https://github.com/rust-random/getrandom/pull/78

## [0.1.8] - 2019-07-29
### Changed
- Explicitly specify types to arguments of 'libc::syscall'. [#74]

[#74]: https://github.com/rust-random/getrandom/pull/74

## [0.1.7] - 2019-07-29
### Added
- Support for hermit and l4re. [#61]
- `Error::raw_os_error` method, `Error::INTERNAL_START` and
`Error::CUSTOM_START` constants. Use `libc` for retrieving OS error descriptions. [#54]

### Changed
- Remove `lazy_static` dependency and use custom structures for lock-free
initialization. [#51] [#52]
- Try `getrandom()` first on FreeBSD. [#57]

### Removed
-  Bitrig support. [#56]

### Deprecated
- `Error::UNKNOWN`, `Error::UNAVAILABLE`. [#54]

[#51]: https://github.com/rust-random/getrandom/pull/51
[#52]: https://github.com/rust-random/getrandom/pull/52
[#54]: https://github.com/rust-random/getrandom/pull/54
[#56]: https://github.com/rust-random/getrandom/pull/56
[#57]: https://github.com/rust-random/getrandom/pull/57
[#61]: https://github.com/rust-random/getrandom/pull/61

## [0.1.6] - 2019-06-30
### Changed
- Minor change of RDRAND AMD bug handling. [#48]

[#48]: https://github.com/rust-random/getrandom/pull/48

## [0.1.5] - 2019-06-29
### Fixed
- Use shared `File` instead of shared file descriptor. [#44]
- Workaround for RDRAND hardware bug present on some AMD CPUs. [#43]

### Changed
- Try `getentropy` and then fallback to `/dev/random` on macOS. [#38]

[#38]: https://github.com/rust-random/getrandom/issues/38
[#43]: https://github.com/rust-random/getrandom/pull/43
[#44]: https://github.com/rust-random/getrandom/issues/44

## [0.1.4] - 2019-06-28
### Added
- Add support for `x86_64-unknown-uefi` target by using RDRAND with CPUID
feature detection. [#30]

### Fixed
- Fix long buffer issues on Windows and Linux. [#31] [#32]
- Check `EPERM` in addition to `ENOSYS` on Linux. [#37]

### Changed
- Improve efficiency by sharing file descriptor across threads. [#13]
- Remove `cloudabi`, `winapi`, and `fuchsia-cprng` dependencies. [#40]
- Improve RDRAND implementation. [#24]
- Don't block during syscall detection on Linux. [#26]
- Increase consistency with libc implementation on FreeBSD. [#36]
- Apply `rustfmt`. [#39]

[#30]: https://github.com/rust-random/getrandom/pull/30
[#13]: https://github.com/rust-random/getrandom/issues/13
[#40]: https://github.com/rust-random/getrandom/pull/40
[#26]: https://github.com/rust-random/getrandom/pull/26
[#24]: https://github.com/rust-random/getrandom/pull/24
[#39]: https://github.com/rust-random/getrandom/pull/39
[#36]: https://github.com/rust-random/getrandom/pull/36
[#31]: https://github.com/rust-random/getrandom/issues/31
[#32]: https://github.com/rust-random/getrandom/issues/32
[#37]: https://github.com/rust-random/getrandom/issues/37

## [0.1.3] - 2019-05-15
- Update for `wasm32-unknown-wasi` being renamed to `wasm32-wasi`, and for
  WASI being categorized as an OS.

## [0.1.2] - 2019-04-06
- Add support for `wasm32-unknown-wasi` target.

## [0.1.1] - 2019-04-05
- Enable std functionality for CloudABI by default.

## [0.1.0] - 2019-03-23
Publish initial implementation.

## [0.0.0] - 2019-01-19
Publish an empty template library.

[0.2.16]: https://github.com/rust-random/getrandom/compare/v0.2.15...v0.2.16
[0.2.15]: https://github.com/rust-random/getrandom/compare/v0.2.14...v0.2.15
[0.2.14]: https://github.com/rust-random/getrandom/compare/v0.2.13...v0.2.14
[0.2.13]: https://github.com/rust-random/getrandom/compare/v0.2.12...v0.2.13
[0.2.12]: https://github.com/rust-random/getrandom/compare/v0.2.11...v0.2.12
[0.2.11]: https://github.com/rust-random/getrandom/compare/v0.2.10...v0.2.11
[0.2.10]: https://github.com/rust-random/getrandom/compare/v0.2.9...v0.2.10
[0.2.9]: https://github.com/rust-random/getrandom/compare/v0.2.8...v0.2.9
[0.2.8]: https://github.com/rust-random/getrandom/compare/v0.2.7...v0.2.8
[0.2.7]: https://github.com/rust-random/getrandom/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/rust-random/getrandom/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/rust-random/getrandom/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/rust-random/getrandom/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/rust-random/getrandom/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/rust-random/getrandom/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/rust-random/getrandom/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/rust-random/getrandom/compare/v0.1.15...v0.2.0
[0.1.16]: https://github.com/rust-random/getrandom/compare/v0.1.15...v0.1.16
[0.1.15]: https://github.com/rust-random/getrandom/compare/v0.1.14...v0.1.15
[0.1.14]: https://github.com/rust-random/getrandom/compare/v0.1.13...v0.1.14
[0.1.13]: https://github.com/rust-random/getrandom/compare/v0.1.12...v0.1.13
[0.1.12]: https://github.com/rust-random/getrandom/compare/v0.1.11...v0.1.12
[0.1.11]: https://github.com/rust-random/getrandom/compare/v0.1.10...v0.1.11
[0.1.10]: https://github.com/rust-random/getrandom/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/rust-random/getrandom/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/rust-random/getrandom/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/rust-random/getrandom/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/rust-random/getrandom/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/rust-random/getrandom/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/rust-random/getrandom/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/rust-random/getrandom/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/rust-random/getrandom/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/rust-random/getrandom/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/rust-random/getrandom/compare/v0.0.0...v0.1.0
[0.0.0]: https://github.com/rust-random/getrandom/releases/tag/v0.0.0
