# Changelog

## 1.10

* Minimum rust version is now 1.55, and the non-const-generic impls are removed.
  This reduces build times by over a second on average, which is
  significant enough for a library crate.

## 1.9

* Adds a `latest_stable_rust` cargo feature, which will automatically pull in
  other cargo features related to the latest Stable version of rust.
* Adds `ArrayVec::try_from_array_len`
* Adds `TinyVec::into_vec` and `TinyVec::into_boxed_slice`
* Adds support for `generic-array` crate
* Adds support for the `borsh` crate

## 1.8.1

* [e00E](https://github.com/e00E) updated the rustc features so that they all
  correctly depend on the lower version feature.
  [pr 199](https://github.com/Lokathor/tinyvec/pull/199)

## 1.8

* [Fuuzetsu](https://github.com/Fuuzetsu) added the `ArrayVec::as_inner` method.
  [pr 197](https://github.com/Lokathor/tinyvec/pull/197)

## 1.7

* [Fuuzetsu](https://github.com/Fuuzetsu) added the `rustc_1_61` cargo feature, which adds the `retain_mut` method.
  [pr 198](https://github.com/Lokathor/tinyvec/pull/198) 

## 1.6.1

* [e00E](https://github.com/e00E) fixed the Arbitrary impl to work on Stable
  without using a feature gate.
  [pr 180](https://github.com/Lokathor/tinyvec/pull/180)

## 1.6.0

* [i509VCB](https://github.com/i509VCB) added the `try_` functions for fallable reallocation.
  [pr 158](https://github.com/Lokathor/tinyvec/pull/158)
* [ajtribick](https://github.com/ajtribick) added more error impls to `TryFromSliceError`.
  [pr 160](https://github.com/Lokathor/tinyvec/pull/160)
* The `std` feature now automatically enables the `alloc` feature as well.

## 1.5.1

* [madsmtm](https://github.com/madsmtm) fixed an error with the `alloc` feature on very old rustc versions.
  [pr 154](https://github.com/Lokathor/tinyvec/pull/154)

## 1.5.0

* [eeeebbbbrrrr](https://github.com/eeeebbbbrrrr) added an impl for [std::io::Write](https://doc.rust-lang.org/std/io/trait.Write.html) to `TinyVec` when the element type is `u8`.
  This is gated behind the new `std` feature.
  [pr 152](https://github.com/Lokathor/tinyvec/pull/152)

## 1.4.0

* [saethlin](https://github.com/saethlin) stabilized the usage of const generics and array map with the `rustc_1_55` feature.
  [pr 149](https://github.com/Lokathor/tinyvec/pull/149)

## 1.3.1

* Improved the performance of the `clone_from` method [pr 144](https://github.com/Lokathor/tinyvec/pull/144)

## 1.3.0

* [jeffa5](https://github.com/jeffa5) added arbitrary implementations for `TinyVec` and `ArrayVec` [pr 146](https://github.com/Lokathor/tinyvec/pull/146).
* [elomatreb](https://github.com/elomatreb) implemented `DoubleEndedIterator` for `TinyVecIterator` [pr 145](https://github.com/Lokathor/tinyvec/pull/145).

## 1.2.0

* [Cryptjar](https://github.com/Cryptjar) removed the `A:Array` bound on the struct of `ArrayVec<A:Array>`,
  and added the `from_array_empty` method, which is a `const fn` constructor
  [pr 141](https://github.com/Lokathor/tinyvec/pull/141).

## 1.1.1

* [saethlin](https://github.com/saethlin) contributed many PRs (
  [127](https://github.com/Lokathor/tinyvec/pull/127),
  [128](https://github.com/Lokathor/tinyvec/pull/128),
  [129](https://github.com/Lokathor/tinyvec/pull/129),
  [131](https://github.com/Lokathor/tinyvec/pull/131),
  [132](https://github.com/Lokathor/tinyvec/pull/132)
  ) to help in several benchmarks.

## 1.1.0

* [slightlyoutofphase](https://github.com/slightlyoutofphase)
  added "array splat" style syntax to the `array_vec!` and `tiny_vec!` macros.
  You can now write `array_vec![true; 5]` and get a length 5 array vec full of `true`,
  just like normal array initialization allows. Same goes for `tiny_vec!`.
  ([pr 118](https://github.com/Lokathor/tinyvec/pull/118))
* [not-a-seagull](https://github.com/not-a-seagull)
  added `ArrayVec::into_inner` so that you can get the array out of an `ArrayVec`.
  ([pr 124](https://github.com/Lokathor/tinyvec/pull/124))

## 1.0.2

* Added license files for the MIT and Apache-2.0 license options.

## 1.0.1

* Display additional features in the [docs.rs/tinyvec](https://docs.rs/tinyvec) documentation.

## 1.0.0

Initial Stable Release.
