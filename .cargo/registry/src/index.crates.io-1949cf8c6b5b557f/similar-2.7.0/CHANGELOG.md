# Changelog

All notable changes to similar are documented here.

## 2.7.0

* Add optional support for `web-time` to support web WASM targets.  #73
* Crate will no longer panic wheh deadlines are used in WASM.  At worst
  deadlines are silently ignored.  To enforce deadlines enable the
  `wasm32_web_time` feature.  #74

## 2.6.0

* Bump bstr dependency to 1.5.  #69

## 2.5.0

* Added support for `TextDiff::iter_inline_changes_deadline`.  #61
* Raise MSRV to 1.60.  #62
* Bump bstr dependency to 1.0.  #62

## 2.4.0

* Fixed a bug where the LCS diff algorithm didn't always call `D::finish`.  (#58)
* Fixed a bug in LCS that caused a panic if the common prefix and the
  common suffix overlapped.  (#59)

## 2.3.0

* Added support for `Change::value_ref` and `Change::value_mut`.

## 2.2.1

* Fixed a panic in LCS diffs on matching input.  (#43)

## 2.2.0

* Fixed a panic in text diff generation. (#37)

## 2.1.0

* Removed deprecated alternative slice diffing functions.
* Added `serde` feature to allow serialization with serde.

## 2.0.0

* Change the `Change` type and associated methods to work on any `T: Clone` instead
  of `&T`.  This makes the `iter_changes` method also work on slices of integers
  or other values.

## 1.3.0

* Performance improvements for the LCS algorithm.
* Small performance improvements by adding an early opt-out for and inline highlighting.
* Added `IdentifyDistinct` to convert sequences to ints.
* Small performance improvements for larger text diffs by using `IdentifyDistinct`
  automatically above a threshold.
* Added deadlines to all diffing algorithms to bail early.
* Deprecated slice diffing methods in the individual algorithm modules.
* Use a default timeout for the inline highlighting feature.
* Added a compacting step to clean up diffs.  This results in nicer looking diffs and
  fewer edits.  This is happening automatically for captured diffs and is exposed
  through the `Capture` type.
* Fix incorrect ranges in unified diff output.

## 1.2.2

* Added support for Rust 1.41.0 for better compatibility.

## 1.2.1

* Added support for Rust 1.43.0 for better compatibility.

## 1.2.0

* Make the unicode feature optional for inline diffing.
* Added Hunt–McIlroy LCS algorithm (`lcs`).
* Changed the implementation of Mayer's diff.  This has slightly changed the
  behavior but resulted in significantly improved performance and more
  readable code.
* Added `NoFinishHook` to aid composing of diff hooks.

## 1.1.0

* More generic lifetimes for `iter_changes` and `iter_inline_changes`.
* Added `iter_all_changes` shortcut as this is commonly useful.
* Added `iter_slices` to `DiffOp` to quickly get an iterator over the
  encoded slices rather than individual items like `iter_changes` does.
* Added the `utils` module with various text diffing utilities.
* Added `TextDiffRemapper` which helps with working with the original, pre
  `TextDiff` tokenization slices.

## 1.0.0

* Add `get_diff_ratio`.
* Add support for byte diffing and change the text interface to abstract
  over `DiffableStr`.
* Restructured crate layout greatly.  Text diffing is now on the crate root,
  some functionality remains in the algorithms.
* The `Change` type now also works for non text diffs.

## 0.5.0

* Add `DiffOp::apply_to_hook` to apply a captured op to a diff hook.
* Added missing newline handling to the `Changes` type.
* Made unified diff support more flexible through the introduction of
  the `UnifiedDiff` type.
* Fixed grouped diff operation to return an empty result if the diff
  does not show any changes.
* Added inline diff highlighting support.
* Changed word splitting to split into words and whitespace.
* Added support for unicode based word splitting (`TextDiff::from_unicode_words`).

## 0.4.0

* Change `get_close_matches` to use Python's quick ratio optimization
  and order lexicographically when tied.

## 0.3.0

* Added grapheme and character level diffing utilities.
* `DiffOp::as_tag_tuple` is now taking the argument by reference.
* Added `TextDiff::ratio`.
* Added `get_close_matches`.

## 0.2.0

* Fixed a bug in the patience algorithm causing it not not work.

## 0.1.0

* Initial release.
