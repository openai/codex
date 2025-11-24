# Changelog

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.6.0] - 2024-04-08 <a name="2.6.0"></a>
### Changed
- Fixed some incorrect minimum supported versions of dependencies that weren't caught due to
  improper `Cargo.lock`:
  * `num-traits` 0.2.14 -> 0.2.16
  * `zerocopy` 0.8.0 -> 0.8.23
  * `arbitrary` 1.3.2 -> 1.4.1

### Added
- `f16` and `bf16` now implement `Immutable` and `KnownLayout` for `zerocopy` crate. By [@usamoi].

## [2.5.0] - 2024-03-13 <a name="2.5.0"></a>
### Changed
- Updated optional dependencies to latest major versions: 
  * `zercopy` 0.6 -> 0.8
  * `rand` 0.8 -> 0.9
  * `rand_distr` 0.4 -> 0.5
  * `rkyv` 0.7 -> 0.8
  * (dev) `criterion` 0.4 -> 0.5
- Minimum supported Rust version has been changed to 1.81 due to above dependency updates.
- Minor restructuring of included license file locations to be more consistent with crates ecosystem.

### Added
- Added support for `arbitrary` crate. Fixes [#110]. By [@FL33TW00D].
- New `num-traits` implementations: `FromBytes` and `ToBytes` for `f16` and `bf16`. By [@kpreid].

### Fixed
- Suppressed unexpected_cfg lint warnings on newer versions of stable Rust.
- Resolved ambiguous rustdoc warnings due to new unstable `f16` primitive in compiler.

## [2.4.1] - 2024-04-06 <a name="2.4.1"></a>
### Fixed
- Missing macro import causing build failure on `no_std` + `alloc` feature set. Fixes [#107].
- Clippy warning on nightly rust.

## [2.4.0] - 2024-02-25 <a name="2.4.0"></a>
### Added
- Optional `rkyv` support. Fixes [#100], by [@comath].
- New `num-traits` implementations: `AsPrimitive<f16>` for `bf16` and `AsPrimitive<bf16>` for
  `f16`, allowing lossy conversions between the two types. By [@charles-r-earp].
- `Cargo.lock` added to vcs as is now recommended for library crates.
### Fixed
- Remove some unit NaN conversion sign tests due to non-deterministic hardware. Fixes [#103].
- Redundant import warnings on nightly Rust.

## [2.3.1] - 2023-06-24 <a name="2.3.1"></a>
### Fixed
- Compile error on x86 (not x86_64) targets. Fixes [#93].

## [2.3.0] - 2023-06-24 <a name="2.3.0"></a>
### Added
- Support for Kani Rust Verifier. By [@cameron1024].
- Support for `rand_distr::Distribution` implementations behind `rand_distr` optional cargo
  feature. By [@coreylowman].
- Floating point formatting options in `Display` and `Debug` implementations. By [@eiz].

### Changed
- **Breaking Change** Minimum supported Rust version is now 1.70.
- **Breaking Change** Minimum supported Rust version policy reverted to original policy of allowing
  minimum supported Rust version updates for minor releases instead of only major to avoid
  segmentation and allow optimizing hardware implementations without unnecessary major releases.
- Hardware intrinsics/assembly is finally available on stable Rust, including using hardware
  feature detection (`std` only), including:
    - AArch64 now uses FP16 hardware instructions for conversions and math operations when
    available.
    - x86/x86-64 now uses F16C hardware instructions for conversions (but no math operations) when
    available. Fixes [#54].

### Deprecated
- `use-intrinsics` cargo feature no longer used. Hardware support will now always be used whenever
  possible. A future version may output deprecation warnings if this feature is enabled.

### Fixed
- Improve code generation of `leading_zeros` functions by inlining. By [@encounter].
- `Sum` implementation of `bf16` incorrectly performed product instead of sum. By [@wx-csy].
- Compile failed when `serde` cargo feature enabled but `std` not enabled.
- Incorrect black boxing of benchmark tests.
- Rustdoc cfg display on docs.rs not getting enabled.

## [2.2.1] - 2023-01-08 <a name="2.2.1"></a>
### Changed
- Reduced unnecessary bounds checks for SIMD operations on slices. By [@Shnatsel].
- Further slice conversion optimizations for slices. Resolves [#66].

## [2.2.0] - 2022-12-30 <a name="2.2.0"></a>
### Added
- Add `serialize_as_f32` and `serialize_as_string` functions when `serde` cargo feature is enabled.
  They allowing customizing the serialization by using 
  `#[serde(serialize_with="f16::serialize_as_f32")]` attribute in serde derive macros. Closes [#60].
- Deserialize now supports deserializing from `f32`, `f64`, and string values in addition to its
  previous default deserialization. Closes [#60].

### Changed
- Add `#[inline]` on fallback functions, which improved conversion execution on non-nightly rust 
  by up to 50%. By [@Shnatsel].

## [2.1.0] - 2022-07-18 <a name="2.1.0"></a>
### Added
- Add support for target_arch `spirv`. Some traits and functions are unavailble on this
  architecture. By [@charles-r-earp].
- Add `total_cmp` method to both float types. Closes [#55], by [@joseluis].

## [2.0.0] - 2022-06-21 <a name="2.0.0"></a>
### Changed
- **Breaking Change** Minimum supported Rust version is now 1.58.
- **Breaking Change** `std` is now enabled as a default cargo feature. Disable default features to
  continue using `no_std` support.
- Migrated to Rust Edition 2021.
- Added `#[must_use]` attributes to functions, as appropriate.

### Fixed
- Fix a soundness bug with `slice::as_ptr` not correctly using mutable reference. By [@Nilstrieb].

### Added
- Added `const` conversion methods to both `f16` and `bf16`. These methods never use hardware
  intrinsics, unlike the current conversion methods, which is why they are separated into new
  methods. The following `const` methods were added:
  - `from_f32_const`
  - `from_f64_const`
  - `to_f32_const`
  - `to_f64_const`
- Added `Neg` trait support for borrowed values `&f16` and `&bf16`. By [@pthariensflame].
- Added `AsPrimitive` implementations from and to self, `usize`, and `isize`. By [@kali].

### Removed
- **Breaking Change** The deprecated `serialize` cargo feature has been removed. Use `serde` cargo
  feature instead.
- **Breaking Change** The deprecated `consts` module has been removed. Use associated constants on
  `f16` instead.
- **Breaking Change** The following deprecated functions have been removed:
  - `f16::as_bits`
  - `slice::from_bits_mut`
  - `slice::to_bits_mut`
  - `slice::from_bits`
  - `slice::to_bits`
  - `vec::from_bits`
  - `vec::to_bits`

## [1.8.2] - 2021-10-22 <a name="1.8.2"></a>
### Fixed
- Remove cargo resolver=2 from manifest to resolve errors in older versions of Rust that still
  worked with 1.8.0. Going forward, MSRV increases will be major version increases. Fixes [#48].

## [1.8.1] - 2021-10-21 - **Yanked** <a name="1.8.1"></a>
### ***Yanked***
*Not recommended due to introducing compilation error in Rust versions that worked with 1.8.0.*
### Changed
- Now uses cargo resolver version 2 to prevent dev-dependencies from enabling `std` feature on
  optional dependencies.

### Fixed
- Fixed compile failure when `std` feature is not enabled and `num-traits` is enabled under new
  resolver. Now properly uses `libm` num-traits feature.

## [1.8.0] - 2021-10-13 <a name="1.8.0"></a>
### Changed
- Now always implements `Add`, `Div`, `Mul`, `Neg`, `Rem`, and `Sub` traits. 
  Previously, these were only implemented under the `num-traits` feature. Keep in mind they still
  convert to `f32` and back in the implementation.
- Minimum supported Rust version is now 1.51.
- Made crate package [REUSE compliant](https://reuse.software/).
- Docs now use intra-doc links instead of manual (and hard to maintain) links.
- The following methods on both `f16` and `bf16` are now `const`:
  - `to_le_bytes`
  - `to_be_bytes`
  - `to_ne_bytes`
  - `from_le_bytes`
  - `from_be_bytes`
  - `from_ne_bytes`
  - `is_normal`
  - `classify`
  - `signum`

### Added
- Added optional implementations of `zerocopy` traits `AsBytes` and `FromBytes`
  under `zerocopy` cargo feature. By [@samcrow].
- Implemented the `core::iter::Product` and `core::iter::Sum` traits, with the same caveat as above
  about converting to `f32` and back under the hood.
- Added new associated const `NEG_ONE` to both `f16` and `bf16`.
- Added the following new methods on both `f16` and `bf16`:
  - `copysign`
  - `max`
  - `min`
  - `clamp`

### Fixed
- Fixed a number of minor lints discovered due to improved CI.

## [1.7.1] - 2021-01-17 <a name="1.7.1"></a>
### Fixed
- Docs.rs now generates docs for `bytemuck` and `num-traits` optional features.

## [1.7.0] - 2021-01-17 <a name="1.7.0"></a>
### Added
- Added optional implementations of `bytemuck` traits `Zeroable` and `Pod` under `bytemuck` cargo
  feature. By [@charles-r-earp].
- Added optional implementations of `num-traits` traits `ToPrimitive` and `FromPrimitive` under
  `num-traits` cargo feature. By [@charles-r-earp].
- Added implementations of `Binary`, `Octal`, `LowerHex`, and `UpperHex` string format traits to
  format raw `f16`/`bf16` bytes to string.

### Changed
- `Debug` trait implementation now formats `f16`/`bf16` as float instead of raw bytes hex. Use newly
  implemented formatting traits to format in hex instead of `Debug`. Fixes [#37].


## [1.6.0] - 2020-05-09 <a name="1.6.0"></a>
### Added
- Added `LOG2_10` and `LOG10_2` constants to both `f16` and `bf16`, which were added to `f32` and
  `f64` in the standard library in 1.43.0. By [@tspiteri].
- Added `to_le/be/ne_bytes` and `from_le/be/ne_bytes` to both `f16` and `bf16`, which were added to
  the standard library in 1.40.0. By [@bzm3r].

## [1.5.0] - 2020-03-03 <a name="1.5.0"></a>
### Added
- Added the `alloc` feature to support the `alloc` crate in `no_std` environments. By [@zserik]. The
  `vec` module is now available with either `alloc` or `std` feature.

## [1.4.1] - 2020-02-10 <a name="1.4.1"></a>
### Fixed
- Added `#[repr(transparent)]` to `f16`/`bf16` to remove undefined behavior. By [@jfrimmel].

## [1.4.0] - 2019-10-13 <a name="1.4.0"></a>
### Added
- Added a `bf16` type implementing the alternative
  [`bfloat16`](https://en.wikipedia.org/wiki/Bfloat16_floating-point_format) 16-bit floating point
  format. By [@tspiteri].
- `f16::from_bits`, `f16::to_bits`, `f16::is_nan`, `f16::is_infinite`, `f16::is_finite`,
  `f16::is_sign_positive`, and `f16::is_sign_negative` are now `const` fns.
- `slice::HalfBitsSliceExt` and `slice::HalfBitsSliceExt` extension traits have been added for
  performing efficient reinterpret casts and conversions of slices to and from `[f16]` and
  `[bf16]`.  These traits will use hardware SIMD conversion instructions when available and the
  `use-intrinsics` cargo feature is enabled.
- `vec::HalfBitsVecExt` and `vec::HalfFloatVecExt` extension traits have been added for
   performing efficient reinterpret casts to and from `Vec<f16>` and `Vec<bf16>`. These traits
   are only available with the `std` cargo feature.
- `prelude` has been added, for easy importing of most common functionality. Currently the
  prelude imports `f16`, `bf16`, and the new slice and vec extension traits.
- New associated constants on `f16` type to replace deprecated `consts` module.

### Fixed
- Software conversion (when not using `use-intrinsics` feature) now matches hardware rounding
  by rounding to nearest, ties to even. Fixes [#24], by [@tspiteri].
- NaN value conversions now behave like `f32` to `f64` conversions, retaining sign. Fixes [#23],
  by [@tspiteri].

### Changed
- Minimum rustc version bumped to 1.32.
- Runtime target host feature detection is now used if both `std` and `use-intrinsics` features are
  enabled and the compile target host does not support required features.
- When `use-intrinsics` feature is enabled, will now always compile and run without error correctly
  regardless of compile target options.

### Deprecated
- `consts` module and all its constants have been deprecated; use the associated constants on `f16`
  instead.
- `slice::from_bits` has been deprecated; use `slice::HalfBitsSliceExt::reinterpret_cast` instead.
- `slice::from_bits_mut` has been deprecated; use `slice::HalfBitsSliceExt::reinterpret_cast_mut`
  instead.
- `slice::to_bits` has been deprecated; use `slice::HalfFloatSliceExt::reinterpret_cast` instead.
- `slice::to_bits_mut` has been deprecated; use `slice::HalfFloatSliceExt::reinterpret_cast_mut`
  instead.
- `vec::from_bits` has been deprecated; use `vec::HalfBitsVecExt::reinterpret_into` instead.
- `vec::to_bits` has been deprecated; use `vec::HalfFloatVecExt::reinterpret_into` instead.

## [1.3.1] - 2019-10-04 <a name="1.3.1"></a>
### Fixed
- Corrected values of constants `EPSILON`, `MAX_10_EXP`, `MAX_EXP`, `MIN_10_EXP`, and `MIN_EXP`
  in `consts` module, as well as setting `consts::NAN` to match value of `f32::NAN` converted to
  `f16`. By [@tspiteri].

## [1.3.0] - 2018-10-02 <a name="1.3.0"></a>
### Added
- `slice::from_bits_mut` and `slice::to_bits_mut` for conversion between mutable `u16` and `f16`
  slices. Fixes [#16], by [@johannesvollmer].

## [1.2.0] - 2018-09-03 <a name="1.2.0"></a>
### Added
- `slice` and optional `vec` (only included with `std` feature) modules for conversions between
  `u16` and `f16` buffers. Fixes [#14], by [@johannesvollmer].
- `to_bits` added to replace `as_bits`. Fixes [#12], by [@tspiteri].
### Fixed
- `serde` optional dependency no longer uses its default `std` feature.
### Deprecated
- `as_bits` has been deprecated; use `to_bits` instead.
- `serialize` cargo feature is deprecated; use `serde` instead.

## [1.1.2] - 2018-07-12 <a name="1.1.2"></a>
### Fixed
- Fixed compilation error in 1.1.1 on rustc < 1.27, now compiles again on rustc >= 1.10. Fixes
  [#11].

## [1.1.1] - 2018-06-24 - **Yanked** <a name="1.1.1"></a>
### ***Yanked***
*Not recommended due to introducing compilation error on rustc versions prior to 1.27.*
### Fixed
- Fix subnormal float conversions when `use-intrinsics` is not enabled. By [@Moongoodboy-K].

## [1.1.0] - 2018-03-17 <a name="1.1.0"></a>
### Added
- Made `to_f32` and `to_f64` public. Fixes [#7], by [@PSeitz].

## [1.0.2] - 2018-01-12 <a name="1.0.2"></a>
### Changed
- Update behavior of `is_sign_positive` and `is_sign_negative` to match the IEEE754 conforming
  behavior of the standard library since Rust 1.20.0. Fixes [#3], by [@tspiteri].
- Small optimization on `is_nan` and `is_infinite` from [@tspiteri].
### Fixed
- Fix comparisons of +0 to -0 and comparisons involving negative numbers. Fixes [#2], by
  [@tspiteri].
- Fix loss of sign when converting `f16` and `f32` to `f16`, and case where `f64` NaN could be
  converted to `f16` infinity instead of NaN. Fixes [#5], by [@tspiteri].

## [1.0.1] - 2017-08-30 <a name="1.0.1"></a>
### Added
- More README documentation.
- Badges and categories in crate metadata.
### Changed
- `serde` dependency updated to 1.0 stable.
- Writing changelog manually.

## [1.0.0] - 2017-02-03 <a name="1.0.0"></a>
### Added
- Update to `serde` 0.9 and stable Rust 1.15 for `serialize` feature.

## [0.1.1] - 2017-01-08 <a name="0.1.1"></a>
### Added
- Add `serde` support under new `serialize` feature.
### Changed
- Use `no_std` for crate by default.

## 0.1.0 - 2016-03-17 <a name="0.1.0"></a>
### Added
- Initial release of `f16` type.

[#2]: https://github.com/starkat99/half-rs/issues/2
[#3]: https://github.com/starkat99/half-rs/issues/3
[#5]: https://github.com/starkat99/half-rs/issues/5
[#7]: https://github.com/starkat99/half-rs/issues/7
[#11]: https://github.com/starkat99/half-rs/issues/11
[#12]: https://github.com/starkat99/half-rs/issues/12
[#14]: https://github.com/starkat99/half-rs/issues/14
[#16]: https://github.com/starkat99/half-rs/issues/16
[#23]: https://github.com/starkat99/half-rs/issues/23
[#24]: https://github.com/starkat99/half-rs/issues/24
[#37]: https://github.com/starkat99/half-rs/issues/37
[#48]: https://github.com/starkat99/half-rs/issues/48
[#55]: https://github.com/starkat99/half-rs/issues/55
[#60]: https://github.com/starkat99/half-rs/issues/60
[#66]: https://github.com/starkat99/half-rs/issues/66
[#54]: https://github.com/starkat99/half-rs/issues/54
[#93]: https://github.com/starkat99/half-rs/issues/54
[#100]: https://github.com/starkat99/half-rs/issues/100
[#103]: https://github.com/starkat99/half-rs/issues/103
[#107]: https://github.com/starkat99/half-rs/issues/107
[#110]: https://github.com/starkat99/half-rs/issues/110

[@tspiteri]: https://github.com/tspiteri
[@PSeitz]: https://github.com/PSeitz
[@Moongoodboy-K]: https://github.com/Moongoodboy-K
[@johannesvollmer]: https://github.com/johannesvollmer
[@jfrimmel]: https://github.com/jfrimmel
[@zserik]: https://github.com/zserik
[@bzm3r]: https://github.com/bzm3r
[@charles-r-earp]: https://github.com/charles-r-earp
[@samcrow]: https://github.com/samcrow
[@pthariensflame]: https://github.com/pthariensflame
[@kali]: https://github.com/kali
[@Nilstrieb]: https://github.com/Nilstrieb
[@joseluis]: https://github.com/joseluis
[@Shnatsel]: https://github.com/Shnatsel
[@cameron1024]: https://github.com/cameron1024
[@encounter]: https://github.com/encounter
[@coreylowman]: https://github.com/coreylowman
[@wx-csy]: https://github.com/wx-csy
[@eiz]: https://github.com/eiz
[@comath]: https://github.com/comath
[@FL33TW00D]: https://github.com/FL33TW00D
[@kpreid]: https://github.com/kpreid
[@usamoi]: https://github.com/usamoi


[Unreleased]: https://github.com/starkat99/half-rs/compare/v2.6.0...HEAD
[2.6.0]: https://github.com/starkat99/half-rs/compare/v2.5.0...v2.6.0
[2.5.0]: https://github.com/starkat99/half-rs/compare/v2.4.1...v2.5.0
[2.4.1]: https://github.com/starkat99/half-rs/compare/v2.4.0...v2.4.1
[2.4.0]: https://github.com/starkat99/half-rs/compare/v2.3.1...v2.4.0
[2.3.1]: https://github.com/starkat99/half-rs/compare/v2.3.0...v2.3.1
[2.3.0]: https://github.com/starkat99/half-rs/compare/v2.2.1...v2.3.0
[2.2.1]: https://github.com/starkat99/half-rs/compare/v2.2.0...v2.2.1
[2.2.0]: https://github.com/starkat99/half-rs/compare/v2.1.0...v2.2.0
[2.1.0]: https://github.com/starkat99/half-rs/compare/v2.0.0...v2.1.0
[2.0.0]: https://github.com/starkat99/half-rs/compare/v1.8.2...v2.0.0
[1.8.2]: https://github.com/starkat99/half-rs/compare/v1.8.1...v1.8.2
[1.8.1]: https://github.com/starkat99/half-rs/compare/v1.8.0...v1.8.1
[1.8.0]: https://github.com/starkat99/half-rs/compare/v1.7.1...v1.8.0
[1.7.1]: https://github.com/starkat99/half-rs/compare/v1.7.0...v1.7.1
[1.7.0]: https://github.com/starkat99/half-rs/compare/v1.6.0...v1.7.0
[1.6.0]: https://github.com/starkat99/half-rs/compare/v1.5.0...v1.6.0
[1.5.0]: https://github.com/starkat99/half-rs/compare/v1.4.1...v1.5.0
[1.4.1]: https://github.com/starkat99/half-rs/compare/v1.4.0...v1.4.1
[1.4.0]: https://github.com/starkat99/half-rs/compare/v1.3.1...v1.4.0
[1.3.1]: https://github.com/starkat99/half-rs/compare/v1.3.0...v1.3.1
[1.3.0]: https://github.com/starkat99/half-rs/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/starkat99/half-rs/compare/v1.1.2...v1.2.0
[1.1.2]: https://github.com/starkat99/half-rs/compare/v1.1.1...v1.1.2
[1.1.1]: https://github.com/starkat99/half-rs/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/starkat99/half-rs/compare/v1.0.2...v1.1.0
[1.0.2]: https://github.com/starkat99/half-rs/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/starkat99/half-rs/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/starkat99/half-rs/compare/v0.1.1...v1.0.0
[0.1.1]: https://github.com/starkat99/half-rs/compare/v0.1.0...v0.1.1
