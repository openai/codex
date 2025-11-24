# icu_normalizer [![crates.io](https://img.shields.io/crates/v/icu_normalizer)](https://crates.io/crates/icu_normalizer)

<!-- cargo-rdme start -->

Normalizing text into Unicode Normalization Forms.

This module is published as its own crate ([`icu_normalizer`](https://docs.rs/icu_normalizer/latest/icu_normalizer/))
and as part of the [`icu`](https://docs.rs/icu/latest/icu/) crate. See the latter for more details on the ICU4X project.

## Functionality

The top level of the crate provides normalization of input into the four normalization forms defined in [UAX #15: Unicode
Normalization Forms](https://www.unicode.org/reports/tr15/): NFC, NFD, NFKC, and NFKD.

Three kinds of contiguous inputs are supported: known-well-formed UTF-8 (`&str`), potentially-not-well-formed UTF-8,
and potentially-not-well-formed UTF-16. Additionally, an iterator over `char` can be wrapped in a normalizing iterator.

The `uts46` module provides the combination of mapping and normalization operations for [UTS #46: Unicode IDNA
Compatibility Processing](https://www.unicode.org/reports/tr46/). This functionality is not meant to be used by
applications directly. Instead, it is meant as a building block for a full implementation of UTS #46, such as the
[`idna`](https://docs.rs/idna/latest/idna/) crate.

The `properties` module provides the non-recursive canonical decomposition operation on a per `char` basis and
the canonical compositon operation given two `char`s. It also provides access to the Canonical Combining Class
property. These operations are primarily meant for [HarfBuzz](https://harfbuzz.github.io/) via the
[`icu_harfbuzz`](https://docs.rs/icu_harfbuzz/latest/icu_harfbuzz/) crate.

Notably, this normalizer does _not_ provide the normalization “quick check” that can result in “maybe” in
addition to “yes” and “no”. The normalization checks provided by this crate always give a definitive
non-“maybe” answer.

## Examples

```rust
let nfc = icu_normalizer::ComposingNormalizerBorrowed::new_nfc();
assert_eq!(nfc.normalize("a\u{0308}"), "ä");
assert!(nfc.is_normalized("ä"));

let nfd = icu_normalizer::DecomposingNormalizerBorrowed::new_nfd();
assert_eq!(nfd.normalize("ä"), "a\u{0308}");
assert!(!nfd.is_normalized("ä"));
```

<!-- cargo-rdme end -->

## More Information

For more information on development, authorship, contributing etc. please visit [`ICU4X home page`](https://github.com/unicode-org/icu4x).
