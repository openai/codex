# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.10.7 (2023-05-19)
### Changed
- Loosen `subtle` version requirement ([#1260])

[#1260]: https://github.com/RustCrypto/traits/pull/1260

## 0.10.6 (2022-11-17)
### Added
- `Mac::verify_reset` and `Mac::verify_slice_reset` methods ([#1154])

[#1154]: https://github.com/RustCrypto/traits/pull/1154

## 0.10.5 (2022-09-16)
### Fixed 
- MSRV build ([#1117])

[#1117]: https://github.com/RustCrypto/traits/pull/1117

## 0.10.4 (2022-09-16)
### Added
- Feature-gated implementation of the `const_oid::AssociatedOid` trait
for the core wrappers. ([#1098])

[#1098]: https://github.com/RustCrypto/traits/pull/1098

## 0.10.3 (2022-02-16)
### Fixed
- Minimal versions build ([#940])

[#940]: https://github.com/RustCrypto/traits/pull/940

## 0.10.2 (2022-02-10)
### Changed
- Relax bounds on the `Mac` trait ([#849])

[#849]: https://github.com/RustCrypto/traits/pull/849

## 0.10.1 (2021-12-14) [YANKED]
### Added
- `Update::chain` and `Digest::new_with_prefix` methods. ([#846])
- `Mac::generate_key` method. ([#847])

### Fixed
- Doc cfg attribute for `CtOutput` and `MacError`. ([#842])
- Expose `KeyInit::generate_key` method in docs. ([#847])

[#842]: https://github.com/RustCrypto/traits/pull/842
[#846]: https://github.com/RustCrypto/traits/pull/846
[#847]: https://github.com/RustCrypto/traits/pull/847

## 0.10.0 (2021-12-07) [YANKED]
### Changed
- Dirty traits are removed and instead block-level traits are introduced.
Variable output traits reworked and now support both run and compile time selection of output size. ([#380], [#819])
- The `crypto-mac` traits are reworked and merged in. ([#819])

[#819]: https://github.com/RustCrypto/traits/pull/819
[#380]: https://github.com/RustCrypto/traits/pull/380

## 0.9.0 (2020-06-09)
### Added
- `ExtendableOutputDirty` and `VariableOutputDirty` traits ([#183])
- `FixedOutputDirty` trait + `finalize_into*` ([#180])
- `XofReader::read_boxed` method ([#178], [#181], [#182])
- `alloc` feature ([#163])
- Re-export `typenum::consts` as `consts` ([#123])
- `Output` type alias ([#115])

### Changed
- Rename `*result*` methods to `finalize` ala IUF ([#161])
- Use `impl AsRef<[u8]>` instead of generic params on methods ([#112])
- Rename `Input::input` to `Update::update` ala IUF ([#111])
- Upgrade to Rust 2018 edition ([#109])
- Bump `generic-array` to v0.14 ([#95])

[#183]: https://github.com/RustCrypto/traits/pull/183
[#181]: https://github.com/RustCrypto/traits/pull/181
[#182]: https://github.com/RustCrypto/traits/pull/182
[#180]: https://github.com/RustCrypto/traits/pull/180
[#178]: https://github.com/RustCrypto/traits/pull/178
[#163]: https://github.com/RustCrypto/traits/pull/163
[#161]: https://github.com/RustCrypto/traits/pull/161
[#123]: https://github.com/RustCrypto/traits/pull/123
[#115]: https://github.com/RustCrypto/traits/pull/115
[#111]: https://github.com/RustCrypto/traits/pull/111
[#112]: https://github.com/RustCrypto/traits/pull/112
[#109]: https://github.com/RustCrypto/traits/pull/109
[#95]: https://github.com/RustCrypto/traits/pull/95

## 0.8.1 (2019-06-30)

## 0.8.0 (2018-10-01)

## 0.7.6 (2018-09-21)

## 0.7.5 (2018-07-13)

## 0.7.4 (2018-06-21)

## 0.7.3 (2018-06-20)

## 0.7.2 (2017-11-17)

## 0.7.1 (2017-11-15)

## 0.7.0 (2017-11-14)

## 0.6.2 (2017-07-24)

## 0.6.1 (2017-06-18)

## 0.6.0 (2017-06-12)

## 0.5.2 (2017-05-02)

## 0.5.1 (2017-05-02)

## 0.5.0 (2017-04-06)

## 0.4.0 (2016-12-24)

## 0.3.1 (2016-12-16)

## 0.3.0 (2016-11-17)

## 0.2.1 (2016-10-14)

## 0.2.0 (2016-10-14)

## 0.1.0 (2016-10-06)
