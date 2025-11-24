# Release 0.4.2 (2024-05-07)

- [Upgrade to 2021 edition, **MSRV 1.60**][126]
- [Add `Ratio::approximate_float_unsigned` to convert `FloatCore` types to unsigned][109]
- [Add `const ZERO` and `ONE`, and implement `num_traits::ConstZero` and `ConstOne`][128]
- [Add `Ratio::into_raw` to deconstruct the numerator and denominator][129]

**Contributors**: @cuviper, @Enyium, @flavioroth, @waywardmonkeys

[109]: https://github.com/rust-num/num-rational/pull/109
[126]: https://github.com/rust-num/num-rational/pull/126
[128]: https://github.com/rust-num/num-rational/pull/128
[129]: https://github.com/rust-num/num-rational/pull/129

# Release 0.4.1 (2022-06-23)

- [Fewer `clone` calls are used when reducing a new `Ratio<T>`][98].
- [Conversions to floating point are better at avoiding underflow][104].
- [`Ratio<T>` now implements `Default`][105], returning a zero value.

**Contributors**: @cuviper, @lemmih, @MattX

[98]: https://github.com/rust-num/num-rational/pull/98
[104]: https://github.com/rust-num/num-rational/pull/104
[105]: https://github.com/rust-num/num-rational/pull/105

# Release 0.4.0 (2021-03-05)

- The optional `num-bigint` dependency is now 0.4.
- [The `Rational` alias for `Ratio<usize>` is now deprecated][92]. It is
  recommended to use specific type sizes for numeric computation, like
  `Rational32` and `Rational64`.

**Contributors**: @cuviper, @vks

[92]: https://github.com/rust-num/num-rational/pull/92

# Release 0.3.2 (2020-11-06)

- [Fix always rebuilding with --remap-path-prefix][88]

**Contributors**: @Nemo157

[88]: https://github.com/rust-num/num-rational/pull/88

# Release 0.3.1 (2020-10-29)

- [Handle to_f64() with raw division by zero][83].
- [Better document panic behaviour][84].
- Clarify the license specification as "MIT OR Apache-2.0".

**Contributors**: @cuviper, @zetok

[83]: https://github.com/rust-num/num-rational/pull/83
[84]: https://github.com/rust-num/num-rational/pull/84

# Release 0.3.0 (2020-06-13)

### Enhancements

- [`Ratio` now implements `ToPrimitive`][52].
- [`Ratio` now implements additional formatting traits][56]:
  - `Binary`, `Octal`, `LowerHex`, `UpperHex`, `LowerExp`, `UpperExp`
- [The `Pow` implementations have been expanded][70].
  - `Pow<BigInt>` and `Pow<BigUint>` are now implemented.
  - `Pow<_> for &Ratio<T>` now uses `&T: Pow`.
  - The inherent `pow` method now uses `&T: Pow`.

### Breaking Changes

- [`num-rational` now requires Rust 1.31 or greater][66].
  - The "i128" opt-in feature was removed, now always available.
- [The "num-bigint-std" feature replaces "bigint" with `std` enabled][80].
  - The "num-bigint" feature without `std` uses `alloc` on Rust 1.36+.

**Contributors**: @cuviper, @MattX, @maxbla

[52]: https://github.com/rust-num/num-rational/pull/52
[56]: https://github.com/rust-num/num-rational/pull/56
[66]: https://github.com/rust-num/num-rational/pull/66
[70]: https://github.com/rust-num/num-rational/pull/70
[80]: https://github.com/rust-num/num-rational/pull/80

# Release 0.2.4 (2020-03-17)

- [Fixed `CheckedDiv` when both dividend and divisor are 0][74].
- [Fixed `CheckedDiv` with `min_value()` numerators][76].

[74]: https://github.com/rust-num/num-rational/pull/74
[76]: https://github.com/rust-num/num-rational/pull/76

# Release 0.2.3 (2020-01-09)

- [`Ratio` now performs earlier reductions to avoid overflow with `+-*/%` operators][42].
- [`Ratio::{new_raw, numer, denom}` are now `const fn` for Rust 1.31 and later][48].
- [Updated the `autocfg` build dependency to 1.0][63].

**Contributors**: @cuviper, @dingelish, @jimbo1qaz, @maxbla

[42]: https://github.com/rust-num/num-rational/pull/42
[48]: https://github.com/rust-num/num-rational/pull/48
[63]: https://github.com/rust-num/num-rational/pull/63

# Release 0.2.2 (2019-06-10)

- [`Ratio` now implements `Zero::set_zero` and `One::set_one`][47].

**Contributors**: @cuviper, @ignatenkobrain, @vks

[47]: https://github.com/rust-num/num-rational/pull/47

# Release 0.2.1 (2018-06-22)

- Maintenance release to fix `html_root_url`.

# Release 0.2.0 (2018-06-19)

### Enhancements

- [`Ratio` now implements `One::is_one` and the `Inv` trait][19].
- [`Ratio` now implements `Sum` and `Product`][25].
- [`Ratio` now supports `i128` and `u128` components][29] with Rust 1.26+.
- [`Ratio` now implements the `Pow` trait][21].

### Breaking Changes

- [`num-rational` now requires rustc 1.15 or greater][18].
- [There is now a `std` feature][23], enabled by default, along with the
  implication that building *without* this feature makes this a `#![no_std]`
  crate.  A few methods now require `FloatCore` instead of `Float`.
- [The `serde` dependency has been updated to 1.0][24], and `rustc-serialize`
  is no longer supported by `num-rational`.
- The optional `num-bigint` dependency has been updated to 0.2, and should be
  enabled using the `bigint-std` feature.  In the future, it may be possible
  to use the `bigint` feature with `no_std`.

**Contributors**: @clarcharr, @cuviper, @Emerentius, @robomancer-or, @vks

[18]: https://github.com/rust-num/num-rational/pull/18
[19]: https://github.com/rust-num/num-rational/pull/19
[21]: https://github.com/rust-num/num-rational/pull/21
[23]: https://github.com/rust-num/num-rational/pull/23
[24]: https://github.com/rust-num/num-rational/pull/24
[25]: https://github.com/rust-num/num-rational/pull/25
[29]: https://github.com/rust-num/num-rational/pull/29


# Release 0.1.42 (2018-02-08)

- Maintenance release to update dependencies.


# Release 0.1.41 (2018-01-26)

- [num-rational now has its own source repository][num-356] at [rust-num/num-rational][home].
- [`Ratio` now implements `CheckedAdd`, `CheckedSub`, `CheckedMul`, and `CheckedDiv`][11].
- [`Ratio` now implements `AddAssign`, `SubAssign`, `MulAssign`, `DivAssign`, and `RemAssign`][12]
  with either `Ratio` or an integer on the right side.  The non-assignment operators now also
  accept integers as an operand.
- [`Ratio` operators now make fewer `clone()` calls][14].

Thanks to @c410-f3r, @cuviper, and @psimonyi for their contributions!

[home]: https://github.com/rust-num/num-rational
[num-356]: https://github.com/rust-num/num/pull/356
[11]: https://github.com/rust-num/num-rational/pull/11
[12]: https://github.com/rust-num/num-rational/pull/12
[14]: https://github.com/rust-num/num-rational/pull/14


# Prior releases

No prior release notes were kept.  Thanks all the same to the many
contributors that have made this crate what it is!
