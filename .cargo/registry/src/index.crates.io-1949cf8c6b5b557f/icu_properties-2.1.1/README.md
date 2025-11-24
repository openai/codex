# icu_properties [![crates.io](https://img.shields.io/crates/v/icu_properties)](https://crates.io/crates/icu_properties)

<!-- cargo-rdme start -->

Definitions of [Unicode Properties] and APIs for
retrieving property data in an appropriate data structure.

This module is published as its own crate ([`icu_properties`](https://docs.rs/icu_properties/latest/icu_properties/))
and as part of the [`icu`](https://docs.rs/icu/latest/icu/) crate. See the latter for more details on the ICU4X project.

APIs that return a `CodePointSetData` exist for binary properties and certain enumerated
properties.

APIs that return a `CodePointMapData` exist for certain enumerated properties.

## Examples

### Property data as `CodePointSetData`s

```rust
use icu::properties::{CodePointSetData, CodePointMapData};
use icu::properties::props::{GeneralCategory, Emoji};

// A binary property as a `CodePointSetData`

assert!(CodePointSetData::new::<Emoji>().contains('🎃')); // U+1F383 JACK-O-LANTERN
assert!(!CodePointSetData::new::<Emoji>().contains('木')); // U+6728

// An individual enumerated property value as a `CodePointSetData`

let line_sep_data = CodePointMapData::<GeneralCategory>::new()
    .get_set_for_value(GeneralCategory::LineSeparator);
let line_sep = line_sep_data.as_borrowed();

assert!(line_sep.contains('\u{2028}'));
assert!(!line_sep.contains('\u{2029}'));
```

### Property data as `CodePointMapData`s

```rust
use icu::properties::CodePointMapData;
use icu::properties::props::Script;

assert_eq!(CodePointMapData::<Script>::new().get('🎃'), Script::Common); // U+1F383 JACK-O-LANTERN
assert_eq!(CodePointMapData::<Script>::new().get('木'), Script::Han); // U+6728
```

[`ICU4X`]: ../icu/index.html
[Unicode Properties]: https://unicode-org.github.io/icu/userguide/strings/properties.html

<!-- cargo-rdme end -->

## More Information

For more information on development, authorship, contributing etc. please visit [`ICU4X home page`](https://github.com/unicode-org/icu4x).
