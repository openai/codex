# Changelog

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [1.0.5] - 2025-03-10 <a name="1.0.5"></a>
### Fixed
- Unix: Fix build on redox os. See PR #104
- macOS: Fix packed alignment of LSLaunchURLSpec. See PR #102

### Added
- WASM: support wasm64

## [1.0.4] - 2025-03-10 <a name="1.0.4"></a>
### Changed
- iOS: Return appropriate errors if called from non-main thread or before UIApplication initialization. See PRs #95 and #99

## [1.0.3] - 2024-11-28 <a name="1.0.3"></a>
### Added
- Unix: add support for solaris

## [1.0.2] - 2024-09-16 <a name="1.0.2"></a>
### Changed
- macOS: move to core-foundation 0.10

## [1.0.1] - 2024-05-06 <a name="1.0.1"></a>
### Added
- Support for visionOS. See PR #86 and #87

## [1.0.0] - 2024-04-20 <a name="1.0.0"></a>
### Added
- Move to 1.0! MSRV has now been defined too.

### Fixed
- Unix: default to standard unix implementation, instead of whitelisting each flavour of unix
- WASM: return an error on dry_run if window object isn't available

## [0.8.15] - 2024-04-10 <a name="0.8.15"></a>
### Fixed
- Fix crash on iOS. See #83 and #84

