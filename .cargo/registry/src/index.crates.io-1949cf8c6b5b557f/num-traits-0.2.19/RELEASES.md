# Release 0.2.19 (2024-05-03)

- [Upgrade to 2021 edition, **MSRV 1.60**][310]
- [The new `Float::clamp` limits values by minimum and maximum][305]

**Contributors**: @cuviper, @michaelciraci

[305]: https://github.com/rust-num/num-traits/pull/305
[310]: https://github.com/rust-num/num-traits/pull/310

# Release 0.2.18 (2024-02-07)

- [The new `Euclid::div_rem_euclid` and `CheckedEuclid::checked_div_rem_euclid` methods][291]
  compute and return the quotient and remainder at the same time.
- [The new `TotalOrder` trait implements the IEEE 754 `totalOrder` predicate.][295]
- [The new `ConstZero` and `ConstOne` traits offered associated constants][303],
  extending the non-const `Zero` and `One` traits for types that have constant values.

**Contributors**: @andrewjradcliffe, @cuviper, @tarcieri, @tdelabro, @waywardmonkeys

[291]: https://github.com/rust-num/num-traits/pull/291
[295]: https://github.com/rust-num/num-traits/pull/295
[303]: https://github.com/rust-num/num-traits/pull/303

# Release 0.2.17 (2023-10-07)

- [Fix a doc warning about custom classes with newer rustdoc.][286]

**Contributors**: @robamu

[286]: https://github.com/rust-num/num-traits/pull/286

# Release 0.2.16 (2023-07-20)

- [Upgrade to 2018 edition, **MSRV 1.31**][240]
- [The new `ToBytes` and `FromBytes` traits][224] convert to and from byte
  representations of a value, with little, big, and native-endian options.
- [The new `Float::is_subnormal` method checks for subnormal values][279], with
  a non-zero magnitude that is less than the normal minimum positive value.
- Several other improvements to documentation and testing.

**Contributors**: @ctrlcctrlv, @cuviper, @flier, @GuillaumeGomez, @kaidokert,
@rs017991, @vicsn

[224]: https://github.com/rust-num/num-traits/pull/224
[240]: https://github.com/rust-num/num-traits/pull/240
[279]: https://github.com/rust-num/num-traits/pull/279

# Release 0.2.15 (2022-05-02)

- [The new `Euclid` trait calculates Euclidean division][195], where the
  remainder is always positive or zero.
- [The new `LowerBounded` and `UpperBounded` traits][210] separately describe
  types with lower and upper bounds. These traits are automatically implemented
  for all fully-`Bounded` types.
- [The new `Float::copysign` method copies the sign of the argument][207] to
  to the magnitude of `self`.
- [The new `PrimInt::leading_ones` and `trailing_ones` methods][205] are the
  complement of the existing methods that count zero bits.
- [The new `PrimInt::reverse_bits` method reverses the order of all bits][202]
  of a primitive integer.
- [Improved `Num::from_str_radix` for floats][201], also [ignoring case][214].
- [`Float` and `FloatCore` use more from `libm`][196] when that is enabled.

**Contributors**: @alion02, @clarfonthey, @cuviper, @ElectronicRU,
@ibraheemdev, @SparrowLii, @sshilovsky, @tspiteri, @XAMPPRocky, @Xiretza

[195]: https://github.com/rust-num/num-traits/pull/195
[196]: https://github.com/rust-num/num-traits/pull/196
[201]: https://github.com/rust-num/num-traits/pull/201
[202]: https://github.com/rust-num/num-traits/pull/202
[205]: https://github.com/rust-num/num-traits/pull/205
[207]: https://github.com/rust-num/num-traits/pull/207
[210]: https://github.com/rust-num/num-traits/pull/210
[214]: https://github.com/rust-num/num-traits/pull/214

# Release 0.2.14 (2020-10-29)

- Clarify the license specification as "MIT OR Apache-2.0".

**Contributors**: @cuviper

# Release 0.2.13 (2020-10-29)

- [The new `OverflowingAdd`, `OverflowingSub`, and `OverflowingMul` traits][180]
  return a tuple with the operation result and a `bool` indicating overflow.
- [The "i128" feature now overrides compiler probes for that support][185].
  This may fix scenarios where `autocfg` probing doesn't work properly.
