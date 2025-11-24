# Changelog

This project follows semantic versioning.

The MSRV (Minimum Supported Rust Version) is 1.37.0, and typenum is tested
against this Rust version.

### Unreleased

### 1.18.0 (2025-02-17)
- [changed] Remove build scripts; instead check-in the built code (PR #219)
- [added] Constants for 3600 (PR #220)
- [added] Elixir-style syntax for `tarr!` macro (PR #214)
- [added] `FoldAdd` and `FoldMul` to get the sum/product of an array (PR #209)

### 1.17.0 (2023-09-15)
- [removed] Remove `force_unix_path_separator` feature, make it the default
- [added] docs.rs metadata and cfg options
- [added] Playground metadata

### 1.16.0 (2022-12-05)
- [added] `const INT` field to the `ToInt` trait.
- [added] `const-generics` field with `U<N>` mapping where `N` is a const generic.

### 1.15.0 (2021-12-25)
- [fixed] Cross-compilation issue due to doing math in build script. (PR #177)
- [added] New feature `scale_info` for using inside
  [Substrate](https://github.com/paritytech/substrate.git)-based runtimes (PR
  #175)

### 1.14.0 (2021-09-01)
- [changed] Sealed all marker traits. Documentation already stated that these
  should not be implemented outside the crate, so this is not considered a
  breaking change.

### 1.13.0 (2021-03-12)
- [changed] MSRV from 1.22.0 to 1.37.0.
- [fixed] `op` macro with 2018 edition import.
- [changed] Allowed calling `assert_type_eq` and `assert_type` at top level.
- [added] Marker trait `Zero` for `Z0`, `U0`, and `B0`.
- [added] Implementation of `Pow` trait for f32 and f64 with negative exponent.
- [added] Trait `ToInt`.

### 1.12.0 (2020-04-13)
- [added] Feature `force_unix_path_separator` to support building without Cargo.
- [added] Greatest common divisor operator `Gcd` with alias `Gcf`.
- [added] `gcd` to the `op!` macro.
- [changed] Added `Copy` bound to `Rhs` of `Mul<Rhs>` impl for `<TArr<V, A>`.
- [changed] Added `Copy` bound to `Rhs` of `Div<Rhs>` impl for `<TArr<V, A>`.
- [changed] Added `Copy` bound to `Rhs` of `PartialDiv<Rhs>` impl for `<TArr<V, A>`.
- [changed] Added `Copy` bound to `Rhs` of `Rem<Rhs>` impl for `<TArr<V, A>`.
- [fixed] Make all functions #[inline].

### 1.11.2 (2019-08-26)
- [fixed] Cross compilation from Linux to Windows.

### 1.11.1 (2019-08-25)
- [fixed] Builds on earlier Rust builds again and added Rust 1.22.0 to Travis to
  prevent future breakage.

### 1.11.0 (2019-08-25)
- [added] Integer `log2` to the `op!` macro.
- [added] Integer binary logarithm operator `Logarithm2` with alias `Log2`.
- [changed] Removed `feature(i128_type)` when running with the `i128`
  feature. Kept the feature flag.  for typenum to maintain compatibility with
  old Rust versions.
- [added] Integer `sqrt` to the `op!` macro.
- [added] Integer square root operator `SquareRoot` with alias `Sqrt`.
- [fixed] Bug with attempting to create U1024 type alias twice.

### 1.10.0 (2018-03-11)
- [added] The `PowerOfTwo` marker trait.
- [added] Associated constants for `Bit`, `Unsigned`, and `Integer`.

### 1.9.0 (2017-05-14)
- [added] The `Abs` type operator and corresponding `AbsVal` alias.
- [added] The feature `i128` that enables creating 128-bit integers from
  typenums.
- [added] The `assert_type!` and `assert_type_eq!` macros.
- [added] Operators to the `op!` macro, including those performed by `cmp!`.
- [fixed] Bug in `op!` macro involving functions and convoluted expressions.
- [deprecated] The `cmp!` macro.

### 1.8.0 (2017-04-12)
- [added] The `op!` macro for conveniently performing type-level operations.
- [added] The `cmp!` macro for conveniently performing type-level comparisons.
- [added] Some comparison type-operators that are used by the `cmp!` macro.

### 1.7.0 (2017-03-24)
- [added] Type operators `Min` and `Max` with accompanying aliases `Minimum` and
  `Maximum`

### 1.6.0 (2017-02-24)
- [fixed] Bug in `Array` division.
- [fixed] Bug where `Rem` would sometimes exit early with the wrong answer.
- [added] `PartialDiv` operator that performs division as a partial function --
  it's defined only when there is no remainder.

### 1.5.2 (2017-02-04)
- [fixed] Bug between `Div` implementation and type system.

### 1.5.1 (2016-11-08)
- [fixed] Expanded implementation of `Pow` for primitives.

### 1.5.0 (2016-11-03)
- [added] Functions to the `Pow` and `Len` traits. This is *technically* a
  breaking change, but it would only break someone's code if they have a custom
  impl for `Pow`. I would be very surprised if that is anyone other than me.

### 1.4.0 (2016-10-29)
- [added] Type-level arrays of type-level integers. (PR #66)
- [added] The types in this crate are now instantiable. (Issue #67, PR #68)

### 1.3.1 (2016-03-31)
- [fixed] Bug with recent nightlies.

### 1.3.0 (2016-02-07)
- [changed] Removed dependency on libstd. (Issue #53, PR #55)
- [changed] Reorganized module structure. (PR #57)

### 1.2.0 (2016-01-03)
- [added] This change log!
- [added] Convenience type aliases for operators. (Issue #48, PR #50)
- [added] Types in this crate now derive all possible traits. (Issue #42, PR
  #51)
