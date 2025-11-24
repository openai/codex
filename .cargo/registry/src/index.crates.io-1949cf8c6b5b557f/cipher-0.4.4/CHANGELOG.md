# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.4.4 (2022-03-09)
### Changed
- Move `ParBlocks`/`ParBlocksSizeUser` to the `crypto-common` crate ([#1052])

### Fixed
-  Unsoundness triggered by zero block size ([#1277])

[#1052]: https://github.com/RustCrypto/traits/pull/1052
[#1277]: https://github.com/RustCrypto/traits/pull/1277

## 0.4.3 (2022-02-22)
### Fixed
- Do not enable the `alloc` feature by default ([#953])

[#953]: https://github.com/RustCrypto/traits/pull/953

## 0.4.2 (2022-02-16) [YANKED]
### Fixed
- Rename `BlockDecryptMut::decrypt_padded_vec` to `decrypt_padded_vec_mut` for consistency with other methods ([#941])

[#941]: https://github.com/RustCrypto/traits/pull/941

## 0.4.1 (2022-02-16) [YANKED]
### Added
- Allocating padded encrypt/decrypt ([#936])

### Fixed
- Minimal versions build ([#940])

[#940]: https://github.com/RustCrypto/traits/pull/940
[#936]: https://github.com/RustCrypto/traits/pull/936

## 0.4.0 (2022-02-10)
### Changed
- Major rework of traits. Core functionality of block and stream ciphers
is defined using rank-2 closures with convenience methods built on top of
it. Expose block-level trait for stream ciphers and add generic wrapper
around it. The async stream cipher trait is defined as sub-trait of
mutable block cipher traits. ([#849])

### Added
- Re-export `rand_core` ([#683])

[#683]: https://github.com/RustCrypto/traits/pull/683
[#849]: https://github.com/RustCrypto/traits/pull/849

## 0.3.0 (2021-04-28)
### Added
- Encrypt/decrypt-only block cipher traits ([#352])
- Re-export `blobby` from root ([#435])
- Block cipher trait blanket impls for refs ([#441])
- `generate_key` method to `New*` trait ([#513])

### Changed
- Consolidate error types ([#373])
- Change `SeekNum` impls to fit with the new `BlockBuffer` ([#435])
- Reorganize modules ([#435])
- Renamed `new_var` to `new_from_slice(s)` ([#442])

[#352]: https://github.com/RustCrypto/traits/pull/352
[#373]: https://github.com/RustCrypto/traits/pull/373
[#435]: https://github.com/RustCrypto/traits/pull/435
[#441]: https://github.com/RustCrypto/traits/pull/441
[#442]: https://github.com/RustCrypto/traits/pull/442
[#513]: https://github.com/RustCrypto/traits/pull/513

## 0.2.5 (2020-11-01)
### Fixed
- Nested macros used old deprecated names ([#360])

[#360]: https://github.com/RustCrypto/traits/pull/360

## 0.2.4 (2020-11-01)
### Fixed
- Macro expansion error ([#358])

[#358]: https://github.com/RustCrypto/traits/pull/358

## 0.2.3 (2020-11-01) [YANKED]
### Fixed
- Legacy macro wrappers ([#356])

[#356]: https://github.com/RustCrypto/traits/pull/356

## 0.2.2 (2020-11-01) [YANKED]
### Added
- `BlockCipher::{encrypt_slice, decrypt_slice}` methods ([#351])

### Changed
- Revamp macro names ([#350])

[#351]: https://github.com/RustCrypto/traits/pull/351
[#350]: https://github.com/RustCrypto/traits/pull/350

## 0.2.1 (2020-10-16)
### Added
- Re-export `generic_array` from toplevel ([#343])

### Fixed
- `dev` macro imports ([#345])

[#343]: https://github.com/RustCrypto/traits/pull/343
[#345]: https://github.com/RustCrypto/traits/pull/345

## 0.2.0 (2020-10-15) [YANKED]
### Changed
- Unify `block-cipher` and `stream-cipher` into `cipher` ([#337])

[#337]: https://github.com/RustCrypto/traits/pull/337

## 0.1.1 (2015-06-25)

## 0.1.0 (2015-06-24)
- Initial release
