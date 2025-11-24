# Changelog

Entries are listed in reverse chronological order.

## 2.5.0

* Add constant-timedness note to the documentation for `CtOption::unwrap_or_else`.
* Add `CtOption::expect`.
* Add `ConstantTimeEq::ct_ne` with default implementation.
* Add new `core_hint_black_box` feature from Diane Hosfelt and Amber
  Sprenkels which utilises the original `black_box` functionality from
  when subtle was first written, which has now found it's way into the
  Rust standard library.
* Add new `const-generics` feature from @survived which adds support
  for subtle traits for generic arrays `[T; N]`.
* Add new feature for supporting `core::cmp::Ordering` for types which
  implement subtle traits, patch from @tarcieri.
* Update `rand` dependency to 0.8.

## 2.4.1

* Fix a bug in how the README was included in the documentation builds
  which caused nightly builds to break.

## 2.4.0

* Add new `ConstantTimeGreater` and `ConstantTimeLess` traits, as well
  as implementations for unsigned integers, by @isislovecruft.

## 2.3.0

* Add `impl ConstantTimeEq for Choice` by @tarcieri.
* Add `impl From<CtOption<T>> for Option<T>` by @CPerezz.  This is useful for
  handling library code that produces `CtOption`s in contexts where timing
  doesn't matter.
* Introduce an MSRV policy.

## 2.2.3

* Remove the `nightly`-only asm-based `black_box` barrier in favor of the
  volatile-based one, fixing compilation on current nightlies.

## 2.2.2

* Update README.md to clarify that 2.2 and above do not require the `nightly`
  feature.

## 2.2.1

* Adds an `or_else` combinator for `CtOption`, by @ebfull.
* Optimized `black_box` for `nightly`, by @jethrogb.
* Optimized `black_box` for `stable`, by @dsprenkels.
* Fixed CI for `no_std`, by @dsprenkels.
* Fixed fuzz target compilation, by @3for.

## 2.2.0

* Error during `cargo publish`, yanked.

## 2.1.1

* Adds the "crypto" tag to crate metadata.
* New shorter, more efficient ct_eq() for integers, contributed by Thomas Pornin.

## 2.1.0

* Adds a new `CtOption<T>` which acts as a constant-time `Option<T>`
  (thanks to @ebfull for the implementation).
* `Choice` now itself implements `ConditionallySelectable`.

## 2.0.0

* Stable version with traits reworked from 1.0.0 to interact better
  with the orphan rules.
