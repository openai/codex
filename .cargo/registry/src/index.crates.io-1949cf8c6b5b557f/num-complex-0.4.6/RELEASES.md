# Release 0.4.6 (2024-05-07)

- [Upgrade to 2021 edition, **MSRV 1.60**][121]
- [Add `const ZERO`/`ONE`/`I` and implement `ConstZero` and `ConstOne`][125]
- [Add `c32` and `c64` functions to help construct `Complex` values][126]

**Contributors**: @cuviper

[121]: https://github.com/rust-num/num-complex/pull/121
[125]: https://github.com/rust-num/num-complex/pull/125
[126]: https://github.com/rust-num/num-complex/pull/126

# Release 0.4.5 (2024-02-06)

- [Relaxed `T` bounds on `serde::Deserialize` for `Complex<T>`.][119]

**Contributors**: @cuviper, @WalterSmuts

[119]: https://github.com/rust-num/num-complex/pull/119

# Release 0.4.4 (2023-08-13)

- [Fixes NaN value for `powc` of zero][116]

**Contributors**: @cuviper, @domna

[116]: https://github.com/rust-num/num-complex/pull/116

# Release 0.4.3 (2023-01-19)

- [`Complex` now optionally supports `bytecheck` 0.6 and `rkyv` 0.7][110].

**Contributors**: @cuviper, @zyansheep

[110]: https://github.com/rust-num/num-complex/pull/110

# Release 0.4.2 (2022-06-17)

- [The new `ComplexFloat` trait][95] provides a generic abstraction between
  floating-point `T` and `Complex<T>`.
- [`Complex::exp` now handles edge cases with NaN and infinite parts][104].

**Contributors**: @cuviper, @JorisDeRidder, @obsgolem, @YakoYakoYokuYoku

[95]: https://github.com/rust-num/num-complex/pull/95
[104]: https://github.com/rust-num/num-complex/pull/104

# Release 0.4.1 (2022-04-29)

- [`Complex::from_str_radix` now returns an error for radix > 18][90], because
  'i' and 'j' as digits are ambiguous with _i_ or _j_ imaginary parts.
- [`Complex<T>` now implements `bytemuck` traits when `T` does][100].
- [`Complex::cis` creates a complex with the given phase][101], _e_<sup>_i_ θ</sup>.

**Contributors**: @bluss, @bradleyharden, @cuviper, @rayhem

[90]: https://github.com/rust-num/num-complex/pull/90
[100]: https://github.com/rust-num/num-complex/pull/100
[101]: https://github.com/rust-num/num-complex/pull/101

# Release 0.4.0 (2021-03-05)

- `rand` support has been updated to 0.8, requiring Rust 1.36.

**Contributors**: @cuviper

# Release 0.3.1 (2020-10-29)

- Clarify the license specification as "MIT OR Apache-2.0".

**Contributors**: @cuviper

# Release 0.3.0 (2020-06-13)

### Enhancements

- [The new "libm" feature passes through to `num-traits`][73], enabling `Float`
  features on no-`std` builds.

### Breaking Changes

- `num-complex` now requires Rust 1.31 or greater.
  - The "i128" opt-in feature was removed, now always available.
- [Updated public dependences][65]:
  - `rand` support has been updated to 0.7, requiring Rust 1.32.
- [Methods for `T: Float` now take values instead of references][82], most
  notably affecting the constructor `from_polar`.

**Contributors**: @cuviper, @SOF3, @vks

[65]: https://github.com/rust-num/num-complex/pull/65
[73]: https://github.com/rust-num/num-complex/pull/73
[82]: https://github.com/rust-num/num-complex/pull/82

# Release 0.2.4 (2020-01-09)

- [`Complex::new` is now a `const fn` for Rust 1.31 and later][63].
- [Updated the `autocfg` build dependency to 1.0][68].

**Contributors**: @burrbull, @cuviper, @dingelish