- [Casts from large `f64` values to `f32` now saturate to infinity][186]. They
  previously returned `None` because that was once thought to be undefined
  behavior, but [rust#15536] resolved that such casts are fine.
- [`Num::from_str_radix` documents requirements for radix support][192], which
  are now more relaxed than previously implied. It is suggested to accept at
  least `2..=36` without panicking, but `Err` may be returned otherwise.

**Contributors**: @cuviper, @Enet4, @KaczuH, @martin-t, @newpavlov

[180]: https://github.com/rust-num/num-traits/pull/180
[185]: https://github.com/rust-num/num-traits/pull/185
[186]: https://github.com/rust-num/num-traits/pull/186
[192]: https://github.com/rust-num/num-traits/issues/192
[rust#15536]: https://github.com/rust-lang/rust/issues/15536

# Release 0.2.12 (2020-06-11)

- [The new `WrappingNeg` trait][153] will wrap the result if it exceeds the
  boundary of the type, e.g. `i32::MIN.wrapping_neg() == i32::MIN`.
- [The new `SaturatingAdd`, `SaturatingSub`, and `SaturatingMul` traits][165]
  will saturate at the numeric bounds if the operation would overflow. These
  soft-deprecate the existing `Saturating` trait that only has addition and
  subtraction methods.
- [Added new constants for logarithms, `FloatConst::{LOG10_2, LOG2_10}`][171].

**Contributors**: @cuviper, @ocstl, @trepetti, @vallentin

[153]: https://github.com/rust-num/num-traits/pull/153
[165]: https://github.com/rust-num/num-traits/pull/165
[171]: https://github.com/rust-num/num-traits/pull/171

# Release 0.2.11 (2020-01-09)

- [Added the full circle constant τ as `FloatConst::TAU`][145].
- [Updated the `autocfg` build dependency to 1.0][148].

**Contributors**: @cuviper, @m-ou-se

[145]: https://github.com/rust-num/num-traits/pull/145
[148]: https://github.com/rust-num/num-traits/pull/148

# Release 0.2.10 (2019-11-22)

- [Updated the `libm` dependency to 0.2][144].

**Contributors**: @CryZe

[144]: https://github.com/rust-num/num-traits/pull/144

# Release 0.2.9 (2019-11-12)

- [A new optional `libm` dependency][99] enables the `Float` and `Real` traits
  in `no_std` builds.
- [The new `clamp_min` and `clamp_max`][122] limit minimum and maximum values
  while preserving input `NAN`s.
- [Fixed a panic in floating point `from_str_radix` on invalid signs][126].
- Miscellaneous documentation updates.

**Contributors**: @cuviper, @dingelish, @HeroicKatora, @jturner314, @ocstl,
@Shnatsel, @termoshtt, @waywardmonkeys, @yoanlcq

[99]: https://github.com/rust-num/num-traits/pull/99
[122]: https://github.com/rust-num/num-traits/pull/122
[126]: https://github.com/rust-num/num-traits/pull/126

# Release 0.2.8 (2019-05-21)

- [Fixed feature detection on `no_std` targets][116].

**Contributors**: @cuviper

[116]: https://github.com/rust-num/num-traits/pull/116

# Release 0.2.7 (2019-05-20)

- [Documented when `CheckedShl` and `CheckedShr` return `None`][90].
- [The new `Zero::set_zero` and `One::set_one`][104] will set values to their
  identities in place, possibly optimized better than direct assignment.
- [Documented general features and intentions of `PrimInt`][108].

**Contributors**: @cuviper, @dvdhrm, @ignatenkobrain, @lcnr, @samueltardieu

[90]: https://github.com/rust-num/num-traits/pull/90
[104]: https://github.com/rust-num/num-traits/pull/104
[108]: https://github.com/rust-num/num-traits/pull/108

# Release 0.2.6 (2018-09-13)

- [Documented that `pow(0, 0)` returns `1`][79].  Mathematically, this is not
  strictly defined, but the current behavior is a pragmatic choice that has
  precedent in Rust `core` for the primitives and in many other languages.
- [The new `WrappingShl` and `WrappingShr` traits][81] will wrap the shift count
  if it exceeds the bit size of the type.

**Contributors**: @cuviper, @edmccard, @meltinglava

[79]: https://github.com/rust-num/num-traits/pull/79
[81]: https://github.com/rust-num/num-traits/pull/81

# Release 0.2.5 (2018-06-20)

- [Documentation for `mul_add` now clarifies that it's not always faster.][70]
- [The default methods in `FromPrimitive` and `ToPrimitive` are more robust.][73]

**Contributors**: @cuviper, @frewsxcv

[70]: https://github.com/rust-num/num-traits/pull/70
[73]: https://github.com/rust-num/num-traits/pull/73

# Release 0.2.4 (2018-05-11)

- [Support for 128-bit integers is now automatically detected and enabled.][69]
  Setting the `i128` crate feature now causes the build script to panic if such
  support is not detected.

**Contributors**: @cuviper

[69]: https://github.com/rust-num/num-traits/pull/69

# Release 0.2.3 (2018-05-10)

- [The new `CheckedNeg` and `CheckedRem` traits][63] perform checked `Neg` and
  `Rem`, returning `Some(output)` or `None` on overflow.
- [The `no_std` implementation of `FloatCore::to_degrees` for `f32`][61] now
  uses a constant for greater accuracy, mirroring [rust#47919].  (With `std` it
  just calls the inherent `f32::to_degrees` in the standard library.)
- [The new `MulAdd` and `MulAddAssign` traits][59] perform a fused multiply-
  add.  For integer types this is just a convenience, but for floating point
  types this produces a more accurate result than the separate operations.
- [All applicable traits are now implemented for 128-bit integers][60] starting
  with Rust 1.26, enabled by the new `i128` crate feature.  The `FromPrimitive`
  and `ToPrimitive` traits now also have corresponding 128-bit methods, which
  default to converting via 64-bit integers for compatibility.

**Contributors**: @cuviper, @LEXUGE, @regexident, @vks

[59]: https://github.com/rust-num/num-traits/pull/59
[60]: https://github.com/rust-num/num-traits/pull/60
[61]: https://github.com/rust-num/num-traits/pull/61
[63]: https://github.com/rust-num/num-traits/pull/63
[rust#47919]: https://github.com/rust-lang/rust/pull/47919

# Release 0.2.2 (2018-03-18)

- [Casting from floating point to integers now returns `None` on overflow][52],
  avoiding [rustc's undefined behavior][rust-10184]. This applies to the `cast`
  function and the traits `NumCast`, `FromPrimitive`, and `ToPrimitive`.

**Contributors**: @apopiak, @cuviper, @dbarella

[52]: https://github.com/rust-num/num-traits/pull/52
[rust-10184]: https://github.com/rust-lang/rust/issues/10184


# Release 0.2.1 (2018-03-01)

- [The new `FloatCore` trait][32] offers a subset of `Float` for `#![no_std]` use.
  [This includes everything][41] except the transcendental functions and FMA.
- [The new `Inv` trait][37] returns the multiplicative inverse, or reciprocal.
- [The new `Pow` trait][37] performs exponentiation, much like the existing `pow`
  function, but with generic exponent types.
- [The new `One::is_one` method][39] tests if a value equals 1.  Implementers
  should override this method if there's a more efficient way to check for 1,
  rather than comparing with a temporary `one()`.

**Contributors**: @clarcharr, @cuviper, @vks

[32]: https://github.com/rust-num/num-traits/pull/32
[37]: https://github.com/rust-num/num-traits/pull/37
[39]: https://github.com/rust-num/num-traits/pull/39
[41]: https://github.com/rust-num/num-traits/pull/41


# Release 0.2.0 (2018-02-06)

- **breaking change**: [There is now a `std` feature][30], enabled by default, along
  with the implication that building *without* this feature makes this a
  `#![no_std]` crate.
  - The `Float` and `Real` traits are only available when `std` is enabled.
  - Otherwise, the API is unchanged, and num-traits 0.1.43 now re-exports its
    items from num-traits 0.2 for compatibility (the [semver-trick]).

**Contributors**: @cuviper, @termoshtt, @vks

[semver-trick]: https://github.com/dtolnay/semver-trick
[30]: https://github.com/rust-num/num-traits/pull/30


# Release 0.1.43 (2018-02-06)

- All items are now [re-exported from num-traits 0.2][31] for compatibility.

[31]: https://github.com/rust-num/num-traits/pull/31


# Release 0.1.42 (2018-01-22)

- [num-traits now has its own source repository][num-356] at [rust-num/num-traits][home].
- [`ParseFloatError` now implements `Display`][22].
- [The new `AsPrimitive` trait][17] implements generic casting with the `as` operator.
- [The new `CheckedShl` and `CheckedShr` traits][21] implement generic
  support for the `checked_shl` and `checked_shr` methods on primitive integers.
- [The new `Real` trait][23] offers a subset of `Float` functionality that may be applicable to more
  types, with a blanket implementation for all existing `T: Float` types.

Thanks to @cuviper, @Enet4, @fabianschuiki, @svartalf, and @yoanlcq for their contributions!

[home]: https://github.com/rust-num/num-traits
[num-356]: https://github.com/rust-num/num/pull/356
[17]: https://github.com/rust-num/num-traits/pull/17
[21]: https://github.com/rust-num/num-traits/pull/21
[22]: https://github.com/rust-num/num-traits/pull/22
[23]: https://github.com/rust-num/num-traits/pull/23


# Prior releases

No prior release notes were kept.  Thanks all the same to the many
contributors that have made this crate what it is!
