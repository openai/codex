# Release 0.4.6 (2024-06-27)

- [Fixed compilation on `x86_64-unknown-linux-gnux32`.][312]

**Contributors**: @cuviper, @ralphtandetzky, @yhx-12243

[312]: https://github.com/rust-num/num-bigint/pull/312

# Release 0.4.5 (2024-05-06)

- [Upgrade to 2021 edition, **MSRV 1.60**][292]
- [Add `const ZERO` and implement `num_traits::ConstZero`][298]
- [Add `modinv` methods for the modular inverse][288]
- [Optimize multiplication with imbalanced operands][295]
- [Optimize scalar division on x86 and x86-64][236]

**Contributors**: @cuviper, @joelonsql, @waywardmonkeys

[236]: https://github.com/rust-num/num-bigint/pull/236
[288]: https://github.com/rust-num/num-bigint/pull/288
[292]: https://github.com/rust-num/num-bigint/pull/292
[295]: https://github.com/rust-num/num-bigint/pull/295
[298]: https://github.com/rust-num/num-bigint/pull/298

# Release 0.4.4 (2023-08-22)

- [Implemented `From<bool>` for `BigInt` and `BigUint`.][239]
- [Implemented `num_traits::Euclid` and `CheckedEuclid` for `BigInt` and `BigUint`.][245]
- [Implemented ties-to-even for `BigInt` and `BigUint::to_f32` and `to_f64`.][271]
- [Implemented `num_traits::FromBytes` and `ToBytes` for `BigInt` and `BigUint`.][276]
- Limited pre-allocation from serde size hints against potential OOM.
- Miscellaneous other code cleanups and maintenance tasks.

**Contributors**: @AaronKutch, @archseer, @cuviper, @dramforever, @icecream17,
@icedrocket, @janmarthedal, @jaybosamiya, @OliveIsAWord, @PatrickNorton,
@smoelius, @waywardmonkeys

[239]: https://github.com/rust-num/num-bigint/pull/239
[245]: https://github.com/rust-num/num-bigint/pull/245
[271]: https://github.com/rust-num/num-bigint/pull/271
[276]: https://github.com/rust-num/num-bigint/pull/276

# Release 0.4.3 (2021-11-02)

- [GHSA-v935-pqmr-g8v9]: [Fix unexpected panics in multiplication.][228]

**Contributors**: @arvidn, @cuviper, @guidovranken

[228]: https://github.com/rust-num/num-bigint/pull/228
[GHSA-v935-pqmr-g8v9]: https://github.com/rust-num/num-bigint/security/advisories/GHSA-v935-pqmr-g8v9

# Release 0.4.2 (2021-09-03)

- [Use explicit `Integer::div_ceil` to avoid the new unstable method.][219]

**Contributors**: @catenacyber, @cuviper

[219]: https://github.com/rust-num/num-bigint/pull/219

# Release 0.4.1 (2021-08-27)

- [Fixed scalar divide-by-zero panics.][200]
- [Implemented `DoubleEndedIterator` for `U32Digits` and `U64Digits`.][208]
- [Optimized multiplication to avoid unnecessary allocations.][199]
- [Optimized string formatting for very large values.][216]

**Contributors**: @cuviper, @PatrickNorton

[199]: https://github.com/rust-num/num-bigint/pull/199
[200]: https://github.com/rust-num/num-bigint/pull/200
[208]: https://github.com/rust-num/num-bigint/pull/208
[216]: https://github.com/rust-num/num-bigint/pull/216

# Release 0.4.0 (2021-03-05)

### Breaking Changes

- Updated public dependences on [arbitrary, quickcheck][194], and [rand][185]:
  - `arbitrary` support has been updated to 1.0, requiring Rust 1.40.
  - `quickcheck` support has been updated to 1.0, requiring Rust 1.46.
  - `rand` support has been updated to 0.8, requiring Rust 1.36.
- [`Debug` now shows plain numeric values for `BigInt` and `BigUint`][195],
  rather than the raw list of internal digits.

**Contributors**: @cuviper, @Gelbpunkt

