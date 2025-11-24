# unreleased

# 0.5.0

- Add `no_std` support.
- Remove non-additive `unicode` feature. The library now uses `char::is_alphanumeric`
  instead of the `unicode-segmentation` library to determine word boundaries in all cases.

# 0.4.1

Improvements:

- Add Train-Case support

# 0.4.0

Breaking changes:

* Make unicode support optional (off by default). Enable the `unicode` crate
  feature if you need unicode support.
* Rename all traits from `SomeCase` to `ToSomeCase`, matching `std`s convention
  of beginning trait names with a verb (`ToOwned`, `AsRef`, …)
* Rename `ToMixedCase` to `ToLowerCamelCase`
* Rename `ToCamelCase` to `ToUpperCamelCase`
* Add `ToPascalCase` as an alias to `ToUpperCamelCase`
