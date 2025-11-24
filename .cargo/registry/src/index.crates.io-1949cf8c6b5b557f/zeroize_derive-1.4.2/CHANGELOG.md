# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 1.4.2 (2023-03-30)
### Changed
- Inject where clauses; skip unused ([#882])

[#882]: https://github.com/RustCrypto/utils/pull/882

## 1.4.1 (2023-03-27)
### Changed
- Do not automatically inject bounds ([#879])

[#879]: https://github.com/RustCrypto/utils/pull/879

## 1.4.0 (2023-03-26)
### Changed
- 2021 edition upgrade; MSRV 1.56 ([#869])
- Bump `syn` to v2 ([#858])

### Removed
- `synstructure` dependency ([#858])

[#858]: https://github.com/RustCrypto/utils/pull/858
[#869]: https://github.com/RustCrypto/utils/pull/869

## 1.3.3 (2022-11-30)
### Fixed
- Deriving `ZeroizeOnDrop` on items with generics ([#787])

[#787]: https://github.com/RustCrypto/utils/pull/787

## 1.3.2 (2022-02-18)
### Fixed
- Min versions build ([#732])

[#732]: https://github.com/RustCrypto/utils/pull/732

## 1.3.1 (2021-01-14) [YANKED]
### Removed
- `ZeroizeOnDrop` implementation for `#[zeroize(drop)]` ([#715])

[#715]: https://github.com/RustCrypto/utils/pull/715

## 1.3.0 (2021-01-14) [YANKED]
### Added
- `#[zeroize(bound = "T: MyTrait")]` ([#663])
- Custom derive for `ZeroizeOnDrop` ([#699], [#700])

[#663]: https://github.com/RustCrypto/utils/pull/663
[#699]: https://github.com/RustCrypto/utils/pull/699
[#700]: https://github.com/RustCrypto/utils/pull/700

## 1.2.2 (2021-11-04) [YANKED]
### Added
- `#[zeroize(skip)]` attribute ([#654])

[#654]: https://github.com/RustCrypto/utils/pull/654

## 1.2.1 (2021-11-04)
### Changed
- Moved to `RustCrypto/utils` repository

## 1.2.0 (2021-09-21) [YANKED]
### Changed
- Bump MSRV to 1.51+
- Reject `#[zeroize(drop)]` on struct/enum fields, enum variants

## 1.1.1 (2021-10-09) [YANKED]
### Changed
- Backport 1.2.0 `#[zeroize(drop)]` fixes but with a 1.47+ MSRV.

## 1.1.0 (2021-04-19)
### Changed
- Bump MSRV to 1.47+

## 1.0.1 (2019-09-15)
### Added
- Add docs for the `Zeroize` proc macro

## 1.0.0 (2019-10-13)

- Initial 1.0 release
