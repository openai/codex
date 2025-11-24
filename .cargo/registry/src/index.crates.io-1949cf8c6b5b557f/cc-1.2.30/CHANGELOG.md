# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.30](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.29...cc-v1.2.30) - 2025-07-18

### Other

- define _REENTRANT by default ([#1496](https://github.com/rust-lang/cc-rs/pull/1496))

## [1.2.29](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.28...cc-v1.2.29) - 2025-07-05

### Other

- Fix target parsing for powerpc ([#1490](https://github.com/rust-lang/cc-rs/pull/1490))

## [1.2.28](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.27...cc-v1.2.28) - 2025-07-04

### Other

- Recognize `mlibc` environment ([#1488](https://github.com/rust-lang/cc-rs/pull/1488))
- Fix clippy warnings about not using variables in `format!` strings ([#1489](https://github.com/rust-lang/cc-rs/pull/1489))

## [1.2.27](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.26...cc-v1.2.27) - 2025-06-13

### Other

- Regenerate windows sys bindings ([#1485](https://github.com/rust-lang/cc-rs/pull/1485))
- Update windows-bindgen requirement from 0.61 to 0.62 ([#1484](https://github.com/rust-lang/cc-rs/pull/1484))
- Regenerate target info ([#1483](https://github.com/rust-lang/cc-rs/pull/1483))

## [1.2.26](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.25...cc-v1.2.26) - 2025-06-06

### Other

- Also set `SDKROOT` when building apple platforms ([#1475](https://github.com/rust-lang/cc-rs/pull/1475))
- use windows 2022 in CI ([#1479](https://github.com/rust-lang/cc-rs/pull/1479))
- Detect -Wslash-u-filename warning on clang-cl ([#1477](https://github.com/rust-lang/cc-rs/pull/1477))

## [1.2.25](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.24...cc-v1.2.25) - 2025-05-30

### Other

- make `powerp64` use `powerpc64-linux-gnu` prefix ([#1474](https://github.com/rust-lang/cc-rs/pull/1474))

## [1.2.24](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.23...cc-v1.2.24) - 2025-05-23

### Other

- Regenerate windows sys bindings ([#1471](https://github.com/rust-lang/cc-rs/pull/1471))

## [1.2.23](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.22...cc-v1.2.23) - 2025-05-16

### Other

- support "vxworks" and "nto" OSes on `get_base_archiver_variant` ([#1456](https://github.com/rust-lang/cc-rs/pull/1456))

## [1.2.22](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.21...cc-v1.2.22) - 2025-05-09

### Other

- Add `flags` method to `cc::Build` for adding multiple flags ([#1466](https://github.com/rust-lang/cc-rs/pull/1466))

## [1.2.21](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.20...cc-v1.2.21) - 2025-05-02

### Other

- Fix wasm32-unknown-unknown by passing -c ([#1424](https://github.com/rust-lang/cc-rs/pull/1424))

## [1.2.20](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.19...cc-v1.2.20) - 2025-04-25

### Other

- Regenerate target info ([#1461](https://github.com/rust-lang/cc-rs/pull/1461))
- Fix parser.rs on latest rustc nightly ([#1459](https://github.com/rust-lang/cc-rs/pull/1459))

## [1.2.19](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.18...cc-v1.2.19) - 2025-04-11

### Other

- Fix musl compilation: Add musl as a prefix fallback ([#1455](https://github.com/rust-lang/cc-rs/pull/1455))

## [1.2.18](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.17...cc-v1.2.18) - 2025-04-04

### Other

- Regenerate target info ([#1450](https://github.com/rust-lang/cc-rs/pull/1450))
- Use `std::thread::available_parallelism` for determining the default number of jobs ([#1447](https://github.com/rust-lang/cc-rs/pull/1447))
- Fix mips64-openwrt-linux-musl parsing ([#1449](https://github.com/rust-lang/cc-rs/pull/1449))
- Use compiler prefix `x86_64-linux-musl` ([#1443](https://github.com/rust-lang/cc-rs/pull/1443))

## [1.2.17](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.16...cc-v1.2.17) - 2025-03-21

### Other

- Regenerate target info ([#1439](https://github.com/rust-lang/cc-rs/pull/1439))
- Regenerate windows sys bindings ([#1437](https://github.com/rust-lang/cc-rs/pull/1437))
- Fix wasm32-wali-linux-musl target parsing ([#1434](https://github.com/rust-lang/cc-rs/pull/1434))
- Parse `rustc` target names ([#1413](https://github.com/rust-lang/cc-rs/pull/1413))
- Regenerate target info ([#1429](https://github.com/rust-lang/cc-rs/pull/1429))
- Added base support for `wasm32-wali-linux-musl` target ([#1373](https://github.com/rust-lang/cc-rs/pull/1373))

## [1.2.16](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.15...cc-v1.2.16) - 2025-02-28

### Fixed

- force windows compiler to run in `out_dir` to prevent artifacts in cwd (#1415)

### Other

- use `/arch:SSE2` for `x86` target arch (#1425)
- Regenerate windows-sys binding ([#1422](https://github.com/rust-lang/cc-rs/pull/1422))
- Regenerate target info ([#1418](https://github.com/rust-lang/cc-rs/pull/1418))
- Add LIB var when compiling flag_check (#1417)
- Change flag ordering ([#1403](https://github.com/rust-lang/cc-rs/pull/1403))
- Fix archiver detection for musl cross compilation ([#1404](https://github.com/rust-lang/cc-rs/pull/1404))

## [1.2.15](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.14...cc-v1.2.15) - 2025-02-21

### Other

- Regenerate target info ([#1406](https://github.com/rust-lang/cc-rs/pull/1406))
- Always read from all `CFLAGS`-style flags ([#1401](https://github.com/rust-lang/cc-rs/pull/1401))
- Simplify the error output on failed `Command` invocation ([#1397](https://github.com/rust-lang/cc-rs/pull/1397))

## [1.2.14](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.13...cc-v1.2.14) - 2025-02-14

### Other

- Regenerate target info ([#1398](https://github.com/rust-lang/cc-rs/pull/1398))
- Add support for setting `-gdwarf-{version}` based on RUSTFLAGS ([#1395](https://github.com/rust-lang/cc-rs/pull/1395))
- Add support for alternative network stack io-sock on QNX 7.1 aarch64 and x86_64 ([#1312](https://github.com/rust-lang/cc-rs/pull/1312))

## [1.2.13](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.12...cc-v1.2.13) - 2025-02-08

### Other

- Fix cross-compiling for Apple platforms ([#1389](https://github.com/rust-lang/cc-rs/pull/1389))

## [1.2.12](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.11...cc-v1.2.12) - 2025-02-04

### Other

- Split impl Build ([#1382](https://github.com/rust-lang/cc-rs/pull/1382))
- Don't specify both `-target` and `-mtargetos=` on Apple targets ([#1384](https://github.com/rust-lang/cc-rs/pull/1384))

## [1.2.11](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.10...cc-v1.2.11) - 2025-01-31

### Other

- Fix more flag inheritance ([#1380](https://github.com/rust-lang/cc-rs/pull/1380))
- Include wrapper args. in `stdout` family heuristics to restore classifying `clang --driver-mode=cl` as `Msvc { clang_cl: true }` ([#1378](https://github.com/rust-lang/cc-rs/pull/1378))
- Constrain `-Clto` and `-Cembed-bitcode` flag inheritance to be `clang`-only ([#1379](https://github.com/rust-lang/cc-rs/pull/1379))
- Pass deployment target with `-m*-version-min=` ([#1339](https://github.com/rust-lang/cc-rs/pull/1339))
- Regenerate target info ([#1376](https://github.com/rust-lang/cc-rs/pull/1376))

## [1.2.10](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.9...cc-v1.2.10) - 2025-01-17

### Other

- Fix CC_FORCE_DISABLE=0 evaluating to true ([#1371](https://github.com/rust-lang/cc-rs/pull/1371))
- Regenerate target info ([#1369](https://github.com/rust-lang/cc-rs/pull/1369))
- Make hidden lifetimes explicit. ([#1366](https://github.com/rust-lang/cc-rs/pull/1366))

## [1.2.9](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.8...cc-v1.2.9) - 2025-01-12

### Other

- Don't pass inherited PGO flags to GNU compilers (#1363)
- Adjusted zig cc judgment and avoided zigbuild errors([#1360](https://github.com/rust-lang/cc-rs/pull/1360)) ([#1361](https://github.com/rust-lang/cc-rs/pull/1361))
- Fix compilation on macOS using clang and fix compilation using zig-cc ([#1364](https://github.com/rust-lang/cc-rs/pull/1364))

## [1.2.8](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.7...cc-v1.2.8) - 2025-01-11

### Other

- Add `is_like_clang_cl()` getter (#1357)
- Fix clippy error in lib.rs ([#1356](https://github.com/rust-lang/cc-rs/pull/1356))
- Regenerate target info ([#1352](https://github.com/rust-lang/cc-rs/pull/1352))
- Fix compiler family detection issue with clang-cl on macOS ([#1328](https://github.com/rust-lang/cc-rs/pull/1328))
- Update `windows-bindgen` dependency ([#1347](https://github.com/rust-lang/cc-rs/pull/1347))
- Fix clippy warnings ([#1346](https://github.com/rust-lang/cc-rs/pull/1346))

## [1.2.7](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.6...cc-v1.2.7) - 2025-01-03

### Other

- Regenerate target info ([#1342](https://github.com/rust-lang/cc-rs/pull/1342))
- Document new supported architecture names in windows::find
- Make is_flag_supported_inner take an &Tool ([#1337](https://github.com/rust-lang/cc-rs/pull/1337))
- Fix is_flag_supported on msvc ([#1336](https://github.com/rust-lang/cc-rs/pull/1336))
- Allow using Visual Studio target names in `find_tool` ([#1335](https://github.com/rust-lang/cc-rs/pull/1335))

## [1.2.6](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.5...cc-v1.2.6) - 2024-12-27

### Other

- Don't inherit the `/Oy` flag for 64-bit targets ([#1330](https://github.com/rust-lang/cc-rs/pull/1330))

## [1.2.5](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.4...cc-v1.2.5) - 2024-12-19

### Other

- Check linking when testing if compiler flags are supported ([#1322](https://github.com/rust-lang/cc-rs/pull/1322))

## [1.2.4](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.3...cc-v1.2.4) - 2024-12-13

### Other

- Add support for C/C++ compiler for Neutrino QNX: `qcc` ([#1319](https://github.com/rust-lang/cc-rs/pull/1319))
- use -maix64 instead of -m64 ([#1307](https://github.com/rust-lang/cc-rs/pull/1307))

## [1.2.3](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.2...cc-v1.2.3) - 2024-12-06

### Other

- Improve detection of environment when compiling from msbuild or msvc ([#1310](https://github.com/rust-lang/cc-rs/pull/1310))
- Better error message when failing on unknown targets ([#1313](https://github.com/rust-lang/cc-rs/pull/1313))
- Optimize RustcCodegenFlags ([#1305](https://github.com/rust-lang/cc-rs/pull/1305))

## [1.2.2](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.1...cc-v1.2.2) - 2024-11-29

### Other

- Inherit flags from rustc ([#1279](https://github.com/rust-lang/cc-rs/pull/1279))
- Add support for using sccache wrapper with cuda/nvcc ([#1304](https://github.com/rust-lang/cc-rs/pull/1304))
- Fix msvc stdout not shown on error ([#1303](https://github.com/rust-lang/cc-rs/pull/1303))
- Regenerate target info ([#1301](https://github.com/rust-lang/cc-rs/pull/1301))
- Fix compilation of C++ code for armv7-unknown-linux-gnueabihf ([#1298](https://github.com/rust-lang/cc-rs/pull/1298))
- Fetch target info from Cargo even if `Build::target` is manually set ([#1299](https://github.com/rust-lang/cc-rs/pull/1299))
- Fix two files with different extensions having the same object name ([#1295](https://github.com/rust-lang/cc-rs/pull/1295))
- Allow disabling cc's ability to compile via env var CC_FORCE_DISABLE ([#1292](https://github.com/rust-lang/cc-rs/pull/1292))
- Regenerate target info ([#1293](https://github.com/rust-lang/cc-rs/pull/1293))

## [1.2.1](https://github.com/rust-lang/cc-rs/compare/cc-v1.2.0...cc-v1.2.1) - 2024-11-14

### Other

- When invoking `cl -?`, set stdin to null ([#1288](https://github.com/rust-lang/cc-rs/pull/1288))

## [1.2.0](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.37...cc-v1.2.0) - 2024-11-11

### Added

- add i686-pc-windows-gnullvm prefix detection ([#1283](https://github.com/rust-lang/cc-rs/pull/1283))

### Other

- Allow only specifying the architecture ([#1285](https://github.com/rust-lang/cc-rs/pull/1285))
- Fix WASM vs. WASI options ([#1284](https://github.com/rust-lang/cc-rs/pull/1284))

## [1.1.37](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.36...cc-v1.1.37) - 2024-11-08

### Other

- Use relative directory for obj files hash ([#1270](https://github.com/rust-lang/cc-rs/pull/1270))
- Regenerate target info ([#1280](https://github.com/rust-lang/cc-rs/pull/1280))

## [1.1.36](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.35...cc-v1.1.36) - 2024-11-05

### Other

- Fix CUDA build with clang++. ([#1273](https://github.com/rust-lang/cc-rs/pull/1273))

## [1.1.35](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.34...cc-v1.1.35) - 2024-11-04

### Other

- Remove support for FRC ([#1268](https://github.com/rust-lang/cc-rs/pull/1268))
- Do not add -fPIC by default on UEFI targets ([#1263](https://github.com/rust-lang/cc-rs/pull/1263))
- Use -windows-gnu for all UEFI targets ([#1264](https://github.com/rust-lang/cc-rs/pull/1264))

## [1.1.34](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.33...cc-v1.1.34) - 2024-11-02

### Other

- Remove redundant flags ([#1256](https://github.com/rust-lang/cc-rs/pull/1256))

## [1.1.33](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.32...cc-v1.1.33) - 2024-11-02

### Other

- Reduce size of `cc::Build`  and size of generated targets ([#1257](https://github.com/rust-lang/cc-rs/pull/1257))

## [1.1.32](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.31...cc-v1.1.32) - 2024-11-02

### Other

- Use `rustc`'s knowledge of LLVM/Clang target triples ([#1252](https://github.com/rust-lang/cc-rs/pull/1252))
- Use Cargo's target information when possible ([#1225](https://github.com/rust-lang/cc-rs/pull/1225))

## [1.1.31](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.30...cc-v1.1.31) - 2024-10-19

### Other

- Add comment explaining why cc does not rebuild on env PATH change ([#1247](https://github.com/rust-lang/cc-rs/pull/1247))

## [1.1.30](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.29...cc-v1.1.30) - 2024-10-11

### Other

- Don't pass -fPIC by default on wasm ([#1245](https://github.com/rust-lang/cc-rs/pull/1245))

## [1.1.29](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.28...cc-v1.1.29) - 2024-10-11

### Other

- Regenerate target info ([#1243](https://github.com/rust-lang/cc-rs/pull/1243))

## [1.1.28](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.27...cc-v1.1.28) - 2024-10-06

### Other

- Environment variables: For one accepting boolean, treat "0", "false" and empty env as false ([#1238](https://github.com/rust-lang/cc-rs/pull/1238))

## [1.1.27](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.26...cc-v1.1.27) - 2024-10-06

### Other

- Revert "Use debug version of MSVC runtime library on debug ([#1231](https://github.com/rust-lang/cc-rs/pull/1231))" ([#1237](https://github.com/rust-lang/cc-rs/pull/1237))
- Disable `CC_ENABLE_DEBUG_OUTPUT` if it is set to "0" ([#1234](https://github.com/rust-lang/cc-rs/pull/1234))

## [1.1.26](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.25...cc-v1.1.26) - 2024-10-06

### Other

- Use debug version of MSVC runtime library on debug ([#1231](https://github.com/rust-lang/cc-rs/pull/1231))

## [1.1.25](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.24...cc-v1.1.25) - 2024-10-05

### Other

- Remove incorrect "lib" prefixes in CXXSTDLIB doc comments ([#1228](https://github.com/rust-lang/cc-rs/pull/1228))

## [1.1.24](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.23...cc-v1.1.24) - 2024-10-01

### Other

- Fix wasm32-wasip1-threads:  shared-memory disallowed due to not compiled with 'atomics' or 'bulk-memory' features ([#1221](https://github.com/rust-lang/cc-rs/pull/1221))
- Reduce the need for the host target triple ([#1224](https://github.com/rust-lang/cc-rs/pull/1224))
- Add auto cancellation for CI jobs ([#1222](https://github.com/rust-lang/cc-rs/pull/1222))

## [1.1.23](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.22...cc-v1.1.23) - 2024-09-30

### Other

- Update doc for detecting changes/upgrades of compilers ([#1218](https://github.com/rust-lang/cc-rs/pull/1218))

## [1.1.22](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.21...cc-v1.1.22) - 2024-09-27

### Other

- Don't rerun if PATH changes ([#1215](https://github.com/rust-lang/cc-rs/pull/1215))

## [1.1.21](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.20...cc-v1.1.21) - 2024-09-18

### Other

- disable pic for targets that end in `-none` ([#1212](https://github.com/rust-lang/cc-rs/pull/1212))

## [1.1.20](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.19...cc-v1.1.20) - 2024-09-17

### Other

- Add buildcache as known Rust and C/C++ compiler wrapper ([#1209](https://github.com/rust-lang/cc-rs/pull/1209))

## [1.1.19](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.18...cc-v1.1.19) - 2024-09-15

### Other

- Add support arm64e-apple-darwin ([#1207](https://github.com/rust-lang/cc-rs/pull/1207))

## [1.1.18](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.17...cc-v1.1.18) - 2024-09-07

### Other
- Fixed unsoundness in `StderrForwarder::forward_available` ([#1203](https://github.com/rust-lang/cc-rs/pull/1203))

## [1.1.17](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.16...cc-v1.1.17) - 2024-09-06

### Fixed
- fix finding toolchains when invoked by msbuild ([#1201](https://github.com/rust-lang/cc-rs/pull/1201))

## [1.1.16](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.15...cc-v1.1.16) - 2024-09-04

### Other
- Treat VxWorks wr-cc as a Gnu compiler ([#1198](https://github.com/rust-lang/cc-rs/pull/1198))

## [1.1.15](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.14...cc-v1.1.15) - 2024-08-26

### Other
- Add -mfloat-abi=hard as a default argument when using any arm/thumb-none-eabihf target ([#1194](https://github.com/rust-lang/cc-rs/pull/1194))

## [1.1.14](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.13...cc-v1.1.14) - 2024-08-23

### Other
- allow finding tools from path if VisualStudioDir is set

## [1.1.13](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.12...cc-v1.1.13) - 2024-08-16

### Other
- Fix detect family: should detect emscripten as clang, closes [#1185](https://github.com/rust-lang/cc-rs/pull/1185) ([#1186](https://github.com/rust-lang/cc-rs/pull/1186))

## [1.1.12](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.11...cc-v1.1.12) - 2024-08-15

### Other
- improve docs ([#1183](https://github.com/rust-lang/cc-rs/pull/1183))

## [1.1.11](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.10...cc-v1.1.11) - 2024-08-14

### Other
- Add support for parsing shell encoded `*FLAGS` ([#1181](https://github.com/rust-lang/cc-rs/pull/1181))
- Replace vector of tuples with BTreeMap which already is sorted and free of duplicates ([#1177](https://github.com/rust-lang/cc-rs/pull/1177))

## [1.1.10](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.9...cc-v1.1.10) - 2024-08-11

### Other
- Remap Windows targets triples to their LLVM counterparts ([#1176](https://github.com/rust-lang/cc-rs/pull/1176))

## [1.1.9](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.8...cc-v1.1.9) - 2024-08-11

### Other
- Add custom CC wrapper to the wrapper whitelist ([#1175](https://github.com/rust-lang/cc-rs/pull/1175))

## [1.1.8](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.7...cc-v1.1.8) - 2024-08-06

### Other
- Fix broken link in docs.rs ([#1173](https://github.com/rust-lang/cc-rs/pull/1173))

## [1.1.7](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.6...cc-v1.1.7) - 2024-07-29

### Other
- add `.objects` ([#1166](https://github.com/rust-lang/cc-rs/pull/1166))

## [1.1.6](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.5...cc-v1.1.6) - 2024-07-19

### Other
- Clippy fixes ([#1163](https://github.com/rust-lang/cc-rs/pull/1163))

## [1.1.5](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.4...cc-v1.1.5) - 2024-07-15

### Other
- Fix cyclic compilation: Use vendored once_cell ([#1154](https://github.com/rust-lang/cc-rs/pull/1154))

## [1.1.4](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.3...cc-v1.1.4) - 2024-07-14

### Other
- Support compiling on wasm targets (Supersede [#1068](https://github.com/rust-lang/cc-rs/pull/1068)) ([#1160](https://github.com/rust-lang/cc-rs/pull/1160))

## [1.1.3](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.2...cc-v1.1.3) - 2024-07-14

### Other
- Reduce msrv to 1.63 ([#1158](https://github.com/rust-lang/cc-rs/pull/1158))
- Revert "Use raw-dylib for windows-sys ([#1137](https://github.com/rust-lang/cc-rs/pull/1137))" ([#1157](https://github.com/rust-lang/cc-rs/pull/1157))
- Fix typos ([#1152](https://github.com/rust-lang/cc-rs/pull/1152))
- Fix `doc_lazy_continuation` lints ([#1153](https://github.com/rust-lang/cc-rs/pull/1153))

## [1.1.2](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.1...cc-v1.1.2) - 2024-07-12

### Other
- Add empty `jobserver` feature. ([#1150](https://github.com/rust-lang/cc-rs/pull/1150))

## [1.1.1](https://github.com/rust-lang/cc-rs/compare/cc-v1.1.0...cc-v1.1.1) - 2024-07-12

### Other
- Fix is_flag_supported not respecting emit_rerun_if_env_changed ([#1147](https://github.com/rust-lang/cc-rs/pull/1147)) ([#1148](https://github.com/rust-lang/cc-rs/pull/1148))

## [1.1.0](https://github.com/rust-lang/cc-rs/compare/cc-v1.0.106...cc-v1.1.0) - 2024-07-08

### Added
- add cargo_output to eliminate last vestiges of stdout pollution ([#1141](https://github.com/rust-lang/cc-rs/pull/1141))

## [1.0.106](https://github.com/rust-lang/cc-rs/compare/cc-v1.0.105...cc-v1.0.106) - 2024-07-08

### Other
- Drop support for Visual Studio 12 (2013) ([#1046](https://github.com/rust-lang/cc-rs/pull/1046))
- Use raw-dylib for windows-sys ([#1137](https://github.com/rust-lang/cc-rs/pull/1137))
- Bump msrv to 1.67 ([#1143](https://github.com/rust-lang/cc-rs/pull/1143))
- Bump msrv to 1.65 ([#1140](https://github.com/rust-lang/cc-rs/pull/1140))
- Fix clippy warnings ([#1138](https://github.com/rust-lang/cc-rs/pull/1138))

## [1.0.105](https://github.com/rust-lang/cc-rs/compare/cc-v1.0.104...cc-v1.0.105) - 2024-07-07

### Other
- Regenerate windows sys bindings ([#1132](https://github.com/rust-lang/cc-rs/pull/1132))
- Fix generate-windows-sys-bindings ([#1133](https://github.com/rust-lang/cc-rs/pull/1133))
- Fix gen-windows-sys-binding ([#1130](https://github.com/rust-lang/cc-rs/pull/1130))
- Fix gen-windows-sys-binding ([#1127](https://github.com/rust-lang/cc-rs/pull/1127))
- Update windows-bindgen requirement from 0.57 to 0.58 ([#1123](https://github.com/rust-lang/cc-rs/pull/1123))

## [1.0.104](https://github.com/rust-lang/cc-rs/compare/cc-v1.0.103...cc-v1.0.104) - 2024-07-01

### Other
- Fixed link break about compile-time-requirements ([#1118](https://github.com/rust-lang/cc-rs/pull/1118))

## [1.0.103](https://github.com/rust-lang/cc-rs/compare/cc-v1.0.102...cc-v1.0.103) - 2024-06-30

### Other
- Fix compilation for wasm: env WASI_SYSROOT should be optional ([#1114](https://github.com/rust-lang/cc-rs/pull/1114))

## [1.0.102](https://github.com/rust-lang/cc-rs/compare/cc-v1.0.101...cc-v1.0.102) - 2024-06-29

### Other
- Fix invalid wasi targets compatibility ([#1105](https://github.com/rust-lang/cc-rs/pull/1105))
- Speedup regenerate-target-info and regenerate-windows-sys ([#1110](https://github.com/rust-lang/cc-rs/pull/1110))

## [1.0.101](https://github.com/rust-lang/cc-rs/compare/cc-v1.0.100...cc-v1.0.101) - 2024-06-25

### Other
- Use `Build::getenv` instead of `env::var*` in anywhere that makes sense ([#1103](https://github.com/rust-lang/cc-rs/pull/1103))

## [1.0.100](https://github.com/rust-lang/cc-rs/compare/cc-v1.0.99...cc-v1.0.100) - 2024-06-23

### Other
- Update publish.yml to use release-plz ([#1101](https://github.com/rust-lang/cc-rs/pull/1101))
- Accept `OsStr` instead of `str` for flags ([#1100](https://github.com/rust-lang/cc-rs/pull/1100))
- Use `dep:` syntax to avoid implicit features. ([#1099](https://github.com/rust-lang/cc-rs/pull/1099))
- Minor clippy fixes. ([#1098](https://github.com/rust-lang/cc-rs/pull/1098))
- Fix WASI compilation for C++ ([#1083](https://github.com/rust-lang/cc-rs/pull/1083))
- Regenerate windows sys bindings ([#1096](https://github.com/rust-lang/cc-rs/pull/1096))
- Rename regenerate-windows-sys to regenerate-windows-sys.yml ([#1095](https://github.com/rust-lang/cc-rs/pull/1095))
- Create regenerate-windows-sys.yml ([#1094](https://github.com/rust-lang/cc-rs/pull/1094))
- Update windows-bindgen requirement from 0.56 to 0.57 ([#1091](https://github.com/rust-lang/cc-rs/pull/1091))
- Eagerly close tempfile to fix [#1082](https://github.com/rust-lang/cc-rs/pull/1082) ([#1087](https://github.com/rust-lang/cc-rs/pull/1087))
- Output msvc.exe in the output directory ([#1090](https://github.com/rust-lang/cc-rs/pull/1090))
- Fix clippy warnings on Windows ([#1088](https://github.com/rust-lang/cc-rs/pull/1088))
- Don't try to free DLL on drop ([#1089](https://github.com/rust-lang/cc-rs/pull/1089))
- Fix panic safety issue in StderrForwarder ([#1079](https://github.com/rust-lang/cc-rs/pull/1079))