[185]: https://github.com/rust-num/num-bigint/pull/185
[194]: https://github.com/rust-num/num-bigint/pull/194
[195]: https://github.com/rust-num/num-bigint/pull/195

# Release 0.3.3 (2021-09-03)

- [Use explicit `Integer::div_ceil` to avoid the new unstable method.][219]

**Contributors**: @catenacyber, @cuviper

# Release 0.3.2 (2021-03-04)

- [The new `BigUint` methods `count_ones` and `trailing_ones`][175] return the
  number of `1` bits in the entire value or just its least-significant tail,
  respectively.
- [The new `BigInt` and `BigUint` methods `bit` and `set_bit`][183] will read
  and write individual bits of the value. For negative `BigInt`, bits are
  determined as if they were in the two's complement representation.
- [The `from_radix_le` and `from_radix_be` methods][187] now accept empty
  buffers to represent zero.
- [`BigInt` and `BigUint` can now iterate digits as `u32` or `u64`][192],
  regardless of the actual internal digit size.

**Contributors**: @BartMassey, @cuviper, @janmarthedal, @sebastianv89, @Speedy37

[175]: https://github.com/rust-num/num-bigint/pull/175
[183]: https://github.com/rust-num/num-bigint/pull/183
[187]: https://github.com/rust-num/num-bigint/pull/187
[192]: https://github.com/rust-num/num-bigint/pull/192

# Release 0.3.1 (2020-11-03)

- [Addition and subtraction now uses intrinsics][141] for performance on `x86`
  and `x86_64` when built with Rust 1.33 or later.
- [Conversions `to_f32` and `to_f64` now return infinity][163] for very large
  numbers, rather than `None`. This does preserve the sign too, so a large
  negative `BigInt` will convert to negative infinity.
- [The optional `arbitrary` feature implements `arbitrary::Arbitrary`][166],
  distinct from `quickcheck::Arbitrary`.
- [The division algorithm has been optimized][170] to reduce the number of
  temporary allocations and improve the internal guesses at each step.
- [`BigInt` and `BigUint` will opportunistically shrink capacity][171] if the
  internal vector is much larger than needed.

**Contributors**: @cuviper, @e00E, @ejmahler, @notoria, @tczajka

[141]: https://github.com/rust-num/num-bigint/pull/141
[163]: https://github.com/rust-num/num-bigint/pull/163
[166]: https://github.com/rust-num/num-bigint/pull/166
[170]: https://github.com/rust-num/num-bigint/pull/170
[171]: https://github.com/rust-num/num-bigint/pull/171

# Release 0.3.0 (2020-06-12)

### Enhancements

- [The internal `BigDigit` may now be either `u32` or `u64`][62], although that
  implementation detail is not exposed in the API. For now, this is chosen to
  match the target pointer size, but may change in the future.
- [No-`std` is now supported with the `alloc` crate on Rust 1.36][101].
- [`Pow` is now implemented for bigint values][137], not just references.
- [`TryFrom` is now implemented on Rust 1.34 and later][123], converting signed
  integers to unsigned, and narrowing big integers to primitives.
- [`Shl` and `Shr` are now implemented for a variety of shift types][142].
- A new `trailing_zeros()` returns the number of consecutive zeros from the
  least significant bit.
- The new `BigInt::magnitude` and `into_parts` methods give access to its
  `BigUint` part as the magnitude.

### Breaking Changes

- `num-bigint` now requires Rust 1.31 or greater.
  - The "i128" opt-in feature was removed, now always available.
- [Updated public dependences][110]:
  - `rand` support has been updated to 0.7, requiring Rust 1.32.
  - `quickcheck` support has been updated to 0.9, requiring Rust 1.34.
- [Removed `impl Neg for BigUint`][145], which only ever panicked.
- [Bit counts are now `u64` instead of `usize`][143].

**Contributors**: @cuviper, @dignifiedquire, @hansihe,
@kpcyrd, @milesand, @tech6hutch

