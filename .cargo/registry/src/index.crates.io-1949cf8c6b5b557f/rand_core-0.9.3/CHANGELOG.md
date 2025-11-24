# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.9.3] — 2025-02-29
### Other
- Remove `zerocopy` dependency (#1607)
- Deprecate `rand_core::impls::fill_via_u32_chunks`, `fill_via_u64_chunks` (#1607)

## [0.9.2] - 2025-02-22
### API changes
- Relax `Sized` bound on impls of `TryRngCore`, `TryCryptoRng` and `UnwrapMut` (#1593)
- Add `UnwrapMut::re` to reborrow the inner rng with a tighter lifetime (#1595)

## [0.9.1] - 2025-02-16
### API changes
- Add `TryRngCore::unwrap_mut`, providing an impl of `RngCore` over `&mut rng` (#1589)

## [0.9.0] - 2025-01-27
### Dependencies and features
- Bump the MSRV to 1.63.0 (#1207, #1246, #1269, #1341, #1416, #1536); note that 1.60.0 may work for dependents when using `--ignore-rust-version`
- Update to `getrandom` v0.3.0 (#1558)
- Use `zerocopy` to replace some `unsafe` code (#1349, #1393, #1446, #1502)
- Rename feature `serde1` to `serde` (#1477)
- Rename feature `getrandom` to `os_rng` (#1537)

### API changes
- Allow `rand_core::impls::fill_via_u*_chunks` to mutate source (#1182)
- Add fn `RngCore::read_adapter` implementing `std::io::Read` (#1267)
- Add trait `CryptoBlockRng: BlockRngCore`; make `trait CryptoRng: RngCore` replacing `CryptoRngCore` (#1273)
- Add traits `TryRngCore`, `TryCryptoRng` (#1424, #1499)
- Rename `fn SeedableRng::from_rng` -> `try_from_rng` and add infallible variant `fn from_rng` (#1424)
- Rename `fn SeedableRng::from_entropy` -> `from_os_rng` and add fallible variant `fn try_from_os_rng` (#1424)
- Add bounds `Clone` and `AsRef` to associated type `SeedableRng::Seed` (#1491)

## [0.6.4] - 2022-09-15
- Fix unsoundness in `<BlockRng64 as RngCore>::next_u32` (#1160)
- Reduce use of `unsafe` and improve gen_bytes performance (#1180)
- Add `CryptoRngCore` trait (#1187, #1230)

## [0.6.3] - 2021-06-15
### Changed
- Improved bound for `serde` impls on `BlockRng` (#1130)
- Minor doc additions (#1118)

## [0.6.2] - 2021-02-12
### Fixed
- Fixed assertions in `le::read_u32_into` and `le::read_u64_into` which could
  have allowed buffers not to be fully populated (#1096)

## [0.6.1] - 2021-01-03
### Fixed
- Avoid panic when using `RngCore::seed_from_u64` with a seed which is not a
  multiple of four (#1082)
### Other
- Enable all stable features in the playground (#1081)

## [0.6.0] - 2020-12-08
### Breaking changes
- Bump MSRV to 1.36, various code improvements (#1011)
- Update to getrandom v0.2 (#1041)
- Fix: `next_u32_via_fill` and `next_u64_via_fill` now use LE as documented (#1061)

### Other
- Reduce usage of `unsafe` (#962, #963, #1011)
- Annotate feature-gates in documentation (#1019)
- Document available error codes (#1061)
- Various documentation tweaks
- Fix some clippy warnings (#1036)
- Apply rustfmt (#926)

## [0.5.1] - 2019-08-28
- `OsRng` added to `rand_core` (#863)
- `Error::INTERNAL_START` and `Error::CUSTOM_START` constants (#864)
- `Error::raw_os_error` method (#864)
- `Debug` and `Display` formatting for `getrandom` error codes without `std` (#864)
### Changed
- `alloc` feature in `no_std` is available since Rust 1.36 (#856)
- Added `#[inline]` to `Error` conversion methods (#864)

## [0.5.0] - 2019-06-06
### Changed
- Enable testing with Miri and fix incorrect pointer usages (#779, #780, #781, #783, #784)
- Rewrite `Error` type and adjust API (#800)
- Adjust usage of `#[inline]` for `BlockRng` and `BlockRng64`

## [0.4.0] - 2019-01-24
### Changed
- Disable the `std` feature by default (#702)

## [0.3.0] - 2018-09-24
### Added
- Add `SeedableRng::seed_from_u64` for convenient seeding. (#537)

## [0.2.1] - 2018-06-08
### Added
- References to a `CryptoRng` now also implement `CryptoRng`. (#470)

## [0.2.0] - 2018-05-21
### Changed
- Enable the `std` feature by default. (#409)
- Remove `BlockRng{64}::inner` and `BlockRng::inner_mut`; instead making `core` public
- Change `BlockRngCore::Results` bound to also require `AsMut<[Self::Item]>`. (#419)
### Added
- Add `BlockRng{64}::index` and `BlockRng{64}::generate_and_set`. (#374, #419)
- Implement `std::io::Read` for RngCore. (#434)

## [0.1.0] - 2018-04-17
(Split out of the Rand crate, changes here are relative to rand 0.4.2.)
### Added
- `RngCore` and `SeedableRng` are now part of `rand_core`. (#288)
- Add modules to help implementing RNGs `impl` and `le`. (#209, #228)
- Add `Error` and `ErrorKind`. (#225)
- Add `CryptoRng` marker trait. (#273)
- Add `BlockRngCore` trait. (#281)
- Add `BlockRng` and `BlockRng64` wrappers to help implementations. (#281, #325)
- Add `RngCore::try_fill_bytes`. (#225)
### Changed
- Revise the `SeedableRng` trait. (#233)
- Remove default implementations for `RngCore::next_u64` and `RngCore::fill_bytes`. (#288)

## [0.0.1] - 2017-09-14 (yanked)
Experimental version as part of the rand crate refactor.