[63]: https://github.com/rust-num/num-complex/pull/63
[68]: https://github.com/rust-num/num-complex/pull/68

# Release 0.2.3 (2019-06-11)

- [`Complex::sqrt()` is now more accurate for negative reals][60].
- [`Complex::cbrt()` computes the principal cube root][61].

**Contributors**: @cuviper

[60]: https://github.com/rust-num/num-complex/pull/60
[61]: https://github.com/rust-num/num-complex/pull/61

# Release 0.2.2 (2019-06-10)

- [`Complex::l1_norm()` computes the Manhattan distance from the origin][43].
- [`Complex::fdiv()` and `finv()` use floating-point for inversion][41], which
  may avoid overflows for some inputs, at the cost of trigonometric rounding.
- [`Complex` now implements `num_traits::MulAdd` and `MulAddAssign`][44].
- [`Complex` now implements `Zero::set_zero` and `One::set_one`][57].
- [`Complex` now implements `num_traits::Pow` and adds `powi` and `powu`][56].

**Contributors**: @adamnemecek, @cuviper, @ignatenkobrain, @Schultzer

[41]: https://github.com/rust-num/num-complex/pull/41
[43]: https://github.com/rust-num/num-complex/pull/43
[44]: https://github.com/rust-num/num-complex/pull/44
[56]: https://github.com/rust-num/num-complex/pull/56
[57]: https://github.com/rust-num/num-complex/pull/57

# Release 0.2.1 (2018-10-08)

- [`Complex` now implements `ToPrimitive`, `FromPrimitive`, `AsPrimitive`, and `NumCast`][33].

**Contributors**: @cuviper, @termoshtt

[33]: https://github.com/rust-num/num-complex/pull/33

# Release 0.2.0 (2018-05-24)

### Enhancements

- [`Complex` now implements `num_traits::Inv` and `One::is_one`][17].
- [`Complex` now implements `Sum` and `Product`][11].
- [`Complex` now supports `i128` and `u128` components][27] with Rust 1.26+.
- [`Complex` now optionally supports `rand` 0.5][28], implementing the
  `Standard` distribution and [a generic `ComplexDistribution`][30].
- [`Rem` with a scalar divisor now avoids `norm_sqr` overflow][25].

### Breaking Changes

- [`num-complex` now requires rustc 1.15 or greater][16].
- [There is now a `std` feature][22], enabled by default, along with the
  implication that building *without* this feature makes this a `#![no_std]`
  crate.  A few methods now require `FloatCore`, and the remaining methods
  based on `Float` are only supported with `std`.
- [The `serde` dependency has been updated to 1.0][7], and `rustc-serialize`
  is no longer supported by `num-complex`.

**Contributors**: @clarcharr, @cuviper, @shingtaklam1324, @termoshtt

[7]: https://github.com/rust-num/num-complex/pull/7
[11]: https://github.com/rust-num/num-complex/pull/11
[16]: https://github.com/rust-num/num-complex/pull/16
[17]: https://github.com/rust-num/num-complex/pull/17
[22]: https://github.com/rust-num/num-complex/pull/22
[25]: https://github.com/rust-num/num-complex/pull/25
[27]: https://github.com/rust-num/num-complex/pull/27
[28]: https://github.com/rust-num/num-complex/pull/28
[30]: https://github.com/rust-num/num-complex/pull/30


# Release 0.1.43 (2018-03-08)

- [Fix a usage typo in README.md][20].

**Contributors**: @shingtaklam1324

[20]: https://github.com/rust-num/num-complex/pull/20


# Release 0.1.42 (2018-02-07)

- [num-complex now has its own source repository][num-356] at [rust-num/num-complex][home].

**Contributors**: @cuviper

[home]: https://github.com/rust-num/num-complex
[num-356]: https://github.com/rust-num/num/pull/356


# Prior releases

No prior release notes were kept.  Thanks all the same to the many
contributors that have made this crate what it is!

