# icu_collections [![crates.io](https://img.shields.io/crates/v/icu_collections)](https://crates.io/crates/icu_collections)

<!-- cargo-rdme start -->

Efficient collections for Unicode data.

This module is published as its own crate ([`icu_collections`](https://docs.rs/icu_collections/latest/icu_collections/))
and as part of the [`icu`](https://docs.rs/icu/latest/icu/) crate. See the latter for more details on the ICU4X project.

ICU4X `CodePointTrie` provides a read-only view of `CodePointTrie` data that is exported
from ICU4C. Detailed information about the design of the data structure can be found in the documentation
for the `CodePointTrie` struct.

ICU4X `CodePointInversionList` provides necessary functionality for highly efficient querying of sets of Unicode characters.
It is an implementation of the existing [ICU4C UnicodeSet API](https://unicode-org.github.io/icu-docs/apidoc/released/icu4c/classicu_1_1UnicodeSet.html).

ICU4X `Char16Trie` provides a data structure for a space-efficient and time-efficient lookup of
sequences of 16-bit units (commonly but not necessarily UTF-16 code units)
which map to integer values.
It is an implementation of the existing [ICU4C UCharsTrie](https://unicode-org.github.io/icu-docs/apidoc/released/icu4c/classicu_1_1UCharsTrie.html)
/ [ICU4J CharsTrie](https://unicode-org.github.io/icu-docs/apidoc/released/icu4j/com/ibm/icu/util/CharsTrie.html) API.

<!-- cargo-rdme end -->

## More Information

For more information on development, authorship, contributing etc. please visit [`ICU4X home page`](https://github.com/unicode-org/icu4x).