[62]: https://github.com/rust-num/num-bigint/pull/62
[101]: https://github.com/rust-num/num-bigint/pull/101
[110]: https://github.com/rust-num/num-bigint/pull/110
[123]: https://github.com/rust-num/num-bigint/pull/123
[137]: https://github.com/rust-num/num-bigint/pull/137
[142]: https://github.com/rust-num/num-bigint/pull/142
[143]: https://github.com/rust-num/num-bigint/pull/143
[145]: https://github.com/rust-num/num-bigint/pull/145

# Release 0.2.6 (2020-01-27)

- [Fix the promotion of negative `isize` in `BigInt` assign-ops][133].

**Contributors**: @cuviper, @HactarCE

[133]: https://github.com/rust-num/num-bigint/pull/133

# Release 0.2.5 (2020-01-09)

- [Updated the `autocfg` build dependency to 1.0][126].

**Contributors**: @cuviper, @tspiteri

[126]: https://github.com/rust-num/num-bigint/pull/126

# Release 0.2.4 (2020-01-01)

- [The new `BigUint::to_u32_digits` method][104] returns the number as a
  little-endian vector of base-2<sup>32</sup> digits. The same method on
  `BigInt` also returns the sign.
- [`BigUint::modpow` now applies a modulus even for exponent 1][113], which
  also affects `BigInt::modpow`.
- [`BigInt::modpow` now returns the correct sign for negative bases with even
  exponents][114].

[104]: https://github.com/rust-num/num-bigint/pull/104
[113]: https://github.com/rust-num/num-bigint/pull/113
[114]: https://github.com/rust-num/num-bigint/pull/114

**Contributors**: @alex-ozdemir, @cuviper, @dingelish, @Speedy37, @youknowone

# Release 0.2.3 (2019-09-03)

- [`Pow` is now implemented for `BigUint` exponents][77].
- [The optional `quickcheck` feature enables implementations of `Arbitrary`][99].
- See the [full comparison][compare-0.2.3] for performance enhancements and more!

[77]: https://github.com/rust-num/num-bigint/pull/77
[99]: https://github.com/rust-num/num-bigint/pull/99
[compare-0.2.3]: https://github.com/rust-num/num-bigint/compare/num-bigint-0.2.2...num-bigint-0.2.3

**Contributors**: @cuviper, @lcnr, @maxbla, @mikelodder7, @mikong,
@TheLetterTheta, @tspiteri, @XAMPPRocky, @youknowone

# Release 0.2.2 (2018-12-14)

- [The `Roots` implementations now use better initial guesses][71].
- [Fixed `to_signed_bytes_*` for some positive numbers][72], where the
  most-significant byte is `0x80` and the rest are `0`.

[71]: https://github.com/rust-num/num-bigint/pull/71
[72]: https://github.com/rust-num/num-bigint/pull/72

**Contributors**: @cuviper, @leodasvacas

# Release 0.2.1 (2018-11-02)

- [`RandBigInt` now uses `Rng::fill_bytes`][53] to improve performance, instead
  of repeated `gen::<u32>` calls.  The also affects the implementations of the
  other `rand` traits.  This may potentially change the values produced by some
  seeded RNGs on previous versions, but the values were tested to be stable
  with `ChaChaRng`, `IsaacRng`, and `XorShiftRng`.
- [`BigInt` and `BigUint` now implement `num_integer::Roots`][56].
- [`BigInt` and `BigUint` now implement `num_traits::Pow`][54].
- [`BigInt` and `BigUint` now implement operators with 128-bit integers][64].

**Contributors**: @cuviper, @dignifiedquire, @mancabizjak, @Robbepop,
@TheIronBorn, @thomwiggers

[53]: https://github.com/rust-num/num-bigint/pull/53
[54]: https://github.com/rust-num/num-bigint/pull/54
[56]: https://github.com/rust-num/num-bigint/pull/56
[64]: https://github.com/rust-num/num-bigint/pull/64

# Release 0.2.0 (2018-05-25)

### Enhancements

- [`BigInt` and `BigUint` now implement `Product` and `Sum`][22] for iterators
  of any item that we can `Mul` and `Add`, respectively.  For example, a
  factorial can now be simply: `let f: BigUint = (1u32..1000).product();`
