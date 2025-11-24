# 2.1.1

- Change the internal algorithm to better accomodate large hashmaps.
  This mitigates a [regression with 2.0 in rustc](https://github.com/rust-lang/rust/issues/135477).
  See [PR#55](https://github.com/rust-lang/rustc-hash/pull/55) for more details on the change (this PR was not merged).
  This problem might be improved with changes to hashbrown in the future.

## 2.1.0

- Implement `Clone` for `FxRandomState`
- Implement `Clone` for `FxSeededState`
- Use SPDX license expression in license field

## 2.0.0

- Replace hash with faster and better finalized hash.
  This replaces the previous "fxhash" algorithm originating in Firefox
  with a custom hasher designed and implemented by Orson Peters ([`@orlp`](https://github.com/orlp)).
  It was measured to have slightly better performance for rustc, has better theoretical properties
  and also includes a significantly better string hasher.
- Fix `no_std` builds

## 1.2.0 (**YANKED**)

**Note: This version has been yanked due to issues with the `no_std` feature!**

- Add a `FxBuildHasher` unit struct
- Improve documentation
- Add seed API for supplying custom seeds other than 0
- Add `FxRandomState` based on `rand` (behind the `rand` feature) for random seeds
- Make many functions `const fn`
- Implement `Clone` for `FxHasher` struct