## [0.8.14] - 2024-04-06 <a name="0.8.14"></a>
### Added
- Illumos: experimental support. See PR #82
### Fixed
- Previously configured MSRV was stale (i.e. wouldn't have compiled), and has been modified to 1.60. This is being treated as bug, and as such not requiring a major release.

## [0.8.13] - 2024-03-05 <a name="0.8.13"></a>
### Added
- AIX: experimental support. See PR #79

## [0.8.12] - 2023-10-11 <a name="0.8.12"></a>
### Fixed
- Linux: fix message about BROWSER env variable. See PR #76

## [0.8.11] - 2023-08-17 <a name="0.8.11"></a>
### Fixed
- WSL: handle `&` in URL correctly in WSL environment. See issue #73 and PR #74

## [0.8.10] - 2023-04-12 <a name="0.8.10"></a>
### Changed
- Linux: move to `home` as a dependency, instead of `dirs`

## [0.8.9] - 2023-04-12 <a name="0.8.9"></a>
### Added
- Linux: add support for running under Flatpak sandboxes. See issue #67 and PR #70

### Fixed
- Windows: fix a bug where browser command parsing failed. See issue #68 and PR #69

## [0.8.8] - 2023-01-30 <a name="0.8.8"></a>
### Changed
- Android: bumped `jni` dependency version to `0.21`

## [0.8.7] - 2023-01-30 <a name="0.8.7"></a>
### Fixed
- Fixes a bug on WSL, when `xdg-settings` executes successfully but returns no default browser name. Thanks to [@krsh732](https://github.com/krsh732). See #64.

## [0.8.6] - 2023-01-26 <a name="0.8.6"></a>
### Fixed
- For Windows 32-bit, fix ABI to be used, which was broken in v0.8.5. Thanks to [@alula](https://github.com/alula). See #62 and #63.

## [0.8.5] - 2022-12-31 <a name="0.8.5"></a>
### Fixed
- For Windows platform, removes the `windows` crate dependency, relying on selective FFI bindings instead, thus avoiding the large dependency.
See #62. Thanks to [@Jake-Shadle](https://github.com/Jake-Shadle).

## [0.8.4] - 2022-12-31 <a name="0.8.4"></a>
### Fixed
- Urgent bug fix for windows, where rendering broke on Firefox & Chrome. See #60

## [0.8.3] - 2022-12-30 <a name="0.8.3"></a>
### Added
- Web browser is guaranteed to open for local files even if local file association was to a non-browser app (say an editor). This now is formally
incorporated as part of this crate's [Consistent Behaviour](https://github.com/amodm/webbrowser-rs/blob/main/README.md#consistent-behaviour)
- WSL support, thanks to [@Nachtalb](https://github.com/Nachtalb). This works even if `wslu` is not installed in WSL environments.
- A new feature `hardened` now available for applications which require only http(s) urls to be opened. This acts as a security feature.

### Changed
- On macOS, we now use `CoreFoundation` library instead of `open` command.
- On Linux/*BSD, we now parse xdg configuration to execute the command directly, instead of using `xdg-open` command. This allows us to open the
browser for local html files, even if the `.html` extension was associated with an edit (see #55)

### Fixed
- The guarantee of web browser being opened (instead of local file association), now solves for the scenario where the URL is crafted to be an
executable. This was reported privately by [@offalltn](https://github.com/offalltn).

## [0.8.2] - 2022-11-08 <a name="0.8.2"></a>
### Fixed
- Fix app crashes when running under termux on Android. See #53 and #54.

## [0.8.1] - 2022-11-01 <a name="0.8.1"></a>
### Fixed
- On Android, app crashes due to ndk-glue dependency. See #51 and #52. Thanks to [@rib](https://github.com/rib) for the fix.

## [0.8.0] - 2022-09-09 <a name="0.8.0"></a>
### Added
- Support for iOS is finally here. Thanks to [hakolao](https://github.com/hakolao) for this. See [PR #48](https://github.com/amodm/webbrowser-rs/pull/48)

### Changed
- Updated all dependencies to current versions

## [0.7.1] - 2022-04-27 <a name="0.7.1"></a>
### Added
- Introduce `Browser::is_available()` and `Browser::exists(&self)` to check availability of browsers without opening a URL

### Changed
- Modify `BrowserOptions` to be constructable only via the builder pattern, to prevent future API compatibility issues

## [0.7.0] - 2022-04-24 <a name="0.7.0"></a>
### Added
- Introduce way to provide a target hint to the browser via `BrowserOptions::target_hint` [PR #45](https://github.com/amodm/webbrowser-rs/pull/45)

### Changed
- Breaking API change for users of `BrowserOptions`. We've now shifted to a non-consuming builder pattern to avoid future breakages, as more items get added to `BrowserOptions`

## [0.6.0] - 2022-02-19 <a name="0.6.0"></a>
### Changed
- Define consistent non-blocking behaviour on all UNIX platforms. Now, unless it's specifically a text browser (like lynx etc.), we make sure that the browser is launched in a non-blocking way. See #18 and https://github.com/amodm/webbrowser-rs/commit/614cacf4a67ae0a75323768a1d70c16d792a760d
- Define default behaviour on all UNIX platforms to make sure that stdout/stderr are suppressed. See #20 and https://github.com/amodm/webbrowser-rs/commit/ecfbf66daa0cc139bd557bd7899a183bd6575990
- (Low probability) breaking change: All public functions now return a `Result<()>`. As almost all the uses of this library do a `.is_ok()` or equivalent, there should not be any breaks, but please report a bug if you do. See #42 and #43
- @VZout modified Android implementation to use JNI instead of `am start` because of permission issues in more recent Android.
- Define consistent behaviour for non-ascii URLs, where they're now encoded automatically before being invoked. See #34 and https://github.com/amodm/webbrowser-rs/commit/11789ddfe36264bbbe7d596ab61e3fff855c3adb
- Richer set of underlying commands used for UNIX to cater to different scenarios at runtime. See https://github.com/amodm/webbrowser-rs/commit/d09eeae4f2ab5664fc01f4dba4a409e1bc11f10e

### Fixed
- On WASM, by default URLs are opened with a target of `_blank`. See #39. Thanks to @vbeffa for pointing out the issue.
- @tokusumi fixed #41 where addition of `open` command (done for Haiku) was breaking things in some places.

## [0.5.5] - 2020-07-20 <a name="0.5.5"></a>
### Added
- Support for WASM [PR #26](https://github.com/amodm/webbrowser-rs/pull/26)

## [0.5.4] - 2020-06-09 <a name="0.5.4"></a>
### Fixed
- Fix README to reflect platform support for Android and Haiku

## [0.5.3] - 2020-06-09 <a name="0.5.3"></a>
### Changed
- Added support for Haiku (Untested right now!) [PR #21](https://github.com/amodm/webbrowser-rs/pull/21)
- Added support for Android [PR #19](https://github.com/amodm/webbrowser-rs/pull/19)
- Added support for kioclient and x-www-browser [PR #17](https://github.com/amodm/webbrowser-rs/pull/17)

## [0.5.2] - 2019-08-22 <a name="0.5.2"></a>
### Fixed
- Fix a COM leak bug on Windows [PR #15](https://github.com/amodm/webbrowser-rs/pull/15)

## [0.5.1] - 2019-04-01 <a name="0.5.1"></a>
### Fixed
- Fix the behaviour that open() was blocking on Linux and BSD family. [Issue #13](https://github.com/amodm/webbrowser-rs/issues/13)
- Fix tests on macos

## [0.5.0] - 2019-03-31 <a name="0.5.0"></a>
### Added
- Add BSD family to supported platforms. [PR #12](https://github.com/amodm/webbrowser-rs/pull/12)

## [0.4.0] - 2018-12-18 <a name="0.4.0"></a>
### Changed
- Use `ShellExecuteW` on Windows as the earlier approach of using cmd.exe was breaking on
  special characters. [PR #11](https://github.com/amodm/webbrowser-rs/pull/11)

### Fixed
- Fixed Apache Licensing format

## [0.3.1] - 2018-06-22 <a name="0.3.1"></a>
### Fixed
- Fix broken examples header. [PR #7](https://github.com/amodm/webbrowser-rs/pull/7)
- Fix undeclared reference to `env` that breaks Linux. [PR #8](https://github.com/amodm/webbrowser-rs/pull/8)

## [0.3.0] - 2018-06-18 <a name="0.3.0"></a>
### Changed
- Change the OS test to use conditional complication and raise a compile error if the target OS is unsupported. 
  [PR #6](https://github.com/amodm/webbrowser-rs/pull/6)
- Implement useful trait from StdLib for Browser such as `Display`, `Default` and `FromStr`.
  [PR #6](https://github.com/amodm/webbrowser-rs/pull/6)

### Fixed
- Fix the command in `open_on_windows` to use `cmd.exe` instead of `start`. [PR #5](https://github.com/amodm/webbrowser-rs/pull/5)

## [0.2.2] - 2017-01-23 <a name="0.2.2"></a>
### Fixed
- Honour the right syntax for `$BROWSER`. Closes [#3](https://github.com/amodm/webbrowser-rs/issues/3)
- Include `gvfs-open` and `gnome-open` for [#2](https://github.com/amodm/webbrowser-rs/issues/2)

## [0.2.1] - 2017-01-22 <a name="0.2.1"></a>
### Changed
- Honour `$BROWSER` env var on Linux, before choosing to fallback to `xdg-open`. [Issue #2](https://github.com/amodm/webbrowser-rs/issues/2)

## [0.1.3] - 2016-01-11 <a name="0.1.3"></a>
### Added
- Add Apache license [Issue #1](https://github.com/amodm/webbrowser-rs/issues/1)

## [0.1.2] - 2015-12-09 <a name="0.1.2"></a>
### Added
- Initial release.

[Unreleased]: https://github.com/amodm/webbrowser-rs/compare/v1.0.5...HEAD
[1.0.5]: https://github.com/amodm/webbrowser-rs/compare/v1.0.4...v1.0.5
[1.0.4]: https://github.com/amodm/webbrowser-rs/compare/v1.0.3...v1.0.4
[1.0.3]: https://github.com/amodm/webbrowser-rs/compare/v1.0.2...v1.0.3
[1.0.2]: https://github.com/amodm/webbrowser-rs/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/amodm/webbrowser-rs/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/amodm/webbrowser-rs/compare/v0.8.15...v1.0.0
[0.8.15]: https://github.com/amodm/webbrowser-rs/compare/v0.8.14...v0.8.15
[0.8.14]: https://github.com/amodm/webbrowser-rs/compare/v0.8.13...v0.8.14
[0.8.13]: https://github.com/amodm/webbrowser-rs/compare/v0.8.12...v0.8.13
[0.8.12]: https://github.com/amodm/webbrowser-rs/compare/v0.8.11...v0.8.12
[0.8.11]: https://github.com/amodm/webbrowser-rs/compare/v0.8.10...v0.8.11
[0.8.10]: https://github.com/amodm/webbrowser-rs/compare/v0.8.9...v0.8.10
[0.8.9]: https://github.com/amodm/webbrowser-rs/compare/v0.8.8...v0.8.9
[0.8.8]: https://github.com/amodm/webbrowser-rs/compare/v0.8.7...v0.8.8
[0.8.7]: https://github.com/amodm/webbrowser-rs/compare/v0.8.6...v0.8.7
[0.8.6]: https://github.com/amodm/webbrowser-rs/compare/v0.8.5...v0.8.6
[0.8.5]: https://github.com/amodm/webbrowser-rs/compare/v0.8.4...v0.8.5
[0.8.4]: https://github.com/amodm/webbrowser-rs/compare/v0.8.3...v0.8.4
[0.8.3]: https://github.com/amodm/webbrowser-rs/compare/v0.8.2...v0.8.3
[0.8.2]: https://github.com/amodm/webbrowser-rs/compare/v0.8.1...v0.8.2
[0.8.1]: https://github.com/amodm/webbrowser-rs/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/amodm/webbrowser-rs/compare/v0.7.1...v0.8.0
[0.7.1]: https://github.com/amodm/webbrowser-rs/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/amodm/webbrowser-rs/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/amodm/webbrowser-rs/compare/v0.5.5...v0.6.0
[0.5.5]: https://github.com/amodm/webbrowser-rs/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/amodm/webbrowser-rs/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/amodm/webbrowser-rs/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/amodm/webbrowser-rs/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/amodm/webbrowser-rs/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/amodm/webbrowser-rs/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/amodm/webbrowser-rs/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/amodm/webbrowser-rs/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/amodm/webbrowser-rs/compare/v0.2.2...v0.3.0
[0.2.2]: https://github.com/amodm/webbrowser-rs/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/amodm/webbrowser-rs/compare/v0.1.3...v0.2.1
[0.1.3]: https://github.com/amodm/webbrowser-rs/compare/v0.1.2...v0.1.3
