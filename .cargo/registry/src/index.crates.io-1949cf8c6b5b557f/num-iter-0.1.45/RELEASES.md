# Release 0.1.45 (2024-05-03)

- [Use `Integer::dec` in `DoubleEndedIterator`][29]

**Contributors**: @cuviper

[29]: https://github.com/rust-num/num-iter/pull/29

# Release 0.1.44 (2024-02-07)

- [Upgrade to 2018 edition, **MSRV 1.31**][22]

**Contributors**: @cuviper

[22]: https://github.com/rust-num/num-iter/pull/22

# Release 0.1.43 (2022-04-26)

- [`Range`, `RangeInclusive`, and `RangeFrom` now implement `RangeBounds`][21]
  from Rust 1.28 and later.

**Contributors**: @chrismit3s, @cuviper

[21]: https://github.com/rust-num/num-iter/pull/21

# Release 0.1.42 (2020-10-29)

- [The "i128" feature now bypasses compiler probing][20]. The build script
  used to probe anyway and panic if requested support wasn't found, but
  sometimes this ran into bad corner cases with `autocfg`.

**Contributors**: @cuviper

[20]: https://github.com/rust-num/num-iter/pull/20

# Release 0.1.41 (2020-06-11)

- [The new `RangeFrom` and `RangeFromStep` iterators][18] will count from a
  given starting value, without any terminating value.

**Contributors**: @cuviper, @sollyucko

[18]: https://github.com/rust-num/num-iter/pull/18

# Release 0.1.40 (2020-01-09)

- [Updated the `autocfg` build dependency to 1.0][14].

**Contributors**: @cuviper, @dingelish

[14]: https://github.com/rust-num/num-iter/pull/14

# Release 0.1.39 (2019-05-21)

- [Fixed feature detection on `no_std` targets][11].

**Contributors**: @cuviper

[11]: https://github.com/rust-num/num-iter/pull/11

# Release 0.1.38 (2019-05-20)

- Maintenance update -- no functional changes.

**Contributors**: @cuviper, @ignatenkobrain

# Release 0.1.37 (2018-05-11)

- [Support for 128-bit integers is now automatically detected and enabled.][5]
  Setting the `i128` crate feature now causes the build script to panic if such
  support is not detected.

**Contributors**: @cuviper

[5]: https://github.com/rust-num/num-iter/pull/5

# Release 0.1.36 (2018-05-10)

- [The iterators are now implemented for `i128` and `u128`][7] starting with
  Rust 1.26, enabled by the new `i128` crate feature.

**Contributors**: @cuviper

[4]: https://github.com/rust-num/num-iter/pull/4

# Release 0.1.35 (2018-02-06)

- [num-iter now has its own source repository][num-356] at [rust-num/num-iter][home].
- [There is now a `std` feature][2], enabled by default, along with the implication
  that building *without* this feature makes this a `#[no_std]` crate.
  - There is no difference in the API at this time.

**Contributors**: @cuviper

[home]: https://github.com/rust-num/num-iter
[num-356]: https://github.com/rust-num/num/pull/356
[2]: https://github.com/rust-num/num-iter/pull/2


# Prior releases

No prior release notes were kept.  Thanks all the same to the many
contributors that have made this crate what it is!

