# Change Log
All notable changes to this project will be documented in this file.


## 2.2.0
* Add support for deriving traits on `repr(packed)` types ([#84]).
* Fix bug with `Debug` bounds ([#83]).
* Migrate documentation to `mdbook` and fix issues found in examples ([#83]).

## 2.1.3
* Fix Clippy warning ([#81]).

## 2.1.2
* Fix bug when used in combination with other attributes ([#79]).

## 2.1.1
* Improve error reporting. ([#70])
* Fix a Clippy warning in generated code. ([#71]).

## 2.1.0
* `feature_allow_slow_enum` is not required anymore on `enum` with `PartialEq`. ([#64])
* `PartialEq` generates more efficient code for C-like `enum`. ([#65])
* Fix issue with deriving `Hash` on generic `enums` #68. ([#68])

## 2.0.2
* Fix a bug with `format_with` on `Debug` derives with generic types with trait bounds.

## 2.0.1
* Fix a hygiene bug with `Debug`. ([#60])

## 2.0.0
This release should be compatible with version 1.*, but now requires rustc version 1.34 or later.
* Update `syn`, `quote`, and `proc-macro2` dependencies. ([#59])

## 1.0.4
This is the last version to support rustc versions 1.15 to 1.33.

* Implement `PartialOrd` and `Ord` deriving.

## 1.0.3
* Do not require `syn`'s `full` feature anymore. ([#38], [#45])
* Fix an issue with using `#[derivative(Debug(format_with = "…"))]` on non-generic types. ([#40])
* Fix some warnings in the library with recent versions of `rustc`.
* Fix some `clippy::pedantic` warnings in generated code. ([#46])

## 1.0.2
* Add `use_core` feature to make `Derivative` usable in `core` crates.

## 1.0.1
* Updated `syn` to `0.15`. ([#25])
* Updated `quote` to `0.6`. ([#25])

## 1.0.0
* Make stable.

## 0.3.1
* Fix a warning in `derivative(Debug)`.
* Remove all `feature`s, this makes the crate usable on `beta`.

[#25]: https://github.com/mcarton/rust-derivative/issues/25
[#38]: https://github.com/mcarton/rust-derivative/pull/38
[#40]: https://github.com/mcarton/rust-derivative/pull/40
[#45]: https://github.com/mcarton/rust-derivative/pull/45
[#46]: https://github.com/mcarton/rust-derivative/pull/46
[#59]: https://github.com/mcarton/rust-derivative/pull/59
[#60]: https://github.com/mcarton/rust-derivative/pull/60
[#61]: https://github.com/mcarton/rust-derivative/pull/61
[#64]: https://github.com/mcarton/rust-derivative/pull/64
[#65]: https://github.com/mcarton/rust-derivative/pull/65
[#68]: https://github.com/mcarton/rust-derivative/pull/68
[#70]: https://github.com/mcarton/rust-derivative/pull/70
[#71]: https://github.com/mcarton/rust-derivative/pull/71
[#79]: https://github.com/mcarton/rust-derivative/pull/79
[#81]: https://github.com/mcarton/rust-derivative/pull/81
[#83]: https://github.com/mcarton/rust-derivative/pull/83
[#84]: https://github.com/mcarton/rust-derivative/pull/84