- [`BigInt` now supports two's-complement logic operations][26], namely
  `BitAnd`, `BitOr`, `BitXor`, and `Not`.  These act conceptually as if each
  number had an infinite prefix of `0` or `1` bits for positive or negative.
- [`BigInt` now supports assignment operators][41] like `AddAssign`.
- [`BigInt` and `BigUint` now support conversions with `i128` and `u128`][44],
  if sufficient compiler support is detected.
- [`BigInt` and `BigUint` now implement rand's `SampleUniform` trait][48], and
  [a custom `RandomBits` distribution samples by bit size][49].
- The release also includes other miscellaneous improvements to performance.

### Breaking Changes

- [`num-bigint` now requires rustc 1.15 or greater][23].
- [The crate now has a `std` feature, and won't build without it][46].  This is
  in preparation for someday supporting `#![no_std]` with `alloc`.
- [The `serde` dependency has been updated to 1.0][24], still disabled by
  default.  The `rustc-serialize` crate is no longer supported by `num-bigint`.
- [The `rand` dependency has been updated to 0.5][48], now disabled by default.
  This requires rustc 1.22 or greater for `rand`'s own requirement.
- [`Shr for BigInt` now rounds down][8] rather than toward zero, matching the
  behavior of the primitive integers for negative values.
- [`ParseBigIntError` is now an opaque type][37].
- [The `big_digit` module is no longer public][38], nor are the `BigDigit` and
  `DoubleBigDigit` types and `ZERO_BIG_DIGIT` constant that were re-exported in
  the crate root.  Public APIs which deal in digits, like `BigUint::from_slice`,
  will now always be base-`u32`.

**Contributors**: @clarcharr, @cuviper, @dodomorandi, @tiehuis, @tspiteri

[8]: https://github.com/rust-num/num-bigint/pull/8
[22]: https://github.com/rust-num/num-bigint/pull/22
[23]: https://github.com/rust-num/num-bigint/pull/23
[24]: https://github.com/rust-num/num-bigint/pull/24
[26]: https://github.com/rust-num/num-bigint/pull/26
[37]: https://github.com/rust-num/num-bigint/pull/37
[38]: https://github.com/rust-num/num-bigint/pull/38
[41]: https://github.com/rust-num/num-bigint/pull/41
[44]: https://github.com/rust-num/num-bigint/pull/44
[46]: https://github.com/rust-num/num-bigint/pull/46
[48]: https://github.com/rust-num/num-bigint/pull/48
[49]: https://github.com/rust-num/num-bigint/pull/49

# Release 0.1.44 (2018-05-14)

- [Division with single-digit divisors is now much faster.][42]
- The README now compares [`ramp`, `rug`, `rust-gmp`][20], and [`apint`][21].

**Contributors**: @cuviper, @Robbepop

[20]: https://github.com/rust-num/num-bigint/pull/20
[21]: https://github.com/rust-num/num-bigint/pull/21
[42]: https://github.com/rust-num/num-bigint/pull/42

# Release 0.1.43 (2018-02-08)

- [The new `BigInt::modpow`][18] performs signed modular exponentiation, using
  the existing `BigUint::modpow` and rounding negatives similar to `mod_floor`.

**Contributors**: @cuviper

[18]: https://github.com/rust-num/num-bigint/pull/18


# Release 0.1.42 (2018-02-07)

- [num-bigint now has its own source repository][num-356] at [rust-num/num-bigint][home].
- [`lcm` now avoids creating a large intermediate product][num-350].
- [`gcd` now uses Stein's algorithm][15] with faster shifts instead of division.
- [`rand` support is now extended to 0.4][11] (while still allowing 0.3).

**Contributors**: @cuviper, @Emerentius, @ignatenkobrain, @mhogrefe

[home]: https://github.com/rust-num/num-bigint
[num-350]: https://github.com/rust-num/num/pull/350
[num-356]: https://github.com/rust-num/num/pull/356
[11]: https://github.com/rust-num/num-bigint/pull/11
[15]: https://github.com/rust-num/num-bigint/pull/15


# Prior releases

No prior release notes were kept.  Thanks all the same to the many
contributors that have made this crate what it is!

