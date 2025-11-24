# Release 0.1.46 (2024-02-07)

- [Upgrade to 2018 edition, **MSRV 1.31**][51]
- [The `Integer::divides` method is now properly deprecated][42],
  rather than just documented so.
- [The new `Integer::dec` and `inc` methods change the value by one.][53]

**Contributors**: @aobatact, @cuviper, @hkBst, @MiguelX413

[42]: https://github.com/rust-num/num-integer/pull/42
[51]: https://github.com/rust-num/num-integer/pull/51
[53]: https://github.com/rust-num/num-integer/pull/53

# Release 0.1.45 (2022-04-29)

- [`Integer::next_multiple_of` and `prev_multiple_of` no longer overflow -1][45].
- [`Integer::is_multiple_of` now handles a 0 argument without panicking][47]
  for primitive integers.
- [`ExtendedGcd` no longer has any private fields][46], making it possible for
  external implementations to customize `Integer::extended_gcd`.

**Contributors**: @ciphergoth, @cuviper, @tspiteri, @WizardOfMenlo

[45]: https://github.com/rust-num/num-integer/pull/45
[46]: https://github.com/rust-num/num-integer/pull/46
[47]: https://github.com/rust-num/num-integer/pull/47

# Release 0.1.44 (2020-10-29)

- [The "i128" feature now bypasses compiler probing][35]. The build script
  used to probe anyway and panic if requested support wasn't found, but
  sometimes this ran into bad corner cases with `autocfg`.

**Contributors**: @cuviper

[35]: https://github.com/rust-num/num-integer/pull/35

# Release 0.1.43 (2020-06-11)

- [The new `Average` trait][31] computes fast integer averages, rounded up or
  down, without any risk of overflow.

**Contributors**: @althonos, @cuviper

[31]: https://github.com/rust-num/num-integer/pull/31

# Release 0.1.42 (2020-01-09)

- [Updated the `autocfg` build dependency to 1.0][29].

**Contributors**: @cuviper, @dingelish

[29]: https://github.com/rust-num/num-integer/pull/29

# Release 0.1.41 (2019-05-21)

- [Fixed feature detection on `no_std` targets][25].

**Contributors**: @cuviper

[25]: https://github.com/rust-num/num-integer/pull/25

# Release 0.1.40 (2019-05-20)

- [Optimized primitive `gcd` by avoiding memory swaps][11].
- [Fixed `lcm(0, 0)` to return `0`, rather than panicking][18].
- [Added `Integer::div_ceil`, `next_multiple_of`, and `prev_multiple_of`][16].
- [Added `Integer::gcd_lcm`, `extended_gcd`, and `extended_gcd_lcm`][19].

**Contributors**: @cuviper, @ignatenkobrain, @smarnach, @strake

[11]: https://github.com/rust-num/num-integer/pull/11
[16]: https://github.com/rust-num/num-integer/pull/16
[18]: https://github.com/rust-num/num-integer/pull/18
[19]: https://github.com/rust-num/num-integer/pull/19

# Release 0.1.39 (2018-06-20)

- [The new `Roots` trait provides `sqrt`, `cbrt`, and `nth_root` methods][9],
  calculating an `Integer`'s principal roots rounded toward zero.

**Contributors**: @cuviper

[9]: https://github.com/rust-num/num-integer/pull/9

# Release 0.1.38 (2018-05-11)

- [Support for 128-bit integers is now automatically detected and enabled.][8]
  Setting the `i128` crate feature now causes the build script to panic if such
  support is not detected.

**Contributors**: @cuviper

[8]: https://github.com/rust-num/num-integer/pull/8

# Release 0.1.37 (2018-05-10)

- [`Integer` is now implemented for `i128` and `u128`][7] starting with Rust
  1.26, enabled by the new `i128` crate feature.

**Contributors**: @cuviper

[7]: https://github.com/rust-num/num-integer/pull/7

# Release 0.1.36 (2018-02-06)

- [num-integer now has its own source repository][num-356] at [rust-num/num-integer][home].
- [Corrected the argument order documented in `Integer::is_multiple_of`][1]
- [There is now a `std` feature][5], enabled by default, along with the implication
  that building *without* this feature makes this a `#[no_std]` crate.
  - There is no difference in the API at this time.

**Contributors**: @cuviper, @jaystrictor

[home]: https://github.com/rust-num/num-integer
[num-356]: https://github.com/rust-num/num/pull/356
[1]: https://github.com/rust-num/num-integer/pull/1
[5]: https://github.com/rust-num/num-integer/pull/5


# Prior releases

No prior release notes were kept.  Thanks all the same to the many
contributors that have made this crate what it is!

